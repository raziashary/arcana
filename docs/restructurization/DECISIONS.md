# Decisions

New decision log, started 2026-11-03. Add an entry only when there was a real choice between alternatives.
Keep each one short: what was decided, by whom, why, and what it changes.

The 412 earlier entries (`ADR-LOG-RESTRUCT-0001`..`0412`, `ADR-0043`, `ADR-0044`, …) are in
[`archive/development/ADR_LOG.md`](archive/development/ADR_LOG.md). They remain valid history and code
comments may cite them; this file does not renumber or supersede them except where stated below.

## D-001 · Ship Arcana v1 as a Windows-only installer — 2026-11-03, Razi

Linux and macOS parity moves to the post-ship backlog. Unsupported execution stays fail-closed.
**Why:** the programme had spent weeks on sandbox parity and proof while P5, P6, P7 and P9 (the parts
a user sees) had not started. **Supersedes:** the cross-platform clause of the old `P9` goal and `P9-16`
as a v1 requirement; `ADR-LOG-RESTRUCT-0391` still holds for what it defers.

## D-002 · Close P4 on the middle path — 2026-11-03, Razi

Windows sandbox containment (`P4-18`, `P4-61`) is finished before P4 closes; observability depth,
continuity and worker-health depth move to the backlog. **Why:** containment is a safety boundary;
the rest is hardening. **Changes:** `P4_EXPANDED_GOAL.md` is no longer a ship requirement.

## D-003 · Slim the process — 2026-11-03, Razi

The master plan's `§0` rules are replaced by the nine rules in [`SHIP_PLAN.md`](SHIP_PLAN.md) §3.
Suspended: two audit subagents after every phase, adversarial verification before every phase,
per-phase learnings and public-brief refreshes, and an ADR plus receipt for every unit.
**Why:** about 62% of recent commits were tests or docs, and the plan itself had become the largest
artifact in the repo (about 2.5 MB of planning documents).

## D-004 · Archive the old planning tree — 2026-11-03, Razi

`docs/restructurization/development/`, `vision/`, `public-facing/` and the early `_verify` audits moved
to `archive/` with `git mv`; nothing deleted. `ui-anchor.md`, `memory-anchor.md`, the engineering learnings
and the project map moved to `reference/`. Two files that tests read were repointed: the gate register
(`archive/development/TESTING_GATES.md`, read by a `ci_gate.rs` test) and the P4-58 query inventory
(now `arcana-core/tests/fixtures/p4-58-query-inventory.md`).

## D-005 · Cuts confirmed, Linux/macOS work parked, Windows-first reaffirmed — 2026-11-03, Razi

Razi confirmed the proposed cuts in `SHIP_PLAN.md` (deferred: incremental graph updates `P5-08..13`, dark mode,
UI drift gate, reconnect replay, command type checking, performance bench, frontend reachability gate,
interaction-grammar gate, the production updater; kept: the daily spend ceiling `P2-21`). `P9-02`
(Windows code signing) is still an open ruling. The Linux smoke harness and macOS helper are committed on the
local branch `park/linux-macos-sandbox` (`bc54874`, unpushed). **Why Windows-only:** Razi's whole company runs
Windows, so the Windows harness is the product; nothing on `main` was removed to park the other platforms.

## D-006 · Terminal-control work stays in v1 scope — 2026-11-03, Razi (clarification) / Claude (edit, to confirm)

Razi clarified that most of the P4 effort was about not letting an agent run loose in the terminal. Code check:
none of it was lost (no code file deleted since `8356aea`; only a test path and e2e files changed). The plan was
however thin on the unfinished half: W9 (capability-scoped consent for the freeform shell, ADR-LOG-RESTRUCT-0401)
and attack testing of the command path sat in the backlog. They are added to `SHIP_PLAN.md` R1 as v1 items,
pending Razi's confirmation. This refines D-002 (the middle path), which named only `P4-18` and `P4-61`.

## D-007 · R0 done except one unreproduced flake: stale gates fixed, not weakened — 2026-11-03, Claude (to confirm)

The four red `ci_gate` tests were stale static scans. `tg_p1_05`, `tg_p3_03` and `tg_p3_14` read test code as
shipped code because they cut files at the first `#[cfg(test)]`; they now share `production_source`, which skips
modules declared under a `test` cfg and blanks each `#[cfg(test)]`/`#[cfg(all(test, …))]` item (brace-matched) instead
of cutting the file, so shipped code after a test-only helper, field or variant is now scanned too (25 of 351 files have such an item). No file is
excluded by name; self-tests run in both directions. The `e2e_probe` gate required DNS resolution and a runner that were removed on
purpose (commit `ef7c0eb`); it now pins the denial contract and re-activates the original rules if a runner
returns. **Retraction:** the earlier working note that these might be containment holes was wrong; none of the
four hid a hole. **Open:** one `fleet_offline` flake (a missing completion event in the log export) is
unreproduced in 40 runs and no production change was made for it; the test is not run by CI. Dependency installs
under the sandbox remain refused (`SHIP_PLAN.md` R1, ruling needed).


## D-008 · Dependency installs are staged from the lockfile — 2026-11-03, Razi

Razi ruled "Stage from lockfile" on the question raised in `SHIP_PLAN.md` R1: `project_command` refuses every
install except pinned Ruff, so a fresh project cannot build. The host will stage dependencies from the project's
lockfile, outside the sandbox and with the network open only for that step, then the build runs inside the
sandbox with the network denied. Design, the consent model and the first proof (a fresh npm and a fresh Cargo
project build under the sandbox) are the next R1 work; the refusal test stays until staging replaces it.

Razi confirmed (2026-11-03) that staged dependencies live in a **host-owned cache outside the workspace**, which the sandbox
reads through a read-only grant (a sandbox contract change). An independent audit of the first design (Haiku 5.5 High) found that a
cache inside the workspace would let the sandboxed build plant a config file that the next host staging run reads. Still open:
whether the approval shows and binds the package list and lockfile digest, and which toolchain comes first.

Razi also ruled the approval **binds to the lockfile digest** and that **Cargo comes first, then npm**. Cargo is built
(`arcana-core/src/agent/builtin_tools/dependency_staging.rs`). A first `project_command install cargo` call reads the project
and returns the plan and a digest; the second call pins that digest (`plan_sha256`), shows the package list in the approval
(new `Tool::approval_details` hook) and is refused if anything it covers changed. The digest covers the `Cargo.lock` cargo
actually uses (the enclosing workspace's, not the nearest) and every `Cargo.toml` reachable through workspace members and path
dependencies. Staging refuses: non-crates.io sources or missing checksums; a `.cargo/config*` or `rust-toolchain*` file on the
path; links and junctions (including `..` after one); paths outside the workspace; a project excluded from, or not listed in,
its workspace; workspace patterns it cannot expand; and odd package names. The host `cargo fetch --locked` runs with a rebuilt
environment into a scratch directory that is renamed to `~/.arcana/dependency-cache/cargo/<digest>/` only after the files are
re-checked; sandboxed builds get `.../home` read-only with `CARGO_NET_OFFLINE=true`, and the sandbox accepts a read-only grant
only at exactly that shape. Proof: an ignored live test (`staged_cargo_dependencies_build_offline_inside_the_sandbox`) shows a
fresh project with a crates.io dependency fails to build in the sandbox until staged and then builds offline, and a hostile
`build.rs` cannot write into the cache. Two independent security reviews (Haiku 5.5 High) found a lockfile-binding bypass and
unbound manifests; both are fixed. **Accepted residuals:** a file swapped and restored during one fetch is not caught (the
second digest check only sees the end state; it needs a concurrent writer, and tool writes are serialised); the approval shows a
`Cargo.toml` count, not their paths; only `cargo` itself is killed on timeout; build evidence does not fingerprint the cache.

**npm (2026-11-04)** is built (`arcana-core/src/agent/builtin_tools/npm_staging.rs`), same two-call shape (`project_command
install npm`, then the call pinned to `plan_sha256`). It differs from Cargo in one way that matters: a build finds packages by
walking up to a `node_modules`, so a read-only grant to a cache elsewhere cannot serve them. The host therefore runs
`npm ci --ignore-scripts` in a scratch project written from the approved `package.json` and lockfile bytes, publishes the result
as `~/.arcana/dependency-cache/npm/<digest>/`, and **copies** it into `<project>/node_modules`. The cache is never granted to the
sandbox. Every stage re-copies (the build can edit `node_modules`, so a matching marker proves nothing), replaces only a
`node_modules` carrying Arcana's `.arcana-staged` marker, and refuses links and junctions. The digest covers `package.json` and
**every** lockfile present (`npm-shrinkwrap.json` wins over `package-lock.json`; both are bound and policy-checked). Refused before
any process starts: lockfileVersion below 2; any entry whose `resolved` is not exactly the canonical
`registry.npmjs.org/<name>/-/<unscoped>-<version>.tgz` for its own name and version, or lacks sha512 integrity (`inBundle` entries
included unless they have no `resolved` and their parent's lock entry lists them in `bundleDependencies`); aliases (`name` differing from the key), `link` entries, odd or
Windows-device path segments; dependency and `overrides` specs in `package.json` that are not plain version ranges (a leading dot, npm's spelling of a directory spec, included); any `.npmrc` on
the path; npm workspaces; pnpm, yarn, bun and go (still refused). `npm ci` also runs with `--allow-git/remote/file/directory=none`,
a rebuilt environment, empty user and global configs and no stdout capture. A security review (Haiku 5.5 High) found the `inBundle`
policy skip, spec and canonical-URL gaps and the always-recopy issue; all are fixed, with a Windows test that a junction planted
inside a replaced `node_modules` is never followed. Proof: an ignored live test
(`staged_npm_dependencies_build_offline_inside_the_sandbox`) stages `is-number`, runs the test intent offline in the sandbox,
and shows a repeat call restores a tampered tree. **Accepted residuals:** the sandboxed build can modify `node_modules` after
staging (the next stage restores it); a hard link in npm's output is not detected; a decompression-bomb package is bounded only
after it is written (2 GiB / 400k entries, scratch swept after an hour); a sandbox process that outlives its call could race the
copy; install scripts of staged packages never run, so packages that need them (for example `esbuild`) will not work.

## D-009 · W9 and P9-02 approved, Linux-only failing tests skipped, P4-49 later — 2026-11-04, Razi

Razi approved the two items that waited on a ruling and settled three others:

- **W9 v1** (the smaller version in `SHIP_PLAN.md` R1: typed git tools for the worker recipe, `bash` hidden from default Fleet
  workers, intent suite re-run through the production `execute` path) is in v1 scope.
- **P9-02** (Windows code-signing certificate) is approved as a v1 item. Obtaining the certificate is Razi's action; the
  installer build is wired to sign when one is available and ships unsigned (SmartScreen warning) until then.
- **Linux CI:** tests that fail only on Linux because the sandbox/platform behaviour they check exists on Windows only are
  skipped on Linux (`#[cfg(windows)]` or an `#[ignore]` with the reason), not fixed and not removed from Windows. This keeps
  the Linux CI job honest for the code that does run there; Linux stays parked for v1 (D-005). The CI job itself is unchanged.
- **P4-49** (Razi's Calendar acceptance) is deferred until Razi asks for it; R1 can close without it, and the plan row stays open.

## D-010 · Log writers drain before the process exits — 2026-11-04, build decision

The Fleet CLI offline suite failed in CI with the run successful but its last telemetry event missing from a 3.2 MB event
file. `tracing-appender`'s `WorkerGuard` waits at most one second for its worker on drop, and the worker redacts each record
(measured: 10,000 events emit in 245 ms and drain in 1.25 s in a debug build; the queue holds 128k lines, so overflow and the
existing drop counters are ruled out). Per the rule "change production logging only if the diagnostics point at the cause",
`LoggingGuards` now counts writes accepted and finished per queue and waits in `Drop`, up to 10 s in total across all queues,
until they match (plus lossy-queue drops) before the guards drop. This takes effect where `LoggingGuards` is actually dropped,
which is `arcanactl` returning from `main`. The desktop host keeps `_log_guard` in `main()` but leaves through Tauri's event loop,
which exits the process without unwinding `main`, so the drain does not run there; flushing on `RunEvent::Exit` is a backlog
item (`HARDENING_BACKLOG.md` B). Proof is a unit test with a
control (700 lines at 3 ms: lost with the guard alone, none lost with the wait); the CI flake itself is not reproduced locally,
so the next CI runs are the check. If `fleet_offline` fails again with the same signature, the cause was something else.

Also built the same day, no ruling needed: command-path attack tests (P4-56 narrowed), the execute-path intent suite (W9), and a
fix for Cargo lint through the registered tool (a plain workspace root against a verbatim project path made the Clippy config
walk climb out of the workspace; found by the new execute-path test). See `SHIP_PLAN.md` R1.

## D-011 · The Windows sandbox gates run locally, not on hosted runners — 2026-11-04, Razi

The hosted `windows-latest` job failed `project_command_stage_logs` (TG4-32) for six runs. Staging the PATH interpreter was a
real bug and is fixed (`resolve_stdlib_root` demanded a per-user Python install even for a `Lib` inside the prefix; it
surfaced as `command_spawn_failed` and took diagnostics at three layers to find). The next failure is not ours: Arcana's Windows
sandbox loads `processmodel.dll` (the experimental SBOX process API) and the hosted runner does not have it (os error 126), so
no sandboxed command can start there, and the sandbox correctly refuses to run unconfined. Razi chose **skip on hosted** over a
self-hosted Windows 11 runner. The TG4-32 step now runs on Linux only and the "Project command portability corpora" step runs
on every OS except Windows. **Consequences:** the Windows sandbox, the `npm.cmd` shim path and the Windows corpus (TG4-18, TG4-31,
TG4-32 on Windows) are proven only by running the suites on a Windows 11 machine (the 2026-11-04 local runs), not by CI; the
other Windows build-smoke steps after TG4-32 (TG4-19, P4-39, build) had never run on a hosted runner because the job stopped
at TG4-32, so they may still fail. A self-hosted runner stays the way to close this if Windows regressions slip through.

## D-012 · The macOS build-smoke leg is dropped from CI for v1 — 2026-11-07, Razi

The `build-smoke` matrix had a macOS leg (`macos-14`). In run `358fd74` it never started: GitHub refused the runner ("The
configuration 'macos-14-arm64-us-default' is not supported"), so no step ran and the whole run read as "cancelled". Before that
it had failed at "Structured telemetry identity and redaction" (exit 101, no panic text); that cause was never found. v1 is
Windows-only and macOS is parked, so Razi chose **drop the leg** over fixing it. The `build-smoke` matrix is now Windows and
Linux, and the `tg0_02_every_required_check_is_present_and_distinct` assertion in `ci_gate.rs` no longer requires `macos-14`.
**Consequences:** nothing in CI compiles or tests the app on macOS, and the unexplained telemetry test failure there stays
unexplained. `build-release.yml` still has a macOS DMG job; it only runs for releases and is parked with the rest of the macOS
work. Re-adding macOS (a matrix entry plus the `ci_gate.rs` assertion) is part of the post-v1 Linux/macOS parity work in
`HARDENING_BACKLOG.md`.
