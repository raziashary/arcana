# P4-53: process-loss quarantine of an uncertain file write

The focused Windows test `dead_worker_process_keeps_unknown_write_for_supervisor_handoff` launches a separate test process with an isolated Arcana home and workspace. That process claims a task with its real PID, binds the task to a durable worker run, invokes the registered `WriteFileTool` through `ToolExecutor`, and injects a SQLite outcome-write failure after the file mutation. It then exits without graceful worker cleanup.

The parent reopens the stores and invokes the production interrupted-run recovery. The test reads back the written file, task-scoped checkpoint, unresolved journal intent, interrupted task-bound run, and the task held in `needs_info` with its exact claim released. It does not infer that the missing acknowledgement means the write failed.

Validation: `cargo test -p arcana-core --lib dead_worker_process_keeps_unknown_write_for_supervisor_handoff -- --nocapture` passed 1/1 on Windows. `git diff --check` passed. The changed scheduler file has no `cargo fmt --check` diff; the workspace-wide format check remains red on unrelated pre-existing/uncommitted files.

This proves the process-loss quarantine slice only. P4-53/TG4-27 remain open for a complete restarted-Arcana Supervisor handoff after process loss, multiple effect classes, conflict refusal, and task-scoped restore. P4 stays 49/59; TG4 stays 24/33. No provider call or Calendar test was run.
