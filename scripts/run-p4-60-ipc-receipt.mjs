import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const appBinary = join(repository, 'target', 'debug', 'arcana.exe');
const frontendOnly = process.argv.includes('--frontend-only');
const fixtureRoot = await mkdtemp(join(tmpdir(), 'arcana-p4-60-receipt-'));
const profile = join(fixtureRoot, 'profile');
const workspace = join(fixtureRoot, 'workspace');
const emptyViteEnv = join(fixtureRoot, 'empty-vite-env');
const marker = '.arcana-e2e-ipc-receipt';
const receiptPath = join(profile, '.arcana', 'p4-60-ipc-receipt.json');
const failurePath = join(profile, '.arcana', 'p4-60-ipc-failure.json');
let app;
let cleanupError;
let cleanupConfirmed = false;
const ownedProcessIds = new Map();

async function cleanEnvironment() {
  const env = { ...process.env };
  const developerProfile = process.env.USERPROFILE ?? process.env.HOME;
  for (const name of Object.keys(env)) {
    if (/API_KEY|ACCESS_TOKEN|TOKEN|SECRET|DATABASE_URL|GEMINI|GOOGLE|OPENAI|ANTHROPIC|VAULT/i.test(name)) {
      delete env[name];
    }
  }
  for (const name of ['ARCANA_LIVE_SMOKE', 'ARCANA_LIVE_PROVIDER_SMOKE', 'ARCANA_PROVIDER_SMOKE']) delete env[name];
  env.ARCANA_E2E_IPC_RECEIPT = '1';
  env.ARCANA_E2E_FIXTURE_ROOT = fixtureRoot;
  env.ARCANA_E2E_WORKSPACE = workspace;
  env.ARCANA_E2E_VITE_ENV_DIR = emptyViteEnv;
  env.npm_package_version = JSON.parse(await readFile(join(repository, 'package.json'), 'utf8')).version;
  if (developerProfile) {
    env.RUSTUP_HOME ??= join(developerProfile, '.rustup');
    env.CARGO_HOME ??= join(developerProfile, '.cargo');
  }
  delete env.TAURI_DEV_HOST;
  return env;
}

function isolatedAppEnvironment(buildEnv) {
  const env = { ...buildEnv };
  // The fixture's readable INFO producer must not be suppressed by an
  // unrelated developer RUST_LOG setting (this host uses `warn`).
  env.RUST_LOG = 'arcana_lib=debug,info';
  env.USERPROFILE = profile;
  env.HOME = profile;
  env.APPDATA = join(profile, 'AppData', 'Roaming');
  env.LOCALAPPDATA = join(profile, 'AppData', 'Local');
  env.XDG_CONFIG_HOME = join(profile, '.config');
  env.XDG_DATA_HOME = join(profile, '.local', 'share');
  env.WEBVIEW2_USER_DATA_FOLDER = join(profile, 'WebView2');
  delete env.RUSTUP_HOME;
  delete env.CARGO_HOME;
  return env;
}

function runBuild(command, args, env) {
  const timeoutMs = 10 * 60 * 1000;
  return new Promise((resolvePromise, rejectPromise) => {
    cleanupConfirmed = false;
    const child = spawn(command, args, {
      cwd: repository,
      env,
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const stdout = [];
    const stderr = [];
    let outputBytes = 0;
    let outputLimitError;
    const collect = (target, chunk) => {
      outputBytes += chunk.length;
      if (outputBytes > 16 * 1024 * 1024) {
        outputLimitError ??= new Error(`${command} exceeded its 16 MiB output bound.`);
        return;
      }
      target.push(chunk);
    };
    child.stdout.on('data', chunk => collect(stdout, chunk));
    child.stderr.on('data', chunk => collect(stderr, chunk));
    const treeWatch = setInterval(() => {
      try { captureOwnedDescendants(child.pid); } catch (error) { cleanupError = error; }
    }, 500);

    void (async () => {
      let result;
      let runError;
      try {
        captureOwnedDescendants(child.pid);
        result = await waitForExit(child, timeoutMs);
        if (outputLimitError) throw outputLimitError;
        if (result.code !== 0) {
          throw new Error(`${command} ${args.join(' ')} failed (${result.code ?? result.signal}).\n${Buffer.concat(stdout).toString()}\n${Buffer.concat(stderr).toString()}`);
        }
      } catch (error) {
        runError = error;
        if (child.pid && child.exitCode === null) {
          try { await stopProcessTree(child); } catch (cleanup) { cleanupError = cleanup; }
        }
      } finally {
        clearInterval(treeWatch);
        try {
          captureOwnedDescendants(child.pid);
          stopAndVerifyOwnedDescendants();
        } catch (error) {
          cleanupError = error;
        }
      }
      if (runError || cleanupError) {
        const detail = [runError && String(runError), cleanupError && `Process cleanup failed: ${String(cleanupError)}`]
          .filter(Boolean)
          .join('\n');
        rejectPromise(new Error(detail));
      } else {
        resolvePromise();
      }
    })();
  });
}

function waitForExit(child, timeoutMs) {
  return new Promise((resolvePromise, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Process ${child.pid} exceeded the ${timeoutMs} ms deadline.`)), timeoutMs);
    child.once('error', error => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once('exit', (code, signal) => {
      clearTimeout(timeout);
      resolvePromise({ code, signal });
    });
  });
}

function snapshotOwnedDescendants(rootPid) {
  const script = `$all = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CreationDate); $owned = [System.Collections.Generic.HashSet[int]]::new(); [void]$owned.Add(${rootPid}); do { $changed = $false; foreach ($process in $all) { if ($owned.Contains([int]$process.ParentProcessId) -and $owned.Add([int]$process.ProcessId)) { $changed = $true } } } while ($changed); $descendants = @($all | Where-Object { $owned.Contains([int]$_.ProcessId) -and [int]$_.ProcessId -ne ${rootPid} } | ForEach-Object { [PSCustomObject]@{ pid = [int]$_.ProcessId; created = [string]$_.CreationDate } }); if ($descendants.Count -eq 0) { '[]' } else { ConvertTo-Json -InputObject $descendants -Compress }`;
  const result = spawnSync('powershell.exe', ['-NoProfile', '-Command', script], {
    encoding: 'utf8',
    timeout: 10_000,
    maxBuffer: 2 * 1024 * 1024,
  });
  if (result.status !== 0) throw new Error(`Could not inspect the owned Windows process tree: ${result.stderr}`);
  const value = JSON.parse(result.stdout.trim());
  return Array.isArray(value) ? value : value ? [value] : [];
}

function captureOwnedDescendants(rootPid) {
  for (const process of snapshotOwnedDescendants(rootPid)) {
    ownedProcessIds.set(Number(process.pid), String(process.created));
  }
}

function currentOwnedProcesses() {
  const requested = [...ownedProcessIds.entries()];
  if (requested.length === 0) return [];
  const ids = requested.map(([pid]) => pid).join(',');
  const script = `$ids = @(${ids}); $current = @(Get-CimInstance Win32_Process | Where-Object { $ids -contains [int]$_.ProcessId } | ForEach-Object { [PSCustomObject]@{ pid = [int]$_.ProcessId; created = [string]$_.CreationDate } }); if ($current.Count -eq 0) { '[]' } else { ConvertTo-Json -InputObject $current -Compress }`;
  const result = spawnSync('powershell.exe', ['-NoProfile', '-Command', script], {
    encoding: 'utf8',
    timeout: 10_000,
    maxBuffer: 2 * 1024 * 1024,
  });
  if (result.status !== 0) throw new Error(`Could not verify owned Windows processes: ${result.stderr}`);
  const value = JSON.parse(result.stdout.trim());
  return Array.isArray(value) ? value : value ? [value] : [];
}

function stopAndVerifyOwnedDescendants() {
  const originallyOwned = new Map(ownedProcessIds);
  const current = currentOwnedProcesses();
  const survivors = current.filter(process => originallyOwned.get(Number(process.pid)) === String(process.created));
  for (const process of survivors) {
    const result = spawnSync('taskkill.exe', ['/PID', String(process.pid), '/T', '/F'], {
      encoding: 'utf8',
      timeout: 10_000,
      stdio: 'ignore',
    });
    if (result.error?.code === 'ETIMEDOUT') throw new Error(`Timed out stopping owned child PID ${process.pid}.`);
  }
  const remaining = currentOwnedProcesses().filter(process => originallyOwned.get(Number(process.pid)) === String(process.created));
  if (remaining.length > 0) throw new Error(`Owned process cleanup remains unconfirmed for PID ${remaining.map(row => row.pid).join(', ')}.`);
  cleanupConfirmed = true;
}

async function stopProcessTree(child) {
  if (child?.pid && child.exitCode === null) {
    const result = spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {
      encoding: 'utf8',
      timeout: 10_000,
      stdio: 'ignore',
    });
    if (result.error?.code === 'ETIMEDOUT') throw new Error(`Timed out stopping Arcana PID ${child.pid}.`);
    await Promise.race([
      new Promise(resolvePromise => child.once('exit', resolvePromise)),
      new Promise(resolvePromise => setTimeout(resolvePromise, 10_000)),
    ]);
    if (child.exitCode === null) throw new Error(`Could not confirm cleanup of Arcana PID ${child.pid}.`);
  }
  if (child?.pid) {
    captureOwnedDescendants(child.pid);
    stopAndVerifyOwnedDescendants();
  }
}

try {
  await mkdir(profile, { recursive: true });
  await mkdir(workspace, { recursive: true });
  await mkdir(emptyViteEnv, { recursive: true });
  await Promise.all(['AppData/Roaming', 'AppData/Local', '.config', '.local/share', 'WebView2']
    .map(path => mkdir(join(profile, path), { recursive: true })));
  await writeFile(join(fixtureRoot, marker), 'arcana-p4-60-ipc-receipt-v1\n', { flag: 'wx' });
  const env = await cleanEnvironment();
  if ((await readdir(emptyViteEnv)).length !== 0) throw new Error('The isolated Vite env fixture is not empty.');

  await runBuild(process.execPath, ['node_modules/typescript/bin/tsc', '--noEmit'], env);
  await runBuild(process.execPath, ['node_modules/vite/bin/vite.js', 'build', '--config', 'e2e/p4-60-vite.config.ts'], env);
  if (frontendOnly) {
    console.log(JSON.stringify({ result: 'no-env-frontend-build-passed', envDirectoryEmpty: true }));
  } else {
    await runBuild('cargo.exe', ['build', '-p', 'arcana', '--bin', 'arcana', '--features', 'e2e-ipc-receipt', '--offline'], env);

    const appEnv = isolatedAppEnvironment(env);
    cleanupConfirmed = false;
    app = spawn(appBinary, [], {
      cwd: repository,
      env: appEnv,
      windowsHide: true,
      stdio: 'ignore',
    });
    const treeWatch = setInterval(() => {
      try { captureOwnedDescendants(app.pid); } catch (error) { cleanupError = error; }
    }, 1_000);
    let result;
    try {
      captureOwnedDescendants(app.pid);
      result = await waitForExit(app, 120_000);
    } finally {
      clearInterval(treeWatch);
      captureOwnedDescendants(app.pid);
    }
    let failureStage;
    let failureReason;
    try {
      const failure = JSON.parse(await readFile(failurePath, 'utf8'));
      if (typeof failure.stage === 'string' && /^[a-z_]{1,40}$/.test(failure.stage)) failureStage = failure.stage;
      if (typeof failure.reason === 'string' && /^[a-z_]{1,40}$/.test(failure.reason)) failureReason = failure.reason;
    } catch {}
    if (failureStage || result.code !== 0) {
      throw new Error(`Arcana exited with code ${result.code}, signal ${result.signal ?? 'none'}, stage: ${failureStage ?? 'no renderer failure receipt'}, reason: ${failureReason ?? 'unavailable'}.`);
    }
    stopAndVerifyOwnedDescendants();
    if (cleanupError) throw new Error(`Windows process-tree capture failed: ${String(cleanupError)}`);

    const receiptText = await readFile(receiptPath, 'utf8');
    const receipt = JSON.parse(receiptText);
    if (receipt.schema !== 'arcana-p4-60-ipc-receipt-v1'
        || receipt.native_export_verified !== true
        || receipt.private_canary_absent !== true
        || Object.values(receipt.renderer).some(value => value !== true)) {
      throw new Error('The native receipt is incomplete or failed export/privacy verification.');
    }
    console.log(JSON.stringify({
      result: 'passed',
      schema: receipt.schema,
      rendererClaims: Object.keys(receipt.renderer).length,
      nativeExportVerified: receipt.native_export_verified,
      exportBytes: receipt.export_bytes,
      exportEventCount: receipt.export_event_count,
      privateCanaryAbsent: receipt.private_canary_absent,
      appExitCode: result.code,
      fixtureCleaned: true,
    }));
  }
} catch (error) {
  if (app) {
    try { await stopProcessTree(app); } catch (cleanup) { cleanupError = cleanup; }
  }
  try {
    const logDirectory = join(profile, '.arcana', 'logs');
    const summaries = await Promise.all((await readdir(logDirectory))
      .filter(name => /^arcana(?:-events)?\.\d{4}-\d{2}-\d{2}\.(?:log|jsonl)$/.test(name))
      .map(async name => {
        const path = join(logDirectory, name);
        const bytes = (await stat(path)).size;
        const markerVisible = (await readFile(path, 'utf8')).includes('P4-60 IPC receipt readable producer');
        return { name, bytes, markerVisible };
      }));
    console.error(`Isolated log diagnostics: ${JSON.stringify(summaries)}`);
  } catch {
    console.error('Isolated log diagnostics unavailable.');
  }
  console.error(String(error));
  if (cleanupError) console.error(`Process cleanup failed: ${String(cleanupError)}`);
  process.exitCode = 1;
} finally {
  if (cleanupConfirmed) {
    await rm(fixtureRoot, { recursive: true, force: true });
    try {
      await import('node:fs/promises').then(({ access }) => access(fixtureRoot));
      console.error(`Fixture cleanup could not be verified: ${fixtureRoot}`);
      process.exitCode = 1;
    } catch {}
  } else {
    console.error(`Fixture preserved because process cleanup is unconfirmed: ${fixtureRoot}`);
    process.exitCode = 1;
  }
}
