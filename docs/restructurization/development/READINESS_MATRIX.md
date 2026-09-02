# AutoDev Restructure — Readiness Matrix

> **LIVING DOCUMENT, append-only history.** Updated at every phase boundary. Never edit a past update-log entry.

| | |
|---|---|
| **Parents** | `MASTER_DEVELOPMENT_PLAN.md` · `TESTING_GATES.md` · `../vision/00-…-MASTER-PLAN.md` |
| **Rev** | 1.0 — 2026-08-31 |
| **Overall readiness** | **8.0%** — P0 + P1 built and committed; P1 smoke green, P1 gates one clause short. Cap active. |

---

# §0 · Scoring mechanics

**One scored area per phase.** Weights sum to 100, with the headline capability weighted heaviest.

Each area has **three binary rows**:

| Row | Means |
|---|---|
| **Build** | Every task in the phase is checked, each with a cited task ID and evidence class |
| **Smoke** | Every `S<p>-<nn>` in the phase passes |
| **Gates** | Every `TG-P<p>` gate assigned to the phase passes |

**Rules, without exception:**
- ✅ only with a **cited ID and an evidence class**. No partial credit. No "should work".
- Area % = rows checked ÷ 3.
- Overall = Σ (weight × area %).
- A gate **skipped** in CI counts as **failed** (§0.5 of the MDP).
- **A phase is never scored above its weakest row.** Build ✅ with Gates ❌ is 33%, not "nearly done".

## The safety overlay

Cross-cutting invariants, scored **independently of the phase areas**.

> **Any RED row caps overall readiness at 50%, regardless of the arithmetic.**

**Overlay rows are scored from "is the system actually guarded", not from gate colour.** A green gate on a guard that nothing calls is **AMBER**, and the reason is written in the row. This project has already produced four such guards — `Tool::requires_permission()` never called, the Fix-8c build sentinel unreachable by construction, the subagent recursion guard inert, and the `qc_review` evidence gate skipped on the native path. A matrix that scored those green would have been lying.

---

# §1 · Executive snapshot

| Dial | Status | Why |
|---|---|---|
| **Overall readiness** | 🔴 **8.0%** | P0 and P1 complete and committed. P1: Build ✅, Smoke ✅ (`S1-01…03` proven, `S1-04` end to end). Gates ☐ — `TG1-01`'s registration clause is unmet and is not claimed. No partial credit. |
| Security posture | 🟡 AMBER | All three P0 holes closed, and **P2 closed a live credential leak found in its own pre-build audit**: Google authenticated with `?key=` in the URL, and `reqwest`'s error Display includes the request URL — which was logged *and* returned as `ProviderStatus::Error` across the IPC boundary into Settings. Redaction did not save it, because `SENSITIVE_KEYS` carried `api_key` but not bare `key`. Now a header, gate-proven. `TG-P2-07` additionally proves no shipped code reads a provider key from argv or the environment. **Still AMBER:** the updater has no production key yet (so it refuses everything), and the Linux encrypted-file vault fallback has no CI job exercising a D-Bus-less runner — it is written and unit-tested, not platform-proven. |
| Gate honesty | 🟢 GREEN | Every `continue-on-error` gone; all checks blocking. **CI is now green end to end** (run `33467890406`) — the first passing run in this repo's recent history; the three before P0 all failed on a build that was structurally incapable of passing. |
| Cross-platform | 🔴 RED | All three build smokes now pass on PRs, and the app is verified to **start** on Windows and Linux (macOS builds but is never launched). Still RED: `platform/command.rs` exists but **nothing calls it** — `bash.rs` still rewrites pipes into PowerShell, so no fleet task can complete on Windows. Wiring is P4. |
| Agent correctness | 🟡 AMBER | **P2 closed the provider half, and proved it live.** Caching works — Anthropic wrote 11,923 tokens and read back 11,923; OpenAI went 8,659 input to 211 + 8,448 cached — and the alarm that can detect it *not* working shipped first. Context is no longer unbounded at a hardcoded 200k: compaction reads the model's real window, which had been compacting Gemini at 16% of capacity. Truncation is now detectable on all three (it was Anthropic-only). **Still AMBER, not GREEN: the permission gate is not in force — that is P3** — and `panic = "abort"` means one panic still ends every in-process agent. |
| Product capability | 🔴 RED | Agent output lands where the user never looks; no run step exists. P4. |
| Data integrity | 🟡 AMBER | **P1 closed the structural half.** All 10 databases carry a `user_version` ladder (was 1); the ladder refuses a database from a newer build, applies each rung and its stamp in one transaction, and **backs up automatically before any destructive rung**. `LIVE-DB-PROVEN` on the real data root: zero row-count changes across 720 rows. AMBER not GREEN — nothing yet restores *from* a backup automatically, and `M-43` (a failed checkpoint still lets the write proceed) is open. |
| Fault tolerance | 🔴 RED | `panic = "abort"` + in-process agents: one panic kills everything. P3. |

---

# §2 · Scored areas

| # | Area (phase) | Weight | Build | Smoke | Gates | Area % | Weighted |
|---|---|---|---|---|---|---|---|
| A0 | **P0 · Stop the bleeding** | 8 | ✅ | ☐ | ☐ | 33% | 2.7 |
| A1 | **P1 · Structural** | 8 | ✅ | ✅ | ☐ | 67% | 5.3 |
| A2 | **P2 · Providers** | 14 | ✅ | ✅ | ✅ | 100% | 14.0 |
| A3 | **P3 · Runtime + memory** | 18 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A4 | **P4 · The Fleet** ← headline | **20** | ☐ | ☐ | ☐ | 0% | 0.0 |
| A5 | **P5 · Graph** | 6 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A6 | **P6 · UI** | 10 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A7 | **P7 · Experience** | 8 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A8 | **P8 · Submodule deletion** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A9 | **P9 · Delivery** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |
| | **Total** | **100** | | | | | **22.0** |

**Why P4 carries the most weight.** It is the only phase whose completion means AutoDev does what it claims: a task runs end to end on three operating systems and produces a build-verified, reversible artifact the user can run. Everything before it is a precondition; everything after it is polish, cleanup or distribution.

---

# §3 · The safety overlay

| # | Invariant | Proving gates | Status | Note |
|---|---|---|---|---|
| **SO-1** | **INV-1** — no agent work in a foreign runtime or language | `G-NOSIDECAR`, P8 entry gate | 🔴 RED | Subagents moved in-process onto a nested `AgentEngine` and Mode 2 is inert, so no *agent turn* runs in Python. **P1 added the structural half:** `autodev-core` holds the runtime, both binaries link it, and `cargo tree` reports **0** Tauri/GTK/webkit edges for core and `airsctl` against 28 for the desktop crate. `src-cli/tests/core_runtime.rs` constructs an `AgentEngine` in a process with no UI framework — a file that could not have compiled before. INV-1 restated as runtime-and-language-scoped (`P1-08`). Still RED: `main.rs` continues to spawn `memory-mcp-server` and `core-orchestrator`. P8. |
| **SO-2** | **INV-2** — no key crosses the IPC boundary outbound | `G-KEY-ASYM` | 🟡 AMBER | `get_provider_key`/`get_github_pat` deleted; the vault command set is closed and gate-proven (`TG0-03` static half). AMBER not GREEN: the runtime half — a key entered in the UI cannot be read back by *any* route — has no test, and needs the e2e harness to execute. |
| **SO-3** | **INV-3** — no shipped artifact contains a credential | `G-BUNDLE` | 🟡 AMBER | ⚠ **Note was stale and is corrected.** The 225 MB Python tree is **no longer a declared bundle resource** — `bundle.resources` is `[]`, fixed in P0, and a gate asserts every declared bundle path exists **and is git-tracked**. Zero files under `resources/` are tracked. AMBER not GREEN: proving a *built installer* carries no credential needs P9's packaging gate, and the tree still exists on disk locally — its deletion is P8. |
| **SO-4** | **INV-4** — a gate that cannot fail is not a gate | `G-CI-BLOCKING` | 🟢 GREEN | All four `continue-on-error` flags removed; `cargo test` runs `--all-targets` (it ran `--lib`, silently skipping `tests/`). `TG0-02` asserts this over the workflow files; `S0-05` proves a failing test exits non-zero in both languages. |
| **SO-5** | **INV-5** — no artifact written against process CWD | `G-DELIVER` | 🔴 RED | Live artifacts observed in `~/.autodev/agents/` |
| **SO-6** | **INV-6** — platform-neutral agent commands | `G-NO-POSIX`, `G-CMD-INTENT` | 🔴 RED | `platform/command.rs` now exists and is unit-proven (10 tests, including the Windows `.cmd` shim). It changes nothing yet: **no caller uses it** — `bash.rs` still rewrites pipes into PowerShell under `cmd /C`. A guard nothing calls is not a guard. Wiring is P4-11. |
| **SO-7** | **INV-7** — measurements are provider-reported, not estimated | `G-USAGE`, `G-PRICE` | 🔴 RED | Streaming input tokens are 0 on three providers |
| **SO-8** | **INV-8** — a writer must have a proven reader | `G-MEM-READ`, `G-GRAPH-SCOPE`, reader-exists family | 🔴 RED | Graph, `ModelConfigsDb`, budget config, `api_failures` all unread |
| **SO-9** | **INV-9** — every blocking wait has a timeout and a cancel path | `G-CANCEL`, `G-SUBPROC-TIMEOUT` | 🔴 RED | Permission oneshot has no timeout; `validate.rs` has zero |
| **SO-10** | **INV-10** — one definition per schema | `G-OWNER` | 🟢 GREEN | `src-shared`'s mirrors of `tasks`, `workers` and `plans` are **deleted** (22/19/23 write statements between them); `airsctl` uses `autodev_core::db`. `app_settings`' three creators route through `db/config_db.rs`, which owns the file because `user_version` belongs to the file, not a table. `TG-P1-04` asserts exactly one creator per table and was falsified by planting a second. `plans.db`'s two incompatible designs carried **no data risk** — the file had never existed on disk. |
| **SO-11** | **INV-11** — the updater installs nothing unverified | `G-UPDATE-REFUSE` | 🟢 GREEN | All three bypasses closed and unit-proven: empty checksum, empty `sig_url`, and `.unwrap_or_default()` on a failed checksum fetch. Verification also repeats at install time, closing the TOCTOU. **Note:** with no production key yet the updater refuses *every* update — correct for this invariant, but `TG0-01`'s accept-path stays unproven until Razi generates the keypair. |
| **SO-12** | **INV-12** — BYOK credential provenance | `G-KEY-NOENV` | 🟡 AMBER | `airsctl --api-key` and its `ANTHROPIC_API_KEY` fallback removed; the updater's runtime `AUTODEV_UPDATE_PUBKEY` override removed in favour of a compile-time key. AMBER because `main.rs` still loads `.env` via dotenvy walking **up** from the working directory — a credential channel that remains open by design for development. |
| **SO-13** | *(new, G-01)* **Fault isolation** — one panic cannot kill the app | `G-PANIC-CONTAINED` | 🔴 RED | `panic = "abort"` + in-process agents |
| **SO-14** | *(new, G-04)* **Spend has a ceiling that stops** | `G-SPEND-CAP` | 🔴 RED | No ceiling, no kill switch; per-task budget inert |
| **SO-15** | *(new, G-07)* **Secrets never enter durable memory** | `G-REDACT`, `G-MEM-GATEWAY` | 🟡 AMBER | ⚠ **This row was AMBER on a false basis and is re-stated.** The redaction was wired to a method with **no production caller**, so nothing was protected — three post-phase auditors found it, and a real `sk-` credential was sitting unredacted in a live log file at the time. Now on the production path (`execute_batch`, both branches), plus the log appender and analytics, proven end to end by `S1-04` and falsified. Still AMBER, for two stated reasons: a **bare high-entropy token in prose** — no assignment context, no recognisable shape — is deliberately not caught, because catching it means destroying every commit SHA; and `G-MEM-GATEWAY` (what memory chooses to store at all) is P3. **Updated rev 1.6:** the *input* side is now covered too — hook payloads, the write branch's UI event and its `messages` row were all still carrying raw tool arguments after the A-01 fix, which addressed only results. The one raw path left is the permission dialog, deliberately: the user is approving that exact call, and a redacted prompt is a weaker control, not a safer one. |
| **SO-16** | *(new, G-08)* **Agent writes are reversible** | `G-CHECKPOINT-BLOCKS` | 🔴 RED | ⚠ **Note was false and is corrected (`ADR-LOG-RESTRUCT-0010`).** Undo **does** ship — `git/checkpoint.rs`, five registered commands, called from `gitStore.ts`, with a snapshot taken before every agent write. `db/checkpoints.rs` was **deleted**, not declared (`ADR-0012`). Still RED for the real reason: `auto_checkpoint_before_write` returns `()` and logs a failure at `debug!` only, so a failed checkpoint is invisible **and the overwrite proceeds anyway** (`M-43`, P4/P9). |

**9 of 16 RED** — SO-4, SO-10 and SO-11 GREEN; SO-2, SO-3, SO-12 and SO-15 AMBER. The 50% cap remains active and will until every row clears. Two notes in this table were **wrong** rather than merely out of date and are corrected above (SO-3's bundle resource, SO-16's checkpoint claim); finding that is what the no-stale sweep is for.

---

# §4 · Mechanics coverage

Every mechanic reaches a phase and a scored area. **A mechanic with no phase is a planning defect, not a formatting gap** — this table exists because the review found that Part 1B had not been regenerated since rev 0.3, so none of ADR-05-01…05-06 owned a mechanic and an MDP harvested at that moment would have omitted the entire memory programme.

| Mechanics | Phase | Area |
|---|---|---|
| M-26 (platform command), M-27 (bundle/sign, part), M-28 (gates, part) | P0 | A0 |
| M-17 (compile the graph tools), M-40 (redaction), M-08 (one tool path, prep) | P1 | A1 |
| M-03, M-04, M-06, M-07, M-30 (BYOK), **M-42 (spend ceiling)** | P2 | A2 |
| M-01, M-02, M-05, M-09, M-10 (×5 sub-mechanics), **M-41 (fault isolation)** | P3 | A3 |
| M-11, M-12, M-13, M-14, M-15, M-16, M-31…M-34, **M-43 (reversibility)** | P4 | A4 |
| M-18 (graph-scoped context), M-19 | P5 | A5 |
| M-20, M-21, M-22, M-23, M-24, M-25 | P6 | A6 |
| M-35, M-36, M-37, M-38, M-39 | P7 | A7 |
| M-29 (submodule excision) | P8 | A8 |
| M-27 (remainder), M-28 (remainder) | P9 | A9 |

**Coverage: 43 of 43 mechanics assigned.** M-10 is flagged in the MDP as **five mechanics wearing one ID** (substrate · embeddings · scopes+provenance · gateway · reranker) and is decomposed at the start of P3.

---

# §5 · Arithmetic

```
overall = Σ (weight_i × area_pct_i)  for i in A0..A9
        = 8 × 0.33  (A0: Build ✅, Smoke ☐, Gates ☐)
        + 8 × 0.67  (A1: Build ✅, Smoke ✅, Gates ☐)
        = 2.7 + 5.3
        = 8.0   at rev 1.4

safety_cap = 50% if any overlay row is RED
           = ACTIVE  (9 of 16 RED; SO-4, SO-10, SO-11 GREEN; SO-2, SO-3, SO-12, SO-15 AMBER)

reported = min(overall, safety_cap) = 8.0%
```

---

# §6 · Update log

*Append-only. Never edit a past entry.*

| Date | Rev | Overall | Cap | Change |
|---|---|---|---|---|
| 2026-08-31 | 1.0 | **0%** | Active (16/16 RED) | Created. Ten scored areas weighted to 100, P4 heaviest as the headline capability. Sixteen safety-overlay rows — the twelve invariants plus four raised by the adversarial review (fault isolation, spend ceiling, secrets-in-memory, reversibility). All 43 mechanics mapped to a phase and an area. No phase started; awaiting GO. |
| 2026-09-01 | 1.1 | **2.7%** | Active (12/16 RED) | **P0 closed out.** Eight commits. Build ✅; Smoke and Gates NOT ✅ — no partial credit, and both have named unproven clauses. Smoke: `S0-02…S0-06` proven, `S0-01` proven on Windows only (Linux/macOS need a CI run). Gates: `TG0-02` GREEN; `TG0-01`/`03`/`04`/`05` PARTIAL, each with the missing clause recorded. Four overlay rows moved: SO-4 and SO-11 to GREEN, SO-2 and SO-12 to AMBER. SO-6 stays RED deliberately — `platform/command.rs` is proven but has no caller, and a guard nothing calls is AMBER at best. Cap remains active. |
| 2026-09-01 | 1.2 | **2.7%** | Active (12/16 RED) | **Post-phase audit applied.** The two mandatory §0.2a auditors returned 27 code findings and 41 stale-doc rows; 14 code findings are fixed, the rest triaged in `_verify/P0-AUDIT-TRIAGE.md` with an owning phase. **Four were regressions P0 itself introduced** — the fleet was silently dead on any machine without Python (`F-10`), subagent depth was incremented twice (`F-21`), the shipped supervisor's `request_review` was broken (`F-22`), and QC annotations were re-attributed to "user" (`F-04`). Score unchanged at 2.7%: Build stays ✅, and Smoke/Gates were already ☐. Two overlay rows would have been wrong without the audit — SO-4's gates tested one spelling of each defect (`F-18`), and the identity assertion behind SO-2 had become type-impossible (`F-09`); both are now falsifiable, proven by re-breaking. |
| 2026-09-01 | 1.3 | **2.7%** | Active (12/16 RED) | **First green CI run in the repo's recent history** (`33467890406`, all 7 jobs). Getting there found a startup crash P0 itself introduced: registering `tauri-plugin-dialog` made Tauri parse a `"dialog": {}` block it cannot deserialize, and **the application did not start on any platform**. Nothing else in CI could see it — the three build smokes stop at `cargo build` — so it surfaced only through the e2e job, which I had argued for deleting. Added a `xvfb` launch smoke to the Linux build so that class cannot hide behind a webdriver again. **Score deliberately unchanged at 2.7%:** Build ✅ stands, but Smoke stays ☐ because `S0-01` is 2 of 3 (macOS is never launched) and Gates stays ☐ because 2 of 5 are green — the no-partial-credit rule is doing its job, and relaxing it to reflect "real progress" is exactly the dishonesty this matrix exists to prevent. `S0-01` was un-ticked: it had been recorded green from `head -5` of a launch that crashed immediately after. New learnings `L-C9` (a verification whose shape cannot express failure, ×3) and `L-C10` (a failing check read as worthless). |
| 2026-09-01 | 1.4 | **8.0%** | Active (9/16 RED) | **P1 closed out.** Eleven commits. `autodev-core` exists and both binaries link it — `cargo tree` reports **0** Tauri/GTK/webkit edges for core and `airsctl` against **28** for the desktop crate, and `src-cli/tests/core_runtime.rs` constructs an `AgentEngine` in a process with no UI framework, a file that could not have compiled before this phase. All **10** databases carry a `user_version` ladder (was 1), `LIVE-DB-PROVEN` with **zero** row-count changes across 720 rows. Redaction (`M-40`), dropped from the plan entirely, is ported and wired, proven end to end by `S1-04`. **A1 scores 67%, not 100%:** Build ✅ and Smoke ✅, but Gates ☐ because `TG1-01`'s registration clause — the 27 graph-tool structs still implement no `Tool` and nothing registers them — is unmet, and no-partial-credit means that is a ☐. Compiling is not wiring, and this is the phase that keeps saying so. Three overlay rows moved (SO-10 → GREEN, SO-3 and SO-15 → AMBER) and **two were found to be wrong rather than stale**: SO-3 described a bundle resource P0 had already emptied, and SO-16 repeated the false "undo does not ship" claim retracted in `ADR-0010`. Four new learnings, three of them one shape: **`L-C11`** — a claim about state inferred from code instead of checked against the state, ×3 (the graph columns were never missing; `rusqlite` already set `busy_timeout`; chrono accepts the space separator and rejects the missing offset). **`L-C12`** — the backup helper had zero callers while I took the phase's backups by hand. **`L-C13`** — a fresh-database branch that no live-data check could reach, which shipped in the P1-10 commit and was caught by an unrelated test. **`L-C14`** — a redaction assertion that checked something had changed rather than that the secret was gone, and a bearer token walked straight through it. |
| 2026-09-01 | 1.5 | **8.0%** | Active (9/16 RED) | **P1 post-phase audit (§0.2a) and its remediation.** Four auditors over disjoint areas returned **116 findings**; the triage register is `_verify/P1-AUDIT-TRIAGE.md` — **26 fixed, 28 deferred with a named owner, 5 no-change**, plus 56 documentation rows. **The phase's headline claim was false.** Redaction was wired to `ToolExecutor::execute`, which has no production caller; the agent loop calls `execute_batch`, which redacted nothing. Raw tool output reached the model, the `messages` table and the UI. **Two auditors found it independently and a third refuted the claim resting on it** — and both of my own proofs were green, because `S1-04` drove the dead method and `TG-P1-06` matched a substring inside it. `P1-21` and `S1-04` were un-ticked, fixed, and re-proven with a falsification. `config.db` was found to hold **six** tables from five modules, not three from one: `INV-10` had been scored GREEN on my wrong count. Folded into the ladder at v2 — `LIVE-DB-PROVEN`, 108 rows of undo history preserved. **Score unchanged at 8.0%.** A1 returns to 67% because Smoke is genuinely proven again; Gates stays ☐ on `TG1-01`'s registration clause. The number is the same as rev 1.4 and it did not mean the same thing then — which is the argument for auditing rather than for scoring. Three recurring classes incremented rather than new ones opened: **`L-C12` → 2** (built, proven, wired to nothing), **`L-C9` → 4** (a verification whose shape cannot express failure), **`L-C6` → 4** (a check written from the claim, not the invariant). Per §0.2n a repeat is worse than a novel finding, and all three are repeats. |
| 2026-09-01 | 1.6 | **8.0%** | Active (9/16 RED) | **Razi's three rulings on the audit triage, shipped.** Supersedes rev 1.5's counts: **29 fixed, 27 deferred, 5 no-change** (rev 1.5 said 26/28/5 — that row stands as written, this one corrects it). **`A-08` pulled forward from P3:** all four PreToolUse/PostToolUse hook payloads now carry redacted tool arguments. Hooks are user-authored shell, so a credential in that payload reaches whatever the hook does with it. **`A-08a`, which no auditor found:** pulling A-08 forward exposed that `execute_batch`'s **write branch** had been passing the **raw input** to the UI event and the `messages` row all along — the A-01 fix redacted that branch's *result* and left its *input*, and `TG-P1-06` was structurally blind to it because it only inspects the result side. That is `L-C6`'s **fifth** instance: I wrote the check from the claim I had just implemented ("results are redacted") rather than from the invariant it protects ("no secret reaches a durable surface"). A second gate now pins the input side and was falsified. **`A-22` shipped rather than narrowed:** core gains `claim_next_ready`, a single guarded `UPDATE`; the CLI's three-round-trip claim and its compensating revert are deleted. Proven under real contention — 8 threads, 8 connections, one file, barrier-released: 8 distinct tasks, 0 stranded. Falsified: the old shape makes all eight claim task #1. **`~/.airs` deleted** after verifying the live store held the same 1 session / 14 messages, and after a backup that was read back before the original was removed. **Score unchanged at 8.0%**, and deliberately so: none of this adds capability. Two of the three were already-known findings and the third was a leak that had been live since P1 — fixing what was wrong does not move a readiness number, it makes the existing number honest. |
