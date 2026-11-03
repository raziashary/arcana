# TG4-30 / TG4-25 claim-boundary falsification sweep — 2026-10-18

Commit: `821b20f` (`test(p4): pin TG4-30 claim boundary and fix operation-journal tool identity`).

Three enforced-but-untested TG4-30 boundary clauses now have named refusal tests through the
production ToolExecutor / production tools, plus one journal-identity defect fix:

1. **No-claim worker writes are refused at the executor boundary.**
   `arcana-core::agent::builtin_tools::write_file::workspace_tests::production_tool_executor_refuses_worker_writes_without_a_current_claim`
   — a worker with no task claim and a worker holding a foreign claim are both refused with
   bounded versioned JSON envelopes (`schema_version:1`) before any parent directory or file is
   created. Command: `cargo test -p arcana-core --lib production_tool_executor_refuses_worker_writes_without_a_current_claim --locked` → 1/1 (2026-10-18, exit 0).
2. **A worker cannot release another worker's dependency chain.**
   `update_task_status_tool::qc_gate_tests::worker_cannot_release_another_workers_dependency_chain`
   — cross-worker `done` is denied; the denied completion leaves the dependent task held
   (`needs_info`), not unblocked. Command: `cargo test -p arcana-core --lib worker_cannot_release_another_workers_dependency_chain --locked` → 1/1.
3. **Batch assignment intents journal the registered tool identity.**
   `create_task::p4_51_board_matrix_tests::batch_assignment_intents_journal_the_registered_tool_identity`
   — the intent is journaled as `assign_tasks` (the registered FastDevelopment name) instead of a
   retired `assign_tasks_batch` alias that fell through to `external_tool`, restoring the
   operation/tool identity the P4-53 reconciliation contract promises. Auto-follows from the
   fix in `tool_executor.rs::begin_operation_journal`. 1/1.
4. **`annotate_file` claimlessness is ruled deliberate** (ADR-LOG-RESTRUCT-0396): it mutates only
   `annotations.db`; a worker-claim gate would refuse the shipped QC reviewer flow. Its defenses
   are workspace-relative normalization + `path_jail`, the executor journal, and redaction.
   Record exists so TG4-30's "file/process writes" clause is not misread as satisfied for it.

Validation: `cargo test -p arcana-core --lib --locked -- agent::tool_executor agent::builtin_tools::create_task qc_gate_tests tool_result_error_tests --test-threads=4` → 44/44, exit 0 (2026-10-17). `cargo fmt --all -- --check`: all touched files clean (remaining drift is the documented untracked `platform/process_sandbox/linux_smoke/staging.rs` and `src-tauri/{settings,updater,vault}_commands.rs` WIP files).

Residual: the full boundary matrix still needs its `_verify` legend papering (P4-56 step D), and the pre-existing co-run interference between the three shipped lost-ack tests (individually 1/1 per receipt; see TG4-27 flag file) is a separate suite-hygiene item.
