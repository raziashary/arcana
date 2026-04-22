import { useEffect, useCallback } from 'react'
import { useToastStore, type Toast } from '../stores/toastStore'

const TYPE_STYLES: Record<Toast['type'], { bg: string; border: string; icon: string }> = {
  info:    { bg: 'bg-[#00233B]', border: 'border-[#82AAFF]', icon: 'ℹ️' },
  warning: { bg: 'bg-[#00233B]', border: 'border-[#FFCB8B]', icon: '⚠️' },
  error:   { bg: 'bg-[#00233B]', border: 'border-[#FF6363]', icon: '❌' },
  success: { bg: 'bg-[#00233B]', border: 'border-[#ADDB67]', icon: '✅' },
}

function ToastItem({ toast }: { toast: Toast }) {
  const removeToast = useToastStore((s) => s.removeToast)
  const style = TYPE_STYLES[toast.type]

  useEffect(() => {
    const timer = setTimeout(() => removeToast(toast.id), toast.durationMs)
    return () => clearTimeout(timer)
  }, [toast.id, toast.durationMs, removeToast])

  return (
    <div
      className={`pointer-events-auto flex max-w-xs items-start gap-3 rounded-lg border-l-4 p-3 shadow-lg backdrop-blur-sm transition-all ${style.bg} ${style.border}`}
      role="alert"
    >
      <span className="mt-0.5 text-sm" aria-hidden="true">{style.icon}</span>
      <div className="flex-1">
        <p className="text-sm font-semibold text-[#D6DEEB]">{toast.title}</p>
        <p className="text-xs text-[#637777]">{toast.message}</p>
        {toast.workerSlug && (
          <p className="mt-1 text-[10px] font-mono uppercase tracking-wider text-[#C792EA]">
            {toast.workerSlug}
          </p>
        )}
      </div>
      <button
        onClick={() => removeToast(toast.id)}
        className="text-xs text-[#637777] hover:text-[#D6DEEB]"
        aria-label="Dismiss toast"
      >
        ✕
      </button>
    </div>
  )
}

export default function ToastStack() {
  const toasts = useToastStore((s) => s.toasts)

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        useToastStore.getState().clearAll()
      }
    },
    []
  )

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  if (toasts.length === 0) return null

  return (
    <div
      className="fixed right-4 top-4 z-[9999] flex flex-col gap-2"
      aria-live="polite"
      aria-atomic="false"
    >
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} />
      ))}
    </div>
  )
}
