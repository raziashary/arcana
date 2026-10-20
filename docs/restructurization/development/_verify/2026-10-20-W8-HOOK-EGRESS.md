# W8 — Hook egress consent and log-path confinement

**Date:** 2026-10-20
**Decision:** `ADR-LOG-RESTRUCT-0400`
**Owner authorisation:** W8 approved as a security fix ("persisted hooks must obey the
same consent and jailing as provider/MCP traffic and the Shell hook action").

## Defect

`HookAction::Http` expanded its URL through `substitute(url, ctx)`. `HookContext` carries
`tool_call.input`, which is model-produced. A hook the user authored for one URL could
therefore be made to send to a destination chosen by the model, with no consent check —
the only egress path in the product without one.

## Change

| Layer | Change |
|---|---|
| `arcana-core/src/egress_consent.rs` | `hook_destination(id)` → `hook:<id>`, admitted by `validate_destination` **by shape only** (`hook:` + ≤64 id-shaped chars). URLs, paths and free text remain refused, so the destination allow-list stays closed. |
| `arcana-core/src/hooks/types.rs` | `HookContext.workspace_root: Option<String>` (`#[serde(default)]`) — consent is workspace-scoped and the executor had no workspace identity. |
| `arcana-core/src/hooks/executor.rs` | `Http`: after template expansion, `authorize_and_audit(workspace, "hook:<id>", "hook_http")`; refusal on `false`, on error, and on absent workspace **before** a client is built. `Log`: empty and parent-traversing paths refused. |
| firing sites | `engine.rs` ×3, `tool_executor.rs` ×2, `write_file.rs`, `edit_file.rs`, `read_file.rs`, `assign_task.rs`, `update_task_status_tool.rs` populate `workspace_root`; `delete_session` passes `None` (no workspace exists — HTTP hooks there are refused, not assumed allowed). |
| `src-tauri/src/commands/egress_consent_commands.rs` | `scope()` routes `hook:` through `hook_destination`; no new command. |
| `src/tabs/Settings.tsx` | `WorkspaceEgressConsentCard` lists each HTTP hook by name, destination `hook:<id>`, using the existing grant/revoke commands. |

## Evidence

New tests (6/6 passing):

```
cargo test -p arcana-core --lib -- --test-threads=1 egress_boundary hook_destination
test egress_consent::hook_destination_tests::hook_destinations_are_id_shaped_and_everything_else_is_refused ... ok
test hooks::executor::egress_boundary_tests::an_http_hook_without_consent_is_refused_before_any_request ... ok
test hooks::executor::egress_boundary_tests::a_granted_hook_destination_passes_the_consent_gate ... ok
test hooks::executor::egress_boundary_tests::an_http_hook_without_a_workspace_is_refused ... ok
test hooks::executor::egress_boundary_tests::a_log_hook_refuses_upward_traversal ... ok
test hooks::executor::egress_boundary_tests::a_log_hook_still_writes_an_explicit_path ... ok
test result: ok. 6 passed; 0 failed
```

Frontend: `npx vitest run src/tabs/Settings.test.tsx` → 3/3 passing, including a new case
that grants an HTTP hook by `hook:<id>` and ignores a non-network hook.
`npm run typecheck` → no errors.

**Falsification (the gate can fail).** The denial arm was temporarily widened to
`Ok(true) | Ok(false) => {}` and the suite re-run:

```
test hooks::executor::egress_boundary_tests::an_http_hook_without_consent_is_refused_before_any_request ... FAILED
the refusal must come from the consent gate, not from the network:
error sending request for url (http://127.0.0.1:9/should-never-be-contacted)
test result: FAILED. 4 passed; 1 failed
```

With the gate bypassed the request is attempted (unroutable port → network error), which
is exactly what the test forbids. Reverted; the suite is green again. The granted-path
test is the complement: it proves the gate is not a blanket refusal.

## Corrected finding

The audit's S2 said `HookAction::Log` was "unjailed" relative to Shell. It is not comparable:
`shell_portability::validate_working_directory` is a **portability** check (rejects `/tmp/`),
not a jail — the sandbox confines a shell hook, and Arcana's own file I/O has no sandbox.
There was no jail rule to mirror. What landed is the confinement actually available: the
path is user-authored, is not substituted, and may not traverse upward.

## Regression sweep

`cargo check --workspace --all-targets --locked` → Finished: every target compiles, which is
the check that matters most for a new struct field (it proves no `HookContext` construction
site was missed).

`... --test-threads=4 hooks:: egress_consent:: builtin_tools:: tool_executor::` → **273
passed, 2 failed**. Both failures are in files this change does not touch
(`git_commit::tests::registered_tool_git_commit_success_and_guard_failure_reach_executor`,
`project_command::tests::checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher`),
and both pass in isolation at `--test-threads=1` (0.92s, 207.98s). They are the
order/concurrency coupling `W5` exists to fix — recorded, not waved away. A full-lib run at
`--test-threads=4` was attempted first and exceeded a 40-minute window; no processes were
left behind.

`cargo fmt --all -- --check` → only the known owner-WIP
`platform/process_sandbox/linux_smoke/staging.rs:572`, untouched.

## Residuals (named, not implied solved)

1. **Infra failure vs policy denial** in `registry.rs::fire` (`false`) / `fire_with_logs`
   (`vec![]`) is unchanged — W8's second half, tracked as P4-51.
2. **No workspace jail for Arcana-authored file writes.** The Log path is confined against
   traversal and model influence, not against a user-authored path outside the workspace.
   This is a product decision about what a hook may write, not a bug to patch.
3. A `SessionEnd` hook (session deletion) has no workspace, so an HTTP hook there is
   refused. Deliberate: no consent scope means no egress.
