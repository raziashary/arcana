# AutoDev Master Notes

This notes contain notes of Razi for developing AutoDev such as rule and extra notes for each phases.

---

## Development Rule

> Every AutoDev development needs to reference `@DEVELOPMENT_PLAN.md` phase development section, while also referencing the entire (related part in the development step) `@DEVELOPMENT_PLAN.md` and `@autodev-master-plan.md` to ensure full context, information, and knowledge in developing.
>
> Every development that has finished needs to be updated in `@DEVELOPMENT_PLAN.md`. If there are major architectural or decision changes that need to be made, list them in a document in `@autodev/docs/development` and let me decide what to do about that.
>
> Every development step should also refer to the related part in `@claude-code-source-main` and `@goose-main` as the source of reference. You also need to take a look at `@autodev/submodules/core-orchestrator` and `@autodev/submodules/memory-mcp-server` submodules to ensure the architecture flow.

---

## Extra Context for Phase 2

### Phase 2 Development Notes

1. Ensure the movement of `__autodev` tools to the `submodules/core-orchestrator` is properly created especially the pathway and architecture compliance to the submodule
2. Both `submodules/` are already in a branch called `autodev`, and we will continue developing there for submodules changes, but we need to make sure that it is in the `autodev` branch before developing and update from the main branch if needed.
3. Both `submodules/` needs an API key in a .env, ensure a way to manage the .env for each submodule, either traverse it from `src-tauri` or create a separate .env for each submodule.
4. I have created a more tools in `src-tauri/builtin_tools`, please check the tools once more and see if there are any issues or improvements needed or more tools needs to be added.


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

**Milestone:** Working desktop chat app with streaming LLM responses

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **PA Mode Chat** | User can send messages, see streaming tokens, tool calls execute |
| **Provider System** | Multiple LLM providers (OpenAI, Anthropic, local) selectable in UI |
| **Session Management** | Conversations persist to SQLite, can switch between sessions |
| **Cost Tracking** | Real-time token/cost display in status bar |
| **Build Passes** | `npx tsc --noEmit = 0 errors`, `vite build` succeeds |

---

### Phase 2 — Recipes + AutoDev Panel + GitHub

**Milestone:** YAML recipe engine running + GitHub integration visible

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **Recipe Parser** | YAML recipes parse correctly with steps/triggers/schedules |
| **Recipe Runner** | Can execute recipe steps, agent processes them |
| **Cron Scheduler** | `tokio-cron-scheduler` runs recurring recipes automatically |
| **AutoDev Panel UI** | Tab shows agent roster, task board, dispatch signals |
| **GitHub Client** | Can list/create/label/comment on issues from within AutoDev |
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

**Milestone:** Multi-agent orchestration actually working

| Deliverable | What "Done" Means |
|-------------|-----------------|
| **Subagent Spawning** | Can spawn child agents with isolated tools/context |
| **Model Router** | Role-based routing (e.g., "reviewer" → GPT-4, "coder" → Claude) |
| **Dispatch Signals** | Filesystem watcher detects `.autodev/dispatch/` signals |
| **Agent Teams** | Supervisor → worker coordination pattern works |
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

| Phase | Status | Key Signifier of Completion |
|-------|--------|----------------------------|
| 1 | ✅ Done | You can chat with LLM, see streaming, costs tracked |
| 2 | 🔲 Not started | First recipe runs end-to-end |
| 3 | 🔲 Not started | Edit a file in the IDE tab |
| 4 | 🔲 Not started | Two agents coordinate on a task |
| 5 | 🔲 Not started | Install on fresh machine without dev tools |