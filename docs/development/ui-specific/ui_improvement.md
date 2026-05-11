# AutoDev — Complete UI/UX Specification
### Single Fleet Tauri v2 Desktop App

**Document Status:** Draft v2.0 (Expanded from Tool Call Spec v1.0)  
**Author:** Razi (Programming Lead, AIRS Foundry)  
**Date:** May 2026  
**Classification:** Internal — AutoDev / Single Fleet  
**Reference Docs:** `AutoDev_Development_Plan`, `AutoDev_Master_Plan`

---

## Table of Contents

1. [Design Language & Tokens](#1-design-language--tokens)
2. [Global App Chrome](#2-global-app-chrome)
3. [App Startup & Sidecar Loading Screen](#3-app-startup--sidecar-loading-screen)
4. [Tab: Chat](#4-tab-chat)
   - 4.1 Chat Header & PA Mode Toggle
   - 4.2 Message Bubbles
   - 4.3 Streaming Text Rendering
   - 4.4 Thought Blocks
   - 4.5 Tool Call Cards (inline approval system)
   - 4.6 Input Bar & Input Meta
   - 4.7 @-Mention File Picker
   - 4.8 /Skill Command Picker
   - 4.9 Plan Display Cards (inline)
5. [Tab: AutoDev Panel](#5-tab-autodev-panel)
   - 5.1 Agent Roster
   - 5.2 Task Board (Kanban)
   - 5.3 Metrics Row
   - 5.4 Sidecar Status Badge
6. [Tab: IDE](#6-tab-ide)
   - 6.1 File Tree
   - 6.2 Code Viewer
   - 6.3 Git Sub-Panel
   - 6.4 Status Bar
7. [Tab: Recipes](#7-tab-recipes)
   - 7.1 Recipe List
   - 7.2 Recipe Detail
8. [Tab: Settings](#8-tab-settings)
   - 8.1 Providers Table
   - 8.2 Ollama Settings
   - 8.3 Model Router
   - 8.4 AIRS AutoDev Config
   - 8.5 Services Panel (Sidecars)
   - 8.6 MCP Tool Browser
9. [Tab: Logs](#9-tab-logs)
10. [Persistent Sidebar](#10-persistent-sidebar)
    - 10.1 Context Files
    - 10.2 MCP Active Connections
    - 10.3 Skills
    - 10.4 Hooks
    - 10.5 Session Info (Tokens & Cost)
    - 10.6 Plan Card
11. [Plan System UI](#11-plan-system-ui)
12. [Notification System](#12-notification-system)
13. [Permission Mode System](#13-permission-mode-system)
14. [Cost & Token Visualization](#14-cost--token-visualization)
15. [Memory Health Visualization](#15-memory-health-visualization)
16. [Tool Call Card Spec (Full)](#16-tool-call-card-spec-full)
    - 16.1 Design Principles
    - 16.2 Card Anatomy & State Machine
    - 16.3 `edit_file` Card
    - 16.4 `bash` Card
    - 16.5 `read_file` Card
    - 16.6 `write_file` Card (new file)
    - 16.7 `list_directory` Card
    - 16.8 MCP Tool Call Card
    - 16.9 `planner` Card
    - 16.10 Collapsed Card States
17. [Future Tool Cards Stub](#17-future-tool-cards-stub)
18. [Implementation Reference](#18-implementation-reference)

---

## 1. Design Language & Tokens

All components in AutoDev derive from a single shared token set. Every value below is authoritative.

### 1.1 Color Tokens

| Token | Value | Usage |
|---|---|---|
| `--bg-app` | `#0A0A0A` | App root background |
| `--bg-surface` | `#0D0D0D` | TitleBar, TabBar, Sidebar, Panels |
| `--bg-card` | `#111111` | Message bubbles, cards, inputs |
| `--bg-code` | `#161616` | Code blocks, diff views, terminal output |
| `--bg-darker` | `#0A0A0A` | Terminal inner, deepest nesting |
| `--border-primary` | `#1A1A1A` | Most borders (subtle) |
| `--border-secondary` | `#222222` | Heavier borders, active card outlines |
| `--accent` | `#00FF88` | Active states, CTAs, success, running indicators |
| `--accent-muted` | `#00FF8812` | User message background, hover states |
| `--accent-border` | `#00FF8825` | User message borders, subtle green tint |
| `--danger` | `#FF4444` | Errors, agent error state, denied cards |
| `--danger-muted` | `rgba(255,68,68,0.08)` | Error background tints in diff/terminal |
| `--warning` | `#FFAA00` | In-progress, working, expiry warnings |
| `--warning-muted` | `rgba(255,170,0,0.08)` | Destructive command warning band |
| `--info` | `#4477FF` | Tool names, keywords, links, MCP labels |
| `--info-muted` | `rgba(68,119,255,0.08)` | Info section backgrounds |
| `--text-primary` | `#888888` | Primary text (intentionally NOT white — industrial muted) |
| `--text-secondary` | `#555555` | Secondary text, sidebar items |
| `--text-muted` | `#444444` | Labels, column headers |
| `--text-dim` | `#333333` | Inactive items, timestamps |
| `--text-bright` | `#F0F0F0` | Code content, diff lines, terminal output |
| `--green-add` | `#00FF88` | Diff additions prefix |
| `--red-del` | `#FF4444` | Diff deletions prefix |
| `--add-bg` | `rgba(0,255,136,0.08)` | Diff addition line background |
| `--del-bg` | `rgba(255,68,68,0.08)` | Diff deletion line background |

### 1.2 Typography Tokens

| Token | Value | Usage |
|---|---|---|
| `--font-mono` | `'IBM Plex Mono', monospace` | Code, agent names, tool calls, metrics, all labels |
| `--font-sans` | `'IBM Plex Sans', sans-serif` | Prose, message text, descriptions |
| `--radius` | `2px` | **All** border radius — sharp, industrial, no soft rounding |

### 1.3 Typography Scale

| Size | Usage |
|---|---|
| 9px | Metric labels (uppercase), badge inner text |
| 10px | Section headers, cost/token readouts, timestamps, meta text |
| 11px | Sidebar items, roster rows, task cards, recipe items, settings rows |
| 12px | Tab labels, code text, send button, diff line numbers |
| 13px | Chat messages, input text, tool arg previews |
| 16px | Metric values (large numbers in MetricsRow) |

### 1.4 Spacing System

Base unit: `8px`. Use multiples: `4px`, `6px`, `8px`, `10px`, `12px`, `14px`, `16px`, `24px`.

### 1.5 Agent State Color Mapping

| State | Color | Usage |
|---|---|---|
| Active (polling) | `#00FF88` accent | Dot, badge outline |
| Working (task running) | `#FFAA00` warning | Dot, pulsing animation |
| Idle (created, not started) | `#4477FF` info | Dot |
| Error | `#FF4444` danger | Dot, border |
| Stopped / Offline | `#333333` dim | Dot |

---

## 2. Global App Chrome

### 2.1 TitleBar

```
┌──────────────────────────────────────────────────────────────────────┐
│  ·  AutoDev — v1.0.0                            [ _ ] [ □ ] [ × ]   │
└──────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `height: 32px`, `background: var(--bg-surface)`, `border-bottom: 1px solid var(--border-primary)` |
| App name | `IBM Plex Mono 11px`, `color: var(--text-secondary)`, `letter-spacing: 0.05em` |
| Version badge | `10px`, `color: var(--text-dim)`, inline after dash |
| Window controls | Native Tauri decorations OR custom 3-dot: close `#FF4444`, minimize `#FFAA00`, maximize `#00FF88` — no labels, just dots |
| Drag region | Full width via `data-tauri-drag-region` — all non-button area draggable |

### 2.2 TabBar

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│  Chat  │  AutoDev  │  IDE  │  Recipes  │  Logs  │  Settings  │   model  │ $cost │
└──────────────────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `height: 36px`, `background: var(--bg-surface)`, `border-bottom: 1px solid var(--border-primary)` |
| Tabs | `IBM Plex Mono 12px`, `padding: 0 14px`, `color: var(--text-dim)` inactive, `color: var(--text-primary)` active |
| Active tab | Bottom border `2px solid var(--accent)`, background `var(--bg-card)` |
| Hover tab | `color: var(--text-secondary)`, background `rgba(255,255,255,0.02)` |
| Model pill | Right-aligned, `10px`, `background: var(--bg-card)`, `border: 1px solid var(--border-secondary)`, `border-radius: 2px`, `padding: 2px 8px` — shows active model short name (e.g. `claude-sonnet-4-6`) |
| Cost pill | Adjacent to model pill, `color: var(--accent)`, shows `$0.04` — resets on `/clear context` |
| Tab ordering | Chat → AutoDev → IDE → Recipes → Logs → Settings |
| Badge on AutoDev tab | Shows count of active tasks in a `#FFAA00` dot badge when workers are running |
| Badge on Logs tab | Shows `ERROR` count in a `#FF4444` dot badge when errors present in last 100 lines |

### 2.3 StatusBar

```
┌──────────────────────────────────────────────────────────────────────┐
│  ● PA  ·  Fast Dev Mode  │  ● github-mcp  ● slack-mcp  │  12,450 tk │
└──────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `height: 22px`, `background: var(--bg-surface)`, `border-top: 1px solid var(--border-primary)` |
| Font | `IBM Plex Mono 10px`, `color: var(--text-dim)` |
| PA mode indicator | Colored dot (`var(--accent)`) + current PA sub-mode name — click to open mode dropdown |
| MCP status dots | `● green` active, `● red` error per MCP server — click to open sidebar MCP section |
| Token count | Far right, `color: var(--text-secondary)` — updates live as tokens accumulate |
| All sections | Separated by `·` dividers, `color: var(--text-dim)` |

---

## 3. App Startup & Sidecar Loading Screen

> **Problem**: Python sidecar cold start (memory-mcp-server + core-orchestrator) takes 5–10 seconds. The user must see exactly what is happening — not a blank screen, not a generic spinner.

### 3.1 Startup Sequence Screen

Shown only on first load. Full-window overlay on `--bg-app` background.

```
┌──────────────────────────────────────────────────────────────────────┐
│                                                                        │
│                          AUTODEV                                      │
│                        AIRS Foundry                                   │
│                                                                        │
│   [■■■■■■■■■■■■■■■■■■■■■■■■■■░░░░░░░░░░░░░░░░░░░░░░]  58%          │
│                                                                        │
│   ● Starting memory-mcp-server...              :8000  ✓ 1.2s         │
│   ◌ Starting core-orchestrator...              :4000  (3.4s)         │
│   ○ Connecting autodev-mcp-server...           :8767  waiting...     │
│   ○ Checking Redis...                          :6379  waiting...     │
│                                                                        │
│                   Press Esc to continue without fleet                 │
│                                                                        │
└──────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| App name | `IBM Plex Mono 18px`, `color: var(--text-primary)`, centered |
| Org name | `IBM Plex Mono 11px`, `color: var(--text-dim)`, below app name |
| Progress bar | `height: 3px`, `background: var(--border-primary)`, fill `var(--accent)`, `width: 320px` — animated fill as each service comes up |
| Progress % | `IBM Plex Mono 11px`, `color: var(--text-secondary)`, right of bar |
| Service row — waiting | `○` circle, `color: var(--text-dim)` |
| Service row — starting | `◌` spinning, `color: var(--warning)`, elapsed time in parentheses, animating |
| Service row — done | `●` solid dot, `color: var(--accent)`, real elapsed time shown |
| Service row — error | `✗` red, `color: var(--danger)`, error short message |
| Port label | Right-aligned, `color: var(--text-dim)` — shows `:port` for each service |
| Escape hint | `10px`, `color: var(--text-dim)`, centered at bottom — allows using app in PA-only mode without fleet |
| Background | `--bg-app` (`#0A0A0A`) — no logo, no splash art. Pure text, industrial |

### 3.2 Already-Running Detection

If both `:4000` and `:8000` are already healthy on startup (externally started), the loading screen skips entirely and shows a brief **1-line inline toast** in the StatusBar:

```
↑ Fleet already running · core-orch :4000 · memory-mcp :8000 · autodev-mcp :8767
```

This matches the startup probe logic in `main.rs`.

### 3.3 Partial Start (PA-Only Mode)

If the user presses Escape or sidecars fail, the app enters **PA-Only Mode**. The AutoDev tab gets a grayed-out badge `FLEET OFFLINE`. All other tabs function normally. A persistent warning strip sits at the top of the AutoDev tab:

```
┌──────────────────────────────────────────────────────────────────┐
│  ⚠  Fleet sidecars not running  ·  [Start Fleet]  [Dismiss]     │
└──────────────────────────────────────────────────────────────────┘
```

---

## 4. Tab: Chat

### 4.1 Chat Header & PA Mode Toggle

```
┌──────────────────────────────────────────────────────────────────────┐
│  PA  [🔧 Fast Development ▾]    4 MCP · 4 skills · AUTODEV.md ✓    │
└──────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `height: 40px`, `background: var(--bg-card)`, `border-bottom: 1px solid var(--border-primary)`, `padding: 0 12px` |
| "PA" label | `IBM Plex Mono 10px`, `color: var(--text-dim)`, uppercase |
| Mode dropdown | `IBM Plex Mono 11px`, `color: var(--text-primary)`, chevron `▾`, dropdown on click |
| MCP count | `color: var(--info)`, `10px` — clicking opens sidebar MCP section |
| Skills count | `color: var(--text-secondary)`, `10px` |
| AUTODEV.md status | `✓` green if loaded, `✗` red if missing, `~` yellow if fallback |

**Mode Dropdown Options:**

```
┌─────────────────────────────────────────────────────────────────┐
│  🔧  Fast Development  — Full file tools + all MCP             │
│  ✋  Review Mode       — Full tools + permission checks         │
│  📋  Delegate Mode     — create_task only · zero file tools     │
└─────────────────────────────────────────────────────────────────┘
```

| Mode | Dot Color | Tool Set | Behavior |
|---|---|---|---|
| Fast Development | `var(--accent)` green | All builtin + all MCP | Direct file editing, no approval prompts |
| Review Mode | `var(--warning)` amber | All builtin + all MCP + permission wrapper | Every write/bash shows Allow/Deny card |
| Delegate Mode | `var(--info)` blue | `create_task` + read-only MCP | No file editing; creates tasks on internal board |

Switching mode triggers system prompt reload (swaps between `BASE_SYSTEM_PROMPT`, `REVIEW_SYSTEM_PROMPT`, `DELEGATE_SYSTEM_PROMPT`) and tool registry refilter. The PA sub-mode selection is **persistent per session** in Zustand and survives tab switches.

### 4.2 Message Bubbles

#### User Message

```
┌─────────────────────────────────────────────────────────────────┐
│                    Add /health endpoint to orchestrator         │
│                                            Razi · 14:32 ─────  │
└─────────────────────────────────────────────────────────────────┘
```

| Property | Value |
|---|---|
| Alignment | Right-aligned (flex-end) |
| Background | `var(--accent-muted)` (`#00FF8812`) |
| Border | `1px solid var(--accent-border)` (`#00FF8825`) |
| Border-radius | `2px` |
| Font | `IBM Plex Sans 13px`, `color: var(--text-primary)` |
| Timestamp | `IBM Plex Mono 10px`, `color: var(--text-dim)` |
| Sender label | `IBM Plex Mono 10px`, `color: var(--text-dim)` — uses agent identity name |

#### Agent Message

```
┌─────────────────────────────────────────────────────────────────┐
│  PA                                                              │
│  I'll fix those 2 compilation errors now.                       │
│                                                        14:32 ─  │
└─────────────────────────────────────────────────────────────────┘
```

| Property | Value |
|---|---|
| Alignment | Left-aligned (flex-start) |
| Background | `var(--bg-card)` (`#111111`) |
| Border | `1px solid var(--border-primary)` |
| Font | `IBM Plex Sans 13px`, `color: var(--text-primary)` |
| Agent label | `IBM Plex Mono 10px`, `color: var(--text-dim)` top-left of bubble |

#### Inline Code in Messages

Inline code spans (backtick-wrapped) render as:
- Background: `var(--bg-code)` (`#161616`)
- Font: `IBM Plex Mono 12px`
- Border: `1px solid var(--border-secondary)`
- Border-radius: `2px`
- Padding: `1px 5px`

Multi-line code fences render as a full code block with a language badge, line numbers, and a `[copy]` button in the top-right corner.

### 4.3 Streaming Text Rendering

While the agent is generating, the message bubble renders with a blinking cursor appended to the last word. Text streams token-by-token with no word-by-word flicker — the StreamBroker's adaptive cadence buffering means chunks arrive ~every 50ms in batches.

```
I'll fix those 2 compilation errors now. Let me read the affected▌
```

| Element | Spec |
|---|---|
| Streaming cursor | `▌` character, blinks at 700ms interval, `color: var(--accent)` |
| On `done: true` event | Cursor removed; timestamp appended |
| Abort button | `[✗ Stop]` button appears in InputBar while streaming — sends cancellation signal |
| Error mid-stream | Cursor replaced with `⚠ Generation interrupted`, `color: var(--warning)` |

### 4.4 Thought Blocks

Between tool calls, the agent emits thinking blocks. These render as collapsible sections inside the agent message bubble:

```
▶ Thoughts  ···
```

Expanded:

```
▼ Thoughts
  ─────────────────────────────────────────────────────────
  The PAMode enum is missing from mod.rs. I need to add
  Review and Delegate variants before engine.rs will compile.
  Let me read mod.rs first to understand the current state.
  ─────────────────────────────────────────────────────────
```

| Element | Spec |
|---|---|
| Toggle | `▶ Thoughts` collapsed, `▼ Thoughts` expanded — `IBM Plex Mono 11px`, `color: var(--text-dim)` |
| Content area | `background: var(--bg-code)`, `border-left: 2px solid var(--border-secondary)`, `padding: 8px 12px` |
| Text | `IBM Plex Mono 12px`, `color: var(--text-secondary)` — noticeably dimmer than main message content |
| Streaming state | `···` animated dots after label while thinking is in progress |
| Default state | Always collapsed after the turn completes. Expanded only while streaming (so user can optionally watch) |

### 4.5 Tool Call Cards

> **Full spec lives in Section 16.** Summary of placement rules here.

Tool call cards render **inline inside the assistant message bubble**, in execution order. The agent text above the first card, then card, then more text or more cards, then final text. This creates a single flowing turn that contains narrative + evidence + results.

A turn with multiple tool calls looks like:

```
┌─ agent message ──────────────────────────────────────────────────────────┐
│  I'll fix those 2 compilation errors now.                                │
│  ▶ Thoughts                                                               │
│  [edit_file card — engine.rs — ✓ Done — collapsed]                       │
│  ▶ Thoughts                                                               │
│  Now fix the PAMode enum issue:                                          │
│  ▶ Thoughts                                                               │
│  [edit_file card — mod.rs — ✓ Done — collapsed]                          │
│  ▶ Thoughts                                                               │
│  Both errors resolved. Running cargo check to verify:                    │
│  [bash card — cargo check 2>&1 — ● PENDING — EXPANDED]                   │
│  [✓ Allow]  [✗ Deny]                      [Edit command before run]      │
└──────────────────────────────────────────────────────────────────────────┘
```

The **pending card** is always the last card in the turn and always stays expanded. All resolved cards collapse to single-line summaries.

### 4.6 Input Bar & Input Meta

```
┌──────────────────────────────────────────────────────────────────────┐
│  Say something...                                              [Send] │
└──────────────────────────────────────────────────────────────────────┘
│  Attach · /skill · @agent · #recipe              12,450 tk · $0.04  │
```

| Element | Spec |
|---|---|
| Textarea | `background: var(--bg-card)`, `border: 1px solid var(--border-secondary)`, `border-radius: 2px`, `font: IBM Plex Sans 13px`, auto-resize up to 8 lines then scroll, `min-height: 40px` |
| Placeholder | `color: var(--text-dim)` |
| Send button | `background: var(--accent)`, `color: #000`, `IBM Plex Mono 12px`, `border-radius: 2px`, `padding: 6px 14px` |
| Meta bar | `IBM Plex Mono 10px`, `color: var(--text-dim)` — below input, single row |
| Meta actions | `Attach` (file attach), `/skill` (opens skill picker), `@agent` (agent mention), `#recipe` (recipe trigger) — each on click opens picker |
| Token/cost readout | Far right of meta bar — live token count and session cost |
| While streaming | Send button replaced with `[✗ Stop]`, `background: var(--danger)` |

### 4.7 @-Mention File Picker

Triggered by typing `@` in the input bar. Fuzzy-matches file and directory names from the working directory.

```
┌────────────────────────────────────────────┐
│  @src                                      │
├────────────────────────────────────────────┤
│  □  src/                     (47 files)   │
│  □  src/api/                  (3 files)   │
│  </>  src/main.py                         │
│  </>  src/agent/engine.rs                 │
│  </>  src/agent/mod.rs                    │
└────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `background: var(--bg-card)`, `border: 1px solid var(--border-secondary)`, `border-radius: 2px`, positioned above input bar, max 8 items before scroll |
| Directory icon | `□` in `#FFAA00` (gold) with `(N files)` count in `var(--text-dim)` |
| File icon | `</>` in `var(--info)` |
| Keyboard nav | `↑↓` to move, `Enter` to select (inserts absolute path), `Escape` to close |
| Fuzzy matching | Applied to both file and folder names simultaneously |
| Inserted text | Full absolute path wrapped in backticks, appended to message |

### 4.8 /Skill Command Picker

Triggered by typing `/` in the input bar.

```
┌────────────────────────────────────────────┐
│  /review                                   │
├────────────────────────────────────────────┤
│  /review-pr     Review open pull request  │
│  /run-tests     Run test suite & report   │
└────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | Same as @-mention picker |
| Skill name | `IBM Plex Mono 11px`, `color: var(--accent)` |
| Description | `IBM Plex Sans 11px`, `color: var(--text-secondary)` |
| Selection | Replaces the `/word` with a special `[skill: /review-pr]` token in the message, rendered as a green pill in the input bar |
| Active skills | Only shows skills currently loaded per sidebar |

### 4.9 Plan Display Cards (Inline)

When the PA agent calls the `planner` tool to create a plan, a plan card renders inline in the chat message:

```
┌──────────────────────────────────────────────────────────────────┐
│  📋  Plan: Add /health endpoint to orchestrator        [● Active] │
├──────────────────────────────────────────────────────────────────┤
│  ▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░  3/7 steps done                     │
├──────────────────────────────────────────────────────────────────┤
│  ✓  1. Read existing route structure                              │
│  ✓  2. Add health endpoint handler function                      │
│  ✓  3. Register route in api router                              │
│  ◌  4. Add response model to schemas.py            (in progress) │
│  ○  5. Write unit tests                                           │
│  ○  6. Run tests and fix failures                                 │
│  ○  7. Verify with cargo check / pytest                           │
├──────────────────────────────────────────────────────────────────┤
│  [Jump to Plan]                              Scope: personal      │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Header | `📋 Plan:` label + plan title, `IBM Plex Mono 12px` |
| State badge | `[● Active]` — same state colors as agent roster states |
| Progress bar | `height: 4px`, fill `var(--accent)`, proportional to done/total |
| Progress label | `N/M steps done`, `10px`, `color: var(--text-secondary)` |
| Done step | `✓` green prefix, text `color: var(--text-secondary)` (dimmed — already done) |
| In-progress step | `◌` spinning amber, text `color: var(--text-primary)`, `(in progress)` suffix |
| Pending step | `○` hollow circle, text `color: var(--text-dim)` |
| Blocked step | `⊘` orange circle, text `color: var(--warning)`, `(blocked)` suffix |
| Jump button | Opens Plan Detail View in sidebar |
| Scope badge | `personal` / `fleet` / `epic` in bottom-right, `10px`, `color: var(--text-dim)` |

When a step completes, the card mutates in place — step icon changes from `◌` to `✓`, progress bar fills, counter increments.

---

## 5. Tab: AutoDev Panel

This tab is the fleet command center. It is only fully functional when sidecars are running. In PA-Only mode, a warning strip shows and all roster/task data is empty.

### 5.1 Agent Roster

A compact table showing all configured agents and their live status.

```
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  AGENT ROSTER                                               [+ Add Worker]           │
├──────────────────────────────────────────────────────────────────────────────────────┤
│  STATUS    NAME               CURRENT TASK                          TIME    ACTIONS  │
├──────────────────────────────────────────────────────────────────────────────────────┤
│  ● Active  SUPERVISOR         Polling task board (idle)              —       [■ Stop]│
│  ● Active  QC AGENT           Reviewing PR #87                      04:22    [■ Stop]│
│  ● Working WORKER-BACKEND     Issue #142 — /agents/health endpoint  01:08    [■ Stop]│
│  ○ Idle    WORKER-FRONTEND    —                                      —       [▶ Start]│
│  ○ Idle    WORKER-PLATFORM    —                                      —       [▶ Start]│
│  ○ Idle    WORKER-MCP         —                                      —       [▶ Start]│
│  ● Working WORKER-CLEANUP     Formatting src/api/**                 00:33    [■ Stop]│
├──────────────────────────────────────────────────────────────────────────────────────┤
│  + WORKER-DOCS  + WORKER-TESTING                                                     │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `background: var(--bg-card)`, `border: 1px solid var(--border-primary)`, `border-radius: 2px` |
| Header | `IBM Plex Mono 10px uppercase`, `color: var(--text-muted)`, `padding: 8px 12px` |
| Row font | `IBM Plex Mono 11px` |
| Status dot | 8px circle, colored per agent state table (Section 1.5) |
| Name | `color: var(--text-primary)`, UPPERCASE |
| Current task | `color: var(--text-secondary)`, truncated to fit with `…` |
| Time | `IBM Plex Mono 10px`, `color: var(--text-dim)`, MM:SS format, counts up while working |
| System agents | SUPERVISOR and QC AGENT cannot be deleted — no `[✗ Delete]` button. `[■ Stop]` / `[▶ Start]` only |
| Worker agents | Have `[■ Stop]` / `[▶ Start]` / `[✗ Delete]` |
| Add Worker button | Opens modal to create a new custom worker recipe |
| Working dot | `var(--warning)` with a subtle CSS `pulse` animation (opacity 0.6 → 1.0 → 0.6, 1.5s loop) |
| Row click | Expands inline to show last 5 log lines from that agent's recipe run, with `[Open in Logs]` link |

### 5.2 Task Board (Kanban)

4-column kanban. The single most-watched view during a live fleet run.

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│   READY (3)          IN PROGRESS (2)      QC REVIEW (1)         DONE TODAY (5)      │
├─────────────┬─────────────────────────┬────────────────────┬──────────────────────┤
│  #142 p2    │  #141 p1                │  #139 p1           │  ✓ #138 merged       │
│  /health    │  Add auth middleware    │  PR #87 open       │  ✓ #137 merged       │
│  endpoint   │  WORKER-BACKEND        │  QC AGENT          │  ✓ #136 merged       │
│             │  ████░░░░░░░  40%      │  04:22 elapsed     │  ✓ #135 merged       │
│  #143 p3    │                         │                    │  ✓ #134 merged       │
│  Cleanup    │  #140 p2                │                    │                      │
│  dead routes│  Refactor logging       │                    │                      │
│             │  WORKER-CLEANUP         │                    │                      │
│  #144 p2    │  ████████░░  80%       │                    │                      │
│  Add linting│                         │                    │                      │
│  hook       │                         │                    │                      │
└─────────────┴─────────────────────────┴────────────────────┴──────────────────────┘
```

| Element | Spec |
|---|---|
| Column header | `IBM Plex Mono 10px uppercase`, `color: var(--text-muted)`, count badge in parens |
| Column background | `var(--bg-app)` |
| Card background | `var(--bg-card)`, `border: 1px solid var(--border-primary)`, `border-radius: 2px`, `padding: 8px 10px` |
| Task ID | `IBM Plex Mono 10px`, `color: var(--info)` — e.g. `#142` |
| Priority badge | `p0` red, `p1` orange, `p2` yellow, `p3` gray — all `9px uppercase` |
| Task title | `IBM Plex Sans 11px`, `color: var(--text-primary)`, max 2 lines |
| Worker label | `IBM Plex Mono 10px`, `color: var(--warning)` when working |
| Progress bar | `height: 3px`, `background: var(--border-primary)`, fill `var(--accent)` — only on in-progress cards |
| % label | `10px`, right-aligned below bar |
| QC column card | Border `1px solid var(--warning)`, PR link shown |
| Done card | `✓` prefix `color: var(--accent)`, text `color: var(--text-dim)` — muted since it's complete |
| Card click | Opens Task Detail modal (title, description, acceptance criteria, history, linked PR, worker log) |
| Drag-and-drop | Cards draggable between Ready, Blocked, and Done columns — In Progress and QC columns are read-only (set by agents) |

**Task Detail Modal:**

Click any card to open a full-width modal:
- Title, description, acceptance criteria (rendered markdown)
- Status history timeline (who changed what when)
- Assigned worker
- GitHub PR link (if created)
- Token cost so far
- `[View in IDE]` button — opens file tree to the worktree for this task

### 5.3 Metrics Row

4 metric cards at the top of the AutoDev tab, always visible:

```
┌────────────────┬────────────────┬────────────────┬────────────────┐
│  COST TODAY    │  TOKENS        │  CLOSED        │  QC PASS RATE  │
│  $2.14         │  1.2M          │  5             │  80%           │
└────────────────┴────────────────┴────────────────┴────────────────┘
```

| Element | Spec |
|---|---|
| Container | `background: var(--bg-card)`, `border: 1px solid var(--border-primary)`, `border-radius: 2px`, `padding: 10px 14px` |
| Label | `IBM Plex Mono 9px uppercase`, `color: var(--text-muted)` |
| Value | `IBM Plex Mono 16px`, `color: var(--text-primary)` |
| Cost card value | `color: var(--accent)` when under budget, `color: var(--warning)` when over |
| QC Pass Rate | `color: var(--warning)` when below 100%, `color: var(--accent)` at 100% |
| Metrics update | Live via Tauri event subscription — no polling required |
| Click behavior | Each card expands to a mini-chart (sparkline) of the last 24h trend |

### 5.4 Sidecar Status Badge

Shown in the AutoDev tab header, adjacent to the "AUTODEV PANEL" title:

```
● memory-mcp  ● core-orch  ● autodev-mcp  ○ redis
```

- Dots colored per service health state (green healthy, amber starting, red error, gray stopped)
- Click opens the **Services panel** in Settings tab
- If any required service (memory-mcp, core-orch) is unhealthy, the dot pulses red and a tooltip explains

---

## 6. Tab: IDE

The IDE tab is a minimal but functional single-pane-of-glass for watching agents work in the codebase — and for doing quick Razi-driven edits. It is NOT a full IDE replacement. It complements the Chat tab.

### 6.1 File Tree

```
┌─────────────────────────────────┐
│  REPO  core-orchestrator  ▾    │
├─────────────────────────────────┤
│  ▶ src/                         │
│    ▼ api/                       │
│        health.py  ● modified    │
│        routes.py                │
│        __init__.py              │
│    ▶ agent/                     │
│    ▶ memory/                    │
│  ▶ tests/                       │
│  ▶ config/                      │
│      requirements.txt           │
└─────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `width: 180px`, `background: var(--bg-surface)`, `border-right: 1px solid var(--border-primary)` |
| Repo selector | Dropdown at top — shows all repos with active worktrees + main |
| Folder icons | `▶` collapsed, `▼` expanded, `IBM Plex Mono 11px`, `color: var(--text-dim)` |
| File | `IBM Plex Mono 11px`, `color: var(--text-secondary)` |
| Modified indicator | `● modified` dot `color: var(--warning)`, suffix text `color: var(--text-dim)` |
| Added file | `● new` dot `color: var(--accent)` |
| Deleted file | Strikethrough text, `color: var(--danger)` |
| Active file | Background `var(--accent-muted)`, border-left `2px solid var(--accent)` |
| Context menu | Right-click: Open, Copy Path, Reveal in Explorer, View Git Diff |

### 6.2 Code Viewer

```
┌──────────────────────────────────────────────────────────────────┐
│  src/api/health.py  ·  3 changes        [< prev]  [next >]  [✓] │
├──────────────────────────────────────────────────────────────────┤
│    1 │  from fastapi import APIRouter                            │
│    2 │  from ..services import get_agent_service                 │
│    3 │                                                            │
│    4 │  router = APIRouter()                                     │
│    5 │                                                            │
│  + 6 │  @router.get("/agents/health")                            │
│  + 7 │  async def agents_health():                               │
│  + 8 │      """Return agent status, uptime, and memory usage.""" │
│  + 9 │      svc = get_agent_service()                            │
│  +10 │      return await svc.health()                            │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Header | File path, change count, prev/next change navigation buttons, accept all `[✓]` |
| Line numbers | `IBM Plex Mono 12px`, `color: var(--text-dim)`, right-aligned in 4-char column |
| Code | `IBM Plex Mono 12px`, `color: var(--text-bright)` |
| `+` lines | Background `var(--add-bg)`, prefix `color: var(--accent)`, line number `color: var(--accent)` |
| `-` lines | Background `var(--del-bg)`, prefix `color: var(--danger)`, strikethrough |
| Context lines | No prefix (space), no highlight |
| Change navigation | `[< prev]` / `[next >]` jump between changed hunks — `IBM Plex Mono 10px` |
| Accept all button | `[✓]` — marks all changes accepted in worktree tracking |
| Syntax highlighting | Full language-aware token coloring based on file extension |
| Gutter | 16px left gutter — future: breakpoints, annotations |

### 6.3 Git Sub-Panel

Toggle button `[⎇ Git]` in the IDE tab header opens/closes the git sub-panel below the code viewer (split horizontal — code 65%, git 35%).

```
┌──────────────────────────────────────────────────────────────────┐
│  ⎇ Git  ·  main  ←  feat/agents-health  ·  3 commits ahead     │
├──────────────────────────────────────────────────────────────────┤
│  WORKTREES                                                        │
│  ● feat/agents-health  ·  task #142  ·  WORKER-BACKEND          │
│  ○ main                ·  clean                                   │
├──────────────────────────────────────────────────────────────────┤
│  STAGED                               UNSTAGED                   │
│  src/api/health.py  +5               src/api/routes.py  ~2      │
├──────────────────────────────────────────────────────────────────┤
│  Commit message:  [Add GET /agents/health endpoint_________]     │
│  [Stage All]  [✓ Commit]  [↑ Push]  [⤴ Create PR]              │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Branch indicator | Active branch + base branch + commit count ahead |
| Worktree list | Each entry: `●/○` status dot, branch name, linked task ID, assigned worker |
| Staged/Unstaged | Two columns showing file name + change count (`+N` green, `~N` amber, `-N` red) |
| Commit field | Editable single-line input, pre-filled by agent if it just created changes |
| Action buttons | Stage All, Commit, Push, Create PR — all require explicit click (no auto-action) |
| Push button | Always triggers explicit confirmation: "Push `N commits` to origin/`branch`?" |
| Create PR button | Opens PR creation modal with title, description (pre-filled by agent), base branch selector |

### 6.4 Status Bar (IDE-specific)

At the bottom of the IDE tab only:

```
┌──────────────────────────────────────────────────────────────────────┐
│  pytest ✅ 47/47  │  mypy ✅ 0 errors  │  ⎇ feat/agents-health     │
└──────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| pytest result | `✅ passed / ❌ failed` + count |
| mypy result | `✅ 0 errors / ❌ N errors` |
| Active git branch | `⎇ branch-name`, click to switch branches |
| All items | `IBM Plex Mono 10px`, `height: 22px`, `background: var(--bg-surface)` |

---

## 7. Tab: Recipes

### 7.1 Recipe List

```
┌──────────────────────────────────────────┐
│  RECIPES                    [+ New]      │
├──────────────────────────────────────────┤
│  ● worker-backend-poll                   │
│    Worker Backend polling loop           │
│    */2 * * * *  ·  autonomous            │
│                                          │
│  ● supervisor-dispatch                   │
│    Supervisor task distribution          │
│    */2 * * * *  ·  autonomous            │
│                                          │
│  ○ qc-review-cycle                       │
│    QC Agent PR review                    │
│    */5 * * * *  ·  manual                │
│                                          │
│  ● create-task-from-spec                 │
│    PA delegate task creator              │
│    on-demand  ·  review                  │
└──────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | `width: 220px`, `background: var(--bg-surface)`, `border-right: 1px solid var(--border-primary)` |
| Recipe name | `IBM Plex Mono 11px`, `color: var(--text-primary)` |
| Description | `IBM Plex Sans 11px`, `color: var(--text-secondary)` |
| Schedule | `IBM Plex Mono 10px`, `color: var(--text-dim)` |
| Permission mode | `autonomous` green, `review` amber, `manual` blue — `10px` badge |
| Active indicator | `●` green dot — recipe is scheduled and enabled |
| Inactive | `○` hollow dot — disabled |
| Selected | Border-left `2px solid var(--accent)`, background `var(--accent-muted)` |

### 7.2 Recipe Detail

```
┌──────────────────────────────────────────────────────────────────┐
│  worker-backend-poll.yaml              [▶ Run Now]  [Edit YAML]  │
├──────────────────────────────────────────────────────────────────┤
│  1 │  version: "1.0.0"                                          │
│  2 │  title: Worker Backend — Polling Loop                      │
│  3 │  schedule: "*/2 * * * *"                                   │
│  4 │  permission_mode: autonomous                               │
│  5 │  extensions:                                               │
│  6 │    - github-mcp                                            │
│  7 │    - developer                                             │
│  8 │  parameters:                                               │
│  9 │    - name: repo                                            │
│ 10 │      default: "core-orchestrator"                          │
├──────────────────────────────────────────────────────────────────┤
│  LAST 5 RUNS                                                      │
│  ✓ 14:28  00:47  3 tasks claimed    ·  View log                  │
│  ✓ 14:26  00:31  1 task claimed     ·  View log                  │
│  ✗ 14:24  00:12  Error: git lock    ·  View log                  │
│  ✓ 14:22  00:52  2 tasks claimed    ·  View log                  │
│  ✓ 14:20  00:44  0 tasks (idle)     ·  View log                  │
└──────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Filename header | `IBM Plex Mono 12px`, `color: var(--text-primary)` |
| YAML viewer | Syntax-highlighted: keys `color: var(--info)`, values `color: var(--text-bright)`, strings `color: var(--accent)`, comments `color: var(--text-dim)` |
| `[▶ Run Now]` | Triggers immediate recipe execution, overriding schedule. Shows running indicator in recipe list |
| `[Edit YAML]` | Opens inline editor mode — YAML becomes editable. Save / Cancel buttons appear |
| `[Schedule]` | Opens cron expression editor with human-readable preview |
| Run history | Last 5 runs with: status icon, time, duration, outcome summary, view log link |
| Error run | `✗` red, error message truncated |
| `View log` | Opens Logs tab filtered to this recipe's run timestamp |

---

## 8. Tab: Settings

### 8.1 Providers Table

```
┌────────────────────────────────────────────────────────────────────────────────────┐
│  PROVIDER     API KEY                        MODEL              STATUS              │
├────────────────────────────────────────────────────────────────────────────────────┤
│  Anthropic    sk-ant-•••••••••••••••••5a3f   claude-sonnet-4-6  ● Active            │
│  OpenAI       sk-••••••••••••••••••••1c8d    gpt-4o             ○ Off               │
│  Gemini       AI••••••••••••••••••••••3k1j   gemini-pro         ○ Off               │
│  Ollama       Local — no key required        llama3.2:8b        ● Active            │
├────────────────────────────────────────────────────────────────────────────────────┤
│  [+ Add Provider]                                                                    │
└────────────────────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Row font | `IBM Plex Mono 11px` |
| API key | Masked — shows first 8 + `•••` + last 4 chars. Click to reveal/edit. Eye icon `[👁]` |
| Status | `● Active` green, `○ Off` dim |
| Model selector | Dropdown per provider — fetches available models from provider on open |
| `[Test Connection]` | Per-row button — fires test_provider_connection, shows ✓ or ✗ result inline |
| Expiry warning | If PAT/key has an expiry detected, shows `expires 89d` in `color: var(--warning)` |
| Row click | Expands to full API key input, model dropdown, test connection, remove button |

### 8.2 Ollama Settings

Ollama requires a dedicated expanded view because it has no API key — only host/port configuration:

```
┌─────────────────────────────────────────────────────────────────┐
│  Ollama                              [LOCAL] badge              │
├─────────────────────────────────────────────────────────────────┤
│  Host:  [localhost_______________]   Port:  [11434_]            │
│  [Test Connection]           ✓ Connected · 6 models available   │
├─────────────────────────────────────────────────────────────────┤
│  MODEL                 CONTEXT     FAMILY                       │
│  llama3.2:8b           128K        llama3                       │
│  qwen2.5:7b            32K         qwen                         │
│  deepseek-coder:6.7b   16K         deepseek                     │
│  codellama:13b         16K         codellama                     │
│  phi4:latest           128K        phi                           │
│  gemma:7b              8K          gemma                         │
│  [↻ Refresh]                                                    │
├─────────────────────────────────────────────────────────────────┤
│  GENERATION OPTIONS (per-model overrides)                       │
│  num_ctx:    [4096__]   Temperature: [────●──]  0.7             │
│  Top-P:      [──●────]  0.4   Seed:  [optional____________]    │
└─────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| `[LOCAL]` badge | Distinguishes Ollama from cloud providers — `background: var(--info-muted)`, `color: var(--info)`, `10px` |
| Host/Port inputs | Standard text inputs, `IBM Plex Mono 12px`, default `localhost` / `11434` |
| Connected status | Live after test — green ✓ with model count |
| Model table | Hides embedding-only models (`nomic-embed-text`, etc.) |
| Context window | Pulled from hardcoded family table; shows `K` suffix |
| Sliders | Temperature (0.0–2.0), Top-P (0.0–1.0) — sliders with live readout |
| Seed field | Optional integer — for reproducible outputs |

### 8.3 Model Router

Assigns which model handles which agent role:

```
┌────────────────────────────────────────────────────────────────────┐
│  AGENT ROLE           MODEL                REASON                  │
├────────────────────────────────────────────────────────────────────┤
│  Supervisor           claude-opus-4-6      Complex routing logic   │
│  Worker Backend       claude-sonnet-4-6    Balanced quality/cost   │
│  Worker Frontend      claude-sonnet-4-6    Balanced quality/cost   │
│  Worker Cleanup       claude-haiku-4-5     Simple refactors        │
│  QC Agent             claude-opus-4-6      Detailed review         │
└────────────────────────────────────────────────────────────────────┘
```

Each MODEL cell is a dropdown. The REASON is an editable text note — free text, just a reminder for Razi.

### 8.4 AIRS AutoDev Config

```
┌─────────────────────────────────────────────────────────────────┐
│  GitHub Org:         [AIRS-Foundry________________]             │
│  Task Board Repo:    [airs-task-board______________]             │
│  Agent Identity:     ● Personal Assistant (PA)                  │
│  GitHub PAT:         [ghp_•••••••••••••••••••••3j1]  expires 89d│
│  Workers DB Path:    ~/.autodev/workers.db                      │
│  Tasks DB Path:      ~/.autodev/tasks.db                        │
│  Plans DB Path:      ~/.autodev/plans.db                        │
│  AUTODEV.md Path:    ./AUTODEV.md  ✓ found                      │
└─────────────────────────────────────────────────────────────────┘
```

The expiry warning on the PAT (`expires 89d`) uses `color: var(--warning)`. When under 14 days, changes to `color: var(--danger)` and adds a `⚠` icon.

### 8.5 Services Panel (Sidecars)

Shows all 4 services (memory-mcp, core-orchestrator, autodev-mcp, redis) in a live grid:

```
┌──────────────────────────────────────────────────────────────────┐
│  SERVICES                                                         │
├────────────────┬──────────┬──────────┬────────────┬─────────────┤
│  SERVICE       │  PORT    │  STATUS  │  REQUIRED  │  ACTIONS    │
├────────────────┼──────────┼──────────┼────────────┼─────────────┤
│  memory-mcp    │  :8000   │  ● OK    │  required  │  [■ Stop]   │
│  core-orch     │  :4000   │  ● OK    │  required  │  [■ Stop]   │
│  autodev-mcp   │  :8767   │  ● OK    │  required  │  in-process │
│  redis         │  :6379   │  ● OK    │  optional  │  external   │
└────────────────┴──────────┴──────────┴────────────┴─────────────┘
│  [▶ Start Fleet]  [■ Stop Fleet]  [↻ Restart All]  [🔄 10s auto] │
└──────────────────────────────────────────────────────────────────┘
```

Clicking a service name opens a modal with two tabs:
- **Logs tab** — last 500 lines of the service-specific log file with live tail, filter, and copy button
- **Resources tab** — CPU % + memory bytes via sysinfo, refreshing every 5 seconds

### 8.6 MCP Tool Browser

Shows all tools exposed by the autodev-mcp-server:

```
┌──────────────────────────────────────────────────────────────────┐
│  MCP TOOLS  [🔍 search tools...]                  15 tools       │
├──────────────────────────────────────────────────────────────────┤
│  ▶  read_file               Read file contents                   │
│  ▶  write_file              Write or create a file               │
│  ▶  edit_file               Apply a targeted code change         │
│  ▶  bash                    Execute a shell command              │
│  ▶  list_directory          List directory contents              │
│  ▶  grep_search             Search file contents                 │
│  ▶  create_task             Create task on internal board        │
│  ▶  list_tasks              Query task board with filters        │
│  ▶  update_task_status      Transition task status               │
│  ▶  planner                 Create and manage execution plans    │
│  ▼  assign_task             Assign task to worker                │
│     Input schema:                                                 │
│     {                                                             │
│       "task_id": { "type": "integer", "required": true },       │
│       "worker_slug": { "type": "string", "required": true }      │
│     }                                                             │
└──────────────────────────────────────────────────────────────────┘
```

Clicking a tool row expands to show its full JSON input schema. Search filters by tool name. The count badge updates live as tools are registered/deregistered.

---

## 9. Tab: Logs

The Logs tab is a centralized log viewer for all three log streams: AutoDev Rust app, core-orchestrator, and memory-mcp. Added in Phase 2.5.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│  LOGS  [all ▾]  [date: today ▾]  [level: ALL ▾]  [🔍 filter...]  [📋 Export]   │
├──────────────────────────────────────────────────────────────────────────────────┤
│  14:32:01.004  [core-orch]    INFO   LivingAgent started for session abc123      │
│  14:32:01.108  [autodev]      INFO   Tool call: read_file src/api/health.py      │
│  14:32:01.892  [core-orch]    INFO   Memory recall: 12 memories loaded           │
│  14:32:02.441  [autodev]     WARN   Token budget at 84% — compressing context   │
│  14:32:03.211  [memory-mcp]   INFO   store_memory: session abc123 stored 3 items │
│  14:32:04.003  [autodev]     ERROR  Bash exit 1: cargo check failed             │
│  14:32:04.100  [core-orch]    DEBUG  Retrying tool call (attempt 2/3)            │
└──────────────────────────────────────────────────────────────────────────────────┘
│  ↓ tail  ·  4,231 lines today  ·  last update: 00:01 ago                        │
└──────────────────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Container | Full-height, `background: var(--bg-app)`, `IBM Plex Mono 12px` |
| Source filter | Dropdown: `all`, `autodev`, `core-orch`, `memory-mcp` |
| Date picker | Dropdown with last 7 dates + custom — reads from `~/.autodev/logs/` directory |
| Level filter | `ALL`, `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE` |
| Text filter | Live-searches rendered lines — hides non-matching |
| Timestamp | `color: var(--text-dim)` |
| Source | `[core-orch]` / `[autodev]` / `[memory-mcp]` — `color: var(--info)` |
| `ERROR` lines | Full line background `var(--danger-muted)`, level text `color: var(--danger)` |
| `WARN` lines | Full line background `var(--warning-muted)`, level text `color: var(--warning)` |
| `INFO` lines | Level text `color: #4477FF` (info/periwinkle) |
| `DEBUG` lines | `color: var(--text-dim)` |
| `TRACE` lines | `color: var(--text-muted)` (darkest — filtered out by default) |
| Tail behavior | Auto-scrolls to bottom as new lines arrive. Pauses auto-scroll when user scrolls up. Resumes on `[↓ tail]` button |
| Export button | Saves filtered log view to `~/Desktop/autodev-log-{date}.txt` |
| Footer | Shows total line count + last update time |

---

## 10. Persistent Sidebar

Always visible on the right side of every tab. Width: `176px`.

### 10.1 Context Files

```
CONTEXT
  ● AUTODEV.md
  ● .autodevhints
  ○ user-prefs
```

| Element | Spec |
|---|---|
| `●` active | `color: var(--accent)` — file exists and is loaded |
| `○` inactive | `color: var(--text-dim)` — not found or not loaded |
| Click | Opens inline editable text field for that context file |
| Section header | `IBM Plex Mono 9px uppercase`, `color: var(--text-muted)` |

### 10.2 MCP Active Connections

```
MCP ACTIVE
  ● github-mcp
  ● slack-mcp
  ○ sentry-mcp
```

| Element | Spec |
|---|---|
| Connected | `●` `color: var(--accent)`, name `color: var(--text-secondary)` |
| Error / Disconnected | `●` `color: var(--danger)`, name `color: var(--danger)` |
| Not enabled | `○` `color: var(--text-dim)`, name `color: var(--text-dim)` |
| Click | Expands inline to show MCP server details + reconnect button |

### 10.3 Skills

```
SKILLS
  ● /review-pr
  ● /run-tests
  ● /claim-issue
  ● /open-pr
```

Each item is a clickable slash command. Clicking inserts `/skill-name` into the chat input and triggers the skill picker auto-complete.

### 10.4 Hooks

```
HOOKS
  ● post-write:lint
  ● pre-bash:guard
  ● session:telemetry
```

| Element | Spec |
|---|---|
| Active | `●` green, hook name `color: var(--text-secondary)` |
| Firing | `◌` spinning amber — hook is currently executing |
| Error | `●` red, hover to see last error |
| Click | Opens hook detail card showing: trigger event, script path, last execution status |

### 10.5 Session Info

```
SESSION
  12,450 tokens
  $0.04 this session
  ⊕ /clear context
```

| Element | Spec |
|---|---|
| Token count | `IBM Plex Mono 11px`, `color: var(--text-secondary)` — live updates |
| Cost | `color: var(--accent)` |
| `/clear context` | `color: var(--info)`, click opens confirmation dialog: "Clear context and reset token count? This will not delete messages." |

### 10.6 Plan Card (Sidebar)

When a plan is active, a compact plan card replaces or appears below Session Info:

```
ACTIVE PLAN
  ▓▓▓▓▓▓░░░  3/7
  Add /health endpoint
  Step 4: Add schemas
  [PA]  personal                [→ View]
```

| Element | Spec |
|---|---|
| Mini progress bar | `height: 3px`, width fills sidebar, fill `var(--accent)` |
| Plan title | `IBM Plex Sans 11px`, `color: var(--text-primary)`, truncated |
| Current step | `IBM Plex Sans 11px`, `color: var(--text-secondary)`, `Step N:` prefix |
| Agent badge | `[PA]` / `[SUPERVISOR]` / `[WORKER-X]` — `10px`, colored per agent type |
| Scope badge | `personal` / `fleet` / `epic`, `10px`, `color: var(--text-dim)` |
| View button | Opens Plan Detail View overlay |

---

## 11. Plan System UI

### 11.1 Plan Detail View

Full-width overlay panel (not a popup modal — slides in from right, 400px wide, over the sidebar). Triggered by `[→ View]` in plan card or `[Jump to Plan]` in chat.

```
┌────────────────────────────────────────────────────────────────────┐
│  ← PLAN                                          [● Active]       │
│  Add /health endpoint to orchestrator                              │
│  Personal · Scope · Created by PA · 3/7 done                     │
│  ████████████░░░░░░░░░░░░░░░░  43%                                │
├────────────────────────────────────────────────────────────────────┤
│  ✓  1. Read existing route structure                               │
│  ✓  2. Add health endpoint handler function                        │
│  ✓  3. Register route in api router                               │
│  ◌  4. Add response model to schemas.py             in progress   │
│         Step note: Working on HealthResponse Pydantic model        │
│  ○  5. Write unit tests                                            │
│  ○  6. Run tests and fix failures                                  │
│  ○  7. Verify with pytest + cargo check                           │
├────────────────────────────────────────────────────────────────────┤
│  [Activate]  [Add Step]  [✗ Abandon]                               │
└────────────────────────────────────────────────────────────────────┘
```

| Element | Spec |
|---|---|
| Slide panel | Appears from right, `width: 400px`, `background: var(--bg-surface)`, `border-left: 1px solid var(--border-secondary)` |
| Progress bar | `height: 6px`, full width, fill `var(--accent)` |
| Step icons | `✓` green, `◌` amber spinning, `○` hollow, `⊘` orange blocked |
| Step note | Indented below in-progress step, `IBM Plex Sans 11px italic`, `color: var(--text-dim)` |
| PA actions | Activate, Add Step, Mark Done, Abandon |
| Context-aware buttons | Fleet agents (Supervisor, Worker, QC) get different action sets per Section 4.9 of dev plan |

---

## 12. Notification System

### 12.1 In-App Toast Notifications

Non-blocking toasts appear in the top-right corner of the window, stack vertically, auto-dismiss after 4 seconds (errors stay until dismissed).

```
┌────────────────────────────────────────────┐
│  ✓  Task #142 completed · /health endpoint  │
│                                      × 3s  │
└────────────────────────────────────────────┘
```

| Type | Border color | Icon |
|---|---|---|
| Success | `var(--accent)` | `✓` |
| Warning | `var(--warning)` | `⚠` |
| Error | `var(--danger)` | `✗` |
| Info | `var(--info)` | `i` |

Common toast triggers:
- Agent completes a task
- Sidecar goes down or comes up
- API key expires within 14 days
- Plan step completes
- QC agent approves or rejects a PR
- `cargo check` passes after fixes

### 12.2 System Tray

AutoDev sits in the system tray when minimized. Tray icon states:

| State | Icon | Tooltip |
|---|---|---|
| Idle | Dim AIRS logo | AutoDev — idle |
| Agent working | Animated pulse | AutoDev — N agents working |
| Pending approval | `⚠` overlay | AutoDev — waiting for your approval |
| Error | `✗` overlay | AutoDev — fleet error |

Tray right-click menu:
```
Open AutoDev
────────────
▶ Start Fleet
■ Stop Fleet
────────────
PA Mode: Fast Dev ▾
────────────
Quit
```

The **"Pending approval"** state is critical — if the app is minimized and a bash card is waiting for Allow/Deny, the tray icon shows the `⚠` overlay and clicking it brings AutoDev to focus with the pending card scrolled into view.

### 12.3 Desktop Notifications

OS-level native notifications for high-priority events when the window is not focused:

- Task completed: `✓ Task #142 Done — /agents/health endpoint merged`
- Fleet error: `⚠ WORKER-BACKEND crashed — requires attention`
- Approval pending: `AutoDev needs your approval — bash command waiting`
- QC rejection: `QC rejected PR #87 — changes requested`

Clicking a notification brings the window to focus and navigates to the relevant tab.

---

## 13. Permission Mode System

Permission mode is a **session-level setting** that controls whether tool cards show approval buttons. It is separate from PA sub-mode (Fast Dev / Review / Delegate) — permission mode applies to the execution layer, sub-mode controls the tool set.

| Mode | Trigger | Behavior |
|---|---|---|
| ManualApproval | Default when PA sub-mode is "Review Mode" | Every tool card shows Allow/Deny and pauses the agent |
| SmartApproval | Default when PA sub-mode is "Fast Development" | Read tools auto-execute; write/bash tools ask |
| Autonomous | When session.permission_mode = Autonomous | All cards render but no approval needed; agent runs continuously |

**SmartApproval Rules (auto-execute vs always ask):**

| Tool | SmartApproval behavior |
|---|---|
| `read_file` | Auto-execute |
| `list_directory` | Auto-execute |
| `grep_search` | Auto-execute |
| `write_file` (new file) | Always ask |
| `edit_file` | Always ask |
| `bash` (safe commands) | Always ask |
| `bash` (destructive pattern) | Always ask + warning band |
| `bash` (push to main) | BLOCKED — no override, ever |
| `github_create_issue` | Auto-execute |
| `github_create_pr` | Always ask |
| `github_merge_pr` | Always ask |
| `planner` (create) | Auto-execute |
| `planner` (abandon) | Always ask |

The permission mode toggle is accessible from:
1. ChatHeader dropdown (adjacent to PA sub-mode)
2. StatusBar left section
3. Right-click on any pending tool card

---

## 14. Cost & Token Visualization

### 14.1 Session Token Gauge

Shown in the sidebar Session section and as a tooltip on the TabBar cost pill:

```
TOKEN BUDGET
[████████████████████████░░░] 84%
84,210 / 100,000 ctx tokens
⚠ Context compressing at 90%
```

| Element | Spec |
|---|---|
| Bar | `height: 4px`, `background: var(--border-primary)`, fill `var(--accent)` at <70%, `var(--warning)` at 70-90%, `var(--danger)` at 90%+ |
| Percentage | `IBM Plex Mono 10px`, right-aligned |
| Warning text | Appears at 70%: `⚠ Context compressing at 90%` — `color: var(--warning)` |
| At 90%+ | Core-orchestrator's token_validator triggers auto-degradation — UI shows `◌ Compressing...` with spinning indicator |

### 14.2 Session Cost Display

Cost is shown in three places:
1. **TabBar pill** — `$0.04`, always visible, updates in real-time
2. **Sidebar Session section** — `$0.04 this session`
3. **Metrics Row** in AutoDev tab — `COST TODAY: $2.14` (aggregate across all agents)

The `session:cost-update` Tauri event drives all three simultaneously.

---

## 15. Memory Health Visualization

When core-orchestrator is running (Worker mode), the sidebar shows a compact memory health section below MCP Active:

```
MEMORY HEALTH
  episodic    ████░░  67%  hot
  semantic    ██████  92%  warm
  procedural  ████░░  71%  hot
  staleness   ●  nominal
```

| Element | Spec |
|---|---|
| Per-type mini bar | `height: 2px`, width proportional to freshness score, color = cache tier color |
| Cache tier | `hot` = `var(--accent)`, `warm` = `var(--info)`, `cold` = `var(--text-dim)` |
| Staleness indicator | `●` green nominal, `●` amber degraded, `●` red critical |
| Click | Opens full Memory Inspector overlay showing: recall latency, cache hit rate, compression events, staleness scores per memory type |

This section only renders when `sidecarStore.orchestratorStatus === 'running'`. In PA-only mode, the section is hidden entirely.

---

## 16. Tool Call Card Spec (Full)

> This section supersedes the previous `autodev-tool-call-ui-spec-v1.0` document and is now the canonical reference.

### 16.1 Design Principles

1. **Cards, not pop-ups.** All tool interactions live in the chat stream as cards.
2. **Approve / Deny lives in the card.** No modal dialogs for permission.
3. **Cards are stateful.** They mutate in place through pending → running → success/error/denied.
4. **Pending cards never collapse.** Completed cards collapse to one line.
5. **Permission mode controls card behavior.** ManualApproval waits; Autonomous executes immediately.

### 16.2 Card Anatomy & State Machine

See the full card skeleton ASCII art and state machine in the original v1.0 spec. All states remain valid:

| State | Border | Badge | Footer |
|---|---|---|---|
| `pending-approval` | `var(--accent)` | 🟡 `PENDING` | Allow + Deny visible |
| `running` | `var(--warning)` pulsing | 🟠 `RUNNING HH:MM` | Spinner |
| `success` | `var(--border-primary)` | 🟢 `DONE` | Hidden (result shown) |
| `error` | `var(--danger)` | 🔴 `ERROR exit: N` | Hidden (error output) |
| `denied` | `var(--danger)` muted | `DENIED` | Hidden (denial note) |
| `blocked-by-policy` | `var(--danger)` | `BLOCKED` | No buttons — policy explanation |

### 16.3 `edit_file` Card

Diff view with line numbers, green additions, red deletions. Multi-file variant groups files in collapsible sub-sections with individual and batch Allow/Deny. See full spec in original v1.0 document for complete element table.

**New addition (v2.0):** When the IDE tab is open and shows the same file, the diff card and the IDE code viewer sync — accepting in one reflects in the other.

### 16.4 `bash` Card

Command preview, destructive warning band, live terminal output streaming, elapsed timer, exit code. "Edit command before run" inline editor. `git push origin main` always blocked. See v1.0 for complete element table.

**New addition (v2.0):** If the bash command contains a test runner (`pytest`, `cargo test`, `npm test`), the card's result section shows a **test summary chip** in addition to raw output:

```
┌─ result ─────────────────────────────────────── exit: 0 ──────┐
│  ✅  47 passed  ·  0 failed  ·  2 skipped  ·  1.24s           │
│                                                                  │
│  [Show full output ▾]                                            │
└──────────────────────────────────────────────────────────────────┘
```

Test summary chip: `background: var(--add-bg)`, `border: 1px solid var(--accent)`, `border-radius: 2px`.

**New addition (v2.0):** For `cargo check` / `cargo build` commands, the error output parser identifies rust error codes (`E0433`, etc.) and renders them as clickable chips that navigate to the relevant line in the IDE tab.

### 16.5 `read_file` Card

Compact, auto-executes in SmartApproval. Shows file path + collapsible 20-line preview. No approval buttons (in SmartApproval). In ManualApproval, shows Allow/Deny.

### 16.6 `write_file` (new file) Card

Same diff view as `edit_file` but all lines are additions (no deletions). Header shows `write_file (new)`. File path includes a `(new)` badge.

### 16.7 `list_directory` Card

Compact. Auto-executes. Shows rendered directory tree like `tree` output. `IBM Plex Mono 12px`, `color: var(--text-secondary)`.

### 16.8 MCP Tool Call Card

Distinct `⚡` icon in header. Shows MCP server name. Arguments rendered as `key: value` table. After success, result shows with a link if the tool created an external resource (issue URL, PR URL).

### 16.9 `planner` Card

Special card format for the planner tool:

```
┌──────────────────────────────────────────────────────────────────┐
│  📋  planner  ·  action: create                     [● PENDING]  │
├──────────────────────────────────────────────────────────────────┤
│  title:       "Add /health endpoint to orchestrator"             │
│  description: "Implement GET /agents/health..."                  │
│  scope:       personal                                            │
│  steps:       7 (to be added)                                    │
├──────────────────────────────────────────────────────────────────┤
│  [✓ Allow]  [✗ Deny]                                             │
└──────────────────────────────────────────────────────────────────┘
```

After approval, the card success state shows the plan ID and `[View Plan]` button that opens the Plan Detail View. `action: updateStep` cards are compact — just show step number + new status.

### 16.10 Collapsed Card States

All resolved tool cards collapse to a single line in the chat history. Format:

```
✏ edit_file  ·  engine.rs  ·  +3 -0  ·  ✓ Done          [▼ expand]
>_ bash  ·  cargo check 2>&1  ·  ✓ exit 0  ·  00:08      [▼ expand]
>_ bash  ·  cargo check 2>&1  ·  ✗ exit 1  ·  00:03      [▼ expand]
📋 planner  ·  plan #p4a2  ·  7 steps  ·  ● Active        [▼ expand]
⚡ github_create_issue  ·  Issue #142  ·  ✓ Created       [▼ expand]
```

---

## 17. Future Tool Cards Stub

These tools will follow the card anatomy from Section 16.2. Brief notes on their content section:

| Tool | Content Section Notes | Default Permission |
|---|---|---|
| `git_commit` | Staged files list + editable commit message | Always ask |
| `git_push` | Branch name + N commits ahead + target remote | Always ask (SmartApproval) |
| `create_pr` | PR title, description, base/head branch, reviewer assignment | Always ask |
| `grep_search` | Query + match count + top 5 results preview | Auto-execute |
| `apply_diff` | Same diff view as `edit_file` for patch files | Always ask |
| `web_fetch` | URL preview + response code | Auto-execute |
| `airs_claim_task` | Issue number + lock file path | Always ask |
| `airs_dispatch_signal` | Worker name + issue number + signal type | Always ask |
| `update_task_status` | Task ID + old status → new status | Auto-execute |
| `assign_task` | Task ID + worker slug | Auto-execute |

---

## 18. Implementation Reference

### 18.1 React Component Map

```tsx
src/
├── tabs/
│   ├── Chat.tsx                # Tab container + message list
│   ├── AutoDevPanel.tsx        # Roster + TaskBoard + MetricsRow
│   ├── IDE.tsx                 # FileTree + CodeViewer + GitPanel
│   ├── Recipes.tsx             # RecipeList + RecipeDetail
│   ├── Logs.tsx                # LogViewer + filters
│   └── Settings.tsx            # Providers + Router + Config + Services
├── components/
│   ├── layout/
│   │   ├── TitleBar.tsx
│   │   ├── TabBar.tsx          # + badge system
│   │   ├── StatusBar.tsx
│   │   └── Sidebar.tsx         # Persistent sidebar
│   ├── startup/
│   │   └── StartupScreen.tsx   # Sidecar loading screen
│   ├── chat/
│   │   ├── MessageBubble.tsx
│   │   ├── StreamingText.tsx
│   │   ├── ThoughtBlock.tsx
│   │   ├── ToolCallCard.tsx     # Dispatcher
│   │   ├── cards/
│   │   │   ├── EditFileCard.tsx
│   │   │   ├── BashCard.tsx
│   │   │   ├── ReadFileCard.tsx
│   │   │   ├── WriteFileCard.tsx
│   │   │   ├── ListDirCard.tsx
│   │   │   ├── McpToolCard.tsx
│   │   │   └── PlannerCard.tsx
│   │   ├── PlanDisplay.tsx      # Inline plan card in chat
│   │   ├── InputBar.tsx
│   │   ├── AtFilePicker.tsx
│   │   └── SkillPicker.tsx
│   ├── autodev/
│   │   ├── AgentRoster.tsx
│   │   ├── TaskBoard.tsx
│   │   ├── TaskCard.tsx
│   │   ├── TaskDetailModal.tsx
│   │   └── MetricsRow.tsx
│   ├── ide/
│   │   ├── FileTree.tsx
│   │   ├── CodeViewer.tsx
│   │   └── GitPanel.tsx
│   ├── plan/
│   │   ├── PlanDetailView.tsx
│   │   └── PlanCard.tsx         # Sidebar compact card
│   ├── sidebar/
│   │   ├── ContextFiles.tsx
│   │   ├── McpStatus.tsx
│   │   ├── Skills.tsx
│   │   ├── Hooks.tsx
│   │   ├── SessionInfo.tsx
│   │   └── MemoryHealth.tsx
│   ├── notifications/
│   │   ├── Toast.tsx
│   │   └── ToastQueue.tsx
│   └── settings/
│       ├── ProvidersTable.tsx
│       ├── OllamaSettings.tsx
│       ├── ModelRouter.tsx
│       ├── AirsConfig.tsx
│       ├── SidecarPanel.tsx
│       └── McpToolBrowser.tsx
```

### 18.2 Zustand Store Map

```tsx
stores/
├── chatStore.ts        # messages, streaming state, active session
├── agentStore.ts       # roster, agent statuses, current tasks
├── taskStore.ts        # kanban columns, task CRUD
├── planStore.ts        # active plan, plan history, fleet plans
├── sidecarStore.ts     # service statuses, health
├── settingsStore.ts    # providers, model router, AIRS config
├── sessionStore.ts     # tokens, cost, mode, permission mode
├── ideStore.ts         # active file, diff state, git status
└── notificationStore.ts # toast queue
```

### 18.3 Tauri Event Map

| Event | Direction | Payload | Consumer |
|---|---|---|---|
| `agent:stream-token` | Backend → Frontend | `{ text, done }` | StreamingText |
| `agent:tool-call-start` | Backend → Frontend | `{ id, name, args }` | ToolCallCard |
| `agent:tool-call-result` | Backend → Frontend | `{ id, result, is_error }` | ToolCallCard |
| `agent:tool-bash-output` | Backend → Frontend | `{ id, chunk, stream }` | BashCard |
| `agent:status-change` | Backend → Frontend | `{ status: AgentStatus }` | StatusBar, Roster |
| `session:cost-update` | Backend → Frontend | `{ tokens_in, tokens_out, cost_usd }` | TabBar, Sidebar |
| `sidecar:status` | Backend → Frontend | `{ name, status }` | StartupScreen, SidecarPanel |
| `task:updated` | Backend → Frontend | `{ task_id, new_status }` | TaskBoard |
| `plan:created` | Backend → Frontend | `{ plan }` | PlanDisplay, Sidebar |
| `plan:step_completed` | Backend → Frontend | `{ plan_id, step_index }` | PlanDisplay, Sidebar |
| `tool:allow` | Frontend → Backend | `{ tool_call_id }` | agent_commands.rs |
| `tool:deny` | Frontend → Backend | `{ tool_call_id }` | agent_commands.rs |
| `tool:edit-command` | Frontend → Backend | `{ tool_call_id, edited_command }` | agent_commands.rs |

---

*This document is the canonical UI specification for AutoDev Single Fleet. Update each section as new phases are implemented and as UX decisions evolve.*