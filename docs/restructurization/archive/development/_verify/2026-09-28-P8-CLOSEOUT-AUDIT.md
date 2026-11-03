# P8 Closeout Audit

- Scope: Independent read-only audit of the P8 native coding conversion and reopened host commit safety closeout, focused on MDP P8/P8-74, TG8-13, S8-06, ADR-0075, and current host/Bash/registry/recipe source.
- Date: 2026-09-28
- Progress: Source and plan tracing complete. Audit conclusions below are source-based; no tests, app, server, live DB/provider, secrets, or Git state were accessed.
- Constraints: Source, plans, gates, and Git remain untouched; only this report was written.

## Findings

### Overall verdict

**Partial.** The host commit tool is registered on the actual worker engine path, default worker recipes consume it, normal traversal and symlink escapes are refused, branch checks cover the literal names `main`, `master`, and `trunk`, and the tested clean-index named-file path has meaningful positive/negative smoke coverage. The source does not justify treating Bash as a general staging/commit sandbox, secret scanning as exhaustive, index rollback as safe under concurrent workers, or the message parser as enforcing the documented blank-line contract. I did not run the recorded test commands, so test status here is not independently revalidated.

### Production registry and worker consumer — CONFIRMED with end-to-end limit

`AgentEngine::new` calls `builtin_tools::register_for_mode`; `register_for_mode(FastDevelopment)` registers `GitCommitTool`. `RecipeRunner::run_recipe` constructs `AgentEngine` in `FastDevelopment` and runs the recipe. The scheduler calls `run_recipe`, and worker defaults share a Commit Protocol containing a `git_commit(...)` call. `src-tauri/src/ci_gate.rs` separately seeds and loads default recipes and checks every `FLEET_WORKERS` recipe for the call. This is a real production wiring path, not an unused registry symbol.

The S8-06 integration test invokes the actual `BashTool` and `GitCommitTool` in an isolated repository, but does not run a model-produced tool call through `AgentEngine` and the scheduler. TG8-13's registry/recipe assertions are source and configuration checks. Thus registry and recipe wiring are confirmed from source; a live worker-turn consumer is not part of the cited smoke.

Evidence: [engine.rs](../../../../arcana-core/src/agent/engine.rs) (constructor and registry dispatch); [builtin_tools/mod.rs](../../../../arcana-core/src/agent/builtin_tools/mod.rs); [runner.rs](../../../../arcana-core/src/recipes/runner.rs); [scheduler.rs](../../../../arcana-core/src/recipes/scheduler.rs); [defaults.rs](../../../../arcana-core/src/recipes/defaults.rs); [ci_gate.rs](../../../../src-tauri/src/ci_gate.rs); [s8_git_safety.rs](../../../../arcana-core/tests/s8_git_safety.rs).

### Protected trunk aliases — PARTIAL

The implementation reads `repo.head().shorthand()` and rejects exact branch names `main`, `master`, and `trunk`; detached or unborn HEAD is also rejected. This closes the literal `trunk` alias called out in the 2026-09-28 strengthening. It does not resolve the repository's configured default branch or `origin/HEAD`, protect custom default names such as `stable`/`production`, or treat case variants as aliases. The shipped Commit Protocol still says only `main` and `master` are refused, so its prompt-level statement is stale against the tool and current MDP/gate.

Evidence: [git_commit.rs](../../../../arcana-core/src/agent/builtin_tools/git_commit.rs); [defaults.rs](../../../../arcana-core/src/recipes/defaults.rs) Commit Protocol; MDP P8-74 and TG8-13; ADR-0075.

### Path traversal and symlinks — CONFIRMED for stable filesystem state; race remains

The validator rejects absolute paths and any component other than `Normal`, rejects directories, canonicalizes existing targets and missing-target parents, and requires canonical paths to start under the canonical repository root. This covers ordinary `..` traversal and stable symlink escapes (including symlinked parents).

Validation and later `index.add_path`/`remove_path` are separate pathname operations. There is no handle-relative staging or revalidation at the staging point, so a concurrently changed symlink/path can race the check. The executor also silently drops non-string members from `paths` with `filter_map`; a malformed array can therefore commit the remaining strings instead of being rejected as a whole.

Evidence: [git_commit.rs](../../../../arcana-core/src/agent/builtin_tools/git_commit.rs), `execute`, `validate_path`, and staging loop.

### Unrelated staged changes — CONFIRMED sequentially; PARTIAL for shared concurrent workers

Before staging, `index_changed_paths` compares the index with HEAD and refuses any existing staged delta. After staging, the tool compares changed path names with the explicit set, which prevents a clean sequential call from committing unrelated unstaged files. The S8 smoke checks `.env` and generated output remain outside the committed tree.

There is no repository-wide commit/index lock. `AgentEngine`'s executor serializes Execute-effect calls within one tool batch, but separate worker sessions share the configured workspace and can execute independently. Two simultaneous commit calls can both observe an empty index, stage into the same index, and race their set checks/commit. A failure rollback can also reset another worker's staged changes. The fleet's shared checkout is expressly the recipe architecture, so this is a material concurrency hole in the sequential safety proof.

Evidence: [git_commit.rs](../../../../arcana-core/src/agent/builtin_tools/git_commit.rs); [tool_executor.rs](../../../../arcana-core/src/agent/tool_executor.rs) (`Execute` calls are partitioned from concurrent-safe reads); [runner.rs](../../../../arcana-core/src/recipes/runner.rs) (shared workspace resolver and per-run engine); [defaults.rs](../../../../arcana-core/src/recipes/defaults.rs) (shared fleet branch/check-out protocol).

### Refusal and index rollback — PARTIAL

Protected-branch, path-validation, and pre-staged refusals occur before this call's staging and leave the index alone. Once `index.write()` succeeds, errors in the closure (path-set mismatch, diff inspection/redaction, tree/parent/signature/ref/commit errors) call `reset_index_to_head`, which uses a mixed reset: index staged deltas are cleared while the worktree is retained. This supports the tested credential-refusal rollback.

The guarantee is narrower than “every error after staging restores the index”: `add_path`/`remove_path` and `index.write` can return before the rollback closure is entered. More materially, reset is repository-wide and will erase concurrent staged deltas, not just this call's entries. The recorded seven-unit-test summary includes a credential rollback case, not a demonstrated failure at each post-staging operation. S8's trunk refusal is pre-staging and therefore does not exercise rollback.

Evidence: [git_commit.rs](../../../../arcana-core/src/agent/builtin_tools/git_commit.rs), staging loop, closure, and `reset_index_to_head`; [s8_git_safety.rs](../../../../arcana-core/tests/s8_git_safety.rs); MDP P8-74 and TG8-13.

### Sensitive-path and secret-diff checks — PARTIAL

The tool blocks basenames `.env` and `.env.*`, except `.env.example`, and rejects the commit when `redact_text(patch) != patch`. ADR-0075 correctly limits the claim to credential patterns recognized by the scanner. The scanner documentation itself says free-text redaction requires assignment context, a recognizable token shape/auth scheme, or URL credentials; a bare high-entropy value without such context is not caught. The checked patch is rendered as a textual unified diff, so binary payload contents are not inspected as raw bytes. Filename checks are case-sensitive and are not a complete sensitive-file policy (`.envrc`, `secrets.json`, etc. are not blocked by name alone).

Evidence: [git_commit.rs](../../../../arcana-core/src/agent/builtin_tools/git_commit.rs); [diff.rs](../../../../arcana-core/src/git/diff.rs), `render`; [redaction.rs](../../../../arcana-core/src/security/redaction.rs), module contract; ADR-0075.

### Bash shell-syntax guard — PARTIAL; not a sandbox

The guard runs before shell launch, splits command text at `&`, `|`, `;`, and newline, normalizes wrappers/quotes and path-qualified executable names, and scans up to five following words for `git add` or `git commit`. Its unit examples cover direct commands, `git.exe`, `git -C`, `cmd /c`, `sh -c`, and a Python subprocess literal. The source and ADR explicitly state it is textual heuristic coverage, not process isolation.

It does not reject Git's equivalent staging/ref mutation commands such as `git update-index --add` or `git update-ref`, and code can run Git through an encoded payload, external script, compiled helper, or other arbitrary process API. Those are straightforward semantic bypasses beyond command spelling. Therefore the narrow direct `git add`/`git commit` refusal is confirmed; the stronger “cannot stage or commit through Bash” reading is refuted unless constrained to the covered textual spellings/wrappers.

Evidence: [bash.rs](../../../../arcana-core/src/agent/builtin_tools/bash.rs), guard and tests; [tool_executor.rs](../../../../arcana-core/src/agent/tool_executor.rs); [ci_gate.rs](../../../../src-tauri/src/ci_gate.rs); ADR-0075.

### Commit-message validation — PARTIAL

The validator requires a first line with a lowercase/hyphen type, `: `, and nonblank summary; it accepts an optional parenthesized scope with only weak scope checks; and it requires nonblank text after the subject. It does not require a blank line before the body even though the tool description, recipe example, and ADR say “subject followed by a blank line and a descriptive body.” A message such as `fix: summary\nBody text` passes. It also accepts scope text containing spaces/punctuation because only closing `)` and length are checked. The meaningful-body requirement is confirmed; strict Conventional Commit structure is not fully enforced.

Evidence: [git_commit.rs](../../../../arcana-core/src/agent/builtin_tools/git_commit.rs), `validate_commit_message`; Commit Protocol in [defaults.rs](../../../../arcana-core/src/recipes/defaults.rs); ADR-0075.

### Platform and OS proof — PARTIAL / NOT ESTABLISHED by source audit

`BashTool` dispatches Windows commands through `cmd /C` and other platforms through `sh -c`; the same text heuristic is applied before either shell. The S8-06 test exercises the real host tool and Bash direct-command refusal against libgit2's temporary repository, but the source does not establish a Windows/Linux/macOS matrix for this closeout. The smoke also asserts `Repository::init` chose `main` or `master`, so a host-level custom `init.defaultBranch` (for example literal `trunk`) can make that fixture fail independently of host refusal behavior. Current code's path semantics follow the host filesystem; cross-platform case and symlink behavior are not behaviorally demonstrated by the cited closeout test.

Evidence: [bash.rs](../../../../arcana-core/src/agent/builtin_tools/bash.rs); [s8_git_safety.rs](../../../../arcana-core/tests/s8_git_safety.rs); [TESTING_GATES.md](../TESTING_GATES.md) TG8-13; MDP P8-74 and S8-06.

### Documentation consistency — REFUTED in one worker contract

~~MDP P8-74, TG8-13, ADR-0075, and the readiness row describe the 2026-09-28 literal `trunk` protection and the documented textual-guard limit. The shared worker recipe's Commit Protocol still says only `main`/`master`; the `git_commit` tool description also lists only `main/master`. These are stale descriptions under the project no-stale rule. This report does not change those records.~~

**Correction (2026-09-28):** This paragraph was stale against the live tree when
the coordinator appended the remediation. Both the shared Commit Protocol and
the `git_commit` tool description already name `main`, `master`, and `trunk`.
The no-stale update was missed in this report; ADR-0086 records the correction,
and TG8-13 now asserts that each shipped worker recipe contains the protected
alias instruction.

Evidence: [defaults.rs](../../../../arcana-core/src/recipes/defaults.rs); [git_commit.rs](../../../../arcana-core/src/agent/builtin_tools/git_commit.rs); MDP P8-74; [TESTING_GATES.md](../TESTING_GATES.md); ADR-0075.

## Scope and closeout note

This report independently assessed source claims and recorded material limits. It does not amend plan rows or mark P8 closed/open. No test, application, server, live resource, provider credential, secret value, or Git command/state was accessed.

## Coordinator disposition after audit (2026-09-28)

The coordinator verified the findings and implemented the P8-74 remediation in
the current worktree. A process-wide Arcana path lock now serializes each
in-process worker's complete index transaction; malformed path-array entries
are rejected; secret scanning reads staged blobs (so binary contents and the
actual committed bytes are checked); and Conventional Commit validation
requires the documented blank line. Bash rejects the identified common Git
index/ref mutation commands, with its existing textual-heuristic boundary
preserved. The TG8-13 static assertion was updated to check the current guard
and command coverage.

Current-tree evidence rerun on 2026-09-28:

- `cargo test -p arcana-core --lib agent::builtin_tools::git_commit::tests --locked -- --nocapture` — 10 passed, including concurrent valid commits plus secret-refusal rollback and binary secret rejection.
- `cargo test -p arcana-core --lib agent::builtin_tools::bash::git_mutation_guard_tests --locked -- --nocapture` — passed.
- `cargo test -p arcana --lib ci_gate::tests::tg8_13_host_commit_safety_is_registered_and_used_by_workers --locked -- --nocapture` — passed after repairing the stale assertion.
- `cargo test -p arcana-core --test s8_git_safety --locked -- --nocapture` — S8-06 passed against the real tools and an isolated Git repository.

P8-74/TG8-13/S8-06 are now reverified in the current tree. The process-local
lock covers Arcana's in-process fleet, not a second Arcana process or external
Git writers. Bash remains a heuristic and cannot stop arbitrary encoded code;
P8's supported worker commit path is the registered host tool. P8 source/docs
changes are not yet committed.
