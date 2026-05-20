// workerChatStore — Frontend state for the AutoDev Communication Protocol.
//
// Subscribes to the Rust-side `worker:message` event and maintains a chronological
// log of every cross-worker message, plus a per-recipient unread count for the
// AgentLiveCard badge. On mount the store seeds itself from
// `list_recent_worker_messages` (Tauri command) and from
// `count_unread_worker_messages_for(slug)` per fleet worker.
//
// Why a Zustand store and not React state? The Fleet Chat panel, the badge on
// every AgentLiveCard, and the future Worker Activity Graph all need the same
// underlying data — keeping it in a single store avoids prop drilling and
// keeps live-event ingestion in one place.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// ─── Types (mirror Rust WorkerMessage struct) ────────────────────────────────

export type WorkerMessageKind = 'nudge' | 'question' | 'answer' | 'fyi' | 'broadcast'
export type WorkerMessageStatus = 'pending' | 'read' | 'acknowledged'

export interface WorkerMessage {
  id:          number
  from_slug:   string
  to_slug:     string
  body:        string
  kind:        WorkerMessageKind
  task_id:     number | null
  status:      WorkerMessageStatus
  created_at:  string
  read_at:     string | null
}

// Payload shape of the `worker:message` Tauri event (camelCase'd from Rust).
interface WorkerMessageEventPayload {
  message_id: number
  from:       string
  to:         string
  body:       string
  kind:       string
  task_id:    number | null
  timestamp:  string
}

// ─── Store shape ─────────────────────────────────────────────────────────────

interface WorkerChatState {
  // Newest-first chronological log of all received messages.
  messages:        WorkerMessage[]
  // Per-recipient unread counts keyed by worker slug.
  unreadByTarget:  Record<string, number>
  // Selected task id in the Fleet Chat panel (null = "all tasks" view).
  selectedTaskId:  number | null
  // Hydration state.
  initialized:     boolean

  // Actions.
  setSelectedTaskId: (id: number | null) => void
  /** Seed from the DB on mount and start listening for live events. */
  hydrate:           () => Promise<void>
  /** Refresh the unread badge counts (called after the user clears messages). */
  refreshUnread:     (slugs: string[]) => Promise<void>
  /** Re-pull a task's full transcript on demand (Fleet Chat task-drill-down). */
  loadMessagesForTask: (taskId: number) => Promise<WorkerMessage[]>
}

// Track the global listener so we don't double-subscribe on hot reload.
let _unlistenFn: UnlistenFn | null = null

export const useWorkerChatStore = create<WorkerChatState>((set, get) => ({
  messages:       [],
  unreadByTarget: {},
  selectedTaskId: null,
  initialized:    false,

  setSelectedTaskId: (id) => set({ selectedTaskId: id }),

  hydrate: async () => {
    if (get().initialized) return

    // Seed recent messages from SQLite.
    try {
      const recent = await invoke<WorkerMessage[]>('list_recent_worker_messages', { limit: 200 })
      // Sort oldest-first for chat-style rendering.
      const sorted = [...recent].sort((a, b) => a.created_at.localeCompare(b.created_at))
      set({ messages: sorted })
    } catch (err) {
      console.error('[workerChatStore] hydrate: list_recent_worker_messages failed:', err)
    }

    // Seed unread counts for the known fleet slugs. These match
    // FLEET_WORKERS in defaults.rs plus the supervisor / qc agents.
    await get().refreshUnread([
      'supervisor', 'qc-agent', 'qc-dispatcher',
      'worker-backend', 'worker-frontend', 'worker-platform',
      'worker-mcp', 'worker-cleanup',
    ])

    // Subscribe to the live event stream.
    if (_unlistenFn) {
      _unlistenFn()
      _unlistenFn = null
    }
    _unlistenFn = await listen<WorkerMessageEventPayload>('worker:message', (event) => {
      const p = event.payload
      const msg: WorkerMessage = {
        id:         p.message_id,
        from_slug:  p.from,
        to_slug:    p.to,
        body:       p.body,
        kind:       (p.kind as WorkerMessageKind) ?? 'nudge',
        task_id:    p.task_id,
        status:     'pending',
        created_at: p.timestamp,
        read_at:    null,
      }
      set((state) => {
        const next = [...state.messages]
        // Insert maintaining chronological order (newest at end).
        next.push(msg)
        const unread = { ...state.unreadByTarget }
        unread[p.to] = (unread[p.to] ?? 0) + 1
        return { messages: next, unreadByTarget: unread }
      })
    })

    set({ initialized: true })
  },

  refreshUnread: async (slugs) => {
    const next: Record<string, number> = {}
    for (const slug of slugs) {
      try {
        const count = await invoke<number>('count_unread_worker_messages_for', { slug })
        next[slug] = count
      } catch (err) {
        console.warn(`[workerChatStore] refreshUnread('${slug}') failed:`, err)
        next[slug] = 0
      }
    }
    set({ unreadByTarget: next })
  },

  loadMessagesForTask: async (taskId) => {
    try {
      const msgs = await invoke<WorkerMessage[]>('list_worker_messages_for_task', {
        taskId, limit: 500,
      })
      return msgs
    } catch (err) {
      console.error(`[workerChatStore] loadMessagesForTask(${taskId}) failed:`, err)
      return []
    }
  },
}))

// Convenience selectors.
export const selectMessagesForTask = (taskId: number | null) => (state: WorkerChatState) => {
  if (taskId === null) return state.messages
  return state.messages.filter((m) => m.task_id === taskId)
}

export const selectUnreadFor = (slug: string) => (state: WorkerChatState) => {
  return state.unreadByTarget[slug] ?? 0
}
