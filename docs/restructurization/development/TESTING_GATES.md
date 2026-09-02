# TESTING_GATES.md — AutoDev Restructure

> **Status:** authored 2026-08-31 from the answered vision suite (`docs/restructurization/vision/`, master plan rev 0.9, ADR ledger complete at 51/0 open).
> **Scope:** every test gate the ten specs asked for, plus eight gates added because an invariant or a risk was otherwise uncovered.
> **Companion documents:** `MASTER_DEVELOPMENT_PLAN.md` (the build sequence and its inline smoke checks) · `READINESS_MATRIX.md` (phase readiness) · `ADR_LOG.md` (runtime decisions) · `NOVELTY_LEDGER.md`.
> **Cross-checked against the MDP** on 2026-08-31. Phase names and the `TG<p>-<nn>` scheme agree. Three assignments where the MDP builds a capability **earlier** than Part 6 implies are annotated on the gate line (`TG1-05`, `TG3-38`, `TG4-18`); IDs were **not** renumbered, per `§1`.

**115 gates across P0–P9.** This document is the definition of done for the programme.

---

## 1. The contract

- **Every gate from the specs appears exactly once**, assigned to the **earliest phase whose build makes it runnable** — not the phase whose spec happens to contain it. Where those differ, the assignment follows the build, and the divergence is stated on the gate line.
- **A gate is not a smoke check.** Smoke checks live inline in the MDP, are throwaway, and answer *"did this phase's build come up?"*. Gates are **permanent, named, automated spec-invariant tests** run **after** build + smoke. Once a gate exists it runs in every later phase forever; a gate is never retired because its phase closed.
- **Evidence class is recorded per gate when it is checked** — replace `_pending_` with the class actually achieved, never with a promise. Classes:
  | Class | Means | Maps to master plan `§7.1` |
  |---|---|---|
  | `CODE-PRESENT` | The code exists and was read. **Weakest class; never sufficient on its own for a gate.** | `CODE-PRESENT` |
  | `UNIT-PROVEN` | A unit/property test that can fail passes | `TEST-GREEN` |
  | `LIVE-DB-PROVEN` | Proven against a real database file, not a mock | `TEST-GREEN` + `PROVEN-READ` |
  | `SMOKE-PROVEN` | Proven by the end-to-end smoke driving the real app | `TEST-GREEN` |
  | `GATE-PROVEN` | A blocking CI job that can fail is green | `TEST-GREEN` |
  | `REPLAY-PROVEN` | Proven against recorded provider/network fixtures, offline | `TEST-GREEN` |
  | `BENCH-PROVEN` | A number produced by running something, against a recorded baseline | `MEASURED` |
  | `DOC-GATE` | A static check over document or config text (the only class where no runtime exists to prove) | `TEST-GREEN` |
  | — | *(the plan's `PLATFORM-TRIPLE` is not a class here; it is a **multiplier** — a gate tagged `[portability]` must carry its class on all three OSes)* | `PLATFORM-TRIPLE` |
- **A skipped gate is not green.** A gate disabled in CI, marked `#[ignore]`, `continue-on-error`, `skip`, `xfail`, or guarded behind an env var that CI does not set **counts as failing** for the phase it belongs to. This is INV-4 applied to this document itself; `TG0-02` enforces it mechanically.
- **IDs are `TG<phase>-<nn>` and are stable forever.** Never renumber. Append only. A gate that dies is struck through in place with a one-line reason and the date — never deleted, never reused.
- **A cross-phase gate is listed at its spec's phase but owned by the later phase** that can actually run it, marked `→ owned by P<n>`. The owning phase cannot close until it is green. Where only a *clause* of a gate is cross-phase, the clause is named.
- **Family tags** are one of: `[policy]` `[security]` `[determinism]` `[conservation]` `[seam]` `[perf]` `[governance]` `[correctness]` `[portability]` `[ux]` `[data]`.
- **Gates marked `NEW`** were not asked for by any spec. Each exists because an invariant or a Part-9.2 risk had no covering gate, and each names the invariant or risk it closes. There are **eight**; every one is justified on its line.
- **Merged gates** are recorded in `§18 Merge register`. A merge never loses an assertion — the surviving gate carries both sources and the union of the assertions.

---

## 2. Phase index

| Phase | Slice (master plan Part 6) | Gates | IDs |
|---|---|---|---|
| **P0** | Stop the bleeding | 5 | `TG0-01…05` |
| **P1** | Make the invisible visible | 6 | `TG1-01…06` |
| **P2** | The provider layer | 17 | `TG2-01…17` (implemented as `TG-P2-01…09`, see note) |
| **P3** | The runtime | 39 | `TG3-01…39` |
| **P4** | The fleet | 23 | `TG4-01…23` |
| **P5** | The graph | 4 | `TG5-01…04` |
| **P6** | The UI | 9 | `TG6-01…09` |
| **P7** | Experience | 5 | `TG7-01…05` |
| **P8** | Submodule deletion | 2 | `TG8-01…02` |
| **P9** | Delivery | 5 | `TG9-01…05` |
| | **Total** | **115** | |

---

## 3. P0 — Stop the bleeding

**Slice:** updater trust root (M-27 partial) · asymmetric vault (M-30 / ADR-03-01) · existing CI gates made blocking (M-28 partial) · `dump.rdb` deleted · the unauthenticated `/mcp` HTTP server deleted (ADR-03-02, moved here from P8 by **G-15**) · the end-to-end harness and macOS CI moved here (review fix 10).
**Exit gate (Part 6):** an unsigned/unverified update **cannot** install; no command returns a key; a failing test fails CI.

- [ ] **TG0-01** `[security]` — An update whose signature is **missing** is refused; one whose signature is **bad** is refused; one whose signature is **good and chains to the pinned production trust root** installs. Absence of a signature is a failure path, never a skip path — the empty-`sig_url` branch must not exist in the binary. Refusal is asserted by observing the installer never executes, not by a return code alone.
      Source: spec `09 §6 G-UPDATE-REFUSE` · Catches: `IN-01…IN-05`, `verify.rs:90` · Proves: **INV-11**
      **Status 2026-09-01: PARTIAL — not green.**
      ✅ Missing signature refused · bad signature refused · plaintext signature URL refused · empty checksum refused. `UNIT-PROVEN`: `updater::verify::tests` (8 tests), notably `verify_update_refuses_an_absent_signature_url` and `signature_accepts_the_matching_key_and_refuses_another`.
      ✅ The empty-`sig_url` skip branch no longer exists in the binary — `verify_update` bails on an absent signature URL.
      ❌ *"good signature chaining to the pinned **production** trust root installs"* is **unproven**: no production keypair exists yet. Generating it and setting the `AUTODEV_UPDATE_PRIVKEY`/`AUTODEV_UPDATE_PUBKEY` secrets is Razi's action (`docs/updater-signing.md`, `ADR-LOG-RESTRUCT-0003`).
      ❌ *"refusal asserted by observing the installer never executes"* is unproven — the tests assert on return values, not on process execution.

- [x] **TG0-02** `[governance]` — Static check over `.github/workflows/**`: no `continue-on-error`, no `if: push && ref == main` guard, and no `|| true` on any correctness job. The job list must contain every row of spec `10 §6`'s target column — `cargo test` (both crates), `cargo fmt --check`, `tsc --noEmit`, ESLint, frontend unit tests, and three-OS build smokes **on PR**. A row present but permitted to fail is a failure of this gate, as is a row absent.
      Source: spec `09 §6 G-CI-BLOCKING` + spec `10 §6` · Catches: `G0 §2` — today only clippy and tsc can fail CI · Proves: **INV-4**
      **Status 2026-09-01: GREEN** (strengthened after the post-phase audit — the first version tested one spelling of each defect and the idiomatic re-break passed it, `F-18`). `GATE-PROVEN` — `src-tauri/src/ci_gate.rs`, three tests reading `.github/workflows/**`: no `continue-on-error` and no `|| true`; no job gated to pushes on `main`; every required check present (`cargo fmt`, `cargo clippy`, `cargo test`, `npm run typecheck`, `npm run lint`, `npm test`), all three OSes built, and `npm run lint` asserted NOT to be an alias for the typecheck.
      Corroborated by `S0-05`: a deliberately failing Rust test exits 101 and a failing Vitest test exits 1; with `continue-on-error` gone, both fail the job.

- [ ] **TG0-03** `[security]` — **Static:** no Tauri command, event payload, or IPC return type carries key material — `get_provider_key`, `get_github_pat` and every successor return a presence/last-4 descriptor, never a secret; and the CSP `connect-src` allow-list contains no provider origin and no `api.github.com` reachable from the renderer. **Runtime:** a key entered in the UI travels inbound only and cannot be read back out by any route.
      Source: spec `02 §11 G-KEY-ASYM` · Catches: `IN-47` — raw secrets returned to the renderer, plus `connect-src api.github.com` as an allowed exfiltration path · Proves: **INV-2**
      **Status 2026-09-01: PARTIAL — static half green, runtime half unproven.**
      ✅ Static, key material: `GATE-PROVEN` — `vault_commands::tests::no_command_returns_key_material`. `get_provider_key`/`get_github_pat` deleted; the vault command set is closed and no command that reads the vault returns a String-bearing type.
      ✅ Static, CSP: `GATE-PROVEN` — `ci_gate::tests::tg0_03_csp_allows_no_external_origin_from_the_renderer`. This gate was **failing for real**: `connect-src` allowed `https://api.github.com` and `https://releases.airs.dev`. Both removed; nothing in `src/` called either.
      ❌ Runtime half — *"a key entered in the UI travels inbound only and cannot be read back out by any route"* — is unproven. There is no test that drives the UI. Needs the e2e harness (`TG0-05`) to execute.
      → the *"a saved key validates against the provider before storage"* clause is **owned by P2** (it needs the provider layer). The static half must be green at P0.

- [ ] **TG0-04** `[security]` — **NEW (uncovered invariant INV-3).** No `.env`, no `.git` tree, no `dump.rdb`, no private key, and no string matching the credential-scan ruleset appears in (a) any bundled resource directory, (b) any produced installer or `.app`/`.deb`/`.exe` payload, or (c) the committed tree. The scan runs on every PR **over the built artifact**, not the source tree — gitignore does not protect a built artifact.
      Source: **NEW** — INV-3's only spec gate is `09 §6 G-BUNDLE`, which cannot be green before P9, leaving INV-3 ungated for nine phases · Catches: `G0 §3.1` — a 225 MB sidecar bundled with its `.env` and `.git`; the recurrence INV-3 exists to prevent · Proves: **INV-3**
      **Status 2026-09-01: PARTIAL — clause (c) green, clauses (a) and (b) not implemented.**
      ✅ (c) committed tree: `GATE-PROVEN` — `ci_gate::tests::tg0_04_no_secret_or_stray_artifact_is_committed` asserts `.env` and `dump.rdb` are not **tracked** by git and are gitignored. `src-tauri/dump.rdb` (175 KB) deleted; `*.rdb` gitignored.
      *The first draft of this test asserted `.env` must not EXIST and failed correctly — a developer `.env` holding provider keys is expected in a working tree. The invariant is "never committed", so it now asks git whether the file is tracked.*
      ❌ (a) bundled resource directory and (b) built installer payload: **not implemented.** Both need a produced artifact, and the scan must run over that artifact rather than the source tree. `src-tauri/resources/` still holds the 225 MB sidecar copy, untracked.
      ⚠ **The check tests the INDEX, not history** — deliberately, and the scope is stated rather than implied. `dump.rdb` is still reachable in history. It was extracted and scanned: it holds agent procedural memory and **no credential-shaped string**, and history is otherwise clean (`.env` was never added on any branch; no key file ever was). A rewrite of 202 commits and a force-push over published `main` is not justified by 175 KB of non-secret text, so it stays — see `_verify/P0-AUDIT-TRIAGE.md`. `*.pem`/`*.key`/`*.p12`/`*.pfx` were added to both `.gitignore` and this gate in the same pass, since the signing runbook has the owner generating a private key in the working directory.

- [ ] **TG0-05** `[portability]` — **NEW (uncovered risks R-4, R-10).** CI runs Windows, Linux **and macOS** jobs, on pull requests, all blocking; and the end-to-end harness (WebDriver/Playwright against the built Tauri app) exists and executes at least one assertion on each. This gate asserts that the lanes exist and can fail; what runs inside them grows through the programme and is finally sealed by `TG9-01`.
      Source: **NEW** — `00 §9.2 R-4`'s mitigation ("macOS enters CI in P0, not P9") and review fix 10 ("harness and macOS CI moved into P0") are mandates that no spec turned into a gate · Catches: "there is no macOS job at all" (`09 §5`); P4's exit gate naming three OSes with no lane to run them in · Covers: **R-4**, **R-10**
      **Status 2026-09-01 (revised after the first green CI run): PARTIAL — much stronger, still not green.**
      ✅ CI runs Windows, Linux **and** macOS (`macos-14`) build smokes, on pull requests, all blocking — a `build-smoke` matrix with `fail-fast: false`. `GATE-PROVEN` by `ci_gate::tests::tg0_02_every_required_check_is_present_and_distinct`.
      ✅ Windows leg of `S0-01` is `SMOKE-PROVEN` locally: the app builds in 2m24s and starts (logging initialises, retention runs).
      ✅ The e2e harness **executes and asserts** on Linux, verified in run `33467890406`:
      `✓ opens a window and renders the React root` · `✓ reports a document title` · `2 passing (1.7s)`, driving `wry 0.55.1` against the real binary. Not a vacuous pass — the assertions are named in the log.
      ✅ **NEW — a launch smoke on the Linux build job.** Compiling is not starting: the three build smokes stop at `cargo build`, so an app that compiled and then died in `setup()` was invisible to all of them. The job now runs the binary under `xvfb` for 20s and requires it to still be alive (`OK - the application was still running after 20s`). This is the check that would have caught `"dialog": {}`; it needs no webdriver and is far more robust than the harness.
      ❌ **"executes at least one assertion on EACH" is not met — Linux only.** `tauri-driver` has no WKWebView backend, so macOS can never run this harness, and Windows needs an Edge WebDriver matching the runner's Edge build. Recorded rather than hidden behind a skip that would report green.
      ❌ The app is never **launched** on macOS — that lane builds only. Adding a macOS launch smoke is the cheapest way to close this and is carried to P1.

---

## 4. P1 — Make the invisible visible

**Slice:** declare the undeclared modules (M-17) · **delete** `db/checkpoints.rs` rather than declaring it (`ADR-LOG-RESTRUCT-0012` — undo already ships via `git/checkpoint.rs`) · the reachability guard test · the workspace resolver fails loudly (M-14).
**Exit gate (Part 6):** `graph_tools` and `graph_prompts` are in the binary; a guard test proves every source file is reachable; no agent write can land outside a resolved workspace.

> ⚠ **Corrected 2026-09-01.** This gate required `checkpoints` to be *in the binary*. `db/checkpoints.rs` was **deleted**, not declared (`ADR-LOG-RESTRUCT-0012`), so as written the P1 exit gate could never be satisfied. Undo ships via `git/checkpoint.rs`. Found by the post-phase doc audit (`D-16`).

> P1 is where the **structural static guards** live. Every gate here is a check over the tree itself, which is why it can run before any subsystem is rebuilt — and why it must, since every later phase adds code these gates govern.

- [x] **TG1-01** `[seam]` — Every `.rs` file under `src-tauri/src/` (and each workspace crate's `src/`) is reachable from its crate root by a `mod` chain. A file present on disk and absent from the module graph **fails the build**. Named minima: `agent/builtin_tools/graph_tools.rs`, `graph_prompts.rs` and `db/checkpoints.rs` are in the binary, and their tools/commands are registered — presence in the module graph is not enough if nothing registers them.
      Source: spec `05 §5 G-REACHABLE` · Catches: 918 LOC never compiled (`G2a T32`) · **Evidence: `GATE-PROVEN`** — `src-tauri/src/reachability_gate.rs::every_source_file_is_reachable_from_its_crate_root`, walking **all four** crates (280 files) with a per-crate floor. Falsified by planting an orphan in `autodev-core`: caught and named.
      ⚠ **Clause corrected (`ADR-LOG-RESTRUCT-0012`).** This gate's named minima required `db/checkpoints.rs` to be *in the binary*. It was **deleted**, not declared: its justification — "undo does not exist" — was false (undo ships via `git/checkpoint.rs`), and declaring it would have created the second checkpoint store `P1-16` then had to merge. `graph_tools.rs` and `graph_prompts.rs` are declared and compiling. **The registration half of this gate is still unmet and is not claimed:** the 27 structs in `graph_tools.rs` do not implement `Tool` and nothing registers them — that is P3 (BUG-GRAPH). Compiling is not wiring.

- [x] **TG1-02** `[data]` — Every migration ladder in the workspace applies cleanly to an **empty** database, in order, with no swallowed errors: a statement SQLite rejects fails the test loudly rather than being logged and skipped.
      Source: spec `08 §4 G-MIGRATE-FRESH` (merged with spec `05 §5 G-MIGRATE`, fresh half) · Catches: the swallow-and-continue pattern that hid `F-31` · **Evidence: `UNIT-PROVEN`** — `db/migrate.rs`: `a_fresh_database_runs_the_whole_ladder`, and `a_failing_migration_neither_stamps_nor_half_applies`, which feeds the ladder the exact invalid `ADD COLUMN IF NOT EXISTS` that graph v5 swallowed and asserts the version is **not** stamped and the successful half is rolled back.

- [x] **TG1-03** `[data]` — Every migration ladder applies cleanly **from the live head** captured in the `fx-migrate-heads` corpus, and a malformed statement fails the test. Applies to all eleven live databases.
      Source: spec `08 §4 G-MIGRATE-HEAD` (merged with spec `05 §5 G-MIGRATE`, head half) · Catches: `F-31` · **Evidence: `LIVE-DB-PROVEN`** — `examples/migrate_check` runs core's own shipped `open()` paths against the live data root: **10 of 10** databases reach their ladder head with `integrity_check = ok` and **zero row-count changes** (720 rows). Proven on copies first, then applied. Backed up to `~/autodev-backups/p1-10-preflight` and `p1-11-preflight`.
      → ~~the **`graph/` ladder clause is owned by P5**~~ → **the quarantine is LIFTED.** `P1-11` repaired v5 and added v6, so the justification no longer exists — and §1's own rule is that a quarantine outliving its reason is a skipped gate. The graph ladder is inside this gate: the live store reaches **v6** through the shipped `open()` path, verified with `integrity_check = ok` and unchanged row counts.

- [x] **TG1-04** `[data]` — A migration that drops, truncates, rewrites or type-changes a column **refuses to run** unless a restorable backup of that database was recorded in the same invocation, and the backup location is written to the migration log. No backup, no destruction.
      Source: spec `08 §4 G-MIGRATE-BACKUP` · Catches: the absent backup path · **Evidence: `UNIT-PROVEN`** — `run_ladder` backs up automatically before any rung containing `DROP`/`DELETE FROM`/`RENAME`/`TRUNCATE`, and **a failed backup stops the migration**. `a_destructive_migration_backs_up_first` reads the backup back and asserts it contains the row the migration then deleted — not merely that a file appeared. Baseline rungs are `CREATE … IF NOT EXISTS` and are skipped, so a fresh install does not copy everything it just created.

- [x] **TG1-05** `[seam]` — Static check over the whole workspace: exactly **one** `CREATE TABLE` (or schema definition) per table name. Two crates defining the same table is a failure, not a warning.
      Source: spec `08 §4 G-OWNER` · Catches: `app_settings` × 3; the `src-shared` mirror of `tasks`, `workers`, `plans`, `plan_steps` (review fix 11) · Proves: **INV-10**
      **Evidence: `GATE-PROVEN`** — `TG-P1-04` in `ci_gate.rs` asserts exactly one creator for each of `config.db`'s **six** tables (three when this line was first written — the post-phase audit found `checkpoints`, `hooks` and `hook_logs` outside the ladder that claimed to own the file; `ADR-LOG-RESTRUCT-0020`, and the gate now reads `config_db::TABLES` so it cannot drift from the owner again), falsified by planting a second (caught, named both files). `src-shared`'s mirrors of `tasks`, `workers` and `plans` are **deleted** (`P1-09`); `app_settings`' three creators now route through `db/config_db.rs`, which owns the file because `user_version` belongs to the file rather than a table. Its detector ignores comments and `#[cfg(test)]` fixtures — the first version matched its own falsification test (`L-C7`).
      → **RESOLVED against the MDP.** The pass condition depends on the **core-crate extraction** (`autodev-core` / `src-cli` / `src-shared`), which review fix 8 (`R1 B-05/B-06`) added to the build sequence without assigning it a slice. The MDP schedules it at **P1-B**, in this phase, *"before the runtime work in P3, so the rework lands in its final home"*. The assignment holds and the gate has an owner. Was finding F-2.

- [x] **TG1-06** `[data]` — Two processes opening and writing the same database file both succeed: `busy_timeout` is set on every connection, and a write contending with another writer retries rather than erroring. This is not in tension with INV-1 — under the **restated INV-1** (G-02) it is the requirement, since `airsctl` and the desktop app are two binaries linking one agent-core crate.
      Source: spec `08 §4 G-CONCURRENT` · **Evidence: `UNIT-PROVEN`** — `tests/concurrent_writers.rs`: four writers starting on a barrier, 100 inserts, all commit with no rows lost. All 17 production connection sites go through `db::open_tuned`, enforced by `TG-P1-05` (falsified by planting a direct `Connection::open`).
      ⚠ **Two claims corrected by measurement.** (1) `busy_timeout` was **not** missing: SQLite defaults to 0 but **`rusqlite` sets 5000 ms on every connection**, verified by reading `PRAGMA busy_timeout` from an unconfigured `Connection::open`. Setting it explicitly makes the value ours rather than a dependency default; it fixed nothing. (2) Re-running these tests under a forced `journal_mode = DELETE` showed the writer and reader tests **pass under both modes** — only the pragma assertion discriminates. The genuine WAL defect was that `analytics.db`, `annotations.db` and `workers.db` sat in `delete` mode while the rest were WAL; all are now consistent.

---

## 5. P2 — The provider layer

**Slice:** trait v2 with `Item::Opaque` and 6-field `Usage` (M-03) · prompt caching + the cache-hit alarm (M-04) · usage/cost rebuild (M-06) · model persistence (M-07) · BYOK lifecycle (M-30). *(the Grok adapter was struck with `P2-07`, 2026-09-01.)*
**Exit gate (Part 6):** all four providers pass a conformance suite; a 5-turn session shows non-zero cache reads from turn 2; an unknown model is an explicit error.


> **⚠ Two ID schemes, one set of gates (recorded 2026-09-02).** This register
> numbers P2's gates `TG2-nn`; the implementations in `src-tauri/src/ci_gate.rs`
> are named `tg_p2_nn`, and the two do **not** correspond one-to-one. The
> register is organised by spec invariant, the code by the defect each gate was
> written to catch, and several register rows are satisfied by the adapter
> conformance and cache suites rather than by a file-scanning gate.
>
> The mapping is given per row below. It is written down rather than renumbered
> because renumbering a shipped register breaks every citation in the MDP and the
> readiness matrix, and `§0` forbids renumbering IDs.

- [ ] **TG2-01** `[seam]` — Per provider × model, the conformance suite asserts each capability the model **claims**: streaming, tools, **parallel** tools, structured output, system prompt, vision where claimed, reasoning, usage completeness, truncation detection, and error classification. A capability the capability table advertises and the adapter cannot deliver is a failure; a capability absent from the table is not exercised. Capabilities are keyed by **model, not provider** (spec `02 §1`). → **AMBER.** `capabilities()` is keyed by model (P2-03) and the conformance suite asserts streaming, tools, parallel tools, system prompt, usage and truncation across all three. **Not exhaustive per provider × model**, and structured output and vision are unexercised. The catalogue test proves every shipped model *claims* parallel tools; it does not prove each one delivers.
      **Error classification carries two sharpened clauses, each closing a named defect no spec gated:** (i) every `ErrorClass` has a **declared policy that is exercised** — `Retryable` is actually retried, with a stated interval, backoff and `max_retries`; a class defined with no policy fails. (ii) A classified failure is **persisted through `record_api_failure` and read back by its dashboard consumer** — INV-8's proven-read applied at the point failures originate.
      Source: spec `02 §11 G-CONFORM`, sharpened to absorb review fix 13 (`interval`/`cadence`/`backoff`/`max_retries` appear **zero times in all thirteen documents**; `ErrorClass::Retryable` is a writer with no reader inside INV-8's home spec) and the `Reader-exists` family's one uncovered named instance · Catches: every ⚠️ and ❌ in the `G2b` capability matrix; `IN-53` — `record_api_failure` with zero callers while the dashboard reads the table · Proves: **INV-8** · Evidence: _pending_

- [x] **TG2-02** `[conservation]` — `max_tokens` reaches the wire on all four providers (asserted against the recorded request body, not the request struct), **and** a response that hits it yields `StopReason::MaxTokens` on all four. An unknown finish reason surfaces as `Unknown(String)`, never silently as `EndTurn`. → **GREEN.** `conformance_tests::every_adapter_sends_an_output_ceiling` (asserted on the built request body, not the struct) · `usage::StopReason` tests · live `p2_10_truncation_is_classified_the_same_way_on_every_provider`, which measured `max_tokens` / `length` / `MAX_TOKENS` all classifying as `MaxTokens`. An unknown value becomes `Other(provider:raw)`, never `EndTurn`. *(Three providers, not four — Grok was dropped by ruling.)*
      Source: spec `02 §11 G-MAXTOK` (merged with spec `01 §8 G-TRUNC`) · Catches: OpenAI and Google silently dropping `max_tokens`; silent truncation; xAI's undocumented finish-reason set · Evidence: _pending_

- [ ] **TG2-03** `[conservation]` — Tool arguments survive the round trip **byte-identically**, including nested objects, arrays, empty strings, numeric precision and non-ASCII unicode. An adapter that emits `{}` where arguments were supplied fails; property-tested over generated argument trees. → **AMBER.** Arguments are asserted to survive on all three, and the fixture suites pin that streamed fragments only parse once concatenated. **Not property-tested over generated argument trees**, so numeric precision and unicode edge cases are unexercised.
      Source: spec `02 §11 G-TOOLARGS` · Catches: `google.rs:313` discarding args; the laundered empty `{}` · Evidence: _pending_

- [x] **TG2-04** `[conservation]` — Two tools requested in one assistant turn both execute, and **both results return** — as one message on Anthropic (which requires it) and in the provider-correct shape elsewhere. Asserting only the first result is the defect this gate exists to catch. → **GREEN.** `anthropic_keeps_parallel_results_in_one_user_message` · `openai_flattens_parallel_results_into_one_tool_message_each_in_call_order` · `google_emits_one_function_response_part_per_parallel_result`. Live: Gemini returned two parallel calls carrying real arguments.
      Source: spec `02 §11 G-PARALLEL` · Catches: first-only on OpenAI and Google; Anthropic's single-message requirement · Evidence: _pending_

- [x] **TG2-05** `[perf]` — A 5-turn session over an identical prefix reports **non-zero cache reads from turn 2** on every provider whose model advertises caching. The assertion is provider-neutral: OpenAI and Gemini report cached tokens *inside* the prompt total, Anthropic reports them separately, so the check reads each provider's own field — the Anthropic-only arithmetic is a corrected defect (review fix 5), not the contract. → **GREEN, live.** `p2_16_anthropic_prompt_caching_actually_reads_from_cache`: call 1 `cache_write: 11923` → call 2 `cached_read: 11923`. OpenAI: `input: 8659` → `input: 211, cached_read: 8448`, which also validates the adapter-owned normalisation against the real wire.
      Source: spec `02 §11 G-CACHE-HIT` (merged with spec `03 §6 G-CTX-CACHE`) · Catches: zero prompt caching anywhere (`G2a T1`); over-reported cost on three of four providers · Covers: **R-2** · Evidence: _pending_

- [ ] **TG2-06** `[determinism]` — The prefix builder produces **byte-identical** output across 100 invocations with identical inputs, in one process and across processes. Map iteration order, `HashMap` serialization, timestamps, PIDs and locale must not reach the prefix. Asserted on the serialized bytes, not on a structural equality. → **AMBER.** `cache::prefix` tests pin identical-inputs-identical-hash, tool order not affecting the prefix (the cross-process `HashMap` hazard), and volatile content staying out of the hashed region. **Not asserted over 100 invocations, and not across processes** — the cross-process half needs a spawned binary.
      Source: spec `02 §11 G-CACHE-STABLE` (merged with spec `03 §6 G-CTX-STABLE`) · Catches: non-deterministic key order; a volatile byte defeating the prefix · Covers: **R-2** · Evidence: _pending_
      → the **system-prompt-assembly clause is owned by P3**: the prefix is `tools → system → messages`, and the system half is built by P3's context strategy. Byte-stability of the full prefix is only provable once P3 lands; P2 proves the tools + static-system portion.

- [x] **TG2-07** `[perf]` — A prefix below the model's documented cache minimum produces a **warning that a test can observe** — the cache-hit alarm — rather than a silent non-cache. Per ADR-01-05 the alarm ships **before** the cache, so this gate must be green before `TG2-05` can be trusted. → **GREEN, and it shipped first.** `cache::monitor` was built and falsified before any `cache_control` reached a request. `cache::prefix::will_cache` warns below the minimum, and the monitor stays silent in the three states where zero cached reads is correct — an alarm that fires when nothing is wrong gets muted.
      Source: spec `02 §11 G-CACHE-MIN` · Catches: silent non-caching below the per-model minimum · Covers: **R-2** · Evidence: _pending_

- [ ] **TG2-08** `[conservation]` — A turn carrying reasoning/opaque provider bytes is persisted, reloaded, and replayed to the provider, and the provider **accepts it**. The engine never interprets, reorders or re-serializes `Item::Opaque` — asserted byte-for-byte through the persistence round trip. → **AMBER, and now half-proven by accident.** `MessageContent::ToolUse::opaque` is exactly this contract: an opaque provider token carried verbatim and never interpreted. The live round trip proves the **provider accepts it** — Gemini rejects a replayed call whose `thoughtSignature` is missing, and it no longer does. **The persistence round trip is untested**: nothing yet saves a turn to disk, reloads it, and replays it.
      Source: spec `02 §11 G-OPAQUE` · Catches: dropped signatures; xAI cache misses caused by mutated opaque bytes · Covers: **R-7** · Evidence: _pending_

- [x] **TG2-09** `[correctness]` — Every response yields all six `Usage` fields, and **streaming yields non-zero input tokens** on all three cloud providers. No field is estimated, inferred or defaulted; a provider that does not report a field yields an explicit absence, never a zero that reads as a measurement. → **GREEN.** Six-field `Usage` on all three, with `message_start` handled so streaming reports non-zero input (P2-43) and OpenAI's empty-`choices` usage frame no longer discarded (P2-42). A turn that returns content and reports zero tokens is an explicit error, not a $0 turn (P2-52).
      Source: spec `02 §11 G-USAGE` · Catches: `P-30` — input tokens 0 for all three cloud providers in streaming · Proves: **INV-7** · Evidence: _pending_

- [x] **TG2-10** `[correctness]` — Known models price correctly, including across the 200k context cliff and with cache-read/cache-write priced at their own rates; and **an unknown model returns an explicit error** rather than a default rate. The `_ => (1.0, 3.0)` shape must not exist. → **GREEN.** `cost::pricing` — exact-id table, tiered pricing across Google's 200k cliff, cache reads and writes at their own rates, and an unknown model returns an error. `TG-P2-02` holds the picker and the table together; `changes_on` fails the build when a published price change date passes.
      Source: spec `02 §11 G-PRICE` · Catches: `C-02` — the fallback catching 7 of 11 models · Proves: **INV-7** · Evidence: _pending_

- [x] **TG2-11** `[security]` — With the OS credential store **empty** and every provider environment variable set (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, …), the app **refuses to run a turn** and says why. A development `.env` is build-time only and is never a runtime fallback. **On Linux with no D-Bus Secret Service**, the encrypted-file fallback under the app data root is used, is created `0600`, and the active store is reported to the user — CI runs the same path so shipped and tested binaries do not diverge. → **GREEN.** `TG-P2-07` proves no shipped code reads a provider key from argv or the environment — a stronger claim than observing one refusal — falsified with a planted `env::var`. The Linux encrypted-file fallback exists (`vault/fallback.rs`, `0600` set AT CREATION), and `vault_backend_info` reports the active store. ⚠ **Not yet covered: a CI job that exercises the fallback path on a D-Bus-less runner.** Carried to P3.
      Source: spec `02 §11 G-KEY-NOENV`, sharpened by **G-06** (the Linux credential store), whose amendment to INV-12 no spec turned into an assertion · Catches: a test credential reaching production; `airsctl` reading a key from a CLI flag and plaintext TOML (`G0 §5.6`); `ci.yml:64`'s admission that Linux has no Secret Service · Proves: **INV-12** · Evidence: _pending_

- [ ] **TG2-12** `[correctness]` — Select a provider and model, restart the app, send a message: the **same** model serves it, and the UI shows it as active. Persistence is asserted by reading the row back through the startup hydration path, not by asserting the write.
      Source: spec `02 §11 G-MODEL-PERSIST` · Catches: `set_active_provider` never persisted; BUG-MODELSWITCH · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG2-13** `[correctness]` — Temperature, `top_p`, reasoning effort and thinking budget set in the UI appear in the **outbound request body**. `ModelConfigsDb` is written *and read*; a config with no reader fails this gate.
      Source: spec `02 §11 G-MODEL-CONFIG` · Catches: `ModelConfigsDb` written, never read (`G5c F01`) · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG2-14** `[seam]` — No reference to Ollama remains in Rust source, `Cargo.toml`, shipped UI copy, settings schemas, or docs shipped in the bundle. Azure likewise. Exactly four providers exist: Anthropic, OpenAI, Google, xAI.
      Source: spec `02 §11 G-NO-OLLAMA` · Catches: incomplete removal leaving a fifth provider half-present · Evidence: _pending_

- [ ] **TG2-15** `[data]` — "Today" is the **user's** today. Property-tested at UTC−12, UTC+0 and UTC+14: `get_today_activity` and `run_daily_rollup` bucket a spend at 23:30 local into that local day, and the daily rollup completes at least once within a sub-hour session.
      Source: spec `08 §4 G-TZ` · Catches: `IN-61`, `IN-62` — `chrono::Local` compared against SQLite's UTC `DATE()`, omitting 10 hours and including 10 of the next; `IN-56` — a rollup that sleeps an hour and only aggregates yesterday · Proves: **INV-7** · Evidence: _pending_

- [ ] **TG2-16** `[governance]` — **NEW (uncovered risk R-9).** The full conformance suite (`TG2-01…TG2-10`) runs **offline** against the `fx-provider-replay` corpus with every provider credential absent from the environment, and produces the same verdicts as the live run. Live-provider runs are a separate, explicitly-invoked job — never the default, and never a prerequisite for a PR to pass.
      Source: **NEW** — `00 §9.2 R-9`'s mitigation names "a conformance harness (ADR-03-05) rather than ad-hoc live calls" but no spec gates it; without this gate `TG2-01` cannot run in CI at all · Catches: the planning run's own account-limit hit; a credential-shaped variable silently enabling live spend (INV-12's *presence is not intent*) · Covers: **R-9** · Evidence: _pending_

- [ ] **TG2-17** `[policy]` — **NEW (uncovered mechanic M-42 / decision G-04).** A user-set **daily spend ceiling is enforced in the provider layer** — the single point every call passes. Crossing it **pauses the fleet, surfaces the pause, and refuses the next call**; it never fails silently and never merely logs. A **global kill switch** halts all agent work immediately, including in-flight worker ticks and broadcasts. A worker broadcast is capped per tick.
      Source: **NEW** — M-42 / `00 §Group G G-04`; spec `04 §8 G-BUDGET-FIRES` gates only the **per-task** budget, which G-04 explicitly demotes to advisory · Catches: `with_task_budget` with zero callers; no daily ceiling; no off switch; a broadcast firing one full LLM tick per worker uncapped (`G9 F-BUD-04`) · **FINDING: no Part-9.2 risk names the runaway-spend failure, though under BYOK the bill is the user's — the risk log has a hole here** · Evidence: _pending_
      → the **visible kill-switch surface** clause is **owned by P6** (it is a UI control); the provider-layer enforcement and the pause behaviour must be green at P2 and are testable headlessly.

---

## 6. P3 — The runtime

**Slice:** context strategy (M-05) · one tool path (M-08) · truthful `ToolContext` + a live permission gate (M-09) · in-process subagents (M-02) · memory in-house (M-10).
**Exit gate (Part 6):** a long session's context stays bounded and measurably cached; the recursion guard, ACP identity and build-evidence gate all demonstrably fire; subagents run with no sidecar.

> **P3 is the largest gate block in the programme (39 of 115).** That is not an accident of assignment: M-09 alone is *"one fix, three capabilities"* (spec `04 §1`) — it is the single change that makes the recursion guard, ACP sender identity and the `qc_review` build-evidence gate stop being inert, and every gate that proves a guard fires depends on it.

### 6.1 The loop (spec `01`)

- [ ] **TG3-01** `[correctness]` — Every exit path emits **exactly one** `TurnEnded`. Parameterised over: normal completion, tool-limit reached, user cancel, provider error, unsupported provider, budget exceeded, and a panic contained by `TG3-39`. Zero events and two events are both failures.
      Source: spec `01 §8 G-TERM` · Catches: `engine.rs:299/164/92`, `agent_commands.rs:134-137` · Evidence: _pending_

- [ ] **TG3-02** `[conservation]` — After a turn ending in **each** `StopReason`, reloading the session from disk reproduces every item in order, with its usage, its tool calls and its opaque bytes. An assistant item with empty content persists as empty, not as a poisoned or dropped record.
      Source: spec `01 §8 G-PERSIST` · Catches: the last-message-only save; the empty-content poisoning · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG3-03** `[correctness]` — Cancel during (a) a provider stream, (b) tool execution and (c) a pending permission request all return **within the declared timeout** and release every resource: no leaked task, no held permit, no undrained `PendingPermissions` entry. A permission whose listener has died must time out rather than block the engine.
      Source: spec `01 §8 G-CANCEL` · Catches: the permission oneshot with no timeout (`G5a §8`); the undrained `PendingPermissions` · Proves: **INV-9** · Evidence: _pending_

- [ ] **TG3-04** `[determinism]` — `ToolContext` fields are **non-default at every construction site**. Asserted two ways: a static check that no construction site passes `None`/`0` for `agent_role`, `depth` or session identity; and a runtime assertion that `agent_role` matches the invoking agent and `depth` reflects real nesting during a subagent call.
      Source: spec `01 §8 G-CTX-TRUTH` · Catches: `T48`/`T49`/`T50` — hardcoded at `agent/tool_executor.rs:89`, `:252`, `:316`, set truthfully in exactly one place (`mcp_server/tool_adapter.rs:280`) · Evidence: _pending_

- [ ] **TG3-05** `[security]` — A subagent spawning a subagent past the configured depth cap is **refused**, with the refusal observable to the caller. The guard is exercised through the production construction path, not by hand-building a `ToolContext` the production code never builds.
      Source: spec `01 §8 G-DEPTH` · Catches: the inert recursion guard (a direct consequence of `T48`) · Evidence: _pending_

- [x] **TG3-06** `[correctness]` — N fire-and-forget subagent spawns beyond the pool size still permit spawn N+1 once they finish. Permits are released by `Drop`, not by an explicit call, so an aborted or panicking spawn cannot leak one.
      **`GATE-PROVEN` 2026-09-02 (P3-22, `TG-P3-22`).** Proven by execution: fill the global cap, confirm the next acquire blocks, release one, confirm the next acquire succeeds. ⚠ The gate's own clause — *"permits are released by `Drop`, not by an explicit call"* — was **true and insufficient**. `PoolPermit` did have a `Drop` impl; the leak was that the permit lived inside the registry entry, which was retained for `TERMINAL_RETENTION` and released only by a prune that `spawn` called **after** the `acquire` waiting on those very permits. `prune_terminal`'s only other caller is `list()`, driven by the UI — so **with the subagent monitor panel closed, 16 completed subagents blocked the 17th spawn permanently**. Drop still covers the aborted/panicking case; termination now releases earlier, which is stricter, not weaker.
      Source: spec `01 §8 G-POOL` · Catches: `S-03` — the permit leak that deadlocks after 16 spawns · Proves: **INV-9** · Evidence: _pending_

- [x] **TG3-07** `[seam]` — Static check: **exactly one** tool-execution entry point exists and it has callers. Every other execution path is deleted, not merely unused — an unreferenced second path is a defect waiting for a caller.
      **`GATE-PROVEN` 2026-09-02 (P3-64, `TG-P3-11`).** `execute_batch` is the only `pub async fn execute*` on `ToolExecutor`, and it has exactly one production caller (`engine.rs:738`). The second path, `ToolExecutor::execute()`, was **deleted** rather than left unreferenced. ⚠ The gate's phrasing — *"an unreferenced second path is a defect waiting for a caller"* — proved exactly right in a way it did not anticipate: the surviving path's **concurrent read partition performed no permission check at all**, running read-only tools under `// Execute (read-only always approved)` with `_mode` and `_always_allow` bound underscore-prefixed to silence the warning. One entry point is necessary and was not sufficient. Permission is now decided **before** the partition, once per tool, keyed on `ToolEffect`.
      Source: spec `01 §8 G-ONEPATH` · Catches: `ToolExecutor::execute()`, the only correct path, with zero callers (`G2a`) · Evidence: _pending_

### 6.2 Context and memory (spec `03`)

- [ ] **TG3-08** `[perf]` — A 30-turn session's **input tokens do not grow linearly** with turn count: measured against a recorded baseline, the regression slope over turns 10–30 is bounded, and the bound is asserted as a number.
      ⚠ **NOT satisfied by `P3 A–E`, deliberately.** `S3-01` asserts the *ordering* that bounds growth — the clear trigger fires within the window, compaction does not fire at or below it, a full window does trigger compaction, and the estimator responds to size. This gate asks for something stronger: **a regression slope over turns 10–30, measured against a recorded baseline and asserted as a number.** That needs a recorded baseline corpus that does not exist yet. Left unchecked rather than checked against the weaker property it resembles.
      Source: spec `03 §6 G-CTX-BOUND` · Catches: BUG-TOKENS (b) — the full-history clone (`G2a T2`) · Proves: `00 §7.3` product proof 2 · Evidence: _pending_

- [ ] **TG3-09** `[correctness]` — With a small configured context window, compaction **triggers**, and the **compacted history is what the next turn actually sends** — asserted on the outbound request bytes, not on the compaction function's return value.
      Source: spec `03 §6 G-COMPACT-FIRES` · Catches: BUG-TOKENS (c,d) — a trigger that is unreachable *and* a result that is discarded · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG3-10** `[perf]` — `clear_tool_uses` fires **only** when the tokens saved exceed the cost of the forced cache re-write it causes. The decision is a stated formula with an asserted threshold; clearing that costs more than it saves is a failure.
      ⚠ **The gate's PREMISE is refuted, measured 2026-09-02 (`P3-28`).** It requires `clear_tool_uses` to fire only when the saving exceeds *"the cost of the forced cache re-write it causes"*. **There is no such cost.** Clearing edits the message region, which would forfeit a cache write only if a breakpoint were set there — and `anthropic.rs` marks the **system block only**. A threshold derived from that cost would be derived from zero. `P3-25`/`P3-28` therefore state the formula against what clearing *does* cost (tool-result content the model may still need): trigger at 60% of the window with compaction at 80%, `keep = 3`, `clear_at_least = 5%`, and the `0.60 < 0.80` inequality is asserted. **Left unchecked because the gate as written asks for a different number than the one that exists.** It must be re-pointed — or the premise restored by adding a message-region breakpoint — before it can pass. Effect measured live regardless: 11,310 → 2,750 input tokens, 75.7%.
      Source: spec `03 §6 G-CLEAR-WORTH` · Catches: a context edit that increases spend · Evidence: _pending_

- [ ] **TG3-11** `[conservation]` — A memory known to have been recalled appears in the **outbound prompt bytes**. The assertion reads the serialized request, not the recall function's return — this is INV-8's canonical instance.
      Source: spec `03 §6 G-MEM-READ` · Catches: recall that reaches nothing · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG3-12** `[correctness]` — Two **different** queries in one session produce **different** recall sets, and the same query twice produces the same set. The vector query is the caller's query, not a fixed template; the cache key includes the query. **Two sharpened clauses close ported defects the spec listed but gated nowhere:** (i) a newly stored memory is retrievable on the **next** recall — the cache is invalidated on write, not up to 30 minutes later (defect 7); (ii) a negative-result marker, if written, is **honoured** on the next identical query — a marker written on every empty result and never read is a writer with no reader (defect 6).
      Source: spec `03 §6 G-MEM-QUERY`, sharpened to cover spec `03 §4.3` defects 6 and 7, which the fix-list names and no gate claimed · Catches: ported defects 1, 2, 6 and 7 (`G4c §5.4`) — query-independent semantic recall; an episodic cache key of `episodic:{user}:{session}`; a negative cache gated behind `if not strict_lossless` with `strict_lossless: true`; a semantic cache never invalidated on write · Covers: **R-3** · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG3-13** `[security]` — Scope is a **mandatory filter, applied inside the KNN**: agent A cannot recall agent B's episodic memory; project A cannot recall project B's facts; a task-scoped memory is unreachable from another task. Asserted with a selective scope over a large corpus — a scope applied as a post-filter over top-k returns few or zero results and **fails this gate**.
      Source: spec `03 §6 G-MEM-SCOPED` (merged with spec `03 §6 G-MEM-SCOPE`, which restated it) · Catches: ported defect 5 — episodic recall never forwarding `agent_id` despite documented "defence in depth"; defect 4 — procedural recall with no type filter; **G-12**'s finding that scope must be a declared `vec0` partition key · Covers: **R-3** · Evidence: _pending_

- [ ] **TG3-14** `[conservation]` — Every declared memory subtype has **at least one successful write** in a full end-to-end run, and its reader returns it. A subtype that is declared, has a reader, and has never once been written fails.
      Source: spec `03 §6 G-MEM-SUBTYPES` · Catches: ported defect 3 — `learn_hitl_pattern` always throwing on a bad argument, its `TypeError` swallowed, so no `hitl_interaction` record has ever existed · Proves: **INV-8** · Covers: **R-3** · Evidence: _pending_

- [ ] **TG3-15** `[perf]` — With a graph present, context selection sends **measurably fewer files** than without, **and the right ones**: precision and recall against the `fx-graph-repo` corpus's labelled relevant-file set meet stated thresholds. Fewer-but-wrong fails.
      Source: spec `03 §6 G-GRAPH-SCOPE` (merged with spec `05 §5 G-GRAPH-SCOPE`) · Catches: BUG-GRAPH (b) — a graph that is built and read by nothing (`G2a`) · Proves: **INV-8** · Evidence: _pending_
      → **owned by P5.** M-18 (graph-scoped context) is in the P5 slice; P3 builds the selection seam, P5 fills it. P5 cannot close until this is green.

- [x] **TG3-16** `[conservation]` — Every tool's output is bounded: `grep` bounds bytes **per line as well as in total**, `list_directory` bounds entries, `read_file` bounds bytes, and each states in its result that truncation occurred. An unbounded tool result is an unbounded context entry.
      **`GATE-PROVEN` 2026-09-02 (P3-29).** All four clauses, and two of them were not met by `P3-29`'s task text alone — the task named grep and `list_directory`, this gate is broader, and checking it honestly required going past the task. **grep** bounds **per line** (`MAX_LINE_BYTES = 2 KB`; it was `line.to_string()`, so one match on a minified bundle line put tens of MB into context while `max_results` reported nothing wrong) **and in total** (`MAX_TOTAL_BYTES = 256 KB`; the per-line bound alone leaves the product unbounded, since `max_results` is caller-raisable and each match carries context on both sides — whole matches are dropped rather than the JSON string cut, because a truncated document the model cannot parse is worse than fewer results). **`list_directory`** bounds entries — the cap was checked only at function entry, never in the push loop, making it **fully inert when `recursive=false`**. **`read_file`** bounds **bytes** (`256 KB`), which it did not: it bounded LINES only, and 2,000 lines of base64 or minified output passes a line cap untouched. **Each states truncation in its result** — `truncated=true` for grep and `list_directory`, an explicit `[⚠ Output truncated …]` note for `read_file`, on both the line cap and the byte cap. ⚠ `list_directory`'s signal was `entries.len() == max_entries`, which is `false` **exactly when the cap was overshot** — the one indicator that would have revealed the inert cap reported clean.
      Source: spec `03 §6 G-TOOLCAP` (and spec `03 §2.3`) · Catches: unbounded context entry from a single tool call · Evidence: _pending_

- [ ] **TG3-17** `[correctness]` — With the embedding model **absent or still downloading**, recall still returns results via FTS5/BM25 plus metadata filtering, **reports itself degraded**, and never returns empty and never errors. Degradation is a state the caller can observe, not an inference from an empty list.
      **Sharpened clause:** every circuit breaker that is **constructed** in the retrieval path is **used** — a breaker built and never consulted is a guard that cannot fire (spec `03 §4.3` defect 4, whose type-filter half is gated by `TG3-13` and whose breaker half was gated nowhere).
      Source: spec `03 §6 G-MEM-DEGRADE`, sharpened to cover the unused-breaker half of ported defect 4 · Catches: a skipped or incomplete model download turning memory off silently; a cold-fetch path with a constructed-but-unused circuit breaker · Covers: **R-3** · Evidence: _pending_

- [ ] **TG3-18** `[perf]` — Embedding **never delays a turn**: a memory write enqueues and embeds behind the turn, measured as turn latency unchanged within noise against a no-embedding baseline. Inference runs on `spawn_blocking` — asserted by proving other Tokio tasks make progress during a batch of embeddings, since `fastembed` is synchronous and Tokio-free.
      Source: spec `03 §6 G-MEM-ASYNC` · Catches: embedding on the critical path; a sync model call starving a Tokio worker · Evidence: _pending_

- [ ] **TG3-19** `[ux]` — The first-run model download shows progress, **resumes after interruption**, and **can be skipped** — and skipping yields a working, degraded app. The model cache path is set explicitly under the app data root: `fastembed`'s default `.fastembed_cache` is **relative to process CWD**, which on a macOS `.app` is `/`, and that is an INV-5 violation on the newest subsystem.
      Source: spec `03 §6 G-MEM-DOWNLOAD`, sharpened by **G-14** (whose CWD finding no gate carried) · Catches: a ~130 MB silent stall at first launch; `.fastembed_cache` written outside the app data root · Proves: **INV-5** · Evidence: _pending_
      → the **progress-and-skip UI** clause is **owned by P7** (M-35's first-run flow); the resume, skip-to-degraded and cache-path clauses must be green at P3 and are testable headlessly.

- [ ] **TG3-20** `[data]` — Every stored memory carries a `source` from the declared set (`user`, `agent_observation`, `repository`, `tool_result`, `test_result`, `conversation`); a repository-sourced memory carries **file and line**. A memory with a null or unrecognised source is rejected at write, not stored and flagged.
      Source: spec `03 §6 G-MEM-PROVENANCE` · Catches: unauditable memory; the inability to distinguish "the user told me" from "I observed it" · Evidence: _pending_

- [ ] **TG3-21** `[policy]` — `remember_this` **no longer exists** in the tool registry (static check). A `propose_memory` proposal that fails the deterministic gateway's importance / novelty / duplicate / confidence check is **not stored**, and the agent is told **which check it failed**. A proposal that passes is stored and is retrievable. Memory has a writer and a gate from day one (**G-03**).
      Source: spec `03 §6 G-MEM-GATEWAY` · Catches: unrestricted agent writes; the ADR-05-04 boundary · Evidence: _pending_

- [ ] **TG3-22** `[correctness]` — Recall@5, Recall@10 and MRR over the `fx-memory-labelled` corpus meet thresholds recorded in this document at the Phase-2 reranker's landing, and **do not regress** against the previous recorded run. Thresholds are numbers, checked in, and a regression fails the gate.
      Source: spec `03 §6 G-MEM-RECALL-QUALITY` (the anchor's `§23` retrieval suite) · Catches: retrieval quality drifting invisibly · Evidence: _pending_
      → **FINDING:** spec `03 §4.6` still asserts the anchor's `§10` reranker *"is"* the ported competitive model. **G-13 overturned that** — 6 of 8 terms and 7 of 9 are disjoint — so this gate's thresholds must be set against the **anchor's** model, not the ported one. Spec `03 §4.6` is stale and should be corrected.

- [ ] **TG3-23** `[correctness]` — Over a repeated task against `fx-memory-labelled`, the number of **rediscovered facts falls** run over run: the agent re-derives measurably less of what it already knows. This is the anchor's real metric and the only one that distinguishes memory that works from memory that merely exists.
      Source: spec `03 §6 G-MEM-VALUE` · Catches: memory that exists but prevents no work · Evidence: _pending_
      → **owned by P4.** The metric requires a task to run end to end, which is P4's exit gate. P4 cannot close until this is green.

### 6.3 Tools, permissions and sandboxing (spec `06`)

- [ ] **TG3-24** `[security]` — Every tool passes **exactly one** permission check, keyed on `ToolEffect` (`Read` / `Write` / `Execute` / `Network` / `Board`). Static check: no code path reaches an effect without traversing the gate. `is_read_only` gates nothing — it reverts to the concurrency flag it was written as. Adopted as the decided permission model by **G-11**.
      Source: spec `06 §7 G-PERM-GATE` · Catches: `G10-111`; `Tool::requires_permission()` never called (`G2a T47`) · Evidence: _pending_

- [ ] **TG3-25** `[policy]` — Each `PermissionMode` **measurably changes prompting behaviour**: parameterised over every mode, the same tool call produces a different, asserted prompt-or-not outcome. A mode that behaves identically to another fails. Delegate mode in particular grants zero file tools — any divergence from that documented promise is a product-promise defect, not a config detail.
      Source: spec `06 §7 G-PERM-MODE` (and `§2`) · Catches: the unconditional `ManualApproval`; `G10 §1.4`'s Delegate-mode divergence · Evidence: _pending_

- [ ] **TG3-26** `[security]` — **No workspace content leaves the machine without a recorded, revocable consent** for that workspace **and** that destination. Unattended work runs under the standing consent; **its absence blocks egress** rather than prompting a machine nobody is watching. Asserted three ways: consent present → egress proceeds and is logged; consent absent → egress refused; consent revoked → the next egress is refused.
      Source: spec `06 §7 G-PERM-NETWORK`, as resolved by **G-09** · Catches: `analyze_diff` shipping the diff off-machine; the decided-spec collision with `04 §8 G-QC-ANALYZE` · Evidence: _pending_

- [ ] **TG3-27** `[security]` — Static check: **every tool taking a path calls the jail** — including the read tools, which bypass it entirely today and are auto-approved. A new tool with a path parameter and no jail call fails the build.
      Source: spec `06 §7 G-JAIL-COVERAGE` · Catches: the six bypassing tools; `G2a T12`/`T13` · Proves: **INV-5** · Evidence: _pending_

- [ ] **TG3-28** `[security]` — An unnormalisable or escaping path is **refused, never rewritten**. Property-tested over the `fx-paths-hostile` corpus: `..` traversal, symlinks, Windows junctions, UNC paths, case variants, trailing separators, reserved device names, long paths, and unicode-normalisation collisions. Paired with the `fx-paths-benign` **no-fire** corpus: every legitimate path must be **accepted** — a jail that refuses everything passes a refusal-only test and breaks the product.
      Source: spec `06 §7 G-JAIL-REFUSE` · Catches: `G10-87`, `G10-88` — silent path rewriting · Proves: **INV-5** · Evidence: _pending_

- [ ] **TG3-29** `[determinism]` — All spellings of one path take the **same** mutex, across every caller. Canonicalise, resolve symlinks, unify separators, case-fold where the platform is case-insensitive — one shared helper, asserted to be the same helper used by the graph path key (`TG5-02`) and checkpoint retention.
      Source: spec `06 §7 G-LOCK-KEY` · Catches: `G10-09`, `G10-92` — `C:\a\b.rs` and `c:\a\b.rs` as two mutexes for one file, on Windows *and* macOS; the same defect in three subsystems (`G3b GI-28`) · Evidence: _pending_

- [ ] **TG3-30** `[correctness]` — No lock is held across an unbounded await. A hung child process cannot block another write: asserted by holding a subprocess open past its timeout and proving an independent write on a different path completes.
      Source: spec `06 §7 G-LOCK-BOUND` · Catches: `G10-10` · Proves: **INV-9** · Evidence: _pending_

- [ ] **TG3-31** `[correctness]` — Every spawned process has a timeout **and is killed on drop**. Asserted by dropping the handle of a process that ignores SIGTERM and proving the OS process is gone.
      Source: spec `06 §7 G-SUBPROC-TIMEOUT` · Catches: `G10-102` · Proves: **INV-9** · Evidence: _pending_

- [ ] **TG3-32** `[correctness]` — A **missing validator binary is an error**, surfaced as one. `"no syntax errors"` is never printed for a file that was not actually checked — the string must be unreachable on any path where the validator did not run.
      Source: spec `06 §7 G-VALIDATOR-HONEST` · Catches: `G10-16`; `validate.rs` as advisory dressed as verification (`§3.4`) · Evidence: _pending_

- [ ] **TG3-33** `[security]` — **No executable is resolved from the workspace under inspection.** A workspace containing a `node`, `cargo`, `python` or `.cmd` shim on a path the resolver would search must not have it executed; resolution comes from the controlled toolchain path only.
      Source: spec `06 §7 G-NO-WORKSPACE-EXEC` · Catches: `G10-86` — arbitrary code execution from a repository the agent was asked to read · Evidence: _pending_

- [ ] **TG3-34** `[correctness]` — Build evidence originates from a **Rust-executed command's exit code**. A model-authored or model-narrated command that merely *mentions* a build records **nothing**; a substring match against tool output can never produce an evidence row.
      Source: spec `06 §7 G-EVIDENCE-EXACT` · Catches: `G10-96`; the whole of `06 §3.3` — a hard gate satisfied by a substring · Evidence: _pending_
      → **owned by P4.** M-13 (Rust-native build verification) is the P4 slice; P3 removes the substring path, P4 supplies the exit-code path. Pairs with `TG4-07`, which gates the storage side.

- [ ] **TG3-35** `[correctness]` — Truncation is **char-boundary-safe at every site**. Property-tested over multi-byte and combining-character inputs at every truncation call. One shared helper — the correct implementation already exists at `validate.rs:846-853`.
      Source: spec `06 §7 G-UTF8-TRUNC` · Catches: the three identical byte-slice panics at `bash.rs:134-141`, `remember_this.rs:94`, `annotate_file_tool.rs:99` · Evidence: _pending_

- [ ] **TG3-36** `[governance]` — Every registered tool's description states **what the tool does, its constraints, and nothing it does not do**. A description promising behaviour the code does not implement fails — checked against a per-tool assertion list, not by prose review. A docstring that contradicts its code is a bug, not a style issue.
      Source: spec `06 §7 G-TOOLDESC` (and `§4`) · Catches: the 33 defective descriptions; `delete_file` documented as "empty directory" while running `remove_dir_all` (`GI-05`) · Evidence: _pending_

- [ ] **TG3-37** `[conservation]` — A provisional cost charge is **reconciled or released on every exit path**, including each error return. Parameterised over all eight error returns of the charging site; an unreleased provisional charge is a leak that becomes a self-inflicted denial of service.
      Source: spec `06 §7 G-PROVISIONAL-COST` · Catches: `G10-67` — a $0.10 provisional charge before eight error returns, never reconciled, tripping the $5 hourly ceiling and blocking all QC for an hour at zero real spend · Evidence: _pending_

### 6.4 Gates added at P3

- [ ] **TG3-38** `[security]` — **NEW (uncovered decision G-07 / mechanic M-40).** `redact_dict` and its **22**-entry `SENSITIVE_KEYS` list are ported (⚠ the count was 20 here and in the plan; the source has 22, counted in `P1-20`), **together with the second Python redactor** (`PIIRedactor`) that neither document mentioned, and redaction is applied at **every boundary where content leaves the process or enters durable storage**: outbound prompts, logs, memory writes, analytics, and any egress. Proven with the `fx-secrets` corpus — every planted credential is absent from the destination — **and with its no-fire half**: strings that merely resemble secrets (a base64 blob, a UUID, a git SHA, a licence key in a test fixture) survive **unredacted**, because a redactor that eats legitimate content is its own defect.
      Source: **NEW** — `G4b §A.14` ruled this **PORT** ("the `SENSITIVE_KEYS` list is the spec"); the plan's own correction #1 records that `redact`/`scrub`/`sanitiz` appear **zero times** across all eleven documents, and G-07 makes it a condition of shipping memory · Catches: `G4:486` — unredacted tool arguments landing in procedural memory verbatim, which now means **permanently** · Proves: **INV-2** (its spirit: no secret leaves the process), supports **INV-12** · **Evidence (P1): `GATE-PROVEN` + `SMOKE-PROVEN`, partial — the memory-write half is still P3.** Six tests named `tg_p1_06_*` in `ci_gate.rs` (the `tg_p1_NN` prefix is `ci_gate.rs`'s own sequence and does **not** track this register's IDs — `tg_p1_06` is *this* gate, not `TG1-06`): every `tool.execute` is wrapped in `redact_result`; **raw tool *input* reaches exactly one site, the permission dialog** (added after the audit — the first fix covered results only, and `execute_batch`'s write branch went on handing raw arguments to the UI and the `messages` table); every named boundary reaches the redactor; the 22-key list is intact; and two detector tests prove the checks can fail. Behavioural proof is `tests/s1_04_secret_never_reaches_the_prompt.rs` (three tests over `execute_batch`'s read branch, write branch and a mixed batch) plus `tests/redaction_boundaries.rs`. All falsified. ⚠ **Two clauses of this gate are NOT green and are not claimed to be.** The *no-fire half* is only partly built — `author`, `passed`, `prompt_tokens`, commit SHAs and code-shaped values are pinned by tests, but there is no `fx-secrets` corpus yet. And **egress is deliberately not redacted at the request body** (`ADR-LOG-RESTRUCT-0019`): what reaches durable storage from that path — `record_api_failure` and the log appender — is covered, the wire is not. The box stays unticked until P3 builds the corpus and the memory-write half
      → the **memory browser / export / forget** surfaces that G-07 also decided are **owned by P6** (`TG6-07` routes them); the redaction-on-write path must be green before memory stores anything.
      → **runnable from P1.** The MDP builds redaction in **P1** (*"redaction restored"* is a P1 headline outcome), earlier than Part 6 implies. This gate is therefore expected green from P1 onward; the ID stays `TG3-38` because `§1` forbids renumbering, and P3 remains the phase whose memory writes it must cover.

- [ ] **TG3-39** `[correctness]` — **NEW (uncovered risk R-6 / decision G-01).** The release profile is `panic = "unwind"` (static check on `Cargo.toml`), and **every agent turn and every subagent runs inside `catch_unwind`**. An induced panic inside a worker turn fails **that turn**, reports it through the normal error contract, and leaves the app and every other session alive — asserted by running a second session to completion after the panic.
      Source: **NEW** — M-41 / `00 §Group G G-01`; ten specs never mention fault isolation (`R1 B-04`) · Catches: `panic = "abort"` plus in-process workers, so one panic kills the app and every session; the sidecar's **unenumerated** role as a process boundary, removed by INV-1 and not replaced · Covers: **R-6** · Evidence: **`GATE-PROVEN`/`SMOKE-PROVEN` 2026-09-02 (`P3-01`/`P3-02`/`P3-03`, `TG-P3-01`…`TG-P3-04`, `S3-02`).** ⚠ **The line reference in this gate was itself the finding.** `[profile.release]` lived at `src-tauri/Cargo.toml` — a **non-root workspace member**, which Cargo discards with a warning — so the profile had never applied and `panic = "abort"` was never even in force in the way this gate assumed. It now lives at the workspace root as `panic = "unwind"`, `TG-P3-01` asserts it is where Cargo reads it, and the containment above it is real rather than configured.

---

## 7. P4 — The fleet

**Slice:** event dispatch + exhaustive state enum (M-11) · atomic claim (M-12) · Rust-native build verification (M-13) · delivery + handoff (M-15) · reconciliation timestamps (M-16) · coordination capabilities (M-31…M-34).
**Exit gate (Part 6):** a task runs end to end on **all three OSes** and produces a build-verified artifact in the user's chosen workspace; no worker can steal another's task; a stranded task is actually released.

> **Sequencing constraints that this phase's gates encode:**
> **(a)** `TG4-03` (atomic claim, M-12) must be green **before** `TG4-13` (affinity, M-33) is enabled, or routing continues to train on corrupted attributions (Part 6; **R-8**).
> **(b)** `TG3-04` (truthful `ToolContext`, M-09) must be green **before** the MCP HTTP server is deleted, or `TG4-10` regresses silently. **⚠ FINDING — see `§19 F-3`: G-15 moved that deletion to P0, four phases ahead of M-09.**
> **(c)** P4's exit gate names three OSes, which is why `TG4-17` and `TG4-18` are pulled forward from spec `09` into this phase.

- [ ] **TG4-01** `[determinism]` — Worker state is a Rust enum matched **exhaustively**; adding a variant without handling it **fails compilation**. And — the half the review added — an **unknown status string read from the database is a loud error, never a silent default**, with a `CHECK` constraint on the column. Without that half the enum merely relocates the dead latch into `FromStr`.
      Source: spec `04 §8 G-DISPATCH-EXHAUSTIVE`, corrected by review fix 4 · Catches: `SK-01`'s dead latch on `fallback_idle`; `src-shared` writing arbitrary strings into unconstrained `TEXT` from a second process (`R1 B-11`) · Evidence: _pending_

- [ ] **TG4-02** `[correctness]` — A transient database error does not terminate a worker's schedule: the worker retries and resumes. Injected mid-dispatch, a DB blip must not remove the worker from scheduling for the process lifetime.
      Source: spec `04 §8 G-DISPATCH-RESILIENT` · Catches: `SK-02`; `kick_inflight` released only by the spawned task's final line, so an abort or panic locks the worker out permanently (`SK-41`), and the kicked run awaited with no timeout (`SK-42`) · Proves: **INV-9** · Evidence: _pending_

- [ ] **TG4-03** `[conservation]` — Concurrent claims on one task: **exactly one wins**. A worker cannot claim a task assigned to another — the claim is one atomic statement that names the owner, and the one-task-per-worker guard lives inside the same statement rather than in a preceding read.
      Source: spec `04 §8 G-CLAIM-ATOMIC` · Catches: BUG-ASSIGN (b,c) — task theft (`G3d F-55`); the one-task guard failing open · Covers: **R-8** · Evidence: _pending_

- [ ] **TG4-04** `[correctness]` — An epic can **never** be assigned to or claimed by a worker as work. `assign_task` filters `task_type='epic'`; an epic inserted `status='ready'` is not assignable.
      Source: spec `04 §8 G-EPIC-NOT-TASK` · Catches: `F-EPI-02` · Evidence: _pending_

- [ ] **TG4-05** `[seam]` — The Supervisor can create an epic **through the tool surface it actually has** — asserted by driving the Supervisor's real registered tool set, not by calling the underlying function.
      Source: spec `04 §8 G-EPIC-REACHABLE` · Catches: `F-EPI-01` — a capability present in code and absent from the agent's registry · Evidence: _pending_

- [ ] **TG4-06** `[correctness]` — Completing a dependency **unblocks its dependents and progresses the epic**, in autonomous operation with no human step. Asserted end to end, from completion event to a dependent becoming claimable.
      Source: spec `04 §8 G-UNBLOCK` · Catches: `F-EPI-03` · Evidence: _pending_

- [ ] **TG4-07** `[conservation]` — Build evidence **survives a restart** (it is a row, not a 15-minute in-memory map); `qc_review` is **refused** without a passing record; and a **passing build is never recorded as failed**. The last clause is asserted on Windows specifically, against a real PowerShell and `cmd` invocation.
      Source: spec `04 §8 G-BUILD-EVIDENCE` · Catches: BUG-COMPILE — evidence in an in-memory map (`F-02`); the PowerShell/cmd mismatch recording passing builds as failed (`F-03`, `F-05`) · Proves: **INV-8** · Pairs with `TG3-34` (provenance side) · Evidence: _pending_

- [ ] **TG4-08** `[policy]` — Three consecutive build failures engage the debug step, and **the verdict still comes from the exit code**. The model reads the build output and proposes a cause; **Rust still composes the argv and still runs it**. A model-authored build command must be unreachable — a literal reading of ADR-02-02(B) would re-create BUG-COMPILE on the retry path (**G-05**).
      Source: spec `04 §8 G-BUILD-ESCALATE`, as decided by **G-05** · Catches: ADR-02-02's shape; the plan's own silent narrowing of the owner's answer · Evidence: _pending_

- [ ] **TG4-09** `[correctness]` — Agent output lands **under the resolved workspace** — never `~/.autodev/agents/`, never the process CWD — and the handoff **opens and runs it**. The workspace resolver **fails loudly**: it never returns an empty string, and a resolution failure aborts the write rather than defaulting.
      Source: spec `04 §8 G-DELIVER` · Catches: BUG-DELIVER — `load_workspace_dir()` returning `""` on error, three divergent fallbacks (`F-DEL`); `open_in_ide` with zero listeners; `graph_tools.rs:176` writing a bare `.autodev/GRAPH_REPORT.md` · Proves: **INV-5** · Evidence: _pending_

- [ ] **TG4-10** `[seam]` — Worker→worker messaging works **with the MCP HTTP server absent**. All three ACP tools resolve a truthful sender identity from the in-house construction sites; the self-message guard fires; a message storm is bounded.
      Source: spec `04 §8 G-ACP-IDENTITY` · Catches: `G9 F-XCUT-01`/`F-ACP-01` — `ToolContext.agent_role` set truthfully in exactly one place, `mcp_server/tool_adapter.rs:280`, so messaging functions **only** because workers reach tools through the server being deleted · Evidence: _pending_

- [ ] **TG4-11** `[conservation]` — A worker message is delivered **once and only once**, and an unacknowledged message is **recoverable** — the at-most-once implementation and the "lossless" claim must be reconciled into one asserted contract, and the gate asserts whichever the reconciliation chooses, not both.
      Source: spec `04 §8 G-ACP-DELIVERY` · Catches: the at-most-once / "lossless" contradiction · Evidence: _pending_

- [ ] **TG4-12** `[correctness]` — An unresolved supervisor-escalation request **cannot starve dispatch indefinitely**: it expires or is surfaced, and the fleet continues. Asserted by raising a blocker no one resolves and proving other tasks still progress.
      Source: spec `04 §8 G-ESCALATION-LIVENESS` · Catches: the Supervisor STOP starvation; a queue that is filled and never drained · Proves: **INV-9** · Evidence: _pending_

- [ ] **TG4-13** `[correctness]` — Autonomous task completion **records a routing outcome**, and after N outcomes routing **measurably changes**. The recorded attribution must name the worker that actually did the work — which is why this gate cannot be enabled before `TG4-03`.
      Source: spec `04 §8 G-AFFINITY-CLOSES` · Catches: `F-ROU-01`; affinity learning from corrupted attributions where theft overwrote `assigned_worker` while `routing_history` kept the old name · Covers: **R-8** · Evidence: _pending_

- [ ] **TG4-14** `[correctness]` — `suggest_worker` **never returns a system-defined agent** (Supervisor, QC) for implementation work. Asserted over the seeded default team, which ships as a default and is user-editable (**OQ-2**).
      Source: spec `04 §8 G-SUGGEST` · Catches: `F-ROU-02` — `suggest_worker` currently recommending the Supervisor · Evidence: _pending_

- [ ] **TG4-15** `[policy]` — A task exceeding its budget **trips the breaker and surfaces to the user**. Per **G-04** the per-task budget is advisory on top of the hard daily ceiling of `TG2-17`; this gate asserts the advisory path fires and is visible, not that it is the only stop.
      Source: spec `04 §8 G-BUDGET-FIRES` · Catches: `F-BUD-01`; `with_task_budget` with zero callers · Evidence: _pending_

- [ ] **TG4-16** `[correctness]` — The QC reviewer receives a repo path and `analyze_diff` **actually runs**, under the workspace's **recorded egress consent** (`TG3-26`). With no consent recorded, QC **degrades to the deterministic structural risk score and says so** — it does not silently ship the diff, and it does not prompt an unattended machine. The diff-classification call must not be capped at `max_tokens: 512`, which guarantees truncation.
      Source: spec `04 §8 G-QC-ANALYZE`, as resolved by **G-09** · Catches: `F-QC-01`; the decided-spec gate collision; `GI-94`/`GI-95` — a 512-token cap guaranteeing truncation while discarding the free deterministic risk score · Evidence: _pending_

- [ ] **TG4-17** `[portability]` — Static check: **no prompt, recipe, skill, hook or tool description contains POSIX-only shell syntax** (`tee`, `$?`, `;` chaining, `&&` in a `cmd /C` context, backticks) **or a hardcoded `/tmp`**. Every command is expressed platform-neutrally or carries a per-OS variant.
      Source: spec `09 §6 G-NO-POSIX` — **pulled forward from P9**, because P4's exit gate requires a task to run end to end on all three OSes and `SK-50` is precisely what makes that impossible today · Catches: `SK-50` (every worker recipe mandating `tee`/`$?`/`;` while the bash tool runs `cmd /C` on Windows ⇒ **no Windows task can reach QC**), `SK-51`, `SK-70` · Proves: **INV-6** · Covers: **R-10** · Evidence: _pending_

- [ ] **TG4-18** `[portability]` — Build, test, lint and run **intents** resolve to a correct `argv` on all three platforms, and `.cmd` shims resolve on Windows. Callers express intent; Rust picks the invocation. Asserted against the `fx-repo-{rust,node,python}` corpora on each OS.
      Source: spec `09 §6 G-CMD-INTENT` — **pulled forward from P9**: Part 6 lists **ADR-04-01 as a P4 dependency**, so the `Command` abstraction exists by P4; P9 hardens it under `TG9-01` · Catches: `SK-50`; `P-01…P-23`; `.cmd` shims unresolvable; the validator degradation · Proves: **INV-6** · Covers: **R-10** · Evidence: _pending_
      → **runnable from P0.** The MDP builds `autodev-core/platform/command.rs` at **P0-23**, earlier still. Expected green from P0; the ID stays `TG4-18` because `§1` forbids renumbering, and P4 remains the phase whose three-OS exit gate depends on it.

- [ ] **TG4-19** `[data]` — Every timestamp written is readable by **its** reader. Property-tested over a year of instants across DST boundaries and both directions, at every write/read pair. One format, one helper, across every database.
      Source: spec `08 §4 G-TIME-ROUNDTRIP` · Catches: `GI-01` — the orphan TTL that can never fire; `F-45`/`F-45b` — a QC-age alert always 0 s because SQLite `datetime('now')` is parsed as RFC3339 at 2+ sites; `IN-59`/`IN-60` · Proves: `00 §7.3` product proof 3 (the fleet recovers) · Evidence: _pending_

- [ ] **TG4-20** `[perf]` — `EXPLAIN QUERY PLAN` on **every** dashboard and dispatch query shows an index, not a scan. New queries are added to the gate's query list, so an unindexed query cannot be introduced silently.
      Source: spec `08 §4 G-INDEX` · Catches: `IN-63` · Evidence: _pending_

- [ ] **TG4-21** `[data]` — Every append-only table has a **written retention policy** (per **OQ-5**), and pruning keeps it under its cap: `token_usage`, `system_events`, `api_failures`, `worker_stats`, `hook_logs`. A table with no policy fails; a policy that never runs fails.
      Source: spec `08 §4 G-RETENTION` · Catches: `IN-64`, `SK-23` — unbounded growth on a desktop machine · Evidence: _pending_

- [ ] **TG4-22** `[data]` — A crash injected mid-multi-write leaves a **consistent board**: no half-assigned task, no orphaned claim, no epic with a phantom child. Each multi-statement mutation is one transaction.
      Source: spec `08 §4 G-TXN` · Catches: `SK-34` · Evidence: _pending_

- [ ] **TG4-23** `[security]` — **NEW (uncovered decision G-08 / mechanic M-43).** A **failed checkpoint blocks the write** — `auto_checkpoint_before_write` cannot return `()` and let the overwrite proceed; each task runs on its **own worker branch**; and **the two never disagree about current state** (checkpoints are per-write and short-lived, the branch is the durable record). The two competing checkpoint tables — `checkpoints` in `config.db` and `file_checkpoints` in `checkpoints.db` — **collapse to one**, asserted by `TG1-05`.
      Source: **NEW** — M-43 / `00 §Group G G-08` and its binding condition; spec `05 §5 G-REACHABLE` only proves the module is *compiled*, and spec `07 §8 G-DESTRUCTIVE` only proves the UI *confirms* — neither proves undo exists · Catches: ⚠ **`G3d F-44` is half wrong — corrected 2026-09-01.** `db/checkpoints.rs` is not compiled (true), but undo DOES exist via the live `git/checkpoint.rs`. The half that stands, and that this gate must prove, is **a failed checkpoint being invisible while the write proceeds** — `auto_checkpoint_before_write` returns `()` and logs at `debug!` only; workers running unattended with no blast-radius limit · Proves: **INV-5** · Evidence: _pending_

---

## 8. P5 — The graph

**Slice:** canonical path + non-destructive upsert + incremental correctness (M-19) · graph-scoped context (M-18).
**Exit gate (Part 6):** incremental update preserves cross-file edges and re-clusters; context selection provably narrows the prompt.

> P5 also **owns two gates listed earlier**: `TG3-15` (graph-scoped context, M-18) and the `graph/`-ladder clause of `TG1-03` (the broken v5 head must be repaired before v6). P5 cannot close until both are green.

- [ ] **TG5-01** `[conservation]` — Build the graph → edit **one** file → run the incremental update: **cross-file edges survive**, communities are re-assigned, and **no edge elsewhere is deleted**. Upsert is non-destructive; `INSERT OR REPLACE` with a cascade must not exist on any edge table.
      Source: spec `05 §5 G-INCREMENTAL` · Catches: `F-21`, `F-22`, `F-23` — a registry created and never populated; `INSERT OR REPLACE` + cascade silently deleting edges; incremental dropping cross-file edges and never re-clustering · Evidence: _pending_

- [ ] **TG5-02** `[determinism]` — `C:\p`, `C:/p`, `c:\p` and `C:\p\` all resolve to **one** database. Normalize before hashing — canonicalize, resolve symlinks, unify separators, case-fold where the platform is case-insensitive — using the **same shared helper** asserted by `TG3-29`.
      Source: spec `05 §5 G-PATHKEY` · Catches: verbatim path hashing; two competing DB path schemes; the stray `src-tauri/.autodev/graph.db` on disk (`G0 §6`) · Proves: **INV-5** · Evidence: _pending_

- [ ] **TG5-03** `[correctness]` — A build over a tree containing an unreadable directory, a permission-denied file, a broken symlink and a binary blob **completes** and **reports what it skipped**. Partial success is reported as partial, never as success.
      Source: spec `05 §5 G-BUILD-RESILIENT` · Catches: `F-42` · Evidence: _pending_

- [ ] **TG5-04** `[seam]` — The visualization payload carries `line_start`, `line_end` and `confidence_tier`, and **no community id is negative** in a rendered payload. The contract is asserted on the payload the command emits, independently of whether anything renders it.
      Source: spec `05 §5 G-VIZ-CONTRACT` (the rendering contract with spec `07`) · Catches: `M1`, `M2`, `M3` — the flat skyline and the community `-1` bucket · Pairs with `TG6-08` (the rendering side) · Evidence: _pending_

---

## 9. P6 — The UI

**Slice:** anchor implementation (M-20) · session-lifetime events (M-21) · contract reconciliation (M-22) · Settings persistence (M-23) · Graph City repair (M-24) · surface reachability (M-25).
**Exit gate (Part 6):** the anchor conformance test passes; no event is lost across a tab switch; every setting survives restart; zero unreachable shipped surfaces.

> P6 also **owns the kill-switch surface clause** of `TG2-17` and the **memory browser / export / forget** surfaces of `TG3-38` (G-07), both of which live inside the ~3,700 unreachable LOC that `TG6-07` routes.

- [ ] **TG6-01** `[seam]` — Border radii are drawn from the anchor's set — 7px, 2px, and pill on status dots only; fonts are the anchor's three (**self-hosted**, not fetched from a font CDN — **G-16**); every referenced CSS custom property is defined; the `.airs-*` class vocabulary is the one in use; **no dark palette exists**; and banned motion is absent. Checked over the built CSS, not the source.
      Source: spec `07 §8 G-ANCHOR` · Catches: the whole of `07 §1.1` — 7px appearing **zero** times against 12+ radii in use, IBM Plex shipping instead of Urbanist/Manrope/Roboto Mono, 1 of 34 anchor classes existing (in a dead file), two conflicting dark palettes, ~22 undefined CSS vars (`G5c F11`/`F12`) · Evidence: _pending_

- [ ] **TG6-02** `[correctness]` — Every computed style resolves; no invalid declaration ships. A malformed value that the browser silently drops is a failure, not a cosmetic issue.
      Source: spec `07 §8 G-CSS-VALID` · Catches: `"var(--accent-color)20"` and its class · Evidence: _pending_

- [ ] **TG6-03** `[conservation]` — Start a turn, switch tabs, return: **no event is lost and the transcript is complete**. The subscription is owned above the tab boundary for the session's whole life, with backfill on remount — a listener that lives inside `handleSend` or a mounted tab fails this gate by construction.
      Source: spec `07 §8 G-EVENT-LIFETIME` · Catches: BUG-CHAT; Live Monitor staleness (`G5a §1`, `G5c F07`) · Evidence: _pending_

- [ ] **TG6-04** `[seam]` — **Build-time, both directions:** every `invoke` name in the frontend resolves to a registered Tauri command; every registered command has a caller; every `listen` name has an emitter; every emitted event name has a consumer. An unmatched name on either side **fails the build**.
      Source: spec `07 §8 G-CONTRACT` (merged with spec `01 §8 G-EVENTS`, whose engine-side `agent:*` symmetry check this subsumes) · Catches: 2 frontend calls to commands that do not exist; **47 dead registered commands (18%)**; 10 dead listeners; 3 dead events including `plan:changed` with **16 emit sites and no listener**; `agent:thinking` and the five phantom `agent:*` (`G5c`, `G2a`) · Proves: **INV-8** · Evidence: _pending_
      → the engine-side half is produced by **P3**'s runtime; P3 must not introduce a new unmatched `agent:*` name, and this gate is what detects it.

- [ ] **TG6-05** `[correctness]` — **Every** setting survives a restart **and is applied**. Asserted by changing each control, restarting, and observing the change take effect in behaviour — not merely that the control re-renders with the stored value.
      Source: spec `07 §8 G-SETTINGS-PERSIST` · Catches: BUG-SETTINGS — no persistence, the `AppState` rebuild discarding values (`G5c F01`/`F02`/`F03`) · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG6-06** `[seam]` — **Every control's value has a reader.** A control that writes to a store nothing consumes is deleted or wired — it is not shipped. Static check plus a per-control assertion.
      Source: spec `07 §8 G-NO-WRITEONLY-UI` · Catches: budget config with no reader; the 5 dead hook templates; unwired controls; a failed save showing green "Saved!" (`G5c F06`) · Proves: **INV-8** · Evidence: _pending_

- [ ] **TG6-07** `[ux]` — **Every shipped component is reachable from a route.** An unreachable surface is routed or deleted; there is no third option. The gate enumerates components and asserts each is reachable, so a newly orphaned surface is caught at the phase it is orphaned in.
      Source: spec `07 §8 G-REACHABLE-UI` · Catches: `G5c F08` — ~3,700 LOC of memory, plugin and plan UI and their three stores reachable from no route · Evidence: _pending_

- [ ] **TG6-08** `[correctness]` — With a built graph, the city renders districts, walls and towers **offline** — no CDN request, self-hosted fonts, no hanging `<Suspense>` — and loads on `workspaceDir` rather than requiring a manual tab visit. Geometry is memoized: no per-`mousemove` churn.
      Source: spec `07 §8 G-CITY` · Catches: BUG-CITY — CSP blocking drei/troika CDN fonts so every `<Suspense>` hangs; the tab never auto-loading; the flat skyline; per-mousemove geometry churn (`G5b`) · Pairs with `TG5-04` (the payload side) · Evidence: _pending_

- [ ] **TG6-09** `[ux]` — Focus management, keyboard navigation, no colour-only status encoding, contrast thresholds met under the light palette, and `prefers-reduced-motion` honoured. Asserted per the anchor's accessibility block, over the shipped surfaces.
      Source: spec `07 §8 G-A11Y` · Catches: the anchor's a11y block being unimplemented alongside the rest of `07 §1.1` · Evidence: _pending_

---

## 10. P7 — Experience

**Slice:** first-run / BYOK flow (M-35) · error contract (M-36) · progress + cancellation (M-37) · destructive-action safety (M-38) · coherence (M-39).
**Exit gate (Part 6):** **a new user reaches a working agent turn without documentation.**

> P7 also **owns the progress-and-skip UI clause** of `TG3-19` (the ~130 MB embedding-model download is the single heavyweight step in an otherwise light onboarding).

- [ ] **TG7-01** `[ux]` — An **induced failure renders a visible, actionable toast** carrying what failed, why, and what to do next. Nothing user-facing is `console.error`-only; no `alert()` remains; every retryable failure offers retry. Asserted by injecting a failure at each of the named producer sites and observing the rendered DOM.
      Source: spec `07 §8 G-TOAST` (and `§7.3`) · Catches: **no toast has ever been shown** — the two producers destructure `addToast` (a store *method*, not a module export) via `require`, unavailable in Vite's browser ESM; 88 `console.*`-only sites, 8 `alert()`, 5 no-op `.catch(()=>{})`, 48 unhandled store rejections, 3 retry affordances in 35,000 LOC; `qc:decision` notifications that have never fired · Evidence: _pending_

- [ ] **TG7-02** `[ux]` — **From a clean profile, with no documentation:** launch → pick a workspace → choose a provider → enter a key → the key is **validated against the provider before storage** → send one suggested prompt → receive a successful turn. Every step is discoverable from the previous one. **This is the acceptance test of `00 §7.2` and spec `10 §2`.**
      Source: spec `07 §8 G-FIRSTRUN` · Catches: the 10-step first run with 6 undiscoverable steps (`G11`, `07 §7.1`) · Covers: **R-5** · Evidence: _pending_
      → this gate is the **UX half** of the headline acceptance test; `TG9-01` is the **three-OS half**. Both must be green for `00 §7.2` to be satisfied.

- [ ] **TG7-03** `[ux]` — With **no key**, and separately with an **invalid key**, the user sees a specific, actionable error naming the provider's own reason — **never an empty bubble, and never a destroyed message**. An unconfigured app never presents a chat box that silently fails.
      Source: spec `07 §8 G-BYOK-VISIBLE` · Catches: `"No active provider set"` surfaced as a blank bubble with the message destroyed (`G5a §7`); the silent primary failure · Covers: **R-5** · Evidence: _pending_

- [ ] **TG7-04** `[ux]` — If startup never completes, the user reaches a **usable state with a diagnosis** within a bounded time. A watchdog exists; a stalled startup phase is named to the user rather than presenting an indefinite splash.
      Source: spec `07 §8 G-BOOT-ESCAPE` · Catches: the missing watchdog; the scripted startup FSM (which **OQ-3** deletes with Demo Mode, removing the fallback that masked this) · Proves: **INV-9** · Evidence: _pending_

- [ ] **TG7-05** `[ux]` — **Every destructive control confirms, in one idiom**, with confirmation proportional to blast radius — and the reversible/irreversible inversion is gone. Enumerated over the ten named controls; a new destructive control with no confirmation fails.
      Source: spec `07 §8 G-DESTRUCTIVE` · Catches: the 10 unconfirmed controls, 6 of them irreversible — one-click `git_discard` destroying uncommitted work (`G3b GI-07`); *Delete checkpoint* unconfirmed beside a confirmed *Restore*; three coexisting confirmation idioms; a 3-second auto-revert guarding the least destructive action of the set · Pairs with `TG4-23` (the reversibility mechanism) · Evidence: _pending_

---

## 11. P8 — Submodule deletion

**Slice:** remove `sidecar/` (3,161 LOC), the 29 sidecar-dependent commands, `resources/sidecars` (225 MB), both `.gitmodules` entries, the remaining MCP HTTP server code, and the now-dead dependencies.
**Entry gate (Part 6, strengthened by review fix 9):** every ported capability passes its tests with the sidecar **absent — not merely stopped**.
**Exit gate:** the tree contains no Python; the installer contains no `.py`; the app is fully functional on all three platforms.

> **P8 has only two gates of its own, and that is correct.** A deletion phase proves itself by **re-running the entire standing gate set (`TG0-01…TG7-05`) under the sidecar-absent condition**. Any gate that regresses here identifies a coupling that was not enumerated — which is exactly `R-6`. P8's exit dependency on the no-`.py` clause of `TG9-02` is noted there.

- [ ] **TG8-01** `[seam]` — The **full suite** passes with the sidecar **absent** — the directory renamed or deleted, not the process stopped. "Not running" is too weak: persona YAML still reads `{orch}/config/agents/*.yaml`, so a stopped-but-present sidecar can still satisfy a file read. The corrected ported-capability list is the population; the P4-era "10 provider families" and the P7-era startup FSM (deleted by **OQ-3**) are excluded from it.
      Source: spec `01 §8 G-NOSIDECAR` + spec `09 §6 G-NOSIDECAR` (merged — both specs asked for the same P8 entry gate) · Catches: any residual coupling; `R1 B-08`'s finding that the original entry gate was unmeetable as written · Proves: **INV-1** (as restated by **G-02**) · Covers: **R-6** · Evidence: _pending_

- [ ] **TG8-02** `[governance]` — **NEW (plan-mandated by correction #1; covers R-6).** The deletion phase **cannot complete while any capability grounding marked `PORT` remains unported.** The gate reads the grounding reports' PORT verdicts as its checklist and asserts a named, tested destination in the Rust tree for each — the ported-capability count is **ten, not nine**, and the tenth is `M-40` redaction (`TG3-38`).
      → **The tenth PORT entry is now supplied.** `M-40` redaction was ported in `P1-20`/`P1-21` — both Python redactors, wired at the tool-result funnel and all four memory writes, proven end to end by `S1-04`. This gate's checklist can therefore be satisfied for that row at P8; the other nine remain its job.
      Source: **NEW** — `00 §Part 1B` M-40's closing note: *"adds a P8 gate: the deletion phase cannot complete while a capability grounding marked PORT remains unported. That is the check which would have caught this without a review."* No spec turned that mandate into a named gate · Catches: the failure mode that actually occurred — `G4b §A.14` ruled redaction **PORT** and the plan dropped it, undetected until an adversarial review · Covers: **R-6** · Evidence: _pending_

---

## 12. P9 — Delivery

**Slice:** per-platform bundle targets + signing (M-27) · the platform `Command` abstraction hardened (M-26) · the end-to-end smoke in CI on three OSes (M-28).
**Exit gate (Part 6):** a signed, verified installer for Windows, Linux and macOS, each of which installs and runs a task to completion.

- [ ] **TG9-01** `[portability]` — The **end-to-end smoke passes on Windows, Linux and macOS**: install → enter a key → point at a repo → one sentence → a build-verified change in that repo → run it. This is the headline acceptance test of `00 §7.2` executed on the triple; `TG7-02` is its UX half. Every `[portability]`-tagged gate must carry its evidence class on all three OSes before this can be green.
      Source: spec `09 §6 G-PLATFORM-TRIPLE` · Catches: BUG-COMPILE and the whole `P-01…P-35` portability class · Covers: **R-10** · Evidence: _pending_

- [ ] **TG9-02** `[security]` — Each platform produces **its own** installer (not `targets: ["nsis"]` for all three), and **the installer contains no `.py` and no `.env`**. The no-`.py` clause is **P8's exit gate** and must be green there; the per-platform-targets clause is P9's.
      Source: spec `09 §6 G-BUNDLE` · Catches: `targets: ["nsis"]`; a 225 MB Python tree bundled with its `.env` and `.git` (`G0 §3.1`) · Proves: **INV-3** · Evidence: _pending_
      → the **credential half runs from P0** as `TG0-04`, which exists because this gate cannot be green before P9 and INV-3 cannot wait nine phases.

- [ ] **TG9-03** `[security]` — Every released artifact is **signed**, and notarised where the platform requires it. A null or placeholder signing identity is a failure. **Procurement-gated (ADR-03-04)** — if certificates are not yet available, this gate is **red and blocking**, never skipped; a skipped gate is not green (`§1`).
      Source: spec `09 §6 G-SIGNED` · Catches: the null signing identities; unsigned artifacts on macOS and Windows · Proves: **INV-11** · Covers: **R-4** · Evidence: _pending_

- [ ] **TG9-04** `[portability]` — On **each** platform, an update actually **replaces the binary and the app restarts on the new version**. Asserted by reading the running version after the update, not by the installer's exit code.
      Source: spec `09 §6 G-UPDATE-INSTALLS` · Catches: the Linux install that is a no-op returning success; the Windows RunOnce mismatch; the macOS orphan plist (`IN-13`) · Proves: **INV-11** · Evidence: _pending_

- [ ] **TG9-05** `[portability]` — On a machine with **no Rust, no Node, no Python and no `uv`**, the installer produces a working app that can complete a turn. Run on a clean image per platform, not on a developer machine with tooling on `PATH`.
      Source: spec `09 §6 G-CLEAN-INSTALL` · Catches: the `uv`-from-`PATH` assumption; the installer needing Python present · Proves: **INV-1** · Evidence: _pending_

---

## 13. (a) Fixture corpus specification

Gates are only as honest as what they run against. Every corpus below is **checked into the repository, versioned, and owned by the phase that first consumes it** — a fixture that drifts silently turns a green gate into a lie. Recorded provider traffic is scrubbed by the `TG3-38` redactor before it is committed.

### 13.1 Positive corpora

| Corpus | Contains | Consumed by |
|---|---|---|
| **`fx-repo-rust`** | A minimal `cargo` workspace that builds, tests and lints clean; plus a `.cmd`-shimmed toolchain variant for Windows resolution | `TG4-07`, `TG4-08`, `TG4-18`, `TG9-01` |
| **`fx-repo-node`** | A minimal `npm` project with `build`/`test`/`lint` scripts, including one script only resolvable through a `.cmd` shim | `TG4-07`, `TG4-18`, `TG9-01` |
| **`fx-repo-python`** | A minimal `pyproject` project with a test suite — the ecosystem detector's third arm | `TG4-07`, `TG4-18`, `TG9-01` |
| **`fx-repo-broken`** | Per-ecosystem variants that fail in **distinguishable** ways: compile error, test failure, missing toolchain, and a build that **succeeds while printing the word "error"** | `TG4-07` (a passing build must never be recorded as failed), `TG4-08` (three-failure escalation), `TG3-34` |
| **`fx-repo-hostile`** | A repo carrying a shadowing `node`/`cargo`/`python` executable on a searched path, an unreadable directory, a broken symlink, a 200 MB binary blob, and a file with no trailing newline | `TG3-33`, `TG5-03` |
| **`fx-graph-repo`** | A multi-language repo with **known** cross-file edges and known communities, plus a scripted single-file edit and its expected post-edit graph; and a labelled relevant-file set per task prompt | `TG5-01`, `TG5-04`, `TG3-15`, `TG6-08` |
| **`fx-provider-replay`** | Recorded request/response pairs per **provider × model × capability**: SSE frame sequences, tool calls, parallel tool calls, structured output, reasoning/opaque bytes, cache-write and cache-read reports in each provider's own field shape, `max_tokens` truncation, and one instance of every error class | `TG2-01…TG2-10`, `TG2-16` |
| **`fx-memory-labelled`** | A memory corpus with query → relevant-memory judgments, spanning all three tiers, every subtype, every provenance source, and every scope (`global` / `project` / `task`), sized so a selective scope filter is genuinely selective | `TG3-11…TG3-14`, `TG3-20`, `TG3-22`, `TG3-23` |
| **`fx-paths-benign`** | Legitimate paths that must be **accepted**: unicode filenames, spaces, deep nesting, long-but-legal Windows paths, case variants of the same real file, relative paths inside the workspace | `TG3-27`, `TG3-28`, `TG3-29`, `TG5-02` |
| **`fx-update-valid`** | A well-formed, correctly-signed update per platform that **must install** and must leave the app running the new version | `TG0-01`, `TG9-04` |
| **`fx-migrate-heads`** | A snapshot of each of the eleven databases at its live head, **including the broken `graph/` v5**, plus one snapshot per intermediate ladder version | `TG1-02`, `TG1-03`, `TG1-04` |
| **`fx-time`** | Instants across a full year at UTC−12, UTC+0 and UTC+14; both DST transitions in both hemispheres; leap day; the second either side of local midnight | `TG2-15`, `TG4-19` |
| **`fx-anchor-manifest`** | The `ui-anchor.md` token set as machine-readable truth: permitted radii, the three fonts, the `.airs-*` class vocabulary, the light palette, banned motion, contrast thresholds | `TG6-01`, `TG6-02`, `TG6-09` |

### 13.2 Hostile corpora

| Corpus | Contains | Consumed by |
|---|---|---|
| **`fx-paths-hostile`** | `..` traversal in every encoding, symlinks and Windows junctions escaping the workspace, UNC paths, reserved device names (`CON`, `NUL`, `AUX`), trailing separators, mixed separators, case-collision pairs, unicode-normalisation collisions, over-long paths | `TG3-28` (refused, never rewritten), `TG3-27`, `TG5-02` |
| **`fx-update-hostile`** | An update with **no** signature, a bad signature, a signature from a non-production key, a truncated payload, a valid signature over a swapped payload, a replayed older manifest, and a manifest with an empty `sig_url` | `TG0-01`, `TG9-03` |
| **`fx-secrets`** | Synthetic credentials matching each of the **22** `SENSITIVE_KEYS` entries (the source has 22; "20" was never right — counted in `P1-20`), planted in tool arguments, file contents, environment dumps, prompts, log lines and memory proposals | `TG3-38`, `TG0-04` |

### 13.3 The no-fire / noise corpus — **first-class, not an afterthought**

**A gate that only proves the system acts is half a gate.** Every guard in this programme has a symmetric failure mode: a jail that refuses everything, a redactor that eats legitimate content, a permission gate that prompts on every read, a memory that confabulates a hit for an unrelated query, a breaker that trips at rest. Each of those passes a fire-only test and destroys the product. **Honesty — doing nothing, visibly and correctly, when nothing is called for — is gated here.**

| Corpus | The system must do **nothing** | Consumed by |
|---|---|---|
| **`fx-nofire-memory`** | Queries with **no** relevant memory in the store: recall returns **empty and says so**. It does not return the nearest neighbour at any distance, does not confabulate, and does not report degraded when it is not | `TG3-12`, `TG3-17`, `TG3-22` |
| **`fx-nofire-gateway`** | `propose_memory` proposals that are trivial, duplicate, low-confidence or already-known: **nothing is stored**, and the agent is told which check rejected it | `TG3-21` |
| **`fx-nofire-secrets`** | Strings that **resemble** secrets and are not — a base64 blob, a UUID, a git SHA, a JWT-shaped test fixture, a public key, a licence identifier: all survive **unredacted** | `TG3-38` |
| **`fx-nofire-paths`** | = `fx-paths-benign`. A jail that refuses a legitimate path fails as hard as one that admits a hostile one | `TG3-28` |
| **`fx-nofire-perm`** | Read-only tool calls in an auto-approve mode: **no prompt is raised**. A gate that prompts unconditionally is the defect `TG3-25` exists to catch, and it looks identical to a working gate under a fire-only test | `TG3-24`, `TG3-25` |
| **`fx-nofire-egress`** | Tool calls that touch nothing outside the machine: **no consent check is raised and no consent record is created** | `TG3-26` |
| **`fx-nofire-fleet`** | An idle board, a completed epic, an already-claimed task, a worker at rest: **no dispatch, no escalation, no re-claim, no orphan release**. An orphan reaper that releases live claims is worse than one that never fires | `TG4-01`, `TG4-03`, `TG4-12`, `TG4-19` |
| **`fx-nofire-budget`** | Spend well under both the per-task budget and the daily ceiling: **no breaker trips, no pause, no toast**. Pairs directly with `TG3-37` — a provisional charge leak makes a breaker trip at zero real spend | `TG2-17`, `TG3-37`, `TG4-15` |
| **`fx-nofire-ui`** | A successful save, a successful turn, a non-destructive control: **no error toast, no confirmation dialog**. A confirmation on everything trains the user to click through the one that mattered | `TG7-01`, `TG7-05` |
| **`fx-nofire-cache`** | A prefix **above** the model minimum and byte-stable: **the cache-hit alarm does not fire**. An alarm that always fires is an alarm nobody reads | `TG2-07` |

---

## 14. (b) Traceability — spec → gate section → TG IDs → phase

| Spec | Its gate section | Gates it contributed | TG IDs | Phase(s) |
|---|---|---|---|---|
| `01-AGENT-RUNTIME` | `§8 Test gates` | 10 → 7 own + 3 merged away | `TG3-01…TG3-07` | **P3** |
| | | `G-TRUNC` merged into `G-MAXTOK` | `TG2-02` | P2 |
| | | `G-EVENTS` merged into `G-CONTRACT` | `TG6-04` | P6 |
| | | `G-NOSIDECAR` merged with spec `09`'s | `TG8-01` | P8 |
| `02-PROVIDERS…` | `§11 Test gates` | 15 | `TG0-03`; `TG2-01…TG2-14` | **P0** (1), **P2** (14) |
| `03-CONTEXT-AND-MEMORY` | `§6 Test gates` | 19 → 16 own + 3 merged away | `TG3-08…TG3-23` | **P3** (14) · `TG3-15` → **P5** · `TG3-23` → **P4** |
| | | `G-CTX-CACHE` → `G-CACHE-HIT`; `G-CTX-STABLE` → `G-CACHE-STABLE` | `TG2-05`, `TG2-06` | P2 |
| | | `G-MEM-SCOPE` merged into `G-MEM-SCOPED` | `TG3-13` | P3 |
| `04-THE-FLEET` | `§8 Test gates` | 16 | `TG4-01…TG4-16` | **P4** |
| `05-THE-CODE-GRAPH` | `§5 Test gates` | 7 → 4 own + 3 merged/moved | `TG5-01…TG5-04` | **P5** |
| | | `G-REACHABLE` | `TG1-01` | **P1** |
| | | `G-MIGRATE` merged into `G-MIGRATE-FRESH`/`-HEAD` | `TG1-02`, `TG1-03` | P1 (graph clause → P5) |
| | | `G-GRAPH-SCOPE` merged with spec `03`'s | `TG3-15` | P3 → owned P5 |
| `06-TOOLS-PERMISSIONS…` | `§7 Test gates` | 14 | `TG3-24…TG3-37` | **P3** · `TG3-34` → **P4** |
| `07-THE-UI` | `§8 Test gates` | 14 | `TG6-01…TG6-09`; `TG7-01…TG7-05` | **P6** (9), **P7** (5) |
| `08-DATA-MODEL…` | `§4 Test gates` | 10 | `TG1-02…TG1-06`; `TG2-15`; `TG4-19…TG4-22` | **P1** (5), **P2** (1), **P4** (4) |
| `09-DELIVERY` | `§6 Test gates` | 10 | `TG0-01`, `TG0-02`; `TG4-17`, `TG4-18`; `TG8-01`; `TG9-01…TG9-05` | **P0** (2), **P4** (2), **P8** (1), **P9** (5) |
| `10-PROOF` | *(no gate table — by design)* | 0 gates; contributes the evidence classes (`§1`), the headline acceptance test (`TG7-02` + `TG9-01`), the CI blocking table (`TG0-02`) and the ten test families (`§17`) | — | — |
| **NEW** | *(this document)* | 8 | `TG0-04`, `TG0-05`, `TG2-16`, `TG2-17`, `TG3-38`, `TG3-39`, `TG4-23`, `TG8-02` | P0 (2), P2 (2), P3 (2), P4 (1), P8 (1) |

**Spec gate totals:** 115 named in the specs − 8 merged = **107 surviving** + **8 NEW** = **115**.

---

## 15. (c) Risk-gate coverage — master plan `§9.2`

| Risk | Covering gates | Verdict |
|---|---|---|
| **R-1** — the bug register swallows the restructure (~415 defects) | **none** | ⚠️ **UNCOVERED, and correctly so.** This is a programme-management risk, not a system property; its mitigation is *"slices are gated on outcomes, not defect counts — a defect not on a slice's path is logged, not fixed."* No test can assert that. **This document is its control surface**: 115 gates and no more is the outcome-gating in practice. Recorded here so the absence is a decision, not an oversight |
| **R-2** — caching is adopted and silently does nothing | `TG2-05` (non-zero cache read from turn 2, provider-neutral) · `TG2-06` (byte-stable prefix) · `TG2-07` (the alarm, which ADR-01-05 ships **before** the cache) · `TG3-08` (bounded context) | ✅ covered, and in the mandated order |
| **R-3** — the port reproduces the sidecar's own defects | `TG3-12` (defects 1, 2, 6, 7) · `TG3-13` (defects 4-partial, 5) · `TG3-14` (defect 3) · `TG3-17` (defect 4, the unused breaker) | ✅ covered — **all seven** of spec `03 §4.3`'s fix-don't-reproduce defects, but **only after the sharpening recorded in `§18`**: as the specs stood, defects 4-partial, 6 and 7 had no gate |
| **R-4** — macOS verified last and blocks release | `TG0-05` **(NEW)** · `TG9-03` | ✅ covered **only by a NEW gate**. The mitigation ("macOS enters CI in P0") was a mandate no spec turned into an assertion |
| **R-5** — BYOK turns first-run into a wall | `TG7-02` · `TG7-03` | ✅ covered |
| **R-6** — deleting the sidecar strands a capability nobody enumerated | `TG8-01` (sidecar **absent**, not stopped) · `TG8-02` **(NEW)** (every `PORT` verdict has a tested destination) · `TG3-39` **(NEW)** (the fault-boundary the sidecar silently provided) | ✅ covered — two of the three gates are NEW, which is itself the evidence that R-6 is the live risk it claims to be |
| **R-7** — `Item::Opaque` deferred and every adapter rewritten twice | `TG2-08` | ✅ covered |
| **R-8** — affinity trained on stolen-task attributions | `TG4-03` (atomic claim) · `TG4-13` (routing measurably changes) | ⚠️ **partially covered.** Both halves of the *behaviour* are gated, but the mitigation's two operational clauses are not: **(i)** the sequencing constraint *M-12 before M-33* is enforced only by prose in `§7`'s preamble — no check prevents affinity being enabled first; **(ii)** *"consider discarding existing `routing_history`"* is an undecided action with no gate. Both belong in the MDP as ordered tasks, not gates |
| **R-9** — rate limits and cost during development | `TG2-16` **(NEW)** | ✅ covered **only by a NEW gate**. The mitigation names a conformance harness; no spec asserted it runs offline, which meant `TG2-01` could not have run in CI at all |
| **R-10** — Windows stays the only truly tested platform | `TG0-05` **(NEW)** · `TG4-17` · `TG4-18` · `TG9-01` | ✅ covered. Note the shape: without `TG0-05` and the two gates **pulled forward** into P4, R-10 would have been unmitigated until P9 — which is the exact failure the risk describes |

---

## 16. Invariant coverage — master plan `§1.4`

**All twelve invariants have at least one gate. There is no uncovered invariant.** Three, however, were covered only after intervention, and that is recorded rather than smoothed over.

| Invariant | Covering gates | Note |
|---|---|---|
| **INV-1** — no agent work in a foreign runtime *(as restated by G-02)* | `TG8-01`, `TG9-05` | Under the restatement, `TG1-06` (two processes, one database) is a **requirement**, not a violation |
| **INV-2** — no provider API from the webview; no key outbound | `TG0-03`, `TG3-38` | ⚠️ `G-KEY-ASYM` as written gated only the **key-return** half. The **CSP `connect-src`** half of `IN-47` was ungated; folded into `TG0-03` as a sharpening |
| **INV-3** — no `.env`, `.git` or credential in a shipped artifact | `TG0-04` **(NEW)**, `TG9-02` | ⚠️ Its only spec gate (`G-BUNDLE`) cannot be green before **P9** — INV-3 would have been unguarded for nine phases. `TG0-04` closes that |
| **INV-4** — a gate that cannot fail is not a gate | `TG0-02` | Also enforced against **this document** by `§1`'s skipped-gate rule |
| **INV-5** — no write resolved against the process CWD | `TG3-19`, `TG3-27`, `TG3-28`, `TG4-09`, `TG4-23`, `TG5-02` | `TG3-19` carries **G-14**'s finding that `fastembed`'s cache defaults to CWD — an INV-5 violation on the newest subsystem, which no spec gated |
| **INV-6** — platform-neutral commands | `TG4-17`, `TG4-18` | Both **pulled forward from P9 to P4**, because P4's exit gate requires three-OS end-to-end |
| **INV-7** — measurements are provider-reported, split four ways | `TG2-09`, `TG2-10`, `TG2-15` | |
| **INV-8** — a writer must have a proven reader | `TG2-01`, `TG2-12`, `TG2-13`, `TG3-02`, `TG3-09`, `TG3-11`, `TG3-12`, `TG3-14`, `TG3-15`, `TG4-07`, `TG6-04`, `TG6-05`, `TG6-06` | The most-gated invariant, correctly — it is the one the register keeps re-proving |
| **INV-9** — every blocking wait has a timeout and a cancellation path | `TG3-03`, `TG3-06`, `TG3-30`, `TG3-31`, `TG4-02`, `TG4-12`, `TG7-04` | |
| **INV-10** — one definition per schema | `TG1-05` | ⚠️ Single-gated, and its pass condition depends on an **unscheduled** slice (see `§19` F-2) |
| **INV-11** — the updater installs nothing unverified | `TG0-01`, `TG9-03`, `TG9-04` | |
| **INV-12** — credential provenance is BYOK, single-sourced | `TG2-11`, supported by `TG3-38` | ⚠️ **G-06** amended INV-12 to name the Linux encrypted-file fallback; no spec gated the amendment. Folded into `TG2-11` as a sharpening |

---

## 17. Test-family coverage — master plan `§7.4` / spec `10 §4`

**All ten families are represented. No family has zero gates.**

| Family | Gates | Count |
|---|---|---|
| **Reachability** | `TG1-01`, `TG6-04`, `TG6-06`, `TG6-07` | 4 |
| **Provider conformance** | `TG2-01`, `TG2-02`, `TG2-03`, `TG2-04`, `TG2-08`, `TG2-09`, `TG2-10`, `TG2-16` | 8 |
| **Cache efficacy** | `TG2-05`, `TG2-06`, `TG2-07` | 3 |
| **Reader-exists (INV-8)** | `TG2-01`, `TG2-12`, `TG2-13`, `TG3-02`, `TG3-09`, `TG3-11`, `TG3-12`, `TG3-14`, `TG3-15`, `TG4-07`, `TG6-04`, `TG6-05`, `TG6-06` | 13 |
| **Guard-the-guard** | `TG3-04`, `TG3-05`, `TG3-24`, `TG3-27`, `TG3-34`, `TG4-07`, `TG3-17` | 7 |
| **Platform triple** | `TG0-05`, `TG4-17`, `TG4-18`, `TG9-01`, `TG9-04`, `TG9-05` | 6 |
| **Destructive safety** | `TG3-36`, `TG4-23`, `TG7-05` | 3 |
| **Anchor conformance** | `TG6-01`, `TG6-02`, `TG6-09` | 3 |
| **Migration** | `TG1-02`, `TG1-03`, `TG1-04` | 3 |
| **No-sidecar** | `TG8-01`, `TG8-02` | 2 |

**Thinnest families, flagged rather than padded:** *Destructive safety* (3) and *No-sidecar* (2). Both are thin for structural reasons — destructive safety is largely an enumeration inside `TG7-05` and `TG4-23`, and the no-sidecar family's real weight is that **P8 re-runs all 115 gates** under the sidecar-absent condition rather than adding gates of its own. Neither is a coverage gap; both are recorded so a later reader does not mistake thinness for oversight.

---

## 18. Merge register

Eight gates from the specs restate or subsume another. Each merge preserves **both** assertions; none loses one.

| # | Merged away | Survives as | Why | Anything gained |
|---|---|---|---|---|
| 1 | `01 §8 G-TRUNC` | `TG2-02` (`02 §11 G-MAXTOK`) | Both assert `max_tokens` truncation surfaces as `StopReason::MaxTokens` on all four providers | The survivor asserts **two** halves separately — the parameter reaching the wire (OpenAI/Google drop it) **and** the stop reason coming back |
| 2 | `03 §6 G-CTX-CACHE` | `TG2-05` (`02 §11 G-CACHE-HIT`) | Identical assertion — non-zero `cached_read` from turn 2 on every provider that supports it | Restated **provider-neutrally** per review fix 5; the Anthropic-only arithmetic is a corrected defect |
| 3 | `03 §6 G-CTX-STABLE` | `TG2-06` (`02 §11 G-CACHE-STABLE`) | Both assert the prefix builder is byte-identical across identical inputs | The survivor carries a **cross-phase clause**: the system-prompt half is P3's, so full-prefix stability is owned by P3 |
| 4 | `03 §6 G-MEM-SCOPE` | `TG3-13` (`03 §6 G-MEM-SCOPED`) | Two gates in **one spec** restating the same scope-isolation assertion | Merged with **G-12**'s correction: scope must be a declared `vec0` partition key, so a selective scope is asserted to return results rather than post-filtering to zero |
| 5 | `05 §5 G-GRAPH-SCOPE` | `TG3-15` (`03 §6 G-GRAPH-SCOPE`) | The same gate name in two specs | Sharpened from *"fewer, better files"* to **precision and recall against a labelled set** — fewer-but-wrong now fails |
| 6 | `09 §6 G-NOSIDECAR` | `TG8-01` (`01 §8 G-NOSIDECAR`) | Both specs name the identical P8 entry gate | Strengthened per review fix 9 to **sidecar absent, not merely stopped**, over the **corrected** capability list |
| 7 | `05 §5 G-MIGRATE` | `TG1-02` + `TG1-03` (`08 §4 G-MIGRATE-FRESH` / `-HEAD`) | Spec `05`'s gate is spec `08`'s pair applied to one ladder | Generalised to **every** ladder, with the `graph/` ladder's broken v5 head quarantined and the quarantine list itself asserted |
| 8 | `01 §8 G-EVENTS` | `TG6-04` (`07 §8 G-CONTRACT`) | Event emitter/consumer symmetry is the event half of the invoke/listen contract check | One build-time check covers commands **and** events in both directions; the P3 runtime is bound by it even though P6 owns it |

---

## 19. Findings

Findings are numbered and are **not** formatting notes. Each is something a reader should act on.

**F-1 — `R-1` has no covering gate, by design.** Recorded in `§15`. A programme-management risk is not gateable; saying so is more honest than manufacturing a gate that cannot fail.

**F-2 — ~~`TG1-05` (INV-10, one schema owner) has a home but no owner.~~ RESOLVED 2026-08-31 by the MDP.** Raised because Part 6 contains no slice for the **core-crate extraction** (`autodev-core` / `src-cli` / `src-shared`) that `TG1-05`'s pass condition requires, though review fix 8 (`R1 B-05/B-06`) says it was added to the build sequence. The MDP, written in parallel, schedules it at **P1-B** — the same phase this document assigned the gate to, independently. Struck in place per `§1`; kept visible because the underlying gap in Part 6 is real and Part 6 should still be corrected.

**F-3 — ⚠ `G-15` and the ACP collision are in direct conflict, and the conflict is currently unresolved.**
- Spec `04 §1` establishes that worker→worker messaging functions **only** because workers reach their tools through the MCP HTTP server, which is the sole site that sets `ToolContext.agent_role` truthfully. Its stated resolution: *"`ToolContext` must carry a truthful `agent_role` from the in-house construction sites **before** the MCP server is deleted… not optional and not deferrable."*
- **G-15** then moved that deletion to **P0** — four phases before M-09 lands in **P3**.
- Consequence: between P0 and P3, every ACP call begins refusing, **silently** — the refusal is a tool-level response the worker narrates around, so nothing surfaces. `TG4-10` cannot be green during that window and there is no gate that would notice.
- Both decisions are individually correct (an unauthenticated local RCE surface should not wait eight slices; ACP identity should not be deferred). **They are jointly unsatisfiable as sequenced.** Options: (a) move M-09's `ToolContext` fix into P0 alongside the deletion — it is a three-site change; (b) delete only the HTTP listener in P0 and retain `tool_adapter.rs`'s identity assignment until P3; (c) accept a known-dead capability for three phases and gate its return.
- **Cross-checked against the MDP (2026-08-31): the collision is acknowledged but not decided.** `P0-13` deletes `tool_adapter.rs` outright; `P3-13` populates `ToolContext` truthfully; `P4-22` then restores ACP identity and states plainly *"set only by the MCP adapter today, which P0 deleted. Without this, worker messaging is silently dead."* That is **option (c), chosen implicitly by build order** — a preserved capability (ADR-02-06) is dead from P0 to P3 with no gate that would notice, and spec `04 §1` calls the fix *"not optional and not deferrable"*.
- **This remains a Rule-4 stop.** Deferring a decided-preserved capability across three phases is the owner's call to make explicitly, not a consequence of task ordering. The cheapest resolution is (a): pull `P3-13` forward next to `P0-13` — it is a three-site change and it is what spec `04 §1` already directs.

**F-4 — `R-8`'s operational clauses are ungated.** The M-12-before-M-33 sequencing is prose in `§7`, and *"consider discarding existing `routing_history`"* is an undecided action. Both belong in the MDP as ordered tasks. See `§15`.

**F-5 — spec `03 §4.6` is stale against `G-13`.** It still asserts the anchor's `§10` reranker *"is"* the ported competitive model. G-13 established that 6 of 8 and 7 of 9 terms are disjoint. `TG3-22`'s thresholds must be set against the **anchor's** model. **The spec line should be corrected** (no-stale rule).

**F-6 — three named defects were gated only by sharpening, not by any spec gate.** Recorded on the gate lines and in `§15`/`§16`: spec `03 §4.3` defects 4-partial, 6 and 7 (folded into `TG3-12` and `TG3-17`); `ErrorClass::Retryable` with no retry policy and `record_api_failure` with no writer (folded into `TG2-01`); the CSP half of `IN-47` (folded into `TG0-03`); `G-06`'s Linux credential store (folded into `TG2-11`); `G-14`'s `fastembed` CWD violation (folded into `TG3-19`). **A spec gate whose assertion is narrower than the defect it cites is a gate that cannot catch its own stated catch** — worth a sweep of the vision suite for the same pattern.

**F-7 — the eight NEW gates cluster around the review's own findings, not the specs'.** Six of the eight (`TG0-04`, `TG0-05`, `TG2-17`, `TG3-38`, `TG3-39`, `TG4-23`) close mechanics **M-40…M-43** and decisions **G-01, G-04, G-06, G-07, G-08** — all added by the adversarial review at rev 0.9, **after** the ten specs were written. The specs were never regenerated against them. That is the same class of miss as review fix 2 (Part 1B stale since rev 0.3). **Recommend: a regeneration pass over specs `01`–`10`'s gate tables against Part 1B's current mechanic list before the MDP is built.**

**F-8 — spec `10 §5`'s frontend gap is gated only indirectly.** "There is no frontend test framework at all" is a spec requirement with no `G-*` name. It is folded into `TG0-02`, which asserts the job list contains every row of spec `10 §6`'s target column — including ESLint and frontend unit tests, neither of which exists today. Adequate, but the reader should know the coverage is by reference rather than by a named gate.

---

## 20. Amendment log

| Date | Change |
|---|---|
| 2026-08-31 | Created from vision suite rev 0.9. 115 gates: 115 named across specs `01`–`09`, 8 merged (`§18`), 8 added as NEW (`§14`). All 12 invariants covered; all 10 test families represented; 9 of 10 risks covered by gates, `R-1` recorded as intentionally ungateable. Eight findings raised (`§19`), of which **F-3 is a Rule-4 stop requiring the owner's decision**. |
| 2026-08-31 | Cross-checked against `MASTER_DEVELOPMENT_PLAN.md`, authored in parallel. Phase names and the ID scheme agree. **F-2 struck — resolved:** the MDP schedules the core-crate extraction at P1-B, the phase this document had independently assigned `TG1-05` to. **F-3 sharpened:** the MDP acknowledges the ACP/MCP collision at `P4-22` but resolves it by build order alone (option (c)) rather than by decision — the stop stands. Three gates annotated as runnable **earlier** than assigned, because the MDP builds their capability sooner (`TG1-05` P1, `TG3-38` P1, `TG4-18` P0); **no ID renumbered**, per `§1`. |
