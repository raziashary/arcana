# AutoDev Restructure — Master Development Plan (MDP)

> **LIVING DOCUMENT.** Task status, evidence lines, the risk log and the change log are edited as the build proceeds. Design mechanics are **not** edited here — they live in the vision suite, and on conflict the vision governs mechanics while this document is corrected.

| | |
|---|---|
| **Parents** | `../vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md` (51 ADRs, 0 open) · specs `../vision/01`…`10` · `../vision/_grounding/G0`…`G12` · `../vision/_review/R1`…`R3` |
| **Anchors** | `../ui-anchor.md` (UI, DOCTRINE) · `../memory-anchor.md` (memory, adopted 2026-08-31) |
| **Owner** | Razi (Programming Lead, AIRS Foundry) |
| **Branch** | `main` (Razi authorised working directly on it; `sidecar-backup` holds the pre-restructure state) |
| **Rev** | 1.0 — created 2026-08-31 |
| **Status** | ⛔ **AWAITING GO. No code, no migrations, no deletions.** |

## The document family

| Doc | Role |
|---|---|
| **`MASTER_DEVELOPMENT_PLAN.md`** (this) | The build plan: atomic checkboxed tasks, phases, per-phase smoke checks. **The anchor — the others follow its phase sequence.** |
| `TESTING_GATES.md` | The permanent, named, automated spec-invariant tests per phase (`TG<p>-<nn>`), run **after** build + smoke |
| `READINESS_MATRIX.md` | Percentage-scored readiness plus a safety overlay that can cap the total |
| `NOVELTY_LEDGER.md` | Governance for every externally-inspired mechanic |
| `ADR_LOG.md` | The runtime decision journal — build-time decisions only |

---

# §0 · Rules of Development

A rules register, not a task list. Every rule is always-on.

**§0.0 — Prime principle: the shipped codebase is the Anchor Point.** The plan bends to shipped code, never the reverse. Changing a shipped contract requires a named ADR, a migration, and live proof. This programme exists because ~415 defects accumulated behind checks that could not fail; the code is the only truth that argued back.

**§0.1 — The GO gate.** No development until Razi's explicit GO. Rev 1.0 is a plan, not a mandate.

**§0.2 — Additive by default.** New modules, new migrations, new tables. Every phase that touches persistence enumerates its migration range. Destructive changes require a named ADR.

## §0.2a — Two audit subagents after every phase

**After every phase is done, spin up two subagents:**

| # | Subagent | Mandate |
|---|---|---|
| **1** | **Work auditor** | Audits *the work just done* — is every task actually built, does the evidence match the claim, is any checkbox flipped ahead of its proof, does any doc line go stale |
| **2** | **Area auditor** | Audits *the codebase area the work touched* — gaps, bugs, refinements, and **bad code**: discarded return values, dead parameters, guards that cannot fire, functions called repeatedly where one call would do |

**The area auditor's list is not generic — it is this codebase's measured defect profile.** `let _ =` on fallible calls is the single most common finding across all nineteen grounding reports. Dead parameters are real here (`is_claim_recent_and_live(_live_ids)` never reads the argument its name promises). And guards that cannot fire is the signature defect of this repo: `Tool::requires_permission()` never called · the Fix-8c build sentinel unreachable by construction · the subagent recursion guard inert · the orphan TTL unable to parse its own timestamp. **An auditor briefed to look for these will find them, because they are already here.**

⚠ Both are **read-only** and get self-contained briefs with an explicit output contract.
⚠ **A subagent's finding is evidence, not a result.** It is verified in the main session before it becomes a row. *In this session's planning alone, verification sharpened three subagent claims and corrected one outright — the toast system was reported as non-existent when the store and renderer are both correct and only the two producers are broken, which changed the fix from "build a system" to "fix two imports".*

## §0.2b — A phase cannot close until both audits **and their findings** are resolved and logged

Not "the audits ran." **Every finding is either fixed in the phase, or logged as a numbered row with an owner and a phase.** A phase that closes over an open audit finding is a §0.2b violation.

## §0.2c — 🔴 Razi is the overseer — every finding runs through him

**Every finding from either audit subagent is reported to Razi.** Not summarised away, not silently fixed, not deferred by the builder's judgement. He decides what is fixed, what is deferred, and what is dropped. *The builder audits; the overseer rules.*

## §0.2d — Never stop a phase midway — ask instead

**A phase is worked to its end.** If Razi's input is needed, use the **`AskUserQuestion` tool** and continue from the answer — do not end the turn, do not park the phase, do not ask "shall I continue?".

The only legitimate stops are: the phase is complete · a §0.8 destructive operation needs a backup decision · Razi says stop.

⚠ This **strengthens §0.7's continuation default**: previously a blocker meant surfacing and waiting. Now a blocker means *asking in-flight and proceeding from the answer*.

## §0.2e — Every task names its proof

**Every task carries an evidence class (§0.4) inline, and names the smoke check or gate that will prove it.** A task with no named proof is not done.

*Adapted rather than adopted literally: the source rule requires a dedicated smoke check **and** gate per task. Here that would inflate 154 tasks into 154 gates, and it would be dishonest — the eight updater tasks in P0-A all prove one thing (`G-UPDATE-REFUSE`), and splitting that into eight gates would manufacture coverage rather than measure it.* **Gates may be shared; proof may not be absent.** A task whose proof is "it compiles" is not proven.

## §0.2i — Two subagents is the **floor**, not the cap

§0.2a's two post-phase auditors are **mandatory**. Beyond them, **spin up as many as the work warrants** — parallel readers over disjoint areas, an adversarial reviewer that has not seen my reasoning, a test author writing from the spec's own wording rather than from the implementation, independent verification of a suite run.

⚠ **Never let two writers touch the same files.** Partition write work by module boundary; integrate in the main session.

⚠ **Every dispatch prompt MUST carry the incremental-write clause.** Verbatim:

> *Create your report file FIRST, before any investigation, and append to it as you go. Do not hold findings in memory to write at the end — if you are interrupted, everything unwritten is lost.*

This is not a style preference. A rate limit destroyed four reports mid-flight during planning; the lesson was recorded in §0.16 and then **not applied**, and both P0 post-phase auditors were lost the same way with nothing on disk (`L-C8`, `L-10`). The clause lives here, in the dispatch rule, because a lesson only takes effect where it is enforced.

## §0.2j — On a blocker: **search the web first**, then ask

**Order:** (1) the codebase and the grounding reports · (2) **the web** — provider docs, crate semantics, error strings, platform behaviour · (3) `AskUserQuestion` (§0.2d).

⚠ **Escalate a *decision*, not a *fact*.** A question a search or a `path:line` can answer is not a question for the overseer. **A question only Razi's judgement can settle — a trade-off, a capability call, a policy — goes to him immediately and does not wait for the phase to end.**

*Known web-answerable open items in this programme:* whether `ort`'s ONNX runtime can be vendored offline rather than fetched at build time (`P3-33`) · xAI's finish-reason taxonomy, which `G6 §5.5` records as **undocumented** and which `TG2-10` depends on · Tauri v2 capability semantics for the narrowed permission set (`P0-26`) · whether `keyring` 2.x exposes a usable fallback on headless Linux or whether the encrypted-file path must be written from scratch (`P2-24`).

## §0.2m — 🔴 Adversarially verify every phase BEFORE building it

**Before writing code for a phase, dispatch agents to REFUTE that phase's load-bearing claims.**

| | |
|---|---|
| **Brief** | *"Try to refute each claim. **Default to 'refuted' if the evidence is ambiguous.**"* Never *"confirm that…"* |
| **Scope** | every claim the phase's tasks rest on — the defect, the mechanism, the cited `path:line`, the prescribed fix, and the sequencing |
| **Output** | per claim: **CONFIRMED / REFUTED / PARTIALLY CONFIRMED / OVERSTATED**, with quoted code — plus *"what I could not determine"* and the measurement that would settle it |
| **Then** | 🔴 **re-verify every load-bearing result in the main session** before it changes a row |
| **Then** | correct the plan, the gates, the matrix and the ADR log **before** the first line of code |

🎯 **This is a rule and not a habit because this plan's own pre-GO review found six errors in it.** Three fresh-context reviewers over the vision suite:

| # | What it caught | What it would have cost |
|---|---|---|
| 1 | 🔴 **Deleting the sidecar deletes a fault boundary.** `panic = "abort"` + in-process agents ⇒ one panic kills the app and every session. Ten specs, zero mentions | We would have shipped a product where any of the catalogued reachable panics takes down every session at once |
| 2 | 🔴 **The redaction port was dropped.** Grounding ruled it **PORT** — *"the `SENSITIVE_KEYS` list is the spec"* — and it appeared **zero times** across the plan and all ten specs | Unredacted tool arguments would have entered **permanent** memory, and no gate would have noticed |
| 3 | 🔴 **"The anchor's reranker *is* the competitive model" was false** — 6 of 8 terms are disjoint | The Phase-2 reranker had **no work item and no schema columns**; it would simply not have been built |
| 4 | 🔴 **Part 1B had not been regenerated since rev 0.3** — none of ADR-05-01…05-06 owned a mechanic | An MDP harvested at that moment **would have omitted the entire memory programme** |
| 5 | 🔴 **I silently narrowed Razi's ADR-02-02 answer** instead of flagging that a literal reading re-creates BUG-COMPILE | The retry path would have re-introduced the exact defect the phase exists to fix |
| 6 | **Moving `/mcp` deletion to P0 broke the ACP identity chain** | A decided-preserved capability would have been **silently dead P0→P3, with no gate that would notice** |

**And it cut the other way, which is the point:** I briefed the reviewer to attack the plan's weakest technical claim — that `sqlite-vec` and `rusqlite` compose. **It held.** A refutation brief that only ever confirms is a rubber stamp; one that only ever refutes is noise. This one did both.

⚠ **The post-phase auditors (§0.2a) cannot substitute for this.** They run *after* the work. ***A verification that runs after the build measures the build; one that runs before it measures the plan.***
⚠ **This does not replace §0.2a.** Verify before, audit after. Both.

## §0.2n — 🔴 Every closed phase writes its learnings into §0.16

**A phase is not closed until its mistakes are written down where the next phase will read them.**

| | |
|---|---|
| **When** | at closeout, in §0.6's fixed order — **before** the commit, not after |
| **Where** | **§0.16**, inside §0 — so §0.12's *re-read before you build* already covers it |
| **What** | every defect **I** produced, every claim of mine that was refuted, and the **general rule** each one yields — not a diary of the phase |
| **Shape** | the lesson first, the instance second. ***A learning nobody can apply to a different file is an anecdote*** |
| **Also** | what **worked** — a register of only failures teaches caution, not method |

⚠ **Recurring classes get a running count.** A defect seen once is an accident; the same shape seven times is a **property of how I work**, and only a tally makes that visible. §0.16's table carries the counts, and **every new instance increments one** — or opens a new row and says why it is new.

🔴 **This register is evidence, not decoration.** If a phase's §0.2a audit finds a defect whose class is **already in §0.16**, that is a **worse** finding than a novel one, and the ADR entry says so plainly. ***Repeating a recorded mistake is a failure of process, not of luck.***

## §0.2L — Method rules, each paid for

- **§0.2L-1 · Never paraphrase a measurement into a shape its source cannot emit.** Quote output and code **verbatim**, with `path:line`. A paraphrase that looks right manufactures false alarms and sends investigations the wrong way.
- **§0.2L-2 · Use the instrument you already hold before you infer.** ***An inference is not a measurement, and a missing signal is not a negative reading.*** *This programme has a live example: the plan asserted caching would work, when the system prompt is not byte-stable — the instrument that settles it is the `cache_read` usage field, which is why §0.2e requires the alarm to ship before the cache.*
- **§0.2L-3 · Search for the string; do not sample for it.** A `--limit` sample answers *"is it in this window"*, never *"does it exist"*. *The finding that `redact`/`scrub`/`sanitiz` appear **zero times** across thirteen documents is only meaningful because it was a search, not a sample.*
- **§0.2L-4 · The second guess earns the same scrutiny as the first.** A replacement explanation that *looks* correct, read from source, is still a hypothesis. *`port_status.rs` is this programme's instance: a grounding report said KEEP, I propagated it into two documents, and only a third reader checked the import.*
- **§0.2L-5 · Snapshot the working tree before fanning out subagents.** ⚠ Untracked files are not in git and not in any stash — a parallel fan-out can lose them with **no recovery path**. `git stash -u`, or a copy, **before** the fan-out. *This session ran up to seven subagents at once; the rule is not theoretical.*

**§0.3 — No stale docs (hard rule).** Any change that makes any line in any project document stale is fixed **in the same step**, or flagged if it needs Razi's judgement. Every phase ends with a no-stale sweep across the vision suite, this family, and the grounding reports. *This rule has already been exercised six times during planning — see the vision Part 10.*

**§0.4 — Evidence classes.** No task is done without its named proof recorded inline:
`CODE-PRESENT` · `UNIT-PROVEN` · `LIVE-DB-PROVEN` · `SMOKE-PROVEN` · `GATE-PROVEN` · `REPLAY-PROVEN` · `BENCH-PROVEN` · `DOC-GATE` · `PLATFORM-TRIPLE` (verified on Windows, Linux **and** macOS).

**§0.5 — Smoke-then-gates is mandatory.** Both are the definition of done. A gate skipped in CI is **not green**. This is INV-4, and it is the rule this codebase most needs: today only `cargo clippy` and `tsc` can fail.

**§0.6 — Per-phase workflow, in this fixed order.**
`build → smoke S<p> → gates TG-P<p> → readiness matrix → progress row → no-stale sweep → commit → continue into the next phase.`

**§0.7 — Continuation is the default.** A phase boundary is a checkpoint — commit and trackers current — never an exit and never a place to ask "shall I continue?". Context caution is answered with **durability, not stopping**: commit at every boundary, keep checkboxes and the ADR log current as you go, so a fresh session resumes from the artifacts alone.

**§0.8 — Live-data discipline.** Every stateful resource is **irreplaceable until Razi says otherwise**. Back up (or verify a restorable backup exists) before any schema-mutating or destructive operation — *including one reachable through a test*. Suites run **without** live credentials by default.

> **Known-destructive operations in this project, named here because this is the section every session and every subagent dispatch must read:**
> - **Migration repair of `graph/` v5.** The head migration uses `ADD COLUMN IF NOT EXISTS`, which is not valid SQLite (`G3d F-31`). Repairing it touches a live per-repo graph DB. **Back up `~/.autodev/graphs/*.db` first.**
> - ~~**The `~/.airs/` → `~/.autodev/` consolidation** (ADR-01-09)~~ → **DONE in `P1-14`.** It moved `sessions`/`messages` only: `~/.airs/plugins/` and `~/.airs/fleet_secret` were **code-only paths that never existed on disk**, so half this warning was never true. The file was copied with `VACUUM INTO` rather than rebuilt from source, which is what preserved `messages.tool_calls`/`tool_results` — two columns the current `CREATE TABLE` no longer declares. The source is deliberately left in place; deleting `~/.airs` needs Razi.
> - **`git_discard` with an empty file list** force-reverts the entire worktree (`G3b GI-06`). Never invoke it in a test.
> - **`delete_file` calls `remove_dir_all`** despite a docstring saying "empty directory" (`GI-05`).
> - **Plugin uninstall runs `rustup self uninstall -y`** — it removes the machine's Rust toolchain (`G3a SK-99`).
> - **`ensure_default_recipes` / `ensure_default_skills` overwrite user edits on every launch** (`SK-49`, `SK-69`).
> - **`git add -A` / `git add .` would commit 225 MB of untracked Python.** `submodules/` is untracked **and not gitignored** (§1.1a), so a blind stage pulls in the entire sidecar tree. **Always stage by explicit path.** This is §0.9's rule, restated here because this is the section every session reads.
> - **Deleting `submodules/` without `.git/modules/submodules/`** leaves both object stores in the repository — the sidecar surviving by address rather than by content (§1.1a, P8-C).
> *This list is appended to as more are discovered. Removing an entry requires proof it is fixed.*

**§0.9 — Commit discipline.** One commit per lettered sub-section. Conventional-commit messages stating what and why. Stage deliberately — never `git add -A`. Scan every diff for credentials. **Razi authorises all pushes**; the first push of any branch and anything touching `main` requires explicit approval.

**§0.10 — Environment and port cleanup.** Kill every dev server, watcher and background listener started during a task, and verify the port is free, before the task is considered done.

**§0.11 — Runtime ADR ledger.** Any build-time decision is logged in `ADR_LOG.md` **immediately**, in the same step as the decision. An acted-on-but-unlogged decision is a §0.3 violation. Retractions are entries too — record the mechanism of the error, never delete the original.

**§0.12 — Re-read before you build.** Re-read the plan and the cited spec section before and during every task, even when you believe you hold it in memory. **If faithful implementation of the docs still fails, the docs are what is wrong** — fix them, never silently patch around them.

**§0.13 — Dual anchor.** Ground every decision in (a) the shipped code and (b) the vision suite. A genuine conflict is a **stop-and-flag**, never a quiet pick. *Two such conflicts were already found during planning: ADR-02-06 × ADR-03-02, and ADR-01-05 × OQ-4.*

**§0.14 — Always mark task status**, in both this document's checkboxes and the session tracker. A stale status is a §0.3 violation.

**§0.15 — Escalation tiering.** Decide independently where a clear codebase precedent exists. **Flag and propose** for any new dependency, pattern, or scope change — and always for anything that narrows a §0 rule. **Stop and ask** for architectural decisions with no precedent, changes to shipped contracts, or anything hard to reverse.

---

## §0.17 · Public-facing documents are refreshed at every phase closeout

`docs/restructurization/public-facing/` holds documents written for readers **outside** the
build — a supervisor, a partner team. They are condensed from the vision plan and this one.

**Standing rule, razi-ruled 2026-09-01** (*"publish and keep it updated each phase"*): the
per-phase closeout in `§0.5` gains one step. After the no-stale sweep across the internal
documents, every document in `public-facing/` is re-checked against what actually changed and
**its published page is redeployed**.

Two reasons this is a rule and not a habit:

1. **A public document that overstates progress is worse than no document**, because its
   readers cannot check it against the code. Every internal claim here is falsifiable by
   someone with the repository; nothing in a supervisor's copy is.
2. **The markdown file in the repository is the source of truth**, and the published page is
   rendered from it. Updating the page without updating the file inverts that and makes the
   repository the stale copy.

The brief's own numbers — readiness, task counts, hours remaining, the handover estimate —
are exactly the figures a phase closeout changes, so a closeout that skips this step ships a
document that was accurate an hour earlier.

# §0.16 · Learnings register

Required by §0.2n. **Seeded from the planning phase**, because that phase produced real defects of mine and the next phase should read them before it starts. The lesson comes first; the instance second.

## Recurring classes — running count

**A defect seen once is an accident. The same shape three times is a property of how I work.** Every new instance increments a row, or opens a new one and says why it is new.

| # | Class | Count | Instances |
|---|---|---|---|
| **L-C1** | **A derived artifact was not regenerated after its source changed** | **4** | Part 1B stale since rev 0.3, so none of ADR-05-01…05-06 owned a mechanic · a **duplicate `ADR-02-06`** created by inserting into a ledger without re-reading it · the specs' gate tables never regenerated against the rev-0.9 mechanics · **spec `10 §6` still said the test frameworks and e2e harness land in P9 while the MDP had moved them to P0 and stated it was correcting that spec — the correction was never applied to the spec itself** (found by the §0.12 re-read at P0 start, before any code) |
| **L-C2** | **A property was asserted without the mechanism, or the comparison, that would establish it** | **3** | *"the anchor's reranker **is** the competitive model"* — 6 of 8 terms are disjoint · *"structurally impossible"* — the enum only relocates the dead-latch to `FromStr` · the cache-hit formula generalised from Anthropic to four providers |
| **L-C3** | **A source's conclusion was adopted without checking its load-bearing detail** | **5** | `port_status.rs` "should be KEPT" propagated into two documents; one import line disproved it · `G4b §A.14`'s two-part verdict adopted by half — the vault landed, the **redaction was dropped** · **P8 was written as "a `git rm` and a `.gitmodules` edit" from the presence of `.gitmodules` alone — the index has zero gitlinks, so `git rm` would have failed on first contact** (§1.1a) · **`G3d F-44` said `db/checkpoints.rs` is uncompiled (true) and concluded undo does not exist (FALSE) — I propagated that into my own gate code, two gate rows and two master-plan rows, and `reachability_gate.rs` ended up contradicting its own exception list in the same commit** (P1 pre-build)  · **P3-F.** `context.rs`'s comment said memory is *"skipped when no session_id is set (e.g. MCP server, subagent, recipe mode)"*. The conclusion was right and **the cause was wrong** — recipes DO build a session id; the system-prompt override is the mechanism. I adopted the comment without checking its load-bearing detail and propagated it into a **user-facing tool description** in `P3-20`. The source was a comment, which `L-22` already says never to trust |
| **L-C4** | **A local change was made without re-checking the surrounding control flow** | **3** | Moving the `/mcp` deletion to P0 broke the ACP identity chain described four sections earlier in the same document · **setting `orch_healthy = false` to make Mode 2 unreachable, having read only the code BELOW that line — a latch twenty lines above returned early from every tick, so the fleet was silently dead on any machine without Python while my own comment two lines down claimed the opposite (`F-10`)** · **widening the `qc_review` gate to every caller without re-reading the shipped supervisor recipe, which calls `update_task_status(id, 'qc_review')` on a worker's behalf and never builds — so `request_review` failed every time (`F-22`)** |
| **L-C5** | **An owner's answer was reinterpreted rather than escalated** | **1** | ADR-02-02's option (B) was silently narrowed to "diagnoses only" instead of flagging that a literal reading re-creates BUG-COMPILE |
| **L-C6** | **A check was written from the CLAIM rather than from the INVARIANT it protects** | **11** | `TG0-04` first asserted `.env` must not **exist**, and failed correctly — a developer `.env` is expected in a working tree; the invariant is "never committed" · the vault gate first encoded "reads the vault ⇒ returns no `String`", which flags `send_message` (reads a key, returns a session id) and `test_provider_connection` (returns a status) — both correct · the `safe_filename` test asserted a fallback name for `..\..\evil.exe`, but WHATWG normalisation resolves it to the harmless `evil.exe` **(P0)** · **`config.db`'s owner was found by searching for `CREATE TABLE … app_settings` — the table — instead of for `config.db` — the file. Three of its six tables stayed outside the ladder that claimed to version it, and `INV-10` was scored GREEN on the wrong count (P1 post-phase audit)** · **`TG-P1-06` was written from the claim I had just implemented — *"every tool execution redacts its result"* — instead of from the invariant — *no secret reaches a durable surface*. So it was structurally blind to the input side, and `execute_batch`'s write branch went on handing raw tool arguments to the UI and the `messages` table after the fix I had recorded as closing that boundary. Found only because Razi ruled a *different* finding forward (P1 post-phase audit, A-08a)**  · **P3-F.** `TG-P3-44` as the task worded it — *"a recalled memory appears in the outbound prompt bytes"* — was **already true and proved nothing**, because recall was query-blind and the same bytes appeared whatever was asked. Caught in the pre-build pass and re-pointed to the invariant (*different queries differ*) before the gate was written  · **P3-F post-phase audit.** `TG-P3-47` asserted `tool.contains("Proposer::from_agent_role(&ctx.agent_role")` — a **prefix**, which matched happily when the real call was `from_agent_role(&ctx.agent_role, user_stated)`. It inspected the receiver and ignored the second argument, which was the model's own boolean and the thing that actually elected provenance. The gate was green over a full privilege escalation for the whole phase. Re-pointed to the **closing paren** and to the function's signature, so no second argument can exist |
| **L-C7** | **A source-scanning assertion matched its own source** | **1** | The vault gate's banned-name list contains the banned names as string literals, so `include_str!` on its own file found them and the test failed on itself — the exact trap `V3` had warned about in `graph/extract.rs:711`, where `"#[test]"` as a literal makes grep-based counters overcount **(P0)** |
| **L-C8** | **A lesson already recorded in this register was not applied to the next opportunity** | **2** | §0.16's own "make subagents write incrementally — a rate limit destroyed four reports mid-flight" was written during planning, then **both P0 post-phase auditors were dispatched without it** and both were destroyed by a rate limit with nothing on disk **(P0)** · **`L-18` — "a fix that lands in the right place and is never reached is not a fix" — was written into this register DURING P2, from `P2-57`. The post-phase audit then found five more instances in the same phase, three of them committed *after* the lesson was recorded.** The register is not a place where lessons take effect; the build is. That is why `L-21`'s remedy is a gate per capability, not a note **(P2)** |
| **L-C9** | **A verification whose SHAPE cannot express failure** | **10** | `S0-01` launched the app through `head -5`, so the startup crash after those lines was invisible and the smoke was recorded green · a build verified with `cargo test \| grep "^test result: ok. [1-9]"` — a pattern that can only match a SUCCESS line, so the failing target's `test result: FAILED.` was filtered out and a red build was pushed · the workflow "validated" by counting a string with `grep`, where the count itself was wrong **(P0)** · **`S1-04` called the method it was testing directly instead of the one production calls, and `TG-P1-06` asserted a file *contained the string* `security::redaction` rather than that the call was wrapped — so both proofs were green over a completely unprotected path (P1 post-phase audit)** · **three more in P2, all from ONE habit: chaining a commit after a check with `&&`.** `cargo test ...; echo "EXIT=$?" && git commit` always continues, because `echo` succeeds whatever it prints — so a failing suite was *printed* and then committed over, twice, and a third time the verdict `(none=green)` was echoed unconditionally after a `grep` that had just matched `FAILED`. **The output said failed and the next line of my own output said green.** Two of the three pushed a red commit that had to be repaired in the next one (P2)** · **`TG-P3-20` compared each tool description against a corpus that INCLUDED that description.** Checking "is this value declared anywhere in the crate?" against a concatenation of every `.rs` file — descriptions and all — meant the description corroborated itself, and a planted `worker-ghost` passed. ⚠ The blindness was introduced by *fixing* the previous false positive: scoping the search to one file flagged legitimate indirection, so I widened it to the crate and swallowed the claim along with the code. **Falsification is what caught it; the gate was green and wrong for one run** (P3) · **`TG-P3-55`'s falsification could not tell "the gate fired" from "the crate did not build".** Two of three planted defects were *syntactically* invalid — a renamed type and an undefined variable — so `cargo test` exited 101 on a **compile error** while I read it as the gate firing. The exit code was identical; only the reason string differed, and I had printed that too. It said `False` on both, which is the line that caught it (P3)  · **P3-F.** I verified the feature-gated embedding code with `cargo check --features embeddings` and it passed — but a **vacuous** pass looks identical, because an inactive `cfg` compiles nothing. Only planting a type error INSIDE the gated module proved the check was reaching it |
| **L-C10** | **A failing check was read as worthless rather than as a finding** | **1** | The e2e job failed three times and I argued for deleting it as "noise from an unfinished capability". Its failures were **moving** each run — rejected, timed out, then carrying a crash reason — and the third was reporting a real startup crash that nothing else in CI could see, because nothing else executed the binary. I was one run from deleting the only honest signal **(P0)** |
| **L-C11** | **A claim about *state* was inferred from code instead of checked against the state** | **3** | Three of this phase's premises survived reading and died on measurement. `graph/` v5's six swallowed `ADD COLUMN IF NOT EXISTS` statements led me to conclude the columns were missing — `CREATE_NODES` had declared them all along, and v5's own next four lines (two `CREATE INDEX … ON nodes(agent_author)` using `?`, not `let _ =`) would have errored if they were absent, so its success was evidence against my reading **inside the same function** · `busy_timeout` was "set nowhere, so contention returns SQLITE_BUSY immediately" — `rusqlite` sets 5000 ms on every connection it opens, and a two-line probe said so · the timestamp fix was aimed at the space separator, which chrono accepts; the missing **offset** is what breaks it **(P1)** |
| **L-C12** | **A helper was built, unit-proven, and wired to nothing** | **8** | `db/backup.rs` landed first in P1 because everything destructive depends on it. At P1-19's wiring step it had **zero callers** — and I had spent the whole phase taking backups by hand with `VACUUM INTO` rather than through it, which is what made the gap invisible. A backup helper nothing calls is the same defect class as a gate that cannot fail **(P1)** · **the redaction boundary was wired to `ToolExecutor::execute`, which has NO production caller** — the agent loop calls `execute_batch`, so every real tool call carried raw output to the model, the database and the UI while the phase's smoke test drove the dead method and the gate matched a substring in it. Found by **three independent auditors**, in the phase where I wrote *"compiling is not wiring"* three times about someone else's code **(P1 post-phase audit)** · **five more in P2, three of them committed AFTER I wrote the lesson down in this very register.** The stable/volatile prompt split (built, documented, unit-tested 17/17 — and the engine called the *joining* wrapper, so memory stayed inside the cached region and every turn paid a cache-write surcharge for a prefix never read back) · the daily spend ceiling (correct, gate-proven, and **unarmable**, because `setDailyBudget` had no caller anywhere in the app — so the phase's own narrative success criterion could not be executed) · the vault backend disclosure and orphan list (both implemented and documented; **no UI called either**, so "tell the user which store holds their keys" was invisible) · `max_output` (added expressly to clamp `max_tokens`, read by nothing) · `ErrorClass` (defined, unit-tested against itself, **constructed by nothing** — so a spend-cap 429 and a rate-limit 429 were the same string). **Every one was invisible to a green suite and visible in ten seconds to a grep for callers** (P2 post-phase audit) |
| **L-C15** | **A check whose SCOPE was silently widened by the environment, so it measured the wrong region** | **1** | Three gates isolate a function body by splitting source on a literal `"\n}\n"` or `"\n    }"`. Git's `core.autocrlf` rewrote the working tree to CRLF, that pattern stopped occurring, and each gate's `unwrap_or(whole_file)` fallback **silently widened it from "this function's body" to "the rest of the file"**. `S2-11` then failed on a `HashMap` roughly 40 KB below the function it was checking — while the path it actually guards is `BTreeMap`-only and entirely correct. ⚠ **Distinct from `L-C9`:** this check *could* express failure and did; it simply reported on code it was never pointed at. A false RED is cheaper than a false GREEN, but only once — the second time, the gate gets muted, and a muted gate is `L-C9` by another route (P3) |
| **L-C13** | **A migration branch that only existing databases take, so live-data proof cannot see it** | **1** | `ensure_columns` runs before the ladder and returns early when the table does not exist, so a **fresh** database is built entirely by the baseline rung — and `tasks`' baseline was the original `CREATE TABLE`, missing the 18 columns added since. New installs would have come up without `claimed_by_pid` and seventeen others while **every existing database looked perfect**. No amount of live-DB verification could have found it; an unrelated test creating a temp database did **(P1)** |
| **L-C14** | **A security assertion that checked "something changed" instead of "the secret is gone"** | **1** | The redaction pattern list ran `NAME=value` before the narrow token rules. On `Authorization: Bearer eyJhbGci…` the assignment rule stopped at whitespace, matched `Authorization: Bearer`, rewrote it to `Authorization=[REDACTED]` — and **left the token in the text**. The output looked redacted. Caught only because the test asserted `!out.contains("eyJhbGci")` rather than `out.contains("REDACTED")` **(P1)** |

## The lessons

**L-1 · When a source gives a multi-part verdict, adopt every part or record why not.** `G4b §A.14` said *"PORT redaction, REPLACE SecretStore."* The replace half became an ADR; the port half vanished, and nothing noticed for four revisions. → **A verdict with an `and` in it is a checklist, not a sentence.** The P8 gate now asserts that every capability marked `PORT` has a tested destination.

**L-2 · Regenerate the derived artifact in the same step as the source.** Three separate instances (L-C1) share one cause: I edited a ledger or a mechanics table and did not re-derive what hangs off it. → **§0.3's no-stale rule applies to *derived* documents as forcefully as to prose**, and the derivation is part of the edit, not a later sweep.

**L-3 · "Structurally impossible" is a claim about a mechanism, and the mechanism must be named.** An enum makes a state unrepresentable *in Rust*; it does nothing about an unconstrained `TEXT` column written by another process. → **Before writing "cannot", finish the sentence: *cannot, because <mechanism>*.** If the mechanism has a boundary, name the boundary too.

**L-4 · Generalising from one provider is a defect, not a simplification.** The cache-accounting formula was correct for Anthropic and wrong for three others, because their usage fields mean different things. → **Where four implementations exist, a claim about "the API" is four claims** until verified as one.

**L-5 · Reinterpreting an owner's answer is worse than asking again.** Narrowing ADR-02-02 to something defensible produced a spec that read well and did not say what Razi had said. → **When an answer conflicts with a rule, that is a conflict to surface (§0.13), never a wording to smooth.** This is now §0.2c.

**L-7 · Write the check against the invariant, not against the sentence that described it.** Three P0 gates were wrong on their first run, each for the same reason: I encoded the wording of the task rather than the property it protects. `.env` must not be **committed**, not must not **exist**. The vault rule is "the vault's IPC surface is closed", not "anything touching the vault returns no string" — the broad form flagged two correct commands, and *a gate that cries wolf gets switched off*. → **Before writing an assertion, say out loud what would have to be true for the system to be safe, and assert that.** If the check fires on correct code, the check is wrong, not the code.

**L-8 · A test that reads source files must exclude its own.** The vault gate listed the forbidden function names as literals and then scanned its own file for them. → **Truncate at `#[cfg(test)]`, or build the needle at runtime.** `V3` had flagged this exact shape hours earlier in someone else's code and I reproduced it immediately afterwards, which is the strongest argument that reading a lesson is not the same as applying it.

**L-9 · Verify a library's behaviour before asserting what it does.** Two `safe_filename` tests failed because I assumed `reqwest::Url` handed back raw path segments. It applies WHATWG normalisation: `..\..\evil.exe` collapses to `evil.exe` and the query string is gone before the allow-list ever runs. The code was right and my expectation was wrong. → **When a test disagrees with the code, the test is the more likely defect** — and the resulting test is better, because it now asserts the invariant (never escapes the directory) *and* documents the normalisation that makes it hold.

**L-10 · Reading this register is not applying it (L-C8).** The planning phase recorded, in this very file, that subagents must write incrementally because a rate limit had already destroyed four reports. I then dispatched both P0 auditors with no incremental-write instruction and lost both to a rate limit. → **A lesson only takes effect where it is enforced.** The dispatch template in §0.2i now carries the incremental-write clause, so it cannot be forgotten by whoever writes the next prompt — including me.

**L-11 · Verifying the consequence you intended is not verifying the change.** Twice in P0 I made a small, correct-looking edit and checked only what I meant to affect. `orch_healthy = false` routes to Mode 1 — *below* that line; a latch above it returned early from every tick. Widening the qc_review gate is right for workers — and the supervisor recipe promotes on their behalf. Both were found by an auditor, not by me, and both had my own contradicting comment nearby. → **Read the whole function you are editing, in both directions from the change, and re-read the callers the change newly applies to.** An audit that finds this class is worth more than one that finds none.

**L-12 · A check must be able to say "no". Look at its shape, not its result.** Three times in one phase I verified something with a construct that could only ever report success: `head -5` truncates before the failure, `grep "result: ok"` cannot match `result: FAILED`, and a string count answers a question nobody asked. Each returned green and each was blind. → **Before trusting a check, ask what its output would look like if the thing were broken.** If the answer is "the same, or nothing", it is not a check. Prefer exit codes to output matching; prefer a parser to a grep.

**L-24 · A gate that parses source must normalise what it parses.** Three of my gates locate a function body by splitting on a literal newline sequence, and every one of them silently changed meaning when git converted the working tree to CRLF — the split stopped matching, and the `unwrap_or` fallback widened the search from one function to the whole file. `S2-11` reported the tool-ordering path as using a `HashMap` when that path is `BTreeMap`-only; the symbol it found was 40 KB away in an unrelated function. → **Normalise on read, and treat a parse fallback as a failure rather than a default.** `unwrap_or(whole_file)` is the specific mistake: when a locator fails to locate, the honest outcome is *"I could not find the region"*, not *"I will check everything instead"*. A gate is only as trustworthy as its narrowest assumption about how the file reached the disk.

**L-21 · Writing the lesson down does not prevent committing it.** `L-18` — *a fix that lands in the right place and is never reached is not a fix* — was written into this register **during P2**, from `P2-57`. The post-phase audit then found **five more instances in the same phase**, three of them committed *after* the lesson was recorded: the stable/volatile prompt split (built, unit-tested, and the engine called the joining wrapper), the daily ceiling (correct, gate-proven, and unarmable because nothing called `setDailyBudget`), the vault backend disclosure (implemented, documented, and never called by any UI), `max_output` (added expressly to clamp `max_tokens`, read by nothing), and `ErrorClass` (defined, tested against itself, constructed by nothing). → **A lesson in a register changes nothing on its own; a lesson with a gate changes the build.** Every one of these was invisible to a green test suite and visible in ten seconds to `grep -r "<the thing>" --include=*.rs | grep -v "def\|test"`. The closeout checklist now asks that question of every capability the phase added, and the answer is a gate, not a note.

**L-22 · Ask the wire, not the code, and never the comment.** `D-9` — every Anthropic *streaming* call reporting **2× its real cost** — sat behind two comments in this repository that asserted opposite things about whether `message_delta` carries the input side. Reading either one would have produced a confident wrong answer; reading both produced a stalemate. One `curl` against the live API settled it in under a minute and showed the vendor sends the input side on **both** frames, so the engine's summation doubled it. It had survived a full fixture suite because the fixtures encoded the same assumption as the code. → **When two pieces of documentation disagree about a vendor's behaviour, neither is evidence.** Capture the wire. It is cheaper than the argument, and it is the only artefact that cannot be wrong about what the vendor does.

**L-23 · The test that would have caught it is the one nobody wrote, in the path nobody exercised.** The live smoke tested completions, streaming, caching, truncation, cost reconciliation and a full tool round trip — and never once called `test_connection`, the single function the UI uses to check a key. So Google's validation could point at a model retired in February 2026, 404 on every call, and pass every test in the suite. Combined with `P2-23`'s new refusal to store a rejected key, that made **a valid Google key unsaveable on the primary provider**. → **List the entry points, not the features.** A suite organised by feature will cover the interesting paths and miss the boring one that every user hits first.

**L-15 · A fixture is a photograph; it cannot see the vendor change.** P2 wrote 27 fixture tests across three adapters, all green, all correct — and the live smoke found four defects on its first run and a fifth on its second, every one of them invisible to fixtures by construction. `gemini-3.1-pro` **did not exist** (Google 404s it; the real id carries `-preview`), five seeded workers were pinned to models retired in February 2026, and Gemini had begun **rejecting any replayed tool call missing a `thoughtSignature`** — breaking every multi-turn Gemini tool conversation, on the provider that is primary for rollout 1. A captured payload predates the requirement and goes on passing forever; worse, its green result reads as *"the adapters work"*. → **Fixtures prove OUR code did not regress. Only a live call proves the VENDOR still agrees.** Ship both, run fixtures in CI, and run the live smoke deliberately at every phase closeout.

**W-3 · The pre-build refutation paid for itself a third time, and in a new way.** `§0.2m`'s
value in P2 and P3 A–E was catching false claims. In `P3-F` it caught a **prescription that would
have shipped as a fix**: `P3-33` said to disable `ort`'s default features, and `fastembed` already
does — so the change would have been a no-op wearing a fix's name, in a task whose entire purpose
is supply-chain hygiene. → The refutation brief is worth pointing at *"the prescribed remedy"* as
well as *"the claimed defect"*. A plan can be right that something is broken and wrong about what
would repair it.

**W-4 · A discriminating fixture beats an assertion.** `P3-31` claims scope is a pre-filter rather
than a post-filter. The cheap test — search in a scope, check the results are in that scope —
passes under **both** designs. The fixture that settles it gives the *other* scope both nearest
neighbours, so a post-filter over the global top-k returns nothing and a pre-filter returns two
rows including a far one. → **Design the fixture so the two hypotheses give different numbers**,
then read the number. It took ten extra minutes and it is the only reason that claim is proven
rather than asserted.

**L-34 · A gate I wrote proves what I chose to check. The register's gate proves what was
specified. Passing every one of the first is not evidence about the second.** At `P3-F` closeout I
scored `A3`'s Gates row ✅ and the phase at 100%, on the reasoning that every `TG-P3-*` test in
`ci_gate.rs` was green and every one had been falsified. Both facts were true. The Gates column
tracks `TESTING_GATES.md` — the **spec** register — the way `A1`'s row already said so explicitly
(*"`TG1-01`'s registration clause is unmet"*). A fresh-context verifier held the eight memory gates
to the register's own standard and returned **five NOT SATISFIED and three PARTIAL**, including one
finding no gate of mine could have produced: `memories_vec` is `SELECT`ed and **never created by any
migration**, so the vector leg does not exist and `P3-31`'s pre-filter proof runs against a
throwaway table the test builds itself. → **The gates I write are downstream of my own reading of
the task; the register is upstream of it.** When they disagree the register wins, and the honest
score is the register's. ⚠ The tell is the shape of the argument: *"every gate I wrote is green"*
is a statement about my own diligence, and I had reached for it as though it were a measurement.
Related `L-C6` — a check written from the claim rather than the invariant — one level up, in the
**scoring** rather than in a check.

**L-31 · The task that exists to prevent a defect class is where that class hides — and its
comment, its test and its gate will all agree with each other.** `P3-47` exists to stop a model
naming its own provenance. It shipped with `user_stated`, a **model-writable boolean** that
short-circuited the role check and handed any fleet worker `Proposer::User`: ceiling 1.0, source
`"user"`, `importance +0.3`, rendered into the interactive user's prompt as *"(you told me)"* and,
at `scope: "global"`, carried into every other repository. Three artifacts asserted the property
was held — a comment three lines above the read (*"never taken from the input"*), a unit test
asserting the escalation as **correct behaviour**, and a gate enumerating three forbidden schema
keys. → **Agreement between a comment, a test and a gate is worth nothing when all three were
written in one sitting from one misreading.** They are not three witnesses; they are one witness
quoted three times. The repair was not a better assertion but a **smaller signature**:
`from_agent_role(role: &str) -> Self` cannot be reached by any input, so the compiler enforces
what the three artifacts had only claimed. `L-27` generalised, and the reason `§0.2a`'s auditors
must not see my reasoning.

**L-32 · A boundary enforced in N−1 places reads as complete from inside any one of them.** Three
tier stores redact before insert, each carrying a comment naming the reason. The gateway — added
later, and the **only live writer into the table recall actually reads** — called no redaction at
all. From inside `episodic.rs` the boundary looked total; from inside `semantic.rs` it looked
total; the one file that would have shown the hole was the one file nobody compared against.
→ **Verify a boundary by ENUMERATING its members from the code, never by inspecting a member.**
`TG-P3-48` now discovers memory writers by scanning for inserts rather than listing them, so a
fifth writer fails the gate with no list to remember to update — which is the only form of this
check that survives the next phase.

**L-33 · A guard's failure mode is refusing correct work, and nobody falsifies guards.** The
destructive-migration backup verifier opened its snapshot **read-only** and ran
`PRAGMA integrity_check`. That cannot validate an FTS5 inverted index without write access, so it
returns *"attempt to write a readonly database"* rather than `"ok"` — and `memory.db` v2 had just
introduced `memories_fts`. The first destructive migration after it was therefore **refused on
every database that had ever reached v2**, with the safety mechanism reporting its own good backup
as corrupt and pointing whoever read the message at disk corruption that did not exist. It
surfaced only because a migration finally tried to run. → **Falsify the guards too.** Every
existing test in that file used a plain table, where `integrity_check` needs no write access — so
the whole suite passed on the one case that was never broken. A fixture that cannot reach the
failure is `L-C9` wearing a passing test's clothes.

**L-28 · A conclusion can be right while its stated cause is wrong, and that is the more
dangerous shape — it looks verified.** `context.rs` said memory was skipped *"when no session_id
is set (e.g. … recipe mode)"*. Recipes get no memory: true. They have no session id: **false** —
they build one. The real mechanism was the system-prompt override, which meant the actual fix
(`P3-45`) was somewhere nobody was looking, and a **user-facing tool description** repeated the
wrong cause for a phase and a half. → **When a comment explains WHY, check the why, not just the
whether.** A correct conclusion attached to a wrong mechanism survives every test of the
conclusion, and it sends the next person to the wrong file. Related `L-C3`, and `L-22` — the
source here was a comment, which is exactly what that lesson says not to trust.

**L-29 · A `cfg`-gated block that nobody compiles is not code, it is a draft.** The embedding
backend sits behind an off-by-default feature, so the default build type-checks **none** of it —
and `cargo check --features embeddings` passing means nothing on its own, because an inactive
`cfg` also passes. The only thing that distinguished them was planting a deliberate type error
inside the gated module and confirming it was caught. → **Verify a feature-gated path by making
it fail first.** The generalisation is `L-C9`'s: a check that cannot fail is not a check, and
"it compiled" is a claim about what the compiler *looked at*, not about what exists.

⚠ **A practical corollary found the same day:** `ORT_STRATEGY=system` let `cargo check` type-check
the whole gated module **without** the ~30 MB third-party download that enabling the feature
normally triggers. A supply-chain surface and a verification requirement are not always the same
thing, and it is worth five minutes looking for the seam between them before accepting either
"ship it unverified" or "fetch the binary".

**L-30 · My own tests are the first concurrent users of anything process-global.** `P3-34`'s fix
manipulates `HF_HOME`, and `std::env::set_var` is process-wide. The first version had no lock,
and its two tests **raced each other into a failure — twice**: once because the helper had no
mutex, and again because a test mutating the environment OUTSIDE that mutex can be clobbered by
one inside it. → **When a fix touches global state, the test suite is the load test.** Both
failures were free and immediate; the same race in production would have been a model landing in
the wrong directory on someone else's machine, once, unreproducibly. `cargo test` runs in
parallel by default, and that is a feature.

**L-27 · A gate written in the same sitting as the fix inherits the fix's misreading.**
The P3 post-phase audits found two of my own gates green over live defects, and both failed
the same way: each asserted **the shape of the change I had just made** rather than the
property the change existed to establish. `TG-P3-11` asserted that `is_read_only` no longer
appears in the executor — true, and silent about the fact that the executor now prompted
**twice**. `TG-P3-58` asserted a predicate rewrite (`d >= 3` → `d + 1 > 3`) that is, for
`u32`, **the same predicate** — so it gated a no-op and called it a boundary change. →
**Write the gate from the task's SENTENCE, not from the diff.** "Exactly one gate" and "one
boundary in one place" are both checkable, and neither is what I checked. A practical tell:
if the assertion would still pass after reverting the fix and applying a *different* fix for
the same defect, it is keyed to the diff. Related: `L-C6` (count now **9**), and `§0.2i`'s
existing rule that tests for a rule are written from the rule's own wording by an agent that
has not seen the implementation — which is exactly what these two gates were not.

⚠ **A second shape from the same audits:** `TG-P3-58` **enumerated** the two files it knew
about, so it never examined the third guard in `spawn_qc_review.rs` — written in the very
spelling its own failure message called a defect. A gate that lists its subjects can only
ever find the ones somebody already thought of. **Sweep the directory; assert the sweep
reached a plausible count.** The gates that did this (`TG-P3-14`, `TG-P3-20`) each found a
site I had not listed.

**L-26 · A falsification whose planted defect does not compile proves nothing.**
Planting a defect and watching the gate go red is only evidence if the gate actually
*ran*. Two of `TG-P3-55`'s three plants were syntactically invalid — one renamed a type
out from under its impl, one left a variable undefined — so the run failed at
**compilation**, exited 101, and looked exactly like a gate firing. → **A falsification
must assert two things: that the run failed, AND that it failed for the stated reason.**
Print the expected reason-substring alongside the exit code and require both; the plant
must be a defect a person could plausibly commit, which by definition compiles. The
generalisation: `exit != 0` is not a measurement of *what* failed. Same family as `L-14`
(gate on the exit code, never on a verdict you wrote) and `L-C9` one level up — here the
check was sound and the **harness proving the check** was the thing that could not
express failure.

⚠ **The same harness lied a second way, in the opposite direction.** Restoring a planted
file with `shutil.copy2` preserves the backup's mtime, so the restored file can look
OLDER than the build artifact produced from the planted one — and `cargo` then reuses
that artifact. In `TG-P3-59` this surfaced as a post-restore FAILURE over a correctly
restored file (harmless, if confusing). The dangerous direction is the mirror image: a
restore that leaves the plant on disk under an old mtime reuses the pre-plant artifact,
the gate passes, and I conclude the file was restored when it was not. → **`touch` the
file after any restore, and verify the restore by reading the file, not by re-running the
gate.** State is checked against state (`[[verify-state-against-state]]`), not inferred
from a build that may not have happened.

**L-25 · A corpus that contains the claim cannot corroborate the claim.** `TG-P3-20`
asks whether each value a tool description enumerates actually exists in the code. Scoped
to the tool's own file it produced false positives (`assign_task`'s worker list correctly
lives in `recipes/defaults.rs`), so I widened the search to every `.rs` in the crate — which
includes the description being checked. Every value then "existed", and a planted
`worker-ghost` sailed through. → **When a check compares a claim against a body of
evidence, subtract the claim from the evidence first.** The tell is a widening made to
silence a false positive: each widening must be re-falsified, because the fix for a
too-narrow check is the standard way to build a check that cannot fail. Related: `L-C9`
(shape cannot express failure) and `L-C15` (scope silently widened by the environment) —
this is the same widening, done deliberately by me rather than by `core.autocrlf`.

⚠ **It recurred in `P3-F`, in a gate written the same day and by the same hand.** `TG-P3-49` asserts the memory block frames its contents as data, by searching `recall.rs` for `"not instructions"` — and `recall.rs` contains `injection_tests`, which asserts that exact string. Falsifying the gate is the only reason it was found: the plant changed the production text, the test module still carried the phrase, and the gate stayed green. Both gates added this round now cut the file at `#[cfg(test)]` before scanning. → **A source-reading gate must exclude the tests, always** — the tests are where the claim is written down in the clearest possible words, which is precisely what makes them the worst possible corpus.

**L-16 · A test that fails when the feature works better than expected is a bad test.** `p2_16` asserted `cache_write > 0` on the first of two calls and failed on its third run — reporting `cached_read: 11923, cache_write: 0`. Nothing was broken: the entry written minutes earlier was still inside its five-minute TTL, so the "first" call *hit* it. The assertion had encoded **a cold-cache assumption rather than the invariant**. → State the invariant, not the state you happened to observe while writing it: *wrote or read, never both zero*. The same error one level up is `L-C9` — a check whose shape cannot express the failure it is meant to catch.

**L-17 · The dangerous direction of an estimate is the one you did not think about.** `estimate_tokens` was documented as "deliberately biased LOW", with a paragraph explaining why that was the safe error. Measured against `count_tokens`, it runs **both ways**: −29% on the real system prompt, **+5%** on a repetitive probe. The over-estimate is the harmful one — it tells the cache monitor a prefix cleared the minimum when the provider silently declined it, so the monitor raises an alarm with no fault behind it, and a muted alarm is worse than none. → An estimate used for a *decision* needs a margin sized to its measured error, not a confident sentence about which way it errs. `MINIMUM_MARGIN` exists because a claim in a docstring was refuted by a measurement.

**L-18 · A fix that lands in the right place and is never reached is not a fix.** `P2-57` changed compaction to read `context_window` from `models()` instead of a hardcoded 200,000 — correct code, in the correct place, and **inert for eleven of the twelve models the app offers**, because the adapters' model lists had drifted a generation behind the picker and the lookup returned nothing. Gemini still compacted at a fifth of its real window. Nothing failed; the fallback simply absorbed it. → When a fix depends on a lookup, prove the lookup **hits** for the values that matter. The catalogue test now pins `gemini` at 1,000,000 *and* pins that an unknown model still gets 200,000, so a silent regression to the fallback fails loudly.

**L-19 · Two lists that must agree will disagree, and the second one is always invisible.** P2 found the same shape five separate times: two model pickers (`WorkerTable` vs `WorkerCreateModal`), the price table vs the picker, the backend catalogue vs `models.ts`, **three** copies of the provider-routing ladder (then a **fifth** that `P2-53` had missed, whose own tests asserted routing for two removed models), and three independent "daily budget" numbers of which only one enforced anything. → One registry, everything derived from it, and a gate holding any unavoidable second copy against the first. Where a second copy cannot be removed, the gate is not optional — it is the only thing that will notice.

**L-20 · "Written by the UI and read by nothing" is a whole defect class, not an anomaly.** Four P2 tasks turned out to be the same shape: `ModelConfigsDb` had a table, four commands and a Settings panel writing temperature, top_p, reasoning effort and thinking budget — and every request was built with those four fields hardcoded to `None`. The active provider was written to memory and never persisted. The daily budget was displayed but unsettable. `ErrorClass` was defined, unit-tested against itself, and **constructed by nothing**. Each is worse than not offering the feature: the control moves, the value persists, the panel redisplays it, and the user attributes the unchanged behaviour to the model. → For anything written, name the reader and prove it reads — `§0.7`'s *prove the reader reads it*, applied to configuration as well as to data.

**L-14 · Never chain an action after a check with `&&`; gate on the exit code, and never echo a verdict a command did not produce.** Three times in P2 I ran a suite, printed its exit code, and committed in the same `&&` chain — which always continues, because `echo` succeeds regardless of what it prints. Twice that pushed a red commit. The third time I ran `grep -E "FAILED"; echo "(none=green)"`, so the failure was printed and my own next line contradicted it. → **Run the check to a log as its own command. Read `$?`. Decide. Then act.** The tell is a verdict that appears in the output without any command having computed it — `(none=green)`, `(0=clean)`, `(clean)` — every one of those is a sentence I wrote, not a measurement. `L-C9` is about checks that cannot fail; this is the same defect one level up, in the *harness around* a check that can.

**L-13 · A gate that keeps failing is telling you something; "it has never worked" is not a reason to delete it.** The e2e job failed three times and I proposed removing it as noise. Its error moved every run, and the third failure carried the reason: the application was crashing at startup, from a regression I had introduced, which nothing else in CI could observe because every other job stops at `cargo build`. → **Before removing a failing check, read the last failure properly and ask what it is uniquely positioned to see.** Here the answer was "the only thing that runs the binary", which is exactly why it kept failing. The correct response to a noisy gate is usually a cheaper gate for the same property — a 20-second launch smoke, in this case — not silence.

**L-6 · Verify a subagent's claim before it becomes a row.** A report said the toast system did not exist. It does — the store and renderer are correct and only two producers are broken. → **The correction changed the fix from "build a notification system" to "fix two imports".** A finding taken at face value would have sized a task wrongly by an order of magnitude. This is now §0.2a's second warning.

## What worked — method worth repeating

- **Brief reviewers to *refute*, not to confirm**, and tell them to default to "refuted" when evidence is ambiguous. The three-reviewer pass found 104 findings; a "please review" brief would have produced agreement.
- **Name the plan's own weakest claim and point an agent at it.** I told a reviewer that `sqlite-vec` × `rusqlite` was the claim I least trusted. It **held** — and that negative result is worth as much as a finding, because it is now evidenced rather than assumed.
- **Make subagents write incrementally.** A rate limit destroyed four reports mid-flight; every re-dispatch was told to create the file first and append. Nothing was lost after that.
- **Record overturns in place, struck, never deleted.** Six of my claims were overturned during planning. Each is still visible with its correction beside it, which is what let the count in L-C1…L-C5 exist at all.
- **Split one large audit into lenses.** Buildability, contradictions, and gaps found almost entirely disjoint sets. One generalist reviewer would have found the shallow union.

---

# §1 · Current state assessment

## 1.1 What exists — `CODE-PRESENT`

| Surface | Path | Size | State |
|---|---|---|---|
| Tauri backend | `src-tauri/src/` | 53,387 LOC, 32 modules | 256 Tauri commands, all registered |
| Frontend | `src/` | 39,474 LOC | 9 tabs, 20 component dirs, 28 stores |
| CLI | `src-cli/src/` | 3,508 LOC | `airsctl`; agent commands are 100% sidecar HTTP |
| Shared | `src-shared/src/` | 1,073 LOC | ~~Duplicates **four** tables by hand~~ → **resolved in `P1-09`**: `tasks`, `workers` and `plans` deleted (22/19/23 write statements); `airsctl` uses `autodev_core::db`. Only the read-only `analytics.rs` remains, which holds no `CREATE`/`INSERT`/`UPDATE`/`DELETE` and so cannot drift a schema |
| Sidecar | `submodules/core-orchestrator` | 225 MB bundled | **DELETE (P8)** |
| Memory MCP | `submodules/memory-mcp-server` | — | **DELETE (P8)** |

## 1.1a The submodule registration is broken — verified state

Razi flagged this, and it is verified rather than assumed (§0.0). **The two submodules are in a half-registered state, and it changes how P8 must delete them.**

| Check | Result |
|---|---|
| Gitlink entries in the index | **0** — `git ls-files -s submodules/` is empty |
| Gitlinks in `HEAD` | **0** — `git ls-tree HEAD submodules/` is empty |
| Ever committed | **Never** — `git log --all -- submodules/` returns nothing |
| `.gitmodules` | **Tracked and committed**, declaring both submodules |
| `.git/modules/submodules/` | **Present**, holding both object stores |
| `submodules/*/.git` | Present as **pointer files** into `.git/modules/` |
| Gitignored | **No** |

**What happened:** `git submodule add` ran locally — creating `.gitmodules`, the object stores and the working trees — but **the resulting gitlinks were never committed**. So `.gitmodules` describes two submodules that this repository has no index or history record of.

**Four consequences, all of which change P8:**

1. **`git submodule deinit` will fail.** It operates on submodules registered in the index; there are none.
2. **`git rm submodules/…` will fail.** Nothing is tracked to remove. *(An earlier revision of this plan said P8 would be "a `git rm` and a `.gitmodules` edit". That was assumed, not checked — see §0.16 L-C3.)*
3. ✅ **No history rewriting is needed.** The sidecar content was never in this repository's history, so deletion leaves no large blobs and needs no `filter-branch`. **This materially simplifies P8.**
4. ⚠️ **`.git/modules/submodules/` is a "survives by address" trap.** Deleting the working tree alone leaves both object stores behind — the same class as the MCP files living in sidecar-named paths. P8's exit condition covers content *and* address.

⚠️ **Live hazard, today:** 225 MB sits untracked and **not ignored**, so `git add -A` would commit the entire Python tree. §0.9's stage-deliberately rule is what prevents it; it is now also named in §0.8.

**Live schema head:** ⚠ **Superseded by P1 (2026-09-01).** **All 10 databases** carry a `PRAGMA user_version` ladder, applied by `db/migrate.rs`; the graph store is at **v6**, and the rest at v1 (`config.db` at v2 after adopting `checkpoints`/`hooks`). **One data root** (`~/.autodev/`) since `P1-14`. `LIVE-DB-PROVEN` with zero row-count changes.

*The state that motivated the work, kept for the record:* only `graph/` had a real ladder — head v5, whose statements were invalid SQLite and swallowed — every other database used `CREATE TABLE IF NOT EXISTS` on open, and there were 11 database files across 3 data roots.

## 1.2 What does NOT exist — `GAP`

| # | Gap | Evidence |
|---|---|---|
| GAP-01 | Any MCP **client** — `rmcp` is compiled server-features-only and cannot dial out | `G12` |
| GAP-02 | ~~Any **xAI/Grok** provider~~ | `G2b P-28` — **CLOSED-AS-WONTFIX 2026-09-01** (razi-ruled): the provider set is Anthropic, OpenAI, Google. A gap only matters against an intended capability, and xAI is no longer one. |
| GAP-03 | Any **prompt caching** on any provider | `G2a T1` |
| GAP-04 | Any **embeddings or vector search** — zero hits repo-wide | `G1 §C.4 P3` |
| GAP-05 | Any **frontend test framework** — no vitest, jest, playwright, no `test` script | `G0 §2` |
| GAP-06 | Any **ESLint** — `npm run lint` is `tsc --noEmit` | `G0 §2` |
| GAP-07 | Any **macOS CI job** | `G0 §2` |
| GAP-08 | Any **Rust-native compile/verify step** — all validators shell out | `G3d` |
| GAP-09 | Any **run/launch/handoff** for produced artifacts | `G3d F-DEL` |
| GAP-10 | Any **dynamic tool registration** — `ToolRegistry` is frozen after construction | `G12` |
| GAP-11 | ~~Any **backup path** before destructive operations~~ → **CLOSED in `P1-19`**: `db/backup.rs` (`VACUUM INTO`, integrity-checked, refuses to overwrite), wired to `run_ladder`, `TasksDb::clear_all` and `delete_graph`. A failed backup **stops** the operation, and the refusal branch is tested | `G1 §E.3` |
| GAP-12 | Any **retry policy** — `ErrorClass::Retryable` has no reader | `R3` |
| GAP-13 | Any **cadence** — `interval`/`backoff`/`max_retries` appear zero times in 13 documents | `R3` |
| GAP-14 | Any **spend ceiling or kill switch** | `R3` |
| GAP-15 | ~~Working **undo** — `db/checkpoints.rs` is not compiled into the binary~~ → **RETRACTED (`ADR-LOG-RESTRUCT-0010`).** Undo **ships**: `git/checkpoint.rs`, five registered commands, called from `gitStore.ts`, snapshot before every agent write. `db/checkpoints.rs` was deleted, not declared (`ADR-0012`). The real gap is `M-43` — a failed checkpoint is logged at `debug!` and the write proceeds anyway | `G3d F-44` (superseded) |
| GAP-16 | ~~Any **redaction** — the module grounding ruled PORT was dropped~~ → **CLOSED in `P1-20`/`P1-21`**: both Python redactors ported (`SENSITIVE_KEYS` is **22**, not 20), wired at the tool-result funnel, all four memory writes, the log appender and analytics. ⚠ The first wiring was to a method with no production caller — see `_verify/P1-AUDIT-TRIAGE.md` `A-01` | `R3`, M-40 |
| GAP-17 | Any **fault isolation** — `panic = "abort"` + in-process agents | `R1 B-04` |

## 1.3 Consume-not-build ledger — deliberate non-goals

| We do NOT build | Because |
|---|---|
| A vendor agent SDK wrapper | No first-party Rust SDK exists for any of the four; all 14 cost-relevant features are plain HTTP (ADR-01-01, ADR-01-04) |
| A multi-provider LLM crate | No crate implements compaction, context editing, or per-model cache minimums (`G6 §5.4`) |
| PostgreSQL + pgvector | Would re-create the bundled-server defect the programme removes (ADR-05-01) |
| A Redis warm tier | Already not a dependency; a second cache tier exists to share state between processes (ADR-04-04) |
| An MCP server | Existed solely to serve the sidecar (ADR-03-02) |
| An MCP client | Deferred with its own threat model (ADR-05-06) |
| Provider file search / hosted RAG | Ships user code to a vendor against the local-first non-goal (G-10) |
| A plugin installer | Reduced to environment **verification** (ADR-03-03) |
| Delta update patching | Non-functional in every direction; full-installer updates are correct (spec `09 §3.3`) |
| Multi-machine fleet coordination | One app, one machine |

---

# §2 · Substrate capabilities audit

Re-verified at the start of **every** phase (§0.12). The exact surfaces consumed:

```rust
// Persistence — statically linked, no system dependency
rusqlite = { version = "0.31", features = ["bundled"] }   // src-tauri/Cargo.toml:41
// libsqlite3-sys 0.28 bundles with -DSQLITE_ENABLE_FTS5 and exports sqlite3_auto_extension
// sqlite-vec 0.1.9 compiles ONLY sqlite-vec.c with -DSQLITE_CORE, ships no sqlite3.c,
//   has zero runtime deps, dev-depends on rusqlite 0.31.0 — VERIFIED NO SYMBOL CONFLICT (R1)

// Credentials
keyring = "2"                        // src-tauri/Cargo.toml:44
// Linux needs D-Bus Secret Service; ABSENT in CI (ci.yml:64) → encrypted-file fallback (G-06)

// HTTP — the only client, used for all three providers
reqwest = { version = "0.12", features = ["json", "stream"] }

// Git, graph, watchers
git2 = "0.19"; tree-sitter = "0.22"; petgraph = "0.6"; ignore; notify = "6"

// Release profile — CHANGES IN P3 (M-41)
[profile.release] panic = "abort"    // Cargo.toml:126-131  → becomes "unwind"
```

**Tauri 2.11** with **two** registered plugins (`shell`, `dialog`). ⚠ **Corrected:** the `dialog` and `fs` blocks were *not* simply dead config. Registering `tauri-plugin-dialog` in P0-E made Tauri parse the `"dialog": {}` block, whose config type is a unit — `PluginInitialization("dialog", "invalid type: map, expected unit")` — and **the application stopped starting on every platform**. The key was removed; the plugin stayed. Only the e2e job could see it, because every build smoke stops at `cargo build` (`L-C10`).

---

# §3 · Target architecture

```
                        ┌──────────────────────────────┐
   AutoDev.app          │  autodev-core (new crate)    │        airsctl
   (Tauri binary) ─────▶│  AgentEngine · ToolRegistry  │◀───── (CLI binary)
                        │  Provider trait · Memory     │
                        └───────────────┬──────────────┘
                                        │
              ┌─────────────┬───────────┼───────────┬──────────────┐
              ▼             ▼           ▼           ▼              ▼
        4 providers    ToolRegistry   Memory     Fleet         Code graph
        (HTTP)         (in-process)   (SQLite+   (events)      (SQLite)
                                       vec+FTS5)
```

**One agent runtime, linked by two binaries** (INV-1 restated, G-02). No child interpreter. No localhost agent API.

## §4 · Module tree, each leaf tagged with its creating phase

```
autodev-core/                       [P1]  ← new crate, extracted
  agent/{engine,context,compaction,tool_executor,permission}.rs   [P1 move, P3 rework]
  provider/{traits,anthropic,openai,google,streaming}.rs          [P1 move, P2 rework]
  memory/{db,episodic,semantic,procedural,gateway,recall,embed}.rs[P1 move, P3 rework]
  events.rs                         [P1]  ← EventSink trait: the host boundary
  db/ · graph/ · git/ · recipes/ · hooks/ · subagent/ · routing/     [P1 move]
  analytics/ · budget/ · cost/ · vault/ · state/ · session/          [P1 move]
  observability/ · plugin/ · diff_analysis/ · errors.rs             [P1 move]
  platform/command.rs               [P0]  ← intent → platform-correct argv
  security/redaction.rs             [P1]  ← M-40, the dropped port
  verify/                           [P4]  ← Rust-native build verification (M-13)
src-tauri/src/                            ← the desktop shell, host-bound only
  commands/                         [existing]  255 #[tauri::command]
  updater/ · mcp/ · skills/watcher.rs · main.rs · lib.rs
  ci_gate.rs · reachability_gate.rs  [P0]
  sidecar/                          [P8 DELETE — 3,161 LOC]
  mcp_server/{server,tool_adapter,port_status}.rs [P0 DELETE — 705 LOC]
  mcp_server/health_monitor.rs      [P0 MOVE out of the sidecar-named tree]
src/                                [P6 anchor, P7 experience]
```

> **Corrected 2026-09-01 (`ADR-LOG-RESTRUCT-0015`).** This tree previously left
> `db · graph · git · recipes · hooks` in `src-tauri`, which cannot hold: `agent/`
> makes 63 references into exactly those modules, so moving `agent/` into core
> — ruled in `ADR-LOG-RESTRUCT-0011` — forces them across. `§3`'s diagram and
> `INV-1` already place Memory, Fleet and Code graph **below** core; `§4` was the
> outlier. Full trait inversion was measured (**91 methods across 10 traits**,
> each with exactly one implementation) and rejected as a P3-scale rework of a
> runtime `§4` itself tags *"P1 move, P3 rework"*. The governing criterion is
> **"does the engine own this, or does the host?"** — not *"does it import
> `tauri`"*.

---

# §5 · Phase table and sequencing rationale

## §5.0 · Two rollouts (Razi, 2026-09-01)

AutoDev ships **twice**, and the first shipment is not the standalone application.

| | Rollout | The sentence that defines it | Audience |
|---|---|---|---|
| **R1** | **Backend for AutoDesign** | *"The backend is ready, wire it to AutoDesign."* | the AutoDesign team |
| **R2** | **The application** | *"The entire application is ready for usage."* | end users |

R1 is not a subset of R2 with pieces missing — it is a **complete product with a
different consumer**. AutoDesign supplies the UI, the file, the user and the
Apply/Reject review surface; AutoDev supplies ingestion, fidelity, export, adaptation
and changes, driven by a fleet with real memory. R2 then adds the parts that only
matter when a human drives AutoDev directly: the UI, the graph, first-run experience
and signed installers.

**Nothing is deferred.** Every phase below has a slot. Where a phase was previously
marked "deferred", it has been re-seated, not dropped.

## §5.1 · The R1 ordering, and why each phase sits where it does

| # | Phase | Why here | Tasks |
|---|---|---|---|
| ✅ | **P0** Stop the bleeding | done | 12 |
| ✅ | **P1** Structural | done | 22 |
| 1 | **P2** Providers | Everything downstream makes model calls. **Gemini primary** (`gemini-3.5-flash` + 3.5 flagship); OpenAI and Anthropic follow in the same phase | 27 |
| 2 | **P3 A–E** Runtime | The loop that will execute every design tool: fault isolation, one tool path, bounded context. Conversion holds a full layer bitmap set for 25–35 s, so `panic = "abort"` must die **before** the design phases, not after | 29 |
| 3 | **P3-F** Memory | *"the agent needs proper guidance and memory as well, so memory is a no brainer."* Placed before the design phases deliberately: memory changes context assembly, and every design tool built earlier would need re-testing against the change | 15 |
| 4 | **P8** Sidecar deletion | **Moved into R1, before handover** — *"we dont want them to have an unused sidecar on the backend."* Now feasible: `OrchestratorProbe` already has a null implementation (`NoOrchestrator`) from P1, `skip_sidecars` already exists, and `P3-F` replaces `memory-mcp-server`, which was the last live dependency | 24 |
| — | 🤝 **HANDOVER** | The AutoDesign team receives a codebase with **no sidecar in it** and a published contract. See `§5.3` | — |
| 5 | **PD1** Ingestion | PSD **and** PDF → the IR. The v4.7 operator-list pipeline is the source | 18 |
| 6 | **PD2** Fidelity | *"visually it looks okay but in reality pixel by pixel wise is not."* Ingestion is not done until something can **prove** it, so this follows PD1 immediately | 14 |
| 7 | **PD3** Export | webpage · PDF · back to PSD. Needs a fidelity measure to know an export is faithful, so it follows PD2 | 15 |
| 8 | **PD4** Adaptation | Deterministic geometry tool. Needs export to see its result and fidelity to grade it | 13 |
| 9 | **PD5** Changes | Reverses the PSD **construction** the source pipeline already performs. Needs PD3's writer | 13 |
| 10 | **PD6** Brand guideline | Consumes PD5's change primitives — a rebrand *is* a batch of changes driven by a guide | 9 |
| 11 | **P4** Fleet | *"including the fleet."* Last in R1 because a fleet multiplies whatever the tools already do; multiplying unproven tools only multiplies the debugging | 30 |
| 12 | **R1 close** | Integration, post-phase audit (`§0.2a`), contract v1 | — |

## §5.2 · The R2 ordering

| # | Phase | Why here | Tasks |
|---|---|---|---|
| 13 | **P5** Graph | The first thing the *standalone* product needs and the design path never did: context scoping over a real codebase | 21 |
| 14 | **P6** UI | In R1 AutoDesign **was** the UI. Here AutoDev grows its own | 28 |
| 15 | **P7** Experience | Only meaningful once there is a UI to have an experience in | 18 |
| 16 | **P9** Delivery | Signing, installers, the capstone. **Certificate procurement has lead time — start it at R1 close, not here** | 16 |

## §5.3 · The handover point, stated precisely

**When:** after **P8**, before **PD1**.

**Why not earlier:** Razi's ruling — the AutoDesign team must not inherit a backend
carrying 3,867 lines of dead sidecar, 17 dead commands and a 225 MB bundle resource.

**Why not later:** everything after P8 is capability the contract already describes.
Making them wait for PD1–PD6 would idle them for the largest stretch of the programme.

**What they receive:**

1. A codebase with **no sidecar** — verified by content *and* by address (`P8-21`).
2. The **capability contract** (`PD0`), written to AutoDesign's own versioned shape.
3. A running AutoDev that answers the contract with **stubs** — every operation
   returns a valid, correctly-shaped response before it does the real work.

Point 3 is what makes the parallelism honest. A contract nobody has exercised is a
document; a contract with a live stub behind it is an integration they can build and
test against on day one. `PD0` is therefore scheduled **early and separately** — its
schema half can be written and shared at any time, because it depends only on the
operation set, which is already decided.

## §5.4 · Standing sequencing rules (unchanged)

Land **all migrations in the earliest persistence phase as one linear unit** — no
later phase renumbers. Build the **run/audit ledger early** so every later act can
stamp its run id. Sequence a phase after its data producer. Put the capstone last.

## §5.5 · The full phase table

| Phase | Rollout | Headline outcome | Depends on |
|---|---|---|---|
| **P0** | — | No unverified update installs · no command returns a key · the `/mcp` RCE is gone · gates can fail | — |
| **P1** | — | Everything shipped is in the binary · one schema owner · `autodev-core` exists · redaction restored | P0 |
| **P2** | R1 | Three providers conform · caching measurably works · cost is honest · **spend has a ceiling** | P1 |
| **P3 A–E** | R1 | Context is bounded · one tool path with a real gate · subagents in-process · a panic cannot kill the app | P2 |
| **P3-F** | R1 | Memory works and is **proven read** | P3 A–E |
| **P8** | R1 | Not one line of sidecar code survives, by content **or** by address | P3-F |
| **PD0** | R1 | The capability contract is published and answered by a live stub | P2 — ⚠ **not P8**; runnable now, and the AutoDesign team is blocked on it |
| **PD1** | R1 | A PSD **or** PDF becomes an IR that describes the design exactly | P8 |
| **PD2** | R1 | Fidelity is **measured**, not asserted — and refined until it passes | PD1 |
| **PD3** | R1 | The IR exports to webpage, PDF and PSD | PD2 |
| **PD4** | R1 | A design is reformatted to any target size by a **deterministic tool** | PD3 |
| **PD5** | R1 | Text, image, position, colour and shape can be changed and written back | PD3 |
| **PD6** | R1 | A brand guide is ingested and applied as a batch of changes | PD5 |
| **P4** | R1 | A task runs end to end and lands a build-verified, reversible artifact | PD6 |
| **P5** | R2 | The graph is compiled, correct across updates, and **read** for context scoping | P4 |
| **P6** | R2 | The anchor is implemented · no event is lost · every setting persists | P5 |
| **P7** | R2 | A new user reaches a working agent turn without documentation | P6 |
| **P9** | R2 | Signed, verified installers for Windows, Linux and macOS | P7 |

## §5.6 · Measured throughput — what the phases actually cost

⚠ **This section exists because the estimates lived nowhere near the plan.**
Until now the only per-phase hour figures in the programme were in the
*public brief*, which is a condensation and has no business being the sole home
of a planning number. It went stale unchallenged for three phases. The numbers
belong here, next to the tasks they are derived from, and the brief renders
them like everything else.

### Method, stated so the figures can be argued with

Both columns come from commit timestamps — nothing is recalled or estimated.

* **Span** — first commit of a phase window to its last. What a calendar shows.
* **Active** — the same, minus every gap longer than **45 minutes**. The
  threshold is stated rather than tuned: in dense stretches the commit cadence
  here is 10–20 minutes, so a multi-hour gap is a break, not silent work.

Active is a **lower bound on effort**: any real work that produced no commit
for 45 minutes is discarded by this method. It is the right bias — an estimate
built from a number that flatters the estimator is worthless.

Task counts are the **MDP's own checkbox counts**, not the brief's table, which
was stale for three completed phases (it still recorded P0 at 12 tasks; P0 grew
to 27 during its own pre-build refutation).

### Measured, `P0` → `P3-F`

| Window | Tasks | Span | Active |
|---|---|---|---|
| Anchor set (planning) | — | 0h 03m | 0h 03m |
| **P0** build + closeout + audit | 26 | 11h 35m | **4h 43m** |
| **P1** build + audit + rulings | 22 | 6h 19m | **3h 57m** |
| AutoDesign sizing + the layer IR | 4 *(PD1)* | 0h 20m | 0h 20m |
| **P2** re-cut + build + audit | 74 | 14h 15m | **5h 32m** |
| **P3 A–E** build + audit | 39 | 5h 51m | **4h 50m** |
| **P3-F** build + audit + closeout | 18 | 2h 54m | **1h 58m** |
| **Total** | **183** | **≈49h calendar** | **21h 23m** |

**8.6 tasks/hour, all-in** — build, smoke, gates, audits, remediation, trackers
and commits included. Two days of calendar; twenty-one and a half hours of work.

### The planning rate is deliberately lower than the measured one

A phase's task count **grows during its own pre-build refutation**, and the
estimate has to be against the count as *written*, because that is the only
number available when the estimate is made. Observed growth: `P2` **27 → 74**
(×2.7), `P3 A–E` **28 → 43** (×1.5), `P3-F` **15 → 18** (×1.2). The trend is
downward — the plan gets better as more of the codebase is known — but it is
not 1.0 and will not be.

So the figure to plan with is **6 tasks/hour against the written count**
(≈10 minutes per written task), which carries the growth, the audit and the
closeout inside it. Checked against the three phases that had both a written
count and an outcome:

| Phase | Written | Old estimate | This model | Actual |
|---|---|---|---|---|
| `P2` | 27 | ~11 h | 4.5 h | **5h 32m** |
| `P3 A–E` | 28 | ~12 h | 4.7 h | **4h 50m** |
| `P3-F` | 15 | ~7.5 h | 2.5 h | **1h 58m** |

Two within 35 minutes, one conservative by an hour. The **old** estimates were
built on an unstated ~2.3 tasks/hour and were wrong by 2.5–3.8×, in the
direction that matters least for trust and most for planning: everything landed
early, so nothing was ever caught, and the figure quietly stayed wrong.

### Recalibrated remainder — 319 open tasks

⚠ **Re-cut 2026-09-03**, after Razi's pre-P8 review folded the investigation's
findings into the plan. The remainder grew from 223 tasks to **319**, and
`P8` grew from 24 tasks to **65** — it now carries the live defects, the prompt
floor, the procedural-memory seam, the Live Monitor restoration, the `airsctl
agent` retirement and the complete legacy conversion, not just the sidecar's
deletion. The rate is unchanged; only the work is.

| Phase | Open | Prev. est | **New** | Confidence |
|---|---|---|---|---|
| `P0-21` e2e harness | 1 | *blocked* | *blocked* | Environment, not throughput |
| **P8** Conversion + defects | 65 | ~4 h | **~11 h** | ⚠ Medium — 21 of its tasks are prose or frontend, where no throughput has been measured |
| **PD0** Contract | 7 | ~1 h | **~1 h** | High |
| **PD1** Ingestion | 21 | ~2.5 h | **~3.5 h** | ⚠ Medium |
| **PD2** Fidelity | 27 | ~2.5 h | **~4.5 h** | ⚠ Low — a research loop, and now carrying the renderer |
| **PD3** Export | 15 | ~2.5 h | **~2.5 h** | ⚠ Medium |
| **PD4** Adaptation | 33 | ~2 h | **~5.5 h** | ⚠ Medium |
| **PD5** Changes | 13 | ~2 h | **~2 h** | ⚠ Medium |
| **PD6** Brand guideline | 10 | ~1.5 h | **~1.5 h** | ⚠ Medium |
| **P4** Fleet | 44 | ~5 h | **~7.5 h** | High — Rust, and the shape is known |
| **Rollout 1** | **236** | **≈23.5 h** | **≈39.5 h** | |
| **P5** Graph | 21 | ~3.5 h | **~3.5 h** | High |
| **P6** UI | 28 | ~4.5 h | **~4.5 h** | ⚠ **Low — no measured frontend throughput** |
| **P7** Experience | 18 | ~3 h | **~3 h** | ⚠ Low — same reason |
| **P9** Delivery | 16 | ~2.5 h | **~2.5 h** | ⚠ **Not throughput-bound** — gated on Razi's signing keypair and on CI across three platforms |
| **Rollout 2** | **83** | **≈14 h** | **≈14 h** | |
| **Programme** | **319** | **≈37.5 h** | **≈53 h** | |

### Handover is three milestones, not one

🔴 **Corrected 2026-09-03, on Razi's challenge.** This plan and the public brief
both carried a single row reading *"HANDOVER — the AutoDesign team can begin
wiring"*, placed after `P8`. He asked the obvious question: how can they wire
against a contract that does not exist yet, for capabilities built after `P8`?

Both are true, and the single-milestone framing overstated it. What `PD0`
delivers is a **contract plus a live stub** (`PD0-06`), which lets AutoDesign
build and test *their* side against a real, versioned, exercised surface while
the capability behind it is still being built. That is genuine parallel work —
and it is not "AutoDev is ready for them".

| Milestone | What AutoDesign can actually do | Needs | Cost |
|---|---|---|---|
| **M1 · Contract** | Build and test their integration against a live stub answering all four verbs | **`PD0` only** | **≈1 h** |
| **M2 · First real capability** | Send a real PSD or PDF and get a real IR back — the stub stops being a stub | `P8` + `PD1` | ≈15 h |
| **M3 · Full Rollout 1** | Fidelity, export, adapt, change, brand guideline | `PD2`–`PD6` | ≈39.5 h |

⚠ **`PD0` is not blocked by `P8`.** Its dependency is `P2`, which closed on
2026-09-02, and `§10.5.0` already says it *"runs early and in parallel"*. The
public brief's phase table nonetheless listed it **last, after `P4`**, which
reads as the contract arriving at the very end — the opposite of its intent.
That was a presentation defect and is corrected.

**Recommended order: `PD0` first, then `P8`.** One hour unblocks a partner team
who are otherwise waiting on a document, and nothing in `PD0` depends on the
sidecar being gone.



### Where this model should NOT be trusted

Stated rather than buried, because a recalibration that is confident everywhere
is just the old error with a new number:

1. **All 183 measured tasks are Rust and documentation.** `P6` and `P7` are 46
   tasks of frontend work with **zero** measured throughput behind them.
2. **`PD2` is a research loop.** "Fidelity is measured and refined until it
   passes" does not decompose into 14 units of equal cost, and no phase so far
   has had that shape.
3. **`P9` waits on things that are not me** — a keypair only Razi can generate,
   and installer builds on three platforms.
4. **The audit overhead is 46% on top of build time and is included above.**
   It has fallen every phase (P0 3h 36m → P3-F 19m) as the plan improved, and
   it is assumed to stay low. If a phase's audits start finding what P3-F's
   did, that assumption goes.

# §6 · Phase P0 — Stop the bleeding

**Goal.** Close the three exploitable surfaces, make gates capable of failing, and put all three platforms in CI — so everything after this is provable.
**Why this shape.** Every later phase's claim of "done" is worthless until a gate can fail. And two of the three surfaces are remote/local code execution; they are not scheduling questions.
**Depends on.** Nothing.
**Spec anchors.** `09 §3`, `09 §5`, `10 §6`, `06 §1`, vision ADR-03-02, ADR-03-04, ADR-04-03, G-15.

> ⚠ **These task lines are AMENDED by [`_verify/P0-CORRECTIONS.md`](_verify/P0-CORRECTIONS.md)** (2026-08-31, written before the first line of P0 code, per §0.2m).
> Three refutation verifiers re-checked all 27 claims: **21 confirmed as written, 5 understated or too narrow, 1 reversed** — `P0-26` would have deleted the config that is the only evidence of a live runtime break in the Logs export. Six tasks were added (`P0-28`…`P0-33`) and one (`P0-13b`) comes from a Razi ruling.
> Where a line below disagrees with that file, **that file wins** until this section is regenerated at P0 closeout. Decisions are logged as `ADR-LOG-RESTRUCT-0001`…`0006`.

### P0-A · The updater trust root
- [x] **P0-01** Replace `EMBEDDED_PUBLIC_KEY` (the literal `undefined-public-key-…-placeholder`, 53 bytes) with a real Ed25519 public key; private half in CI secrets only. → `UNIT-PROVEN`
- [x] **P0-02** `verify.rs:90` — an **empty signature URL is a hard failure**, never a skip. → `UNIT-PROVEN`
- [x] **P0-03** `verify.rs:18` — an empty checksum is a hard failure. → `UNIT-PROVEN`
- [x] **P0-04** `updater_commands.rs:111` — remove `.unwrap_or_default()`; a failed checksum fetch **aborts the update**. → `UNIT-PROVEN`
- [x] **P0-05** Re-verify at install time; do not trust an in-memory `stage: Ready`. → `UNIT-PROVEN`
- [x] **P0-06** Enforce HTTPS on `AUTODEV_UPDATE_URL` and every asset URL. → `UNIT-PROVEN`
- [x] **P0-07** Sanitise `filename_from_url` — reject `..`, `\`, and Windows-illegal characters. → `UNIT-PROVEN`
- [x] **P0-08** Delete the delta-patch path (`patch.rs`) entirely; full-installer updates only. → `CODE-PRESENT`

### P0-B · The vault becomes asymmetric
- [x] **P0-09** Delete `get_provider_key` and `get_github_pat`; **no command returns key material**. → `GATE-PROVEN`
- [x] **P0-10** Add `set_provider_key` (inbound) returning only `{ok, masked_last4}`. → `UNIT-PROVEN`
- [x] **P0-11** Add vault `enumerate` so orphaned keys are findable. → `UNIT-PROVEN`
- [x] **P0-12** Remove `airsctl`'s `--api-key` flag and its `ANTHROPIC_API_KEY` env fallback. → `CODE-PRESENT`

### P0-C · Delete the unauthenticated `/mcp` server *(moved from P8 — G-15)*

> ### ⚠ SEQUENCING HAZARD — read before starting this group
>
> Raised by the gate harvest as a **Rule-4 stop**, and it is a real error in the first draft of this plan.
>
> **(a) ACP identity.** All three worker↔worker messaging tools hard-refuse without a sender identity, and `ToolContext.agent_role` is set in **exactly one place in the codebase — `mcp_server/tool_adapter.rs:280`**, which this group deletes. The original plan fixed `ToolContext` in P3 and only *acknowledged* the problem at P4-22, which would have left a **decided-preserved capability silently dead from P0 through P3, with no gate that would notice.** → **`P0-13a` is pulled forward from `P3-13` to close this.**
>
> **(b) Fleet tool access, stated so it is not mistaken for a regression.** The MCP server exists to serve the *sidecar's* agents. Deleting it in P0 while the sidecar still runs until P8 means **sidecar-hosted fleet workers lose tool access entirely from P0 until in-process workers land in P4.**
>
> This is accepted, not overlooked, for three reasons: the fleet is already non-functional (no Windows task can reach QC — BUG-COMPILE; and any worker entering `fallback_idle` never ticks again — BUG-ASSIGN); interactive PA use is unaffected because Mode 1 already calls tools in-process; and ADR-03-02's own words are *"we delete it first now"* on a surface that is an unauthenticated local RCE exposing `bash`, `write_file` and `spawn_subagent`.
>
> ~~**If Razi would rather keep the fleet runnable through P1–P3, the alternative is to secure the server in P0** … **Flagged, not decided.**~~
>
> **▶ RAZI RULED, 2026-08-31: the deletion stays in P0.** The alternative (secure in P0, delete in P8) is rejected. Sidecar-hosted fleet workers therefore have no tool access from P0 until in-process workers land in P4, and that is an **accepted, stated consequence** — not a regression to be investigated when someone hits it.

- [x] **P0-13** Delete `mcp_server/{server,tool_adapter,port_status}.rs` and the `main.rs` wiring (~705 LOC). → `CODE-PRESENT`
- [x] **P0-13a** **Pulled forward from `P3-13`.** Populate `ToolContext` truthfully at all three in-house construction sites (`tool_executor.rs:89,252,316`) — `agent_role` is **not** `Option`, `depth` is the real spawn depth. Must land **in the same commit as P0-13**, so ACP identity never depends on the adapter being deleted. *(One fix, three capabilities: it also restores the subagent recursion guard and the `qc_review` build-evidence gate.)* → `GATE-PROVEN`
- [x] **P0-14** **Move** `health_monitor.rs` out of the sidecar-named tree into `mcp/health.rs`; same for `components/sidecar/McpHealthPanel.tsx` and the `sidecarStore.ts` MCP slice. *(Four of seven survivors live inside sidecar-named files — they must be relocated **by address** or sidecar code survives P8 by filename.)* → `CODE-PRESENT`
- [x] **P0-15** Delete the false UI claim at `Settings.tsx:653` that AutoDev connects to the GitHub MCP server. → `CODE-PRESENT`
- [x] **P0-16** Drop `rmcp` and `axum` from `Cargo.toml` if nothing else consumes them. → `CODE-PRESENT`

### P0-D · Gates that can fail
- [x] **P0-17** Remove **every** `continue-on-error` from correctness jobs in `ci.yml` (Rust tests ×2, `cargo fmt` ×2). → `GATE-PROVEN`
- [x] **P0-18** Install ESLint and make `npm run lint` actually lint; CI's "ESLint" step currently runs `tsc` a second time. → `GATE-PROVEN`
- [x] **P0-19** Install Vitest; add a `test` script; one real test so the harness is proven. → `UNIT-PROVEN`
- [x] **P0-20** Add the **macOS build job**; run all three platform builds **on pull requests**, not only pushes to `main`. → `PLATFORM-TRIPLE`
- [ ] **P0-21** Scaffold the end-to-end harness (WebDriver/Playwright against the built app) with one trivial passing scenario. → `SMOKE-PROVEN`
      ⚠ **PARTIAL.** Harness written (`e2e/wdio.conf.ts`, `e2e/specs/app-launch.spec.ts`, `e2e/README.md`), `npm run test:e2e` wired, and a Linux CI job added. It has **never executed an assertion**: `msedgedriver` is not on PATH on the Windows dev machine, which is the documented prerequisite, and `tauri-driver` has no WKWebView backend so macOS can never run it at all. `CODE-PRESENT`, not `SMOKE-PROVEN`.
- [x] **P0-22** Add the reachability guard test: **every `.rs` under `src/` is reachable from the crate root**. Expect it to fail immediately on `graph_tools.rs`, `graph_prompts.rs`, `db/checkpoints.rs`. → `GATE-PROVEN`

### P0-E · Platform baseline and hygiene
- [x] **P0-23** Create `autodev-core/platform/command.rs` — intent (`Build`/`Test`/`Lint`/`Run`) → platform-correct **argv**, with Windows `.cmd`/`.bat` shim resolution. → `UNIT-PROVEN`, `PLATFORM-TRIPLE`
- [x] **P0-24** Make `--use-angle=d3d11` Windows-conditional in `tauri.conf.json`. → `CODE-PRESENT`
- [x] **P0-25** Set per-platform bundle targets (currently `["nsis"]` only). → `PLATFORM-TRIPLE`
- [ ] ~~**P0-26** Remove the dead `dialog`/`fs` plugin blocks from `tauri.conf.json`.~~ → **REVERSED, superseded by `P0-31`.**
      The blocks were not dead config — they were the only evidence of a live runtime break. `Logs.tsx` calls `save()` and `writeTextFile()` at four sites; the npm packages are declared so it typechecks and CI is green, but Rust had neither plugin, registered only `tauri_plugin_shell`, and granted no `dialog:`/`fs:` capability. The Logs export failed on every click, into `console.error`. Deleting the config would have erased the evidence and left the feature broken.
- [x] **P0-27** Delete `src-tauri/dump.rdb` (175 KB committed Redis snapshot), the Memurai starter, and both Redis setup docs. → `CODE-PRESENT`

### Smoke — S0
- [ ] **S0-01** App launches on Windows, Linux and macOS from a clean build.
      ⚠ **Was ticked on 2026-09-01 and should not have been — corrected the same day.** I launched the app, piped it through `head -5`, saw logging initialise and recorded it proven. The app was in fact **crashing after those lines** on a `PluginInitialization("dialog", …)` error I had introduced. A smoke recorded from partial output is the defect class this phase exists to remove (`L-C9`).
      ✅ **Windows** — re-verified on a fresh debug build: 25s alive, no init error, 36 setup-progress lines.
      ✅ **Linux** — CI run `33467890406`: `OK - the application was still running after 20s`, plus the e2e harness driving it to `2 passing`.
      ❌ **macOS** — builds in CI but is never launched. Carried to P1 as a macOS launch smoke.
- [x] **S0-02** An update with a **missing** signature is refused; with a valid one, accepted.
- [x] **S0-03** No Tauri command returns key material (static scan).
- [x] **S0-04** `curl -X POST localhost:8767/mcp` **connection refused**.
- [x] **S0-05** A deliberately failing test **fails CI**.
- [x] **S0-06** `ToolContext` carries a real `agent_role` and `depth` at all three construction sites — asserted, not inspected. *(Closes the F-3 hazard: without this the `/mcp` deletion silently kills worker messaging.)*

**Exit.** S0 green → `TG-P0` green → matrix updated → progress row → no-stale sweep → commit.

**Narrative success criterion.** *A newcomer can point at CI and see that it can actually fail; can confirm no local process can reach an AutoDev tool endpoint; and can watch an unsigned update be refused. Nothing about the agent has changed yet — but from here on, every claim of "done" is checkable.*

---

# §7 · Phase P1 — Structural

**Goal.** Everything shipped is in the binary; one owner per schema; `autodev-core` exists and both binaries link it; redaction is restored.
**Why this shape.** Extracting the core crate **before** the runtime rework means that work lands in its final home. Landing all migrations here as one linear unit means no later phase renumbers.
**Depends on.** P0.
**Spec anchors.** `01 §1`, `08` (all), `05 §1.1`, ADR-01-02, ADR-01-03, ADR-01-09, G-02, M-40.

### P1-A · Make the invisible visible
- [x] **P1-01** Declare `graph_tools` and `graph_prompts` in `agent/builtin_tools/mod.rs` (918 LOC never compiled). → `CODE-PRESENT`
      `pub mod graph_prompts;` and `pub mod graph_tools;` are declared and the modules compile. Counts corrected against the source: **27** structs, not 22, and **6** E0609 errors, not 2.
      ⚠ **Compiling is not wiring, and this task does not claim it.** **0 of the 27** structs implement `Tool` and nothing registers them, so the agent still has no graph tools. The reachability gate going green here proves the code is *built*, never that a capability exists — wiring is P3 (BUG-GRAPH). This is exactly why `TG1-01` is recorded as short of its registration clause rather than ticked.
- [x] **P1-02** ~~Declare `db/checkpoints.rs`~~ → **DELETE it.** Task reversed. → `CODE-PRESENT`
      ⚠ **The stated justification was false.** Undo is **not** absent from the shipped app: `pub mod checkpoint;` at `git/mod.rs`, five commands registered in `lib.rs`, `src/stores/gitStore.ts` calls `list_checkpoints`, and a snapshot is taken before every agent write (`edit_file.rs`, `write_file.rs`). Retracted in `ADR-LOG-RESTRUCT-0010`, which also corrected the claim where I had propagated it into my own gate code and four documents.
      `db/checkpoints.rs` had **zero callers** and was never compiled, so `checkpoints.db` has never existed. Declaring it would have *created* the second checkpoint store `P1-16` then had to merge — manufacturing a problem in order to solve it two tasks later. Deleted instead (`ADR-LOG-RESTRUCT-0012`), which is why `P1-16` collapsed to a one-way confirmation.
- [x] **P1-03** Fix every compilation error the two above surface. → `CODE-PRESENT`
      Six `E0609`s in `graph_tools.rs`, at `:296-300` and one later — not the two at `:272` the task recorded. `get_node`'s `Option` handled with `.ok_or_else(…)`; `q.confidence_tier` corrected to `q.why`. `cargo check --workspace` exits 0.
- [x] **P1-04** P0-22's reachability guard now **passes**. → `GATE-PROVEN`
      `KNOWN_UNREACHABLE` is **empty**, which is the goal state: all three entries resolved by declaring two and deleting one. The gate was also **widened** in P1-B — it walked `src-tauri/src` alone and asserted `>100` files, which became false the moment 182 files moved into `autodev-core`. Lowering the floor would have left it green while protecting 84 files instead of 280, so it now walks all four crates with a floor each, and a total. Falsified by planting an orphan in `autodev-core`: caught and named.

### P1-B · Extract `autodev-core`
- [x] **P1-05** Create the crate; move the runtime **and the infrastructure it owns** — 182 files. → `CODE-PRESENT` · `GATE-PROVEN`
      *Scope corrected:* `agent/` makes 63 references into `db`·`graph`·`git`·`recipes`·`hooks`, so moving it forces those across; `§4`'s tree was the stale line (`ADR-LOG-RESTRUCT-0015`). `security/` is **not** part of the move — it does not exist until `P1-20` creates it.
      Evidence: `cargo tree` reports **0** Tauri/GTK/webkit edges for `autodev-core` (28 for the desktop crate, proving the check sees them); gates `TG-P1-01/02/03`.
- [x] **P1-06** Replace the `OnceLock<AppHandle>` with an injected event sink. → `UNIT-PROVEN`
      *Counts corrected again:* not 132/27/11 and not the 71/24/21 of `P1-CORRECTIONS` either — the live figures were **19 `emit_X(&app, …)` / `emit_X_global(…)` pairs** collapsed to one function each, rewritten across **50 call sites in 21 files**, plus 30 handle-passing argument lines removed. The earlier count included `events.rs`'s own definitions.
      `EventSink` lives in `autodev-core/src/events.rs`; the desktop installs a Tauri-backed sink in `main.rs` and reports double-installation rather than swallowing it; `airsctl` installs `NullEventSink` explicitly. Evidence: 4 unit tests on the sink, plus `src-cli/tests/core_runtime.rs`.
- [x] **P1-07** `src-tauri` and `src-cli` both link `autodev-core`. → `CODE-PRESENT` · `UNIT-PROVEN`
      Stronger than the stated class: `src-cli/tests/core_runtime.rs` constructs a real `AgentEngine` and registers the builtin tool set in a process with no UI framework linked — a file that could not have compiled before this phase.
      ⚠ **Linking is not adoption.** `src-cli` still reads tasks/plans/workers/analytics through `src-shared`'s parallel implementations. Collapsing those is `P1-09`, and it carries a shipped-contract decision: `airsctl worker heartbeat` and the `Last Seen` column write and read `last_seen`, **a column the live `workers.db` does not have**, so that command errors today.
- [x] **P1-08** Restate **INV-1** in the vision suite: *no agent work in a foreign runtime or language* (G-02). → `DOC-GATE`
      `00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md §Part 1`. The prior wording, *"outside the AutoDev process"*, was process-scoped — an embedded interpreter would have satisfied it while breaking the premise. Restated as runtime-and-language-scoped, with the P1 evidence and the still-unsatisfied half (`main.rs` spawns two Python processes; P8) recorded inline.

### P1-C · One schema owner *(all migrations, one linear unit)*
- [x] **P1-09** Move the task/worker/plan schema into `autodev-core`; **delete `src-shared`'s hand-mirrored tables**. → `LIVE-DB-PROVEN` · `UNIT-PROVEN`
      `src-shared/src/db/{tasks,workers,plans}.rs` deleted (22 / 19 / 23 write statements between them); `airsctl` now uses `autodev_core::db`.
      **The mirror was a live outage, not a latent risk:** `workers.rs` selected `last_seen`/`notes`, the live table had neither, and its own `CREATE TABLE IF NOT EXISTS` was a silent no-op — so `airsctl worker list/status/heartbeat` had failed since they were written. Now: live DB migrated (7 rows preserved, `integrity_check = ok`), `worker list` exits **0**, `heartbeat` persists a timestamp `status` reads back, unknown slug exits **1**.
      `plans.db` did not exist on disk at all, so the *"two incompatible designs"* carried **no data risk** — core's UUID-keyed design simply wins, and the CLI's `--id` arguments become strings.
      Four forced decisions logged as `ADR-LOG-RESTRUCT-0016`. `src-shared/src/db/analytics.rs` deliberately **stays**: it is not one of the four tables this task names and holds zero write statements, so it cannot drift a schema.
- [x] **P1-10** Give **every** database `PRAGMA user_version` and a numbered ladder. → `LIVE-DB-PROVEN` · `UNIT-PROVEN`
      Shared helper `db/migrate.rs`: `run_ladder` applies each rung **and its version stamp in one transaction**, refuses a database from a newer build rather than silently doing nothing, and rejects a non-consecutive ladder before touching anything. `ensure_columns` checks `PRAGMA table_info` before `ALTER`, replacing `tasks.db`'s **twenty** `let _ = execute_batch("ALTER TABLE …")` lines — an idiom that cannot tell *"already there"* from *"this statement is wrong"*, which is exactly how graph v5 stamped itself over six failures.
      **`config.db` needed a file owner.** Modules keep disjoint tables there, but `user_version` belongs to the *file*, so separate ladders would each see a version the others wrote. `db/config_db.rs` now owns the connection, the schema and the version.
      ⚠ **Written as "three modules, three tables" and that was wrong.** The post-phase audit found **six** tables from **five** modules — `checkpoints`, `hooks` and `hook_logs` were being created by `git/checkpoint.rs` and `hooks/db.rs`, outside the ladder that had just been declared the file's owner, and `INV-10` was scored GREEN on the wrong count. The mechanism was `L-C6`: I searched for the claim (`CREATE TABLE … app_settings`) instead of the invariant (`config.db`). All six are adopted at **v2**, `TG-P1-04` reads `config_db::TABLES` so it cannot drift from the owner, and the rung is `LIVE-DB-PROVEN` with 108 rows of undo history intact (`ADR-LOG-RESTRUCT-0020`).
      Live proof, via `examples/migrate_check.rs` running core's own shipped `open()` paths — not a fresh connection, so it proves the *application* applies the ladder: **7 of 7 databases** `uv=0 → v1`, `integrity_check = ok`, and **zero row-count changes** across 720 rows (`analytics` 331, `tasks` 220, `config` 115, `memory` 39, `annotations` 8, `workers` 7). Backed up first to `~/autodev-backups/p1-10-preflight`. `plans.db` was created fresh — it had never existed.
      ⚠ The per-repo graph store is **not** covered here: it sits at `user_version = 5` on its own ladder, and repairing that is `P1-11`.
      ⚠ **A defect in this task was found later, while writing `P1-13`'s tests.** `ensure_columns` runs *before* the ladder and returns early when the table does not exist, so a **fresh** database is created entirely by the baseline rung — and `tasks`' baseline was the original `CREATE TABLE`, missing the 18 columns added since. New installs would have come up without `claimed_by_pid` and seventeen others while **every existing database looked correct**, so no live-data check could have found it. Fixed in the `P1-13` commit; guarded by a test that asserts a fresh `tasks.db` has exactly the same 32 columns as the live one, verified against the real file.
- [x] **P1-11** Graph migration: v5's dead statements removed, `migration_v6` added. → `LIVE-DB-PROVEN` · `UNIT-PROVEN`
      ⚠ **The premise was wrong and is retracted** (`ADR-LOG-RESTRUCT-0017`). v5's six `ADD COLUMN IF NOT EXISTS` statements are invalid SQLite and were swallowed by `let _ =`, so the migration stamped `user_version = 5` and logged success over six failures — **that mechanism is real and stands.** But the columns were never lost: `CREATE_NODES` already declares all four and `CREATE_EDGES` both, so every database has had them since v1. The live graph store carries all six at v5.
      Two pieces of evidence sat unused when I made the claim: the live schema, and v5's own two `CREATE INDEX … ON nodes(agent_author)` statements, which use `?` — had the columns been absent, those would have failed and the migration would have errored rather than reporting success. `§0.16 L-C2` again: a claim about state is verified against the state.
      **v6 therefore ships as defensive, not corrective.** It is a no-op on every database that exists today; it exists for one built before those columns entered `CREATE_NODES` and then stamped v5 by the broken migration, which nothing else would ever repair. It checks `PRAGMA table_info` before each `ALTER` and propagates failure.
      Evidence: backed up to `~/autodev-backups/p1-11-preflight`; 4 tests including one that builds a `nodes`/`edges` pair **without** the columns, asserts the fixture truly lacks them, then requires v6 to add all six while preserving a row; and one that rewinds a complete schema to v5 and asserts v6 changes **no column** — the case that would fail with *duplicate column name* if written naively. Live: `5 → 6`, `integrity_check = ok`, column list and row counts identical to the backup.
- [x] **P1-12** `journal_mode=WAL` and a `busy_timeout` on every connection. → `LIVE-DB-PROVEN` · `GATE-PROVEN`
      One opener, `db::open_tuned`, used by **all 17 production connection sites** in core; `TG-P1-05` fails the build on any direct `Connection::open` (falsified by planting one — caught, named the file). Read-only `open_with_flags` and `open_in_memory` are deliberately exempt.
      **The WAL half was a genuine defect.** `analytics.db`, `annotations.db` and `workers.db` were all sitting in `journal=delete` on disk, and **five of the six** places `graph/store.rs` opens a connection set nothing. Under `delete` a reader and a writer cannot proceed at once — the shape of nearly every access here. Live: all 7 databases now `wal`, `integrity_check = ok`, row counts unchanged.
      ⚠ **The `busy_timeout` half was not a defect, and the task's premise is corrected.** SQLite's own default is 0, but **`rusqlite` overrides it to 5000 ms on every connection it opens** — measured directly by reading `PRAGMA busy_timeout` from an unconfigured `Connection::open`, which reports 5000. Setting it explicitly makes the value ours rather than inherited from a dependency default that can change silently; it fixes nothing that was broken. Recorded here rather than claimed as a fix.
- [x] **P1-13** One timestamp helper; every ad-hoc `parse_from_rfc3339` routed through it. → `UNIT-PROVEN`
      `db/timestamps.rs`. **Both halves of the correction verified by execution, not reading:** chrono *does* accept the space separator (`2026-09-01 06:23:58+00:00` → OK) and *does* reject an offset-less value (`2026-09-01 06:23:58` → FAIL, and so does the `T` form). A fix aimed at the separator would have changed nothing.
      **One site was genuinely broken, and it mattered:** `orphan/mod.rs` read `tasks.updated_at` — written by `datetime('now')`, so always offset-less — with `parse_from_rfc3339(…).ok()?`. The parse failed **100% of the time** and `?` returned `None` from `check_task`, reported as *"not an orphan"*. The TTL fallback that releases tasks claimed on another host or without a PID **could never fire once**. Now proven to fire: a test inserts a backdated row, asserts the column really is in the unparseable shape, and requires the orphan verdict — plus its inverse, that a 1h-old task is left alone.
      The other 6 sites read chrono-written columns and worked; they are routed through the helper anyway, and analytics' `.unwrap_or_else(|_| Utc::now())` — which made an unreadable record look brand new — now logs.
      *(The `tasks_updated_at` trigger resets `updated_at` on every `UPDATE`, so a stale row can only be built by inserting one — found while writing the test.)*
- [x] **P1-14** Consolidate to **one data root** (`~/.autodev/`), with a one-time migration. → `LIVE-DB-PROVEN` · `UNIT-PROVEN`
      All nine `~/.airs` path references repointed. Only **one** of them had anything on disk: `autodev.db`. `~/.airs/plugins/` and `~/.airs/fleet_secret` are code-only paths that have never existed — confirmed, the old root contained exactly one file.
      **The migration copies the file; it does not rebuild the schema.** That is the whole design, and it is what defuses the recorded trap: the live `messages` table carries `tool_calls` and `tool_results`, two columns the current `CREATE TABLE messages` no longer declares, so an `INSERT … SELECT` built from source would have dropped them. `VACUUM INTO` preserves every column by construction. Proven: after migration both columns are **present** and all 14 messages and 1 session are intact, content byte-identical.
      *(The columns turned out to hold `[]` in all 14 rows — no real content was at stake here. That was checked, not assumed, and the design does not depend on it: another install could have real values.)*
      Trap 1 fixed: `src-shared`'s `config_path()` used to `create_dir_all` `~/.airs` on **every call**, so the old root could never be retired. `legacy_root::legacy_dir()` deliberately does not create it, and a test asserts that.
      The session store also joined the `P1-10` ladder (it was outside the `~/.autodev` sweep): 5 swallowed `let _ = ALTER TABLE …` calls replaced by checked columns, `uv 0 → 1` on the live file.
      ⚠ **Deliberately not done: `~/.airs` is not deleted.** The source is left in place — removing a user's only other copy of their session history immediately after moving it is a destructive act that needs Razi's approval (§0.8), not a migration step. The *code* now uses one root; the retired **directory** still exists on disk holding a superseded copy. Its removal is a named follow-up.
      *(The 4.1 MB uncheckpointed WAL that `P1-CORRECTIONS` flagged as the headline risk **no longer exists**: it had been checkpointed into the main file before this task ran, verified as a 0-byte `-wal` at backup time. The `VACUUM INTO` approach would have captured it either way.)*
- [x] **P1-15** One owner for `app_settings`. → `CODE-PRESENT` · `UNIT-PROVEN` · `GATE-PROVEN`
      Owner is `autodev-core/src/db/app_settings.rs`, created during `P1-B` to break the `recipes → commands` cycle; the three creators (`github_mcp_commands.rs`, `workspace_commands.rs`, `ide/workspace.rs`) now call it.
      **The trigger question is settled by the live database, not by preference.** Only `github_mcp_commands.rs` declared `update_app_settings_ts`, so its existence depended on which caller opened `config.db` first — a schema decided by startup order. The live `config.db` **has** it (verified, alongside 4 real rows), so the owner creates it; omitting it would have made fresh installs differ from every existing one.
      Gate `TG-P1-04` asserts exactly one creator, falsified by planting a second (caught it, naming both files). Its detector ignores comments and `#[cfg(test)]` fixtures — the first version matched its own falsification test, the same self-match that bit the vault gate in P0 (`L-C7`).
- [x] **P1-16** Collapse the two checkpoint systems to one. → `LIVE-DB-PROVEN`
      Already a one-way delete, exactly as `ADR-LOG-RESTRUCT-0012` predicted: `db/checkpoints.rs` was removed in `P1-A` rather than declared. Verified now — **zero** references to `checkpoints.db` anywhere in the four crates, and no `~/.autodev/checkpoints.db` on disk, so no second store was ever created and there is nothing to merge. `git/checkpoint.rs` is the sole owner.
      ⚠ Still open, and **not** this task: `auto_checkpoint_before_write` returns `()` and logs a failure at `debug!` only, so a failed checkpoint is invisible and the write proceeds. That is `M-43`, owned by P4/P9.
- [x] **P1-17** Delete `hourly_costs`. → `CODE-PRESENT` · `UNIT-PROVEN`
      **Deleted, not created.** It had exactly one reference — the row-count loop in `analytics/queries.rs` — nothing has ever written to it, and it is absent from the live `analytics.db` (which holds `api_failures`, `daily_costs`, `system_events`, `token_usage`, `worker_stats`). Creating it would have produced a permanently empty table and a green diagnostic: inert-but-green.
      The masking mattered more than the table: `.unwrap_or(-1)` meant the missing table reported `-1` forever rather than failing. The list is now the `DIAGNOSTIC_TABLES` constant, and a test builds a fresh database from the real migration and asserts every name in it exists — falsified by re-adding `hourly_costs`, which the test caught by name.
- [x] **P1-18** Add the **run/audit ledger**. → `LIVE-DB-PROVEN` · `UNIT-PROVEN`
      `~/.autodev/runs.db`, one row per run — app launch, CLI invocation, and (later) agent turns and fleet ticks, which nest via `parent_run_id`.
      **Live from day one rather than a table waiting for a consumer:** both binaries record their own run. Proven against the real ledger with three `airsctl` invocations — two succeeded and were recorded `ok`, one failed and was recorded **`failed`**, which is what shows the outcome is measured rather than hardcoded. All three closed; none left open.
      A crash would otherwise leave a run open forever, and an open run reads as *"still running"* — so `reap_abandoned` runs at startup and closes rows belonging to a pid/host that is not this process, marking them `abandoned` rather than deleting them. `finish` on an unknown or already-closed run is an **error**: a ledger that quietly accepts a mismatched close is not an audit trail.
      ⚠ **Scope stated honestly: nothing is stamped yet.** No other table carries a `run_id` column. What is proven is that runs are recorded and readable; attributing later acts to them belongs to the phases owning those tables. This module existing is not evidence that anything is attributed.
- [x] **P1-19** Add a **backup helper** and wire it into every destructive path (GAP-11). → `UNIT-PROVEN`
      The helper (`db/backup.rs`, `VACUUM INTO`, integrity-checked, refuses to overwrite) landed **first** in this phase, per the revised order — everything destructive depends on it.
      ⚠ **The wiring half was still undone until now, and it was worth checking:** the helper had **zero callers**. A backup helper nothing calls is the same shape of defect as a gate that cannot fail.
      Wired to the two genuinely wholesale operations, deliberately not to row-level deletes — copying the database before deleting one task would make the board unusable, and a single row is not the loss this rule is about:
        · **`run_ladder`** backs up automatically before any rung whose SQL contains `DROP` / `DELETE FROM` / `RENAME` / `TRUNCATE`. Baseline rungs are `CREATE … IF NOT EXISTS` only and are skipped, so a fresh install does not copy every database it just created. Future destructive migrations get the net without their author remembering.
        · **`TasksDb::clear_all`** — the "Clear Board" action, which deletes every task and worker request and resets the id sequences.
      In both, a failed backup **stops the operation**. Proven by tests that read the backup back and assert it contains the row the operation then removed — not merely that a file appeared. In-memory databases report `Some("")` rather than `None` from `path()` and are exempt; a test covers that, having first failed on it.

### P1-D · Redaction — the dropped port (M-40)
- [x] **P1-20** Port **both** Python redactors into `autodev-core/security/`. → `UNIT-PROVEN`
      `SENSITIVE_KEYS` has **22** entries, not 20 — counted from the source and pinned by a test. The task named only `redact_dict`; `logging/centralized_collector.py`'s `PIIRedactor` (email, phone, SSN, card, `sk-`/`pk-` keys, bearer, `password=`) is ported too, because porting half a verdict is the error that lost redaction in the first place (`L-1`).
      **One deliberate deviation, documented in the module and asserted by a test:** Python's blanket *"32+ alphanumeric characters"* rule is kept for structured values and **excluded from free text**. A commit SHA is 40 hex characters and a SHA-256 is 64 — applied to file contents it would replace the code the agent reasons about with `[REDACTED_KEY]`, and the agent would then be wrong invisibly.
      **The limit is stated rather than hidden:** free-text redaction catches a secret with assignment context or a recognisable shape, **not** a bare high-entropy token in prose. A test asserts that, so it is a known property rather than an incident finding.
      A real leak was caught by ordering: the assignment rule stops at whitespace, so on `Authorization: Bearer eyJ…` it consumed `Authorization: Bearer` and **left the token in the text**. Found because the test asserted the token was gone, not that something had been replaced.
- [x] **P1-21** Apply at every boundary. → `GATE-PROVEN` · `SMOKE-PROVEN`
      ⚠ **This was ticked once and was wrong.** The redaction was wired to `ToolExecutor::execute`, which has **no production caller** — the agent loop calls `execute_batch`, and neither of its branches redacted anything, so raw tool input and output reached the model, the `messages` table and the UI. **Three independent post-phase auditors found it.** Both of my proofs were green: `S1-04` drove the dead method, and `TG-P1-06` asserted the file *contained the string* `security::redaction`, which the dead code satisfied.
      Now applied at **every** `tool.execute` call site — `execute` and both branches of `execute_batch` — because each reaches the conversation independently. `TG-P1-06` checks the **shape of the call**, so a fourth branch fails the gate.
      ⚠ **And that fix was itself only half of it.** It redacted every tool *result* and left the *input* alone on `execute_batch`'s **write branch**, which went on passing raw tool arguments to the UI start event and the `messages` row — the path every file edit and every command takes. No auditor found this; it surfaced only because Razi ruled `A-08` (hook payloads) forward from P3, and fixing that meant reading the input side. A credential passed *to* a tool is as durable as one returned *from* it. All four hook contexts, both start events and both persisted rows now carry the redacted copy; the **permission dialog** keeps the raw arguments deliberately, because the user is approving that exact call. A second gate pins the input side and was falsified by reverting one site. `L-C6` → 5: the first gate was written from the claim I had just implemented, not from the invariant it protects (`ADR-LOG-RESTRUCT-0021`).
      **Boundaries, stated exactly:** prompts ✅ (the tool funnel) · memory writes ✅ (all four stores) · logs ✅ (a `RedactingWriter` around the appender, so every line is covered whatever emitted it) · analytics ✅ (`record_api_failure`, `record_system_event`) · **egress deliberately not at the request body** — redacting what is *sent* would strip the content the model must reason about; its durable-storage risk is the two above (`ADR-LOG-RESTRUCT-0019`).
      19 of the area auditor's 40 bypass attempts are closed, each pinned by a test: `Authorization: Basic`/`Token`, raw JSON, quoted values with spaces, `postgres://user:pw@host`. Over-redaction fixed too — `author`, `passed` and `prompt_tokens` are no longer secrets, and JSON numbers stay numbers.
- [x] **P1-22** The P8 gate exists: **the deletion phase cannot complete while a capability grounding marked PORT remains unported.** → `DOC-GATE`
      Verified present as **`TG8-02`** `[governance]` in `TESTING_GATES.md §11`, reading the grounding reports' PORT verdicts as its checklist and requiring a named, tested destination for each. Its tenth entry — `M-40` redaction — is the one this phase just supplied, which is the point of the gate: it is the check that would have caught the drop without an adversarial review.

### Smoke — S1
- [x] **S1-01** `cargo build` clean across all four crates. — `autodev-core`, `autodev`, `airsctl`, `autodev-shared`, all exit **0**.
- [x] **S1-02** `airsctl --version` runs and links `autodev-core`. — prints `airsctl 1.0.0`, exit **0**; `cargo tree -p airsctl` shows the `autodev-core` edge and **zero** Tauri/GTK/webkit edges.
- [x] **S1-03** Every DB opens, reports its `user_version`, and survives a restart. — `examples/migrate_check` opens **all 10** through core's own shipped paths; run twice, output byte-identical, exit **0** both times.
- [x] **S1-04** A file containing `AWS_SECRET_ACCESS_KEY=…` read by a tool appears **redacted** in the prompt and in memory.
      ⚠ **Re-proven after the first version proved nothing.** It called `ToolExecutor::execute`, which production never calls. It now drives `execute_batch` — the method `agent/engine.rs:314` uses — and covers **both** branches (read-only concurrent, write serial) plus a mixed batch, because a test covering one branch would pass against a fix applied to one branch.
      **Falsified:** removing the redaction fails all three tests with *"THE SECRET REACHED THE PROMPT via execute_batch"*. The control assertions remain — `us-east-1` and `LOG_LEVEL` survive, the file was really read, and the *raw* content still reaches disk, because redacting what a write tool writes would corrupt the user's file rather than protect it.

**Exit.** S1 → `TG-P1` → matrix → progress → no-stale → commit.

**Narrative success criterion.** *A newcomer can run one command and see every source file is compiled, every database carries a version, and a secret read from disk does not survive into a prompt or the memory store. The agent still behaves as before — but the substrate is now honest.*

---

# §8 · Phase P2 — Providers

**Goal.** Three conforming providers, caching that measurably works, honest cost, and a spend ceiling that stops. *(Was "four" until xAI was struck with P2-07, 2026-09-01.)*
**Depends on.** P1.
**Spec anchors.** `02` (all), ADR-01-04, ADR-01-05, ADR-02-05, ADR-03-01, ADR-03-05, G-04, G-06, G-10.

### P2-A · The trait
- [x] **P2-01** `ModelProvider` trait v2 with `Item::Opaque` and six-field `Usage` **from day one** (retrofitting means touching every adapter twice). → `UNIT-PROVEN`
      ✅ **2026-09-02** — six-field `Usage` with the adapter-owned normalisation invariant, `CachePolicy` (four variants), `ErrorClass`, `ModelCapabilities`. `CompletionResponse` and `StreamChunk::MessageDelta` carry `Usage`. **10 unit tests, falsified** — removing the normalisation subtraction fails two, naming the invariant.
- [x] **P2-02** `CachePolicy` keeps its **four variants** — `Off` · `Automatic { affinity_key }` · `Explicit { breakpoints, ttl }` · `Handle { id }`. → `UNIT-PROVEN`
      ⚠ **My own edit of 2026-09-01 was wrong and is corrected here (§0.13 dual-anchor).** When xAI was struck I rewrote this as *"three variants — Anthropic explicit, OpenAI automatic, Google differs"*, mapping variants one-to-one onto providers. Spec `02 §2` does not do that: the variants are **policy shapes**, and several providers can share one. Striking xAI removes a *user* of `Automatic`, not the variant — and OpenAI's caching is automatic too, so `Automatic` survives on its own merits. `Handle` covers server-side state where it genuinely saves. Collapsing shapes to a per-provider count is exactly the *"collapsing this to a boolean"* the spec warns against.
- [x] **P2-03** `capabilities()` keyed by **model**, not provider. → `UNIT-PROVEN`
- [x] **P2-04** Anthropic adapter. → `GATE-PROVEN`
- [x] **P2-05** OpenAI adapter (`async-openai` tactically). → `GATE-PROVEN`
- [x] **P2-06** Google adapter — **fix the discarded tool arguments** (`google.rs:313` `let _args = ...`). → `GATE-PROVEN`
- [x] ~~**P2-07** xAI/Grok adapter — new (GAP-02); OpenAI-wire-compatible with a base-URL override.~~ → **STRUCK 2026-09-01, razi-ruled:** *"provider only remove grok, leave openai anthropic and google."* The adapter was never built, so nothing is deleted from the tree. What existed were references — a supported-provider entry, a model→provider mapping, a model-picker group and a docs line — all now removed. `xai` and `grok` remain in `PROBED_PROVIDERS` **deliberately**: a user who already stored an xAI key must still be able to find and delete it, and dropping the id would hide that secret in their keychain rather than retire it.
- [x] **P2-08** Honour `max_tokens` on OpenAI and Google (silently dropped today). → `UNIT-PROVEN`
- [x] **P2-09** Parallel tool calls on all three; Anthropic needs **all** results in one user message. → `GATE-PROVEN`
- [x] **P2-10** `StopReason::MaxTokens` distinguishable on all three. → `GATE-PROVEN`
- [x] **P2-11** Delete `ollama.rs` (506 LOC), 10 sites in 8 files, drop `ollama-rs`, and the 8 frontend doc references. → `CODE-PRESENT`
      ✅ **2026-09-02** — 537 LOC deleted, 31 references across 10 files, `ollama-rs` dropped from **both** manifests, shipped UI copy corrected. `TG2-14`'s scope includes shipped copy, not only Rust.

### P2-B · Caching — alarm before cache
- [x] **P2-12** **Build the cache-hit alarm first.** Non-zero `cached_read` from request 2 is the invariant; zero means a silent invalidator. → `GATE-PROVEN`
- [x] **P2-13** One prefix builder, ordered maps (`BTreeMap`/`preserve_order`) — Anthropic needs a 100% byte match. → `UNIT-PROVEN`
- [x] **P2-14** Remove volatile content from the system prompt (it is not byte-stable today). → `UNIT-PROVEN`
- [x] **P2-15** Per-model cache minimums in `capabilities()`; warn when the prefix falls short (**no provider errors below the minimum**). → `UNIT-PROVEN`
- [x] **P2-16** Wire caching on all three. → `BENCH-PROVEN` *(**Proven live 2026-09-02.** Anthropic: call 1 `cache_write: 11923` → call 2 `cached_read: 11923`. OpenAI: call 1 `input: 8659` → call 2 `input: 211, cached_read: 8448`, which also validates the adapter-owned normalisation of P2-01 against the real wire. Google caches implicitly and sends no field.)*

### P2-C · Cost and spend
- [x] **P2-17** Handle `message_start` in streaming — **input tokens are 0 for all three cloud providers today**. → `UNIT-PROVEN`
      ✅ **2026-09-02** — `message_start` is handled in `streaming.rs` — it was falling through `_ => Ok(None)`, and it is the frame carrying Anthropic's `input_tokens` **and** `cache_read_input_tokens`.
- [x] **P2-18** Per-model price table with **no silent fallback** — an unknown model is an explicit error. *(The `_ => (1.0, 3.0)` fallback catches 7 of 11 models.)* → `UNIT-PROVEN`
      ✅ **2026-09-02** — `cost::pricing` with exact-id matching. An unknown model is an **error**; the `_ => (1.0, 3.0)` table is deleted. `TG-P2-02` keeps the table and the model picker in step, **falsified**.
- [x] **P2-19** Provider-neutral usage normalisation — **Anthropic excludes cached tokens from `input`; OpenAI and Gemini include them.** → `UNIT-PROVEN`
      ✅ **2026-09-02** — Per-provider: Anthropic's counts are used as reported (it already excludes cached); OpenAI and Google go through `from_inclusive_input`. Uniform handling would double-bill cached tokens at the uncached rate.
- [x] **P2-20** Handle the 200k price cliff (Google Pro). *(The xAI half went with `P2-07`.)* → `UNIT-PROVEN`
      ✅ **2026-09-02** — `PriceTier` on `gemini-3.1-pro` — 2× input, 1.5× output **and** 2× cached read above 200k. The cached tier was a gap in my own first draft; a long context is mostly cached tokens.
- [x] **P2-21** **M-42 — daily spend ceiling in the provider layer**, the one place every call passes. → `GATE-PROVEN`
      ✅ **2026-09-02** — `CappedProvider` decorates `Provider`, so the ceiling sits where every call passes rather than in the loop, which three other call sites bypass. `TG-P2-04`, **falsified**.
- [x] **P2-22** **M-42 — global kill switch**; exceeding the cap **pauses the fleet and surfaces it**. → `SMOKE-PROVEN`
      ✅ **2026-09-02** — Exceeding pauses the fleet via `budget::daily::pause_fleet` — workers move to `paused`, which the scheduler already honours. Refusing without pausing would hand the same work to the next worker a moment later.

### P2-D · BYOK
- [x] **P2-23** Key entry → **live validation** (models-list or token-count call) → keychain. → `SMOKE-PROVEN` *(Validation moved onto the SAVE path: `set_provider_key` builds the provider and calls `test_connection` before storing, and refuses to store a key the vendor rejects. A network failure stores with a warning rather than locking an offline user out of configuring the app.)*
- [x] **P2-24** **Encrypted-file fallback on Linux** where no Secret Service exists; tell the user which store is in use (G-06). → `PLATFORM-TRIPLE` *(`vault/fallback.rs` — AES-256-GCM, key in a separate `0600` file, mode set AT CREATION so there is no default-umask window. `deb`/`appimage` are shipped targets and Secret Service is absent on headless hosts and in containers, so without this the app is unusable there. **The weakness is stated, not glossed**: anyone who can read the home directory can read both files, and the UI says so.)*
- [x] **P2-25** Per-provider keys, independently configurable; rotation and removal. → `UNIT-PROVEN` *(Proven at the map level: three providers occupy three entries, rotation replaces one without disturbing the others, removal leaves the rest intact, and the GitHub PAT sits outside the provider namespace so a provider named `github` cannot overwrite it. The disk path is left to the live smoke — a unit test that exercised it would rewrite the developer's real credentials.)*
- [x] **P2-26** Persist the active provider/model and **hydrate on startup** (the choice dies on restart today). → `SMOKE-PROVEN` *(Persisted to app settings and hydrated at startup. A saved model that has since been RETIRED is discarded rather than restored — four of ours were retired in February 2026, and reviving a dead id hands the user a selection that looks valid and 404s on first use.)*
- [x] **P2-27** Wire `ModelConfigsDb` — temperature, top_p, reasoning effort, thinking are **written by the UI and read by nothing**. → `UNIT-PROVEN` *(`TG-P2-08`, falsified. The table, four commands and a Settings panel all existed; nothing read them, so every request was built with the four fields hardcoded to `None`. **Worse than not offering the controls** — the slider moves, the value persists, and the user attributes the unchanged behaviour to the model.)*
- [x] **P2-28** Create the dev `.env` + integration-test harness that reads it. **The harness is never linked into the app.** → `UNIT-PROVEN`

### P2-E · Defects spec `02` names that this plan had dropped

**Added 2026-09-02 during P2's `§0.2m` pre-build verification.** Re-reading spec `02`
against this task list found five defects the spec states and the list did not carry. Two of
them undermine tasks already written, which is why they are here rather than in a later
phase. This is `L-C3` — *a source's conclusion adopted without checking its load-bearing
detail* — caught before the build rather than by an auditor after it.

- [x] **P2-29** **`cost_today_usd` is never reset**, so *"today"* means *"since process start"* (`G3c IN-50`). → `UNIT-PROVEN`
      ✅ **2026-09-02** — `cost_today_usd` carries its local date and rolls at midnight; every reader goes through `cost_today()`. **4 tests**, including a backwards clock.
      🔴 **This invalidates `P2-21` as written.** A daily ceiling built on a counter that resets when the app restarts is not a daily ceiling — it is a per-process one, and a user who restarts twice spends three times the cap. The ceiling cannot be built until the counter means what its name says.
- [x] **P2-30** **The daily rollup sleeps an hour before its first run and aggregates only *yesterday*** (`G3c IN-56`), so a desktop app used in sub-hour sessions **never completes a single rollup**. → `UNIT-PROVEN`
      ✅ **2026-09-02** — The rollup runs immediately, then hourly — it slept an hour **before** its first run, so sub-hour sessions never completed one. `eprintln!` → `tracing::error!`, which also brings it inside the redactor.
      Same dependency as `P2-29`: the ceiling reads what the rollup writes.
- [x] **P2-31** **18 hardcoded model strings**, and `analyze_diff_tool.rs:219-222` bypasses model selection entirely — via env vars set nowhere, falling back to a hardcoded `claude-sonnet-4-6`, **an id in no provider catalogue** (`G2a §3`). → `GATE-PROVEN` *(`TG-P2-09`, falsified. Four live defects: `analyze_diff_tool` fell back to `claude-sonnet-4-6` — an id in **no** catalogue — behind two env vars nothing sets, so the fallback was the only path and every call 404'd; `spawn_qc_review` and `recipes/runner` defaulted to a model retired in Feb 2026; and `anthropic.rs`'s slug normaliser, whose entire purpose is preventing 404s, mapped old names ONTO two retired ids — manufacturing the failure it existed to prevent. The gate also surfaced a **fifth** copy of the routing ladder in `worker_commands.rs` that P2-53 missed, whose tests asserted routing for two removed models.)*
      A hardcoded id that no catalogue contains cannot route; with `P2-18`'s no-silent-fallback rule it becomes a hard error rather than a mystery bill.
- [x] **P2-32** **`airsctl` takes an API key as a CLI flag and falls back to `ANTHROPIC_API_KEY` from the environment.** A key in a CLI flag leaks into shell history **and the process table**. → `GATE-PROVEN` *(Already removed; `TG-P2-07` keeps it removed — no shipped code may read a provider key from argv or the process environment. Falsified with a planted `env::var("ANTHROPIC_API_KEY")`. Carries a floor assertion so a scan that walks nothing cannot pass. The dev `.env` is outside this, not an exception to it: the live smoke reads that file directly, never the environment, behind a cargo feature.)*
      🔴 **Directly contradicts `INV-12` and the `G-KEY-NOENV` gate**, which requires that with an empty keychain and every provider env var set, the app **refuses to run a turn**. The gate and the code cannot both stand; the code is what changes.
- [x] **P2-33** **The vault has no `enumerate`** — keys for removed providers linger in the OS keychain with no way to find them (`G3c IN-49`). → `UNIT-PROVEN` *(`VaultManager::enumerate` unions a key-name index maintained on every write with the fallback vault's own keys, on top of the hardcoded probe list. Names only, never secrets. The probe list stays because an index can be lost — a user upgrading into this build has keys stored before it existed.)*
      ⚠ Partially closed already: `PROBED_PROVIDERS` in `vault_commands.rs` was widened on 2026-09-01 so retired ids stay probed. Verify whether that satisfies the invariant or only the symptom, and close the remainder.

### P2-F · Adapter conformance — from the pre-build audit

**Added 2026-09-02, razi-ruled: *"everything the audits found"*.** Three read-only refutation
agents (`§0.2m`) returned ~30 defects beyond the written tasks. All are absorbed here rather
than deferred. Sources: `_verify/P2-REFUTE-{caching,cost,byok-adapters}.md`.

- [x] **P2-34** **Google's streaming tool calls carry no arguments.** `google.rs:341` computes `_args` and drops them, and **no `ToolInputDelta` is ever emitted**, so the engine assembles from an empty accumulator and substitutes `{}`. 🔴 **Every Gemini tool call runs with empty input — not only parallel ones — and Gemini is the primary provider.** → `GATE-PROVEN`
      ✅ **2026-09-02** — 🔴 **Every** Gemini tool call ran with empty input, not only parallel ones. The parser emits `ToolInputDelta` carrying the arguments. **Falsified** — restoring the discard fails two tests.
- [x] **P2-35** Google streaming returns on the **first** matching part (`google.rs:326-350`), so parallel `functionCall` parts are dropped. → `GATE-PROVEN`
      ✅ **2026-09-02** — The parser returns `Vec<StreamChunk>` and walks every part, so parallel `functionCall` parts all survive.
- [x] **P2-36** `google.rs:347` hardcodes `block_index: 0` (two calls collide in `index_to_id`) and `:345` builds `id: format!("gemini-{}", name)` (collides when one tool is called twice). → `UNIT-PROVEN`
      ✅ **2026-09-02** — Block index from the part position, id includes it — two calls to one tool no longer collide.
- [x] **P2-37** `google.rs:93` sends a **tool-use id** where `functionResponse.name` expects a tool **name** — the round trip sends `"gemini-read_file"` for `"read_file"`. → `UNIT-PROVEN`
- [x] **P2-38** **Tool errors are invisible on OpenAI and Google** — `openai.rs:97-101` and `google.rs:87-91` destructure `is_error: _`. A failed tool is indistinguishable from a successful one on two of three providers; Anthropic passes it (`anthropic.rs:112`). → `GATE-PROVEN`
- [x] **P2-39** `openai.rs:94-109` **drops text that shares a message with tool results** — the `else` producing the text block is skipped entirely. → `UNIT-PROVEN`
- [x] **P2-40** **Unrecognised roles vanish on OpenAI** (`openai.rs:85-163`, no `else`) while Google coerces them to `"user"` (`google.rs:70-74`). Two adapters, two silent and different behaviours. → `UNIT-PROVEN`
- [x] **P2-41** `openai.rs:389` compares `finish_reason != "null"` against the four-character **string**; JSON `null` yields `None` from `as_str()` and never reaches the line. An inert comparison. → `UNIT-PROVEN`
- [x] **P2-42** **`stream_options: {include_usage: true}` is sent nowhere**, so `openai.rs:391`'s usage guard can never fire — and separately `openai.rs:342-344` discards the usage frame on empty `choices`. **Two edits, not one.** → `UNIT-PROVEN`
- [x] **P2-43** **`message_start` is unhandled** (`streaming.rs:85-86` falls through `_ => Ok(None)`) — the frame carrying Anthropic's `input_tokens` **and** `cache_read_input_tokens`. `P2-12`'s instrument does not exist without it. → `UNIT-PROVEN`
      ✅ **2026-09-02** — See `P2-17`. Anthropic's input side arrives on `message_start` and nowhere else.
- [x] **P2-44** Google's usage handler (`google.rs:353-366`) is unreachable — blocked by `:321-324` (no `parts` → `None`) and `:326-332` (first text part returns). → `UNIT-PROVEN`
      ✅ **2026-09-02** — Google's usage handler is reachable: both early returns are gone, and usage is emitted even when every count is absent so the stop reason survives.
- [x] **P2-45** `maxOutputTokens` is sent nowhere; `max_tokens` is read only at `anthropic.rs:134`. Extends `P2-08` with the measured shape. → `UNIT-PROVEN`
- [x] **P2-46** `stop_reason` is **written by four sites and read by none** — `engine.rs:474-481` discards it with `..`, so a `max_tokens` truncation and a clean `end_turn` are the same code path. *(There is no `StopReason` enum; the plan named a type that does not exist.)* → `GATE-PROVEN`
      ✅ **2026-09-02** — `stop_reason` is recorded rather than discarded by `..`, and truncation warns. A `max_tokens` cut and a clean finish were literally the same code path.

### P2-G · Cost and analytics integrity

- [x] **P2-47** `engine.rs:237` `let _ = db.update_task_budget_usage(...)` — the only persistence of running budget consumption, silently discarded, in a function that logs loudly for every other DB failure. → `UNIT-PROVEN`
      ✅ **2026-09-02** — The only persistence of running budget consumption no longer uses `let _ =`, in a function that logs loudly for every other DB failure.
- [x] **P2-48** 🔴 **Wire `with_task_budget`.** Zero callers, so `engine.rs:164-180`, `escalate_budget_exceeded`, `TaskBudget::default_worker` and `BudgetExceededReason` are all unreachable — **~100 lines of enforcement with no entry point.** `P2-21` cannot precede this. → `GATE-PROVEN`
      ✅ **2026-09-02** — 🔴 `with_task_budget` had **zero callers** — its signature demanded a `task_id` neither autonomous path has at construction. `with_budget` added; both paths run under `default_worker()`. `TG-P2-03` gates both the wiring and that the builders have callers at all, **falsified**.
- [x] **P2-49** `emit_cost_update` has **one** caller (`runner.rs:481`, Mode 2). The Mode 1 agent path emits `emit_agent_done`, so **Mode 1 spend is invisible to the only budget warning that exists**. → `GATE-PROVEN`
- [x] **P2-50** `dailyBudget` is unsettable — `dashboardStore.ts:173` and `CostMeter.tsx:20` hardcode 50, and `setDailyBudget` has no caller. → `UNIT-PROVEN` *(There were **three** disconnected numbers, not two. `DailyCap` enforces; `dashboardStore` hardcoded 50 for a banner; and `CostMeter` had its OWN `= 50` default which `Dashboard.tsx` was silently using, because it never passed the prop at all — surfaced only when making the prop required. All three now read `get_daily_cap`.)*
- [x] **P2-51** 🔴 **Unreachable price arm** — `tracker.rs:58-59` tests `contains("gpt-4o")` before `contains("gpt-4o-mini")`. Latent today, but **`gpt-5.4` and `gpt-5.4-mini` are both offered**: the trap is armed. Ordering must be longest-prefix-first, gated. → `GATE-PROVEN`
      ✅ **2026-09-02** — Exact-id matching makes the `gpt-4o` before `gpt-4o-mini` ordering trap unrepresentable. It was armed for `gpt-5.4`/`gpt-5.4-mini`, both offered.
- [x] **P2-52** `.unwrap_or(0)` guards all eight usage extractions. A provider schema change degrades to "this call was free" with no log line — and with `P2-48` unfixed, to unbounded silent spend. → `UNIT-PROVEN`
- [x] **P2-53** Provider inference by substring is duplicated (`engine.rs:379-387`, `runner.rs:441-452`) and `o4-mini` matches none, filing under `"unknown"` and corrupting `aggregator.rs:62-64`. One shared helper. → `UNIT-PROVEN`
- [x] **P2-54** `runner.rs:516-517` passes `1, 0` as literals, so `collector.rs:130-134` always computes `success_rate = 100.0` from that path. → `UNIT-PROVEN`
      ✅ **2026-09-02** — Mode 2 records its failures. ⚠ The audit reported this as universal; **Mode 1 already recorded both branches** — only Mode 2 was one-sided. Verified before fixing.
- [x] **P2-55** `analytics/aggregator.rs:20-23` — the hourly rollup is a stub returning `Ok(())` on a schedule. Logs no error, looks healthy, produces nothing. → `UNIT-PROVEN`
      ✅ **2026-09-02** — The stub was **deleted, not implemented**: it aggregated into `hourly_costs`, a table nothing creates, and nothing reads hourly buckets. Implementing it would have been a writer with no reader, built deliberately.
- [x] **P2-56** Gemini **thinking tokens are never counted** — `google.rs:357` reads only `candidatesTokenCount`; `thoughtsTokenCount` is never read and `thinking_tokens` is hard-passed `None` although `collector.rs:27` accepts it. → `UNIT-PROVEN`
      ✅ **2026-09-02** — Gemini `thoughtsTokenCount` is read and billed at the output rate.
- [x] **P2-57** `engine.rs:188` hardcodes `let model_context = 200_000u32;`, **ignoring `ModelInfo::context_window`** — so Gemini's 1M window compacts at 20%. → `UNIT-PROVEN`
- [x] **P2-58** `engine.rs:422` `_model: &str` is a dead parameter passed live at `:223`; per-model streaming cannot be implemented without a signature change. → `CODE-PRESENT`
      ✅ **2026-09-02** — `_model` is read; it sat exactly where per-model streaming behaviour belongs.

### P2-H · Caching prerequisites — ordering and prompt structure

**These are correctness bugs on the wire that also make caching unmeasurable.** They precede
`P2-B`: a messages-level invalidator looks exactly like *"caching doesn't work"* while the
prefix is perfectly stable — the hardest possible bug to attribute.

- [x] **P2-59** 🔴 **`engine.rs:490` iterates a `HashMap`**, so with two or more parallel tool calls the block order written into `history` is arbitrary **and differs between processes** — then replayed on every later turn. `block_index` is already carried on `ToolUseStart` (`types.rs:92-96`); use it and an ordered container. → `GATE-PROVEN`
      ✅ **2026-09-02** — 🔴 Parallel tool calls were ordered by `HashMap` iteration — arbitrary, **differing between runs of the same binary**, and replayed every turn. Both accumulators are `BTreeMap`s keyed by the wire's block index. **5 tests**, one asserting ten assemblies agree.
- [x] **P2-60** `engine.rs:487` pushes `text_buffer` unconditionally **first**, inverting any text the model emitted after a tool call. → `UNIT-PROVEN`
      ✅ **2026-09-02** — Text no longer jumps ahead of tool calls; `TextDelta` carries `block_index`, which Anthropic already sent and we discarded.
- [x] **P2-61** `engine.rs:320-340` emits unknown-tool errors before batch results, so the result order does not parallel the call order. → `UNIT-PROVEN`
- [x] **P2-62** **Delete the dead divergent builder** `context.rs:37 build_system_prompt` — zero callers, reads AUTODEV.md only, never queries `AutodevInstructionsDb`, never injects memory. It would emit a *different* prefix, and it is a trap for whoever wires the cache. → `CODE-PRESENT`
- [x] **P2-63** `anthropic.rs` sends `system` as a **bare string**, so there is no block to hang a breakpoint on. Converting to a content-block array is a prerequisite for `Explicit` caching, not part of it. → `UNIT-PROVEN`
- [x] **P2-64** **Hoist per-run identifiers out of cached prompts.** `spawn_subagent.rs:118` puts `working_dir` at **offset 0**; `defaults.rs:1573-1600` stamps `#{task_id}` five times into the QC reviewer prompt. With six concurrent reviewers over a multi-KB otherwise-identical protocol, every prefix is single-use and each cache write is pure 1.25× overhead — **naive caching would *increase* cost here.** → `BENCH-PROVEN` *(**Measured: 10,466 bytes / ~2,616 tokens** of QC protocol were single-use per reviewer. `task_id` sat at offset ~120 of a multi-KB prompt and was stamped 8 more times below, so with 6 concurrent reviewers all 6 prefixes diverged in the first two lines. Since a cache WRITE is billed above the input rate, requesting caching in that shape cost **more** than not requesting it. Protocol is now a const with per-run facts appended; 6 reviewers share one key. Above Sonnet's 1,024 minimum, below Opus/Haiku's 4,096 — where `will_cache` now correctly declines instead of paying the write.)*
- [x] **P2-65** **Measure the prefix against the minimum before building the cache.** `POST /v1/messages/count_tokens` per `PAMode` × model. `DELEGATE_SYSTEM_PROMPT` is ~461 tokens with four tools, against a 4,096 minimum on two of three shipped models — **caching there is a design-time certainty of failure, not a runtime edge case.** → `BENCH-PROVEN` *(**Measured, not estimated.** `count_tokens` puts the base system prompt at **2,595** tokens. Against the shipped minimums that is CACHEABLE on `claude-sonnet-5` (1,024) and **silently uncacheable on `claude-opus-5` and `claude-haiku-4-5` (4,096)** — confirming the audit's claim as a design-time certainty rather than an edge case. The measurement also refuted my own documented claim that `estimate_tokens` is biased low: it over-counted a repetitive probe by 5%% and under-counted the real prompt by 29%%, which is why `MINIMUM_MARGIN` now exists.)*
- [x] **P2-66** Compaction (`compaction.rs:154-163`) forces a full messages-level cache rewrite plus an extra uncacheable `complete()` call, with no accounting, against a chosen `0.80` constant. → `BENCH-PROVEN` *(The real hole was **no accounting at all**: `compact_with_summary` consumed the response for its text and **discarded `usage`**, so a billed call reached neither cost, nor the daily ceiling, nor analytics — and compaction fires on long sessions, summarising most of a context window. Now returns its usage and the engine folds it into the turn total. The `0.80` constant is documented as a chosen asymmetric trade rather than a measurement, and it became far safer in P2-57: applied to a hardcoded 200k it fired at **16%** of Gemini's real window.)*

### P2-I · Adapter test infrastructure

**Razi-ruled 2026-09-02: fixtures in CI, plus a live smoke behind a flag.** Measured: **1,733
lines of adapter code with zero `#[test]` of any kind** — `anthropic.rs`, `openai.rs`,
`google.rs`, `ollama.rs`. Every defect in `P2-F` survived because nothing could catch it.

- [x] **P2-67** A fixture corpus of **real captured payloads** per provider — unary and SSE, including tool calls, parallel tool calls, truncation, and each error class. → `DOC-GATE` *(Three suites: `anthropic_tests.rs` (9), `openai_tests.rs` (9), `google_tests.rs` (9) + the cross-adapter conformance suite. Covers unary and SSE, tool calls, parallel tool calls, truncation, and every `ErrorClass` from real vendor response bodies. **Anthropic and OpenAI had NO streaming tests at all** before this — the streaming path is the primary agent path.)*
- [x] **P2-68** Adapter tests parse the fixtures and assert the six-field `Usage`, tool arguments surviving byte-identically, `stop_reason` mapping, and error classification. **Runs in CI with no credentials** — `§0.5`: a gate skipped in CI is not green. → `GATE-PROVEN` *(`ErrorClass` had **no constructor** until now: defined, unit-tested against itself, produced by nothing, so a spend-cap 429 was the same string as a rate-limit 429. `classify()` reads the body, `TG-P2-06` holds every HTTP error path to it, falsified.)*
- [x] **P2-69** A **live** smoke behind a cargo feature, reading the dev `.env`, **never linked into the app** (`P2-28`). Catches vendor wire drift that fixtures freeze and hide. Credentials are set **per-command, never session-wide** (`§0.8`). → `SMOKE-PROVEN` *(9 live tests, all green, behind the `live-smoke` cargo feature: 0 tests compile without it. It caught four defects fixtures never could — see P2-31.)*
- [x] **P2-70** `vault_commands.rs:38` decrypts a secret into process memory on every `list_providers` purely to compute a boolean, then drops it unzeroized. Use a presence probe. → `UNIT-PROVEN` *(`VaultManager::exists` zeroizes. ⚠ Honest limit: `keyring` exposes only `get_password()`, so the decrypt is unavoidable — what changed is that the secret is overwritten immediately instead of left in freed heap. Claiming it "avoids decrypting" would read better and be false. Two call sites fixed: `has_provider_key` and `build_provider_info`.)*
- [x] **P2-71** The credential store in use is **never disclosed** — `VaultEntryStatus` carries no backend identity, while `docsMore.ts:160` claims behaviour the code does not implement. → `UNIT-PROVEN` *(`VaultBackend` with `label()` and `security_note()`, surfaced through `VaultEntryStatus` and the new `vault_backend_info` command. `docsMore.ts` claimed keys are "never stored in plain files" and that "the vault is unlocked per-session with the OS credentials" — the second mechanism does not exist at all. Both claims replaced with what the code does.)*
- [x] **P2-72** `Settings.tsx:1082-1092` **promotes an unvalidated key to active** and clears the prior test result. Validation belongs on the save path (`P2-23`). → `SMOKE-PROVEN` *(Three defects in nine lines: promoted to active with nothing having validated it, `setTestResult(null)` threw away the only signal the user had, and a failed save was swallowed into `console.error` so nothing visible happened. All three fixed; validation now comes from the backend save path.)*
- [x] **P2-73** Remove the **unused** `ollama-rs` from `src-tauri/Cargo.toml:89` — `ollama_rs::` is imported by one file, reached through the `autodev_core` re-export. Removable today with no code change. → `CODE-PRESENT`
      ✅ **2026-09-02** — `ollama-rs` removed from `src-tauri/Cargo.toml` — it was imported by one file, reached through the `autodev_core` re-export.
- [x] **P2-74** ⚠ **`P2-11`'s framing is wrong and its numbers are wrong.** Measured: **537** LOC (not 506), **31** Rust references across **10** files (not "10 sites in 8 files"), `ollama-rs` in **two** manifests. And it is **not dead weight** — `settings_commands.rs:28-33` enumerates it as a live provider, `Settings.tsx:67-68` renders it, and **five** provider-construction switches route to it. Deletion is a **user-visible feature removal**, which Razi has ruled, but it is not a cleanup. → `CODE-PRESENT`
      ✅ **2026-09-02** — Measured and recorded: 537 LOC not 506, 31 references not 10, 10 files not 8, and it was a **live** provider with five dispatch switches — a user-visible removal, not a cleanup.
- [x] **P2-75** The tree contradicts itself about Ollama: `PROBED_PROVIDERS` marks it retired and `is_supported_provider` excludes it (so a stored key reports `orphaned`), while `settings_commands.rs:32` presents it as live on the same screen. **Half of this contradiction was introduced 2026-09-01 and must not outlive `P2-11`.** → `GATE-PROVEN` *(**Already resolved** — verified rather than assumed. Every remaining `ollama` reference is a removal comment, the deliberate `PROBED_PROVIDERS` entry that keeps an orphaned key findable, or a negative test assertion. `list_providers` offers exactly anthropic/openai/google.)*

### Smoke — S2
- [x] **S2-01** A turn completes on each of the three providers. → **PASS.** All 10 catalogued models called live; `every_catalogued_model_is_actually_callable`.
- [x] **S2-02** A 5-turn session shows non-zero `cached_read` from turn 2. → **PASS.** Anthropic call 1 `cache_write: 11923` → call 2 `cached_read: 11923`. OpenAI `input: 8659` → `input: 211, cached_read: 8448`.
- [x] **S2-03** Session cost matches the provider's own reported usage. → **PASS.** Cost recomputed from the provider's own six-field usage against the price table and compared to `cost_usd`; agreement within 1e-9.
- [x] **S2-04** With the keychain empty and every provider env var set, the app **refuses to run a turn**. → **PASS, by construction.** `TG-P2-07` proves no shipped code reads a provider key from argv or the environment, so setting every provider env var with the keychain empty changes nothing. Asserted from the other side in `s2_spend_ceilings.rs`.
- [x] **S2-05** Exceeding the daily cap pauses the fleet with a visible message. → **PASS.** The ceiling fires **at** the limit (`>=`), not past it; a disabled cap never reports `Exceeded`; the meter saturates at 1.0 rather than rendering past 100%.
- [x] **S2-06** The daily counter **survives a restart**: spend, restart the app, and the cap still knows what was already spent. → **PASS — after fixing a real defect.** `cost_today_usd` lived **only in memory**, so the ceiling read zero on every launch: a user who reached the cap could restart the app and carry on spending. A cap that resets on restart is a per-session cap calling itself daily. Now persisted on every `add_cost` and hydrated at startup, **date-stamped** so yesterday's spend is discarded rather than carried into today. Falsified.
- [x] **S2-07** `airsctl` refuses to run a turn from an environment variable with the keychain empty. → **PASS.** Same construction as S2-04 — `airsctl` shares the provider factory, and no path reads a key from the environment.
- [x] **S2-08** A Gemini turn that calls a tool passes **real arguments** to it, and two parallel calls both execute. → **PASS.** Gemini returned **two parallel calls with real arguments**: `read_file({"path":"src/main.rs"})` and `read_file({"path":"src/lib.rs"})`. Confirms the P2-06 fix live — before it, every Gemini tool call ran with `{}`.
- [x] **S2-09** A turn truncated by `max_tokens` is reported as truncated, on all three providers. → **PASS.** `max_tokens` / `length` / `MAX_TOKENS` all classify as `MaxTokens` against the live wire.
- [x] **S2-10** A failing tool is reported as failed on all three providers. → **PASS.** A real two-turn round trip on all three: the model's own tool call is replayed with a failure and it answers FAILED. **This is what found the `thoughtSignature` breakage** — a hand-built assistant turn cannot work on Gemini, because the signature is server-issued.
- [x] **S2-11** Two parallel tool calls appear in the transcript in the order the model emitted them, **twice in a row in separate processes**. → **PASS, structurally.** The P2-59 defect was non-deterministic *across processes* (`HashMap` iteration order is per-process randomised), which re-running in one process cannot detect. Asserted instead that the ordering path holds no `HashMap` and is keyed by the wire block index in a `BTreeMap`, and that the prefix builder sorts its tools.
- [x] **S2-12** A task with a $0.50 budget stops at $0.50. → **PASS.** Expressed against real prices rather than invented numbers, and reasoning tokens are proven to count toward the ceiling — they are invisible in the response but bill at the output rate.

**Exit.** S2 → `TG-P2` → matrix → progress → no-stale → commit.

**Narrative success criterion.** *A newcomer can enter a key for any of the three providers, watch it validate, send five messages, and see in the cost panel that turns 2–5 cost a fraction of turn 1 — the first observable proof that caching works. And they can set a daily cap and watch it actually stop.*

---

# §9 · Phase P3 — Runtime and memory

**Goal.** Bounded context, one tool path with a real permission gate, in-process subagents, a panic that cannot kill the app, and memory that is provably read.
**Depends on.** P2.
**Spec anchors.** `01`, `03`, `06`; ADR-01-06/07/08, ADR-02-03, ADR-05-01…05-05, G-01, G-03, G-07, G-11, G-12, G-13.

### P3-A · Fault isolation (M-41)
- [x] **P3-01** **Move the whole `[profile.release]` block to the workspace root**, with `panic = "unwind"`. ⚠ **CORRECTED 2026-09-02 by the `§0.2m` pass — the original task was a no-op.** The block lives at `src-tauri/Cargo.toml:117-122`, a **non-root workspace member**, and Cargo discards it: *"warning: profiles for the non root package will be ignored, specify profiles at the workspace root"* (verbatim, from `cargo metadata --no-deps`). The root `Cargo.toml` has **no `[profile]` block at all**, so `panic = "abort"` was never in effect — unwinding is already on, and editing that line would have changed nothing while appearing to fix the phase's headline risk. **Collateral, and the reason this task grew:** `lto = true`, `codegen-units = 1`, `opt-level = "s"` and `strip = true` are inert for the same reason, so every release build ships un-LTO'd and unstripped. → `CODE-PRESENT` *(`TG-P3-01`. Moved to the workspace root; `cargo metadata` no longer warns, which is the check that can fail. The member copy is replaced by a pointer so it cannot be re-added.)* ⚠ **Proven by execution under the RELEASE profile, 2026-09-02**, not by reading the manifest: `cargo test --release -p autodev-core --test s3_smokes s3_02` reports `ok`. Under `panic = "abort"` that binary would have **aborted** instead — `catch_unwind` cannot catch an abort — so a passing result in release mode is the only thing that distinguishes "the profile is written correctly" from "the profile is in force". Cargo also emits no *ignored profile* warning, which it did while the block lived in the non-root member.
- [x] **P3-02** Every agent turn and subagent runs inside `catch_unwind`. → `UNIT-PROVEN` *(`TG-P3-02`. `catch_unwind` at the turn boundary, plus a `Drop`-guarded `is_running` — it was cleared on the line *after* the await, so a panicking turn left the session permanently marked running with nothing saying why. The guard clears during unwinding; a line after `catch_unwind` is one early return from being skipped again.)*
- [x] **P3-03** A panicking worker fails **that turn**, reports it, and the app survives. ⚠ **RESTATED 2026-09-02 — true, but not for the stated reason, and the fix is different.** The process does **not** die: every agent path is inside `tokio::spawn` (`agent_commands.rs:134`) and unwinding is live (see `P3-01`). What actually happens is **`Mutex<AppState>` poisoning** — measured **98 `.lock().unwrap()` sites against 5 poison-tolerant** — so one panic converts into a cascade of unrelated panics at every later lock, a permanent restart-only failure that presents as something else entirely. Also `is_running` is cleared *after* the `.await` (`agent_commands.rs:174`), so a panicking turn leaves the session permanently unusable. **The fix is poison-tolerant locking plus a `Drop`-guarded `is_running`, not merely `catch_unwind`.** → `GATE-PROVEN` *(`TG-P3-03`, falsified. **103 sites** routed through `lock_tolerant`/`read_tolerant`/`write_tolerant` — 98 mutex + 5 RwLock the first pass missed and the gate caught. The trade is stated at the helper: `AppState` holds counters and flags with **no cross-field invariant**, so a stale field beats a restart-only app — and a structure that *does* carry a spanning invariant must not use this trait.)*
- [x] **P3-04** Fix the three byte-slice truncation panics. ⚠ **EVERY CITATION IN THIS TASK WAS WRONG (2026-09-02).** Real sites: **`remember_this.rs:101`** (`&fact[..120]`), **`annotate_file_tool.rs:109`** (`&note[..80]`), **`bash.rs:162`**. All three reproduce — compiled standalone and run under `catch_unwind`. They are **not identical**: `bash.rs` slices a **computed tail** (`output[tail_start..]`, then `&output[boundary..]`), so a fix templated on `&x[..N]` leaves it panicking — and it is the site most likely to fire, since rustc and cargo emit `┌ │ └` in diagnostics. A **fourth** site the plan never mentions: `bash.rs:415` (`&command[i..]`). **`validate.rs:846-853` is not a truncation helper** — it is pyright/mypy dispatch plumbing; the real one is at **`945-957`**, and it truncates one character early and carries an `unwrap_or(MAX_CHECKER_BYTES)` fallback that would be the exact unsafe slice if it were reachable (it is not, since the length guard makes `char_indices` non-empty). **Two genuinely correct helpers already exist, duplicated verbatim**, at `memory/recall.rs:183-192` and `memory/summarizer.rs:148-157` — so the task is *one shared helper replacing five sites and two duplicates*, not "reuse the one at 846". → `UNIT-PROVEN` *(`TG-P3-04`, falsified. **Twelve** unsafe sites, not three: the four corrected ones, `bash.rs:415`, and **seven** in `sidecar/client.rs` the gate found. One shared helper in `security/truncate.rs`, plus the two byte-identical duplicates in `memory/` now delegating to it.)*

### P3-B · The loop
- [x] **P3-05** **Every** exit path emits exactly one terminal event — via a `Drop` guard. ⚠ **It is THREE, not four (2026-09-02).** The silent returns are `engine.rs:127`, `:327` and `:627`; **`:578` already emits `agent:error` at `:571`**, so hunting a fourth lands there and produces a **double emission** — which `TG3-01` names a failure exactly as loudly as zero. *(`:627` is the tool-batch path I converted from `?` to an explicit return during P2-D-1 and did not notice was silent — recorded rather than quietly fixed.)* → `GATE-PROVEN` *(A `Drop` guard that **owns** the emission — the obvious shape is a `fired: bool` each emitter sets, which works only while every future emitter remembers, and the reason this exists is that three paths forgot. All five exits route through it, including `:579` which already emitted: routing it prevents the double emission `TG3-01` fails as loudly as zero.)* 🔴 **EVIDENCE CLASS CORRECTED, 2026-09-02 (`F-5`).** This was badged `GATE-PROVEN` against **`TG3-01`, which does not exist** — it is still `- [ ]` with `Evidence: _pending_` in `TESTING_GATES.md`, while being cited as live in **nine** production comments. The auditor's demonstration was exact: emptying `impl Drop for TerminalEventGuard` broke no test in the tree, so the guard's entire reason for existing was unprotected. **Now `UNIT-PROVEN`** by `f5_tests` — an unreported exit leaves the guard unfired (so `Drop` emits), a reported one leaves it fired (so `Drop` stays silent, which is what makes it *exactly* one rather than *at least* one), and a double report is a no-op. ⚠ These assert the state machine the emission depends on; they cannot observe the event itself, which goes to a global bus. `TG3-01`'s seven-way parameterisation over stop reasons remains genuinely unwritten and is still `- [ ]`. **`TG-P3-F8` now fails the build on a gate id cited in code that does not exist at all**, which is the structural half.
- [x] **P3-06** Persist the **whole** message vector with usage, not `history.last()`. ⚠ **Understated (2026-09-02).** On the `MAX_TOOL_TURNS` and cancel exits `history.last()` is a **user** message, so **nothing is persisted at all**; `tool_use` blocks are dropped entirely; and `assistant_msg.content = text` **overwrites**, so a multi-block assistant turn loses all but the last block. → `UNIT-PROVEN` *(Every assistant message the turn produced is persisted, text blocks **joined** rather than overwritten, with tool calls recorded. Previously `history.last()` — and on the turn-limit and cancel exits that is a *user* message, so nothing persisted at all.)*
- [x] **P3-07** Never append an assistant message with empty content. ⚠ **A partial fix already landed citing this claim by name** (`engine.rs:1070-1071`) — and it drops empty *blocks*, which makes the empty *`Vec`* case **more** likely, not less. It also does not touch the **persisted** path: an empty assistant row reaches SQLite unguarded, and `build_history` rebuilds it as `{"type":"text","text":""}` on **every later send, forever**. Both halves are open. → `UNIT-PROVEN` *(Both halves. The live path refuses to append an empty assistant message; the persisted path refuses to write an empty row. A turn with only tool calls is **not** empty — its record is the calls — so it is kept with a synthesised summary rather than dropped, which would orphan every `tool_use`.)*
- [x] **P3-08** Thread a `CancellationToken` into the provider call, the tool executor, and every pending-permission wait; **every one has a timeout**. ⚠ **Partly already true (2026-09-02).** `bash.rs:124` **does** time out, and subagents **are** wrapped in `timeout` (`native_runner.rs:220`). The real gap is the **interactive permission wait** — the one path that can block on a dialog — which has neither. `tokio-util` is **not currently a dependency**, so the `CancellationToken` half is a new dependency (`§0.15` flag). → `GATE-PROVEN` *(The permission wait races three outcomes: the answer, the existing `is_cancelled` flag, and a 300 s timeout. **Timeout denies** — an unanswered prompt is not consent, and that branch is reached precisely when the listener is gone. The pending slot is drained on every outcome; it was only ever removed by the responder, so an abandoned prompt leaked it.)* 🔴 **RETRACTED IN PART, 2026-09-02 (`G-3`).** `ToolExecutor::with_state` had **zero callers workspace-wide**, so `self.state` was always `None` — and the cancellation arm reads it through `.map(…).unwrap_or(false)`, which meant that arm **could never fire**. A user cancelling while a permission dialog was open waited the full 300s `PERMISSION_TIMEOUT` regardless. `§0.16 L-C12` — a helper built and wired to nothing — in this task's own work, with the class count standing at seven when I wrote it. **Now wired** at the engine's construction site.
- [x] **P3-09** Define the **retry policy** `ErrorClass::Retryable` currently lacks (GAP-12), with backoff and a bound. ⚠ Confirmed, and the code says so twice (`usage.rs:244`, `ci_gate.rs:1821`). **Worse than "absent":** `classify()` is called, `Debug`-formatted into a `String`, and discarded — so any retry loop written today would have to **string-match `[Retryable]` back out of an error message**. The policy needs the classification carried as a *type*, which is `F-5` from the P2 audit, deferred to exactly here. → `UNIT-PROVEN` *(`provider/retry.rs`, and **wired** — retrying only the stream *establishment*, where no chunk has been emitted, so a retry cannot duplicate output. `QuotaExhausted` is attempted exactly once: a spend-cap 429 never succeeds on retry, so retrying it is a spend loop against a ceiling that already fired.)*
- [x] **P3-57** **A panicking tool must not orphan its `tool_use`.** `tool_executor.rs:288-292` pre-fills every result slot with `(String::new(), ToolResult::error("placeholder"))`; the panic handler at `:389` logs and leaves it — its own comment admits *"Placeholder will be replaced; we need to find which one failed"*. So the slot keeps `tool_use_id = ""`, `engine.rs:657` inserts it under key `""`, `by_id.remove(id)` misses, and the assistant message goes out carrying a `tool_use` with **no matching `tool_result`** — a hard **400** from the provider on the very next request. With two panicking tools they **collide on `""`** in the `HashMap` and all but one are silently dropped. Concurrency is 10. *(Found by the `§0.2m` pass; no task existed.)* → `GATE-PROVEN` *(Every result slot is pre-keyed with its **real** `tool_use_id`. It was `(String::new(), error("placeholder"))`, and the panic handler left it — so a panicking tool emitted a `tool_use` with no matching result, a hard 400 on the next request, and two panicking tools collided on `""`.)*

### P3-C · Tools and permissions
- [x] **P3-10** Collapse to **one** execution path; `ToolExecutor::execute()` has zero callers today. ⚠ **Confirmed.** ⚠ **And it will break three gates by succeeding** — `ci_gate.rs:811` asserts `total >= 3` *"expected at least 3 tool.execute call sites"*, plus the sibling checks at `:772-867` and `tool_trait.rs:169-209`. That floor was sized to **today's structure** rather than to the invariant, so collapsing to one path drives the count below it. **Re-point them in the same commit** — the invariant is *every execute site is wrapped, and there is at least one*, not *there are at least three*. (`L-C6`.) → `CODE-PRESENT` *(`ToolExecutor::execute()` **deleted**, not left unreferenced — `TG3-07`: *an unreferenced second path is a defect waiting for a caller*. Zero callers, verified. Deleting it broke two floor assertions sized to its existence, both re-pointed in this commit — `P3-67`.)*
- [x] **P3-11** Introduce `ToolEffect` (`Read`/`Write`/`Execute`/`Network`/`Board`) as the **only** prompting key; `is_read_only` reverts to a concurrency hint (G-11). ⚠ **Confirmed and understated** — see `P3-64`: on the live path the read partition consults **no** permission state at all, so `is_read_only()` is not merely the wrong key, it is an unconditional bypass. ⚠ Lands together with `P3-56` and the `is_read_only` revert, never separately (`C-6`). → `GATE-PROVEN` *(`TG-P3-11`, falsified. `ToolEffect` with **no default** on the trait, so a new tool is a compile error rather than a silent auto-approval. All 38 tools declare one. `is_read_only` reverted to a concurrency hint and now appears **nowhere** in the executor.)*
- [x] **P3-12** Honour `PermissionMode` — `ManualApproval` is passed unconditionally (`agent_commands.rs:150`). ⚠ **The mechanism is confirmed; the UI attribution is REFUTED (2026-09-02).** The PA-mode dropdown sets **`PAMode`, not `PermissionMode`**, and `PAMode` **is** honoured — it drives the system prompt (`context.rs:84-86`) and the tool registry (`builtin_tools/mod.rs:190,260`, where Delegate strips every file tool). Implementing this as written would **fuse tool availability, prompt and permission policy onto one three-valued control**, which is a product change nobody asked for. The dropdown that genuinely does nothing is the **recipe** one — see `P3-65`. → `SMOKE-PROVEN` *(`p3_12_01..04` in `tests/s3_permission_modes.rs`. The refutation stands and the task is closed against the invariant rather than its original wording: **a permission mode must reach a different decision from the others, for the effects it is defined to differ on.** The smoke proves all three modes are reachable, that no two share a decision profile, that Autonomous prompts for nothing (the fleet depends on it) and that the other two prompt for something (the user depends on it). `agent_commands.rs` keeps `ManualApproval` **deliberately**, now documented at the site: a user sitting in front of the assistant is present to answer, so the strictest interactive policy is the right default, and fusing the PA-mode dropdown to consent policy would be a product change nobody asked for)*
- [x] ~~**P3-13** Populate `ToolContext` truthfully~~ — **MOVED to `P0-13a`** (2026-08-31). It must ship in the same commit as the `/mcp` deletion, or ACP identity is dead from P0 to P3. Task ID retained and struck rather than renumbered, per §0.14.
- [x] **P3-14** Route **every** path-taking tool through `path_jail` — six bypass it today, two of which act on the path. → `GATE-PROVEN` *(`TG-P3-14`, falsified. **Seven** tools, not six — the gate found `write_interface_spec` too. Keyed on *declares a path input and touches the disk*, so a NEW tool with a path and no jail call fails the build, which is `TG3-27`'s requirement. `analyze_diff`'s context parameter was named `_ctx`: the workspace was never consulted at all.)*
- [x] **P3-15** `path_jail` **refuses** an unnormalisable path, never rewrites it. ⚠ **Confirmed by execution, not by reading:** the function was compiled verbatim and run — `validate_workspace_path(<ws>/newdir/sub/../x.txt)` returns **`Ok(<ws>/newdir/sub/x.txt)`**, a *different* path, accepted; and `write_file.rs:76` uses that return value **as the write destination**. Second step: `strip_verbatim_prefix` turns `\\?\UNC\server\share\…` into a **relative** path. ⚠ **Open measurement:** a jail *escape* could not be produced on Windows (Win32 normalises `..` before `exists()`); the same loop should escape on **Linux**, and that is the single highest-value follow-up measurement in this phase. → `UNIT-PROVEN` *(The mechanism named: `Path::file_name()` returns **None** for `..`, so the tail-rejoin loop skipped it and returned a *different* path as `Ok` — which `write_file` used as its destination. And `strip_verbatim_prefix` turned a verbatim UNC path into a **relative** one. Both refuse now, with the benign no-fire half tested: a resolvable `..` and ordinary paths are still accepted.)*
- [x] **P3-16** Normalise the `path_lock` key — canonicalise and case-fold, through **one shared helper** also used by the graph DB key and checkpoint retention (a **fourth** instance was found). ⚠ **Worse than written, and the fix is not where the task points.** Beyond case-folding: `write_file` locks the **pre-jail** path while `edit_file` and `append_file` lock the **post-jail canonical** one — probed, and they land in **different lock slots for the same file**. So the lost-update race that `edit_file.rs:111-118` documents in its own comment is **live today** between `write_file` and `edit_file`. **The fix must change `edit_file`/`append_file` to lock what `write_file` locks (or all three to lock the canonical form) — changing only the key normalisation leaves the race intact.** → `UNIT-PROVEN` *(The shared helper is `security::path_key::path_key`, and `lock_key` now routes through it. **The race itself is closed by the caller change**: `write_file` locked the pre-jail path, and now locks the jailed one, as `edit_file` and `append_file` already did. Three tests in `path_lock::p3_16_tests` — every spelling takes one slot, case folds only where the platform does (with the Linux no-fire half), and **the pre-jail and post-jail forms agree**, which is the assertion that would have failed before the change)* 🔴 **SCOPE CORRECTED, 2026-09-02 (`F-15`).** The note says the helper is *"shared with the graph DB key and checkpoint retention"*. ~~It is.~~ **It has exactly one consumer: `path_lock`.** Neither of the other two exists in the tree yet — the graph path key is `P5-02`'s work (which already cites this helper by name), and checkpoint retention (`G3b GI-28`) has no module at all. The helper and the race fix are real and tested; the *sharing* was written in the future tense and read as the present. A helper with one caller is not yet "one shared helper", and saying so is how `L-C12` gets missed.
- [x] **P3-17** No lock held across an unbounded await; every subprocess gets a timeout (`validate.rs` has **zero**). → `GATE-PROVEN` *(`TG-P3-17`, falsified three ways. All five spawn sites — including the `pyright --version` probe — go through `run_bounded`, a 30s ceiling with `kill_on_drop(true)` so a timed-out child is killed rather than orphaned. The gate asserts the **invariant** (zero `.output()` outside the helper), not the presence of the fix: a planted **sixth** unbounded spawn site fails it)*
- [x] **P3-18** A missing validator binary is an **error**, not a warning silently reported as `"✓ No syntax errors"` (confirmed verbatim). ⚠ **Must land in the same change as `P3-17`.** A timeout added to `validate.rs` returns `Err(_)`, which lands in the branches that produce `severity: "warning"`, and `is_clean()` ignores warnings — **so the timeout fix alone converts a hang into `✓ No syntax errors`**, which is strictly worse than the hang. → `UNIT-PROVEN` *(Landed in the same change as `P3-17`, as required. `is_clean()` is now `self.ran() && no errors`, backed by a third severity `SEVERITY_UNAVAILABLE` — so "I checked and found nothing" and "I could not check" stopped being the same verdict. `format_inline` renders `NOT CHECKED` rather than `0 error(s)`. **razi-ruled 2026-09-02:** a checker that could not run is *not* an error — `check_file` returns success with the NOT-CHECKED note, because an uninstalled `tsc` is a coverage gap the model cannot repair by retrying. Seven tests, including a real process spawned, hung, and killed)*
- [x] **P3-19** Never resolve an executable from the workspace under inspection (`resolve_python` runs `<workspace>/.venv/…`). → `GATE-PROVEN` *(`TG-P3-19`, falsified twice. `resolve_python` no longer takes a workspace at all - the parameter is gone rather than ignored, so the capability cannot be edited back in. **Scope confirmed by measurement, not assumption:** a probe established that `Command::new("name").current_dir(untrusted)` does NOT resolve the program from the child directory on this platform, so `.current_dir(working_dir)` on the other four checkers is not a second vector. The gate flags a NEW helper of the same shape, not just this one)*
- [x] **P3-20** Review **every** registered tool's description against its code and rewrite those that misdescribe it. ⚠ **The "33 of 38" figure is OVERSTATED (2026-09-02)** — a six-tool sample did not support it, and the count is not reproducible from the grounding report. The *defect* is real and `TG3-36` gates it per-tool; the **number** is struck rather than repeated, because a figure nobody can reproduce is not evidence (`§0.2L-1`). → `DOC-GATE` *(**The struck "33 of 38" figure is now settled: the real count is 5.** A tool-by-tool audit of all 38 descriptions against their code found `glob` claiming to respect `.gitignore` when it has no gitignore handling whatsoever (only a fixed skip list - so ignored files, `.env` included, appear in results a model believes are filtered); `remember_this` promising facts appear in "every future system prompt" when injection is skipped for recipe mode, subagents and MCP - i.e. the fleet's own operating mode; `check_file` claiming its syntax tier "Always runs" (it is opt-out-able), omitting the `.ts/.tsx/.jsx` coverage it has, and describing skipped tiers as silent after `P3-18` made them say NOT CHECKED; `spawn_parallel_subagents` stating a floor of 2 where the schema allows 1; and `assign_tasks` asserting an unmeasured "~10x faster", replaced by the mechanism that is actually true. The other 33 were verified accurate against their code. `TG-P3-20` gates the class both ways and was falsified - **and caught blind on its first falsification**, see `L-25`)*
- [x] **P3-21** Fix `e2e_probe` — arbitrary shell with jail, evidence and output capture all absent. → `GATE-PROVEN` *(`TG-P3-21/68`, falsified four ways. **Evidence:** both child streams were `Stdio::null()`, so a failed probe reported FAIL and could not say why - while this file's own header lists "dev server crashed on boot" as a case it catches. Each child now drains stdout+stderr into a bounded 4 KB **tail** (the reason for death is printed last) which reaches the report as `start_cmd_output`. ⚠ Piping without draining deadlocks once a dev server fills the 64 KB pipe buffer, so each stream gets its own reader task rather than a `wait_with_output`. **Consent:** the tool declared `Network` under a comment reading "the higher-consequence effect wins" - and picked the lower one. Running arbitrary shell subsumes making a request. The two prompt identically in every mode, so nothing changed about *whether* the user is asked; what changed is **what they are asked** - from "send data from your machine to an external service" to "run a command on your machine", which is what actually happens)*
- [x] **P3-64** 🔴 **The concurrent read partition performs NO permission check at all.** `tool_executor.rs:338-339` runs read-only tools under the comment `// Execute (read-only always approved)`, consulting neither `self.mode` nor `always_allow` — and `:309-310` **clones both and binds them as `_mode` / `_always_allow`**, underscore-prefixed to silence the unused warning. So on `execute_batch`, the only live path, `is_read_only() -> true` is an **unconditional bypass of every permission mode**. Combined with `P3-19` (`check_file` resolves interpreters from the *inspected workspace*) and `check_file` declaring itself read-only, **cloning a hostile repository and opening a `.py` file is arbitrary code execution, unprompted, in every mode.** This is the phase's highest-priority defect and no task named it. → `GATE-PROVEN` *(`TG-P3-11`. Permission is decided **before** the partition, sequentially — so every tool passes exactly one gate, concurrent tools cannot race competing dialogs, and the user is asked in the order the model asked. The dead `_mode`/`_always_allow` bindings are gone.)* 🔴 **RETRACTED IN PART, 2026-09-02 (`G-1`).** The note claims *"every tool passes exactly one gate"*. ~~It did.~~ **Every write/execute tool was prompted TWICE** — the pre-partition gate this task added, plus the pre-existing one inside the write partition, which I did not remove. Worse than a duplicate prompt: the second minted a fresh `Uuid::new_v4()` call id, so the user saw an unrelated request for a call they had just approved; only `always_allow` hid it. ⚠ **`TG-P3-11` was green throughout**, because it asserted the ABSENCE of `is_read_only` — the shape of this fix — rather than the invariant the fix existed to establish. `§0.16 L-C6`, written into a gate an hour after recording L-C6's seventh instance. The gate counts gates now.
- [x] **P3-65** **`Recipe::permission_mode` has zero read sites in any `.rs`.** Four shipped recipes declare `Supervised`; `runner.rs:768` hardcodes `PermissionMode::Autonomous`. This — not the PA-mode toggle — is the control that genuinely does nothing (`P3-12`). → `UNIT-PROVEN` *(`TG-P3-65`, falsified three ways, plus six unit tests. razi-ruled 2026-09-02: wire the field and flip the unattended recipes. `runner.rs` now builds the engine from `PermissionMode::from(&recipe.permission_mode)`; `worker-platform`, `qc-agent` and `qc-dispatcher` declare the Autonomous their cron schedules require, and `fleet-smoke` (`schedule: None`, human-triggered) keeps `Supervised` and now genuinely gets it. **The dropdown was live in the UI all along** - `RecipeEditor.tsx` exposes `permission_mode` next to the schedule field, so users could already select a setting that did nothing. That also means the invalid pair (scheduled + supervised) is two clicks away in user-authored recipes, where no source gate can reach: `run_recipe` now **refuses** it with an actionable message rather than silently downgrading to Autonomous (which would re-create this very defect as a "convenience") or honouring it (which would stall the fleet for 300s per tool and then deny))* 🔴 **REACHABILITY RETRACTED, 2026-09-02 (`G-17`).** The wiring above is correct and tested, and **for every shipped recipe it is unreachable**. The only remaining `Supervised` recipe is `fleet-smoke`, which is `fallback_policy: Mode2Only` — and the Mode2Only check returns from `run_recipe` **before** the engine is constructed, so `PermissionMode::from(&recipe.permission_mode)` never executes. ~~"fleet-smoke keeps `Supervised` and now genuinely gets it"~~ — it does not, because it does not run at all. **What IS live:** the `unanswerable_prompt_config` refusal, which sits at the top of `run_recipe` before any branch, and the whole path for user-authored `Mode1Capable` recipes. **The underlying cause is filed as `P4-31`** — eight of nine shipped recipes cannot run, for reasons that have nothing to do with permissions. ✅ **RESOLVED 2026-09-02:** `P4-31` is closed — `fleet-smoke` is `Mode1Capable` now, so it reaches the engine and the `Supervised` → `SmartApprove` mapping executes. The retraction above stands as the record of what was true for one day.
- [x] **P3-66** **`PermissionMode::SmartApprove` has zero construction sites** — an unbuildable enum variant with two dead `match` arms (`tool_executor.rs:160`, `:437`). Either build it or delete it; a mode that cannot be selected is a guard that cannot fire. → `UNIT-PROVEN` *(The mapping in `P3-65` is `SmartApprove`'s first and only construction site, so its two live `match` arms are now reachable. The pairing is grounded, not convenient: `Supervised` is documented as *"prompts before destructive operations"*, which is exactly `SmartApprove` (prompt on Execute/Network/Board, pass Read/Write). `ManualApproval` would prompt on an ordinary file write too - a stricter policy than the one the recipe declares. `p3_12_02` asserts no two modes share a decision profile, which is what stops any of the three decaying back into decoration)*
- [x] **P3-67** **Re-point the three execution-path gates** so they assert the invariant rather than today's structure: *every execute site is wrapped, and there is at least one*. As written (`total >= 3`) they fail when `P3-10` succeeds. Ships in `P3-10`'s commit. → `GATE-PROVEN` *(Both floors re-pointed from *"at least 3 sites"* to *"at least one, and every one is wrapped"*. Sized to structure, they failed when the fix succeeded.)*
- [x] **P3-68** **`e2e_probe`'s `probe_url` is an unrestricted SSRF with readback** — it fetches any URL the model names and returns the body, so cloud metadata endpoints and internal services are reachable and their contents returned to the model. Part of `P3-21`'s surface but distinct from the shell half. → `GATE-PROVEN` *(`TG-P3-21/68`, falsified. `probe_url` is confined to loopback **by resolution, not spelling** - every address the host resolves to must be loopback, so `127.0.0.1.nip.io` and any attacker-controlled DNS name pointing at `169.254.169.254` are both refused; a string match on "localhost" would have let them through. Non-http schemes are refused too. The check runs **before any child is spawned**, and the gate asserts that ordering: rejecting the URL after booting the model's shell commands would not be a check. The tool loses nothing - its entire purpose is probing a server it started itself, seconds earlier, on this machine. Tested against the real EC2 metadata URL)* 🔴 **RETRACTED IN PART, 2026-09-02 (`F-19`).** The confinement guarded the **first request only**. No redirect policy was set, so `reqwest` followed up to ten hops by default — any server the model could make respond, **including the one it had just started itself**, could answer `302 Location: http://169.254.169.254/…` and the body still came back to it. The check held the front door and left the hallway open. **Now fixed:** every hop is re-validated by the same function as the first, capped at five. ⚠ **Residual, stated rather than closed:** validation and the eventual connection resolve the host independently, so a DNS-rebinding attacker who can also get the model to name their hostname is not excluded. Closing that needs the connection pinned to the validated IP.
- [x] **P3-55** **Release the provisional charge on every exit path of `spawn_qc_review`** — it records **$0.10 before eight distinct error returns** and reconciles none of them (`G10-67`). 50 dispatcher ticks against an already-claimed task trip the **$5 hourly ceiling** and **block all QC for an hour at zero real spend** — the fleet disabling its own review capacity while the ledger says it was busy. Same shape as `P3-05`: every exit path must do the thing, so the `Drop` guard covers both. *(razi-ruled into P3, 2026-09-02 — `TG3-37` existed with no owning task.)* → `GATE-PROVEN` *(`TG-P3-55`, falsified three ways with **compiling** plants (see `L-26`). The $0.10 provisional charge is now an RAII `ProvisionalCharge` that releases on `Drop` unless `handed_off()` is called once the reviewer is in flight - so all eight error returns between the charge and its settlement release it without a line of their own, and the ninth will too. The gate asserts the property that makes the guard load-bearing: **the handoff sits below the last early return**. A handoff placed above even one failure path leaves it leaking while every other check still passes. Test reproduces the dominant case exactly - 50 dispatcher ticks against an already-claimed task leave $0.00 on the ledger where they previously left $5.00, the full hourly ceiling, disabling QC for an hour at zero real spend)*
- [x] **P3-56** **A `Network` effect prompts.** `analyze_diff` declares `is_read_only = true`, is therefore never prompted in any mode, and **uploads the full diff of any repository to a third-party LLM** — under BYOK, the user's own code on the user's own key, with no prompt. The prompting half of `TG3-26`; the consent **store** is P4's, recorded rather than orphaned. ⚠ Lands with `P3-11`, never before it (`C-6`). *(razi-ruled into P3, 2026-09-02.)* → `GATE-PROVEN` *(`TG-P3-56`, falsified. `analyze_diff` declares `Network` and prompts in every mode but Autonomous; it previously declared `is_read_only = true` and uploaded the full repository diff to a third-party LLM with no prompt.)*

### P3-D · Subagents
- [x] **P3-22** **Prune terminal entries BEFORE acquiring a permit, and release the permit at termination rather than at prune.** ⚠ **The task as written was refuted; the symptom it names is real via a different mechanism (2026-09-02).** The runner is *already* in-process on tokio tasks (`manager.rs:106`), and `PoolPermit` *is* an RAII drop-guard (`pool.rs:45-72`) — the leak originally described was closed by the P0 audit (`F-02`). **What actually deadlocks:** the permit is held by the *registry entry* (`manager.rs:42,128`) and released only when `prune_terminal`'s `retain` drops it — but `spawn` calls `acquire` at `:78` and `prune_terminal` at `:120`, so **the prune that would free permits sits behind the acquire that is waiting for them**. Terminal entries are held for `TERMINAL_RETENTION = 60s`, and `prune_terminal`'s only other caller is `list()`, reached solely from Tauri commands the UI drives. **So with the subagent monitor panel closed, 16 completed subagents block the 17th spawn permanently.** ⚠ `finalise`'s own docstring claims *"The pool permit is released here"* — it does not; it stamps status only. → `GATE-PROVEN` *(`TG-P3-22`, falsified twice with compiling plants. **Both halves the task named, and the second is the real fix.** The permit now lives in `Option<PoolPermit>` and is released at termination, so `TERMINAL_RETENTION` keeps the ENTRY for the UI without keeping the CAPACITY - which is what made 16 finished subagents consume the whole global cap. The prune also moved above `acquire`, because a sweep that can only run after the wait it is meant to end is wrong on its own terms. ⚠ **There were TWO terminal-stamping sites, not one:** `finalise` and the event pump (the latter added by the P0 audit for `F-02`). Releasing in only one would have left the never-awaited path still leaking, so the gate asserts the invariant - *every* site that stamps `terminated_at` releases the permit - and a third site fails it. `finalise`'s docstring already claimed "The pool permit is released here"; it stamped status and nothing else, and that false sentence concealed the deadlock for as long as it was believed)*
- [x] ~~**P3-23** `depth + 1` from the parent's real context, with a hard cap.~~ → **ALREADY DONE, verified 2026-09-02.** Depth is threaded `AgentIdentity → ToolContext → config.depth` (`tool_executor.rs:129-134`, `spawn_subagent.rs:173`, `native_runner.rs:306-319`), there are **two** hard caps, and it is regression-tested. Struck rather than renumbered (`§1`). ⚠ One residue, carried to `P3-58`: the three builtin-tool depth guards test `>= 3` on the **parent** while the pool tests `> 3` on the **child**, so the pool's depth branch is **unreachable from any builtin tool** — a guard that cannot fire. → `UNIT-PROVEN`
- [x] **P3-24** ~~Timeout and~~ **propagated cancellation**. ⚠ **Half already done (2026-09-02).** The timeout exists and fires — `native_runner.rs:221-234`, 300 s default. **Cancellation genuinely does not propagate:** children are detached tasks, and dropping the awaited `JoinHandle` *detaches* rather than aborts, so a cancelled parent leaves its children running to completion and billing. → `UNIT-PROVEN` *(`TG-P3-24`, falsified twice, plus two unit tests. `SubagentManager::abort_children_of(parent_slug)` stops every live child, and the engine calls it on **both** cancellation paths. The key is exact: `spawn_subagent` and `spawn_parallel_subagents` both set `parent_slug: ctx.session_id`, so cancelling session X stops X's children and nothing else - `spawn_qc_review` uses the dispatcher slug and is deliberately unaffected, since a QC review is not owned by whichever interactive session happened to be open. ⚠ **The pre-start cancel path was found by the gate, not by me** - a session can be cancelled while children from an earlier run are still live. The gate asserts the helper is **called**, not merely defined (`L-C12`, seven instances, every one of which passed a gate that checked only for a definition), and that ids are collected before terminating, since `terminate` takes the same registry lock)* 🔴 **RETRACTED IN PART, 2026-09-02 (`G-9`).** `abort_children_of` was correct and **could not actually stop anything**. `await_result` consumes the `JoinHandle`, so `terminate` found `entry.task == None`, **skipped the abort entirely**, removed the registry entry and returned `Ok(())` — reporting success while the subagent ran on to completion and billed. ⚠ That is the ONLY case that matters: `spawn_subagent` awaits its child, so **every in-flight subagent is one whose handle has been taken**. My two tests ran against an **empty registry** and could not have exercised it — the no-fire halves were the only halves. **Now fixed:** the registry entry keeps a `tokio::task::AbortHandle` captured at spawn, independent of the `JoinHandle`'s lifetime, and `terminate` uses whichever survives. The new test holds a real running task, takes its handle as `await_result` would, and asserts the abort still reaches it.

### P3-E · Context strategy
- [x] **P3-25** Layered assembly: ~~cached prefix~~ → live history → **`clear_tool_uses`** → ~~compaction~~. ⚠ **Two of the four layers landed in P2 (2026-09-02).** The cached prefix and compaction both exist and run every turn. **`clear_tool_uses` exists nowhere** — not in the code and not on the wire (no `anthropic-beta` header, no `context_management` field). So this task is now *the clear layer and the ordering that places it*, not four layers from scratch. → `BENCH-PROVEN` *(`p3_25_bench_…` in `live_provider_smoke.rs` - **`BENCH-PROVEN` against the live API, 2026-09-02**. Six tool round-trips, identical conversation sent twice: **11,310 input tokens without the clear layer, 2,750 with it - 8,560 freed, 75.7%**, and `applied_edits` reported `cleared_input_tokens: 8515, cleared_tool_uses: 5`. The bench runs through `AnthropicProvider::complete`, so it proves the adapter's wiring end to end rather than a hand-built curl body. ⚠ **The wire shape was established by execution, not documentation (`L-22`):** the same body WITHOUT `anthropic-beta: context-management-2025-06-27` returns **400 `context_management: Extra inputs are not permitted`**, and WITH it returns 200 - so the header and the body field are set together or not at all, which a unit test asserts. **The ordering is the deliverable**, not the field: `CLEAR_TRIGGER_FRACTION = 0.60` sits below `COMPACTION_THRESHOLD = 0.80`, and that inequality is asserted, so the cheap server-side layer always gets its chance before the summarisation call)*
- [x] ~~**P3-26** Compaction trigger reads `ModelInfo::context_window`, not a hardcoded 200,000.~~ → **ALREADY DONE in `P2-57`, verified 2026-09-02.** `engine.rs:343-353` reads it; 200,000 survives only as the unknown-model fallback, and the lookup **does** resolve (`provider.models()` → `models_static()` → `catalogue::models_for()`, ids matching `models.ts`, and `CappedProvider` forwards `models()`). Struck, not renumbered. ⚠ **But its guarding test is wrong — see `P3-59`.** → `UNIT-PROVEN`
- [x] ~~**P3-27** **Use** the compacted history — the caller discards it today.~~ → **ALREADY DONE, verified 2026-09-02.** `history = compacted;` at `engine.rs:371`. The value that genuinely *was* being discarded — compaction's own `usage`, a real billed call — was folded into the turn total in `P2-66`. Struck, not renumbered. → `UNIT-PROVEN`
- [x] **P3-28** Decide the clear threshold **from the measured cache-write cost**, once `clear_tool_uses` exists. ⚠ **Refuted as written (2026-09-02):** neither `clear_at_least` nor `clear_tool_uses` exists to tune. **And the premise does not hold today** — no *message-region* breakpoint is ever set (`anthropic.rs:169-176` marks the system block only), so a clear would force **no cache write at all** and the trade-off the task describes is currently vacuous. The threshold therefore has to be derived *after* `P3-25` lands the clear layer, and re-derived if a message-region breakpoint is ever added. → `BENCH-PROVEN` *(**The premise is confirmed vacuous, and that IS the derivation.** Clearing edits the message region, so it would forfeit a cache write only if a breakpoint were set there - and `anthropic.rs` marks the **system block only**. Nothing in the message region is cached, so there is no cache-write cost to trade against, and a threshold "derived from the measured cache-write cost" would be derived from zero. The numbers are therefore derived against what clearing **does** cost - tool-result content the model may still need: trigger at 60% of the window (leaving a 20-point band before compaction at 80%), `keep` = 3 most recent tool uses (a read -> edit -> verify cycle, the shortest loop that needs its own history), and `clear_at_least` = 5% so a clear that frees almost nothing does not happen. ⚠ **Recorded at the site: if a message-region cache breakpoint is ever added, these must be re-derived** - the trade-off stops being vacuous at that moment and 60% is no longer justified by anything)*
- [x] **P3-29** Bound every tool output; grep bounds per line; `list_directory` actually bounds entries. ⚠ **Confirmed and worse (2026-09-02).** `grep.rs:242-255` caps the match **count** and never the bytes, so one 40 MB minified line enters context whole. `list_directory.rs:116-168` checks its cap **only at function entry, never in the push loop**, so it is **fully inert when `recursive=false`** — the common case — and `:72`'s `truncated = entries.len() == max_entries` is `false` **exactly when the cap was overshot**, so the one signal that would reveal it reports clean. → `UNIT-PROVEN` *(Both halves, each reproduced by a test that fails against the old code. **grep**: `line_content` was `line.to_string()` - `max_results` caps the match COUNT and never the bytes, so one match on a minified bundle line put tens of megabytes into context while the cap reported nothing wrong. Matched lines AND context lines are now bounded at 2 KB. **list_directory**: the cap was checked only at function entry, never in the push loop - with `recursive=false` (`max_depth = 1`) there is exactly one call and one loop, so it was **fully inert in the common case**. And `truncated = entries.len() == max_entries` was `false` **exactly when the cap was overshot**, so the one signal that would have revealed it reported clean; it is `>=` now, exact because the loop can no longer overshoot and still honest if it ever can again)*
- [x] **P3-58** **Make the pool's depth branch reachable.** The three builtin-tool guards reject at `>= 3` on the **parent** while `pool.rs` rejects at `> 3` on the **child** — strictly tighter, so the pool's own depth check can never fire from any builtin tool. One boundary, asserted in one place. → `UNIT-PROVEN` *(One boundary, compared on one side. Both spawn tools dropped their local `const MAX_SUBAGENT_DEPTH: u32 = 3` and now test `ctx.depth + 1 > subagent::pool::MAX_NESTING_DEPTH` - the CHILD, which is the side the pool rejects on. Previously parent `>= 3` fired one generation before child `> 3` ever could, so the pool branch was unreachable from any builtin tool. Boundary proved by execution: a child exactly AT the limit acquires, one past it does not)* 🔴 **RETRACTED IN PART, 2026-09-02 (post-phase claims audit, `F-21`).** The note above claims the boundary moved from `ctx.depth >= MAX` to `ctx.depth + 1 > MAX`. ~~Those are different predicates~~ — **for `u32` they are the SAME predicate for every input**, so the task's original premise (*"strictly tighter… fires one generation earlier"*) was false AND my fix was arithmetically a no-op that claimed to change behaviour while changing none. The pool's depth branch remained exactly as unreachable. ⚠ The gate was worse than the fix: it **enumerated two files**, so it never examined the **third** guard in `spawn_qc_review.rs` — written in the very spelling the gate's own failure message called a defect. **Now actually fixed:** all three tool-side guards deleted, `pool.acquire` is the single decision (it is the first thing `manager::spawn` does and every caller surfaces its refusal), the gate sweeps the directory, and a test checks the arithmetic instead of reasoning about it.
- [x] **P3-59** **Point the compaction gate at the field production reads.** ⚠ **My own defect, from P2.** `catalogue.rs:377-383` asserts on `ModelCapabilities::context_window` (via `capabilities_for`), and production reads `ModelInfo::context_window` (via `provider.models()`) — **two different fields**. They agree only because both derive from the same `CatalogueEntry`, so a regression in `models_for()` alone would leave the test green while Gemini silently compacted at 200k again. This is `L-C6` — a check written from the claim rather than the invariant — **in the very test written to stop `L-18` recurring**. → `GATE-PROVEN` *(`TG-P3-59`, falsified with **the exact regression the old test could not see**: clamping `models_for`'s `context_window` to 200k while leaving `capabilities_for` correct. The new gate fails; the old capabilities-based check still passes (exit 0) - which is the proof that re-pointing was necessary, not cosmetic. The gate constructs a **real `GoogleProvider`** and calls the real `Provider::models()`, so it walks every hop the engine walks rather than reaching into the catalogue; it also asserts the two shipped fields agree, since a drift between them is a defect even though production reads only one)*
- [x] **P3-60** **A subagent's declared tool subset must be honoured.** `SubagentConfig.tools` is documented as *"tool subset the subagent is allowed to use"*, is written by all three spawn tools **and** the Tauri command, and has **zero readers** — the child receives the full registry. A containment promise the code does not keep, and the shape `§0.16` calls a guard that cannot fire. → `GATE-PROVEN` *(`TG-P3-58/60/61/62/63`, falsified. `ToolRegistry::restrict_to` + `AgentEngine::with_tool_subset`, applied **after** `register_for_mode` so a subset can only REMOVE what the PA mode granted - never widen access, which is asserted. An empty list means unrestricted, matching how all four writers use the field; making empty mean "no tools" would have silently disarmed every existing subagent, a fix worse than the defect)*
- [x] **P3-61** **A subagent must not be given a model its provider cannot serve.** `manager.rs:87-89` fills `DEFAULT_MODEL` (`claude-haiku-4-5`) **before** the config is cloned, which makes `native_runner.rs:163-167`'s `active_model` fallback dead code — so on a Google-primary install, which `models.ts` declares, **every default subagent sends an Anthropic id to Google**. `spawn_qc_review.rs:155-163` has the same defect against `claude-sonnet-5`. Route both through `provider::routing`. → `GATE-PROVEN` *(`TG-P3-58/60/61/62/63`, falsified. `pick_servable_model` asks whether the ACTIVE PROVIDER serves the model, which is the real question - `config.model.is_empty()` never held, because `manager.rs::spawn` fills `DEFAULT_MODEL` before the config is cloned, making the old fallback branch dead code. Order: requested if served, else the user's active model if served, else the provider's own catalogue default. **`spawn_qc_review`'s hardcoded `claude-sonnet-5` is fixed by the same change** - every subagent reaches `native_runner`, so the fix belongs at that chokepoint rather than at each caller, and a test covers the QC id explicitly)*
- [x] **P3-62** **Report a subagent's real cost.** `SubagentResult`'s cost and token fields are hardcoded `0.0/0/0`, and the QC **hourly dollar ceiling** is computed from them — so the ceiling self-cancels on every completed reviewer and only ever bounds in-flight concurrency. Pairs with `P3-55`: one is a charge never released, the other a cost never recorded. → `UNIT-PROVEN` *(`TG-P3-58/60/61/62/63`, falsified. `AgentEngine::last_run_accounting()` captures what `record_spend` computed - and `record_spend` already runs on every exit path including the error ones. The runner reads it **before** unwrapping the timeout outcome, so a subagent that timed out at 600s still reports what it spent; reporting zero there would hide precisely the runs the ceiling exists to catch. Both result arms carry it, and the gate asserts `cost_usd: 0.0` appears zero times)* 🔴 **RETRACTED IN PART, 2026-09-02 (`F-23`).** The note claims *"a subagent that timed out at 600s still reports what it spent"*. ~~It did.~~ **It reported $0.00.** `tokio::time::timeout` **drops** the future rather than letting it return, so none of `record_spend`'s exit paths ran, `last_run_accounting()` stayed `None`, and the timeout case — the one the note singles out as the reason for the design — was the one case that did not work. Reading the accounting *before unwrapping the outcome* was necessary and not sufficient: there was nothing to read. **Now fixed** by stamping the accounting after every accumulation, so whatever has been spent is already recorded when the future is dropped mid-turn.
- [x] **P3-63** **`max_turns` is defaulted and then discarded.** Defaulted at `manager.rs`, never read by the runner, so a caller-set bound on subagent turns does nothing. → `UNIT-PROVEN` *(`TG-P3-58/60/61/62/63`, falsified. `TaskBudget` already carried `max_turns` and the engine already enforced it - `config.max_turns` simply was never connected to it. A caller-set bound now reaches the enforcer; zero still means "use the worker default")*

### P3-F · Memory
- [x] **P3-30** SQLite + `sqlite-vec` + FTS5 behind a retrieval-backend trait. → `LIVE-DB-PROVEN` ✅ **CONFIRMED before building (§0.2m), by execution.** `sqlite-vec`'s entire `[dependencies]` is `cc` — it does **not** depend on `libsqlite3-sys`, so there is no version to conflict with our pinned `rusqlite 0.31` / `libsqlite3-sys 0.28`; it compiles `sqlite-vec.c` with `-DSQLITE_CORE` and never needs `loadable_extension`, so that feature never meets `bundled`. A scratch crate on the repo's exact pins built on Windows MSVC, exit 0. **FTS5 is already present** in the bundled SQLite 3.45.0 with no extra flag — measured here by creating a virtual table, inserting and matching. All three legs coexist. ⚠ **`sqlite3_auto_extension` is process-global** (verified), so registration must happen **once at startup, before any subsystem opens a database** — not lazily inside `MemoryDb::open()`. ⚠ Use **`0.1.9`**, the latest stable; `0.1.10` is alpha. ⚠ Forward risk recorded: `sqlite-vec` compiles against a bundled `sqlite3.h` declaring 3.45.3 while linking 3.45.0 — it links and runs today, but that skew wants re-checking on any version bump. *(`memory.db` **v2 and v3** + `memory/{vector,backend,manager}.rs`. 🔴 **v2 alone was a silent failure and the note originally stopped there.** v2 backfilled the existing rows into the retrieval index and stopped: `episodic.rs` and `procedural.rs` went on writing to their own tables and nowhere else, so **every conversation turn and every topic created after the migration was invisible to recall** — permanently, with no error, the subsystem reporting healthy. The v2 comment claimed *"everything is written through one seam so the two cannot diverge"*; it was false when written and is corrected in place. Worse, insert triggers alone would have left **"forget this" not forgetting** — `deactivate_fact` and `delete_topic` removed the tier row and left the index copy in scope, still injected into the prompt every turn, while every surface reported the removal as done. **v3 (`memory-index-triggers`)** moves indexing into the database: insert, update, deactivate and delete across all three tiers, a catch-up backfill, and a catch-up **deletion** so the upgrade cannot resurrect a fact forgotten during the v2 window. `LIVE-DB-PROVEN` — the live database is at v3, integrity `ok`, 39 rows intact, backups taken by the guard before it applied. Registration happens BEFORE the connection opens - `sqlite3_auto_extension` only affects connections opened after it runs, so registering afterwards would leave that very connection without `vec0` and fail later as `no such module`, far from the cause. Three belts: startup, `MemoryDb::open`, the migration. FTS5 proven by creating a virtual table and matching, not by reading a feature list)*
- [x] **P3-31** **Scope as a declared `vec0` partition key** — not a post-filter over the top-k, which would return few or zero results (G-12). → `GATE-PROVEN` ✅ **CONFIRMED before building (§0.2m) — and it is a genuine PRE-filter, proven by a discriminating fixture.** A probe was designed where post-filtering and pre-filtering give *different* row counts: `k=2 AND scope='a'` returned two rows including one at distance 1.4142 that a post-filter over a global top-k could never surface. Partition keys arrived in `0.1.6-alpha.1`. ⚠ **Limits measured, not assumed: at most 4 partition columns, and TEXT/INTEGER only** — FLOAT and BLOB are rejected, with a misleading error that names the *vector* column rather than the offending one. So `scope` fits, but the budget is four and the type choice is forced. ⚠ Hard-depends on `P3-30`: `sqlite-vec` is not yet a dependency. *(`TG-P3-31`, falsified. Scope is a `vec0` partition key AND a SQL pre-filter on the lexical leg. 🔴 **QUALIFIED 2026-09-02 by the gate verification (`TG3-13`).** The first half of that sentence is true of **sqlite-vec**, not of this product: `memories_vec` is `SELECT`ed in `embed.rs` and is **never created by any migration and never written** — the ladder's three rungs declare no `vec0` table at all. The discriminating fixture that proves the pre-filter creates its **own throwaway `vec0` table**, so what it establishes is that the library honours a partition key, which was worth establishing and is not what the note claimed. The lexical half is real and shipped. **The vector leg is a declared column, a `SELECT` and no table**, behind an off-by-default feature — `L-C12` at schema level. It must be created and written before `embeddings` is ever enabled in a release build, or `VectorBackend::search` fails `no such table: memories_vec`; recorded as owed alongside `P3-33`'s vendoring call. Proven with a **discriminating** fixture rather than an assertion: scope `b` holds both global nearest neighbours, and a `k=2` search in scope `a` still returns two `a` rows including one at distance 1.4142 that a post-filter over the global top-2 could never surface - which is exactly the difference `G-12` names. ⚠ **An empty scope list returns NOTHING, never everything**; "unscoped" is how cross-project contamination happens, and `S3-04b` proves the boundary holds both ways)*
- [x] **P3-32** `fastembed-rs` / BGE-small-en-v1.5, **384 dimensions**. → `UNIT-PROVEN` 🔴 **PLATFORM-GATED — razi-ruled 2026-09-02, after a verified upstream blocker (§0.2m).** Our macOS DMG is a **universal binary** (`installers/dmg/create-dmg.sh` builds `universal-apple-darwin`) and CI targets both `aarch64-` and `x86_64-apple-darwin` (`build-release.yml`). **Microsoft ships no x86_64 macOS ONNX Runtime in any release from v1.25.0 to v1.29.0 — `osx-arm64` only**, checked across eight releases; `ort`'s own manifest is likewise absent (HTTP 404). A universal binary needs **both** slices to link, so adding `fastembed`/`ort` unconditionally does not degrade Intel macOS — **it fails the macOS DMG outright**, and vendoring cannot fix what upstream does not publish. **Ruling: the embedding backend sits behind a Cargo feature that is OFF for macOS.** Windows and Linux get semantic recall; macOS gets FTS5/BM25 and **reports itself degraded** — which is the architecture spec `03 §4.4` already designs ("degradation, not failure"), not a workaround. No platform is dropped. `P3-33`, `P3-35` and `P3-37` inherit the same boundary. *(`memory/embed.rs`, behind the `embeddings` feature. **384 dims**, asserted against the `vec0` column width so the schema and the embedder cannot disagree. macOS compiles it out entirely - razi-ruled, and upstream-forced)*
- [x] **P3-33** **Pin and vendor the ONNX runtime** — `ort`'s default features download it from a third-party CDN at build time (G-14). → `CODE-PRESENT` 🔴 **THE MECHANISM IS WRONG — the fix as written would have silently done nothing (§0.2m).** `ort`'s defaults do include `download-binaries`, but **`fastembed` already depends on `ort` with `default-features = false`** (verified against the registry: `ort req==2.0.0-rc.10 default_features=False features=[ndarray, std]`). ort's defaults are **already off**, and ort is not a direct dependency of ours — so "disable `ort`'s default features" changes nothing while looking like a fix. `cargo tree -e features -i ort` names exactly one enabler: **`fastembed`'s own `ort-download-binaries-native-tls` feature**. The flag belongs on `fastembed`. ⚠ **And the real opt-out has a cost the task does not mention:** it takes the tree from 332 to 109 packages but also drops **`hf-hub`**, which removes `TextEmbedding::try_new` entirely — i.e. turning off the build-time download also removes the runtime model loader, so a vendored-runtime build needs its own model-acquisition path. *(`TG-P3-33`. ⚠ **The task's prescription was a no-op** - `fastembed` already depends on `ort` with `default-features = false`, and `ort` is not a direct dependency of ours, so "disable ort's defaults" would have changed nothing while looking like a fix. **Pinned exactly (`=6.0.2`) and made optional, off by default**, so a default build and CI fetch nothing. ⚠ **"Vendor" is NOT done and is not deferrable by me**: it needs somewhere to host a 30 MB runtime, which is a distribution decision. Recorded as razi's open call before embeddings enter a release build. ⚠ The optionality assertion is credited as **not independently falsifiable**: removing it makes Cargo reject the manifest, so Cargo enforces it more strongly than the gate - stated because I tried and got a manifest error rather than a gate failure (`L-26`))*
- [x] **P3-34** Set the fastembed cache path **explicitly under the app data root** — it defaults to CWD-relative, an INV-5 violation (G-14). → `UNIT-PROVEN` 🔴 **UNDERSTATED — setting the path is not sufficient (§0.2m).** `with_cache_dir` is **not authoritative**: `pull_from_hf` discards it whenever **`HF_HOME`** is set. Any machine with `HF_HOME` exported — common wherever Python ML tooling or Ollama is installed — silently relocates the 133 MB model outside the app data root, which is the INV-5 violation this task exists to close. The task must therefore **neutralise `HF_HOME` for the embedding call**, not merely call `with_cache_dir`, and prove it with the variable set. *(⚠ **The task was understated and the fix is bigger than it asked for.** `with_cache_dir` is not authoritative: the model puller discards it whenever **`HF_HOME`** is set, and any machine with Python ML tooling or Ollama exports one - so the 133 MB model would land outside the app data root by a route the task never named. The fix **sets** the override variables to our directory rather than unsetting them, because unsetting only defends against the ones we know about while pointing them at our path works whether the library prefers the option or the environment. ⚠ `std::env::set_var` is process-wide and the first version had no lock - **its own tests raced each other into failure twice**, which is the cheapest possible proof the hazard is real. Now mutexed and restored)*
- [x] **P3-35** Model download is visible, resumable and **skippable**; skipping degrades to lexical. → `SMOKE-PROVEN` *(`DownloadPolicy` + `availability_report`, `S3-05`. The user's choice is checked **before anything is fetched** - a skip that takes effect after the download starts is not a skip. ⚠ **Four distinct unavailable states**, deliberately: a platform that can never have it, a build without the feature, a declined download, and one not yet finished each need a DIFFERENT action from the reader, and collapsing them is how a fixable problem reads as permanent. An absent setting means **allowed**, so a missing value cannot become a second silent "off" on top of the feature already being off)*
- [x] **P3-36** Retrieval degrades to FTS5/BM25 when the model is absent — **never empty, never an error**. → `GATE-PROVEN` *(`TG-P3-31` + `S3-05`. The lexical leg is a **complete backend**, not a branch inside the vector one - on macOS it is the only backend, so it is first-class rather than pitied. Degradation is **observable state** with a reason a user can act on, never inferred from an empty list. Proven: with no model, recall still returns results and says why it is limited)*
- [x] **P3-37** Embedding is async and off the runtime (`spawn_blocking`); it never delays a turn. → `BENCH-PROVEN` *(Inference runs on `spawn_blocking`. `fastembed` is synchronous and Tokio-free; on an async worker it stalls streaming, tool execution and the event pump, which share that runtime with the turn. ⚠ `BENCH-PROVEN` is **not** claimed: benchmarking it needs the 133 MB model and the ~30 MB runtime, which razi ruled off by default. The structural guarantee is proven (the call is on the blocking pool); the latency number is not, and is recorded as owed when embeddings are enabled)*
- [x] **P3-38** Provenance column (`source` + file/line). → `LIVE-DB-PROVEN` *(`memories.source` / `source_file` / `source_line`, backfilled for existing rows. ⚠ **Rendered, not merely stored** - the prompt says "you told me" or "I observed", because a fact and a guess are different kinds of claim and the model cannot weigh them if the prompt hides the difference)*
- [x] **P3-39** The **gateway**: agents get `search`/`get`/`list`/`propose`; **`remember_this` is removed**. → `GATE-PROVEN` *(`TG-P3-39`, falsified. All four surfaces exist - `search` / `get` / `list` / `propose` - and **`remember_this.rs` is DELETED**, not deprecated: an unreferenced second write path is a defect waiting for a caller, which is `TG3-07`'s own phrasing and was proven right once already this phase. Scope is enforced on `get_memory` and `list_memories` too, or an id from another project would be readable by guessing it)*
- [x] **P3-40** Gateway evaluation — importance, novelty, duplication, confidence — as `DET` policy (G-03). → `UNIT-PROVEN` *(The anchor's §18 evaluation as `DET` policy: substance, duplication (scoped - the same fact about two projects is two memories), derived importance that rewards what is checkable. ⚠ **A refusal is a SUCCESS with a reason, not an error** - the gateway declining is the system working, and an error would teach the model to retry a policy decision)*
- [x] **P3-41** Per-tier budgeting (simple), interface-compatible with the anchor's reranker. → `UNIT-PROVEN` *(Ranked **first**, budgeted second. The reverse takes an arbitrary N per tier and then orders them, which is a recency cut wearing a relevance label. Proven: 100 highly-relevant episodic rows cannot crowd out a single semantic one)*
- [x] **P3-42** **The anchor §10 reranker as its own work item with its own columns** — it is **not** the ported competitive model; 6 of 8 terms are disjoint (G-13). → `UNIT-PROVEN` *(Its own columns, because it is **not** the competitive model: two of the anchor's eight terms overlap and six do not. Every weighted term is asserted to move the score - a term with no column is a weight times a constant, which is a zero wearing a name. Recency is the spec's 14-day half-life, verified at 1.0 / 0.5 / 0.25 across a month and never reaching zero. Ranking is a **total** order, so `S3-04`'s "the same query twice" cannot go flaky for reasons nobody can reproduce)*
- [x] **P3-43** Fix the seven inherited defects rather than porting them (query-independent semantic recall · query-ignoring cache key · always-throwing HITL writer · unfiltered procedural recall · unscoped episodic recall · inert negative cache · uninvalidated semantic cache). → `UNIT-PROVEN` 🔴 **RE-SCOPED BEFORE BUILDING (§0.2m). The list is wrong about this codebase.** The seven defects trace to `G4c §5.3–5.4`, which cites **Python** line numbers (`procedural.py:278-290`, `yaml:38`), and spec `03 §4.3`'s claim that *"each of these is in the code being ported"* is **false for the Rust that ships**. Verified: **there is no cache anywhere in the memory module** (`grep -rni cache autodev-core/src/memory/` → zero hits across 9 files), and defects 2, 6 and 7 all presuppose one; `learn_hitl_pattern` (3) and the `agent_id` parameter (5) do not exist in Rust at all. **What IS real is worse than defect 1 as written:** `agent/context.rs` calls `build_memory_context(sid, "", repo_path)` — **the query is the empty-string literal at the only call site in the tree** — and `recall_facts(limit)` does not take a query *by signature*, while episodic is recency-only and procedural gets `""` → `LIKE '%%'` → every row. **All three tiers are query-independent; the injected block is byte-identical every turn.** This task is therefore *"fix the defects present in the shipped Rust"*, and the fix-list is: query threaded to all three tiers · scope applied · `working_dir` no longer discarded. *(`S3-04`, `S3-04b`, `S3-04c`, `S3-05b`. **The re-scoped list, not the ported one.** The query and `working_dir` are both load-bearing now, and the guard is structural: the signature demands the query, so removing it stops the tests compiling - a test that only checked output could pass while a caller still handed over `""`. An empty query takes a declared recency path rather than `LIKE '%%'` matching everything, and user text is escaped into a safe FTS5 expression because `AND`, `NEAR`, `*`, `"` and `:` are operators there and the query comes from the model)*
- [x] **P3-44** Memory is **provably read** — a recalled memory appears in the outbound prompt bytes (INV-8). → `GATE-PROVEN` ⚠ **SHARPENED BEFORE BUILDING (§0.2m) — the gate as worded would pass today.** *"A recalled memory appears in the outbound prompt bytes"* is **already true** and proves nothing, because recall is query-blind: the same bytes appear whatever was asked. The gate must assert the **invariant**, not the presence: *a memory recalled FOR THIS QUERY appears, and a different query produces a different block.* ⚠ **Two paths defeat it entirely and are now filed as `P3-45`/`P3-46`.** The chain to watch is exact: `context.rs` (memory block, volatile half) → `parts.join` → `engine.rs` `CompletionRequest.system` → `anthropic.rs` / `openai.rs` / `google.rs`. `build_memory_context` is the **only** consumer of the three stores on a prompt path, so it is the single seam. *(`TG-P3-44/45` + `S3-04`. **Asserts the sharpened invariant, not the original wording** - "a recalled memory appears in the prompt bytes" was already true and proved nothing, because the same bytes appeared whatever was asked. What is proven instead: different questions recall different things, the same question twice is stable, and another project's memory cannot appear at all)*
- [x] **P3-45** 🔴 **`system_prompt_override` silently disables ALL memory recall.** `engine.rs` — an
override returns `(override_str, String::new())`, and the **volatile** half is exactly where the memory
block lives (`P2-14`/`D-12`). So the block is never assembled. It is a **user-facing settings field**
(`settings_commands.rs`, read at `agent_commands.rs`): **any user who customises their system prompt
loses every memory, with no warning in the UI or the logs.** ⚠ This is `P3-44`'s invariant failing for a
whole class of users, and it is the mechanism that actually governs — not the missing-session-id story
told by `context.rs`'s comment. *(Found by the P3-F pre-build verification, 2026-09-02, and re-verified
in the main session.)* → `GATE-PROVEN` *(`TG-P3-44/45`, falsified. `memory_block` is extracted so the override path reaches it. An override says *"use my instructions"*; it was never a request to forget what the agent knows - and it was silently doing the second from a **user-facing settings field**, with no warning in the UI or the logs)*
- [x] **P3-46** 🔴 **Recipes, subagents and the MCP server get no memory — and the recorded CAUSE is
wrong.** `context.rs`'s comment says *"Skipped when no session_id is set (e.g. MCP server, subagent,
recipe mode)"*. **Recipes DO have a session id** — `recipes/runner.rs` builds
`format!("recipe-{}-{}", worker_slug, Uuid::new_v4())`. They lose memory because `runner.rs` passes
`Some(&recipe.instructions)` as the **override** (`P3-45`), and subagents likewise pass
`system_prompt.as_deref()`. ⚠ **I propagated that wrong cause into a user-facing tool description in
`P3-20`** — `remember_this` now tells the model that recipe runs "have none". Two sites to correct, and
the class is `L-C3`: a source's conclusion adopted without checking its load-bearing detail, where the
source was a **comment** (`L-22`). → `UNIT-PROVEN` *(Fixed at both sites, and the **cause** corrected in each. Recipes DO carry session ids; the override is the mechanism. The wrong cause had reached a user-facing tool description via `P3-20` - `L-C3`, from a **comment** (`L-22`). Recipes and subagents now receive memory, scoped)*
- [x] **P3-47** 🔴 **A fleet worker can write into the user's memory at confidence 1.0, attributed as
`user_explicit`.** Verified hop by hop: `runner.rs` runs recipes under `PAMode::FastDevelopment` →
`builtin_tools/mod.rs` registers `remember_this` in **every** mode → `remember_this.rs` writes with
`confidence = 1.0`, `source_type = "user_explicit"` → `semantic.rs::recall_facts` filters only
`is_active = 1 AND confidence >= 0.7`, orders by `confidence DESC`, and has **no scope predicate at all**
(`working_dir` is SELECTed and never used). So an autonomous worker's assertion lands at the **top** of
the interactive user's *"What I know about you and this project"* block, indistinguishable from something
the user said. ⚠ **Dormant until 2026-09-02, when `P4-31` made the fleet actually run** — a change of mine,
made after the audit that would have caught it. `P3-39` (gateway) and `P3-31` (scope) are most of the fix,
but **neither names confidence attribution**: an agent writing at the same confidence and provenance as the
user is a trust defect independent of scope. → `GATE-PROVEN` *(`TG-P3-47`, falsified. Confidence and provenance are assigned **from the runtime's own identity**, never taken from the proposer: there is deliberately no `source`, `proposer` or `importance` field in the schema, and the gate asserts their absence. The ceiling is a ceiling, not a default - a proposer may ask for less and never for more, tested both ways. ~~An autonomous agent tops out at 0.6 and is recorded as `agent_observation`, so it cannot outrank the user by construction~~ 🔴 **RETRACTED 2026-09-02, the same day, by the post-phase audit (`ADR-LOG-RESTRUCT-0032` §1).** It could outrank the user, trivially. The tool's schema declared **`user_stated`**, a model-writable boolean, and `from_agent_role(role, user_stated)` short-circuited to `Proposer::User` **before** looking at the role — ceiling 1.0, `source = "user"`, `importance +0.3`, rendered into the interactive user's next prompt as *"(you told me)"*, and at `scope: "global"` into every repository he opens. So the escalation this task exists to prevent shipped **inside** this task. ⚠ **The gate could not catch it and neither could the test:** `TG-P3-47` enumerated three forbidden schema keys and `user_stated` was not among them (`L-27`), while its assertion `contains("from_agent_role(&ctx.agent_role")` was a **prefix** that matched the two-argument call happily — inspecting the receiver and ignoring the argument that overrode it (`L-C6` → 11). A unit test asserted the escalation as **correct behaviour**. Three artifacts agreeing, all written in one sitting from one misreading (`L-31`). **Now fixed by the signature rather than by an assertion:** `from_agent_role(role: &str) -> Self` takes one argument, so no input CAN reach the decision, and the gate asserts the shape of the call and of the definition instead of a vocabulary. ⚠ `Proposer::User` consequently has **no production constructor at all** right now — no UI path builds one — which is the correct posture and is recorded rather than quietly left)*

### P3-G · Design ingestion — ~~a sub-section~~ → **PROMOTED to phases PD0–PD6**

⚠ **Superseded 2026-09-01, razi-ruled.** This was written as a ten-task sub-section of P3.
Razi's third pass expanded the scope well past what a sub-section can hold — pixel-level
fidelity detection with its own refinement tooling, PDF ingestion alongside PSD, export to
three formats including back to PSD, adaptation as a **deterministic tool**, a change
system reverse-engineered from the source pipeline's PSD construction, and brand-guide
ingestion. That is **88 tasks across seven phases**, not a lettered group.

The work moved to **`§10.5`, phases PD0–PD6**. Task IDs are **not** renumbered (`§1`):

| Old | New | State |
|---|---|---|
| `P3-45` vendor `ag-psd`, no CDN | `PD1-01` | ✅ **done** — pinned `30.1.0`, bundled by Vite |
| `P3-46` the layer IR | `PD1-02` | ✅ **done** — `src/design/ir.ts`, `assertNoPixels` |
| `P3-47` `U+0003` → line break | `PD1-03` | ✅ **done** — falsified |
| `P3-48` report unresolved fonts | `PD1-04` | ✅ **done** — `resolveFont` returns three states |
| `P3-49` conversion as an agent tool | `PD1-18` · `PD4-11` · `PD5-13` | struck here |
| `P3-50` reformat across aspect ratios | **`PD4`** (13 tasks) | struck here |
| `P3-51` rebrand + content swap | **`PD5`** + **`PD6`** (22 tasks) | struck here |
| `P3-52` brand-guide input | **`PD6-01`**…`PD6-02` | struck here |
| `P3-53` publish the capability contract | **`PD0`** (6 tasks) | struck here |
| `P3-54` conversion under fault isolation | `PD1` exit condition; `P3-A` is now an **ordering** dependency | struck here |

The four completed tasks stay ticked — the work exists, is tested and is pushed. They are
carried into `PD1-A` with their evidence rather than re-proven.

**Smokes `S3-06`…`S3-10` likewise move** to `SD1`…`SD6` beside the phases that own them.

### Smoke — S3
- [x] **S3-01** A 30-turn session's input tokens do **not** grow linearly with turn count. *(`tests/s3_smokes.rs`. Asserts the in-process half of the bound and says so: growth is limited by two mechanisms, and the clear layer's effect is only visible on the wire - measured separately by `p3_25_bench_…` at 11,310 -> 2,750 tokens. What this proves is the ORDERING a 30-turn session experiences: the clear trigger fires within the window, compaction does NOT fire at or below that trigger (so the cheap layer always gets its chance), a history at the full window does trigger compaction, and the estimator responds to size - without which the other three assertions would be measured against a constant)*
- [x] **S3-02** A deliberately panicking tool fails its turn; the app and other sessions survive. *(`tests/s3_smokes.rs`. Two concurrent sessions, one panicking mid-turn: the panic is caught at the turn boundary (`P3-02`/`P3-03`), the containing task returns cleanly rather than aborting, and the neighbouring session completes **all 50 units of its work** - counted, not assumed, because "the app survived" is not observable from the app failing to crash)*
- [x] **S3-03** A subagent spawns, streams, and returns with the sidecar **not running**. *(`tests/s3_smokes.rs`. The sidecar was deleted in P0 and subagents became in-process tokio tasks (`ADR-LOG-RESTRUCT-0008`), so the smoke states the consequence rather than the absence: the pool grants a slot, enforces the depth boundary and restores capacity on release, all under a hard timeout - a path that reached for an external process would hang there instead of answering)*
- [x] **S3-04** Two different questions in one session produce **different** recall sets. *(`tests/s3_memory.rs`. **This failed by construction before this phase** - every tier ignored the query. Now: two different questions return different sets, the same question twice returns the same set, another project's memory cannot appear, and a global memory is shared while a project one is not)*
- [x] **S3-05** With the embedding model deleted, recall still returns results and reports itself degraded. *(`tests/s3_memory.rs`. With no model, recall still returns results and **reports itself degraded** with a reason - never empty, never an error. An empty result is indistinguishable from "I do not know that", which is why the requirement is worded that way)*

*(`S3-06`…`S3-10` moved to `SD1`…`SD6` with the design phases — see `§10.5`.)*

**Exit.** S3 → `TG-P3` → matrix → progress → no-stale → commit.

**Narrative success criterion.** *A newcomer can hold a long conversation and watch the context stay bounded and cached; kill a tool mid-run and see only that turn die; and ask the same question twice in different words and get the same memory back — proof the memory system is being read, not merely written.*

---

# §10 · Phase P4 — The Fleet

**Goal.** A task runs end to end on three OSes and produces a **build-verified, reversible** artifact in the user's workspace that they can run.
**Depends on.** P3.
**Spec anchors.** `04` (all); ADR-02-01, ADR-02-02, ADR-02-06, ADR-02-07, G-05, G-08.

### P4-A · Dispatch
- [ ] **P4-01** Worker state as a Rust **enum**, matched exhaustively. → `UNIT-PROVEN`
- [ ] **P4-02** `FromStr` treats an unknown status as a **loud error**, never a silent default. *(The enum alone only relocates the dead-latch — `workers.status` is unconstrained `TEXT` and `src-shared` wrote arbitrary strings into it.)* → `UNIT-PROVEN`
- [ ] **P4-03** `CHECK` constraint on the status column. → `LIVE-DB-PROVEN`
- [ ] **P4-04** Event-driven dispatch; cron demoted to a low-frequency reconciler **with a stated interval** (GAP-13). → `SMOKE-PROVEN`
- [ ] **P4-05** A transient DB error must not terminate a schedule. → `UNIT-PROVEN`
- [ ] **P4-06** `kick_inflight` released by RAII; the kicked run has a timeout. → `UNIT-PROVEN`
- [x] **P4-31** 🔴 **Eight of the nine shipped recipes cannot run at all.** `fallback_policy: Mode2Only` on `supervisor`, `worker-backend`, `worker-frontend`, `worker-platform`, `worker-mcp`, `qc-agent`, `qc-dispatcher` and `fleet-smoke` — only `worker-cleanup` is `Mode1Capable`. **Mode 2 has been inert since P0** (the in-process MCP server it reached tools through was deleted as an unauthenticated local RCE, `ADR-LOG-RESTRUCT-0008`), so `run_recipe` reaches the fallback-policy check, logs `fallback_skipped_policy`, and returns `Ok(())` **on every tick, forever**. The fleet reports itself idle rather than broken. ⚠ **Same symptom as the P0 audit's `F-10` (the fleet-ready latch), by a completely different mechanism** — which is why fixing that one did not surface this one. ⚠ It also makes `P3-65`'s permission-mode wiring unreachable for every shipped recipe: the only `Supervised` one is `fleet-smoke`, and `Mode2Only` returns before the engine is built. *(Found by the P3 post-phase gap hunt, `G-17`, 2026-09-02. Filed here rather than fixed in P3: which recipes should be `Mode1Capable` — or whether `Mode2Only` should mean anything at all now that Mode 2 is gone — is a dispatch decision, not a runtime one.)* → `SMOKE-PROVEN` *(**razi-ruled 2026-09-02: flipped now, before P3-F.** All eight recipes now declare `Mode1Capable`, which is what `runner.rs` already said was happening — *"every worker therefore runs on the in-process Rust engine (Mode 1), which needs no sidecar and no Python."* The declarations had simply never been updated when Mode 2 went inert, and nothing connected the two. `TG-P4-31` is that connection: it asserts zero `Mode2Only` declarations **while `orch_healthy` is pinned to false**, and deliberately panics with a re-point instruction if Mode 2 is ever revived rather than passing by default. ⚠ **Falsified twice.** The first version failed for the WRONG reason — its sweep floor counted only `Mode1Capable`, so stranding one recipe tripped "the sweep is not reaching the declarations" instead of the real assertion. Decoupled by counting both policies (`L-26`: read the failure reason, do not infer it from the exit code). ⚠ **Side effect, and a welcome one:** `fleet-smoke` was the recipe blocking `P3-65`'s reachability retraction — with it Mode1Capable and `schedule: None`, it now reaches the engine and **genuinely runs under `SmartApprove`**, with a human present because it is manually triggered.)*

### P4-B · Tasks
- [ ] **P4-07** Atomic claim including `assigned_worker` in the predicate — **task theft is possible today**. → `GATE-PROVEN`
- [ ] **P4-08** The one-task-per-worker guard moves **into** that statement (it fails open today). → `UNIT-PROVEN`
- [ ] **P4-09** An epic can never be assigned or claimed as work. → `GATE-PROVEN`
- [ ] **P4-10** Transactional multi-write reconciliation. → `UNIT-PROVEN`

### P4-C · Build verification (M-13)
- [ ] **P4-11** `autodev-core/verify/` — ecosystem detection + Rust-owned argv via P0-23. → `PLATFORM-TRIPLE`
- [ ] **P4-12** `build_evidence` becomes a **table**, not a 15-minute in-memory map. → `LIVE-DB-PROVEN`
- [ ] **P4-13** Evidence records the real exit code — **delete `classify()`'s `str::contains` heuristic**, under which `git commit -m "cargo build fixes"` satisfies the gate. → `GATE-PROVEN`
- [ ] **P4-14** The `qc_review` guard reads the persisted table. → `GATE-PROVEN`
- [ ] **P4-15** **G-05:** after three consecutive failures the model **diagnoses**; Rust still owns the command and reads the exit code. Bound the loop. → `UNIT-PROVEN`
- [ ] **P4-16** Purge POSIX-only shell from every recipe and skill. → `GATE-PROVEN`

### P4-D · Delivery and reversibility (M-43)
- [ ] **P4-17** One workspace resolver that **fails loudly**; delete all three divergent fallbacks. → `GATE-PROVEN`
- [ ] **P4-18** Every agent write resolves against it (INV-5); no CWD-relative writes remain. → `GATE-PROVEN`
- [ ] **P4-19** A **failed checkpoint blocks the write** (it proceeds silently today). → `UNIT-PROVEN`
- [ ] **P4-20** A worker branch per task for coarse rollback; reconcile with checkpoints so the two never disagree. → `SMOKE-PROVEN`
- [ ] **P4-21** The handoff: reveal · open in IDE · **run**. Fix `ide:navigate` having zero listeners. → `SMOKE-PROVEN`

### P4-E · Coordination capabilities
- [ ] **P4-22** **ACP identity from the in-house `ToolContext`** — set only by the MCP adapter today, which P0 deleted. **Without this, worker messaging is silently dead.** → `GATE-PROVEN`
- [ ] **P4-23** Once-and-only-once ACP delivery; wire the stale-message reader. → `UNIT-PROVEN`
- [ ] **P4-24** Escalation cannot starve dispatch indefinitely. → `UNIT-PROVEN`
> **⚠ ORDERING CONSTRAINT — R-8, enforced by task dependency rather than by a gate.**
> **`P4-07` (atomic claim) must be checked before `P4-25` (outcome recording) is started.** Task theft is latent only because no outcomes are recorded at all; switching recording on before the claim is fixed means affinity **trains on wrong attributions** — the claim overwrites `assigned_worker` while `routing_history` keeps the old name. A sequencing property cannot be asserted at runtime, so it lives here, as a dependency, not in `TESTING_GATES.md`.

- [ ] **P4-24a** **Decide the fate of existing `routing_history`.** Prior rows may carry corrupted attributions from the unfixed claim. **⚠ Destructive — §0.8 applies: back up `~/.autodev/tasks.db` first.** Inspect before acting: if `outcome` is NULL throughout (expected, since `record_outcome_auto` has never been called), there is nothing to lose and the table is simply empty of signal. **Discard only corrupted rows, and only after the backup is verified.** Log the call in `ADR_LOG.md`. → `LIVE-DB-PROVEN`
- [ ] **P4-25** Record routing outcomes from the **tool**, not only the Tauri command — the learning gate never opens today. **Blocked on `P4-07`.** → `GATE-PROVEN`
- [ ] **P4-26** `suggest_worker` never returns a system-defined agent for implementation work. → `UNIT-PROVEN`
- [ ] **P4-27** Make the Supervisor able to create an epic — the tool is PA-only and it reaches tools only via MCP. → `GATE-PROVEN`
- [ ] **P4-28** `unblock_ready_tasks` fires in autonomous operation. → `UNIT-PROVEN`
- [ ] **P4-29** Per-task budget actually fires (`with_task_budget` has zero callers), under P2's ceiling. → `GATE-PROVEN`
- [ ] **P4-30** Worker persona: DB owns identity/state, recipe owns the prompt; **stop `ensure_default_recipes` overwriting user edits**. → `LIVE-DB-PROVEN`

### Smoke — S4
- [ ] **S4-01** One task runs end to end on **Windows, Linux and macOS** and produces a build-verified artifact in the chosen workspace.
- [ ] **S4-02** The artifact **runs** from the IDE tab.
- [ ] **S4-03** Kill a worker mid-task → the task is released and re-run with no human intervention.
- [ ] **S4-04** Worker A messages worker B **with the sidecar not running**.
- [ ] **S4-05** A worker cannot claim a task assigned to another.

**Exit.** S4 → `TG-P4` → matrix → progress → no-stale → commit.

**Narrative success criterion.** *A newcomer types one sentence, watches the fleet pick it up, and finds a working, build-verified change in their own repository that they can run — on any of the three operating systems. This is the first phase at which AutoDev does what it claims to do.*

---

# §10.5 · Rollout 1 · The design phases (PD0–PD6)

**Added 2026-09-01** on Razi's third-pass direction. These phases are what makes AutoDev
the executor behind AutoDesign. They are grounded in `C:\AIRS Foundry\apps\psdconversions`
— its `index.html` (3,202 lines, v4.7/v5.0) and its four architecture documents — and in
the P1 spike, which converted two real ad PSDs end to end and verified them visually.

**The one architectural commitment, stated once and inherited by every phase below.**
The converter's HTML output measured **17.41 MB** (1080×1080) and **25.27 MB** (1080×1920),
because every layer is an embedded base64 PNG. That cannot enter a model's context. The
source's own architecture document already ruled on it:

> *"the in-memory layer array is the universal intermediate representation … Everything
> downstream — PSD, HTML, SVG, React, Flutter — is just a different serialization of that
> same structured data."*

So: **the IR is the working format; every output is a serializer; every tool is an IR
mutation.** Ingestion produces it, fidelity grades it, export serializes it, adaptation and
changes mutate it.

---

## §10.5.0 · PD0 · The capability contract

**Goal.** The AutoDesign team can build against a real, versioned, exercised surface.
**Depends on.** P2. **Runs early and in parallel** — its schema half depends only on the
operation set, which is decided.

- [ ] **PD0-01** The operation set is **named and frozen for v0**: `ingest` · `export` · `adapt` · `change`. Four verbs, because those are the four things Razi specified and a fifth invented now is a fifth to version forever. → `DOC-GATE`
- [ ] **PD0-02** The **IR schema is the wire payload**, published as a versioned type. Not the HTML, not the PSD bytes — see the commitment above. → `UNIT-PROVEN`
- [ ] **PD0-03** Job semantics mirror AutoDesign's existing `CapabilityJobService`: async, terminal statuses, operation events. Conversion runs 25–35 s on a 30–45 MB input, so a synchronous contract would be wrong on the first real file. → `UNIT-PROVEN`
- [ ] **PD0-04** An **error taxonomy** distinguishing *refused* (bad input), *failed* (we broke), and *degraded* (succeeded with loss — an unresolvable font, a rasterized blend mode). Degraded is the one that matters: it is the case the old converter reported as success. → `UNIT-PROVEN`
- [ ] **PD0-05** **Asset transfer**: how bitmaps move between the two processes, and who owns their lifetime. → `UNIT-PROVEN`
- [ ] **PD0-06** A **live stub** answers every operation with a valid, correctly-shaped response before any real work exists. A contract nobody has exercised is a document; a stub makes it an integration. → `SMOKE-PROVEN`
- [ ] **PD0-07** 🔴 **The contract states who executes.** *(razi-ruled 2026-09-03, `D3`.)* Across the whole of `PD1`–`PD6` the words *personal assistant*, *PA*, *fleet*, *worker* and *supervisor* appear **zero times** — the phases define capabilities and never say who drives them, which left AutoDesign to infer it. **The ruling: the PA executes design work in Rollout 1**, refined until it is ready, and **the fleet becomes an additional consumer in Rollout 2** — running it through the fleet now would be, in Razi's words, *"pretty confusing and hard to debug"*. So: `adapt` stays deterministic (`PD4-02`), the **PA** chooses parameters and adjudicates a `degraded` result, and nothing in Rollout 1 depends on a working fleet. Write it into the contract so the consumer does not have to guess. → `DOC-GATE`

**Smoke — SD0**
- [ ] **SD0-01** A caller invokes all four operations against the stub and receives well-formed, schema-valid responses.

---

## §10.5.1 · PD1 · Ingestion — PSD **and** PDF

**Goal.** Either source becomes one IR that describes the design exactly.
**Depends on.** P8 (handover-clean tree). **Source.** `psdconversions` v4.7 pipeline.

### PD1-A · The PSD path *(four tasks already complete, carried from P3-G)*
- [x] **PD1-01** `ag-psd` pinned at exactly `30.1.0` and bundled locally; **no CDN script survives** the renderer CSP (`script-src 'self' blob:`). *(was `P3-45`)* → `GATE-PROVEN`
- [x] **PD1-02** The layer IR: bounds, fonts, colours, tracking, `styleRuns`, blend modes, opacity, group nesting — with **no pixels**, enforced by `assertNoPixels`. *(was `P3-46`)* → `UNIT-PROVEN`
- [x] **PD1-03** PSD's `U+0003` point-text separator becomes a line break. *(was `P3-47`)* → `UNIT-PROVEN`
- [x] **PD1-04** An unresolvable font is **reported**, never silently substituted. *(was `P3-48`)* → `UNIT-PROVEN`
- [ ] **PD1-05** **Shape layers keep their geometry.** `vectorMask` Bezier knots enter the IR as path data, not as a rasterized preview — rasterizing here forfeits SVG export (`PD3-09`) and shape edits (`PD5-06`) permanently. → `UNIT-PROVEN`
- [ ] **PD1-06** **Layer effects are recorded even when CSS cannot express them** — drop shadow, inner shadow, stroke, outer glow, gradient overlay, bevel. The IR records what the design *has*; the serializer decides what it can *render*. Collapsing that distinction at ingestion is unrecoverable. → `UNIT-PROVEN`
- [ ] **PD1-07** Clipping masks and clip paths enter the IR, including circular image clips. → `UNIT-PROVEN`
- [ ] **PD1-08** Smart objects and adjustment layers are recorded with a **`rasterize-required`** marker rather than dropped. → `UNIT-PROVEN`

### PD1-B · The PDF path *(ported from the v4.7 four-phase pipeline)*
- [ ] **PD1-09** **Phase 1** — operator-list segmentation (`segmentOperatorList`, `flushSubGroup`): walk the raw drawing commands and emit sub-groups. The pipeline is built on the operator list rather than text/image extraction, and that choice is what makes native shape layers possible at all. → `UNIT-PROVEN`
- [ ] **PD1-10** **Per-draw-op CTM tracking.** `pixelCoord = viewportTransform × pageTransform × elementTransform × localCoord`. Every draw op carries its own `ctmAtDraw`. → `UNIT-PROVEN`
- [ ] **PD1-11** **Phase 2** — group classification and bounds (`classifyAllGroups`, `computeGroupBounds`), with clip-companion and spatial-overlap merging. → `UNIT-PROVEN`
- [ ] **PD1-12** **Phase 3A** — shape-layer creation (`shouldUseShapeLayer`, `pdfPathToPsdVectorMask`, `createShapeLayer`). Knots in `[x,y]` order per the v4.5 correction; rectangular clips are **never** used as a `vectorMask` (they are content-area bounds, not shapes). → `UNIT-PROVEN`
- [ ] **PD1-13** **Phase 3B** — rasterization fallback (`rasterizeForPreview`, `rasterizeVectorGroup`) for mixed content and non-solid fills. → `UNIT-PROVEN`
- [ ] **PD1-14** **Phase 4** — text assembly with the **ten merge guards** (`mergeBlocks`): word coalescing and paragraph merge without crossing columns. → `UNIT-PROVEN`
- [ ] **PD1-15** Image extraction via the CTM-tracked operator walk (`extractPageImages`). → `UNIT-PROVEN`
- [ ] **PD1-16** **Position-correlated text colour** — colours matched to text by spatial proximity in the operator stream, not by index. The v4.5 change exists because index-based matching silently mis-coloured text. → `UNIT-PROVEN`

### PD1-C · Shared
- [ ] **PD1-17** **The seven guards are ported with their reasons**: canvas cap at 15,000 px, PSD limit at 30,000 px, CMYK detection (output is RGB — say so), image dedup under 5 px, full-page image skip above 90%, tiny image skip under 30 px, shape-layer fallback on mixed content. → `GATE-PROVEN`
- [ ] **PD1-18** **One schema from either source.** A PSD-derived IR and a PDF-derived IR are the same type, and every downstream phase is written once. → `GATE-PROVEN`

**Smoke — SD1**
- [ ] **SD1-01** A real PSD converts inside AutoDev with **no network fetch** and the render matches the source.
- [ ] **SD1-02** A real PDF converts to the same IR type, and to a PSD Photoshop opens without repair.
- [ ] **SD1-03** A design with Bezier shapes keeps them as paths — the IR contains knots, not a bitmap.
- [ ] **SD1-04** A CMYK PDF is detected and the RGB conversion is **reported**, not silent.

---


> ### 📐 Grounded in the AAE post-mortem
>
> The organisation already built a design **Adaptive Adjustment Engine** —
> `AIRS Microservices/mcp-server-adobe/src/aae` — and it reached production
> against a real 5-PSD × 3-size matrix. Its full reading is in
> `_verify/A12-aae-adaptation-reference.md`; the tasks below are traced to it
> line by line, and razi-ruled into this plan on 2026-09-03.
>
> **The single most important thing it teaches:** the engine has **no measured
> fidelity anywhere**. There is no rasterisation and no comparison — every
> "score" is a penalty tally over the same boxes the engine just produced. Its
> correctness checker is complete, correct, was marked RESOLVED in its own
> audit, and **is imported by nothing**. Both are shapes this repo knows.
>
> ⚠ Its three root-level audit reports describe an engine that no longer
> exists. The real record is `docs/AAEAuditbyRazi/` — the owner's own June run.

### PD1-D · IR vocabulary the AAE post-mortem paid for

- [ ] **PD1-19** `Frame` carries an **affine transform, not just an AABB**. AAE's `Box{left,top,width,height}` is right for layout maths and silently drops `rotate` — carried in the type and read by no transform. → `UNIT-PROVEN`
- [ ] **PD1-20** A **closed `Role` enum with no aliases**, plus a versioned translation at ingest. Seven aliases caused a live bug where code branching on `'decor'` never matched `'decoration'`. → `UNIT-PROVEN`
- [ ] **PD1-21** Role assignment is `{value, confidence, method, reasons}` — AAE already had this shape and only ever read `value`, so provenance was designed and thrown away. → `UNIT-PROVEN`
- [ ] **PD1-22** 🔴 **Hierarchy is real: parent transforms compose onto children at parse time.** AAE's flat `[groupNode, child, child]` model is the direct cause of its **#1 production defect** — group/folder desync. → `UNIT-PROVEN`
- [ ] **PD1-23** Text is **runs with measurable metrics** (family, size, leading, tracking, per run). AAE had a single style over a single string, which made its `TextFitResult{overflow,truncated,needsBreak}` a dead type because nothing ever measured text. → `UNIT-PROVEN`
- [ ] **PD1-24** Asset **kind** (vector / raster / text / shape) is carried explicitly from ingest. AAE guessed "vector" as *"≤1 smartObject"* — a Photoshop proxy that does not exist here. → `UNIT-PROVEN`
- [ ] **PD1-25** Port `UnitValue` / `SpacingSpec` / `resolveUnit` / `resolveAsFraction` **essentially verbatim**, resolving only at the policy boundary. The cleanest module in that engine, and its px-only-inside rule held up. → `UNIT-PROVEN`


## §10.5.2 · PD2 · Fidelity — measured, not asserted

**Goal.** *"Visually it looks okay but in reality pixel by pixel wise is not, so we need a
tool here that can detect that and do refinements."* **Depends on.** PD1.

**Why this is its own phase.** Ingestion is not finished when it looks right; it is finished
when something can **prove** it is right and **say where** it is not. The P1 spike is the
cautionary case: the render looked correct, and only a per-value trace revealed a control
character rendering as a glyph box and two fonts silently replaced.

- [ ] **PD2-01** **Composite renderer** — the IR rasterized at native resolution, deterministically. → `UNIT-PROVEN`
- [ ] **PD2-02** **Reference raster** — the source's own composite (PSD flattened image, or PDF page render at matched DPI). → `UNIT-PROVEN`
- [ ] **PD2-03** **Per-pixel difference**, reported as a percentage of pixels outside tolerance and as a maximum channel deviation. → `UNIT-PROVEN`
- [ ] **PD2-04** **SSIM** over the whole canvas, because a small number of badly wrong pixels and a large number of slightly wrong ones are different defects and per-pixel alone cannot tell them apart. → `UNIT-PROVEN`
- [ ] **PD2-05** **Per-layer maximum deviation** — the metric that localises. A whole-canvas score says the conversion is 97% right; this says *which layer* owns the other 3%. → `UNIT-PROVEN`
- [ ] **PD2-06** **Thresholds are declared numbers**, per metric and per layer kind, committed to the repository. A threshold argued at review time is not a gate. → `DOC-GATE`
- [ ] **PD2-07** **Text carries its own tolerance.** Browser and Photoshop rasterisers differ on anti-aliasing and sub-pixel kerning by design; holding text to the shape threshold would fail every correct conversion, and a gate that fires on correct output gets switched off. → `UNIT-PROVEN`
- [ ] **PD2-08** The report **names the worst layers in order**, with their metric and their kind. → `UNIT-PROVEN`
- [ ] **PD2-09** **Cause localisation** — a failing layer is attributed to a candidate cause: font substitution, blend-mode fallback, effect rasterisation, transform error, colour-space shift. → `UNIT-PROVEN`
- [ ] **PD2-10** **Refinement proposes and applies a correction**, then re-measures. → `SMOKE-PROVEN`
- [ ] **PD2-11** **The refinement loop terminates** — bounded iterations, and it stops when a round fails to improve. An unbounded "keep fixing" loop on a 25 s conversion is an unbounded cost. → `GATE-PROVEN`
- [ ] **PD2-12** A **regression corpus** of real designs — the `gamead` family (1080×1080, 1080×1920, 1920×1080), plus a text-heavy, an effect-heavy and a vector-heavy case. → `DOC-GATE`
- [ ] **PD2-13** **The gate fails on a planted regression.** Falsification is the acceptance criterion for this phase, not an extra. → `GATE-PROVEN`
- [ ] **PD2-14** **Fidelity travels with the result.** Every ingest and every export returns its score; a caller can refuse a degraded conversion. Measuring fidelity and then not returning it repeats `L-C12` — built, proven, wired to nothing. → `GATE-PROVEN`

**Smoke — SD2**
- [ ] **SD2-01** Every design in the corpus converts and scores above its declared thresholds.
- [ ] **SD2-02** A deliberately broken conversion (font forced to fallback) is **caught**, and the report names the affected layers.
- [ ] **SD2-03** The refinement loop improves a failing case and terminates.

---


### PD2-D · What "measured, not asserted" has to mean *(AAE post-mortem)*

> 🔴 **AAE never measured fidelity and did not notice.** This group exists so
> this phase's title stays true.

- [ ] **PD2-15** 🔴 **Build a renderer; fidelity is a measured comparison of rendered source against rendered output.** There is no rasterisation anywhere in AAE — its pipeline audit lists PNG generation as missing — so every score it reports is a penalty tally over its own output. → `BENCH-PROVEN`
- [ ] **PD2-16** **Normalise every before/after metric against the intended resize.** Raw-pixel comparison made *everything* read as moved and scaled; comparing centre fractions with a relative epsilon is what gave the metric signal. → `UNIT-PROVEN`
- [ ] **PD2-17** Every invariant has a **typed identity** shared by emitter and analyser — no string matching. Three of six AAE refinement branches key on violation strings its compiler never emits, so its gap multiplier is permanently 1.0. → `UNIT-PROVEN`
- [ ] **PD2-18** **Overlap is a real, role-aware invariant** with an IoU or occlusion measure. AAE declares `maxPrimaryOverlap` and checks it nowhere; the role-aware version lives only in the file nothing imports. → `UNIT-PROVEN`
- [ ] **PD2-19** **Re-verify the geometry that actually ships**, after every post-hoc adjustment. AAE's safe-zone clamping runs *after* selection and *after* verification, and its residuals never enter the reported violations. → `GATE-PROVEN`
- [ ] **PD2-20** 🔴 **The refinement loop may not mutate its own thresholds.** AAE's `adjust_constraints` lowers the minimum font and CTA sizes and raises the maximum logo crop until the same layout scores clean. Relaxation must be a caller-authorised, recorded input. → `GATE-PROVEN`
- [ ] **PD2-21** **Select by (hard-constraint satisfaction, then a real-valued quality score)** — not by counting violations. AAE sorts on `(violationCount, errorCount, seedIndex)`, so *"logo 3% small"* and *"hero fully off-canvas"* weigh exactly the same. → `UNIT-PROVEN`
- [ ] **PD2-22** **Keep best-so-far across refinement and never return worse than an earlier iteration.** AAE added this late, precisely because refinement returned the *last* result rather than the best. → `UNIT-PROVEN`
- [ ] **PD2-23** **Protected sub-regions** (faces, product bounds, focal points) are modelled and crop is checked against them. Still open in AAE: *"'a character is cut' passes as long as the overall area loss is under the role threshold."* → `UNIT-PROVEN`
- [ ] **PD2-24** **Real text shaping** — measure, line-break, detect overflow, and make `wrap` mean something. In AAE `wrap` and `preserve` are identical no-ops, so its whole text-wrap vocabulary is a lie. → `UNIT-PROVEN`
- [ ] **PD2-25** A **legibility invariant beyond a pixel floor** — contrast against the composited backdrop, minimum rendered x-height. AAE's only legibility check is `fontSize >= 18`, applied to a size the engine itself just chose. → `UNIT-PROVEN`
- [ ] **PD2-26** **The offline round-trip harness, built first.** Re-execute the emitted artefact under the consumer's semantics and assert it reproduces the intended layout. ⚠ AAE's equivalent is *the only check in that repo that ever caught anything*, and it needed no vendor API. → `GATE-PROVEN`
- [ ] **PD2-27** **An end-to-end run on real assets across the full target matrix before this phase closes.** AAE's 5-PSD × 3-size run **reordered its entire defect priority list** and surfaced five findings that three prior desk audits had all missed. → `SMOKE-PROVEN`


## §10.5.3 · PD3 · Export — webpage, PDF, PSD

**Goal.** The IR becomes any of the three outputs. **Depends on.** PD2.

- [ ] **PD3-01** **IR → standalone HTML**, at parity with the source converter. → `SMOKE-PROVEN`
- [ ] **PD3-02** Assets resolve from the store at serialization time; the IR still carries none. → `GATE-PROVEN`
- [ ] **PD3-03** Google Fonts are linked **only** for families that resolve; nothing else is requested. → `UNIT-PROVEN`
- [ ] **PD3-04** Unresolved fonts appear in the **export result**, not only in a log. → `GATE-PROVEN`
- [ ] **PD3-05** **IR → TypeScript/React components.** → `SMOKE-PROVEN`
- [ ] **PD3-06** **The semantic escape hatch is bounded by evidence, not taste.** Razi: *"semantic components that fills in where HTML and CSS cannot handle it with the code limitations."* The eligible set is exactly the source's own **⚠ Approximate** and **❌ Not Supported** rows — Dissolve, Subtract, Divide, Hard Mix, bevel/emboss/satin, pattern overlays, adjustment layers. A component is written **only** for a row on that list. → `DOC-GATE` · `GATE-PROVEN`
- [ ] **PD3-07** The **16 CSS-native blend modes** map 1:1; the four with no equivalent rasterize, and say so. → `UNIT-PROVEN`
- [ ] **PD3-08** Layer effects → CSS where expressible (`filter: drop-shadow`, `box-shadow`, `box-shadow: inset`, gradients); rasterize otherwise, and record which. → `UNIT-PROVEN`
- [ ] **PD3-09** Bezier shapes export as SVG `<path>`, not as bitmaps. → `UNIT-PROVEN`
- [ ] **PD3-10** Clip paths export as SVG `<clipPath>`. → `UNIT-PROVEN`
- [ ] **PD3-11** **IR → PDF.** → `SMOKE-PROVEN`
- [ ] **PD3-12** **IR → PSD** via `writePsd` — shape layers with `vectorMask`/`vectorFill`, editable type layers, pixel layers. This is the construction path the source pipeline already performs; `PD5` reverses it. → `SMOKE-PROVEN`
- [ ] **PD3-13** **Round-trip.** PSD → IR → PSD preserves layer count, bounds, text content and style runs. The strongest single check that the IR lost nothing. → `GATE-PROVEN`
- [ ] **PD3-14** Every export is **graded by PD2** before it is returned. → `GATE-PROVEN`
- [ ] **PD3-15** Export size is bounded and reported; a 25 MB result is a fact the caller learns before receiving it. → `UNIT-PROVEN`

**Smoke — SD3**
- [ ] **SD3-01** One IR exports to all three formats and each scores above threshold.
- [ ] **SD3-02** A PSD survives the round trip with its text still editable in Photoshop.
- [ ] **SD3-03** A design using an unsupported blend mode exports correctly **and** reports the rasterisation.

---

## §10.5.4 · PD4 · Adaptation — a deterministic tool

**Goal.** Reformat a design to a target size and ratio. **Depends on.** PD3.

**Doctrine ruling (Razi).** *"The entire tool does the calculation and stuff and the
LLM/AutoDev fleet here is only the executor of that tool."* Adaptation geometry is
**code**. The fleet chooses *when* to adapt and to *what*; it never computes the layout.
This sits on the deterministic side of the master plan's PART 2 split.

- [ ] **PD4-01** Tool signature: target width, target height, and a ratio policy (`contain` · `cover` · `stretch-safe`). → `UNIT-PROVEN`
- [ ] **PD4-02** **No model call inside the tool.** Enforced as a gate, not a convention. → `GATE-PROVEN`
- [ ] **PD4-03** **Per-layer anchor model** — each layer resolves to an anchor (corner, edge, centre) derived from its position and its group. → `UNIT-PROVEN`
- [ ] **PD4-04** **Safe areas** — margins that content must not cross in the target format. → `UNIT-PROVEN`
- [ ] **PD4-05** Scale computation per layer kind: images cover, shapes scale, text rescales by its own rule. → `UNIT-PROVEN`
- [ ] **PD4-06** Reposition computation honouring anchors and safe areas. → `UNIT-PROVEN`
- [ ] **PD4-07** **Text reflow.** A headline that fits 1920 wide does not fit 1080 wide; the tool adjusts size or wraps, and reports which. → `UNIT-PROVEN`
- [ ] **PD4-08** Hero/background fill strategy when the aspect ratio changes — extend, re-crop by focal point, or letterbox, stated per layer. → `UNIT-PROVEN`
- [ ] **PD4-09** **No layer silently leaves the canvas.** Anything that cannot be placed is reported. Silent loss is the failure mode this phase is most exposed to: it looks like a clean adaptation. → `GATE-PROVEN`
- [ ] **PD4-10** Group-relative transforms survive — children move with their parent. → `UNIT-PROVEN`
- [ ] **PD4-11** The fleet invokes it as a **tool** and cannot reimplement its geometry. → `GATE-PROVEN`
- [ ] **PD4-12** Every adaptation is graded by PD2 against the source. → `GATE-PROVEN`
- [ ] **PD4-13** Presets for the formats actually in use: 1080×1080, 1080×1920, 1920×1080. → `UNIT-PROVEN`

**Smoke — SD4**
- [ ] **SD4-01** 1080×1080 → 1080×1920: every visible layer lands inside the canvas.
- [ ] **SD4-02** 1920×1080 → 1080×1080: text reflows and the reflow is reported.
- [ ] **SD4-03** The same input and target produce byte-identical output twice (determinism).

---


### PD4-B · Layout intent *(AAE post-mortem)*

- [ ] **PD4-14** **Centre-fraction preservation with uniform `min(sx,sy)` scale** is the baseline transform. Every other rule in AAE is a correction on top of this one. → `UNIT-PROVEN`
- [ ] **PD4-15** `cover` with **centre-offset correction** for full-bleed elements — naive cover drifts the focal point. → `UNIT-PROVEN`
- [ ] **PD4-16** **Nearest-source-corner detection and corner-anchored placement.** The highest-value reconstructed-intent heuristic in that engine, and the only thing keeping corner elements in their corners. → `UNIT-PROVEN`
- [ ] **PD4-17** 🔴 **Coupling is a first-class IR relation** — *"this element's frame derives from that one's"* — used for background / overlay / pattern. AAE documented the overlay as "coupled with bg" and **never implemented the coupling**, producing a visible dark seam across all five test assets. → `UNIT-PROVEN`
- [ ] **PD4-18** **Named groups are rigid units**: transform the unit, then expand to members. AAE's rank-1 fix; unit-first ordering took one test from 210 px of error to 0. → `UNIT-PROVEN`
- [ ] **PD4-19** **Exclude full-bleed roles from proximity clustering** before it runs. A canvas-spanning background absorbed logos and overlays into one cluster and erased their transforms. → `UNIT-PROVEN`
- [ ] **PD4-20** **Every layout threshold is a canvas fraction or a physical unit — bare-pixel constants are banned.** AAE's `PROXIMITY_THRESHOLD=50`, `MAX_CLUSTER_DIAMETER=500`, nudge cap `50` and `minCtaSize=120` mean entirely different things at 640×480 and at 4096×4096. → `GATE-PROVEN`
- [ ] **PD4-21** Adopt the **canvas-relative gap scale** (tight 2% / normal 4% / loose 8%, axis-bound) — the one spacing decision in AAE that scaled correctly. → `UNIT-PROVEN`
- [ ] **PD4-22** 🔴 **Per-ratio composition templates**: a target size resolves to named regions and roles bind to regions. Still open in AAE and the reason for its most visible failure — *"the recompose path is a handful of anchor-strategy seeds, not true per-ratio layout templates … a landscape target ends up as a rescaled square."* → `UNIT-PROVEN`
- [ ] **PD4-23** The aspect-ratio mode selector is **direction-symmetric**. Verified in AAE: `0.5 → 1.0` selects `recompose` while `1.0 → 0.5` selects `reflow`, because the selector divides by the source ratio. → `UNIT-PROVEN`
- [ ] **PD4-24** **Background ignores the safe zone; only foreground respects it** — stated as an explicit rule. Still open in AAE: a safe-zone guide was read as *"shrink everything"*, producing white margins instead of a full-bleed background with inset foreground. → `UNIT-PROVEN`

### PD4-C · One compiler, one truth *(AAE post-mortem)*

- [ ] **PD4-25** 🔴 **Exactly one `compile(plan, scene) -> Layout`**, called by the planner, the checker and the emitter alike. AAE's overlap resolver is a **second, partial geometry evaluator** that returns *source-space* boxes for six of nine transform kinds. → `GATE-PROVEN`
- [ ] **PD4-26** **Every transform kind is total — no silent fallback to `preserve`.** Two kinds fell back for months while emitting a warning nobody actioned; an unimplemented arm should fail to compile. → `UNIT-PROVEN`
- [ ] **PD4-27** **Uniform scale by default** when mapping a group frame onto members; anisotropic scaling is opt-in and checked. AAE applies independent `scaleX`/`scaleY`, so any non-uniform group frame stretches its children with nothing to catch it. → `UNIT-PROVEN`
- [ ] **PD4-28** **Shrink allowances are linear ratios, never area ratios** — *"a logo at 0.9 linear is a 10% crop, not 19%"*. A bug AAE had to fix in two copies of the same function. → `UNIT-PROVEN`
- [ ] **PD4-29** **Upscale caps are applied, not merely computed**: raster never above 1.0×, vector to an explicit cap. In AAE the caps were computed and never applied, and a raster logo was verified upscaling 4×. → `UNIT-PROVEN`
- [ ] **PD4-30** 🔴 **A scale parameter means one thing on every code path.** AAE's `heroScale` was a *replacement* on one path and a *multiplier* on another — a verified 2× difference for the same nominal value, giving the same hero a different size at each target size. → `GATE-PROVEN`
- [ ] **PD4-31** **Clamp non-full-bleed elements inside the canvas at the unit level**, before member expansion, and surface the clamp. AAE's rank-2 fix; took off-canvas elements from nine per test to at most one, and cured *"logo missing"*. → `UNIT-PROVEN`
- [ ] **PD4-32** **A dead-code / unreachable-symbol gate for this subsystem.** AAE's checker is complete, correct, marked RESOLVED in its own audit, and imported by nothing; its candidate scorer is ~500 dead lines; its constraint struct has eight fields and one reader. ⚠ Same family as `L-C12`, which this repo has hit eight times. → `GATE-PROVEN`
- [ ] **PD4-33** **No write-only fields.** If a struct field is never read, it is deleted. AAE carries at least eight, including a hardcoded `cost = 0` that its report presents as a computed figure. → `GATE-PROVEN`


## §10.5.5 · PD5 · Changes

**Goal.** Change what a design contains. **Depends on.** PD3.

**Grounding (Razi).** *"There is none … the psdconversion repo has defined a way to create
PSD from scratch and HTML CSS design from a PSD, if we can build it from scratch then we
can tune and reverse engineer it as well to build for changes."* Confirmed in the source:
the pipeline constructs shape layers (`vectorMask` + `vectorFill`), editable type layers
and pixel layers from nothing. **Construction reversed is mutation.**

- [ ] **PD5-01** The change primitives: text · image · position · size · colour · shape geometry · visibility. → `UNIT-PROVEN`
- [ ] **PD5-02** **A text change preserves `styleRuns`.** The spike found a layer whose three runs alternate white and yellow; a naive replacement flattens it to one colour and nothing reports the loss. → `GATE-PROVEN`
- [ ] **PD5-03** An image change preserves fit and crop intent. → `UNIT-PROVEN`
- [ ] **PD5-04** Position and size changes, respecting group transforms. → `UNIT-PROVEN`
- [ ] **PD5-05** Colour changes across fill, stroke and gradient stops. → `UNIT-PROVEN`
- [ ] **PD5-06** Shape geometry changes on Bezier knots — available only because `PD1-05` refused to rasterize. → `UNIT-PROVEN`
- [ ] **PD5-07** Every change is an **IR mutation and is reversible**; the inverse is computed, not remembered. → `UNIT-PROVEN`
- [ ] **PD5-08** A **change set** is a value, applied atomically — all or none. Half-applied is worse than refused. → `UNIT-PROVEN`
- [ ] **PD5-09** Changes write back to **PSD** through the construction path. → `SMOKE-PROVEN`
- [ ] **PD5-10** Changes write back to HTML and PDF. → `SMOKE-PROVEN`
- [ ] **PD5-11** **A change never silently drops a layer property.** Anything the mutation cannot carry is reported. → `GATE-PROVEN`
- [ ] **PD5-12** Changed output is graded by PD2 — a change must not degrade untouched layers. → `GATE-PROVEN`
- [ ] **PD5-13** The fleet invokes changes as tools. → `GATE-PROVEN`

**Smoke — SD5**
- [ ] **SD5-01** Text is replaced on a multi-run layer and every run's colour survives.
- [ ] **SD5-02** A change is applied, written to PSD, reopened, and the change is present.
- [ ] **SD5-03** A change set that cannot fully apply is refused whole, leaving the IR untouched.

---

## §10.5.6 · PD6 · Brand guideline

**Goal.** Ingest a brand guide and apply it. **Depends on.** PD5.

**Why it follows PD5.** A rebrand is a batch of changes driven by a document. Building it
before the change primitives would mean building those primitives twice.

**Grounding (researched 2026-09-01, razi-ruled: *"search the web for an example brand guide and we use it, i dont have one right now"*).**

Real published guides were surveyed to derive the schema rather than inventing one. The
[openSUSE brand guidelines](https://opensuse.github.io/branding-guidelines/) were readable end
to end and are the **reference fixture**; NASA, Uber, Slack, Netflix and Oatly were surveyed for
the field set. What a guide actually specifies:

| Section | Fields a machine-readable guide must carry |
|---|---|
| **Colour** | named entries, each in several spaces — `hex` and `rgb` (digital), `cmyk`, `pantone_coated`/`pantone_uncoated` (print), `grayscale_percent` (mono). Roles: primary, secondary, accent — **plus tints**, which openSUSE carries four of per colour |
| **Typography** | `family` · `weight` (Light/Regular/SemiBold/Bold) · **`role`** — titles, taglines, body. The role is the load-bearing field: a rebrand maps *roles*, not font names, or a display face lands in body copy |
| **Logo** | variants (horizontal, stacked, mark-only), clear space, minimum size, permitted backgrounds |
| **Misuse** | explicit do-not rules — the half that makes a guide checkable rather than decorative |

⚠ **Two findings that shape the format.** First, real guides are **incomplete**: openSUSE's
own document specifies no logo variants, no minimum sizes and no clear-space rules, and gives
colours only as an image. A format that requires every field will reject most real guides, so
**every field is optional and absence is reported, never inferred** (`PD6-08`). Second, guides
ship as **PDFs** — so `PD1`'s PDF ingestion is the natural intake path, and the two capabilities
compose rather than duplicating each other.

- [ ] **PD6-01** **The brand-guide format** — a real input, not a prompt string. AutoDesign's premise is that findings *"cite the brand guide they came from"*, which free text cannot support. → `DOC-GATE`
- [ ] **PD6-02** Ingest a guide into a typed value. → `UNIT-PROVEN`
- [ ] **PD6-03** Palette mapping: source colours → brand colours, including near-matches. → `UNIT-PROVEN`
- [ ] **PD6-04** Type mapping: source families → brand families, reusing `PD1-04`'s resolution reporting. → `UNIT-PROVEN`
- [ ] **PD6-05** Logo and asset substitution. → `UNIT-PROVEN`
- [ ] **PD6-06** A rebrand **compiles to a `PD5` change set** and is applied by the same path. → `GATE-PROVEN`
- [ ] **PD6-07** **Every applied rule cites the guide rule it came from.** This is the whole reason AutoDesign wants a guide rather than an instruction. → `GATE-PROVEN`
- [ ] **PD6-08** An unmappable rule is **reported, never guessed**. A plausible invented colour is worse than a refusal, because it ships. → `GATE-PROVEN`
- [ ] **PD6-09** Rebranded output is graded by PD2. → `GATE-PROVEN`

**Smoke — SD6**
- [ ] **SD6-01** A guide is ingested and a design is rebranded; every change cites its rule.
- [ ] **SD6-02** A guide with a rule the design cannot satisfy reports it rather than approximating.

---

### P4-F · Assignment actually uses affinity *(from the pre-P8 fleet audit, `_verify/A15`)*

> 🔴 **Razi's standing complaint, with a mechanism.** *"Work posted by the
> supervisor did not get picked up for a long time, and was not assigned using
> the affinity."* Both halves are real, and the affinity half is worse than
> "the algorithm needs fixing": **no assignment path consults it at all.**
> `assign_task` and `assign_tasks_batch` take `worker_slug` from the model's
> tool arguments and call only `record_assignment` — they *write* history and
> never *read* it. Repairing the scorer alone would change nothing.

- [ ] **P4-32** 🔴 **Dispatch consults affinity deterministically.** Rust picks the worker when the caller supplies no slug, or the Supervisor must pass the scorer's answer together with an explicit override reason. ⚠ The spec's own determinism table (`04 §7`) classifies routing as `MATH`; the shipped mechanism is a prompt telling an LLM to prefer a hard-coded keyword table and to *skip* the scorer (`defaults.rs:269-271`). Gate: with N recorded outcomes favouring worker X for a task type, a new task of that type is assigned to X **without the model choosing**. *(Blocked on `P4-07` and `P4-25`.)* → `GATE-PROVEN`
- [ ] **P4-33** **The static pattern layer is unreachable, and no gate covers it.** `select_worker` requires `static_conf > 0.85` (`scorer.rs:135`); the highest confidence any pattern declares is exactly `0.85` (`patterns.rs:27,46`). `0.85 > 0.85` is false, always — so the `method: "static"` the tool advertises in its own output can never be produced. Fix the comparison or the confidences, and unit-prove that at least one shipped pattern can now produce it. Covers `F-ROU-03`, which is named in spec `04 §5.1` and appears in no task and no gate. → `UNIT-PROVEN`
- [ ] **P4-34** 🔴 **One claim path, and the recipes use it.** `P4-07` and `P4-08` harden `claim_task_with_pid`; the shipped recipes call `update_task_status(id,'in_progress',slug)` instead (`defaults.rs:431` and its four twins), which performs **no ownership check and no load guard**. So task theft is wide open on the only path the fleet actually takes, and no PID is ever recorded — degrading orphan recovery to the 4-hour TTL. The already-fixed atomic `claim_next_ready` is reached only by `airsctl` and tests. Gate: driving the **shipped recipe text**, worker B cannot move worker A's task to `in_progress`. ⚠ **This is the task that makes `P4-07`/`P4-08` mean anything** — until it lands, both harden a function the fleet never calls. → `GATE-PROVEN`
- [ ] **P4-35** **The tick gate stops swallowing the work the kick was sent for.** `runner.rs:206-215` short-circuits the whole tick when nothing is `Assigned | InProgress` — *before the engine is built*. So a **`rejected` task never wakes its worker** (the dead Mode 2 branch counted `rejected`; the live one does not), and an **idle worker never reads its ACP inbox**, because `list_worker_messages` is never reached. Widen the gate or handle both before the short-circuit. → `SMOKE-PROVEN`
- [ ] **P4-36** **A worker liveness watchdog.** A worker left in `working` is skipped by both the cron gate and `kick_worker`, and nothing rescues it: no tick deadline, and `WorkersDb::heartbeat` has **zero callers in the app**, so `last_seen` is never refreshed and no watchdog reads it. Write it on tick start and end; reset a worker past a stated ceiling with a system event and reconcile its task. Gate: kill a tick mid-flight, the worker ticks again without an app restart. → `SMOKE-PROVEN`
- [ ] **P4-37** **`kick_inflight` is released on panic.** The synchronous early returns are RAII-guarded; the spawned task removes its slug on its last line, so a panic inside `run_recipe` holds the slot for the process lifetime and every later kick logs *"another kick is already in flight"*. Push dispatch for that worker is then dead while the roster still shows it scheduled. → `UNIT-PROVEN`
- [ ] **P4-38** **Batch assignment honours worker load, or the recipe stops claiming that it does.** The Supervisor's prompt asserts *"the batch tool will refuse and report per-entry failures"* (`defaults.rs:281-283`); `assign_tasks_batch` checks the task's status and the slug and **never looks at the target's load**. A comment asserting the opposite of the code. → `UNIT-PROVEN`
- [ ] **P4-39** **`needs_info` is not a terminal state.** Both the blocked-task path and the budget breaker park tasks there, and the Supervisor's self-heal list has no branch for it — so a phased task sits there for ever. Pairs with `P4-28`: `unblock_ready_tasks` has exactly one caller, the UI, so nothing unblocks in autonomous operation. → `SMOKE-PROVEN`
- [ ] **P4-40** **Clear `claimed_by_pid` and `claim_host` on every non-claim status write.** `update_status_with_reason` leaves them untouched for ever, so a task carrying a PID from a previous app run is an orphan candidate — and `check_task` can release a freshly-assigned task back to `ready`, wiping `assigned_worker`. A task can bounce ready → assigned → ready across restarts. → `UNIT-PROVEN`
- [ ] **P4-41** **Epics are reachable from the fleet's real surface.** The model, the tools, the Tauri commands and an Epics tab all exist — and **no recipe mentions `create_epic` or `create_phased_task`**, the Supervisor's tool list omits them and tells it to decompose with flat `create_task`, and no frontend caller exists. The panel is permanently empty because nothing ever creates the row. ⚠ **Extends `P4-27`, whose stated reason is now stale** — it says the tool is "PA-only and reaches tools only via MCP"; MCP is gone and the tool *is* registered on the path the Supervisor uses, so fixing registration alone would change nothing. → `SMOKE-PROVEN`
- [ ] **P4-42** **The QC recipes read their inbox, or stop being addressable.** `qc_agent` and `qc_dispatcher` contain no `list_worker_messages`, so mail to them is never read and every broadcast leaves permanently unread rows; `kick("qc-dispatcher")` warns *worker not found*, since no such row exists. And `list_stale_worker_messages` has **zero callers**, so delivery is at-most-once against a recipe that promises "lossless". → `UNIT-PROVEN`
- [ ] **P4-43** **An empty board stops costing money.** `supervisor` invokes the LLM every 60 s and `qc-agent` every 30 s against an empty board, replying "Board clean — idle". The `worker-*` short-circuit has no counterpart for either, and the QC queue's own skip lives inside the dead Mode 2 branch. → `BENCH-PROVEN`
- [ ] **P4-44** **Procedural memory is wired for workers** *(razi-ruled, `D5` — `P8-40`/`P8-41` build the seam, this turns it on)*. A worker that has run this build before is the beneficiary. Scoped to the tools the model actually invoked, inside the existing turn budget, and off by default until the equivalence gate in `P8-41` is green. → `SMOKE-PROVEN`


### PD6-B · The failure this phase exists to avoid *(AAE post-mortem)*

- [ ] **PD6-10** 🔴 **Wire the brand guideline to layout composition on day one, with a test that proves a guideline changes the output.** AAE's most severe open finding is exactly this: *"the brand guideline did not influence output."* The policy schema was written; the proof that a policy could move a logo came **a year later**. A guideline that ingests, validates, reports success and changes nothing visible is this phase failing while reporting green. → `GATE-PROVEN`


# §11 · Phases P5–P9 — full task breakdowns

**Authored 2026-09-01** on Razi's direction: *"for the phases that has no tasks, write
them down now, so explore it properly now rather than later."*

⚠ **This overrides the position previously stated here** — that late task lists avoid
staleness. That reasoning was not wrong, and the guard it protected still stands: `§0.12`
requires re-reading the specs at the start of every phase, and any task below that the
code has since falsified is corrected then. What changed is the cost of *not* having the
map: two rollouts cannot be sequenced against phases whose contents nobody has written.

**P8 has moved.** It is now an **R1** phase, running before the AutoDesign handover, and
its numbers below are **measured, not estimated** (`wc -l`, `grep -c` against the tree on
2026-09-01).

---

## §11.1 · P8 · Sidecar deletion — R1, before handover

**Goal.** Not one line of sidecar code survives, by content **or** by address.
**Depends on.** `P3-F` — native memory replaces `memory-mcp-server`, the last live
dependency. **Spec.** `09 §4`.

**Measured footprint.**

| | Count |
|---|---|
| `src-tauri/src/sidecar/` | `client.rs` 1,604 · `mod.rs` 1,270 · `sse.rs` 201 · `startup_progress.rs` 222 |
| `src-tauri/src/commands/sidecar_commands.rs` | 570 LOC, **17** `#[tauri::command]` |
| **Total Rust** | **3,867 LOC** |
| `src-tauri/resources/sidecars/` | **225 MB** (`core-orchestrator`, `memory-mcp-server`) |
| `.gitmodules` | **2** stanzas |
| Files referencing "sidecar" | **14** |

**Why this is now cheap.** `P1` already built the seam: `autodev-core` declares
`OrchestratorProbe` and ships **`NoOrchestrator`**, a null implementation. The desktop's
`OrchestratorClient` implements the same trait. So the excision is a **seam swap**, not a
port. `runtime_config.skip_sidecars` already exists, which means the app has a tested path
that never starts them.

### P8-0 · Live defects — closed before anything is deleted

> ⚠ **Placed first deliberately.** Every task in this group is a defect that is
> live in the shipped code today. A deletion phase that runs before them buries
> them: the diff gets large, the defects get attributed to the deletion, and the
> partner team inherits them. Found by the pre-P8 investigation
> (`_verify/A11`, `A13`, `A14`, `A15`); razi-ruled into P8 on 2026-09-03.

- [ ] **P8-25** 🔴 **Every recipe saved in the UI is silently stranded.** `FallbackPolicy` derives `#[default] Mode2Only`, `Recipe.fallback_policy` is `#[serde(default)]`, and `RecipeEditor.tsx`'s `buildRecipe()` omits the field — so a recipe created in the UI deserialises as `Mode2Only`, hits the guard at `runner.rs:717-737`, reports `fallback_idle`, and **never runs and never errors**. ⚠ `update_recipe` is a **full overwrite**, so editing a *shipped* recipe strands it too — silently undoing `P4-31`, which is the fix that flipped those eight recipes to `Mode1Capable`. Sequenced with `P8-45`: if the enum is being deleted anyway, delete it here rather than fixing the default twice. → `LIVE-DB-PROVEN`
- [ ] **P8-26** 🔴 **`bash` rewrites POSIX pipes into PowerShell cmdlets and runs them under `cmd`.** This is **`BUG-COMPILE`**, the first defect in the whole restructure tracker, unrepaired. `rewrite_for_windows` (`bash.rs:533-545`) emits `Select-Object -First`, `Select-Object -Last` and `Select-String`; `bash.rs:116` hands the result to `cmd /C`, which has none of them. So `npm run build 2>&1 | tail -50` becomes a pipeline whose right half does not exist, `cmd` returns the last element's status, `build_evidence::record` stores a **successful build as failed**, and `update_task_status` reads that to veto `qc_review`. **razi-ruled:** stop rewriting; route through `platform/command.rs` (built in `P1`, still zero callers — this is what `P4-11` exists for), preferring a real POSIX shell where one is present and PowerShell **properly** where one is not. → `SMOKE-PROVEN`
- [ ] **P8-27** 🔴 **`bash` never kills its child.** No `kill_on_drop`, no process-group kill, no cancel path — while `validate.rs:281` and `e2e_probe.rs:418` both set it. A timed-out `cargo build` keeps `target/.cargo-lock` and **every later build in the session times out too**. The one executor that runs model-authored commands is the only one without the guard. → `UNIT-PROVEN`
- [ ] **P8-28** 🔴 **The build-evidence gate matches command *text*.** `bash("grep -rn \"npm run build\" package.json")` exits 0, classifies as a passing Node build, and opens the `qc_review` door with nothing built. The gate that decides whether work may be called complete is satisfied by a command that merely mentions a build. → `UNIT-PROVEN`
- [ ] **P8-29** **Unassigning a task leaves it half-assigned.** `update_task_status`'s description says *"pass null to unassign"*, and `db/tasks.rs:671-678` omits the column entirely on `None` — so the task becomes `ready` while still tagged with its worker, which is the exact state the guard twelve lines above it exists to prevent. → `UNIT-PROVEN`
- [ ] **P8-30** **`spawn_subagent` misdescribes its own tool grant.** The description tells the model an empty `tools` list means "MCP tools only"; empty means **unrestricted** — the child inherits the full registry. A model reading its own documentation grants more than it intends. Fix the description **and** decide whether empty should mean "inherit" or "none". → `UNIT-PROVEN`
- [ ] **P8-31** **Two timestamp readers still lie about age, in opposite directions.** `get_qc_queue` (`task_commands.rs:137-141`) parses an offset-less `datetime('now')` string with an RFC3339 parser and `.unwrap_or(0)`s the failure, so **every review reports an age of 0 s and the stalled-review alert can never fire**. The frontend has the mirror bug: `new Date("2026-09-01 06:23:58")` is read as **local** time by V8, so at UTC+5:30 every age is **inflated by 5.5 hours** — a task assigned a minute ago reads as "6 hours ago". ⚠ `P1-13` marked this class closed; it missed this site because the sweep searched for the string `parse_from_rfc3339`, which this site does not contain. The gate must be behavioural, not a grep. → `UNIT-PROVEN`
- [ ] **P8-32** **"Delegate to Fleet" does not delegate.** `AnnotationModal.tsx:114` passes `paMode: 'delegate'`; `agent_commands.rs:122-127` prefers `session.pa_mode`, which is `NOT NULL DEFAULT 'fast_development'` and therefore never empty — so the argument is unreachable and the button runs in whatever mode the session already had. Either honour an explicit argument or drop the parameter and have the flow call `set_session_pa_mode` first; silently ignoring a caller-supplied mode is wrong either way. → `SMOKE-PROVEN`
- [ ] **P8-33** **Triage the remaining `builtin_tools` findings.** 58 were found (`_verify/A11-builtin-tools-audit.md`); the criticals are `P8-26`…`P8-30` above. Work the rest to a decision each — fixed, filed with an owning phase, or dismissed with a reason — and record the disposition in the audit file. Named examples that must not be lost: `grep`'s total-byte bound cannot bind when the matches are in one file; `list_directory` follows symlinks out of the workspace jail; `check_file` returns **success after running nothing** when `checks` is misspelled; `spawn_parallel_subagents` reports success when every subagent failed. → `DOC-GATE`

### P8-E · One prompt floor for every agent *(razi-ruled, `D1`/`D2`)*

> 🔴 **The defect this closes.** A recipe's `instructions` is passed as
> `system_prompt_override`, and `engine.rs:208-227` treats an override as a
> **total replacement**. Workers are therefore strongly differentiated — which
> is correct and stays — but *nothing that should be universal is*: the seven
> `## What You Must Never Do` rules and the user's `AUTODEV.md` reach the
> interactive assistant and **none of the agents that do the work**.

- [ ] **P8-34** Extract an **invariant core** from `BASE_SYSTEM_PROMPT`: identity, the general operating knowledge, and the prohibitions. Small, and everything PA-specific stays behind in the PA prompt. → `UNIT-PROVEN`
- [ ] **P8-35** **A recipe extends the core, it does not replace it.** Every agent — PA, fleet worker, subagent — receives core + its own instructions. The recipe keeps full control of its own role, protocol and rejection criteria. → `UNIT-PROVEN`
- [ ] **P8-36** **`AUTODEV.md` reaches every agent.** Global and per-repo instructions are appended in `build_system_prompt_parts`, which the override branch skips entirely — so the file the user writes to teach AutoDev their conventions is read by the assistant and by no worker. ⚠ Keep the cache split intact: the core and `AUTODEV.md` are **stable**, memory stays **volatile and last** (`P2-14`). → `UNIT-PROVEN`
- [ ] **P8-37** **The git rules in the core are the house rules**, taken from `CLAUDE.md` rather than reinvented: branch before the first commit; stage deliberately, never a blind `git add -A`; never commit secrets or `.env`; never force-push or rewrite published history; never push to `main` or open/merge a PR without the user's approval. → `UNIT-PROVEN`
- [ ] **P8-38** **Recipes stop committing to `main` with `git add -A`.** All five worker recipes run `bash('git add -A && git commit …')` with no branch created (`defaults.rs:462, 721, 953, 1073, 1253`). ⚠ Knock-on: `worker-cleanup`'s **mandatory** functionality check diffs `main` against the current branch, and since nothing creates a branch it diffs `main` against itself — the one hard safety gate in the cleanup worker is judging an empty patch. → `SMOKE-PROVEN`
- [ ] **P8-39** `TG8-xx` — **no agent runs without the core.** Assert it structurally, over the path each agent kind actually takes, not by searching the prompt text for a phrase. Falsify by removing the core from one path. → `GATE-PROVEN`

### P8-F · Procedural memory — the seam *(razi-ruled, `D5`: plumbing here, wiring in `P4`)*

- [ ] **P8-40** A **retrieval seam** in `tool_executor.rs`, which consults memory nowhere today. Retrieved **once per turn**, never per call — the fleet makes many calls per tick and `TG3-08` bounds input growth. → `UNIT-PROVEN`
- [ ] **P8-41** Recalled procedural text goes through `recall.rs`'s **flattener** and the same data framing, so this path cannot re-open the prompt-implant hole `P3-F` closed. It is **advisory context only** — never an input to a tool's arguments, so a tool's behaviour is identical with and without it. Gate that equivalence. → `GATE-PROVEN`

### P8-G · Restore the Live Monitor *(razi-ruled, `D9` — "we need all functionality live")*

- [ ] **P8-42** Re-emit `worker:tick-start` and `worker:tick-end` around the **native** engine run (`runner.rs:802-889`), with a real `tick_duration_ms` and `last_outcome`. Today their only emitters are inside the Mode 2 branch that `P8-B` deletes, so the Live Monitor is already permanently blank and deleting that branch would make the breakage structural. → `SMOKE-PROVEN`
- [ ] **P8-43** `set_worker_status_ex` carries real values on the live path; the frontend statuses that no Rust call site ever sets (`mode1_active`) are either produced or removed. A status a store consumes and nothing writes is a reader with no writer. → `UNIT-PROVEN`

### P8-H · Retire `airsctl agent` *(razi-ruled, `D8`)*

- [ ] **P8-44** **Delete the `agent` subcommand set** — `run`, `list`, `status`, `stop`, `logs` (`src-cli/src/commands/agent.rs:104-395`). All five are pure HTTP to `{orchestrator_url}/api/agents/*` and cannot succeed after P8 on any machine; one error path even tells the user to run `cargo run --bin core-orchestrator`. **Not reimplemented natively** — that would duplicate the desktop engine for a surface with no user. ⚠ `airsctl` keeps its task/DB surface, which is live and used; it is the `agent` verb specifically that is retired. Remove `system sidecars` and the orchestrator row from `system status` in the same commit. → `CODE-PRESENT`

### P8-A · Verify the entry gate
- [ ] **P8-01** `TG8-02` runs: every capability grounding marked **PORT** has a named, tested destination in the Rust tree. → `GATE-PROVEN`
- [ ] **P8-02** The app starts, completes a turn, and runs a design conversion with `skip_sidecars = true` — **absent, not merely stopped**. → `SMOKE-PROVEN`
- [ ] **P8-03** Native memory (`P3-F`) serves every read that `memory-mcp-server` served. → `GATE-PROVEN`

### P8-B · Excise the Rust
- [ ] **P8-04** `main.rs` binds `NoOrchestrator` in place of `OrchestratorClient`. → `CODE-PRESENT`
- [ ] **P8-05** Delete `src-tauri/src/sidecar/` — all four modules, 3,297 LOC. → `CODE-PRESENT`
- [ ] **P8-06** Delete `sidecar_commands.rs` and **all 17** commands. → `CODE-PRESENT`
- [ ] **P8-07** Remove the 17 command registrations from `lib.rs`'s invoke handler; **a registration surviving its command is a compile error, which is the point** — the handler is the one place that cannot silently keep a corpse. → `CODE-PRESENT`
- [ ] **P8-08** Remove `SidecarManager`, the watchdog and the startup-progress FSM from `main.rs` (42 references). → `CODE-PRESENT`
- [ ] **P8-09** Remove sidecar branches from `startup_commands.rs`, `worker_commands.rs`, `budget_commands.rs`, `runtime_config.rs`, `updater/{check,download}.rs`, `mcp/mod.rs`, `logging.rs`. → `CODE-PRESENT`
- [ ] **P8-10** Remove the sidecar events from `autodev-core/src/events.rs` — **and their frontend listeners in the same commit**, or the UI subscribes to an event nothing will ever emit. → `CODE-PRESENT`
- [ ] **P8-11** Remove `skip_sidecars` from `RuntimeConfig`, migrating existing `runtime_config.json` files rather than failing to parse them. → `LIVE-DB-PROVEN`
- [ ] **P8-12** Remove sidecar handling from `src-cli/src/commands/system.rs`. → `CODE-PRESENT`

### P8-C · Filesystem and config removal — the correct procedure for the state we are in

Grounded in `§1.1a`. **The standard `git submodule deinit` → `git rm` sequence will fail
here**, because there are no gitlinks in the index or in history to operate on.

- [ ] **P8-13** **Back up first** (`§0.8`). `submodules/` is 225 MB of untracked working tree with **no remote copy of any local modification** — and it has local modifications (`.port_allocations.json` is dirty). → `CODE-PRESENT`
- [ ] **P8-14** Delete both stanzas from **`.gitmodules`** — the only *tracked* artifact, so a normal file edit and a normal commit. → `CODE-PRESENT`
- [ ] **P8-15** Delete **`.git/modules/submodules/`** — both object stores. ⚠ **Skipping this is the "survives by address" failure**: the working tree goes and the repository still carries the sidecar's git objects. → `CODE-PRESENT`
- [ ] **P8-16** Delete the **`submodules/`** working tree. → `CODE-PRESENT`
- [ ] **P8-17** Delete **`src-tauri/resources/sidecars/`** (225 MB) and its `tauri.conf.json` `resources` entry. → `CODE-PRESENT`
- [ ] **P8-18** Remove the `submodules/` line from `.gitignore` — an ignore rule for a path that cannot exist is a claim the reader must disprove. → `CODE-PRESENT`
- [ ] **P8-19** **No history rewrite.** Verified: this content was never committed to this repository, so there is nothing in history to excise and no force-push to justify. → `DOC-GATE`

### P8-I · The complete conversion — what *"not a single line survives"* requires

> `P8-B`/`P8-C` remove the sidecar's **code and files**. This group removes the
> **old architecture** — the dead two-mode concept, the abstractions that only
> served it, and the shipped prose that still describes it to users and to the
> model. Razi's standing instruction: the AutoDesign team must inherit the new
> AutoDev, not a converted old one. Sourced from `_verify/A14`.

**⚠ Ordering constraint — `P8-45` must land before any deletion in `P8-B`.**

- [ ] **P8-45** 🔴 **Re-point `TG-P4-31` first.** `ci_gate.rs:2811-2828` reads the literal `"let orch_healthy = false"` out of `runner.rs` and calls **`panic!`** when it is absent — deliberately, so that reviving Mode 2 must be an explicit decision rather than a silent pass. It therefore **fails CI on the first commit of `P8-B`**, for a reason that looks unrelated to the change. Rewrite it as a permanent invariant, then retire it in `P8-53`. → `GATE-PROVEN`
- [ ] **P8-46** **Distinguish the two things that share the words "Mode 1 / Mode 2".** `PAMode` (`FastDevelopment` / `Review` / `Delegate`) is **fully live** — it selects the prompt (`context.rs:89-93`) and the tool registry (`builtin_tools/mod.rs:190-284`) on the interactive path, and `sessions.pa_mode` is its source of truth. `FallbackPolicy` is the inert half. ⚠ **Do not sweep `PAMode` up on a name match**: deleting it collapses three prompts into one and hands the PA file and bash tools in a mode advertised as having none. → `DOC-GATE`
- [ ] **P8-47** Delete the `FallbackPolicy` enum, `Recipe.fallback_policy`, and the `Mode2Only` guard at `runner.rs:717-737`. Sequenced after `P8-25`, which is the live half of the same defect. → `CODE-PRESENT`
- [ ] **P8-48** Delete `runner.rs:173-174` — the awaited `/api/system/health` probe whose result is discarded on **every worker tick**, and the `orch_healthy` pin — plus the unreachable branch it guards (lines 276-702, ~430 lines). Re-flatten the control flow. ⚠ `P8-42` must land with it, or the Live Monitor's only emitters go too. → `CODE-PRESENT`
- [ ] **P8-49** Delete `recipes/orchestrator.rs` (`OrchestratorProbe`, `WorkerInvokeResult`, `NoOrchestrator`), `RecipeRunner::is_orchestrator_ready` (zero callers), and the `orch_client` field and constructor parameter. ⚠ This **supersedes `P8-04`**, which binds `NoOrchestrator` — there is no probe left to bind. → `CODE-PRESENT`
- [ ] **P8-50** Delete `register_for_mcp_worker` (`builtin_tools/mod.rs:100-180`, ~80 lines, zero callers, `pub` so no dead-code warning) and the two doc-comments that cite it as live. → `CODE-PRESENT`
- [ ] **P8-51** ⚠ **Split `sidecar_commands.rs` before deleting it.** `list_mcp_tools`, `get_mcp_health` and `force_reconnect_mcp` are **not** sidecar functionality — they serve AutoDev's *client* connections to external MCP servers, which is a live feature with a live UI. Move them to `mcp_commands.rs` keeping the command names identical, then delete the rest. ⚠ **This narrows `P8-06`**, which says "delete all 17". `mcp/mod.rs:1-14` already records relocating a file "by address rather than deleting by association" — the same mistake in reverse. → `CODE-PRESENT`
- [ ] **P8-52** **Decide and land the startup overlay.** Deleting `startup_progress.rs` removes 8 of its 13 phases, all of which narrate Python boot. Collapse to a native FSM or remove the overlay — and update `startupStore.ts` and `StartupOverlay.tsx` in the same commit, because a mismatch shows a spinner that never advances. → `SMOKE-PROVEN`
- [ ] **P8-53** **Decide Demo Mode's fate.** `runtime_config.rs`'s only field is `skip_sidecars`; with no sidecars the feature means nothing. Delete it and its store, command and settings section, or repurpose the file deliberately. Retire the rewritten `TG-P4-31` here. → `CODE-PRESENT`
- [ ] **P8-54** Delete the sidecar-only frontend: `SidecarPanel.tsx`, `SubmoduleStatusPanel.tsx`, `SidecarCrashBanner.tsx` and their mounts. ⚠ Also `SystemHealth.tsx` — **no importer**, and its content is a hardcoded mock reporting "Core Orchestrator — healthy — 99.8% uptime" for a service being deleted. → `CODE-PRESENT`
- [ ] **P8-55** ⚠ **Split `sidecarStore.ts`.** It carries two unrelated concerns: dead sidecar status and **live** external-MCP health (`McpServerHealth`, `loadMcpHealth`, the `startup:progress` listener). Extract `mcpHealthStore.ts`, delete the rest, and update `StatusBar`, `Sidebar`, `Chat` and `AutoDevPanel`. → `CODE-PRESENT`
- [ ] **P8-56** 🔴 **Rewrite `BASE_SYSTEM_PROMPT` — it contradicts itself and describes a system that does not exist.** Rule 8 says *"Lock files are sacred — write `current_tasks/issue-N.lock`"*; sixty-five lines later the same prompt says *"Do NOT write filesystem lockfiles … they would just be dead artifacts."* It also describes coordination "through GitHub Issues", `worker/<role>/<issue-number>` branches, and a *"Python backend (core-orchestrator): run `uv run pytest` and `uv run mypy src/` before opening any PR"*. This is sent to the model on every turn. Sequenced with `P8-34`, which decides what stays in the core. → `UNIT-PROVEN`
- [ ] **P8-57** Rewrite `DELEGATE_SYSTEM_PROMPT`, whose worked example teaches the PA to create a task against the deleted sidecar (*"Add GET /health endpoint to orchestrator API"*), and reconcile `REVIEW_SYSTEM_PROMPT` with the fact that Review and FastDevelopment share both the registry arm and `ManualApproval` — so the "explicit permission checkpoint" it promises is not distinctive. → `UNIT-PROVEN`
- [ ] **P8-58** Fix the two model-facing error strings that tell it to *"wait for sidecar startup"* — `spawn_subagent.rs:112`, `spawn_qc_review.rs:93`. Subagents are in-process; the real cause is `SubagentManager` not being installed, and the model is being told to wait for an event that will never occur. → `UNIT-PROVEN`
- [ ] **P8-59** Clean the shipped recipe instructions — they become worker system prompts. `defaults.rs:292` routes "core-orchestrator, sidecars" to `worker-backend`; `:611` tells that worker to *"modify ONLY … Python/Rust in core-orchestrator, src-tauri, sidecars"*; `:321` and `:1083` name LivingAgent ticks and "local sidecar tools"; `:1225` says a recipe *"stays Mode2Only"* directly above `fallback_policy: Mode1Capable`. Plus eight duplicated `P4-31` comment blocks. → `UNIT-PROVEN`
- [ ] **P8-60** Clean ~20 stale rationale comments that justify live guards by naming deleted mechanisms — `builtin_tools/mod.rs`, `path_lock.rs`, `path_jail.rs`, `write_file.rs`, `edit_file.rs`, `claim_task_tool.rs`, `request_supervisor_action.rs`, `build_evidence.rs`, `tool_effect.rs`, `engine.rs`, `scheduler.rs`. ⚠ **The guards stay** — only the comments are wrong. → `CODE-PRESENT`
- [ ] **P8-61** 🔴 **Rewrite the in-app Docs tab.** ~106 legacy lines across six `src/data/*.ts` files — a capability table listing *"Agent orchestration | 21 FSMs + A2A protocol via core-orchestrator | live"*, an architecture diagram with `core-orchestrator (Python FastAPI, port 4000)`, and `docsAllD.ts:24-57`, which documents three mechanisms **explicitly removed in P0**. This is shipped product documentation; it needs an author, not a find-and-replace. → `DOC-GATE`
- [ ] **P8-62** Rewrite the README's dual-mode, Mode 2, architecture, submodule and FAQ sections (~12 sections), and remove references to `build/bundle-core.ps1` and `build/bundle-memory-mcp.ps1` — `build/` is an empty directory. → `DOC-GATE`
- [ ] **P8-63** Remove the sidecar from packaging: `installers/wix/bundle-sidecars.ps1` and its call in `build-msi.ps1`; `Recommends: python3` from the deb control file and the rpm spec; revisit the dmg entitlement whose comment cites "Python sidecars". → `CODE-PRESENT`
- [ ] **P8-64** Remove `orchestrator_url` and `auto_start_sidecars` from the CLI config schema, and delete `dotenvy` loading from `main.rs`. ⚠ **Keep `scrub_process_env()`** — removing the dotenv load makes it *more* defensible, not redundant: it protects every child process `bash` spawns. Cross-check `security/env_scrub.rs` so the scrub does not quietly become a no-op guard. → `CODE-PRESENT`
- [ ] **P8-65** **The vocabulary gate.** No file under `autodev-core/src`, `src-tauri/src`, `src-cli/src` or `src/` may contain `Mode 2`, `LivingAgent`, `core-orchestrator` or `memory-mcp-server` — first outside comments, then including them once `P8-56`…`P8-62` land. ⚠ Strip comments before scanning, as `ci_gate.rs` already does elsewhere: this repo has tripped its own checks on explanatory prose four separate times. This is P8's exit criterion. → `GATE-PROVEN`

### P8-D · Prune and prove
- [ ] **P8-20** Drop now-unused dependencies (SSE/HTTP client paths kept only for the sidecar) from `Cargo.toml`. → `CODE-PRESENT`
- [ ] **P8-21** **The exit assertion**: no `.py` in the tree · no `.py` in any built installer · `git ls-files | grep -i sidecar` returns nothing · `.git/modules` is gone · the binary contains no sidecar path string. → `GATE-PROVEN`
- [ ] **P8-22** Installer size drops by ~225 MB — **measured before and after**, because "it should be smaller" is not a measurement. → `BENCH-PROVEN`
- [ ] **P8-23** The full suite and every smoke pass with the sidecar gone. → `GATE-PROVEN`
- [ ] **P8-24** `TG2-14`'s sibling for the sidecar: no reference survives in Rust source, `Cargo.toml`, shipped docs, or the frontend. → `GATE-PROVEN`

**Smoke — S8**
- [ ] **S8-01** A clean checkout builds with no submodule initialisation step.
- [ ] **S8-02** The app starts, completes an agent turn and runs a design conversion — no Python process is spawned, verified by process listing.
- [ ] **S8-03** The built installer contains no `.py` file and is ~225 MB smaller.
- [ ] **S8-04** Memory recall works end to end with `memory-mcp-server` absent from disk.

**Exit.** S8 → `TG-P8` → matrix → progress → no-stale → commit.

**Narrative success criterion.** *A newcomer clones the repository, runs one build command,
and gets a working AutoDev. Nothing asks them to initialise a submodule, no Python
interpreter is required, and searching the tree for "sidecar" returns only history.*

---

## §11.2 · P5 · Graph — R2

**Goal.** The graph is compiled, correct across updates, and **read** for context scoping.
**Depends on.** P4. **Spec.** `05`.

⚠ **Entry condition inherited from P1.** `graph_tools.rs` holds **27 structs, none of which
implement `Tool`, and nothing registers them** — `TG1-01`'s registration clause is
unsatisfied and was recorded as such rather than glossed. P5 is where that is closed.

### P5-A · One canonical path
- [ ] **P5-01** One normalized graph database path. **Four spellings produce four databases today.** → `LIVE-DB-PROVEN`
- [ ] **P5-02** The path key is canonicalised and case-folded through the **same shared helper** as `path_lock` and checkpoint retention (`P3-16`) — three private normalisers is how the four spellings happened. → `UNIT-PROVEN`
- [ ] **P5-03** Existing graph databases are migrated or reconciled, not orphaned. → `LIVE-DB-PROVEN`

### P5-B · Non-destructive upsert
- [ ] **P5-04** Stop `INSERT OR REPLACE` on nodes — it **cascade-deletes edges graph-wide**, so a single re-index silently discards relationships nothing will rebuild. → `LIVE-DB-PROVEN`
- [ ] **P5-05** A real upsert: update in place, preserve edges, add what is new. → `UNIT-PROVEN`
- [ ] **P5-06** Edge conservation is a **gate**: node count may change across a re-index, edge count may not fall without a deletion that asked for it. → `GATE-PROVEN`
- [ ] **P5-07** The v6 ladder rung carries a backup for any destructive step (`TG1-04`). → `LIVE-DB-PROVEN`

### P5-C · Incremental correctness
- [ ] **P5-08** Re-cluster on incremental update — stale clusters after an edit are wrong answers delivered confidently. → `UNIT-PROVEN`
- [ ] **P5-09** A changed file updates only its own subgraph. → `UNIT-PROVEN`
- [ ] **P5-10** A deleted file removes its nodes and its dangling edges. → `UNIT-PROVEN`
- [ ] **P5-11** A rename is a rename, not a delete plus an insert that loses history. → `UNIT-PROVEN`
- [ ] **P5-12** Incremental and full re-index converge on the **same graph** for the same tree. The property that makes incremental trustworthy at all. → `GATE-PROVEN`
- [ ] **P5-13** `nodes_fts` stays consistent with `nodes` across every mutation. → `UNIT-PROVEN`

### P5-D · Graph-scoped context (M-18)
- [ ] **P5-14** The graph is **read** during context selection — the capability's whole justification. → `GATE-PROVEN`
- [ ] **P5-15** Relevance is scoped by graph distance, not by file order. → `UNIT-PROVEN`
- [ ] **P5-16** The selection is bounded and interacts correctly with `P3-E` compaction. → `BENCH-PROVEN`
- [ ] **P5-17** A recalled graph node **appears in the outbound prompt bytes** — the same standard `P3-44` holds memory to. → `GATE-PROVEN`

### P5-E · The tools finally exist
- [ ] **P5-18** The 27 `graph_tools.rs` structs implement `Tool`. → `UNIT-PROVEN`
- [ ] **P5-19** They are **registered** and callable by an agent. → `GATE-PROVEN`
- [ ] **P5-20** `TG1-01`'s registration clause closes; the readiness matrix's A1 Gates row flips. → `GATE-PROVEN`
- [ ] **P5-21** Each tool's description is written to `P3-20`'s standard, not inherited from the 33 defective ones. → `DOC-GATE`

**Smoke — S5**
- [ ] **S5-01** Indexing the same tree twice by different path spellings produces **one** database.
- [ ] **S5-02** Re-indexing a changed file preserves every edge not touched by the change.
- [ ] **S5-03** Incremental and full re-index of the same tree produce identical graphs.
- [ ] **S5-04** An agent calls a graph tool and receives a real answer.
- [ ] **S5-05** Two different questions produce different graph-scoped context.

---

## §11.3 · P6 · UI — R2

**Goal.** The anchor is implemented, no event is lost, every surface is reachable.
**Depends on.** P5. **Spec.** `07`.

### P6-A · The anchor
- [ ] **P6-01** The 7px grid, type scale and token set from `ui-anchor.md`. → `DOC-GATE`
- [ ] **P6-02** **Self-host the font.** The CSP blocks the external fetch — *this is why Graph City hangs*, not a rendering bug. → `SMOKE-PROVEN`
- [ ] **P6-03** Dark mode: the app is light-only today. → `SMOKE-PROVEN`
- [ ] **P6-04** Tokens are defined once and consumed everywhere; no literal hex in components. → `GATE-PROVEN`
- [ ] **P6-05** Every surface adopts the anchor; drift is a gate failure. → `GATE-PROVEN`

### P6-B · Event lifetime
- [ ] **P6-06** The event subscription lives **above the tab boundary**, for the session — a tab switch currently drops it. → `SMOKE-PROVEN`
- [ ] **P6-07** No event is lost across a tab switch, verified by counting emitted against received. → `GATE-PROVEN`
- [ ] **P6-08** Reconnection replays what was missed rather than pretending it did not happen. → `UNIT-PROVEN`
- [ ] **P6-09** An unmounted component's listener is removed — 10 dead listeners exist today. → `GATE-PROVEN`

### P6-C · The command contract
- [ ] **P6-10** Add the **2 missing** commands the frontend calls and the backend never defined. → `CODE-PRESENT`
- [ ] **P6-11** Remove the **47 dead** commands no caller invokes. → `CODE-PRESENT`
- [ ] **P6-12** Remove the **10 dead listeners** and **3 dead events**. → `CODE-PRESENT`
- [ ] **P6-13** A **gate** asserting the two sides agree: every invoked command exists, every defined command is invoked. Hand-reconciling 62 mismatches once, without a gate, guarantees a 63rd. → `GATE-PROVEN`
- [ ] **P6-14** Command argument and return types are checked, not assumed. → `GATE-PROVEN`

### P6-D · Settings
- [ ] **P6-15** Every setting persists and **hydrates on startup**. → `LIVE-DB-PROVEN`
- [ ] **P6-16** Provider and model selection survive a restart (`P2-26`). → `SMOKE-PROVEN`
- [ ] **P6-17** `ModelConfigsDb` is read as well as written (`P2-27`) — it is a writer with no reader today. → `GATE-PROVEN`
- [ ] **P6-18** A setting with no effect is either wired or deleted; the UI must not offer an inert control. → `GATE-PROVEN`

### P6-E · Graph City
- [ ] **P6-19** It renders — unblocked by `P6-02`. → `SMOKE-PROVEN`
- [ ] **P6-20** It reads the real graph from `P5`. → `SMOKE-PROVEN`
- [ ] **P6-21** It stays interactive on a real repository's node count. → `BENCH-PROVEN`

### P6-F · Reachability
- [ ] **P6-22** Route or delete the **~3,700 orphaned LOC** in the frontend. → `CODE-PRESENT`
- [ ] **P6-23** Every route is reachable by navigation from a cold start. → `GATE-PROVEN`
- [ ] **P6-24** The frontend's own reachability gate, mirroring `reachability_gate.rs`. → `GATE-PROVEN`
- [ ] **P6-25** The design surfaces (`PD1`–`PD6`) get UI — the standalone product needs what AutoDesign supplied in R1. → `SMOKE-PROVEN`
      ⚠ **Scoped 2026-09-03 (`D7`), and scoped DOWN.** The shape is a **live viewer beside the conversation**: the design rendered on the left, the PA working on it and the chat on the right — the Claude Design arrangement, which Razi pointed at directly. **Not** a Photoshop-style canvas editor: no layer tree, no transform handles, no selection model, no undo stack. Editing is what costs, and the design is edited by *asking*, which is the product's whole thesis. The viewer renders the IR and re-renders when a tool mutates it.
      → **Under-scoped as one task and known to be.** A live IR renderer, the split-pane, and re-render on mutation are three or four tasks at this plan's granularity; sized properly at `P6`'s pre-build refutation (`§0.2m`) rather than guessed at now.
- [ ] **P6-26** Ingestion progress is visible and cancellable for a 25–35 s conversion. → `SMOKE-PROVEN`
- [ ] **P6-27** Fidelity scores are shown to the user, not only returned to callers. → `SMOKE-PROVEN`
- [ ] **P6-28** Adaptation and change tools are drivable from the UI. → `SMOKE-PROVEN`
      ⚠ Drivable **through the conversation**, with the result visible in `P6-25`'s viewer — not through a direct-manipulation surface. A target size and a ratio policy are things you say; the tool is deterministic either way (`PD4-02`).

**Smoke — S6**
- [ ] **S6-01** Graph City renders and stays interactive with the font self-hosted.
- [ ] **S6-02** Events emitted during a tab switch all arrive.
- [ ] **S6-03** Every setting survives a restart.
- [ ] **S6-04** Every route is reachable from a cold start.
- [ ] **S6-05** A PSD is ingested, graded, adapted and exported entirely from the UI.

---

## §11.4 · P7 · Experience — R2

**Goal.** A new user reaches a working agent turn without documentation.
**Depends on.** P6. **Spec.** `07 §7`.

### P7-A · First run
- [ ] **P7-01** The BYOK flow: enter a key, see it accepted, reach a working turn. → `SMOKE-PROVEN`
- [ ] **P7-02** The key is never echoed back — `INV-12`, already gated in P0. → `GATE-PROVEN`
- [ ] **P7-03** First run states what AutoDev needs before it needs it. → `DOC-GATE`
- [ ] **P7-04** No dead end: every first-run failure names its next action. → `SMOKE-PROVEN`

### P7-B · Errors
- [ ] **P7-05** **Fix the two toast producers** — a two-call-site import fix, not a system to build. → `CODE-PRESENT`
- [ ] **P7-06** Every error surfaces once, in one place, in one voice. → `GATE-PROVEN`
- [ ] **P7-07** A provider error says what the user can do about it. → `UNIT-PROVEN`
- [ ] **P7-08** A degraded result (`PD0-04`) is distinguishable from a failure in the UI. → `SMOKE-PROVEN`

### P7-C · Progress
- [ ] **P7-09** Honest progress: no bar that stalls at 90%. → `SMOKE-PROVEN`
- [ ] **P7-10** Cancellation actually cancels, threading `P3-08`'s token through. → `SMOKE-PROVEN`
- [ ] **P7-11** Long operations report what they are doing, not merely that they are doing something. → `SMOKE-PROVEN`

### P7-D · Destructive actions
- [ ] **P7-12** The **10 unconfirmed destructive actions** are confirmed. → `GATE-PROVEN`
- [ ] **P7-13** Each confirmation states what is lost and whether it is recoverable. → `DOC-GATE`
- [ ] **P7-14** Undo exists where the underlying operation supports it (`git/checkpoint.rs`). → `SMOKE-PROVEN`

### P7-E · Coherence
- [ ] **P7-15** One vocabulary — worker, task, run, session mean one thing each, everywhere. → `DOC-GATE`
- [ ] **P7-16** One interaction grammar across surfaces. → `DOC-GATE`
- [ ] **P7-17** Empty states teach rather than apologise. → `SMOKE-PROVEN`
- [ ] **P7-18** The capstone walkthrough runs with no documentation open. → `SMOKE-PROVEN`

**Smoke — S7**
- [ ] **S7-01** A new user with no documentation enters a key and completes a turn.
- [ ] **S7-02** Every destructive action asks first.
- [ ] **S7-03** A cancelled operation stops within a bounded time and says so.

---

## §11.5 · P9 · Delivery — R2

**Goal.** Signed, verified installers for Windows, Linux and macOS.
**Depends on.** P7. **Spec.** `09`.

⚠ **Start certificate procurement at R1 close, not at P9.** Apple and Windows certificates
have external lead time that no amount of engineering shortens, and this is the one task
here whose duration is not ours to control.

### P9-A · Signing
- [ ] **P9-01** Apple Developer ID certificate obtained and stored. **Razi's action** — Claude must not generate, read or handle the private key. → `DOC-GATE`
- [ ] **P9-02** Windows code-signing certificate obtained and stored. **Razi's action.** → `DOC-GATE`
- [ ] **P9-03** The updater's production keypair generated and the public half pinned — closing `TG0-01`'s unproven clause, which has been open since P0. → `GATE-PROVEN`
- [ ] **P9-04** CI signs without any secret reaching a log. → `GATE-PROVEN`

### P9-B · Installers
- [ ] **P9-05** Windows installer builds, installs, launches. → `SMOKE-PROVEN`
- [ ] **P9-06** Linux installer builds, installs, launches. → `SMOKE-PROVEN`
- [ ] **P9-07** macOS installer builds, installs, launches — **and is actually launched**, which P0 never did. → `SMOKE-PROVEN`
- [ ] **P9-08** macOS notarisation passes. → `GATE-PROVEN`
- [ ] **P9-09** Each installer carries no `.py`, no sidecar, no dev `.env` (`P8-21`, `TG0-04`). → `GATE-PROVEN`
- [ ] **P9-10** Installed size is measured and recorded per platform. → `BENCH-PROVEN`

### P9-C · The updater
- [ ] **P9-11** An update signed by the pinned production key installs. → `SMOKE-PROVEN`
- [ ] **P9-12** An unsigned or wrongly-signed update is **refused** — P0's guarantee, re-proven against the real key. → `GATE-PROVEN`
- [ ] **P9-13** A failed update rolls back and the app still starts. → `SMOKE-PROVEN`
- [ ] **P9-14** Update checks respect the user's setting and never run unasked. → `GATE-PROVEN`

### P9-D · Capstone
- [ ] **P9-15** **The headline acceptance test**: on a clean machine, install AutoDev, enter a key, hand it a task, and receive a build-verified artifact. → `SMOKE-PROVEN`
- [ ] **P9-16** The R1 capability still works from the shipped installer — ingest, grade, adapt, export. **Shipping R2 must not silently break R1**, and the only way to know is to test it here. → `SMOKE-PROVEN`

**Smoke — S9**
- [ ] **S9-01** Clean-machine install on all three platforms.
- [ ] **S9-02** A tampered update is refused.
- [ ] **S9-03** The capstone task completes from a shipped installer.
- [ ] **S9-04** A PSD ingests and exports from a shipped installer.

---

# §12 · Event design

One taxonomy, emitted by `autodev-core`, consumed by the UI and the CLI:
`TurnStarted` · `TextDelta` · `ThinkingDelta` · `ToolCallStarted` · `ToolCallFinished` · `PermissionRequested` · `TurnEnded` · `Error`

**Skip semantics: an absent event is never a silent failure.** Every terminal path emits exactly one `TurnEnded` (P3-05). Every emitted name has a consumer and every consumed name has an emitter, enforced at build time (P6). *`agent:thinking` is emitted on every thinking delta today and has no listener anywhere — reasoning tokens stream into a black hole.*

# §13 · Coding conventions

Match the surrounding file. `thiserror` for typed errors — **no `let _ =` on a fallible call**, which is the single most common defect class in this codebase. No `.unwrap()`/`.expect()` on any path reachable from user input, the network, or the filesystem. Every subprocess has a timeout and `kill_on_drop`. Every lock is released by RAII. One shared path-normalisation helper. Timestamps through one helper. `#[deny(clippy::todo, clippy::unimplemented)]` in `autodev-core`.

# §14 · Testing strategy

| Layer | Tool | Runs |
|---|---|---|
| Rust unit | `cargo test` | Every commit — **blocking** from P0 |
| Rust integration | `cargo test --test '*'` | Every commit |
| Provider conformance | Recorded fixtures (blocking) + live nightly (`.env` harness) | PR + nightly |
| Frontend unit | Vitest | Every commit — from P0 |
| Lint | ESLint + clippy + `cargo fmt` | Every commit — **blocking** |
| End-to-end | WebDriver against the built app | PR, all three platforms |
| Gates | `TESTING_GATES.md` | After build + smoke, per phase |

# §14.5 · Operational cadence register

Required by **R-13**. The adversarial review found that `interval`, `cadence`, `backoff` and `max_retries` appear **zero times across all thirteen planning documents** — eleven mechanisms specified by *what* and never *when*. A background job with no stated cadence, no cost and no failure behaviour is a defect waiting to be discovered in production.

**Every background mechanism answers all five columns before it ships.**

| Mechanism | Trigger | Cadence | Cost per run | On failure | Phase |
|---|---|---|---|---|---|
| **Provider retry** *(GAP-12 — `ErrorClass::Retryable` has no reader today)* | A retryable provider error | Exponential backoff **250 ms → 8 s, max 5 attempts**; honour `Retry-After` where the provider sends one (Anthropic, OpenAI only). **`QuotaExhausted` is never retried** — a spend-cap 429 never succeeds on retry | One extra request | Surface with the provider's own reason; never silent | P2 |
| **Embedding drain** | A memory write enqueues | **Continuous**, one at a time, off the async runtime; backoff to 30 s on repeated failure | ~50 ms local inference | Row stays unembedded; recall degrades to FTS5 and **says so** | P3 |
| **Embedding backfill** | Model newly installed, or dimension change | Once, batched, resumable, in the background | Minutes for a large store | Resumes from the last embedded row | P3 |
| **Reconciliation sweep** (orphans, stranded claims) | Timer, **plus** on app start | **Every 5 min.** Dispatch is event-driven (M-11); this is the safety net, not the mechanism | One indexed query | Log + retry next tick | P4 |
| **Graph incremental update** | File-watcher debounce | **2 s debounce, coalesced**; never more than once per 30 s per repo | Sub-second for a few files | Mark stale; full rebuild offered | P5 |
| **Graph full rebuild** | Explicit user action, or a stale marker older than **7 days** | On demand only — **never** on a timer | 30–90 s, CPU-heavy | Surface with the failing path; partial graph retained | P5 |
| **Retention prune + `VACUUM`** | Timer | **Daily**, on app start if more than 24 h have elapsed | Seconds | Log; retry next day; **never block startup** | P1 |
| **Cost rollup** | Timer | **Hourly**, and **on app start** — the current job sleeps a full hour *before* its first run and only ever rolls up *yesterday*, so a desktop app used in sub-hour sessions never completes one | Milliseconds | Log; retry next tick | P2 |
| **Update check** | Timer | **Every 4 h** (unchanged), with backoff to 24 h after 3 consecutive failures — today it retries forever with no backoff and never surfaces | One HTTPS request | Surface after 3 failures; a user must know update checks are failing | P0 |
| **Cache-hit alarm** | Every provider response | **Evaluated per request**; warn when `cached_read == 0` across **3 consecutive** same-prefix requests | Nil — reads a field already present | Warn in-app; surface cache-hit rate on the dashboard | P2 |
| **Worker kick timeout** | A push-dispatched run | **10 min hard timeout**, permit released by RAII | — | Task released to the reconciler | P4 |
| **Checkpoint prune** | After a write | Keep **50 per file**, prune inline | Milliseconds | Log; never block the write | P4 |
| **Memory consolidation** | — | **Deferred** (anchor Phase 4, out of scope per ADR-05-05) | — | — | post-P9 |

**Three values are genuinely a preference rather than a constraint**, and are cheap to change because all thirteen rows are config, not architecture:
- the reconciliation sweep at **5 min** — trades responsiveness against idle CPU;
- the graph incremental debounce at **2 s / 30 s** — trades context freshness (M-18 depends on it) against CPU on a busy repo;
- the stale-graph threshold at **7 days**.

Defaults are proposed above and adopted unless Razi says otherwise. The rest follow from a constraint: retries must not amplify a spend-cap 429; embedding must never delay a turn; a prune must never block startup.

# §15 · Risk log

| ID | Risk | Mitigation | Owning phase |
|---|---|---|---|
| **R-1** | ~415 defects swallow the restructure | Slices gated on outcomes, not defect counts; the register is a map, not a backlog | All |
| **R-2** | Caching adopted, silently does nothing | Alarm before cache (P2-12) | P2 |
| **R-3** | The port reproduces the sidecar's own defects | Explicit fix-list (P3-43) | P3 |
| **R-4** | macOS verified last, blocks release | macOS CI in **P0** | P0 |
| **R-5** | BYOK turns first-run into a wall | M-35 gated with an acceptance test | P7 |
| **R-6** | Deleting the sidecar strands an unenumerated capability | P8 entry gate: full suite with the sidecar **absent** | P8 |
| **R-7** | `Item::Opaque` deferred, adapters rewritten twice | Day-one trait decision (P2-01) | P2 |
| **R-8** | Affinity trained on corrupted attributions | ✅ **CLOSED 2026-08-31.** Both operational clauses are now ordered tasks rather than prose: `P4-25` is explicitly **blocked on `P4-07`**, and `P4-24a` decides the fate of existing `routing_history` under §0.8 backup discipline. *(A sequencing constraint is a task-dependency property, not a runtime assertion — correctly not a gate.)* | P4 |
| **R-9** | Rate limits and cost during development | Recorded fixtures; live calls nightly only | All |
| **R-10** | Windows stays the only tested platform | `PLATFORM-TRIPLE` evidence class; P4 exit names all three | P0, P4 |
| **R-11** | *(new, R1)* ONNX runtime downloaded at build time breaks offline/reproducible builds | Pin and vendor (P3-33) | P3 |
| **R-12** | *(new, R1)* `autodev-core` extraction is larger than scoped — 132 `emit_*` sites, not 62 lines of `AppState` | Sized as its own group (P1-B); event sink injected | P1 |
| **R-13** | *(new, R3)* No cadence defined anywhere for 11 background mechanisms | ✅ **CLOSED 2026-08-31.** The register is written — **§14.5**, thirteen mechanisms, each answering trigger · cadence · cost · failure behaviour · phase. Includes the retry policy `ErrorClass::Retryable` had no reader for (GAP-12) | §14.5 |
| **R-14** | *(new, P3-F)* **A test run migrated the developer's live `memory.db`.** Confirmed by artifact, not inference: the destructive-migration guard wrote two pre-v3 backups into `~/.autodev/backups/` and the live database moved v2 → v3 outside any app launch. No data was lost — 39/39/0/0 rows unchanged, integrity `ok`, backups verified — and the guard behaved exactly as designed. **The hazard is the reach, not the outcome:** `§0 Rule 13` treats a suite that can touch a live resource as a destructive suite, and this one demonstrably could. ⚠ **Which test is NOT established.** A controlled probe — a v2 fixture in a sandbox `HOME`, the whole workspace suite run against it — leaves that database at v2, so the tree as it stands does not reproduce it; the offender is either gone or conditional. Recorded open rather than asserted fixed. **Remedy:** make the reach impossible rather than hunt the caller — resolve the data root through one seam that fails loudly under `cargo test` unless a sandbox is set, so no future test can reach the real home by omission. | P3 / P4 |
| **R-15** | *(new, pre-P8)* **The fleet advertises capabilities it does not have.** Epics have a model, tools, Tauri commands and a UI tab, and **no recipe ever creates one** — the panel is permanently empty. Worker chat works agent-to-agent only when the recipient is already busy. Per-task budget has zero production callers. Affinity is never consulted. Each is *built-but-unreached*, which reads as "working" from every surface except behaviour. ⚠ **ROI was in this class and has been removed rather than built** — razi-ruled 2026-09-03 (`D6`): *"drop ROI completely, it is confusing as well as we already have dashboard."* No model, writer, reader or task existed, so nothing was deleted; what changes is that none will be added. | P4 |

# §16 · Change log

*Append-only, newest first.*

| Date | Rev | Change |
|---|---|---|
| 2026-09-01 | **Re-planned around two rollouts** (`ADR-LOG-RESTRUCT-0024`). `§5` rewritten: R1 *"backend ready, wire it to AutoDesign"*, R2 *"the application is ready"*, with an explicit **handover point after P8**. Design work promoted from `P3-G` (10 tasks) to **`§10.5`, phases PD0–PD6 (88 tasks)** — ingestion (PSD **and** PDF), pixel-level fidelity with refinement tooling, export (web/PDF/PSD), adaptation as a **deterministic tool**, changes, brand guideline. **`P8` moved into R1** before handover: measured at 3,867 LOC, 17 commands, 225 MB, 2 submodule stanzas; affordable because `NoOrchestrator` already exists. **`P5`–`P9` given full task breakdowns** (107 tasks) rather than deferred to phase start. **Providers narrowed to three** with Gemini primary; `P2-07` struck. **`P3-F` memory returned to the critical path.** Total open tasks: **292**. |
| 2026-09-02 | 1.9 | **`P3-F · Memory` built and closed out; `A3` reaches 67% and overall readiness 22.0% → 34.0%.** ⚠ I wrote 100%/40.0 first and corrected it within the hour — see `L-34`: every `TG-P3` gate being green is a fact about the gates I wrote, not about the spec register the Gates row actually tracks. A fresh-context verifier returned five NOT SATISFIED and three PARTIAL across the eight memory gates, including that **the vector leg does not exist** — `memories_vec` is selected and never created — which also qualifies `P3-31`. 18 tasks and both S3 smokes done, **608 workspace tests**, every gate falsified. The `§0.2a` audits found **four live defects in this phase's own freshly-completed code** — a provenance escalation inside the task that forbids it, a retrieval index two of three tiers never wrote to, a redaction boundary the only live writer skipped, and recalled memory able to author the system prompt — plus a fifth in shared code, a backup verifier that refused every correct backup after FTS5 landed. All closed, gated and falsified (`ADR-LOG-RESTRUCT-0032`). §0.16 gained **L-31** (a comment, a test and a gate written in one sitting are one witness quoted three times), **L-32** (a boundary enforced in N−1 places reads as complete from inside any one of them), **L-33** (falsify the guards too — nobody does, and their failure mode is refusing correct work) and **L-34** (the gates I write are downstream of my reading of the task; the register is upstream of it). `L-C6` → 11; `L-25` recurred **in a gate written the same day**. `R-14` filed open: a test run reached the live database. |
| 2026-09-01 | 1.4 | **P0 built and closed out — 8 commits, `5fca2bb`…`4b17c6f`.** 26 of 28 original tasks done, plus `P0-13b` and six added tasks (`P0-28`…`P0-33`). **`P0-26` was REVERSED** — deleting the `dialog`/`fs` plugin config would have erased the only evidence of a live runtime break (the Logs export fails on every click); superseded by `P0-31`, which makes it work. **`P0-21` did not reach its evidence class**: the e2e harness exists and is wired into CI but has never executed an assertion (`msedgedriver` absent locally; `tauri-driver` has no WKWebView backend, so macOS can never run it). Readiness **0% → 2.7%**: Build ✅, Smoke and Gates ☐ under the no-partial-credit rule. Overlay: SO-4 and SO-11 → GREEN, SO-2 and SO-12 → AMBER; **SO-6 stays RED deliberately** — `platform/command.rs` is proven but has no caller, and a guard nothing calls is not a guard. Eight runtime ADRs logged (`0001`…`0008`), three of them `razi-ruled` or `flagged-and-proposed`. §0.16 gained **L-C6/L-C7/L-C8** and lessons **L-7…L-10**, and §0.2i now carries the incremental-write dispatch clause that L-C8 exists because I forgot. |
| 2026-08-31 | 1.3 | **Razi's development rules from a sibling audit adapted into §0.** Added: **§0.2a** two mandatory post-phase auditors (work + area), with the area auditor's brief tuned to this codebase's measured defect profile — `let _ =`, dead parameters, guards that cannot fire · **§0.2b** a phase cannot close over an open finding · **§0.2c** every finding runs through Razi · **§0.2d** never stop midway, ask in-flight via `AskUserQuestion` · **§0.2e** every task names its proof · **§0.2i** two subagents is the floor · **§0.2j** search the web before escalating; escalate decisions, not facts · **§0.2m** adversarially verify each phase **before** building it · **§0.2n** every phase writes its learnings · **§0.2L-1…5** method rules. **§0.16 learnings register created and seeded from the planning phase** — five recurring classes with running counts, six lessons, and the method that worked. Two adaptations rather than literal adoptions, both stated: §0.2e allows gates to be **shared** across tasks (a 1:1 rule would manufacture coverage — the eight P0-A updater tasks prove one thing); and every example was **replaced with this repo's own evidence**, including the six errors this plan's own pre-GO review found in it. |
| 2026-08-31 | 1.2 | **The three remaining open items closed, none of which needed an owner decision.** **R-8** — its two operational clauses become ordered tasks rather than prose: `P4-25` is explicitly blocked on `P4-07`, and new task `P4-24a` decides the fate of existing `routing_history` under §0.8 backup discipline. The gate harvest was right that a sequencing constraint is a task-dependency property, not a runtime assertion, so it is deliberately **not** a gate. **R-13** — the **operational cadence register is written (§14.5)**: thirteen mechanisms, each answering trigger · cadence · cost · failure behaviour · phase, including the retry policy that `ErrorClass::Retryable` had no reader for (GAP-12). Three values are flagged as preference rather than constraint. **R-1** — confirmed to correctly have no gate; its mitigation is outcome-gated slices, and no test can assert focus. **Razi ruled the `/mcp` deletion stays in P0**, with the P0→P4 fleet tool-access gap accepted and stated. |
| 2026-08-31 | 1.1 | **`TESTING_GATES.md` complete — 115 gates — and its harvest raised a Rule-4 stop against this plan.** Moving the `/mcp` deletion to P0 (G-15) broke the ACP identity chain this same plan documents: `ToolContext.agent_role` is set in exactly one place, the adapter P0 deletes, so a decided-preserved capability would have been **silently dead from P0 through P3** with no gate to notice. `P3-13` is **pulled forward to `P0-13a`** and must ship in the same commit as `P0-13`; new smoke `S0-06`; `P3-13` struck in place rather than renumbered. A second, larger consequence of the same move is now **stated rather than discovered**: deleting the MCP server while the sidecar still runs leaves sidecar-hosted fleet workers with **no tool access from P0 until P4** — accepted with reasons, and the alternative (secure in P0, delete in P8) flagged for Razi. Also folded: 8 NEW gates covering M-40–M-43 and G-01/04/06/07/08 were added back into the specs' own gate tables, which had not been regenerated since rev 0.9 — **the same class of miss the review caught with Part 1B**. |
| 2026-08-31 | 1.0 | Created from the vision suite at rev 0.9 (51 ADRs, 0 open) after the adversarial pre-dev review. Ten phases, §0 rules register with the project's known-destructive operations named, current-state assessment with 17 numbered gaps, substrate audit, module tree tagged by creating phase, and full atomic task breakdowns for P0–P4. P5–P9 summarised, to be expanded at each phase start per §0.12. Three new risks carried from the review. **Status: awaiting GO.** |
