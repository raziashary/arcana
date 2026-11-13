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

## D-013 · R2 (Graph) build plan after the adversarial audit — 2026-11-12, Razi for the tool surface, the rest mine

A read-only plan audit (Haiku 5.5 High) found the first R2 draft wrong in places that matter; the code confirmed each of the
following (`SHIP_PLAN.md` R2 keeps the task list, this entry records how it is built).
- **`update_graph` loses edges today.** It diffs only against `HEAD~1`, deletes each changed file's nodes, and the cascade
  removes edges from *unchanged* files that point into them; `build` then drops outbound edges to unchanged files. The UI calls
  it (`Graph.tsx`). Decision: it becomes a full re-index with non-destructive upserts and a key-set prune, which is the plan's own
  "re-index in full on change" (P5-08..13 stay deferred); the `incremental.rs` path is no longer used by the command.
- **Node identity depends on path spelling** (`qualified_name` of a file node is the raw path), so one DB path is not enough.
  Decision: canonicalise the workspace root once at every entry point and form every key from it; `graph_db_path` uses the
  shared `path_key` and `db::arcana_dir()`. S5-01 proves equal node and edge key sets across four spellings, not just one file.
- **Upsert and migration.** `ON CONFLICT(qualified_name) DO UPDATE` (with `updated_at`) for nodes; schema v7 adds a unique index on
  edges `(kind, source, target, file_path, line)` after removing duplicate rows, with an **unconditional** backup that honours the
  test redirect (Rule 13), and the version pins at 6 move in the same step. `INSERT OR REPLACE` on `snapshots` stays (no FK).
- **Graph text in the prompt is untrusted repository data.** It goes through the memory module's flattening and caps, FTS input is
  tokenised like `LexicalBackend::to_fts_query`, `module::` stubs are excluded, ordering has `qualified_name` tie-breaks, any graph
  error yields an empty block, and one composer serves both the normal and the override prompt path.
- **Tools: Razi ruled "interactive agent, working tools only".** Register only the tools that return a real answer, for interactive
  sessions only (not fleet workers, headless runs or QC). `detect_graph_changes` (runs `git` in the process directory),
  `get_graph_diff` (no snapshot producer), `get_graph_report` (placeholder) and `get_flow_criticality` (edges labelled as flows) go to
  the backlog until fixed. One `impl Tool` per tool, matching the codebase and the TG-P3-20 gate, not a table adapter. Definition
  bytes are measured before and after.
- **Legacy graph DBs:** a non-empty DB named by an old raw-spelling hash is copied (`VACUUM INTO`, no new rusqlite feature) to the
  canonical name; the legacy file is left alone. The one live graph DB on this machine is empty and backed up
  (`~/.arcana/backups/graph-43cee5c8d519-20261112-212330-before-r2-graph-work.db`).

## D-014 · R2 graph tools and the prompt block, as built — 2026-11-12, mine within D-013's ruling

Razi's ruling in D-013 was "interactive agent, working tools only". Building it changed three things the plan assumed; the plan
bends to the code (`SHIP_PLAN.md` R2 now says what shipped).

- **The 27 `graph_tools.rs` structs were not registered as they stood, and not by an oversight.** Every one loaded the whole node
  and edge tables per call, `list_callers` was a transitive closure, and the helpers they call (`query.rs`, `analyze.rs`,
  `flows.rs`, `changes.rs`) compared edge kinds with `"Calls"`, `"ImportsFrom"`, `"TestedBy"` while the extractor stores
  `"CALLS"`, `"IMPORTS_FROM"`, `"TESTED_BY"` (`EdgeKind`'s `Display`). Callers, callees, impact, flows and tests therefore returned
  nothing on any real graph. The literals are fixed (a real defect in code the Graph tab's commands also use), but the structs are
  not the tool surface. **Decision:** eight new bounded tools in `builtin_tools/graph_query_tools.rs` (`graph_search`,
  `graph_node`, `graph_callers`, `graph_callees`, `graph_impact`, `graph_file`, `graph_path`, `graph_overview`), one `impl Tool`
  each, all `ToolEffect::Read`, using the indexed per-node edge lookup, with caps and a reported `truncated`. They cost 3.9 KB of
  tool definitions per request (measured by a test, budget 5 KB). The 27 structs and `graph_prompts.rs` stay inert and unregistered:
  deleting files is Razi's call, so they are flagged in `HARDENING_BACKLOG.md` for deletion rather than removed.
- **Interactive only, by opt-in.** `AgentEngine::with_graph_tools()` registers them; its one production caller is the interactive
  path in `agent_commands.rs`, and not in Delegate mode. `register_for_mode` does not include them, so fleet workers, subagents,
  QC and headless runs are never offered tools that would answer "no graph". `TG-P5-18` pins the caller list, the published names,
  the Read effect, and that the file can neither write the graph nor touch the filesystem.
- **A tool never builds a graph or creates a file.** `location::find_existing` resolves the canonical path and reports none if the
  database is absent; the tool then returns `graph_not_built` with the next step. The prompt block uses the same function and
  opens the database read-only. A graph stored under a legacy spelling is adopted by the Graph tab (`locate`), not on read.
- **The extractor's Python patterns were wrong on PEP 8 code.** `^(\s*)def` lets `\s` match the blank lines above the `def`, so a
  top-level function after a blank line was labelled a "method", got no CONTAINS edge and had its `line_start` on the blank line
  (the same for classes and `from`/`import` lines; a JS `import` pattern had it too). Fixed to `[ \t]*`, with a regression test.
  This is why the first fixture's caller edge was missing; it would have understated every Python graph.
- **The extractor records calls only within one file** (`extract_python`, step 6). Cross-file dependence exists only as file-level
  `IMPORTS_FROM` edges. The tool descriptions say so, and `graph_impact` on a file name finds its importers. Cross-file call
  resolution is a larger extractor change and goes to the backlog, not into R2.
- **Graph block order:** `volatile_prompt` composes memory, then the graph block, then the roster; both the plain and the
  override path call it (`TG-P5-14`, and `TG-P3-44/45` now follows the composer).

End-of-phase review (2026-11-12, `implementation-reviewer`) and what I did with it (Razi asleep; I decided what the existing
rules already settle and recorded the rest):

- **A tool must not migrate.** `GraphStore::open` creates the directory, backs up and migrates an older graph (v7 deletes duplicate
  edge rows). The tools used it, contradicting "read-only". Now `GraphStore::open_current` opens read-only, checks the schema is
  the head, and creates or changes nothing; an older graph returns `graph_needs_rebuild` ("rebuild from the Graph tab"). The
  builder alone upgrades. `TG-P5-18` bans `GraphStore::open(`, `open_tuned`, `journal_mode` and `create_dir_all` in the tools file,
  and a test ages a real graph to v6, calls all eight tools and checks the version and the directory tree are unchanged. The
  mutation (back to `GraphStore::open`) fails that test.
- **`graph_impact` no longer promises tests.** Nothing in the extractor produces `TESTED_BY` edges (`EdgeKind::TestedBy` is never
  constructed), so the tests list was always empty. Removed from the output and the description; the consumers of that edge kind
  in `analyze.rs` and `changes.rs` stay until a producer exists (backlog G).
- **The prompt block no longer fans out through a file or class.** From a seed, a CONTAINS edge names members, but a node reached
  that way is not expanded through CONTAINS again, so alphabetical siblings of a match cannot fill the 12 slots. Test added
  (fails without the rule).
- **The graph block reaches every engine, as the memory block does.** It is part of the volatile prompt both the plain and the
  override path build, so fleet workers and subagents in a workspace with a graph also get the bounded block (3 KB, read-only,
  empty with no graph) but no tools. I judged this consistent with the memory block and useful orientation; recorded here as
  intended. Gating it behind an opt-in is a one-line change if Razi prefers.
- **Truncation is reported where it was silent:** `graph_search` with a `kind` filter, and an ambiguous or not-found symbol when the
  name index was cut off at its cap.
- **Claims corrected:** `find_existing` creates no graph database or `graphs` directory (it still ensures the app's `~/.arcana`,
  like every store); the module-stub gate now needs all five exclusions.

## D-015 · R3 start: the audit's findings, what was built first, and what waits for Razi — 2026-11-13, mine within the plan

A start-of-phase `plan-auditor` pass over R3 (read-only, against the code) found the plan's counts roughly right and its stated
causes partly wrong. What changed in the plan, and what I did:

- **Graph City's hang was not the Google Fonts fetch.** The CSP allows `fonts.googleapis.com` and `fonts.gstatic.com`. The hang was
  drei's `<Text>` (troika): it loads its default and unicode-fallback fonts from a CDN (`connect-src` allows only IPC) and starts a
  blob web worker (`index.html`'s CSP has no `worker-src`/`blob:`). The text never resolved, and each district and god-node
  landmark sat inside a `<Suspense fallback={null}>`, so the districts, walls and landmarks never drew. Reproduced in a browser
  under the shipped CSP (buildings only, no districts, two worker-CSP errors), then fixed: labels are DOM overlays
  (`city/CityLabel.tsx`, drei `<Html>`), with no font file and no fetch, and the districts render. **`TG-P6-02`** bans a drei
  `<Text>` import. Built with no new dependency and no download.
- **Still open under `P6-02`, Razi's call:** the app still loads IBM Plex from Google Fonts at runtime (works online, falls back to
  system fonts offline) while `ui-anchor.md` names Urbanist/Manrope/Roboto Mono. By the "code wins" rule the anchor is what is
  wrong; bundling IBM Plex needs a font download, which I will not do unasked. Options: (a) bundle IBM Plex (download the OFL
  files, add `@font-face`, drop the Google origins from both CSPs); (b) keep the runtime fetch and correct `ui-anchor.md`; (c) a
  system-font stack. I recommend (a).
- **The command contract has two gates now.** `TG-P6-10`: every `invoke('…')` in the frontend names a registered command (this is
  the "missing" side; the audit found one, `navigate_to_logs`, not two — fixed by switching the tab through `uiStore`, since tab
  navigation is a frontend concern; the Logs tab has no per-agent filter, so it opens unfiltered). `TG-P6-11`: the 49 commands that
  are registered and never invoked are a list that may only shrink (the plan said 47). **Deleting them is not done**: several back
  later R3 rows (`has_provider_key` for the P7-01 first-run check, `get_user_preferences` for P6-15 hydration,
  `get_daily_budget_summary` for P2-21), the git and checkpoint ones are possible undo UI (P7-12..14), and removing a registered
  command is a shipped-contract change. Razi decides which to delete once those rows are built.
- **Proof ids:** `S6-01..04` and `S7-01..03` exist only in the archived master plan as one-line bullets. `SHIP_PLAN.md` R3 states
  the real check for the rows built so far (`TG-P6-02`, `TG-P6-10`, `TG-P6-11`); each remaining row gets its runnable check when it starts.
- **Open owner decisions, not blocking the next build steps:** the font option above; whether the 3D city's colours are exempt from
  a no-literal-hex gate (recommend: allow hex only in the token file and one city palette module); the BYOK proof (recommend a
  mock-provider automated path plus one manual run with a real key); which of the 49 uncalled commands to delete; whether to wire
  the checkpoint commands as undo.
- **Counts re-derived from source (2026-11-13):** 255 registered commands, 206 distinct `invoke` names in non-test frontend code;
  709 hex literals in 80 files (plan: 75); 37 `listen` names; dead listeners and the 3,700 orphaned lines were not recounted by
  the audit (event names are Rust constants, so a literal search is invalid; "orphan" is undefined).

## D-016 · Razi's R3 rulings: bundle IBM Plex, one city palette, mock-provider proof, decide dead commands later — 2026-11-13, Razi

Asked and answered on 2026-11-13, after the D-015 audit.

- **Font: bundle IBM Plex.** Four Latin woff2 files (Mono 400/500, Sans 400/500, 70 KB in all) and the OFL licence were downloaded from the
  `@fontsource` packages on jsDelivr (v5.1.0, packaging of IBM's open-source Plex) into `src/assets/fonts/`. `globals.css` declares them with `@font-face`;
  the Google Fonts link and import are gone; both CSPs now say `font-src 'self'` and `style-src` has no external origin. `TG-P6-03` pins this
  (it fails if a CDN font reference returns). `ui-anchor.md` Typography is rewritten to match the shipped fonts. Non-Latin text falls back to system fonts.
- **Hex gate scope: exempt through one palette module.** When P6-01/04 are built, literal hex is allowed only in the token file and a single city
  palette module (three.js colours); everything else uses tokens.
- **First-run proof (P7-01..04): a mock-provider automated test, plus one manual run with a real key.** The manual run needs a key from Razi and
  is not run unattended; suites stay free of live credentials.
- **The 49 uncalled commands: decide after the P6/P7 rows.** `TG-P6-11` keeps them as a ratchet; deletion happens at the end of R3 for those still unused.

## D-017 · Razi's R3 rulings: confirm without undo, delete the inert graph code, delete uncalled commands, define orphans — 2026-11-13, Razi

Asked and answered on 2026-11-13, with the instruction to carry R3 through end to end.

- **Destructive actions (P7-12..14): confirm only, no undo in v1.** Each unconfirmed destructive action asks first and states what is lost. Checkpoint-backed undo
  (`git/checkpoint.rs`) is not wired; it goes to the post-ship backlog.
- **Delete `graph_tools.rs` and `graph_prompts.rs`** (unregistered since R2, superseded by `graph_query_tools.rs`), with their `mod` lines. The four parked tool ideas stay
  in `HARDENING_BACKLOG.md` section G.
- **The 49 uncalled commands: delete those with no caller anywhere** (no frontend, CLI or test). A command that looks like a planned v1 UI is kept and listed to Razi.
  `TG-P6-11`'s list shrinks to match.
- **Orphaned frontend code: "orphan" means a non-test file not imported, directly or transitively, from the app entry.** Those files are deleted, and the commit body lists them.

## D-018 · The daily spend cap ships off by default, and the first-run key step offers it — 2026-11-13, Razi

Asked and answered on 2026-11-13 while closing `P2-21`. The cap is built, tested and fail-closed (see the `P2-21` row), and a model it cannot price is refused. v1 keeps it **off by
default** so a stop is never a surprise, and the key-entry step of first run (`P7-01..04`) says the cap exists and offers to enable it at the $50 default. If that step is cut, this
decision must be revisited, since a paid-key app would then ship with the cap findable only in Settings.
