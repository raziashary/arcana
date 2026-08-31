# AutoDev Restructure — Readiness Matrix

> **LIVING DOCUMENT, append-only history.** Updated at every phase boundary. Never edit a past update-log entry.

| | |
|---|---|
| **Parents** | `MASTER_DEVELOPMENT_PLAN.md` · `TESTING_GATES.md` · `../vision/00-…-MASTER-PLAN.md` |
| **Rev** | 1.0 — 2026-08-31 |
| **Overall readiness** | **0%** — awaiting GO, no phase started |

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
| **Overall readiness** | 🔴 **0%** | Awaiting GO. No phase started. |
| Security posture | 🔴 RED | Unverified updater · unauthenticated `/mcp` RCE · keys readable from the webview. All three close in P0. |
| Gate honesty | 🔴 RED | Only `cargo clippy` and `tsc` can fail today. Closes in P0. |
| Cross-platform | 🔴 RED | Windows-only bundling, no macOS CI, and no fleet task can complete on Windows. P0 + P4. |
| Agent correctness | 🔴 RED | No caching, unbounded context, no permission gate in force. P2 + P3. |
| Product capability | 🔴 RED | Agent output lands where the user never looks; no run step exists. P4. |
| Data integrity | 🔴 RED | One real migration ladder and its head is a broken statement; no backup path anywhere. P1. |
| Fault tolerance | 🔴 RED | `panic = "abort"` + in-process agents: one panic kills everything. P3. |

---

# §2 · Scored areas

| # | Area (phase) | Weight | Build | Smoke | Gates | Area % | Weighted |
|---|---|---|---|---|---|---|---|
| A0 | **P0 · Stop the bleeding** | 8 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A1 | **P1 · Structural** | 8 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A2 | **P2 · Providers** | 14 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A3 | **P3 · Runtime + memory** | 18 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A4 | **P4 · The Fleet** ← headline | **20** | ☐ | ☐ | ☐ | 0% | 0.0 |
| A5 | **P5 · Graph** | 6 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A6 | **P6 · UI** | 10 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A7 | **P7 · Experience** | 8 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A8 | **P8 · Submodule deletion** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |
| A9 | **P9 · Delivery** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |
| | **Total** | **100** | | | | | **0.0** |

**Why P4 carries the most weight.** It is the only phase whose completion means AutoDev does what it claims: a task runs end to end on three operating systems and produces a build-verified, reversible artifact the user can run. Everything before it is a precondition; everything after it is polish, cleanup or distribution.

---

# §3 · The safety overlay

| # | Invariant | Proving gates | Status | Note |
|---|---|---|---|---|
| **SO-1** | **INV-1** — no agent work in a foreign runtime or language | `G-NOSIDECAR`, P8 entry gate | 🔴 RED | The sidecar runs today |
| **SO-2** | **INV-2** — no key crosses the IPC boundary outbound | `G-KEY-ASYM` | 🔴 RED | Two commands return raw secrets |
| **SO-3** | **INV-3** — no shipped artifact contains a credential | `G-BUNDLE` | 🔴 RED | 225 MB Python tree is a declared bundle resource |
| **SO-4** | **INV-4** — a gate that cannot fail is not a gate | `G-CI-BLOCKING` | 🔴 RED | Tests and fmt are `continue-on-error` |
| **SO-5** | **INV-5** — no artifact written against process CWD | `G-DELIVER` | 🔴 RED | Live artifacts observed in `~/.autodev/agents/` |
| **SO-6** | **INV-6** — platform-neutral agent commands | `G-NO-POSIX`, `G-CMD-INTENT` | 🔴 RED | No Windows task can reach QC |
| **SO-7** | **INV-7** — measurements are provider-reported, not estimated | `G-USAGE`, `G-PRICE` | 🔴 RED | Streaming input tokens are 0 on three providers |
| **SO-8** | **INV-8** — a writer must have a proven reader | `G-MEM-READ`, `G-GRAPH-SCOPE`, reader-exists family | 🔴 RED | Graph, `ModelConfigsDb`, budget config, `api_failures` all unread |
| **SO-9** | **INV-9** — every blocking wait has a timeout and a cancel path | `G-CANCEL`, `G-SUBPROC-TIMEOUT` | 🔴 RED | Permission oneshot has no timeout; `validate.rs` has zero |
| **SO-10** | **INV-10** — one definition per schema | `G-OWNER` | 🔴 RED | `src-shared` mirrors four tables; `plans.db` has two designs |
| **SO-11** | **INV-11** — the updater installs nothing unverified | `G-UPDATE-REFUSE` | 🔴 RED | Absence of a signature is currently a pass |
| **SO-12** | **INV-12** — BYOK credential provenance | `G-KEY-NOENV` | 🔴 RED | `airsctl` accepts a key as a CLI flag |
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
        = 0.0   at rev 1.0

safety_cap = 50% if any overlay row is RED
           = ACTIVE  (16 of 16 RED)

reported = min(overall, safety_cap) = 0%
```

---

# §6 · Update log

*Append-only. Never edit a past entry.*

| Date | Rev | Overall | Cap | Change |
|---|---|---|---|---|
| 2026-08-31 | 1.0 | **0%** | Active (16/16 RED) | Created. Ten scored areas weighted to 100, P4 heaviest as the headline capability. Sixteen safety-overlay rows — the twelve invariants plus four raised by the adversarial review (fault isolation, spend ceiling, secrets-in-memory, reversibility). All 43 mechanics mapped to a phase and an area. No phase started; awaiting GO. |
