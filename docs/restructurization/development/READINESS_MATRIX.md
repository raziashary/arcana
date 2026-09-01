# AutoDev Restructure — Readiness Matrix

> **LIVING DOCUMENT, append-only history.** Updated at every phase boundary. Never edit a past update-log entry.

| | |
|---|---|
| **Parents** | `MASTER_DEVELOPMENT_PLAN.md` · `TESTING_GATES.md` · `../vision/00-…-MASTER-PLAN.md` |
| **Rev** | 1.0 — 2026-08-31 |
| **Overall readiness** | **2.7%** — P0 built and committed; smoke and gates not fully green. Cap active. |

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
| **Overall readiness** | 🔴 **2.7%** | P0 code is complete and committed (8 commits). Build ✅. Smoke and Gates NOT ✅ — no partial credit. |
| Security posture | 🟡 AMBER | All three P0 holes closed: the updater refuses anything unverified, the `/mcp` RCE is deleted, and no command returns key material. Still AMBER because the updater has no production key yet (so it refuses *everything*), the runtime key-path is untested, and `.env` is still read at startup. |
| Gate honesty | 🟢 GREEN | Every `continue-on-error` gone; all checks blocking. **CI is now green end to end** (run `33467890406`) — the first passing run in this repo's recent history; the three before P0 all failed on a build that was structurally incapable of passing. |
| Cross-platform | 🔴 RED | All three build smokes now pass on PRs, and the app is verified to **start** on Windows and Linux (macOS builds but is never launched). Still RED: `platform/command.rs` exists but **nothing calls it** — `bash.rs` still rewrites pipes into PowerShell, so no fleet task can complete on Windows. Wiring is P4. |
| Agent correctness | 🔴 RED | No caching, unbounded context, no permission gate in force. P2 + P3. |
| Product capability | 🔴 RED | Agent output lands where the user never looks; no run step exists. P4. |
| Data integrity | 🔴 RED | One real migration ladder and its head is a broken statement; no backup path anywhere. P1. |
| Fault tolerance | 🔴 RED | `panic = "abort"` + in-process agents: one panic kills everything. P3. |

---

# §2 · Scored areas

| # | Area (phase) | Weight | Build | Smoke | Gates | Area % | Weighted |
|---|---|---|---|---|---|---|---|
| A0 | **P0 · Stop the bleeding** | 8 | ✅ | ☐ | ☐ | 33% | 2.7 |
| A1 | **P1 · Structural** | 8 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A2 | **P2 · Providers** | 14 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A3 | **P3 · Runtime + memory** | 18 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A4 | **P4 · The Fleet** ← headline | **20** | ☐ | ☐ | ☐ | 0% | 0.0 |
| A5 | **P5 · Graph** | 6 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A6 | **P6 · UI** | 10 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A7 | **P7 · Experience** | 8 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A8 | **P8 · Submodule deletion** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A9 | **P9 · Delivery** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |
| | **Total** | **100** | | | | | **2.7** |

**Why P4 carries the most weight.** It is the only phase whose completion means AutoDev does what it claims: a task runs end to end on three operating systems and produces a build-verified, reversible artifact the user can run. Everything before it is a precondition; everything after it is polish, cleanup or distribution.

---

# §3 · The safety overlay

| # | Invariant | Proving gates | Status | Note |
|---|---|---|---|---|
| **SO-1** | **INV-1** — no agent work in a foreign runtime or language | `G-NOSIDECAR`, P8 entry gate | 🔴 RED | Subagents moved in-process onto a nested `AgentEngine` and Mode 2 is inert, so no *agent turn* runs in Python. Still RED: `main.rs` continues to spawn `memory-mcp-server` and `core-orchestrator`, and `src-tauri/resources/` still carries the 225 MB Python tree. P8. |
| **SO-2** | **INV-2** — no key crosses the IPC boundary outbound | `G-KEY-ASYM` | 🟡 AMBER | `get_provider_key`/`get_github_pat` deleted; the vault command set is closed and gate-proven (`TG0-03` static half). AMBER not GREEN: the runtime half — a key entered in the UI cannot be read back by *any* route — has no test, and needs the e2e harness to execute. |
| **SO-3** | **INV-3** — no shipped artifact contains a credential | `G-BUNDLE` | 🔴 RED | 225 MB Python tree is a declared bundle resource |
| **SO-4** | **INV-4** — a gate that cannot fail is not a gate | `G-CI-BLOCKING` | 🟢 GREEN | All four `continue-on-error` flags removed; `cargo test` runs `--all-targets` (it ran `--lib`, silently skipping `tests/`). `TG0-02` asserts this over the workflow files; `S0-05` proves a failing test exits non-zero in both languages. |
| **SO-5** | **INV-5** — no artifact written against process CWD | `G-DELIVER` | 🔴 RED | Live artifacts observed in `~/.autodev/agents/` |
| **SO-6** | **INV-6** — platform-neutral agent commands | `G-NO-POSIX`, `G-CMD-INTENT` | 🔴 RED | `platform/command.rs` now exists and is unit-proven (10 tests, including the Windows `.cmd` shim). It changes nothing yet: **no caller uses it** — `bash.rs` still rewrites pipes into PowerShell under `cmd /C`. A guard nothing calls is not a guard. Wiring is P4-11. |
| **SO-7** | **INV-7** — measurements are provider-reported, not estimated | `G-USAGE`, `G-PRICE` | 🔴 RED | Streaming input tokens are 0 on three providers |
| **SO-8** | **INV-8** — a writer must have a proven reader | `G-MEM-READ`, `G-GRAPH-SCOPE`, reader-exists family | 🔴 RED | Graph, `ModelConfigsDb`, budget config, `api_failures` all unread |
| **SO-9** | **INV-9** — every blocking wait has a timeout and a cancel path | `G-CANCEL`, `G-SUBPROC-TIMEOUT` | 🔴 RED | Permission oneshot has no timeout; `validate.rs` has zero |
| **SO-10** | **INV-10** — one definition per schema | `G-OWNER` | 🔴 RED | `src-shared` mirrors four tables; `plans.db` has two designs |
| **SO-11** | **INV-11** — the updater installs nothing unverified | `G-UPDATE-REFUSE` | 🟢 GREEN | All three bypasses closed and unit-proven: empty checksum, empty `sig_url`, and `.unwrap_or_default()` on a failed checksum fetch. Verification also repeats at install time, closing the TOCTOU. **Note:** with no production key yet the updater refuses *every* update — correct for this invariant, but `TG0-01`'s accept-path stays unproven until Razi generates the keypair. |
| **SO-12** | **INV-12** — BYOK credential provenance | `G-KEY-NOENV` | 🟡 AMBER | `airsctl --api-key` and its `ANTHROPIC_API_KEY` fallback removed; the updater's runtime `AUTODEV_UPDATE_PUBKEY` override removed in favour of a compile-time key. AMBER because `main.rs` still loads `.env` via dotenvy walking **up** from the working directory — a credential channel that remains open by design for development. |
| **SO-13** | *(new, G-01)* **Fault isolation** — one panic cannot kill the app | `G-PANIC-CONTAINED` | 🔴 RED | `panic = "abort"` + in-process agents |
| **SO-14** | *(new, G-04)* **Spend has a ceiling that stops** | `G-SPEND-CAP` | 🔴 RED | No ceiling, no kill switch; per-task budget inert |
| **SO-15** | *(new, G-07)* **Secrets never enter durable memory** | `G-REDACT`, `G-MEM-GATEWAY` | 🔴 RED | Redaction was dropped from the plan; restored as M-40 |
| **SO-16** | *(new, G-08)* **Agent writes are reversible** | `G-CHECKPOINT-BLOCKS` | 🔴 RED | `db/checkpoints.rs` is not compiled into the binary |

**16 of 16 RED.** The 50% cap is therefore active and will remain so until every row clears. That is the correct reading of this codebase today, not pessimism: each row cites a specific, verified defect.

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
        = 2.7   at rev 1.1

safety_cap = 50% if any overlay row is RED
           = ACTIVE  (12 of 16 RED; SO-4 and SO-11 GREEN, SO-2 and SO-12 AMBER)

reported = min(overall, safety_cap) = 2.7%
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
