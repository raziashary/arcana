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

### Phase 1: State split — ✅ COMPLETE 14 May 2026

- [x] Replaced `expandedId` with two independent state slots in `WorkerRequestsWidget` — `src/tabs/AutoDevPanel.tsx`. `panelOpen: boolean` + `openRequestId: number | null`.
- [x] Outer header chevron now toggles `panelOpen` (`setPanelOpen(o => !o)`) and the chevron icon condition uses `panelOpen`.
- [x] Outer list render guard now reads `panelOpen && (…)`, so clicking a row no longer unmounts the list.
- [x] Per-row chevron toggles `openRequestId` (`setOpenRequestId(isOpen ? null : req.id)`), with `isOpen = openRequestId === req.id`.

### Phase 2: Show payload JSON when row expanded — ✅ COMPLETE 14 May 2026

- [x] Parsed payload JSON renders in a monospace `<pre>` when expanded — `src/tabs/AutoDevPanel.tsx`. `try { parsed = JSON.parse(req.payload) } catch { parseError = true }`, pretty-printed via `JSON.stringify(parsed, null, 2)`, raw string fallback on parse error. `max-height: 240px` with overflow scroll so huge payloads don't blow up the layout.
- [x] Single-line preview moved into the collapsed-row header — `src/tabs/AutoDevPanel.tsx`. Hidden when row is open (full JSON renders below); shown as `— <preview>` with `title` attribute for the full text on hover.

### Phase 3: Add resolve actions — ✅ COMPLETE 14 May 2026

- [x] Added "Acknowledge" / "Reject" buttons — `src/tabs/AutoDevPanel.tsx`. Each calls `resolveWorkerRequest(id, 'completed' | 'rejected', note?)` which invokes the existing Tauri command via `useTaskStore`. Per-row `pendingId` state — button shows `Acknowledging…` and is disabled during the round-trip.
- [x] Confirmed the Tauri command exists — `src-tauri/src/commands/task_commands.rs:172` defines `resolve_worker_request(id: i64, action: String, note: Option<String>) -> Result<()>`. Signature matches the frontend invocation in `src/stores/taskStore.ts:197-199`. No new command needed.
- [x] Inline error UX on resolve failure — `src/tabs/AutoDevPanel.tsx`. On `invoke` failure the row stays open; a `Failed: <message>` line appears under the buttons in danger color. Users can retry without losing context. Successful resolve refreshes the list (handled by the store's `resolveWorkerRequest` re-querying).
