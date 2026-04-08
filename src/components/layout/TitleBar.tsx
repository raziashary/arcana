// TitleBar — macOS-style window chrome with traffic-light dots.
// Design from autodevmockup.html .titlebar

import { getCurrentWindow } from '@tauri-apps/api/window'

export default function TitleBar() {
  const win = getCurrentWindow()

  return (
    <div
      className="flex items-center justify-between px-3.5 py-2.5 select-none"
      style={{
        background:   'var(--bg-surface)',
        borderBottom: '1px solid var(--border-1)',
      }}
      data-tauri-drag-region
    >
      {/* Traffic-light dots */}
      <div className="flex gap-1.5" data-tauri-drag-region="false" style={{ pointerEvents: 'auto', position: 'relative', zIndex: 10 }}>
        <button
          type="button"
          onClick={() => win.close().catch(console.error)}
          className="w-3 h-3 rounded-full cursor-pointer border-none p-0 hover:opacity-80"
          style={{ background: '#FF5F57' }}
          aria-label="Close"
        />
        <button
          type="button"
          onClick={() => win.minimize().catch(console.error)}
          className="w-3 h-3 rounded-full cursor-pointer border-none p-0 hover:opacity-80"
          style={{ background: '#FEBC2E' }}
          aria-label="Minimize"
        />
        <button
          type="button"
          onClick={() => win.toggleMaximize().catch(console.error)}
          className="w-3 h-3 rounded-full cursor-pointer border-none p-0 hover:opacity-80"
          style={{ background: '#28C840' }}
          aria-label="Maximize"
        />
      </div>

      {/* App title */}
      <span
        className="font-mono text-xs"
        style={{ color: '#555', letterSpacing: '0.05em' }}
      >
        AutoDev — v1.0.0
      </span>

      {/* Right spacer (symmetrical with dots) */}
      <div className="w-16" />
    </div>
  )
}
