// UpdateToast — non-intrusive banner that appears when a new version is available.
// Phase 5.3 — Auto-updater UI.
// Shown at the bottom of the app when the background checker finds a newer release.
// Critical updates use an orange/red variant and cannot be dismissed without acting.

import { useUpdateStore } from '@/stores/updateStore'

export function UpdateToast() {
  const {
    availableUpdate,
    isCritical,
    isToastVisible,
    dismissToast,
    openDialog,
    skipVersion,
  } = useUpdateStore()

  if (!isToastVisible || !availableUpdate) return null

  const handleUpdate = () => {
    openDialog()
    dismissToast()
  }

  const handleSkip = async () => {
    await skipVersion(availableUpdate.version)
  }

  return (
    <div
      className={`
        fixed bottom-4 right-4 z-50 flex items-center gap-3 rounded-lg px-4 py-3 shadow-lg
        border text-sm max-w-sm animate-in slide-in-from-bottom-2
        ${isCritical
          ? 'bg-orange-950 border-orange-700 text-orange-100'
          : 'bg-gray-900 border-gray-700 text-gray-100'}
      `}
      role="alert"
      aria-live="polite"
    >
      {/* Icon */}
      <span className="text-xl flex-shrink-0" aria-hidden>
        {isCritical ? '🔴' : '🔵'}
      </span>

      {/* Text */}
      <div className="flex-1 min-w-0">
        <p className="font-semibold leading-tight">
          {isCritical ? 'Critical update available' : 'Update available'}
        </p>
        <p className="text-xs opacity-75 mt-0.5 truncate">
          Arcana {availableUpdate.version} is ready to install
        </p>
      </div>

      {/* Actions */}
      <div className="flex gap-2 flex-shrink-0">
        {!isCritical && (
          <button
            onClick={handleSkip}
            className="text-xs opacity-60 hover:opacity-100 transition-opacity px-1 py-0.5"
            aria-label="Skip this update"
          >
            Skip
          </button>
        )}
        <button
          onClick={handleUpdate}
          className={`
            text-xs font-medium px-3 py-1.5 rounded-md transition-colors
            ${isCritical
              ? 'bg-orange-600 hover:bg-orange-500 text-white'
              : 'bg-blue-600 hover:bg-blue-500 text-white'}
          `}
        >
          Update
        </button>
        {!isCritical && (
          <button
            onClick={dismissToast}
            className="text-xs opacity-50 hover:opacity-80 transition-opacity px-1"
            aria-label="Dismiss update notification"
          >
            ✕
          </button>
        )}
      </div>
    </div>
  )
}
