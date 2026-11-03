# Adversarial refutation — P3-F · Memory claims vs. shipped Rust
Repo: `C:\AIRS Foundry\autodev\autodev` · read-only pass · 2026-09-02

Method note: the seven-defect list traces to
`docs/restructurization/vision/_grounding/G4c-memory-tiers-port-spec.md` §5.3–5.4,
which is a **line-by-line spec of the deleted Python sidecar** (it cites `procedural.py:278-290`,
`context_service` `:1204-1211`, `yaml:38`). `03-CONTEXT-AND-MEMORY.md:127` restates it as
"`G4c §5.4` — each of these is **in the code being ported**". The MDP's `P3-43`
(`MASTER_DEVELOPMENT_PLAN.md:1069`) inherits that framing.

---

## A-0 · "Is there a cache in the Rust code at all?" · **NO — REFUTED**

**Claim (implied by defects 2, 6, 7):** the code being ported has an episodic cache with a
key, a negative cache with `strict_lossless` gating, and a semantic cache with a ~30-minute TTL.

**Reality:** the string `cache` does not occur **anywhere** in `autodev-core/src/memory/`:

```
$ grep -rni "cache" autodev-core/src/memory/
(no output — 0 matches across all 9 files, 1696 lines)
```

Repo-wide ripgrep for `strict_lossless|strictLossless|negative_cache|negative cache`
returns hits in **documentation only** — `03-CONTEXT-AND-MEMORY.md:147`,
`G4c-memory-tiers-port-spec.md:161,349,531,532,583`, `NOVELTY_LEDGER.md:87`,
`MASTER_DEVELOPMENT_PLAN.md:1069`, `TESTING_GATES.md:258`. **Zero `.rs`, `.ts` or `.tsx` hits.**

Every read path in the Rust memory module opens the database on every call —
`MemoryDb::open()?` appears at `semantic.rs:43,109,120,154,169,184,199`,
`episodic.rs:36,87,129,148,163,179,187,201`, `procedural.rs:138,167,178,207`,
`recall.rs:145`. There is no memoisation layer of any kind, no TTL, no LRU, no Redis.

**Verdict rationale:** three of the seven defects (2, 6, 7) presuppose a cache subsystem
that does not exist in the shipped Rust. They cannot be "fixed rather than ported" because
there is nothing to port them from and nothing to fix.

**Consequence for the plan:** `P3-43` must be re-scoped. As written it commits `UNIT-PROVEN`
evidence for three defects that cannot be reproduced as unit failures against today's tree.
`TESTING_GATES.md:258` (`TG-…` for `G-MEM-QUERY`) explicitly promises to catch
"an episodic cache key of `episodic:{user}:{session}`" and "a negative cache gated behind
`if not strict_lossless`" — **a gate that can only ever pass vacuously**, because neither
construct exists. That is precisely the "inert-but-green" failure mode.

---

## A-1 · Semantic recall drops its `query` · **PARTIALLY CONFIRMED (right symptom, wrong mechanism)**

**Claim:** "Semantic recall drops its `query` — the vector query is a fixed template string,
despite a docstring saying otherwise" → "Recall is query-independent; identical for every
turn until the cache expires" (`03-CONTEXT-AND-MEMORY.md:143`).

**Reality — the symptom is real.** `autodev-core/src/memory/recall.rs:31-35`:

```rust
pub fn build_memory_context(session_id: &str, query: &str, _working_dir: &str) -> Option<String> {
    let summaries =
        EpisodicStore::get_recent_summaries(session_id, EPISODIC_LIMIT).unwrap_or_default();
    let facts = SemanticStore::recall_facts(SEMANTIC_LIMIT).unwrap_or_default();
    let procs = ProceduralStore::search(query, PROCEDURAL_LIMIT).unwrap_or_default();
```

`query` is forwarded to the **procedural** tier only. The semantic tier calls
`recall_facts(limit)`, whose signature takes no query at all (`semantic.rs:153`):

```rust
pub fn recall_facts(limit: u32) -> Result<Vec<SemanticFact>> {
```

and whose SQL is confidence-ordered, not relevance-ordered (`semantic.rs:155-163`):

```sql
FROM   semantic_memories
WHERE  is_active = 1
  AND  confidence >= 0.7
ORDER  BY confidence DESC, updated_at DESC
LIMIT  ?1
```

**But every stated mechanism is wrong.** (a) There is **no vector query** — no embedding,
no `sqlite-vec`, no template string; retrieval is plain SQL. (b) There is **no contradicting
docstring** — `recall.rs:7` states the actual behaviour honestly: *"Semantic — 10
highest-confidence facts"*, and only the procedural line says *"keyword match on query"*
(`recall.rs:8`). (c) "until the cache expires" is false — see **A-0**; it is
query-independent on *every* turn, forever, with no expiry.

Note also that a query-aware semantic search **does** exist and is used elsewhere:
`SemanticStore::search(query, limit)` (`semantic.rs:183`) with `LOWER(fact_text) LIKE ?1`,
called from `recall::search_all` (`recall.rs:105`). The prompt-injection path simply
does not use it.

**Verdict rationale:** the *consequence* (query-independent semantic recall in the injected
memory block) is confirmed and is arguably worse than described. The *defect description* is
false in three particulars because it describes Python that no longer exists.

**Consequence for the plan:** `P3-43`'s first bullet should be restated as
"`build_memory_context` passes `query` to the procedural tier only; semantic recall is
confidence-ordered and episodic recall is recency-ordered" — with `recall.rs:31-35` as the
citation. The words "vector query", "fixed template string" and "docstring saying otherwise"
must go, or the fix will be written against a mechanism nobody can find.

---

## A-2 · Episodic cache key ignores the query · **REFUTED (vacuous)**

**Claim:** "Episodic cache key ignores the query (`episodic:{user}:{session}`), with a comment
claiming it hashes the query" → "Two different questions in one session share a cache entry"
(`03-CONTEXT-AND-MEMORY.md:144`).

**Reality:** there is no cache and therefore no cache key. See **A-0**. Grep for `episodic:`
as a key literal, for any hashing of a query, and for `cache` across
`autodev-core/src/memory/` returns nothing. Episodic reads go straight to SQLite on every
call (`episodic.rs:162-175`).

**Verdict rationale:** the named artefact does not exist in the shipped code.

**Consequence for the plan:** ⚠ **The underlying risk survives the refutation, by a different
route.** `build_memory_context` never passes `query` to the episodic tier either —
`EpisodicStore::get_recent_summaries(exclude_session, limit)` (`episodic.rs:162`) is ordered
`created_at DESC` with no query term. So *"two different questions in one session"* still
receive **identical** episodic content — not because a cache key collides, but because
nothing is query-dependent to begin with. This is exactly what smoke `S3-04`
(`MASTER_DEVELOPMENT_PLAN.md:1105`, *"Two different questions in one session produce
different recall sets"*) would catch. `P3-43` bullet 2 should be rewritten to name the real
cause; `TESTING_GATES.md:258`'s promise to catch "an episodic cache key of
`episodic:{user}:{session}`" must be replaced or it will pass without testing anything.

---

## A-3 · `learn_hitl_pattern` always throws · **REFUTED (does not exist)**

**Claim:** "`learn_hitl_pattern` always throws — it passes a `metadata` argument the callee
does not accept; the `TypeError` is swallowed" → "No record with `subtype == "hitl_interaction"`
has ever been written, and its reader can only ever return `[]`"
(`03-CONTEXT-AND-MEMORY.md:145`).

**Reality:** repo-wide ripgrep for `hitl_interaction|learn_hitl_pattern|hitl_pattern` returns
**documentation only**:

```
docs/restructurization/vision/03-CONTEXT-AND-MEMORY.md:144
docs/restructurization/vision/_grounding/G4c-memory-tiers-port-spec.md:293,315,357,537,585
```

No `.rs` file mentions HITL patterns. There is no `subtype` column anywhere in `memory.db`'s
schema (`db.rs:40-123` — the only near-neighbour is `summary_type TEXT` at `db.rs:61`, whose
domain is `NULL | 'session_summary'`, `episodic.rs:137`). `procedural_memories` has columns
`id, topic, title, content, tags, created_at, updated_at` (`db.rs:109-117`) — no subtype, no
tool-success records at all. `G4c:537` cites `procedural.py:278-290` — a Python file that no
longer exists in this repo.

**Verdict rationale:** nothing to fix; the function, the subtype, its writer and its reader are
all absent from the Rust tree.

**Consequence for the plan:** `P3-43` bullet 3 ("always-throwing HITL writer") is not a *fix*
but a **new feature** if HITL-pattern learning is wanted at all. `TESTING_GATES.md:264`
(`G-MEM-SUBTYPES`) is written as a regression gate over a defect that cannot regress; it
needs restating as a positive requirement or striking. This is a scope question for the owner,
not a build decision.

---

## A-4 · Unfiltered procedural recall + unused circuit breaker · **PARTIALLY CONFIRMED / OVERSTATED**

**Claim:** "Procedural recall has no type filter and its cold fetch has no circuit breaker —
the constructed breaker is never used" → "Cross-tier bleed by construction"
(`03-CONTEXT-AND-MEMORY.md:146`).

**Reality, first half — no type filter: CONFIRMED, trivially.** `procedural.rs:177-186`:

```rust
pub fn search(query: &str, limit: u32) -> Result<Vec<ProceduralMemory>> {
    ...
    "SELECT id, topic, title, content, tags, created_at, updated_at
     FROM   procedural_memories
     WHERE  LOWER(content) LIKE ?1 OR LOWER(title) LIKE ?1 OR LOWER(tags) LIKE ?1
     ORDER  BY updated_at DESC
     LIMIT  ?2",
```

There is no type/subtype/kind predicate — and there is no column to filter on
(`db.rs:109-117`). `recall::search_all` calls it with no filter either (`recall.rs:119`).

**Reality, second half — circuit breaker: REFUTED.** No breaker is *constructed* in the memory
module, so none can be "never used". The only circuit breakers in the tree are the Budget
Circuit Breaker (`autodev-core/src/budget/mod.rs:2`, `agent/engine.rs:64,378`) and the sidecar
watchdog (`src-tauri/src/sidecar/mod.rs:1044-1148`) — both unrelated to memory.

**"Cross-tier bleed by construction" is OVERSTATED against this code.** The Rust tiers are
three separate tables queried by three separate functions; a procedural search cannot return a
semantic or episodic row. The real weakness is different and narrower: `LIKE '%query%'` over
the *whole* `content` column with no scoping means **every** stored topic is a candidate
regardless of project, and `ProceduralStore::search` is the *only* place `query` reaches the
injected prompt at all (**A-1**).

**Verdict rationale:** half the defect is real but for a structural reason (the column does not
exist), half names an artefact that does not exist.

**Consequence for the plan:** bullet 4 should be split. "No type filter" is a **schema** task
(belongs with `P3-31`/`P3-38`, which add columns), not a recall-logic fix. The circuit-breaker
clause should be struck or reframed as a new requirement.

---

## A-5 · Episodic recall never forwards `agent_id` · **REFUTED (no such parameter exists)**

**Claim:** "Episodic recall never forwards `agent_id` despite the callee supporting it as
documented 'defence in depth'" → "Recall is unscoped by agent — a cross-agent leakage risk
with a defence that is present and unused" (`03-CONTEXT-AND-MEMORY.md:147`).

**Reality:** `grep -rn "agent_id" autodev-core/src/memory/` returns **zero matches**. No
callee supports it; there is no unused defence. The `episodic_memories` table has no
`agent_id` column (`db.rs:48-64`) — its scoping columns are `session_id` and `working_dir`
only.

**The leakage exposure is nonetheless real and is broader than the claim.**
`get_recent_summaries` (`episodic.rs:162-175`) filters on `session_id != ?1` and
`summary_type = 'session_summary'` — nothing else:

```sql
WHERE  session_id  != ?1
  AND  summary_type = 'session_summary'
ORDER  BY created_at DESC
```

`working_dir` is stored (`db.rs:60`) but **never used as a predicate in any read**. And
`build_memory_context` takes `_working_dir` with a leading underscore — deliberately unused
(`recall.rs:31`). So every session's summaries are visible to every other session, across
every project on the machine, with no filter available to prevent it. Semantic recall is the
same: `recall_facts` (`semantic.rs:153-165`) has no `working_dir` predicate either.

**Verdict rationale:** the stated defect (an unused parameter) is false. The stated
consequence (unscoped recall, cross-context leakage) is confirmed, worse, and by a mechanism
the plan does not name — a missing column, not a dropped argument.

**Consequence for the plan:** bullet 5 must be rewritten as "recall is unscoped by
project/agent because no scoping column exists and `working_dir` is written but never read".
That makes it a **dependency on `P3-31`** (scope as a first-class column), not an independent
fix — which changes the ordering inside P3-F.

---

## A-6 · The negative cache never fires · **REFUTED (vacuous)**

**Claim:** "The negative cache never fires — every tier gates its return on
`if not strict_lossless`, and the config sets `strict_lossless: true`"
(`03-CONTEXT-AND-MEMORY.md:147`).

**Reality:** no cache (**A-0**), no `strict_lossless` symbol in any source file, no config key.
The only occurrences are the docs listed in A-0. `G4c:531-532` names the exact Python line
numbers (`:886-891`, `:1055-1059`, `:1152-1156`, `yaml:38`) — none of which exist here.

Note `G4c:532` itself ends *"Decide deliberately in Rust: keep it inert, or invert the gate."*
The Rust decision has effectively already been made by omission: there is no negative cache.

**Verdict rationale:** vacuous — the artefact is absent.

**Consequence for the plan:** bullet 6 is a **design choice for a component that does not yet
exist**, not a defect fix. It should move out of `P3-43` and into the retrieval-backend task
(`P3-30`) as an explicit yes/no, or be struck.

---

## A-7 · Semantic cache is not invalidated on write · **REFUTED (vacuous)**

**Claim:** "Semantic cache is not invalidated on write" → "A newly extracted fact is invisible
for up to 30 minutes" (`03-CONTEXT-AND-MEMORY.md:148`).

**Reality:** no cache (**A-0**). `SemanticStore::store_fact` (`semantic.rs:30-105`) writes
directly to SQLite and `recall_facts` (`semantic.rs:153`) reads directly from SQLite on the
next call — a fact stored by `remember_this` is visible to the very next
`build_memory_context` with **zero** delay. The claimed 30-minute invisibility window does not
exist.

**Verdict rationale:** vacuous, and the shipped behaviour is the *opposite* of the claim.

**Consequence for the plan:** bullet 7 must be struck from `P3-43`, and
`TESTING_GATES.md:258`'s clause "a semantic cache never invalidated on write" with it — a gate
asserting freshness would pass today against no implementation, which is the definition of an
inert green row.

---

## A · Summary of the seven

| # | Defect as written | Verdict in shipped Rust |
|---|---|---|
| 1 | Semantic recall drops `query` (vector template, lying docstring) | **PARTIALLY CONFIRMED** — query-independence real; no vector, no lying docstring, no cache expiry |
| 2 | Episodic cache key ignores query | **REFUTED (vacuous)** — no cache; but episodic recall *is* query-independent anyway |
| 3 | `learn_hitl_pattern` always throws | **REFUTED** — function, subtype, writer and reader all absent |
| 4 | Procedural recall unfiltered + unused breaker | **PARTIALLY CONFIRMED / OVERSTATED** — no filter (no column to filter on); no breaker exists; "cross-tier bleed" false |
| 5 | Episodic recall never forwards `agent_id` | **REFUTED** — no such parameter; but recall *is* unscoped, via a missing column |
| 6 | Negative cache never fires | **REFUTED (vacuous)** |
| 7 | Semantic cache not invalidated on write | **REFUTED (vacuous)** — writes are immediately visible |

**Net: 0 of 7 exist as described. 2 of 7 have a real symptom by a different mechanism.
1 of 7 is half-real. 4 of 7 are entirely absent.** The claim "each of these is in the code
being ported" (`03-CONTEXT-AND-MEMORY.md:127`) is **false for the code that actually ships**;
it was true of the deleted Python sidecar.

**Two genuine defects the seven-item list does NOT name** (found while checking it):

- **G-a · Recall is unscoped by project.** `working_dir` is written by all three tiers
  (`db.rs:60,83`) and read as a predicate by **none**. `build_memory_context`'s parameter is
  `_working_dir` (`recall.rs:31`).
- **G-b · Fact extraction is a substring scan that keeps everything after the match.**
  `semantic.rs:236-276`: eight preference triggers including bare `"i like "` and `"i use "`,
  and the excerpt taken is `content[pos..]` to end-of-line, stored at confidence 0.75. Any
  message containing "I use tabs" is silently promoted into permanent injected prompt state.
  This is the fact **source** for everything `build_memory_context` injects.

---
## B · Scope and provenance become "first-class columns" (P3-31, P3-38) · **CONFIRMED as future work — no scope column exists; provenance exists PARTIALLY**

**Claim:** *"**Scopes** (ADR-05-03) become first-class columns and a **mandatory retrieval
filter**: `global` (user / agent) · `project` · `task`"* (`03-CONTEXT-AND-MEMORY.md:185`).
`P3-31` — *"Scope as a declared `vec0` partition key"* (`MASTER_DEVELOPMENT_PLAN.md:1057`).
`P3-38` — *"Provenance column (`source` + file/line)"* (`:1064`).

**Reality — scope: absent, completely.**

```
$ grep -rniE "scope|provenance|source_file|source_line" autodev-core/src/memory/
No matches found
```

Not one of the nine files in the memory module contains the string `scope` in any casing.
There is no `global`/`project`/`task` enum, no partition key, no filter.

**Reality — provenance: half-present, and older than the plan admits.**
`semantic_memories` already carries two provenance-ish columns (`db.rs:81-83`):

```sql
source_type    TEXT NOT NULL DEFAULT 'extracted',
source_session TEXT,
working_dir    TEXT,
```

`source_type` is written with the literals `"extracted"` (`semantic.rs:224`) and
`"user_explicit"` (`remember_this.rs:102`). There is **no file/line provenance** anywhere, and
`episodic_memories` / `procedural_memories` have no `source_*` columns at all.
Critically, **`source_type` is never read as a predicate or surfaced in recall** —
`recall_facts` (`semantic.rs:155-163`) selects it into the struct and `build_memory_context`
(`recall.rs:51-61`) renders only `confidence` and `fact_text`. So the provenance that exists
is write-only.

**The actual `CREATE TABLE` set — all of it, one rung, one file.**
`autodev-core/src/memory/db.rs:40-123`, four tables:

| Table | Columns (`db.rs` lines) |
|---|---|
| `episodic_memories` | `id, session_id, turn_index, role, content, model, cost_usd, tokens_in, tokens_out, thinking, tool_calls, working_dir, summary_type, metadata, created_at` (`:48-64`) |
| `semantic_memories` | `id, fact_text, category, confidence, source_type, source_session, working_dir, tags, is_active, created_at, updated_at` (`:76-88`) |
| `fact_history` | `id, fact_id, old_text, new_text, changed_at` (`:96-102`) |
| `procedural_memories` | `id, topic, title, content, tags, created_at, updated_at` (`:109-117`) |

Plus seven indexes (`:65-71, :89-93, :118-121`). No virtual tables, no `vec0`, no FTS5.

**Reality — migration ladder: yes, and its head is `1`.**
`autodev-core/src/db/migrate.rs` provides a shared numbered ladder (`Migration { version, name,
sql }`, `run_ladder`, backup-before-destructive, stamp-and-work-in-one-transaction).
`memory.db`'s ladder has exactly one rung (`db.rs:40-42`):

```rust
const LADDER: &[Migration] = &[Migration {
    version: 1,
    name: "memory-baseline",
```

`MemoryDb::schema_version()` (`db.rs:23-25`) reads it back through the application handle.
So **current head version = 1**, and every P3-F schema change is rung 2 onwards.

⚠ **One caveat that bears on `P3-31`/`P3-38` directly.** The file header at `db.rs:3-4` says:

> *"Migration is purely additive via CREATE TABLE IF NOT EXISTS so new columns can be appended
> in later schema revisions without breaking old installs."*

That sentence is **false and is the exact defect `migrate.rs:1-12` was written to prevent** —
*"`CREATE TABLE IF NOT EXISTS` in an `open()` path is not a migration: it creates a table that
is absent and does nothing at all to a table that exists but is missing a column. That is not a
theoretical distinction. It is exactly how `airsctl` broke."* The ladder underneath is correct;
the comment above it instructs the next author to do the thing that broke `airsctl`. Anyone
adding a scope column by editing rung 1's SQL (as the comment invites) will ship a no-op on
every existing install.

**Verdict rationale:** the plan's *intent* is unrefuted — these genuinely are new columns. But
the plan reads as though it is upgrading something; it is building from zero for scope, and
extending a write-only half-implementation for provenance.

**Consequence for the plan:**
- `P3-31`'s evidence class is `GATE-PROVEN`; there is no existing behaviour for a gate to
  regress against, so the gate must be written as a positive assertion over new schema.
- `P3-31` says *"a declared `vec0` partition key"*. **`sqlite-vec` is not a dependency** —
  `autodev-core/Cargo.toml:27` has only `rusqlite = { version = "0.31", features = ["bundled"] }`.
  There is no `vec0` table to declare a partition key on. `P3-31` therefore **hard-depends on
  `P3-30`** and cannot be scheduled independently.
- `P3-38` should say "extend `source_type` to a real provenance triple and **read it**",
  not "provenance column", or the task will be marked done by adding a column nothing consumes
  — the same write-only state it is already in.
- `db.rs:3-4`'s header comment must be corrected in the same change (no-stale).

---
## C · The gateway (P3-39) · **`propose_memory` does not exist · deleting `remember_this` breaks 6 live Rust/TS sites + 3 doc surfaces**

**Claim:** *"The **gateway**: agents get `search`/`get`/`list`/`propose`; **`remember_this` is
removed**"* (`MASTER_DEVELOPMENT_PLAN.md:1065`). Spec form: *"agents get `search_memory`,
`get_memory`, `list_memories` and **`propose_memory`**"* (`03-CONTEXT-AND-MEMORY.md:191`).

### C.1 Which of the four exist today

| Gateway tool | Exists as an agent tool? | Evidence |
|---|---|---|
| `search_memory` | **YES** | `autodev-core/src/agent/builtin_tools/search_memory.rs`, `SearchMemoryTool`, `effect() = Read` (`:17-19`) |
| `get_memory` | **NO** | no Rust/TS match. Only `src-tauri/resources/sidecars/core-orchestrator/autodev_mcp_config.yaml:98` and `mcp_config.yaml:119` — **Python sidecar config, not an agent tool** |
| `list_memories` | **NO** | same — `autodev_mcp_config.yaml:101`, `mcp_config.yaml:122`, and the sidecar's `.github/workflows/ci.yml:220` |
| `propose_memory` | **NO — zero occurrences anywhere in the repo**, source or docs | `grep -rn "propose_memory|proposeMemory"` over `src autodev-core src-tauri src-cli src-shared e2e scripts` → no matches |

`P3-39` is therefore **1 of 4 present**, and the two that "exist" do so only inside the Python
sidecar that `P8` deletes — they are not a starting point, they are a thing being removed.
**`propose_memory` is entirely unwritten**, so `P3-39` is a from-scratch build of the write
path, not a migration.

### C.2 What EXACTLY breaks if `remember_this` is deleted — complete enumeration

**Compile-breaking (Rust, 4 sites):**

1. `autodev-core/src/agent/builtin_tools/mod.rs:37` — `pub mod remember_this;`
2. `autodev-core/src/agent/builtin_tools/mod.rs:228` — `registry.register(Arc::new(remember_this::RememberThisTool));` — **`PAMode::FastDevelopment | PAMode::Review`**
3. `autodev-core/src/agent/builtin_tools/mod.rs:266` — same, **`PAMode::Delegate`**, under the comment *"Memory tools available in Delegate mode — read-only and write facts"*
4. `autodev-core/src/agent/builtin_tools/remember_this.rs` — the file itself, 125 lines

**Behaviour-breaking — it is the last agent-facing writer.** Deleting it removes the **only**
tool by which an agent can write semantic memory. The remaining writers are both non-agent:
the Tauri command `store_fact_cmd` (`src-tauri/src/commands/memory_commands.rs:42`, driven by
the Memory sidebar) and the rule-based auto-extractor `SemanticStore::extract_and_store`
(`semantic.rs:218`). This is **`R2 X-03`** verbatim —
`00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:1189`: *"With `remember_this` removed, this decides
whether anything writes memory at all early on."* If `P3-39` lands before `P3-40`, agents have
no memory write path at all.

**Stale-comment sites (2 — compile fine, read false):**

5. `autodev-core/src/memory/mod.rs:10` — *"Rule-based extraction + explicit `remember_this` tool."*
6. `autodev-core/src/memory/semantic.rs:11` — *"2. Explicit:   `remember_this` builtin tool writes directly"*

**User-visible string generated at runtime (1):**

7. `autodev-core/src/memory/procedural.rs:267` — `default_memory_md()` writes this into the
   user's `~/.autodev/memory/MEMORY.md` on first read:
   *"(No topics yet — use `remember_this` or the Memory sidebar to add learnings)"*

**Frontend, `src/` (2 live UI strings):**

8. `src/components/memory/MemorySidebar.tsx:110` — renders the literal `remember_this` in a `<code>` tag
9. `src/components/memory/MemorySidebar.tsx:246` — *"No patterns stored yet. Use the `remember_this` tool or create a topic above."*

⚠ **Name collision to guard.** `src/stores/memoryStore.ts:216` calls
`invoke<RecallResult[]>('search_memory', …)` — that is the **Tauri command**
(`memory_commands.rs:137`), a different surface that happens to share the tool's name.
`P3-39` does not distinguish tool-name from command-name; a careless rename sweep breaks the
Memory sidebar.

**Recipes and system prompts: NO references — and the absence is itself the finding.**
`grep -rn "remember_this|search_memory" autodev-core/src/recipes/` → **no matches**. Neither
name appears in `BASE_SYSTEM_PROMPT`, `REVIEW_SYSTEM_PROMPT` or `DELEGATE_SYSTEM_PROMPT`
(`autodev-core/src/agent/context.rs`). Recipes carry no per-recipe tool allowlist naming it;
they inherit whatever `register_for_mode(PAMode::FastDevelopment)` grants
(`recipes/runner.rs:830`). So removal **silently narrows every recipe worker's tool set**, with
nothing in any recipe file to update and nothing to notice the change.

**Documentation surfaces that go stale (load-bearing ones):**
`docs/development/documentation-panel/COMPLETE_FEATURE_LIST.MD:257,498` ·
`docs/development/DEVELOPMENT_PLAN.md:4562,4593` ·
`docs/autodev-vision/autodev-master-plan.md:1310` ·
`src/data/docsMore.ts:187` — the in-app docs panel, **already wrong today**: it describes
memory as *"via mem0"* on *"port 3001"* with tools `add_memory, search_memory, get_memories`,
none of which is the shipped Rust implementation.

### C.3 A live contradiction inside the gate set

`TESTING_GATES.md:336` (`G-UTF8-TRUNC`) still lists `remember_this.rs:94` as a site it must
prove char-safe, while `TESTING_GATES.md:288` (`TG3-21`) asserts `remember_this` **no longer
exists in the tool registry**. `R2-contradictions.md:194` logged this as **X-12**; it is still
open in the gate file. Both gates cannot pass.

**Verdict:** **PARTIALLY CONFIRMED — the plan materially understates the work.** The removal
half is mechanically small (6 Rust/TS edits) but deletes the only agent write path; the
"agents get search/get/list/propose" half is **75% unwritten**.

**Consequence for the plan:** `P3-39` must (a) be sequenced **after or with `P3-40`**, never
before, or memory has no writer; (b) describe `get_memory`/`list_memories`/`propose_memory` as
**new** tools rather than an inherited set — the yaml hits are sidecar config; (c) carry the
9-site removal checklist above, including the two `MemorySidebar.tsx` strings and the
generated `MEMORY.md` line, which no task currently mentions; (d) resolve **X-12** against
`TESTING_GATES.md:336` in the same change.

---

## D · Memory is provably read (P3-44) · **the trace is real end-to-end — and it carries a hard-coded empty query**

**Claim:** *"Memory is **provably read** — a recalled memory appears in the outbound prompt
bytes (INV-8)"* (`MASTER_DEVELOPMENT_PLAN.md:1070`).

### D.1 The real path, table to wire

```
semantic_memories / episodic_memories / procedural_memories   (~/.autodev/memory.db)
  |- SemanticStore::recall_facts            semantic.rs:153
     EpisodicStore::get_recent_summaries    episodic.rs:162
     ProceduralStore::search                procedural.rs:177
        |- memory::build_memory_context     recall.rs:31    -> Option<String>  "## Memory ..."
             |- ContextBuilder::build_system_prompt_parts   context.rs:133-139  -> `volatile`
                  |- AgentEngine::run       engine.rs:210-213 -> (stable_prompt, volatile_prompt)
                       |- engine.rs:217-222   system_prompt = "{stable}\n\n---\n\n{volatile}"
                            |- provider request  (stable half separately hashed into
                               cache::prefix::build, engine.rs:233-247)
```

Entry point: `autodev-core/src/agent/context.rs:133-139`. The path is genuinely wired — this
is **not** an inert subsystem, and `P3-44` has something real to prove.

### D.2 ⚠ Recall is query-independent by a hard-coded literal

`autodev-core/src/agent/context.rs:138`:

```rust
        let volatile = self
            .session_id
            .as_ref()
            .and_then(|sid| crate::memory::build_memory_context(sid, "", repo_path))
            .unwrap_or_default();
```

**The `query` argument is the empty string literal `""`.** There is exactly one call site of
`build_memory_context` in the entire tree, and it passes `""`.

Followed through: `recall.rs:35` calls `ProceduralStore::search("", 3)`; at `procedural.rs:179`
that builds `let pattern = format!("%{}%", query.to_lowercase())` → `"%%"`, so the predicate
`LOWER(content) LIKE '%%'` (`procedural.rs:183`) is **true for every row**, ordered
`updated_at DESC LIMIT 3`. The procedural tier returns the three most recently touched topics
regardless of what the user asked.

Combined with **A-1** (semantic tier takes no query by signature) and **A-2** (episodic tier
takes no query by signature):

> **All three tiers of the injected memory block are entirely independent of the user's
> query. The block is byte-identical on every turn of every session until something writes to
> memory.** The parameter named `query` on `build_memory_context` is effectively dead — no
> caller ever supplies one.

`_working_dir` is unused too (`recall.rs:31`), so the block is also independent of the project:
`repo_path` is passed in and discarded.

### D.3 Is there anything today that proves it, or disproves it?

**Nothing proves it.** No test asserts that a stored fact reaches `system_prompt`. The nearest
existing guard is `src-tauri/src/ci_gate.rs:3322-3339` (**D-12**), which asserts only that
`engine.rs` *contains the string* `"build_system_prompt_parts"` — a grep-for-a-name check that
would pass over a function returning `("", "")`. Structural, not a read-proof. `S3-04` and
`S3-05` (`MASTER_DEVELOPMENT_PLAN.md:1105-1106`) are the intended proofs and both remain
unchecked, explicitly deferred to `P3-F`.

**Something disproves a large part of it today.** `S3-04` is *"Two different questions in one
session produce **different** recall sets."* Given `context.rs:138`, that smoke **fails right
now, deterministically, by construction** — and it cannot be made to pass by any change inside
`memory/`. The literal `""` at the call site has to change first.

### D.4 ⚠ Two paths where memory is never read at all — and the reason on record is false

`engine.rs:210-213`:

```rust
        let (stable_prompt, volatile_prompt) = match system_prompt_override {
            Some(override_str) => (override_str.to_string(), String::new()),
            None => context_builder.build_system_prompt_parts(self.pa_mode, working_dir),
        };
```

When a `system_prompt_override` is supplied, `build_system_prompt_parts` is **never called**
and `volatile_prompt` is empty — no memory block at all.

- **Recipe/fleet workers always take this branch** — `recipes/runner.rs:843-849` passes
  `Some(&recipe.instructions)`.
- **Subagents take it whenever they carry a prompt** — `subagent/native_runner.rs:318-328`.

⚠ **`remember_this.rs:37-39` states the wrong cause.** Its description tells the model:

> *"Stored facts are injected only into sessions that carry a session id. Recipe-mode runs,
> subagents and the MCP server have none, so a fact stored there is persisted but will NOT
> appear in those prompts."*

Recipe runs **do** carry a session id — `recipes/runner.rs:245`:
`let session_id = format!("recipe-{}-{}", worker_slug, Uuid::new_v4());`, passed to
`engine.run` at `:844`; and `ContextBuilder::new(working_dir).with_session(session_id)`
(`engine.rs:193`) is unconditional, so `self.session_id` is **always** `Some`. The outcome the
description predicts is right; the mechanism it names is false. Anyone who "fixes" this by
attaching a session id to recipes will change nothing.

**Verdict:** **CONFIRMED as necessary, and the task text understates it.** `GATE-PROVEN` is
the right instrument and there is a real path to gate. But `P3-44` as written reads as
verification over working recall, and it is not: a gate asserting only *"a recalled memory
appears in the outbound prompt bytes"* **would pass today** — facts do reach `system_prompt` —
while recall stays totally query-independent and absent from every fleet run.

**Consequence for the plan:**
- `P3-44` must assert **query-dependence**, not presence: the same session, two different
  questions, two different blocks (i.e. absorb `S3-04`). Presence alone is an inert-but-green
  gate that today's code already satisfies.
- Add an explicit task for `context.rs:138` — thread the user's message into
  `build_memory_context`. It appears in **no** current task and is the single
  highest-leverage line in the subsystem.
- Add an explicit decision for the `system_prompt_override` branch (`engine.rs:210-213`):
  either memory is appended to overrides too, or `P3-44`/`P8-03` must state that fleet runs
  are out of scope. Today it is neither stated nor tested.
- Correct `remember_this.rs:37-39`'s stated cause in the same change, or delete it with the
  tool under `P3-39`.

---
## E · Sequencing · **the declared dependency is satisfied; six UNDECLARED ones are not — and the fleet is coupled to memory in a way no task names**

### E.1 The declared dependency holds

`MASTER_DEVELOPMENT_PLAN.md:603` — `| **P3-F** | R1 | Memory works and is **proven read** | P3 A–E |`.

Checkbox tally over the MDP's P3 sections:

| Section | done | open |
|---|---|---|
| `P3-A` Fault isolation | 4 | 0 |
| `P3-B` The loop | 6 | 0 |
| `P3-C` Tools and permissions | 18 | 0 |
| `P3-D` Subagents | 2 | 0 |
| `P3-E` Context strategy | 9 | 0 |
| **`P3-F` Memory** | **0** | **15** |

`grep "^- \[ \] \*\*P3-(0[0-9]|1[0-9]|2[0-9])\*\*"` returns nothing. **`P3 A–E` is complete —
the stated precondition is met.**

### E.2 Six undeclared dependencies on work that is NOT done

1. **`P3-31` → `P3-30`.** `P3-31` requires *"a declared `vec0` partition key"*. There is no
   `vec0` table because **`sqlite-vec` is not a dependency**: `autodev-core/Cargo.toml:27` has
   only `rusqlite = { version = "0.31", features = ["bundled"] }`. FTS5 exists in the tree but
   only for the code graph (`graph/schema.rs:143`, `graph/store.rs:753`), never for memory.
   `P3-31` cannot start before `P3-30` lands the backend.
2. **`P3-33` has nothing to pin yet.** *"`ort`'s default features download it from a
   third-party CDN at build time"* — **`ort` is not in any Cargo.toml.** Its evidence class is
   `CODE-PRESENT`, which for an absent crate can be satisfied by adding the crate and the pin
   in one step; the task reads as remediation of an existing hazard and is actually
   *"introduce this dependency correctly the first time"*.
3. **`P3-34` likewise.** `fastembed` is not a dependency; there is no cache path to relocate.
4. **`P3-39` → `P3-40`, and the plan says the opposite by ordering.** `P3-39` deletes the only
   agent memory writer (see **C.2**); `P3-40` builds its replacement. Running them in listed
   order leaves a window with **no agent write path at all**. This is `R2 X-03`
   (`00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:1189`) and it is unresolved in the MDP.
5. **`P3-43` bullet 5 → `P3-31`.** "Unscoped episodic recall" cannot be fixed until a scope
   column exists (**A-5**, **B**). It is not an independent defect fix.
6. **`P3-44` depends on a line no task owns.** `context.rs:138` passes `""` as the query
   (**D.2**). No task in `P3-30…P3-44` mentions `context.rs`, `build_memory_context`'s call
   site, or threading the user message into recall.

### E.3 ⚠ The Python sidecar is still present and still wired — `P8-03` is a live comparison

The premise that the sidecar has been deleted does **not** hold in this working tree:

- `submodules/memory-mcp-server/` and `src-tauri/resources/sidecars/memory-mcp-server/` both
  exist and are populated (`main.py`, `src/`, `config/`, `Dockerfile`, `pyproject.toml`).
- `submodules/core-orchestrator/` + `src-tauri/resources/sidecars/core-orchestrator/` —
  **323 `.py` files** under the resources copy alone.
- It is still referenced from live Rust: `src-tauri/src/sidecar/client.rs:107`
  `const MEMORY_URL: &str = "http://127.0.0.1:8000";` (used at `:564` for `/health`),
  `src-tauri/src/sidecar/mod.rs:422,556,572`, and the whole of
  `src-tauri/src/commands/sidecar_commands.rs` (`:92-159` start, `:191` kill, `:206-237`
  restart, `:291-315` `start_memory_mcp`), registered in `src-tauri/src/lib.rs:147-151,303-313`.

`MASTER_DEVELOPMENT_PLAN.md:1464` — `P8-03` *"Native memory (`P3-F`) serves every read that
`memory-mcp-server` served"* — is therefore a **comparison against a live component**, not a
historical note. The Rust memory module makes **no** HTTP calls (no `reqwest`/URL in
`autodev-core/src/memory/`), so the two are wholly independent stores today; `P8-03` needs a
read-surface inventory of the Python server that no P3-F task produces.

### E.4 ⚠ The fleet IS coupled to memory — and the coupling went live on 2026-09-02

**Fleet workers can WRITE memory.** `recipes/runner.rs:830` builds the engine with
`PAMode::FastDevelopment`, and that mode registers both memory tools —
`builtin_tools/mod.rs:227` (`SearchMemoryTool`) and `:228` (`RememberThisTool`). No recipe
declares a tool allowlist that excludes them (`grep` over `autodev-core/src/recipes/` for
either name: **no matches**).

**Fleet workers can never READ memory.** `engine.rs:210-213` skips
`build_system_prompt_parts` whenever `system_prompt_override` is `Some`, and
`recipes/runner.rs:843-849` always passes `Some(&recipe.instructions)` (**D.4**).

**So the coupling is one-way and unscoped, and it points at the PA.** A fact a fleet worker
writes via `remember_this` goes to `semantic_memories` with `source_session =
"recipe-<slug>-<uuid>"` (`runner.rs:245`) and `working_dir` = the worker's directory. Nothing
filters on either: `SemanticStore::recall_facts` (`semantic.rs:155-163`) selects on
`is_active = 1 AND confidence >= 0.7` only. **Fleet-written facts therefore appear in the
interactive PA's injected system prompt**, at confidence 1.0 (`remember_this.rs:101`), ahead
of the user's own 0.75 extracted facts in the `ORDER BY confidence DESC`.

Symmetrically, a fleet worker calling `search_memory` reaches
`EpisodicStore::get_recent_summaries(session_id, …)` (`recall.rs:89`), whose only predicate is
`session_id != ?1` (`episodic.rs:169`) — so **a worker can read the PA's session summaries**.

**Why the fleet's state matters right now:** `P4-31` (`MASTER_DEVELOPMENT_PLAN.md:1129`) records
that eight of nine recipes were `Mode2Only` and *"return `Ok(())` on every tick, forever"* —
the fleet never reached the Rust engine, so this write path was dormant. That was flipped
**2026-09-02**, and the code confirms it: `autodev-core/src/recipes/defaults.rs` has nine
`fallback_policy` declarations, **all `Mode1Capable`**; the nine `Mode2Only` matches in that
file are all inside `// ⚠ P4-31 (razi-ruled 2026-09-02)` comments (`:220, :396, :665, :904,
:1031, :1225, :1387, :1959, :2052`). **The fleet now genuinely runs the Rust engine, so the
unscoped fleet→PA memory write path is live as of the day before P3-F starts.**

**Verdict:** **PARTIALLY CONFIRMED.** The one declared dependency is satisfied; six real ones
are undeclared, and the fleet coupling is entirely unmentioned.

**Consequence for the plan:** add the six dependency edges above to the P3-F ordering;
re-order `P3-39` after `P3-40`; add a task for `context.rs:138`; and add an explicit
scope decision for fleet-written facts — otherwise `P3-31`'s `G-MEM-SCOPE` gate
(*"project A cannot recall project B's facts"*, `03-CONTEXT-AND-MEMORY.md:245`) will be
written without anyone noticing that the fleet is the loudest violator.

---

## F · The reranker contradiction · **the MDP (`P3-42`) is RIGHT; `03 §4.6` is WRONG**

**The two claims.**
- `docs/restructurization/vision/03-CONTEXT-AND-MEMORY.md:206`: *"simple per-tier budgeting
  ships first; the anchor's §10 reranker (which **is** the competitive model) arrives in Phase 2."*
- `MASTER_DEVELOPMENT_PLAN.md:1068` (`P3-42`): *"The anchor §10 reranker as its own work item
  with its own columns — it is **not** the ported competitive model; **6 of 8 terms are
  disjoint** (G-13)."*

**The anchor's §10 score — `docs/restructurization/memory-anchor.md:442-452` (terms at `:444-451`), verbatim:**

```text
Score =
    semantic_relevance
  + lexical_relevance
  + importance
  + recency
  + access_frequency
  + scope_match
  + task_similarity
  + confidence
```

Eight terms, followed by *"The exact weighting should be determined experimentally."*

**The competitive model — `_grounding/G4c-memory-tiers-port-spec.md:442-450`** (restated at
`03-CONTEXT-AND-MEMORY.md:117-127`), Stage 1 `U_abs`, nine weighted terms summing to 1.00:

```
relevance 0.25 · authority 0.15 · recency 0.12 · type_bonus 0.08 · priority_class 0.10
· correction_boost 0.08 · session_anchor 0.10 · procedural_fit 0.08 · length_penalty 0.04
```

plus a Stage 2 the anchor has no analogue for at all (`G4c:480-487`):
`U_marg = max(0, U_abs − 0.30·redundancy + 0.25·gap_fill − 0.20·contradiction)`, greedy
selection, early stop at `0.15`.

**Term-by-term.**

| # | Anchor §10 term | Counterpart in `U_abs`? |
|---|---|---|
| 1 | `semantic_relevance` | **shared** — `relevance` (0.25) |
| 2 | `recency` | **shared** — `recency` (0.12) |
| 3 | `lexical_relevance` | **disjoint as a term.** BM25 exists but as a *separate* Reciprocal-Rank-Fusion stage, `bm25_weight 0.3 / semantic_weight 0.7`, `k = 60` — and **episodic only** (`G4c:` Step 2). Not a term of the score |
| 4 | `importance` | **disjoint.** `priority_class` (0.10) is the nearest, and is a class not an importance measure |
| 5 | `access_frequency` | **disjoint — absent entirely** |
| 6 | `scope_match` | **disjoint — absent entirely** (and scope does not exist in the schema at all, see **B**) |
| 7 | `task_similarity` | **disjoint — absent.** `session_anchor` (0.10) is session identity, not task similarity |
| 8 | `confidence` | **disjoint — absent from `U_abs`** |

**2 shared, 6 disjoint. The MDP's arithmetic is exact.**

And the disjointness runs the other way too: `authority`, `type_bonus`, `correction_boost`,
`session_anchor`, `procedural_fit`, `length_penalty` — six `U_abs` terms — plus all three
Stage-2 terms have **no counterpart anywhere in anchor §10.** These are not two spellings of
one model; they are two different models that happen to share "relevance" and "recency".

**Verdict: `03 §4.6` is wrong.** Its parenthetical *"(which **is** the competitive model)"* is
unsupported by either source document, and the MDP's `P3-42` is correct on both the claim and
the count.

⚠ **Note on the citation.** `P3-42` cites **`G-13`** for this, and `G-13` is ambiguous across
two documents: `_review/R2-contradictions.md:329` (*"Regenerate the mechanics inventory against
the completed ledger"* — about `M-10` being split, related) and
`_verify/A10-P3-gap-hunt.md:385` (*"Compaction slices the history at a fixed offset…"* —
entirely unrelated). The reader following `(G-13)` from the MDP has a 50% chance of landing on
compaction.

**Consequence for the plan:** `P3-42` needs no change. **`03-CONTEXT-AND-MEMORY.md:206` must be
corrected** — the parenthetical struck or rewritten to *"the anchor's §10 reranker, which is a
different model from the sidecar's competitive scorer (6 of 8 terms disjoint)"*. Under the
no-stale rule this is a doc fix owed in the same step. `P3-42`'s `(G-13)` citation should be
disambiguated to the specific document.

---
## F.1 · Addendum to F — the contradiction is already logged, and `03 §4.6` is not the only stale site

Sweeping `grep -rn "competitive model" docs/restructurization/` after settling F turned up
**independent corroboration and a wider blast radius than the brief implies.**

**Corroborated, by two documents that reached the same numbers separately:**
- `_review/R2-contradictions.md:30` (**`X-04`**) lists the disjoint anchor terms explicitly —
  *"`lexical_relevance + importance + access_frequency + scope_match + task_similarity +
  confidence`, none of which appear in the ported model"*. **That is the same six I derived
  term-by-term in F, independently.**
- `TESTING_GATES.md:293` and `:732` (**`F-5`**) add the reverse count: *"6 of 8 terms and 7 of
  9 are disjoint"* — 7 of 9 being `U_abs` minus `relevance` and `recency`. **Also matches.**
- `00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:1228` records it as gap **`G-13`**:
  *"⚠ The reranker equivalence claim was false… Because I declared them identical, the Phase-2
  reranker had **no work item and no schema columns**."*

This **resolves the citation question**: `P3-42`'s `(G-13)` points to
`00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:1228`, not to `R2-contradictions.md:329` or
`A10-P3-gap-hunt.md:385`. Three documents define a `G-13`; only one is the intended referent.
The MDP already lists this among its own corrected errors at `:110` and `:219` (`L-C2`,
*"A property was asserted without the mechanism, or the comparison, that would establish it"*).

**But the correction was never propagated. Three sites still assert the false claim:**

1. `docs/restructurization/vision/03-CONTEXT-AND-MEMORY.md:206` — *"the anchor's §10 reranker
   (which **is** the competitive model)"* — the site named in the brief.
2. `docs/restructurization/vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:1074` — *"the anchor's
   §10 reranker **is** the competitive model, so the two decisions agree"* — **in the same
   document that refutes it 154 lines later at `:1228`.**
3. `docs/restructurization/vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:1076` — inside
   **`▶ RAZI'S DECISION:`** for ADR-05-05, answered 2026-08-31: *"the anchor's §10 reranker —
   which is the competitive model — arrives in Phase 2."*

Site 3 is the one that matters most: the false premise sits inside the **answered decision
text itself**, so a reader reconstructing the mandate from the ledger inherits the error even
after reading the gap that overturns it. `TESTING_GATES.md:732` already flags the spec line
under the no-stale rule but names only `03 §4.6`.

**Consequence for the plan:** all three sites need the same correction, not just `03 §4.6`.
Site 3 is an owner-facing decision record — amending answered ADR text is a judgement call for
Razi, not a mechanical doc fix, so it should be surfaced rather than silently edited.

---

## G · Cross-cutting: the test surface over memory is empty

Stated as a check that can fail, because "no tests exist" is exactly the kind of claim that is
usually asserted rather than measured:

- `grep -rn "#\[test\]|#\[cfg(test)\]|#\[tokio::test\]" autodev-core/src/memory/` → **zero
  matches** across all nine files.
- The only integration test touching the memory stores is
  `autodev-core/tests/redaction_boundaries.rs`, whose four tests
  (`:75, :107, :139, :178`) assert **redaction on write** — that a secret does not land in the
  store, and that non-secret content does. None reads `build_memory_context`.
- `autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs` tests **tool-result** redaction
  (`:74, :129, :174`), not memory injection.
- `autodev-core/tests/s3_smokes.rs:3` says so itself in its own header: *"`S3-04` (recall sets)
  and `S3-05` (degraded recall with the embedding model…"* — the file implements `s3_01`,
  `s3_02`, `s3_03` (`:50, :110, :156`) and stops.
- `src-tauri/src/ci_gate.rs` touches memory only structurally: **D-12** at `:3322-3341`
  (asserts `engine.rs` *contains the string* `"build_system_prompt_parts"`) and the redaction
  site list at `:4016-4027`.

**Consequence:** every P3-F task carrying `UNIT-PROVEN` / `GATE-PROVEN` is starting from zero
coverage, not extending a suite. `P3-43`'s `UNIT-PROVEN` in particular cannot be a regression
proof for defects that (per **A**) do not exist to regress.

---

## H · Verdict table

| Claim | Verdict |
|---|---|
| **A-0** A cache exists in the Rust code | **REFUTED** — zero occurrences of `cache` in `autodev-core/src/memory/` |
| **A-1** Semantic recall drops `query` (vector template + lying docstring) | **PARTIALLY CONFIRMED** — symptom real, all three mechanisms false |
| **A-2** Episodic cache key ignores query | **REFUTED (vacuous)** — but episodic recall is query-independent anyway |
| **A-3** `learn_hitl_pattern` always throws | **REFUTED** — no such function, subtype, writer or reader |
| **A-4** Unfiltered procedural recall + unused breaker | **PARTIALLY CONFIRMED / OVERSTATED** |
| **A-5** Episodic recall never forwards `agent_id` | **REFUTED** — no such parameter; recall unscoped by a different mechanism |
| **A-6** Negative cache never fires | **REFUTED (vacuous)** |
| **A-7** Semantic cache not invalidated on write | **REFUTED (vacuous)** — writes are immediately visible |
| **B** Scope + provenance become first-class columns | **CONFIRMED as future work** — no scope at all; provenance half-present and write-only; ladder head = `1` |
| **C** Gateway: search/get/list/propose; `remember_this` removed | **PARTIALLY CONFIRMED** — 1 of 4 tools exists; `propose_memory` has zero occurrences repo-wide; 9 removal sites enumerated |
| **D** Memory is provably read | **CONFIRMED as necessary, UNDERSTATED as written** — path is real; `context.rs:138` passes `""`; fleet/subagent runs bypass it entirely |
| **E** Sequencing | **PARTIALLY CONFIRMED** — declared dependency met; six undeclared ones unmet; fleet coupling unmentioned; sidecar still present |
| **F** Reranker: `03 §4.6` vs `P3-42` | **`P3-42` is RIGHT, `03 §4.6` is WRONG** — 2 shared terms, 6 disjoint, exactly as the MDP says |

**The single most consequential finding:** `autodev-core/src/agent/context.rs:138` passes the
empty string as the recall query. It is the only call site of `build_memory_context` in the
tree, it makes all three tiers query-independent, it is not named in any task in
`P3-30…P3-44`, and it alone falsifies smoke `S3-04` today.

---

## What I could not determine

1. **Whether the four `memory.db` tables actually exist on this machine with the columns
   `db.rs` declares.** I read the ladder, not the database. `db.rs:3-4`'s "purely additive
   `CREATE TABLE IF NOT EXISTS`" comment describes exactly the failure mode
   (`migrate.rs:1-12`, the `airsctl` `last_seen` incident) that leaves a live table missing a
   column the source claims. Whether that already happened here is unknown.
   **Measurement:** `sqlite3 "$USERPROFILE/.autodev/memory.db" "PRAGMA user_version; .schema"`
   and diff the column list against `db.rs:48-117`. Read-only; I did not run it because it
   touches a live user database.

2. **Whether the memory tables contain any rows at all** — which decides whether `P3-44`'s
   gate would even have a memory to observe reaching the prompt, and whether the episodic tier
   is ever non-empty (summaries are written only at `turn_count % 20 == 0`,
   `summarizer.rs:19,27-29`, and only from the PA path `agent_commands.rs:373`, never from
   `recipes/runner.rs` — so most sessions produce none).
   **Measurement:** `SELECT COUNT(*) FROM episodic_memories WHERE summary_type='session_summary';`
   plus counts for `semantic_memories` / `procedural_memories`.

3. **Whether `semantic.rs:236-276`'s extractor can panic.** `extract_facts` computes
   `pos` from `content.to_lowercase().find(pat)` and then slices the **original**
   `content[pos..]` (`:251-252`, `:267-268`). Byte offsets in a lowercased string do not
   always match the original (e.g. `İ` U+0130 lowercases to two chars), so a mid-codepoint
   slice looks reachable — and this runs on every user message
   (`agent_commands.rs:90`). I did not confirm it because doing so needs execution, which is
   outside this pass's read-only scope. **Measurement:** a unit test feeding
   `"İ".repeat(n) + "i prefer x"` to `extract_facts`. If it panics, it is a live crash on the
   PA input path and belongs in `P3-43` regardless of how the rest of that task is rewritten.

4. **Whether `memory-mcp-server` serves any read the Rust store does not.** I established it
   is present and wired for lifecycle/health only from Rust (`MEMORY_URL` used solely at
   `client.rs:564` for `/health`), but I did not inventory its Python read surface.
   `P8-03` depends on that inventory. **Measurement:** enumerate the FastAPI/MCP routes under
   `submodules/memory-mcp-server/src/` and map each to a Rust equivalent.

*(Two further questions were opened and then closed during this pass, recorded here so the
negative results are not re-investigated:)*

- **Does anything outside `autodev-core/src/memory/` open `memory.db` directly?** **No.**
  `grep -rn "memory\.db" src-cli e2e scripts src src-tauri/src autodev-core/src` excluding the
  memory module returns exactly one hit — `src/data/docsAllD.ts:148`, a docs-panel string
  naming the old Python path `src/airs/memory/memory_core.py`. The 17 Tauri commands in
  `memory_commands.rs` are the only other consumer, and they go through the stores.
- **Does the false reranker equivalence appear anywhere besides `03 §4.6`?** **Yes — see
  F.1 below.** Question closed and folded into the F verdict.
