# P4-53 Journal Path Review — 2026-10-12

Scope: current diffs in `arcana-core/src/agent/builtin_tools/path_jail.rs` and `arcana-core/src/agent/tool_executor.rs`; the specifically named runner test only if present.

## Findings

Review in progress.

## Scoped assessment

No confirmed defect found in the reviewed diff. `journal_target_paths` still resolves the supplied workspace with the existing workspace helper, resolves relative inputs under that root, and calls `validate_workspace_path` before deriving a journal reference (`tool_executor.rs:171-203`). Thus invalid/out-of-workspace paths still produce no reference and this diff does not broaden handler file access. For accepted paths, the jail canonicalizes and de-verbatimizes both target and workspace; the executor now applies that same de-verbatimization to its independently resolved root before `strip_prefix`. This aligns Windows drive and UNC spellings, while the helper preserves UNC as `\\server\share\...` (`path_jail.rs:137-160`) and retains the existing component-boundary containment check (`path_jail.rs:163-200`). Non-Windows behavior is unchanged by the helper.

The named `shipped_worker_preserves_multiple_mutations_and_quarantines_lost_acknowledgement` test (`recipes/runner.rs:1643`, assertions at 1805-1809) drives two real relative-path writes through the worker and asserts the journal keeps `acknowledged.txt` and `lost-ack.txt`, with the latter unresolved after lost acknowledgement. Existing path-jail unit tests check verbatim UNC conversion and drive-path stripping. I did not execute tests or builds.

Remaining proof limit: the named journal consumer test uses a local temporary workspace and does not exercise `journal_target_paths` with an actual UNC workspace. Existing tests cover the shared UNC normalization helper, but not the journal's post-validation relative-reference extraction for a UNC root. No claim of UNC end-to-end execution is made.
