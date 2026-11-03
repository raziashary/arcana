# Restructure Planning — Working Tracker

**Purpose:** durable state for the AutoDev restructure planning run, so a fresh session can resume from artifacts alone (Universal Rule 9).
**Owner:** Razi · **Started:** 2026-08-31 · **Skill:** `vision-master-planning`
**Standing instruction:** NO CODE until the owner's explicit GO on the finished plan.

---

## Mandate (as given, 2026-08-31)

Restructure AutoDev across three surfaces, and produce the vision document suite in `docs/restructurization/vision/`.

1. **`src/`** — refactor the UI against `docs/restructurization/ui-anchor.md`.
2. **`src-tauri/`** — sweep for gaps, bugs and improvements; refactor; absorb all agent capability in-house.
3. **`submodules/`** — retire `core-orchestrator` and `memory-mcp-server` completely, along with the sidecar architecture.

Agents move in-house into Rust, built to the semantics of the vendor Agents SDKs, following the way `LivingAgent` works in `core-orchestrator` and aligning the Episodic / Semantic / Procedural memory model with those SDKs.

### Owner's answered pre-planning decisions

| # | Question | Answer |
|---|---|---|
| 1 | Deliverable shape | **The full suite**: Master Plan + deep specs `01–NN` + ADR ledger |
| 2 | ADR-001 (agent runtime) | **Native Rust, SDK-shaped** — *and* adopt SDK-native/server-side features wherever they help. Report the findings back. |
| 3 | Providers | **Anthropic, OpenAI, Gemini, Grok. Exactly these.** Ollama is retired. |
| 4 | Bug scope | The nine named bugs are **not** the full set. Sweep the codebase end to end, line by line. |
| 5 | End state | A **properly working, installable AutoDev** that Windows, Linux and macOS users can program with. |

### The owner's nine named bugs — ALL NINE TRACED

| # | Reported symptom | ID | Traced mechanism |
|---|---|---|---|
| 1 | Compiling of the code does not happen | BUG-COMPILE | **DONE.** Three-layer failure. **(a)** Build evidence is a process-global `HashMap` with a 15-min `Instant` TTL, **never persisted** (`build_evidence.rs:63-66`). **(b)** `bash.rs:404-437` rewrites pipes into **PowerShell** cmdlets then runs them under `cmd /C` (`:89-97`); the recipe's `… \| tee build.log; echo "EXIT:$?"` returns `tee`'s failure as the pipeline exit code, so **a successful build is recorded as failed** and `qc_review` becomes unreachable (G3d F-02/F-03/F-05, G3a SK-50). **(c)** There is **no Rust-native compile step** — every validator shells out, and on Windows `Command::new("tsc")`/`("pyright")` cannot resolve `.cmd` shims, so validation silently degrades to a warning. |
| 2 | Token blowup | BUG-TOKENS | **DONE.** **(a)** **Zero prompt caching on any provider** — `CompletionRequest` has no field for it and `build_body` emits none. **(b)** `engine.rs:196` clones the full history every turn; turn *k* re-bills turns 1..k-1 across up to `MAX_TOOL_TURNS=20`. **(c)** Compaction is **inert** — the trigger is a hardcoded `0.80 × 200_000` that ignores `ModelInfo::context_window`, ≈640 KB of JSON inside a 20-turn window. **(d)** Even when it fires it is **wasted**: it makes an extra full-context LLM call to summarise, then the caller reads only `updated_history.last()` and **discards the compacted history** (G2a T1/T2). |
| 3 | Worker not being assigned work properly | BUG-ASSIGN | **DONE.** **(a)** The cron gate omits `fallback_idle`/`mode1_idle_no_provider`, so a worker entering either state **never ticks again** (G3a SK-01); a transient SQLite error reads as "deleted" and kills the schedule permanently (SK-02). **(b)** **Task theft** — the claim is `WHERE id=? AND status='assigned'` and **never checks `assigned_worker`**, so any worker can steal another's task and overwrite the owner (G3d F-55). **(c)** The one-task-per-worker guard is a non-atomic `SELECT COUNT` that **fails open** via `.unwrap_or(0)`. **(d)** Reconciliation is dead — SQLite `datetime('now')` parsed with an RFC3339 parser at two sites, so every qc_review task reports age 0 s and the stalled-review alert can never fire (F-45/F-45b). |
| 4 | Graph not working; workers not using it for token management | BUG-GRAPH | **DONE, and worse than reported.** **`graph_tools.rs` (840 lines) and `graph_prompts.rs` (78 lines) have no `mod` declaration anywhere — they are never compiled.** The agent has no graph access at all, not even opt-in (G2a T32). `context.rs` has no graph import; the only `crate::graph` consumers are the UI command layer. Plus: two competing DB path schemes, the registry DB created but never populated, `INSERT OR REPLACE` + `ON DELETE CASCADE` silently deleting edges graph-wide, and incremental update dropping every cross-file edge (G3d F-21/F-22/F-23). |
| 5 | UI parts broken on Settings and more | BUG-SETTINGS | **DONE.** Every Settings control invokes a command that *exists* — the breakage is three other things. **(a) No persistence:** `set_active_provider` and `set_system_prompt` write only to `AppState`, which has no load/save and is rebuilt empty each launch — so the model choice and custom prompt vanish on restart and the first chat message then errors "No active provider set". **(b) Writes with no reader:** "Default Worker Budget" persists 4 values to `runtime_config.json` that **nothing in Rust ever reads**; 5 of 13 shipped hook templates target `pre_push`/`pre_commit`/`post_commit`/`on_plan_complete`, which are among the 16 of 30 `HookType` variants never fired — so "Block force push" and "Block .env commits" install, show ENABLED, and **protect nothing**. **(c) Unwired controls:** Browser Testing's Staging URL and Enable toggle are pure `useState`; `UpdateSettings.tsx:9` renders the literal `__APP_VERSION__`; `runtimeConfigStore` swallows errors so a failed save shows a green "Saved!" (G5c F01/F02/F03/F06). |
| 6 | Graph City UI broken | BUG-CITY | **DONE** (G5b) — six ranked root causes; headline is a CSP block on drei/troika CDN fonts that makes every `<Suspense>` hang forever, so districts, walls and towers never render. |
| 7 | Code created but the user cannot use the end product | BUG-DELIVER | **DONE, with live evidence on this machine.** `load_workspace_dir()` returns `""` on unset *or* error (its docstring says "Never fails"), so each consumer invents its own fallback — `recipes/runner.rs:175-190` → `~/.autodev/agents/<slug>/`, `mcp_server/tool_adapter.rs:70-88` → `current_dir()`. **Observed artifacts: `~/.autodev/agents/worker-backend/count_words.py`, `backend/`, `docs/`, `worker-frontend/README.md`.** And **there is no run/launch/handoff step at all** — `e2e_probe` boots the app and then kills it; `open_in_ide` emits `ide:navigate`, which has **zero frontend listeners** (G3d F-DEL). |
| 8 | Chat broken — functionality and UI | BUG-CHAT | **DONE** (G5a) — ten ranked defects; headline is that event listeners exist only inside `handleSend` and die on tab switch, orphaning the agent and hanging permission requests forever with no timeout. |
| 9 | Changing a model must be possible from the UI | BUG-MODELSWITCH | **DONE (backend).** Selection *is* per-request, so a switch takes effect next message. The real defects: `set_active_provider` is **in-memory only with no startup hydration**, so the choice **dies on restart**; `ModelConfigsDb` (temperature/top_p/reasoning_effort/thinking) is a **writer with no reader** — every `CompletionRequest` hardcodes those to `None`; and `analyze_diff_tool.rs:219-222` bypasses selection entirely via env vars set nowhere, falling back to a hardcoded `claude-sonnet-4-6` that exists in no provider catalogue (G2a). Frontend half in flight (G5c). |

Bugs beyond these nine are numbered `BUG-EXTRA-nn`. **Roughly 415 distinct defects catalogued so far.**

### Defects that were not on the list and are as serious

| Finding | Evidence |
|---|---|
| **The undo/rollback feature is not in the binary** — `db/checkpoints.rs` has no `mod` declaration | G3d F-44 |
| **`ToolExecutor::execute()` — the only fully-correct execution path — has zero callers** | G2a |
| **`Tool::requires_permission()` is never called anywhere** | G2a T47 |
| **`ToolContext.depth`/`.agent_role` hardcoded at all three construction sites** ⇒ the subagent recursion guard is inert, ACP tools permanently self-refuse, and the `qc_review` build-evidence gate is **skipped entirely on the native path** | G2a T48/T49/T50 |
| **Read tools (`read_file`, `grep`, `glob`, `list_directory`) have no path jail and are auto-approved** | G2a T12/T13 |
| **`edit_file(old_string="")` is an infinite loop with unbounded `Vec` growth** | G2a T14 |
| **Migration v5 uses `ADD COLUMN IF NOT EXISTS`** — not valid SQLite; a silently-swallowed syntax error that can never retry | G3d F-31 |
| **Subagent pool leaks a permit per fire-and-forget spawn; after 16, all spawning deadlocks silently** | G2b S-03 |
| **The Budget Circuit Breaker never fires** — `with_task_budget` has zero callers, so `task_budget` is always `None` | G2b B-01 |
| **Input tokens are 0 for all three cloud providers in streaming**; the cost fallback `_ => (1.0, 3.0)` catches 7 of 11 models | G2b P-30/C-02 |

---

## Phase state

| Phase | What | Status |
|---|---|---|
| P0 | Grounding sweep — **19 workstreams** (G0–G12 + G9/G10/G11 second pass) | **COMPLETE** |
| P1 | Reconciliation — exists / port / build-new / delete | **COMPLETE** (Master Plan Parts 1B, 1C) |
| P2 | Author `00-…-MASTER-PLAN.md` incl. ADR ledger | **COMPLETE** — Parts 1–10, rev 0.8 |
| P3 | Author deep specs `01–10` | **COMPLETE — all ten written** |
| P5 | Owner answers ADRs → fold through the suite | **COMPLETE — 35 ADRs, 0 open.** Two owner retractions recorded in place (ADR-01-07, ADR-05-02) |
| P4 | Adversarial pre-dev gap review | **COMPLETE** — `R1` buildability (23) · `R2` contradictions (47) · `R3` gaps & policy (34) = **104 findings, 39 proposed ADRs** → `../_review/` |
| P6 | Fold review findings; **Group G** | **COMPLETE** — 16 Group-G ADRs (8 Razi-answered, 8 decided), 3 new mechanics, 6 own-claim corrections, 13 fixes. **Ledger: 51 ADRs, 0 open** |
| P7 | `dev-anchor-set` → `../../development/` | **4 of 5 written** — MDP · READINESS_MATRIX · NOVELTY_LEDGER · ADR_LOG. `TESTING_GATES.md` harvest in flight |
| — | **GO gate** | ⛔ awaiting Razi |

## Review headlines

- **`R1 B-04` — deleting the sidecar deletes a fault boundary.** `panic = "abort"` + in-process agents ⇒ one panic kills the app and every session. Ten specs, zero mentions. → **G-01**, mechanic **M-41**.
- **`R1` disproved the plan's own weakest claim:** `sqlite-vec` and `rusqlite` **do** compose — `sqlite-vec 0.1.9` compiles only `sqlite-vec.c` with `-DSQLITE_CORE`, ships no `sqlite3.c`, and dev-depends on `rusqlite 0.31.0`. ADR-05-01 stands.
- **`R3` — the redaction port was dropped.** `G4b §A.14` ruled it **PORT** ("the `SENSITIVE_KEYS` list is the spec"); `redact`/`scrub`/`sanitiz` appeared **zero times** across plan and specs. → mechanic **M-40**; ported capabilities **9 → 10**.
- **`R2 X-04`** — the plan claimed the memory anchor's §10 reranker *is* the ported competitive model. **6 of 8 terms are disjoint**, and the false equivalence had left the Phase-2 reranker with no work item.
- **`R2 X-18`** — **Part 1B had not been regenerated since rev 0.3**, so none of ADR-05-01…05-06 owned a mechanic. An MDP harvested before this fix would have omitted the entire memory programme.
- **`R2 X-07`** — I had silently narrowed Razi's ADR-02-02 answer instead of flagging that a literal reading re-creates BUG-COMPILE. Now explicitly decided (G-05).
- Also corrected in place: the cache-hit formula was Anthropic-only · "structurally impossible" was an overclaim (the enum only relocates the dead-latch to `FromStr`) · `port_status.rs` does not survive the MCP-server deletion.

## Deliverables

| File | Status | Contents |
|---|---|---|
| `../00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md` | **COMPLETE (rev 0.4)** | Mandate (+ Razi's 8 second-pass directions) · grounding sources · Part 1 (thesis, **12 invariants**, non-goals, worked example, reconciliation) · Part 1B (**39 mechanics**, 6 workstreams) · 1C · Part 2 (det↔organ doctrine) · Part 3 (component mechanics) · Part 4 (**11 databases**, 7 reconciliations, migration discipline) · Part 5 (6 seams) · Part 6 (**9-slice build sequence, P8 = the gated deletion phase**) · Part 7 (acceptance test, 10 test families) · **Part 8: the 29-ADR ledger** · Part 9 (6 OQs, 10 risks) · Part 10 (amendment log) |
| `../01-AGENT-RUNTIME.md` | **COMPLETE** | The loop, DTOs (`Item::Opaque`), termination as a contract, persistence, cancellation, truthful `ToolContext`, in-process subagents, events. 10 gates |
| `../02-PROVIDERS-AND-THE-SDK-NATIVE-SURFACE.md` | **COMPLETE** | The trait, the divergence table, prompt caching + the alarm-before-cache rule, cost/usage rebuild, **BYOK lifecycle**, model switching, Ollama removal. 15 gates |
| `../03-CONTEXT-AND-MEMORY.md` | **COMPLETE** | The layered strategy, graph-scoped selection, the three tiers, the competitive-budgeting decision, **7 defects to fix-not-reproduce**, what not to port. 11 gates |
| `../05-THE-CODE-GRAPH.md` | **COMPLETE** | Not-in-the-binary → no-reader → self-corrupting-update; canonical path keys; the rendering contract. 7 gates |
| `../08-DATA-MODEL-AND-MIGRATIONS.md` | **COMPLETE** | 11 DBs, the 7 reconciliations, migration/connection/timestamp/timezone/index/retention/ownership discipline. 10 gates |
| `../09-DELIVERY.md` | **COMPLETE** | Cross-platform command execution, packaging, signing, **the updater RCE chain**, the submodule excision phase, CI. 10 gates |
| `../10-PROOF.md` | **COMPLETE** | Evidence classes, the headline acceptance test, the product proof, 10 test families, the frontend gap, what blocks. |
| `../04-THE-FLEET.md` | **COMPLETE** | Dispatch, atomic claim, Rust-native build verification, delivery, and the six coordination capabilities — **of which exactly one works end to end**. Documents the ADR-02-06 × ADR-03-02 collision. 16 gates |
| `../06-TOOLS-PERMISSIONS-SANDBOXING.md` | **COMPLETE** | The real permission model (auto-approval keyed on `is_read_only`, a concurrency hint), the four safety helpers, tool descriptions as a defect surface. 14 gates |
| `../07-THE-UI.md` | **COMPLETE** | Anchor implementation, event lifetime, contract reconciliation, Settings, Graph City, reachability, and the experience workstream. 14 gates |

**The vision suite is complete: master plan + 10 specs + 19 grounding reports.**

### G9 / G10 headlines

- **G9 (fleet, 54 findings):** of six coordination capabilities, **only escalation works end to end**. Routing affinity has *never influenced a decision* (outcomes are recorded only by a Tauri command autonomous operation never calls). **The Supervisor structurally cannot create an epic** — the tool is PA-only and the Supervisor reaches tools only via MCP. Per-task budget is fully inert. And **ACP sender identity is set in exactly one place: the MCP adapter being deleted.**
- **G10 (tools, 113 findings):** the permission gate in force is **`is_read_only()` — a documented *concurrency* hint** (`G10-111`). `path_jail` is a sound barrier that **six tools bypass**, two of which *act* on the path. `path_lock`'s key is a **raw path string**, so `C:\a\b.rs` and `c:\a\b.rs` take different mutexes for one file — the third subsystem with this same defect. `build_evidence.classify()` is `str::contains`, so `git commit -m "cargo build fixes"` satisfies the `qc_review` gate. `validate.rs` has **zero subprocess timeouts** and prints "✓ No syntax errors" for files it never checked. **33 of 38 tool descriptions are defective.**

## P0 addendum — workstreams added by Razi's second pass

| ID | Workstream | Output | Status |
|---|---|---|---|
| G9 | Fleet coordination capabilities — escalation, ACP worker↔worker messaging, dynamic routing/affinity, epic/phase decomposition, interface contracts, worker budget | `G9-worker-capabilities.md` | **IN FLIGHT** |
| G10 | The 44-module tool surface — per-consumer registration matrix, per-tool audit, tool-description defects, safety helpers | `G10-tool-surface-audit.md` | **IN FLIGHT** |
| G11 | UX and first-run — the BYOK onboarding flow, error surfacing, progress, destructive actions, orphaned surfaces, a11y | `G11-ux-audit.md` (688 ln) | **COMPLETE** |

### G11 headlines

- **No toast has ever been shown** — *verified first-hand by the main session, and the scout's framing sharpened.* The store (`toastStore.ts:27`) and the renderer (`ToastStack.tsx`) are both **correct**. The app's only two producers (`workerStore.ts:283`, `:324`) do `const { addToast } = require('../stores/toastStore')`, which is wrong twice: `addToast` is a store *method*, not a module export, so the destructure yields `undefined`; and `require` is unavailable in Vite browser ESM. **This is a two-call-site import fix, not a system to build.** `qc:decision` notifications have never fired once.
- **The boot overlay can lock the user out permanently.** `startupStore.ts:80-82` promises a watchdog that **does not exist**; if `ready` never fires there is no escape. It advertises "~15s"; the real worst case is ~113 s. Boot has **no failure state** — phases turn green for services that never started.
- **The primary BYOK failure is completely silent.** The send guard checks `activeProvider` only, never `has_key`, so a missing or invalid key produces an empty bubble and no error at all.
- **The key is never validated on save** — `handleSaveKey` stores and activates without calling `test_provider_connection`; the Test button only appears afterwards. There is **no key-replace path** (rotating an expired key means deleting it first), and **zero "get a key" links** anywhere.
- **First run is 10 steps, 6 of them undiscoverable.** Providers is the **8th of 14** flat Settings sections, below six infrastructure panels.
- **Error surfacing:** 88 `console.*`-only sites (72 are catch blocks whose entire body is `console.error`) · 8 native `alert()` · 22 inline error renders · 5 true no-op `.catch(()=>{})` · 48 unhandled store rejections (15 in `gitStore` alone) · **3 retry affordances in 35k LOC**.
- **10 destructive actions with no confirmation, six irreversible** — including *Delete checkpoint*, which is unconfirmed while the **reversible** Restore beside it is confirmed.

## P0 — grounding workstreams

| ID | Workstream | Output | Status |
|---|---|---|---|
| G0 | Repo shape · CI false-green · bundling/distribution · master-plan reconciliation · `src-cli`/`src-shared` | `G0-author-direct-findings.md` | **DONE** |
| G1 | `src-tauri` architecture map + sidecar coupling inventory | `G1-rust-architecture-map.md` (642 ln) | **DONE** |
| G2a | Bug hunt — agent engine, context, compaction, tool executor, builtin tools · **BUG-TOKENS** · **BUG-MODELSWITCH** · graph-reader question | `G2a-bugs-agent-engine.md` | **IN FLIGHT** |
| G2b | Bug hunt — provider, routing, budget, cost, subagent, session, Rust memory · prompt caching · pricing | `G2b-bugs-provider-routing-memory.md` | **IN FLIGHT** |
| G3a | Bug hunt — skills, recipes, hooks, plugin | `G3a-bugs-skills-recipes-hooks-plugin.md` | **DONE** (102 findings) |
| G3b | Bug hunt — git, ide, diff_analysis, orphan | `G3b-bugs-git-ide-diff-orphan.md` | **DONE** (115 findings) |
| G3c | Bug hunt — updater, mcp_server, vault, state, analytics, observability | `G3c-bugs-updater-mcp-vault-state-analytics-obs.md` | **DONE** (95 findings) |
| G3d | Bug hunt — graph, commands, db, plan · **BUG-COMPILE** · **BUG-DELIVER** · **BUG-ASSIGN** | `G3d-bugs-graph-commands-db-plan.md` | **IN FLIGHT** |
| G4 | LivingAgent loop semantics | `G4-livingagent-memory-semantics.md` (524 ln) | **PARTIAL — file truncated at §1.16** (agent sibling modules missing; low value, G4b covers the verdicts) |
| G4b | Sidecar subsystems · HTTP API surface · infrastructure deps | `G4b-sidecar-subsystems-api-infra.md` | **DONE** |
| G4c | Three-tier memory port specification | `G4c-memory-tiers-port-spec.md` | **DONE** |
| G5a | Frontend — Chat surface (**BUG-CHAT**) | `G5a-frontend-chat-audit.md` | **DONE** |
| G5b | Frontend — Graph / Graph City (**BUG-CITY**) | `G5b-frontend-graph-city-audit.md` | **DONE** |
| G5c | Frontend — Settings, stores, contract drift, design-system gap | `G5c-frontend-settings-stores-contract.md` | **IN FLIGHT** |
| G6 | Agents SDK research — Anthropic / OpenAI / Gemini / Grok | `G6-agents-sdk-research.md` (934 ln) | **DONE** |
| G7 | Docs corpus mining — authority, staleness, contradictions, known-issue ledger | `G7-docs-corpus.md` (807 ln) | **DONE** |
| G8 | Build · `airsctl` · installers · release pipeline | folded into `G0` §3 and §5 | **DONE (partial)** — a full line-by-line `src-cli` sweep is still outstanding |

> **Incident, 2026-08-31:** the first wave of 7 scouts hit an account rate limit mid-run. G1/G4/G6/G7 had written their files and survived; G2/G3/G5 died with their findings in flight. Their *sub*-agents had completed and reported, and that content was recovered from conversation context and written to G3a/G3b/G3c/G4b/G4c/G5a/G5b. **All re-dispatched scouts are now instructed to write their report file incrementally rather than batching the write to the end.**

---

## Headline findings so far

Full detail and citations in the `G*` files. This is the index.

### Existential / security
1. **The installer ships the retired sidecar — including a `.env` and a `.git`.** `tauri.conf.json` declares `resources/sidecars` (225 MB of Python source) as a bundle resource. Gitignore does not protect a built artifact. → G0 §3.1.
2. **The auto-updater performs no cryptographic verification when a release ships without sidecar files.** `verify.rs:90` skips the whole Ed25519 branch on an empty sig URL; `verify.rs:18` treats an absent checksum as a pass; the embedded public key is the literal placeholder string. **This is a remote-code-execution path.** → G3c IN-01…IN-05.
3. **The in-process MCP server exposes `bash`, `write_file`, `edit_file` and `spawn_subagent` on localhost with no auth, no Origin validation, and a caller-chosen working directory.** → G3c IN-06/IN-07.
4. **The plugin system is arbitrary code execution by design** — manifest strings go straight to `cmd /C`/`sh -c`, with an unsanitised install path and an LCG-seeded "fleet secret". → G3a SK-83/SK-90/SK-76.
5. **API keys and the GitHub PAT are returned to the webview in plaintext** by `get_provider_key`/`get_github_pat`, and the CSP permits `connect-src https://api.github.com`. → G3c IN-47.
6. **`.env` in `submodules/core-orchestrator` carries live AWS keys, provider keys, a Fernet vault key, and `DEBUG_AUTH_BYPASS=True`.** Rotate regardless of the port. → G4b §C.12.

### Correctness / product
7. **Nothing in CI can fail except `cargo clippy` and `tsc`.** Rust tests and `cargo fmt` are `continue-on-error`; there is no frontend test framework at all; `npm run lint` is `tsc --noEmit` wearing an ESLint label; build smokes skip PRs and skip macOS entirely. → G0 §2.
8. **On Windows, no fleet task can ever reach QC** (BUG-COMPILE). → G3a SK-50.
9. **A worker that ever enters `fallback_idle` never ticks again** (BUG-ASSIGN). → G3a SK-01.
10. **The orphan reconciler's TTL fallback can never fire** (`parse_from_rfc3339` on a `datetime('now')` column), and its cross-machine guard is void on Linux/macOS (`COMPUTERNAME` only). → G3b GI-01/GI-105.
11. **Graph City never renders districts, walls, or towers** — drei/troika fetch fonts from a CDN the CSP blocks, so every `<Suspense>` hangs forever. → G5b B-CSP.
12. **Chat listeners exist only inside `handleSend` and die on tab switch**, orphaning the agent and hanging permission requests with no timeout. → G5a §1/§8.
13. **Every dashboard cost and date figure is wrong** — local-vs-UTC date split, `DATE()` defeating the index, "today" meaning "since process start", and a rollup that effectively never runs. → G3c IN-50, IN-56, IN-61/62/63.
14. **`airsctl`'s entire agent command group is an HTTP client for the Python orchestrator** and dies with the sidecar. → G0 §5.1.
15. **Bundle targets are `["nsis"]` — Windows only**, and neither macOS nor Windows builds are signed. → G0 §3.2/§3.4.

### Structural
16. **The port surface is ~7% of the sidecar's API** — 9 endpoints consumed out of ~129 built, of which three matter. → G4b §B.2.
17. **The AutoDev MCP server already IS Rust** — today's flow is a round-trip (Rust tools → HTTP MCP → Python bridge → LLM → back). Removing the sidecar collapses it to a direct call. → G4b §C.7.
18. **The memory tiers are budgeted competitively, not proportionally** — one pooled two-stage utility ranking, not per-tier quotas. Any port assigning fixed per-tier budgets changes the behaviour. → G4c §4.4.
19. **The master plan contradicts itself in six places** — `~/.airs/` vs `~/.autodev/`, CLAUDE.md vs AUTODEV.md, compaction vs memory, internal board vs GitHub Issues. → G0 §4.1.
20. **Artifacts escape to the process CWD** (a bare `.autodev/GRAPH_REPORT.md`; an observed stray `src-tauri/.autodev/graph.db`). → G0 §6.

---

## Amendment log

| Date | Entry |
|---|---|
| 2026-08-31 | Tracker created. P0 dispatched: 7 scouts + 2 direct workstreams. G0 v1 complete. |
| 2026-08-31 | Rate-limit incident. Recovered sub-agent findings from context into G3a/G3b/G3c/G4b/G4c/G5a/G5b. Re-dispatched G2a, G2b, G3d, G5c with incremental-write instructions. Bug register now >300 findings. |
