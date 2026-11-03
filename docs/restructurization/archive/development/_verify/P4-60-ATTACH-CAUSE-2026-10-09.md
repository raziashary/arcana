# P4-60 Attach Cause — 2026-10-09

Investigation complete; root cause remains unconfirmed. The scope was read-only source and approved diagnostics evidence; only this report was created or edited.

## Scope and method

- Report file created before source investigation, as requested.
- No tests, builds, app/server launches, network fixtures, installs, Git operations, or reads of credentials or user data.
- The five named inputs and their directly referenced safe evidence were inspected; findings were recorded incrementally.

## Progress 1: named inputs

- The named diagnostic has two modes: default runtime plus environment-provided debug flag, and `--fixed-runtime-152` selecting ignored WebView2 152 runtime/driver paths. `--isolated-test-build` selects a separate executable and fixed test port. It waits for `/json/version` before starting EdgeDriver, so current failure is upstream of WebDriver session creation.
- The fixed-152 report says the browser/renderer received the flag but exposed no owned TCP listener before the unchanged 30-second bound; this falsifies runtime 153 alone as the cause. The report also records cleanup success.
- The isolated-build report says the same no-listener symptom occurred when the port was supplied through Tauri `additionalBrowserArgs`, with the browser and renderer command lines carrying the flag. Its early capture with blank flags was explicitly invalidated and later corrected; the corrected result is the usable evidence.
- These prior reports establish the symptom across two runtime versions and two argument delivery paths, but do not establish why the browser process does not listen.

## Progress 2: source and safe-log evidence

- `e2e/windows-attach-diagnostic.mjs` has a fixed flow: assert both loopback ports free; build a credential-cleared child environment with a fresh WebView2 user-data folder; launch Arcana; poll `http://127.0.0.1:<debugPort>/json/version` up to 30 seconds; only then launch the selected EdgeDriver and attempt WDIO attach. Therefore current evidence places failure before EdgeDriver startup or WebDriver attach.
- `captureOwnedBrowserState()` snapshots descendants of this diagnostic-owned Arcana PID, extracts only selected flags/process types/profile markers, and lists listeners owned by those descendants. `cleanup()` terminates owned process trees and bind-checks both ports. The fixed runtime mode selects the signed 152 folder; the default mode uses the existing installed-path EdgeDriver/runtime.
- Safe retained logs for both available runs show Arcana -> WebView2 browser -> renderer process ancestry; browser and renderer include the remote-debugging flag; `listeners: []`; endpoint polling ended with `TypeError`. The fixed-152 log reproduces this symptom. No EdgeDriver process is captured because code launches it only after endpoint readiness.
- Shipped `src-tauri/tauri.conf.json` sets only the existing GPU arguments. `src-tauri/src/main.rs::main` constructs `tauri::Builder::default()`, then `.build(tauri::generate_context!())`; `src-tauri/Cargo.toml` enables Tauri `devtools`. Source does not add a separate debugging server or alter WebView2 debugging startup. In the harness, app code is launched as a built executable by the diagnostic, while `wdio.conf.ts::onPrepare` belongs to the normal tauri-driver flow and configures a disposable profile/clears selected credentials before spawning tauri-driver.
- Current logs and source establish no listener, not a particular cause. Browser argument presence is not proof WebView2 accepted/acted on it; the 152 result means version 153 alone is insufficient.

## Progress 3: cause assessment and next experiment

### Confirmed findings

- No root cause is confirmed. In both the Tauri-configured default-runtime run and Microsoft-signed fixed-152 run, the owned process snapshot shows the browser and renderer carrying the remote-debugging switch but no listener. In the fixed-152 run, `/json/version` polling still ends in a fetch `TypeError`; `runDiagnostic()` fails before launching EdgeDriver or attempting attachment. Thus driver mismatch/first-session behavior cannot cause this observed endpoint failure.
- The filtered app stderr has no WebView2/DevTools/browser diagnostic message. This is not evidence that the WebView2 child emitted none: the harness captures Arcana parent stdout/stderr, while the WebView2 browser is a descendant process whose output channel is not established by the source.
- Microsoft docs confirm `remote-debugging-port` enables remote debugging over HTTP, programmatic `AdditionalBrowserArguments` is supported, and environment-provided flags can be ignored by elevated hosts. The prior fixed-runtime report says the host was not elevated and no queried policy value was set; that check is report evidence, not independently repeated here. Source flag presence alone still does not prove the runtime accepted the setting.

### Hypotheses (unproven)

- **Fixed-port-specific behavior:** WebView2 may be ignoring or failing the fixed port setting despite the command line. The current runs do not distinguish that from remote debugging being inactive.
- **Remote debugging inactive due to runtime/host behavior or policy:** Both runtime versions and both environment/API argument paths produced the same no-listener symptom. The prior host policy checks reduce, but do not eliminate, this hypothesis.
- **Arcana/Tauri host interaction:** possible, but the logs show successful desktop initialization and a renderer; no source/log evidence identifies a Tauri startup defect. A standalone WebView2 host would be needed to isolate the host layer and would require a new harness beyond the existing Arcana-owned diagnostic contract.

### One falsifiable next experiment

Use the already approved isolated Tauri test-build/diagnostic boundary with the same fixed Microsoft 152 runtime and matching driver, fresh profile, credential clearing, owned process capture, sanitized logs, unchanged 30-second bound, and existing cleanup. In the disposable copied Tauri config, change only `additionalBrowserArgs` from the fixed `--remote-debugging-port=9223` to `--remote-debugging-port=0`; keep the shipped config untouched. Extend only the owned diagnostic to poll the WebView2 user-data directory for `DevToolsActivePort`, read its selected port, validate that the WebSocket endpoint is loopback, then test that endpoint and pass its discovered port as `debuggerAddress`. The harness currently supplies `WEBVIEW2_USER_DATA_FOLDER=<fixture>\profile\webview`; Microsoft documentation notes the runtime adds `\EBWebView`, so probe the resolved `profile\webview\EBWebView\DevToolsActivePort` and capture the owned browser `--user-data-dir` to verify the path. Do not extend any timeout.

Falsification: if the file appears and its port serves `/json/version`, fixed-port handling was the discriminating factor and the matched driver can continue to attach. If the file does not appear and the same owned process tree has no listener, the fixed-port hypothesis is falsified; stop the Arcana diagnostic line and request a decision to add a minimal standalone WebView2 host (or another separately approved diagnostic) to distinguish host integration from machine/runtime policy. This experiment changes the debug-port mode and adds its runtime-produced marker as an independent observable; it is not another delay or runtime-version retry.

Microsoft source: [WebView2 browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags) states the flag behavior and programmatic argument path. [Chrome DevTools MCP for WebView2](https://learn.microsoft.com/en-us/microsoft-edge/web-platform/devtools-mcp-server) documents `--remote-debugging-port=0`, discovery through `DevToolsActivePort`, and that the file is created when remote debugging is active (lines 306-328, 342-350, 373-378, 424-440).

## Verified map and useful next reads

- Diagnostic ownership/flow: `e2e/windows-attach-diagnostic.mjs` — `startOwnedProcess` (71), `captureOwnedBrowserState` (91), `waitForHttp` (158), `childEnvironment` (209), `runDiagnostic` (254), `cleanup` (326), `main` (361).
- Normal WDIO entry: `e2e/wdio.conf.ts` — exported `config` (45), `onPrepare` (86) creates disposable fixture and launches `tauri-driver`; this harness uses the separate native driver proxy path, while the attach diagnostic launches `msedgedriver` directly after DevTools readiness.
- Desktop startup/config boundary: `src-tauri/src/main.rs::main` builds `tauri::Builder` and generated context (122, 315); `src-tauri/tauri.conf.json` owns shipped window arguments (24); `src-tauri/Cargo.toml` enables Tauri `devtools` (21).
- Safe evidence read: `target/p4-60-isolated/attach-with-process-diagnostics.log` and `target/p4-ci-checks/fixed-webview-152-2026-10-09.log`; child Arcana logs are under the recorded disposable `arcana-webview-attach-*` folders in OS temp. The root remains unknown; no consent UI or WebDriver attachment was reached.
- Next useful read if the experiment fails: inspect only the new diagnostic-owned `DevToolsActivePort` outcome and exact owned browser `--user-data-dir`; if absent, stop and obtain a scope decision before introducing a minimal standalone host or altering machine policy. Do not close P4-60/TG4-16 from these symptoms.

During the read-only investigation, no source/config/build/test/driver changes or phase/gate closure were performed; that investigation created only this report.



## Progress 4: ephemeral-port refinement implementation

- New task authorized edits are limited to this report and e2e/windows-attach-diagnostic.mjs; no tests, builds, app launches, installs, Git operations, credential/user-data access, or network fixtures are authorized.
- Planned contract: add --ephemeral-debug-port only with --isolated-test-build; resolve only a bounded numeric port from a marker file in the owned profile's webview/EBWebView or exact webview root; reject non-local/untrusted marker content; retain the existing total 30-second endpoint deadline; then use existing WebDriver attach and cleanup. Parent owns the rebuilt isolated config and run.


## Progress 5: implementation grounding

- The current diagnostic uses a fixed debugPort before launch, validates /json/version WebSocket protocol/host/port, and gives waitForHttp a fresh 30-second timer. The ephemeral branch must instead begin one absolute endpoint deadline immediately after launch, resolve the marker within it, and pass only the remaining time to HTTP readiness.
- The user-data root is created as <fixture>/profile/webview by childEnvironment() and directory setup. Microsoft’s cited guide confirms DevToolsActivePort discovery and the WebView2 EBWebView suffix; the implementation will check only those two fixed candidate paths under that owned root and will never navigate to the marker's second-line path.
- Existing main() accepts only two flags and chooses fixed 9223 for isolated builds. Existing driver attach uses debugPort, so resolving it before that step preserves the attach and cleanup path.


## Progress 6: implementation complete; parent run pending

- `e2e/windows-attach-diagnostic.mjs` now accepts `--ephemeral-debug-port` only alongside `--isolated-test-build`; no executable paths or dependencies changed.
- The ephemeral branch searches only `<fixture>/profile/webview/EBWebView/DevToolsActivePort` then `<fixture>/profile/webview/DevToolsActivePort`. It verifies the root and marker realpaths remain under the owned fixture, rejects symlink/non-file markers, caps marker size at 1 KiB, parses a decimal port in 1..65535, and validates the marker path shape without following it.
- The marker and `/json/version` readiness share one absolute 30-second deadline from Arcana launch. The endpoint request uses only `http://127.0.0.1:<validated-port>/json/version`; in ephemeral mode the response WebSocket URL must be `ws:` on `127.0.0.1` or `localhost`, match the numeric port, and have a `/devtools/browser/...` path with no query or fragment. The driver port is re-reserved if the discovered port collides. Existing driver attach, React-root check, teardown, and port-free checks remain in use.
- Default and fixed-port modes still use their original selected port and the original 30-second `waitForHttp` behavior; no marker is read in those modes.
- Parent-owned rebuild config must set programmatic `additionalBrowserArgs` to `--enable-gpu --use-angle=d3d11 --remote-debugging-port=0`. Exact run arguments after that rebuild: `node e2e/windows-attach-diagnostic.mjs --isolated-test-build --fixed-runtime-152 --ephemeral-debug-port`. This task did not run syntax checks, tests, builds, or the application; parent owns the diagnostic run.
- Edited symbols: `isWithinPath` (159), `waitForDevToolsActivePort` (167), bounded-deadline extension to `waitForHttp` (244), ephemeral branch in `runDiagnostic` (344), flag validation in `main` (479), and mode output (521). `--ephemeral-debug-port` declaration is at line 15.
- No endpoint/attachment/consent proof or phase/gate closure is claimed. The result remains pending the parent-owned run.


## Coordinator results

The isolated Tauri ephemeral-port test reproduces no marker/listener in30s. The separately approved minimal Wry host also reproduces it without Arcana initialization. Both owned trees are stopped; source config is unchanged. See P4-60-MINIMAL-HOST-2026-10-09.md for compile correction, actual command and remaining unknowns. This is not desktop consent proof.
