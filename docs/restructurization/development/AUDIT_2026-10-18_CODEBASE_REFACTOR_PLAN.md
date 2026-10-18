# Codebase refactor plan — wave 1 (2026-10-18)

**Status: PROPOSAL. Awaiting the owner's decision. Nothing in this document authorises a
cross-cutting refactor.** The active programme remains
`MASTER_DEVELOPMENT_PLAN.md` (P0–P9) and its §0 rules; this plan is subordinate to it and does
not change any P4 scope, gate, or checkbox. Where this plan touches a shipped contract it says
so and stops at owner judgment.

## 1. Why this exists

The owner's standing complaint is that the codebase accumulated *patch-on-patch* code:
workarounds layered on workarounds rather than defects fixed in the layer that owns them. Six
read-only engineering-quality audits were commissioned against disjoint areas to test that
claim with evidence.

**Method.** Each audit ran read-only (no edits, no commits, no live credentials, no billable
provider calls), returned file:line citations plus CONFIRMED/SUSPECTED verdicts, and was told
not to re-derive findings already confirmed elsewhere. Every load-bearing finding was
re-verified in the main session by execution before it was acted on.

| # | Area | State |
|---|---|---|
| 1 | agent / tool executor / permissions / journal | complete |
| 2 | fleet orchestration / recipes / workers / subagents | complete |
| 3 | persistence / migrations / schemas | complete |
| 4 | provider layer / observability / secrets | complete |
| 5 | platform, sandbox, app shell, hooks, vault, CLI | running |
| 6 | frontend (React/TS), e2e, tooling, CI | running |

Wave 1 covers audits 1–4. Audits 5–6 will be folded in as wave 2 when they return; their areas
(UI ↔ command boundary, hooks, sandbox, CI honesty) are expected to add to themes B, D and E
below rather than introduce a new theme.

## 2. Headline finding

The audits independently converged on one signature, repeated across every area: **a single
responsibility implemented more than once, with the copies drifting apart silently.** In three
cases the drift was already live and user-visible:

- **Groq could not run the fleet.** Four `match provider_id` provider constructions existed
  outside `provider/factory.rs`; two had drifted and refused `groq`, which the catalogue,
  Settings and the vault all offer. A Groq user lost fleet workers and subagents completely.
  → **fixed today** (`894bbe0`).
- **A worker was permanently locked out after one successful run.** `kick_worker`'s success
  path suppressed the process-wide `kick_inflight` slot release, contradicting the code's own
  comment. Both dispatch paths consult that set, so the worker never dispatched again for the
  life of the process. → **fixed today** (`a2e7aa8`).
- **`memory.db` and `arcana.db` reported a wrong identity** (`"other"`) in migration telemetry,
  so a reader could not distinguish them from an unknown database. → **fixed today** (`5953f8c`).

The rest of the class is latent but mechanically preventable, which is why several proposed
workstreams below are *guards*, not rewrites.

## 3. Findings by theme

Severity: **S1** = live user-visible defect or security-relevant; **S2** = latent drift that
will become a defect; **S3** = maintainability/dead code.

### Theme A — hand-maintained tables that must be updated by memory (S2)

| Finding | Where | Consequence |
|---|---|---|
| Six hand-maintained tool-name maps, including a 47-name copy in `agent/engine.rs` (`expected_live_names`) | agent audit §3.1 | A tool rename updates some maps and not others; the registry and the copy disagree silently |
| Telemetry label allow-list: 349 entries (55 event types, 294 operations), edited by hand for every new producer | `observability/telemetry_labels.rs` | **Was red on `main`** until seven missing labels were added; a new producer fails the suite until a human adds it |
| 16 `HookType` variants with no firing site | `hooks/types.rs` (agent audit §2.3, §4.4) | The type surface promises behaviour nothing implements |
| `catalogue.rs` ↔ `src/data/models.ts::MODEL_GROUPS` compared by text-scraping | gate `TG-P2-05` | Fields not scraped can drift freely; the test proves the scraper, not the models |
| `TG-P2-04` asserts only that the source *contains* `CappedProvider::wrap` | `src-tauri/src/ci_gate.rs` | The named provider gate cannot catch a new `match provider_id` copy — the hole the Groq drift passed through |
| Provider-id lists without construction: `vault_commands.rs` (`is_supported_provider`), `settings_commands.rs` ×2 | provider audit §4 | Three more places to forget |

### Theme B — one responsibility, several implementations (S2)

| Finding | Where | Note |
|---|---|---|
| `arcana_dir()` implemented twice plus three inline re-derivations | db audit F9 (`arcana-core/src/db/mod.rs`, `src-shared/src/db/mod.rs`, `db/backup.rs`, `memory/db.rs`, `session/storage.rs`, plus 2 in `src-tauri`) | A `.airs` fallback added to one copy desynchronises which root a component reads |
| Four path resolvers | agent audit §3.2 | Same question answered four ways |
| Three retry/backoff authorities with unrelated constants, one a hard-coded `sleep(60s)` that `shutdown()` cannot interrupt | fleet audit A1/A2 (`runner.rs:74-79,99-106`; `scheduler.rs:287-291`; `provider/retry.rs`) | Two policies can disagree about the same failure |
| Four dispatch gates for one concern | fleet audit A6 (`scheduler.rs:469,491,696,704`) | Divergent skip semantics; skips are silent |
| Four-to-five paths that write a worker's task status, several without build evidence | fleet audit A4/A5, §4.3–4.4; db audit F13 | Contends with the P4-14 transition boundary |
| `PRAGMA foreign_keys=ON` re-stated in four openers | db audit F5 | A new opener silently loses FK enforcement |
| Graph store bypasses `migrate::run_ladder` entirely (own version checks, no backup guard, `DROP TABLE` on a rung) | db audit F2 (`graph/migrations.rs:12-50,116`) | The one database the migration engine was built to protect is not using it |
| Graph timestamp triggers created on every open, outside any ladder version | db audit F3 (`graph/store.rs:65-89,122`) | Unversioned, unbacked-up schema mutation |
| Every table has two live schema definitions (baseline + `ADDED_COLUMNS`) | db audit F1 | Pinned by tests, but two hand-maintained column lists per table |
| Budget/spend guards stacked across ≥5 layers | provider audit §4 | Overlap not judgeable: one layer is the owner's frozen WIP |

### Theme C — writes outside the owning boundary, and instrumentation that can lie (S1/S2)

| Finding | Where | Consequence |
|---|---|---|
| `guard_and_validate` looks for the cargo sentinel where sandboxed builds never write, then flips an `in_progress` task to `build_failed` by an out-of-band DB write | fleet audit A4 (`runner.rs:1008-1064`; `hooks/ecosystem.rs:81` vs `process_sandbox/mod.rs:533`) | A successful tick can silently rewrite task state outside the journal, claim and build-evidence paths |
| Hook **infrastructure** failure is reported as a **policy denial** | agent audit §4.1 (`tool_executor.rs:1202-1209` ← `hooks/registry.rs:23,31`) | A DB-open failure reads to the user as "permission denied"; a named P4-51 clause |
| `journal_target_paths` returns empty for both "capture failed" and "genuinely pathless" | agent audit §4.2 | P4-53 clause: the journal cannot distinguish the two |
| Reconciliation envelope omits `outcome_unknown` | agent audit §4.3 | An unknown outcome must be expressed as something it is not |
| `do_not_retry` reconciliation skips `RecoverySnapshot::verify_state` | existing TG4-27 receipt | Safe, but conflict wording is enforced only on continuation paths |
| When the log directory cannot be prepared, logging degrades to console-only with **no writer and no counter** | TG4-24 receipt (`observability/logging.rs:102-111,169-178`) | "No telemetry sink" is indistinguishable from "no telemetry activity" in the Logs UI |
| Error class is encoded into the message text and parsed back out of the `Display` string | provider audit (`agent/engine.rs:1466-1474`) | The typed retry policy is bypassed; changing an adapter's wording silently changes retry behaviour |

**Security note (positive):** audit 4 traced every provider-boundary path and found **no
defect** — bodies are bounded (16 KiB), failures fail closed, keys stay in headers, and no
adapter echoes a provider body into an error or telemetry. Its one warning is that this safety
rests on adapter *authoring discipline*, not on a type.

### Theme D — tests and harnesses that cannot fail (S2)

| Finding | Where | Consequence |
|---|---|---|
| Process-global singletons installed first-wins, second install discarded, and the home they point at is a tempdir that gets deleted | fleet audit A7 (`analytics/mod.rs:136-141`; `runtime.rs:186-189`; `subagent/manager.rs:456-469`; `scheduler.rs:684`) | Order-dependent tests; this is the contamination surface behind the lost-ack trio. `scheduler.rs:1755-1770` already has the guarded pattern to copy |
| Test fixtures hand-write schemas that can drift from the ladder | db audit F10 (`hooks/db.rs:423-426`; `src-shared/src/db/analytics.rs:341-464`) | "A fixture that drifts from the migration proves the fixture, not the product" — the repo's own words |
| Tests share a process-global runtime cache without priming it | fixed today (`project_command` corpus) | Passed only because of test ordering |
| Production struct carries `#[cfg(test)]`/feature seams | fleet audit A8 (`runner.rs:358-414`) | Production shape defined by test needs |

### Theme E — verified dead code (S3)

`RecipeRun`/`RecipeRunStatus` (no construction site anywhere), `SubagentResult.files_modified`
(always empty but surfaced to the model), `cleanup_orphans` (deliberate no-op),
`_session_mgr` (stored, never read), `let _ = worker_opt`, `Tool::is_read_only` (zero call
sites), `hooks/registry.rs::fire_with_logs` (dead), `cost/tracker.rs::format_cost`/`format_tokens`
(no callers), `pricing.rs::is_priced`/`priced_models` (tests only), `mod dirs_next` (removed
today). Note: absence of a static caller is **not** proof of deadness in this repo (registries,
macros and Tauri's invoke surface can reach code dynamically), so each removal needs its own
named proof.

### Theme F — docs and gates that disagree with code (S3)

`db/mod.rs` store map (corrected today), `app_settings.rs` future-tense comment that describes
work already done, `catalogue.rs` "TG-P2-05 fails the build" when it is a `cargo test` failure,
and the project instructions citing `src-shared/src/cost/pricing.rs` where the file is
`arcana-core/src/cost/pricing.rs`.

## 4. Already landed (wave 0) — no approval needed, done at the owning layer

Nine commits, each with its own failing-then-passing evidence:
`49e87e0` `eea621e` `c00a6f0` `ce909be` **`a2e7aa8`** `1e03e08` **`894bbe0`** `5953f8c` `60c5534`.
Two of those are production defects: the leaked worker slot (`a2e7aa8`) and the duplicated
provider dispatch that locked Groq out of the fleet and subagents (`894bbe0`). See
`_verify/2026-10-18-RED-BASELINE-AND-AUDIT-FIXES.md` and ADR-LOG-RESTRUCT-0397/0398.

## 5. Proposed workstreams

Each is independently shippable, leaves the tree green, and carries its own proof. "OD" = needs
an owner decision before work starts.

| WS | Scope | Owning layer | Size | Risk | Proof | OD |
|---|---|---|---|---|---|---|
| **W1** | Mechanical guards for the theme-A tables: one tool-name source + scan; hook-firing scan; make `TG-P2-04` fail on any provider construction outside `factory.rs`; derive `is_supported_provider` from `factory::supported_ids()` | `agent/`, `hooks/`, `ci_gate.rs`, `src-tauri/commands` | M | Low (tests only) | New guard tests + existing gate suite | — |
| **W2** | Delete verified-dead code, one unit per commit, each with a no-caller proof that accounts for registries/macros/invoke | theme E list | S–M | Low | `cargo check` + the owning module's tests | — |
| **W3** | Collapse theme-B duplicates onto one owner: single `arcana_dir`; one path resolver; one retry policy (incl. the uninterruptible 60 s scheduler sleep); one dispatch gate; one task-status writer fed by build evidence; one opener helper that sets every pragma | `db/`, `platform/`, `recipes/`, `agent/` | L | Med | Owning module suites + a gate run in the touched area | — |
| **W4** | Theme-C honesty: distinguish hook infrastructure failure from policy denial (P4-51); add `outcome_unknown` and a "capture failed" journal value (P4-53); surface "no telemetry sink"; decide the fate of `guard_and_validate` | `hooks/`, `agent/`, `observability/`, `recipes/` | M | **Med-High** (visible behaviour) | New negative tests + TG4-24/27/28/30 re-runs | **OD** |
| **W5** | Test-harness integrity: adopt the guarded once-per-process singleton pattern; make `install_global` return `Result`; derive fixtures from the real ladders | `analytics/`, `recipes/`, `hooks/`, `src-shared/` | M | Med | The previously order-dependent suites run in one process | — |
| **W6** | Theme-F doc/gate truth, including a generated (not hand-maintained) telemetry label set | docs + `observability/`, `src-tauri/ci_gate.rs` | S–M | Low | Doc no-stale sweep + suite | **OD** for generation |
| **W7** | Wave 2: fold in audits 5–6 (UI ↔ command contract, hooks/sandbox, CI honesty) | TBD | TBD | TBD | TBD | — |

**Sequencing proposal:** W1 → W2 → W6 (all no-behaviour-change) → W3 → W5 → W4. W7 slot in when
audits 5–6 land. Do **not** run two workstreams that touch the same files concurrently.

## 6. Guardrails for any refactor

1. One workstream at a time, one commit per logical unit, never two writers in one file.
2. Before and after any workstream, run the owning gate(s) for every area it touches — and re-run
   the P4 gates listed in `TESTING_GATES.md`, because these refactors touch files that open P4
   clauses rely on.
3. No refactor may be used to *infer* a P4 gate green; gates are closed only by their named proof.
4. Receipts under `_verify/`, ADR entries at decision time, and doc lines fixed in the same step
   as the change that staled them (§0 no-stale).
5. Destructive-schema work (graph ladder port, dual-format retirement) requires a backup of any
   live database first, per §0.8 — and an explicit owner decision.
6. The owner's uncommitted WIP files stay frozen until the owner says otherwise.

## 7. Excluded from this plan until the owner rules

`arcana-core/src/platform/process_sandbox/mod.rs`,
`platform/process_sandbox/linux_smoke*`, `provider/run_budget.rs`, `e2e/wdio.conf.ts`, `native/`,
`scripts/build-macos-sandbox.sh`, `docs/restructurization/development/P4_EXPANDED_GOAL.md`, and
the untracked session-checkpoint receipts.

## 8. Open questions for the owner

1. **Scope of the frozen WIP:** are those uncommitted files part of the refactor surface, or
   frozen inputs I must design around?
2. **Push approval:** 15 local commits are unpushed on `main`. Approve pushing them (they are all
   individually validated, none touches `main` history rewrites)?
3. **Escalations I want a ruling on** (each is a shipped-contract or user-visible change, so I
   stopped): (a) carry `ErrorClass` structurally instead of parsing it back out of the error
   string; (b) generate the telemetry label set instead of hand-maintaining 349 entries;
   (c) port the graph store onto the shared migration ladder and version its triggers; (d) the
   fate of `guard_and_validate`'s sentinel check (delete, or re-point at the sandboxed build dir).
4. **Priority:** continue closing the remaining P4 rows alongside the refactor, or pause P4 and
   land W1–W3 first?
5. **Wave 2:** fold audits 5–6 into W7 as they land, or re-brief them against this plan's themes?
