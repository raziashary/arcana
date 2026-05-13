// BuildingContextMenu.tsx — Right-click context menu for buildings
import { useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { GraphNode } from '../../../stores/graphStore'

interface BuildingContextMenuProps {
  nodeId: string
  nodes: GraphNode[]
  x: number
  y: number
  onClose: () => void
}

export function BuildingContextMenu({ nodeId, nodes, x, y, onClose }: BuildingContextMenuProps) {
  const node = nodes.find(n => n.id === nodeId)
  const label = node?.label || node?.id || nodeId

  const handleOpenInIde = useCallback(async () => {
    if (!node?.file_path) return
    try {
      await invoke('open_in_ide', { filePath: node.file_path })
    } catch (e) {
      console.error('open_in_ide failed:', e)
    }
    onClose()
  }, [node, onClose])

  const handleCopyQualifiedName = useCallback(() => {
    navigator.clipboard.writeText(nodeId).catch(() => {})
    onClose()
  }, [nodeId, onClose])

  const handleCopyFilePath = useCallback(() => {
    navigator.clipboard.writeText(node?.file_path ?? '').catch(() => {})
    onClose()
  }, [node, onClose])

  const menuStyle: React.CSSProperties = {
    position: 'fixed',
    left: x,
    top: y,
    zIndex: 1000,
    background: '#1a1a1f',
    border: '1px solid rgba(255,255,255,0.12)',
    borderRadius: 6,
    padding: '4px 0',
    minWidth: 200,
    boxShadow: '0 8px 24px rgba(0,0,0,0.6)',
    fontFamily: 'system-ui, sans-serif',
    fontSize: 12,
    color: '#ccc',
  }

  const itemStyle: React.CSSProperties = {
    padding: '6px 16px',
    cursor: 'pointer',
    display: 'block',
    width: '100%',
    background: 'none',
    border: 'none',
    textAlign: 'left',
    color: '#ccc',
    fontSize: 12,
    fontFamily: 'inherit',
  }

  const divStyle: React.CSSProperties = {
    height: 1,
    background: 'rgba(255,255,255,0.08)',
    margin: '4px 0',
  }

  const titleStyle: React.CSSProperties = {
    padding: '4px 16px 6px',
    color: '#00ff88',
    fontSize: 11,
    fontWeight: 600,
    fontFamily: 'monospace',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    maxWidth: 200,
  }

  return (
    <>
      {/* Backdrop to close */}
      <div
        style={{ position: 'fixed', inset: 0, zIndex: 999 }}
        onClick={onClose}
        onContextMenu={e => { e.preventDefault(); onClose() }}
      />
      <div style={menuStyle}>
        <div style={titleStyle}>{label}</div>
        <div style={divStyle} />
        <button style={itemStyle} onClick={handleOpenInIde}
          onMouseEnter={e => (e.currentTarget.style.background = 'rgba(255,255,255,0.06)')}
          onMouseLeave={e => (e.currentTarget.style.background = 'none')}>
          Open in IDE Tab
        </button>
        <div style={divStyle} />
        <button style={itemStyle} onClick={handleCopyQualifiedName}
          onMouseEnter={e => (e.currentTarget.style.background = 'rgba(255,255,255,0.06)')}
          onMouseLeave={e => (e.currentTarget.style.background = 'none')}>
          Copy qualified name
        </button>
        <button style={itemStyle} onClick={handleCopyFilePath}
          onMouseEnter={e => (e.currentTarget.style.background = 'rgba(255,255,255,0.06)')}
          onMouseLeave={e => (e.currentTarget.style.background = 'none')}>
          Copy file path
        </button>
      </div>
    </>
  )
}
