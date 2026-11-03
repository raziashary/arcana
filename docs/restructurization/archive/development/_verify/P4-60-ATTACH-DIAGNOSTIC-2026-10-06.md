# P4-60 WebDriver attachment diagnostic — 2026-10-06

## Scope and durability note

Read-only diagnostic of the fresh-profile WebDriver attachment failure, with report creation/append as the only authorized write. No tests, servers, Git changes, credentials, `.env`, vault, Calendar, or expanded-goal access. The raw log is not reproduced here.

**Ordering lapse:** Investigation began before the parent supplied the instruction to create this file first. Before that instruction, I read `e2e/wdio.conf.ts`, `e2e/README.md`, both existing specs, the P4-60 work report, the paused checkpoint passages, and the requested log's aggregate match counts. I had not run tests or altered files. Recording those observations now before continuing.

## Initial observations

- `e2e/wdio.conf.ts` uses a built Tauri binary with `tauri-driver`; Windows assigns a disposable `webviewOptions.userDataFolder` and matching `WEBVIEW2_USER_DATA_FOLDER`, sets `connectionRetryCount: 0`, and removes live API/database environment variables. The config currently has no `additionalBrowserArguments` override.
- `e2e/specs/workspace-consent.spec.ts` exercises the production Settings UI/native IPC path, persists/reloads consent across an application restart, switches between two disposable workspaces, revokes consent, and checks structural-only diff behavior. Its `finally` block revokes fixture consent and clears workspace selection.
- The P4-60 work report says the consent scenario passed, including restart; the first fresh-profile application-launch session still failed attachment. The paused checkpoint (§ “Exact next issue” and later consent checkpoint) says prior timeout increases and profile-folder alignment did not fix it. A dynamic explicit port was interrupted, and the checkpoint explicitly says not to treat it as a fix.
- The requested diagnostic log exists. Aggregate scanning found four `chrome not reachable` / session-creation-failure markers, no `DevToolsActivePort` literal, and no literal remote-debugging-port or profile-setting markers. No raw line/value was emitted.
- Initial official-source search found Tauri's driver README describes two distinct WebDriver ports (proxy and native server), and Microsoft documents WebView2's need for a remote debugging port plus a matching user-data directory for DevTools auto-attach. These are background facts, not yet evidence of this failure's cause.

## Pending

## Likely cause and narrow recommendation

**Likely cause (not confirmed):** cold-start WebView2 attachment/discovery race on the first EdgeDriver session. The paused checkpoint says the first fresh-profile attach times out while later consent sessions, including a native restart, pass. The Edge/WebView2 build and driver were recorded as matching. Increasing the timeout and aligning the app/driver user-data folder did not change the symptom. The explicit-port run also did not change the first-session failure. This points to the first-session WebView2 endpoint being unavailable to EdgeDriver when it attempts attachment, rather than a general app boot or consent defect.

The specified log supports only the generic attach symptom: four session-creation / `chrome not reachable` markers and no literal `DevToolsActivePort` message or logged remote-debugging-port value. It does not show the WebView2 process's actual data directory or prove whether an endpoint was created. The cause therefore remains uncertain: delayed WebView2 creation, discovery-path mismatch, and EdgeDriver's attach mode cannot be distinguished from this artifact alone.

**Smallest correction to evaluate:** use the documented WebView2 *attach* path for the harness, with a known remote-debugging endpoint and a matching EdgeDriver debugger address, and wait for that endpoint before creating the WebDriver session. Keep this local to the test harness/profile. The prior dynamic `additionalBrowserArguments` attempt changed only the WebView2 argument and did not pass, so do not treat it as evidence that a complete attach configuration was tried. Microsoft documents the pair: set `--remote-debugging-port=<port>` on the WebView2 host and set EdgeDriver's `DebuggerAddress` to that same port. This is narrower than increasing deadlines and directly addresses the unavailable attachment endpoint. It is a candidate correction, not a recommendation to change shipped app behavior.

**Falsifiable check:** from a newly created disposable profile, run one clean attach attempt with the same explicit port supplied to both WebView2 and EdgeDriver. Before session creation, confirm the expected WebView2 endpoint is present in the actual user-data directory; then require the first WebDriver session to reach `#root`, pass the workspace-consent scenario, and survive its application restart. The hypothesis is falsified if the endpoint is present and correctly addressed before the first session yet EdgeDriver still reports unreachable; it is supported if endpoint absence/mismatch coincides with the failed attempt and the matched attach succeeds on the first session. Do not count a passing second spec/session as cold-start proof.

**Official references:** [Tauri `tauri-driver` README](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-driver/README.md) documents the intermediary and separate proxy/native WebDriver ports. [Microsoft Edge WebDriver for WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webdriver) distinguishes launch from attach and requires a matching remote-debugging port and `DebuggerAddress` for the attach path. [Microsoft Edge capabilities](https://github.com/MicrosoftDocs/edge-developer/blob/main/microsoft-edge/webdriver/capabilities-edge-options.md) identifies `webviewOptions.additionalBrowserArguments` as the way to pass flags to the WebView2 browser process.

## Owner-approved implementation contract — 2026-10-06

Owner approved testing matched WebView2 attach mode. Implement only `e2e/windows-attach-diagnostic.mjs`, using the existing installed WebdriverIO remote package and `target/e2e-tools/bin/msedgedriver.exe`; add no dependency and do not change production or other harness files. The script must create fresh disposable profile/workspace paths; clear live credential variables; select owned loopback ephemeral debugging and separate WebDriver ports; launch the built desktop with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<debug-port>`; bounded-wait for `/json/version`; create `remote` with `browserName: 'webview2'` and matching `ms:edgeOptions.debuggerAddress`; assert the first session renders nonempty `#root` and a title; report safe timings; and deterministically delete the session and terminate only its owned app/driver process trees. It must verify both ports are free and retain safe fixture logs. Signal/error cleanup is required. This is a diagnostic only and does not replace the full consent/restart scenario.

Execution is expressly out of scope for this worker. The root agent will run the exact diagnostic. No execution result or attach-mode outcome is claimed here.

### Read-only implementation preparation

The installed `webdriverio` package is version 9.31.5 and exports `remote` (`node_modules/webdriverio/build/index.d.ts`). The requested installed EdgeDriver and built app paths both currently exist. Existing `e2e/wdio.conf.ts` establishes the repository's fixture environment names and credential-clearing set. I have not run either binary or the diagnostic.

### Implementation progress

Created `e2e/windows-attach-diagnostic.mjs` only. It selects distinct temporary loopback ports; starts the built app with the owned WebView2 debug port and fixture profile; bounded-checks `/json/version`; starts the installed EdgeDriver on its own port; uses WDIO `remote` with `browserName: webview2` and the matching debugger address; and checks the React root and title on the first attached session. App and driver logs are filtered for common credential-shaped fields and retained under a unique OS-temp fixture. Cleanup deletes the remote session, taskkills only each recorded process PID tree, and verifies both ports bind after shutdown. SIGINT/SIGTERM are handled through the same cleanup path. The EdgeDriver child environment omits WebView2 debug/profile overrides so only the app owns those settings.

This script has not been executed or syntax-checked under the owner's no-execution boundary. Static review confirms signal handling rejects bounded waits into cleanup; cleanup always attempts session deletion, driver/app process-tree stop, port-free checks, and log flush, collecting errors rather than swallowing them. Credential environment names containing API-key, token, secret, password, or credential terms are omitted from child processes. Log sanitization redacts credential-shaped fields, user-home paths, and loopback port values. The root agent owns execution and will need to assess any runtime-only issues.

## Main-session execution — 2026-10-06

`node --check e2e/windows-attach-diagnostic.mjs` passes. The root then runs the approved diagnostic twice against fresh disposable profiles. Both return exit 1 before creating a driver/session: the `/json/version` endpoint remains unavailable for the selected 30-second bound. The second run adds read-only inspection of the recorded Node/app descendant tree: a WebView2 browser and renderer use the fresh fixture profile, and the browser receives a remote-debugging-port flag. A subsequent TCP listener check at the inspected port returns zero listeners. No proxy environment is configured for Node. These observations do not identify the cause or prove the app's WebView2 debugging policy.

Both runs finish their scripted owned-tree termination, port bind checks and log flush without reporting cleanup errors. No provider generation, credential, Calendar or user profile is used. Safe ignored output: `target/e2e-tools/p4-matched-attach-2026-10-06.log` and `p4-attach-flags-2026-10-06.{log,err}`. Fixture process logs remain in OS-temp `arcana-webview-attach-8wWKKl` and `arcana-webview-attach-aQtP0R`. Do not treat absent cleanup errors as WebDriver or consent success.

The owner is asked to authorize a separate test-only build with an explicit Tauri browser-argument override and browser diagnostics, preserving shipped settings. Permission is pending. The matched endpoint/driver pair remains unproven; longer timeouts alone are not proposed as a fix.
