# 03 — Worker Requests "REPORT ISSUE" dropdown does nothing

## Symptom
The Worker Requests panel shows a row like:
`REPORT ISSUE worker-backend #8 ▸`
Clicking the row chevron visually toggles but no payload / actions appear.
Per the user, "clicking the dropdown does nothing".

## Where the widget lives
Inline in `AutoDevPanel.tsx` as `WorkerRequestsWidget`
([`AutoDevPanel.tsx:565-714`](../../../src/tabs/AutoDevPanel.tsx)). There is
no separate `WorkerRequestsWidget.tsx` file — it's defined inline.

## The interaction model — there are TWO chevrons
The widget tracks a single `expandedId: number | null` state. It uses
sentinel `-1` to mean "the outer panel is expanded" and any positive number
to mean "request with that id is open".

```ts
const [expandedId, setExpandedId] = useState<number | null>(null)
```

### Outer chevron — panel header
[`AutoDevPanel.tsx:599-621`](../../../src/tabs/AutoDevPanel.tsx):

```tsx
onClick={() => setExpandedId(expandedId === -1 ? null : -1)}
```

So clicking the header toggles between `null` (collapsed) and `-1`
(expanded). The list of requests only renders when `expandedId === -1`.

### Inner chevron — per request item
[`AutoDevPanel.tsx:646-664`](../../../src/tabs/AutoDevPanel.tsx):

```tsx
onClick={() => setExpandedId(isOpen ? -1 : req.id)}
```

Where `isOpen = expandedId === req.id`.

## The bug — opening an item collapses the list, hiding the expanded item
Walk the state machine:

1. Click panel chevron → `expandedId = -1` → list renders, all rows
   collapsed.
2. Click request #8 chevron → `expandedId = 8`.
3. Render pass: outer guard `expandedId === -1` is now **false**, so the
   whole `<div>` containing the request list (and request #8's expanded
   body) is unmounted entirely
   ([`AutoDevPanel.tsx:623`](../../../src/tabs/AutoDevPanel.tsx)).
4. The user sees the panel snap shut. To them it looks like "clicking the
   dropdown does nothing" — actually it briefly collapses the whole
   widget.

The state values `-1` and `req.id > 0` are mutually exclusive, so a single
`expandedId` variable cannot model "panel open AND row N open" at the same
time. The outer render guard incorrectly hides the list whenever a row is
opened.

## Confirming the data path is fine
- `loadWorkerRequests('pending')` runs on mount and on each `doRefresh`
  ([`AutoDevPanel.tsx:763`](../../../src/tabs/AutoDevPanel.tsx)) — req #8
  is present in the array.
- `payload` is already a JSON string carried inline in the
  `WorkerRequest` row
  ([`taskStore.ts:77-83`](../../../src/stores/taskStore.ts)) — no lazy
  Tauri fetch needed. So the secondary hypothesis (missing
  `get_worker_request_payload` command) is **not** the issue.
- `payloadPreview` is computed locally at
  [`AutoDevPanel.tsx:627-634`](../../../src/tabs/AutoDevPanel.tsx) and
  would render correctly if the surrounding div were not unmounted.

## Log evidence
`REPORT_ISSUE` request for task #8 exists in the backend (the user's
report cites the visible chip). The log shows task #8 itself being
reconciled to qc_review at
[line 491](../../../logs/autodev-.2026-05-14..log):

```
2026-05-14T11:19:53.504Z  [RecipeRunner] Reconciled stranded in_progress
  task #8 → qc_review (worker 'worker-backend' session ended without
  explicit completion)
```

The request payload itself isn't logged (it's a tool-call result inside the
LivingAgent's stream, not a tracing event) but its existence is consistent
with the UI showing the chip.

## Suggested fix (no code change in this audit)
Use **two independent state slots** instead of overloading one:

- `panelOpen: boolean` — controls the outer guard.
- `openRequestId: number | null` — controls which inner row is expanded
  (allow zero or one open at a time, or convert to a `Set<number>` for
  multi-open).

Then:

- Outer `<div>` guard: `panelOpen && …`
- Inner item: `isOpen = openRequestId === req.id`

This makes the two interactions orthogonal and the bug disappears.
Alternatively, change the outer guard to
`(expandedId === -1 || expandedId !== null && expandedId > 0)` so any
opened row also keeps the panel rendered — a one-character fix but uglier
to reason about.

---

## Atomic implementation tasks

### Phase 1: State split

- [ ] Replace `expandedId` with two independent state slots — `src/tabs/AutoDevPanel.tsx:565-714` (inline `WorkerRequestsWidget`) — declare `const [panelOpen, setPanelOpen] = useState(false)` and `const [openRequestId, setOpenRequestId] = useState<number | null>(null)`; remove the `expandedId` declaration entirely.
- [ ] Rewire the outer header chevron — `src/tabs/AutoDevPanel.tsx:599-621` — change `onClick` to `() => setPanelOpen(o => !o)` and the visual chevron condition to use `panelOpen`.
- [ ] Rewire the outer list render guard — `src/tabs/AutoDevPanel.tsx:623` — change the guard from `expandedId === -1 && …` to `panelOpen && …` so opening a row no longer unmounts the list.
- [ ] Rewire the per-row chevron — `src/tabs/AutoDevPanel.tsx:646-664` — change `onClick` to `() => setOpenRequestId(isOpen ? null : req.id)` and update `const isOpen = openRequestId === req.id`.

### Phase 2: Show payload JSON when row expanded

- [ ] Render the `payload` JSON in a `<pre>` block — `src/tabs/AutoDevPanel.tsx` (inside the per-request item, where `isOpen` is true) — parse `req.payload` with a try/catch (it is a JSON string per `src/stores/taskStore.ts:77-83`), pretty-print with `JSON.stringify(parsed, null, 2)`, and render in a monospaced `<pre className="text-xs whitespace-pre-wrap">`. Fall back to raw string on parse error.
- [ ] Hide the locally computed `payloadPreview` when the row is open — `src/tabs/AutoDevPanel.tsx:627-634` — only show the truncated preview when `!isOpen`; full payload renders below.

### Phase 3: Add resolve actions

- [ ] Add "Acknowledge" / "Reject" buttons — `src/tabs/AutoDevPanel.tsx` (inside the per-request expanded body) — render two buttons that call `invoke('resolve_worker_request', { id: req.id, resolution: 'ack' | 'reject' })`, then trigger `loadWorkerRequests('pending')` to refresh.
- [ ] Confirm or add the Tauri command — `src-tauri/src/commands/` (read-only: locate `resolve_worker_request`; if missing, file a follow-up task to expose it) — verify the command signature matches `{ id: i64, resolution: String }` and returns `Result<(), String>`.
- [ ] Surface action errors to the user — `src/tabs/AutoDevPanel.tsx` — show a small inline error string under the buttons on `invoke` failure; do not collapse the row on success — instead remove the item from the local list or refresh in place.
