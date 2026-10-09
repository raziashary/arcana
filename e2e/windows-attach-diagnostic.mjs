import assert from 'node:assert/strict'
import { createServer } from 'node:net'
import { spawn, spawnSync } from 'node:child_process'
import { createWriteStream, mkdtempSync, mkdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { finished } from 'node:stream/promises'
import { fileURLToPath } from 'node:url'
import { setTimeout as delay } from 'node:timers/promises'
import { remote } from 'webdriverio'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const testBuildMode = process.argv.includes('--isolated-test-build')
const fixedRuntimeMode = process.argv.includes('--fixed-runtime-152')
const ephemeralDebugPortMode = process.argv.includes('--ephemeral-debug-port')
const minimalHostMode = process.argv.includes('--minimal-host')
const fixedRuntimeRoot = path.join(repoRoot, 'target', 'e2e-tools', 'webview2-152')
const fixedRuntimePath = path.join(fixedRuntimeRoot, 'runtime', 'Microsoft.WebView2.FixedVersionRuntime.152.0.4191.62.x64')
const appRelativePath = minimalHostMode
  ? path.join('target', 'p4-60-isolated', 'debug', 'examples', 'webview_attach_probe.exe')
  : testBuildMode
  ? path.join('target', 'p4-60-isolated', 'debug', 'arcana.exe')
  : path.join('target', 'debug', 'arcana.exe')
const appPath = path.join(repoRoot, appRelativePath)
const edgeDriverPath = fixedRuntimeMode
  ? path.join(fixedRuntimeRoot, 'driver', 'msedgedriver.exe')
  : path.join(repoRoot, 'target', 'e2e-tools', 'bin', 'msedgedriver.exe')
const isWindows = process.platform === 'win32'
const debugWaitMs = 30_000
const driverWaitMs = 15_000
const sessionWaitMs = 90_000
const domWaitMs = 30_000

let fixtureRoot
let appProcess
let driverProcess
let browser
let receivedSignal
let resolveSignal
const logErrors = []
const signalHandlers = new Map()
const logStreams = []
const signalReceived = new Promise(resolve => { resolveSignal = resolve })

function sanitizeLog(value) {
  return value
    .replace(/\bBearer\s+[^\s"']+/gi, 'Bearer <redacted>')
    .replace(/\b([\w.-]*(?:api[_-]?key|token|secret|password|credential)[\w.-]*\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;]+)/gi, '$1<redacted>')
    .replace(/\b(?:127\.0\.0\.1|localhost):\d+\b/gi, '<loopback-endpoint>')
    .replace(/([A-Za-z]:\\Users\\)[^\\\s"']+/g, '$1<redacted>')
    .replace(/([A-Za-z]:\\Users\\[^\\\s"']+\\)[^\s"']+/g, '$1<path>')
}

function captureSanitized(child, streamName, outputPath) {
  const output = createWriteStream(outputPath, { flags: 'wx' })
  logStreams.push(output)
  output.on('error', error => logErrors.push(`${streamName} log write failed: ${sanitizeLog(error.message)}`))
  let pending = ''
  const readable = child[streamName]
  readable.setEncoding('utf8')
  readable.on('data', chunk => {
    pending += chunk
    const lines = pending.split(/\r?\n/)
    pending = lines.pop() ?? ''
    for (const line of lines) output.write(`${sanitizeLog(line)}\n`)
  })
  readable.on('end', () => {
    if (pending) output.write(`${sanitizeLog(pending)}\n`)
    output.end()
  })
  readable.on('error', error => logErrors.push(`${streamName} stream failed: ${sanitizeLog(error.message)}`))
  return output
}

function startOwnedProcess(executable, args, env, logPrefix) {
  const child = spawn(executable, args, {
    cwd: fixtureRoot,
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
  })
  child.on('error', error => {
    child.spawnError = true
    logErrors.push(`${logPrefix} process error: ${sanitizeLog(error.message)}`)
  })
  captureSanitized(child, 'stdout', path.join(fixtureRoot, `${logPrefix}.stdout.log`))
  captureSanitized(child, 'stderr', path.join(fixtureRoot, `${logPrefix}.stderr.log`))
  return child
}

function isRunning(child) {
  return Boolean(child?.pid && !child.spawnError && child.exitCode === null && child.signalCode === null)
}

function captureOwnedBrowserState() {
  if (!isRunning(appProcess)) return
  // Scope every process and socket observation to this launch's descendants.
  // Emit only selected flags and booleans, never complete command lines.
  const script = `
    $taskProcesses = @(Get-CimInstance Win32_Process)
    $taskIds = [System.Collections.Generic.HashSet[int]]::new()
    [void]$taskIds.Add(${appProcess.pid})
    do {
      $taskAdded = $false
      foreach ($taskProcess in $taskProcesses) {
        if ($taskIds.Contains([int]$taskProcess.ParentProcessId)) {
          if ($taskIds.Add([int]$taskProcess.ProcessId)) { $taskAdded = $true }
        }
      }
    } while ($taskAdded)
    $taskOwned = @($taskProcesses | Where-Object { $taskIds.Contains([int]$_.ProcessId) })
    $taskRows = @($taskOwned | ForEach-Object {
      [pscustomobject]@{
        name = $_.Name; pid = $_.ProcessId; parent = $_.ParentProcessId
        debugFlag = [regex]::Match($_.CommandLine, '--remote-debugging-port=[0-9]+').Value
        processType = [regex]::Match($_.CommandLine, '--type=[a-z-]+').Value
        isolatedProfile = ($_.CommandLine -like '*arcana-webview-attach-*')
      }
    })
    $taskListeners = @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
      Where-Object { $taskIds.Contains([int]$_.OwningProcess) } |
      Select-Object LocalPort, OwningProcess)
    [pscustomobject]@{processes=$taskRows;listeners=$taskListeners} | ConvertTo-Json -Depth 4 -Compress
  `
  const result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], {
    windowsHide: true, encoding: 'utf8', timeout: 10_000, maxBuffer: 64 * 1024,
  })
  if (result.status === 0) console.info(`owned-browser-state=${result.stdout.trim()}`)
  else console.info('owned-browser-state=unavailable')
}

async function reserveLoopbackPort() {
  const server = createServer()
  await new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  const address = server.address()
  assert(address && typeof address === 'object', 'Could not reserve an owned loopback port.')
  const port = address.port
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
  return port
}

async function reserveDistinctLoopbackPort(otherPort) {
  let port
  do {
    port = await reserveLoopbackPort()
  } while (port === otherPort)
  return port
}

async function assertPortFree(port, label) {
  const server = createServer()
  await new Promise((resolve, reject) => {
    server.once('error', error => reject(new Error(`${label} port remained occupied after cleanup (${error.code ?? 'bind failed'}).`)))
    server.listen(port, '127.0.0.1', resolve)
  })
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
}

function isWithinPath(root, candidate) {
  const relative = path.relative(root, candidate)
  return relative !== ''
    && relative !== '..'
    && !relative.startsWith(`..${path.sep}`)
    && !path.isAbsolute(relative)
}

async function waitForDevToolsActivePort(deadline) {
  const webviewRoot = path.join(fixtureRoot, 'profile', 'webview')
  const ownedFixture = await import('node:fs/promises').then(fs => fs.realpath(fixtureRoot))
  const ownedWebviewRoot = await import('node:fs/promises').then(fs => fs.realpath(webviewRoot))
  if (!isWithinPath(ownedFixture, ownedWebviewRoot)) {
    throw new Error('The WebView2 user-data root resolved outside the owned fixture.')
  }

  const markerCandidates = [
    { label: 'webview-EBWebView', path: path.join(webviewRoot, 'EBWebView', 'DevToolsActivePort') },
    { label: 'webview-root', path: path.join(webviewRoot, 'DevToolsActivePort') },
  ]
  const fs = await import('node:fs/promises')

  while (Date.now() < deadline) {
    if (receivedSignal) throw new Error(`Interrupted by ${receivedSignal}.`)
    if (!isRunning(appProcess)) {
      throw new Error('Arcana exited before the WebView2 DevToolsActivePort marker appeared.')
    }

    for (const candidate of markerCandidates) {
      let markerRealPath
      let markerInfo
      try {
        markerInfo = await fs.lstat(candidate.path)
        if (!markerInfo.isFile() || markerInfo.isSymbolicLink()) {
          throw new Error('The WebView2 DevToolsActivePort marker is not a regular owned file.')
        }
        markerRealPath = await fs.realpath(candidate.path)
      } catch (error) {
        if (error?.code === 'ENOENT') continue
        if (error instanceof Error && error.message === 'The WebView2 DevToolsActivePort marker is not a regular owned file.') {
          throw error
        }
        throw new Error('Could not resolve the WebView2 DevToolsActivePort marker safely.')
      }

      if (!isWithinPath(ownedWebviewRoot, markerRealPath)) {
        throw new Error('The WebView2 DevToolsActivePort marker resolved outside the owned user-data root.')
      }
      if (markerInfo.size > 1024) {
        throw new Error('The WebView2 DevToolsActivePort marker exceeded the 1 KiB bound.')
      }

      let markerText
      try {
        markerText = await fs.readFile(markerRealPath, 'utf8')
      } catch {
        throw new Error('Could not read the WebView2 DevToolsActivePort marker safely.')
      }
      if (Buffer.byteLength(markerText, 'utf8') > 1024) {
        throw new Error('The WebView2 DevToolsActivePort marker exceeded the 1 KiB bound.')
      }

      const lines = markerText.trimEnd().split(/\r?\n/)
      if (lines.length < 2) continue
      if (!/^\d{1,5}$/.test(lines[0])) {
        throw new Error('The WebView2 DevToolsActivePort marker contained an invalid port.')
      }
      const port = Number(lines[0])
      if (!Number.isInteger(port) || port < 1 || port > 65535) {
        throw new Error('The WebView2 DevToolsActivePort marker contained an out-of-range port.')
      }
      if (!/^\/devtools\/browser\/[A-Za-z0-9._-]+\/?$/.test(lines[1])) {
        throw new Error('The WebView2 DevToolsActivePort marker contained an invalid browser endpoint path.')
      }

      console.info(`debug-port-source=DevToolsActivePort location=${candidate.label}`)
      return port
    }

    await delay(100)
  }

  throw new Error('WebView2 DevToolsActivePort marker did not appear within the existing 30000ms endpoint bound.')
}

async function waitForHttp(url, timeoutMs, predicate, label, child, hardDeadline) {
  const deadline = hardDeadline ?? Date.now() + timeoutMs
  let lastCode = 'no response'
  while (Date.now() < deadline) {
    if (receivedSignal) throw new Error(`Interrupted by ${receivedSignal}.`)
    if (!isRunning(child)) throw new Error(`${label} process exited before becoming ready.`)
    try {
      const requestTimeout = hardDeadline === undefined
        ? 750
        : Math.max(1, Math.min(750, deadline - Date.now()))
      const response = await fetch(url, { signal: AbortSignal.timeout(requestTimeout) })
      lastCode = `HTTP ${response.status}`
      if (response.ok) {
        const body = await response.json()
        if (predicate(body)) return body
        lastCode = 'response did not contain the expected readiness fields'
      }
    } catch (error) {
      lastCode = error instanceof Error ? error.name : 'request failed'
    }
    const remaining = deadline - Date.now()
    if (remaining > 0) await delay(hardDeadline === undefined ? 150 : Math.min(150, remaining))
  }
  throw new Error(`${label} did not become ready within ${timeoutMs}ms (${lastCode}).`)
}

async function waitForChildExit(child, timeoutMs) {
  if (!isRunning(child)) return true
  return await new Promise(resolve => {
    const timer = setTimeout(() => finish(false), timeoutMs)
    const onExit = () => finish(true)
    function finish(exited) {
      clearTimeout(timer)
      child.off('exit', onExit)
      resolve(exited || !isRunning(child))
    }
    child.once('exit', onExit)
  })
}

async function stopOwnedTree(child, label) {
  if (!child?.pid || !isRunning(child)) return
  const result = spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], {
    windowsHide: true,
    encoding: 'utf8',
    timeout: 10_000,
  })
  if (result.error) throw new Error(`Could not request cleanup of owned ${label} process tree.`)
  const exited = await waitForChildExit(child, 5_000)
  if (!exited) throw new Error(`Owned ${label} process tree did not stop after taskkill.`)
  if (result.status !== 0 && isRunning(child)) {
    throw new Error(`taskkill could not stop the owned ${label} process tree.`)
  }
}

function childEnvironment() {
  const env = { ...process.env }
  for (const key of Object.keys(env)) {
    if (/(?:API[_-]?KEY|TOKEN|SECRET|PASSWORD|CREDENTIALS?)/i.test(key)) delete env[key]
  }
  delete env.DATABASE_URL
  delete env.ARCANA_LIVE_SMOKE

  const profile = path.join(fixtureRoot, 'profile')
  const webviewData = path.join(profile, 'webview')
  env.HOME = profile
  env.USERPROFILE = profile
  env.XDG_CONFIG_HOME = path.join(profile, '.config')
  env.XDG_DATA_HOME = path.join(profile, '.local', 'share')
  env.XDG_CACHE_HOME = path.join(profile, '.cache')
  env.WEBVIEW2_USER_DATA_FOLDER = webviewData
  if (fixedRuntimeMode) env.WEBVIEW2_BROWSER_EXECUTABLE_FOLDER = fixedRuntimePath
  else delete env.WEBVIEW2_BROWSER_EXECUTABLE_FOLDER
  if (testBuildMode) delete env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
  else env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = `--remote-debugging-port=${debugPort}`
  env.RUST_LOG = 'info'
  env.ARCANA_E2E_FIXTURE_ROOT = fixtureRoot
  env.ARCANA_E2E_WORKSPACE_A = path.join(fixtureRoot, 'workspace-a')
  env.ARCANA_E2E_WORKSPACE_B = path.join(fixtureRoot, 'workspace-b')
  return env
}

let debugPort
let driverPort

async function bounded(promise, timeoutMs, label) {
  let timer
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${label} exceeded ${timeoutMs}ms.`)), timeoutMs)
      }),
      signalReceived.then(signal => { throw new Error(`Interrupted by ${signal}.`) }),
    ])
  } finally {
    clearTimeout(timer)
  }
}

async function runDiagnostic() {
  const started = performance.now()
  if (ephemeralDebugPortMode) {
    await assertPortFree(driverPort, 'EdgeDriver')
  } else {
    await Promise.all([assertPortFree(debugPort, 'WebView2 debug'), assertPortFree(driverPort, 'EdgeDriver')])
  }

  const environment = childEnvironment()
  const webviewStarted = performance.now()
  const minimalProfile = path.join(fixtureRoot, 'profile', 'webview')
  const args = minimalHostMode
    ? ['--profile', minimalProfile, '--runtime-dir', fixedRuntimePath,
      '--browser-args', '--enable-gpu --use-angle=d3d11 --remote-debugging-port=0',
      '--lifetime-seconds', '45']
    : []
  appProcess = startOwnedProcess(appPath, args, environment, minimalHostMode ? 'minimal-host' : 'arcana')
  const endpointDeadline = Date.now() + debugWaitMs
  if (minimalHostMode) {
    const fs = await import('node:fs/promises')
    while (true) {
      try {
        if (!(await fs.lstat(minimalProfile)).isDirectory()) throw new Error('Minimal host profile is not a directory.')
        break
      } catch (error) {
        if (error.code !== 'ENOENT') throw error
        if (appProcess.exitCode !== null || Date.now() >= endpointDeadline) {
          throw new Error('Minimal host did not create its profile within the endpoint bound.')
        }
        await delay(50)
      }
    }
  }
  if (ephemeralDebugPortMode) {
    debugPort = await waitForDevToolsActivePort(endpointDeadline)
    if (driverPort === debugPort) {
      driverPort = await reserveDistinctLoopbackPort(debugPort)
      await assertPortFree(driverPort, 'EdgeDriver')
      console.info('webdriver-port-reselected=yes reason=ephemeral-debug-port-collision')
    }
  }
  const endpointRemainingMs = endpointDeadline - Date.now()
  if (endpointRemainingMs <= 0) {
    throw new Error('WebView2 DevTools endpoint did not become ready within the existing 30000ms endpoint bound.')
  }
  await waitForHttp(
    `http://127.0.0.1:${debugPort}/json/version`,
    endpointRemainingMs,
    body => {
      if (typeof body.webSocketDebuggerUrl !== 'string') return false
      try {
        const endpoint = new URL(body.webSocketDebuggerUrl)
        return ['127.0.0.1', 'localhost'].includes(endpoint.hostname)
          && endpoint.port === String(debugPort)
          && endpoint.protocol === 'ws:'
          && (!ephemeralDebugPortMode
            || (/^\/devtools\/browser\/[A-Za-z0-9._-]+\/?$/.test(endpoint.pathname)
              && !endpoint.search
              && !endpoint.hash))
      } catch {
        return false
      }
    },
    'WebView2 DevTools endpoint',
    appProcess,
    ephemeralDebugPortMode ? endpointDeadline : undefined,
  )
  console.info(`webview-endpoint-ready=yes elapsed-ms=${Math.round(performance.now() - webviewStarted)}`)
  if (minimalHostMode) {
    console.info('minimal-host-endpoint-proof=yes arcana-consent-proof=no')
    return
  }

  const driverStarted = performance.now()
  const driverEnvironment = { ...environment }
  delete driverEnvironment.WEBVIEW2_USER_DATA_FOLDER
  delete driverEnvironment.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
  driverProcess = startOwnedProcess(edgeDriverPath, [`--port=${driverPort}`], driverEnvironment, 'msedgedriver')
  await waitForHttp(
    `http://127.0.0.1:${driverPort}/status`,
    driverWaitMs,
    body => body?.value?.ready === true,
    'Microsoft Edge WebDriver',
    driverProcess,
  )
  console.info(`webdriver-ready=yes elapsed-ms=${Math.round(performance.now() - driverStarted)}`)

  const attachStarted = performance.now()
  browser = await bounded(remote({
    protocol: 'http',
    hostname: '127.0.0.1',
    port: driverPort,
    path: '/',
    connectionRetryTimeout: 10_000,
    connectionRetryCount: 0,
    capabilities: {
      browserName: 'webview2',
      'ms:edgeOptions': {
        debuggerAddress: `127.0.0.1:${debugPort}`,
      },
    },
  }), sessionWaitMs, 'first WebDriver attach')
  console.info(`first-attach-succeeded=yes elapsed-ms=${Math.round(performance.now() - attachStarted)}`)

  const domStarted = performance.now()
  await bounded(browser.waitUntil(
    async () => await browser.execute(() => {
      const root = document.querySelector('#root')
      return Boolean(root && root.innerHTML.trim().length > 0)
    }),
    { timeout: domWaitMs, interval: 250, timeoutMsg: 'The first attached session never rendered a nonempty React root.' },
  ), domWaitMs + 1_000, 'React root readiness')
  const title = await bounded(browser.getTitle(), 10_000, 'document title read')
  assert.equal(typeof title, 'string')
  assert(title.trim().length > 0, 'The first attached session returned an empty document title.')
  console.info(`react-root-ready=yes title-present=yes elapsed-ms=${Math.round(performance.now() - domStarted)}`)
  console.info(`diagnostic-result=pass total-elapsed-ms=${Math.round(performance.now() - started)}`)
}

async function cleanup() {
  const errors = []
  if (browser) {
    try {
      await bounded(browser.deleteSession(), 10_000, 'WebDriver session deletion')
    } catch {
      errors.push('WebDriver session deletion did not complete cleanly.')
    }
  }

  for (const [child, label] of [[driverProcess, 'EdgeDriver'], [appProcess, 'Arcana']]) {
    try {
      await stopOwnedTree(child, label)
    } catch (error) {
      errors.push(error instanceof Error ? sanitizeLog(error.message) : `${label} cleanup failed.`)
    }
  }

  for (const [port, label] of [[driverPort, 'EdgeDriver'], [debugPort, 'WebView2 debug']]) {
    if (!port) continue
    try {
      await assertPortFree(port, label)
    } catch (error) {
      errors.push(error instanceof Error ? sanitizeLog(error.message) : `${label} port verification failed.`)
    }
  }

  const logResults = await Promise.allSettled(logStreams.map(stream => finished(stream)))
  for (const result of logResults) {
    if (result.status === 'rejected') errors.push('A retained process log did not flush cleanly.')
  }
  errors.push(...logErrors)
  return errors
}

async function main() {
  if (!isWindows) throw new Error('This diagnostic is Windows-only.')
  if (!process.versions.node) throw new Error('Node.js runtime is unavailable.')
  if (process.argv.slice(2).some(argument => !['--isolated-test-build', '--fixed-runtime-152', '--ephemeral-debug-port', '--minimal-host'].includes(argument))) {
    throw new Error('Unsupported diagnostic argument.')
  }
  if (ephemeralDebugPortMode && !testBuildMode) {
    throw new Error('--ephemeral-debug-port requires --isolated-test-build.')
  }
  if (minimalHostMode && !(testBuildMode && fixedRuntimeMode && ephemeralDebugPortMode)) {
    throw new Error('--minimal-host requires isolated build, fixed runtime and ephemeral port modes.')
  }
  const expectedDriverPath = fixedRuntimeMode
    ? path.resolve(fixedRuntimeRoot, 'driver', 'msedgedriver.exe')
    : path.resolve(repoRoot, 'target', 'e2e-tools', 'bin', 'msedgedriver.exe')
  if (appPath !== path.resolve(repoRoot, appRelativePath)
      || edgeDriverPath !== expectedDriverPath) {
    throw new Error('The diagnostic executable paths failed their workspace-boundary check.')
  }
  for (const [candidate, label] of [[appPath, 'built Arcana'], [edgeDriverPath, 'installed EdgeDriver']]) {
    try {
      await import('node:fs/promises').then(fs => fs.access(candidate))
    } catch {
      throw new Error(`Required ${label} binary is missing at its expected workspace path.`)
    }
  }
  if (fixedRuntimeMode) {
    await import('node:fs/promises').then(fs => fs.access(path.join(fixedRuntimePath, 'msedgewebview2.exe')))
  }

  fixtureRoot = mkdtempSync(path.join(tmpdir(), 'arcana-webview-attach-'))
  const profile = path.join(fixtureRoot, 'profile')
  const directories = [
    profile,
    ...(minimalHostMode ? [] : [path.join(profile, 'webview')]),
    path.join(profile, '.config'),
    path.join(profile, '.local', 'share'),
    path.join(profile, '.cache'),
    path.join(fixtureRoot, 'workspace-a'),
    path.join(fixtureRoot, 'workspace-b'),
  ]
  for (const directory of directories) mkdirSync(directory, { recursive: true })
  if (ephemeralDebugPortMode) {
    debugPort = undefined
    driverPort = await reserveLoopbackPort()
  } else {
    debugPort = testBuildMode ? 9223 : await reserveLoopbackPort()
    driverPort = await reserveDistinctLoopbackPort(debugPort)
  }
  console.info('diagnostic=windows-webview2-matched-attach')
  console.info(`browser-argument-source=${testBuildMode ? 'isolated-tauri-config' : 'runtime-environment'}`)
  console.info(`debug-port-mode=${ephemeralDebugPortMode ? 'ephemeral' : 'fixed'}`)
  console.info('credentials-cleared=yes fresh-profile=yes scratch-workspaces=yes')
  await runDiagnostic()
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  const handler = () => {
    if (!receivedSignal) {
      receivedSignal = signal
      resolveSignal(signal)
    }
  }
  signalHandlers.set(signal, handler)
  process.on(signal, handler)
}

let exitCode = 0
let runError
try {
  await main()
} catch (error) {
  captureOwnedBrowserState()
  runError = error instanceof Error ? sanitizeLog(error.message) : 'Diagnostic failed with a non-error value.'
  exitCode = receivedSignal ? 130 : 1
} finally {
  const cleanupErrors = await cleanup()
  for (const [signal, handler] of signalHandlers) process.off(signal, handler)
  if (cleanupErrors.length) {
    exitCode = 1
    console.error(`cleanup-failed count=${cleanupErrors.length}`)
    for (const error of cleanupErrors) console.error(`cleanup-error=${sanitizeLog(error)}`)
  }
  if (runError) console.error(`diagnostic-error=${runError}`)
  if (receivedSignal) console.error(`diagnostic-interrupted=${receivedSignal}`)
  if (fixtureRoot) console.info(`safe-fixture-logs-retained=os-temp/${path.basename(fixtureRoot)}`)
}

process.exitCode = exitCode
