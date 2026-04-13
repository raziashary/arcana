# AutoDev Master Notes

This notes contain notes of Razi for developing AutoDev such as rule and extra notes for each phases.

---

# AutoDev Development Rule

## 1. Context & Planning Reference

Before starting any development task, always reference the relevant phase in `@DEVELOPMENT_PLAN.md`. In addition to the specific phase, read the **entire related section** of both:

- `@DEVELOPMENT_PLAN.md` — for current scope, task breakdown, and progress tracking
- `@autodev-master-plan.md` — for overarching architecture decisions, goals, and design intent

This ensures every development step is grounded in full context and aligned with the project's direction.

---

## 2. Progress Updates

Upon completing any development step, **update `@DEVELOPMENT_PLAN.md`** to reflect what was done.

If the implementation introduces significant architectural changes or requires a decision that deviates from the existing plan, do the following:

1. Document the change or decision in a new file under `@autodev/docs/development/`
2. Clearly describe the trade-offs, implications, and available options
3. **Do not proceed** — present it for review and wait for a decision before continuing

---

## 3. Source Reference

For every development step, cross-reference the relevant portions of the following source codebases:

- `@claude-code-source-main` — Claude Code source reference
- `@goose-main` — Goose source reference

Additionally, always inspect the following submodules to validate alignment with the architecture flow:

- `@autodev/submodules/core-orchestrator`
- `@autodev/submodules/memory-mcp-server`

---

## 4. Codebase Documentation

All codebase documentation lives under `autodev/codebase/`, organized into three directories:

| Directory | Contents |
|---|---|
| `autodev/` | AutoDev codebase — `.md` for narrative explanation, `.json` for graph-based structure |
| `claude-code/` | Claude Code source — `.md` for narrative explanation, `.json` for graph-based structure |
| `goose/` | Goose source — `.md` for narrative explanation, `.json` for graph-based structure |

**Read the relevant documentation before writing or modifying any code.** Use it to understand how the codebase is organized and how components interact architecturally, so you can precisely locate the files and modules you need to work with.

Since we are developing `autodev`, naturally the Codebase will grow, and we need to make sure that the documentation is updated accordingly to reflect the current state of the codebase

---

## Extra Context for Phase 2

### Phase 2 Development Notes

0. IMPORTANT, there are major changes of vision and development plan so read the markdown files first.
1. Ensure the movement of `__autodev` tools to the `submodules/core-orchestrator` is properly created especially the pathway and architecture compliance to the submodule and there is a docs on tool tarversal in @TOOL_FLOW_ARCHITECTURE.md
2. Both `submodules/` are already in a branch called `autodev`, and we will continue developing there for submodules changes, but we need to make sure that it is in the `autodev` branch before developing and update from the main branch if needed.
3. Both `submodules/` needs an API key in a .env, ensure a way to manage the .env for each submodule, either traverse it from `src-tauri` or create a separate .env for each submodule.
4. I have created several more tools and refined some current tools in `src-tauri/builtin_tools`, so take note of that, the `create_task` tool have not been defined as it will be created in phase 2
5. In `src-tauri/src/agent/compaction.rs` there was a note to add summary compaction, make sure to check and implement it



### Current `submodules/` branch

```
Path: C:\AIRS Foundry\autodev\autodev\submodules\core-orchestrator
```

| Branch   | Commit    | Remote            | Message                              |
|----------|-----------|-------------------|--------------------------------------|
| `autodev` *(active)* | `82cf130` | `origin/autodev` | Flora API Internal Tool Integration |
| `main`   | `82cf130` | `origin/main`    | Flora API Internal Tool Integration  |

**Status:** Modified (unstaged)
- `docs/MASTER_EVENT_INTEGRATION_TRACKER.md` — modified


```
Path: C:\AIRS Foundry\autodev\autodev\submodules\memory-mcp-server
```

| Branch   | Commit    | Remote            | Message                                               |
|----------|-----------|-------------------|-------------------------------------------------------|
| `autodev` *(active)* | `1c1269d` | `origin/autodev` | Merge pull request #3 from AIRS-Foundry/Hasan---Living-Agent |
| `main`   | `1c1269d` | `origin/main`    | Merge pull request #3 from AIRS-Foundry/Hasan---Living-Agent |

**Status:** Clean — nothing to commit, working tree clean

---

## Per Phase Milestones

Based on your [DEVELOPMENT_PLAN.md](cci:7://file:///c:/AIRS%20Foundry/autodev/autodev/docs/development/DEVELOPMENT_PLAN.md:0:0-0:0), here are the **milestones** (definition of "done") for each phase:

### Phase 1 — App Shell + Provider + Chat ✅ **DONE**

**Milestone:** Working desktop chat app with streaming LLM responses + PA Mode Toggle (UI only)

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **PA Mode Chat** | User can send messages, see streaming tokens, tool calls execute |
| **PA Mode Toggle (UI)** | Fast Development ↔ Review ↔ Delegate Mode switchable in Chat header |
| **Delegate Mode (UI only)** | UI shows Delegate Mode, but no `create_task` tool yet (Phase 2) |
| **Provider System** | Multiple LLM providers (OpenAI, Anthropic, local) selectable in UI |
| **Session Management** | Conversations persist to SQLite, can switch between sessions |
| **Built-in Tools** | read_file, write_file, edit_file, bash, grep, glob, list_directory all working |
| **Cost Tracking** | Real-time token/cost display in status bar |
| **Build Passes** | `npx tsc --noEmit = 0 errors`, `vite build` succeeds |
| **Phase 1 Extensions** | Multiple Chat Sessions UI + Rich Tool Display + Grep/Glob tools — all completed |

---

### Phase 2 — Recipes + AutoDev Panel + GitHub 

**Milestone:** PA → Board → Supervisor → Worker workflow FULLY FUNCTIONAL

**The Vision Lives Here:**
```
PA (Delegate Mode) → Internal Board (SQLite) → Supervisor (polls every 2 min) 
→ Assigns to Worker recipe → Worker executes via core-orchestrator 
→ QC reviews → Supervisor merges → Task closed
```

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **Internal Task Board** | SQLite-based board with tasks table (ready/in-progress/qc-review/done) |
| **Supervisor Recipe** | Polls board every 2 min, assigns tasks to appropriate workers |
| **Worker Recipes** | Backend/Frontend/Cleanup workers execute tasks via core-orchestrator |
| **QC Agent Recipe** | Reviews worker output, approves/rejects with feedback |
| **Agent Roster UI** | Live view of Supervisor + Workers + QC with status and current task |
| **Task Board UI** | Kanban view showing tasks flowing through columns |
| **Recipe Parser** | YAML recipes parse correctly with steps/triggers/schedules |
| **Cron Scheduler** | `tokio-cron-scheduler` runs Supervisor/Worker/QC recipes automatically |
| **GitHub Integration** | Workers can create PRs, QC posts reviews, Supervisor merges |
| **Submodule Bridge** | `__autodev` tools route properly to core-orchestrator |

---

### Phase 3 — IDE + Skills + Hooks + MCP Server

**Milestone:** Full IDE experience with file editing + MCP extensibility

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **IDE Tab** | File explorer, code editor with syntax highlighting, tabs |
| **Git Integration** | Status, diff, commit, push (sub-panel, not separate tab) |
| **Skills System** | Reusable prompt workflows (like Claude Code skills) |
| **Hooks System** | Pre/post tool use hooks for customization |
| **MCP Server Mode** | AutoDev can BE an MCP server for other clients |
| **Sidecars Running** | Redis + core-orchestrator + memory-mcp-server start/manage from UI |

---

### Phase 4 — Subagents + Model Router + Dispatch 

**Milestone:** Workers can spawn subagents — Full multi-agent orchestration

**Final Vision Piece:**
```
Worker Backend (assigned by Supervisor) → Spawns subagents for parallel tasks
→ Subagents execute with isolated tools/context → Results aggregated 
→ Worker reports back to Supervisor → QC reviews → Complete
```

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **Subagent Spawning** | Workers can spawn child agents with isolated tools/context (up to 10 parallel) |
| **Agent Teams** | Full Supervisor → Worker → Subagent hierarchy works end-to-end |
| **Model Router** | Role-based routing (Supervisor → Opus, Workers → Sonnet, Cleanup → Haiku) |
| **Dispatch Signals** | Filesystem watcher detects `.autodev/dispatch/` signals |
| **Lock/Heartbeat** | Task claiming via lock files, 5-min heartbeat reporting |
| **Azure OpenAI** | Additional provider support |

---

### Phase 5 — Plugin + CLI + Installers

**Milestone:** Headless operation + distribution ready

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **`airsctl` CLI** | Command-line tool for headless agent execution |
| **AIRS Plugin** | One-click setup for "worker PCs" (non-dev machines) |
| **Platform Installers** | `.msi` (Windows), `.dmg` (macOS), `.AppImage` (Linux) |
| **Build Scripts** | Automated release pipeline |

---

### Summary Progress

| Phase | Status | Key Signifier of Completion | Vision Status |
|-------|--------|----------------------------|---------------|
| 1 |  Done | You can chat with LLM, see streaming, costs tracked | Foundation: Chat works, tools work, PA Mode Toggle UI ready |
| 2 |  **IN PROGRESS** | **PA creates tasks → Board → Supervisor → Worker → QC workflow LIVE** | **Vision Comes Alive** |
| 3 |  Not started | Edit a file in the IDE tab | Enhancement |
| 4 |  Not started | Worker spawns subagents, full hierarchy works | Vision Complete |
| 5 |  Not started | Install on fresh machine without dev tools | Distribution |