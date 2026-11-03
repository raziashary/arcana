# P3-F · verified in the MAIN SESSION before building (§0.2m)

Not agent output. Each of these was established here, by execution or by reading
the primary source, and each is load-bearing for a task.

---

## V-1 · `sqlite-vec` cannot conflict with our `rusqlite` pin — CONFIRMED

**Claim (P3-30):** *"`sqlite-vec` — Rust crate compiles the C source and statically
links it; no extension loading, no runtime dependency."*

**Measured.** Workspace pins `rusqlite 0.31.0` (`bundled`) → `libsqlite3-sys 0.28.0`.
crates.io dependency metadata for `sqlite-vec`:

```
sqlite-vec 0.1.9        deps: cc ^1.0  (build)   — and nothing else
sqlite-vec 0.1.10-a.4   deps: cc ^1.0  (build)   — and nothing else
```

**It does not depend on `libsqlite3-sys` at all**, so there is no version to
conflict with. It compiles C with `cc` and is registered through
`sqlite3_auto_extension`. Latest **stable** is `0.1.9`; `0.1.10` is alpha —
prefer the stable unless a needed feature is alpha-only.

⚠ The pre-GO review reported this claim held. This is an **independent**
confirmation by a different route (registry metadata, not a build), which is
what §0.2m's "re-verify in the main session" is for.

---

## V-2 · FTS5 is present in the shipped build — CONFIRMED, by execution

**Claim (P3-30, P3-36):** lexical fallback via FTS5, *"built into SQLite"*.

**Measured**, not assumed — a probe compiled against the workspace's own
`rusqlite`:

```
sqlite_version = 3.45.0
CREATE VIRTUAL TABLE probe_fts USING fts5(body)  → OK
INSERT + MATCH 'brown'                            → 1 row
```

A C toolchain is likewise proven present, because `bundled` already compiles
SQLite's amalgamation in every build we do.

---

## V-3 · Recall is query-independent in production — CONFIRMED, and BROADER than the plan says

**Claim (P3-43 defect 1):** *"Semantic recall drops its `query` … recall is
query-independent."* The plan attributes the seven defects to *"the code being
ported"* (the deleted Python sidecar).

**Reality — it is in the shipped Rust, and it is all three tiers, not one:**

* `agent/context.rs:138` — `build_memory_context(sid, "", repo_path)`.
  **The query is the empty string at the only production call site.**
* `memory/recall.rs:34` — `SemanticStore::recall_facts(SEMANTIC_LIMIT)`; the
  signature is `recall_facts(limit: u32)`, so the query is not merely dropped,
  it is **not a parameter**. Ordering is `confidence DESC, updated_at DESC`.
* `memory/recall.rs:33` — episodic is `get_recent_summaries(session_id, 5)`,
  recency only.
* `memory/recall.rs:35` — procedural is `search(query, 3)`, i.e. `search("")`.

So the injected `## Memory` block is **identical for every turn of a session**,
whatever the user asked. `P3-43` must be re-scoped from "port defects" to
"defects present in the shipped Rust", and `TG3-12`'s *"two different queries
produce different recall sets"* currently fails by construction.

**Also:** `build_memory_context`'s third parameter is `_working_dir` —
underscore-bound, discarded. There is **no project scope at recall today**,
which is `P3-31`'s premise confirmed.

---

## V-4 · The anchor's §10 reranker is NOT the competitive model — MDP right, spec stale

**Conflict.** `vision/03-CONTEXT-AND-MEMORY.md §4.6` says the anchor's §10
reranker *"(which **is** the competitive model)"*. MDP `P3-42` says it is **not**
and that *"6 of 8 terms are disjoint"*.

**Read both primary sources.**

| anchor `memory-anchor.md` §10 | competitive model, spec `03 §4.2` |
|---|---|
| semantic_relevance | w_rel·relevance |
| recency | w_rec·recency |
| lexical_relevance | — |
| importance | — |
| access_frequency | — |
| scope_match | — |
| task_similarity | — |
| confidence | — |
| — | authority · type_bonus · priority · correction · session_anchor · procedural_fit · length_penalty |

Two terms overlap; **six of the anchor's eight are absent from the competitive
model**. `P3-42` is correct and **spec §4.6 is the stale line** — a no-stale fix
this phase owes.

**Consequence:** the reranker needs columns that do not exist —
`importance`, `access_count` / `last_accessed_at`, `scope`, and a task handle.
`confidence` exists on `semantic_memories` only.

---

## V-5 · Live data, backed up before any schema work (Rule 13 / §0)

`~/.autodev/memory.db` — `user_version = 1`, `integrity_check = ok`, WAL.

| table | rows |
|---|---|
| episodic_memories | **39** |
| semantic_memories | 0 |
| procedural_memories | 0 |
| fact_history | 0 |

**Backup taken and read back before being trusted** (`VACUUM INTO`, the same
mechanism `db/backup.rs` uses — WAL-consistent, unlike a file copy):

```
~/.autodev/backups/memory.db.pre-P3F.20260902-200211.bak
102,400 bytes · integrity ok · user_version 1 · 39/0/0/0 rows verified
```

---

## V-6 · Migration mechanics for P3-30/31/38

* `db/migrate.rs::validate_ladder` requires versions **consecutive from 1**, so
  memory.db's next rung is **v2**.
* Each rung runs in its own transaction **with** its version stamp, so a
  database is never left having done the work without recording it.
* `run_ladder` refuses a database from a newer build.
* Destructive rungs auto-backup first and **abort if the backup fails**.
  `ALTER TABLE … ADD COLUMN` is additive, so this will not fire — the manual
  backup in V-5 is the protection that matters here.

⚠ **`L-C13` applies directly.** The v1 baseline must NOT be edited to add new
columns: a database already at v1 would never receive them, and only a *fresh*
database would look correct. New columns go in a **new v2 rung**, which both a
fresh database and the live one traverse.

---

## V-7 · Two memory-defeating paths that P3-F has NO task for

Found in `_grounding/G2b §5`, then re-verified against the code here.

### (a) `system_prompt_override` bypasses memory entirely

`agent/engine.rs` — when the user sets a custom system prompt,
`build_system_prompt_with_mode` **never runs**, so the `## Memory` block is never
assembled:

```rust
let system_prompt = if let Some(override_str) = system_prompt_override {
    override_str.to_string()        // ← memory assembly skipped
} else { context_builder.build_system_prompt_with_mode(...) };
```

It is a user-facing settings field (`settings_commands.rs`, read at
`agent_commands.rs`). **Any user who customises their system prompt silently
loses all memory recall**, with no warning in the UI or the logs.

⚠ This is `P3-44`'s invariant ("memory is provably read") failing for an entire
class of users, and **no task covers it**.

### (b) Memory is Mode-1 PA only

`agent/context.rs` states it plainly: *"Skipped when no session_id is set (e.g.
MCP server, subagent, recipe mode)."* Recipes, subagents and the MCP server get
**no memory at all**.

This is the same defect whose *description* I corrected in `P3-20`
(`remember_this` claimed facts "appear in every future system prompt"). The
description is now honest; **the behaviour is unchanged and unowned**.

→ Both need tasks. Proposed `P3-45` and `P3-46`.

---

## V-8 · There is no vector search anywhere today — greenfield confirmed

`_grounding/G2b §5`: a grep for `embedding|vector|cosine|faiss|hnsw|sqlite-vss|
sqlite-vec` over the memory module returns **zero hits**. All three tiers are
substring or plain ordering:

| Tier | Retrieval today |
|---|---|
| Episodic | `ORDER BY created_at DESC LIMIT 5`, no scoring |
| Semantic | `ORDER BY confidence DESC LIMIT 10`, **no query, no project scope** |
| Procedural | `LIKE '%q%'`; with `q=""` it matches everything |

So `P3-30`/`P3-32` build on nothing rather than replacing something — which
removes the migration risk from the retrieval path, and concentrates it on the
schema.

## V-9 · The injection chain P3-44 must prove, named end to end

`context.rs` (memory block) → `parts.join("\n\n---\n\n")` → `engine.rs`
`CompletionRequest.system` → the wire at `anthropic.rs`, `openai.rs`,
`google.rs`. `build_memory_context` is the **only** consumer of the three
stores on a prompt path, so that is the single seam a gate has to watch.

---

# Re-verification of the state refutation (§0.2m: "re-verify in the main session")

## V-10 · 🔴 My own P3-20 fix states the WRONG CAUSE — confirmed

The agent flagged `remember_this.rs`'s description. I checked, and it is right.

I wrote (P3-20): *"Recipe-mode runs, subagents and the MCP server have none [no
session id], so a fact stored there is persisted but will NOT appear in those
prompts."*

**Recipes DO have a session id.** `recipes/runner.rs:245`:
```rust
let session_id = format!("recipe-{}-{}", worker_slug, Uuid::new_v4());
```

The real mechanism is the **system-prompt override**, `agent/engine.rs:208-213`:
```rust
let (stable_prompt, volatile_prompt) = match system_prompt_override {
    Some(override_str) => (override_str.to_string(), String::new()),
    None => context_builder.build_system_prompt_parts(...),
};
```
The memory block lives in the **volatile** half (P2-14/D-12), and an override
makes that half empty. `runner.rs:849` passes `Some(&recipe.instructions)`;
`native_runner.rs:327` passes `system_prompt.as_deref()`.

**The conclusion was right, the cause was wrong.** I took it from
`context.rs`'s own comment — *"Skipped when no session_id is set (e.g. MCP
server, subagent, recipe mode)"* — and propagated it into a **user-facing tool
description** without checking it. That is **`L-C3`** (a source's conclusion
adopted without checking its load-bearing detail), and the source was a comment,
which `L-22` already says is the thing never to trust.

⚠ Two sites to fix: `remember_this.rs`'s description AND `context.rs`'s comment.

---

## V-11 · 🔴 A fleet worker can write into the user's memory at confidence 1.0 — and MY `P4-31` made it live

Verified end to end, every hop:

1. `recipes/runner.rs:826` — a recipe engine runs `PAMode::FastDevelopment`.
2. `builtin_tools/mod.rs:228` — that mode registers `remember_this`
   (it is registered in **every** mode, including `Delegate` at `:266`).
3. `remember_this.rs:98-100` — writes with **`confidence = 1.0`** and
   `source_type = "user_explicit"`.
4. `memory/semantic.rs:153-164` — `recall_facts` filters on
   `is_active = 1 AND confidence >= 0.7`, orders by `confidence DESC`, and has
   **no scope predicate at all**. `working_dir` is SELECTed and never used.
5. So the worker's fact lands at the **top** of the interactive user's
   `## Memory` block, under the heading *"What I know about you and this
   project"*, labelled as something the user said explicitly.

⚠ **This was dormant until today.** Eight of nine recipes could not run
(`P4-31`); flipping them to `Mode1Capable` this afternoon made this path live.
A change of mine, and the audit that found it ran before the change — so
nothing would have caught it but this pass.

`P3-39` (gateway, `remember_this` removed) and `P3-31` (scope) are the fix, but
neither task names **confidence attribution**: an autonomous agent writing at
the same confidence and provenance as the user is a trust defect independent of
scope.

---

## V-12 · The Python sidecar — OVERSTATED, and the distinction matters

Agent: *"the Python sidecar is still present and wired — 323 `.py` files,
`MEMORY_URL` at `client.rs:107`."*

**Measured:**
* **13,665** `.py` files on disk (not 323).
* `git ls-files "*.py"` → **0**. **None are tracked.** This is the untracked
  deletion backlog `P8` owns and `§0.9` warns about (`git add -A` would commit
  225 MB of it).
* `MEMORY_URL` occurs exactly twice: the constant at `client.rs:107` and a
  `/health` probe at `:564`.

**The memory read/write path is pure Rust** — `build_memory_context` →
`EpisodicStore` / `SemanticStore` / `ProceduralStore` → `memory.db`. Nothing in
it traverses the sidecar.

So: *present on disk* — yes, and untracked. *Wired for memory* — no. My brief's
premise ("since deleted") was imprecise; the operative fact holds, and P3-F does
not interact with the sidecar.
