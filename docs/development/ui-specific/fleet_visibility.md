# AutoDev — Fleet Live Monitor
### Embedded inside the AutoDev Panel Tab

**Document Status:** Draft v1.0  
**Author:** Razi (Programming Lead, AIRS Foundry)  
**Date:** May 2026  
**Scope:** AutoDev Panel tab — new LIVE MONITOR section between Agent Roster and Task Board  
**Companion Docs:** `AutoDev_UI_Spec_v2`, `AutoDev_Development_Plan`

---

## Table of Contents

1. [Problem Statement](#1-problem-statement)
2. [Solution Overview](#2-solution-overview)
3. [AutoDev Panel — Revised Full Layout](#3-autodev-panel--revised-full-layout)
4. [Section: LIVE MONITOR](#4-section-live-monitor)
   - 4.1 Section Header & Toggle
   - 4.2 Agent Live Card Grid
   - 4.3 Per-Agent Live Card Spec
   - 4.4 Stage Pipeline Strip
   - 4.5 Mini Log Tail
   - 4.6 Token Sparkline
   - 4.7 Card States
5. [Section: Agent Roster (Enhanced)](#5-section-agent-roster-enhanced)
6. [Data Layer: New Tauri Events](#6-data-layer-new-tauri-events)
7. [TypeScript Types](#7-typescript-types)
8. [Zustand Store: agentLiveStore](#8-zustand-store-agentlivestore)
9. [Frontend Component Reference](#9-frontend-component-reference)
   - 9.1 LiveMonitorSection.tsx
   - 9.2 AgentLiveCard.tsx
   - 9.3 StagePipeline.tsx
   - 9.4 MiniLogTail.tsx
   - 9.5 TokenSparkline.tsx
   - 9.6 AgentRoster.tsx (enhanced)
10. [Rust Backend: New Emit Points](#10-rust-backend-new-emit-points)
11. [Design Tokens Reference](#11-design-tokens-reference)

---

## 1. Problem Statement

The current AutoDev Panel shows every agent as `Working — polling...` regardless of what it is actually doing. The only way to get more detail is to open the Logs tab and manually tail the relevant log file. This breaks the "single pane of glass" principle — Razi has to switch tabs to understand what the fleet is doing.

**What is currently invisible:**
- Which tool call an agent is executing right now
- Which file/repo the agent is touching
- Whether the agent is stuck, retrying, or erroring silently
- Progress within a task (reading → writing → testing → committing)
- Per-agent token consumption rate
- Inter-agent events (supervisor dispatching to a worker)
- Whether a bash command failed and is being retried

**Goal:** Surface all of this information directly in the AutoDev Panel tab, without adding a new tab, and without requiring the user to click away.

---

## 2. Solution Overview

A new collapsible section called **LIVE MONITOR** is inserted between the existing Agent Roster and Task Board inside the AutoDev Panel tab. It contains a responsive card grid — one card per agent — showing that agent's live state in real time.

The Agent Roster above it gets a lightweight enhancement: the status text column becomes more informative, showing the current tool name and active file instead of just `Working — polling...`.

The Dashboard tab in the tab bar is **not affected** by this change. It remains available for future use.

```
AutoDev Panel tab layout (after change):

  ┌─ AGENT ROSTER ─────────────────────────────────────────────────┐
  │  Enhanced: shows current tool + file inline per row            │
  └────────────────────────────────────────────────────────────────┘
          ↕ 8px gap
  ┌─ LIVE MONITOR ─────────────────── [◉ LIVE] [▼ collapse] ──────┐
  │  2×N card grid, one card per agent                             │
  │  Each card: stage pipeline, current tool, file, log tail,      │
  │  token sparkline                                               │
  └────────────────────────────────────────────────────────────────┘
          ↕ 8px gap
  ┌─ TASK BOARD ───────────────────────────────────────────────────┐
  │  4-column kanban (unchanged)                                   │
  └────────────────────────────────────────────────────────────────┘
          ↕ 8px gap
  ┌─ METRICS ROW ──────────────────────────────────────────────────┐
  │  Cost Today · Tokens · Closed · Build Failed · QC Pass Rate    │
  └────────────────────────────────────────────────────────────────┘
```

---

## 3. AutoDev Panel — Revised Full Layout

The full tab layout with pixel-level sizing for each section:

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  AGENT ROSTER                            ● LivingAgent  ↻ Refresh  + New Task       │
│  ──────────────────────────────────────────────────────────────────────────────────  │
│  ● SUPERVISOR        dispatching → assign_task #143              SUPERVISOR     ■ ▶  │
│  ○ QC-AGENT          idle — awaiting PR                          QC AGENT        ▶   │
│  ● WORKER-BACKEND    writing → edit_file health.py               BACKEND        ■    │
│  ● WORKER-FRONTEND   reading → read_file App.tsx                 FRONTEND       ■    │
│  ● WORKER-PLATFORM   polling → list_tasks                        PLATFORM       ■    │
│  ● WORKER-MCP        polling → list_tasks                        MCP BUILDER    ■    │
│  ● WORKER-CLEANUP    linting → bash ruff check src/              CLEANUP        ■    │
│  ──────────────────────────────────────────────────────────────────────────────────  │
│  height: auto (min 180px), each row 32px                                             │
├──────────────────────────────────────────────────────────────────────────────────────┤
│  LIVE MONITOR                                            ◉ LIVE  [▼]  4s ago        │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐             │
│  │ ● SUPERVISOR       │  │ ● WORKER-BACKEND   │  │ ● WORKER-FRONTEND  │             │
│  │ ─────────────────  │  │ ─────────────────  │  │ ─────────────────  │             │
│  │ [claim][think]     │  │ [claim][read]      │  │ [claim][read]      │             │
│  │ [dispatch ●][mon]  │  │ [write ●][test]    │  │ [write][test]      │             │
│  │                    │  │ [push]             │  │ [push]             │             │
│  │ assign_task #143   │  │ edit_file          │  │ read_file App.tsx  │             │
│  │ tasks.db           │  │ src/api/health.py  │  │ src/App.tsx        │             │
│  │ ████░ 1.2k ∿∿∿∿∿  │  │ ████████ 8.4k ∿∿  │  │ ████░░ 2.1k ∿∿∿   │             │
│  │ 00:04 list_tasks→3 │  │ 00:12 read_file ✓  │  │ 00:09 claim ✓      │             │
│  │ 00:18 assign #142✓ │  │ 00:27 read_file ✓  │  │ ◌  read_file...   │             │
│  │ ◌  assign #143...  │  │ ◌  edit_file...    │  │                    │             │
│  └────────────────────┘  └────────────────────┘  └────────────────────┘             │
│  height: collapsible, default 220px, expand to auto on focus                         │
├──────────────────────────────────────────────────────────────────────────────────────┤
│  TASK BOARD                                                     ⊘ Clear Board       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │
│  │ ● READY (0)  │  │ ● IN PROG(0) │  │ ● QC REV (0) │  │ ● DONE (0)  │            │
│  │   empty      │  │   empty      │  │   empty      │  │   empty     │            │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘            │
│  height: 320px fixed                                                                 │
├──────────────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────┐ │
│  │ COST TODAY   │  │ TOKENS       │  │ CLOSED       │  │ BUILD FAILED │  │ QC %  │ │
│  │ $0.00        │  │ —            │  │ 0            │  │ 0            │  │ —     │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘  └───────┘ │
│  height: 80px                                                                        │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

**Section height policy:**
- Agent Roster: `min-height: 180px`, auto-grows per row
- Live Monitor: `220px` collapsed (shows full cards), `collapse to 32px` (header only) when toggled off, `expand to auto` when a card is focused
- Task Board: `320px` fixed
- Metrics Row: `80px` fixed
- Total: approximately `700px` at normal state — fits in a 1080p window without inner scrolling

---

## 4. Section: LIVE MONITOR

### 4.1 Section Header & Toggle

```
LIVE MONITOR                                           ◉ LIVE  [▼]  4s ago
```

| Element | Spec |
|---|---|
| Section label | `IBM Plex Mono 9px uppercase`, `color: var(--text-muted)`, `letter-spacing: 0.1em` |
| `◉ LIVE` badge | Pulsing dot + `LIVE` text. Dot: `6px`, `background: var(--accent)`, CSS `pulse` animation (opacity 1 → 0.4 → 1, 1.5s). Text: `IBM Plex Mono 9px`, `color: var(--accent)`. Shown when at least one agent is active and emitting events |
| `◉ STALE` badge | Same dot in `var(--warning)` if last event was > 30s ago |
| `◉ OFFLINE` badge | Dot in `var(--danger)` if sidecars are down |
| `[▼]` collapse toggle | `IBM Plex Mono 10px`, `color: var(--text-dim)`. Chevron flips to `[▲]` when expanded. Click toggles the card grid |
| Last event time | `IBM Plex Mono 10px`, `color: var(--text-dim)`. `"4s ago"` — updates every second via `Date.now()` diff |
| Section separator | `1px solid var(--border-primary)` top and bottom |
| Container padding | `12px 0 10px` |

**Collapse behavior:**
- Collapsed: section header only visible (`32px` total height). Card grid `display: none`.
- Expanded: card grid shown. Default state on app open is **expanded**.
- Preference persisted in `localStorage` key `autodev.live_monitor.collapsed`.

### 4.2 Agent Live Card Grid

The card grid is a CSS grid that auto-fits columns:

```css
.live-monitor-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 8px;
  padding: 8px 0;
}
```

This means:
- 7 agents at 1400px width → 4 columns × 2 rows (last row partial)
- 7 agents at 1200px width → 3 columns × 3 rows
- 7 agents at 900px width → 2 columns × 4 rows

Cards that belong to idle agents are visually de-emphasized (lower opacity) but still shown — so the user can confirm "QC-AGENT is idle, that's expected."

### 4.3 Per-Agent Live Card Spec

```
┌─────────────────────────────────────────────┐
│  ● WORKER-BACKEND                ⬡ BACKEND  │  ← card header
│  ─────────────────────────────────────────  │
│  [claim ✓] [read ✓] [write ●] [test] [push] │  ← stage pipeline
│  ─────────────────────────────────────────  │
│  edit_file                                  │  ← current tool
│  src/api/health.py  +10 -0                  │  ← current file
│  ─────────────────────────────────────────  │
│  ████████████████░░░░  8.4k   ∿∿∿∿∿∿∿∿∿   │  ← token bar + sparkline
│  ─────────────────────────────────────────  │
│  00:12  read_file  routes.py         ✓      │  ← log tail
│  00:27  read_file  health.py         ✓      │
│  ◌       edit_file  health.py               │  ← in-progress line
└─────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `background: var(--bg-card)` (`#111111`), `border: 1px solid var(--border-primary)`, `border-radius: 2px`, `padding: 10px 12px` |
| Active agent border | `border-color: var(--border-secondary)` (`#222222`) — slightly brighter when actively emitting events |
| Idle agent | `opacity: 0.5` on entire card |
| Error agent | `border-color: var(--danger)` (`#FF4444`), `background: rgba(255,68,68,0.03)` |
| Card min-height | `180px` — ensures uniform grid alignment |
| Card header | Agent name `IBM Plex Mono 11px`, `color: var(--text-primary)`. Status dot left. Role badge right. |
| Role badge | `IBM Plex Mono 9px uppercase`, `background: var(--bg-code)`, `border: 1px solid var(--border-secondary)`, `color: var(--text-dim)`, `padding: 1px 6px`, `border-radius: 2px` |
| Dividers | `1px solid var(--border-primary)` between each sub-section |

**Card click behavior:** Clicking a card expands it vertically and increases the log tail from 3 lines to 12 lines. The expanded card spans 2 columns (`grid-column: span 2`). Click again or press Escape to collapse.

### 4.4 Stage Pipeline Strip

Visually shows the agent's progress through its task lifecycle. Stages differ per agent type.

**Worker stage sequence:**
```
[claim] → [read] → [write] → [test] → [push]
```

**Supervisor stage sequence:**
```
[scan] → [think] → [dispatch] → [monitor]
```

**QC Agent stage sequence:**
```
[claim PR] → [read diff] → [review] → [verdict]
```

**Stage visual states:**

| State | Visual |
|---|---|
| `done` | `background: rgba(0,255,136,0.06)`, `border: 1px solid rgba(0,255,136,0.20)`, `color: var(--accent)`, `IBM Plex Mono 9px` |
| `current` | `background: rgba(255,170,0,0.06)`, `border: 1px solid rgba(255,170,0,0.25)`, `color: var(--warning)`, `IBM Plex Mono 9px`. **Pulsing border animation** (`border-opacity 0.25 → 0.5 → 0.25`, 1.2s loop) |
| `pending` | `background: transparent`, `border: 1px solid var(--border-primary)`, `color: var(--text-muted)`, `IBM Plex Mono 9px` |
| `error` | `background: rgba(255,68,68,0.06)`, `border: 1px solid rgba(255,68,68,0.25)`, `color: var(--danger)` |

**Stage pill sizing:** `padding: 2px 7px`, `border-radius: 2px`. Pills flow as a flex row with `gap: 4px`. Separator `›` between pills in `color: var(--border-secondary)`.

**Stage transitions:** Driven by the `agent:stage-change` Tauri event (see Section 6). Stage state is persisted in `agentLiveStore` per agent ID.

**Stage completion time:** On hover over a `done` stage, a tooltip shows how long that stage took: `"read — 00:18"`.

### 4.5 Mini Log Tail

Shows the last N tool calls this agent made. Default N = 3 lines (collapsed card), N = 12 (expanded card).

```
00:12  read_file  routes.py          ✓
00:27  read_file  health.py          ✓
◌       edit_file  health.py
```

| Element | Spec |
|---|---|
| Container | `background: var(--bg-code)` (`#161616`), `border-radius: 2px`, `padding: 6px 8px`, `margin-top: 6px` |
| Font | `IBM Plex Mono 10px` throughout |
| Timestamp | `color: var(--text-dim)` (`#333333`), width `36px`, flex-shrink 0 |
| Tool name | `color: var(--info)` (`#4477FF`), flex-shrink 0 — uses the same color as tool names in Chat tab tool cards |
| Argument | `color: var(--text-secondary)` (`#555555`), truncated to `120px` with ellipsis |
| `✓` success | `color: var(--accent)` (`#00FF88`), right-aligned |
| `✗` error | `color: var(--danger)` (`#FF4444`), right-aligned. The log line itself gets `background: rgba(255,68,68,0.05)` |
| `◌` spinner | 8×8px CSS border-spin, `border-top-color: var(--accent)`, replaces timestamp for the active line |
| Line hover | `background: var(--bg-card)` — reveals full un-truncated argument in a tooltip |
| Max lines shown | 3 in collapsed card, 12 in expanded. Older lines fade out above with a gradient mask |
| Auto-scroll | New lines append to the bottom. Log tail auto-scrolls to show the latest line |

**Error line special behavior:** If a tool call returns an error (`is_error: true`), the entire log line renders with a left red border accent:
```css
border-left: 2px solid var(--danger);
padding-left: 6px;
```

**Collapse overflow:** When more than N lines exist, the top line fades out under a `linear-gradient(to bottom, var(--bg-code), transparent)` mask so the user knows there is older history. Clicking the card expands it to see more.

### 4.6 Token Sparkline

Combines a linear progress bar (total session tokens) with a 10-bar activity sparkline (tokens per 5-second window).

```
████████████████░░░░  8,412   ∿∿∿∿∿∿∿∿∿∿
```

| Element | Spec |
|---|---|
| Token bar container | `flex: 1`, `height: 3px`, `background: var(--border-primary)`, `border-radius: 1px` |
| Token bar fill | `background: var(--accent)`, proportional to `(agent_tokens / session_token_budget)`. Turns `var(--warning)` when > 70%, `var(--danger)` when > 90% |
| Token count | `IBM Plex Mono 10px`, `color: var(--text-dim)`, format: `8.4k` if > 1000 |
| Sparkline | 10 bars, each 3px wide, 1px gap, `border-radius: 1px`. Height scales 2px–16px proportional to tokens emitted in each 5s window |
| Sparkline bar color | `var(--accent)` at 40% opacity for low activity, 100% opacity for high |
| Idle sparkline | All bars at `2px` height in `var(--border-primary)` color |
| Container layout | `display: flex`, `align-items: center`, `gap: 6px`, `margin-top: 8px` |

### 4.7 Card States

| Agent State | Card Appearance |
|---|---|
| `active` (working) | Full opacity. Amber dot. Stage pipeline shows current stage pulsing. Log tail appending live. |
| `idle` | `opacity: 0.5`. Blue dot. Stage pipeline all `pending`. Log tail shows last completed run greyed. Token sparkline flat. |
| `error` | Red border `var(--danger)`. Red dot. Last log line shown in red. Stage shows errored step in red. Error message shown below log tail. |
| `working-blocked` | Amber dot but stage pipeline has a `blocked` step (orange ⊘). Tooltip on blocked step explains reason. |
| `stopped` | Gray dot. `opacity: 0.4`. `[▶ Start]` button shown in card footer. |
| `starting` | Amber dot pulsing slowly. Stage at `claim`. Spinner on first log line. |

---

## 5. Section: Agent Roster (Enhanced)

The existing Agent Roster rows get a more informative status column. No layout change needed — just richer content in the existing status text area.

**Before:**
```
● WORKER-BACKEND    Working — polling...                    BACKEND    ■
```

**After:**
```
● WORKER-BACKEND    edit_file · health.py  +10 -0          BACKEND    ■
```

| New status format | When |
|---|---|
| `{tool_name} · {filename}` | Agent actively executing a tool with a file argument |
| `{tool_name} · {short_arg}` | Tool without a file (e.g. `assign_task · #143`) |
| `polling → {N} tasks found` | After list_tasks completes |
| `idle — awaiting tasks` | No active tool call |
| `⚠ exit 1 — retrying` | Last bash call failed, agent is in retry loop |
| `⚠ blocked on #{task_id}` | Task claimed but locked by another worker |

The status text update is driven by the same `agent:tool-call-start` and `agent:tool-call-result` events that feed the Live Monitor cards.

**Elapsed time column** (new, replaces empty space at right of row):
- Shows `MM:SS` elapsed since current tool call started
- Resets to `—` when tool completes
- If a single tool call exceeds 2 minutes, turns `var(--warning)` color — possible hang indicator

---

## 6. Data Layer: New Tauri Events

These events are the backbone of the Live Monitor. Several already exist in the system (marked ✅). New events are marked 🆕.

| Event | Direction | Payload | Drives |
|---|---|---|---|
| ✅ `agent:tool-call-start` | Backend → Frontend | `{ agent_id, tool_call_id, name, args }` | Log tail new line (◌ spinner), status text update, elapsed timer start, stage advance |
| ✅ `agent:tool-call-result` | Backend → Frontend | `{ agent_id, tool_call_id, result, is_error, elapsed_ms }` | Log tail line result (`✓` or `✗`), elapsed timer reset |
| ✅ `agent:status-change` | Backend → Frontend | `{ agent_id, status: AgentStatus }` | Card state (active/idle/error/stopped) |
| 🆕 `agent:stage-change` | Backend → Frontend | `{ agent_id, stage: string, status: StageStatus }` | Stage pipeline pill state |
| 🆕 `agent:token-update` | Backend → Frontend | `{ agent_id, total_tokens: number, window_tokens: number }` | Token bar fill, sparkline new bar |
| 🆕 `agent:thought-snippet` | Backend → Frontend | `{ agent_id, text: string }` | Optional: shown as italic dim line in log tail, prefixed with `···` |
| 🆕 `agent:error` | Backend → Frontend | `{ agent_id, error_message: string, tool_call_id?: string }` | Card state → error, last log line red, error message under log tail |
| 🆕 `agent:retry` | Backend → Frontend | `{ agent_id, attempt: number, max_attempts: number, reason: string }` | Status text shows `⚠ retrying (2/3)`, log tail shows retry line |

### 6.1 Event Frequency

`agent:token-update` fires on every streamed chunk from the LLM — potentially hundreds per second per agent. To avoid overwhelming the React render loop:

- Frontend **throttles** this event to one state update per 500ms via a local buffer in the store
- `window_tokens` is the sum of tokens in the current 5-second window, accumulated in the store before committing to state

`agent:tool-call-start` and `agent:tool-call-result` fire once per tool call — no throttling needed.

### 6.2 Event Payload Schemas

```typescript
// agent:stage-change
interface AgentStageChangePayload {
  agent_id: string;          // e.g. "worker-backend"
  stage: string;             // e.g. "read", "write", "test"
  status: "done" | "current" | "pending" | "error" | "blocked";
  elapsed_ms?: number;       // how long this stage took (on done)
  reason?: string;           // for blocked/error states
}

// agent:token-update
interface AgentTokenUpdatePayload {
  agent_id: string;
  total_tokens: number;      // cumulative session tokens for this agent
  window_tokens: number;     // tokens in current 5-second window
}

// agent:thought-snippet
interface AgentThoughtPayload {
  agent_id: string;
  text: string;              // first 120 chars of thinking block
}

// agent:error
interface AgentErrorPayload {
  agent_id: string;
  error_message: string;
  tool_call_id?: string;
  is_fatal: boolean;         // true = agent stopped, false = retrying
}
```

---

## 7. TypeScript Types

All types in `src/types/agent.ts` and `src/types/live.ts`.

```typescript
// src/types/live.ts

export type StageStatus = "done" | "current" | "pending" | "error" | "blocked";

export interface Stage {
  id: string;           // e.g. "read", "write", "test"
  label: string;        // display text, e.g. "read"
  status: StageStatus;
  elapsed_ms?: number;
}

// Agent-type-specific stage sequences
export const WORKER_STAGES: Stage[] = [
  { id: "claim",  label: "claim",  status: "pending" },
  { id: "read",   label: "read",   status: "pending" },
  { id: "write",  label: "write",  status: "pending" },
  { id: "test",   label: "test",   status: "pending" },
  { id: "push",   label: "push",   status: "pending" },
];

export const SUPERVISOR_STAGES: Stage[] = [
  { id: "scan",     label: "scan",     status: "pending" },
  { id: "think",    label: "think",    status: "pending" },
  { id: "dispatch", label: "dispatch", status: "pending" },
  { id: "monitor",  label: "monitor",  status: "pending" },
];

export const QC_STAGES: Stage[] = [
  { id: "claim_pr", label: "claim PR", status: "pending" },
  { id: "read_diff", label: "read diff", status: "pending" },
  { id: "review",   label: "review",   status: "pending" },
  { id: "verdict",  label: "verdict",  status: "pending" },
];

export interface LogLine {
  id: string;              // uuid for React key
  timestamp_offset_ms: number;   // ms since agent started current task
  tool_name: string;
  argument: string;        // short arg string, truncated at 40 chars
  full_argument: string;   // full arg for tooltip
  status: "running" | "success" | "error" | "thought";
  elapsed_ms?: number;     // for completed lines
}

export interface SparklineWindow {
  window_index: number;    // 0–9, circular
  tokens: number;          // tokens in this 5s window
}

export interface AgentLiveState {
  agent_id: string;
  agent_type: "supervisor" | "worker" | "qc" | "pa";
  display_name: string;
  status: "active" | "idle" | "error" | "stopped" | "starting";
  stages: Stage[];
  current_tool: string | null;
  current_file: string | null;
  current_file_diff?: string;      // e.g. "+10 -0"
  log_lines: LogLine[];            // capped at 50 entries
  total_tokens: number;
  sparkline: SparklineWindow[];    // always 10 entries
  error_message: string | null;
  last_event_at: number;           // Date.now()
  task_id: number | null;
  elapsed_since_last_tool_ms: number;
}
```

---

## 8. Zustand Store: agentLiveStore

Full store definition in `src/stores/agentLiveStore.ts`.

```typescript
// src/stores/agentLiveStore.ts
import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import type { AgentLiveState, Stage, LogLine, SparklineWindow } from "../types/live";

interface AgentLiveStore {
  agents: Record<string, AgentLiveState>;   // keyed by agent_id
  isMonitorCollapsed: boolean;

  // Actions
  initAgents: (agentIds: string[]) => void;
  setMonitorCollapsed: (v: boolean) => void;
  handleToolCallStart: (payload: AgentToolCallStartPayload) => void;
  handleToolCallResult: (payload: AgentToolCallResultPayload) => void;
  handleStageChange: (payload: AgentStageChangePayload) => void;
  handleTokenUpdate: (payload: AgentTokenUpdatePayload) => void;
  handleStatusChange: (payload: AgentStatusChangePayload) => void;
  handleError: (payload: AgentErrorPayload) => void;

  // Subscriptions
  subscribeToEvents: () => () => void;   // returns cleanup fn
}

// Token update throttle buffer (not in store, module-level)
const tokenBuffer: Record<string, { total: number; window: number }> = {};
let tokenFlushTimer: ReturnType<typeof setInterval> | null = null;

export const useAgentLiveStore = create<AgentLiveStore>((set, get) => ({
  agents: {},
  isMonitorCollapsed: false,

  initAgents: (agentIds) => {
    const agents: Record<string, AgentLiveState> = {};
    agentIds.forEach(id => {
      agents[id] = {
        agent_id: id,
        agent_type: inferAgentType(id),
        display_name: id.toUpperCase(),
        status: "idle",
        stages: getInitialStages(id),
        current_tool: null,
        current_file: null,
        log_lines: [],
        total_tokens: 0,
        sparkline: Array(10).fill({ tokens: 0 }),
        error_message: null,
        last_event_at: Date.now(),
        task_id: null,
        elapsed_since_last_tool_ms: 0,
      };
    });
    set({ agents });
  },

  setMonitorCollapsed: (v) => {
    localStorage.setItem("autodev.live_monitor.collapsed", String(v));
    set({ isMonitorCollapsed: v });
  },

  handleToolCallStart: ({ agent_id, tool_call_id, name, args }) => {
    set(state => {
      const agent = state.agents[agent_id];
      if (!agent) return state;
      const newLine: LogLine = {
        id: tool_call_id,
        timestamp_offset_ms: Date.now() - (agent.last_event_at || Date.now()),
        tool_name: name,
        argument: truncate(args, 40),
        full_argument: args,
        status: "running",
      };
      return {
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            current_tool: name,
            current_file: extractFile(args),
            current_file_diff: null,
            log_lines: [...agent.log_lines.slice(-49), newLine],
            last_event_at: Date.now(),
          }
        }
      };
    });
  },

  handleToolCallResult: ({ agent_id, tool_call_id, is_error, elapsed_ms }) => {
    set(state => {
      const agent = state.agents[agent_id];
      if (!agent) return state;
      return {
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            current_tool: null,
            log_lines: agent.log_lines.map(line =>
              line.id === tool_call_id
                ? { ...line, status: is_error ? "error" : "success", elapsed_ms }
                : line
            ),
            last_event_at: Date.now(),
          }
        }
      };
    });
  },

  handleStageChange: ({ agent_id, stage, status, elapsed_ms }) => {
    set(state => {
      const agent = state.agents[agent_id];
      if (!agent) return state;
      return {
        agents: {
          ...state.agents,
          [agent_id]: {
            ...agent,
            stages: agent.stages.map(s =>
              s.id === stage ? { ...s, status, elapsed_ms } : s
            ),
          }
        }
      };
    });
  },

  handleTokenUpdate: ({ agent_id, total_tokens, window_tokens }) => {
    // Buffer token updates — only flush to store every 500ms
    tokenBuffer[agent_id] = { total: total_tokens, window: window_tokens };
  },

  handleStatusChange: ({ agent_id, status }) => {
    set(state => ({
      agents: {
        ...state.agents,
        [agent_id]: { ...state.agents[agent_id], status, last_event_at: Date.now() }
      }
    }));
  },

  handleError: ({ agent_id, error_message, is_fatal }) => {
    set(state => ({
      agents: {
        ...state.agents,
        [agent_id]: {
          ...state.agents[agent_id],
          status: is_fatal ? "error" : state.agents[agent_id]?.status,
          error_message,
          last_event_at: Date.now(),
        }
      }
    }));
  },

  subscribeToEvents: () => {
    const store = get();
    const unlisteners: Promise<() => void>[] = [];

    unlisteners.push(listen("agent:tool-call-start",  e => store.handleToolCallStart(e.payload as any)));
    unlisteners.push(listen("agent:tool-call-result", e => store.handleToolCallResult(e.payload as any)));
    unlisteners.push(listen("agent:stage-change",     e => store.handleStageChange(e.payload as any)));
    unlisteners.push(listen("agent:token-update",     e => store.handleTokenUpdate(e.payload as any)));
    unlisteners.push(listen("agent:status-change",    e => store.handleStatusChange(e.payload as any)));
    unlisteners.push(listen("agent:error",            e => store.handleError(e.payload as any)));

    // Flush token buffer to store every 500ms
    tokenFlushTimer = setInterval(() => {
      const updates: Record<string, Partial<AgentLiveState>> = {};
      Object.entries(tokenBuffer).forEach(([agent_id, { total, window }]) => {
        const agent = get().agents[agent_id];
        if (!agent) return;
        const newSparkline = [...agent.sparkline.slice(1), { window_index: 9, tokens: window }];
        updates[agent_id] = { total_tokens: total, sparkline: newSparkline };
        delete tokenBuffer[agent_id];
      });
      if (Object.keys(updates).length > 0) {
        set(state => {
          const newAgents = { ...state.agents };
          Object.entries(updates).forEach(([id, patch]) => {
            newAgents[id] = { ...newAgents[id], ...patch };
          });
          return { agents: newAgents };
        });
      }
    }, 500);

    return () => {
      unlisteners.forEach(p => p.then(fn => fn()));
      if (tokenFlushTimer) clearInterval(tokenFlushTimer);
    };
  },
}));

// ─── Helpers ──────────────────────────────────────────────────────────────────

function inferAgentType(id: string): AgentLiveState["agent_type"] {
  if (id === "supervisor") return "supervisor";
  if (id === "qc-agent")   return "qc";
  if (id === "pa")         return "pa";
  return "worker";
}

function getInitialStages(id: string): Stage[] {
  const type = inferAgentType(id);
  if (type === "supervisor") return [...SUPERVISOR_STAGES];
  if (type === "qc")         return [...QC_STAGES];
  return [...WORKER_STAGES];
}

function extractFile(args: string): string | null {
  const match = args.match(/([^\s/]+\.[a-z]{2,5})/i);
  return match ? match[1] : null;
}

function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n) + "…" : s;
}
```

---

## 9. Frontend Component Reference

### 9.1 LiveMonitorSection.tsx

Top-level container. Reads `isMonitorCollapsed` from store. Manages the subscription lifecycle.

```tsx
// src/components/autodev/LiveMonitorSection.tsx
import { useEffect, useRef } from "react";
import { useAgentLiveStore } from "../../stores/agentLiveStore";
import { AgentLiveCard } from "./AgentLiveCard";

export function LiveMonitorSection() {
  const { agents, isMonitorCollapsed, setMonitorCollapsed, subscribeToEvents } =
    useAgentLiveStore();

  const lastEventTime = Math.max(
    ...Object.values(agents).map(a => a.last_event_at)
  );
  const secondsAgo = Math.floor((Date.now() - lastEventTime) / 1000);
  const isLive = secondsAgo < 30 && Object.values(agents).some(a => a.status === "active");

  // Subscribe to Tauri events on mount
  useEffect(() => {
    const cleanup = subscribeToEvents();
    return cleanup;
  }, []);

  return (
    <div className="live-monitor-section">
      <div className="section-header">
        <span className="section-label">LIVE MONITOR</span>
        <div className="section-header-right">
          {isLive ? (
            <span className="live-badge live-badge--live">◉ LIVE</span>
          ) : secondsAgo < 120 ? (
            <span className="live-badge live-badge--stale">◉ STALE</span>
          ) : (
            <span className="live-badge live-badge--offline">◉ OFFLINE</span>
          )}
          {!isLive && (
            <span className="last-event-time">
              {secondsAgo < 60 ? `${secondsAgo}s ago` : `${Math.floor(secondsAgo / 60)}m ago`}
            </span>
          )}
          <button
            className="collapse-toggle"
            onClick={() => setMonitorCollapsed(!isMonitorCollapsed)}
            aria-label={isMonitorCollapsed ? "Expand live monitor" : "Collapse live monitor"}
          >
            {isMonitorCollapsed ? "▲" : "▼"}
          </button>
        </div>
      </div>

      {!isMonitorCollapsed && (
        <div className="live-monitor-grid">
          {Object.values(agents).map(agent => (
            <AgentLiveCard key={agent.agent_id} agentId={agent.agent_id} />
          ))}
        </div>
      )}
    </div>
  );
}
```

### 9.2 AgentLiveCard.tsx

Individual agent card. Reads its own slice from the store. Manages expand/collapse state locally.

```tsx
// src/components/autodev/AgentLiveCard.tsx
import { useState } from "react";
import { useAgentLiveStore } from "../../stores/agentLiveStore";
import { StagePipeline } from "./StagePipeline";
import { MiniLogTail } from "./MiniLogTail";
import { TokenSparkline } from "./TokenSparkline";
import { invoke } from "@tauri-apps/api/core";

interface Props { agentId: string; }

export function AgentLiveCard({ agentId }: Props) {
  const agent = useAgentLiveStore(s => s.agents[agentId]);
  const [expanded, setExpanded] = useState(false);

  if (!agent) return null;

  const cardClass = [
    "agent-live-card",
    `agent-live-card--${agent.status}`,
    expanded ? "agent-live-card--expanded" : "",
  ].filter(Boolean).join(" ");

  function openInLogs() {
    // Switch to Logs tab and filter to this agent
    invoke("navigate_to_logs", { filter_agent: agentId });
  }

  return (
    <div
      className={cardClass}
      style={expanded ? { gridColumn: "span 2" } : undefined}
      onClick={() => setExpanded(e => !e)}
    >
      {/* Header */}
      <div className="card-header">
        <span className={`status-dot status-dot--${agent.status}`} />
        <span className="card-agent-name">{agent.display_name}</span>
        <span className="card-role-badge">{agent.agent_type.toUpperCase()}</span>
      </div>

      {/* Stage pipeline */}
      <StagePipeline stages={agent.stages} />

      {/* Current tool + file */}
      {agent.current_tool && (
        <div className="card-current-action">
          <span className="current-tool-name">{agent.current_tool}</span>
          {agent.current_file && (
            <span className="current-file-name">
              {agent.current_file}
              {agent.current_file_diff && (
                <span className="diff-badge">{agent.current_file_diff}</span>
              )}
            </span>
          )}
        </div>
      )}

      {/* Token bar + sparkline */}
      <TokenSparkline
        totalTokens={agent.total_tokens}
        sparkline={agent.sparkline}
      />

      {/* Log tail */}
      <MiniLogTail
        lines={agent.log_lines}
        maxLines={expanded ? 12 : 3}
      />

      {/* Error message */}
      {agent.error_message && (
        <div className="card-error-message">
          ✗ {agent.error_message}
        </div>
      )}

      {/* Expanded footer */}
      {expanded && (
        <div className="card-expanded-footer" onClick={e => e.stopPropagation()}>
          <button className="card-action-btn" onClick={openInLogs}>
            View full log ↗
          </button>
          {agent.task_id && (
            <span className="card-task-id">Task #{agent.task_id}</span>
          )}
        </div>
      )}
    </div>
  );
}
```

### 9.3 StagePipeline.tsx

Stateless — receives `stages` array as prop and renders the pipeline strip.

```tsx
// src/components/autodev/StagePipeline.tsx
import { Stage } from "../../types/live";

interface Props { stages: Stage[]; }

export function StagePipeline({ stages }: Props) {
  return (
    <div className="stage-pipeline">
      {stages.map((stage, i) => (
        <div key={stage.id} className="stage-pill-wrapper">
          <div
            className={`stage-pill stage-pill--${stage.status}`}
            title={stage.elapsed_ms ? `${stage.label} — ${formatMs(stage.elapsed_ms)}` : stage.label}
          >
            {stage.label}
          </div>
          {i < stages.length - 1 && (
            <span className="stage-sep" aria-hidden>›</span>
          )}
        </div>
      ))}
    </div>
  );
}

function formatMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}
```

### 9.4 MiniLogTail.tsx

Renders the last N log lines. Auto-scrolls to bottom when new lines arrive.

```tsx
// src/components/autodev/MiniLogTail.tsx
import { useEffect, useRef } from "react";
import { LogLine } from "../../types/live";

interface Props {
  lines: LogLine[];
  maxLines: number;
}

export function MiniLogTail({ lines, maxLines }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const visible = lines.slice(-maxLines);

  useEffect(() => {
    if (ref.current) {
      ref.current.scrollTop = ref.current.scrollHeight;
    }
  }, [lines.length]);

  return (
    <div className="mini-log-tail" ref={ref}>
      {visible.map(line => (
        <div
          key={line.id}
          className={`log-line log-line--${line.status}`}
          title={line.full_argument}
        >
          {line.status === "running" ? (
            <span className="log-spinner" aria-hidden />
          ) : (
            <span className="log-time">{formatOffset(line.timestamp_offset_ms)}</span>
          )}
          <span className="log-tool">{line.tool_name}</span>
          <span className="log-arg">{line.argument}</span>
          {line.status === "success" && <span className="log-ok">✓</span>}
          {line.status === "error"   && <span className="log-err">✗</span>}
        </div>
      ))}
    </div>
  );
}

function formatOffset(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  return `${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}
```

### 9.5 TokenSparkline.tsx

```tsx
// src/components/autodev/TokenSparkline.tsx
import { SparklineWindow } from "../../types/live";

interface Props {
  totalTokens: number;
  sparkline: SparklineWindow[];
  budgetTokens?: number;  // defaults to 100_000
}

export function TokenSparkline({ totalTokens, sparkline, budgetTokens = 100_000 }: Props) {
  const fillPct = Math.min((totalTokens / budgetTokens) * 100, 100);
  const barClass = fillPct > 90 ? "token-bar--danger"
                 : fillPct > 70 ? "token-bar--warning"
                 : "token-bar--normal";

  const maxWindow = Math.max(...sparkline.map(w => w.tokens), 1);

  function formatTokens(n: number): string {
    return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
  }

  return (
    <div className="token-sparkline">
      <div className="token-bar-row">
        <div className="token-bar-bg">
          <div className={`token-bar-fg ${barClass}`} style={{ width: `${fillPct}%` }} />
        </div>
        <span className="token-count">{formatTokens(totalTokens)}</span>
      </div>
      <div className="sparkline-bars" aria-hidden>
        {sparkline.map((w, i) => {
          const h = Math.max(2, Math.round((w.tokens / maxWindow) * 16));
          const active = w.tokens > maxWindow * 0.5;
          return (
            <div
              key={i}
              className={`spark-bar ${active ? "spark-bar--active" : ""}`}
              style={{ height: `${h}px` }}
            />
          );
        })}
      </div>
    </div>
  );
}
```

### 9.6 AgentRoster.tsx (enhanced)

Only the status column render is shown — the rest of the existing component remains unchanged.

```tsx
// src/components/autodev/AgentRoster.tsx  (status column enhancement)
import { useAgentLiveStore } from "../../stores/agentLiveStore";

// Replace the existing status text cell with this:
function AgentStatusCell({ agentId }: { agentId: string }) {
  const agent = useAgentLiveStore(s => s.agents[agentId]);
  const elapsedMs = agent?.elapsed_since_last_tool_ms ?? 0;
  const isSlowTool = elapsedMs > 120_000;  // > 2 minutes

  if (!agent || agent.status === "idle") {
    return <span className="status-text status-text--idle">Idle — awaiting tasks</span>;
  }
  if (agent.status === "error") {
    return (
      <span className="status-text status-text--error">
        ⚠ {agent.error_message ?? "error"}
      </span>
    );
  }
  if (agent.current_tool) {
    return (
      <span className="status-text status-text--active">
        <span className="status-tool">{agent.current_tool}</span>
        {agent.current_file && (
          <>
            <span className="status-sep"> · </span>
            <span className="status-file">{agent.current_file}</span>
            {agent.current_file_diff && (
              <span className="status-diff"> {agent.current_file_diff}</span>
            )}
          </>
        )}
        {isSlowTool && <span className="status-slow-warn"> ⚠</span>}
      </span>
    );
  }
  return <span className="status-text">Working…</span>;
}
```

---

## 10. Rust Backend: New Emit Points

These are the locations in the Rust backend where new events need to be emitted.

### 10.1 agent:stage-change

Stage changes are inferred from tool call names. Add to `src-tauri/src/agent/engine.rs`, inside the tool execution loop, after each tool call completes:

```rust
// src-tauri/src/agent/engine.rs

fn infer_stage_from_tool(tool_name: &str) -> Option<(&'static str, &'static str)> {
    match tool_name {
        "list_tasks" | "claim_task"                => Some(("claim", "done")),
        "read_file" | "grep_search" | "list_directory" => Some(("read", "current")),
        "edit_file" | "write_file"                 => Some(("write", "current")),
        "bash" if /* test command */ true          => Some(("test", "current")),
        "git_commit" | "git_push" | "create_pr"   => Some(("push", "current")),
        "assign_task"                              => Some(("dispatch", "current")),
        _                                          => None,
    }
}

// After tool execution:
if let Some((stage, status)) = infer_stage_from_tool(&tool_name) {
    window.emit("agent:stage-change", AgentStageChangePayload {
        agent_id: session.agent_id.clone(),
        stage: stage.to_string(),
        status: status.to_string(),
        elapsed_ms: Some(elapsed.as_millis() as u64),
        reason: None,
    })?;
}
```

### 10.2 agent:token-update

Emit from the streaming loop in `src-tauri/src/provider/streaming.rs` every N tokens:

```rust
// src-tauri/src/provider/streaming.rs

// Inside the stream chunk handler:
session_token_count += chunk.token_count.unwrap_or(0);
window_token_count  += chunk.token_count.unwrap_or(0);

// Emit every 20 tokens to keep frontend responsive without flooding
if session_token_count % 20 == 0 {
    let _ = window.emit("agent:token-update", AgentTokenUpdatePayload {
        agent_id: session.agent_id.clone(),
        total_tokens: session_token_count,
        window_tokens: window_token_count,
    });
    window_token_count = 0;  // reset window counter
}
```

### 10.3 agent:error and agent:retry

Emit from the error handling path in `src-tauri/src/agent/engine.rs`:

```rust
// On tool error:
window.emit("agent:error", AgentErrorPayload {
    agent_id: session.agent_id.clone(),
    error_message: err.to_string(),
    tool_call_id: Some(call.id.clone()),
    is_fatal: false,
})?;

// On retry:
window.emit("agent:retry", AgentRetryPayload {
    agent_id: session.agent_id.clone(),
    attempt: retry_count,
    max_attempts: MAX_RETRIES,
    reason: err.to_string(),
})?;
```

---

## 11. Design Tokens Reference

All values must match the existing AutoDev design system. These are the specific tokens used in this feature and their exact values:

| Token | Value | Used In |
|---|---|---|
| `--bg-app` | `#0A0A0A` | Card expanded background |
| `--bg-card` | `#111111` | Agent live card background |
| `--bg-code` | `#161616` | Mini log tail background |
| `--border-primary` | `#1A1A1A` | Card default border, log line separator |
| `--border-secondary` | `#222222` | Card hover/active border |
| `--accent` | `#00FF88` | Done stage, ✓ icon, LIVE badge dot, active sparkline bar |
| `--accent-muted` | `#00FF8812` | Done stage background |
| `--warning` | `#FFAA00` | Current stage, slow tool warning, stale badge, token bar 70%+ |
| `--warning-muted` | `rgba(255,170,0,0.06)` | Current stage background |
| `--danger` | `#FF4444` | Error stage, ✗ icon, error border, token bar 90%+ |
| `--danger-muted` | `rgba(255,68,68,0.03)` | Error card background |
| `--info` | `#4477FF` | Tool name color in log tail |
| `--text-primary` | `#888888` | Agent name in card header |
| `--text-secondary` | `#555555` | Log line argument text |
| `--text-muted` | `#444444` | Pending stage text, section label |
| `--text-dim` | `#333333` | Log timestamps, section last-event time |
| `--font-mono` | `'IBM Plex Mono', monospace` | Everything in Live Monitor |

### CSS Class Naming Conventions (BEM-style)

```
.live-monitor-section          # Section wrapper
.live-monitor-grid             # Card grid
.agent-live-card               # Card base
.agent-live-card--active       # State modifier
.agent-live-card--idle
.agent-live-card--error
.agent-live-card--expanded
.stage-pipeline                # Pipeline strip wrapper
.stage-pill                    # Individual stage pill
.stage-pill--done
.stage-pill--current
.stage-pill--pending
.stage-pill--error
.mini-log-tail                 # Log container
.log-line                      # Single log line
.log-line--running
.log-line--success
.log-line--error
.log-spinner                   # Animated spinner dot
.token-sparkline               # Sparkline + bar container
.token-bar-fg--normal
.token-bar-fg--warning
.token-bar-fg--danger
.spark-bar
.spark-bar--active
```

---

*This document is the complete spec for the Fleet Live Monitor feature. The companion document `AutoDev_UI_Spec_v2` covers the rest of the AutoDev Panel tab. The event schema in Section 6 should be kept in sync with `src-tauri/src/events/` as the Rust backend is implemented.*