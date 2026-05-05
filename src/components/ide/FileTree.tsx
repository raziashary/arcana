// File tree — collapsible folder browser with git-status badges.
// Phase 3.2 — now includes:
//   - Right-click context menu: Open, Copy Path, Rename, Delete, New File, New Folder
//   - Drag-and-drop file movement (drag a file onto a folder to move it)

import { useEffect, useRef, useState } from 'react'
import {
  ChevronDown, ChevronRight,
  File as FileIcon, Folder, FolderOpen, RefreshCw,
  FilePlus, FolderPlus, Clipboard, Pencil, Trash2,
} from 'lucide-react'
import { useIdeStore, type FileTreeNode, type GitStatus } from '../../stores/ideStore'
import { useWorkspaceStore } from '../../stores/workspaceStore'

// ── Context menu state ────────────────────────────────────────────────────────

interface ContextMenuState {
  x:    number
  y:    number
  node: FileTreeNode
}

// ── FileTree root ─────────────────────────────────────────────────────────────

export default function FileTree() {
  const {
    rootTree, expandedDirs, treeLoading, treeError,
    refreshTree, toggleDir, openFile,
    activeTab, createFile, createDirectory, deleteFile, renameFile, moveFile,
  } = useIdeStore()

  const { workspaceDir } = useWorkspaceStore()

  const [menu,        setMenu]        = useState<ContextMenuState | null>(null)
  const [renaming,    setRenaming]    = useState<{ node: FileTreeNode; value: string } | null>(null)
  const [newItem,     setNewItem]     = useState<{ parentDir: string; type: 'file' | 'dir'; value: string } | null>(null)
  const [dragOver,    setDragOver]    = useState<string | null>(null)   // relative_path of hovered dir
  const dragSource    = useRef<string | null>(null)

  useEffect(() => {
    if (workspaceDir && !rootTree && !treeLoading) void refreshTree()
  }, [workspaceDir])

  // Dismiss context menu on outside click
  useEffect(() => {
    if (!menu) return
    const dismiss = () => setMenu(null)
    window.addEventListener('click', dismiss)
    return () => window.removeEventListener('click', dismiss)
  }, [menu])

  if (!workspaceDir) {
    return (
      <div
        className="flex-1 flex flex-col items-center justify-center p-4 text-center"
        style={{ color: 'var(--text-3)' }}
      >
        <p className="text-xs font-mono mb-2">No workspace</p>
        <p className="text-[11px]" style={{ color: 'var(--text-2)' }}>
          Pick a workspace folder in Settings to browse files here.
        </p>
      </div>
    )
  }

  // ── Context menu actions ───────────────────────────────────────────────────

  function openContextMenu(e: React.MouseEvent, node: FileTreeNode) {
    e.preventDefault()
    e.stopPropagation()
    setMenu({ x: e.clientX, y: e.clientY, node })
  }

  async function ctxOpen(node: FileTreeNode) {
    setMenu(null)
    if (!node.is_directory) await openFile(node.relative_path)
  }

  function ctxCopyPath(node: FileTreeNode) {
    setMenu(null)
    void navigator.clipboard.writeText(node.relative_path)
  }

  function ctxRename(node: FileTreeNode) {
    setMenu(null)
    setRenaming({ node, value: node.name })
  }

  async function ctxDelete(node: FileTreeNode) {
    setMenu(null)
    if (!window.confirm(`Delete "${node.name}"? This cannot be undone.`)) return
    await deleteFile(node.relative_path)
  }

  function ctxNewFile(node: FileTreeNode) {
    setMenu(null)
    const dir = node.is_directory ? node.relative_path : node.relative_path.split('/').slice(0, -1).join('/')
    setNewItem({ parentDir: dir, type: 'file', value: '' })
  }

  function ctxNewFolder(node: FileTreeNode) {
    setMenu(null)
    const dir = node.is_directory ? node.relative_path : node.relative_path.split('/').slice(0, -1).join('/')
    setNewItem({ parentDir: dir, type: 'dir', value: '' })
  }

  async function commitRename() {
    if (!renaming) return
    const newName = renaming.value.trim()
    if (!newName || newName === renaming.node.name) { setRenaming(null); return }
    const parts   = renaming.node.relative_path.split('/')
    parts[parts.length - 1] = newName
    const newPath = parts.join('/')
    setRenaming(null)
    await renameFile(renaming.node.relative_path, newPath)
  }

  async function commitNewItem() {
    if (!newItem) return
    const name = newItem.value.trim()
    if (!name) { setNewItem(null); return }
    const path = newItem.parentDir ? `${newItem.parentDir}/${name}` : name
    setNewItem(null)
    if (newItem.type === 'file') await createFile(path)
    else                         await createDirectory(path)
  }

  // ── Drag-and-drop ──────────────────────────────────────────────────────────

  function handleDragStart(relativePath: string) {
    dragSource.current = relativePath
  }

  function handleDragOver(e: React.DragEvent, relativePath: string) {
    e.preventDefault()
    e.stopPropagation()
    setDragOver(relativePath)
  }

  async function handleDrop(e: React.DragEvent, targetDir: string) {
    e.preventDefault()
    e.stopPropagation()
    setDragOver(null)
    const src = dragSource.current
    dragSource.current = null
    if (!src || src === targetDir) return
    // Only allow dropping into a directory
    await moveFile(src, targetDir)
  }

  function handleDragLeave() {
    setDragOver(null)
  }

  // ── Render ─────────────────────────────────────────────────────────────────

  return (
    <div className="flex-1 flex flex-col overflow-hidden" onDragLeave={handleDragLeave}>
      {/* Header */}
      <div
        className="flex items-center justify-between px-2 py-1.5 border-b"
        style={{ borderColor: 'var(--border-1)' }}
      >
        <span
          className="text-[10px] font-mono uppercase tracking-wider"
          style={{ color: 'var(--text-2)' }}
          title={workspaceDir}
        >
          Explorer
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setNewItem({ parentDir: '', type: 'file', value: '' })}
            className="p-1 rounded hover:bg-[var(--bg-hover)]"
            title="New File"
          >
            <FilePlus size={11} style={{ color: 'var(--text-3)' }} />
          </button>
          <button
            onClick={() => setNewItem({ parentDir: '', type: 'dir', value: '' })}
            className="p-1 rounded hover:bg-[var(--bg-hover)]"
            title="New Folder"
          >
            <FolderPlus size={11} style={{ color: 'var(--text-3)' }} />
          </button>
          <button
            onClick={() => void refreshTree()}
            className="p-1 rounded hover:bg-[var(--bg-hover)]"
            title="Refresh tree"
          >
            <RefreshCw
              size={11}
              style={{ color: 'var(--text-3)' }}
              className={treeLoading ? 'animate-spin' : ''}
            />
          </button>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto py-1">
        {treeError && (
          <div className="px-3 py-2 text-[11px]" style={{ color: 'var(--danger)' }}>
            {treeError}
          </div>
        )}

        {/* New item at root */}
        {newItem && newItem.parentDir === '' && (
          <NewItemInput
            type={newItem.type}
            value={newItem.value}
            indent={6}
            onChange={v => setNewItem(s => s ? { ...s, value: v } : s)}
            onCommit={commitNewItem}
            onCancel={() => setNewItem(null)}
          />
        )}

        {rootTree?.children.map((child) => (
          <TreeRow
            key={child.relative_path}
            node={child}
            depth={0}
            expanded={expandedDirs}
            activeTab={activeTab}
            dragOver={dragOver}
            renaming={renaming}
            newItem={newItem}
            onToggle={toggleDir}
            onOpen={openFile}
            onContextMenu={openContextMenu}
            onDragStart={handleDragStart}
            onDragOver={handleDragOver}
            onDrop={handleDrop}
            onRenameChange={v => setRenaming(s => s ? { ...s, value: v } : s)}
            onRenameCommit={commitRename}
            onRenameCancel={() => setRenaming(null)}
            onNewItemChange={v => setNewItem(s => s ? { ...s, value: v } : s)}
            onNewItemCommit={commitNewItem}
            onNewItemCancel={() => setNewItem(null)}
          />
        ))}
      </div>

      {/* Context menu */}
      {menu && (
        <div
          className="fixed z-50 rounded shadow-lg py-1 text-xs font-mono"
          style={{
            left:       menu.x,
            top:        menu.y,
            background: 'var(--bg-panel)',
            border:     '1px solid var(--border)',
            minWidth:   160,
          }}
          onClick={e => e.stopPropagation()}
        >
          {!menu.node.is_directory && (
            <MenuItem
              label="Open"
              onClick={() => void ctxOpen(menu.node)}
            />
          )}
          <MenuItem label="Copy Path" icon={<Clipboard size={11} />} onClick={() => ctxCopyPath(menu.node)} />
          <MenuItem label="Rename"    icon={<Pencil size={11} />}    onClick={() => ctxRename(menu.node)} />
          <div style={{ borderTop: '1px solid var(--border)', margin: '4px 0' }} />
          <MenuItem label="New File"   icon={<FilePlus size={11} />}   onClick={() => ctxNewFile(menu.node)} />
          <MenuItem label="New Folder" icon={<FolderPlus size={11} />} onClick={() => ctxNewFolder(menu.node)} />
          <div style={{ borderTop: '1px solid var(--border)', margin: '4px 0' }} />
          <MenuItem label="Delete" icon={<Trash2 size={11} />} danger onClick={() => void ctxDelete(menu.node)} />
        </div>
      )}

      {/* Inline rename dialog (if renaming a node that's scrolled off) */}
    </div>
  )
}

// ── MenuItem ──────────────────────────────────────────────────────────────────

function MenuItem({
  label, icon, danger, onClick,
}: {
  label:   string
  icon?:   React.ReactNode
  danger?: boolean
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-2 px-3 py-1 text-left hover:bg-[var(--bg-hover)]"
      style={{ color: danger ? 'var(--danger)' : 'var(--text-2)' }}
    >
      {icon && <span style={{ color: danger ? 'var(--danger)' : 'var(--text-3)' }}>{icon}</span>}
      {label}
    </button>
  )
}

// ── NewItemInput ──────────────────────────────────────────────────────────────

function NewItemInput({
  type, value, indent, onChange, onCommit, onCancel,
}: {
  type:     'file' | 'dir'
  value:    string
  indent:   number
  onChange: (v: string) => void
  onCommit: () => void
  onCancel: () => void
}) {
  return (
    <div className="flex items-center gap-1 pr-2" style={{ paddingLeft: indent + 12 }}>
      {type === 'file'
        ? <FileIcon size={12} style={{ color: 'var(--text-3)' }} />
        : <Folder   size={12} style={{ color: 'var(--accent)'  }} />
      }
      <input
        autoFocus
        value={value}
        onChange={e => onChange(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter')  onCommit()
          if (e.key === 'Escape') onCancel()
        }}
        onBlur={onCommit}
        className="flex-1 text-[12px] font-mono px-1 py-0.5 rounded"
        style={{
          background: 'var(--bg-input)',
          border:     '1px solid var(--accent)',
          color:      'var(--text-1)',
          outline:    'none',
        }}
        placeholder={type === 'file' ? 'filename.rs' : 'foldername'}
      />
    </div>
  )
}

// ── TreeRow ───────────────────────────────────────────────────────────────────

interface TreeRowProps {
  node:           FileTreeNode
  depth:          number
  expanded:       Set<string>
  activeTab:      string | null
  dragOver:       string | null
  renaming:       { node: FileTreeNode; value: string } | null
  newItem:        { parentDir: string; type: 'file' | 'dir'; value: string } | null
  onToggle:       (rel: string) => void
  onOpen:         (rel: string) => void
  onContextMenu:  (e: React.MouseEvent, node: FileTreeNode) => void
  onDragStart:    (rel: string) => void
  onDragOver:     (e: React.DragEvent, rel: string) => void
  onDrop:         (e: React.DragEvent, rel: string) => Promise<void>
  onRenameChange: (v: string) => void
  onRenameCommit: () => void
  onRenameCancel: () => void
  onNewItemChange:  (v: string) => void
  onNewItemCommit:  () => void
  onNewItemCancel:  () => void
}

function TreeRow({
  node, depth, expanded, activeTab, dragOver, renaming, newItem,
  onToggle, onOpen, onContextMenu,
  onDragStart, onDragOver, onDrop,
  onRenameChange, onRenameCommit, onRenameCancel,
  onNewItemChange, onNewItemCommit, onNewItemCancel,
}: TreeRowProps) {
  const isExpanded = expanded.has(node.relative_path)
  const isActive   = activeTab === node.relative_path
  const isDragOver = dragOver === node.relative_path
  const isRenaming = renaming?.node.relative_path === node.relative_path
  const indent     = 6 + depth * 10

  const rowStyle: React.CSSProperties = {
    paddingLeft: indent,
    color:       isActive ? 'var(--text-1)' : 'var(--text-2)',
    background:  isDragOver
      ? 'var(--bg-selected)'
      : isActive
      ? 'var(--bg-selected)'
      : undefined,
    outline: isDragOver ? '1px solid var(--accent)' : undefined,
  }

  if (node.is_directory) {
    return (
      <div>
        {/* Directory row */}
        {isRenaming ? (
          <div style={{ paddingLeft: indent }}>
            <NewItemInput
              type="dir"
              value={renaming!.value}
              indent={0}
              onChange={onRenameChange}
              onCommit={onRenameCommit}
              onCancel={onRenameCancel}
            />
          </div>
        ) : (
          <button
            draggable
            onDragStart={() => onDragStart(node.relative_path)}
            onDragOver={e  => onDragOver(e, node.relative_path)}
            onDrop={e      => void onDrop(e, node.relative_path)}
            onClick={() => onToggle(node.relative_path)}
            onContextMenu={e => onContextMenu(e, node)}
            className="w-full flex items-center gap-1 py-0.5 pr-2 text-left hover:bg-[var(--bg-hover)]"
            style={rowStyle}
          >
            {isExpanded
              ? <ChevronDown  size={10} style={{ color: 'var(--text-3)' }} />
              : <ChevronRight size={10} style={{ color: 'var(--text-3)' }} />}
            {isExpanded
              ? <FolderOpen size={12} style={{ color: 'var(--accent)' }} />
              : <Folder     size={12} style={{ color: 'var(--accent)' }} />}
            <span className="text-[12px] font-mono truncate">{node.name}</span>
            <GitBadge status={node.git_status} />
          </button>
        )}

        {/* Children */}
        {isExpanded && !node.has_more && node.children.map((c) => (
          <TreeRow
            key={c.relative_path}
            node={c}
            depth={depth + 1}
            expanded={expanded}
            activeTab={activeTab}
            dragOver={dragOver}
            renaming={renaming}
            newItem={newItem}
            onToggle={onToggle}
            onOpen={onOpen}
            onContextMenu={onContextMenu}
            onDragStart={onDragStart}
            onDragOver={onDragOver}
            onDrop={onDrop}
            onRenameChange={onRenameChange}
            onRenameCommit={onRenameCommit}
            onRenameCancel={onRenameCancel}
            onNewItemChange={onNewItemChange}
            onNewItemCommit={onNewItemCommit}
            onNewItemCancel={onNewItemCancel}
          />
        ))}

        {isExpanded && node.has_more && (
          <div
            className="text-[11px] font-mono py-0.5"
            style={{ paddingLeft: indent + 22, color: 'var(--text-3)' }}
          >
            loading…
          </div>
        )}

        {/* New item inside this directory */}
        {isExpanded && newItem?.parentDir === node.relative_path && (
          <NewItemInput
            type={newItem.type}
            value={newItem.value}
            indent={indent + 22}
            onChange={onNewItemChange}
            onCommit={onNewItemCommit}
            onCancel={onNewItemCancel}
          />
        )}
      </div>
    )
  }

  // File row
  return isRenaming ? (
    <div style={{ paddingLeft: indent }}>
      <NewItemInput
        type="file"
        value={renaming!.value}
        indent={0}
        onChange={onRenameChange}
        onCommit={onRenameCommit}
        onCancel={onRenameCancel}
      />
    </div>
  ) : (
    <button
      draggable
      onDragStart={() => onDragStart(node.relative_path)}
      onClick={() => void onOpen(node.relative_path)}
      onContextMenu={e => onContextMenu(e, node)}
      className="w-full flex items-center gap-1 py-0.5 pr-2 text-left hover:bg-[var(--bg-hover)]"
      style={rowStyle}
    >
      <span style={{ width: 10 }} />
      <FileIcon size={12} style={{ color: 'var(--text-3)' }} />
      <span className="text-[12px] font-mono truncate">{node.name}</span>
      <GitBadge status={node.git_status} />
    </button>
  )
}

// ── Git badge ─────────────────────────────────────────────────────────────────

function GitBadge({ status }: { status: GitStatus | null }) {
  if (!status || status === 'clean' || status === 'ignored') return null
  const color  = gitStatusColor(status)
  const letter = gitStatusLetter(status)
  return (
    <span
      className="ml-auto text-[10px] font-mono font-semibold px-1 rounded"
      style={{ color, borderColor: color }}
      title={status}
    >
      {letter}
    </span>
  )
}

function gitStatusColor(s: GitStatus): string {
  switch (s) {
    case 'staged':     return 'var(--success)'
    case 'modified':   return 'var(--warning)'
    case 'untracked':  return 'var(--aqua)'
    case 'deleted':    return 'var(--danger)'
    case 'conflicted': return 'var(--danger)'
    default:           return 'var(--text-3)'
  }
}

function gitStatusLetter(s: GitStatus): string {
  switch (s) {
    case 'staged':     return 'S'
    case 'modified':   return 'M'
    case 'untracked':  return 'U'
    case 'deleted':    return 'D'
    case 'conflicted': return '!'
    default:           return ''
  }
}
