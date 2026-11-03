// End-to-end harness — WebdriverIO driving the BUILT Tauri application.
//
// This runs the real binary, not the dev server. That distinction is the whole
// point: every defect in the restructure's register survived `tsc` and
// `cargo clippy`, and the headline acceptance test is phrased as "a user
// installs Arcana on a clean machine … and receives a build-verified change".
// Only a harness against the shipped artifact can ever assert that.
//
// Prerequisites (see e2e/README.md):
//   cargo install tauri-driver --locked
//   Linux:   apt-get install webkit2gtk-driver xvfb
//   Windows: Microsoft Edge WebDriver matching the installed Edge build
//   macOS:   not supported by tauri-driver — the macOS leg of the platform
//            triple is covered by the build smoke until WKWebView gains a driver
//
// Run with: npm run test:e2e   (after `npm run tauri -- build --debug`)

import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import os from 'node:os'
import path from 'node:path'
import { mkdtempSync, mkdirSync } from 'node:fs'

let tauriDriver: ChildProcess | undefined
let fixtureRoot: string | undefined
const originalEnvironment = new Map<string, string | undefined>()

function setFixtureEnvironment(key: string, value: string | undefined) {
  originalEnvironment.set(key, process.env[key])
  if (value === undefined) delete process.env[key]
  else process.env[key] = value
}

const isWindows = os.platform() === 'win32'
const binaryName = isWindows ? 'arcana.exe' : 'arcana'

/** The debug binary produced by `npm run tauri -- build --debug`. */
const application = path.resolve(
  __dirname,
  '..',
  'target',
  'debug',
  binaryName,
)

export const config: WebdriverIO.Config = {
  runner: 'local',
  specs: ['./specs/**/*.spec.ts'],
  maxInstances: 1,

  capabilities: [
    {
      // tauri-driver proxies to the platform's native webview driver, forwarding
      // everything except `tauri:options` to it.
      //
      // Deliberately NO `browserName`. It was set to 'wry', which tauri-driver
      // passes straight through to WebKitWebDriver, and WebKitWebDriver rejects
      // an unknown browserName with "Failed to match capabilities" — which is
      // exactly how this failed on its first CI run. tauri-driver's own examples
      // set only `tauri:options`.
      // @ts-expect-error — native tauri-driver capability is outside WDIO's browser capability types
      'tauri:options': { application },
    },
  ],

  hostname: '127.0.0.1',
  port: 4444,

  // The app has to boot a Rust process, open a database and install event
  // listeners before anything is assertable, so these are deliberately
  // generous compared with a browser-only suite.
  waitforTimeout: 20_000,
  // Bound transport waiting without replaying session-creation mutations.
  // The native adapter has its own attachment deadline.
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 0,

  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 120_000,
  },

  // Fail loudly and early when the prerequisites are missing, rather than
  // timing out against a driver that was never started.
  onPrepare: async (_config, capabilities) => {
    const probe = spawnSync('tauri-driver', ['--help'], { encoding: 'utf8' })
    if (probe.error) {
      throw new Error(
        'tauri-driver is not on PATH. Install it with:\n' +
          '  cargo install tauri-driver --locked\n' +
          'See e2e/README.md for the per-platform webdriver prerequisites.',
      )
    }

    // tauri-driver is only a proxy: without the platform's native webview
    // driver it starts and then refuses every connection, which surfaces as a
    // misleading "make sure browser driver is running on 127.0.0.1:4444".
    // Name the actual missing prerequisite instead.
    const nativeDriver = isWindows ? 'msedgedriver' : 'WebKitWebDriver'
    if (spawnSync(nativeDriver, ['--version'], { encoding: 'utf8' }).error) {
      throw new Error(
        `${nativeDriver} is not on PATH, so tauri-driver has nothing to proxy to.\n` +
          (isWindows
            ? '  Install Microsoft Edge WebDriver matching your installed Edge build.\n'
            : '  apt-get install webkit2gtk-driver\n') +
          'See e2e/README.md.',
      )
    }

    // The real desktop must never migrate/prune the developer's profile or
    // persist scenario consent against their selected workspace.
    fixtureRoot = mkdtempSync(path.join(os.tmpdir(), 'arcana-desktop-e2e-'))
    const profile = path.join(fixtureRoot, 'profile')
    const workspaceA = path.join(fixtureRoot, 'workspace-a')
    const workspaceB = path.join(fixtureRoot, 'workspace-b')
    for (const directory of [profile, workspaceA, workspaceB]) mkdirSync(directory)
    if (isWindows) {
      if (!Array.isArray(capabilities)) throw new Error('The desktop harness requires standalone capabilities.')
      for (const capability of capabilities) {
        const native = capability as unknown as {
          'tauri:options': { application: string; webviewOptions?: { userDataFolder: string } }
        }
        // Keep the driver's declared profile aligned with the actual WebView
        // environment. Alignment alone did not solve first-session attachment.
        native['tauri:options'].webviewOptions = {
          userDataFolder: path.join(profile, 'webview'),
        }
      }
    }
    for (const [key, value] of Object.entries({
      HOME: profile,
      USERPROFILE: profile,
      XDG_CONFIG_HOME: path.join(profile, '.config'),
      XDG_DATA_HOME: path.join(profile, '.local', 'share'),
      XDG_CACHE_HOME: path.join(profile, '.cache'),
      WEBVIEW2_USER_DATA_FOLDER: path.join(profile, 'webview'),
      RUST_LOG: 'info',
      ARCANA_E2E_FIXTURE_ROOT: fixtureRoot,
      ARCANA_E2E_WORKSPACE_A: workspaceA,
      ARCANA_E2E_WORKSPACE_B: workspaceB,
    })) setFixtureEnvironment(key, value)
    for (const key of ['GOOGLE_API_KEY', 'GEMINI_API_KEY', 'ANTHROPIC_API_KEY',
      'OPENAI_API_KEY', 'DATABASE_URL', 'ARCANA_LIVE_SMOKE']) {
      setFixtureEnvironment(key, undefined)
    }
    console.info(`Disposable desktop fixture: ${fixtureRoot}`)
    const nativeOverride = process.env.ARCANA_E2E_NATIVE_DRIVER
    tauriDriver = spawn('tauri-driver', nativeOverride ? ['--native-driver', nativeOverride] : [], {
      stdio: [null, process.stdout, process.stderr],
      windowsHide: true,
    })
  },

  onComplete: () => {
    try {
      if (tauriDriver?.pid && tauriDriver.exitCode === null) {
        if (isWindows) {
          const stopped = spawnSync('taskkill', ['/PID', String(tauriDriver.pid), '/T', '/F'], {
            windowsHide: true, encoding: 'utf8',
          })
          if (stopped.status !== 0 && tauriDriver.exitCode === null) {
            throw new Error('Could not terminate the owned desktop WebDriver process tree.')
          }
        } else {
          tauriDriver.kill()
        }
      }
    } finally {
      for (const [key, value] of originalEnvironment) {
        if (value === undefined) delete process.env[key]
        else process.env[key] = value
      }
      originalEnvironment.clear()
      // Retain only this disposable fixture for native logs/failure diagnosis.
      if (fixtureRoot) console.info(`Desktop evidence retained at: ${fixtureRoot}`)
    }
  },
}
