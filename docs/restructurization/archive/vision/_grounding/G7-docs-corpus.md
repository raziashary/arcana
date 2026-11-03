# G7 — Documentation Corpus Grounding

**Purpose.** Mine the existing AutoDev documentation corpus so the restructure master-vision plan is grounded in what is already known, and state honestly which documents are authoritative and which are stale.

**Method.** Every document in the assigned read-set was read directly (large documents in full-file chunks; `DEVELOPMENT_PLAN.md` read by section with an exhaustive sweep of every heading, phase table, status marker, and checkbox, plus full reads of the agent / memory / provider / graph / gates / worker / sidecar sections). No source code was read.

**Evidence discipline.** Everything below is a **DOC-CLAIM** unless explicitly labelled otherwise — it is what a document asserts, not what the code does. Where two documents assert incompatible things, both are cited and neither is silently preferred. Citations are `path` + section heading; no line numbers (these are living documents).

**Corpus dates observed.** March 2026 (oldest, `old-multiple-fleet.md`) → 14 May 2026 (newest substantive engineering record, `docs/endtoendrun/14may2026/**`). **There is no documented run, audit, or phase closeout after 14 May 2026.** Anything the corpus says about "current state" is, at best, three-and-a-half months old relative to the restructure.

---

## 1. Doc register

Authority levels: **VISION** (what we want to build) · **DOCTRINE** (a binding rule/decision about how we build) · **SPEC** (a design contract for a subsystem) · **STATUS** (a record of what is/isn't built) · **POST-MORTEM** (observed behaviour of a real run) · **REFERENCE** (external/background material).

Freshness verdicts are relative to the restructure direction (sidecar deleted, agents in-house Rust, providers narrowed to Anthropic/OpenAI/Gemini/Grok, UI refactored to `ui-anchor.md`).

### 1.1 Assigned read-set

| Path | What it is | Date / rev stated | Authority | Freshness | Why |
|---|---|---|---|---|---|
| `docs/autodev-vision/autodev-master-plan.md` | The product spec + build prompt; self-declared "ground truth for AutoDev" | Draft v1.0, April 2026 | VISION | **SUPERSEDED IN PART** | Vision (PA→Supervisor→Worker→QC, AUTODEV.md, internal board, model router, cost meter) survives. Everything about sidecars, Ollama/Azure, the dark `#0A0A0A`/`#00FF88` design language, `~/.airs/` paths, and the TypeScript agent engine is dead on arrival. Also internally inconsistent (§1.6 below). |
| `docs/autodev-vision/old-multiple-fleet.md` | The pre-AutoDev 8-PC Claude-Code-Desktop fleet architecture | Draft v1.0, March 2026 | VISION (historical) | **SUPERSEDED** — but keep | Explicitly superseded by the master plan ("Old system … New system (AutoDev)"). Retains value as the *only* full statement of the org chart, task lifecycle, QC checklist, and dispatch protocol that the whole product is still modelled on. Read it as the source of the semantics, not the deployment. |
| `docs/development/DEVELOPMENT_PLAN.md` | Master development plan, 8,660 lines, 18 numbered sections + Phases 1–11 and many out-of-band sub-phases | "Living Document — Updated as phases complete" · no top-level date; newest internal marker is 14 May 2026 | STATUS + SPEC | **SUPERSEDED IN PART / partially STALE** | The definitive phase register and the single best record of what shipped. But its entire §2, §4.1, §5, §12 and Risk Log are built on the premise that the Python sidecars are load-bearing and must NOT be re-implemented in Rust — the exact premise the restructure inverts. Several "✅ COMPLETE" phases were later proved broken (§8.3). |
| `docs/development/ARCHITECTURE_CLARIFICATIONS.md` | The dual-mode (PA-Rust / Worker-Python) rationale doc | Created April 8, 2026 | DOCTRINE | **SUPERSEDED — do not build on** | Its entire thesis ("this is NOT duplication; AutoDev is two execution engines sharing a UI") is exactly what the restructure abolishes. It is also the doc that resolved the "which tools run in Rust vs Python" question that must now be re-opened. Historically important; forward-looking value ≈ zero. |
| `docs/development/PA_MEMORY_SYSTEM_SPEC.md` | Three-tier PA memory design (episodic / semantic / procedural) | Version 1.0, "Phase 4", Status: **Design Document** | SPEC | **AUTHORITATIVE for the model, STALE for the plumbing** | The tier model, SQLite schemas, MEMORY.md hybrid store, injection order, `search_memory`/`remember_this` tools, and migration path are all reusable and largely provider-agnostic. Everything from §6.2 "Recall API" and §8 "Integration with Core Orchestrator" onward routes through the sidecar and must be rewritten. |
| `docs/development/GRAPH_BUILD_FIX.md` | Note on moving `graph.db` out of the repo root | undated (context ≈ April 2026) | STATUS | **STALE / SUPERSEDED** | Its "expected behaviour" (124 file nodes, 0 edges, DB at `~/.autodev/graph.db`) was superseded twice: by real extraction (930–1,280 nodes) and by `graph_fixes.md` G-4, which moved the DB again to per-repo `~/.autodev/graphs/<sha256-12>.db`. Cite only as history. |
| `docs/development/MODE1_FALLBACK_RECIPES.md` | The `mode2_only` / `mode1_capable` fallback-policy taxonomy | "Doc 05 Phase 2" (14 May 2026) | DOCTRINE | **AUTHORITATIVE as a lesson, OBSOLETE as a mechanism** | Mode 1 vs Mode 2 ceases to exist when the sidecar goes. **But its core finding must survive the restructure**: a recipe/prompt written assuming a tool surface misbehaves catastrophically when that surface is absent — the agent loops on unmet preconditions rather than failing. That is a permanent design constraint, not a sidecar artefact. |
| `docs/development/PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md` | The approved deviation making subagents ephemeral via create→invoke→delete | Status: **Approved**, 2026-04-27 | DOCTRINE | **SUPERSEDED — but its three-layer cleanup pattern is reusable** | Records that four spec'd core-orchestrator endpoints **did not exist**; the whole workaround is sidecar-shaped. The RAII `Drop` + startup-orphan-sweep resilience pattern, the slug convention, and the "boundary is the bridge" argument transfer to an in-house Rust agent. |
| `docs/development/LOGGING.md` | Host logging config: paths, daily rotation, 14-day retention | "Doc 08 Phase 3" (14 May 2026) | SPEC | **AUTHORITATIVE** | Sidecar-independent. `~/.autodev/logs/autodev-<date>.log`, `Rotation::DAILY`, `AUTODEV_LOG_RETENTION_DAYS`. Carry forward as-is. |
| `docs/development/LOG_ERROR_KINDS.md` | The `kind=` error taxonomy (9 constants) | "Doc 08 Phase 2" (14 May 2026) | SPEC | **AUTHORITATIVE, partially unimplemented** | Carry forward. Note 6 of 9 kinds are marked **(planned)** at their emit sites — see §7. Two kinds (`orch_crash`, `watchdog_restart`) become meaningless once the sidecar is gone; `workspace_escape`, `tool_failed`, `budget_exceeded`, `qc_reviewer_failed`, `analytics_write_failed`, `mcp_unreachable` all survive. |
| `docs/development/ENVIRONMENT_SETUP.md` | Install checklist (Python 3.13, uv, Rust, Node, Tauri CLI, submodules, Redis/Memurai) | undated; describes Phase-1-era setup | STATUS | **STALE** | Over half the document (Python, uv, submodule venvs, Redis/Memurai) is deleted by the restructure. Rust/Node/Tauri prerequisites survive. Must be rewritten, not amended. |
| `docs/development/IT_RUNBOOK.md` | IT/DevOps runbook: build, sign, release, auto-update, enterprise deploy | "Version: Phase 5.3", last updated 2026-05 | SPEC | **AUTHORITATIVE with excisions** | The release/signing/auto-update/enterprise machinery is real, detailed and sidecar-independent. Only the "sidecar binary bundling" and Redis/Python prerequisite parts die. Highest-value operational doc in the corpus. |
| `docs/reference/autodev-graph-master-spec.md` | The full graph engine specification — data model, 7-stage pipeline, SQLite schema, 22 MCP tools, 51 atomic tasks | Reference v1.0, April 2026 | SPEC | **AUTHORITATIVE as design, ASPIRATIONAL as status** | Almost entirely provider- and sidecar-independent (one optional "or HTTP call to core-orchestrator" for Louvain). This is the best-preserved design document in the corpus. But it describes a system substantially larger than what was built — see §4. |
| `docs/codebase/autodev/autodev_codebase_dissected.md` | Narrative dissection of the AutoDev codebase | **"Last Updated: 2025"** | REFERENCE | **BADLY STALE — actively dangerous** | Claims 72 Rust files / 48 TS files; lists Phase 3.3+ (git), sidecar integration, graph, subagents, memory, analytics, mcp_server, observability as *"Future Extensions"* — all of which shipped. `master-notes.md` §4 instructs agents to read this before touching code. That instruction now actively misleads. See §8. |
| `docs/endtoendrun/14may2026/00-index.md` … `09-work-product-audit.md` | Deep audit of the 14 May 2026 end-to-end run; 108 atomic tasks across 9 docs, Phases 0–5 all marked ✅ COMPLETE 14 May 2026 | 14 May 2026 | POST-MORTEM | **AUTHORITATIVE — highest-value docs in the corpus** | The most recent, most rigorous, most honest engineering record. Every finding has a named root cause, a named file, and a stated status. Much of it (path jail, build-evidence gate, interface spec, e2e probe, validation tiers, log hygiene) is sidecar-independent and must be **carried into the new architecture, not rebuilt**. |
| `docs/endtoendrun/5may2026/calendar_run_analysis_1pm.md` | 5 May run analysis + deep `builtin_tools`/`mcp_server` audit (12 phases A–L, T1–T14, M1–M4) | 5 May 2026 | POST-MORTEM | **AUTHORITATIVE, partly self-invalidated** | Superb tool-level audit. One of its own "✅ Done" fixes (M2 — MCP bearer auth) directly caused the 6 May fleet-wide outage. See §8.1. |
| `docs/endtoendrun/6may2026/calendar6may.md` | 6 May run analysis; Issues A–I, BUG-001…007, PERF-001/2, DOCS-001 | 6 May 2026; **"Status: Issues documented — no code changes applied in this session"** | POST-MORTEM | **AUTHORITATIVE; fix status mostly UNKNOWN from this doc alone** | Contains the single most important architectural fact in the corpus for the restructure: *the core-orchestrator LivingAgent has zero built-in tools of its own.* See §6.2. |
| `docs/notes/complete-run-analysis-2026-04-29.md` | 24-minute fleet run post-mortem; 10 numbered issues | 2026-04-29 | POST-MORTEM | **AUTHORITATIVE as history; fixes tracked in DEVELOPMENT_PLAN Phase 7** | All 10 issues map to Phase 7.1–7.10, all of which are marked ✅ COMPLETE. |
| `docs/notes/core-orch-memory-namespace-fix.md` | Working-directory memory contamination; Rust workaround applied, Python fix deferred | 2026-04-29, Phase 7.10; **"Status: Rust-side workaround applied; Python-side fix deferred"** | SPEC + STATUS | **AUTHORITATIVE — and the deferred half is now moot in the best way** | The Python-side fix (project-scoped mem0 `agent_id`) was never done. Deleting the sidecar deletes the bug class. **But the underlying lesson is permanent**: any persistent memory store must be project-scoped, or an old workspace path is recalled as "relevant" and agents write into the wrong directory. |
| `docs/notes/memory-ttl-config.md` | mem0/Qdrant TTL configuration and cleanup-job design | 2026-04-29, Phase 7.10; **"Status: Documentation / deferred Python-side implementation"** | SPEC | **SUPERSEDED (mechanism) / AUTHORITATIVE (requirement)** | All three mechanisms are mem0/Qdrant-specific and die with the sidecar. The three-layer requirement — workspace header injection ✅ done, project-scoped namespace ⬜ deferred, TTL on ephemeral facts ⬜ deferred — is a real spec for the in-house memory store. |
| `docs/notes/feature-after-phase.md` | Post-Phase-6 backlog + a "what went where" mapping table | undated | STATUS | **PARTIALLY STALE** | Its routing table (PA Memory→P4, model enhancements→P2, Dashboard→P5, Graphify→P6) is accurate and useful. Its remaining backlog contains the Gemini path-handling bug and a never-written `developer-guide.md`. |
| `docs/notes/master-notes.md` | Razi's standing development rules + per-phase notes + phase milestone tables | undated; phase table shows Phase 2 as "IN PROGRESS" | DOCTRINE + STATUS | **DOCTRINE partly authoritative; STATUS badly stale** | The five development rules (ground in the plan, log deviations, cross-reference sources, read codebase docs first) are the house rules — but rules 3 and 4 point at the submodules and at the stale `autodev_codebase_dissected.md`. The milestone table says Phase 2 IN PROGRESS / Phases 3–5 "Not started", while `DEVELOPMENT_PLAN.md` marks Phases 3, 4, 5, 6, 7, 8, 9, 10, 11 complete. **Direct contradiction — see §8.2.** |
| `docs/submodules-management/SUBMODULE_SETUP_GUIDE.md` | Step-by-step git submodule setup + PyInstaller bundling | undated | REFERENCE | **OBSOLETE ON EXECUTION** | Becomes a deletion checklist. Useful only to enumerate what must be torn out: `.gitmodules`, `submodules/`, `build/bundle-*.sh`, `build/fetch-redis.sh`, `sidecars/`, the `sidecars` block in `tauri.conf.json`, and the `.gitignore` entries. |
| `docs/notes/github-actions-guide.md` | Tutorial on the two CI workflows (`ci.yml`, `build-release.yml`) + 4 bugs fixed | undated (post Phase 5.3) | REFERENCE + SPEC | **AUTHORITATIVE** | Fully sidecar-independent. Documents the real gates: `npm run typecheck`, `npm run lint`, Clippy (note: `-- -D warnings` was removed as Bug 2), rust-checks / frontend-checks / cli-checks / smoke-build jobs. This is the closest thing the corpus has to a written CI gate contract. |
| `docs/restructurization/ui-anchor.md` | The new UI design anchor: light-only palette, 7px radius, Urbanist/Manrope/Roboto Mono, glass surfaces, motion table, class vocabulary | undated (the restructure anchor) | DOCTRINE | **AUTHORITATIVE — the new source of truth for UI** | Supersedes *every* other UI statement in the corpus. See §8.1. |

### 1.2 Corpus documents outside the assigned read-set (surveyed, one line each)

Included because the restructure plan must know they exist and what authority they carry.

| Path | What it is | Authority | Freshness |
|---|---|---|---|
| `docs/development/graph/graph_fixes.md` | Graph defect spec, G-1…G-14, each with a per-fix `**Status:**` line; 13 of 14 ✅ Done, G-9 self-healing | STATUS | **AUTHORITATIVE — the real graph status doc** (supersedes `GRAPH_BUILD_FIX.md`) |
| `docs/development/graph/graph_improvement.md` | Graph canvas visual refinement spec (LOD, force params, community hulls, render modes) | SPEC | **AUTHORITATIVE as design, but its "Dark Industrial Aesthetic" (P5) is killed by `ui-anchor.md`** |
| `docs/development/graph/graph-3d-city.md` | CodeCity-style 3D city view spec; explicitly human-only, "zero agent exposure" | SPEC | **AUTHORITATIVE as design; unbuilt; same dark-palette conflict** |
| `docs/development/tools/TOOLS_AUDIT_2026_05_11.md` | Full audit of 19 builtin tools + MCP wrapper; 26 IDs with per-item status | STATUS | **AUTHORITATIVE — and self-contradicting on completeness (§8.1)** |
| `docs/development/tools/BUILTIN_TOOLS.md` | The original 7-tool Phase 1 catalogue | STATUS | **STALE** — dated 2026-04-09; the tool set grew to 19+ (`TOOLS_AUDIT`) then to ~25 with `check_file`, `append_file`, `write_interface_spec`, `e2e_probe`, `spawn_qc_review`, graph tools |
| `docs/development/tools/TOOL_FLOW_ARCHITECTURE.md` | Tool traversal Tauri→sidecar→LivingAgent | SPEC | **SUPERSEDED** — describes the `__autodev`-suffix routing through Python |
| `docs/development/analysis/token-explosion-analysis.md` | The 616,937-token incident | POST-MORTEM | **AUTHORITATIVE as evidence; mechanism sidecar-specific; internally self-contradicting (§8.1)** |
| `docs/development/analysis/startup_sequence_and_agent_persistence_analysis.md` | Startup toast FSM + the agent-UUID-proliferation fix (name-based IDs, no UUID) | STATUS | **AUTHORITATIVE**; "✅ IMPLEMENTED — both parts shipped April 20, 2026" |
| `docs/development/runtime-fixes/phase2/**`, `phase3/**` | Earlier submodule-integration and fleet-flow bug rounds | POST-MORTEM | **SUPERSEDED** — folded into `DEVELOPMENT_PLAN` Phases 2.8 and 3.6, all ✅ COMPLETE |
| `docs/development/ui-specific/ui_improvement.md` | "Complete UI/UX Specification", Draft v2.0, May 2026 — 16 sections incl. all 16 tool-call card types | SPEC | **SUPERSEDED IN PART** — structure/behaviour survive, all design tokens die under `ui-anchor.md`; §8.2 "Ollama Settings" dies outright |
| `docs/development/ui-specific/fleet_visibility.md` | Fleet Live Monitor spec (became Phase 11) | SPEC | **AUTHORITATIVE as design; shipped** |
| `docs/development/documentation-panel/AUTODEV_DOCUMENTATION.md` | In-app end-user documentation (12 sections incl. Graph Panel, Fleet & Delegate Mode) | REFERENCE | **STALE ON EXECUTION** — user-facing text describing sidecars/Ollama; must be rewritten or it ships a lie |
| `docs/reference/ollamainrust.md` | ollama-rs 0.3.4 integration guide | REFERENCE | **OBSOLETE** — Ollama is removed |
| `docs/reference/rustmcpexplained.md` | `rmcp` Rust MCP SDK guide | REFERENCE | **AUTHORITATIVE and now more important** — MCP moves fully in-house |
| `docs/reference/claudemdexplained.md`, `claudecodememoryexplained.md` | Background on CLAUDE.md and Claude Code auto-memory | REFERENCE | **AUTHORITATIVE background** — the stated inspiration for AUTODEV.md and the PA memory tiers |
| `docs/reference/ruflo-references/RUFLO_ARCHITECTURE_DEEP_DIVE.md`, `RUFLO_IMPL_PLAN.md` | Ruflo-inspired features (semantic routing, PID orphan detection, diff risk, budget breaker, epic decomposition, MCP health, Playwright QC) | SPEC | **AUTHORITATIVE; shipped as Phase 10**; explicitly scoped "no changes to submodules" so the work is already Rust-side |
| `docs/codebase/claude-code/**` (5 files) | Reference dissection of Anthropic's Claude Code — tool system, MCP integration, agent loop & state | REFERENCE | Background only; not AutoDev status |
| `docs/codebase/goose-main/**` (5 files) | Reference dissection of Block's Goose — agent system, provider system, MCP integration | REFERENCE | Background only; not AutoDev status |

---

## 2. The intended architecture, as documented

All DOC-CLAIM.

### 2.1 The org chart and the pipeline

The pipeline is the most stable idea in the corpus — it survives essentially unchanged from March 2026 to May 2026, across a complete change of deployment model.

**Original form** (`old-multiple-fleet.md`, §2 The Org Chart / §3 Role Definitions): Razi → Personal Claude (PA) → GitHub Issues board → Supervisor Agent (line manager) → 5 Worker Agents (Backend, Frontend, Platform, MCP Builder, Cleanup) → QC Agent. Eight physical PCs, each running Claude Code Desktop, coordinating through GitHub Issues, file-based lock files, and dispatch signal files.

**Current form** (`autodev-master-plan.md`, §4.1 "Worker Management & Agent Hierarchy"): one desktop app, the same org chart, workers as parallel Tokio tasks driven by cron recipes.

| Role | Count | User-creatable? | Spawns subagents? | Source |
|---|---|---|---|---|
| PA | 1 | — | ❌ No | `autodev-master-plan.md` §4.1 "Subagent Spawning Rules" |
| Supervisor | Exactly 1, system-defined, locked | ❌ No | ❌ No — "delegates, doesn't execute" | ibid. |
| Workers | 5 pre-defined + custom | ✅ Yes, via Settings → Workers | ✅ Yes, max 10 parallel | ibid. |
| QC Agent | Exactly 1, system-defined, locked | ❌ No | ✅ Yes, max 10 | ibid. |
| Subagents | — | — | ❌ No (prevents recursion) | ibid. |

Worker lifecycle states are enumerated in the same section: `created` → `active` → `working` → `queued` → `error` → `paused` → `deleted` (soft).

**The PA never assigns work directly.** In Delegate Mode the PA only calls `create_task`; the Supervisor polls the board and assigns. `autodev-master-plan.md` §4.1 "Delegate Mode Flow" states this explicitly and cites `old-multiple-fleet.md` as the pattern being preserved.

**PA sub-modes** (`autodev-master-plan.md` §3 "What AutoDev Adds Natively" and §4.1; restated in `DEVELOPMENT_PLAN.md` §4.1 "PA Mode Toggle"):

| Mode | Tools | System prompt |
|---|---|---|
| Fast Development | full builtin suite + all MCP | `BASE_SYSTEM_PROMPT` |
| Review | same tools + permission prompt before any write | base + caution |
| Delegate | **zero file tools** — only `create_task` + read-only MCP | `DELEGATE_SYSTEM_PROMPT` |

### 2.2 The task lifecycle

Two lifecycles are documented and they do **not** agree.

**Lifecycle A** — `old-multiple-fleet.md` §8 "Task Lifecycle":
`BACKLOG → ASSIGNED → IN PROGRESS → IN REVIEW → (APPROVED | REJECTED→IN PROGRESS) → MERGED → CLOSED`, plus a `status-blocked` side state.

**Lifecycle B** — `DEVELOPMENT_PLAN.md` §4.1 "Internal Board Storage", as a SQL `CHECK` constraint:
`ready | assigned | in-progress | in-review | qc-approved | qc-rejected | blocked | completed`.

**Lifecycle C** — what the post-mortems actually observe, which matches neither:
`ready | assigned | in_progress | qc_review | done | rejected | build_failed` (underscores, not hyphens). `qc_review` and `build_failed` and `rejected` all appear throughout `docs/endtoendrun/14may2026/**` and the 5 May audit. The 5 May audit records that `rejected` and `build_failed` were **missing from the tool schemas** (T8, T9) and had to be added — Phase A of its atomic to-do list.

> **For the restructure plan:** there is no single authoritative task state machine in the corpus. One must be written. The observed set (Lifecycle C) is the one the code converged on; the documented sets (A and B) are both stale.

Two transitions are performed by the *system*, not by agents, and this matters:
- **Reconciliation.** `docs/endtoendrun/14may2026/08-log-error-scan.md` §"Suspicious task transitions" records four `Reconciled stranded in_progress task #N → qc_review` events in one 15-minute run and states plainly: *"the worker LivingAgent never called `update_task_status(..., 'qc_review')` itself."* In the 14 May run, **3 of the 4 tasks that reached `qc_review` got there by reconciliation, not by worker self-promotion** (`00-index.md` §"Run statistics").
- **Orphan release.** `docs/endtoendrun/14may2026/08-log-error-scan.md` §"Orphan release on restart": a PID-based orphan detector releases tasks held by a dead host process back to `ready`.

### 2.3 The memory model

See §5 for the full treatment. In outline: three tiers (Episodic / Semantic / Procedural), a hybrid file+SQLite store for procedural memory (`MEMORY.md` + topic files, UI-editable), and injection into the system prompt in a fixed order (`PA_MEMORY_SYSTEM_SPEC.md` §6.1).

### 2.4 The graph's intended role

See §4. In outline: the graph exists to **replace token-burning full-file reads with surgical, scoped context**, feeding the PA's system prompt, the workers' pre-task discovery, and the QC agent's blast-radius review.

### 2.5 The provider / routing model

**Provider set as documented** (`autodev-master-plan.md` §4 diagram + §10 Phase 1 item 2): Anthropic, OpenAI, Gemini, **Ollama (local)**, **Azure OpenAI**. `DEVELOPMENT_PLAN.md` §8 Module Breakdown lists `provider/{anthropic,openai,gemini,ollama,model_router}.rs`. Ollama has a dedicated sub-phase ("Phase X — Ollama Provider (Local LLM)") and a dedicated 978-line reference guide.

> **All of that narrows to Anthropic / OpenAI / Gemini / Grok.** Grok appears **nowhere** in the entire corpus. It is a genuinely new surface with zero documented precedent.

**Model Router** (`autodev-master-plan.md` §6.8) routes on three inputs — agent role, task priority, and cost budget — e.g. `p0-critical → Opus`, `p3-low → Haiku`, `supervisor → Opus`, `worker-cleanup → Haiku`. Observed in practice, per `docs/endtoendrun/14may2026/04-qc-backlog.md`: workers and supervisor on haiku, qc-agent on sonnet, qc-dispatcher moved to haiku with sonnet reviewers.

**Key Vault** (`autodev-master-plan.md` §6.9): OS keychain (Windows Credential Manager / macOS Keychain / libsecret), never plaintext, 90-day PAT rotation reminders, keys scoped per agent role and per provider.

**Two documented provider landmines to carry forward:**
1. **Model-name translation.** `docs/notes/complete-run-analysis-2026-04-29.md` Issue 3: the Rust provider mapped the config key `claude-4-5-haiku` to the **LiteLLM-format** string `anthropic/claude-haiku-4-5-20251001` and sent it to the direct Anthropic API, producing 9 recorded 404s. The doc's recommendation — *"store the canonical name directly so no translation is needed"* — becomes mandatory once every provider is called directly with no LiteLLM in the path.
2. **Empty-model fallback.** `docs/endtoendrun/6may2026/calendar6may.md` Issue A: `unwrap_or("")` on a missing `model` key, with a doc-comment promising a fallback that *"is never implemented"*. Tracked as B-01 in `TOOLS_AUDIT_2026_05_11.md` §1 and still **⬜ Open** there.

### 2.6 The AUTODEV.md system

`autodev-master-plan.md` §6.2 is the authoritative statement and is unusually emphatic: *"Not CLAUDE.md. Not AGENTS.md. AUTODEV.md."*

- SQLite-backed in `~/.autodev/config.db`, **not** files scattered in repos; optional filesystem sync to `{repo}/.autodev/autodev.md`
- Exactly **2 tiers**: Global (`~/.autodev/config/autodev_global.md`) then Repo; precedence Repo > Global
- 40,000-character limit, enforced in the UI
- Monaco editor in the sidebar, live reload, no session restart
- Philosophy: *"AutoDev treats them as app configuration — because this is a desktop application, not a CLI tool."*

**Critically, AUTODEV.md is for the PA only.** `autodev-master-plan.md` §6.5 "Subagents + Agent Teams" contains a `CRITICAL DISTINCTION` block: **AUTODEV.md** = "How Razi wants his PA to behave"; **Recipes (YAML)** = "How the fleet agents should execute their tasks". Two instruction systems, two audiences, two stores.

> This split is load-bearing and easy to lose in a restructure. The fleet has never been driven by AUTODEV.md.

### 2.7 Skills / Recipes / Hooks / Plugins / Subagents / MCP

| System | Documented design | Source |
|---|---|---|
| **Skills** | Directory with `SKILL.md` + helper scripts, in `.airs/skills/` or `.claude/skills/`; frontmatter controls auto-invoke, slash-command visibility, subagent execution. Built-ins: `/review-pr`, `/run-tests`, `/write-spec`, `/claim-issue`, `/open-pr`, `/check-dispatch` | `autodev-master-plan.md` §6.4 |
| **Recipes** | YAML: `version`, `title`, `schedule` (cron), `permission_mode`, typed `parameters`, `instructions` (the agent prompt), `extensions`, `sub_recipes`. The fleet's unit of work | `autodev-master-plan.md` §6.6 |
| **Hooks** | `settings.json` lifecycle hooks: `PreToolUse`, `PostToolUse`, `SessionEnd`, matcher-based, `command` or `http` type. Shipped defaults: post-write auto-lint, pre-bash block on `git push origin main`, session-end cost POST | `autodev-master-plan.md` §6.5 |
| **Plugins** | Bundle Skills + MCP servers + Hooks into one installable; the "AIRS Plugin" configures a worker PC in one click | `autodev-master-plan.md` §3, §6; `DEVELOPMENT_PLAN.md` Phase 5.2 (✅ COMPLETE) |
| **Subagents** | Isolated context, custom system prompt, tool subset, parent blocks for summary, ≤10 parallel per parent. Pool caps: `MAX_GLOBAL_CONCURRENT=16`, `MAX_PER_PARENT=10`, `MAX_NESTING_DEPTH=3`, plus `MAX_CONCURRENT_QC=6` | `autodev-master-plan.md` §6.5; `PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md`; `docs/endtoendrun/14may2026/00-index.md` §B, `04-qc-backlog.md` |
| **MCP — client** | Connect to GitHub / Slack / Sentry / custom | `autodev-master-plan.md` §6.7 |
| **MCP — server** | AutoDev *is* an MCP server (`autodev-mcp-server`, in-process Tokio task on `127.0.0.1:8767`) exposing `airs_get_tasks`, `airs_claim_task`, `airs_complete_task`, `airs_dispatch_signal`, `airs_agent_status` — and, as built, the whole builtin tool surface | `autodev-master-plan.md` §6.7; `LOG_ERROR_KINDS.md`; `docs/endtoendrun/6may2026/calendar6may.md` Issue B |

### 2.8 Compile / build / QC gates

This is the most recently designed and least sidecar-dependent part of the architecture, and it is the part the restructure should most carefully preserve. All of it is from `docs/endtoendrun/14may2026/09-work-product-audit.md` (Phases 1–4, all ✅ COMPLETE 14 May 2026).

**Validation tiers**, run automatically after every `write_file` (T1.1, T1.4):
- Tier 1 — syntax (JSON/YAML/TOML/JS/Python; `tsc --noEmit --allowJs --isolatedModules` for TS/TSX/JSX, degrading to a warning when `tsc` is absent)
- Tier 2 — imports cross-referenced against `package.json` / `requirements.txt`
- Tier 2.5 — exports: every relative `import { X } from './foo'` must find a matching export in the target
- Tier 2.6 — API contract: fetch/axios/apiClient URLs substring-matched against `INTERFACE.md`
- Tier 3 — full types, still opt-in (slow)

**The build-evidence gate** (T2.4) is the sharpest idea in the corpus. `bash` records every recognised build/test/smoke invocation in a session-keyed ring with a 15-minute TTL. `update_task_status(id, 'qc_review')` **is refused** unless a recent build exists *and exited zero*. QC reviewers are exempted by `agent_role`. Recipes are required to use the form `bash('<cmd> 2>&1 | tee build.log; echo "EXIT:$?"')` so the exit code is echoed in the same tick.

**The cross-worker contract** (T3.1–T3.4): backend calls `write_interface_spec` to emit `<workspace>/INTERFACE.md` before `qc_review`; frontend reads it (Step 3.0, discovery-budget-exempt) before writing any consumer; `analyze_diff` emits `interface_drift: {consumer_only | contract_only | both | neither}` and `consumer_only`/`contract_only` are **hard-fail** signals for QC.

**The runtime gate** (T4.1–T4.3): `e2e_probe` spawns the start commands, waits, issues a real HTTP GET, and asserts expected text; `fleet_smoke_recipe` runs it on demand for an epic; the QC reviewer prompt mandates `e2e_probe` before approval.

**The path jail** (`07-workspace-escape-rootcause.md` Phase 1): `validate_workspace_path` canonicalises both target and workspace, walks up to an existing ancestor for non-existent files, strips the Windows `\\?\` prefix, and does lowercase component-boundary matching so `C:\foo` does not accept `C:\foobar`. Wired into `write_file`, `edit_file`, `append_file`, `bash` cwd, plus a best-effort scan of `>`/`>>`/`tee` redirect targets. 7 unit tests.

**The QC fan-out** (`04-qc-backlog.md`, Phase 3): the single serial `qc-agent` (304 s per tick on sonnet) was replaced by a `qc-dispatcher` (haiku, 30 s cron) that spawns one ephemeral sonnet reviewer per task, with `try_claim_for_qc_review` / `upgrade_qc_claim` / `release_qc_claim` atomic primitives, an `under_review_by` column, a stranded-reviewer reconciler, and a **$5/hour rolling cost guard** with a $0.10 provisional reserve per in-flight reviewer that trips *before* the atomic claim so a denied spawn cannot leak claim state.

**CI gates** (`docs/notes/github-actions-guide.md` §"Our CI Workflow"): jobs `rust-checks`, `frontend-checks`, `cli-checks`, and smoke-build jobs; `npm run typecheck` and `npm run lint` were added (its Bug 1) and Clippy's `-- -D warnings` was removed (its Bug 2) because it failed on all pre-existing warnings.

**The verification ritual** repeated verbatim at the end of every 14 May phase: `cargo check` clean · targeted `cargo test --lib` suites green · `npx tsc --noEmit` clean (exit 0).

### 2.9 Where the docs contradict each other on architecture

Stated here inline because they are the highest-value output; consolidated in §8.

**(a) Which language the agent engine is written in.** `autodev-master-plan.md` §7 "Tech Stack Decision" says *Agent engine: **TypeScript** (compiled to WASM or Node sidecar)* and *MCP client: **TypeScript MCP SDK***. `autodev-master-plan.md` §6.1 "How Workers Modify the Codebase" says *"File tools run in Rust natively"* and MCP via `rmcp`; `DEVELOPMENT_PLAN.md` §4.1 and `ARCHITECTURE_CLARIFICATIONS.md` §Question 2 both say Rust `rmcp`. The §7 table was never updated. **The restructure must not read §7 as a decision.**

**(b) GitHub Issues vs the internal board as the command layer.** `autodev-master-plan.md` §9 "GitHub Issues as the Command Layer" describes a native GitHub Issues widget reading "the full kanban board live". `autodev-master-plan.md` §4.1 "Internal Board vs GitHub" says the internal SQLite board replaced it — *"10-100x faster, no rate limits, offline capable"* — and that GitHub is now only for PRs. §9 was never updated.

**(c) `~/.airs/` vs `~/.autodev/`, and CLAUDE.md vs AUTODEV.md.** `autodev-master-plan.md` §8 "File & Config Structure" specifies `~/.airs/config.yaml`, `~/.airs/CLAUDE.md`, `~/.airs/sessions/sessions.db`, and per-repo `CLAUDE.md` + `.goosehints`. §6.2 and §4.1 specify `~/.autodev/{config.db,tasks.db,workers.db,memory.db,recipes/,worktrees/}` and AUTODEV.md. §10 Phase 1 deliverable 5 compounds it: *"CLAUDE.md injection: read `~/.airs/CLAUDE.md` + `{cwd}/CLAUDE.md`"*. All post-mortem evidence shows `~/.autodev/` in reality (`02-dashboard-not-live.md` lists the actual directory contents). **§8 and the §10 build prompt are stale; §6.2/§4.1 are current.**

**(d) The core-orchestrator port.** `autodev-master-plan.md` §4.1 "Startup Sequence" says **8765** (memory MCP **8766**). `ARCHITECTURE_CLARIFICATIONS.md` §Question 1 says **localhost:4000**. `DEVELOPMENT_PLAN.md` §2.1, §4.1 and §5.1 say **4000** (memory MCP **5000**). Every log line in every post-mortem shows **4000**. The master plan's ports are fiction.

**(e) Whether A2A is used at all.** `ARCHITECTURE_CLARIFICATIONS.md` and `DEVELOPMENT_PLAN.md` §2/§4.1 both list the **A2A protocol** as a primary justification for keeping the sidecar — *"impossible in simple Rust"*, *"supervisor ↔ workers coordination"*. But `DEVELOPMENT_PLAN.md` §18 Open Question 3 says A2A exposure is a **Phase 4 recommendation**, and no post-mortem anywhere describes agents actually communicating via A2A. Every observed coordination in every run goes through the **SQLite task board** and the **scheduler's push-kick**. **A2A appears to be an unexercised justification.** This materially lowers the cost of deleting the sidecar and the restructure plan should say so.

---

## 3. Known-issue ledger from the post-mortems

Exhaustive mining of `docs/endtoendrun/**`, `docs/notes/complete-run-analysis-2026-04-29.md`, plus the two audits those runs produced (`TOOLS_AUDIT_2026_05_11.md`, `graph_fixes.md`) which are the disposition record for many of these.

**Reading the "Fixed?" column.** `FIXED` = a document explicitly says so. `UNKNOWN` = no document says. `OPEN` = a document explicitly says it is still open. `MOOT` = the restructure deletes the component.

### 3.1 Run of 2026-04-29 (`docs/notes/complete-run-analysis-2026-04-29.md`)

| # | Issue | Root cause as documented | Fixed? |
|---|---|---|---|
| 1 | Panel costs blank | `AnalyticsCollector::record_token_usage()` exists but is never called from any execution path; Mode 2 usage stays inside Python's LiteLLM store and is never forwarded | FIXED per `DEVELOPMENT_PLAN` Phase 7.5 ✅ — **but see §3.3 #2: the analytics pipeline was still architecturally dead two weeks later** |
| 2 | Dashboard blank | Same as #1 — `analytics.db` has no rows | FIXED per Phase 7.6 ✅ — same caveat |
| 3 | qc-agent / supervisor 404 (9 occurrences) | `provider/anthropic.rs` maps `claude-4-5-haiku` → LiteLLM-format `anthropic/claude-haiku-4-5-20251001`, invalid against the direct Anthropic API | FIXED per Phase 7.1 ✅ |
| 4 | **App freeze, force-kill required** | Three compounding: (A) scheduler set `next_fire = 0ns` on idle-completion and on fallback error — 32 occurrences, one burst of 6 fires in 99 ms; (B) no error backoff — ~2 failing invocations/second; (C) 281 MB of logs in 24 minutes | FIXED per Phase 7.2 (backoff) + 7.3 (rotation) ✅ |
| 5 | Calendar app shows black screen | CRA project never compiled — workers wrote source but never ran `npm run build`; recipe treats build as validation only, never as a deliverable | FIXED per Phase 7 — **and recurs at §3.4 Gap 1 sixteen days later** |
| 6 | Redis/Memurai offline | Memurai service not running; cascades into 6 agents' "failed to load dynamic prompt", per-agent-creation warm-tier warnings, and a 12-line stack trace per agent init | FIXED per Phase 2.8.2 ✅ — **recurs at §3.3 Issue C** |
| 7 | MCP server 409 on startup | `POST /api/mcp/servers` registration is not idempotent; core-orch persists registrations across restarts | FIXED per Phase 7.9 ✅ |
| 8 | False-negative health check → unnecessary Mode 1 fallback | `/health` times out (<1 s) while core-orch is busy on a long LiteLLM call; a busy event loop is read as a dead process | FIXED per Phase 7.8 ✅ (debounce; later hardened with a TCP-alive probe — proven working in the 14 May run) |
| 9 | Working-directory memory contamination | mem0/Qdrant recalled a `working_dir` from a *previous project* by semantic similarity; workers wrote into the old directory | **PARTIAL** — Rust `[AUTODEV WORKSPACE]` header injection done (Phase 7.10 ✅); Python namespace scoping + TTL **explicitly deferred and never done** |
| 10 | No log rotation (142 MB / 24 min; 281 MB total) | No rolling file config; `memory-mcp.log` logs the full ~1,000-char system prompt on every memory store, at INFO | FIXED per Phase 7.3 ✅ + `LOGGING.md` |

### 3.2 Run of 5 May 2026 (`docs/endtoendrun/5may2026/calendar_run_analysis_1pm.md`)

Systemic issues (§9):

| ID | Issue | Fixed? |
|---|---|---|
| S1 | No rejection-reason tracking for QC | FIXED (Phase A: `rejection_reason` + `retry_count` added) |
| S2 | No `rejected` status in the task state machine | FIXED (Phase A) |
| S3 | Frontend has no live data polling loop for dashboard/cost | FIXED (Phase C) — **recurs at §3.3 Issue F and again at §3.4 Doc 02** |
| S4 | Backend analytics events not bridged to the Tauri event bus | FIXED (Phase C) — **recurs** |
| S5 | **Memory MCP grows unbounded within a session** | **UNKNOWN — no doc records a fix.** Scoped to "Mem0 / vector store sharding", i.e. submodule work that was never in scope |
| S6 | Agent toast states are binary (dispatched / done) | FIXED (Phase D) — **recurs at §3.3 Issue D** |
| S7 | Build validation runs even on `build_failed` tasks | FIXED (Phase E: QC build-failed short-circuit + retry count) |

Isolated bugs (§9) and phase-level fixes:

| ID | Issue | Fixed? |
|---|---|---|
| B5 | **Build commands not quoting Windows paths with spaces** — `C:\AIRS Foundry\...` broke every agent-executed shell command; *uniformly* failed both backend and frontend builds | FIXED (Phase B, `bash.rs`) |
| B6 | 59 consecutive health-check failures logged at startup before the gateway was ready | FIXED (Phase G, startup grace window) |
| B7 | Windows `WinError 10054` → memory-MCP disconnects (ProactorEventLoop + anyio SSE) | FIXED per Phase 3.6.4 ✅ |
| — | QC↔Ready cycling loop; QC invocations >110 s each, burning tokens | FIXED (Phases A + E) |
| — | Startup health-check race: watchdog declared unhealthy *before* the gateway was ready | FIXED (Phase G) |
| — | Theme visibility — text illegible on dark background (`--bg-primary #0a0a0f`) | FIXED (Phase H) — **now MOOT: `ui-anchor.md` is light-only** |
| — | Idle agents invoking the LLM with nothing to do | FIXED (Phase I: pre-flight task count; est. "up to 80% of idle LLM invocations" eliminated) |
| — | Sidecar `ClosedResourceError` log noise drowning genuine errors | FIXED (Phase J) |

Tool-level findings (§13, T1–T14 / M1–M4) — **this list is the best single inventory of tool-layer defects in the corpus and every entry is Rust-side, i.e. survives the restructure**:

| ID | Issue | Fixed? |
|---|---|---|
| T1 | `bash.rs` `rewrite_for_windows` missing `working_dir` param — cannot strip the `cd` prefix | FIXED (Phase B) |
| T2 | `bash.rs` stdout/stderr unlabelled — agents cannot tell build warnings from errors | FIXED (K.1) |
| T3 | `bash.rs` no `working_dir` existence check before spawn — opaque error | FIXED (K.2) |
| T4 | **`edit_file.rs` never fires the `PostFileWrite` hook** — the graph engine and event bus are blind to every edit | FIXED (K.3) |
| T5 | **`grep.rs` `case_insensitive` silently broken** — lowercases the line but the regex still expects original case | FIXED (K.4) |
| T6 | `grep.rs` `context_lines` deserialized but never used | FIXED (K.5) |
| T7 | `grep`/`glob` filter matches filename only — `src/**/*.py` patterns do not work | FIXED (K.6) |
| T8 | `list_tasks_tool.rs` — `build_failed`/`rejected` missing from schema; `rejection_reason`/`retry_count` absent from output | FIXED (K.7) |
| T9 | **`update_task_status_tool.rs` — `rejected` missing from the schema enum**, so QC agents fell back to `assigned`, *recreating the QC↔Ready loop* | FIXED (K.7/K.8) |
| T10 | No `rejected` kick; no `build_failed`/`rejected` hooks | FIXED (K.8) |
| T11 | `assign_task.rs` — `rejected` not dispatchable, so the supervisor could not re-dispatch rejected tasks | FIXED (K.9) |
| T12 | **`spawn_subagent.rs` sets `mcp_servers: Vec::new()`** — subagents had *zero* tool access and produced hallucinated reviews | FIXED (K.10) |
| T13 | `spawn_subagent.rs` `depth` always 0 — unbounded recursive spawning possible | FIXED (K.11) |
| T14 | Default subagent system prompt has no project/task context | FIXED (K.12) |
| M1 | `tool_adapter.rs` — no `__working_dir` per call | "✅ Done" (L.1) — **but §3.4 Doc 07 proves the Python side never sent it** |
| M2 | `mcp_server/server.rs` — no auth on the MCP server | "✅ Done" (L.2) — **and this fix caused the 6 May fleet-wide outage; see §3.3 Issue B** |
| M3 | `tool_adapter.rs` — no `__session_id` per call | FIXED (L.3) |
| M4 | Workers could call `assign_task` / `spawn_subagent` via MCP | FIXED (L.4, `register_for_mcp_worker()`) |

### 3.3 Run of 6 May 2026 (`docs/endtoendrun/6may2026/calendar6may.md`)

Header status: **"Issues documented — no code changes applied in this session."**

| ID | Issue | Root cause as documented | Fixed? |
|---|---|---|---|
| A | Empty model string in `spawn_parallel_subagents` | `unwrap_or("")` forwards `""` to `POST /api/agents` → `Model '' not found in registry` → 500 → `SubagentStatus::Failed`, silently, from the caller's view. The `SubagentConfig` doc-comment promises a fallback that *"is never implemented"* | **OPEN** — tracked as B-01 in `TOOLS_AUDIT_2026_05_11.md` §1, status ⬜ Open |
| B | **MCP server 401 on every tick — the fleet ran tool-less for a whole session** | Three layers: (1) an unnecessary bearer-auth middleware on a localhost-only in-process server; (2) an `anyio` cancel-scope crash on non-2xx teardown that abandons the whole MCP bridge for that invocation; (3) **`runner.rs` never fell back**, because the 401 happens *inside* the LivingAgent session so `invoke_worker()` still returns `Ok(HTTP 200)` | **FIXED (inferred, UNKNOWN from this doc)** — the 14 May run shows Mode 2 with full MCP tool access, so BUG-002 evidently landed; no doc states it |
| C | Redis / Memurai connection refused | `try_start_memurai_service` tries `net start` / `sc start` only; a standalone (non-service) Memurai install fails all three attempts silently, fire-and-forget | **UNKNOWN** — BUG-003 not marked done anywhere; DOCS-001 (an `ENVIRONMENT_SETUP.md` section on standalone vs service install) also never written |
| D | QC decisions invisible — toasts silent | `workerStore.ts` only toasts on transition *to* `error`; `worker:status` carries only `working`/`idle` with no QC context; toast dedupe keys on `slug+status` so repeated `idle` is swallowed | PARTIAL — the 14 May work added `worker:log-line` breadcrumbs and QC card surfaces; a dedicated `qc:decision` event (BUG-004) is **UNKNOWN** |
| E | Rejected tasks stuck — supervisor never re-dispatches | The supervisor recipe logic is **correct**; it is starved by Issue B (zero tools) and by the dead Mode-1 fallback | Blocked-by-B; presumed resolved with B |
| F | Dashboard auto-refresh too slow / `task:changed` not wired | (1) interval regressed 60 s → **180 s**; (2) `emit_task_changed_global()` **returns early and silently** when the Tauri `AppHandle` is not installed — which is exactly the case during a background recipe run | PARTIAL — the 14 May work added a `worker:tick-end` → `refreshDashboard()` listener. **The silent-AppHandle-miss failure mode is not documented as fixed anywhere.** |
| G | High token usage per tick from extended prompts | 8 extended prompts rebuilt from disk every tick because the Redis cache is down (Issue C); `a2a_communication` prompt attached to workers that never communicate | **UNKNOWN** — PERF-001/PERF-002 are config-only changes in submodule YAML; MOOT after restructure |
| H | `AIRSClientManager` unclosed + anyio cancel-scope crash | Async context manager not wrapped in `async with`; resource leak | **UNKNOWN** (Python) — MOOT |
| I | `AgentConfigFSM` invalid `applying → validating` transition on every invocation | Missing `applied()` state between `applying()` and `validate()` | **UNKNOWN** (Python) — MOOT |

### 3.4 Run of 14 May 2026 (`docs/endtoendrun/14may2026/**`)

The most rigorous set. All nine docs carry per-phase `✅ COMPLETE 14 May 2026` markers and a verification block; `00-index.md` states *"The whole 108-task roadmap from the original audit is closed."*

| Doc | Issue | Severity | Root cause as documented | Fixed? |
|---|---|---|---|---|
| 07 | **Worker wrote three files into `src-tauri/` instead of the workspace, restarting the dev server and losing user logs** | CRITICAL | **There is no path jail.** File tools resolve relative paths against `ctx.working_dir` but never validate the *resolved absolute* path. Compounded: the Python `MCPBridge` passes LLM arguments verbatim and **never injects `__working_dir`** (a grep across the whole core-orchestrator tree returned zero hits), so `tool_adapter.rs` fell through to `std::env::current_dir()` — which under `npm run tauri dev` is `src-tauri\`. The `[AUTODEV WORKSPACE]` prompt header is *"advice, not enforcement"* | FIXED (path jail + tauri-stored workspace preference + stray-file scanner + mismatch banner) |
| 02 | **Dashboard shows zero forever, even on manual Refresh** | CRITICAL | `analytics::install_global` is **never called from `main.rs`**; every `analytics::global()` call falls into a pre-init fallback that builds a *fresh schemaless in-memory SQLite connection on every call* — and constructs **four separate** in-memory DBs, so even a successful write would be invisible to reads. Every failure swallowed by `let _ =`. `~/.autodev/analytics.db` did not exist on disk. The doc explicitly retracts the previous audit's diagnosis: *"Previous audit blamed missing event subscriptions. That was wrong."* | FIXED (install_global wired; silent fallback replaced with `expect()`; `let _ =` replaced with logged errors; broken cross-DB foreign key dropped) |
| 04 | Tasks pile up in QC review | HIGH | A single shared `qc-agent` on sonnet serialises every review — one 304-second tick for one task; `kick_worker` is gated on `status == "working"` and **drops** concurrent kicks rather than queueing them; full ~250-line QC prompt replayed every tick | FIXED (qc-dispatcher fan-out) |
| 01 | Live Monitor stays STALE/IDLE while workers are working | HIGH | `handleStatusChange` does `if (!agent) return state` — a **silent no-op** for any slug not pre-registered. `initAgents()` was called from a deep `AgentRoster` mount and raced `subscribeToEvents()`; the STARTUP tick fires ~1 ms after the scheduler boots, long before the frontend has loaded workers. Dep array keyed on `workers.length`, so a *rename* never re-registered | FIXED (lazy create-on-first-event in all handlers; `initAgents` hoisted; slug-set hash dep) |
| 09 | Calendar app shipped non-functional (16 defects) | HIGH | Three structural gaps: workers can declare `qc_review` **without ever running the artifact**; there is **no contract** between worker-backend and worker-frontend; `qc_review` **analyses text diffs, not a live system**. Plus: `worker_frontend_recipe` **literally prescribed `index.html` in `public/`, which is wrong for Vite** — *"The recipe itself bakes in the bug"* | FIXED (all four phases) |
| 03 | Worker Requests dropdown "does nothing" | HIGH | One `expandedId` state overloaded for two orthogonal interactions using a `-1` sentinel; opening a row unmounts the list containing it | FIXED (state split) |
| 06 | QC agent has no Live Monitor visibility | MEDIUM | Same lazy-registration root as 01, **plus** Mode 2 LivingAgent ticks emit no `agent:tool-call-*` events at all (that is the Mode 1 channel), so even a correctly-registered QC card is a bare coloured dot | FIXED (lazy registration + `subagent:event` bridge + `invoke_worker_streaming` SSE for main workers) |
| 05 | Was the Mode 1 fallback justified? | INFO | Yes — one fallback, TCP and HTTP both dead. The TCP-alive escape correctly kept Mode 2 during a busy-event-loop window. **But**: Mode 1 selected and then immediately idled — *"no LLM provider configured"* | N/A (fallback-policy taxonomy added) |
| 08 | Log error sweep | INFO | 4 ERRORs, all downstream of the orch crash. No tool failures, no timeouts, no budget breaches | N/A (log hygiene shipped) |

**Two findings from the 14 May set that are not defects but are the most load-bearing facts in it:**
- The run **ended on an orchestrator crash**, not a Rust-side bug. The new orch came up, returned HTTP 500 on `PUT /api/agents/<name>` for **all six agents within the same 1 ms**, then went TCP-dead 35 seconds later. `08-log-error-scan.md` states the Python traceback lives in a separate sidecar log and the crash *"needs separate investigation"* — **that investigation is nowhere in the corpus.**
- `00-index.md` "Run statistics": 11 tasks created, **4 reached `qc_review`, 3 of them via the reconciler rather than worker self-promotion**, and `~/.autodev/analytics.db` did not exist.

### 3.5 The tools audit (`TOOLS_AUDIT_2026_05_11.md`) — items still marked open

| ID | Issue | Status in doc |
|---|---|---|
| T-05 | `list_directory.rs` recursive listing (depth=3) has no output size cap on large repos | ⬜ **Open** |
| B-01 | Empty model string → "Model '' not found in registry" (= 6 May Issue A) | ⬜ **Open** |
| P-03 | `planner.rs` tool description is 790+ chars — injected on **every** LLM call, largest of any tool | ⬜ **Open** |
| G-01 | `SemanticSearchTool` is misnamed — it does keyword substring matching, not vector search; agents expecting similarity loop on poor results | ⬜ **Open** |
| G-02 | Every graph tool calls `get_all_nodes_for_graph(i64::MAX, 0)` — loads the entire graph on every invocation, O(N) | ⬜ **Open** |
| G-03 | `FindPathTool` uses DFS but the comment says BFS — may return non-shortest paths | ⬜ **Open** |
| G-04 | **`BuildGraphTool` does not build anything** — it only reads current stats, but its name implies construction, so agents call it expecting a rebuild | ⬜ **Open** |

> Four of the seven open items are graph tools. See §4.5.

### 3.6 Cross-cutting root-cause themes

Five patterns produced the majority of the ~90 defects above. Each is a design constraint for the restructure, independent of the sidecar.

1. **Silent failure by default.** `let _ = ...` on analytics writes; `if (!agent) return state` in the event store; `emit_task_changed_global()` returning early with no warning when the `AppHandle` is missing; `save_cached()` discarding write errors; a pre-init fallback that fabricates a working object instead of panicking. In every case the system reported healthy while doing nothing. The 14 May fix pattern — *replace the silent fallback with `expect()` so a regression is impossible to miss* — is the correct doctrine and should be a stated rule.
2. **Prompt-as-enforcement.** The `[AUTODEV WORKSPACE]` header, the recipe's "run `npm run build`" instruction, and the subagent `__depth` propagation note were all **text instructions with no mechanism behind them**, and all three were violated. `07-workspace-escape-rootcause.md` states it directly: *"This is advice, not enforcement."* Every invariant that matters needs a check that can fail.
3. **Contract stated in a comment, never implemented.** `tool_adapter.rs`'s comment declared *"Fleet workers MUST pass `__working_dir` on every tool call"* — and `07` records that **no one upstream honoured it**. `SubagentConfig`'s doc-comment promised an empty-model fallback that was never written. Both were found only by a real run.
4. **State duplicated across the Rust/Python boundary.** Token usage lived in Python's LiteLLM store and never reached `analytics.db`. Tool execution FSMs lived in Python while the UI listened on Tauri events. Worker status lived in both `workers.db` and the LivingAgent registry. Almost every "the UI shows nothing" bug traces back to a value that existed on one side of the boundary and was never bridged. **Deleting the boundary deletes this entire class.**
5. **Errors that are not errors at the transport layer.** The 6 May 401 is the canonical case: the fleet was completely paralysed while every layer reported success, because the failure was *inside* an HTTP 200 body. `runner.rs` had a fallback that could never fire. The proposed remedy — BUG-002b, a **pre-flight tool-capability check** (`get_agent_tools()`; if empty, refuse to proceed) — is the right shape and its status is UNKNOWN.

---

## 4. The graph subsystem, as specified

Sources: `docs/reference/autodev-graph-master-spec.md`, `docs/development/GRAPH_BUILD_FIX.md`, `docs/development/graph/{graph_fixes,graph_improvement,graph-3d-city}.md`, `DEVELOPMENT_PLAN.md` §14 (Phase 6) and Phase 6.17/6.18.

### 4.1 What it is FOR

`autodev-graph-master-spec.md` §1 "Why a Code Graph" opens with the thesis, and it is a **context-and-token thesis, not a visualisation thesis**:

> *"AI agents read your entire codebase on every task. This burns tokens, hits context limits, and produces worse results than targeted context. A code knowledge graph fixes this."*

The mechanism: parse once into nodes and edges, persist in SQLite, *"query it at task time to surface only the nodes the agent actually needs."* The claimed prize, from the `code-review-graph` reference project, is **8.2× average token reduction across 6 real repositories**. `DEVELOPMENT_PLAN.md` §14 restates it as the goal of the phase: *"Gives PA and Worker agents surgical graph queries instead of token-burning full-file reads. Replaces 8× token waste with precise blast-radius context."*

Three named audiences (§1):
1. **PA** — "What does this file depend on? What tests cover it?"
2. **Workers** — "Before I implement feature X, which existing nodes do I need to read?"
3. **QC** — "What is the blast radius of this PR? Which tests should I run?"

### 4.2 What it is supposed to FEED — the context-scoping role

This is the part the restructure most needs and it is specified in three distinct places.

**(a) PA system-prompt injection.** `autodev-graph-master-spec.md` §13.3 "PA Context Injection" specifies `build_graph_context(query, repo)` in `context.rs`:
1. score nodes against query terms (the graphify `_score_nodes` algorithm)
2. BFS from the top-5 scored nodes, depth 2
3. format as `NODE <label> [src=<file> community=<name>]` / `EDGE <src> --<kind> [<confidence>]--> <tgt>`
4. **cap at 2000 tokens (`char_budget = 6000`)**
5. prepend to the system prompt in PA mode

`DEVELOPMENT_PLAN.md` Phase 6.15 marks this `[x]` COMPLETE with a **different budget — 800 tokens** — and adds gating rules the spec does not state: injected only when `graph.db` exists for the current repo AND the query is non-empty; active in Fast Development and Review modes; **omitted in Delegate mode**; gracefully absent until the first build.

**(b) Worker pre-task discovery.** §13.3 specifies the recipe instruction verbatim:
> *"Before starting task, call `get_minimal_context(task="<task_title>")` to understand which existing functions and classes are relevant. This saves token waste from reading raw files."*

`get_minimal_context(task)` is specified in §10.1 as returning **~100 tokens** — god nodes + relevant communities + suggested next tools — as the deliberate cheap entry point.

**(c) QC review context.** §13.4 gives QC a five-call ritual: `detect_changes(base)` → `get_impact_radius(qname)` → `get_affected_flows(file)` → `list_tests(qname)` → `get_review_context(files)`, and states the intent: *"This gives QC a precise, token-efficient review context — no reading entire files."* The Phase 6 success criterion quantifies it: QC reviews *"in < 400 tokens of graph context"*; Phase 6.15's criterion says *"< 500 tokens of context instead of reading entire files."*

**Also specified as consumers:** the query layer has *"BFS/DFS traversal **with token budget**"* and *"Subgraph extraction for system prompt injection"* as first-class components (§5.2); a dedicated MCP tool `annotate_for_worker(task_id, worker)` exists to *"Mark subgraph context for a worker's system prompt"* (§10.6); and §5.3 lists *"AUTODEV.md reflection — Graph infers AUTODEV.md instructions from patterns it finds"* as a key innovation (specified nowhere else, built nowhere).

> **Bottom line for the restructure:** the graph was designed as the **context-scoping and token-budgeting substrate for all three agent classes**, with explicit token caps at every injection point. Whether it ever functioned in that role is a separate question — see §4.5.

### 4.3 Schema and data model

**Node kinds** (§6.1): `File`, `Class`, `Function`, `Type`, `Test`, `Module`, `Import`.

**Edge kinds with weights** (§6.2): `CALLS` 1.0 · `INHERITS` 0.8 · `IMPLEMENTS` 0.7 · `DEPENDS_ON` 0.6 · `IMPORTS_FROM` 0.5 · `SEMANTICALLY_SIMILAR` 0.5 · `REFERENCES` 0.4 · `TESTED_BY` 0.4 · `CONTAINS` 0.3 · `IMPORTS` 0.3.

**Confidence tiers** (§6.3): `EXTRACTED` (explicit in source) · `INFERRED` (deduced) · `AMBIGUOUS` (flagged for human review).

**AutoDev-specific attributes** (§6.4/§6.5) — the genuinely novel part: every node and edge carries `agent_author` (which agent created it), `task_id`, `worker_locked`, and `worktree`.

**Storage** (§8): SQLite via `rusqlite`, WAL mode. Tables: `schema_version`, `nodes` (UNIQUE on `qualified_name`), `edges`, `communities`, `flows`, `snapshots` (compressed JSON for graph-diff), `analysis_cache` (invalidated by `graph_hash`), `nodes_fts` (FTS5 virtual table, `porter unicode61`), `repos`. Thirteen indexes including `idx_nodes_task`, `idx_nodes_author`, `idx_edges_source_kind`, `idx_edges_target_kind`, `idx_edges_tier`.

**Extraction** (§5.1, §5.2): **Rust-native tree-sitter bindings, no Python subprocess** — an explicit design principle. 20–25+ language configs via a `LanguageConfig` struct, parallel parsing via Rayon. SHA-256 file hashing with an extraction cache.

**Where the DB lives — three documented answers, all different:**

| Doc | Location |
|---|---|
| `autodev-graph-master-spec.md` §13.2 | `.autodev/graph.db` — per repo, in the repo |
| `GRAPH_BUILD_FIX.md` §Solution | `~/.autodev/graph.db` — a single global DB in the user's home, *"Shared across all projects"* |
| `graph_fixes.md` Fix G-4 (✅ Done, latest) | `~/.autodev/graphs/<sha256-12>.db` — per-repo, keyed by a hash of the repo path |

G-4 is the newest and explicitly fixes *"single global database regardless of workspace; multi-repo data corruption"* — so `GRAPH_BUILD_FIX.md`'s "shared across all projects" was itself the bug. G-4's own §"Known Unknowns" item 4 flags an unresolved question: *"the old `~/.autodev/graph.db` will be abandoned. Decide: migrate data, delete silently, or warn the user."* **No document records that decision.**

### 4.4 Pipeline, updates, performance

**Seven stages** (§7): Detect → Extract → Build → Cluster → Analyze → Report → Export.

**Update triggers** (§12.1): worker file write (Tauri FSEvents watcher) · git post-commit hook · manual CLI/UI · worker task completion (snapshot + incremental merge) · QC review request.

**Update algorithm** (§12.2): SHA-256 hash all changed files → compare to stored `file_hash` → delete-and-reparse changed files → **find dependents via `SELECT DISTINCT file_path FROM edges WHERE target_qualified IN (changed_qnames)`** and repeat → rebuild FTS5 for changed nodes → invalidate `analysis_cache` → emit `graph:updated`.

**Performance targets** (§12.3, inherited from code-review-graph benchmarks): 100 files <1 s full / <0.1 s incremental · 500 files ~5 s / <0.5 s · 2,900 files ~30 s / <2 s · 10,000 files ~90 s / <5 s. **No document reports measured AutoDev numbers against these targets.**

**Worktree-scoped graphs** (§13.2): a two-layer model — a stable main graph plus an in-flight worktree delta, merged on task completion with before/after snapshots and the diff stored as task metadata for QC. Depends on per-task worktrees, which `autodev-master-plan.md` §3 says are **intentionally deferred** (see §7).

### 4.5 What the docs admit is not working

This is where the graph story turns. All DOC-CLAIM.

**(a) The MCP tool surface is specified but largely unbuilt.** §10 specifies **22 tools + 5 MCP prompts**. `DEVELOPMENT_PLAN.md` Phase 6.12 covers only four task groups; **Phase 6.8 (execution-flow detection), 6.9 (change analysis), 6.10 (report generation) and 6.11 (export formats) have no section in `DEVELOPMENT_PLAN.md` at all** — they exist only in the master spec's task list (9 atomic tasks: 6.8.1, 6.8.2, 6.9.1, 6.9.2, 6.10.1, 6.10.2, 6.11.1, 6.11.2, 6.11.3). The `flows` and `snapshots` tables have DDL but no documented population path; `GRAPH_REPORT.md` is specified in detail (§7 Stage 6) and never planned; `get_graph_report()`, `get_suggested_questions()`, `get_surprising_connections()`, `refactor_tool`, `find_dead_code`, and all five MCP prompts have no build record.

**(b) The graph was structurally empty for a long time, twice.**
- `GRAPH_BUILD_FIX.md` §"Expected Behavior After Fix" reports the post-fix state as *"Nodes: 124, Edges: 0, Communities: 0"* and lists as **Next Steps**: implement tree-sitter AST parsing for function/class extraction, create edges, implement the D3 visualisation, add community detection. That is essentially the entire engine.
- `graph_fixes.md` §"Root Cause Map" Bug B, later: *"10 of 11 language extractors are file-node stubs … the graph is 1280 isolated orange squares connected by almost nothing. avg degree = 0.85 confirms this (only Python + its imports create real edges)."*

**(c) The specific defects, with the doc's own status per fix** (`graph_fixes.md`, "Active — pre-implementation", May 2026):

| Fix | Defect | Status in doc |
|---|---|---|
| G-1 | `format!("{:?}", edge.kind)` emits CamelCase `"Calls"`; the frontend expects `"CALLS"` — every edge rendered fallback gray | ✅ Done |
| G-2 | **10 of 11 language extractors were file-node-only stubs** (JS, Rust, Go, Java, C/C++, Ruby, PHP, Kotlin, Swift, Bash) — no functions, no classes, no imports, no inter-file edges | ✅ Done (all 11 implemented; `EXTRACTOR_VERSION` → 4) |
| G-3 | Edge type filter silently inert — `edgeFilters["Calls"]` is `undefined`, `undefined !== false` is `true`, so **all edges pass the filter even when "disabled"** | ✅ Done (self-healed via G-1) |
| G-4 | Single global DB regardless of workspace → multi-repo data corruption | ✅ Done |
| G-5 | `save_cached()` silently discards write errors; `load_cached` unwraps on the home dir | ✅ Done |
| G-6 | Python `from X import Y` for intra-project imports resolved to `module::X` and was filtered out as external | ✅ Done |
| G-7 | Greedy BFS clustering produced one "miscellaneous" mega-community of **285 nodes = 22% of the graph** | ✅ Done (4-phase modularity-aware algorithm) |
| G-8 | Node size used a fixed field (always 8 or 15) rather than actual degree | ✅ Done |
| G-9 | Edge CSS class embedded the raw kind, producing invalid selectors | ⬜ Self-heals with G-1 |
| G-10 | `get_graph_visualization_data` fetched the whole graph with an `i64::MAX` limit, no pagination | ✅ Done (`NODE_RENDER_LIMIT = 2000`, community-aggregated fallback) |
| G-11 | **The Python extractor had no `CALLS` edge detection at all — "impact radius and flow analysis are dead for all languages"** | ✅ Done (heuristic second pass, confidence 0.8) |
| G-12 | Build button race — `isBuilding` cleared before visualisation loaded | ✅ Done |
| G-13 | `symbolForKind` used `.includes()`, so File nodes rendered as squares identical to Class | ✅ Done |
| G-14 | `run_clustering` layering concern | ✅ Done (finding was wrong — it was already correctly placed) |

**(d) Three "Known Unknowns" that `graph_fixes.md` explicitly leaves open:**
1. Does `store.get_all_edges_for_graph()` transform `kind`, or return it as stored?
2. **Does incremental `update_graph` call `reconcile_edges()`? If it skips reconciliation, edges for changed files may accumulate duplicates after multiple updates.** (This is a silent data-corruption risk in the incremental path — the path the whole worker-integration story depends on.)
3. What to do with the abandoned pre-G-4 `~/.autodev/graph.db`.

**(e) Four graph tools still open in `TOOLS_AUDIT_2026_05_11.md`** — G-01 (misnamed semantic search), G-02 (loads all nodes on every call, O(N)), G-03 (DFS labelled BFS, non-shortest paths), G-04 (`BuildGraphTool` doesn't build).

**(f) The visualisation is diagnosed as unusable in its own spec.** `graph_improvement.md` §1 lists seven named problems: the central-mass "Big Ball of Mud", no community visual separation, no node size hierarchy, undifferentiated edge colour, all labels always visible, node shape not meaningful, no focus/selection state. `graph-3d-city.md` §1 goes further: *"A user looking at 930 nodes and 1,337 edges sees a colorful hairball, not a mental model of their codebase."*

**(g) The gap between "complete" and "working".** `DEVELOPMENT_PLAN.md` Phase 6 is **41 checked boxes, 0 unchecked**, with 6.15 and 6.16 both stamped `✅ COMPLETE` and prose confirmations. Yet a later document (`graph_fixes.md`) found that 10 of 11 extractors emitted nothing but file nodes and that `CALLS` edges did not exist in any language. **Phase 6.15's PA context injection was therefore shipped feeding a graph with almost no structure in it.** This is the single most important cautionary tale in the corpus and §8.3 generalises it.

### 4.6 Agent-visibility boundary (a deliberate, easily-lost constraint)

`graph-3d-city.md` §2 "The Dual-Layer Architecture (Critical Design Constraint)" and §19 "Agent Isolation: What Agents Never See" state that the 3D city is **exclusively a human comprehension tool** — *"a rendering skin, not a data transformation"* — and that agents receive and query the same plain graph data regardless. If the restructure carries the city view forward, this boundary must be carried with it.

---

## 5. The memory system, as specified

Sources: `PA_MEMORY_SYSTEM_SPEC.md`, `autodev-master-plan.md` §6.3, `DEVELOPMENT_PLAN.md` Phase 4.1–4.8 and §2.1, `docs/notes/core-orch-memory-namespace-fix.md`, `docs/notes/memory-ttl-config.md`, `docs/development/analysis/token-explosion-analysis.md`.

### 5.1 The intended design (AutoDev side)

**The premise**, stated identically in both the spec and the master plan: *"Replaces message compaction. The PA never forgets."* Instead of truncating old messages via a sliding window, everything is remembered and the *relevant* subset is retrieved per query.

**Tier 1 — Episodic (`PA_MEMORY_SYSTEM_SPEC.md` §3).** Raw autobiography: every user message, agent response, tool call, session metadata. Table `episodic_memories` with `session_id`, `user_id`, `agent_id`, `content`, `content_type`, **`turn_index` (1-indexed, for ordinal queries like "the third interaction")**, `timestamp`, `metadata` JSON, `embedding` BLOB, `tags`; indexed on `(session_id, turn_index)` and `timestamp DESC`. Four recall strategies: recent context, semantic search, ordinal, timeline. Auto-summarisation every 20 turns producing `{summary, key_actions, files_touched, decisions_made}`.

**Tier 2 — Semantic (§4).** Extracted facts. Table `semantic_memories` with `fact_text`, `fact_type`, `extraction_confidence`, `category`, `project_context`, `last_accessed`, `access_count`, `embedding`, **`superseded_by`** and `is_active`; plus a `fact_history` table recording `previous_value`, `new_value`, `change_reason`. Two design rules stated explicitly:
- **Extraction runs on user messages only** — *"prevents circular reasoning"* (the agent must not learn from its own output).
- Confidence threshold 0.7+ for auto-storage in the spec; `DEVELOPMENT_PLAN.md` Phase 4.2 records what shipped as **0.8+ for rule-based auto-storage, 1.0 for explicit `remember_this`** — and rule-based, not LLM-based, extraction.

**Tier 3 — Procedural (§5).** Skills and accumulated wisdom, with a deliberately different storage model: **hybrid file + SQLite**. `~/.autodev/memory/MEMORY.md` (a UI-editable index) plus topic files (`debugging.md`, `workflow-patterns.md`, `tool-preferences.md`), mirrored into `procedural_memories` for search with `source_file`, `section_heading`, `learning_type`, `applies_to`, `priority`, `is_user_edited`, `is_user_created`. Loading rules: *"first 200 lines of MEMORY.md load at session startup; topic files loaded on demand; each entry ~1 line, ~150 chars max."*

**Injection order (§6.1 `build_system_prompt`)** — five steps, in order:
1. base system prompt (per PA mode)
2. AUTODEV.md
3. MEMORY.md index (first 200 lines)
4. dynamic recall across all three tiers, scored against the current query
5. recent session context (last 10 messages)

**Recall limits:** episodic 5, semantic 10, procedural 3 (spec §6.1; confirmed shipped in `DEVELOPMENT_PLAN.md` Phase 4.4).

**Memory tools for the PA (§7.2):** `search_memory(query, memory_tier, limit, time_range)` and `remember_this`. Both marked shipped (Phase 4.6) and registered in all PA modes.

**Migration (§10).** A one-time idempotent backfill converting `sessions.db` message blobs into episodic memories with `migrated_from: "session_v1"` metadata, generating summaries for sessions >5 messages, leaving the original `sessions` table untouched — *"memory.db is additive-only"*. Marked shipped (Phase 4.8).

**Success metrics (§11):** episodic recall accuracy >90% for "what happened" queries; semantic extraction precision >80%; **combined recall latency <500 ms**. *(For contrast, the 29 April run recorded 76 "slow recall" warnings at 7–12 seconds each against a 1,000 ms threshold.)*

### 5.2 How it relates to the sidecar's memory

The relationship is **layered, and the AutoDev side is the thin layer**.

`PA_MEMORY_SYSTEM_SPEC.md` §8.1 draws it explicitly: Tauri holds the Chat UI, the MEMORY.md sidebar, and a Rust `MemoryService` wrapper; **everything semantic goes over HTTP to the sidecar's `ContextService`**, which owns the three tier implementations and talks to a Memory MCP client, which talks to mem0/Qdrant. §8.2 lists the endpoints AutoDev depends on: `POST /memory/recall`, `POST /memory/store/episodic`, `POST /memory/extract`, `GET|POST /memory/procedural`. §12 "References" points at `submodules/core-orchestrator/src/airs/memory/{episodic,semantic,procedural}.py`, `context_service.py`, and `submodules/memory-mcp-server/src/`.

`DEVELOPMENT_PLAN.md` §2.1 catalogues what the sidecar's `MemoryCore` (2,654 lines) actually contains, and the list is long: token budget / auto-degrade, adaptive L0→L3 compression by age, staleness temperature scoring, query expansion, BM25 + hybrid search, an 8-factor context scorer (R,A,C,D,P,F,X,L), an append-only event journal with BLAKE3 hashing, a contradiction detector, a ≤2 KB "continuity packet" for instant warmup, a predictive stager, and memory metrics. Cache tiers: **Hot (in-process) → Warm (Redis) → Cold (MCP/mem0)**.

§2.2 catalogues the Memory MCP Server: 6 tools (`store_memory`, `search_memories`, `get_memory`, `update_memory`, `delete_memory`, `list_memories`), a universal manager with **circuit breakers (5 failures → open, 60 s recovery) and automatic failover** across mem0 / redis / mongodb / chroma / vertex / custom providers.

**Two distinct database stories, and they are not reconciled.** `PA_MEMORY_SYSTEM_SPEC.md` §3.3/Appendix A specifies a *Rust-owned* `~/.autodev/memory.db` with full DDL for `episodic_memories`, `semantic_memories`, `fact_history`, `procedural_memories`, `session_summaries`, `memory_access_log`. §8.1's diagram routes the same data through Python to mem0/Qdrant. `02-dashboard-not-live.md` confirms `memory.db` really exists on disk. So there were, in practice, **two memory systems running side by side** — a Rust SQLite one and a Python mem0 one — and no document defines which is authoritative for what.

> **This is the single most important thing to resolve in the restructure's memory chapter.** The spec's tier model, schemas, injection order, and `MEMORY.md` hybrid store are all directly reusable and provider-agnostic. What must be *newly designed* is everything §8 delegated: embeddings and vector search, relevance scoring, compression/staleness, and recall latency — none of which has a Rust design anywhere in the corpus.

**What is genuinely lost by deleting the sidecar** (be honest about this in the plan): mem0/Qdrant vector storage, the 8-factor context scorer, adaptive compression, BM25 hybrid search, the contradiction detector, predictive staging, the Redis warm tier, and the multi-provider failover chain. `DEVELOPMENT_PLAN.md` §2.3 states the position that must now be overturned: *"What the sidecars provide (DO NOT re-implement in Rust): Agent consciousness and memory system (episodic, semantic, procedural)…"*

**What is gained:** every defect in §3.6 theme 4 (cross-boundary state duplication), the entire mem0 namespace-contamination bug class (§5.3), the Redis dependency and its cascade of failures (Issue 6, Issue C), the memory-mcp 133 MB log problem, the WinError 10054 SSE disconnects, and — per the token-explosion analysis — an unbounded retrieval path that the AutoDev side could neither see nor cap.

### 5.3 The namespace fix and the TTL note

`docs/notes/core-orch-memory-namespace-fix.md` (2026-04-29, Phase 7.10, **"Rust-side workaround applied; Python-side fix deferred"**):

- **Problem.** mem0's Qdrant store persists worker context across sessions with **no per-project scoping and no TTL**. A `working_dir` fact embedded during a run against `C:\AIRS Foundry\Backup\BACKUPcoreorchestrator` was recalled by semantic similarity in a later, different-project session. **Observed symptom: after a project switch, workers attempted to write files into the old project directory.**
- **Rust-side fix (applied).** `runner.rs` injects an authoritative `[AUTODEV WORKSPACE]` header at the start of **every** `invoke_worker()` call — *"on every tick, not just the first"* — to override any stale recalled path.
- **Python-side fix (deferred, never done).** Option A (recommended): scope the mem0 `agent_id` by an MD5 hash of `working_dir`, giving `worker-backend_a3f9c1d2`, so *"workers in project A cannot see memories from project B."* Option B: purge on project switch via a `POST /api/agents/{slug}/memory/purge` endpoint.
- Also documents a **manual flush procedure** (`Memory().delete_all(agent_id=...)` per fleet slug) and notes a `clear_worker_memory(worker_slug)` MCP tool as *"once implemented"* — i.e. never implemented.

**Three things the restructure must take from this, not discard:**
1. **The prompt-header workaround was not sufficient.** Sixteen days later, `07-workspace-escape-rootcause.md` found workers still writing outside the workspace and stated the reason: *"The `[AUTODEV WORKSPACE]` header at `runner.rs:320` is purely textual guidance … The prompt instruction had no enforcement path behind it."* The path jail is the real fix; the header was never the fix.
2. **Project-scoped namespacing is a permanent requirement**, not a mem0 quirk. Any persistent store the in-house agent uses must be scoped by workspace or the same recall-the-wrong-directory bug returns.
3. **TTL on ephemeral facts is a permanent requirement.** Facts like `working_dir` are session-scoped and must not be durable.

`docs/notes/memory-ttl-config.md` (2026-04-29, **"Documentation / deferred Python-side implementation"**) supplies the design and, usefully, its own limitations: metadata-based expiry via an `expires_at` in the payload, with the caveat repeated twice that **Qdrant does not auto-delete expired points — a cleanup job is required**; a scroll-and-delete purge job; and session-scoped namespacing as *"the most robust solution."* Its recommended-configuration table is the cleanest statement of the requirement:

| Layer | Status |
|---|---|
| Workspace header injection | ✅ Done (`runner.rs`) |
| Project-scoped `agent_id` | ⬜ Deferred |
| 24 h TTL on `working_dir` memories | ⬜ Deferred |
| Startup memory purge on project switch | ⬜ Deferred |

### 5.4 Memory as the documented cause of the token explosion

`docs/development/analysis/token-explosion-analysis.md` records a **616,937-token request** — 20× the 30,000 TPM limit — on iteration 4 of a worker tool loop. Its concrete findings implicate memory retrieval far more than the tool loop:

- **Bug #1** — `context_service.py` `get_timeline_messages` hardcodes `limit=200` while `memory_config.yaml` specifies `timeline: 10`. 200 messages ≈ 40,000 tokens.
- **Bug #2** — a `limit * 3` multiplier in recent-conversation retrieval ("fetch extra to account for filtering") silently triples the memory count.
- **Bug #3** — procedural memory stores the agent's **full 950+-character thinking block** (all five reasoning sections) every tick, and 5 of them are retrieved per tick.
- **Bug #4** — config/code mismatch: `memory_config.yaml` says `procedural: 5`, the code passes 2; timeline says 10, code passes 200.
- **Bug #5** — `recall_with_budget()` exists but **recipe workers never pass a token budget**, so they use the full model context window.

It also records that **recipe instructions (~2,000 tokens) are re-injected as a user message every single tick**, and proposes storing them as a constant system prompt instead.

> **Two permanent lessons for the in-house design, both independent of mem0:** (1) *every* retrieval path needs a hard token budget enforced at the call site, not a configured limit that the code is free to ignore; (2) do not store the agent's own reasoning as a durable, retrievable memory — it compounds.

---

## 6. Sidecar / submodule entanglement, as documented

### 6.1 What the docs say the sidecar is responsible for

`DEVELOPMENT_PLAN.md` §2.3 "Implications for AutoDev Architecture" is the canonical list, framed as *"What the sidecars provide (DO NOT re-implement in Rust)"*:

- Agent consciousness and the memory system (episodic, semantic, procedural)
- MCP client/server communication
- Multi-provider memory storage with failover
- A2A agent-to-agent communication
- Streaming with thinking detection
- **21 state machines** for lifecycle management
- Event bus with 100+ event types
- Intelligence layer (retry, HITL, failure detection)

Same section, the complement — *"What AutoDev Tauri must build"*: sidecar lifecycle management, an MCP client in Rust, **native tool execution (file I/O, bash, git) — explicitly noted as "faster than routing through Python"**, the frontend, OS keychain, session persistence, the recipe engine, GitHub integration, and desktop features.

`autodev-master-plan.md` §4.1 "What We Keep vs Strip" gives the keep-list: 21 FSMs, A2A, the SSE streaming engine, the MCP client manager with connection pooling, tool-execution FSM tracking, 3-tier hot/warm/cold memory, episodic/semantic/procedural separation, context scoring and relevance ranking, the memory algorithm system, feedback-driven synopsis evolution, MCP Wizard logic. Strip-list: Nginx, Let's Encrypt, S3, Docker Compose, YAML auth + JWT, OpenTelemetry.

`DEVELOPMENT_PLAN.md` §2.1 enumerates the 21 FSMs by name and states/tracks — `AgentLifecycleFSM`, `SessionLifecycleFSM`, `TaskExecutionFSM`, `ToolExecutionFSM`, `MemoryStoreFSM`, `MemoryRecallFSM`, `MCPServerFSM`, `MCPToolDiscoveryFSM`, `A2AServerFSM`, `A2ARequestFSM`, `WorkflowExecutionFSM`, `CacheTierFSM`, `SystemStartupFSM`, `SystemShutdownFSM`, `AgentSubprocessFSM`, `AgentConfigFSM`, `SessionRuntimeFSM`, `PromptReloadFSM`, `FileOperationsFSM`, `AuthenticationFSM`. **The restructure plan should decide, explicitly and one at a time, which of these state machines the in-house Rust agent actually needs.** Several (`AuthenticationFSM`, `A2A*`, `AgentSubprocessFSM`, `PromptReloadFSM`) plainly die with the sidecar.

**The runtime contract**, from `DEVELOPMENT_PLAN.md` §5 and the observed logs:
- core-orchestrator: FastAPI + SSE on **:4000** (`POST /api/agents`, `POST /api/agents/{id}/invoke`, `POST /api/agents/{id}/invoke/stream`, `DELETE /api/agents/{id}`, `PUT /api/agents/{name}`, `GET /health`, `POST /api/mcp/servers`, `POST /api/memory/recall`)
- memory-mcp-server: FastMCP streamable HTTP on **:5000** at `/memorymcpserver`
- Redis/Memurai on **:6379**
- autodev-mcp-server (Rust, in-process): **:8767** at `/mcp`
- Startup order: Redis → memory-mcp → core-orch, each health-gated; shutdown SIGTERM with a 5 s grace then SIGKILL

### 6.2 The fact that most changes the restructure's cost estimate

`docs/endtoendrun/6may2026/calendar6may.md` Issue B, §"Observed Behavior":

> *"Critically, the core-orchestrator LivingAgent has **zero built-in tools of its own** — `grep` across `core-orchestrator/src/**/*.py` found no native `read_file`, `bash`, `write_file`, `list_tasks`, `create_task`, or `assign_task` implementations. Everything was offloaded to the MCP server."*

And in the same section:

> *"**Mode 1 (Rust AgentEngine) is the ONLY working path** — `builtin_tools/mod.rs` registers the full tool set natively — but the fleet never reaches it."*

**Read plainly: the entire tool surface the fleet actually uses is already Rust, already in-process, already in this repo.** The sidecar contributed the agent loop, memory, FSMs, streaming, and the LLM call — not capability. That materially shrinks what "move agents in-house" has to replace.

`PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md` adds a second deflationary fact: four of the endpoints Phase 4.9 was specified against **did not exist**, and `/api/agents` is designed for *persistent personas* that write a YAML file per agent — so every ephemeral subagent spawn does file I/O in the sidecar and needs a three-layer cleanup strategy to avoid orphans. That is pure accidental complexity introduced by the boundary.

### 6.3 Documented plans and blockers around removing it

**There is no removal plan anywhere in the corpus.** Every document treats the sidecar as permanent. The nearest things to a removal record are:

1. **`ARCHITECTURE_CLARIFICATIONS.md`** — the doc that *justified keeping it*. Its conclusion (*"AutoDev is not 'one app' — it's two execution engines sharing a UI. The complexity split is intentional and necessary."*) is precisely what the restructure reverses, and the plan should say so explicitly rather than leaving the doc standing.
2. **`DEVELOPMENT_PLAN.md` §18 Open Question 4** — the only place the question is even raised: *"Core-orchestrator as sidecar vs embedded: should we compile core-orchestrator into WASM or keep as subprocess? **Recommendation**: Keep as subprocess. 2400+ line LivingAgent with litellm, asyncio, pydantic, mem0 — far too complex for WASM."* Note the framing: the alternative considered was *WASM-compiling the Python*, never *rewriting it in Rust*. The restructure's option was never on the table.
3. **`RUFLO_IMPL_PLAN.md`** header constraint — *"No changes to `submodules/core-orchestrator/` or `submodules/memory-mcp-server/`. All code lives inside `src-tauri/src/` or `src/`."* The same constraint appears in the token-explosion analysis (§9 "What Can Be Done in src-tauri Without Touching Submodules") and in `07-workspace-escape-rootcause.md` Phase 2 (*"The Python `mcp_bridge.py` injection remains out of scope for this repo (submodule)"*). **This constraint is itself the documented blocker**: several root causes were correctly diagnosed on the Python side and then explicitly left unfixed because the fixer could not or would not touch the submodule. Deleting the submodule dissolves the constraint.
4. **`master-notes.md` §"Extra Context for Phase 2"** records the operational tax: both submodules sit on a separate `autodev` branch that must be checked out and updated before work; each needs its own `.env` with API keys, with an unresolved question about whether to traverse from `src-tauri` or maintain separate files.
5. **`SUBMODULE_SETUP_GUIDE.md`** — becomes the teardown checklist (§1.1 above lists the artefacts).

**Concrete entanglement inventory for the plan** (things that break the moment the sidecars are deleted): the `[AUTODEV WORKSPACE]` prompt-header workaround and the `__working_dir`/`__session_id`/`__agent_role` meta-argument protocol · `OrchestratorClient` and the whole `sidecar/` module (watchdog, circuit breaker, health probe, TCP-alive fallback, crash banner) · `SubagentBridge` and its create→invoke→delete lifecycle plus the orphan sweep · SSE frame parsing (`find_frame_end`, `parse_event_frame`) and `invoke_worker_streaming` · `extract_usage` and its four estimation branches (`dict_split` / `dict_total_70_30` / `int_total_70_30` / `char_fallback`) · the Mode 1 / Mode 2 dispatch split and `FallbackPolicy` · `PUT /api/agents/{name}` model push · MCP server registration into core-orch · `config/agents/*.yaml` persona files (incl. `qc-agent.yaml`, edited as recently as 14 May) · Redis/Memurai lifecycle · the `analytics` sync gap · `LOG_ERROR_KINDS` `orch_crash` and `watchdog_restart` · `PA_MEMORY_SYSTEM_SPEC` §8 recall endpoints · the `SidecarCrashBanner` UI · the `sidecars` block in `tauri.conf.json` · the entire startup sequence's 13-step progress FSM.

---

## 7. Unfinished work register

### 7.1 DEVELOPMENT_PLAN phase status — the full register

Read directly from the headings and status markers.

**Marked complete or shipped:**

| Phase | Title | Marker |
|---|---|---|
| 1 | App Shell + Provider + Chat | ✅ DONE (`master-notes.md` milestone table) |
| 1 ext | Multiple sessions + rich tool display | ✅ completed |
| 2.1 | "The Vision Core" — PA creates tasks → they get done | ✅ Shipped (`autodev_codebase_dissected.md`) |
| 2.3+ | Workspace folder + auto-launch | ✅ **Implemented** |
| 2.5 | Recipe agent sandbox + native task tools + Logs tab | (deliverables listed, no explicit ❌) |
| 2.6 | Fleet sidecar integration | ✅ **Implemented** |
| 2.8.1–2.8.8 | Submodule integration bug fixes (E-01…E-11) | ✅ COMPLETE ×8 |
| 3.0 | Runtime refinements R1–R5 (push dispatch, status stream, session-bound PA, Windows 10054, cleanup cadence) | ✅ DONE |
| 3.1 | Skills + Hooks | (deliverables checklist; one item open — see below) |
| 3.2 | IDE core | ✅ Shipped |
| 3.3 | Git integration + checkpointing | **COMPLETE ✅** |
| 3.3.1 | Startup feedback + agent ID persistence | ✅ IMPLEMENTED (April 20) |
| 3.4 | MCP server + sidecar management + Python cleanup | **COMPLETE** |
| 3.5 | Plan management (planner compiler) | (deliverables checklist) |
| 3.6.1–3.6.8 | Fleet run bug fixes | ✅ COMPLETE ×8 |
| 4.1–4.6, 4.8 | PA memory (episodic, semantic, procedural, injection, auto-summary, tools, migration) | all `[x]` |
| 4.9 | Subagent system | ✅ IMPLEMENTED |
| 5.2 | AIRS Plugin | ✅ COMPLETE |
| 5.3 | Platform installers + auto-updater | ✅ COMPLETE |
| 6.1–6.7, 6.12–6.16 | Graph engine | 41 `[x]`, 0 `[ ]` |
| 6.17 | Graph engine bug fixes G-1…G-14 | (13 ✅ Done, 1 self-heals) |
| 6.18 | Graph UI advanced visualization | (present) |
| 7.1–7.10 | Run quality & stability | ✅ COMPLETE ×10 |
| 8.1–8.4 | Tools audit & hardening | ✅ COMPLETE ×4 |
| 9.1–9.8 | UI/UX v2.0 | ✅ COMPLETE ×8 |
| 10.1–10.7 | Ruflo fleet intelligence | ✅ COMPLETE ×7 |
| 11.1–11.9 | Fleet Live Monitor | ✅ COMPLETE ×9 |

**Explicitly open, deferred, or never completed:**

| Item | Phase / section | Status as written |
|---|---|---|
| Hooks integrated with Plan Management lifecycle | 3.1 deliverables | `[ ]` |
| IDE "Open Editors" section | 3.2 deliverables | `[ ]` *(deferred polish)* |
| Checkpoint compare-any-two UI | 3.3 deliverables | `[ ]` *(deferred; backend ready)* |
| Dedicated `SidecarConfig`/`process.rs`/`health.rs` split | 3.4 | `[ ]` deferred |
| Sidecar auto-restart with exponential backoff | 3.4 | `[ ]` deferred |
| Sidecar log rotation | 3.4 | `[ ]` deferred |
| Sidecar configuration editor (port / auto-start per service) | 3.4 | `[ ]` deferred |
| **Ollama: entire Settings UI surface** (provider card, model dropdown, "Local" badge, per-model `num_ctx`, temperature/top-p/seed/`num_predict` controls) | Phase X | `[ ]` ×9 — **MOOT (Ollama removed)** |
| Ollama vision / embeddings / quantization detection / thinking mode / tool streaming | Phase X | `[ ]` ×5 — **MOOT** |
| Ollama model-list 60 s cache; raw `.tools()` builder | Phase X | `[ ]` deferred — **MOOT** |
| **Memory 4.7: `context_service.py` expose memory endpoints** | 4.7 | `[ ]` |
| **Memory 4.7: Sidecar bridge (Rust → Python memory service)** | 4.7 | `[ ]` |
| **Memory 4.7: Testing — unit tests per tier, integration tests for recall** | 4.7 | `[ ]` — *the memory system shipped with no tests* |
| **Auto-create git worktree per subagent; cleanup on termination; worktree→subagent mapping** | 4.9 | `[ ]` ×3, deferred — *"Will be enabled when Workers begin spawning code-modifying subagents"* |
| Subagent tool-call components embedded inline in ChatMessages | 4.9 | `[ ]` pending |
| **Worker recipes updated to use subagents; supervisor epic breakdown via subagents; subagent cost tracking per worker** | 4.9 | `[ ]` ×3 — *"recipe authors can now USE the tools; the recipes themselves are not yet rewritten to do so"* |
| **Model Router: configurable role→model mapping** | 4 | `[ ]` |
| **Model Router UI in Settings** | 4 | `[ ]` |
| **Dispatch signal watch (FSEvents/inotify via `notify`)** | 4 | `[ ]` |
| **Dispatch signal UI in AutoDev Panel** | 4 | `[ ]` |
| Azure OpenAI provider | 4 | `[ ]` — **MOOT (out of the new provider set)** |
| **Prompt caching for all cloud providers** | 4 | `[ ]` — cost optimisation, still relevant |
| CLI: `src-shared` agent/tools/provider sharing | 5.1 | `[ ]` deferred — *CLI uses the core-orchestrator HTTP API instead* |
| CLI: tab completion, syntax highlighting, multi-line input | 5.1 | `[ ]` deferred |
| **CLI can trigger events in a running GUI instance; session sharing CLI↔GUI** | 5.1 | `[ ]` ×2 deferred |

**Missing phases (specified but never planned):** Phase 6.8 (execution flow detection), 6.9 (change analysis), 6.10 (report generation / `GRAPH_REPORT.md`), 6.11 (export formats JSON/HTML/SVG) — 9 atomic tasks from `autodev-graph-master-spec.md` §14 with **no corresponding section in `DEVELOPMENT_PLAN.md`**.

> ⚠️ **Checkbox hazard — read this before counting.** `DEVELOPMENT_PLAN.md` contains **184 unchecked `- [ ]` boxes**, but a large majority are **specification prose written in checkbox form, not open work**. Phase 5.3 is the clearest case: its "Installer Architecture" / "Installer Features" / per-platform sections are ~120 unchecked boxes describing *what an installer is*, while the actual **"Phase 5.3 Deliverables Checklist"** below them is fully `[x]` and the heading reads **✅ COMPLETE**. The rule that works: **trust the `#### Phase N.M Deliverables Checklist` blocks and the heading markers; ignore raw checkbox counts in the descriptive body.** The table above applies that rule.

### 7.2 Open items from the other status documents

| Source | Item | Status |
|---|---|---|
| `TOOLS_AUDIT_2026_05_11.md` §1 | T-05, B-01, P-03, G-01, G-02, G-03, G-04 (see §3.5) | ⬜ Open ×7 |
| `LOG_ERROR_KINDS.md` table | Emit sites marked **(planned)**: `orch_crash`, `mcp_unreachable`, `budget_exceeded`, `workspace_escape`, `qc_reviewer_failed`, `analytics_write_failed` | 6 of 9 kinds have constants but no keyed call site |
| `graph_fixes.md` §Known Unknowns | Edge-kind transform in `store.rs`; **`reconcile_edges()` in incremental update — duplicate-edge accumulation risk**; abandoned pre-G-4 `graph.db` disposition | 3 unresolved |
| `docs/endtoendrun/6may2026/calendar6may.md` | BUG-002b (pre-flight tool-capability check), BUG-003 (Memurai standalone fallback), BUG-004 (`qc:decision` event), BUG-006, BUG-007, PERF-001, PERF-002, DOCS-001 | UNKNOWN — no disposition recorded |
| `docs/notes/core-orch-memory-namespace-fix.md` / `memory-ttl-config.md` | Project-scoped `agent_id`; 24 h TTL on `working_dir`; startup purge on project switch; `clear_worker_memory` MCP tool | ⬜ Deferred ×4 |
| `docs/notes/feature-after-phase.md` | **`developer-guide.md`** ("Do First"); **Gemini path-handling bug** (fails reading files outside the working dir — *also recorded as a learned "tool preference" in the MEMORY.md example*); AutoDev-vs-Windsurf comparison; voice input, themes, plugin marketplace, prompt A/B testing, mobile companion, browser extension | Backlog, none started |
| `docs/endtoendrun/5may2026/...` §9 | **S5 — memory MCP grows unbounded within a session** | UNKNOWN — never fixed, scoped to submodule work |
| `autodev-master-plan.md` §3 | **Auto-create a fresh worktree per task** — *"intentionally deferred; Workers currently share the configured workspace"* | Deferred (roadmap) |
| `ARCHITECTURE_CLARIFICATIONS.md` §Implementation Checklist | `SessionMode` enum in Rust types; `execution_mode` field in the Recipe YAML schema; `--worker` CLI flag documented; mode selection UI in Settings | `[ ]` ×4 — **MOOT** |
| `graph-3d-city.md` §20 | Atomic build tasks for CityView | Entire feature unbuilt |
| `graph_improvement.md` | 21-section visualisation refinement spec | Partially built (Phase 6.18); no completion record |

---

## 8. Contradiction & staleness ledger

### 8.1 Direct contradictions between documents

**C1 — UI design language: dark industrial vs light glass.** `autodev-master-plan.md` §5 "Design Language" and §10 "Design System" specify `#0A0A0A` background, `#111111` surface, `#00FF88` AIRS green accent, IBM Plex Mono/Sans, **2px** border radius. `docs/development/ui-specific/ui_improvement.md` §1 and `graph_improvement.md` §3 P5 ("Dark Industrial Aesthetic") and all of `graph-3d-city.md` assume the same dark palette. `docs/restructurization/ui-anchor.md` §Colours specifies `#FFFEF7` Milky White background, `#CEF26D` Sunny Herb CTA, Urbanist/Manrope/Roboto Mono, **7px** radius on everything, and states flatly: *"Light mode only — there is no dark palette."*
→ **`ui-anchor.md` wins outright. Every design token in every other document is dead.** The plan must say this once, loudly, because the token values are scattered across at least five documents and the 3D city spec's entire material/lighting design is built on a dark ground.

**C2 — MCP server authentication: add it / remove it.** `docs/endtoendrun/5may2026/...` §13 M2 records *"✅ Done — `~/.autodev/mcp.key` + `auth_middleware` Bearer validation (L.2)"*, with the rationale *"Prevents unauthorized local access to write_file + bash."* One day later, `docs/endtoendrun/6may2026/calendar6may.md` Issue B §"Root Cause — Three Layers" calls that same middleware the fleet-wide outage: *"this auth is entirely unnecessary — the MCP server binds to `127.0.0.1:8767` (localhost only), is an in-process Tokio task inside the Tauri app, and its only client is core-orchestrator's LivingAgent (also a local child process). There is no network boundary to protect. The correct fix is to remove the auth middleware entirely."* BUG-002 is ranked **#1 in the fix priority order**.
→ **A security control added on 5 May paralysed the entire fleet on 6 May.** The 401 left every LivingAgent with an empty tool registry while every layer reported HTTP 200. Do not re-introduce auth on the in-process MCP server without a network boundary that justifies it.

**C3 — The tool-execution split, and where agent intelligence lives.** `ARCHITECTURE_CLARIFICATIONS.md` (Executive Summary and both Conclusions) and `DEVELOPMENT_PLAN.md` §2.3 assert the dual-mode doctrine: *"No duplication … Rust provider = lightweight for humans; litellm provider = full intelligence stack for workers"*, and *"What the sidecars provide (DO NOT re-implement in Rust): agent consciousness and memory, MCP client/server, A2A, streaming, 21 FSMs, event bus, intelligence layer."* `docs/endtoendrun/6may2026/calendar6may.md` Issue B contradicts the practical half of it: *"the core-orchestrator LivingAgent has **zero built-in tools of its own** … **Mode 1 (Rust AgentEngine) is the ONLY working path**."*
→ Both are true at once, which is the point: **the sidecar held the agent loop and memory; Rust already held every tool.** The restructure is a smaller lift than `ARCHITECTURE_CLARIFICATIONS.md` implies — but the memory/scoring/compression layer named in §2.3 is a real gap with no Rust design anywhere.

**C4 — Ports.** `autodev-master-plan.md` §4.1: core-orch **8765**, memory MCP **8766**. `ARCHITECTURE_CLARIFICATIONS.md`: **4000**. `DEVELOPMENT_PLAN.md` §2.1/§5.1: **4000** and **5000**. Every log line in every post-mortem: **4000**. `LOG_ERROR_KINDS.md`: autodev-mcp-server **8767**.
→ The master plan's ports are fiction. Never cite §4.1 for a port.

**C5 — Where the graph database lives.** Three answers (§4.3): `{repo}/.autodev/graph.db` (master spec §13.2) · `~/.autodev/graph.db`, shared across all projects (`GRAPH_BUILD_FIX.md`) · `~/.autodev/graphs/<sha256-12>.db`, per repo (`graph_fixes.md` G-4, latest and explicitly a corruption fix).
→ **G-4 is authoritative.** `GRAPH_BUILD_FIX.md`'s "shared across all projects" was the multi-repo corruption bug.

**C6 — Phase status: `master-notes.md` vs `DEVELOPMENT_PLAN.md`.** `master-notes.md` §"Summary Progress": Phase 1 Done · **Phase 2 IN PROGRESS** · Phases 3, 4, 5 **"Not started"**. `DEVELOPMENT_PLAN.md` marks Phases 3, 4, 5, 6, 7, 8, 9, 10 and 11 complete, and `docs/endtoendrun/14may2026/**` shows an 11-phase system running end to end.
→ `master-notes.md`'s milestone table froze around Phase 2. Its **rules** are still doctrine; its **status** is worthless.

**C7 — `TOOLS_AUDIT_2026_05_11.md` contradicts itself on completeness.** §"Implementation Status": *"All items complete."* with a table showing 6/6, 7/7, 9/9, 5/5. §1 "Summary Matrix": seven rows marked **⬜ Open** (T-05, B-01, P-03, G-01, G-02, G-03, G-04).
→ **The matrix is the truth; the banner is aspirational.** Note that four of the seven open items are graph tools that fall outside the audit's own severity tiers.

**C8 — `token-explosion-analysis.md` contradicts itself on the root cause, three times.** §Part 8 "The REAL 616K Root Cause (New Finding)": *"Not the Tool Loop! … The tool loop itself is healthy at 19.5KB."* §"The Real 616K Math": *"The 616K is from **unbounded tool result accumulation in the tool loop**."* §Conclusion: *"The 616K explosion is **NOT** from the tool loop accumulation."*
→ Treat the *evidence* (the five numbered `context_service.py` bugs, the 200-vs-10 timeline limit, the `limit*3` multiplier, the unbudgeted `recall_with_budget`) as sound, and the *causal narrative* as unresolved. The document never settled its own question.

**C9 — GitHub Issues vs the internal SQLite board.** §2.9(b) above. `autodev-master-plan.md` §9 vs §4.1.

**C10 — Agent engine language.** §2.9(a). `autodev-master-plan.md` §7 (TypeScript/WASM) vs §6.1, `DEVELOPMENT_PLAN.md` §4.1, `ARCHITECTURE_CLARIFICATIONS.md` (Rust).

**C11 — Config root and instruction filename.** §2.9(c). `~/.airs/` + `CLAUDE.md` (`autodev-master-plan.md` §8, §10) vs `~/.autodev/` + `AUTODEV.md` (§6.2, §4.1, and all observed evidence).

**C12 — Task state machine.** §2.2. Three incompatible status vocabularies across `old-multiple-fleet.md` §8, `DEVELOPMENT_PLAN.md` §4.1 SQL `CHECK`, and the observed set in every post-mortem.

**C13 — Semantic-extraction confidence threshold.** `PA_MEMORY_SYSTEM_SPEC.md` §4.4 specifies LLM extraction at **0.7+**; `DEVELOPMENT_PLAN.md` Phase 4.2 records what shipped as **rule-based extraction at 0.8+, 1.0 for explicit `remember_this`**. Minor, but the spec is the doc a reader will reach for and it is wrong about both the mechanism and the number.

**C14 — Graph context-injection budget.** `autodev-graph-master-spec.md` §13.3 says **2000 tokens (`char_budget = 6000`)**; `DEVELOPMENT_PLAN.md` Phase 6.15 says **800 tokens**. Neither cites the other.

**C15 — Where the recipe instructions should live.** `token-explosion-analysis.md` §Part 7 recommends *"Avoid recipe re-injection — store the recipe as a system prompt (constant); don't inject it as a user message each tick"*, and §Part 9 Option 2 proposes condensing 2,000 tokens to ~200. `docs/endtoendrun/14may2026/09-work-product-audit.md` moves in the **opposite direction**, adding substantial mandatory prose to both worker recipes (EXIT-echo build form, smoke probe, Step 3.0 INTERFACE.md read, Step 3.5 write_interface_spec) — and the same doc argues the reason: *"Do not try to make the LLM write better code per task … Tuning prompts to 'remember to declare your deps' is a treadmill. The fix is detection."*
→ Unresolved tension between **prompt minimisation for cost** and **prompt hardening for correctness**. The 14 May position (enforce in tools, not prose) is the more recent and better-argued one, and it implies recipes should get *shorter* as tool-level gates get stronger. **The restructure should pick a side explicitly.**

### 8.2 Staleness — documents dated enough to be suspect

| Doc | Why suspect |
|---|---|
| `docs/codebase/autodev/autodev_codebase_dissected.md` | **"Last Updated: 2025."** Claims 72 Rust / 48 TS files. Lists git, sidecar integration, graph, subagents, memory, analytics, mcp_server, observability, updater as *"Future Extensions (Phase 3.3+)"* — all shipped. Says *"Phase 2.2 ⏸️ Partial"*. **And `master-notes.md` §4 instructs every agent to read it before writing code.** This pairing actively injects a false model of the codebase into the workflow. Fix the note or delete the doc. |
| `master-notes.md` §"Summary Progress" | Frozen at Phase 2 (C6). |
| `master-notes.md` §3 "Source Reference" | Instructs agents to *"always inspect `@autodev/submodules/core-orchestrator` and `@autodev/submodules/memory-mcp-server` to validate alignment with the architecture flow"* — an instruction to consult directories that will not exist. |
| `master-notes.md` §"Current `submodules/` branch" | Pins specific commits (`82cf130`, `1c1269d`) and a working-tree status from months ago. |
| `docs/development/tools/BUILTIN_TOOLS.md` | Dated 2026-04-09, "Phase 1 Complete (7 tools)". The catalogue reached 19 (`TOOLS_AUDIT`) and then ~25. |
| `docs/development/ENVIRONMENT_SETUP.md` | Python 3.13 / uv / submodule venvs / Redis are ~half the document and all die. |
| `docs/development/GRAPH_BUILD_FIX.md` | Superseded twice (C5, §4.5b). |
| `docs/development/ARCHITECTURE_CLARIFICATIONS.md` | Its thesis is the thing being reversed. |
| `docs/development/TOOL_FLOW_ARCHITECTURE.md` | Documents the `__autodev`-suffix routing through Python. |
| `docs/development/documentation-panel/AUTODEV_DOCUMENTATION.md` | **User-facing** documentation describing sidecars, Ollama, and the graph panel. Ships to users; wrong on execution. |
| `docs/reference/ollamainrust.md` | Ollama removed. |
| `docs/submodules-management/SUBMODULE_SETUP_GUIDE.md` | Becomes a teardown checklist. |
| `docs/autodev-vision/autodev-master-plan.md` §5, §7, §8, §9, §10 | The design language, tech stack, config structure, GitHub command layer, and build prompt are all superseded — while §1–§4 and §6 remain the best statement of the product. **The plan should be explicit that this document must be cited section-by-section, never as a whole.** |
| `docs/endtoendrun/**` (all) | 14 May 2026 is the newest engineering evidence in the corpus. Anything asserted as "current" is three-and-a-half months old. |

### 8.3 What the restructure plan must NOT build on

1. **Do not treat a `✅ COMPLETE` phase marker as evidence that the thing works.** Phase 6 is 41/41 checked, with 6.15 PA context injection stamped COMPLETE — and `graph_fixes.md` later found 10 of 11 extractors emitted only file nodes and that `CALLS` edges did not exist in any language, meaning the injected "graph context" was near-empty structure. Phase 7.5/7.6 marked analytics wiring COMPLETE — and two weeks later `02-dashboard-not-live.md` found `install_global` had never been called at all and explicitly retracted the prior diagnosis. **Only a post-mortem from a real run, or a passing test, is evidence.**
2. **Do not carry `ARCHITECTURE_CLARIFICATIONS.md`'s dual-mode doctrine forward in any form** — including the `SessionMode` enum, the recipe `execution_mode` field, the `airsctl --worker` flag, or the Mode 1 / Mode 2 vocabulary that pervades the runner, the Live Monitor statuses (`mode1_active`, `fallback_idle`, `mode1_idle_no_provider`), the fallback-policy taxonomy, and `LOG_ERROR_KINDS`.
3. **Do not build on any design token, palette, font, or radius outside `ui-anchor.md`** (C1).
4. **Do not reuse the master plan's `~/.airs/` paths, port numbers, TypeScript agent engine, or GitHub-Issues-as-board sections** (C4, C9, C10, C11).
5. **Do not re-introduce bearer auth on the in-process MCP server** without a real network boundary (C2).
6. **Do not treat `autodev_codebase_dissected.md` as a map of the code**, and fix or remove `master-notes.md` §4's instruction to read it (§8.2).
7. **Do not assume the graph is a working context source.** It is a strong *design* and a partially-working *implementation* whose analysis, report, export, and flow layers were never planned (§4.5a), whose incremental path has an unresolved duplicate-edge risk, and four of whose agent-facing tools are still open defects (§3.5).
8. **Do not carry forward "prompt-as-enforcement" patterns.** The `[AUTODEV WORKSPACE]` header, "run `npm run build`", and `__depth` propagation were all text with no mechanism, and all three were violated in production (§3.6 theme 2). Every invariant needs a check that can fail.
9. **Do not assume A2A was ever load-bearing.** It is cited as a primary reason to keep the sidecar, and no run evidence shows it in use (§2.9e). All observed coordination is via the SQLite board plus the scheduler's push-kick.
10. **Do not count raw checkboxes in `DEVELOPMENT_PLAN.md` as open work** (§7.1 hazard note).
11. **Do not lose the AUTODEV.md ≠ Recipes distinction** (§2.6). Two instruction systems, two audiences.
12. **Do not assume the task state machine is settled** (C12) — three incompatible vocabularies, none matching the code.
13. **Do not carry `docs/development/documentation-panel/AUTODEV_DOCUMENTATION.md` forward unrevised** — it is user-facing and will ship a description of a system that no longer exists.
14. **Grok has zero documented precedent** anywhere in the corpus. Every provider-specific lesson (model-name canonicalisation, empty-model fallback, streaming tool-call quirks, the Gemini working-directory bug) was learned on other providers.
15. **The 14 May orchestrator crash was never root-caused.** `08-log-error-scan.md` explicitly defers it to a separate sidecar-log investigation that no document contains. If any behaviour is inherited from that run's tail, it rests on an unexplained failure.

---

## Appendix — Carry-forward list

Work that is already done, already Rust-side, already sidecar-independent, and should be **preserved rather than rebuilt** by the restructure. Each is cited above.

- **Path jail** (`validate_workspace_path`) + Windows verbatim-prefix and component-boundary handling + bash cwd and redirect-target checks + 7 tests — `14may/07` Phase 1
- **Build-evidence gate**: session-keyed bash ring with 15-min TTL; `qc_review` refused without a recent zero-exit build; QC reviewers exempt by `agent_role` — `14may/09` T2.4
- **Validation tiers 1 / 2 / 2.5 / 2.6** run inline on every `write_file`; `check_file` defaults — `14may/09` T1.1–T1.4
- **`INTERFACE.md` contract**: `write_interface_spec`, frontend Step 3.0, `tier2_6_api_contract`, `interface_drift` in `analyze_diff` — `14may/09` T3.1–T3.4
- **`e2e_probe`** + `fleet_smoke_recipe` + the QC reviewer's mandatory Phase 4 probe — `14may/09` T4.1–T4.3
- **QC fan-out**: dispatcher + ephemeral reviewers, atomic claim primitives, stranded-reviewer reconciler, `MAX_CONCURRENT_QC=6`, **$5/hour cost guard with provisional reserve** — `14may/04`, `14may/00-index` Phase 3
- **Analytics discipline**: `install_global` at startup, `expect()` instead of a silent fallback, logged write errors, per-insert debug lines, first-hit row-count diagnostic — `14may/02`
- **Live event plumbing**: lazy create-on-first-event, slug-set-hash dep, `worker:log-line` breadcrumbs, `subagent:event` bridge, `worker:tick-end` → dashboard refresh — `14may/01`, `14may/06`
- **Observability**: `LOGGING.md` (daily rotation, 14-day retention, `AUTODEV_LOG_RETENTION_DAYS`), `LOG_ERROR_KINDS.md` taxonomy + uniqueness test, stray-file scanner, workspace-mismatch banner, `runtime_config` OnceLock cache, sub-5s cron-slip demotion — `14may/08`, `14may/05`, `14may/07` Phases 3–4
- **Ruflo intelligence (Phase 10)**: semantic task routing with historical affinity, PID-based orphan detection, diff risk scoring, budget circuit breaker, epic-hierarchy decomposition, MCP health monitoring with auto-reconnect, Playwright QC skill
- **Release machinery (Phase 5.3 + CI)**: WiX/MSI, DMG + notarisation, AppImage/deb/rpm, Ed25519-signed delta auto-updater with data-persistence verification and rollback, the four CI jobs, `IT_RUNBOOK.md`, `github-actions-guide.md`
- **Name-based agent IDs** (no UUID suffix) and the startup progress FSM — `startup_sequence_and_agent_persistence_analysis.md`
- **Tool-layer fixes** T1–T14 / K.1–K.12 from the 5 May audit and the fixed items in `TOOLS_AUDIT_2026_05_11.md` — all Rust-side, all survive
- **Graph fixes G-1 … G-14** — all Rust-side; the extractors, per-repo DB keying, modularity-aware clustering, and render limit are real assets
- **Subagent resilience pattern**: RAII `Drop` cleanup + startup orphan sweep + a bridge as the only boundary — `PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md`
- **The lesson from `MODE1_FALLBACK_RECIPES.md`**: an agent whose prompt assumes a tool surface that is absent will loop on unmet preconditions rather than fail — so tool availability must be checked before dispatch, not discovered mid-run
