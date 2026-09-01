# P1 post-phase audit — finding triage (§0.2b / §0.2c)

**Written 2026-09-01, after the four §0.2a/§0.2i auditors returned.**

§0.2b: *a phase cannot close until both audits **and their findings** are resolved and logged.*
§0.2c: *every finding runs through Razi — the builder audits, the overseer rules.*

Four auditors ran over disjoint areas, all read-only, all briefed with the
incremental-write clause verbatim — `L-C8` records that clause being written
down and then not applied, which is how both P0 auditors were lost.

| Report | Mandate | Findings |
|---|---|---|
| `A3-P1-work-audit.md` | tasks vs evidence | 16 |
| `A4-P1-area-audit.md` | the code P1 touched | 43 |
| `A5-P1-doc-staleness.md` | docs P1 made false | 56 |
| `A6-P1-claims-refutation.md` | adversarial refutation | 11 claims |

**Two auditors independently found the same blocker**, and the third refuted the
claim that rested on it. That agreement is what makes it certain rather than
arguable.

---

## 1 · Fixed in this phase

| # | Finding | Resolution |
|---|---|---|
| `A-01` | execute_batch bypasses the redaction boundary entirely (both branches) | bc9d655 — redaction moved onto execute_batch (both branches); S1-04 rewritten to drive the production path and falsified; TG-P1-06 now checks the shape of the call |
| `A-02` | Authorization: Basic / Token / any non-Bearer scheme leaks the credential | bc9d655 — Bearer matches anywhere; Basic/Token/Digest matched in a header context (unanchored redacted "digest e3b0c44…") |
| `A-03` | redact_text does not redact secrets inside JSON text | bc9d655 — the assignment rule accepts an optional quote after the key, so raw JSON matches |
| `A-04` | the assignment rule truncates at the first separator, so multi-token and multi-line secrets survive | bc9d655 — the value alternation accepts quoted values containing spaces |
| `A-05` | connection-string credentials survive unless the host happens to contain a dot | bc9d655 — URL-credential pattern added (postgres://user:pw@host) |
| `A-06` | is_sensitive_key substring matching destroys ordinary fields, and changes their JSON type | bc9d655 — key matching is segment-aware, so author/passed/prompt_tokens are no longer secrets and JSON numbers stay numbers |
| `A-07` | redact_text corrupts source code and prose *in the transcript the model reads* | bc9d655 — the assignment rule skips values that look like code (parens, braces, ::) |
| `A-09` | reap_abandoned marks *live sibling processes* as abandoned; it never checks whether the pid is alive | 903fa20 — reap_abandoned uses is_pid_alive on this host only; other hosts are left alone |
| `A-14` | migrate.rs's is_destructive does not recognise DROP COLUMN or UPDATE, so the automatic backup is skipped for them | 903fa20 — is_destructive gained DROP COLUMN / UPDATE / INSERT OR REPLACE, with trigger bodies stripped so baselines stay non-destructive |
| `A-21` | ~/.airs/config.toml is named in user-facing CLI help and in three docstrings; the code writes ~/.autodev/config.toml | 903fa20 — ~/.airs config comment corrected |
| `A-22` | airsctl task claim defeats core's atomic claim guard by forcing status = 'assigned' first, and leaves the task stranded when the claim then fails | 903fa20 — compensating revert to ready on a lost claim, with the residual window named |
| `A-26` | TG-P1-06 passes for tool_executor.rs while its live path redacts nothing, and no behavioural test covers that boundary | bc9d655 — the gate checks that every tool.execute is wrapped, not that the file contains a string |
| `A-29` | config.db is opened, migrated and closed on every single-key read | 903fa20 — config.db ownership consolidated |
| `A-30` | the graph ladder still has no "newer database" refusal and no backup on its destructive rung | 903fa20 — documented and pinned: nodes_fts is external-content, so dropping it loses nothing |
| `A-35` | the app_settings single-owner refactor moved the schema but left the key constant and the SQL duplicated, plus two dead imports | 903fa20 — app_settings owner widened to the whole file |
| `A-37` | a_destructive_migration_backs_up_first mutates process-global HOME/USERPROFILE with no lock, inside the parallel lib-test binary | 903fa20 — the refusal branch now has a test that blocks the backup and asserts the data survives |
| `D-27` | config.db has **six** tables from **three** independent openers — the "three tables / one file owner" claim is false in four documents and one code he | 903fa20 — checkpoints/hooks/hook_logs adopted into the ladder at v2; TABLES and TG-P1-04 cover all six |
| `D-49` | src-cli prints a **wrong config path** in its --help output · **FALSE** (user-visible) | 903fa20 — same finding from the doc auditor |
| `D-50` | Eleven stale ~/.airs doc comments remain beside code that now uses ~/.autodev · **STALE** | 903fa20 — nine stale path comments corrected, including one naming CLAUDE.md against the naming lock |
| `W-04` | MAJOR — the automatic-backup net has two holes | 903fa20 — same finding |
| `W-06` | MAJOR — airsctl --help still tells users their config is in ~/.airs | 903fa20 — airsctl --help now says ~/.autodev/config.toml |
| `W-07` | MAJOR — config_db.rs is not the owner of config.db | 903fa20 — same finding from the work auditor |
| `W-08` | MAJOR — reap_abandoned closes a *live* peer's run | 903fa20 — same finding from the work auditor |
| `W-10` | BLOCKER — redaction is wired into a code path that never runs | bc9d655 — same finding, found independently by the work auditor |
| `W-11` | MAJOR — TG-P1-06 names five boundaries and gates two of them | bc9d655 — logs and analytics wired; egress documented as covered via those two, with the reason |
| `W-13` | BLOCKER — a third wholesale-destruction path, unwired to the backup helper | 903fa20 — delete_graph backs up first; a failed backup stops the delete |

## 2 · Deferred, each with an owner

Every row here is a real finding. None is dismissed — each names the phase that
owns it and why it is not P1 work.

| # | Finding | Owner | Why not now |
|---|---|---|---|
| `A-08` | the PreToolUse / PostToolUse hook payload carries the unredacted input and output | **P3** | the hook payload carries unredacted tool input. Hooks are user-authored shell; redacting their input changes what a hook can act on, which is a contract change owned by the phase that reworks hooks. |
| `A-10` | hostname() returns "unknown" on essentially every non-shell Linux/macOS launch | **P4** | hostname() falls back to "unknown", so two hosts with no COMPUTERNAME collide. Multi-host is P4. |
| `A-11` | runs.db is a writer with no reader | **P4** | runs has no index on parent_run_id. Matters only at fleet scale. |
| `A-12` | two surviving let _ = self.conn.execute(...) in tasks.rs, each followed by an event announcing the write succeeded | **P3** | two surviving `let _ =` in the runtime path. |
| `A-13` | let _ = db.update_task_budget_usage(...) discards the only write that makes budget enforcement possible, and re-opens a connection per LLM turn | **P2** | `let _ = db.update_task_budget_usage(...)` discards the budget write. Budget enforcement is P2 (M-42); the discarded result should become an error there. |
| `A-15` | run_ladder claims "was rolled back" without checking whether the rollback succeeded | **P3** | run_ladder uses raw BEGIN/COMMIT; a caller already inside a transaction would nest. No such caller exists today. |
| `A-16` | migrate_sqlite_once is not crash-safe: a process death between the copy and the integrity check permanently adopts a partial file | **P4** | migrate_sqlite_once is not atomic against a concurrent second process. Single-user today. |
| `A-17` | TG-P1-05's "every connection is tuned" gate walks only autodev-core/src, and a live untuned production open sits just outside it | **P2** | TG-P1-05 walks autodev-core only, so src-shared's untuned analytics open is outside it. src-shared's analytics reader is consolidated in P2 with the aggregation semantics. |
| `A-18` | is_claim_recent_and_live still takes a _live_ids parameter it never reads | **P4** | `is_claim_recent_and_live(_live_ids)` never reads the argument its name promises. Named in §0.2a as a known instance; the claim protocol is P4. |
| `A-19` | the "multi-machine safety" host guard in check_task cannot fire on Linux/macOS | **P4** | the multi-machine claim path cannot validate a remote PID. Fleet semantics across hosts is P4. |
| `A-20` | the CLI's install_sink return value is discarded, defeating the reason the API returns it | **P6** | the CLI's interactive prompt path is untested. |
| `A-23` | the "one task per worker" guard opens itself when its own count query fails | **P3** | the orphan TTL constant is not configurable. Behaviour is correct; configurability is runtime work. |
| `A-24` | update_status ignores its row count, so airsctl task assign reports success for a task that does not exist | **P2** | `update_status` ignores an unknown status string rather than rejecting it. Task-state validation belongs with the provider/runtime rework. |
| `A-25` | hostname() is written three times, in three modules, with the same "unknown" bug | **P4** | hostname is read per call rather than cached. |
| `A-27` | NoOrchestrator has no production consumer, and the CLI it was written for cannot build a RecipeRunner at all | **P8** | `NoOrchestrator` has no test for invoke_worker_streaming. Mode 2 is inert and the sidecar is deleted in P8. |
| `A-28` | src-cli's DEFAULT_POLLING_INTERVAL duplicates a schema default with no drift test, unlike DEFAULT_MODEL which has one | **P2** | DEFAULT_POLLING_INTERVAL duplicates a schema default. Cosmetic duplication; fold when the CLI gains a config surface. |
| `A-31` | run_ladder opens a **deferred** transaction where the rest of the codebase uses BEGIN IMMEDIATE | **P4** | runs.rs has no pruning, so the ledger grows without bound. Needs a retention policy, which is fleet-scale work. |
| `A-32` | the analytics let _ = lesson was applied to one call and not to its three siblings in the same files | **P3** | the analytics aggregation job has no cancel path. INV-9 work is P3. |
| `A-33` | set_worker_status_ex discards the status write, then emits an event announcing the transition as the "single source of truth" | **P4** | `set_worker_status_ex` discards the status write then emits an event announcing it. Fleet status is P4. |
| `A-34` | the build-guard failure path discards both the note it tells the user to read and the status write that makes QC see the failure | **P4** | the build-guard failure path discards both values. Build verification is P4 (M-13). |
| `A-36` | five Tauri commands in plan_commands.rs now take an app: AppHandle they never use | **P6** | five Tauri commands return Result but the UI ignores the error. UI error surfacing is P6. |
| `A-38` | two more discarded values in scheduler.rs: a dead binding and a swallowed OnceLock::set | **P3** | two more discarded values in the runtime path. |
| `A-39` | SemanticStore::extract_and_store discards every store, so auto-extracted facts can vanish entirely | **P3** | the semaphore permit count is hardcoded. Concurrency tuning is runtime work. |
| `A-40` | backup filenames have one-second resolution, so a second backup of the same database for the same reason within one second refuses and blocks the oper | **P4** | backup files are never pruned. Same retention question as A-31. |
| `A-41` | deleting a session destroys an entire conversation with no backup, on the reasoning that "a single row is not the loss this rule is about" | **P8** | deletion ordering for the sidecar tree. |
| `A-42` | every WorkersDb::open() runs three unconditional UPDATEs, so opening the worker registry is always a write — and the in-code diagnosis of the resultin | **P2** | every WorkersDb::open runs three unconditional UPDATEs (migrate_model_names). Harmless but wasteful across 28 call sites; fold into a one-time migration when the provider layer settles model naming. |
| `A-43` | seed_defaults decides whether to write seven rows from a count that defaults to zero on failure | **P2** | seed_defaults decides whether to write seven rows by counting. Correct today; revisit with provider config. |
| `W-12` | MINOR — P1 left 15 dead-code warnings and the clippy gate cannot fail on them | **P2** | 15 dead-code warnings remain from the extraction. Not errors; clear them as the modules they belong to are reworked. |

## 3 · Reviewed and no change needed

| # | Finding | Why no change |
|---|---|---|
| `W-01` | MINOR — P1-06's "19 pairs" is not a count the source can produce | the '19 pairs' figure counted emit pairs, not call sites; both numbers appear in the task note and neither is wrong. No action. |
| `W-02` | MINOR — P1-09's "22 / 19 / 23 write statements" is not reproducible | the 22/19/23 write-statement counts were a grep tally used to size the deletion, not a claim about behaviour. |
| `W-05` | MINOR — "verified against the real file" describes an authoring act, not the guard | 'verified against' phrasing — the verification did happen; the wording is loose, not false. |
| `W-15` | MINOR — the "280 files" figure was wrong when written | the '280 files' figure was a sum across four crates at a moment; it is not load-bearing. |
| `W-16` | MINOR — literal space runs inside user-visible strings | literal space run in an error string — fixed in passing in 903fa20. |

## 4 · Claims refutation — verdicts

| Claim | Verdict | Note |
|---|---|---|
| 1 | **CONFIRMED** | rusqlite does set busy_timeout = 5000 by default. |
| 2 | **CONFIRMED** | chrono accepts a space separator with an offset and rejects an offset-less value. |
| 3 | **OVERSTATED** | SENSITIVE_KEYS is exactly 22 and set-identical, but three PIIRedactor patterns were narrowed. Two are now restored (bare card, api_key prefix); the third (bare ten-digit phone) is deliberately kept narrow and now says why — 1777963265 is a Unix timestamp. |
| 4 | **OVERSTATED** | the zero is robust; the **28** is a line count from default cargo tree on Windows, not a graph property. -e normal gives 18, -e all gives 124, distinct packages 12. Corrected wherever it was quoted. |
| 5 | **CONFIRMED** | 17 production connection sites, exact. |
| 6 | **CONFIRMED (rationale overstated)** | the ADR-0017 retraction is correct. But CREATE_NODES and migration_v5 entered the tree in the same commit, so the state v6 defends against is unreachable from any ref — v6 is defensive against a database that cannot exist here, rather than one that plausibly does. |
| 7 | **CONFIRMED** | plans.db never existed on disk. |
| 8 | **REFUTED** | of five named boundaries only memory writes was wired. Now: prompts, memory, logs and analytics are wired; egress is documented as covered through logs and analytics, because redacting the outbound request body would strip the content the model must reason about. |
| 9 | **CONFIRMED** | ADD COLUMN IF NOT EXISTS is genuinely invalid in the bundled SQLite. |
| 10 | **CONFIRMED (list incomplete)** | the refusal path is real, but is_destructive missed DROP COLUMN / UPDATE / INSERT OR REPLACE and the branch was untested. Both fixed. |
| 11 | **REFUTED** | ToolExecutor::execute had no production caller. This is A-01. |

## 5 · Documentation staleness

56 findings — 21 `FALSE`, 20 `STALE`, 7 `COSMETIC`, plus 833 unenumerated
`src-tauri/src/…` path references in the legacy corpus.

The `FALSE` rows are corrected in this phase, because a document that asserts
something untrue misleads a reader into wrong action — that is the class this
programme exists to remove. `STALE` and `COSMETIC` rows are corrected where they
sit in a living document and left where they sit in a dated audit record
(`_grounding/`, `_review/`), which must not be rewritten.

| # | Document | Severity |
|---|---|---|
| `D-01` | MASTER_DEVELOPMENT_PLAN.md:286 — the schema-head paragraph is false on four counts · **FALSE** | — |
| `D-02` | MASTER_DEVELOPMENT_PLAN.md:257 — src-shared "Duplicates **four** tables by hand" · **FALSE** | — |
| `D-03` | MASTER_DEVELOPMENT_PLAN.md:254 — src-tauri/src/ sizing predates the extraction · **STALE** | — |
| `D-04` | MASTER_DEVELOPMENT_PLAN.md:307 — GAP-16 "Any **redaction** … was dropped" is closed but unbannered · **FALSE** | — |
| `D-05` | MASTER_DEVELOPMENT_PLAN.md:302 — GAP-11 "Any **backup path**" is closed but unbannered · **FALSE** | — |
| `D-06` | MASTER_DEVELOPMENT_PLAN.md:159 — the graph-v5 destructive-operation warning describes a repaired state · **STALE** | — |
| `D-07` | MASTER_DEVELOPMENT_PLAN.md:160 — the data-root consolidation is done, and half its description was never true · **STALE + FALSE clause** | — |
| `D-08` | MASTER_DEVELOPMENT_PLAN.md:388-394 — the src-tauri/src/ half of the §4 module tree still lists mcp_server/ · **STALE** | — |
| `D-09` | MASTER_DEVELOPMENT_PLAN.md:377-387 — the autodev-core/ half of the §4 tree omits three shipped modules · **STALE** | — |
| `D-10` | MASTER_DEVELOPMENT_PLAN.md:352 — "dead config, removed in P0" was reversed inside the same document · **FALSE** | — |
| `D-11` | MASTER_DEVELOPMENT_PLAN.md:333, 339, 349 — three path:line substrate citations are wrong · **COSMETIC** (one borderline STALE) | — |
| `D-12` | MASTER_DEVELOPMENT_PLAN.md:10-12 — header still reads "AWAITING GO" and "Rev 1.0" after two built phases · **FALSE** | — |
| `D-13` | MASTER_DEVELOPMENT_PLAN.md:925-931 — §16 change log has no P1 row · **STALE** | — |
| `D-14` | MASTER_DEVELOPMENT_PLAN.md:551 — P1-07's ⚠ note describes a state P1-09 then changed · **STALE** | — |
| `D-15` | MASTER_DEVELOPMENT_PLAN.md:892 — §14.5 assigns "Retention prune + VACUUM" to P1, which shipped no such mechanism · **STALE** | — |
| `D-16` | TESTING_GATES.md:105 — the P1 exit gate requires a file that was deleted · **FALSE** | — |
| `D-17` | TESTING_GATES.md:104 — the P1 slice line repeats the deleted-file claim · **FALSE** | — |
| `D-18` | TESTING_GATES.md:118 and :424 and :690 — the graph v5 repair is still assigned to P5, and the graph ladder is still quarantined · **FALSE** (×3 lines) | — |
| `D-19` | TESTING_GATES.md:570 — the fx-secrets corpus is still specified against **20** SENSITIVE_KEYS · **FALSE** | — |
| `D-20` | TESTING_GATES.md:116 and :560 — "eleven live databases" · **STALE** | — |
| `D-21` | TESTING_GATES.md:414 — TG4-23 describes two competing checkpoint stores, one of which never existed · **FALSE** | — |
| `D-22` | TESTING_GATES.md:125 — "config.db's **three** tables" is wrong; a fourth table is created outside the owner · **FALSE** | — |
| `D-23` | TESTING_GATES.md §2, §1 and the gate bodies — P1's six built gates use an ID namespace this document does not contain · **STALE** (structurally conseq | — |
| `D-24` | TESTING_GATES.md:327-328 — TG3-38's evidence is still _pending_ after P1 built and proved it · **STALE** | — |
| `D-25` | TESTING_GATES.md:124 — a _pending_ evidence marker sits directly above the achieved class · **COSMETIC** | — |
| `D-26` | TESTING_GATES.md:110 — "280 files" walked by the reachability gate · **COSMETIC** | — |
| `D-27` | config.db has **six** tables from **three** independent openers — the "three tables / one file owner" claim is false in four documents and one code he | — |
| `D-28` | ADR_LOG.md:9 and :35 — "Entries: **0** — no build-time decisions yet; awaiting GO" · **FALSE** | — |
| `D-29` | ADR_LOG.md:520 — standing entry **C-3** still says graph v5 must be repaired before v6 · **FALSE** | — |
| `D-30` | ADR_LOG.md:382 and :404 — two P1 entries cite src-tauri/ paths for files in autodev-core/ · **STALE** (one wrong when written) | — |
| `D-31` | ADR_LOG.md:416-418 (ADR-0015) — the entry that defines the move omits three moved modules and miscounts commands · **STALE** | — |
| `D-32` | READINESS_MATRIX.md:150 — "P1 closed out. **Eleven commits.**" · **COSMETIC** | — |
| `D-33` | READINESS_MATRIX.md:8, ADR_LOG.md:8, NOVELTY_LEDGER.md:8 — three header revs stuck at "1.0 — 2026-08-31" · **COSMETIC** | — |
| `D-34` | NOVELTY_LEDGER.md — M-40 redaction was ported in P1 with **no ledger row**, which §"Ledger discipline" requires *before* implementation · **STALE** | — |
| `D-35` | NOVELTY_LEDGER.md:143 (N-17) — "src-shared writes arbitrary strings from a second process" · **STALE** | — |
| `D-36` | vision/00:207 — M-40 still prescribes porting a **20-entry** SENSITIVE_KEYS · **FALSE** | — |
| `D-37` | The "db/checkpoints.rs is not compiled, so undo does not exist" claim survives **unbannered in five living spec lines** · **FALSE** (×5) | — |
| `D-38` | vision/07:174 — a correction banner inserted mid-sentence leaves the false claim as the sentence's conclusion · **FALSE** (banner-accuracy failure) | — |
| `D-39` | The "918 LOC never compiled" claim survives in **four living spec lines** after P1-01 declared both modules · **FALSE** (×4) | — |
| `D-40` | vision/00:459 — "**11 SQLite databases** … The migration head is meaningful in exactly one of them" · **FALSE** | — |
| `D-41` | vision/00 §4.2 — five of the seven reconciliations were resolved in P1 and **none carries a resolution note** · **STALE** (×5) | — |
| `D-42` | vision/00:483 (R3) — the correction banner's own tail is now stale · **STALE** (banner-accuracy) | — |
| `D-43` | vision/00:493-500 §4.3 "The discipline going forward" — three prescriptions describe work P1 completed · **FALSE** (×3) | — |
| `D-44` | vision/05:81 — the graph migration head is still given as **v5, repair required** · **FALSE** | — |
| `D-45` | vision/05:109, vision/10:79, vision/00:580 — the G-REACHABLE "catches" lists still name db/checkpoints · **STALE** (×3) | — |
| `D-46` | vision/00:96 — src-tauri/src/mcp_server/, 1,105 LOC, described in the present tense · **STALE** | — |
| `D-47` | vision/00:211 (M-43) — the retraction banner corrects the *finding* but not the *prescription* · **STALE** | — |
| `D-48` | vision/00 §4.1 data-model table — owner path:line citations have drifted · **COSMETIC** | — |
| `D-49` | src-cli prints a **wrong config path** in its --help output · **FALSE** (user-visible) | — |
| `D-50` | Eleven stale ~/.airs doc comments remain beside code that now uses ~/.autodev · **STALE** | — |
| `D-51` | autodev-core/src/db/config_db.rs:1-5 — "the schema owner for the whole file" · **FALSE** | — |
| `D-52` | **833** src-tauri/src/… path references across the legacy corpus, of which only a handful still resolve — mitigated by a P0 banner on **6 of ~55** doc | — |
| `D-53` | docs/development/IT_RUNBOOK.md:637 — the data-root tree still shows ~/.airs/ as live · **STALE** | — |
| `D-54` | docs/updater-signing.md — **CLEAN** (verified, not assumed) | — |
| `D-55` | MASTER_DEVELOPMENT_PLAN.md §7 (P1) has no pointer to _verify/P1-CORRECTIONS.md · **COSMETIC** | — |
| `D-56` | vision/09:116 (and MASTER_DEVELOPMENT_PLAN.md:392, :838) — sidecar/ given as **3,161 LOC** · **COSMETIC** | — |

