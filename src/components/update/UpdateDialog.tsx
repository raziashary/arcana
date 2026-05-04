// UpdateDialog — full-screen modal for reviewing and installing updates.
// Phase 5.3 — Auto-updater UI.
// Shows release notes, download progress bar, verification status, and install button.

import { useEffect, useRef } from 'react'
import { useUpdateStore, type DownloadStage } from '@/stores/updateStore'

// ─── Helpers ─────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`
}

function formatSpeed(bps: number): string {
  return formatBytes(bps) + '/s'
}

function stageLabel(stage: DownloadStage): string {
  switch (stage) {
    case 'Pending':     return 'Preparing download…'
    case 'Downloading': return 'Downloading…'
    case 'Verifying':   return 'Verifying signature…'
    case 'Ready':       return 'Ready to install'
    case 'Failed':      return 'Download failed'
    default:            return stage
  }
}

function stageColor(stage: DownloadStage): string {
  switch (stage) {
    case 'Ready':   return 'bg-green-500'
    case 'Failed':  return 'bg-red-500'
    default:        return 'bg-blue-500'
  }
}

// ─── Component ───────────────────────────────────────────────────────────────

export function UpdateDialog() {
  const {
    availableUpdate,
    isCritical,
    isDialogOpen,
    closeDialog,

    downloadProgress,
    isDownloading,
    downloadError,
    startDownload,

    isInstalling,
    installResult,
    installError,
    installUpdate,

    skipVersion,
  } = useUpdateStore()

  const closeRef = useRef<HTMLButtonElement>(null)

  // Focus the close button when dialog opens (for keyboard users)
  useEffect(() => {
    if (isDialogOpen) {
      setTimeout(() => closeRef.current?.focus(), 50)
    }
  }, [isDialogOpen])

  // Poll progress while downloading
  const { pollProgress } = useUpdateStore()
  useEffect(() => {
    if (!isDownloading) return
    const interval = setInterval(pollProgress, 500)
    return () => clearInterval(interval)
  }, [isDownloading, pollProgress])

  if (!isDialogOpen || !availableUpdate) return null

  const isReady    = downloadProgress?.stage === 'Ready'
  const isFailed   = downloadProgress?.stage === 'Failed'
  const hasStarted = downloadProgress !== null
  const isComplete = installResult?.success === true

  const handleDownload = () => startDownload(availableUpdate.version)
  const handleInstall  = () => installUpdate(availableUpdate.version)
  const handleSkip     = async () => {
    await skipVersion(availableUpdate.version)
    closeDialog()
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-dialog-title"
    >
      <div className="bg-gray-900 border border-gray-700 rounded-xl shadow-2xl w-full max-w-lg mx-4 overflow-hidden">
        {/* Header */}
        <div className={`px-6 py-4 border-b border-gray-700 flex items-center justify-between
          ${isCritical ? 'bg-orange-950/50' : ''}`}>
          <div>
            <h2 id="update-dialog-title" className="text-lg font-semibold text-gray-100 flex items-center gap-2">
              {isCritical && <span className="text-orange-400">⚠</span>}
              {isCritical ? 'Critical Update Required' : 'AutoDev Update Available'}
            </h2>
            <p className="text-sm text-gray-400 mt-0.5">
              Version {availableUpdate.version}
              {availableUpdate.release_date && (
                <span className="ml-2 opacity-60">· {availableUpdate.release_date}</span>
              )}
            </p>
          </div>
          {!isCritical && (
            <button
              ref={closeRef}
              onClick={closeDialog}
              className="text-gray-500 hover:text-gray-300 transition-colors p-1"
              aria-label="Close dialog"
            >
              ✕
            </button>
          )}
        </div>

        {/* Release notes */}
        {availableUpdate.release_notes && (
          <div className="px-6 py-4 max-h-48 overflow-y-auto border-b border-gray-800">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-2">
              What's new
            </h3>
            <div className="text-sm text-gray-300 whitespace-pre-wrap leading-relaxed">
              {availableUpdate.release_notes}
            </div>
          </div>
        )}

        {/* Download progress */}
        {hasStarted && (
          <div className="px-6 py-4 border-b border-gray-800">
            <div className="flex items-center justify-between text-xs text-gray-400 mb-2">
              <span>{stageLabel(downloadProgress!.stage)}</span>
              <span>
                {downloadProgress!.total_bytes > 0
                  ? `${formatBytes(downloadProgress!.bytes_downloaded)} / ${formatBytes(downloadProgress!.total_bytes)}`
                  : formatBytes(downloadProgress!.bytes_downloaded)}
                {isDownloading && downloadProgress!.speed_bps > 0 && (
                  <span className="ml-2 opacity-60">· {formatSpeed(downloadProgress!.speed_bps)}</span>
                )}
              </span>
            </div>
            <div className="h-2 bg-gray-700 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all duration-300 ${stageColor(downloadProgress!.stage)}`}
                style={{ width: `${Math.round(downloadProgress!.percent)}%` }}
              />
            </div>
            {isFailed && (
              <p className="text-xs text-red-400 mt-2">
                {downloadError || 'Download failed. Please try again.'}
              </p>
            )}
            {isReady && (
              <p className="text-xs text-green-400 mt-2 flex items-center gap-1">
                <span>✓</span> Signature verified — ready to install
              </p>
            )}
          </div>
        )}

        {/* Install result */}
        {isComplete && (
          <div className="px-6 py-4 bg-green-950/30 border-b border-green-900/50">
            <p className="text-sm text-green-400 flex items-center gap-2">
              <span>✓</span>
              {installResult!.message}
              {installResult!.requires_restart && (
                <span className="text-gray-400">&mdash; restarting…</span>
              )}
            </p>
          </div>
        )}

        {installError && (
          <div className="px-6 py-4 bg-red-950/30 border-b border-red-900/50">
            <p className="text-sm text-red-400">{installError}</p>
          </div>
        )}

        {/* Actions */}
        <div className="px-6 py-4 flex items-center justify-between">
          <div className="flex gap-2">
            {!isCritical && !isComplete && (
              <button
                onClick={handleSkip}
                className="text-sm text-gray-500 hover:text-gray-300 transition-colors px-3 py-1.5"
              >
                Skip this version
              </button>
            )}
          </div>

          <div className="flex gap-3">
            {!hasStarted && (
              <button
                onClick={handleDownload}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium rounded-lg transition-colors"
              >
                Download Update
              </button>
            )}

            {isDownloading && (
              <button
                disabled
                className="px-4 py-2 bg-blue-600/50 text-white/50 text-sm font-medium rounded-lg cursor-not-allowed flex items-center gap-2"
              >
                <span className="animate-spin">⟳</span>
                Downloading…
              </button>
            )}

            {isReady && !isComplete && (
              <button
                onClick={handleInstall}
                disabled={isInstalling}
                className="px-4 py-2 bg-green-600 hover:bg-green-500 text-white text-sm font-medium rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isInstalling ? 'Installing…' : 'Install & Restart'}
              </button>
            )}

            {isFailed && (
              <button
                onClick={handleDownload}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium rounded-lg transition-colors"
              >
                Retry Download
              </button>
            )}

            {!isCritical && !isComplete && (
              <button
                onClick={closeDialog}
                className="px-4 py-2 text-sm text-gray-400 hover:text-gray-200 transition-colors"
              >
                Later
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
