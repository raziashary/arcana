import { useState, useEffect, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'

interface OrphansReleasedPayload {
  task_ids: string[]
  count: number
}

interface AlertEntry {
  id: number
  taskIds: string[]
  at: string
}

export function OrphanAlert() {
  const [alerts, setAlerts] = useState<AlertEntry[]>([])

  const dismiss = useCallback((id: number) => {
    setAlerts(prev => prev.filter(a => a.id !== id))
  }, [])

  useEffect(() => {
    const unlisten = listen<OrphansReleasedPayload>('task:orphans-released', event => {
      const { task_ids } = event.payload
      if (!task_ids?.length) return
      const entry: AlertEntry = {
        id:      Date.now(),
        taskIds: task_ids,
        at:      new Date().toISOString(),
      }
      setAlerts(prev => [...prev, entry])
      // Auto-dismiss after 10 seconds
      setTimeout(() => dismiss(entry.id), 10_000)
    })
    return () => { unlisten.then(f => f()) }
  }, [dismiss])

  if (!alerts.length) return null

  return (
    <div style={{ margin: '0 12px 6px', display: 'flex', flexDirection: 'column', gap: 4 }}>
      {alerts.map(alert => (
        <div
          key={alert.id}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            padding: '6px 10px',
            background: 'var(--bg-raised)',
            border: '1px solid var(--yellow, #f5a623)',
            borderRadius: 6,
            fontSize: 12,
          }}
        >
          <span style={{ fontSize: 14 }}>⚠️</span>
          <span style={{ flex: 1, color: 'var(--text-1)' }}>
            <strong>{alert.taskIds.length} task{alert.taskIds.length > 1 ? 's' : ''}</strong>
            {' '}released — worker process exited unexpectedly.
            {' '}Tasks returned to Ready queue.{' '}
            <span style={{ color: 'var(--text-3)', fontSize: 11 }}>
              IDs: {alert.taskIds.join(', ')}
            </span>
          </span>
          <button
            onClick={() => dismiss(alert.id)}
            style={{
              fontSize: 10, padding: '1px 6px',
              border: '1px solid var(--border-1)', borderRadius: 3,
              background: 'transparent', color: 'var(--text-3)', cursor: 'pointer',
            }}
          >
            Dismiss
          </button>
        </div>
      ))}
    </div>
  )
}
