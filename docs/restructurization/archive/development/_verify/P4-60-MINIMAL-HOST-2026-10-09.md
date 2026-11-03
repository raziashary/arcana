# P4-60 Minimal WebView Host Probe — 2026-10-09

## Scope and constraints
- Implement a standalone, test-only WebView2 host to isolate host/runtime startup from Arcana initialization.
- Allowed write paths: `src-tauri/examples/webview_attach_probe.rs`, `src-tauri/Cargo.toml` only if essential, and this report.
- No builds, tests, app launches, network/install operations, git commands, environment/credential inspection, or reads of `.env`/user data. Parent owns all execution and validation.
- No policy/registry edits, shipped configuration changes, or new external dependencies.

## Progress
- [x] Created this report before investigation.
- [x] Read governing plan §0 and isolated diagnostic/cleanup guidance.
- [x] Inspect existing Wry/Tauri API and choose minimal host surface.
- [x] Implement bounded CLI host and record exact launch command and unverified limits.

## Findings
(To be appended as evidence is inspected.)

## Investigation update
- Correction: the six “Progress” entries I initially saw belonged to `P4-60-ATTACH-CAUSE-2026-10-09.md`, not this report. The parent confirmed this newly created report was absent before dispatch and no prior contents were lost; I have not overwritten any other report.
- Governing source: `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md` §0 requires the shipped code as anchor, named evidence, incremental reporting, and no stale claims. `P4-60-ATTACH-CAUSE-2026-10-09.md` captures the fresh-profile process ownership and bounded 30-second endpoint experiment. The standalone host needs a separately bounded lifetime and an owned, newly-created profile path.
- ~~`src-tauri/Cargo.toml` has Tauri 2.11 with `devtools`; `Cargo.lock` pins Wry 0.55.1 and Tao 0.35.2. Tauri 2.11.1 reexports both `tauri::wry` and `tauri::tao` from `tauri_runtime_wry`; no direct dependency edit should be necessary.~~ **RETRACTED:** source inspection missed the export's target condition; see the correction below.
- Wry 0.55.1 exposes `WebContext::new(Some(profile_path))`, `WebViewBuilder::new_with_web_context`, and Windows `WebViewBuilderExtWindows::with_additional_browser_args`. Tao 0.35.2 has a closure-based `EventLoop::run` and `ControlFlow::WaitUntil`; this supports a finite lifetime timer without a helper thread.
- Implemented interface: require exactly `--profile <absolute-new-path> --runtime-dir <existing-WebView2-folder> --browser-args <bounded-allowlisted-args> --lifetime-seconds <1..300>`. Create the profile directory exclusively and pass it to `WebContext`; set `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` before Wry initializes; render only `about:blank`. Parent owns endpoint polling and process cleanup.
- ~~API integration relies on Tauri’s existing reexports rather than a new dependency or a Tauri application builder, keeping Arcana initialization and IPC out of the probe.~~ **RETRACTED:** the exports are Android-only; the example now uses the existing locked runtime crate directly.
- ~~API decision is grounded in Tauri 2.11.1 `src/lib.rs` reexports (`tauri::wry`, `tauri::tao`) and local Wry/Tao 0.55.1/0.35.2 sources. The example will use Wry directly through those reexports and the documented Windows browser-args trait; no new Cargo dependency is needed.~~ **RETRACTED:** I did not carry the `cfg(all(feature = "wry", target_os = "android"))` guard from `tauri-2.11.1/src/lib.rs:182–184` into the claim. This was a source-reading error and caused the parent's Windows compile to fail.
- Parser safety contract: no positionals, unknown switches, duplicates, relative paths, pre-existing profile directory, malformed runtime folder, or unsupported browser args. Profile ownership is established by exclusive directory creation. The runtime folder must contain `msedgewebview2.exe`; host sets it explicitly and clears the environment browser-args/user-data overrides so explicit CLI inputs control the test.

## Implementation and parent handoff
- Added `src-tauri/examples/webview_attach_probe.rs`. It creates one Tao window, an explicit-profile Wry WebView displaying `about:blank`, and applies the supplied browser args through Wry's Windows extension. It contains no Tauri `Builder`, Arcana library initialization, database, provider, or IPC setup.
- The CLI requires all four options: `--profile`, `--runtime-dir`, `--browser-args`, and `--lifetime-seconds`. It rejects unknown/duplicate options, requires a new absolute profile path whose parent exists, validates an absolute runtime directory containing `msedgewebview2.exe`, allowlists only `--remote-debugging-port=<0..65535>`, `--enable-gpu`, and `--use-angle=d3d11`, and bounds lifetime to 1–300 seconds. The Tao loop waits until a fixed deadline and exits on deadline or window close.
- Suggested PowerShell launch (parent owns execution):

  ```powershell
  cargo run --manifest-path .\src-tauri\Cargo.toml --example webview_attach_probe -- --profile 'C:\arcana\target\p4-60-minimal-host\profile-2026-10-09' --runtime-dir 'C:\arcana\target\e2e-tools\webview2-152\runtime\Microsoft.WebView2.FixedVersionRuntime.152.0.4191.62.x64' --browser-args '--enable-gpu --use-angle=d3d11 --remote-debugging-port=0' --lifetime-seconds 90
  ```

  The parent directory `C:\arcana\target\p4-60-minimal-host` must exist and the specified `profile-2026-10-09` directory must not. Parent should use the same fixed 152 runtime and check only `127.0.0.1` `/json/version` plus the profile's `DevToolsActivePort` candidates while the bounded host is alive. The existing P4-60 report documents `EBWebView` under the supplied WebView2 data root; Wry passes `WebContext`'s path directly to `CreateCoreWebView2EnvironmentWithOptions`, so the actual marker location remains to be observed for this host.
- Unverified limits: this agent did not run a Cargo build, test, application launch, endpoint probe, or port check. The parent’s first Windows compile failed on the incorrect Tauri reexport; after the correction below, Rust API/type correctness still awaits the parent’s next compile. Whether WebView2 accepts the selected runtime and flags in this host, marker location, `/json/version` availability, and runtime cleanup behavior also remain for parent validation. The host leaves its caller-owned profile directory in place for parent inspection/cleanup and outputs only fixed status/error labels (no paths, full command lines, or remote endpoint values).

## Correction and recovery
- Verified the full export bodies: `tauri-2.11.1/src/lib.rs:182–184` gates both reexports on `all(feature = "wry", target_os = "android")`. In contrast, `tauri-runtime-wry-2.11.1/src/lib.rs:98–100` publicly reexports `tao`, `wry`, and their public types without that Android-only gate; Wry's Windows extension trait is available from the reexported crate.
- Updated the Windows-only example import to `tauri_runtime_wry::{tao, wry}` and added `[target.'cfg(windows)'.dev-dependencies] tauri-runtime-wry = "=2.11.1"`, matching the already locked runtime stack; no new package version is introduced. The selected `EventLoop::run` API returns `!`, so the trailing semicolon was removed for the function's `Result` return type.
- Retraction mechanism: the incorrect claims above remain visible with strike-through, followed by the exact cfg omission and the compile failure it caused. No source validation was run by this agent; the parent owns the next compile.


## Coordinator compile and controlled launch — 2026-10-09

After the export correction, the example builds successfully offline on Windows (2m25s). Cargo.lock changes only the existing arcana package dependency list; no package/version/checksum changed. The existing owned-process diagnostic now supports --minimal-host, requiring isolated build, fixed152 and ephemeral modes. It supplies a new profile to the host, waits for its creation within the same endpoint deadline, and skips WebDriver/consent assertions after an endpoint check. node --check passes.

Actual launch: `node e2e/windows-attach-diagnostic.mjs --isolated-test-build --fixed-runtime-152 --ephemeral-debug-port --minimal-host`. The host reports probe-window-ready=yes, creates browser and renderer with the debugging flag, but no DevToolsActivePort marker or TCP listener appears within30s. The diagnostic captures only owned-process metadata and cleans the complete tree. Explicit PID readback confirms no owned probe/browser process survives. The shipped tauri.conf.json SHA256 remains72d13a27fcda4f6780e4b3bd1b2678adf625e3a944b13622ff0db9ece991c9c2. This reproduces the endpoint failure without Arcana startup, provider, database or IPC; it does not identify the underlying runtime/host cause or satisfy P4-60/TG4-16.

Microsoft's [debugging guide](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code) describes the additional browser argument endpoint. A [first-hand elevated-host report](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5639) suggests a version150 elevation failure, but the actual diagnostic parent has no Administrator token; that hypothesis does not explain this reproduction. No machine policy was changed, and no unapproved older runtime was installed. Further diagnostic direction is needed rather than repeating the same timeout experiment.

A standalone PowerShell launcher proposal was rejected by automatic approval review as blocked by policy, with no more specific reason. It was not written or executed. The approved existing Node diagnostic supplied the bounded owned-process launch instead.

Parent parser smoke: the built example with no arguments returns exit1 and the fixed missing-browser-args diagnostic; no native window or listener starts. Node syntax validation and workspace formatter pass. This diagnostic unit is usable and its missing endpoint is an observed host condition, not a passing desktop consent proof.
