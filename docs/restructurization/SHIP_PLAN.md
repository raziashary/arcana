# Arcana ship plan — the one live plan

| | |
|---|---|
| **Owner** | Razi (sole developer) |
| **Written** | 2026-11-03, replacing the P0–P9 master plan (now in [`archive/`](archive/README.md)) |
| **Status** | Cuts confirmed by Razi 2026-11-03. One ruling still open: `P9-02`, Windows code signing (see R4). Restructuring work resumes only when Razi says go. |

## 1. What "shipped" means

**Arcana v1 is a Windows-only installer** (Razi's whole company runs Windows). On a clean Windows machine a user installs it,
enters a provider key, hands it a coding task, and receives a build-verified result,
without reading documentation. That is the old headline acceptance test (`P9-15`), cut to
one platform.

Product definition (unchanged from [`archive/development/ARCANA_CODING_SCOPE.md`](archive/development/ARCANA_CODING_SCOPE.md)):
a native coding harness. The model plans and writes code; Arcana owns deterministic
project discovery, command selection, task claims, permissions, execution boundaries,
evidence and recovery. Outcomes AC-01..AC-07 stand; AC-07 (installable app) is Windows-only for v1.

## 2. Rulings (Razi, 2026-11-03)

1. v1 is **Windows-only**. Linux/macOS parity is post-ship ([`HARDENING_BACKLOG.md`](HARDENING_BACKLOG.md)).
2. P4 closes on the **middle path**: finish Windows process-sandbox containment (`P4-18`, `P4-61`,
   gates `TG4-09`/`TG4-18`/`TG4-31`) because it is a safety boundary. Observability depth,
   mid-run continuity and worker-health depth (`P4-50/51/53/54/56`, `P4_EXPANDED_GOAL.md`) move to the backlog.
3. The old MDP `§0` process is **slimmed** to the rules in §3 below.
4. The Linux/macOS sandbox work is **parked** on the local branch `park/linux-macos-sandbox` (commit `bc54874`, not
   pushed); see the backlog. All Windows harness code (process sandbox, `project_command`, egress consent, IDE Run) stays on `main`.
5. Razi may have docs changed and pushed to `main` directly. This does not extend to code pushes (`AGENTS.md`).
6. The proposed cuts in §4 were reviewed and confirmed.

## 3. Working rules (replace MDP §0)

1. **Ground in the code.** Read the code and this plan before building. The shipped code is the
   anchor; if faithful work contradicts a doc, fix the doc.
2. **Every task names a proof that can fail:** a test, a smoke run or a measured result. "It compiles"
   and "the doc says so" are not proof. A checkbox is a claim; re-run the owning check before relying on it.
3. **Live data is irreplaceable until Razi says otherwise.** Back up before destructive or schema-changing
   operations, including ones reachable through tests. Run suites without live provider credentials;
   `.env` keys are for explicitly enabled live smokes only.
4. **Security-sensitive code gets an independent read-only review before it lands:** the vault and keys,
   the process sandbox, egress consent, commit safety. Everything else is reviewed once per phase (rule 5).
5. **One independent review at the end of each phase,** read-only, with a self-contained brief. Findings
   are fixed, or recorded in [`DECISIONS.md`](DECISIONS.md) with a reason for deferral.
6. **Decisions go in `DECISIONS.md` only when there was a real choice** between alternatives. No entry for
   routine implementation.
7. **No stale docs.** The change that makes a line here untrue fixes it in the same commit.
8. **Commits:** one logical unit, Conventional Commit subject plus a body, staged by explicit path
   (never `git add -A`; never `.env` or `target/`). Pushes follow `AGENTS.md`.
9. **A phase boundary is a checkpoint, not an exit.** Commit, update §5, continue. Stop for a real blocker
   or a decision that is Razi's.

Provider-layer and keys conventions stay in [`../../AGENTS.md`](../../AGENTS.md).

## 4. Roadmap

```
R0 baseline green → R1 close P4 (Windows) → R2 graph (P5) → R3 UI + experience (P6, P7) → R4 Windows delivery (P9)
```

A proof column names the check that closes each row. IDs are the old task IDs so existing code comments
and archived receipts stay traceable. `v1` means required. Rows without it were cut or deferred on 2026-11-03.

### R0 · Baseline green

| Item | Proof |
|---|---|
| Fix `cargo fmt` drift in `arcana-core/src/recipes/{runner,scheduler}.rs` | `cargo fmt --all -- --check` clean |
| Fix four red `ci_gate` tests that also fail on a clean checkout of `8356aea`: `tg_p1_05_every_core_connection_is_opened_tuned`, `tg_p3_03_no_lock_propagates_poison`, `tg_p3_14_every_path_taking_tool_calls_the_jail` (names `reconcile_worker_operation`), `tg_p3_21_68_e2e_probe_keeps_evidence_and_stays_on_this_machine`. Both containment gates look like stale static scans, not holes: `reconcile_worker_operation` resolves the workspace from the host context, and the gate appears to match path strings in its test code; `e2e_probe` now validates loopback syntactically with no DNS (commit `ef7c0eb`) while the gate still demands `to_socket_addrs`. Read each, then fix the gate or the code | `cargo test -p arcana --lib ci_gate` all green |
| Fix the intermittent `src-cli/tests/fleet_offline.rs` failure (fails when its 7 tests run in parallel, passes serially; ADR-0412) | Parallel run passes repeatedly |
| Re-run `cargo clippy --workspace --all-targets`, `cargo test --workspace --all-targets --locked`, `npm test` and record the true failure set (Windows AppContainer `OS error 5` cases are recorded platform limits) | Results written into §5 |
| Confirm the old closure prerequisites `TG3-26` and `TG4-16` (persisted egress-consent path); their state was not re-verified when this plan was written | Test or smoke |

### R1 · Close P4 (Windows)

| Task | Scope | Proof |
|---|---|---|
| `P4-18`, `P4-61` v1 | Agent writes stay in the selected workspace; untrusted build/test children run under the OS sandbox with no network and no reads outside the workspace; the Windows command corpus (`.cmd` shims, Node/Python/Cargo) works under it | `TG4-09`, `TG4-18`, `TG4-31` |
| `S4-01` v1 | One task runs end to end on Windows and produces a build-verified artifact in the chosen workspace | Smoke |
| `P4-49` v1 | Razi's acceptance: build the Calendar app from the PA chat in `C:\Playground\arcana-calendar` | Razi runs it through `npm run tauri dev` |
| W9 v1 (added 2026-11-03, Razi to confirm) | The unfinished half of "an agent must not run loose in the terminal": per ADR-LOG-RESTRUCT-0401 the freeform shell (`bash.rs`) stays reachable only through a consent that names the capability the command exercises. Needs a capability object in `ProcessSpec`, consent keyed by capability instead of tool name, `run_ide_project` passing the same claim and build-evidence check as `project_command`, and the intent suite re-run through the production `execute` path rather than the internal launcher | New test per clause; intent suite via `execute` |
| `P4-56` / `TG4-30`, command path only (added 2026-11-03, Razi to confirm) | Independent attack tests of the command and sandbox path: task theft, path escape, secret leakage through command output and environment. The rest of the failure-containment review stays in the backlog | `TG4-30` narrowed to commands |
| `P4-50/51/53/54` | Deferred to the backlog (what is already built stays) | — |
| `TG4-24/25/27/28` | Deferred with their tasks | — |

**Terminal control already built on `main` (kept; nothing here was archived or parked).** Claimed by the archived
receipts and the ADR log; the code was confirmed present on 2026-11-03, but its behaviour was not re-run:
`project_command` (closed intent set, argv from `platform::command::resolve`, workspace, toolchain and task-claim
checks, `install` upgraded to host approval, launched only through the process sandbox); `bash` routed through the
same fail-closed sandbox, prompting in Autonomous mode where a stored `AllowAlways` cannot bypass it; the Windows
AppContainer sandbox (write containment, sibling-read denial, nested children, `.cmd` shims, junction escapes);
the workspace jail and write lease; checkpoint-before-write blocking agent writes (`P4-19`, `P4-20`); persisted,
revocable egress consent for diff, provider and hook traffic (`P4-60`, ADR-0400); env scrubbing and redaction in
`security/`; host-enforced commit safety (`P8-74`); bounded long-running verification (`P4-62`).

### R2 · Graph (P5)

Goal: the graph is compiled, correct, and read when the agent selects context.

| Task | Scope | Proof |
|---|---|---|
| `P5-01..03` v1 | One normalised graph DB path (four spellings produce four databases today); migrate existing DBs | Live-DB check, `S5-01` |
| `P5-04..07` v1 | Stop `INSERT OR REPLACE` cascade-deleting edges; real upsert; edge-conservation test; backup on the destructive ladder step | `S5-02` |
| `P5-14..17` v1 | Graph is read during context selection, scoped by graph distance, bounded; a recalled node appears in the outbound prompt bytes | `S5-05` |
| `P5-18..19` v1 | The 27 `graph_tools.rs` structs implement `Tool` and are registered; descriptions written to the `P3-20` standard (this absorbs `P5-21`) | `S5-04` |
| `P5-08..13` | Incremental re-cluster, rename, convergence: **defer; re-index in full on change** unless full re-index is too slow on a real repo | If kept: `S5-03` |
| `P5-20` | Bookkeeping only; dropped | — |

### R3 · UI and experience (P6, P7)

Goal: a new user reaches a working agent turn, errors are visible, nothing is lost, nothing is inert.

| Task | Scope | Proof |
|---|---|---|
| `P6-02` v1 | Self-host the font (the CSP blocks the external fetch, which is why Graph City hangs) | `S6-01` |
| `P6-01`, `P6-04` v1 | Token set from [`reference/ui-anchor.md`](reference/ui-anchor.md), defined once, no literal hex in components | Static gate |
| `P6-03`, `P6-05` | Dark mode and a drift gate for every surface: defer | — |
| `P6-06/07/09` v1 | Event subscription above the tab boundary; no event lost on tab switch; dead listeners removed | `S6-02` |
| `P6-08` | Reconnect replay: defer | — |
| `P6-10..13` v1 | Add the 2 missing commands, remove the 47 dead commands and 10 dead listeners, gate that both sides agree | Contract test |
| `P6-14` | Argument/return type checking: defer | — |
| `P6-15..18` v1 | Every setting persists and hydrates; provider/model survive restart; `ModelConfigsDb` read as well as written; no inert controls | `S6-03` |
| `P6-19..20` v1 | Graph City renders and reads the real graph; `P6-21` performance bench deferred | `S6-01` |
| `P6-22..23` v1 | Route or delete ~3,700 orphaned frontend lines; every route reachable from a cold start; `P6-24` gate deferred | `S6-04` |
| `P7-01..04` v1 | BYOK first run: enter a key, see it accepted, reach a turn, key never echoed, every failure names its next action | `S7-01` |
| `P7-05..08` v1 | Fix the two toast producers; errors surface once in one place; provider errors say what the user can do; completion, cancel, provider error and tool error are distinguishable | Smoke |
| `P7-09..11` v1 | Honest progress; cancellation actually cancels; long operations say what they are doing | `S7-03` |
| `P7-12..14` v1 | The 10 unconfirmed destructive actions ask first and say what is lost; undo where `git/checkpoint.rs` supports it | `S7-02` |
| `P7-15`, `P7-17` v1 | One vocabulary (worker, task, run, session); empty states that teach | Doc/UI check |
| `P7-16` | Interaction-grammar doc gate: drop | — |
| `P7-18` | Capstone walkthrough without docs: folded into `S7-01` | `S7-01` |
| `P7-19` | Automatic recovery after process loss: backlog | — |
| `P2-21` | Daily spend ceiling (M-42), the one open P2 task. Unpriced models are invisible to the cap, so a paid-key app should not ship without it | Test |

### R4 · Windows delivery (P9)

| Task | Scope | Proof |
|---|---|---|
| `P9-05` v1 | Windows installer (`installers/wix/`) builds, installs, launches | Smoke |
| `P9-09`, `P9-10` v1 | No `.py`, sidecar or dev `.env` inside the installer; installed size recorded | Gate |
| `P9-15` v1 | **Headline acceptance** on a clean Windows machine, from the shipped installer: install, enter key, hand over a task, receive a build-verified artifact (`S9-01`, `S9-03`) | Smoke |
| `P9-02` | Windows code-signing certificate (Razi's action). Without it the installer works but shows a SmartScreen warning, which matters now that the whole company runs Windows. Ruling needed | — |
| `P9-03`, `P9-04`, `P9-11..14` | Production updater key, signed updates, rollback: **ship v1 with the updater inert** (the README already says no feed exists). `S9-02` goes with it | — |
| `P9-01`, `P9-06..08`, `P9-16` | Apple/Linux installers, notarisation, cross-platform parity: backlog | — |

## 5. Status snapshot

Verified on 2026-11-03 in a working tree with uncommitted Linux/macOS work in it:

| Check | Result |
|---|---|
| `npm run typecheck` | passes |
| `npm run lint` | 0 errors, 40 warnings |
| `cargo fmt --all -- --check` | drift in `recipes/runner.rs` (two hunks), `recipes/scheduler.rs` (two hunks) and the untracked `linux_smoke/staging.rs` |
| `cargo test -p arcana --lib ci_gate` (no provider credentials) | 73 pass, **4 fail**, identical on a clean `8356aea` checkout (see R0) |
| `cargo test -p arcana-core --test p4_query_registry` | 9 pass |
| `cargo clippy`, the rest of `cargo test`, `npm test`, `npm run build` | **not re-run** when this was written; R0 does that |

Claimed progress from the archived plan, **not re-verified**: P0–P3 and P8 closed; P4 at 51/59 tasks and
25/33 gates (the old trackers disagreed, giving 46 to 51); P5, P6, P7, P9 untouched. The archived gate
checkboxes are unreliable: 34 of 41 TG3 gates are unchecked although P3 is recorded closed. Treat the
code and a fresh test run as the truth, not those boxes.

Last code commit: `8356aea` (2026-10-28), worker dispatch lineage.

## 6. Where things are

| Need | Go to |
|---|---|
| Deferred work and the parked Linux/macOS branch | [`HARDENING_BACKLOG.md`](HARDENING_BACKLOG.md) |
| Decisions made from now on | [`DECISIONS.md`](DECISIONS.md) |
| Code map (verify against the code; last refreshed 2026-10-14) | [`reference/PROJECT_MAP.md`](reference/PROJECT_MAP.md) |
| UI and memory design anchors | [`reference/ui-anchor.md`](reference/ui-anchor.md), [`reference/memory-anchor.md`](reference/memory-anchor.md) |
| What P4 already delivered, grouped by area | [`reference/P4_DELIVERED.md`](reference/P4_DELIVERED.md) |
| Lessons learned in P4 | [`reference/ARCANA_ENGINEERING_LEARNINGS.md`](reference/ARCANA_ENGINEERING_LEARNINGS.md) |
| Old plan, gate register, 412 ADRs, 230 receipts | [`archive/`](archive/README.md) |
