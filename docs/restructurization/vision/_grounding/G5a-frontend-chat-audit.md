# G5a — Frontend Audit: the Chat surface (BUG-CHAT)

**Scope:** `src/tabs/Chat.tsx`, `src/components/chat/`, `src/stores/agentStore.ts`, `src/stores/sessionStore.ts`, and the Rust event/command contract behind them
**Method:** read-only, traced React → Tauri events → Rust engine end to end; every claim carries `path:line`
**Date:** 2026-08-31
**Status:** COMPLETE — this is the traced mechanism of the owner's bug #8 (BUG-CHAT)

---

## 0. Event contract (ground truth)

**Emitted by Rust** (`src-tauri/src/events.rs:29-36`, helpers at `:181-211`). No `rename` attrs → snake_case as written:

| Event string | Payload struct | serde field names |
|---|---|---|
| `agent:token` | `TokenPayload` `events.rs:117-121` | `session_id`, `text` |
| `agent:thinking` | `ThinkingPayload` `events.rs:123-127` | `session_id`, `text` |
| `agent:tool-start` | `ToolStartPayload` `events.rs:129-135` | `session_id`, `call_id`, `tool_name`, `input` |
| `agent:tool-end` | `ToolEndPayload` `events.rs:137-144` | `session_id`, `call_id`, `tool_name`, `output`, `is_error` |
| `agent:permission-request` | `PermissionPayload` `events.rs:146-153` | `session_id`, `call_id`, `tool_name`, `input`, `description` |
| `agent:done` | `AgentDonePayload` `events.rs:155-162` | `session_id`, `message_id`, `tokens_in`, `tokens_out`, `cost_usd` |
| `agent:error` | `AgentErrorPayload` `events.rs:164-168` | `session_id`, `message` |
| `cost:update` | `CostUpdatePayload` `events.rs:170-177` | `session_id`,`tokens_in`,`tokens_out`,`cost_usd`,`total_today` |

Call sites: `engine.rs:253` (error), `:394` (done), `:429` (token), `:436` (thinking); `tool_executor.rs:93,256,320` (tool-start), `:185,270,412` (tool-end), `:439` (permission-request).

Commands: `send_message` `commands/agent_commands.rs:23-206`, `cancel_agent` `:210-217`, `respond_to_permission` `:221-239`.

---

## 1. STREAMING — how a response reaches the UI

**Happy path (CONFIRMED, traced):** `Chat.handleSend` → `useAgentStore.setupListeners(sessionId)` (`Chat.tsx:294` → `agentStore.ts:135-265`) → `invoke('send_message')` (`Chat.tsx:307`) → Rust spawns the engine (`agent_commands.rs:127`) → `engine.stream_response` emits `agent:token` per `TextDelta` (`engine.rs:429`) → the store appends to the last assistant message (`agentStore.ts:139-163`).

Name and field matching between `agentStore.ts:139/166/187/220/234/252` and `events.rs:29-35` is **exact**. **No typo-level mismatch in the chat path.**

### Dead listeners — listened-for but NEVER emitted (CONFIRMED; grep of `src-tauri/src` finds no such string)
- `agent:tool-call-start` `stores/agentLiveStore.ts:305`
- `agent:tool-call-result` `agentLiveStore.ts:309`
- `agent:stage-change` `agentLiveStore.ts:313`
- `agent:token-update` `agentLiveStore.ts:317`
- `agent:status-change` `agentLiveStore.ts:321`
- `plan:created` / `plan:updated` / `plan:step_completed` / `plan:item_claimed` / `plan:subplan_created` `stores/planStore.ts:74,78,82,86,90` — Rust only emits `plan:changed` (`events.rs:56`) → **PlanDisplay/planStore never updates from the backend.**

### Dead emitters — emitted but NEVER listened for (CONFIRMED)
- **`agent:thinking`** (`events.rs:30`, emitted `engine.rs:436` on every thinking delta). No `listen('agent:thinking')` exists anywhere in `src/`. **Extended-thinking models stream into a black hole**: the UI shows an empty bubble + "thinking…" pill while all reasoning tokens are discarded.
- `cost:update` is emitted only from `recipes/runner.rs:429`, never from the Mode-1 chat engine, so the dashboard cost listener (`dashboardStore.ts:338`) never fires for chat.

### The dominant streaming defect (CONFIRMED)

**Listeners exist only while a send is in flight and only while the Chat tab is mounted.**

- `setupListeners` is called from exactly one place: `Chat.tsx:294`, inside `handleSend`.
- `App.tsx:106` renders `{activeTab === 'chat' && <ChatTab/>}` → switching tabs **unmounts** Chat → `Chat.tsx:250` unsubscribes everything. Tokens, tool events, and permission requests emitted while the user is on another tab are **lost permanently**, and nothing re-subscribes on return (only the *next* send re-registers).
- Consequence chain: the agent keeps running in Rust, the UI shows a frozen half-message; a permission request that fires while unmounted is never answered → the oneshot in `tool_executor.rs:448` never resolves → **the agent task hangs forever** (see §8).

### Backend paths that end without emitting any terminal event (CONFIRMED — UI spins forever)
- `engine.rs:299` `tool_executor.execute_batch(...)?` — the `?` returns `Err` from `run()`; no `emit_agent_error`, no `emit_agent_done`. The caller only does `tracing::error!` (`agent_commands.rs:198-200`).
- `engine.rs:164` `return Err(BudgetExceeded)` — emits `agent:budget-exceeded` but never `agent:error`/`agent:done`; Chat doesn't listen to budget events.
- `agent_commands.rs:134-137` unsupported provider → `return` from the spawned task with **zero events**.
- `engine.rs:92` pre-loop cancel check returns early (no `agent:done`).
- `engine.rs:168-171` `MAX_TOOL_TURNS` break does reach `emit_agent_done`, but the last history entry is a `user` (tool-results) message, so `agent_commands.rs:169-171` saves **nothing** — the turn evaporates on reload.

---

## 2. LISTENER LIFECYCLE

- `unlistenersRef` + unmount cleanup: `Chat.tsx:246`, `:249-251`. Correct in the happy path.
- **Leak/race (CONFIRMED):** `Chat.tsx:293-294` — the old unlisteners are invoked, then `await setupListeners(...)`. If the component unmounts during that await, the unmount cleanup at `:250` runs over the *old* (already-called) array, and the freshly created listeners are assigned to a ref of a dead component → **permanently registered, never removed**. Every subsequent remount+send stacks another orphan set → **duplicated token appends (text doubles/triples)**.
- **Double-send race (CONFIRMED):** `handleSend` guards only `!input.trim() || isStreaming` (`Chat.tsx:278`). `isStreaming` is not set until `startStreaming` at `:305`, i.e. *after* two awaits. `isStarting` is set (`:289`) but only used for the button's `disabled` attribute (`:597`) — the **Enter key path (`:437-440`) ignores it**. Two fast Enters ⇒ two sessions/engines, two user messages, and orphaned listener sets.
- **Unawaited `listen()` promises elsewhere (memory leak + duplicate handlers on remount), all CONFIRMED:** `tabs/Graph.tsx:178,183`; `stores/planStore.ts:74,78,82,86,90` (module-scope, never unlistened); `components/WorkspaceMismatchBanner.tsx:43,127`; `components/dashboard/EpicPhaseView.tsx:186,189`; `components/dashboard/QcRoiPanel.tsx:63`; `components/dashboard/OrphanAlert.tsx:23`; `components/dashboard/WorkerAffinityPanel.tsx:49`; `components/budget/BudgetBar.tsx:81,84`; `components/SidecarCrashBanner.tsx:26`. In each, the effect returns a `Promise`, not a cleanup function — React ignores it, so nothing is ever unsubscribed.
- `stores/workerChatStore.ts:105` stores into a module-global `_unlistenFn`; a second `hydrate()` overwrites it and leaks the first.

---

## 3. MESSAGE RENDERING / KEYING / APPEND SEMANTICS

Keying: `Chat.tsx:483` `key={msg.id}`. IDs come from three generators: `agentStore.ts:81`, `:150` (`crypto.randomUUID()`), and **`:201` where a tool message's `id` is set to the tool `call_id`**, plus DB ids on reload.

- **No key collision in a single array (CONFIRMED)**, but tool messages persisted server-side use a *different* id than `call_id` (`session/types.rs:49+`), so a session reload can show the same tool twice if live and reloaded sets were ever merged (they aren't today — latent, SUSPECTED).
- **Deltas append correctly** (`agentStore.ts:146`). No overwrite bug, no dropped chunks.
- **CONFIRMED O(n) per token → effectively O(n²) per response:** every `agent:token` copies the whole array (`agentStore.ts:141-142`), so React re-renders **every** `MessageBubble` (no `React.memo` on `MessageBubble.tsx:25`), each re-running `ReactMarkdown` + `remark-gfm` over its full content. `agent:tool-end` is worse: `:191-198` `map`s over **all** messages creating a new object for every one.
- **CONFIRMED — `ContextMeter` is permanently 0.0%.** `Chat.tsx:56-67` counts `m.text` (never exists) and `Array.isArray(m.content)` (`ChatMessage.content` is a `string`, `agentStore.ts:39`). Always 0 chars ⇒ `0.0%` regardless of conversation size. It also recomputes on every token and omits `modelId` from deps.
- **CONFIRMED — tool-start events are silently dropped after the first tool completes.** `agentStore.ts:179` only attaches a tool call `if (last?.role === 'assistant')`. But `agent:tool-end` **pushes a `role:'tool'` message onto the end** (`:215`). So in a serial write-tool batch (`tool_executor.rs:309-421` emits start/end/start/end) **every tool after the first has `last.role === 'tool'` ⇒ its `ToolCallDisplay` is never created**, `RunningToolsPill` can't name it, and the "pending → done" spinner never appears.
- **CONFIRMED — live tool input is always empty.** `agentStore.ts:208` sets `toolInput: {}` with the comment "Will be filled from the original tool call" — it never is. The expanded tool bubble shows no arguments for any live tool call.
- **CONFIRMED — reloaded transcripts lose intermediate assistant turns.** `agent_commands.rs:169-177` saves only the *last* history entry's text, and `Message::new_assistant` stores `tokens_in/out = 0`, so on reload the token/cost badge never renders and every mid-loop assistant narration is gone.
- **SUSPECTED, high-confidence — poisoned history breaks the next send.** If a run terminates with a tool-use-only final assistant block, `agent_commands.rs:171-177` appends an assistant `Message` with `content: ""`; `build_history` (`:243-256`) then emits `MessageContent::Text { text: "" }`, which Anthropic rejects → **every subsequent send in that session fails with `agent:error`**. A plausible mechanism for "chat is broken in one session but fine in a new one".

---

## 4. TOOL-CALL DISPLAY / toolOutputParser

Dispatch: `ToolCallDisplay.tsx:98-133` and the persisted twin `MessageBubble.tsx:193-224` both call `parseToolOutput(toolName, output)` (`utils/toolOutputParser.ts:68-115`).

- Matching is a **hardcoded string switch on 5 names** (`:73-107`): `grep`, `glob`, `bash`, `read_file`, `list_directory`. Verified against Rust tool names — those five match.
- **Every other tool falls to `default: {type:"text"}`** (`:110`) and renders as raw truncated text (2000-char cut). That includes `edit_file`, `write_file`, and the **~45 other tools** under `src-tauri/src/agent/builtin_tools/` — so diffs, task-board results, and subagent spawns all render as a wall of JSON. CONFIRMED.
- **`bash` display is wrong by construction.** `bash.rs:137-157` returns **plain text**, not JSON. `toolOutputParser.ts:83-88` always hits the `catch` fallback `{ command:"", exit_code:0, … }` ⇒ `BashOutputDisplay.tsx:28` renders an empty `Command:` and `:36-40` **always renders a green "Exit 0" badge even for failed commands.** CONFIRMED.
- **`read_file` display is wrong.** `:91` hardcodes `path: ""`, so `FileContentDisplay.tsx:11` calls `detectLanguageFromPath("")` → `"text"` ⇒ **no syntax highlighting ever**, and the header renders an empty clickable filename calling `openInIde("")`. Also `read_file.rs:172` prefixes every line with `"%4d | "`, so the "content" fed to Prism contains gutter text. CONFIRMED.
- `grep` returns a plain string on no-match (`grep.rs:265`) which throws inside `JSON.parse` and is rescued only by the outer `try` — works, but by accident.
- `list_directory` guesses `is_directory` from a trailing slash (`:103`) — silently wrong on Windows output.

---

## 5. INPUT HANDLING

- **Enter/Shift-Enter:** `Chat.tsx:437-441`. **No IME guard — CONFIRMED**: no `isComposing` / `keyCode === 229` check anywhere, so Japanese/Chinese/Korean candidate confirmation submits a half-composed message.
- **Input is cleared BEFORE the send can succeed — CONFIRMED, data loss.** `Chat.tsx:280` `setInput('')` runs before the provider check (`:283-286`, which `alert()`s and returns) and before `ensureSession`/`invoke` (`:290,307`). **Any failure — no provider, no API key, session-create failure, backend down — destroys the user's typed message** with only a `console.error`.
- **@-picker:** `Chat.tsx:328-341` regex `/@(\S*)$/` → `invoke('search_workspace_files')`. **No debounce — CONFIRMED**: one IPC call per keystroke. Errors swallowed to `[]`, so a failing search is indistinguishable from "no matches".
- **Slash picker fires on ordinary path text — CONFIRMED:** `Chat.tsx:347` `/\/([\w-]*)$/` matches the `/` inside `src/components`, popping the skill picker mid-sentence and hijacking Enter (`:406-410`) so the message can't be sent. `handleSlashSelect` then cuts at `lastIndexOf('/')`, mangling the typed path.
- **Textarea never auto-grows:** `rows={1}` + `maxHeight 120px` with no `scrollHeight` resize (`:553-567`) → multi-line drafts hidden behind an internal scrollbar.
- Meta row hardcodes `· ManualApproval` (`:620`) regardless of PA mode — and it is in fact always true (`agent_commands.rs:142` passes `PermissionMode::ManualApproval` unconditionally), so **the PA-mode dropdown never changes approval behaviour**.

---

## 6. SCROLL

- Auto-scroll: `Chat.tsx:241-243`, `scrollIntoView({behavior:'smooth'})` with deps `[messages, pendingPermission, isStreaming]`.
- **CONFIRMED — it fights the user.** No "is the user near the bottom?" check and no scroll-position ref anywhere. Scrolling up to read history is undone by the very next token event.
- **CONFIRMED — one smooth-scroll animation per token.** `messages` identity changes on every `agent:token`, so `scrollIntoView({behavior:'smooth'})` retriggers dozens of times per second; browsers cancel/restart the animation, producing the classic juddering "stuck mid-scroll" behaviour and pinning the CPU alongside the full-list re-render from §3.
- Nested scroll containers: `AtFilePicker.tsx:27-30` calls `scrollIntoView({block:'nearest'})` on its own list, which can scroll the outer chat container as a side effect (SUSPECTED).

---

## 7. ERROR STATES

- **`send_message` rejects** (no active provider `agent_commands.rs:36`, no active model `:38`, no API key `:45`, session not found `:52`): `Chat.tsx:312-315` only `console.error` + `stopStreaming()`. **The empty assistant bubble pushed by `startStreaming` stays in the list forever with no text and no error** — the user sees a blank bubble and a lost input. Nothing calls `useToastStore` or any banner here.
- **`invoke` without `.catch` — CONFIRMED:**
  - `Chat.tsx:323` `await invoke('cancel_agent', …)` with no try/catch → unhandled rejection; `stopStreaming()` at `:324` never runs if it throws, **leaving the UI stuck in "streaming"**.
  - `PermissionDialog.tsx:24-31` `await invoke('respond_to_permission')` in a `try/finally` **with no `catch`**.
  - `sessionStore.ts:55` `createSession` and `:176` `delete_session` have no internal catch.
  - `Chat.tsx:372` `skillStore.getSkill(...)` and `:380` `executeSkill(...)` awaited with no catch.
- Where errors *do* surface: only `agent:error` (`agentStore.ts:252-262`), **and even then only if the last message is an assistant message with empty content** (`:257`). If any text streamed before the failure, or if a tool message is last, **the error text is discarded** and the UI just stops.
- `sessionStore.loadSessions` swallows failures (`:47-49`); with `sessions: []` the **entire chat pane including the input box is replaced by the "Create a new session" placeholder** (`Chat.tsx:449-459`) — a backend/DB hiccup at startup presents as "chat is completely gone".

---

## 8. PERMISSION DIALOG

Flow (CONFIRMED): `tool_executor.request_permission` (`tool_executor.rs:427-458`) inserts a oneshot sender into `PendingPermissions` keyed by `call_id` (`:436`), emits `agent:permission-request` (`:439-445`), then blocks on `rx.await` (`:448`). Frontend: `agentStore.ts:220-231` sets `pendingPermission` and flips `isStreaming` to `false`; `InlinePermissionCard` renders it; buttons call `invoke('respond_to_permission', {callId, response})` → `agent_commands.rs:221-239`.

**Races and hangs:**

1. **Dialog never appears → agent hangs forever (CONFIRMED).** The permission listener exists only while Chat is mounted *and* only after a send registered it. Switch tabs and the event is dropped; `rx.await` at `tool_executor.rs:448` **never resolves**, the tokio task and its `Semaphore` permit leak, and `state.agents[sid].is_running` stays `true` forever. **There is no timeout on the oneshot.**
2. **Cancel does not release a pending permission (CONFIRMED).** `cancel_agent` only sets a bool (`state/app_state.rs:44-48`), checked between turns (`engine.rs:188`). Nothing drains `PendingPermissions`, so a cancel during an approval wait leaves the engine blocked permanently while the UI shows idle.
3. **The card is cleared even when the response failed (CONFIRMED).** `PermissionDialog.tsx:28-31` uses `finally` (no `catch`): if `respond_to_permission` rejects, the card disappears, `isStreaming` is forced back to `true`, and the agent stays blocked — a permanent phantom "thinking…".
4. **Sending a new message while a permission is pending (CONFIRMED).** `isStreaming` was set to `false` by the permission handler, so Send is live. `handleSend` tears down the very listener that would deliver the answer (`Chat.tsx:293`), leaves `pendingPermission` set, and starts a *second* engine on the same session (`agent_commands.rs:59-63` overwrites the ActiveAgent entry). The first engine is orphaned.
5. **Cross-session leakage (CONFIRMED).** Listeners are bound to the session id captured at send time, but `sessionStore.switchSession` (`:83-153`) replaces the message array without tearing them down — a still-running old session's tokens/tool messages/permission card append into the newly displayed conversation.
6. **Batch permission ordering (SUSPECTED).** Write tools run serially and each awaits its own approval (`tool_executor.rs:309-341`), but `agentStore` holds a single `pendingPermission` slot (`:65`); if two requests ever overlap the second overwrites the first and the first's oneshot is never answered.

---

## 9. VISUALLY BROKEN / DESIGN-SYSTEM ISSUES

### Undefined CSS custom properties
Checked against the only stylesheet, `src/styles/globals.css:10-66` (`main.tsx:3` is the sole import). These resolve to nothing → **the declaration is dropped**:

- **`--surface-bg`** (correct name is `--bg-surface`): `MessageBubble.tsx:215`, `ToolCallDisplay.tsx:124`, `SessionBar.tsx:60`, `BashOutputDisplay.tsx:21`, `FileContentDisplay.tsx:20`, `GlobResultsDisplay.tsx:44`, `GrepResultsDisplay.tsx:40`, `DirectoryListingDisplay.tsx:43`. **Every rich tool-output panel and the whole session sidebar render with a transparent background.** This alone makes tool results look unstyled/broken.
- **`--accent-color`** (correct: `--accent`): `BashOutputDisplay.tsx:28`, `FileContentDisplay.tsx:27,43,44,45,55,56,57`, `GlobResultsDisplay.tsx:108,115`, `GrepResultsDisplay.tsx:58,66,76` → filenames render in inherited grey.
- **`--text-tertiary`** (correct: `--text-3`): `BashOutputDisplay.tsx:43,105`, `FileContentDisplay.tsx:33,78`, `GlobResultsDisplay.tsx:123,126,138`, `GrepResultsDisplay.tsx:69,100`.
- **`--error-color`**: `SessionBar.tsx:143` (delete affordance invisible).
- **Invalid CSS syntax**, not just a missing var: `"var(--accent-color)20"` / `"…)40"` / `"…)10"` at `FileContentDisplay.tsx:43,45,55,57` and `GrepResultsDisplay.tsx:58` — concatenating a hex-alpha suffix onto a `var()` produces an invalid value and the browser drops the whole property.

### Two competing design systems (CONFIRMED)
`tailwind.config.js` defines a "Winter is Coming" palette (`accent: #82AAFF`, `bg-card: #01111D`, `text-1: #D6DEEB`), while `globals.css:31,12,58` define an entirely different one (`--accent: #00FF88`, `--bg-card: #1e1e2e`, `--text-1: #aaaaaa`). **Any component mixing Tailwind color utilities with `var(--…)` inline styles gets two clashing themes on the same element.**

> **Both palettes are dark.** `ui-anchor.md` mandates **light mode only** (Milky White `#FFFEF7`). The entire colour system is a removal target, not a refactor target.

### `prose prose-invert` are no-op classes (CONFIRMED)
`MessageBubble.tsx:44` applies them, but `tailwind.config.js` has `plugins: []` — `@tailwindcss/typography` is not installed. Combined with Tailwind Preflight resetting `h1/ul/p` margins, markdown renders flat; **GFM tables (enabled via `remark-gfm`) get no styling at all** and come out as run-together text, since only `code/a/h1-h3/ul/ol/li/blockquote` are overridden (`:58-80`).

### Hardcoded off-palette colors
`SessionBar.tsx:69-70` (`#1a73e8` Google blue), `:95-96`, `:112`, `:121`, `:127`, `:130`; `MessageBubble.tsx:48` (user bubble text `#aaa` on a green-tinted bubble → low contrast), `:179-180`; `ToolCallDisplay.tsx:26` (`border: 1px solid #1d1d1d` — invisible), `:88`; `BashOutputDisplay.tsx:36-37,59,79,88,93,95` (**three different "red"s**); `Chat.tsx:639` (`#011627`), `:662`, `:667`.

### Layout defects
- **Floating 🤖 button overlaps the composer (CONFIRMED):** `Chat.tsx:628-651` is `position: fixed; right:12px; bottom:20px; z-index:999` — directly on top of the Send/Cancel region and the status bar, and it renders even in the empty state.
- `MessageBubble.tsx:100-101` renders an empty 8px spacer div conditionally; `:85-90` are two branches rendering the identical blinking `▋`.
- **`PlanDisplay.tsx`/`PlanDisplay.css` are dead code in the chat surface** — nothing under `src/components/chat/` or `Chat.tsx` imports it, and its data source (`planStore`) listens to five events Rust never emits (§1).
- **`PaModeToggle.tsx` is unused** — `Chat.tsx:101-199` re-implements the mode selector inline; the segmented control is orphaned.
- `Chat.tsx:449` gates the header, message list, permission card **and composer** behind `sessions.length !== 0`; combined with `activeSessionId` starting `null` after a reload (`loadSessions` never selects one, `sessionStore.ts:42-52`), **the first message typed into an existing-looking chat silently creates a brand-new session** (`Chat.tsx:266-274`) instead of continuing the one shown as selected in the sidebar.

---

## Top defects, ranked by "chat is broken"

1. Listeners live only inside `handleSend` and die on tab switch → lost streams, orphaned agents, permission hangs (`Chat.tsx:294`, `App.tsx:106`, `Chat.tsx:249-251`).
2. Several Rust exit paths emit no terminal event → UI spins forever (`engine.rs:299,164,92`, `agent_commands.rs:134-137`).
3. `respond_to_permission` failure/absence hangs the engine with no timeout and no cancel path (`tool_executor.rs:448`, `app_state.rs:44-48`, `PermissionDialog.tsx:28-31`).
4. Input cleared before send validation → typed messages vanish on any failure (`Chat.tsx:280` vs `:283-315`).
5. Undefined `--surface-bg`/`--accent-color`/`--text-tertiary` + missing typography plugin → tool panels and markdown look unstyled.
6. `agent:tool-start` dropped whenever the previous message is a tool message → tools appear to never start (`agentStore.ts:179`).
7. Auto-scroll with `behavior:'smooth'` on every token, plus full-list re-render per token → juddering, unusable scrollback.
8. Context meter permanently 0.0% (`Chat.tsx:56-67`).
9. Bash "Exit 0" always green; `read_file` never highlighted; ~45 tools have no rich display (`toolOutputParser.ts:73-110`).
10. `agent:thinking` emitted and thrown away; five `agent:*` and five `plan:*` listeners bound to events no Rust code emits.
