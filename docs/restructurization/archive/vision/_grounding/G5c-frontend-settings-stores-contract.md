# G5c — Frontend Audit: Settings, Stores, Contract Drift, Design System

**Scope:** `src/tabs/Settings.tsx`, `src/components/settings/`, all non-Chat non-Graph tabs and component dirs, all 28 stores, `src/types/`, `src/utils/`, `src/data/`, `src/styles/`, cross-checked against `src-tauri/src/`.
**Excluded (covered by sibling audits):** `src/tabs/Chat.tsx`, `src/components/chat/`, `src/tabs/Graph.tsx`, `src/components/city/`.
**Method:** static read-only. Every load-bearing claim carries `path:line`. Claims are labelled CONFIRMED (read the code both sides) or SUSPECTED (inference).

> STATUS: complete.

**Scale:** `src/` = 39,474 LOC · 9 tabs · 20 component directories · 28 stores · 211 unique `invoke` commands · 49 unique `listen` events, checked against 256 registered Tauri commands and 42 Rust events.

**Headline:** the Settings breakage is **not** missing commands — every Settings control invokes a command that exists. It is (a) persistence that was never written, (b) writes with no reader, and (c) controls with no wiring at all. Separately, ~3,700 LOC of frontend (memory, plugins, plan) and 47 backend commands are unreachable, and the shipped design system has **zero overlap** with `ui-anchor.md`.

---

## 1. Surface inventory

`src/` totals **39,474 lines** across 118 `.tsx`, 30 `.ts`, and 16 `.css` files. Nine tabs are registered in `src/App.tsx:39-49`, all mounted conditionally at `App.tsx:106-114` — **only the active tab is mounted**, which is load-bearing for §5.

### Tabs

| Tab | File | LOC | Stores read | Commands invoked | Events | Notes |
|---|---|---|---|---|---|---|
| Chat | `src/tabs/Chat.tsx` | 687 | — | — | `agent:done` | covered by sibling audit |
| AutoDev | `src/tabs/AutoDevPanel.tsx` | **1379** | `agentLive`, `sidecar`, `task`, `ui`, `worker` | `get_cost_today`, `clear_all_tasks` (+ via stores) | via `taskStore`/`workerStore`/`agentLiveStore` | Kanban (7 status columns) + Agent Roster + Live Monitor + Fleet Chat. Three `setInterval`s: 1 s cosmetic (`:102`), 10 s cosmetic (`:1021`), **60 s data refresh** (`:1054`) |
| IDE | `src/tabs/IDE.tsx` | 149 | `ide`, `workspace` | — (all via `ideStore`) | — | shell only; composes 8 `components/ide/*` + `GitPanel` |
| Recipes | `src/tabs/Recipes.tsx` | 268 | none | `list_recipes` | — | list + `RecipeEditor` modal; **no store**, local `useState` only |
| Logs | `src/tabs/Logs.tsx` | 559 | none | `list_log_files`, `read_log_file`, `read_log_file_full`, `clear_log_files` | — | tail poll via `intervalRef` (`:77`, `:137`); **no store** |
| Settings | `src/tabs/Settings.tsx` | **1430** | `settings`, `worker`, `workspace`, `runtimeConfig`, `modelConfig` | 15 direct + ~25 via children | — | see §3 |
| Graph | `src/tabs/Graph.tsx` | 1295 | `graph` | build/query cmds | `graph:updated`, `graph:build_progress` | covered by sibling audit |
| Dashboard | `src/components/dashboard/Dashboard.tsx` | 316 | `dashboard` | 7 analytics cmds | 4 listeners | 60 s poll + realtime; see §5 |
| Docs | `src/tabs/Docs.tsx` | 252 | none | **none** | — | pure static renderer over `src/data/docs*.ts` (1762 LOC of content) |

### Component directories (excluding `chat/` and `city/`)

| Dir | Files | LOC | What it renders | Key commands | Dead files |
|---|---|---|---|---|---|
| `autodev/` | 8 | ~1500 | Live Monitor card, stage pipeline, token sparkline, mini log tail, AUTODEV.md card+editor, Fleet Chat | `get/set/delete_autodev_instructions`, `list_recent_worker_messages`, **`navigate_to_logs` (MISSING)** | — |
| `dashboard/` | 14 | ~2600 | 13 analytics widgets + shell | `get_dashboard_summary`, `get_cost_analytics`, `get_token_usage`, `get_api_failures`, `predict_monthly_cost`, `export_costs_*`, `get_qc_reviewer_stats`, `get_worker_affinity_stats`, `get_epic_tasks`, `get_epic_phase_summary` | **`SystemHealth.tsx` (329 LOC) never imported** |
| `workers/` | 2 | 757 | Worker table with model/recipe binding; create modal | `list_workers`, `create_worker`, `delete_worker`, `update_worker_status`, `update_worker_model_sync`, `update_worker_repos`, `bind_worker_recipe`, `start/stop_worker_recipe` | — |
| `sidecar/` | 3 | 1148 | Service lifecycle, MCP health, tool browser | `get_full_sidecar_status`, `start/stop_sidecar`, `start/stop_memory_mcp`, `start/stop_orchestrator`, `get_resource_usage`, `read_sidecar_log`, `list_mcp_tools`, `get_mcp_health`, `force_reconnect_mcp`, `get_mcp_port_status` | — |
| `ide/` | 10 | ~2000 | File tree, code viewer, symbol outline, tabs, breadcrumbs, search, annotations | `read_file_tree`, `read_file_content`, `search_files`, `get_symbol_tree`, `create/delete/rename/move_file`, `create_directory`, `get/set_ide_workspace_state`, annotation CRUD | — |
| `git/` | 5 | 965 | Git panel, branches, stash, diff, checkpoints | 24 `git_*` cmds, `list/restore/delete_checkpoint` | — |
| `settings/` | 3 | 1382 | Hooks manager, update settings, submodule status | `list/add/remove/enable/disable/test_hook`, `get_hook_logs`, updater cmds, `get_submodule_paths` | — |
| `plugin/` | 3 | 1308 | Plugin manager, IT dashboard, worker setup wizard | `list_plugins`, `install_plugin*`, `uninstall/update/verify/run_plugin`, `generate/validate_setup_token` | **`PluginManager.tsx` (487) and `ITDashboard.tsx` (439) never imported** |
| `memory/` | 2 | ~470 | Memory sidebar + stats | `get_memory_stats`, `search_memory`, `recall_facts`, `get_memory_md`, topic CRUD | **`MemorySidebar.tsx` (408 LOC) never imported** |
| `plan/` | 2 | 251 | Plan tree + plan view | `get_plan`, `get_plan_tree`, `list_plans` | **both files never imported** |
| `sidebar/` | 1 | 91 | Plan card | — | **`PlanCard.tsx` never imported** |
| `skills/` | 4 | 731 | Palette, slash picker, param modal, creator | `search_skills`, `get_skill`, `execute_skill`, `validate_skill_parameters`, `reload_skills`, `skills_by_category` | — |
| `recipes/` | 1 | 617 | Recipe editor | `create/update/delete_recipe`, `list_recipe_backups`, `restore_recipe_backup`, `generate_recipe_yaml` | — |
| `budget/` | 1 | 177 | Per-task budget bar | `get_task_budget_status` | — |
| `hooks/` | 2 | 418 | Hook log viewer, info modal | `get_hook_logs` | — |
| `subagent/` | 3 | 422 | Subagent monitor, event stream, result card | `list_active_subagents`, `get_subagent_status`, `terminate/abort_subagent` | — |
| `startup/` | 1 | 94 | Boot overlay | `get_startup_phase` | — |
| `update/` | 2 | 348 | Update toast + dialog | updater cmds | — |
| `layout/` | 4 | 463 | TitleBar, TabBar, StatusBar, Sidebar | — | — |
| root loose | 4 | 580 | `ErrorBoundary`, `ToastStack`, `SidecarCrashBanner`, `WorkspaceMismatchBanner` | **`open_workspace_picker` (MISSING)** | — |

### Dead components — never imported anywhere in `src/`

Verified by import-path grep across all `.ts`/`.tsx`. **9 components, ~2,500 LOC, entirely unreachable:**

| File | LOC | What it is |
|---|---|---|
| `src/components/plugin/PluginManager.tsx` | 487 | full plugin install/verify/run UI |
| `src/components/plugin/ITDashboard.tsx` | 439 | IT/fleet admin dashboard |
| `src/components/memory/MemorySidebar.tsx` | 408 | memory topics + fact recall sidebar |
| `src/components/dashboard/SystemHealth.tsx` | 329 | system health widget |
| `src/components/plan/PlanView.tsx` | 180 | plan detail view |
| `src/components/chat/PlanDisplay.tsx` | ~130 | plan renderer for chat |
| `src/components/sidebar/PlanCard.tsx` | 91 | plan card for the sidebar |
| `src/components/plan/PlanTreeView.tsx` | 71 | plan tree |
| `src/components/chat/PaModeToggle.tsx` | ~60 | PA mode switcher |

Their CSS ships too (`PlanView.css` 323, `PlanTreeView.css` 133, `PlanCard.css` 139, `PlanDisplay.css` 138 — 733 lines of dead stylesheet).

This lines up exactly with the dead-command clusters in §6(b): **the entire plan UI is dead code**, which is why `plan:changed` has no listener; the plugin UI is dead, which is why `get_plugin_config`/`set_plugin_config`/`list_installed_plugins` are dead commands; and there is no PA-mode toggle rendered anywhere despite `set_session_pa_mode` being wired in `settingsStore.ts:155`.

## 2. Store inventory — all 28

`R` = number of non-store files importing the hook. `P` = persistence. Refresh: `M`=on-mount fetch, `E`=event-driven, `#s`=poll interval.

| Store (LOC) | State held | Backing commands | Events | R | P | Refresh | Flag |
|---|---|---|---|---|---|---|---|
| `agentLiveStore` (523) | agents, isMonitorCollapsed, lastEventAt | *none — pure event sink* | 11 listeners, **5 dead** | 3 | localStorage | E + 500 ms flush | **5 DEAD LISTENERS** |
| `agentStore` (266) | messages, isStreaming, streamingText, pendingPermission, sessionId, workingDir | *none* | `agent:token`, `tool-start`, `tool-end`, `permission-request`, `done`, `error` | 5 | — | E | ok |
| `annotationStore` (143) | byFile, activeId | 6 annotation cmds | — | 3 | backend | M | ok |
| `autodevMdStore` (171) | globalContent, repoContents, charCounts, modifiedScopes, flags | `get/set/delete_autodev_instructions` | — | 4 | backend | M | ok |
| `costStore` (34) | costTodayUsd, tokensInToday, tokensOutToday | `get_cost_today` | *(via `Chat.tsx` `agent:done`)* | 5 | — | M + E-by-proxy | desync (accumulates) |
| `dashboardStore` (424) | summary, costAnalytics, tokenUsage, apiFailures, costPrediction, dateRange, budgetAlerts, realtimeEnabled, lastRefreshedAt | 7 analytics cmds | `cost:update`, `agent:done`, `task:changed`, `worker:tick-end` | 2 | — | M + E + 60 s | mount race (§5 ST6) |
| `gitStore` (355) | repo, status, branches, log, stashes, checkpoints, worktrees, selectedDiff | 23 `git_*`/checkpoint cmds | — | 4 | — | M + refetch-after-mutation | **15 unhandled rejections** |
| `graphStore` (88) | graphData, viewMode, selectedNodeId, impactData, hoveredNodeId | *none* | *none* | 4 | — | fed by `Graph.tsx` | ok |
| `hookStore` (158) | hooks, logs, isLoading, error | 7 hook cmds | — | 3 | — | M | 5 unhandled rejections |
| `ideStore` (428) | rootTree, expandedDirs, openTabs, activeTab, searchOpen, recentFiles | 9 IDE cmds | — | 9 | backend (hand-rolled `persist()` at `:416`) | M | 5 unhandled rejections |
| `memoryStore` (258) | timeline, facts, topics, memoryMd, searchResults, stats, activeTab, 5 loading flags | 12 memory cmds | — | **2, both unreachable** | — | M + 30 s | **DEAD** |
| `modelConfigStore` (120) | configs, loadingModelId, savingModelId | `get/set/delete_model_config` | — | 1 (`Settings.tsx`) | backend | lazy per model | ok |
| `planStore` (271) | activePlan, planHistory, fleetPlans, planTree | 8 plan cmds | 5 listeners, **all dead AND no-op** | **1, unreachable** | — | M | **DEAD** |
| `pluginStore` (296) | plugins, activePlugin, activeManifest, results, setupToken, tokenClaims, filters | 10 plugin cmds | — | **3, all unreachable** | — | M | **DEAD** |
| `runtimeConfigStore` (73) | config, dirty, isLoading, error | `get_runtime_config`, `set_skip_sidecars`, `set_default_worker_budget` | — | 1 | backend JSON | M | swallows errors (§3 S4) |
| `sessionStore` (190) | sessions, activeSession, activeSessionId | 6 session cmds | — | 3 | localStorage + backend | M | `any[]` payload at `:109` |
| `settingsStore` (171) | providers, activeProvider, activeModel, systemPrompt, defaultSystemPrompt, paMode | 10 settings/vault cmds | — | 8 | localStorage (`paMode` only) + **backend NOT persisted** | M (boot only) | **§3 S1/S2/S8** |
| `sidecarStore` (318) | status, isConnected, mcpServerHealth, mcpDisconnected, mcpPortStatus | 6 sidecar cmds | 8 listeners, all live | 7 | — | E + 5 s self-capping + 10 s (panel) | ok |
| `skillStore` (147) | byCategory, recentSkills, executionHistory, paletteOpen | 6 skill cmds | `skills:reloaded` — **subscriber never invoked** | 4 | — | M | `initWatcher` dead |
| `startupStore` (170) | phases, currentIndex, currentMessage, isComplete, dismissed | `get_startup_phase` | `startup:progress` | 1 | — | E + snapshot | ok |
| `subagentStore` (258) | active, events, results, isListening, unlisten | 6 subagent cmds | `subagent:event` | 2 | — | E + 5 s | **listener never torn down** |
| `taskStore` (284) | tasks, qcQueue, workerRequests, budgetAlerts | 8 task cmds | `task:changed`, `worker:request`, `agent:budget-exceeded` | 7 | — | E + 60 s (panel) | ok |
| `toastStore` (44) | toasts | *none* | *none* | 1 | — | n/a | ok |
| `uiStore` (33) | activeTab, taskColumnPulse | *none* | *none* | 5 | — | n/a | `activeTab` not persisted |
| `updateStore` (301) | availableUpdate, isCritical, progress, history, integrityReport, 8 flags | 8 updater cmds | 5 `updater:*` | 4 | — | E + M + 500 ms | ok |
| `workerChatStore` (166) | messages, unreadByTarget, selectedTaskId, initialized | 3 worker-message cmds | `worker:message` | 3 | backend | M + E | ok |
| `workerStore` (347) | workers, isLoading, error | 9 worker cmds | `worker:status`, `tick-start`, `tick-end`, `qc:decision` | 5 | — | M + E | 8 unhandled rejections; optimistic writes |
| `workspaceStore` (65) | workspaceDir, isLoading, isSaving, error | `get/set/pick_workspace_dir` | — | 8 | backend | on demand | optimistic write (§3 S9d) |

**No store uses zustand's `persist` middleware.** CONFIRMED (`ideStore.ts:416` defines its own `persist()` helper that calls `set_ide_workspace_state` — unrelated to `zustand/middleware`).

### DEAD stores — 3 of 28, 825 LOC

Reachability was checked transitively from `src/App.tsx`, not just "is the hook imported somewhere". Three stores are imported **only** by components that are themselves never imported:

| Store | LOC | Only importers | Why unreachable |
|---|---|---|---|
| `planStore` | 271 | `src/components/plan/PlanView.tsx:2,28` | `PlanView.tsx` is never imported by anything. CONFIRMED |
| `memoryStore` | 258 | `MemorySidebar.tsx:11,17,64,68,169,302,362`, `MemoryStats.tsx:7,10` | `MemorySidebar.tsx` is never imported; `MemoryStats.tsx` is imported **only by `MemorySidebar.tsx:12`** — transitively dead. CONFIRMED |
| `pluginStore` | 296 | `ITDashboard.tsx:8,162`, `PluginManager.tsx:7,232,234`, `WorkerSetupWizard.tsx:6,172` | `ITDashboard.tsx` and `PluginManager.tsx` are never imported; `WorkerSetupWizard.tsx` is imported **only by `ITDashboard.tsx:11`** — transitively dead. CONFIRMED |

A repo-wide grep for `components/memory`, `components/plugin`, `components/plan`, and `sidebar/PlanCard` in `src/` returns **zero** import sites outside those directories themselves. **Three entire feature directories — memory, plugins, plan — plus their stores are unreachable dead code: ~3,700 LOC.**

This explains, at a stroke: the `plan:changed` dead event (§6 d1), the five dead `plan:*` listeners (§6 c6-c10), and the dead plugin/memory command clusters in §6(b).

### Polling — every interval, and whether it is cleared

| Site | Interval | Cleared? |
|---|---|---|
| `agentLiveStore.ts:498` (token flush) | 500 ms | Yes — `:520`, via `LiveMonitorSection.tsx:28`. But the handle is a **module-level singleton** (`:25`) → overlapping subscribes orphan it (§5 ST7) |
| `sidecarStore.ts:190-198` | 5 s, self-caps at 12 attempts / 60 s | **No explicit teardown.** `initEventListeners` is fire-and-forget from `App.tsx:72` and returns no handle. Mitigated only by the self-cap. SUSPECTED |
| `Dashboard.tsx:78` / `:91` | 1 s cosmetic / **60 s data** | Yes, `:95-98` |
| `LiveMonitorSection.tsx:20` | 1 s cosmetic | Yes, `:21` |
| `AutoDevPanel.tsx:102` / `:1021` / `:1054` | 1 s / 10 s cosmetic / **60 s data** | Yes |
| `QcRoiPanel.tsx:61` | 5 s | Yes, `:64-67` |
| `SidecarPanel.tsx:126` | 10 s | Yes, `:127` |
| `SubagentMonitor.tsx:42` | 5 s | Interval yes (`:47`); the `subagent:event` listener is **not** |
| `MemoryStats.tsx:14` | 30 s | Yes (but the component is dead code) |
| `UpdateDialog.tsx:76` | 500 ms during download | Yes, `:77` |

### Listener leaks

| # | Site | Mechanism |
|---|---|---|
| L1 | `src/stores/planStore.ts:73-91` | Five `listen()` calls execute **in the `create()` factory body at module load** — outside any component, effect, or cleanup path. The returned `Promise<UnlistenFn>` is never awaited or stored. Permanently un-unsubscribable. And every handler body is a bare `console.log` — **none of the five calls `set()`**, so even if Rust emitted these events the store would not update. CONFIRMED |
| L2 | `src/stores/subagentStore.ts:144-172` | `stopListening()` is defined at `:168` and **never called anywhere in `src/`**. Its only consumer, `SubagentMonitor.tsx:38-47`, cleans up its interval at `:47` but not the listener. The `isListening` guard at `:144` then blocks any re-subscribe. CONFIRMED |
| L3 | `src/App.tsx:63-79` | `initWorkerEvents`, `initTaskEvents`, `initSidecarEvents`, `hydrateWorkerChat` install listeners; the cleanup at `:78` only calls `cleanupUpdater()`. Intentional per the comment at `:67-69` (App never unmounts), but it means those four stores' subscriptions can never be torn down or re-established. SUSPECTED / by design |
| L4 | `src/components/startup/StartupOverlay.tsx:31-39` | Correct pattern, but if the component unmounts before `subscribe()` resolves, `unlisten` is still `undefined` and that listener leaks. Narrow race. SUSPECTED |

**No component in `src/` uses the `useEffect(async …)` / `return listen(…)` anti-pattern** — verified, zero matches. That specific class of leak does not exist here; the leaks above are different mechanisms.

### Module-level init guards that can never reset

Once tripped by a successful install, these are only reset inside their own `catch` block — so after a success, nothing can re-subscribe short of a page reload:
`sidecarStore.ts:82` (`_listenersInstalled`, reset only at `:315`), `workerStore.ts:120` (reset only at `:344`), `taskStore.ts:142` (`_taskListenersInstalled`, reset only at `:281`), `workerChatStore.ts:69,82` (`initialized` — **never** reset), `subagentStore.ts:144` (`isListening` — never reset, see L2).

### State that can desync from the backend

| # | Site | Mechanism |
|---|---|---|
| D1 | `planStore.ts:73-91`, `:138-153`, `:171-185` | Events discarded (L1); `updateStepStatus` and `claimPlanItem` invoke the backend and never re-fetch or patch local state. Moot while the store is dead, but it is the reason the plan UI was abandoned. CONFIRMED |
| D2 | `settingsStore.ts:142-161` | `setPAMode` writes local state + localStorage synchronously, then fires `set_session_pa_mode` in an **unawaited async IIFE** (`:150`). On failure only `console.error` (`:158`) — no rollback. UI shows a PA mode the backend never stored. CONFIRMED |
| D3 | `costStore.ts:29-33` + `Chat.tsx:296-301` | `addCost` purely accumulates `agent:done` deltas with no periodic re-sync against `get_cost_today`. One missed or duplicated event skews the header cost readout until app restart. CONFIRMED |
| D4 | `workerStore.ts:182-241` | Six mutators optimistically `set()` the assumed value after `invoke` resolves rather than using the returned value or refetching. Correction depends on a `worker:status` event arriving. SUSPECTED |
| D5 | `pluginStore.ts:184-199`, `:214-227` | Same optimistic pattern, self-documented as "Refresh installed flag optimistically". SUSPECTED (dead code) |
| D6 | `skillStore.ts:137-146` | The docstring promises auto-refresh on `skills:reloaded`, but `initWatcher` is **never called** — `grep -rn "initWatcher" src/` returns only its own definition. The palette never live-updates when skills change on disk. This is a docstring that lies about behaviour. CONFIRMED |
| D7 | `annotationStore.ts:90-138` | `setResponse`/`setTaskId`/`resolve`/`remove` patch `byFile` from client-known values; the commands return `void` so any backend-side transformation is invisible. Minor |

### Unhandled `invoke()` rejections in `src/stores/` — 48 sites

Every one of these has no `.catch` and is not inside a `try`. A failure is an unhandled promise rejection with no UI feedback.

- **`gitStore.ts` (15)** — `:270 stage`, `:275 unstage`, `:280 discard`, `:285 commit`, `:297 push`, `:302 pull`, `:307 fetch`, `:312 createBranch`, `:318 deleteBranch`, `:323 checkoutBranch`, `:328 stashPush`, `:337 stashApply`, `:342 stashDrop`, `:347 restoreCheckpoint`, `:352 deleteCheckpoint` — **every mutating git operation in the app**. A failed push or commit shows the user nothing.
- **`workerStore.ts` (8)** — `:177`, `:183`, `:192`, `:201`, `:210`, `:219`, `:226`, `:235`
- **`subagentStore.ts` (7)** — `:146` (`listen`), `:175`, `:181`, `:190`, `:200`, `:205`, `:210`
- **`annotationStore.ts` (5)** — `:68`, `:91`, `:104`, `:117`, `:130`
- **`hookStore.ts` (5)** — `:119`, `:126`, `:133`, `:139`, `:156` (enable/disable/add/remove/test — the entire Settings→Hooks mutation surface)
- **`ideStore.ts` (5)** — `:346`, `:351`, `:356`, `:366`, `:379` (create/delete/rename/move file)
- **`memoryStore.ts` (5)** — `:228`, `:236`, `:241`, `:246`, `:251`
- **`skillStore.ts` (5)** — `:101`, `:105`, `:109`, `:119`, `:123`
- **`pluginStore.ts` (3)** — `:258`, `:265`, `:273`
- **`sessionStore.ts` (2)** — `:55`, `:176`
- **`settingsStore.ts` (2)** — `:96 setActiveProvider` (this is §3 S9b / §4 M5), `:101 testProvider`

Clean: `taskStore`, `autodevMdStore`, `modelConfigStore`, `runtimeConfigStore`, `workspaceStore`, `dashboardStore`, `updateStore`, `planStore`, `sidecarStore`, `workerChatStore`, `agentLiveStore`, `agentStore`, `costStore`, `graphStore`, `toastStore`, `uiStore`.

### `any` on data crossing the Tauri boundary

| Site | What is untyped |
|---|---|
| `src/stores/agentLiveStore.ts:430` | `listen<{ subagent_id: string; event: any }>('subagent:event')` — the whole SSE payload; narrowed ad hoc on `ev.kind`/`ev.input`/`ev.output` at `:436-479` |
| `src/stores/agentLiveStore.ts:406`, `:477` | `status: mapped as any` — string→union cast with no validation |
| `src/stores/planStore.ts:74,78,82,86,90` | `(event: any)` on all five (dead) listeners |
| `src/stores/planStore.ts:123` | `invoke<any>('planner_compile')`, then `result.plan \|\| result` |
| `src/stores/sessionStore.ts:109` | `invoke<any[]>('get_session_messages')`, then `.map` over `msg.role`/`msg.tool_call_id`/`msg.tool_input` |
| `src/stores/sessionStore.ts:99` | `session.pa_mode as any` cast straight into the `PAMode` union |
| `src/stores/pluginStore.ts:166,195,207,223,236,251` | `catch (e: any)` ×6 |

Outside `src/stores/`, 106 `any` occurrences total; the notable boundary ones are `src/tabs/Graph.tsx:158,169` (`data: any[]`, `setStoreGraphData(graphData as any)`) and `src/components/plugin/WorkerSetupWizard.tsx:195` (`catch (e: any)`).

## 3. BUG-SETTINGS — owner's bug #5, "UI parts are broken on settings and more"

`src/tabs/Settings.tsx` is 1430 LOC and renders **13 panels** in one scroll column (`Settings.tsx:39-110`):
Workspace · Demo Mode · Default Worker Budget · Services (`SidecarPanel`) · Submodules · MCP Tools · Browser Testing · Providers · Model Parameters · GitHub (PAT + MCP) · Workers (`WorkerTable`) · System Prompt · Hooks · Updates.

There is no tab/section navigation, no search, and no persistence of scroll position — but those are ergonomics. The real breakage is below. **Nine distinct defects**, ordered by severity.

### S1 — Provider + model selection is never persisted (CONFIRMED, critical)

`set_active_provider` writes **only to in-memory `AppState`**:

```rust
// src-tauri/src/commands/settings_commands.rs:86-95
pub fn set_active_provider(provider: String, model: String, state: …) -> Result<()> {
    let mut s = state.lock().unwrap();
    s.active_provider = Some(provider);
    s.active_model    = Some(model);
    Ok(())
}
```

`AppState` (`src-tauri/src/state/app_state.rs:14-42`) is a plain `#[derive(Default)]` struct with **no load/save path anywhere** — grep for readers/writers returns only the command pair and the three consumers. It is constructed empty at every launch.

**User-visible failure:** the user picks Anthropic + a model in Settings, uses the app, quits. On the next launch `active_provider`/`active_model` are `None`. The Settings tab shows **no ACTIVE badge on any provider**, and the entire "Model Parameters" section is gone, because it is gated on `store.activeModel` (`Settings.tsx:72`). The first chat message then fails with `"No active provider set"` (`src-tauri/src/commands/agent_commands.rs:36`). The user must re-select a model on every single launch. This is the single largest "settings are broken" symptom.

### S2 — Custom system prompt is never persisted (CONFIRMED, high)

Same mechanism: `set_system_prompt` / `get_system_prompt` (`src-tauri/src/commands/settings_commands.rs:117-132`) read and write `AppState.system_prompt_override`, which is in-memory only. The UI copy at `Settings.tsx:1046` says *"Changes take effect on the next message"* and says nothing about the prompt evaporating on restart. The `MODIFIED` badge (`Settings.tsx:975-987`) reappears as "Using default prompt" after any relaunch, silently discarding the user's text.

### S3 — "Default Worker Budget" writes four values that nothing ever reads (CONFIRMED, high)

The card at `Settings.tsx:318-450` states verbatim: *"Applied when Supervisor assigns a task without an explicit budget. Stored in `runtime_config.json` — available to the Rust engine."* (`Settings.tsx:368-371`).

The write path is real: `set_default_worker_budget` (`src-tauri/src/commands/budget_commands.rs:59-81`) validates and persists all four fields into `runtime_config.json`. But grepping `src-tauri/src` for readers of `default_worker_budget_tokens` / `_usd` / `_turns` / `budget_warn_pct` returns **only the struct definition (`src-tauri/src/runtime_config.rs:48-56`), the defaults (`:59-62`), and the write in `budget_commands.rs:73-76`**. Zero consumers. `src-tauri/src/recipes/runner.rs` never reads them.

**User-visible failure:** four sliders/inputs that report "Saved!" and change nothing about how any worker behaves. Classic write-with-no-reader.

### S4 — The budget card reports "Saved!" even when the save failed (CONFIRMED, medium)

`Settings.tsx:342-354` wraps `cfg.setDefaultWorkerBudget(…)` in a `try/catch` and sets `saved = true` on success. But the store method swallows its own error:

```ts
// src/stores/runtimeConfigStore.ts:63-72
setDefaultWorkerBudget: async (…) => {
  try { … } catch (err) { set({ error: String(err) }) }   // never re-throws
}
```

The `catch` in `Settings.tsx:349` is therefore unreachable, and `setError` at `:327` is never populated. The same swallow-and-don't-rethrow shape is in `setSkipSidecars` (`runtimeConfigStore.ts:53-61`). The store's `error` field is rendered by the Demo Mode card (`Settings.tsx:304-308`) but **not** by the budget card, which has its own unused local `error` state. So a rejected budget save (e.g. `warn_pct > 100`, or a config-write failure) shows a green "Saved!" and no error anywhere.

### S5 — Browser Testing: two controls are pure local state, persisted nowhere (CONFIRMED, high)

`BrowserTestingCard` (`Settings.tsx:703-902`):
- **Staging URL** input (`Settings.tsx:708` state, `:852-865` input). Never sent to any command; no `invoke` touches it. The caption at `:866-868` claims *"QC Agent navigates here for visual regression screenshots on high-risk frontend PRs."* Nothing in `src-tauri/src` reads a staging URL from this path. The value resets to `http://localhost:3000` on every remount of the Settings tab.
- **"Enable browser testing for frontend PRs"** toggle (`Settings.tsx:709`, `:876-898`). `onClick` is `setBrowserEnabled(v => !v)` and nothing else. No persistence, no backend call, no consumer. It also **auto-flips itself on** whenever Playwright is detected (`Settings.tsx:716`), so it does not even reflect a user choice.

Both are decorative. The only wired controls in this card are `check_playwright_available` (`:714`) and `install_playwright_browsers` (`:726`, `:831`).

### S6 — Five of the thirteen shipped hook templates fire on events Rust never fires (CONFIRMED, high)

Rust defines 30 `HookType` variants (`src-tauri/src/hooks/types.rs:12-54`) and the TS union mirrors all 30 exactly (`src/stores/hookStore.ts:9-15`) — the type contract is clean. But only **14** are ever passed to `HookRegistry::fire`: `PreToolUse`, `PostToolUse`, `OnToolError`, `PostFileWrite`, `PreFileRead`, `SessionStart`, `SessionEnd`, `AgentLoopStart`, `AgentLoopEnd`, `OnError`, `OnTaskClaim`, `OnTaskComplete`, `OnTaskFail`, `OnQcReviewStart`.

**16 hook types are dead**: `OnPermissionRequest`, `PreFileDelete`, `OnCheckpointCreate`, `OnCheckpointRestore`, `PreCommit`, `PostCommit`, `PrePush`, `OnBranchSwitch`, `OnMergeConflict`, `PrePlanStep`, `PostPlanStep`, `OnPlanActivate`, `OnPlanComplete`, `OnPlanAbandon`, `OnQcApprove`, `OnQcFail`. All 30 are offered in the Add-Hook type dropdown (`src/components/settings/HooksSettings.tsx:153-162`).

Worse, the shipped Template Gallery (`HooksSettings.tsx:22-144`) installs hooks on dead events:

| Template | `hook_type` | Fires? |
|---|---|---|
| Block force push (`HooksSettings.tsx:34-41`) | `pre_push` | **NO** |
| Block .env file commits (`:42-50`) | `pre_commit` | **NO** |
| Clippy lint on commit (`:70-78`) | `pre_commit` | **NO** |
| Webhook on plan complete (`:79-87`) | `on_plan_complete` | **NO** |
| Log git activity (`:135-143`) | `post_commit` | **NO** |
| the other 8 | `pre_tool_use` / `post_file_write` / `on_error` / `on_task_complete` | yes |

Two of the five dead ones are **safety** hooks (`category: 'safety'`, `HooksSettings.tsx:37`, `:46`). A user installs "Block force push" and "Block .env file commits", sees them listed as ENABLED, and is protected by nothing. This is a false-assurance defect, not just a dead control.

### S7 — The Updates panel displays a literal `__APP_VERSION__` (CONFIRMED, medium)

`src/components/settings/UpdateSettings.tsx:9`:
```ts
const VERSION = '__APP_VERSION__'   // replaced by Vite at build time via define
```
`vite.config.ts:16-18` does define `'__APP_VERSION__'`, but esbuild/Vite `define` substitutes **identifiers in the AST**, not text inside string literals. `'__APP_VERSION__'` is a string literal, so it is not replaced. The "Current version" row (`UpdateSettings.tsx:57-59`) renders the raw placeholder.
Second-order fragility even if the quoting were fixed: `process.env.npm_package_version` (`vite.config.ts:17`) is only populated when Vite is launched by an npm script; a `tauri build` that invokes Vite directly falls through to the hard-coded `'1.0.0'`.

### S8 — Providers are loaded exactly once, at app boot, with no retry (CONFIRMED, medium)

`loadProviders()` is called only from `src/App.tsx:64`, inside the app-root mount effect. `Settings.tsx:30-33` deliberately does **not** call it. `settingsStore.loadProviders` swallows its failure into `console.error` (`src/stores/settingsStore.ts:88-90`) and leaves `providers: []`.

**User-visible failure:** if `list_providers` fails at boot — plausible, since it hits the OS keychain four times via `VaultManager::get` (`src-tauri/src/commands/settings_commands.rs:40`) before the keychain may be ready — the Providers section renders **empty forever**, with `isLoadingProviders === false` so not even the "Loading…" line shows (`Settings.tsx:62-70`). There is no refresh button anywhere in the section. Only saving or deleting a key re-triggers a load (`settingsStore.ts:112`, `:117`).

### S9 — Smaller wiring defects

| ID | Site | Defect |
|---|---|---|
| S9a | `Settings.tsx:1067` | `const defaultModel = provider.models[1]?.id ?? provider.models[0]?.id` — saving a key auto-activates the **second** model in the list, not the first or a named default. Arbitrary and undocumented. CONFIRMED |
| S9b | `Settings.tsx:1105-1107` | `handleSetActive` `await`s `store.setActiveProvider` with no try/catch; `setActiveProvider` (`settingsStore.ts:95-98`) has no try/catch either. A failed model switch is an **unhandled promise rejection** with zero UI feedback — the button just does nothing. CONFIRMED |
| S9c | `Settings.tsx:226-228` and `:329-331` | `DemoModeCard` and `DefaultWorkerBudgetCard` each call `cfg.load()` in their own mount effect. Two `get_runtime_config` round-trips per Settings mount, and the second `load()` resets `dirty: false` (`runtimeConfigStore.ts:47`), which can erase the "Restart AutoDev" banner. SUSPECTED (ordering-dependent) |
| S9d | `Settings.tsx:173` | "Clear workspace" calls `clearWorkspaceDir()` → `setWorkspaceDir('')` (`workspaceStore.ts:62-64`). Whether the Rust side accepts and stores an empty string, or rejects it, determines if this button works; the FE assumes success and sets `workspaceDir: ''` optimistically at `workspaceStore.ts:44` regardless. SUSPECTED |
| S9e | `Settings.tsx:596`, `:597` | `get_github_mcp_config` failure is `.catch(() => {})` — a silent swallow that leaves the card showing `enabled:false` with empty fields, indistinguishable from "not configured". CONFIRMED |
| S9f | `Settings.tsx:612` | `set_github_mcp_config` failure only `console.error`s; the button still resets to its idle label with no error surfaced to the user. CONFIRMED |
| S9g | `UpdateSettings.tsx:21`, `:112` | The update **channel** selector is local `useState` only. It is passed to `checkForUpdate(channel)` but never persisted, so it resets to `stable` on every remount — a user on `beta` silently reverts. CONFIRMED |
| S9h | `Settings.tsx:30-33`, and every `useEffect(() => {…}, [])` in the file | Empty dep arrays while closing over store methods. Harmless with Zustand's stable identities, but `Settings.tsx:1269-1271` (`[modelId]`) starts an async `loadConfig` with no cancellation — switching models fast can land a stale config into `draft`. SUSPECTED |
| S9i | `Settings.tsx:143`, `:249`, `:502`, `:626`, `:1121` and throughout | Status "dots" are `<span>`s with no `role`/`aria-label`; state is conveyed by colour alone. Toggle buttons (`Settings.tsx:876`, `:265`, `:639`) are `<button>` with no `aria-pressed` or `role="switch"`. Sliders (`:425`, `:1316`, `:1337`, `:1398`) have `title` but no `<label htmlFor>` or `aria-label`. CONFIRMED |

### Are the Settings commands real? — verification table

Every command Settings and its children invoke, checked against `src-tauri/src`:

| Command | Registered? | Panel |
|---|---|---|
| `get_workspace_dir`, `set_workspace_dir`, `pick_workspace_dir` | yes | Workspace |
| `get_runtime_config`, `set_skip_sidecars`, `set_default_worker_budget` | yes | Demo Mode / Budget |
| `get_submodule_paths` | yes | Submodules |
| `check_playwright_available`, `install_playwright_browsers` | yes | Browser Testing |
| `list_providers`, `get_active_provider`, `set_active_provider`, `test_provider_connection`, `store_provider_key`, `delete_provider_key` | yes | Providers |
| `get_model_config`, `set_model_config`, `delete_model_config` | yes | Model Parameters |
| `get_github_pat`, `store_github_pat`, `delete_github_pat`, `get_github_mcp_config`, `set_github_mcp_config` | yes | GitHub |
| `list_hooks`, `enable_hook`, `disable_hook`, `add_hook`, `remove_hook`, `test_hook`, `get_hook_logs` | yes | Hooks |
| `check_for_update`, `get_update_history`, `verify_data_integrity`, `download_update`, `install_update`, `skip_version`, `get_cached_update`, `get_download_progress` | yes | Updates |
| `get_system_prompt`, `set_system_prompt`, `get_default_system_prompt` | yes | System Prompt |

**No Settings control invokes a missing command.** The Settings breakage is *not* a missing-command problem — it is **persistence that does not exist (S1, S2), writes with no reader (S3, S6), and controls with no wiring at all (S5)**. That distinction matters for the fix plan.

---

## 4. BUG-MODELSWITCH — owner's bug #9, "changing a model must happen from the UI"

### What exists today

The full path, end to end:

1. `src/App.tsx:64` → `settingsStore.loadProviders()` (boot only).
2. `src/stores/settingsStore.ts:81` → `invoke('list_providers')` → `src-tauri/src/commands/settings_commands.rs:27-37`, which returns a **hard-coded four-element vector**: `anthropic`, `openai`, `google`, `ollama`. Each entry's `models` comes from the provider impl's `.models()` and `has_key` from a keychain probe (`settings_commands.rs:39-55`).
3. `src/stores/settingsStore.ts:84` → `invoke('get_active_provider')` → returns `Option<(String, String)>`.
4. UI: `Settings.tsx:67-69` renders one `ProviderCard` per provider. The model picker is `Settings.tsx:1228-1256` — a flat row of buttons, one per model, shown **only when `provider.has_key` is true**.
5. Click → `handleSetActive` (`Settings.tsx:1105`) → `settingsStore.setActiveProvider` (`settingsStore.ts:95`) → `invoke('set_active_provider', { provider, model })`.

### Verdict

**A model change DOES take effect without a restart — CONFIRMED.** `send_message` re-resolves the provider and model from `AppState` on *every* invocation (`src-tauri/src/commands/agent_commands.rs:33-40`), then loads the matching key from the vault (`:42-48`). Same pattern in `src-tauri/src/commands/diff_commands.rs:20-22` and `src-tauri/src/commands/recipe_commands.rs:194-196`. Nothing caches a provider client across calls.

**But the change does not survive a restart — CONFIRMED.** See §3 S1. This is the actual defect behind bug #9: it is not that switching is impossible, it is that the switch **has to be redone every launch**, which reads to a user as "the model setting doesn't work".

### Gaps

| # | Gap | Evidence |
|---|---|---|
| M1 | No persistence of active provider/model | `src-tauri/src/state/app_state.rs:14-42` — no serde load/save; `settings_commands.rs:86-106` |
| M2 | No model switcher outside the Settings tab | The only `set_active_provider` call site in all of `src/` is `settingsStore.ts:96`, reached only from `Settings.tsx:1106` and `:1075`. There is no picker in the chat composer, the status bar, or the sidebar. CONFIRMED |
| M3 | The model list is unreachable without a key | `Settings.tsx:1228` gates the model row on `hasKey`. A user cannot see or pre-select a model before entering a key. CONFIRMED |
| M4 | The provider list is not driven by the model registry the workers use | `Settings.tsx`'s providers come from Rust `list_providers` (4 providers), while `src/components/workers/WorkerTable.tsx:29-121` hard-codes a **completely different** 10-group / 40-model registry (Anthropic, OpenAI, Google, DeepSeek, xAI Grok, Moonshot/Kimi, Groq, SEA-LION, Qwen, NVIDIA NIM) sourced from `core-orchestrator/src/llm/providers/model_registry.py` (comment at `WorkerTable.tsx:26`). **Two disjoint model universes in the same Settings tab.** CONFIRMED |
| M5 | Silent failure on switch | `settingsStore.ts:95-98` has no try/catch; `Settings.tsx:1105-1107` has none either. CONFIRMED |
| M6 | Model-parameter config keys off a stale ID set | `THINKING_MODELS` (`src/stores/modelConfigStore.ts:20-26`) and `REASONING_EFFORT_MODELS` (`:29-35`) are hard-coded sets of 5 model IDs each, and they use *different* ID spellings than `WorkerTable`'s registry (e.g. `claude-opus-4-5` vs `claude-4-1-opus`). Extended-thinking and reasoning-effort controls therefore appear or vanish based on an ID string match that will silently stop matching as model IDs change. CONFIRMED |

### Ollama references in `src/` — every one, with line

The frontend has **no Ollama code path**. Ollama reaches the UI purely as data from Rust `list_providers`. Every literal reference in `src/` is in the Docs tab's static content arrays:

| # | file:line | Context |
|---|---|---|
| O1 | `src/data/docs.ts:48` | feature bullet "Model-agnostic by design (Anthropic / OpenAI / Gemini / **Ollama**)" |
| O2 | `src/data/docs.ts:51` | table row `['Model-agnostic', 'Anthropic, OpenAI, Gemini, Ollama, Azure, Bedrock', 'live']` |
| O3 | `src/data/docsAllA.ts:23` | "(Anthropic API directly or OpenAI/Gemini/**Ollama**)" |
| O4 | `src/data/docsAllA.ts:26` | "Model-agnostic: Any LLM (Anthropic, OpenAI, Gemini, **Ollama**, Azure, Bedrock)" |
| O5 | `src/data/docsAllA.ts:44` | table row `['Provider Layer', 'Anthropic, OpenAI, Gemini, Ollama, Azure', …]` |
| O6 | `src/data/docsAllA.ts:106` | "Offline-first — PA mode + **Ollama** works without internet" |
| O7 | `src/data/docsAllC.ts:35` | table row `['Ollama', 'Local models with custom endpoint']` |
| O8 | `src/data/docsMore.ts:18` | model list bullet "**Ollama** (local models, custom endpoint)" |

That is the complete frontend surface — **8 lines in 4 files, all documentation copy**. CONFIRMED (exhaustive case-insensitive grep of `src/` for `ollama`).

The Ollama *machinery* is all backend and must be removed there: `src-tauri/src/provider/ollama.rs` (506 LOC), plus references in `src-tauri/src/provider/mod.rs`, `commands/settings_commands.rs:10,34,45,60-63`, `commands/agent_commands.rs:42`, `commands/diff_commands.rs`, `commands/recipe_commands.rs`, `recipes/runner.rs`, `agent/builtin_tools/analyze_diff_tool.rs`.

### Where Grok has to be added

Grok is **not** a provider anywhere. It exists in exactly one place in `src/`: `src/components/workers/WorkerTable.tsx:77-83`, as an `<optgroup label="xAI Grok">` with two model IDs (`grok-3-mini-beta`, `grok-3-mini-beta-thinking`) — and that dropdown routes to the *worker* model field, not to the app's provider system. It has no Rust provider behind it.

Frontend work required for the four-provider target (Anthropic, OpenAI, Gemini, Grok):

| # | Location | Change |
|---|---|---|
| G1 | `src-tauri/src/commands/settings_commands.rs:30-35` | Replace the `ollama` entry with `("grok", "xAI Grok")` in `list_providers`. **This is the only change that makes Grok appear in the Settings Providers list** — the frontend list is entirely backend-driven. |
| G2 | `src-tauri/src/commands/settings_commands.rs:39-47` | Add a `"grok" => GrokProvider::new(key).models()` arm to `build_provider_info`; drop the `ollama` arm. |
| G3 | `src-tauri/src/commands/settings_commands.rs:58-83` | Add a `"grok"` arm to `test_provider_connection`; delete the `if provider == "ollama"` no-key shortcut at `:60-63`. |
| G4 | `src-tauri/src/commands/agent_commands.rs:42-48` | Delete the `if provider_id != "ollama"` key bypass — with Ollama gone, every provider requires a vault key unconditionally. |
| G5 | `src/stores/modelConfigStore.ts:20-35` | Add Grok's thinking-capable IDs to `THINKING_MODELS` (`grok-*-thinking` per `WorkerTable.tsx:81`), and decide whether Grok supports `reasoning_effort`. Without this, Grok's Model Parameters card shows only Temperature and Top-P. |
| G6 | `src/components/workers/WorkerTable.tsx:29-121` | Prune the hard-coded registry to the four supported providers — today it advertises 10 provider families (DeepSeek, Moonshot, Groq, SEA-LION, Qwen, NVIDIA NIM) that the Settings provider list does not support and no Rust provider implements. Every one of those is a selectable worker model that cannot run. |
| G7 | `src/data/docs.ts:48,51`, `src/data/docsAllA.ts:23,26,44,106`, `src/data/docsAllC.ts:35`, `src/data/docsMore.ts:18` | Rewrite the eight doc lines to "Anthropic, OpenAI, Gemini, Grok". `docsAllA.ts:106` ("Offline-first — PA mode + Ollama works without internet") describes a capability that ceases to exist and must be deleted, not reworded. |
| G8 | `src/components/city/`, `src/tabs/Graph.tsx` | Confirmed clean — no provider references. |

**Net:** the frontend needs Grok added in exactly **two** functional places (`modelConfigStore.ts` capability sets, `WorkerTable.tsx` registry) plus documentation copy. The Providers UI itself needs no change because it renders whatever `list_providers` returns.

## 5. Worker / dashboard staleness

**Is the data flow polling, event-driven, or neither? — It is all three at once, unevenly, with no single source of truth.**

### The actual topology

| Surface | Mount fetch | Poll | Events | Where |
|---|---|---|---|---|
| Dashboard | `refreshDashboard()` | **60 s** | `cost:update`, `agent:done`, `task:changed`(kind filter), `worker:tick-end` | `src/components/dashboard/Dashboard.tsx:87-99`; `src/stores/dashboardStore.ts:333-413` |
| Live Monitor | none (seeded from workerStore) | 1 s **cosmetic only** | 11 listeners, **5 of them dead** | `src/components/autodev/LiveMonitorSection.tsx:19-29`; `src/stores/agentLiveStore.ts:300-522` |
| AutoDev Panel (tasks, roster, cost) | `doRefresh()` | **60 s** | via `taskStore`/`workerStore` global listeners | `src/tabs/AutoDevPanel.tsx:1047-1056` |
| Agent Roster | — | 1 s cosmetic | `worker:status` etc. via `workerStore` | `src/tabs/AutoDevPanel.tsx:100-103` |
| Worker table | `loadWorkers()` | none | `worker:status`, `worker:tick-*`, `qc:decision` | `src/stores/workerStore.ts:254-345` |
| Sidecar panel | `refresh()` | **10 s** | `mcp:*` | `src/components/sidecar/SidecarPanel.tsx:126` |
| QC ROI panel | `fetchStats()` | **5 s** | `subagent:event` | `src/components/dashboard/QcRoiPanel.tsx:61-63` |
| Subagent monitor | — | poll | — | `src/components/subagent/SubagentMonitor.tsx:42` |
| Memory stats | — | **30 s** | — | `src/components/memory/MemoryStats.tsx:14` |
| Update dialog | — | **500 ms** | `updater:*` | `src/components/update/UpdateDialog.tsx:76` |

Every polling interval is cleared on unmount — no leaked timers from those. CONFIRMED.

### Where the staleness actually comes from — six mechanisms

**ST1 — The Live Monitor's event subscription only exists while the AutoDev tab is mounted. CONFIRMED.**
`subscribeToEvents()` is called from exactly one place: `src/components/autodev/LiveMonitorSection.tsx:27`, inside a mount effect whose cleanup tears every listener down (`:28`). `LiveMonitorSection` lives inside `AutoDevPanel`, which `App.tsx:107` renders only when `activeTab === 'autodev'`. Switch to Chat, Settings, or Dashboard and **all 11 worker/agent listeners are unregistered**. Every tick, status change, and log line that happens while you are on another tab is lost permanently — there is no backfill fetch on remount (`initAgents` only seeds empty shells, `agentLiveStore.ts:124-132`). Come back to AutoDev and the monitor shows the state from before you left plus a `lastEventAt` that is minutes old, which the header renders as the **STALE** badge (`LiveMonitorSection.tsx:41`, `:60`). This is the most likely direct cause of the owner's report.

**ST2 — Five of the six `agent:*` listeners the Live Monitor depends on are never emitted. CONFIRMED.**
See §6(c). `agent:tool-call-start`, `agent:tool-call-result`, `agent:stage-change`, `agent:token-update`, `agent:status-change` (`agentLiveStore.ts:305-323`) have no Rust emitter. This is not cosmetic — it disables four whole features of the card:
- `handleToolCallStart`/`Result` (`agentLiveStore.ts:160`, `:204`) are the **only** writers of `log_lines` from the main agent path, so the card body stays empty except for `worker:log-line` breadcrumbs and subagent-derived lines.
- `handleStageChange` (`:227`) is the **only** writer of `stages` — the pipeline strip on every card is permanently frozen at its initial state.
- `handleTokenUpdate` (`:246`) is the **only** writer of `tokenBuffer`, which the 500 ms flush timer (`:498-516`) drains into `total_tokens` and `sparkline`. With no producer, **the sparkline is a flat line and the token counter never moves**, while a `setInterval` runs forever doing nothing.
- `handleStatusChange` is reachable, but only via the `worker:*` bridge.

**ST3 — `lastEventAt` — the thing that literally decides LIVE/STALE/IDLE — has almost no producers. CONFIRMED.**
`lastEventAt` is initialised to `Date.now()` at store creation (`agentLiveStore.ts:121`) and bumped in `handleToolCallStart` (`:176`), `handleToolCallResult` (`:208`), `handleStatusChange` (`:265`), `handleError` (`:285`), and the `worker:log-line` handler (`:375`). Of those, the first two are fed only by dead events (ST2). So in practice the badge is driven by `worker:tick-start` / `worker:tick-end` / `worker:status` alone. Note `handleStageChange` (`:227-243`) and `handleTokenUpdate` (`:246-248`) do **not** bump `lastEventAt` — even if their events were wired, a worker streaming tokens for ten minutes would still be flagged STALE.

**ST4 — `initAgents` destroys lazily-registered agents. CONFIRMED.**
```ts
// src/stores/agentLiveStore.ts:124-132
initAgents: (agentIds) => {
  const existing = get().agents
  const agents: Record<string, AgentLiveState> = {}
  agentIds.forEach(id => { agents[id] = existing[id] ?? makeInitialAgent(id) })
  set({ agents })          // ← replaces the whole map
}
```
It rebuilds the map from the passed slug list, so any record created lazily by an event — a subagent slug like `qc-rev-task42-abc123` from `subagent:event` (`agentLiveStore.ts:429-495`), or any worker whose `worker:tick-start` arrived before `loadWorkers()` resolved — is **deleted**. `AutoDevPanel.tsx:1078-1082` re-runs `initAgents` whenever `workerSlugKey` or `workerModelKey` changes, i.e. on every worker rename, model change, or roster reload. Live subagent cards vanish mid-run.

**ST5 — Plan state is push-blind end to end. CONFIRMED.**
`planStore.ts:74-90` subscribes to five `plan:*` events Rust never emits, while Rust's real `plan:changed` (16 emit sites) has no listener (§6 c6-c10 and d1). The plan sidebar and Plan tab therefore update **only** when something else triggers a manual reload. A plan advancing through steps in the background is invisible until the user navigates away and back.

**ST6 — Dashboard realtime subscription has a mount race. SUSPECTED (mechanism confirmed, trigger depends on StrictMode/timing).**
`Dashboard.tsx:89` calls `startRealtimeUpdates()` **without awaiting** it, and the cleanup at `:97` calls `stopRealtimeUpdates()`. Inside the store, the idempotency guard is `if (get().realtimeEnabled) return` (`dashboardStore.ts:334`) but `realtimeEnabled` is not set to `true` until **after** all four `await listen(...)` calls resolve (`:409`). Under React 18 StrictMode's mount→unmount→mount, the second `startRealtimeUpdates()` sees `realtimeEnabled === false` and proceeds, producing **eight** registered listeners; `realtimeUnlisteners` (`:403`) is then overwritten by whichever call finishes last, orphaning four unlisteners that `stopRealtimeUpdates` (`:415-419`) can never call. Symptom: `refreshDashboard()` fires twice per event, and the leaked set survives tab switches — a slow accumulation of duplicate refreshes.

**ST7 — `tokenFlushTimer` is a module-level singleton that leaks on re-subscribe. CONFIRMED.**
`let tokenFlushTimer` (`agentLiveStore.ts:25`) is assigned unconditionally at `:498` on every `subscribeToEvents()` call. The cleanup (`:520`) clears whatever the variable currently holds. Two overlapping subscribes (tab switch during an in-flight mount, or StrictMode) overwrite the handle and orphan the first interval, which then runs for the life of the process.

### Verdict

The dashboard and live monitor go stale because **the push channel they were designed around does not exist**, and the polling that was bolted on to compensate is (a) 60 s, (b) only running while the owning tab is mounted, and (c) absent entirely for the Live Monitor, which has **no fetch-on-mount path at all**. Fixing the five dead `agent:*` events and the `plan:changed` mismatch removes most of the symptom; hoisting `subscribeToEvents` to `App.tsx` alongside the other global `init*EventListeners` calls (`App.tsx:70-77`) removes the tab-switch amnesia; and giving the Live Monitor a real backfill command would remove the rest.

## 6. Contract drift (frontend ↔ Rust)

**Extraction method (reproducible):**
- FE commands: every `invoke(<T>)?('name'…)` literal in `src/**/*.ts{,x}` → **241 call sites, 211 unique command names** (extraction is multi-line-aware; a same-line-only regex under-counts by 2 because of multi-line generic type arguments). There are **zero** dynamic/variable `invoke()` calls, so the literal set is complete. CONFIRMED.
- Rust commands: `#[tauri::command]` fn names → 253 unique; the registration list in `src-tauri/src/lib.rs:167-470` (`tauri::generate_handler![…]`) → **256 registered entries**. CONFIRMED.
- FE events: every `listen(…)` literal (incl. multi-line forms) → **49 unique event names**.
- Rust events: `events.rs:29-113` constants (32) + 10 direct string emits, filtered to those with a real emit call site.

### (a) `invoke()` calls with NO matching Rust command — hard runtime failures

**Count: 2.** Both are silently swallowed, so the user sees nothing happen at all.

| # | FE call site | Command | Evidence |
|---|---|---|---|
| a1 | `src/components/autodev/AgentLiveCard.tsx:110` | `navigate_to_logs` | No `fn navigate_to_logs` anywhere in `src-tauri/src`, `src-cli`, `src-shared`. Call is `invoke('navigate_to_logs', { filterAgent: agentId }).catch(() => {})` — the `.catch(() => {})` swallows the "command not found" error, so the "view logs" affordance on every agent card is a **no-op with zero feedback**. CONFIRMED. |
| a2 | `src/components/WorkspaceMismatchBanner.tsx:85` | `open_workspace_picker` | No Rust fn of that name (the real one is `pick_workspace_dir`, registered at `lib.rs`). Also `.catch(() => {})`. The "set workspace" CTA on the workspace-mismatch banner is a **no-op**. CONFIRMED. |

Two near-misses that are NOT bugs (my first-pass `#[tauri::command]` grep missed them because of attribute/argument formatting, but both exist and are registered): `get_task_budget_status` (`src-tauri/src/commands/budget_commands.rs:38`) and `set_default_worker_budget` (`src-tauri/src/commands/budget_commands.rs:59`).

### (b) Registered Rust commands never invoked from `src/` — dead backend surface

**Count: 47** of 256 registered commands (18%). Verified twice: absent from the multi-line-aware `invoke()` extraction AND absent as a quoted string anywhere in `src/`.

```
analyze_diff                       get_orphaned_tasks              list_repos
analyze_pr_diff                    get_plugin_config               list_skills
break_down_epic                    get_provider_key                merge_all_repos_cmd
check_autodev_instructions_exists  get_recipe                      register_repo
cmd_set_task_budget                get_session                     resolve_symbol
compare_checkpoints                get_system_logs                 restart_sidecar
create_checkpoint                  get_task                        save_memory_md
create_phased_task                 get_user_preferences            search_file_contents
find_symbol_references             get_worker_analytics            search_procedural
force_release_task                 get_worker_by_slug              search_semantic
get_autodev_instructions_char_count git_blame                      set_plugin_config
get_daily_budget_summary           git_clone                       skills_by_category
get_file_metadata                  git_init                        suggest_worker_for_task
get_graph_communities              git_worktree_add                summarize_session_cmd
get_graph_edges_for_viz            git_worktree_remove             unregister_repo
get_graph_nodes_for_viz            import_skill
                                   list_installed_plugins
```

Corrections to a naive grep: `skills_by_category` (`src/stores/skillStore.ts:93`) and `update_worker_model_sync` (`src/components/workers/WorkerTable.tsx:175`) ARE invoked — through multi-line generic type arguments — so they are **not** dead. `get_task` appears once at `src/stores/agentLiveStore.ts:96` but only as an agent *tool* name inside a string array; it is never invoked. Remove `skills_by_category`/`update_worker_model_sync` from the block above; the true count is 47.

Notable clusters, because they are whole features with a backend and no UI:
- **Checkpoints** — `create_checkpoint`, `compare_checkpoints` are dead, yet `restore_checkpoint`, `list_checkpoints`, `delete_checkpoint` ARE called from `src/components/git/CheckpointViewer.tsx`. The UI can list/restore/delete checkpoints it can never create. CONFIRMED.
- **Multi-repo graph** — `register_repo`, `unregister_repo`, `list_repos`, `merge_all_repos_cmd` all dead: no repo-registration UI exists.
- **Budget** — `cmd_set_task_budget` and `get_daily_budget_summary` dead; only `get_task_budget_status` / `set_default_worker_budget` are wired. Budgets can be read but never set per task.
- **Orphan detection** — `get_orphaned_tasks` and `force_release_task` dead; `OrphanAlert.tsx` is event-only, so on a cold mount it shows nothing until an event happens (see §5).
- **Diff risk scoring** — `analyze_diff` / `analyze_pr_diff` both dead, yet `src/components/dashboard/DiffAnalysisCard.tsx` exists (see findings).
- **Skills** — `list_skills`, `skills_by_category`, `import_skill` dead; only `search_skills`/`get_skill`/`execute_skill`/`reload_skills`/`validate_skill_parameters` are used.
- **Semantic routing** — `suggest_worker_for_task` dead: the "route this task to the right worker" feature has no UI entry point at all.

### (c) FE `listen()` with no Rust `emit` — dead listeners

**Count: 10.**

| # | FE site | Event | Why dead |
|---|---|---|---|
| c1 | `src/stores/agentLiveStore.ts:305` | `agent:tool-call-start` | Not in `events.rs`, no emit anywhere. CONFIRMED |
| c2 | `src/stores/agentLiveStore.ts:309` | `agent:tool-call-result` | idem. CONFIRMED |
| c3 | `src/stores/agentLiveStore.ts:313` | `agent:stage-change` | idem. CONFIRMED |
| c4 | `src/stores/agentLiveStore.ts:317` | `agent:token-update` | idem. CONFIRMED |
| c5 | `src/stores/agentLiveStore.ts:321` | `agent:status-change` | idem. CONFIRMED |
| c6 | `src/stores/planStore.ts:74` | `plan:created` | Rust emits only `plan:changed` (`events.rs:56`). CONFIRMED |
| c7 | `src/stores/planStore.ts:78` | `plan:updated` | idem. CONFIRMED |
| c8 | `src/stores/planStore.ts:82` | `plan:step_completed` | idem. CONFIRMED |
| c9 | `src/stores/planStore.ts:86` | `plan:item_claimed` | idem. CONFIRMED |
| c10 | `src/stores/planStore.ts:90` | `plan:subplan_created` | idem. CONFIRMED |

This **confirms and completes** the sibling audits' finding. The naming style is itself the tell: Rust uses kebab-case (`tool-start`, `tick-end`), while `plan:step_completed` / `plan:item_claimed` / `plan:subplan_created` use snake_case — they were never contracted with the Rust side.

Note `agentLiveStore.ts:325` listens `agent:error`, which **is** emitted (`events.rs:35`, `emit_agent_error`). So 5 of the 6 `agent:*` listeners in that store are dead, not all 6.

### (d) Rust `emit` with no FE `listen()` — dead events

**Count: 3.**

| # | Rust site | Event | Consequence |
|---|---|---|---|
| d1 | `src-tauri/src/events.rs:56`, emitted from **16** `emit_task_changed`-style sites via `emit_plan_changed_global` (10 call sites) + `emit_plan_changed` (6 call sites) | `plan:changed` | **The one plan event Rust actually fires has no listener.** Combined with c6–c10, the plan UI is push-blind end to end. CONFIRMED |
| d2 | `src-tauri/src/events.rs:30`, emitted `src-tauri/src/agent/engine.rs:436` | `agent:thinking` | Extended-thinking tokens are streamed to a UI that never subscribes; thinking blocks never render. CONFIRMED |
| d3 | `src-tauri/src/commands/graph_commands.rs:862` | `ide:navigate` | `open_in_ide` fires a navigate event the IDE tab never hears; "open in IDE" from the graph does not move the IDE tab. CONFIRMED |

Also dead but harmless: `emit_budget_exceeded_global` (`events.rs:494`) and `emit_worker_log_line_global` (`events.rs:265`) are defined with **0 call sites** — dead helper functions, not dead events (the non-`_global` variants are used).

### (e) TypeScript types vs Rust structs

The brief's premise needs one correction: `src-tauri/src` carries **38** `#[serde(rename_all = …)]` attributes, and **two of them ARE camelCase** — `src-tauri/src/db/annotations.rs:13` (`Annotation`) and `:31` (`CreateAnnotationInput`). Everything else is `snake_case`, `lowercase`, `PascalCase`, or `SCREAMING_SNAKE_CASE`. So the boundary is **mostly** snake_case with two deliberate camelCase exceptions — and the frontend gets both right.

| Type | Rust | TS | Verdict |
|---|---|---|---|
| `Annotation` / `CreateAnnotationInput` | `db/annotations.rs:12-41`, **camelCase** | `src/stores/annotationStore.ts:8-32`, camelCase (`filePath`, `startLine`, `taskId`, `createdAt`) | **MATCH** — the exception is handled correctly. CONFIRMED |
| `HookType` (30 variants) | `hooks/types.rs:11-54`, `snake_case` | `src/stores/hookStore.ts:9-15`, all 30 in snake_case | **MATCH** on names. But 16 of the 30 are never fired (§3 S6) — a *semantic* drift, not a shape drift. CONFIRMED |
| `Task` | `db/tasks.rs:173-…`, snake_case | `src/stores/taskStore.ts:24-61` | **PARTIAL** — TS omits `claimed_by_pid` and `claim_host` (Rust `db/tasks.rs:191-193`). The orphan-detection UI therefore cannot show which PID/host holds a task, which is exactly what `get_orphaned_tasks` (dead command) would have surfaced. CONFIRMED |
| `TaskStatus` (10) | `db/tasks.rs:139-151`, snake_case | `taskStore.ts:10`, all 10 present incl. `build_failed`, `needs_info` | **MATCH**. CONFIRMED |
| `BudgetExceededEvent` | `events.rs:481-488`, snake_case | listened at `taskStore.ts:256-263` with a snake_case inline type, then explicitly re-mapped to a camelCase `BudgetExceededAlert` (`taskStore.ts:265-273`) | **MATCH** — correctly handled, an example of the right pattern. CONFIRMED |
| `Worker` / `WorkerStatus` / `WorkerType` | `db/workers.rs:15-80+`, `lowercase` + `snake_case` | `src/stores/workerStore.ts:12-35` | **MATCH** on shape. Note `WorkerType` is `lowercase` in Rust but typed as bare `string` in TS (`workerStore.ts:16`), so the 8-variant enum is unenforced client-side. Minor |
| `DiffAnalysisResult` | `diff_analysis/mod.rs:17-29`, snake_case | `src/components/dashboard/DiffAnalysisCard.tsx:3-14` | **MATCH** on field names. `classification` is a **PascalCase** enum in Rust (`diff_analysis/classifier.rs:11-19`: `Feature`, `Bugfix`, …) but typed as bare `string` in TS (`DiffAnalysisCard.tsx:6`) — works, but unenforced. And both producing commands (`analyze_diff`, `analyze_pr_diff`) are dead (§6 b), so the card only ever renders a JSON blob written into `Task.diff_analysis` by the agent tool. CONFIRMED |
| `AgentToolCallStartPayload`, `AgentToolCallResultPayload`, `AgentStageChangePayload`, `AgentTokenUpdatePayload`, `AgentStatusChangePayload` | **NO RUST COUNTERPART AT ALL** | `src/types/live.ts:89-121` | **ORPHAN TYPES.** Five carefully specified payload interfaces (33 lines) describing five events Rust never emits. `src/types/live.ts` is 128 lines, ~26% of which types a contract that does not exist. CONFIRMED |
| `AgentErrorPayload` | `events.rs:…` (`agent:error` is emitted) | `src/types/live.ts:123-128` | Real event; TS declares `tool_call_id?` and `is_fatal` — needs a Rust-side field check to confirm. SUSPECTED |
| `RuntimeConfig` | `runtime_config.rs:39-57`, snake_case, `#[serde(default)]` on every field | `src/stores/runtimeConfigStore.ts:15-22` | **MATCH** — all 5 fields present and correctly snake_case. CONFIRMED |
| `ProviderInfo` / `ModelInfo` | `commands/settings_commands.rs:17-24` + `provider/types.rs` | `src/stores/settingsStore.ts:29-44` | **MATCH** (`has_key`, `context_window`, `cost_in_per_1m`, `cost_out_per_1m`). CONFIRMED |
| `ModelConfig` | `commands/model_config_commands.rs` | `src/stores/modelConfigStore.ts:10-17` | **MATCH** on read (snake_case `model_id`, `top_p`, `reasoning_effort`, `thinking_enabled`, `thinking_budget`). Note the **write** side uses camelCase argument names (`modelConfigStore.ts:86-93`: `modelId`, `topP`, `reasoningEffort`, …) — correct, because Tauri camelCases *command arguments* while serde controls *payload* shape. Two different conventions in one file, easy to get wrong. CONFIRMED |

**Summary of (e): the shape contract is in better condition than the event contract.** Real mismatches are: two `Task` fields missing on the TS side, five entirely orphaned payload types in `src/types/live.ts`, and three enums typed as bare `string` client-side (`WorkerType`, `classification`, `HookExecutionLog.hook_type`). The dangerous drift in this codebase is not field casing — it is **events and commands that do not exist on the other side**.

## 7. Design-system gap vs `docs/restructurization/ui-anchor.md`

### Headline

**The shipped UI and the anchor have essentially zero overlap.** Not "drifted" — *disjoint*. The anchor specifies a light-only, 7px-radius, Urbanist/Manrope/Roboto-Mono system with a 34-class `.airs-*` vocabulary. The code ships a dark-only, 2px-radius, IBM-Plex system with none of that vocabulary. This is a rewrite, not a refactor.

### What the anchor mandates (`ui-anchor.md`)

- **Palette**: Milky White `#FFFEF7` (bg), Sunny Herb `#CEF26D` (CTA), Ice Blue `#8DC2FF`, Stellar White, Nebula Purple `#3A2055`, Orbital Blue `#2B4C75`, Deep Space Navy `#0E1131`, + 6 secondary pastels. App tokens `--app-background`, `--app-surface`, `--app-surface-2`, `--app-border` (`ui-anchor.md:26`).
- **"Light mode only — there is no dark palette."** (`ui-anchor.md:28`)
- **Radius**: everything 7px; `--radius-sm` = 2px; `--radius-pill: 9999px` **reserved for status dots only**; explicitly not pill buttons (`ui-anchor.md:74-78`).
- **Fonts**: Urbanist (UI), Manrope (buttons only), Roboto Mono (**technical values only** — JSON/IDs/timestamps/logs) (`ui-anchor.md:60-70`).
- **Motion tokens**: `--ease`, `--motion-hover` 180 ms, `--motion-return` 290 ms, `--motion-panel` 290 ms, `--motion-tab` 360 ms. Banned: bouncy spring, cursor tracking, large spins, auto-changing content, idle movement, heavy connector lines, shake-on-error (`ui-anchor.md:80-94`).
- **34 `.airs-*` / named classes** (`ui-anchor.md:98-108`).

### 7.1 Token diff

**0 of 11 anchor tokens exist in `src/`.** Grepping `--app-background`, `--app-surface`, `--app-surface-2`, `--app-border`, `--ease`, `--motion-hover`, `--motion-return`, `--motion-panel`, `--motion-tab`, `--radius-pill`, `--radius-sm` returns nothing. Grepping every anchor hex (`FFFEF7`, `CEF26D`, `8DC2FF`, `3A2055`, `2B4C75`, `0E1131`, …) and every anchor colour name returns nothing. CONFIRMED.

All actual `:root` tokens live in exactly one place — `src/styles/globals.css:8-67`, **48 custom properties**, headed by the comment "AutoDev Industrial Dark" (`globals.css:7`):

| Group | Tokens |
|---|---|
| Backgrounds (11) | `--bg-app` `--bg-surface` `--bg-card` `--bg-code` `--bg-darker` `--bg-2` `--bg-input` `--bg-selected` `--bg-hover` `--bg-panel` `--bg-1` |
| Borders (5) | `--border-primary` `--border-secondary` `--border-1` `--border-2` `--border` |
| Accent/status (20) | `--accent` `--accent-muted` `--accent-border` `--accent-dim` `--accent-rgb` `--aqua` `--success` `--danger` `--danger-muted` `--warning` `--warning-muted` `--info` `--info-muted` `--purple` `--purple-muted` `--gold` `--green-add` `--red-del` `--add-bg` `--del-bg` |
| Text (9) | `--text-primary` `--text-secondary` `--text-muted` `--text-dim` `--text-bright` `--text-1` `--text-2` `--text-3` `--text-4` |
| Font/radius (3) | `--font-mono` `--font-sans` `--radius` |

There are no same-name/different-value collisions with the anchor, because **no name is shared at all**.

### 7.2 Dark palettes — the sibling audit is CONFIRMED, and it is worse than described

Two dark palettes exist under **overlapping key names with different hex values**:

**Palette A — "AutoDev Industrial Dark"** (`globals.css:7`). This is what actually renders, because `globals.css` is the only file defining real `:root` custom properties and every CSS file consumes them via `var(--x)`.

| Token | Value | Line |
|---|---|---|
| `--bg-app` | `#0f0f17` | `globals.css:10` |
| `--bg-surface` | `#161622` | `globals.css:11` |
| `--bg-card` | `#1e1e2e` | `globals.css:12` |
| `--accent` | `#00FF88` | `globals.css:31` |
| `--danger` | `#FF4444` | `globals.css:38` |
| `--warning` | `#FFAA00` | `globals.css:40` |
| `--info` | `#4477FF` | `globals.css:42` |
| `--text-primary` | `#aaaaaa` | `globals.css:53` |

**Palette B — "Winter is Coming"** (`tailwind.config.js:10`), defined as Tailwind `theme.extend.colors` — reachable only through utility classes like `bg-bg-app`, never through `var()`.

| Key | Value | Line |
|---|---|---|
| `bg-app` | `#011627` | `tailwind.config.js:11` |
| `bg-surface` | `#00233B` | `tailwind.config.js:12` |
| `bg-card` | `#01111D` | `tailwind.config.js:13` |
| `accent` | `#82AAFF` | `tailwind.config.js:22` |
| `danger` | `#FF6363` | `tailwind.config.js:27` |
| `warning` | `#FFCB8B` | `tailwind.config.js:28` |
| `info` | `#C792EA` | `tailwind.config.js:29` |
| `text-1` | `#D6DEEB` | `tailwind.config.js:32` |

Same keys, different colours. A component that writes `className="bg-bg-card"` and one that writes `style={{background:'var(--bg-card)'}}` render **different colours** while appearing to mean the same thing. CONFIRMED.

**Third layer of confusion:** four component stylesheets carry a stale `Winter-is-Coming theme` header comment while consuming Palette-A variables — `src/components/startup/StartupOverlay.css:1`, `src/components/subagent/SubagentEventStream.css:1`, `src/components/subagent/SubagentMonitor.css:1`, `src/components/subagent/SubagentResultCard.css:1`. The comments are wrong about what they render.

**Theming machinery:** `@media (prefers-color-scheme: dark)` and `[data-theme]` have **zero occurrences in `src/`**. There is no light variant and no toggle — dark is unconditional and hard-coded. CONFIRMED.

### 7.3 Radius

**`7px` appears zero times in `src/`** — no `border-radius: 7px`, no `borderRadius: 7`, no `rounded-[7px]`. The anchor's single most-repeated rule has **0% implementation**. CONFIRMED.

What actually drives radius: `globals.css:66` sets `--radius: 2px`, and `tailwind.config.js:41-44` overrides only `borderRadius.DEFAULT` to `'2px'` inside `theme.extend`. Because that override does not touch `sm`, bare `rounded` (2px, overridden) and `rounded-sm` (2px, Tailwind stock `0.125rem`) resolve to **the same value** — so the codebase's dominant class carries no meaning.

**CSS literal `border-radius:` — 104 occurrences across 14 files:**

| Value | Representative |
|---|---|
| `var(--radius)` (2px) | `globals.css:143`, `Dashboard.css:48`, `widgets.css:6`, `StartupOverlay.css:26` |
| `1px` | `Graph.css:480`, `LiveMonitor.css:308` |
| `2px` | `Graph.css:38`, `LiveMonitor.css:111`, `Annotations.css:213` |
| `3px` | `PlanDisplay.css:39`, `PlanCard.css:68` |
| `4px` | `Annotations.css:53`, `PlanTreeView.css:36` |
| `6px` | `PlanDisplay.css:9`, `PlanTreeView.css:19`, `Graph.css:913` |
| `8px` | `SubagentMonitor.css:8`, `PlanView.css:6` |
| `10px` | `SubagentMonitor.css:95` |
| `50%` | `Graph.css:242`, `Annotations.css:12,21,30`, `LiveMonitor.css:155`, `PlanCard.css:32` |

**Tailwind `rounded-*` in `.tsx` — 230 occurrences:**

| Class | Resolves to | Count |
|---|---|---|
| `rounded` (bare) | 2px | 106 |
| `rounded-sm` | 2px (identical) | 75 |
| `rounded-md` | 6px | 4 |
| `rounded-lg` | 8px | 20 |
| `rounded-xl` | 12px | 3 |
| `rounded-full` | 9999px | 22 |

**Inline `borderRadius:` JS props — 218 occurrences across 48 files**: `2`/`'2px'` ×84, `4`/`'4px'` ×43, `3`/`'3px'` ×42, `50%` ×17, `6` ×15, plus ad-hoc 8/9/10/12/14/20/99 in `PluginManager.tsx`, `WorkerSetupWizard.tsx`, `ITDashboard.tsx`.

**Pill violations** — `rounded-full` on controls, not status dots, which the anchor explicitly forbids: `src/components/layout/TitleBar.tsx:23,30,37` (window controls), `src/components/layout/TabBar.tsx:52` (a tab), `src/components/budget/BudgetBar.tsx:109,111,143,145,160,162` (progress bar track and fill), `src/components/update/UpdateDialog.tsx:155,157`. CONFIRMED.

**Distinct radius values in production: at least 12** (1, 2, 3, 4, 6, 8, 9, 10, 12, 14, 20, 50%/9999). The anchor allows 3 (7px, 2px, pill-on-dots).

### 7.4 Fonts

`index.html:10-13` and `globals.css:1` load **only** `IBM Plex Mono` (400/500) and `IBM Plex Sans` (400/500) from Google Fonts.

**`Urbanist`, `Manrope`, and `Roboto Mono` have zero occurrences anywhere in the repo** — `src/`, config files, `index.html`. CONFIRMED. All three anchor fonts are simply absent.

Actual tokens: `--font-mono: 'IBM Plex Mono', monospace` (`globals.css:64`), `--font-sans: 'IBM Plex Sans', sans-serif` (`globals.css:65`), duplicated in `tailwind.config.js:37-40`. One orphan: `Graph.css:1043` requests `'JetBrains Mono', 'Fira Code', monospace`, neither of which is loaded — it silently falls back to system monospace.

**Monospace is used far beyond "technical values":** the `font-mono` utility appears **374 times across 60 files**. Page titles (`Dashboard.css:22`, `.dashboard-header h1`), widget section headers (`widgets.css:23`), and essentially every label and button in `Graph.css` are monospace. The entire Settings tab is monospace body copy (`Settings.tsx` uses `className="font-mono text-xs"` on nearly every text node). The anchor restricts mono to JSON/IDs/timestamps/logs.

### 7.5 `.airs-*` and the named-class vocabulary

**`.airs-*` — 0 of 4 exist.** Grepping `\.airs-` across `src/` returns nothing: `.airs-card`, `.airs-card-nested`, `.airs-badge`, `.airs-mono` are all absent. CONFIRMED.

Across the anchor's full 34-class vocabulary, **exactly 1 class exists**: `.status-dot` (`src/components/sidebar/PlanCard.tsx:64`, `PlanCard.css:29,36,40,44`) — and even that carries state through `dot-green`/`dot-blue`/`dot-dim` modifiers rather than the anchor's `complete|running|failed|waiting`. And `PlanCard.tsx` is a **dead component** (never imported, §8), so in the running app the count is effectively 0.

Missing entirely: `.glass-strong`, `.btn` (only ad-hoc `.btn-primary`/`.btn-secondary`/`.btn-danger`/`.btn-small`/`.btn-refresh`/`.btn-export` exist, e.g. `Graph.css:48-51`, `Dashboard.css:95-96`), `.btn-run-pipeline`, `.status-pill`, `.metric-chip`, `.filter-chip`, `.product-header`, `.product-logo`, `.product-title`, `.dev-tool-banner`, `.module-nav`, `.module-tab`, `.viz-sidebar`, `.viz-sidebar-item`, `.viz-main`, `.matrix-grid`, `.matrix-card`, `.stage-header`, `.stage-item-row`, `.inspector-tabs`, `.inspector-tab`, `.inspector-body`, `.detail-panel`, `.lineage-row`, `.lineage-stage`, `.lineage-rel`, `.lineage-db-tag`, `.audit-analysis`, `.code-preview`, `.cytoscape-container`, `.section-title`, `.section-sub`, `.sidebar-status`. **33 of 34 missing.**

### 7.6 Motion

- **12 `@keyframes`**: `globals.css:156` (`spin`), `:161` (`shimmer`), `:166` (`fadeInUp`), `:171` (`pulse-dot`); `Graph.css:834`; `LiveMonitor.css:80,84,251,418,521`; `StartupOverlay.css:16,131`.
- **Anchor motion tokens — none of `--ease` / `--motion-*` exist.** Every duration in the codebase is a magic number.
- **Infinite spins**: `globals.css:176` (`.anim-spin`, 1 s), `Graph.css:844` (32 px, 0.8 s), `LiveMonitor.css:416` (8 px, 0.8 s), `StartupOverlay.css:128` (900 ms), plus `Logs.tsx:441` and `WorkerTable.tsx:265`. All are icon-scale loaders — borderline against "large spins", flagged for judgment rather than as a hard violation.
- **Idle movement — SUSPECTED violation**: the LIVE/STALE badges pulse continuously while nothing is happening (`LiveMonitor.css:76-87`), and health dots animate at rest. The anchor bans idle movement.
- **Cursor tracking**: exactly one instance in the whole tree, `src/components/city/CityView.tsx:110` (`onMouseMove` → `setCursorPos`). Inside the excluded 3D scene; noted, not counted.
- **Clean**: no `framer-motion`, no `react-spring`, no `useSpring` (not in `package.json` either); no shake-on-error; no auto-rotating content.
- **`prefers-reduced-motion`: zero occurrences in `src/`.** CONFIRMED. None of the spin/pulse animations degrade. This is an accessibility gap as well as an anchor gap.

### 7.7 Is Tailwind actually used?

**Yes — genuinely used, but only for half the job.** `globals.css:3-5` carries `@tailwind base; @tailwind components; @tailwind utilities;`, so it compiles and ships.

| Metric | Count |
|---|---|
| `.tsx` files under `src/` | 118 |
| …using `className=` | 82 |
| …using Tailwind-utility-shaped classes (`flex`, `px-`, `gap-`, `rounded-`, …) | 53 |
| …referencing `var(--…)` anywhere | 67 |
| …using `var(--…)` inside inline `style={{}}` | 62 |

The two sets **overlap heavily** — `Settings.tsx`, `HooksSettings.tsx`, and `SidecarPanel.tsx` are all in both. The pattern throughout is: Tailwind for layout/spacing/radius, hand-rolled inline `style={{ color: 'var(--text-2)', background: 'var(--bg-card)' }}` for every colour. Tailwind's own colour utilities are almost never used — which is why Palette B in `tailwind.config.js` is largely inert despite being configured. **Two parallel styling systems in the same files**, which is the structural reason the palettes could diverge unnoticed.

### 7.8 Undefined CSS custom properties

2,421 `var(--…)` references across 82 files, diffed against the 48 tokens defined at `globals.css:8-67`.

**Referenced, never defined, and with NO fallback — these render as invalid and fall back to inherited/initial values:**

| Property | Refs | Representative sites |
|---|---|---|
| `--accent-color` | 13 | `GrepResultsDisplay.tsx:58,66,76`, `BashOutputDisplay.tsx:28`, `GlobResultsDisplay.tsx:108,115`, `FileContentDisplay.tsx:27,43,44,45,55,56,57` |
| `--text-tertiary` | 9 | `GrepResultsDisplay.tsx:69,100`, `BashOutputDisplay.tsx:43,105`, `GlobResultsDisplay.tsx:123,126,138`, `FileContentDisplay.tsx:33,78` |
| `--surface-bg` | 8 | `MessageBubble.tsx:215`, `DirectoryListingDisplay.tsx:43`, `ToolCallDisplay.tsx:124`, `GrepResultsDisplay.tsx:40`, `SessionBar.tsx:60`, `BashOutputDisplay.tsx:21`, `GlobResultsDisplay.tsx:44`, `FileContentDisplay.tsx:20` |
| `--error` (bare form) | 8 of 15 | `AutoDevPanel.tsx:350`, `stores/taskStore.ts:106`, `AutodevMdCard.tsx:28`, `AutodevMdEditor.tsx:117,200,246,251`, `Recipes.tsx:190` |
| `--bg-3` (bare form) | 4 of 7 | `UpdateSettings.tsx:133,164,217`, `SubagentMonitor.css:54` |
| `--accent-1` / `--accent-2` / `--accent-3` | 2/2/1 | `UpdateSettings.tsx:119,120`, `SubmoduleStatusPanel.tsx:40,42,44` |
| `--error-1` / `--error-1-alpha` | 2/1 | `SubmoduleStatusPanel.tsx:46,110` |
| `--bg-raised` (bare form) | 2 of 3 | `WorkerAffinityPanel.tsx:113`, `OrphanAlert.tsx:50` |
| `--bg-base` | 2 | `Recipes.tsx:174`, `AutodevMdEditor.tsx:212` |
| `--error-color` | 1 | `SessionBar.tsx:143` |
| `--input-bg` | 1 | `GlobResultsDisplay.tsx:62` |
| `--green` | 1 | `WorkerAffinityPanel.tsx:22` |
| `--border-0` | 1 | `WorkerAffinityPanel.tsx:129` |
| `--bg-0` | 4 | `SubmoduleStatusPanel.tsx:40,42,44,46` |

**Referenced, never defined, but guarded with a fallback — degrade to the literal colour, lower severity:**
`--bg-elevated` (11: `Settings.tsx:860,881`, `SidecarPanel.tsx:243,310,344,414,566,578`, `McpToolBrowser.tsx:80,96,131`), `--bg-deep` (4), `--yellow` (2), `--red` (1), `--font-ui` (2: `SubagentEventStream.css:8`, `SubagentMonitor.css:10`), `--danger-bg` (2: `ErrorBoundary.tsx:54,102`), `--aqua-muted` (1), `--orange-950`/`--orange-700`/`--blue-950`/`--blue-700` (1 each: `UpdateSettings.tsx:77,78`).

**~22 distinct undefined custom properties.** The sibling audit's four (`--surface-bg`, `--accent-color`, `--text-tertiary`, `--error-color`) are CONFIRMED and cluster entirely in `src/components/chat/*Display.tsx` — 31 references across 8 files — which strongly suggests that whole family was written against an earlier token scheme and never reconciled with `globals.css`. `SubmoduleStatusPanel.tsx` is the worst single file outside chat: **every** colour it uses (`--accent-1/2/3`, `--error-1`, `--error-1-alpha`, `--bg-0`, `--bg-1`) is undefined, so the badges at `SubmoduleStatusPanel.tsx:40-46` render with no background at all.

One correction to note: `--border-primary` (14 refs) **is** defined at `globals.css:24` — not a defect.

### 7.9 Bottom line for the restructure

| Dimension | Anchor | Shipped | Gap |
|---|---|---|---|
| Theme | light only | dark only, unconditional, **two conflicting dark palettes** | total |
| Tokens | 11 named | 0 present, 48 unrelated, ~22 undefined-but-referenced | total |
| Radius | 7 / 2 / pill-on-dots | 12+ distinct values, 7px never used, pills on buttons | total |
| Fonts | Urbanist / Manrope / Roboto Mono | IBM Plex Sans / IBM Plex Mono; mono used for body copy | total |
| Class vocab | 34 named | 1 present (in a dead component) | 33/34 missing |
| Motion tokens | 5 named | 0; magic numbers; no `prefers-reduced-motion` | total |
| Styling model | one system | Tailwind utilities + inline `var(--…)` in the same files | structural |

## 8. Findings table

Severity: **P0** = users hit it every session / silent data or safety loss · **P1** = a feature is broken or dead · **P2** = degraded, misleading, or fragile · **P3** = hygiene.
Class: `persist` · `wiring` · `contract` · `staleness` · `deadcode` · `design` · `quality` · `a11y`.

| ID | file:line | Sev | Class | What it does | Why it is wrong | User-visible failure | Status |
|---|---|---|---|---|---|---|---|
| F01 | `src-tauri/src/state/app_state.rs:14-42`; `commands/settings_commands.rs:86-95` | **P0** | persist | Stores active provider + model | `AppState` has no load/save path; it is `#[derive(Default)]` and rebuilt empty every launch | Model selection is lost on every restart; first chat message errors `"No active provider set"` (`agent_commands.rs:36`) | CONFIRMED |
| F02 | `src-tauri/src/commands/settings_commands.rs:117-132` | **P0** | persist | Stores the custom system prompt | Same in-memory-only `AppState` field | A saved custom prompt silently reverts to default on restart; UI copy at `Settings.tsx:1046` never warns | CONFIRMED |
| F03 | `src/components/settings/HooksSettings.tsx:34-41,42-50,70-78,79-87,135-143` vs fired set in `src-tauri/src/agent/**` | **P0** | wiring | Ships 13 hook templates; 5 target `pre_push`/`pre_commit`/`post_commit`/`on_plan_complete`, which `HookRegistry::fire` is never called with | 16 of 30 `HookType` variants have zero fire sites | "Block force push" and "Block .env file commits" install, show ENABLED, and protect nothing — false safety assurance | CONFIRMED |
| F04 | `src/stores/planStore.ts:73-91` + `src-tauri/src/events.rs:56` | **P0** | contract | Listens for 5 `plan:*` events; Rust emits only `plan:changed` (16 sites) | Names were never contracted; handlers are `console.log` only and never call `set()` | Plan state never updates from push; combined with F13 the whole plan feature is inert | CONFIRMED |
| F05 | `src/stores/agentLiveStore.ts:305-323` | **P0** | contract | 5 `agent:*` listeners with no Rust emitter | Only `agent:error` (`:325`) is real | Live Monitor card body empty, pipeline strip frozen, token sparkline flat, token counter stuck at 0 | CONFIRMED |
| F06 | `src/components/settings/HooksSettings.tsx` + `src/tabs/Settings.tsx:318-450`; `src-tauri/src/runtime_config.rs:48-56` | **P0** | wiring | "Default Worker Budget" persists 4 values to `runtime_config.json` | **No Rust code ever reads them back** — zero consumers outside the struct and the write | 4 controls that report "Saved!" and change nothing; UI text at `Settings.tsx:369-370` claims otherwise | CONFIRMED |
| F07 | `src/components/autodev/LiveMonitorSection.tsx:26-29` + `src/App.tsx:107` | **P0** | staleness | Installs all 11 worker/agent listeners on `LiveMonitorSection` mount, tears them down on unmount | `AutoDevPanel` is unmounted whenever another tab is active; there is no backfill fetch on remount | Every tick, status change and log line that happens on another tab is lost; the monitor shows a STALE badge with a minutes-old timestamp | CONFIRMED |
| F08 | `src/components/memory/*`, `src/components/plugin/*`, `src/components/plan/*`, `src/components/sidebar/PlanCard.tsx` + `planStore`/`memoryStore`/`pluginStore` | **P1** | deadcode | Three complete feature directories + 3 stores | Nothing in the reachable tree imports them (transitive check from `App.tsx`) | ~3,700 LOC ships in the bundle and is unreachable; memory, plugins, and plan have backends but no UI | CONFIRMED |
| F09 | `src/tabs/Settings.tsx:708-709,852-898` | **P1** | wiring | "Staging URL" input and "Enable browser testing" toggle | Pure `useState`; no `invoke`, no persistence, no consumer. The toggle auto-flips on when Playwright is detected (`:716`) | Two controls that reset on every remount and configure nothing, under a caption claiming the QC agent uses them | CONFIRMED |
| F10 | `src/stores/gitStore.ts:270,275,280,285,297,302,307,312,318,323,328,337,342,347,352` | **P1** | quality | 15 mutating git operations `invoke` with no `.catch` and no `try` | Unhandled promise rejections | A failed `git push`, `commit`, `checkout`, or `restoreCheckpoint` produces **no error anywhere in the UI** — the button just appears to do nothing | CONFIRMED |
| F11 | `src/tabs/Settings.tsx` (whole file) + `globals.css` + `tailwind.config.js` vs `docs/restructurization/ui-anchor.md` | **P1** | design | Renders a dark, 2px-radius, IBM-Plex, monospace-body UI | Anchor mandates light-only, 7px radius, Urbanist/Manrope/Roboto Mono, `.airs-*` vocabulary | 0/11 anchor tokens, 0 uses of 7px, 0/3 anchor fonts loaded, 1/34 anchor classes present (in a dead file) | CONFIRMED |
| F12 | `tailwind.config.js:9-51` vs `src/styles/globals.css:8-67` | **P1** | design | Two dark palettes under overlapping key names (`bg-app`, `accent`, `danger`, `warning`, `info`) with different hex values | `className="bg-bg-card"` and `style={{background:'var(--bg-card)'}}` render different colours | Inconsistent colour across components that appear to use the same token | CONFIRMED |
| F13 | `src/components/autodev/AgentLiveCard.tsx:110` | **P1** | contract | `invoke('navigate_to_logs', …).catch(() => {})` | No Rust command of that name exists anywhere | "View logs" on every agent card is a no-op with the error swallowed | CONFIRMED |
| F14 | `src/components/WorkspaceMismatchBanner.tsx:85` | **P1** | contract | `invoke('open_workspace_picker').catch(() => {})` | No such command; the real one is `pick_workspace_dir` | The "set workspace" CTA on the mismatch banner does nothing | CONFIRMED |
| F15 | `src/components/settings/UpdateSettings.tsx:9` | **P1** | wiring | `const VERSION = '__APP_VERSION__'` | Vite/esbuild `define` substitutes identifiers, not text inside string literals | The Updates panel displays the literal string `__APP_VERSION__` as the app version | CONFIRMED |
| F16 | `src/stores/runtimeConfigStore.ts:53-72` + `src/tabs/Settings.tsx:342-354` | **P1** | quality | Store methods catch their own errors and never rethrow; the caller's `try/catch` expects a throw | `catch` at `Settings.tsx:349` is unreachable; local `error` state at `:327` is never set | A failed budget save shows a green "Saved!" and no error | CONFIRMED |
| F17 | `~22 properties`, e.g. `src/components/chat/FileContentDisplay.tsx:20,27,33,43-45,55-57,78`; `src/components/settings/SubmoduleStatusPanel.tsx:40-46,110` | **P1** | design | References CSS custom properties never defined in `globals.css`, most with no fallback | `--accent-color` (13), `--text-tertiary` (9), `--surface-bg` (8), `--accent-1/2/3`, `--error-1`, `--bg-0`, `--error-color`, … | Those elements render with no colour at all — `SubmoduleStatusPanel`'s status badges are entirely unstyled | CONFIRMED |
| F18 | `src/stores/settingsStore.ts:95-98` + `src/tabs/Settings.tsx:1105-1107` | **P1** | quality | Model switch: `setActiveProvider` has no try/catch on either side | Unhandled rejection | Clicking a model button that fails does nothing visible — reads as "changing the model doesn't work" (owner bug #9) | CONFIRMED |
| F19 | `src/stores/subagentStore.ts:144-172` + `src/components/subagent/SubagentMonitor.tsx:38-47` | **P2** | quality | `stopListening()` defined but never called anywhere; `isListening` guard blocks re-subscribe | Listener outlives the component permanently | Leaked `subagent:event` listener per app session; monitor can never re-subscribe after teardown | CONFIRMED |
| F20 | `src/stores/agentLiveStore.ts:25,498,520` | **P2** | quality | `tokenFlushTimer` is a module-level singleton reassigned on every `subscribeToEvents()` | Overlapping subscribes orphan the previous handle | A 500 ms interval runs for the life of the process doing nothing | CONFIRMED |
| F21 | `src/components/dashboard/Dashboard.tsx:87-99` + `src/stores/dashboardStore.ts:334,403,409` | **P2** | staleness | `startRealtimeUpdates()` is not awaited; its `realtimeEnabled` guard is set only after 4 `await listen()` calls resolve | StrictMode double-mount registers 8 listeners; `realtimeUnlisteners` is overwritten, orphaning 4 | `refreshDashboard()` fires twice per event; duplicates accumulate across tab switches | SUSPECTED |
| F22 | `src/stores/agentLiveStore.ts:124-132` + `src/tabs/AutoDevPanel.tsx:1078-1082` | **P2** | staleness | `initAgents` rebuilds the whole `agents` map from the passed slug list | Deletes lazily-created records (subagent slugs, early-tick workers) | Live subagent cards vanish mid-run whenever the worker roster reloads | CONFIRMED |
| F23 | `src/stores/skillStore.ts:137-146` | **P2** | quality | `initWatcher()` subscribes to `skills:reloaded`; the docstring promises auto-refresh | **Never called anywhere in `src/`** | The skills palette never live-updates when skills change on disk, contrary to its own docstring | CONFIRMED |
| F24 | `src/stores/settingsStore.ts:142-161` | **P2** | quality | `setPAMode` writes local state + localStorage, then fires `set_session_pa_mode` in an unawaited IIFE with no rollback | Optimistic write, failure only `console.error`s at `:158` | UI shows a PA mode the backend never stored | CONFIRMED |
| F25 | `src/stores/costStore.ts:29-33` + `src/tabs/Chat.tsx:296-301` | **P2** | quality | Accumulates `agent:done` cost deltas with no periodic re-sync | One missed/duplicated event skews the total permanently | Header cost readout drifts from truth until app restart | CONFIRMED |
| F26 | `src-tauri/src/commands/settings_commands.rs:30-35` | **P2** | wiring | `list_providers` returns a hard-coded 4-provider vector including `ollama` | Frontend has no provider list of its own — this IS the list | Ollama is offered in Settings; Grok cannot appear until this vector changes (§4 G1) | CONFIRMED |
| F27 | `src/components/workers/WorkerTable.tsx:29-121` | **P2** | wiring | Hard-codes a 10-family / 40-model registry (DeepSeek, Kimi, Groq, SEA-LION, Qwen, NVIDIA NIM, xAI Grok) sourced from a Python file | Disjoint from `list_providers`; no Rust provider implements most of them | The Settings tab offers workers models that cannot run, in a second model universe next to the first | CONFIRMED |
| F28 | `src/App.tsx:64` (only caller of `loadProviders`) | **P2** | staleness | Providers load once, at app boot; failure is swallowed to `console.error` (`settingsStore.ts:88-90`) | No retry, no refresh control in the Providers section | If the boot-time keychain read fails, the Providers section is permanently empty with no "Loading…" and no error | CONFIRMED |
| F29 | 47 registered commands (§6 b) | **P2** | deadcode | 18% of the Tauri command surface has no caller | Whole features have a backend and no UI: checkpoint creation, multi-repo graph, per-task budgets, orphan release, semantic worker routing, diff risk analysis | Backend maintained and shipped for capabilities the user cannot reach | CONFIRMED |
| F30 | `src/types/live.ts:89-121` | **P2** | contract | 5 payload interfaces (33 of 128 lines) for events with no Rust emitter | Types a contract that does not exist | Gives false confidence that the Live Monitor protocol is defined | CONFIRMED |
| F31 | `src/stores/planStore.ts:73-91` | **P2** | quality | 5 `listen()` calls execute in the zustand `create()` factory body at module load | Outside any effect; promises never awaited or stored | Permanently un-unsubscribable listeners installed at import time | CONFIRMED |
| F32 | `src-tauri/src/agent/engine.rs:436` (`agent:thinking`) | **P2** | contract | Rust streams extended-thinking tokens | No frontend `listen('agent:thinking')` anywhere | Extended thinking is computed, billed, and streamed — and never rendered | CONFIRMED |
| F33 | `src-tauri/src/commands/graph_commands.rs:862` (`ide:navigate`) | **P2** | contract | `open_in_ide` emits a navigate event | No frontend listener | "Open in IDE" from the graph does not move the IDE tab to the file | CONFIRMED |
| F34 | `src/components/git/CheckpointViewer.tsx` + `src-tauri/src/lib.rs` | **P2** | wiring | UI calls `list_checkpoints`, `restore_checkpoint`, `delete_checkpoint`; `create_checkpoint` and `compare_checkpoints` are never called | Checkpoints can be listed, restored, deleted — never created from the UI | A restore-only feature with no create path | CONFIRMED |
| F35 | `src/stores/workerStore.ts:182-241`, `pluginStore.ts:184-227`, `annotationStore.ts:90-138` | **P2** | quality | Optimistic `set()` after `invoke` without refetch or using the returned value | Correction depends on an event that may not arrive | Worker status/model can display a value the backend rejected | SUSPECTED |
| F36 | `src/stores/sessionStore.ts:99,109`; `agentLiveStore.ts:430`; `planStore.ts:123` | **P2** | quality | `any` on Tauri-boundary payloads: `invoke<any[]>('get_session_messages')`, `session.pa_mode as any`, `event: any`, `invoke<any>('planner_compile')` | No compile-time shape guarantee on backend data | A backend field rename fails at runtime, not at build | CONFIRMED |
| F37 | `src/stores/taskStore.ts:24-61` vs `src-tauri/src/db/tasks.rs:191-193` | **P3** | contract | TS `Task` omits `claimed_by_pid` and `claim_host` | Fields exist in the serialized payload but are unusable client-side | Orphan detection cannot show which PID/host holds a task | CONFIRMED |
| F38 | `src/components/layout/TitleBar.tsx:23,30,37`; `TabBar.tsx:52`; `budget/BudgetBar.tsx:109,111,143,145,160,162`; `update/UpdateDialog.tsx:155,157` | **P3** | design | `rounded-full` on buttons, tabs, and progress bars | The anchor reserves pill radius for status dots and explicitly forbids pill buttons | Visual identity does not match the anchor | CONFIRMED |
| F39 | `src/styles/globals.css:66` + `tailwind.config.js:41-44` | **P3** | design | `--radius: 2px`; only `borderRadius.DEFAULT` overridden | `rounded` and `rounded-sm` both resolve to 2px — the codebase's dominant class carries no meaning; 12+ distinct radii in production | Inconsistent corner treatment across the app | CONFIRMED |
| F40 | whole of `src/` | **P3** | a11y | Zero `prefers-reduced-motion` blocks; status conveyed by colour alone (`Settings.tsx:143,249,502,626,1121`); toggles are `<button>` without `aria-pressed`/`role="switch"` (`Settings.tsx:265,639,876`); sliders have `title` but no label (`Settings.tsx:425,1316,1337,1398`) | Spinners and pulses never degrade; state is invisible to screen readers | Unusable with a screen reader; motion-sensitive users get continuous animation | CONFIRMED |
| F41 | `src/components/startup/StartupOverlay.css:1`, `SubagentEventStream.css:1`, `SubagentMonitor.css:1`, `SubagentResultCard.css:1` | **P3** | design | Header comments claim "Winter is Coming theme" while the file consumes Palette-A variables | Comments describe a palette the file does not render | Misleads the next maintainer about which palette is authoritative | CONFIRMED |
| F42 | `src/tabs/Settings.tsx:226-228` and `:329-331` | **P3** | quality | Two sibling cards each call `cfg.load()` in their own mount effect | Duplicate `get_runtime_config` per Settings mount; the second `load()` resets `dirty: false` (`runtimeConfigStore.ts:47`) | The "Restart AutoDev" banner can disappear before the user reads it | SUSPECTED |
| F43 | `src/tabs/Settings.tsx:1067` | **P3** | wiring | `provider.models[1]?.id ?? provider.models[0]?.id` | Saving a key auto-activates the **second** model in the list | Arbitrary, undocumented default model on first key entry | CONFIRMED |
| F44 | `src/tabs/Settings.tsx:596-597,612` | **P3** | quality | GitHub MCP config load `.catch(() => {})`; save failure only `console.error`s | Silent swallow both ways | "Not configured" and "failed to load" are indistinguishable; a failed save looks successful | CONFIRMED |
| F45 | `src/components/settings/UpdateSettings.tsx:21,112` | **P3** | persist | Update channel is local `useState` | Never persisted | A user on `beta` silently reverts to `stable` on every remount | CONFIRMED |
| F46 | `src/stores/sidecarStore.ts:190-198` + `src/App.tsx:72` | **P3** | quality | 5 s poll started inside `initEventListeners`, which returns no teardown handle | Only self-clears via its own 12-attempt cap | Bounded leak window at boot; low impact | SUSPECTED |
| F47 | `src/stores/uiStore.ts` | **P3** | persist | `activeTab` is in-memory only | Not persisted | The app always reopens on the Chat tab regardless of where the user left off | CONFIRMED |
| F48 | `src/components/plan/PlanView.css` (323), `PlanTreeView.css` (133), `sidebar/PlanCard.css` (139), `chat/PlanDisplay.css` (138) | **P3** | deadcode | 733 lines of stylesheet for dead components | Shipped in the bundle | Bundle bloat only | CONFIRMED |

### Priority ordering for the restructure

1. **Persistence (F01, F02, F45, F47)** — one change, `AppState` serialization, fixes the loudest two symptoms.
2. **Event contract (F04, F05, F32, F33, F30)** — 5 dead listeners + 5 dead plan listeners + 3 dead events. Pick one direction per pair and delete the other side.
3. **Hooks safety (F03)** — either fire the 16 missing hook types or remove the templates and dropdown entries that target them. Shipping a "Block force push" hook that does nothing is the most dangerous item in this report.
4. **Write-with-no-reader (F06, F09)** — make the budget config actually read, and either wire or delete the browser-testing controls.
5. **Dead code (F08, F29, F48)** — decide, per feature, whether memory/plugins/plan get a UI or get deleted. ~3,700 LOC of frontend and 47 backend commands hang on that answer.
6. **Design system (F11, F12, F17, F38, F39, F41)** — a rewrite against `ui-anchor.md`, not a refactor. Start by collapsing the two palettes into one token file and defining the ~22 missing custom properties.
7. **Error surfacing (F10, F16, F18, F44)** — 48 unhandled `invoke` rejections across the stores; git is the worst offender.
8. **Staleness plumbing (F07, F21, F22)** — hoist `subscribeToEvents` to `App.tsx` and give the Live Monitor a backfill command.

---

*End of report.*
