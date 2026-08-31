# G11 — UX Audit: first run, BYOK onboarding, error surfacing, and the shape of "smooth"

**Scope:** `src/` (React/TypeScript, ~35,100 LOC counted in `src/`; the sibling audit counts 39,474 including CSS. 9 tabs, 20 component dirs, 28 stores)
**Method:** read-only. Every code-grounded claim carries `path:line`. Claims are split into
*the code does X* (mechanism) vs *the user experiences Y* (consequence).
**Siblings read first (not repeated here):** `G5a-frontend-chat-audit.md`, `G5c-frontend-settings-stores-contract.md`,
`G5b-frontend-graph-city-audit.md`, `G3b-bugs-git-ide-diff-orphan.md`. Cited where a code defect has a UX consequence.
**Date:** 2026-08-31
**Status:** COMPLETE

---

## 0. Headline

Three findings, then the evidence.

1. **There is no onboarding of any kind.** An exhaustive case-insensitive grep of `src/` for
   `onboard|welcome|first.run|firstRun|getting.started|setup wizard|tour` returns **five hits, none of them
   an onboarding surface**: two are the word "Onboarding" inside Docs-tab prose (`src/data/docsAllC.ts:319`,
   `src/data/docsMore.ts:138`), one is a code comment (`src/components/chat/SessionBar.tsx:18`), and two are
   `WorkerSetupWizard.tsx` — which is **dead code** (imported only by `ITDashboard.tsx`, which nothing imports).
   The app has never had a first-run experience.
2. **The BYOK key is never validated, and the one control that could validate it is hidden until after the
   key has already been saved and made active.** `handleSaveKey` (`src/tabs/Settings.tsx:1069-1084`) stores the key
   and immediately calls `setActiveProvider` (`:1075`). It never calls `test_provider_connection`. The `Test`
   button is rendered only inside `{hasKey && …}` (`:1143`) — i.e. only *after* the unverified key is committed.
3. **The app has a toast system that has never displayed a toast.** `ToastStack` is mounted at `src/App.tsx:117`,
   but `addToast` is referenced from exactly **three** places in the whole frontend —
   `src/stores/workerStore.ts:283-284` and `:324,326,333` — and both call sites do
   `const { addToast } = require('../stores/toastStore')`. `toastStore.ts` exports **only** `ToastType`, `Toast`,
   and `useToastStore` (`src/stores/toastStore.ts:3,5,27`); there is no `addToast` export, and `require` does not
   exist in the Vite/ESM browser bundle. **Both call sites throw**, and nothing else in `src/` even attempts to
   use the store. The notification channel the entire app should route failures through is inert.

---

## 1. The first-run walkthrough — what a brand-new user actually experiences

Traced from `src/main.tsx` → `src/App.tsx` → the mounted chrome, against `src-tauri/src/main.rs` for the boot FSM.

### Step 1 — Launch. A full-screen modal appears that cannot be dismissed.

`StartupOverlay` is the first child of the app shell (`src/App.tsx:82`). Its CSS is
`position: fixed; inset: 0; z-index: 9999` over an 85 %-opacity blurred backdrop
(`src/components/startup/StartupOverlay.css:3-14`). It renders 13 phase rows
(`src/stores/startupStore.ts:47-61`) and a percentage.

*The code does:* `dismissed` is set only when a `startup:progress` payload arrives with `phase === "ready"`
(`startupStore.ts:117,132-134`), 600 ms later. `StartupOverlay.tsx` contains **no dismiss control of any kind** —
no button, no Escape handler, no click-outside.

> **UX-01 (P0).** `startupStore.ts:80-82` documents the flag as *"Flipped when phase reaches `ready` OR when the
> watchdog fires"*. **There is no watchdog.** A repo-wide grep for `watchdog` in `src/` returns two hits: this
> docstring and an unrelated string in `SidecarCrashBanner.tsx:77`. If `ready` never arrives the user is
> **permanently locked out of the application**, with no escape short of killing the process. A docstring that
> lies, in the one place where the lie costs the whole app.

*The user experiences:* a boot screen whose footer promises **"Sidecars take ~15s to warm up on first launch"**
(`StartupOverlay.tsx:88`). The real worst case on a cold first run is far longer, because the Rust boot path is
serial polling: memory health up to **20 × 1 s** (`src-tauri/src/main.rs:378`), orchestrator health up to
**30 × 1 s** (`main.rs:399`), a fixed **3 s** settle (`main.rs:428`), then fleet verification up to
**60 × 1 s** (`main.rs:433`) before `StartupPhase::Ready` is finally emitted at `main.rs:458`.
**Up to ~113 s of an undismissable modal, under a label that says 15 s.**

> **UX-02 (P0).** The startup phase model has **no failure state**. `StartupPhase.status` is
> `"pending" | "active" | "complete"` (`startupStore.ts:34`) — there is no `failed`. `advance()` is monotonic and
> the frontend marks every earlier phase complete (`startupStore.ts:111-115`). So when the orchestrator fails to
> start (`main.rs:395`, `:416` — both `eprintln!` only), the fleet-verification task still advances
> `Workers → FleetReady → Ready` (`main.rs:429,451,458`) after its timeout, and the user watches
> **"Core Orchestrator ready" turn green for a service that never started.** The only signal of the real failure
> is `eprintln!` to a stderr stream that does not exist in a packaged desktop app.

### Step 2 — The overlay clears. The user lands on Chat, with nothing to say anything is missing.

`uiStore.activeTab` defaults to `chat` and is not persisted (G5c F47), so this is also where every future launch
lands. The persistent chrome is TitleBar / TabBar / StatusBar / Sidebar (`App.tsx:83-121`).

> **UX-03 (P0).** **Both status pills disappear in exactly the unconfigured state.**
> `ActiveModelBadge` returns `null` when no model is set (`src/components/layout/TabBar.tsx:82`), and `CostPill`
> returns `null` when cost is 0 (`TabBar.tsx:95`). On first run — the moment the app most needs to say
> *"no provider configured"* — the chrome renders **less** than in the healthy state. Empty and healthy are
> visually identical.

> **UX-04 (P1).** The status bar renders a raw internal identifier by default.
> `PA_MODE_LABELS` / `PA_MODE_COLORS` (`src/components/layout/StatusBar.tsx:8-18`) key on
> `fast | review | delegate`, but the `PAMode` union is `'fast_development' | 'review' | 'delegate'`
> (`src/stores/settingsStore.ts:6`) and the default is `'fast_development'` (`:67`). Both lookups miss, so
> `StatusBar.tsx:34-35` fall through. **Every user, on every fresh launch, sees `PA · fast_development` beside a
> dead grey dot.** Only `review` and `delegate` ever render correctly.

Nothing in the chrome mentions an API key, a provider, or a workspace. The StatusBar shows PA mode, two sidecar
dots, and either a token count or the hardcoded literal `autodev v1.0` (`StatusBar.tsx:101`) — a second, different,
also-wrong version string beside the literal `__APP_VERSION__` in the Updates panel (G5c F15).

### Step 3 — The user types a first message and presses Enter. The message is destroyed.

`Chat.handleSend` (`src/tabs/Chat.tsx:277-317`):

```
:280   setInput('')                                            ← input cleared FIRST
:283   if (!settingsStore.activeProvider || !activeModel) {
:284     alert('Please configure a provider in the Settings tab first.')
:285     return
```

> **UX-05 (P0).** The typed message is cleared at `Chat.tsx:280`, **before** the provider guard at `:283`, before
> `ensureSession` (`:290`), and before `invoke('send_message')` (`:307`). Any failure on that path destroys the
> user's text with no undo and no draft recovery. (Mechanism also in G5a §5; restated because it is the single
> worst moment in the first-run flow.)

> **UX-06 (P0).** The one place the app tells a new user about API keys is a **native `alert()`**
> (`Chat.tsx:284`) — a blocking OS dialog, unstyled, that names "the Settings tab" but **does not navigate
> there**. `uiStore.setActiveTab` is imported in the same tree and is not called. The user must dismiss the
> modal, find the Settings tab themselves, and scroll.

> **UX-07 (P0).** The guard checks the **frontend store only**, never `provider.has_key` — which the frontend
> already holds (`settingsStore.ts:34`). So the "provider selected but key missing / invalid / revoked" case
> **never reaches the alert**: it goes through to `invoke('send_message')`, Rust rejects with `"No API key"`
> (`src-tauri/src/commands/agent_commands.rs:45`), and `Chat.tsx:312-315` responds with `console.error` +
> `stopStreaming()`. *The user experiences:* their message posted, an empty grey assistant bubble that never
> fills, and no error anywhere on screen. **This is the primary BYOK failure mode and it is completely silent.**

### Step 4 — Hunting for Settings. Providers is the 8th section down an unsegmented scroll.

`src/tabs/Settings.tsx:39-110` renders **14 sections in one flat scroll column** with no sub-navigation, no
search, no anchors, and no scroll-position persistence:

`Workspace · Demo Mode · Default Worker Budget · Services · Submodules · MCP Tools · Browser Testing ·`
**`Providers`** `· Model Parameters · GitHub · Workers · System Prompt · Hooks · Updates`

> **UX-08 (P0).** The single control that makes a BYOK product function is placed **eighth**, below six sections
> of infrastructure plumbing (`Demo Mode`, `Services`, `Submodules`, `MCP Tools`, `Browser Testing`) that a new
> user cannot interpret and must not touch. The sections are ordered by **implementation layer**, not by what the
> user needs to do, and nothing marks which are required versus optional.

### Step 5 — Key entry. The key is stored, activated, and never checked.

`ProviderCard` (`Settings.tsx:1053-1259`):

| What the code does | Where | What the user experiences |
|---|---|---|
| Key input renders only when `!hasKey` | `:1187` | Correct on first run |
| Enter or Save → `storeProviderKey` | `:1195`, `:1213`, `:1069-1073` | Fine |
| Then **immediately** `setActiveProvider(providerId, defaultModel)` | `:1074-1076` | A key of unknown validity becomes the ACTIVE provider |
| `defaultModel = provider.models[1]?.id ?? provider.models[0]?.id` | `:1067` | The user is silently put on the **second** model in the list, which they never chose and whose price they were never shown |
| **No `test_provider_connection` anywhere in the save path** | `:1069-1084` | — |
| Save failure → `console.error` only | `:1079-1081` | The card looks unchanged, `has_key` stays false. **Indistinguishable from not having clicked Save.** |

> **UX-09 (P0).** **No validation on save.** A typo'd, revoked, wrong-provider, or wrong-tier key is accepted,
> written to the OS keychain, marked ACTIVE, and given a "connected" cyan dot (`:1120-1123`). The user only learns
> it is wrong when a chat message silently fails (UX-07). The `Test` button exists (`:1145-1156`) but is gated on
> `{hasKey && …}` (`:1143`) — **it is invisible at the exact moment it would be useful**, and nothing prompts it.

> **UX-10 (P0).** **There is no way to replace a key.** The input renders only when `!hasKey` (`:1187`). To rotate
> an expired key the user must click **Remove** — which `confirm()`s (`:1087`) and then *deletes the stored key* —
> and only then does the input reappear. The core BYOK lifecycle operation is **destructive-first**.

> **UX-11 (P1).** **Zero help getting a key.** An exhaustive grep of `src/` for `console.anthropic`,
> `platform.openai`, `aistudio`, `x.ai`, `openExternal`, `shell.open`, `target="_blank"` finds exactly one hit,
> and it is in dead code (`src/components/plan/PlanView.tsx:144`). There is no link, no "where do I get this",
> no format hint, no expected prefix, no cost warning, no note that the key is stored in the OS keychain.

> **UX-12 (P1).** **Nothing identifies the stored key.** No masked prefix, no added-on date, no last-used, no
> label. The only signal that a key exists is a **6 px dot whose sole difference is colour**
> (`:1120-1123`, `background: hasKey ? 'var(--aqua)' : '#0D3652'`) — no text, no icon, no `aria-label`.

> **UX-13 (P2).** Test-result success is decided by **string matching**:
> `testResult.toLowerCase().includes('connected')` (`:1176,1178`). Any success wording omitting the word
> "connected" renders as a red ✗. On failure the raw Rust error string is dumped verbatim (`:1182`) with no
> interpretation and no next step.

> **UX-14 (P2).** The model buttons show **only `model.name`** (`:1250`), while `ModelInfo` already carries
> `context_window`, `cost_in_per_1m`, and `cost_out_per_1m` (`settingsStore.ts:40-43`). In a BYOK product — where
> the user pays directly — the model picker shows no price and no context window, and the data is already in hand.

> **UX-15 (P1).** Clicking a model can fail with **no feedback whatsoever**. `handleSetActive` (`:1105-1107`) has
> no try/catch, and `setActiveProvider` (`settingsStore.ts:95-98`) has none either (G5c F18). *The user
> experiences:* clicking a model button and nothing changing — no spinner, no error, no selection.

> **UX-16 (P0, inherited).** Even a *successful* activation is lost on quit: `set_active_provider` writes only to
> in-memory `AppState` (G5c F01, `src-tauri/src/state/app_state.rs:14-42`). **The user redoes step 5 on every
> launch**, and step 3's `alert()` is the first thing they see every single session.

### Step 6 — Workspace. Optional-looking, silently required, and its own CTA is broken.

`WorkspaceCard` is section 1 (`Settings.tsx:39-40`, component at `:126`). Nothing in Chat, the TabBar, or the
StatusBar signals that a workspace is unset. `Chat.ensureSession` silently falls back to `localDir` when
`workspaceDir` is empty (`Chat.tsx:270`).

> **UX-17 (P0, inherited + confirmed).** `load_workspace_dir()` returning `""` (sibling finding) means agent
> output lands in `~/.autodev/agents/`. The one banner that exists to fix this — `WorkspaceMismatchBanner` — has a
> "set workspace" CTA that invokes **`open_workspace_picker`, a command that does not exist**, with the error
> swallowed by `.catch(() => {})` (`src/components/WorkspaceMismatchBanner.tsx:85`; G5c F14). The real command is
> `pick_workspace_dir`. *The user experiences:* the app tells them their workspace is wrong, offers a button to
> fix it, and the button does nothing at all.
> The Graph tab is the only surface that *blocks* on a missing workspace, and it does so with a native
> `alert('Set a workspace directory in Settings first.')` (`src/tabs/Graph.tsx:659,674`) — again with no navigation.

> **UX-18 (P2).** The Settings tab calls this one concept **three different names within 160 lines**:
> "Workspace" (`Settings.tsx:39`) → "Workspace **Folder**" (`:147`) → "Select **Folder**" (`:169`) →
> "No **workspace** selected. All agents will use their default **directory**." (`:194`) →
> "operate inside this **folder**" (`:199`). Elsewhere the same thing is "repo" (`Graph.tsx:963`) and
> "Repository" (`WorkerCreateModal.tsx:147`).

### Step count: install → first working agent turn

| # | Step | Discoverable? |
|---|---|---|
| 1 | Wait out an undismissable boot overlay (15 s advertised, up to ~113 s real) | forced |
| 2 | Land on Chat. Nothing indicates setup is incomplete | — |
| 3 | Type a message, press Enter — **message destroyed**, native `alert()` fires | no |
| 4 | Dismiss the alert; find the **Settings** tab (6th of 9, unmarked as required) | no |
| 5 | Scroll past 7 infrastructure sections to **Providers** | no |
| 6 | Type/paste the API key, press Save | yes |
| 7 | *(Nothing validates it. No prompt to press Test.)* | — |
| 8 | Notice the app silently chose `models[1]`; pick the model actually wanted | no |
| 9 | Scroll **back up** to Workspace, click Choose, pick a folder | no |
| 10 | Return to Chat, **retype** the destroyed message, send | — |

**Ten steps, of which six are undiscoverable, one destroys user input, and one silently commits an unverified
credential.** On the next launch, steps 3–8 repeat, because of UX-16.

---

## 2. The BYOK flow that should exist

A concrete design, grounded in what already exists in the code so the delta is small.

### 2.1 Gate on readiness, not on failure

Add a single derived selector — call it `setupState` — computed from data the frontend **already has**:

| Input | Already available at |
|---|---|
| `providers[].has_key` | `src/stores/settingsStore.ts:34`, loaded at `App.tsx:64` |
| `activeProvider` / `activeModel` | `settingsStore.ts:48-49` |
| `workspaceDir` | `src/stores/workspaceStore.ts` |

`setupState` ∈ `{ needs_key, needs_model, needs_workspace, ready }`. Nothing new crosses the Tauri boundary.

### 2.2 First run: a dedicated Setup screen, not a modal over Chat

When `setupState !== 'ready'` **and** no key has ever been stored, `App.tsx` renders a **Setup** surface in place
of the tab content — not an overlay. Three steps, visible at once, each collapsible once done:

1. **Connect a provider.** One card per provider from `list_providers`. Each card carries: provider name, a short
   "what this is for" line, a **link to that provider's key page** (currently zero such links exist anywhere —
   UX-11), the expected key prefix as placeholder text, and the sentence *"Your key is stored in the operating
   system keychain and never leaves this machine."* Anything less is not a BYOK product; it is a key field.
2. **Choose a model.** Rendered from `ModelInfo`, showing **name · context window · $ in / $ out per 1M** —
   all three already in the payload and all three currently discarded (UX-14). No auto-selection of `models[1]`.
3. **Choose a workspace.** Reuse `pick_workspace_dir` (the command that actually exists). Show the resolved path
   and one line naming what will be written there.

A **Skip setup** link is required so the app is explorable without a key — but skipping must set a persistent
banner, not silence.

### 2.3 Validate before saving, always

Replace `handleSaveKey` (`Settings.tsx:1069-1084`) with **verify → then commit**:

```
Save clicked
  → button enters "Verifying…" (the Test spinner already exists at :1155)
  → invoke('test_provider_connection', { provider })   ← already registered, already wired at settingsStore.ts:101
  → PASS  → store_provider_key → setActiveProvider(chosen model) → card shows CONNECTED + key fingerprint + timestamp
  → FAIL  → key is NOT stored, NOT activated; the field keeps the typed value; an inline error explains the class
             of failure and what to do
```

This needs no new Rust: `test_provider_connection` is already registered and already invoked from
`settingsStore.testProvider`. The only change is **calling it before the store, not after**, and replacing the
`includes('connected')` string match (UX-13) with a typed result.

Failure classes must be distinguished — today all four render as the same raw error string:

| Class | What the user must be told | Where the signal exists today |
|---|---|---|
| Malformed key | "That does not look like an `<provider>` key — they start with `sk-ant-`" | client-side prefix check, none exists |
| Rejected (401) | "The provider rejected this key. It may be revoked or from a different account." | Rust error text, currently dumped raw at `Settings.tsx:1182` |
| No credit / wrong tier (402/403) | "The key is valid but the account has no available credit." | same |
| Rate-limited (429) | "Valid key, currently rate limited. Retry in N s." + a **Retry** button | same |
| Network down | "Could not reach the provider. Check your connection." + **Retry** | same |

### 2.4 Make configured-ness visible, permanently

- **TabBar**: invert `ActiveModelBadge` (`TabBar.tsx:82`). When `activeModel` is null, render a
  **"No model — click to set up"** pill that switches to the setup surface, instead of rendering `null` (UX-03).
- **Settings → Providers**: each card shows `CONNECTED · sk-ant-…4f2a · verified 3 Aug` or
  `NOT CONFIGURED`, as **text**, not as a 6 px dot (UX-12). Move Providers and Workspace to the **top** of the
  Settings column and group the six infrastructure sections under a collapsed "Advanced" (UX-08).
- **Key rotation** (UX-10): add a `Replace key` control alongside `Test` and `Remove`, which re-shows the input
  in-place. `Remove` stops being the only route to a new key.

### 2.5 Make the failure path recoverable

- `Chat.handleSend`: move `setInput('')` to **after** the `invoke` resolves (UX-05), and on any rejection restore
  the text into the composer.
- Replace the `alert()` at `Chat.tsx:284` with an **inline composer banner**: *"No provider configured —
  [Set up a provider]"*, where the button calls `setActiveTab('settings')` (UX-06).
- Guard on `has_key`, not only on `activeProvider`/`activeModel` (UX-07), so the silent-failure branch stops
  being reachable at all.
- Every `send_message` rejection must render **in the transcript**, replacing the empty bubble, with the error
  class and a **Retry** button. There is currently no retry affordance anywhere in the app (§3.4).

---

## 3. Error-surfacing inventory

**Denominator:** 234 `invoke()` call sites in `src/`; 194 `try` blocks; 172 `catch` blocks.

### 3.1 Counts by category

| Category | Count | Evidence |
|---|---|---|
| **`console.*` only — invisible to the user** | **88** (86 `console.error` + 2 `console.warn`) across 30 files | worst offenders: `stores/planStore.ts` (10 — in dead code), `stores/taskStore.ts` (9), `tabs/Graph.tsx` (8), `components/workers/WorkerTable.tsx` (6), `stores/settingsStore.ts` (5), `stores/sessionStore.ts` (5) |
| …of which the **entire `catch` body is a `console.error`** | **72** | the failure is fully absorbed; nothing reaches the DOM |
| **Native `alert()` — blocking OS modal** | **8** | `chat/SessionBar.tsx:28`; `tabs/Chat.tsx:284`; `tabs/Graph.tsx:659,667,674,682,693,699` |
| **Toast** | **3 call sites, 0 functional** | `stores/workerStore.ts:283-284,324,326,333` — all broken (§0.3) |
| **Inline banner / inline error text rendered to the DOM** | **22** render sites across **17** files | `tabs/Settings.tsx`, `tabs/AutoDevPanel.tsx`, `git/GitPanel.tsx`, `sidecar/SidecarPanel.tsx`, `settings/HooksSettings.tsx`, `workers/WorkerTable.tsx`, `workers/WorkerCreateModal.tsx`, `recipes/RecipeEditor.tsx`, `skills/SkillCreatorModal.tsx`, `skills/SkillParameterModal.tsx`, `sidecar/McpToolBrowser.tsx`, `hooks/HookLogViewer.tsx`, `dashboard/Dashboard.tsx`, `dashboard/QcRoiPanel.tsx`, `autodev/AutodevMdEditor.tsx`, `subagent/SubagentResultCard.tsx`, `plugin/WorkerSetupWizard.tsx` (dead) |
| **Top-level crash banner (real, works)** | **3 banners + 1 boundary** | `WorkspaceMismatchBanner` / `StrayFilesBanner` (`App.tsx:96-101`), `SidecarCrashBanner` (`App.tsx:103`), `ErrorBoundary` (`App.tsx:106-119`) |
| **Silent — `.catch(() => {})`, a true no-op** | **5** | `autodev/AgentLiveCard.tsx:110`, `city/interaction/BuildingContextMenu.tsx:29,34`, `WorkspaceMismatchBanner.tsx:86`, `tabs/Settings.tsx:597` |
| **Silent — error collapsed into an empty value** | **8** | `recipes/RecipeEditor.tsx:135` `setBackups([])`, `skills/SkillsPalette.tsx:49`, `workers/WorkerTable.tsx:357` `setRecipes([])`, `tabs/Chat.tsx:340` `setAtResults([])`, `:354` `setSlashResults([])`, `tabs/Settings.tsx:464` `setHasPat(false)` — *"the request failed" and "there is nothing" render identically* |
| **Silent — unhandled `invoke` rejection, no `catch`, no `try`** | **48** | enumerated in G5c §2; `gitStore.ts` alone accounts for **15**, covering **every mutating git operation** |
| **Retry affordance anywhere in the app** | **3** | `ErrorBoundary.tsx:64` (inline "retry"), `ErrorBoundary.tsx` full-page variant, `ide/CodeViewer.tsx:183` ("Retry"). **That is the complete list.** |

**Net:** of ~180 identifiable failure paths, **roughly 140 produce nothing a user can see**. The three surfaces that
do work well (`WorkspaceMismatchBanner`, `SidecarCrashBanner`, `ErrorBoundary`) are all *startup/crash* banners —
none of them covers a routine operation failing.

### 3.2 What the user actually sees, per failure

| Failure | What the code does | What the user sees | `path:line` |
|---|---|---|---|
| **No API key** | `send_message` rejects; `console.error` + `stopStreaming()` | **Nothing.** An empty assistant bubble that never fills; the typed message already destroyed | `tabs/Chat.tsx:312-315`; `src-tauri/src/commands/agent_commands.rs:45` |
| **No provider selected** | native `alert()` | A blocking OS dialog naming a tab it will not open to; the typed message destroyed | `tabs/Chat.tsx:280,284` |
| **Invalid / revoked key** | identical to "no API key" — the key was never validated | **Nothing.** Same empty bubble. There is no distinct path | `Settings.tsx:1069-1084` (no validation), `Chat.tsx:312` |
| **Rate limited (429)** | reaches `agent:error` → `agentStore` | Error text rendered **only if the last message is an assistant message with empty content** (G5a §7, `stores/agentStore.ts:257`). If any token streamed first, **the error is discarded and the stream just stops** | `stores/agentStore.ts:252-262` |
| **Network down at boot** | `list_providers` fails → `console.error`, `providers: []`, `isLoadingProviders: false` | The Providers section renders **empty forever**, with no "Loading…" and **no refresh control anywhere in the section** (G5c F28) | `stores/settingsStore.ts:88-90`; `tabs/Settings.tsx:62-70` |
| **Worker crashes** | `worker:status` handler fires `addToast` via `require()` | **Nothing** — the call throws (§0.3). `workerStore.ts:264` already applied the status change, so the row turns red; that is the entire signal | `stores/workerStore.ts:279-291` |
| **QC decision (pass or fail)** | handler body is `require()` + `addToast` and nothing else | **Nothing at all.** The whole `qc:decision` handler is dead — first statement throws | `stores/workerStore.ts:322-340` |
| **A task fails / build fails** | `task:changed` moves the card to a Failed column | The card moves. No notification, no reason surfaced, and `cancelled` tasks are rendered in **no column at all** — they vanish silently | `stores/taskStore.ts:95-104` vs `tabs/AutoDevPanel.tsx:1116-1119` |
| **Any git mutation fails** (push, commit, checkout, discard, restore) | unhandled promise rejection | **Nothing.** The button appears to do nothing (G5c F10) | `stores/gitStore.ts:270-352` |
| **Sidecar dies** | `SidecarCrashBanner` renders with stderr tail + copy-report | **Works well** — the best error surface in the app | `components/SidecarCrashBanner.tsx:59-90` |
| **A component throws during render** | `ErrorBoundary` renders label + message + retry | **Works well** — and every tab is wrapped (`App.tsx:106-114`) | `components/ErrorBoundary.tsx:44-72` |
| **Startup phase fails** | no failure state exists in the model | A **green check** on a service that never started (UX-02) | `stores/startupStore.ts:34,111-115` |

### 3.3 Which failures are visible but unactionable

- `Settings.tsx:1182` dumps the raw Rust error from `test_provider_connection` verbatim, with no class, no cause,
  no next step, and no retry.
- `tabs/Graph.tsx:667,682,693,699` — `alert(\`Build failed: ${err}\`)`, `Update failed`, `Delete failed`,
  `Export ${format} failed`. Four native modals, each carrying a raw serialized Rust error and nothing else.
- `AutoDevPanel.tsx` clear-all error is shown, then **auto-dismissed after 6 s** (`:1261`) — a user who looked away
  loses the only record of the failure.
- `ErrorBoundary` shows `error.message` and a component stack — accurate, but framed for a developer, not a user.

### 3.4 Retry

There are **three** retry affordances in 35k LOC: two inside `ErrorBoundary` and one in `CodeViewer`. No failed
message, no failed provider test, no failed git operation, no failed graph build, no failed dashboard fetch, and
no failed key save offers a retry. Every recovery is "figure out what happened, then redo it by hand."

---

## 4. Findings table

Severity: **P0** = blocks or silently breaks the core loop / destroys user data · **P1** = a feature reads as
broken or a user is misled · **P2** = friction, inconsistency, degraded trust · **P3** = polish.
Area: `onboarding` · `byok` · `errors` · `feedback` · `nav` · `safety` · `consistency` · `a11y`.

| ID | file:line | Sev | Area | What the user experiences | Why it is wrong | The fix, in one line |
|---|---|---|---|---|---|---|
| UX-01 | `src/stores/startupStore.ts:80-82,132-134`; `components/startup/StartupOverlay.tsx:47-93` | P0 | onboarding | If boot never reaches `ready`, a full-screen modal never goes away and the app cannot be used | The docstring promises a watchdog that does not exist; the overlay has no dismiss control | Add a 30 s watchdog that flips `dismissed` and shows a "boot did not complete — continue anyway" state |
| UX-02 | `src/stores/startupStore.ts:34,111-115`; `src-tauri/src/main.rs:395,416,458` | P0 | feedback | Boot phases turn green for services that failed to start | `StartupPhase.status` has no `failed` variant and `advance()` is monotonic | Add `failed` to the status union, emit it from Rust on each error branch, render it red with the reason |
| UX-03 | `src/components/layout/TabBar.tsx:82,95` | P0 | onboarding | On first run the chrome shows *less* than when configured — unconfigured and healthy look identical | Both status pills `return null` in exactly the state that needs a signal | Render "No model — set up" / "No key" pills that navigate to setup instead of returning `null` |
| UX-04 | `src/components/layout/StatusBar.tsx:8-18,34-35` vs `stores/settingsStore.ts:6,67` | P1 | consistency | The status bar reads `PA · fast_development` with a dead grey dot, on every launch | The label/colour maps key on `fast`, the union value is `fast_development` | Key both maps on the `PAMode` union values |
| UX-05 | `src/tabs/Chat.tsx:280` vs `:283-315` | P0 | errors | Typing a message and pressing Enter with no provider **destroys the message** | `setInput('')` runs before every validation and before the `invoke` | Clear the input only after `send_message` resolves; restore on rejection |
| UX-06 | `src/tabs/Chat.tsx:284` | P0 | onboarding | A native OS `alert()` says "configure a provider in the Settings tab" and does not open it | `setActiveTab` exists and is not called; `alert()` cannot carry an action | Replace with an inline composer banner carrying a **Set up a provider** button |
| UX-07 | `src/tabs/Chat.tsx:283` (guard) vs `settingsStore.ts:34` (`has_key`) | P0 | byok | A missing/invalid/revoked key produces an empty grey bubble and no error anywhere | The guard checks `activeProvider`/`activeModel` only, never `has_key`; the reject path is `console.error` | Guard on `has_key` too, and render every `send_message` rejection in the transcript |
| UX-08 | `src/tabs/Settings.tsx:39-110` | P0 | nav | Providers — the one required control — is the 8th of 14 sections in a flat scroll, below six infrastructure panels | Sections are ordered by implementation layer, not by task, with nothing marked required | Reorder to Providers · Workspace · Models first; collapse the rest under "Advanced"; add section nav |
| UX-09 | `src/tabs/Settings.tsx:1069-1084,1143` | P0 | byok | A wrong key is silently accepted, activated, and shown as connected | `handleSaveKey` never calls `test_provider_connection`; `Test` renders only after `hasKey` | Verify before storing: `test_provider_connection` → on pass store + activate; on fail keep the text and explain |
| UX-10 | `src/tabs/Settings.tsx:1187` | P0 | byok | Rotating an expired key requires **deleting** the existing one first | The key input renders only when `!hasKey`; there is no replace path | Add a `Replace key` control that re-shows the input without deleting |
| UX-11 | `src/tabs/Settings.tsx:1186-1225`; grep of `src/` | P1 | byok | No link, hint, prefix, or price guidance anywhere in the key flow | Zero external links exist in the live tree (only `plan/PlanView.tsx:144`, dead) | Add a per-provider "Get a key" link, expected prefix as placeholder, and a keychain-storage note |
| UX-12 | `src/tabs/Settings.tsx:1120-1123` | P1 | byok | The only indication a key is stored is a 6 px colour change | Colour-only encoding, no text, no `aria-label`, no fingerprint, no timestamp | Render `CONNECTED · sk-ant-…4f2a · verified <date>` as text |
| UX-13 | `src/tabs/Settings.tsx:1176-1182` | P2 | byok | Test success is decided by whether the message contains the word "connected"; failures dump raw Rust errors | String-matching a status; no error classification | Return a typed result; map 401/402/429/network to distinct messages with a Retry |
| UX-14 | `src/tabs/Settings.tsx:1250` vs `stores/settingsStore.ts:40-43` | P2 | byok | Model buttons show a name only — no price, no context window, in an app where the user pays | `ModelInfo` already carries all three fields and two are discarded | Render name · context window · $in/$out per 1M on each model chip |
| UX-15 | `src/tabs/Settings.tsx:1105-1107`; `stores/settingsStore.ts:95-98` | P1 | errors | Clicking a model can do nothing at all — no spinner, no error, no selection | No try/catch on either side (G5c F18) | Wrap in try/catch, show a pending state, surface failures inline |
| UX-16 | `src-tauri/src/state/app_state.rs:14-42` (G5c F01) | P0 | byok | The model choice is lost on every restart; the `alert()` fires every session | `AppState` is `#[derive(Default)]` with no load/save path | Serialize active provider/model to disk and rehydrate at boot |
| UX-17 | `src/components/WorkspaceMismatchBanner.tsx:85` (G5c F14) | P0 | onboarding | The banner says the workspace is wrong and offers a button that does nothing | Invokes `open_workspace_picker`, which does not exist; error swallowed by `.catch(() => {})` | Call `pick_workspace_dir` and surface failures |
| UX-18 | `src/tabs/Settings.tsx:39,147,169,194,199` | P2 | consistency | One concept called "Workspace", "Folder", and "directory" within 160 lines | No shared vocabulary | Pick one noun (Workspace) and use it everywhere |
| UX-19 | `src/stores/toastStore.ts:3,5,27` vs `stores/workerStore.ts:283,324` | P0 | errors | **No toast has ever appeared.** Worker errors and QC pass/fail decisions produce zero notification | `require()` does not exist in the ESM bundle and `addToast` is not an exported symbol | Import `useToastStore` and call `useToastStore.getState().addToast(...)`; then route failures into it app-wide |
| UX-20 | `src/stores/workerStore.ts:322-340` | P0 | errors | QC approving or rejecting a task produces **no UI signal at all** | The `qc:decision` handler's first statement throws; the whole body is dead | Same fix as UX-19; the handler has no other effect to preserve |
| UX-21 | `src/stores/gitStore.ts:270-352` (G5c F10) | P0 | errors | A failed commit, push, pull, checkout, discard or restore shows nothing — the button appears inert | 15 mutating operations with no `catch` | Route every git mutation failure to a toast/inline error |
| UX-22 | 72 `catch` blocks whose whole body is `console.error` | P0 | errors | ~140 of ~180 failure paths are invisible | The DevTools console is not a user surface in a packaged desktop app | Establish one rule: every `catch` either renders or rethrows |
| UX-23 | `src/tabs/Graph.tsx:659,667,674,682,693,699` | P1 | errors | Six native `alert()` modals carrying raw serialized Rust errors | `alert()` blocks the app and cannot carry an action or a retry | Replace with inline banners + Retry |
| UX-24 | `src/tabs/Chat.tsx:340,354`; `Settings.tsx:464`; `WorkerTable.tsx:357`; `RecipeEditor.tsx:135`; `SkillsPalette.tsx:49` | P2 | errors | "The search failed" and "there are no results" render identically | 8 sites collapse an error into an empty value | Track an `error` state distinct from `empty` in each |
| UX-25 | `src/tabs/Graph.tsx:975-985,795,799` | P0 | feedback | A 30–90 s graph build cannot be cancelled — Build/Update merely go `disabled` | No `cancel_graph`/`stop_build` invoke exists anywhere in `src/` | Add a cancel command and a Cancel button on the build overlay |
| UX-26 | `src/components/update/UpdateDialog.tsx:117-125,216-224` | P1 | feedback | Closing the update dialog leaves the download running invisibly with no way to stop it | The header `✕` calls `closeDialog` only | Either cancel the download on close or show a background-download indicator with a stop control |
| UX-27 | `src/tabs/Settings.tsx:726,790` | P1 | feedback | Installing Playwright browsers (hundreds of MB) shows only a disabled button — no progress, no cancel | No progress channel and no abort | Stream progress and offer cancel, or state the size and expected duration up front |
| UX-28 | `src/tabs/AutoDevPanel.tsx:322-332`; `styles/globals.css:161-164` | P2 | feedback | A 2 px unlabelled sweeping bar is the only signal that a task is being worked on | *Correction to a first-pass reading: the `shimmer` keyframe translates `-100% → 200%` (`globals.css:161-164`), so the 40 % segment genuinely **sweeps** — it is a legitimate indeterminate indicator, not fake progress.* The defect is that it is 2 px tall, carries no label, no elapsed time, and no turn count, while real determinate data (`BudgetBar`) sits directly below it at `:334` | Give it a label and elapsed time, or fold it into the budget bar |
| UX-29 | `src/tabs/AutoDevPanel.tsx:116,122` | P1 | feedback | With zero workers registered, the roster says **"Loading agents…" forever** — and the text is `color: '#333'` on `--bg-surface: #161622` (`globals.css:11`), roughly **1.5:1 contrast**, so it is barely visible even when it does render | The branch is gated on `workers.length === 0`, not on a loading flag; the colour is a hardcoded near-black on near-black (same at `:222`) | Gate on `workerStore.isLoading`; add a real "No workers yet — add one" empty state; use a token colour |
| UX-30 | `src/components/workers/WorkerTable.tsx:404,420-449` | P1 | feedback | With no workers, Settings shows `0 agents registered` above an empty header-only table | No `workers.length === 0` branch exists | Add an empty state with an **Add worker** call to action |
| UX-31 | 7 stores expose `isLoading`, no component reads it | P2 | feedback | Lists pop from empty to full with no loading indication | `taskStore`, `planStore`, `sessionStore`, `workspaceStore`, `gitStore`, `modelConfigStore` loading flags are never rendered | Render the flags that already exist |
| UX-32 | `src/components/layout/Sidebar.tsx:58,65` | P2 | feedback | Two of seven sidebar status dots are hardcoded `active={false}` and can never light up | `.autodevhints` and `github` are decorative | Wire them or delete them |
| UX-33 | `src/components/layout/Sidebar.tsx:64` | P2 | consistency | The `memory-mcp` dot reports the **core-orchestrator's** status | It is bound to `orcRunning`, not to memory health, which `sidecarStore` tracks separately | Bind to the memory server's own health |
| UX-34 | `src/components/layout/Sidebar.tsx` (whole file) | P2 | nav | A permanent 176 px column in every tab containing **no clickable element at all** | Pure read-only status; its best real estate is two dots, one of them fake | Make its rows navigate (skills → palette, hooks → settings, session → chat) or reclaim the space |
| UX-35 | `src/App.tsx:39-49`; `setActiveTab` grep | P1 | nav | Nothing in the app ever navigates to Settings or Docs | `setActiveTab` is called from only two places outside the TabBar: `AnnotationPanel.tsx:113` and `utils/openInIde.ts:25` | Add cross-links from every "go configure X" message |
| UX-36 | `src/components/layout/TabBar.tsx:39-68` | P2 | a11y | The main tab bar is nine unlabelled buttons to a screen reader | No `role="tablist"`/`role="tab"`/`aria-selected`; no arrow-key navigation | Add tab roles, `aria-selected`, and arrow-key movement |
| UX-37 | `src/components/git/GitPanel.tsx:208` | P0 | safety | A one-click trash icon permanently destroys uncommitted work with **no confirmation** | `discard([e.path])` with `title='Discard'` and no guard; the store call also has no `catch` | Require confirmation naming the file, and offer an undo window via a checkpoint |
| UX-38 | `src/components/git/StashManager.tsx:100` | P0 | safety | A stash is permanently dropped by one click on an icon that is **invisible until hover** | No confirmation; `opacity-0 group-hover:opacity-100` | Confirm, and make destructive controls visible rather than hover-revealed |
| UX-39 | `src/components/git/CheckpointViewer.tsx:100` vs `:72-88` | P1 | safety | Deleting a checkpoint is one click, while *restoring* one (reversible) requires a two-step confirm | The confirmation is on the wrong action | Confirm the delete; the restore confirm can stay |
| UX-40 | `src/components/git/BranchManager.tsx:68,139` | P1 | safety | Deleting a branch is one click with no confirmation, and the failure is silent | No guard; `gitStore.ts:318` has no `catch` | Confirm, and surface git's refusal when the branch is unmerged |
| UX-41 | `src/tabs/AutoDevPanel.tsx:348` | P1 | safety | A task is deleted by one click on a trash icon on the card | No confirmation | Confirm, or offer undo |
| UX-42 | `src/components/settings/HooksSettings.tsx:287` | P1 | safety | A hook is removed by one click | No confirmation | Confirm |
| UX-43 | Three confirmation idioms in one app | P2 | consistency | Native `confirm()` (7 live sites), two-step inline with a **3 s auto-revert** (3 sites), and none at all (6 sites) | No single pattern; the 3 s window silently expires | Standardise on one in-app confirm component; drop the auto-revert timers |
| UX-44 | `src/stores/taskStore.ts:95-104` vs `tabs/AutoDevPanel.tsx:1116-1119` | P1 | consistency | A `cancelled` task **vanishes from the board** with no explanation | The store's canonical column list omits `build_failed`/`cancelled`/`rejected`; the panel renders a different set again | One source of truth for status → column → label → colour |
| UX-45 | `src/components/dashboard/WorkerStatus.tsx:222,227` | P2 | consistency | The Dashboard shows the raw enum (`in_progress`) while the Workers table shows "In Progress" for the same worker | The Dashboard bypasses `workerStatusLabel()` (`stores/workerStore.ts:56-77`) | Use the shared helper everywhere |
| UX-46 | `stores/workerStore.ts:67-76` | P2 | consistency | An unmatched worker status renders as **nothing at all** | `workerStatusLabel()` has no `default` arm | Add a default that echoes the raw value |
| UX-47 | 9 date formats, 5 hand-rolled "x ago" implementations | P2 | consistency | The same instant reads differently in five panes; two "just now" thresholds disagree (5 s vs 10 s) | No shared time formatter; `date-fns` is a dependency used exactly once (`chat/SessionBar.tsx:84`) | One `formatTime`/`formatAgo` util, used everywhere |
| UX-48 | 7 cost formats, 5 copies of the k/M token abbreviator | P2 | consistency | The same token total renders `12.3k` in the sidebar and `12.3K` in the budget bar; costs use 2, 3, 4 and 6 dp | Duplicated formatters with divergent rules | One `formatCost`/`formatTokens` util |
| UX-49 | `src/components/dashboard/SystemHealth.tsx:34,41,48,55,62` | P1 | consistency | A health panel whose timestamps are hardcoded literals (`'2 seconds ago'`) and never change | Static placeholder data shipped as if live | Delete the component (it is already unreachable — §5) or wire it |
| UX-50 | zero `prefers-reduced-motion` in `src/` incl. all 14 CSS files | P1 | a11y | Infinite spinners and pulsing status dots never stop for motion-sensitive users | No reduced-motion guard exists anywhere | Add one `@media (prefers-reduced-motion: reduce)` block disabling infinite animations |
| UX-51 | 0 `<label htmlFor>` against 102 form controls | P1 | a11y | No input, textarea or select is programmatically associated with its label | Labels rely on visual proximity only | Associate every control |
| UX-52 | 0 focus traps; 10 of 14 overlays ignore Escape; 0 restore focus | P1 | a11y | Tab walks out of every dialog into the page behind it; most modals cannot be closed from the keyboard | No focus management exists | One `Modal` primitive with trap, Escape, and focus restore |
| UX-53 | 0 `aria-pressed`, 1 `aria-expanded`, ~15 colour-only status indicators | P1 | a11y | Toggle state and system health are invisible to assistive tech | State is carried by colour and by nothing else | Add `role="switch"`/`aria-pressed`; pair every status dot with text |
| UX-54 | `styles/globals.css:105,107` | P2 | a11y | Amber means both "blocked" and "in-progress"; `.dot-purple` ("QC review") resolves to the same `var(--info)` as `.dot-blue` | Two semantically different states render identically | Give each state a distinct colour **and** a text label |
| UX-55 | `src/tabs/Docs.tsx` + `data/docs*.ts` (1,772 lines) | P2 | nav | The app's only explanation of its own concepts is the **last** tab, with no cross-link from anywhere | Nothing calls `setActiveTab('docs')`; the only contextual explainer in the app is `HooksInfoModal` (`HooksSettings.tsx:198`) | Add `?` affordances that deep-link into Docs from PA modes, Recipes, Workers, QC and Skills |
| UX-56 | `src/components/skills/SkillsPalette.tsx:53-65`; `tabs/IDE.tsx:57-66`; `tabs/Docs.tsx:117-126`; `ToastStack.tsx:49-60` | P2 | nav | Four keyboard shortcuts exist and there is no cheatsheet; two are documented as global but are tab-scoped | `Ctrl+P` and `Ctrl+K` are registered inside tab components and die on unmount | Publish a shortcut list, and hoist genuinely global shortcuts to `App.tsx` |
| UX-57 | `src/components/skills/SkillsPalette.tsx:57` | P3 | a11y | The skills palette shortcut requires a **capital** `S` (`e.key === 'S'`), i.e. Cmd/Ctrl+**Shift**+S, matching the `⌘⇧S` hint at `:207` | Correct, but there is no lower-case fallback, so `Cmd+S` variants silently do nothing | Accept both cases |
| UX-58 | `src/tabs/Graph.tsx:1177-1178` | P3 | a11y | A `role="button" tabIndex={0}` div that is focusable but has **no `onKeyDown`** — reachable by keyboard, not activatable | Focusable non-button with no key handler | Use a real `<button>` |
| UX-59 | `src/components/autodev/LiveMonitorSection.tsx:78` | P3 | consistency | "No **agents** initialised — start a **worker** to see live data" — two words for one concept in one sentence | Vocabulary drift inside a single string | Pick one term (§7.1) |
| UX-60 | `src/tabs/Chat.tsx:628-651` (G5a §9) | P2 | nav | A floating 🤖 button is `position: fixed; z-index: 999` directly on top of the Send/Cancel region, and renders even in the empty state | Overlaps the primary action of the primary screen | Move it into the composer toolbar |

---

## 5. Orphaned and unreachable surfaces

Verified independently of the sibling audit by an import-path grep across every `.ts`/`.tsx` in `src/`,
excluding each file's own definition. **Result matches G5c exactly.**

| File | LOC | External importers | Status |
|---|---|---|---|
| `src/components/plugin/PluginManager.tsx` | 487 | **0** | unreachable |
| `src/components/plugin/ITDashboard.tsx` | 439 | **0** | unreachable |
| `src/components/plugin/WorkerSetupWizard.tsx` | ~380 | 1 — `ITDashboard.tsx:11` | **transitively** unreachable |
| `src/components/memory/MemorySidebar.tsx` | 408 | **0** | unreachable |
| `src/components/memory/MemoryStats.tsx` | ~62 | 1 — `MemorySidebar.tsx:12` | **transitively** unreachable |
| `src/components/dashboard/SystemHealth.tsx` | 329 | **0** | unreachable |
| `src/components/plan/PlanView.tsx` | 180 | **0** | unreachable |
| `src/components/plan/PlanTreeView.tsx` | 71 | **0** | unreachable |
| `src/components/chat/PlanDisplay.tsx` | ~130 | **0** | unreachable |
| `src/components/chat/PaModeToggle.tsx` | ~60 | **0** | unreachable |
| `src/components/sidebar/PlanCard.tsx` | 91 | **0** | unreachable |

Plus 733 lines of stylesheet for those components (`PlanView.css` 323, `PlanTreeView.css` 133, `PlanCard.css` 139,
`PlanDisplay.css` 138) and three stores reachable only from them — `planStore` (271), `memoryStore` (258),
`pluginStore` (296).

**Confirmed:** the **memory**, **plugin**, and **plan** feature directories are entirely unreachable from any
route — **~3,700 LOC** of frontend, matching the sibling audit's figure. Three whole product capabilities have a
Rust backend, a store, a complete UI, and no way in.

### Other unreachable or inert surfaces (UX-relevant)

| Surface | Why the user can never reach or use it | `path:line` |
|---|---|---|
| The **toast system** | Mounted, styled, keyboard-dismissible — and it has never rendered a toast (§0.3) | `App.tsx:117`; `stores/workerStore.ts:283,324` |
| **QC decision notifications** | The `qc:decision` handler throws on its first statement | `stores/workerStore.ts:322-340` |
| **"View logs" on every agent card** | Invokes `navigate_to_logs`, which does not exist; error swallowed | `components/autodev/AgentLiveCard.tsx:110` (G5c F13) |
| **"Set workspace" on the mismatch banner** | Invokes `open_workspace_picker`, which does not exist | `components/WorkspaceMismatchBanner.tsx:85` (G5c F14) |
| **Checkpoint creation** | The UI can list, restore and delete checkpoints; `create_checkpoint` has no caller | G5c F34 |
| **`terminate_subagent`** | Registered and wired in the store; no UI calls it (only `abort_subagent`) | `stores/subagentStore.ts:200` |
| **PA-mode dropdown** | Fully explained, fully rendered — and cannot change approval behaviour, because `agent_commands.rs:142` passes `ManualApproval` unconditionally | `tabs/Chat.tsx:101-199` (G5a §5) |
| **Extended thinking** | Streamed and billed by Rust, never listened for by the frontend | G5c F32 |
| **"Open in IDE" from the Graph** | Emits `ide:navigate`; the IDE tab has no listener | G5c F33 |
| **Staging URL + browser-testing toggle** | Pure local state; no persistence, no consumer, and the toggle auto-flips itself | `tabs/Settings.tsx:708-709,852-898` (G5c F09) |
| **Default Worker Budget (4 controls)** | Persist correctly and are read by nothing | G5c F06 |
| **5 of 13 hook templates** | Install, show ENABLED, and fire on events Rust never fires — including two labelled *safety* | G5c F03 |
| **47 registered Rust commands** | 18 % of the command surface has no UI entry point | G5c F29 |

---

## 6. Destructive actions and confirmation

Every mutating operation that can lose user data, and whether it asks first.

### 6.1 No confirmation at all

| # | Control | `path:line` | What is lost |
|---|---|---|---|
| D1 | Git **Discard** — one-click `Trash2` on each unstaged change | `components/git/GitPanel.tsx:208` → `stores/gitStore.ts:280` | **Uncommitted work, permanently.** The single most dangerous control in the app. The store call also has no `catch`, so a *failed* discard is equally silent |
| D2 | **Drop stash** — one-click `Trash2`, `opacity-0 group-hover:opacity-100` | `components/git/StashManager.tsx:100` → `gitStore.ts:342` | A whole stash, permanently. The control is **invisible until hover** |
| D3 | **Delete checkpoint** — one-click `Trash2` | `components/git/CheckpointViewer.tsx:100` → `gitStore.ts:352` | A restore point. Note the *restore* button beside it (`:72-88`) **does** have a two-step confirm — the guard is on the reversible action |
| D4 | **Delete branch** — one-click `Trash2` | `components/git/BranchManager.tsx:68,139` → `gitStore.ts:318` | A branch (`force=false`, so git refuses unmerged — but the refusal is **silent**, no `catch`) |
| D5 | **Delete task** — one-click `Trash2` on the card | `tabs/AutoDevPanel.tsx:348` → `stores/taskStore.ts:186` | A task and its history |
| D6 | **Remove hook** | `components/settings/HooksSettings.tsx:287` → `stores/hookStore.ts:139` | A configured hook; the store call has no `catch` either |
| D7 | **Abort subagent** | `components/subagent/SubagentMonitor.tsx:123` → `stores/subagentStore.ts:205` | In-flight work. Arguably acceptable for a stop, but it is irreversible and unguarded |
| D8 | **Delete annotation** | `stores/annotationStore.ts:130` (`AnnotationPanel`) | An annotation |
| D9 | **Delete/reset model config** | `stores/modelConfigStore.ts:105` | Tuned model parameters |
| D10 | **Delete AUTODEV.md instructions** | `stores/autodevMdStore.ts:131` | Custom agent instructions |

### 6.2 Two-step inline confirm — with a 3 s auto-revert

| Control | `path:line` | Note |
|---|---|---|
| Delete worker | `components/workers/WorkerTable.tsx:360-368` | 3 s window, then silently reverts |
| Clear **all** tasks | `tabs/AutoDevPanel.tsx:1241-1246` | 3 s window. This is the highest-blast-radius control in the app and it uses the weakest idiom |
| Clear log files | `tabs/Logs.tsx:384-389` | 3 s window |
| Restore checkpoint | `components/git/CheckpointViewer.tsx:72-88` | No timeout — the correct shape |

### 6.3 Native `confirm()` — 7 live sites

`chat/SessionBar.tsx:31` (delete session) · `ide/FileTree.tsx:99` (delete file, *"This cannot be undone"*) ·
`recipes/RecipeEditor.tsx:177` (delete recipe) · `:193` (restore backup) · `tabs/Graph.tsx:689` (delete graph) ·
`tabs/Settings.tsx:484` (remove GitHub PAT) · `:1087` (remove provider API key).
Two more are in dead code: `plugin/ITDashboard.tsx:340`, `plugin/PluginManager.tsx:254`.

### 6.4 Verdict

**Three different confirmation idioms coexist**, and the mapping between danger and guard is inverted:
the most destructive control in the product (D1, permanent loss of uncommitted work) has **no guard**, while a
reversible checkpoint restore has a two-step one. `confirm()` is a native OS modal that is unstyleable,
unbrandable, and inconsistent with the two in-app idioms. **Ten unguarded destructive controls; six of them
irreversible.**

---

## 7. Consistency and accessibility

### 7.1 Vocabulary — the same concept, different words

| Concept | Competing labels the user sees | Where |
|---|---|---|
| **worker / agent / fleet** | **"Worker"** in Dashboard + Settings + Workers table; **"Agent"** in AutoDev, Chat, Recipes, Plan; **"Fleet"** for the collective | `dashboard/WorkerStatus.tsx:91,149`; `Settings.tsx:45,88` vs `AutoDevPanel.tsx:1194` "Agent Roster", `recipes/RecipeEditor.tsx:509` "Agent Role", `hooks/HooksInfoModal.tsx:84` "Fleet (Workers):" |
| …in a **single sentence** | *"No **agents** initialised — start a **worker** to see live data"* | `autodev/LiveMonitorSection.tsx:78` |
| …in a **single section** | Settings §Workers (`:88`) whose body says *"All **agents** (PA, Supervisor, Workers)…"* | `Settings.tsx:88` vs `:194,199` |
| **session / chat** | Tab is called **Chat** (`App.tsx:40`); its own toolbar calls the same object a **Session** (`chat/SessionBar.tsx:102,146`); Memory calls it a **conversation** (`memory/MemoryStats.tsx:36,41`) | as cited |
| **workspace / folder / directory / repo / repository** | All five, for one concept | `Settings.tsx:39,147,169,194,199`; `Graph.tsx:963` "repo"; `WorkerCreateModal.tsx:147` "Repository"; `ide/FileTree.tsx:66` "No workspace" |
| **skill / tool / command** | "Skill" in the Sidebar + palette; "Tool" in Settings/Dashboard/sidecar/Plan; "Command" in Hooks — and `HooksSettings.tsx:514` puts **both** in one label: *"command (tool input)"* | `Sidebar.tsx:70`; `Settings.tsx:54`; `HooksSettings.tsx:514,911,922` |
| **recipe / workflow** | "Recipe" everywhere except the Skill creator, whose placeholder is `"e.g., my-custom-workflow"` | `SkillCreatorModal.tsx:88` |
| **task** | Clean — "Task" throughout; one qualified variant "Fleet Task" (`ide/AnnotationPanel.tsx:107`). No ticket/job/work-item drift |
| **provider / model** | Clean and distinct, except one jargon slip to "LLM" mid-paragraph (`Settings.tsx:282`) |

### 7.2 Status vocabularies — eight independent systems

There is **no central `taskStatusLabel()` / `taskStatusColor()`**. `TaskStatus` has 10 members
(`stores/taskStore.ts:10`) and three mutually inconsistent maps exist:

| Map | Where | Keys | Gap |
|---|---|---|---|
| `KANBAN_COLUMNS` | `stores/taskStore.ts:95-104` | 7 | omits `build_failed`, `cancelled`, `rejected` |
| `COL_HEADER_COLORS` | `tabs/AutoDevPanel.tsx:394-401` | 6 | keyed on **column ids, not statuses**; omits `assigned`, `done`, `failed` → those headers get `undefined` colour |
| runtime column list | `tabs/AutoDevPanel.tsx:1116-1119` | different again | adds `rejected`, folds `build_failed` into `done_today` |

**User-visible result:** the panel renders a different column set than the store declares, and a **`cancelled`
task appears in no column at all** — it vanishes from the board with no explanation (UX-44).

Colour vocabularies diverge too: `taskStore.ts:106-113` uses CSS vars with a fallback; `AutoDevPanel.tsx:397-400`
mixes CSS vars with two hardcoded hexes (`#FF6B35`, `#F59E0B`).

**Worker status** has helpers (`stores/workerStore.ts:56-77`) but `workerStatusLabel()` has **no `default` arm**,
so an unmatched status renders as nothing. `dashboard/WorkerStatus.tsx:222,227` bypasses the helpers entirely and
prints the **raw enum** — so the same worker reads `working` in the Dashboard and "Working" in the Workers table.
`AutoDevPanel.tsx:142-152` hand-rolls a fourth chain, ending in a raw-enum fallback at `:152`.

Counting all of them — task, worker, agent-live (`AgentLiveCard.tsx:100`), subagent (`SubagentMonitor.tsx:18`),
plan (`PlanView.tsx:11-24`), plan-card (`PlanCard.tsx:10`), git (`GitPanel.tsx:414`), system-health
(`SystemHealth.tsx:71,84`) — **eight independent status vocabularies** exist in one app.

### 7.3 Formatting

| Dimension | Distinct implementations | Worst symptom |
|---|---|---|
| **Date / time** | **9** (`toLocaleString`, bare `toLocaleDateString`, two option-ed variants, `toLocaleTimeString`, raw ISO, ISO-split, five hand-rolled "x ago", plus `date-fns` used **exactly once** at `chat/SessionBar.tsx:84`) | Two "just now" thresholds disagree — `< 5 s` at `Dashboard.tsx:18-26` vs `< 10 s` at `Dashboard.tsx:990-999`, in the same file |
| **Elapsed** | 3 (`AutoDevPanel.tsx:50-55`, `MiniLogTail.tsx:16`, `StagePipeline.tsx:12-13`) | `MM:SS` vs `m Ss` side by side |
| **Cost** | **7** (2 dp, 3 dp, 4 dp, adaptive 6/4/2 duplicated across `Sidebar.tsx:108-111` and `TabBar.tsx:103-106`, and a single string mixing 4 dp with 2 dp at `BudgetBar.tsx:157`) | `$0.0042 / $5.00` in one label |
| **Tokens** | **5 copies** of the k/M abbreviator with **inconsistent case** — `k` at `TokenSparkline.tsx:13`, `Sidebar.tsx:15`, `StatusBar.tsx:22`; **`K`** at `BudgetBar.tsx:57`, `TokenUsage.tsx:29` | The same total reads `12.3k` in the sidebar and `12.3K` two panes away |
| **Bytes** | 5 copies with different units (`K`/`M` no space, `KB`/`MB`, `GB` at 2 dp) | `CheckpointViewer.tsx:119`, `CodeViewer.tsx:277`, `IdeStatusBar.tsx:46`, `SidecarPanel.tsx:74`, `UpdateSettings.tsx:16` |
| **Empty value** | `—` is the house convention, used as a bare literal in **12** places with no shared constant; competing forms include prose "no X", a lowercase `'unknown'` (`WorkerLeaderboard.tsx:27`) beside Title-Case peers, and `'error'` as a bare value (`AutoDevPanel.tsx:68`) | `WorkerTable.tsx:290` uses `— no recipe —` where the dash is decorative, giving the same glyph a third meaning |

No `'N/A'` and no bare `'-'` exist; nothing leaks a literal `undefined` into JSX **except** the two exhaustive-switch
gaps noted in §7.2.

### 7.4 Accessibility

**56 `aria-*` attributes across 28 of 118 `.tsx` files (24 %).** The breakdown:

| Attribute | Count | Note |
|---|---|---|
| `aria-label` | 38 | the only broadly-used one |
| `aria-hidden` | 6 | |
| `aria-live` | 3 | `ToastStack.tsx:68` (inert), `UpdateToast.tsx:39`, `StartupOverlay.tsx:48` |
| `aria-selected` | 2 | `AutoDevPanel.tsx:941`, `ide/TabBar.tsx:56` — **not** the main tab bar |
| `aria-valuenow/min/max` | 3 | all in `StartupOverlay.tsx:62-64` |
| `aria-modal` / `aria-labelledby` | 1 each | only `update/UpdateDialog.tsx:98-99` |
| `aria-expanded` | 1 | only `dashboard/DiffAnalysisCard.tsx:102` |
| `aria-describedby` | **0** | |
| `aria-pressed` | **0** | **no toggle button in the app reports its state** |

`role=` — 13 total. `role="tab"`/`tablist` exist on the AutoDev sub-tabs and the IDE tab strip, but **not on the
main application tab bar** (`components/layout/TabBar.tsx:39`), which a screen reader hears as nine unlabelled
buttons.

**Forms:** **0 `<label htmlFor>`** against **102 form controls** (70 `<input>`, 15 `<textarea>`, 17 `<select>`);
32 `<label>` elements exist but none is associated. Every control is labelled by visual proximity only.

**Buttons:** 336 `<button>` elements, 38 with `aria-label`. Roughly 50 rely on `title` alone (a tooltip, not an
accessible name), and 10–15 are pure glyph with neither — e.g. `plan/PlanView.tsx:58`,
`subagent/SubagentEventStream.tsx:45`, `workers/WorkerCreateModal.tsx:98`.

**Focus:** **no focus trap anywhere in `src/`** — no `focus-trap`, no tab-cycle handler, no `inert`. **No overlay
restores focus on close.** Of 14 modals/overlays, **10 ignore Escape**. `update/UpdateDialog.tsx` carries
`role="dialog" aria-modal="true"` (`:97-98`) and focuses its close button on open (`:68`) — and still lets Tab
walk into the page behind it and still ignores Escape. `chat/PermissionDialog.tsx` — the modal that gates whether
an agent may write to disk — has no trap, no Escape, and no close affordance.

**Colour-only status:** ~15 indicators, ~10 with no accompanying text at all — `workers/WorkerTable.tsx:128-145`
(6 colours, zero text), `layout/StatusBar.tsx:72-77` (the `core-orch` dot is the only health signal),
`layout/Sidebar.tsx:153-162`, `AutoDevPanel.tsx:197,442,633,1328` (bare `borderRadius:'50%'` spans),
`sidecar/McpHealthPanel.tsx:49-59`, `dashboard/EpicPhaseView.tsx:96,318`, `Graph.tsx:1181,1185`.
In the shared vocabulary at `styles/globals.css:103-108`, amber is documented as **both** "blocked" and
"in-progress", and `.dot-purple` ("QC review") resolves to the **same** `var(--info)` as `.dot-blue` — two
different states rendering identically.

**Motion:** **`prefers-reduced-motion` has zero occurrences** in all of `src/`, including all 14 CSS files.
Infinite animations that never degrade: `.anim-spin` (`globals.css:176`), `.anim-pulse` (`:178`),
`McpHealthPanel.tsx:57`, `LiveMonitor.css:76-87` (LIVE/STALE badges pulse continuously **while nothing is
happening**), `Graph.css:844`, `StartupOverlay.css:128`.

**Contrast:** the design system is dark-only and unconditional — `@media (prefers-color-scheme: dark)` and
`[data-theme]` have **zero occurrences** in `src/` (G5c §7.2), while `ui-anchor.md` mandates light-only. Two
conflicting dark palettes ship simultaneously (G5c F12), and ~22 custom properties are referenced but never
defined (G5c F17), so those elements render with **no colour at all**. Two concrete measured failures:
`AutoDevPanel.tsx:122` and `:222` set `color: '#333'` on `--bg-surface: #161622` (`globals.css:11`) — roughly
**1.5:1**, against a WCAG AA floor of 4.5:1.

**Other:** 0 `<img>` elements (all iconography is emoji or `lucide-react` SVG), so no alt-text gap.
48 `overflow-y: auto` scroll regions, none with `tabIndex={0}`. `Graph.tsx:1177-1178` is a
`role="button" tabIndex={0}` div with **no `onKeyDown`** — focusable but not activatable by keyboard.

---

## 8. Top 20 UX fixes, ranked by user impact per unit of effort

Effort: **S** ≈ under an hour · **M** ≈ half a day · **L** ≈ multi-day.
"Unblocks" names the findings each fix closes.

| # | Fix | Effort | Impact | Unblocks |
|---|---|---|---|---|
| 1 | **Make the toast system work.** Replace both `require('../stores/toastStore')` calls with `useToastStore.getState().addToast(...)`. One line each; it turns a mounted, styled, keyboard-dismissible notification channel from inert into usable — and is the prerequisite for fixes 2 and 3. | **S** | Very high | UX-19, UX-20 |
| 2 | **Stop clearing the composer before the send succeeds.** Move `setInput('')` (`Chat.tsx:280`) to after `invoke` resolves; restore the text on rejection. Ends silent data loss on the app's most-used control. | **S** | Very high | UX-05 |
| 3 | **Surface `send_message` rejections in the transcript.** Replace the `console.error` at `Chat.tsx:312-315` with an error state on the pending bubble carrying the message and a **Retry**. The single highest-value error fix in the app. | **S** | Very high | UX-07, UX-22 |
| 4 | **Verify the key before storing it.** In `handleSaveKey` (`Settings.tsx:1069`), call the already-registered `test_provider_connection` first; store and activate only on pass. Turns BYOK from "hope" into "confirmed". | **S** | Very high | UX-09, UX-13 |
| 5 | **Add a boot watchdog.** A 30 s timer in `startupStore.subscribe()` that flips `dismissed` and shows "startup did not complete". Removes the only total-lockout path in the product, and makes the docstring true. | **S** | Very high | UX-01 |
| 6 | **Confirm the six irreversible git/task/hook deletes** (D1–D6), starting with `GitPanel.tsx:208`. One shared confirm component; the highest safety-per-line change available. | **S** | Very high | UX-37…42, UX-43 |
| 7 | **Replace the `alert()` at `Chat.tsx:284` with an inline banner carrying a `setActiveTab('settings')` button** — and guard on `has_key`, not just `activeProvider`. Turns the first-run dead end into a one-click path. | **S** | High | UX-06, UX-07, UX-35 |
| 8 | **Show "No model — set up" instead of `null`** in `TabBar.tsx:82`, wired to the same navigation. Makes unconfigured state visible from every tab, permanently. | **S** | High | UX-03 |
| 9 | **Reorder Settings**: Providers · Workspace · Models to the top; the six infrastructure panels collapsed under "Advanced"; add a section nav rail. `Settings.tsx:39-110` is a flat list — this is reordering JSX. | **S** | High | UX-08 |
| 10 | **Persist active provider + model** (`AppState` serialization, G5c F01). Removes the "reconfigure every launch" tax and the `alert()` that greets every returning user. Rust-side, but small and mechanical. | **M** | Very high | UX-16 |
| 11 | **Add the missing key affordances**: a `Replace key` control, a masked fingerprint + verified-on date, a per-provider "Get a key" link, and price + context window on each model chip (the data is already in `ModelInfo`). This is what makes it a BYOK *product*. | **M** | Very high | UX-10, UX-11, UX-12, UX-14 |
| 12 | **Build the first-run Setup surface** — three steps (provider → model → workspace) rendered in place of tab content while `setupState !== 'ready'`, with a Skip that leaves a persistent banner. Depends on 4, 7, 8, 11. | **M** | Very high | UX-01…18 collectively |
| 13 | **Route the 48 unhandled store rejections into toasts**, `gitStore`'s 15 first. One `withErrorToast` wrapper applied per store method. | **M** | Very high | UX-21, UX-22 |
| 14 | **Fix `open_workspace_picker` → `pick_workspace_dir`** and `navigate_to_logs`, and stop swallowing both. Two dead CTAs the user is actively invited to press. | **S** | Medium | UX-17 |
| 15 | **One source of truth for task status** → column, label and colour, replacing the three divergent maps. Stops `cancelled` tasks from vanishing and stops the Dashboard printing raw enums. | **M** | High | UX-44, UX-45, UX-46 |
| 16 | **Extract shared formatters** — `formatCost`, `formatTokens`, `formatTime`, `formatAgo`, `formatBytes`, `EMPTY = '—'` — and delete the 26 duplicate implementations. Mechanical, and it is what makes the app feel like one product. | **M** | High | UX-47, UX-48 |
| 17 | **Add real empty states with actions** to the eight dead ends that currently say only "No X": worker roster (which today says "Loading agents…" forever in 1.5:1 grey), Workers table, kanban columns, Fleet Chat, MCP tools, subagents, IDE file tree, Logs. Model them on `Graph.tsx:963-970`, which is already right. | **M** | High | UX-29, UX-30, UX-31 |
| 18 | **One `Modal` primitive** with focus trap, Escape-to-close and focus restore, applied to all 14 overlays — starting with `PermissionDialog`, which gates disk writes. Plus `aria-pressed` on toggles and text beside every status dot. | **M** | High | UX-52, UX-53 |
| 19 | **Add cancellation to the graph build** (30–90 s, self-declared) and the update download; add progress to the Playwright install. Three long operations that today cannot be stopped or observed. | **M** | Medium | UX-25, UX-26, UX-27 |
| 20 | **Decide the fate of the ~3,700 orphaned LOC** (memory, plugins, plan) — route them or delete them. Then add the `prefers-reduced-motion` block, `role="tablist"` on the main tab bar, `<label htmlFor>` on 102 controls, and a shortcut cheatsheet. Housekeeping, but it is the difference between "mostly works" and "finished". | **L** | Medium | §5, UX-36, UX-50, UX-51, UX-55, UX-56 |

### The shortest path to "smooth"

Items **1–9 are all Small**, touch fewer than 15 files between them, and together close every P0 in the first-run
flow except persistence (10). They convert the current experience —

> *undismissable boot → blank Chat → typed message destroyed → native alert → hunt for Settings → scroll past
> seven infrastructure panels → paste an unverified key → silent failure*

— into:

> *boot with a timeout → Setup screen → verified key → confirmed model → first message works.*

Everything after item 12 is consistency and polish. Everything before it is the product working at all.

---

*End of report.*
