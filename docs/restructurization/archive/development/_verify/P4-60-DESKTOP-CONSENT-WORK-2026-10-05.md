# P4-60 desktop consent preparation — 2026-10-05

The existing WebDriver harness is the production UI/IPC path. Prepared a scenario that grants through Settings, reads native persisted consent, remounts Settings, switches to a second owned workspace, returns and revokes through the UI. It uses the always-available GitHub MCP consent button but never invokes MCP or a provider. The harness creates a disposable HOME/USERPROFILE, XDG/WebView storage and two workspaces, clears live key/database environment values and retains only fixture logs. No user Calendar path is accessed.

Native driver prerequisite installation awaits the owner decision under universal Rule 4. No desktop run or P4-60/TG4-16 closure is claimed. Source type validation is next; cleanup and UI execution require actual driver verification. Official Tauri WebDriver and Microsoft WebView2 references are recorded in ADR-0301. No new package/library or shipped interface is added by preparation.

## Approved prerequisite work

Owner approved local Windows driver installation. Tools remain under ignored target/e2e-tools and PATH is command-local. Edge/WebView2 build is 154.0.4258.53. Removed stale WDIO capability typing (`maxInstances` belongs at top level; `tauri:options` needs no suppression in current types). Guarantee environment restoration even if driver cleanup fails. No acceptance claim before the built desktop is exercised.

## Paused result — 2026-10-05

Driver installation was approved and completed locally. Built native app and the real Settings/native-IPC consent scenario passed (three runs); the scenario now includes application restart plus native structural diff refusal with no provider grant. Consent method matrix passes Windows/Linux 6/6; native diff helper tests 3/3 and local MCP invocation/revocation 1/1. Full harness still fails its first fresh-profile session with missing DevToolsActivePort. Neither deadline increase nor userDataFolder alignment fixed it; the explicit-port diagnostic was deliberately stopped before a result. No P4-60/TG4-16 closure, new commit or push. See the final paused section in P4-SESSION-CHECKPOINT-2026-10-05.md for exact files, tests, raw logs, hypotheses retracted, remaining decisions and cleanup. All owned driver/app processes and ports were stopped.

## Resumed consent work

Owner resumed core P4 and requested explicit decision questions. Seven outstanding decisions were presented; no dependent contract changes or paid trial proceed before answers. Next authorized proof is the existing built-desktop fresh-profile harness, with the previously interrupted explicit-port diagnostic. Main owns harness edits and runs. A Luna High read-only reviewer may inventory consent consumers and identify exact unmet TG4-16 clauses; it may not write, run tests, access credentials, or declare closure. No Calendar or expanded-goal access.


Resumed explicit-port full run: consent scenario passed (9.5s), but initial application-launch session failed with chrome not reachable after the native attach deadline. The dynamic port is not a fix and will be removed unless new evidence supports it. Raw log target/e2e-tools/p4-desktop-resumed-explicit-port.log. Next independent core units follow owner decisions ADR-0302.


ADR0303 resolves the inventory finding: explicit metadata-only Settings checks are permitted with privacy-safe audit, while workspace-bearing operations keep consent. Separate authorized logging correlation unit begins under ADR0302; its writer owns only logging.rs and its integration test.

