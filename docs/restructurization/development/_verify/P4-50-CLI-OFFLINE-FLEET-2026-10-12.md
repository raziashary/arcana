# P4-50 CLI Offline Fleet Evidence

## Source evidence before implementation

- `arcana-core/src/runtime.rs`: `CoreRuntime::initialize` resolves only shipped provider/model catalogue overrides, creates the production `RecipeRunner` and `RecipeScheduler`, and publishes global managers after database construction. Its current provider injection is only `#[cfg(test)]` inside `SubagentManager` construction; this does not reach integration-test builds or `arcanactl`.
- `arcana-core/src/recipes/runner.rs`: `RecipeRunner` already has a test-only provider override used immediately before constructing the real `AgentEngine`. The override preserves scheduler dispatch, recipe resolution, engine, tools, and durable worker lifecycle.
- `src-cli/src/commands/fleet.rs`: the actual CLI Fleet path initializes `CoreRuntime`, requires a Vault API key and explicit workspace egress consent, creates a durable epic, starts recipe-bound workers, kicks Supervisor, polls durable epic/child state until terminal/deadline, emits lifecycle telemetry, and shuts down the scheduler on both success and failure.
- `src-cli/tests/analytics_export.rs`, `src-cli/tests/project_command_stage_logs.rs`, and `src-cli/tests/consent_cli.rs`: integration tests invoke `CARGO_BIN_EXE_arcanactl` with isolated user profiles; analytics and logs have real CLI export readers.
- `arcana-core/Cargo.toml` currently has default-off features only; `src-cli/Cargo.toml` has no features. New feature wiring, if used, must remain opt-in and enable the same code in the actual CLI executable and linked shared core.

## Current implementation

- Added the default-off `offline-fleet-test` feature to `arcana-core` and forwarded it from `arcanactl`.
- Reused the existing RecipeRunner and native subagent provider override slots under the feature so the real CLI's Supervisor, worker, QC dispatcher, and QC reviewer use one scripted provider. Ordinary builds compile neither injection nor marker activation.
- The scripted provider emits actual streamed AgentEngine tool calls. The worker claims its assigned child, runs `project_command` against a locked dependency-free Cargo fixture, and moves it to `qc_review`; the real QC dispatcher launches a reviewer subagent, whose `project_command` build must pass before it requests the legal `done` transition. Fleet's Supervisor observes child completion through the normal durable board path.
- The feature marker accepts only `success` or `deadline`, a UUID-shaped marker ID, a matching file under `.arcana`, and a uniquely named profile rooted under the system temp directory. Failure diagnostics do not include absolute home paths. The runtime's profile path is the same `USERPROFILE`/`HOME` resolution used by the DB paths; the Windows `dirs::home_dir()` helper is not used by this gate.
- The CLI feature bypasses only Vault key lookup. It still requires and checks real persisted workspace egress consent; the test obtains that grant through `arcanactl consent grant` in the isolated profile before running Fleet. The scripted provider uses the production budget and consent wrappers.
- The integration tests invoke the actual `CARGO_BIN_EXE_arcanactl` Fleet subprocess and then read its durable task state through `TasksDb`, its logs through `arcanactl logs export`, and worker analytics through `arcanactl analytics export`.

## Verification and observed behavior

On 2026-10-12, the exact core unit filter `runtime::tests::offline_fleet_provider_emits_agent_engine_tool_call_round_trip` passed (1/1), and the reviewer decision-code plus plain-text grep tests passed (2/2). The core formatter check passed.

The first real CLI success attempt reached QC but stayed in `qc_review` until its 120-second Fleet deadline. The provider trace showed the reviewer reached `grep`; its mock provider emitted zero usage, so the real run-token wrapper correctly treated usage as unknown and charged the full conservative reservation. The following reviewer turn could not start. The feature-gated provider now reports deterministic synthetic usage, preserving the production budget and consent wrappers while allowing reservations to settle accurately for the scripted run. A focused provider test asserts the synthetic usage is observed.

The exact CLI success test then passed (1/1, 20.14 seconds). It invoked the actual `CARGO_BIN_EXE_arcanactl` Fleet subprocess, built the dependency-free Cargo fixture through the worker and QC reviewer, persisted `done` for the child and epic, and read the completion event and worker analytics through the CLI export commands. The full `fleet_offline` integration target passed (4/4, 22.37 seconds): success/export, deadline/shutdown, safe failure-receipt filtering, and rejection of an unmarked test profile. The deadline case now grants consent in its isolated profile so it reaches the intended deadline path rather than stopping at consent validation. Provider/database credentials were removed from each test command; no live provider request was made.

## Scope and limits

This proof establishes CLI Fleet wiring and actual dependency-free Cargo fixture build/QC evidence through deterministic scripted provider responses. It does not establish live/paid provider behavior, arbitrary application correctness, full production observability coverage or owner desktop acceptance.

## Historical implementation checkpoint (superseded)

- Added the opt-in `offline-fleet-test` feature to `arcana-core` and forwarded it from `arcanactl`. It is not in either crate's default feature set.
- Widened the existing RecipeRunner provider-override storage/dispatch to compile under that feature, while retaining the existing unit-test-only setter and adding a feature-only setter for integration harnesses.
- At this earlier checkpoint, no scripted provider, integration test, export assertion, or Fleet execution existed. Later implementation below supersedes this status.

## Verification and observed behavior

- `cargo check -p arcanactl --features offline-fleet-test --offline` completed successfully on Windows. It emitted pre-existing warnings across core/CLI. Live credential/database variables were unset in the command process.
- No Fleet subprocess was launched and no test was run.

## Current implementation and gate status

- [x] Add default-disabled `offline-fleet-test` feature wiring from `arcanactl` into shared core.
- [x] Reuse RecipeRunner's real AgentEngine provider seam under the opt-in feature.
- [x] Add a deterministic scripted provider that responds with actual streamed board-tool calls for Supervisor and worker turns.
- [x] Require a scenario allowlist and a random marker file in a uniquely named profile under the OS temp directory before the feature can activate.
- [x] Restrict Vault key lookup bypass to the validated feature-gated scenario; retain production workspace consent validation and audit.
- [x] Wire real child assignment, worker claim, fixture build evidence, QC subagent build, and legal child/epic completion through production tools and durable task transitions; the child and epic persisted `done` on success.
- [x] Verify actual CLI Fleet success and deadline/shutdown outcomes in the subprocess; the bounded deadline returns and shuts down cleanly.
- [x] Verify actual CLI logs and analytics exports consume completion events and worker stats from the subprocess.
- [x] Verify the unmarked-profile negative case does not create or mutate the task board.

The first success script draft transitioned a claimed board-only task directly to `done`; this was replaced with real fixture build and QC tool paths. The provider override now reaches `SubagentManager` through the parent-approved feature-only plumbing in `native_runner.rs` and `manager.rs`.

The marker variable uses the non-credential-shaped `*_MARKER_ID` name because the environment scrubber removes variables named `*_TOKEN`. The opt-in offline harness proves CLI Fleet wiring with a scripted provider; it does not prove live Gemini completion, the observability UI, full TG4-24 producer coverage, multi-platform launchers, or P4-49 desktop acceptance.

## Verified Windows review corrections

The independent Luna High review identified redirected marker/data paths and an absent outer subprocess deadline. The actual runtime now rejects Windows reparse points (and Unix symlinks) on `.arcana` and its marker before database construction, and rejects nonordinary file/directory types. A real redirected-profile junction test verifies the separate disposable victim receives no `tasks.db`.

The parent harness now polls its owned CLI subprocess until the configured Fleet timeout plus thirty seconds. Windows overrun terminates the owned process tree and reaps the CLI; captured readback is limited to four MiB per stream. The fixture capture files are removed with the disposable profile. Unix overrun currently kills only the parent; descendant cleanup on that outer failure path is unproven and this report claims Windows execution only.

The integration crate is gated on the default-off feature. The ordinary runtime no longer gains an unused-mut warning from feature-only injection.

Exact current check: `cargo test --offline -p arcanactl --features offline-fleet-test --test fleet_offline --locked -- --nocapture --test-threads=1` passes **5/5** on Windows, in **22.54 seconds** after compilation. Scenarios are Fleet build/QC/completion/export, Fleet deadline/shutdown, safe failure receipts, unmarked profile denial and redirected data-directory denial. Credentials/database environment variables were removed. Receipt: ignored `target/p4-ci-checks/p4-offline-fleet-final-2026-10-12.log`. The source review recheck resolves both original Windows findings; its Unix limitation remains explicit in `_verify/P4-50-OFFLINE-CLI-COMMIT-REVIEW-2026-10-12.md`.
