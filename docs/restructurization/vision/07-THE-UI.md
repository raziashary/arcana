# 07 — The UI: design system, surfaces, and the experience

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1B (M-20…M-25, W6: M-35…M-39), §Part 3.7, §Part 5 (S1) |
| **Locks** | Hands **ADR-01-10**, **ADR-03-03**, **ADR-04-06** to `00 §Part 8` |
| **Status** | PLANNED |
| **Grounding** | `docs/restructurization/ui-anchor.md` (**DOCTRINE**) · `_grounding/G5a` (Chat) · `G5b` (Graph City) · `G5c` (Settings, stores, contract drift) · `G11` (UX, 688 ln) |

---

## What this is

39,474 LOC of React across 9 tabs, 20 component directories and 28 stores — and the layer where every backend defect finally becomes something a person experiences. It is also the workstream with the widest gap between what exists and what is specified: the design anchor is essentially **unimplemented**, not partially implemented.

**The subtle failures this subsystem is prone to:**

1. **A notification system that has never notified.** The store and the renderer are both correct; the only two *producers* in the app are broken, so no toast has ever been shown. Verified first-hand — see §7.3.
2. **A subscription that outlives nothing.** Listeners registered inside a send handler, on a conditionally-rendered tab, die on a tab switch — and the backend keeps running.
3. **Two design systems on one element.** Tailwind for layout, inline `var(--…)` for colour, with **two conflicting dark palettes** — while the anchor mandates light-only.
4. **Confirmation on the reversible action and none on the destructive one.** *Delete checkpoint* is unconfirmed; *Restore checkpoint* beside it is confirmed.

---

## 1. The design system (M-20, ADR-01-10)

### 1.1 The distance to the anchor

Not a drift — an absence.

| Anchor requirement | Shipped |
|---|---|
| **Everything 7px** | `7px` appears **zero** times; 12+ distinct radii in use |
| Urbanist / Manrope / Roboto Mono | **Zero occurrences.** IBM Plex ships instead |
| `.airs-*` class vocabulary (34 classes) | **1 exists**, in a dead file |
| Light mode only | **Two conflicting dark palettes** — `globals.css` "Industrial Dark" and `tailwind.config.js` "Winter is Coming", same keys, different hex |
| Defined tokens | **~22 CSS custom properties referenced and never defined** — `--surface-bg` (correct name `--bg-surface`), `--accent-color` (`--accent`), `--text-tertiary` (`--text-3`), `--error-color`. Every rich tool-output panel and the whole session sidebar therefore render with a transparent background |

Plus invalid CSS: `"var(--accent-color)20"` — a hex-alpha suffix concatenated onto a `var()` produces an invalid value and the browser drops the whole property.

And `prose prose-invert` are **no-op classes**: `@tailwindcss/typography` is not installed, so GFM tables render as run-together text.

### 1.2 The Tailwind decision

**Tailwind is used — for layout only.** Colour goes through a parallel inline `var(--…)` system. That split is the mechanism behind the two-palette conflict: a component using Tailwind colour utilities and `var(--…)` inline styles gets both themes on one element.

ADR-01-10 chooses between removing Tailwind and reconfiguring its theme to *be* the anchor tokens. Given the measured usage (layout-only), **(B) — keep Tailwind, make its theme the anchor, ban raw colour `var()` inline** is the smaller change and preserves the layout work. Either way, **both dark palettes are deleted**, not adapted.

---

## 2. Event subscription lifetime (M-21)

**The single mechanism behind both BUG-CHAT and Live Monitor staleness.**

`setupListeners` is called from exactly one place — inside `handleSend`. `App.tsx:106` renders `{activeTab === 'chat' && <ChatTab/>}`, so switching tabs **unmounts** Chat and unsubscribes everything. Tokens, tool events and permission requests emitted while the user is elsewhere are lost permanently, with no backfill on return. The Live Monitor has the same shape: 11 listeners installed on `LiveMonitorSection` mount, torn down when `AutoDevPanel` unmounts.

The consequences compound: the Rust agent keeps running orphaned; a permission request whose listener died blocks the engine forever on a oneshot with no timeout; and a leak/race in the teardown (`Chat.tsx:293-294`) can orphan a listener set into a dead component, so later sends **append tokens twice**.

**The rule: subscription lifetime belongs to the session, above the tab boundary.** Tabs render state; they do not own transport. Remount backfills from persisted state.

---

## 3. Contract reconciliation (M-22)

Measured drift between 211 unique `invoke` names / 49 `listen` names and 256 registered commands / 42 Rust events:

| Direction | Count | Notable |
|---|---|---|
| Frontend → missing command | **2** | `navigate_to_logs`, `open_workspace_picker` — both swallowed by `.catch(()=>{})` |
| Registered → never called | **47 (18%)** | dead surface |
| Listener → no emitter | **10** | 5 `agent:*` (kills log lines, the pipeline strip, the token sparkline), 5 `plan:*` |
| Emitter → no listener | **3** | **`plan:changed` has 16 emit sites and no listener** — the plan UI never live-updates. Also `agent:thinking` (reasoning streamed into a black hole) and `ide:navigate` (so "open in IDE" does nothing) |

**The gate is a build-time check** that every `invoke`/`listen` name resolves in both directions. This class cannot be maintained by review.

---

## 4. Settings (M-23)

**Every Settings control invokes a command that exists.** The breakage is three other things:

1. **No persistence.** `set_active_provider` and `set_system_prompt` write only to `AppState`, which has no load/save and is rebuilt empty each launch. The model choice and custom prompt **vanish on restart**, and the first message then errors.
2. **Writes with no reader.** "Default Worker Budget" persists 4 values to `runtime_config.json` that **nothing in Rust reads**. 5 of 13 shipped hook templates target hook types that are **never fired** — so "Block force push" and "Block .env commits" install, show ENABLED, and **protect nothing**.
3. **Unwired controls.** Browser Testing's Staging URL and Enable toggle are pure `useState`. `UpdateSettings.tsx:9` renders the literal `__APP_VERSION__` (Vite's `define` does not substitute inside string literals). `runtimeConfigStore` swallows errors, so a failed save shows a green "Saved!".

**The rule: every control persists, hydrates, and surfaces its own failure.** A control that cannot do all three is deleted, not shipped.

---

## 5. Graph City (M-24)

Six ranked root causes (`G5b`). The headline is not three.js:

**CSP blocks the fonts.** drei's `<Text>` calls `preloadFont`, troika falls back to a jsdelivr CDN, and both `tauri.conf.json:28` and `index.html:8` block it in `connect-src` (`font-src` does not cover `fetch()`). The promise never settles, so **every `<Suspense>` boundary hangs forever** — and because `DistrictMesh` and `GodNodeBuilding` are inside them, **no district planes, no boundary walls, no landmark towers ever render.** Fix: self-host the font.

**The tab never auto-loads.** A `[]`-dep effect calls `loadGraphStats()` with a `workspaceDir` captured as `""`, the backend rejects it, and the throw is swallowed — so the City button is disabled on every visit until a 30–90 s rebuild. And the tab unmounts on every tab switch, so it repeats every time.

Then: a flat skyline (`line_start`/`line_end` never sent), community `-1` producing undefined colours, and per-mousemove geometry churn that leaks GPU memory until context loss. Contract details in spec `05 §3`.

---

## 6. Reachability (M-25, ADR-03-03)

**~3,700 LOC of built UI is unreachable from any route**, independently confirmed by two audits:

`PluginManager` 487 · `ITDashboard` 439 · `MemorySidebar` 408 · `SystemHealth` 329 · `PlanView` 180 · `PlanDisplay` ~130 · `PlanCard` 91 · `PlanTreeView` 71 · `PaModeToggle` ~60 · `WorkerSetupWizard` and `MemoryStats` transitively · plus 733 lines of CSS and **3 entire stores**.

**Memory, plugin and plan are entirely unreachable.** Each is a decision — route it or delete it — and plugin couples to ADR-03-03, which may remove the subsystem anyway.

---

## 7. The experience (W6, ADR-04-06)

### 7.1 First run — 10 steps, 6 undiscoverable

Under BYOK an unconfigured app does **nothing**, so this is the product's front door.

| # | Friction |
|---|---|
| 1 | **The boot overlay can lock the user out permanently.** `startupStore.ts:80-82` promises a watchdog that **does not exist**; if `ready` never fires there is no escape. It advertises "~15s"; the real worst case is ~113 s |
| 2 | **Boot has no failure state** — phases turn green for services that never started |
| 3 | **Typing a first message destroys it** — `setInput('')` runs before every guard |
| 4 | The only key prompt is a native `alert()` that names Settings but cannot open it |
| 5 | **The primary BYOK failure is fully silent** — the guard checks `activeProvider`, never `has_key`, so a missing or invalid key yields an empty bubble and no error |
| 6 | Providers is the **8th of 14** flat Settings sections, below six infrastructure panels |
| 7 | **The key is never validated on save** — the Test button only appears afterwards |
| 8 | **No key-replace path** — rotating an expired key requires deleting it first |
| 9 | Both status pills `return null` when unconfigured — **empty and healthy look identical** |
| 10 | **Zero "get a key" links** anywhere |

### 7.2 The BYOK flow that should exist (M-35)

```
Launch (no config)
  └─ Setup, 3 steps, skippable only where truly optional
     1. Workspace     — pick a folder; explain what agents will do in it
     2. Provider      — the four, each with a "get a key" link
                        key entry → VALIDATED against the provider → keychain
                        on failure: the provider's actual reason, and a retry
     3. First turn    — a suggested prompt that demonstrably works
Thereafter
  • header pill shows configured providers and the active model
  • an unconfigured app never presents a chat box that silently fails
```

**Validation on save, not on first use** (spec `02 §6`) — otherwise the failure surfaces mid-turn, where it is least interpretable.

### 7.3 The error contract (M-36)

Measured: **88 `console.*`-only sites** (72 of them catch blocks whose entire body is `console.error`) · 8 native `alert()` · 5 true no-op `.catch(()=>{})` · 48 unhandled store rejections (15 in `gitStore` alone) · **3 retry affordances in 35,000 LOC**.

**And no toast has ever been shown.** Verified directly (`CODE-PRESENT`), because the precise shape determines the fix:

- `stores/toastStore.ts:27` exports `useToastStore`, a Zustand store whose state carries `addToast`, `removeToast`, `clearAll`. **The store is correct.**
- `components/ToastStack.tsx` subscribes properly (`:12`, `:47`) and renders. **The renderer is correct.**
- The app's only two producers — `stores/workerStore.ts:283` and `:324` — do `const { addToast } = require('../stores/toastStore')`. That is wrong twice over: `addToast` is a **store method, not a module export**, so the destructure yields `undefined`; and `require` is not available in Vite's browser ESM at all.

So this is **not "build a toast system"** — it is a two-call-site import fix against machinery that already works. It also means `qc:decision` notifications (`workerStore.ts:322-340`) have never fired once.

**Fix it first: it is the smallest change in this spec and it unblocks every other error-surfacing improvement**, since the alternative to a toast is currently `console.error`.

The contract: every failure reaching a user surfaces **what failed, why, and what to do next**; anything retryable offers retry; nothing is `console.error`-only; no `alert()`.

### 7.4 Progress and cancellation (M-37)

Real progress from real events; cancellation wherever the backend supports it; empty states that say what to do next rather than presenting a dead end. *(Note: the AutoDev shimmer bar is a genuine sweep animation, not fake progress — `globals.css:161-164`.)*

### 7.5 Destructive actions (M-38)

**10 with no confirmation, six irreversible:** git **Discard** · **Drop stash** (invisible until hover) · **Delete checkpoint** · **Delete branch** · **Delete task** · **Remove hook** · abort subagent · delete annotation · delete model config · delete AUTODEV.md.

The sharpest tell: **Delete checkpoint is unconfirmed while the reversible Restore beside it is confirmed.** Three confirmation idioms coexist, and a 3-second auto-revert guards "Clear all tasks" — the least destructive of the set.

**The rule: confirmation is proportional to blast radius, in one idiom.** Undo where feasible — and undo **does** ship: `git/checkpoint.rs`, five registered commands, called from `gitStore.ts`, with a snapshot taken before every agent write. ⚠ *An earlier version of this line said undo did not exist because `db/checkpoints.rs` is not compiled. That was false (`ADR-LOG-RESTRUCT-0010`), and the file was **deleted** in P1 rather than declared (`ADR-0012`). A first correction was inserted mid-sentence and left the false claim as the conclusion — found by the post-phase doc audit (`D-38`) and rewritten here.* The real remaining defect is `M-43`: `auto_checkpoint_before_write` returns `()` and logs at `debug!`, so a **failed** checkpoint is invisible and the write proceeds anyway.

### 7.6 Coherence (M-39)

One vocabulary across tabs (task/issue/work-item; worker/agent/fleet). Consistent status rendering, dates, costs and empty values. And the app must **explain its own model** — PA modes, Supervisor/Worker/QC, recipes, skills, hooks — somewhere in-product. Today it explains none of them.

---

## 8. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-ANCHOR** | Radii ∈ {7px, 2px, pill-on-status-dot}; fonts are the anchor's three; every referenced CSS var is defined; **no dark palette exists** | The entire §1.1 table |
| **G-CSS-VALID** | Every computed style resolves; no invalid declarations | `"var(--accent-color)20"` |
| **G-EVENT-LIFETIME** | Start a turn, switch tabs, return — no event is lost and the transcript is complete | BUG-CHAT, Live Monitor staleness |
| **G-CONTRACT** | Build-time: every `invoke`/`listen` name resolves both directions | 2 missing, 47 dead, 10 dead listeners, 3 dead events |
| **G-SETTINGS-PERSIST** | Every setting survives a restart and is applied | The `AppState` rebuild |
| **G-NO-WRITEONLY-UI** | Every control's value has a reader | Budget config, the 5 dead hook templates |
| **G-TOAST** | An induced failure renders a visible, actionable toast | The never-rendering toast system |
| **G-FIRSTRUN** | From a clean profile: launch → key → workspace → a successful turn, **with no documentation** | The 10-step first run — **this is the acceptance test of `00 §7.2`** |
| **G-BYOK-VISIBLE** | With no key, and with an invalid key, the user sees a specific actionable error — never an empty bubble | The silent primary failure |
| **G-BOOT-ESCAPE** | If startup never completes, the user reaches a usable state with a diagnosis | The missing watchdog |
| **G-DESTRUCTIVE** | Every destructive control confirms, in one idiom | The 10, and the reversible/irreversible inversion |
| **G-REACHABLE-UI** | Every shipped component is reachable from a route | The ~3,700 orphaned LOC |
| **G-CITY** | With a built graph, the city renders districts, walls and towers offline | The CSP font hang |
| **G-A11Y** | Focus management, keyboard nav, no colour-only status, contrast under the light palette, `prefers-reduced-motion` | The anchor's a11y block |

---

> No ADR decision is recorded in this spec. It hands **ADR-01-10**, **ADR-03-03** and **ADR-04-06** to `00 §Part 8`.
