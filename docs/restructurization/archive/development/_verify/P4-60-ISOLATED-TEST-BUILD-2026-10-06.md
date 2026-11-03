# P4-60 Isolated Test Build Diagnostic (2026-10-06)

Status: investigation in progress. This file is the sole owned path; all other paths are read-only. No build or process launch is authorized in this task.

## Progress 1: scope and workspace state

- Created this report before source investigation, as required.
- The checkout is `main...origin/main [ahead 1]` and already has unrelated modified/untracked planning, e2e, and verification files. I will leave them untouched.
- Located the requested Tauri entry/config files, e2e README/spec/diagnostic, and existing P4-60 diagnostic reports. No build or process was run.

## Progress 2: source-grounded findings

### Confirmed

- Shipped `src-tauri/tauri.conf.json` configures the only native window with `additionalBrowserArgs: "--enable-gpu --use-angle=d3d11"` (line 24). `main.rs` builds from `tauri::generate_context!()`, so Tauri's resolved build config supplies the window settings; `Cargo.toml` uses Tauri 2.11 and enables `devtools`.
- Local locked sources are Tauri 2.11.1, `tauri-utils` 2.9.1 and Wry 0.55.1. `tauri-utils` maps `additionalBrowserArgs` into the generated `WindowConfig`; Tauri's window builder maps it to Wry's Windows `additional_browser_args`; Wry passes that value to `CoreWebView2EnvironmentOptions::set_additional_browser_arguments` (`wry/src/webview2/mod.rs:294,327`). This is a direct programmatic argument path.
- The current diagnostic sets `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>` in the app environment, and the prior diagnostic report records that the WebView2 browser process received the flag while no listener appeared on that port. The same report records two 30-second `/json/version` waits failing. This establishes failed endpoint readiness, not why WebView2 did not serve it.
- Tauri officially supports `--config` for `build`; supplied config is JSON Merge Patch over the base config. Microsoft documents `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` as appended to the `AdditionalBrowserArguments` API value, and says programmatically supplied browser flags are the alternative diagnostic path. The browser arguments are therefore worth testing via the actual Tauri config/API path in this isolated build, while keeping the base config byte-for-byte untouched.

### Root-cause assessment

**Confirmed symptom:** this process tree does not expose the expected loopback DevTools `/json/version` endpoint within the existing 30-second bound despite the observed browser argument. **Root cause remains unconfirmed.** A test build that bakes the port into `additionalBrowserArgs` will distinguish the explicit Tauri/Wry option path from relying on the runtime environment override. If the explicit path also produces no listener, the environment injection route is not a sufficient explanation; the retained browser stderr/verbose log and listener/PID snapshot become the next discriminating evidence. Do not infer a Windows policy, elevation, WebView2 defect, or application-startup defect from the current artifacts.

## Smallest isolated test-build patch

Keep production Rust and `src-tauri/tauri.conf.json` unchanged. Create a disposable config in OS temp by parsing `src-tauri/tauri.conf.json`, cloning it, and changing only `app.windows[0].additionalBrowserArgs` to:

```text
--enable-gpu --use-angle=d3d11 --remote-debugging-port=9223 --enable-logging=stderr --v=1
```

The script must assert that a structural diff between the copied config and source config consists only of that one field. The complete copied config avoids JSON Merge Patch replacing the `app.windows` array with a partial object; its only intentional difference preserves the shipped GPU flags and adds a fixed test port plus bounded browser diagnostics. Fail before build if 127.0.0.1:9223 is occupied. Build into an isolated Cargo target directory so the normal debug executable is not overwritten:

```powershell
$testConfig = Join-Path $env:TEMP 'arcana-p4-60-test-tauri.conf.json'
$targetDir = 'C:\arcana\target\p4-60-isolated'
# A short PowerShell step reads the checked-in JSON, clones it, changes only
# app.windows[0].additionalBrowserArgs as above, verifies the one-field diff,
# then writes $testConfig. It does not modify the checked-in config.
$env:CARGO_TARGET_DIR = $targetDir
npm run tauri -- build --debug --config $testConfig
```

The build output is expected at `target/p4-60-isolated/debug/arcana.exe`. A minimal change to the owned diagnostic should allow an explicit app path and fixed debug port via `ARCANA_P4_60_APP` / `ARCANA_P4_60_DEBUG_PORT`, retain its disposable profile and owned-tree cleanup, and **remove** `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` from the app child environment. Keep the distinct EdgeDriver port dynamically reserved. Before starting either process, fail if either configured port is occupied. Run the existing 30-second `/json/version` check, preserve sanitized app/EdgeDriver logs, and capture read-only PID/command-line/listener evidence for the owned fixture process tree. Do not increase the endpoint timeout.

Example bounded command after that small diagnostic adjustment:

```powershell
$env:ARCANA_P4_60_APP = 'C:\arcana\target\p4-60-isolated\debug\arcana.exe'
$env:ARCANA_P4_60_DEBUG_PORT = '9223'
node e2e/windows-attach-diagnostic.mjs
```

The test build changes only compile-time window arguments in its disposable Tauri config and emits to its own ignored target directory. Verify the checked-in `tauri.conf.json` hash is identical before and after, and verify the copied config's sole diff is `app.windows[0].additionalBrowserArgs`; do not copy the overlay into `src-tauri/` or add a feature/config default. This keeps shipped settings byte-for-byte stable under ADR-LOG-RESTRUCT-0321.

## Falsification and evidence boundary

- **Supports an environment-path hypothesis:** the new binary exposes `/json/version` on the explicit port on its first launch, while the existing environment-only attempt had no listener. This does not by itself explain why the environment path failed.
- **Falsifies the environment-only explanation:** the Tauri-configured test build's browser command line contains `--remote-debugging-port=9223`, the process is owned and using the fresh fixture profile, but no endpoint/listener appears before the same 30-second deadline. Then inspect its retained WebView2 stderr and actual browser/runtime command line; report root cause as unresolved if those do not discriminate.
- **Attachment is a separate result:** only if `/json/version` is reachable should the diagnostic proceed to matching EdgeDriver `debuggerAddress`, first-session attach, and nonempty `#root` / title. Endpoint success alone is not the consent UI proof; the existing consent scenario still must run through native IPC and its restart.
- Cleanup must use the diagnostic's owned PID process-tree termination only, then bind-check both owned ports. Leave any unrelated process/listener untouched. The existing report says prior diagnostic-owned cleanup passed; this test build has no run result yet.

## Source references

- `src-tauri/tauri.conf.json:24`; `src-tauri/src/main.rs` builder call; `src-tauri/Cargo.toml` Tauri features.
- Installed `tauri-utils-2.9.1/src/config.rs:2080,3882`; `tauri-2.11.1/src/webview/webview_window.rs:1015-1018`; `wry-0.55.1/src/webview2/mod.rs:294,327`.
- [Tauri configuration files](https://v2.tauri.app/develop/configuration-files/) documents `--config` JSON Merge Patch; [Microsoft WebView2 browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags) documents environment and programmatic flag paths; [Microsoft WebView2 environment API](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/webview2-idl) documents environment-provided arguments appending to API options.
- ADR-LOG-RESTRUCT-0321 authorizes the isolated test build and keeps shipped settings unchanged.

No build, app launch, driver launch, listener cleanup, or source/config change was performed here.


## Main-session bounded experiment

The owner approved the test build (ADR-0321). It builds successfully in 13m52s using a full config clone with only `app.windows[0].additionalBrowserArgs` changed, separate ignored Cargo target, no bundle, offline locked Cargo dependencies. Frontend TypeScript/Vite build passed. Shipped config SHA256 remains `72D13A27FCDA4F6780E4B3BD1B2678ADF625E3A944B13622FF0DB9ECE991C9C2`.

The actual command is `node e2e/windows-attach-diagnostic.mjs --isolated-test-build`. This bounded switch selects only the known ignored build path and fixed port9223, removes the environment browser-argument override, retains fresh profiles and credential clearing, and checks cleanup. The earlier suggested environment-variable app-path overrides above were not implemented.

First isolated launch failed the unchanged 30-second endpoint readiness bound. A second diagnostic added selected process/flag/listener capture before cleanup. Its JavaScript template initially erased a regex backslash, so its empty `debugFlag` values are invalid diagnostic evidence. Replacing that expression with `[0-9]+` and rerunning proves the owned fresh-profile browser and renderer both carry `--remote-debugging-port=9223`, but every owned process has zero TCP listeners after the readiness deadline. A renderer exists; neither attachment nor React/consent behavior is proven. Each owned tree is terminated and both ports bind free afterward. No Arcana or EdgeDriver process remains. Raw safe output: `target/p4-60-isolated/attach-with-process-diagnostics.log`; build output: `target/p4-60-isolated/build.log`.

Read-only host checks show the launching principal is not elevated and no queried Edge/WebView2 DeveloperToolsAvailability or RemoteDebuggingAllowed policy value is set. Microsoft’s feedback repository records a similar non-elevated-host regression beginning in153: [WebView2 issue5718](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5718). It is a comparison lead, not proof that Arcana has that same cause. A fixed-version152 runtime and matched-driver comparison is proposed, awaiting owner Rule4 direction for the new isolated test dependency. No machine runtime downgrade or registry mutation is authorized or performed.
