// End-to-end harness — WebdriverIO driving the BUILT Tauri application.
//
// This runs the real binary, not the dev server. That distinction is the whole
// point: every defect in the restructure's register survived `tsc` and
// `cargo clippy`, and the headline acceptance test is phrased as "a user
// installs AutoDev on a clean machine … and receives a build-verified change".
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

let tauriDriver: ChildProcess | undefined

const isWindows = os.platform() === 'win32'
const binaryName = isWindows ? 'autodev.exe' : 'autodev'

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
      maxInstances: 1,
      // tauri-driver proxies to the platform's native webview driver, forwarding
      // everything except `tauri:options` to it.
      //
      // Deliberately NO `browserName`. It was set to 'wry', which tauri-driver
      // passes straight through to WebKitWebDriver, and WebKitWebDriver rejects
      // an unknown browserName with "Failed to match capabilities" — which is
      // exactly how this failed on its first CI run. tauri-driver's own examples
      // set only `tauri:options`.
      // @ts-expect-error — tauri:options is a tauri-driver extension, not in the W3C types
      'tauri:options': { application },
    },
  ],

  hostname: '127.0.0.1',
  port: 4444,

  // The app has to boot a Rust process, open a database and install event
  // listeners before anything is assertable, so these are deliberately
  // generous compared with a browser-only suite.
  waitforTimeout: 20_000,
  // 120s was long enough that a hung launch cost 8 minutes before reporting.
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 3,

  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 120_000,
  },

  // Fail loudly and early when the prerequisites are missing, rather than
  // timing out against a driver that was never started.
  onPrepare: () => {
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

    tauriDriver = spawn('tauri-driver', [], { stdio: [null, process.stdout, process.stderr] })
  },

  onComplete: () => {
    tauriDriver?.kill()
  },
}
