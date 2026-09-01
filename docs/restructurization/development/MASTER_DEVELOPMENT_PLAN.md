# AutoDev Restructure — Master Development Plan (MDP)

> **LIVING DOCUMENT.** Task status, evidence lines, the risk log and the change log are edited as the build proceeds. Design mechanics are **not** edited here — they live in the vision suite, and on conflict the vision governs mechanics while this document is corrected.

| | |
|---|---|
| **Parents** | `../vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md` (51 ADRs, 0 open) · specs `../vision/01`…`10` · `../vision/_grounding/G0`…`G12` · `../vision/_review/R1`…`R3` |
| **Anchors** | `../ui-anchor.md` (UI, DOCTRINE) · `../memory-anchor.md` (memory, adopted 2026-08-31) |
| **Owner** | Razi (Programming Lead, AIRS Foundry) |
| **Branch** | _to be created at GO_ |
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
> - **The `~/.airs/` → `~/.autodev/` consolidation** (ADR-01-09). Moves `sessions`/`messages` and the plugin/fleet-secret tree. **Back up both roots first.**
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

# §0.16 · Learnings register

Required by §0.2n. **Seeded from the planning phase**, because that phase produced real defects of mine and the next phase should read them before it starts. The lesson comes first; the instance second.

## Recurring classes — running count

**A defect seen once is an accident. The same shape three times is a property of how I work.** Every new instance increments a row, or opens a new one and says why it is new.

| # | Class | Count | Instances |
|---|---|---|---|
| **L-C1** | **A derived artifact was not regenerated after its source changed** | **4** | Part 1B stale since rev 0.3, so none of ADR-05-01…05-06 owned a mechanic · a **duplicate `ADR-02-06`** created by inserting into a ledger without re-reading it · the specs' gate tables never regenerated against the rev-0.9 mechanics · **spec `10 §6` still said the test frameworks and e2e harness land in P9 while the MDP had moved them to P0 and stated it was correcting that spec — the correction was never applied to the spec itself** (found by the §0.12 re-read at P0 start, before any code) |
| **L-C2** | **A property was asserted without the mechanism, or the comparison, that would establish it** | **3** | *"the anchor's reranker **is** the competitive model"* — 6 of 8 terms are disjoint · *"structurally impossible"* — the enum only relocates the dead-latch to `FromStr` · the cache-hit formula generalised from Anthropic to four providers |
| **L-C3** | **A source's conclusion was adopted without checking its load-bearing detail** | **4** | `port_status.rs` "should be KEPT" propagated into two documents; one import line disproved it · `G4b §A.14`'s two-part verdict adopted by half — the vault landed, the **redaction was dropped** · **P8 was written as "a `git rm` and a `.gitmodules` edit" from the presence of `.gitmodules` alone — the index has zero gitlinks, so `git rm` would have failed on first contact** (§1.1a) · **`G3d F-44` said `db/checkpoints.rs` is uncompiled (true) and concluded undo does not exist (FALSE) — I propagated that into my own gate code, two gate rows and two master-plan rows, and `reachability_gate.rs` ended up contradicting its own exception list in the same commit** (P1 pre-build) |
| **L-C4** | **A local change was made without re-checking the surrounding control flow** | **3** | Moving the `/mcp` deletion to P0 broke the ACP identity chain described four sections earlier in the same document · **setting `orch_healthy = false` to make Mode 2 unreachable, having read only the code BELOW that line — a latch twenty lines above returned early from every tick, so the fleet was silently dead on any machine without Python while my own comment two lines down claimed the opposite (`F-10`)** · **widening the `qc_review` gate to every caller without re-reading the shipped supervisor recipe, which calls `update_task_status(id, 'qc_review')` on a worker's behalf and never builds — so `request_review` failed every time (`F-22`)** |
| **L-C5** | **An owner's answer was reinterpreted rather than escalated** | **1** | ADR-02-02's option (B) was silently narrowed to "diagnoses only" instead of flagging that a literal reading re-creates BUG-COMPILE |
| **L-C6** | **A check was written from the CLAIM rather than from the INVARIANT it protects** | **3** | `TG0-04` first asserted `.env` must not **exist**, and failed correctly — a developer `.env` is expected in a working tree; the invariant is "never committed" · the vault gate first encoded "reads the vault ⇒ returns no `String`", which flags `send_message` (reads a key, returns a session id) and `test_provider_connection` (returns a status) — both correct · the `safe_filename` test asserted a fallback name for `..\..\evil.exe`, but WHATWG normalisation resolves it to the harmless `evil.exe` **(P0)** |
| **L-C7** | **A source-scanning assertion matched its own source** | **1** | The vault gate's banned-name list contains the banned names as string literals, so `include_str!` on its own file found them and the test failed on itself — the exact trap `V3` had warned about in `graph/extract.rs:711`, where `"#[test]"` as a literal makes grep-based counters overcount **(P0)** |
| **L-C8** | **A lesson already recorded in this register was not applied to the next opportunity** | **1** | §0.16's own "make subagents write incrementally — a rate limit destroyed four reports mid-flight" was written during planning, then **both P0 post-phase auditors were dispatched without it** and both were destroyed by a rate limit with nothing on disk **(P0)** |
| **L-C9** | **A verification whose SHAPE cannot express failure** | **3** | `S0-01` launched the app through `head -5`, so the startup crash after those lines was invisible and the smoke was recorded green · a build verified with `cargo test \| grep "^test result: ok. [1-9]"` — a pattern that can only match a SUCCESS line, so the failing target's `test result: FAILED.` was filtered out and a red build was pushed · the workflow "validated" by counting a string with `grep`, where the count itself was wrong **(P0)** |
| **L-C10** | **A failing check was read as worthless rather than as a finding** | **1** | The e2e job failed three times and I argued for deleting it as "noise from an unfinished capability". Its failures were **moving** each run — rejected, timed out, then carrying a crash reason — and the third was reporting a real startup crash that nothing else in CI could see, because nothing else executed the binary. I was one run from deleting the only honest signal **(P0)** |

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
| Shared | `src-shared/src/` | 1,073 LOC | Duplicates **four** tables by hand |
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

**Live schema head:** only `graph/` has a real migration ladder — `PRAGMA user_version`, 5 migrations, **head = v5, and v5 is a broken statement that must be repaired before v6**. Every other database uses `CREATE TABLE IF NOT EXISTS` on open. **11 databases across 3 data roots.**

## 1.2 What does NOT exist — `GAP`

| # | Gap | Evidence |
|---|---|---|
| GAP-01 | Any MCP **client** — `rmcp` is compiled server-features-only and cannot dial out | `G12` |
| GAP-02 | Any **xAI/Grok** provider | `G2b P-28` |
| GAP-03 | Any **prompt caching** on any provider | `G2a T1` |
| GAP-04 | Any **embeddings or vector search** — zero hits repo-wide | `G1 §C.4 P3` |
| GAP-05 | Any **frontend test framework** — no vitest, jest, playwright, no `test` script | `G0 §2` |
| GAP-06 | Any **ESLint** — `npm run lint` is `tsc --noEmit` | `G0 §2` |
| GAP-07 | Any **macOS CI job** | `G0 §2` |
| GAP-08 | Any **Rust-native compile/verify step** — all validators shell out | `G3d` |
| GAP-09 | Any **run/launch/handoff** for produced artifacts | `G3d F-DEL` |
| GAP-10 | Any **dynamic tool registration** — `ToolRegistry` is frozen after construction | `G12` |
| GAP-11 | Any **backup path** before destructive operations | `G1 §E.3` |
| GAP-12 | Any **retry policy** — `ErrorClass::Retryable` has no reader | `R3` |
| GAP-13 | Any **cadence** — `interval`/`backoff`/`max_retries` appear zero times in 13 documents | `R3` |
| GAP-14 | Any **spend ceiling or kill switch** | `R3` |
| GAP-15 | Working **undo** — `db/checkpoints.rs` is not compiled into the binary | `G3d F-44` |
| GAP-16 | Any **redaction** — the module grounding ruled PORT was dropped | `R3`, M-40 |
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

// HTTP — the only client, used for all four providers
reqwest = { version = "0.12", features = ["json", "stream"] }

// Git, graph, watchers
git2 = "0.19"; tree-sitter = "0.22"; petgraph = "0.6"; ignore; notify = "6"

// Release profile — CHANGES IN P3 (M-41)
[profile.release] panic = "abort"    // Cargo.toml:126-131  → becomes "unwind"
```

**Tauri 2.11** with **one** plugin (`shell`). `tauri.conf.json` declares `dialog` and `fs` plugin blocks for plugins that are **not in `Cargo.toml` and not registered** — dead config, removed in P0.

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
  provider/{traits,anthropic,openai,google,xai,streaming}.rs      [P1 move, P2 rework]
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

| # | Phase | Headline outcome | Depends on |
|---|---|---|---|
| **P0** | Stop the bleeding | No unverified update can install · no command returns a key · the unauthenticated `/mcp` RCE is gone · **gates can fail** · all three platforms build on PR | — |
| **P1** | Structural | Everything shipped is in the binary · one schema owner · `autodev-core` exists · redaction restored | P0 |
| **P2** | Providers | Four providers conform · caching measurably works · cost is honest · **spend has a ceiling** | P1 |
| **P3** | Runtime + memory | Context is bounded · one tool path with a real gate · subagents in-process · a panic cannot kill the app · memory works and is proven read | P2 |
| **P4** | Fleet | A task runs end to end on three OSes and lands a **build-verified, reversible** artifact the user can run | P3 |
| **P5** | Graph | The graph is compiled, correct across updates, and **read** for context scoping | P3 |
| **P6** | UI | The anchor is implemented · no event is lost on a tab switch · every setting persists · every surface reachable | P2 |
| **P7** | Experience | **A new user reaches a working agent turn without documentation** | P6 |
| **P8** | Submodule deletion | Not one line of sidecar code survives, by content **or by address** | P4 |
| **P9** | Delivery | Signed, verified installers for Windows, Linux and macOS | P8 |

## Sequencing rationale

- **P0 is security and gate-honesty first.** An unauthenticated local RCE (`/mcp`) and an updater that installs unverified binaries do not wait eight slices — ADR-03-02's own words are *"delete it first now"*. And until gates can fail, nothing later can be *proven*.
- **macOS CI and the end-to-end harness land in P0, not P9.** R-4 identifies "macOS verified last" as release-blocking, and P4's exit gate needs the harness to exist. *(This corrects an ordering the review caught — spec `10` had put both in P9.)*
- **All migrations land in the earliest persistence phase as one linear unit** (P1), so no later phase renumbers. The `graph/` v5 repair is part of that unit.
- **`autodev-core` is extracted in P1, before the runtime work in P3**, so the rework lands in its final home rather than being moved afterwards. *(The review found no slice scheduled this at all.)*
- **The run/audit ledger is built early** (P1) so every later act can stamp a run id.
- **P6 depends only on P2**, not P3 — the UI needs providers and settings, not the fleet — so it can proceed in parallel.
- **P8 cannot start before P4 finishes.** The sidecar is the reference oracle while porting, and `subagent/bridge.rs`'s SSE parsers are shared with `sidecar/client.rs`.
- **The capstone is last.** P9's product proof is the headline acceptance test.

---

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
- [ ] **P1-01** Declare `graph_tools` and `graph_prompts` in `agent/builtin_tools/mod.rs` (918 LOC never compiled). → `CODE-PRESENT`
- [ ] **P1-02** Declare `db/checkpoints.rs` — **the undo feature is currently absent from the shipped app**. → `CODE-PRESENT`
- [ ] **P1-03** Fix every compilation error the two above surface. → `CODE-PRESENT`
- [ ] **P1-04** P0-22's reachability guard now **passes**. → `GATE-PROVEN`

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
- [ ] **P1-10** Give **every** database `PRAGMA user_version` and a numbered ladder. → `LIVE-DB-PROVEN`
- [ ] **P1-11** **Repair `graph/` migration v5** — `ADD COLUMN IF NOT EXISTS` is not valid SQLite. **⚠ Back up `~/.autodev/graphs/*.db` first (§0.8).** → `LIVE-DB-PROVEN`
- [ ] **P1-12** `journal_mode=WAL` and a `busy_timeout` on every connection. → `LIVE-DB-PROVEN`
- [ ] **P1-13** One timestamp helper; replace every ad-hoc `parse_from_rfc3339` on a SQLite `datetime('now')` column (3+ known sites). → `UNIT-PROVEN`
- [ ] **P1-14** Consolidate to **one data root** (`~/.autodev/`) with a one-time migration from `~/.airs/`. **⚠ Back up both roots first.** → `LIVE-DB-PROVEN`
- [ ] **P1-15** One owner for `app_settings` (currently three independent creators). → `CODE-PRESENT`
- [ ] **P1-16** Collapse the two checkpoint systems to one. → `LIVE-DB-PROVEN`
- [ ] **P1-17** Delete or create `hourly_costs` (queried, never created). → `CODE-PRESENT`
- [ ] **P1-18** Add the **run/audit ledger** so every later act stamps a run id. → `LIVE-DB-PROVEN`
- [ ] **P1-19** Add a **backup helper** and wire it into every destructive path (GAP-11). → `UNIT-PROVEN`

### P1-D · Redaction — the dropped port (M-40)
- [ ] **P1-20** Port `redact_dict` + the 20-entry `SENSITIVE_KEYS` set into `autodev-core/security/`. → `UNIT-PROVEN`
- [ ] **P1-21** Apply at every boundary: prompts, logs, memory writes, analytics, egress. → `GATE-PROVEN`
- [ ] **P1-22** Add the P8 gate: **the deletion phase cannot complete while a capability grounding marked PORT remains unported.** → `DOC-GATE`

### Smoke — S1
- [ ] **S1-01** `cargo build` clean across all four crates.
- [ ] **S1-02** `airsctl --version` runs and links `autodev-core`.
- [ ] **S1-03** Every DB opens, reports its `user_version`, and survives a restart.
- [ ] **S1-04** A file containing `AWS_SECRET_ACCESS_KEY=…` read by a tool appears **redacted** in the prompt and in memory.

**Exit.** S1 → `TG-P1` → matrix → progress → no-stale → commit.

**Narrative success criterion.** *A newcomer can run one command and see every source file is compiled, every database carries a version, and a secret read from disk does not survive into a prompt or the memory store. The agent still behaves as before — but the substrate is now honest.*

---

# §8 · Phase P2 — Providers

**Goal.** Four conforming providers, caching that measurably works, honest cost, and a spend ceiling that stops.
**Depends on.** P1.
**Spec anchors.** `02` (all), ADR-01-04, ADR-01-05, ADR-02-05, ADR-03-01, ADR-03-05, G-04, G-06, G-10.

### P2-A · The trait
- [ ] **P2-01** `ModelProvider` trait v2 with `Item::Opaque` and six-field `Usage` **from day one** (retrofitting means touching every adapter twice). → `UNIT-PROVEN`
- [ ] **P2-02** `CachePolicy` with four variants — Anthropic explicit, xAI automatic, others differ. → `UNIT-PROVEN`
- [ ] **P2-03** `capabilities()` keyed by **model**, not provider. → `UNIT-PROVEN`
- [ ] **P2-04** Anthropic adapter. → `GATE-PROVEN`
- [ ] **P2-05** OpenAI adapter (`async-openai` tactically). → `GATE-PROVEN`
- [ ] **P2-06** Google adapter — **fix the discarded tool arguments** (`google.rs:313` `let _args = ...`). → `GATE-PROVEN`
- [ ] **P2-07** **xAI/Grok adapter — new** (GAP-02); OpenAI-wire-compatible with a base-URL override. → `GATE-PROVEN`
- [ ] **P2-08** Honour `max_tokens` on OpenAI and Google (silently dropped today). → `UNIT-PROVEN`
- [ ] **P2-09** Parallel tool calls on all four; Anthropic needs **all** results in one user message. → `GATE-PROVEN`
- [ ] **P2-10** `StopReason::MaxTokens` distinguishable on all four. → `GATE-PROVEN`
- [ ] **P2-11** Delete `ollama.rs` (506 LOC), 10 sites in 8 files, drop `ollama-rs`, and the 8 frontend doc references. → `CODE-PRESENT`

### P2-B · Caching — alarm before cache
- [ ] **P2-12** **Build the cache-hit alarm first.** Non-zero `cached_read` from request 2 is the invariant; zero means a silent invalidator. → `GATE-PROVEN`
- [ ] **P2-13** One prefix builder, ordered maps (`BTreeMap`/`preserve_order`) — Anthropic needs a 100% byte match. → `UNIT-PROVEN`
- [ ] **P2-14** Remove volatile content from the system prompt (it is not byte-stable today). → `UNIT-PROVEN`
- [ ] **P2-15** Per-model cache minimums in `capabilities()`; warn when the prefix falls short (**no provider errors below the minimum**). → `UNIT-PROVEN`
- [ ] **P2-16** Wire caching on all four. → `BENCH-PROVEN`

### P2-C · Cost and spend
- [ ] **P2-17** Handle `message_start` in streaming — **input tokens are 0 for all three cloud providers today**. → `UNIT-PROVEN`
- [ ] **P2-18** Per-model price table with **no silent fallback** — an unknown model is an explicit error. *(The `_ => (1.0, 3.0)` fallback catches 7 of 11 models.)* → `UNIT-PROVEN`
- [ ] **P2-19** Provider-neutral usage normalisation — **Anthropic excludes cached tokens from `input`; OpenAI and Gemini include them.** → `UNIT-PROVEN`
- [ ] **P2-20** Handle the 200k price cliffs (Google Pro, all xAI models). → `UNIT-PROVEN`
- [ ] **P2-21** **M-42 — daily spend ceiling in the provider layer**, the one place every call passes. → `GATE-PROVEN`
- [ ] **P2-22** **M-42 — global kill switch**; exceeding the cap **pauses the fleet and surfaces it**. → `SMOKE-PROVEN`

### P2-D · BYOK
- [ ] **P2-23** Key entry → **live validation** (models-list or token-count call) → keychain. → `SMOKE-PROVEN`
- [ ] **P2-24** **Encrypted-file fallback on Linux** where no Secret Service exists; tell the user which store is in use (G-06). → `PLATFORM-TRIPLE`
- [ ] **P2-25** Per-provider keys, independently configurable; rotation and removal. → `UNIT-PROVEN`
- [ ] **P2-26** Persist the active provider/model and **hydrate on startup** (the choice dies on restart today). → `SMOKE-PROVEN`
- [ ] **P2-27** Wire `ModelConfigsDb` — temperature, top_p, reasoning effort, thinking are **written by the UI and read by nothing**. → `UNIT-PROVEN`
- [ ] **P2-28** Create the dev `.env` + integration-test harness that reads it. **The harness is never linked into the app.** → `UNIT-PROVEN`

### Smoke — S2
- [ ] **S2-01** A turn completes on each of the four providers.
- [ ] **S2-02** A 5-turn session shows non-zero `cached_read` from turn 2.
- [ ] **S2-03** Session cost matches the provider's own reported usage.
- [ ] **S2-04** With the keychain empty and every provider env var set, the app **refuses to run a turn**.
- [ ] **S2-05** Exceeding the daily cap pauses the fleet with a visible message.

**Exit.** S2 → `TG-P2` → matrix → progress → no-stale → commit.

**Narrative success criterion.** *A newcomer can enter a key for any of the four providers, watch it validate, send five messages, and see in the cost panel that turns 2–5 cost a fraction of turn 1 — the first observable proof that caching works. And they can set a daily cap and watch it actually stop.*

---

# §9 · Phase P3 — Runtime and memory

**Goal.** Bounded context, one tool path with a real permission gate, in-process subagents, a panic that cannot kill the app, and memory that is provably read.
**Depends on.** P2.
**Spec anchors.** `01`, `03`, `06`; ADR-01-06/07/08, ADR-02-03, ADR-05-01…05-05, G-01, G-03, G-07, G-11, G-12, G-13.

### P3-A · Fault isolation (M-41)
- [ ] **P3-01** `[profile.release] panic = "unwind"`. → `CODE-PRESENT`
- [ ] **P3-02** Every agent turn and subagent runs inside `catch_unwind`. → `UNIT-PROVEN`
- [ ] **P3-03** A panicking worker fails **that turn**, reports it, and the app survives. → `GATE-PROVEN`
- [ ] **P3-04** Fix the three identical byte-slice truncation panics (`bash.rs:134`, `remember_this.rs:94`, `annotate_file_tool.rs:99`) — **the correct implementation already exists at `validate.rs:846-853`.** → `UNIT-PROVEN`

### P3-B · The loop
- [ ] **P3-05** **Every** exit path emits exactly one terminal event — via a `Drop` guard, since four paths return silently today. → `GATE-PROVEN`
- [ ] **P3-06** Persist the **whole** `Vec<Item>` with usage, not `history.last()`. → `UNIT-PROVEN`
- [ ] **P3-07** Never append an assistant message with empty content (it poisons every later send in that session). → `UNIT-PROVEN`
- [ ] **P3-08** Thread a `CancellationToken` into the provider call, the tool executor, and every pending-permission wait; **every one has a timeout**. → `GATE-PROVEN`
- [ ] **P3-09** Define the **retry policy** `ErrorClass::Retryable` currently lacks (GAP-12), with backoff and a bound. → `UNIT-PROVEN`

### P3-C · Tools and permissions
- [ ] **P3-10** Collapse to **one** execution path; `ToolExecutor::execute()` has zero callers today. → `CODE-PRESENT`
- [ ] **P3-11** Introduce `ToolEffect` (`Read`/`Write`/`Execute`/`Network`/`Board`) as the **only** prompting key; `is_read_only` reverts to a concurrency hint (G-11). → `GATE-PROVEN`
- [ ] **P3-12** Honour `PermissionMode` — `ManualApproval` is passed unconditionally today, so the PA-mode dropdown does nothing. → `SMOKE-PROVEN`
- [x] ~~**P3-13** Populate `ToolContext` truthfully~~ — **MOVED to `P0-13a`** (2026-08-31). It must ship in the same commit as the `/mcp` deletion, or ACP identity is dead from P0 to P3. Task ID retained and struck rather than renumbered, per §0.14.
- [ ] **P3-14** Route **every** path-taking tool through `path_jail` — six bypass it today, two of which act on the path. → `GATE-PROVEN`
- [ ] **P3-15** `path_jail` **refuses** an unnormalisable path, never rewrites it (two steps corrupt silently today). → `UNIT-PROVEN`
- [ ] **P3-16** Normalise the `path_lock` key — canonicalise and case-fold. **Use the same shared helper as the graph DB key and checkpoint retention: three subsystems, one defect.** → `UNIT-PROVEN`
- [ ] **P3-17** No lock held across an unbounded await; every subprocess gets a timeout (`validate.rs` has **zero**). → `GATE-PROVEN`
- [ ] **P3-18** A missing validator binary is an **error**, not a warning silently reported as "✓ No syntax errors". → `UNIT-PROVEN`
- [ ] **P3-19** Never resolve an executable from the workspace under inspection (`resolve_python` runs `<workspace>/.venv/…`). → `GATE-PROVEN`
- [ ] **P3-20** Rewrite the **33 defective tool descriptions**. → `DOC-GATE`
- [ ] **P3-21** Fix `e2e_probe` — arbitrary shell with jail, evidence and output capture all absent. → `GATE-PROVEN`

### P3-D · Subagents
- [ ] **P3-22** In-process subagent runner on tokio tasks; **permit released by `Drop`** (the leak deadlocks all spawning after 16 today). → `GATE-PROVEN`
- [ ] **P3-23** `depth + 1` from the parent's real context, with a hard cap. → `UNIT-PROVEN`
- [ ] **P3-24** Timeout and propagated cancellation. → `UNIT-PROVEN`

### P3-E · Context strategy
- [ ] **P3-25** Layered assembly: cached prefix → live history → `clear_tool_uses` → compaction. → `BENCH-PROVEN`
- [ ] **P3-26** Compaction trigger reads `ModelInfo::context_window`, not a hardcoded 200,000. → `UNIT-PROVEN`
- [ ] **P3-27** **Use** the compacted history — the caller discards it today. → `UNIT-PROVEN`
- [ ] **P3-28** Tune `clear_at_least` so a clear exceeds the cache-write it forces. → `BENCH-PROVEN`
- [ ] **P3-29** Bound every tool output; grep bounds per line; `list_directory` actually bounds entries. → `UNIT-PROVEN`

### P3-F · Memory
- [ ] **P3-30** SQLite + `sqlite-vec` + FTS5 behind a retrieval-backend trait. → `LIVE-DB-PROVEN`
- [ ] **P3-31** **Scope as a declared `vec0` partition key** — not a post-filter over the top-k, which would return few or zero results (G-12). → `GATE-PROVEN`
- [ ] **P3-32** `fastembed-rs` / BGE-small-en-v1.5, **384 dimensions**. → `UNIT-PROVEN`
- [ ] **P3-33** **Pin and vendor the ONNX runtime** — `ort`'s default features download it from a third-party CDN at build time (G-14). → `CODE-PRESENT`
- [ ] **P3-34** Set the fastembed cache path **explicitly under the app data root** — it defaults to CWD-relative, an INV-5 violation (G-14). → `UNIT-PROVEN`
- [ ] **P3-35** Model download is visible, resumable and **skippable**; skipping degrades to lexical. → `SMOKE-PROVEN`
- [ ] **P3-36** Retrieval degrades to FTS5/BM25 when the model is absent — **never empty, never an error**. → `GATE-PROVEN`
- [ ] **P3-37** Embedding is async and off the runtime (`spawn_blocking`); it never delays a turn. → `BENCH-PROVEN`
- [ ] **P3-38** Provenance column (`source` + file/line). → `LIVE-DB-PROVEN`
- [ ] **P3-39** The **gateway**: agents get `search`/`get`/`list`/`propose`; **`remember_this` is removed**. → `GATE-PROVEN`
- [ ] **P3-40** Gateway evaluation — importance, novelty, duplication, confidence — as `DET` policy (G-03). → `UNIT-PROVEN`
- [ ] **P3-41** Per-tier budgeting (simple), interface-compatible with the anchor's reranker. → `UNIT-PROVEN`
- [ ] **P3-42** **The anchor §10 reranker as its own work item with its own columns** — it is **not** the ported competitive model; 6 of 8 terms are disjoint (G-13). → `UNIT-PROVEN`
- [ ] **P3-43** Fix the seven inherited defects rather than porting them (query-independent semantic recall · query-ignoring cache key · always-throwing HITL writer · unfiltered procedural recall · unscoped episodic recall · inert negative cache · uninvalidated semantic cache). → `UNIT-PROVEN`
- [ ] **P3-44** Memory is **provably read** — a recalled memory appears in the outbound prompt bytes (INV-8). → `GATE-PROVEN`

### Smoke — S3
- [ ] **S3-01** A 30-turn session's input tokens do **not** grow linearly with turn count.
- [ ] **S3-02** A deliberately panicking tool fails its turn; the app and other sessions survive.
- [ ] **S3-03** A subagent spawns, streams, and returns with the sidecar **not running**.
- [ ] **S3-04** Two different questions in one session produce **different** recall sets.
- [ ] **S3-05** With the embedding model deleted, recall still returns results and reports itself degraded.

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

# §11 · Phases P5–P9 — summary

Full task breakdowns are written at the start of each phase (§0.12 requires re-reading the specs at that point, and writing 400 atomic tasks now would guarantee staleness). Each carries the same structure: lettered groups, stable IDs, smoke checklist, exit line, narrative criterion.

| Phase | Groups | Headline tasks | Spec |
|---|---|---|---|
| **P5 Graph** | A canonical path · B non-destructive upsert · C incremental correctness · D graph-scoped context | One normalized DB path (four spellings → four DBs today) · stop `INSERT OR REPLACE` + cascade deleting edges graph-wide · re-cluster on incremental update · **feed context selection** (M-18) | `05` |
| **P6 UI** | A anchor · B event lifetime · C contract · D settings · E Graph City · F reachability | 7px/fonts/tokens/light-only · session-lifetime subscription above the tab boundary · reconcile 2 missing + 47 dead commands + 10 dead listeners + 3 dead events · persist every setting · **self-host the font** (the CSP block is why the city hangs) · route or delete ~3,700 orphaned LOC | `07` |
| **P7 Experience** | A first-run · B errors · C progress · D destructive · E coherence | The BYOK flow · **fix the two toast producers** (a two-call-site import fix, not a system to build) · honest progress + cancellation · confirm the 10 unconfirmed destructive actions · one vocabulary | `07 §7` |
| **P8 Deletion** | A verify entry gate · B excise Rust · C **filesystem + config removal** · D relocate · E prune deps | **Entry gate: all ten ported capabilities pass with the sidecar *absent*, not merely stopped** · remove `sidecar/` (3,161 LOC) + 29 commands + `resources/sidecars` (225 MB) · **see §1.1a — this is not a `git submodule` operation** · verify **no sidecar code survives by content or by address** | `09 §4` |

### P8-C · Submodule removal — the correct procedure for the state we are actually in

Grounded in §1.1a. **The standard `git submodule deinit` → `git rm` sequence will fail here**, because there are no gitlinks in the index or in history to operate on.

1. **Back up first** (§0.8). `submodules/` is 225 MB of untracked working tree with no remote copy of any local modification.
2. Delete both stanzas from **`.gitmodules`** — this is the only *tracked* artifact, so it is a normal file edit and a normal commit.
3. Delete **`.git/modules/submodules/`** — both object stores. ⚠️ **Skipping this is the "survives by address" failure**: the working tree goes and the repository still carries the sidecar's git objects.
4. Delete the **`submodules/`** working tree.
5. Delete **`src-tauri/resources/sidecars/`** and its `tauri.conf.json` `resources` entry — the 225 MB bundle resource.
6. **No history rewrite.** Verified: the content was never committed to this repository.
7. Assert the exit condition: no `.py` in the tree, no `.py` in any built installer, and `git ls-files | grep -i sidecar` returns nothing.
| **P9 Delivery** | A signing · B installers · C updater · D capstone | Apple + Windows certificates (**procurement lead time — start at GO**) · working install on all three · the headline acceptance test | `09` |

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

# §16 · Change log

*Append-only, newest first.*

| Date | Rev | Change |
|---|---|---|
| 2026-09-01 | 1.4 | **P0 built and closed out — 8 commits, `5fca2bb`…`4b17c6f`.** 26 of 28 original tasks done, plus `P0-13b` and six added tasks (`P0-28`…`P0-33`). **`P0-26` was REVERSED** — deleting the `dialog`/`fs` plugin config would have erased the only evidence of a live runtime break (the Logs export fails on every click); superseded by `P0-31`, which makes it work. **`P0-21` did not reach its evidence class**: the e2e harness exists and is wired into CI but has never executed an assertion (`msedgedriver` absent locally; `tauri-driver` has no WKWebView backend, so macOS can never run it). Readiness **0% → 2.7%**: Build ✅, Smoke and Gates ☐ under the no-partial-credit rule. Overlay: SO-4 and SO-11 → GREEN, SO-2 and SO-12 → AMBER; **SO-6 stays RED deliberately** — `platform/command.rs` is proven but has no caller, and a guard nothing calls is not a guard. Eight runtime ADRs logged (`0001`…`0008`), three of them `razi-ruled` or `flagged-and-proposed`. §0.16 gained **L-C6/L-C7/L-C8** and lessons **L-7…L-10**, and §0.2i now carries the incremental-write dispatch clause that L-C8 exists because I forgot. |
| 2026-08-31 | 1.3 | **Razi's development rules from a sibling audit adapted into §0.** Added: **§0.2a** two mandatory post-phase auditors (work + area), with the area auditor's brief tuned to this codebase's measured defect profile — `let _ =`, dead parameters, guards that cannot fire · **§0.2b** a phase cannot close over an open finding · **§0.2c** every finding runs through Razi · **§0.2d** never stop midway, ask in-flight via `AskUserQuestion` · **§0.2e** every task names its proof · **§0.2i** two subagents is the floor · **§0.2j** search the web before escalating; escalate decisions, not facts · **§0.2m** adversarially verify each phase **before** building it · **§0.2n** every phase writes its learnings · **§0.2L-1…5** method rules. **§0.16 learnings register created and seeded from the planning phase** — five recurring classes with running counts, six lessons, and the method that worked. Two adaptations rather than literal adoptions, both stated: §0.2e allows gates to be **shared** across tasks (a 1:1 rule would manufacture coverage — the eight P0-A updater tasks prove one thing); and every example was **replaced with this repo's own evidence**, including the six errors this plan's own pre-GO review found in it. |
| 2026-08-31 | 1.2 | **The three remaining open items closed, none of which needed an owner decision.** **R-8** — its two operational clauses become ordered tasks rather than prose: `P4-25` is explicitly blocked on `P4-07`, and new task `P4-24a` decides the fate of existing `routing_history` under §0.8 backup discipline. The gate harvest was right that a sequencing constraint is a task-dependency property, not a runtime assertion, so it is deliberately **not** a gate. **R-13** — the **operational cadence register is written (§14.5)**: thirteen mechanisms, each answering trigger · cadence · cost · failure behaviour · phase, including the retry policy that `ErrorClass::Retryable` had no reader for (GAP-12). Three values are flagged as preference rather than constraint. **R-1** — confirmed to correctly have no gate; its mitigation is outcome-gated slices, and no test can assert focus. **Razi ruled the `/mcp` deletion stays in P0**, with the P0→P4 fleet tool-access gap accepted and stated. |
| 2026-08-31 | 1.1 | **`TESTING_GATES.md` complete — 115 gates — and its harvest raised a Rule-4 stop against this plan.** Moving the `/mcp` deletion to P0 (G-15) broke the ACP identity chain this same plan documents: `ToolContext.agent_role` is set in exactly one place, the adapter P0 deletes, so a decided-preserved capability would have been **silently dead from P0 through P3** with no gate to notice. `P3-13` is **pulled forward to `P0-13a`** and must ship in the same commit as `P0-13`; new smoke `S0-06`; `P3-13` struck in place rather than renumbered. A second, larger consequence of the same move is now **stated rather than discovered**: deleting the MCP server while the sidecar still runs leaves sidecar-hosted fleet workers with **no tool access from P0 until P4** — accepted with reasons, and the alternative (secure in P0, delete in P8) flagged for Razi. Also folded: 8 NEW gates covering M-40–M-43 and G-01/04/06/07/08 were added back into the specs' own gate tables, which had not been regenerated since rev 0.9 — **the same class of miss the review caught with Part 1B**. |
| 2026-08-31 | 1.0 | Created from the vision suite at rev 0.9 (51 ADRs, 0 open) after the adversarial pre-dev review. Ten phases, §0 rules register with the project's known-destructive operations named, current-state assessment with 17 numbered gaps, substrate audit, module tree tagged by creating phase, and full atomic task breakdowns for P0–P4. P5–P9 summarised, to be expanded at each phase start per §0.12. Three new risks carried from the review. **Status: awaiting GO.** |
