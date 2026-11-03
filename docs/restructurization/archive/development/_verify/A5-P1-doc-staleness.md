# A5 — P1 Documentation Staleness Audit

**Auditor:** A5 (P1 post-phase doc-staleness sweep)
**Date:** 2026-09-01
**Scope:** every living document under `docs/` — `docs/restructurization/development/`,
`docs/restructurization/vision/` (00–10 + master plan), `docs/restructurization/*.md`,
and `docs/` root. `vision/_grounding/` and `vision/_review/` are HISTORICAL AUDIT
RECORDS and are NOT reported as stale (only flagged where a *living* document cites
them for a claim that is now false).

**Status:** COMPLETE. Written incrementally — each finding was appended to disk as it was
verified against code, never held in memory.

---

# 1 · Headline

**20 documents in the governing set audited directly** — the 5 in `development/`, all 11 in
`vision/` (`00`–`10`), both anchors, `docs/updater-signing.md` and `_verify/P1-CORRECTIONS.md`
— plus **7 legacy documents inspected** and a pattern sweep (`grep -rn`, never `--limit`)
across the whole `docs/` tree and all four Rust crates. **55 findings: 25 `FALSE`, 22 `STALE`,
8 `COSMETIC`** — ≈98 distinct lines once the multi-instance findings are counted out,
plus **833** unenumerated `src-tauri/src/…` path references in the legacy corpus outside
`restructurization/` (D-52). Every one of the ten P1 changes was verified against code
*before* any document was judged; two of them (**EventSink**, **INV-1 restated**) produced
**no staleness at all**, and that negative result is evidenced rather than assumed.

**The single most misleading line is `MASTER_DEVELOPMENT_PLAN.md:286`** —

> *"**Live schema head:** only `graph/` has a real migration ladder … **head = v5, and v5 is
> a broken statement that must be repaired before v6**. Every other database uses
> `CREATE TABLE IF NOT EXISTS` on open. **11 databases across 3 data roots.**"*

Four assertions, all false after P1, in the paragraph a phase planner reads to size
persistence work — and contradicted by line **627 of its own file** (*"opens **all 10**"*).
A reader trusting it would re-do `P1-10`, `P1-11` and `P1-14`.

**The finding with the widest blast radius is D-27**, which is not a P1 *regression* but a
P1 *error*: `config.db` holds **six** tables written by **three** independent openers
(`db/config_db.rs`, `git/checkpoint.rs:29-45`, `hooks/db.rs:17-25`), while four documents
and one code header state it has three tables and one schema owner — and `SO-10` (INV-10)
is scored **GREEN** on that basis. Three of the six tables sit outside the versioned
ladder, so `config.db`'s `user_version` does not describe its own schema.

**Runner-up for consequence:** thirteen surviving lines still assert *"`db/checkpoints.rs`
is not compiled, so undo does not exist"* (D-16, D-17, D-21, D-37×5, D-38, D-45×3, D-47) —
including a **P1 exit gate that can never be satisfied** (`TESTING_GATES.md:105` requires
`checkpoints` "in the binary"; the file was deleted) and a **live database-inventory row for
a database that never existed** (`vision/00:473`). Four other instances of the same claim
*were* corrected during P1, which makes these the residue of an incomplete sweep.

---

# 2 · Findings

Index below; each entry is written out in full in the sections that follow, with the quoted
text, the code `path:line` that disproves it, and suggested replacement wording.

| # | Document `path:line` | Subject | Sev |
|---|---|---|---|
| D-01 | `development/MASTER_DEVELOPMENT_PLAN.md:286` | schema head: ladders, v5, 11 DBs, 3 roots — all four false | **FALSE** |
| D-02 | `MASTER_DEVELOPMENT_PLAN.md:257` | `src-shared` "duplicates four tables by hand" | **FALSE** |
| D-03 | `MASTER_DEVELOPMENT_PLAN.md:254` | `src-tauri/src/` 53,387 LOC / 32 modules | STALE |
| D-04 | `MASTER_DEVELOPMENT_PLAN.md:307` | GAP-16 redaction "dropped" — closed in P1, unbannered | **FALSE** |
| D-05 | `MASTER_DEVELOPMENT_PLAN.md:302` | GAP-11 "no backup path" — closed in P1, unbannered | **FALSE** |
| D-06 | `MASTER_DEVELOPMENT_PLAN.md:159` | §0.8 destructive-ops: graph v5 repair still listed | STALE |
| D-07 | `MASTER_DEVELOPMENT_PLAN.md:160` | §0.8: data-root consolidation done; plugin/fleet clause never true | **FALSE** |
| D-08 | `MASTER_DEVELOPMENT_PLAN.md:388-394` | §4 tree still lists `mcp_server/`; omits the shims | STALE |
| D-09 | `MASTER_DEVELOPMENT_PLAN.md:377-387` | §4 tree omits `ide/`, `orphan/`, `plan/` | STALE |
| D-10 | `MASTER_DEVELOPMENT_PLAN.md:352` | "dead config, removed in P0" — reversed in same file | **FALSE** |
| D-11 | `MASTER_DEVELOPMENT_PLAN.md:333,339,349` | three wrong substrate `path:line` citations | COSMETIC |
| D-12 | `MASTER_DEVELOPMENT_PLAN.md:10-12` | "AWAITING GO", "Rev 1.0", branch TBD | **FALSE** |
| D-13 | `MASTER_DEVELOPMENT_PLAN.md:925-931` | §16 change log has no P1 row | STALE |
| D-14 | `MASTER_DEVELOPMENT_PLAN.md:551` | P1-07 note: "`src-cli` still reads … through `src-shared`" | STALE |
| D-15 | `MASTER_DEVELOPMENT_PLAN.md:892` | §14.5 retention prune assigned to P1; never built | STALE |
| D-16 | `development/TESTING_GATES.md:105` | P1 exit gate requires deleted `checkpoints` in the binary | **FALSE** |
| D-17 | `TESTING_GATES.md:104` | P1 slice: declare `db/checkpoints.rs`; M-14 is P4 not P1 | **FALSE** |
| D-18 | `TESTING_GATES.md:118`, `:424`, `:690` | graph v5 repair still owned by P5; ladder still quarantined | **FALSE** ×3 |
| D-19 | `TESTING_GATES.md:570` | `fx-secrets` corpus specified against **20** `SENSITIVE_KEYS` | **FALSE** |
| D-20 | `TESTING_GATES.md:116`, `:560` | "eleven live databases"; "including the broken graph v5" | STALE |
| D-21 | `TESTING_GATES.md:414` | TG4-23: two checkpoint stores, one of which never existed | **FALSE** |
| D-22 | `TESTING_GATES.md:125` | "`config.db`'s three tables" *(superseded by D-27)* | **FALSE** |
| D-23 | `TESTING_GATES.md` §1, §2, gate bodies | P1's six built gates use an ID namespace the doc lacks | STALE |
| D-24 | `TESTING_GATES.md:327-328` | `TG3-38` evidence still `_pending_` after P1 built it | STALE |
| D-25 | `TESTING_GATES.md:124` | `_pending_` sits directly above the achieved class | COSMETIC |
| D-26 | `TESTING_GATES.md:110` | "280 files" walked by the reachability gate (now 292) | COSMETIC |
| D-27 | 4 docs + `db/config_db.rs:1-5` | **`config.db`: six tables, three openers, three ungated** | **FALSE** |
| D-28 | `development/ADR_LOG.md:9`, `:35` | "Entries: 0 — awaiting GO" (there are 17) | **FALSE** |
| D-29 | `ADR_LOG.md:520` | standing entry C-3: graph v5 "must be repaired before v6" | **FALSE** |
| D-30 | `ADR_LOG.md:382`, `:404` | two P1 entries cite `src-tauri/` paths for core files | STALE |
| D-31 | `ADR_LOG.md:416-418` | ADR-0015 omits `ide/`,`orphan/`,`plan/`; 255 vs 257 commands | STALE |
| D-32 | `development/READINESS_MATRIX.md:150` | "P1 closed out. Eleven commits." (fifteen) | COSMETIC |
| D-33 | `READINESS_MATRIX.md:8`, `ADR_LOG.md:8`, `NOVELTY_LEDGER.md:8` | three header revs stuck at 1.0 | COSMETIC |
| D-34 | `development/NOVELTY_LEDGER.md` | M-40 redaction ported with **no ledger row** | STALE |
| D-35 | `NOVELTY_LEDGER.md:143`, `TESTING_GATES.md:348`, `vision/04:55` | "`src-shared` writes arbitrary strings from a second process" | STALE ×3 |
| D-36 | `vision/00:207` | M-40 still prescribes a **20-entry** `SENSITIVE_KEYS` | **FALSE** |
| D-37 | `vision/00:274`,`:439`,`:473`; `vision/05:33`; `vision/10:79` | "undo does not exist" survives unbannered | **FALSE** ×5 |
| D-38 | `vision/07:174` | banner spliced mid-sentence; false claim is still the conclusion | **FALSE** |
| D-39 | `vision/00:241`; `vision/05:31`; `vision/06:69`; `vision/03:84` | "918 LOC never compiled" after P1-01 declared both | **FALSE** ×4 |
| D-40 | `vision/00:459` | "11 SQLite databases … head meaningful in exactly one" | **FALSE** |
| D-41 | `vision/00:481,482,485,486,487` (+ `vision/08:100,103`) | 5 of 7 reconciliations resolved in P1, none annotated | STALE ×5 |
| D-42 | `vision/00:483` | R3's correction banner has a stale tail | STALE |
| D-43 | `vision/00:498-500` (+ `vision/08:51,53,59,60`) | §4.3 "discipline going forward" prescribes completed work | **FALSE** ×3 |
| D-44 | `vision/05:81` | graph head still "v5 — repair required before extension" | **FALSE** |
| D-45 | `vision/05:109`, `vision/10:79`, `vision/00:580` | `G-REACHABLE` catches-lists still name `db/checkpoints` | STALE ×3 |
| D-46 | `vision/00:96` | `src-tauri/src/mcp_server/` in the present tense | STALE |
| D-47 | `vision/00:211` | M-43 banner corrects the finding, not the prescription | STALE |
| D-48 | `vision/00:465-475` | data-model owner `path:line` citations drifted | COSMETIC |
| D-49 | `src-cli/src/main.rs:22`; `commands/config.rs:8` | **`airsctl --help` prints the wrong config path** | **FALSE** |
| D-50 | 11 sites in `autodev-core`, `src-shared` | stale `~/.airs` doc comments beside `~/.autodev` code | STALE |
| D-51 | `autodev-core/src/db/config_db.rs:1-5` | "the schema owner for the whole file" *(see D-27)* | **FALSE** |
| D-52 | legacy `docs/` corpus | **833** `src-tauri/src/…` refs; banner on 6 of ~55, P0-only | STALE |
| D-53 | `docs/development/IT_RUNBOOK.md:637` | data-root tree still shows `~/.airs/` as live | STALE |
| D-54 | `docs/updater-signing.md` | **verified CLEAN** — both citations still resolve | — |
| D-55 | `MASTER_DEVELOPMENT_PLAN.md §7` | no pointer to `_verify/P1-CORRECTIONS.md` (§6 has one) | COSMETIC |
| D-56 | `vision/09:116`; MDP `:392`, `:838` | `sidecar/` given as 3,161 LOC (now 3,297) | COSMETIC |

---

## Method and ground truth

### Ground truth established in code (all 10 P1 changes verified before any doc was judged)

| # | Claim | Verified at | Verdict |
|---|---|---|---|
| 1 | `autodev-core` crate exists; agent/providers/memory/db/graph/git/recipes/subagent/security live in `autodev-core/src/` | `autodev-core/src/` listing: `agent analytics budget cost db diff_analysis errors.rs events.rs git graph hooks ide lib.rs memory observability orphan plan platform plugin provider recipes routing security session state subagent vault`. `src-tauri/src/` retains `commands/ mcp/ sidecar/ skills/ updater/ main.rs lib.rs ci_gate.rs reachability_gate.rs logging.rs runtime_config.rs` + re-export shims (`agent.rs`, `db.rs`, `graph.rs`, `memory.rs`, `provider.rs`, …) | CONFIRMED |
| 2 | Session store now `~/.autodev/autodev.db` | `autodev-core/src/session/storage.rs:2` — "Database path: ~/.autodev/autodev.db (was ~/.airs/autodev.db before P1-14)"; `autodev-core/src/db/legacy_root.rs:1-6`; `autodev-core/src/plugin/registry.rs:17` → `.autodev/plugins`; `autodev-core/src/plugin/token.rs:108` → `.autodev` | CONFIRMED |
| 3 | 10 `user_version` ladders | `migrate::user_version` consumers: `db/annotations.rs:59`, `db/config_db.rs`, `db/plans.rs:213`, `db/runs.rs:119`, `db/tasks.rs:323`, `db/workers.rs:141`, `memory/db.rs:24`, `session/storage.rs:46`, `analytics/db.rs`; plus `graph/migrations.rs` (own ladder) = 10 | CONFIRMED |
| 4 | `src-shared` mirrors of tasks/workers/plans deleted; `analytics.rs` remains | `src-shared/src/db/` contains only `analytics.rs`, `mod.rs` | CONFIRMED |
| 5 | `db/checkpoints.rs` DELETED; undo ships via `git/checkpoint.rs` | `autodev-core/src/db/` has no `checkpoints.rs`; `autodev-core/src/git/checkpoint.rs` exists | CONFIRMED |
| 6 | `SENSITIVE_KEYS` = 22 | `autodev-core/src/security/redaction.rs:65` (const), `:231` `assert_eq!(SENSITIVE_KEYS.len(), 22)`, `:14` header states 22 | CONFIRMED |
| 7 | graph at `user_version = 6`; `migration_v6` added | `autodev-core/src/graph/migrations.rs:219` `PRAGMA user_version = 6` | CONFIRMED |
| 8 | `hourly_costs` deleted from analytics diagnostic | `autodev-core/src/analytics/queries.rs:17` — "a table nothing has ever created" | CONFIRMED |
| 9 | `EventSink` replaced global `OnceLock<AppHandle>`; no `emit_X_global` | `autodev-core/src/events.rs:32` `pub trait EventSink`; `:51` `static SINK: OnceLock<Arc<dyn EventSink>>`; `src-tauri/src/events.rs:17` `TauriEventSink` (37 lines total). Zero `emit_*_global` symbols remain | CONFIRMED |
| 10 | INV-1 restated runtime-and-language-scoped in `vision/00` Part 1 | checked below | — |

### Supporting measurements taken for the findings below

| Measurement | Value | How |
|---|---|---|
| `src-tauri/src/` files / LOC | **84 files, 16,231 LOC** | `find src-tauri/src -name "*.rs" \| wc -l`; `find … -exec cat {} + \| wc -l` |
| `autodev-core/src/` files / LOC | **189 files, 47,353 LOC** | same |
| `src-shared/src/` LOC | **511 LOC** | same |
| `#[tauri::command]` count | **257** | `grep -rho "#\[tauri::command\]" src-tauri/src \| wc -l` |
| `src-tauri/src/mcp_server/` | **does not exist** (P0-13); `src-tauri/src/mcp/{health,mod}.rs` exists | `ls` |
| `rusqlite` declaration | `src-tauri/Cargo.toml:43` **and** `autodev-core/Cargo.toml:14` | `grep -n` |
| `keyring` declaration | `src-tauri/Cargo.toml:46` **and** `autodev-core/Cargo.toml:20` | `grep -n` |
| `[profile.release] panic = "abort"` | `src-tauri/Cargo.toml:118-119` — the **root** `Cargo.toml` is a 7-line workspace manifest with no `[profile]` block at all | `cat Cargo.toml`; `grep -n` |
| Workspace members | `autodev-core`, `src-tauri`, `src-cli`, `src-shared`; `autodev-core` added by commit `eab345c` | `cat Cargo.toml`; `git log -- Cargo.toml` |

---

## Findings — `development/MASTER_DEVELOPMENT_PLAN.md`

### D-01 · `MASTER_DEVELOPMENT_PLAN.md:286` — the schema-head paragraph is false on four counts · **FALSE**

Quoted: *"**Live schema head:** only `graph/` has a real migration ladder — `PRAGMA user_version`, 5 migrations, **head = v5, and v5 is a broken statement that must be repaired before v6**. Every other database uses `CREATE TABLE IF NOT EXISTS` on open. **11 databases across 3 data roots.**"*

**What is actually true.** (a) *Every* database now has a ladder — `autodev-core/src/db/migrate.rs:73-154` (`run_ladder`), consumed by `db/annotations.rs:59`, `db/config_db.rs`, `db/plans.rs:213`, `db/runs.rs:119`, `db/tasks.rs:323`, `db/workers.rs:141`, `memory/db.rs:24`, `session/storage.rs:46`, `analytics/db.rs`. (b) The graph head is **v6** — `autodev-core/src/graph/migrations.rs:219` `tx.execute("PRAGMA user_version = 6", [])`. (c) v5's six dead statements were **removed**, so there is no longer a broken statement to repair. (d) The count is **10** databases — MDP `S1-03:627` says so itself, "opens **all 10**" — across **one** data root after `P1-14` (`autodev-core/src/session/storage.rs:2`).

**Severity: FALSE.** The single most misleading line in the corpus: a reader planning P5 graph work would believe the v5 repair is still owed, and a reader planning any persistence work would believe only one ladder exists. It also contradicts line 627 of its own file.

**Suggested replacement:** *"**Live schema head:** every database carries `PRAGMA user_version` and a numbered ladder (`db/migrate.rs::run_ladder`) — **10 databases, one data root (`~/.autodev/`)**. The per-repo `graph/` store is at **v6**; v5's six invalid `ADD COLUMN IF NOT EXISTS` statements were removed and `migration_v6` added defensively (`P1-11`)."*

### D-02 · `MASTER_DEVELOPMENT_PLAN.md:257` — `src-shared` "Duplicates **four** tables by hand" · **FALSE**

Quoted: *"| Shared | `src-shared/src/` | 1,073 LOC | Duplicates **four** tables by hand |"*

**What is actually true.** `P1-09` deleted `src-shared/src/db/{tasks,workers,plans}.rs`; `src-shared/src/db/` now holds only `analytics.rs` and `mod.rs`, and `airsctl` reads through `autodev_core::db` (MDP `:556-560` records exactly this). Measured LOC is **511**, not 1,073.

**Severity: FALSE** — it asserts a defect (hand-mirrored schema drift) that P1 removed; a reader would open a task to fix it.

**Suggested replacement:** *"| Shared | `src-shared/src/` | 511 LOC | `analytics.rs` only — the task/worker/plan mirrors were deleted in `P1-09`; `airsctl` uses `autodev_core::db` |"*

### D-03 · `MASTER_DEVELOPMENT_PLAN.md:254` — `src-tauri/src/` sizing predates the extraction · **STALE**

Quoted: *"| Tauri backend | `src-tauri/src/` | 53,387 LOC, 32 modules | 256 Tauri commands, all registered |"*

**What is actually true.** 84 files / **16,231 LOC** remain in `src-tauri/src/`; 189 files / 47,353 LOC are in `autodev-core/src/`. Command count is **257**, and the same document's §4 tree (`:390`) says 255 — three different numbers in one file.

**Severity: STALE.** **Suggested replacement:** add an `autodev-core` row and restate: *"| Core runtime | `autodev-core/src/` | 47,353 LOC, 189 files | extracted in `P1-05` |"* and *"| Tauri backend | `src-tauri/src/` | 16,231 LOC, 84 files | 257 Tauri commands, all registered; re-export shims at the old paths |"*

### D-04 · `MASTER_DEVELOPMENT_PLAN.md:307` — GAP-16 "Any **redaction** … was dropped" is closed but unbannered · **FALSE**

Quoted: *"| GAP-16 | Any **redaction** — the module grounding ruled PORT was dropped | `R3`, M-40 |"*

**What is actually true.** `P1-20`/`P1-21` ported **both** Python redactors into `autodev-core/src/security/redaction.rs` (`SENSITIVE_KEYS` at `:65`, 22 entries, pinned by `:231`) and wired them at `agent/tool_executor.rs::execute` plus all four memory write paths. The same document records this at `:613-620`. GAP-15 immediately above got a struck-through RETRACTED banner; GAP-16 did not.

**Severity: FALSE** — the gap register is the list a phase planner reads to find unbuilt work; leaving a built capability listed as absent is the inverse of the "inert-but-green" defect §0.16 warns about.

**Suggested replacement:** *"| GAP-16 | ~~Any redaction — the module grounding ruled PORT was dropped~~ → **CLOSED in P1** (`P1-20`/`P1-21`). `autodev-core/src/security/redaction.rs`, 22 `SENSITIVE_KEYS`, wired at the tool-executor funnel and all four memory writes; `TG-P1-06` fails on a new unredacted boundary | `R3`, M-40 |"*

### D-05 · `MASTER_DEVELOPMENT_PLAN.md:302` — GAP-11 "Any **backup path**" is closed but unbannered · **FALSE**

Quoted: *"| GAP-11 | Any **backup path** before destructive operations | `G1 §E.3` |"*

**What is actually true.** `autodev-core/src/db/backup.rs` exists (`VACUUM INTO`, integrity-checked, refuses to overwrite) and `P1-19` wired it into `run_ladder` (destructive rungs) and `TasksDb::clear_all` — recorded in this same document at `:604-610`, and called from `db/migrate.rs:408` and `db/tasks.rs:1859`.

**Severity: FALSE**, same reasoning as D-04. **Suggested replacement:** strike and mark *"**CLOSED in P1** (`P1-19`) — `db/backup.rs`, wired into `run_ladder` and `TasksDb::clear_all`; a failed backup stops the operation."*

### D-06 · `MASTER_DEVELOPMENT_PLAN.md:159` — the graph-v5 destructive-operation warning describes a repaired state · **STALE**

Quoted: *"- **Migration repair of `graph/` v5.** The head migration uses `ADD COLUMN IF NOT EXISTS`, which is not valid SQLite (`G3d F-31`). Repairing it touches a live per-repo graph DB. **Back up `~/.autodev/graphs/*.db` first.**"*

**What is actually true.** `P1-11` removed the six invalid statements and added `migration_v6`; the live store went `5 → 6` with `integrity_check = ok` (`autodev-core/src/graph/migrations.rs:122-146`, `:219`). The list's own governing sentence at `:167` is *"Removing an entry requires proof it is fixed"* — the proof now exists (MDP `:567-571`).

**Severity: STALE.** The backup advice remains sound for any future graph migration, but the stated defect is gone. **Suggested replacement:** *"**Any `graph/` migration.** v5's invalid statements were removed and v6 landed in `P1-11`; the store is at v6. Any *future* rung still touches a live per-repo graph DB — **back up `~/.autodev/graphs/*.db` first.**"*

### D-07 · `MASTER_DEVELOPMENT_PLAN.md:160` — the data-root consolidation is done, and half its description was never true · **STALE + FALSE clause**

Quoted: *"- **The `~/.airs/` → `~/.autodev/` consolidation** (ADR-01-09). Moves `sessions`/`messages` and the plugin/fleet-secret tree. **Back up both roots first.**"*

**What is actually true.** `P1-14` completed the move (`autodev-core/src/db/legacy_root.rs`, `session/storage.rs:21`). And the clause "**and the plugin/fleet-secret tree**" was never true: `legacy_root.rs:4-7` records that `~/.airs/plugins/` and `~/.airs/fleet_secret` *"neither has ever existed on disk"*, and MDP `:582` confirms "the old root contained exactly one file". Nothing moved but `autodev.db`.

**Severity: STALE** for the operation, **FALSE** for the plugin/fleet-secret clause. **Suggested replacement:** *"**~~The `~/.airs/` → `~/.autodev/` consolidation~~ — DONE in `P1-14`.** Only `autodev.db` existed under the old root; `~/.airs/plugins/` and `fleet_secret` were code-only paths. ⚠ `~/.airs/` is **deliberately not deleted** — its removal is a named follow-up needing Razi's approval (§0.8)."*

### D-08 · `MASTER_DEVELOPMENT_PLAN.md:388-394` — the `src-tauri/src/` half of the §4 module tree still lists `mcp_server/` · **STALE**

Quoted (the block): *"`src-tauri/src/` ← the desktop shell, host-bound only / `commands/ [existing] 255 #[tauri::command]` / `updater/ · mcp/ · skills/watcher.rs · main.rs · lib.rs` / `ci_gate.rs · reachability_gate.rs [P0]` / `sidecar/ [P8 DELETE — 3,161 LOC]` / `mcp_server/{server,tool_adapter,port_status}.rs [P0 DELETE — 705 LOC]` / `mcp_server/health_monitor.rs [P0 MOVE out of the sidecar-named tree]`"*

**What is actually true.** `src-tauri/src/mcp_server/` **does not exist** — `P0-13` deleted it and `P0-14` moved the health monitor to `src-tauri/src/mcp/health.rs` (both ticked at MDP `:482`, `:484`). The command count is **257**, not 255. `skills/` is a full seven-file module (`defaults, engine, loader, mod, registry, types, watcher`), not just `watcher.rs`. The tree also omits `logging.rs`, `runtime_config.rs`, and — most importantly — the **re-export shims** (`agent.rs`, `db.rs`, `graph.rs`, `memory.rs`, `provider.rs`, `recipes.rs`, `session.rs`, `state.rs`, `vault.rs`, `hooks.rs`, `ide.rs`, `orphan.rs`, `plan.rs`, `platform.rs`, `plugin.rs`, `routing.rs`, `subagent.rs`, `analytics.rs`, `budget.rs`, `cost.rs`, `diff_analysis.rs`, `errors.rs`, `events.rs`, `git.rs`, `observability.rs`), the mechanism that keeps the pre-P1 paths compiling and the one thing a reader most needs the tree to show.

**Severity: STALE.** **Suggested replacement:** drop the two `mcp_server/` lines, correct 255 → 257, expand `skills/`, and add a line: *"`agent.rs · db.rs · graph.rs · memory.rs · provider.rs · …` [P1] ← re-export shims, `pub use autodev_core::X::*;`, so pre-P1 paths still resolve"*.

### D-09 · `MASTER_DEVELOPMENT_PLAN.md:377-387` — the `autodev-core/` half of the §4 tree omits three shipped modules · **STALE**

**What is actually true.** `autodev-core/src/` contains `ide/`, `orphan/` and `plan/`, none of which appear anywhere in the §4 tree (which lists agent, provider, memory, events.rs, db, graph, git, recipes, hooks, subagent, routing, analytics, budget, cost, vault, state, session, observability, plugin, diff_analysis, errors.rs, platform, security, verify). `orphan/` in particular is load-bearing — `P1-13` at MDP `:578` fixed its TTL parse.

**Severity: STALE** (a reader auditing "is everything in the tree accounted for" would mis-scope). **Suggested replacement:** append to the `[P1 move]` line: *"`ide/ · orphan/ · plan/`"*.

### D-10 · `MASTER_DEVELOPMENT_PLAN.md:352` — "dead config, removed in P0" was reversed inside the same document · **FALSE**

Quoted: *"`tauri.conf.json` declares `dialog` and `fs` plugin blocks for plugins that are **not in `Cargo.toml` and not registered** — dead config, removed in P0."*

**What is actually true.** `P0-26` was **REVERSED** — MDP `:501-502` strikes it: *"The blocks were not dead config — they were the only evidence of a live runtime break… superseded by `P0-31`, which makes it work."* §16's change log (`:931`) records the same reversal. §2 was never updated to match.

**Severity: FALSE**, and self-contradicting within one file. *(Root cause is P0, not P1; reported because the P0 sweep did not catch it and it is still live and unbannered.)* **Suggested replacement:** *"`tauri.conf.json` declared `dialog` and `fs` plugin blocks for plugins Rust neither depended on nor registered — **not** dead config, but the only evidence of a live break in the Logs export. `P0-26` was reversed; `P0-31` registered the plugins and granted the capabilities."*

### D-11 · `MASTER_DEVELOPMENT_PLAN.md:333, 339, 349` — three `path:line` substrate citations are wrong · **COSMETIC** (one borderline STALE)

Quoted: *"`rusqlite = { version = "0.31", features = ["bundled"] }   // src-tauri/Cargo.toml:41`"* · *"`keyring = "2"                        // src-tauri/Cargo.toml:44`"* · *"`[profile.release] panic = "abort"    // Cargo.toml:126-131`"*

**What is actually true.** `rusqlite` is at `src-tauri/Cargo.toml:43` and `keyring` at `:46` (each shifted by P1's edits; both were already off by one before P1 — `git show eab345c^:src-tauri/Cargo.toml` puts them at 42 and 45). `panic = "abort"` is at **`src-tauri/Cargo.toml:118-119`**; the **root** `Cargo.toml` is a 7-line workspace manifest with no `[profile]` block, and was 7 lines before P1 too — that citation names the wrong file entirely.

**Severity: COSMETIC** for the two off-by-two line numbers; the `Cargo.toml:126-131` citation is closer to **STALE**, since it sends a P3 implementer (`P3-01` flips this value) to a file that has never contained it. §2 also does not record that `rusqlite`/`keyring`/`reqwest`/`git2`/`tree-sitter`/`petgraph`/`notify` are **now declared twice** — in `autodev-core/Cargo.toml` (the crate that actually consumes them) as well as `src-tauri/Cargo.toml`. That duplication is the more consequential omission: a P2/P3 version bump made in one file only would silently diverge.

### D-12 · `MASTER_DEVELOPMENT_PLAN.md:10-12` — header still reads "AWAITING GO" and "Rev 1.0" after two built phases · **FALSE**

Quoted: *"| **Status** | ⛔ **AWAITING GO. No code, no migrations, no deletions.** |"* · *"| **Branch** | _to be created at GO_ |"* · *"| **Rev** | 1.0 — created 2026-08-31 |"*

**What is actually true.** P0 closed (8 commits `5fca2bb`…`4b17c6f`, MDP `:931`) and P1 built through `68d0e89` (16 commits, `f71b8b7`…`68d0e89`). Migrations ran against live databases (`P1-10`, `P1-11`, `P1-14`) and deletions happened (`db/checkpoints.rs`, `src-shared/src/db/{tasks,workers,plans}.rs`, `mcp_server/`, `patch.rs`). Every clause of the status line is false; §16 records rev **1.4**, not 1.0.

**Severity: FALSE.** *(Predates P1; still live and not bannered.)* **Suggested replacement:** *"| **Branch** | *(current working branch)* | · | **Rev** | 1.5 — P1 closeout | · | **Status** | 🟢 **P0 closed · P1 built, closeout in progress.** P2 next. |"*

### D-13 · `MASTER_DEVELOPMENT_PLAN.md:925-931` — §16 change log has no P1 row · **STALE**

**What is actually true.** The newest row is *"2026-09-01 | 1.4 | P0 built and closed out"*. P1 is built, with four reversals/retractions of consequence (`P1-02` reversed, `P1-11` premise retracted, `P1-12` premise corrected, `P1-16` collapsed to a confirmation) and four new learning classes (`L-C11`…`L-C14`, already present in §0.16 at `:205-208`). §0.6 puts the progress row *before* the commit, so this row is owed at closeout.

**Severity: STALE** — flagged rather than drafted, since only the builder can write an accurate closeout row. Note §0.16 *was* updated for P1, so the omission is specific to §16.

### D-14 · `MASTER_DEVELOPMENT_PLAN.md:551` — P1-07's ⚠ note describes a state `P1-09` then changed · **STALE**

Quoted: *"⚠ **Linking is not adoption.** `src-cli` still reads tasks/plans/workers/analytics through `src-shared`'s parallel implementations. Collapsing those is `P1-09`, and it carries a shipped-contract decision: `airsctl worker heartbeat` and the `Last Seen` column write and read `last_seen`, **a column the live `workers.db` does not have**, so that command errors today."*

**What is actually true.** `P1-09` (`:556-560`) deleted the tasks/workers/plans mirrors and migrated the live DB; MDP `:558` records *"`worker list` exits **0**, `heartbeat` persists a timestamp `status` reads back"*. Only **analytics** still routes through `src-shared` (`src-shared/src/db/analytics.rs`, deliberately retained per `:560`). The present tense — "still reads… so that command errors today" — is now false.

**Severity: STALE.** **Suggested replacement:** append *"→ **RESOLVED by `P1-09`.** Only `analytics` still routes through `src-shared`, deliberately."*

### D-15 · `MASTER_DEVELOPMENT_PLAN.md:892` — §14.5 assigns "Retention prune + `VACUUM`" to P1, which shipped no such mechanism · **STALE**

Quoted: *"| **Retention prune + `VACUUM`** | Timer | **Daily**, on app start if more than 24 h have elapsed | Seconds | Log; retry next day; **never block startup** | P1 |"*

**What is actually true.** No P1 task (`P1-01`…`P1-22`) builds a retention prune or a scheduled `VACUUM`. `autodev-core/src/db/backup.rs` uses `VACUUM INTO` for backups — a different operation, not on a timer. P1 is built and this row's owning phase has passed with the mechanism unbuilt.

**Severity: STALE** (an owning-phase attribution reality did not honour). Flagged rather than corrected — reassigning a phase is Razi's call, not a sweep's.


---

## Findings — `development/TESTING_GATES.md`

### D-16 · `TESTING_GATES.md:105` — the P1 exit gate requires a file that was deleted · **FALSE**

Quoted: *"**Exit gate (Part 6):** `graph_tools`, `graph_prompts`, `checkpoints` are in the binary; a guard test proves every source file is reachable; no agent write can land outside a resolved workspace."*

**What is actually true.** `db/checkpoints.rs` was **deleted**, not declared (`P1-02`, `ADR-LOG-RESTRUCT-0012`). `autodev-core/src/db/` contains no `checkpoints.rs`; the sole checkpoint owner is `autodev-core/src/git/checkpoint.rs`, whose five commands are registered at `src-tauri/src/lib.rs:386-390`. `checkpoints` can never be "in the binary" — the exit gate as written is unsatisfiable.

**Severity: FALSE**, and the highest-consequence class the brief names: a reader taking this exit gate at face value would try to restore a deleted file to close P1. `TG1-01` at `:111` carries an accurate correction banner for the *same* claim; this line, four lines above it, does not.

**Suggested replacement:** *"**Exit gate (Part 6):** `graph_tools` and `graph_prompts` are in the binary; a guard test proves every source file is reachable in all four crates. ⚠ `checkpoints` was struck from this exit gate — `db/checkpoints.rs` was deleted (`P1-02`), undo ships via `git/checkpoint.rs`. The workspace-resolver clause is **owned by P4** (`P4-17`/`P4-18`), not P1."*

### D-17 · `TESTING_GATES.md:104` — the P1 slice line repeats the deleted-file claim · **FALSE**

Quoted: *"**Slice:** declare the undeclared modules (M-17 + `db/checkpoints.rs`) · the reachability guard test · the workspace resolver fails loudly (M-14)."*

**What is actually true.** Same as D-16 — the file was deleted. Additionally, "the workspace resolver fails loudly (M-14)" is **not** P1 work: the MDP places it at `P4-17` (`MASTER_DEVELOPMENT_PLAN.md:795`), and no P1 task builds it.

**Severity: FALSE.** **Suggested replacement:** *"**Slice:** declare the undeclared modules (M-17) · delete `db/checkpoints.rs` (`P1-02`, reversed) · the reachability guard test · the migration ladders and one schema owner (`P1-09`…`P1-15`) · redaction (M-40, `P1-20`/`P1-21`). *(The workspace resolver, M-14, is P4-17 — it was never P1.)*"*

### D-18 · `TESTING_GATES.md:118` and `:424` and `:690` — the graph v5 repair is still assigned to P5, and the graph ladder is still quarantined · **FALSE** (×3 lines)

Quoted `:118`: *"→ the **`graph/` ladder clause is owned by P5**: v5 must be repaired before v6, and P5 is the slice that does it. Until then the graph ladder is quarantined out of this gate **with the quarantine list itself asserted** — a quarantine that can silently widen is a skipped gate (`§1`)."*
Quoted `:424`: *"P5 also **owns two gates listed earlier**: `TG3-15` … and the `graph/`-ladder clause of `TG1-03` (the broken v5 head must be repaired before v6). P5 cannot close until both are green."*
Quoted `:690`: *"Generalised to **every** ladder, with the `graph/` ladder's broken v5 head quarantined and the quarantine list itself asserted"*

**What is actually true.** `P1-11` did this work. `autodev-core/src/graph/migrations.rs:122-146` removed v5's six invalid `ADD COLUMN IF NOT EXISTS` statements, and `:219` `tx.execute("PRAGMA user_version = 6", [])` ships `migration_v6`; the live store went `5 → 6` with `integrity_check = ok` (MDP `:567-571`). There is no broken v5 head, and no reason for the graph ladder to remain quarantined out of `TG1-03`.

**Severity: FALSE**, on three counts each: it assigns completed work to a future phase, it declares a false precondition for P5 closing, and — worst — it leaves a gate clause **quarantined** when the reason for the quarantine is gone. §1's own rule at `:28` is *"A skipped gate is not green"*; this is a live skip with a stale justification.

**Suggested replacement (`:118`):** *"→ **RESOLVED in P1.** v5's dead statements were removed and `migration_v6` landed (`P1-11`); the graph store is at v6. The graph ladder is **no longer quarantined** and is inside this gate. P5 owns no clause of `TG1-03`."* — and strike the `TG1-03` half of `:424`, and update `:690`'s note.

### D-19 · `TESTING_GATES.md:570` — the `fx-secrets` corpus is still specified against **20** `SENSITIVE_KEYS` · **FALSE**

Quoted: *"| **`fx-secrets`** | Synthetic credentials matching each of the 20 `SENSITIVE_KEYS` entries, planted in tool arguments, file contents, environment dumps, prompts, log lines and memory proposals | `TG3-38`, `TG0-04` |"*

**What is actually true.** `SENSITIVE_KEYS` has **22** entries — `autodev-core/src/security/redaction.rs:65` (the const), pinned by `:231` `assert_eq!(SENSITIVE_KEYS.len(), 22)` and stated in the module header at `:14`. This document's *own* `TG3-38` at `:327` already carries the correction — *"the count was 20 here and in the plan; the source has 22, counted in `P1-20`"* — and `:570` was not regenerated with it.

**Severity: FALSE.** A corpus built to this spec would under-cover the redactor by two keys, and `TG3-38` would pass while two `SENSITIVE_KEYS` entries were never exercised — a gate that cannot fail on the thing it exists to catch. This is §0.16 **L-C1** ("a derived artifact was not regenerated after its source changed") occurring inside the very edit that recorded the correction.

**Suggested replacement:** *"Synthetic credentials matching each of the **22** `SENSITIVE_KEYS` entries (`security/redaction.rs:65`, pinned by the `len() == 22` test), **plus** the `PIIRedactor` shapes — email, phone, SSN, card, `sk-`/`pk-` keys, bearer tokens, `password=` — planted in tool arguments, file contents, environment dumps, prompts, log lines and memory proposals."*

### D-20 · `TESTING_GATES.md:116` and `:560` — "eleven live databases" · **STALE**

Quoted `:116`: *"Applies to all eleven live databases."*
Quoted `:560`: *"| **`fx-migrate-heads`** | A snapshot of each of the eleven databases at its live head, **including the broken `graph/` v5**, plus one snapshot per intermediate ladder version |"*

**What is actually true.** **Ten.** `TG1-03`'s own evidence line at `:117` says *"**10 of 10** databases reach their ladder head"*, and MDP `S1-03:627` says *"opens **all 10**"*. The `:560` clause "including the broken `graph/` v5" is additionally false per D-18 — v5 is repaired and the head is v6.

**Severity: STALE** for the count (`:116` contradicts `:117` one line below it); the "broken `graph/` v5" clause in `:560` is **FALSE**.

### D-21 · `TESTING_GATES.md:414` — `TG4-23` describes two competing checkpoint stores, one of which never existed · **FALSE**

Quoted: *"The two competing checkpoint tables — `checkpoints` in `config.db` and `file_checkpoints` in `checkpoints.db` — **collapse to one**, asserted by `TG1-05`."*

**What is actually true.** Searched, not sampled: `grep -rn "checkpoints.db\|file_checkpoints"` across `autodev-core/src`, `src-tauri/src`, `src-cli/src`, `src-shared/src` and `src/` returns **zero hits**. There is exactly one `checkpoints` table, created at `autodev-core/src/git/checkpoint.rs:33` in `config.db`. MDP `:594` records the same: *"**zero** references to `checkpoints.db` anywhere in the four crates, and no `~/.autodev/checkpoints.db` on disk, so no second store was ever created and there is nothing to merge."* The gate's remaining substance — a failed checkpoint must block the write (M-43) — is intact; only this clause is false. `TG1-05` also does not assert this: it asserts one `CREATE TABLE` per table name, which a single checkpoint table already satisfies.

**Severity: FALSE.** **Suggested replacement:** *"~~The two competing checkpoint tables … collapse to one, asserted by `TG1-05`.~~ → **VOID (`P1-16`).** There was only ever one store: `checkpoints` in `config.db`, created by `git/checkpoint.rs`. `checkpoints.db` and `file_checkpoints` never existed on disk or in the tree. The live half of this gate is the failed-checkpoint-blocks-the-write clause above."*

### D-22 · `TESTING_GATES.md:125` — "`config.db`'s **three** tables" is wrong; a fourth table is created outside the owner · **FALSE**

Quoted: *"**Evidence: `GATE-PROVEN`** — `TG-P1-04` in `ci_gate.rs` asserts exactly one creator for each of `config.db`'s three tables … `app_settings`' three creators now route through `db/config_db.rs`, which owns the file …"*

**What is actually true.** `config.db` holds a **fourth** table, `checkpoints`, created at `autodev-core/src/git/checkpoint.rs:33` through its own `crate::db::open_tuned(&autodev_dir()?.join("config.db"))` at `:30` — a second connection to the same file, outside `db/config_db.rs`'s ladder, creating a table and two indexes with `CREATE TABLE IF NOT EXISTS`. So the file has one *version* owner but not one *schema* owner, and `TG-P1-04` does not cover the fourth table.

The same claim appears in two more places and is false in all three:
- `MASTER_DEVELOPMENT_PLAN.md:563` — *"Three modules keep disjoint tables there … `db/config_db.rs` now owns the connection, the schema and the version … `TG-P1-04` covers all three tables."*
- `autodev-core/src/db/config_db.rs:1-5` (code header) — *"`~/.autodev/config.db` — the schema owner for the whole file. Three modules keep tables here: `app_settings`, `autodev_instructions` and `model_configs`."*

⚠ This was **wrong when written in P1**, not made wrong later: `git show eab345c^:src-tauri/src/git/checkpoint.rs` shows `Connection::open(autodev_dir()?.join("config.db"))` at `:30` and the same `CREATE TABLE IF NOT EXISTS checkpoints` at `:33` before the extraction. The fourth writer was already there.

**Severity: FALSE** — it asserts a completed invariant (INV-10, one schema owner) that the code does not hold, and it is the exact "guard that cannot fire" shape §0.2a briefs the area auditor to hunt. **Recommend Razi decide** whether `checkpoints` moves into `config_db.rs`'s ladder or `config.db` is documented as having a sanctioned second writer — either way all three lines above need correcting.

### D-23 · `TESTING_GATES.md` §2, §1 and the gate bodies — P1's six built gates use an ID namespace this document does not contain · **STALE** (structurally consequential)

**What is actually true.** P1 built six gates named `TG-P1-01` … `TG-P1-06`:

| Code ID | Location | What it asserts | Nearest doc gate |
|---|---|---|---|
| `TG-P1-01` | `src-tauri/src/ci_gate.rs:449` (+ detector `:451`) | `autodev-core`'s manifest declares no Tauri dependency | **none** |
| `TG-P1-02` | `src-tauri/src/ci_gate.rs:488` | no core source file references `tauri` in code | **none** |
| `TG-P1-03` | `.github/workflows/ci.yml:81` (CI step only — **not** a Rust test) | crate boundary checked transitively via `cargo tree` | **none** |
| `TG-P1-04` | `src-tauri/src/ci_gate.rs:550` | one creator per `config.db` table | doc **`TG1-05`** |
| `TG-P1-05` | `src-tauri/src/ci_gate.rs:670` | one tuned connection path; no direct `Connection::open` | doc **`TG1-06`** (partly) |
| `TG-P1-06` | `src-tauri/src/ci_gate.rs:761` | redaction reaches every boundary | doc **`TG3-38`** |

Three consequences: (a) `TG-P1-01/02/03` have **no entry anywhere in `TESTING_GATES.md`**, so three built, running gates are outside "the definition of done for the programme"; (b) the numbering **collides** — code `TG-P1-04` implements doc `TG1-05`, so a reader resolving "TG-P1-04" against the document lands on `TG1-04` (backup-before-destructive-migration), a different gate; (c) §2's phase index (`:43`) still says P1 has **6** gates and `:8`/`:51` still say **115 total**, when nine distinct P1 gates now exist across the two namespaces.

§1 `:29` states *"IDs are `TG<phase>-<nn>` and are stable forever. Never renumber. Append only."* — P1 introduced a parallel scheme rather than appending to this one.

**Severity: STALE**, but it is the finding with the widest blast radius here: three gates that can fail are invisible to the document that decides whether a phase may close. **Suggested fix:** append `TG1-07`/`TG1-08`/`TG1-09` for the crate-boundary trio, record the `TG-P1-nn` → `TG<p>-<nn>` mapping in a note under §2, and update the counts. *(Renaming the code constants is a code change and out of a sweep's scope — flagged for Razi.)*

### D-24 · `TESTING_GATES.md:327-328` — `TG3-38`'s evidence is still `_pending_` after P1 built and proved it · **STALE**

Quoted `:328`: *"… · Proves: **INV-2** (its spirit: no secret leaves the process), supports **INV-12** · Evidence: _pending_"*

**What is actually true.** The document's own next line (`:330`) says *"**runnable from P1.** … This gate is therefore expected green from P1 onward"*, and P1 delivered it: `autodev-core/src/security/redaction.rs`, wired at `agent/tool_executor.rs::execute` and all four memory writes, with `autodev-core/tests/redaction_boundaries.rs` reading the databases back with plain SQL, `TG-P1-06` in `ci_gate.rs:761` as the static half, and `S1-04` end to end (MDP `:613-620`, `:628`). §1 `:16` requires *"replace `_pending_` with the class actually achieved"*.

**Severity: STALE.** Note the gate is not fully green — its `fx-secrets`/`fx-nofire-secrets` corpus halves and the "logs, analytics, any egress" boundaries are not proven — so the honest record is a **PARTIAL** status block in the style of `TG0-01`/`TG0-03`, not a tick. **Suggested replacement:** *"Evidence: **`UNIT-PROVEN` + `GATE-PROVEN` (PARTIAL)** — ✅ both redactors ported (22 keys); ✅ wired at the tool-result funnel and all four memory write paths, proven behaviourally by `tests/redaction_boundaries.rs`; ✅ `TG-P1-06` fails on a new unredacted boundary. ❌ the `fx-secrets` / `fx-nofire-secrets` corpora do not exist yet; ❌ logs, analytics and other egress boundaries are not covered."*

### D-25 · `TESTING_GATES.md:124` — a `_pending_` evidence marker sits directly above the achieved class · **COSMETIC**

Quoted: *"Source: spec `08 §4 G-OWNER` · Catches: … · Proves: **INV-10** · Evidence: _pending_"* — immediately followed at `:125` by *"**Evidence: `GATE-PROVEN`** — …"*.

**What is actually true.** The P1 editor appended a new evidence line rather than replacing the `_pending_` token, so the gate carries two contradictory evidence records. **Severity: COSMETIC** (nobody is misled about the outcome), but it defeats any mechanical *"count the remaining `_pending_` gates"* progress check.

### D-26 · `TESTING_GATES.md:110` — "280 files" walked by the reachability gate · **COSMETIC**

Quoted: *"walking **all four** crates (280 files) with a per-crate floor."*

**What is actually true.** `find autodev-core/src src-tauri/src src-cli/src src-shared/src -name "*.rs" | wc -l` reports **292**. The figure was accurate when written mid-P1 and drifted as P1-18/P1-19/P1-20 added files (`db/runs.rs`, `db/backup.rs`, `db/legacy_root.rs`, `db/timestamps.rs`, `db/config_db.rs`, `security/redaction.rs`, tests). The gate itself asserts a **floor**, not an equality (`src-tauri/src/reachability_gate.rs:152-161`, `:205-207`), so nothing is broken — only the prose count. **Severity: COSMETIC.** Prefer *"walking all four crates (292 files at P1 close) with a per-crate floor plus a workspace total"*, or drop the absolute number since the gate deliberately does not pin one.

---

## Findings — `development/READINESS_MATRIX.md`, `ADR_LOG.md`, `NOVELTY_LEDGER.md`

### D-27 · `config.db` has **six** tables from **three** independent openers — the "three tables / one file owner" claim is false in four documents and one code header · **FALSE**

This supersedes and strengthens **D-22**. Enumerated by search, not sample:

| Opener | Connection | Tables it creates in `config.db` | Inside the versioned ladder? |
|---|---|---|---|
| `autodev-core/src/db/config_db.rs:83-90` | `open_at` (owns `user_version`) | `app_settings`, `autodev_instructions`, `model_configs` | **yes** |
| `autodev-core/src/git/checkpoint.rs:29-45` | its own `crate::db::open_tuned(&autodev_dir()?.join("config.db"))` | `checkpoints` + `idx_checkpoints_file` + `idx_checkpoints_session` | **no** |
| `autodev-core/src/hooks/db.rs:17-25` | its own `crate::db::open_tuned(&path)`, then its own `migrate()` | `hooks`, `hook_logs` | **no** |

**Six tables, three openers, one ladder covering three of six.** The affected lines:

1. `docs/restructurization/development/TESTING_GATES.md:125` — *"`TG-P1-04` … asserts exactly one creator for each of `config.db`'s three tables … `db/config_db.rs`, which owns the file"*
2. `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md:563` — *"Three modules keep disjoint tables there … `db/config_db.rs` now owns the connection, the schema and the version … `TG-P1-04` covers all three tables."*
3. `docs/restructurization/development/READINESS_MATRIX.md:90` — SO-10 (**INV-10**, one definition per schema) scored 🟢 **GREEN**, its note reading *"`app_settings`' three creators route through `db/config_db.rs`, which owns the file because `user_version` belongs to the file, not a table."*
4. `autodev-core/src/db/config_db.rs:1-5` (code header) — *"`~/.autodev/config.db` — the schema owner for the whole file. … Three modules keep tables here: `app_settings`, `autodev_instructions` and `model_configs`."*

**And `vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:467` already says six:** *"| 3 | `~/.autodev/config.db` | **six modules** | `autodev_instructions`, `model_configs`, `hooks`, `hook_logs`, `app_settings`, `checkpoints` |"* — the older vision document is **correct** and the newer P1-authored lines are wrong. This is a §0.13 dual-anchor conflict that resolves against the new documents.

⚠ **This was wrong when written in P1, not made wrong afterwards.** `git show eab345c^:src-tauri/src/git/checkpoint.rs` shows the same `config.db` open and `CREATE TABLE IF NOT EXISTS checkpoints` at `:30`/`:33` before the extraction; `hooks/db.rs` likewise predates P1. Both writers were visible while `P1-15` was being written.

**Severity: FALSE**, and the most consequential finding in this sweep after D-01, for three reasons:
- **SO-10 is scored GREEN on a guard that covers half the file.** `READINESS_MATRIX.md:38` states overlay rows are *"scored from 'is the system actually guarded', not from gate colour"* — this is the exact case that rule exists for.
- **`TG-P1-04` is narrower than the invariant it claims.** A second creator planted for `hooks` or `checkpoints` would not be caught. That is §0.16's **L-C6** class ("a check written from the CLAIM rather than from the INVARIANT it protects") recurring after being recorded — which §0.2n `:134` says is *"a **worse** finding than a novel one"*.
- **A future migration on `config.db` can be silently skipped.** `hooks/db.rs` runs its own `migrate()` and `git/checkpoint.rs` runs bare `CREATE TABLE IF NOT EXISTS`, both outside `config_db`'s `user_version` — so `run_ladder`'s version stamp does not describe the file's real schema. That is precisely the failure mode `P1-10` exists to remove, surviving inside the file `P1-15` declared owned.

**Suggested wording (all four locations):** *"`config.db` holds **six** tables written by **three** openers: `db/config_db.rs` owns `app_settings`/`autodev_instructions`/`model_configs` **and the file's `user_version`**; `git/checkpoint.rs` creates `checkpoints` and `hooks/db.rs` creates `hooks`/`hook_logs`, each on its own connection outside the ladder. `TG-P1-04` currently covers three of the six."* **Decision for Razi (§0.15 stop-and-ask — a shipped schema):** fold `checkpoints`/`hooks`/`hook_logs` into `config_db`'s ladder, or record a sanctioned exception and widen `TG-P1-04` to all six. SO-10 should read AMBER until one is chosen.

### D-28 · `ADR_LOG.md:9` and `:35` — "Entries: **0** — no build-time decisions yet; awaiting GO" · **FALSE**

Quoted `:9`: *"| **Entries** | **0** — no build-time decisions yet; awaiting GO |"*
Quoted `:35`: *"This log stays empty until the first build-time decision."*

**What is actually true.** The file contains **17** entries, `ADR-LOG-RESTRUCT-0001` … `0017` (`grep -c "^## ADR-LOG-RESTRUCT-"` returns 18, of which one is the `NNNN` template at `:68`). Eight are P0, nine are P1, and two are retractions. `:8` still reads *"Rev 1.0 — 2026-08-31"* and §6's change log at `:537` still carries only the 1.0 row — *"Zero entries — build not started"*.

**Severity: FALSE.** A reader checking "have any build-time decisions been made?" against the header gets the wrong answer for both closed phases. **Suggested replacement:** *"| **Rev** | 1.2 — 2026-09-01 | · | **Entries** | **17** — `0001`…`0008` (P0), `0009`…`0017` (P1); two are retractions (`0010`, `0017`) |"*, and replace `:35`'s sentence with a pointer to §4.

### D-29 · `ADR_LOG.md:520` — standing entry **C-3** still says graph v5 must be repaired before v6 · **FALSE**

Quoted: *"| C-3 | `graph/` migration head **v5 is a broken statement** (`ADD COLUMN IF NOT EXISTS`) | It must be repaired before v6 is added, and the repair touches live per-repo graph DBs — back up first (§0.8). |"*

**What is actually true.** `P1-11` removed v5's six invalid statements and shipped `migration_v6`; the live store is at v6 (`autodev-core/src/graph/migrations.rs:219`). §5 is explicitly a **living advisory table** — its preamble at `:514` says it exists *"because a future session will otherwise rediscover them the hard way"* — so unlike the numbered entries it is not append-only and must be kept current. `ADR-LOG-RESTRUCT-0017`, thirty lines above in the same file, retracts the premise.

**Severity: FALSE.** A future session reading §5 as its briefing would open work P1 completed and treat a live graph DB as needing repair. **Suggested replacement:** *"| C-3 | ~~`graph/` migration head v5 is a broken statement~~ → **DONE in `P1-11`** (`ADR-0013`, retracted in part by `ADR-0017`). v5's dead statements are deleted, `migration_v6` ships, head is v6. | The `let _ =`-swallowing idiom that let v5 stamp a version over six failures is the lasting lesson — any new rung propagates errors and checks `PRAGMA table_info`. Live per-repo graph DBs still need a §0.8 backup before **any** migration. |"*

### D-30 · `ADR_LOG.md:382` and `:404` — two P1 entries cite `src-tauri/` paths for files in `autodev-core/` · **STALE** (one wrong when written)

Quoted `:382` (ADR-0013): *"**Where.** `src-tauri/src/graph/migrations.rs`"*
Quoted `:404` (ADR-0014): *"**Where.** `src-tauri/src/db/backup.rs`"*

**What is actually true.** Neither path exists. `src-tauri/src/` has no `graph/` or `db/` **directory** — only the one-line shim files `src-tauri/src/graph.rs` (`pub use autodev_core::graph::*;`) and `src-tauri/src/db.rs` (`pub use autodev_core::db::*;`). The files are `autodev-core/src/graph/migrations.rs` and `autodev-core/src/db/backup.rs`.

`ADR-0013` was **already wrong when written**: commit ordering is `eab345c` (extraction) → `98b5fc1` (the v6 migration), so `migrations.rs` was in `autodev-core` before `0013` was logged. `ADR-0017:503`, the entry that partially supersedes it, gives the correct path. `ADR-0014` was correct when written (`f71b8b7` predates `eab345c`) and drifted afterwards.

**Severity: STALE.** §6 `:531` forbids editing a past entry, so the fix is a dated correction line inside each entry (the file's own retraction idiom), not a silent path rewrite. *(ADR-0012's `:358` `src-tauri/src/db/checkpoints.rs` is left alone: the file genuinely was at that path when it was deleted in `6c67ebb`, and it no longer exists anywhere.)*

### D-31 · `ADR_LOG.md:416-418` (ADR-0015) — the entry that defines the move omits three moved modules and miscounts commands · **STALE**

Quoted `:416`: *"`autodev-core` takes … `db/`, `graph/`, `git/`, `routing/`, `analytics/`, `hooks/`, `recipes/`, `subagent/`, `agent/`, plus the already-moved `provider/`, `memory/`, `platform/`, `session/`, `state/`, `vault/`, `budget/`, `cost/`, `plugin/`, `observability/`, `diff_analysis/`, `errors/`, `events/`."*
Quoted `:418`: *"`src-tauri` keeps only what is genuinely host-bound: `commands/` (255 `#[tauri::command]`), `updater/`, `mcp/`, `sidecar/`, `skills/watcher.rs`, `main.rs`/`lib.rs`, and the CI gates."*

**What is actually true.** `autodev-core/src/` also contains **`ide/`, `orphan/` and `plan/`**, named in neither list. The count is **257** `#[tauri::command]`, not 255. `src-tauri/src/skills/` retained all seven files (`defaults, engine, loader, mod, registry, types, watcher`), not `watcher.rs` alone; `logging.rs` and `runtime_config.rs` also stayed and are unlisted.

**Severity: STALE.** This matters more than an ordinary omission because `:442` declares this entry **supersedes MDP §4's module tree** — so D-09's gap in §4 is inherited from here, and correcting §4 without correcting this entry re-creates it. **Suggested fix:** a dated correction line adding `ide/ · orphan/ · plan/` to the moved set and `logging.rs · runtime_config.rs · skills/ (all)` to the retained set.

### D-32 · `READINESS_MATRIX.md:150` — "P1 closed out. **Eleven commits.**" · **COSMETIC**

**What is actually true.** **Fifteen.** `git log --oneline 7c5b7a1..HEAD | wc -l` = 15, from `f71b8b7` (backup helper) through `68d0e89` (P1-A task marking), where `7c5b7a1` is the P0 closeout commit. **Severity: COSMETIC** — nothing depends on the number, but §6 is append-only, so the correction belongs in the next row rather than an edit.

### D-33 · `READINESS_MATRIX.md:8`, `ADR_LOG.md:8`, `NOVELTY_LEDGER.md:8` — three header revs stuck at "1.0 — 2026-08-31" · **COSMETIC**

**What is actually true.** `READINESS_MATRIX.md` §5 `:130` says *"= 8.0 at rev 1.4"* and its §6 log reaches rev 1.4 — the header says 1.0. `ADR_LOG.md` carries 17 entries at rev 1.0. `NOVELTY_LEDGER.md` sits at rev 1.0 with an unchanged change log. `MASTER_DEVELOPMENT_PLAN.md:11` has the same defect (D-12).

**Severity: COSMETIC** individually, but it is **four of five** documents in the family, so a reader cannot use the header rev to tell which documents moved in P1. **Suggested fix:** bump each header rev in the same step as the closeout row, and add a `TG`-style doc check asserting header rev == newest change-log rev (this is a `DOC-GATE` the family currently lacks).

### D-34 · `NOVELTY_LEDGER.md` — M-40 redaction was ported in P1 with **no ledger row**, which §"Ledger discipline" requires *before* implementation · **STALE**

**What is actually true.** `P1-20` ported two externally-sourced mechanics out of `core-orchestrator`'s Python — `security/redaction.py`'s `redact_dict`/`SENSITIVE_KEYS` and `logging/centralized_collector.py`'s `PIIRedactor` — into `autodev-core/src/security/redaction.rs`. The ledger's own rule at `:172` is: *"**New rows** — any mechanic adopted from an external source mid-build gets a row **before** it is implemented, not after."* No `N-` row exists for it. The only mention is a clause inside **N-15** (`:126`), where redaction appears as a *"unique addition"* to the Memory Gateway — a different mechanic, still `ADOPTED-INTENT`, whose gateway half is P3.

Comparable ports from the same source *do* have rows: N-16 (the thinking detector), N-17 (the FSMs), N-05 (LiteLLM's abstraction). Redaction is the one the adversarial review singled out as the capability that *"appeared zero times across the plan and all ten specs"* — the ledger existing to prevent exactly that, and then not recording it, is the same shape as **L-C1**.

**Severity: STALE** (a governance artifact not regenerated), with real teeth: `:170`'s closeout rule is *"at final closeout every row is `ADOPTED-PROVEN` or `REJECTED`"*, and a mechanic with no row is outside that check entirely. **Suggested fix:** add **N-21 · The Python redaction pair**, source `G4b §A.14` / `security/redaction.py` + `logging/centralized_collector.py`; adopt both (22-entry `SENSITIVE_KEYS`, `PIIRedactor` shapes); reject the blanket *"32+ alphanumeric"* rule **for free text** (it eats commit SHAs — the documented deviation at MDP `:615`); unique addition — ordering the narrow token rules before the assignment rule, the defect that let a bearer token through (`L-C14`); proof `TG3-38` / `TG-P1-06`; status `ADOPTED-PROVEN` for the write-path half.

### D-35 · `NOVELTY_LEDGER.md:143` (N-17) — "`src-shared` writes arbitrary strings from a second process" · **STALE**

Quoted: *"an enum only helps inside Rust — `workers.status` is unconstrained `TEXT` written by a second process, so `FromStr` must fail loudly and a `CHECK` constraint guards the column."*

**What is actually true.** `src-shared/src/db/workers.rs` was **deleted** in `P1-09`; `airsctl` writes `workers` through `autodev_core::db` — the same code the desktop runs (`ADR-LOG-RESTRUCT-0016`). The *conclusion* still stands (`airsctl` is still a second process, and `workers.status` still carries no `CHECK` — confirmed: no `CHECK` on the status column in `autodev-core/src/db/workers.rs`), but the named mechanism is gone.

The same stale mechanism appears at **`TESTING_GATES.md:348`** (`TG4-01`'s *"Catches"*: *"`src-shared` writing arbitrary strings into unconstrained `TEXT` from a second process (`R1 B-11`)"*) and **`vision/04-THE-FLEET.md:55`** (*"`src-shared` writes arbitrary `&str` into it **from a second process** (`airsctl`)"*).

**Severity: STALE** across all three. The risk survives P1 and `P4-03` still owes the `CHECK`; only the attribution is wrong. **Suggested replacement:** *"`workers.status` is unconstrained `TEXT` and `airsctl` is a second process writing it — through core's own store since `P1-09`, which removes the schema-drift half but not the unconstrained-column half. `FromStr` must fail loudly and a `CHECK` constraint must guard the column (`P4-02`/`P4-03`)."*

---

## Findings — the vision suite (`vision/00`…`vision/10`)

### D-36 · `vision/00:207` — M-40 still prescribes porting a **20-entry** `SENSITIVE_KEYS` · **FALSE**

Quoted: *"Port `redact_dict` + its 20-entry `SENSITIVE_KEYS`; apply at every boundary where content leaves the process or enters durable storage: prompts, logs, memory writes, analytics, and any egress"*

**What is actually true.** **22** entries — `autodev-core/src/security/redaction.rs:65`, pinned by `:231` `assert_eq!(SENSITIVE_KEYS.len(), 22)`. `TESTING_GATES.md:327` records the correction explicitly — *"⚠ the count was 20 **here and in the plan**; the source has 22, counted in `P1-20`"* — naming this very line, and the plan was still not updated. The mechanic also names only `redact_dict`; `P1-20` ported `PIIRedactor` too, and `TESTING_GATES.md:327` says so.

**Severity: FALSE.** This is the M-40 row in the mechanics register — the canonical statement of what the mechanic *is*, which every downstream artifact derives from (it is where `fx-secrets`'s "20" in `TESTING_GATES.md:570`, D-19, comes from). **Suggested replacement:** *"Port `redact_dict` + its **22**-entry `SENSITIVE_KEYS` **and** `logging/centralized_collector.py`'s `PIIRedactor` (email, phone, SSN, card, `sk-`/`pk-`, bearer, `password=`); apply at every boundary where content leaves the process or enters durable storage. **Delivered in `P1-20`/`P1-21`**, with one documented deviation: Python's blanket 32+-alphanumeric rule is kept for structured values and excluded from free text, because a 40-char commit SHA would otherwise be redacted out of file contents."*

### D-37 · The "`db/checkpoints.rs` is not compiled, so undo does not exist" claim survives **unbannered in five living spec lines** · **FALSE** (×5)

Change #5 in the audit brief, searched not sampled. Some instances were corrected in P1; these five were missed:

| # | Location | Quoted text | Bannered? |
|---|---|---|---|
| a | `vision/00:274` (M-38) | *"undo where feasible (note `db/checkpoints.rs` is **not compiled**, so undo does not currently exist — `G3d F-44`)"* | **no** |
| b | `vision/00:439` | *"the same defect independently hides `db/checkpoints.rs`, which is why undo/rollback is not in the shipped app (`G3d F-44`)"* | **no** |
| c | `vision/00:473` (data model row 9) | *"\| 9 \| `~/.autodev/checkpoints.db` \| `db/checkpoints.rs:67` \| `file_checkpoints` — **module not compiled** \|"* | **no** |
| d | `vision/05:33` | *"The same defect independently hides `db/checkpoints.rs`, which is why undo/rollback is not in the shipped app (`G3d F-44`)."* | **no** |
| e | `vision/10:79` | *"`db/checkpoints.rs` (undo missing from the binary)"* | **no** |

**What is actually true.** Undo ships and always did: `autodev-core/src/git/checkpoint.rs`, five commands registered at `src-tauri/src/lib.rs:386-390` (`create_checkpoint`, `restore_checkpoint`, `list_checkpoints`, `delete_checkpoint`, `compare_checkpoints`), `checkpoints` table created at `git/checkpoint.rs:33` in `config.db`, `RETENTION_PER_FILE = 50` at `:24`. `db/checkpoints.rs` was **deleted** in `P1-02` (`ADR-LOG-RESTRUCT-0012`), and `checkpoints.db` never existed — `grep -rn "checkpoints.db\|file_checkpoints"` over all four crates plus `src/` returns **zero hits**.

Row (c) is the worst of the five: it lists `~/.autodev/checkpoints.db` as one of the **live** databases in the verified-state inventory, with a `path:line` owner for a deleted file. It is also one of the two rows that make that table's count 11 rather than 10.

**Severity: FALSE** for all five — the brief's own highest-priority class. `vision/00:211` (M-43), `vision/00:483` (R3), `vision/00:1177` (G-08) and `vision/07:174` all *were* corrected in P1, which makes these five the residue of an incomplete sweep rather than an unknown. **Suggested replacement (pattern):** *"~~undo does not exist — `db/checkpoints.rs` is not compiled~~ → **RETRACTED (`ADR-LOG-RESTRUCT-0010`).** Undo ships via `git/checkpoint.rs`; `db/checkpoints.rs` was deleted in `P1-02` and `checkpoints.db` never existed. The live defect is `M-43`: a failed checkpoint is logged at `debug!` and the write proceeds."* — and delete row 9 from the `vision/00 §4.1` table entirely.

### D-38 · `vision/07:174` — a correction banner inserted mid-sentence leaves the false claim as the sentence's conclusion · **FALSE** (banner-accuracy failure)

Quoted verbatim: *"**The rule: confirmation is proportional to blast radius, in one idiom.** Undo where feasible — noting — ⚠ **corrected 2026-09-01 (`ADR-LOG-RESTRUCT-0010`)**: undo ships via `git/checkpoint.rs`; `db/checkpoints.rs` was deleted in P1, and the real defect is that a *failed* checkpoint is logged at `debug!` while the write proceeds (`M-43`) — that `db/checkpoints.rs` is **not compiled**, so undo does not currently exist at all (spec `05 §1.1`)."*

**What is actually true.** The banner is factually correct, but it was spliced **into the middle** of the original sentence rather than replacing or striking it, so the sentence still terminates on *"`db/checkpoints.rs` is **not compiled**, so undo does not currently exist at all"* — presented as the surviving claim, with a live cross-reference to `spec 05 §1.1` (which is itself stale, D-37d). A reader parsing the sentence from "Undo where feasible — noting — … — that …" gets the retracted claim as the main clause.

**Severity: FALSE.** The brief asks that banners themselves be checked; this one is the case where the check pays. **Suggested replacement:** *"**The rule: confirmation is proportional to blast radius, in one idiom.** Undo where feasible. ⚠ **Corrected 2026-09-01 (`ADR-LOG-RESTRUCT-0010`):** undo ships via `git/checkpoint.rs`; `db/checkpoints.rs` was deleted in `P1-02`. ~~`db/checkpoints.rs` is not compiled, so undo does not currently exist at all~~. The real defect is `M-43` — a *failed* checkpoint is logged at `debug!` while the write proceeds."*

### D-39 · The "918 LOC never compiled" claim survives in **four living spec lines** after `P1-01` declared both modules · **FALSE** (×4)

| # | Location | Quoted text |
|---|---|---|
| a | `vision/00:241` (M-17) | *"`graph_tools.rs` + `graph_prompts.rs` (918 LOC) have no `mod` declaration and **are never compiled** (`G2a T32`)"* |
| b | `vision/05:31` | *"…have **no `mod` declaration anywhere in `src-tauri`** (`G2a T32`). 918 lines that have never been compiled. The agent has no graph access at all — not even opt-in."* |
| c | `vision/06:69` | *"`graph_tools` and `graph_prompts` are in none of them because their modules are **never declared** (spec `05 §1.1`)."* |
| d | `vision/03:84` | *"the two modules that would expose it to an agent are **not compiled at all** (spec `05`)."* |

**What is actually true.** `autodev-core/src/agent/builtin_tools/mod.rs:13-14` declares `pub mod graph_prompts;` and `pub mod graph_tools;`, and both compile (`P1-01`, `P1-03`; `cargo check --workspace` exits 0). The files are now 899 + 78 = **977** lines, not 918. The *conclusion* — the agent still has no graph tools — remains true for a different reason, which the documents do not say: **0 of the 27 structs in `graph_tools.rs` implement `Tool`** (`grep -c "impl Tool for"` returns 0) and nothing registers them; wiring is P3/BUG-GRAPH.

**Severity: FALSE.** These lines send a reader to fix a declaration that exists and would leave the actual blocker — zero `Tool` impls, zero registrations — undiscovered. `TESTING_GATES.md:111` states the correct position precisely (*"Compiling is not wiring"*); the specs were not brought into line. **Suggested replacement (pattern):** *"`graph_tools.rs` (899 LOC) and `graph_prompts.rs` (78) were never compiled until `P1-01` declared them in `agent/builtin_tools/mod.rs`. They compile now — and the agent still has no graph access, because **0 of the 27 structs implement `Tool`** and no registration entry point lists them. Declaring was `P1`; wiring is `P3` (BUG-GRAPH)."*

### D-40 · `vision/00:459` — "**11 SQLite databases** … The migration head is meaningful in exactly one of them" · **FALSE**

Quoted: *"**11 SQLite databases** (`G1 §E.1`, `CODE-PRESENT`). The migration head is meaningful in exactly one of them."*

**What is actually true.** Every database carries a ladder after `P1-10`, and the count is **10**. §4.3 of the *same Part*, 32 lines below at `:491`, carries a `⚠ Superseded by P1-10` banner reading *"All **10** databases now carry a `PRAGMA user_version` ladder"* — so Part 4 contradicts itself, and the contradiction is on the sentence a reader hits first. `vision/08:27` repeats the same *"**11 databases**"* count **above** its own supersession banner at `:29`, so that banner does not cover it either.

**Severity: FALSE.** **Suggested replacement:** *"**10 SQLite databases** (was 11 — `checkpoints.db` never existed; row 9 removed). **Every one carries a `PRAGMA user_version` ladder** since `P1-10`; the table below records ownership."*

### D-41 · `vision/00 §4.2` — five of the seven reconciliations were resolved in P1 and **none carries a resolution note** · **STALE** (×5)

| Row | Line | Quoted (abridged) | Resolved by | Note present? |
|---|---|---|---|---|
| **R1** | `:481` | *"Sessions live under `~/.airs/`, everything else under `~/.autodev/` — a third data root"* | `P1-14` | **no** *(its twin at `vision/08:41` **does** have one)* |
| **R2** | `:482` | *"`app_settings` has three independent creators — `commands/workspace_commands.rs:23`, `commands/github_mcp_commands.rs:30`, `ide/workspace.rs:35`"* | `P1-15` | **no** |
| **R5** | `:485` | *"`src-shared` mirrors the `tasks` schema by hand and has already drifted"* | `P1-09` | **no** |
| **R6** | `:486` | *"`analytics/queries.rs:29` queries `hourly_costs`, a table `migrate()` never creates … \| Delete or create it"* | `P1-17` (deleted) | **no** |
| **R7** | `:487` | *"Timestamp format is inconsistent … disabling the orphan TTL and the QC-age alert"* | `P1-13` | **no** |

**What is actually true.** `P1-14` moved the session store (`autodev-core/src/db/legacy_root.rs`, `session/storage.rs:2`); `P1-15` routed all three `app_settings` creators through `db/config_db.rs`; `P1-09` deleted `src-shared/src/db/{tasks,workers,plans}.rs`; `P1-17` deleted the `hourly_costs` reference (`autodev-core/src/analytics/queries.rs:17` — *"a table nothing has ever created"*); `P1-13` created `db/timestamps.rs` and made the orphan TTL fire for the first time (MDP `:578`).

**R3 is the exception** and shows the standard the others should meet — it has an inline correction at `:483` — though that correction's own tail is now stale (D-42).

**Severity: STALE** (×5). Individually each is a resolved row still reading as open; collectively §4.2 is the table a phase planner reads to find outstanding schema work, and **5 of 7 rows are done**. The duplicates at **`vision/08:100`** (*"`app_settings` has three independent creators — `commands/workspace_commands.rs:23`, `commands/github_mcp_commands.rs:30`, `ide/workspace.rs:35`"* — resolved by `P1-15`) and **`vision/08:103`** (*"Also delete or create `hourly_costs`"* — deleted by `P1-17`) have the same defect and also sit in §3.7, outside the `:29` banner. **Suggested fix:** add a *(Resolved by `P1-nn`: …)* clause to each, in the style `vision/08:41` already uses.

### D-42 · `vision/00:483` (R3) — the correction banner's own tail is now stale · **STALE** (banner-accuracy)

Quoted tail: *"`db/checkpoints.rs` is uncompiled and has **zero callers**, so it creates no second database today"*

**What is actually true.** The banner's head is accurate (undo ships). Its tail is written in the present tense about a file that no longer exists — `P1-02` deleted it (`ADR-0012`). "Uncompiled with zero callers" describes the state *before* the deletion. **Severity: STALE.** **Suggested replacement for the tail:** *"`db/checkpoints.rs` was **deleted** in `P1-02` — it had never been compiled and had zero callers, so no second database was ever created and there was nothing to merge (`P1-16`)."*

### D-43 · `vision/00:493-500` §4.3 "The discipline going forward" — three prescriptions describe work P1 completed · **FALSE** (×3)

The `⚠ Superseded by P1-10` banner at `:491` scopes itself to *"The paragraph below"* (singular) — `:493`. The numbered discipline list at `:498-500` sits outside that scope and is unbannered:

| # | Line | Quoted | Actually true |
|---|---|---|---|
| a | `:498` | *"New migrations are appended after the **verified live head** — v5 for `graph/`, v0 for everything else, and **v5 must be repaired before v6 is added**."* | v5 was repaired and **v6 shipped** in `P1-11` (`autodev-core/src/graph/migrations.rs:219`). Non-graph databases are at **v1**, not v0 (`P1-10`: *"7 of 7 databases `uv=0 → v1`"*). |
| b | `:499` | *"Today only `memory.db` sets WAL; … and no `busy_timeout` anywhere — so a concurrent writer gets an immediate `SQLITE_BUSY`."* | All databases are WAL after `P1-12`; and the `busy_timeout` premise was **retracted by measurement** — `rusqlite` sets 5000 ms on every connection it opens (MDP `:575`). Both halves false. |
| c | `:500` | *"**A backup taken before any destructive migration.** There is no backup path today."* | `autodev-core/src/db/backup.rs` exists and `run_ladder` invokes it automatically before any destructive rung (`db/migrate.rs:408`); a failed backup stops the operation (`P1-19`). |

The identical prescriptions in **`vision/08:51`** (*"v5 for `graph/` (after repair), v0 for everything else"*), **`vision/08:53`** (*"There is no backup path today"*), **`vision/08:59`** (*"`journal_mode = WAL` — today only `memory.db` does"*) and **`vision/08:60`** (*"`busy_timeout` — **today nothing does**, so a concurrent writer gets an immediate `SQLITE_BUSY`"*) are stale for the same three reasons, and `vision/08`'s own banner at `:29` sits in §1 — two sections above §3 — so it plainly does not cover them either.

**Severity: FALSE.** Item (b) is the most misleading: it would send a P2/P3 implementer to "fix" a `busy_timeout` that a dependency already sets, which is exactly the `L-C11` error P1 made and recorded.

### D-44 · `vision/05:81` — the graph migration head is still given as **v5, repair required** · **FALSE**

Quoted: *"Migration head **v5** (`PRAGMA user_version`, `graph/migrations.rs:12-45`) — repair required before extension."*

**What is actually true.** Head is **v6** (`autodev-core/src/graph/migrations.rs:219`), the repair is done, and the file is no longer at `graph/migrations.rs:12-45` — the dispatcher is at `:8` and the ladder now runs to `:219`. `vision/05` is the spec P5 will be written from, so this is the line most likely to cause duplicated work.

**Severity: FALSE.** **Suggested replacement:** *"Migration head **v6** (`PRAGMA user_version`, `autodev-core/src/graph/migrations.rs`). v5's six invalid `ADD COLUMN IF NOT EXISTS` statements were removed and a defensive `migration_v6` added in `P1-11` (`ADR-0013`, premise retracted by `ADR-0017` — no column was ever lost)."*

### D-45 · `vision/05:109`, `vision/10:79`, `vision/00:580` — the `G-REACHABLE` "catches" lists still name `db/checkpoints` · **STALE** (×3)

Quoted `vision/05:109`: *"\| **G-REACHABLE** \| Every `.rs` under `src/` is reachable from the crate root \| `graph_tools`, `graph_prompts`, `db/checkpoints` \|"*
Quoted `vision/00:580`: *"\| **Reachability** \| … \| `graph_tools`, `db/checkpoints`, the 47 dead commands, the 3 dead events \|"*
Quoted `vision/10:79`: *"\| **Reachability** \| … \| `graph_tools` + `graph_prompts` (918 LOC never compiled), `db/checkpoints.rs` (undo missing from the binary), 2 missing commands, 47 dead commands, … \|"*

**What is actually true.** All three entries were resolved by `P1-01`/`P1-02`/`P1-04`: two declared, one deleted, and `KNOWN_UNREACHABLE` in `src-tauri/src/reachability_gate.rs` is **empty** (MDP `:540`). The gate's scope also changed — it walks **all four crates** (`reachability_gate.rs:152`, per-crate floors plus a workspace total), not *"every `.rs` under `src/`"*.

**Severity: STALE.** **Suggested replacement (pattern):** *"Every `.rs` in **each of the four workspace crates** is reachable from its crate root. ✅ **Green since `P1-04`** — `KNOWN_UNREACHABLE` is empty; `graph_tools`/`graph_prompts` declared, `db/checkpoints.rs` deleted. The **registration** clause remains unmet: 0 of 27 graph-tool structs implement `Tool` (P3)."*

### D-46 · `vision/00:96` — `src-tauri/src/mcp_server/`, 1,105 LOC, described in the present tense · **STALE**

Quoted: *"The AutoDev MCP server **already is Rust** (`src-tauri/src/mcp_server/`, 1,105 LOC). Today a worker's tool call travels: Rust tool registry → in-process HTTP MCP server → Python `MCPBridge` → litellm → the model → back."*

**What is actually true.** `src-tauri/src/mcp_server/` does not exist — deleted in `P0-13`; `health_monitor.rs` was relocated to `src-tauri/src/mcp/health.rs` by `P0-14`. `S0-04` proved `curl -X POST localhost:8767/mcp` gives connection refused. *(P0-caused, not P1; reported because it is unbannered and this is the sweep that found it.)*

**Severity: STALE** — the passage's argument (deleting the sidecar collapses the round trip) still holds; only the present tense is wrong. **Suggested replacement:** *"The AutoDev MCP server **was** Rust (`src-tauri/src/mcp_server/`, 1,105 LOC) and was deleted in `P0-13`. Before that a worker's tool call travelled: …"*

### D-47 · `vision/00:211` (M-43) — the retraction banner corrects the *finding* but not the *prescription* · **STALE**

**What is actually true.** The banner correctly retracts *"undo does not exist"*, but the mechanic's fix column still opens with *"**Compile the checkpoint module**; a failed checkpoint blocks the write; …"* and its paths column still lists *"`db/checkpoints.rs`, `git/`, `agent/builtin_tools/`"*. There is no checkpoint module left to compile — `db/checkpoints.rs` was deleted, and the surviving work is entirely in `git/checkpoint.rs`.

**Severity: STALE.** A P4 implementer reading M-43's fix column as their task list starts with an impossible first step. **Suggested replacement:** fix column → *"~~Compile the checkpoint module~~ (void — deleted in `P1-02`); a **failed checkpoint blocks the write** (`auto_checkpoint_before_write` currently returns `()` and logs at `debug!`); every agent write stays inside the resolved workspace; each task gets its own worker branch"*; paths column → *"`git/checkpoint.rs`, `agent/builtin_tools/{edit_file,write_file}.rs`"*.

### D-48 · `vision/00 §4.1` data-model table — owner `path:line` citations have drifted · **COSMETIC**

The table at `:465-475` cites owners by module-relative path plus a line number. The paths are still valid as module paths (they resolve under `autodev-core/src/`), but the line numbers moved when P1 rewrote these files for the ladders. Spot-checked: row 11 cites `graph/registry.rs:62,123` — the two `.autodev/graph.db` joins are now at `:65` and `:129`; row 6 cites `memory/db.rs:122` — the path resolution is now at `:140`. Row 9's citation (`db/checkpoints.rs:67`) points at a deleted file (D-37c).

**Severity: COSMETIC** — no reader is misled about ownership; only the jump targets are off. Worth a single pass rather than eleven separate corrections, and worth prefixing the table with `autodev-core/src/` so the paths are unambiguous post-extraction.

---

## Findings — documentation **inside the code** that P1 made false

These are not under `docs/`, but they are documentation lines that P1 falsified, and two of them are **user-visible**. MDP `§0.3` ("any line in any project document") and Razi's Rule 5 ("a docstring that promises one behaviour while the code does another **is a bug**") both reach them.

### D-49 · `src-cli` prints a **wrong config path** in its `--help` output · **FALSE** (user-visible)

| Location | Quoted | Actually |
|---|---|---|
| `src-cli/src/main.rs:22` | `/// Path to config file (default: ~/.airs/config.toml)` | `src-shared/src/config.rs:9-15` joins **`.autodev`** and returns `~/.autodev/config.toml` |
| `src-cli/src/commands/config.rs:8` | `/// Create default config at ~/.airs/config.toml` | same |

Both are `clap` doc-comments, so they are printed by `airsctl --help` and `airsctl config --help`. A user following the help text looks in a directory the binary no longer writes to. `P1-14` repointed the path (MDP `:585` records fixing `config_path()`'s `create_dir_all` of `~/.airs`) and left the help text describing the old root.

**Severity: FALSE**, and the only finding in this sweep that a **user** can hit directly.

### D-50 · Eleven stale `~/.airs` doc comments remain beside code that now uses `~/.autodev` · **STALE**

MDP `:582` claims *"All nine `~/.airs` path references repointed."* The **paths** were repointed; the **comments describing them** were not. Searched, not sampled:

| Location | Quoted comment | Code beside it |
|---|---|---|
| `autodev-core/src/db/mod.rs:8` | *"Session data lives in the existing `~/.airs/autodev.db` (see session/storage.rs)."* | `session/storage.rs:2` says `~/.autodev/autodev.db` — the two headers now contradict each other |
| `autodev-core/src/memory/migration.rs:2` | *"Reads all existing sessions from `~/.airs/autodev.db`"* | reads the consolidated store |
| `autodev-core/src/plugin/registry.rs:2` | *"The local registry lives at `~/.airs/plugins/<name>/manifest.yaml`."* | `:17` `join(".autodev").join("plugins")` |
| `autodev-core/src/plugin/registry.rs:29` | *"Scan `~/.airs/plugins/` and return all installed plugin manifests."* | same |
| `autodev-core/src/plugin/registry.rs:93` | *"Write a manifest … to `~/.airs/plugins/<name>/`."* | same |
| `autodev-core/src/plugin/registry.rs:119` | *"Remove a plugin directory from `~/.airs/plugins/<name>/`."* | same |
| `autodev-core/src/plugin/token.rs:7` | *"a random 32-byte key stored in `~/.airs/fleet_secret`"* | `:108` `join(".autodev").join("fleet_secret")` |
| `src-shared/src/config.rs:1` | *"`~/.airs/config.toml` — airsctl configuration file types and I/O."* | `:14` `join(".autodev")` |
| `src-shared/src/config.rs:8` | *"Resolved path to `~/.airs/config.toml`."* | same |
| `src-shared/src/config.rs:63` | *"Load from `~/.airs/config.toml` …"* | same |
| `src-shared/src/config.rs:74` / `:82` | *"Persist to / Create a default config file at `~/.airs/config.toml`."* | same |

**Legitimate, not findings:** `db/legacy_root.rs` (the migration *source*, deliberately), `db/backup.rs:16`/`:247` and `examples/migrate_check.rs:71` (historical narrative, past tense), `agent/builtin_tools/list_directory.rs:104` (`".airs"` as an ignore-list entry), `src-cli/src/repl.rs:8` (`.airs_history`, an actual filename), `src-tauri/src/updater/install.rs:167`/`:182` (`com.airs-foundry.autodev.update`, a bundle identifier).

**Severity: STALE.** `db/mod.rs:8` is the sharpest — two module headers in the same crate now give different answers for where the session store lives. **Suggested fix:** repoint all eleven; and correct MDP `:582` to *"All nine `~/.airs` **path expressions** repointed; the surrounding doc comments and the `airsctl --help` text were missed and are fixed in the closeout sweep."*

### D-51 · `autodev-core/src/db/config_db.rs:1-5` — "the schema owner for the whole file" · **FALSE**

Already covered as location 4 of **D-27**; repeated here so the code-side list is complete. Three openers create six tables in `config.db`; this header claims one owner and three tables.

---

## Findings — the legacy `docs/` corpus outside `restructurization/`

### D-52 · **833** `src-tauri/src/…` path references across the legacy corpus, of which only a handful still resolve — mitigated by a P0 banner on **6 of ~55** documents, and that banner was not extended for P1 · **STALE** (corpus-level)

**Measurement.** `grep -rn "src-tauri/src/"` over `docs/development`, `docs/codebase`, `docs/notes`, `docs/reference`, `docs/endtoendrun`, `docs/autodev-vision`, `docs/submodules-management` returns **833** lines. Spot-resolution of representative paths:

| Path cited in legacy docs | Resolves today? |
|---|---|
| `src-tauri/src/logging.rs` | ✅ EXISTS (stayed host-bound) |
| `src-tauri/src/observability/log_retention.rs` | ❌ MISSING → `autodev-core/src/observability/log_retention.rs` |
| `src-tauri/src/agent/builtin_tools/edit_file.rs` | ❌ MISSING → `autodev-core/src/…` |
| `src-tauri/src/agent/context.rs` | ❌ MISSING → `autodev-core/src/…` |
| `src-tauri/src/graph/schema.rs` | ❌ MISSING → `autodev-core/src/…` |

**What already mitigates this.** A P0 sweep added a `⚠ SUPERSEDED IN PART — read this first` banner to six documents (`docs/development/ARCHITECTURE_CLARIFICATIONS.md`, `DEVELOPMENT_PLAN.md`, `IT_RUNBOOK.md`, `PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md`, `documentation-panel/AUTODEV_DOCUMENTATION.md`, `documentation-panel/COMPLETE_FEATURE_LIST.MD`). Its closing rule — *"Where this document and the code disagree, the code is right. The governing plan is `docs/restructurization/`"* — is generic and covers P1 as well as P0.

**Two gaps this sweep found:**

1. **The banner's enumeration is P0-only.** It lists the MCP server, `rmcp`, the MCP meta-arguments, the subagent bridge, delta updates, the vault `get_*` commands and Redis auto-start — all P0. It says nothing about the **182-file move into `autodev-core`**, which is what invalidates the majority of the 833 path references, nor about the data-root consolidation or the `src-shared` deletions. A reader who takes the banner as an exhaustive list of what changed will still trust `src-tauri/src/agent/…`.

2. **Three unbannered documents read as live technical reference and are now substantially wrong:**
   - **`docs/development/tools/BUILTIN_TOOLS.md`** — 12 references, **all** to `src-tauri/src/agent/builtin_tools/…` and `src-tauri/src/agent/context.rs`; every one now missing. It is the tool-implementation reference a developer would open first.
   - **`docs/reference/autodev-graph-master-spec.md`** — 48 references, all `src-tauri/src/graph/…` (`schema.rs`, `migrations.rs`, `store.rs`, `detect.rs`, `extract.rs`, `cluster.rs`); every one now missing. This is the spec P5 graph work will be read alongside `vision/05`.
   - **`docs/development/LOGGING.md:27`** — `src-tauri/src/observability/log_retention.rs::prune`; the module moved to `autodev-core/src/observability/log_retention.rs`. *(`:10`'s `src-tauri/src/logging.rs` is still correct — this file is half right, which is the worst kind of half.)*

**Severity: STALE** at corpus level. **I deliberately did not enumerate 833 rows**: the corpus is predominantly dated run analyses and point-in-time audits (`docs/endtoendrun/*`, `docs/development/runtime-fixes/*`, `docs/codebase/*_dissected.md`), which are historical records of the same class as `vision/_grounding/` and should not be rewritten. **The actual gap is that no document says so.** `vision/_grounding/` and `vision/_review/` are protected by explicit convention; the legacy corpus is protected only by six ad-hoc banners.

**Suggested fix (three parts):** (a) extend the existing six banners with a P1 clause naming the `autodev-core` move and the one data root; (b) add the same banner to `BUILTIN_TOOLS.md`, `autodev-graph-master-spec.md` and `LOGGING.md`, or repoint their paths — they are short enough to fix properly; (c) record a one-line convention in the MDP that `docs/` outside `restructurization/` is a historical corpus governed by the banner, so future sweeps have a rule instead of a judgement call.

### D-53 · `docs/development/IT_RUNBOOK.md:637` — the data-root tree still shows `~/.airs/` as live · **STALE**

Quoted (tree heading): `~/.airs/`

**What is actually true.** After `P1-14` the live data root is `~/.autodev/` alone; `~/.airs/` holds only a superseded copy of `autodev.db`, deliberately left in place pending Razi's approval to delete (MDP `:587`). The runbook's uninstall commands at `:739`, `:746` and `:753` (`rm -rf ~/.autodev ~/.airs`) remain **correct** precisely because of that deliberate non-deletion — so this file is not uniformly stale, and the fix is narrow.

**Severity: STALE.** **Suggested replacement:** relabel the `:637` tree as `~/.autodev/` and add a note: *"`~/.airs/` is a **retired** root — after `P1-14` it holds only a superseded copy of `autodev.db`. It is still removed on uninstall."* *(Its P0 banner covers the file generally; this is the one line specific enough to be worth naming.)*

### D-54 · `docs/updater-signing.md` — **CLEAN** (verified, not assumed)

Both of its code references — `src-tauri/src/updater/verify.rs` at `:6` and `:226` — still resolve: `updater/` is one of the modules that deliberately stayed host-bound in `src-tauri`. No `~/.airs`, no checkpoint, no schema, no `SENSITIVE_KEYS` claims. Recorded here because a sweep that reports only problems does not show its coverage.

### D-55 · `MASTER_DEVELOPMENT_PLAN.md §7` (P1) has no pointer to `_verify/P1-CORRECTIONS.md` · **COSMETIC**

**What is actually true.** `§6` (P0) opens with a prominent banner at `:446-448`: *"⚠ **These task lines are AMENDED by [`_verify/P0-CORRECTIONS.md`]** … Where a line below disagrees with that file, **that file wins**."* `§7` (P1) at `:523-529` has no equivalent, though `_verify/P1-CORRECTIONS.md` exists, was written pre-build under the same §0.2m rule, and is cited by `ADR-LOG-RESTRUCT-0011` (`§A`), `0013` (`§B`), `0015` (`§A`) and `0016` (`§B`).

**Severity: COSMETIC** — P1 is built and its corrections are absorbed into the task evidence lines, so nothing is misread today. It matters only as a precedent: the P2 section should carry the pointer from the start. *(Checked and clean: `P1-CORRECTIONS.md:29-36` **does** carry an accurate `⚠ PARTIALLY RETRACTED — ADR-LOG-RESTRUCT-0017` banner over its `P1-11` section, correctly scoping the false *"v5 failed to add the columns"* premise at `:48`.)*

### D-56 · `vision/09:116` (and `MASTER_DEVELOPMENT_PLAN.md:392`, `:838`) — `sidecar/` given as **3,161 LOC** · **COSMETIC**

Quoted `vision/09:116`: *"| `src-tauri/src/sidecar/` | 3,161 LOC |"* — repeated at MDP `:392` (*"`sidecar/` [P8 DELETE — 3,161 LOC]"*) and MDP `:838` (*"remove `sidecar/` (3,161 LOC)"*).

**What is actually true.** **3,297 LOC** today (`find src-tauri/src/sidecar -name "*.rs" -exec cat {} + | wc -l`). It was **3,267** at the P0 closeout commit `7c5b7a1`, and P1's `eab345c` touched the module (import rewrites for the extraction, +30 lines) — so the figure was already 106 lines stale before P1 and is now 136.

**Severity: COSMETIC.** The *path* is correct and `sidecar/` is correctly still in `src-tauri` — this is the one `src-tauri/src/…` citation in `vision/09` that survives the extraction, which is why the document is otherwise clean. Only the LOC count drifts. Worth noting that three documents carry the same stale number from one source, so it should be fixed in one pass.

---

# 3 · Category summary — which P1 change generated the most staleness

| # | P1 change | Findings | Verdict |
|---|---|---|---|
| **1** | **`autodev-core` exists; 182 files moved; shims at the old paths** | D-03, D-08, D-09, D-11, D-30, D-31, D-46, D-48, D-52, D-53 — plus **833** unenumerated legacy-corpus path references (D-52) | **Largest by volume, by a wide margin.** Inside `restructurization/` the damage is contained: the specs cite modules relatively (`agent/…`, `graph/…`), which still resolve, so only absolute `src-tauri/src/…` citations broke. Outside it, 833 references broke and only six documents carry a banner. **The single most useful fix is not a path edit — it is adding the re-export-shim line to the §4 module tree (D-08) and a P1 clause to the legacy banner (D-52), so a reader understands *why* old paths still compile but new ones are elsewhere.** |
| **3** | **`PRAGMA user_version` ladder on every database (10 of them)** | D-01, D-20, D-40, D-43a | **Highest severity.** D-01 is the most misleading line in the corpus. Three documents still say only `graph/` has a ladder, and the count appears as 10 and 11 in the *same Part* of two different documents. |
| **5** | **`db/checkpoints.rs` DELETED; undo ships via `git/checkpoint.rs`** | D-16, D-17, D-21, D-37 (×5), D-38, D-42, D-45 (×3), D-47 | **Most instances of the `FALSE` class — 13 lines.** The P1 sweep corrected four locations (`vision/00:211`, `:483`, `:1177`, `vision/07:174`) and missed thirteen more, including a P1 **exit gate** that is now unsatisfiable (D-16) and a live database inventory row for a file that never existed (D-37c). Its `vision/07` correction is also a banner that fails to correct (D-38). |
| **7** | **Graph v5 repaired, `migration_v6` added, store at v6** | D-01, D-06, D-18 (×3), D-29, D-43a, D-44 | **Most consequential mis-assignment.** Four documents still tell **P5** to do work P1 finished, and one leaves a gate clause **quarantined** (`TESTING_GATES.md:118`) with a justification that no longer exists — §1's own *"a skipped gate is not green"*. |
| **2** | **Data root consolidated to `~/.autodev/`** | D-07, D-41 (R1), D-49, D-50, D-53 | Well handled inside `restructurization/` — `vision/00:465` and `vision/08:41` were both updated in-flight. The residue is **in the code**: eleven stale doc comments and two wrong lines of `airsctl --help` (D-49, the only user-visible finding in this report). |
| **6** | **Redaction ported; `SENSITIVE_KEYS` = 22, not 20** | D-04, D-19, D-24, D-34, D-36 | **A textbook `L-C1`.** The correction was written into `TESTING_GATES.md:327` — naming *"here and in the plan"* as the wrong places — and then applied to neither the plan (`vision/00:207`) nor the `fx-secrets` corpus spec (`TESTING_GATES.md:570`) six lines later in the same document. The corpus one has teeth: a corpus built to "20" leaves two keys unexercised while `TG3-38` reports green. |
| **4** | **`src-shared` mirrors deleted; `airsctl` uses `autodev_core::db`** | D-02, D-14, D-35 (×3), D-41 (R5) | D-02 is `FALSE` and would send a reader to fix a defect P1 removed. D-35's three instances share one cause: the *conclusion* (unconstrained `TEXT` + a second process) survives, only the named mechanism died. |
| **8** | **`hourly_costs` deleted** | D-41 (R6), and `vision/08:103` | Smallest footprint — two lines, both in reconciliation tables that were not annotated. |
| **9** | **`EventSink` replaced `OnceLock<AppHandle>`; no `emit_X_global`** | **none** | **Clean.** Searched not sampled: `AppHandle`, `OnceLock` and `emit_*_global` return **zero** hits across all of `docs/restructurization/`. The specs never described the global-handle mechanism, so removing it falsified nothing. |
| **10** | **INV-1 restated as runtime-and-language-scoped** | **none** | **Clean, and done well.** `vision/00:120` carries the restatement, its rationale, the P1 evidence *and* the still-unsatisfied half; `vision/00:1151` (G-02) matches it. `READINESS_MATRIX.md:81` (SO-1) tracks the same wording. No document still carries the process-scoped phrasing. |
| **—** | **Not attributable to a P1 change** (found in passing, still live) | D-10, D-12, D-13, D-22/D-27, D-32, D-33, D-46, D-54, D-55 | D-27 is the standout: `config.db` has six tables from three openers, and four documents plus one code header claim three tables and one file owner. That was **wrong when P1 wrote it**, not made wrong afterwards. |

**Severity totals: 25 `FALSE` · 22 `STALE` · 8 `COSMETIC` = 55 findings**, plus `D-54`, which is a verified-clean record rather than a finding. Counting the multi-line findings individually (D-18 ×3, D-20 ×2, D-35 ×3, D-37 ×5, D-39 ×4, D-41 ×5, D-43 ×3, D-45 ×3, D-50 ×11, D-56 ×3, D-52's three named documents) the corpus carries **≈98 distinct stale or false lines**, plus the 833 unenumerated legacy path references.

---

# 4 · What I could not determine

1. **Whether `_verify/` audit reports are living or historical.** The brief exempts `vision/_grounding/` and `vision/_review/` and is silent on `development/_verify/`. I treated the P0-era files (`A1`, `A2`, `V1`–`V3`, `P0-AUDIT-TRIAGE`, `P0-CORRECTIONS`) as historical records of the same class, and checked only the P1-era files that a living document cites (`P1-CORRECTIONS.md` — clean, D-55). **If they are meant to be living, they are unswept.** `V5-P1CD-schema-redaction.md` alone carries 93 `src-tauri/src/` references, the highest count in the whole `restructurization/` tree.

2. **Whether the `TG-P1-nn` ↔ `TG<p>-<nn>` divergence (D-23) is deliberate.** I could find no ADR or note authorising a second gate-ID namespace, and `TESTING_GATES.md:29` forbids renumbering — but nothing states that the P1 gates were *meant* to be entries in that document rather than a separate CI-gate series. `READINESS_MATRIX.md:23` uses the `TG-P<p>` form and `TESTING_GATES.md` uses `TG<p>-`, so the two anchor documents already disagree. **This needs Razi's ruling, not a sweep's guess.**

3. **The correct disposition of §14.5's "Retention prune + `VACUUM` → P1" row (D-15).** I verified no P1 task built it. I could not determine whether the mechanic was silently dropped, deferred by a decision that went unlogged, or is considered satisfied by `db/backup.rs`'s `VACUUM INTO` (which it is not — that is a backup, not a prune, and is not on a timer). No ADR entry addresses it.

4. **Whether `~/.airs/` still exists on this machine.** MDP `:587` says it is deliberately not deleted, and `IT_RUNBOOK.md`'s uninstall commands still remove it. **I did not check the filesystem** — the brief forbids commands that write to `~/.autodev/` or `~/.airs/`, and I judged that reading a user data directory to settle a documentation question was outside a read-only doc audit's remit. D-53's recommendation holds either way.

5. **Whether `checkpoints` / `hooks` / `hook_logs` *should* join `config_db`'s ladder (D-27).** I established the fact — three openers, six tables, three covered by `TG-P1-04` — and that the file has one version owner but not one schema owner. Which way to resolve it is a shipped-schema decision under MDP §0.15 ("stop and ask"), and I am read-only.

6. **The true scale of legacy-corpus staleness beyond path references.** I measured 833 `src-tauri/src/` references and spot-resolved five. I did **not** verify the prose claims in ~55 legacy documents against current code — that is a corpus audit, not a staleness sweep, and most of it is dated run analysis that should not be rewritten. D-52 records the boundary I drew and why.

---

# 5 · Documents checked and found clean

Checked against all ten P1 changes with the same pattern set (`src-tauri/src/`, `~/.airs`, `checkpoints`, `hourly_costs`, `emit_*_global`, `OnceLock`/`AppHandle`, `src-shared`, `user_version`, `20-entry`/`SENSITIVE_KEYS`, `918 LOC`, `never compiled`, `no backup path`, `data roots`, `autodev-core`) — **no finding**:

| Document | Why it is clean |
|---|---|
| `vision/01-AGENT-RUNTIME.md` | Zero hits on every pattern. Describes the loop, `Item::Opaque` and event taxonomy — none of which P1 touched. |
| `vision/02-PROVIDERS-AND-THE-SDK-NATIVE-SURFACE.md` | Zero hits. Provider work is entirely P2. |
| `vision/09-DELIVERY.md` | Clean apart from one cosmetic count (D-56). Its single `src-tauri/src/` reference — `:116` `src-tauri/src/sidecar/` — is the one such path that **survives** the extraction: `sidecar/` deliberately stayed host-bound and is a P8 deletion. Only its LOC figure has drifted. |
| `docs/restructurization/memory-anchor.md` | Zero hits on all patterns. Adopted 2026-08-31; its subject (memory tiers, the reranker) is P3. |
| `docs/restructurization/ui-anchor.md` | Zero hits except `.airs-*`, which is the **CSS class vocabulary**, not the data root — a deliberate naming lock (`vision/00:520`), correctly untouched by `P1-14`. |
| `docs/updater-signing.md` | Verified in D-54 — both `src-tauri/src/updater/verify.rs` citations still resolve. |
| `development/_verify/P1-CORRECTIONS.md` | Its one retracted premise (`P1-11`, graph v5) carries an accurate `⚠ PARTIALLY RETRACTED — ADR-LOG-RESTRUCT-0017` banner at `:29-36` that correctly scopes the false claim at `:48`. |

**Also clean — the correction banners that were checked and hold:**

| Banner | Verdict |
|---|---|
| `MASTER_DEVELOPMENT_PLAN.md:306` (GAP-15 undo retraction) | **Accurate.** Verified: `git/checkpoint.rs` exists, **five** commands registered at `src-tauri/src/lib.rs:386-390`, `gitStore.ts` calls them. Every claim in the banner checks out. |
| `MASTER_DEVELOPMENT_PLAN.md:398-407` (§4 tree correction, `ADR-0015`) | **Accurate** on what it asserts — `db`/`graph`/`git`/`recipes`/`hooks` did have to move. Incomplete only in the same way as its source ADR (D-09/D-31). |
| `vision/00:211` (M-43 first clause retracted) | **Accurate as a retraction**; its prescription column was not updated (D-47). |
| `vision/00:491` (§4.3 superseded by P1-10) | **Accurate**, and correctly says **10** databases — it is the surrounding text that contradicts it (D-40, D-43). |
| `vision/00:1177` (G-08 undo correction) | **Accurate.** Correctly separates the retracted half from the surviving `M-43` half. |
| `vision/08:29` (§1 superseded by P1) | **Accurate** — all five claims verified. Its only defect is scope: it sits in §1 and does not reach §3 (D-43) or the `11 databases` line two above it (D-40). |
| `vision/08:41` (R1 resolved by P1-14) | **Accurate**, including the subtle half — `~/.airs/plugins/` and `fleet_secret` never existed on disk (`db/legacy_root.rs:4-7`). This is the model the other reconciliation rows should follow (D-41). |
| `vision/08:101` (two checkpoint systems resolved by P1-16) | **Accurate**, and independently confirmed: zero references to `checkpoints.db` or `file_checkpoints` anywhere in the four crates. |
| `TESTING_GATES.md:111` (`TG1-01` clause corrected) | **Accurate**, and the most careful banner in the corpus — it verifies that 27 structs implement no `Tool`, which I re-measured (`grep -c "impl Tool for"` = 0) and confirmed. It is let down only by `:104`/`:105`, which repeat the corrected claim four lines above it (D-16, D-17). |
| `READINESS_MATRIX.md:83` (SO-3 note corrected) | **Accurate** — `tauri.conf.json:46` reads `"resources": []`, exactly as claimed. |
| `READINESS_MATRIX.md:96` (SO-16 note corrected) | **Accurate** — all five claims verified against `git/checkpoint.rs` and `lib.rs`. |
| `READINESS_MATRIX.md:81` (SO-1, INV-1) | **Accurate**, including the "not yet satisfied" half — `src-tauri/src/main.rs:10-12`, `:128-130` still construct `SidecarManager` and the orchestrator client. |

**Status: COMPLETE.** 55 findings, written incrementally as each was verified against code.
