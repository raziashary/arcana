# 2026-09-27 Cross-Phase Arcana Harness Audit

## Scope and closeout rule

Three independent read-only lanes reviewed the P1/P2/P3 source and readiness
claims, P8 retained coding paths, and the default fleet workflow. All audit
agents were GPT-6 Luna High. Their reports are evidence inputs, not gate
completion. P4 does not close until each finding has either (a) a production
consumer and behavior proof in P4, or (b) an explicit later owner and an open
gate. Historical phase checkboxes are not rewritten to make this review look
green. The current owner direction is ADR-0074; the P8 host-enforcement choice
is ADR-0075.

## Disposition by audit lane

| Lane | Finding | Action / owner | State |
|---|---|---|---|
| P1 | `graph_tools.rs` exposes tool-shaped structs but no `Tool` implementations or registry entries. No coding turn can call them. | Keep graph access unavailable until P5 implements and proves a real tool consumer. `TG1-01` remains open; P5 is the delivery owner. | Open; not counted as P1 complete |
| P1 | Readiness still asks for a recheck after the Arcana rename, while historical P1 evidence points at old paths and runtime state. | Readiness now records the rename as present and requires current Arcana consumer/runtime proof where the old evidence cannot establish it. Preserve historical evidence with its date. | Documentation corrected; remaining gates open |
| P2 | TG2-01/03/06/08 provider contracts, generated argument edges, opaque replay, and cross-provider coverage remain incomplete. | Keep each listed gate open with P2 as owner. The current P4 fix only prevents recipe worker model IDs from crossing provider boundaries; it does not close the wider P2 matrix. | Open; P2-owned |
| P2 | TG2-12/13 restart and UI-to-wire proof, TG2-14 removal proof, and TG2-17 spend-ceiling proof are still missing. | Keep readiness amber/open and run those production-reader proofs in P2's register. | Open; P2-owned |
| P3 | Memory v3 indexing/provenance/redaction source and focused tests are present, but eight memory gate clauses and several path/effect gates lack current production-path proof. | Keep the P3 source fixes; do not infer reader proof from writer or unit-test presence. The exact open gates remain in `TESTING_GATES.md` under P3; P3 owns completion. | Open; P3-owned |
| P3/P4 | Persisted app-mediated workspace egress must be read by every provider/diff/QC consumer. | P4 continues to own TG3-26/TG4-16 under ADR-0068/0073. The provider wrapper, QC fallback, Tauri Settings grant/revoke commands, and direct-diff no-consent fallback now have focused consumer tests; Tauri PR-diff and provider-assisted diff request/log/revoke proof remain open. | Partial; P4-owned and blocking |
| P4 | Shipped coding-worker recipes called `update_task_status` for claims and Bash for builds, while recovery requires atomic claims and QC reads task-scoped Rust build evidence. | Default worker prompts now call `claim_task` and `project_command`; a file-backed Cargo command test proves persisted evidence survives database reopen and the QC reader consumes it. P4-12 is now closed; P4-13 is separately closed through the shipped status tool. | Implemented; TG4-07 remains open for PowerShell/cmd, three-platform, and desktop worker/QC behavior |
| P4 | The `UpdateTaskStatusTool` evidence guard was bypassable: stranded-task reconciliation and Tauri manual status updates call `TasksDb::update_status` directly. | Reopened P4-14/TG4-07. Owner choice of the shared enforcement boundary is pending; do not close QC evidence until a negative test proves both direct production paths cannot promote without matching passing evidence. | Open; owner direction required |
| P4 | `ide:navigate` had a graph-command emitter but no frontend consumer, and the city context menu supplied the wrong argument name. | Added an app-root event listener, workspace-boundary validation, IDE tree expansion/open behavior, and corrected the city command arguments. Registered event and store behavior tests pass; the end-to-end artifact-to-run smoke remains open under P4-21/TG4-09. | Partially resolved; focused proof passes |
| P4 | Startup rewrote customized system recipes. | `ensure_default_recipes` now creates missing files exclusively and preserves existing system recipe files; a readback test proves the prompt survives startup and missing defaults still seed. | Implemented; focused test passes |
| P4/P2 | Seeded Claude worker model IDs could be sent through Google after a provider switch. | Recipe resolution accepts only catalogue models matching the active provider and falls back to that provider's catalogue default; focused no-mismatch tests cover explicit and default paths. | Implemented; focused test passes |
| P4 | Supervisor had no instruction to use epic/phased-task tools for multi-phase requests. | Default Supervisor recipe now defines the visible epic → phased children → batch dispatch path; default recipe test checks these tools and the worker/tool protocol. | Prompt consumer implemented; TG4-05's real Supervisor-turn proof remains open |
| P4 | Fleet smoke recipe used ad hoc install/build shell commands. | It now requires explicit `project_command` install intent and Rust-owned build intent; unsupported install managers fail clearly rather than silently switching to shell. | Prompt consumer implemented; build smoke still open |
| P4 | Selecting a monorepo root missed nested package manifests and a toolchain selector silently chose only the first matching project. | Rust discovery now scans nested workspace folders with a bound and generated-tree pruning, folds declared Cargo workspace members into their root, and selects all matches for the validated ecosystem. The fixture and actual Arcana mixed-workspace tests pass. | Focused Windows behavior proven; three-OS command proof remains open under P4-11/TG4-18 |
| P4 | Additional delivery and lifecycle gaps from the workflow audit: workspace resolver fallback, task-associated checkpoints and surfaced checkpoint errors, durable ACP replay/ack, stale-message recovery consumer, task-budget reader, deterministic routing threshold, system-worker routing exclusion, and full consent/QC readback. | Keep P4 tasks and TG4-02/05/07/09/10/11/12/13/14/15/16/19 plus S4 smokes open until each real reader and failure path is exercised. The P4 checkpoint and readiness row remain the active task list. | Open; P4-owned |
| P6 | First task discovery and project selection are hidden behind Chat mode and Settings; New Task is disabled, and tool browser can misstate effective mode tools. | Record for P6 coding UI/onboarding work. Do not widen the P4 implementation without a P6 task. | Open; P6-owned |
| P8 | Commit safety was worker prompt text; the prior smoke executed raw Git separately and did not prove the host could stop a bypass. | User directed host enforcement. Registered `git_commit`, added the current-tree path lock, staged-blob scan, malformed-input checks, expanded textual Bash mutation guard, and updated gate assertion; ten unit tests, TG8-13, and S8-06 passed on 2026-09-28. The new P8 hardening is not yet committed; `fff31ec` is the earlier pushed baseline. | Reverified closed on current tree; process-local lock and textual Bash limits remain documented |
| P8 | ADR-0059 retained a prose `DELEGATE` parser obligation superseded by structured subagent tools; a tool-executor comment referenced a removed path. | P8-73 and ADR-0075 record that structured tool calls supersede the prose parser; the stale live-path comment is refreshed. | Resolved in pushed P8 closeout |

## 2026-09-28 revalidation of fleet review findings

Two additional independent GPT-6 Luna High reviewers examined the ACP/QC
production workflow and in-process run recovery. Their source findings were
checked against the working tree and are reconciled as follows:

| Finding | Follow-up and evidence | Current disposition |
|---|---|---|
| Installed Supervisor/QC prompts can miss inbox and stale-mail protocol updates. | `ensure_default_recipes` now performs a narrow idempotent prompt migration; `installed_supervisor_and_qc_recipes_receive_inbox_protocols_without_losing_custom_text` proves the instructions arrive and custom text survives. | Implemented and focused-tested; full scheduling smoke remains open. |
| Deleted workers can remain hard-coded recipients and broadcasts. | Message recipients and broadcasts now use the current persisted active-worker roster. `deleted_system_worker_is_not_a_message_recipient_or_broadcast_target` proves an inactive QC worker is excluded. | Implemented and focused-tested; real message retry behavior remains part of S4-04. |
| QC inbox tests do not prove the persistent QC worker's shipped recipe consumes inbox-only work. | `shipped_qc_recipe_handles_inbox_only_work_through_agent_engine` checks the persisted worker is bound to `qc-dispatcher`, runs the shipped recipe through `AgentEngine`, reads and acknowledges the message, then confirms no pending inbox item remains. | Core consumer proven; event-kick scheduling and automatic worker redispatch after the Supervisor stale-mail nudge remain open under P4-35/P4-42. |
| Stale Supervisor mail reader was tested without the recovery action. | The shipped Supervisor AgentEngine test now consumes the stale-reader message/task IDs, persists a QC nudge through `send_worker_message`, then continues normal dispatch and epic creation. | Reader-to-action consumer proven; automatic worker redispatch after the nudge remains open. |
| Interrupted-run cleanup could leave worker status stuck, panic could terminate the scheduler task, and no test proved a next real tick. | Claim release must succeed before worker status resets; on task-DB failure the worker remains `working` so reconciliation can retry. Run futures catch panic and timeout. Isolated panic/timeout/cancel tests, `task_db_recovery_failure_keeps_worker_working_for_the_next_recovery_pass`, and `recovered_supervisor_state_reaches_a_real_recipe_runner_on_the_next_tick` exercise retryability and subsequent real `RecipeRunner` entry. | Partial P4-36 proof; direct scheduled-loop DB-failure recovery and full S4-03 automatic redispatch smoke remain open. |
| Idle-call instrumentation was synthetic and did not measure the runner. | Empty-queue `RecipeRunner` test now exits before provider resolution; scheduler helper test shows no provider closure runs when no work exists. | Partial P4-43 proof; event/cron call measurement and intentional-turn bound remain open. |
| Consent review did not prove Tauri consumers or QC deterministic fallback. | QC now checks persisted consent before provider key construction. `qc_diff_without_matching_consent_uses_structural_review_without_provider_key` proves mismatched consent returns deterministic structural review without credentials. | Partial TG3-26/TG4-16 proof; actual `AnalyzeDiffTool` Git path, direct Tauri diff commands, and grant/revoke consumer smoke remain open. |

2026-09-28 follow-up: `consent_commands_persist_and_revoke_the_selected_workspace_grant`
proves the production Tauri Settings commands read, grant, and revoke the
configured workspace consent. `direct_diff_command_uses_structural_result_without_matching_consent_or_provider_key`
proves the Tauri diff path falls back locally before key lookup when the active
provider does not match the persisted grant. Tauri PR-diff behavior and the
`AnalyzeDiffTool` repository-diff path remain unproven, so TG3-26/TG4-16 stay
open.

## Proof executed in this continuation

- `cargo fmt --all` passes after the current edits.
- `cargo test -p arcana-core --lib platform::command::tests --locked -- --nocapture`: 12 passed.
- `cargo test -p arcana-core --lib agent::builtin_tools::project_command::tests --locked -- --nocapture`: 6 passed, including Arcana's root Cargo+Node detection and persisted task-keyed QC evidence.
- `cargo test -p arcana-core --lib recipes::defaults::worker_command_protocol_tests --locked -- --nocapture`: 2 passed, including startup recipe readback.
- `cargo test -p arcana-core --lib recipes::runner::p3_65_runner_tests --locked -- --nocapture`: 4 passed, including provider/model compatibility.
- `cargo test -p arcana-core --lib agent::builtin_tools::bash::git_mutation_guard_tests --locked -- --nocapture`: 1 passed.
- `cargo test -p arcana-core --lib agent::builtin_tools::git_commit::tests --locked -- --nocapture`: 4 passed, including two sequential fleet commits and protected trunk refusal.
- `cargo test -p arcana-core --test s8_git_safety --locked -- --nocapture`: 1 passed through the actual `Tool` interface in a throwaway Git repo.
- `cargo test -p arcana --lib consent_commands_persist_and_revoke_the_selected_workspace_grant --locked --quiet`: 1 passed against isolated temporary Arcana data.
- `cargo test -p arcana --lib direct_diff_command_uses_structural_result_without_matching_consent_or_provider_key --locked --quiet`: 1 passed; no provider credentials or live provider request.
- `cargo test -p arcana-core --lib platform::command::tests --locked --quiet`: 13 passed after nested workspace discovery was added.
- `cargo test -p arcana-core --lib agent::builtin_tools::project_command::tests --locked --quiet`: 7 passed, including multiple nested packages under a validated selector.

These focused results do not close TG4-05/07, TG3-26/TG4-16, TG8-13, S8-06,
or the P1/P2/P3 gates listed above. The next proof pass must run the current
Arcana gate suite, workspace validation, and the P4 end-to-end smoke candidates,
then update this disposition table with source, consumer, and test evidence.

### Consent-path correction — 2026-09-28

The preceding table and follow-up are stale for the `AnalyzeDiffTool`
repository-diff path. The later
`analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key`
test drives that production consumer against an isolated Git diff, verifies
deterministic fallback without provider-key lookup, and reads the denied
egress-audit row. Tauri `analyze_pr_diff` invocation and full allowed/revoked
consent behavior across desktop consumers remain open under TG3-26/TG4-16.
