// File tree — collapsible folder browser with git-status badges.
//
// Behaviour:
//   - Click a directory row → toggle expansion (lazy-loads children on first open).
//   - Click a file row → open it in a new tab (ideStore.openFile).
//   - Git badges reflect the workspace_dir's git repo (if any).
//   - Empty workspace → shows a friendly hint pointing to Settings.

import { useEffect } from 'react'
import {
  ChevronDown,
  ChevronRight,
  File as FileIcon,
  Folder,
  FolderOpen,
  RefreshCw,
} from 'lucide-react'
import { useIdeStore, type FileTreeNode, type GitStatus } from '../../stores/ideStore'
import { useWorkspaceStore } from '../../stores/workspaceStore'

export default function FileTree() {
  const {
    rootTree,
    expandedDirs,
    treeLoading,
    treeError,
    refreshTree,
    toggleDir,
    openFile,
    activeTab,
  } = useIdeStore()

  const { workspaceDir } = useWorkspaceStore()

  useEffect(() => {
    if (workspaceDir && !rootTree && !treeLoading) {
      void refreshTree()
    }
  }, [workspaceDir])

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

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
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
        <button
          onClick={() => refreshTree()}
          className="p-1 rounded hover:bg-[var(--bg-hover)]"
          title="Refresh tree"
          aria-label="Refresh file tree"
        >
          <RefreshCw
            size={11}
            style={{ color: 'var(--text-3)' }}
            className={treeLoading ? 'animate-spin' : ''}
          />
        </button>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto py-1">
        {treeError && (
          <div className="px-3 py-2 text-[11px]" style={{ color: 'var(--danger)' }}>
            {treeError}
          </div>
        )}
        {rootTree?.children.map((child) => (
          <TreeRow
            key={child.relative_path}
            node={child}
            depth={0}
            expanded={expandedDirs}
            activeTab={activeTab}
            onToggle={toggleDir}
            onOpen={openFile}
          />
        ))}
      </div>
    </div>
  )
}

// ─── Row ──────────────────────────────────────────────────────────────────────

interface TreeRowProps {
  node:       FileTreeNode
  depth:      number
  expanded:   Set<string>
  activeTab:  string | null
  onToggle:   (rel: string) => void
  onOpen:     (rel: string) => void
}

function TreeRow({ node, depth, expanded, activeTab, onToggle, onOpen }: TreeRowProps) {
  const isExpanded = expanded.has(node.relative_path)
  const isActive   = activeTab === node.relative_path
  const indent     = 6 + depth * 10

  const rowStyle: React.CSSProperties = {
    paddingLeft: indent,
    color:       isActive ? 'var(--text-1)' : 'var(--text-2)',
    background:  isActive ? 'var(--bg-selected)' : undefined,
  }

  if (node.is_directory) {
    return (
      <div>
        <button
          onClick={() => onToggle(node.relative_path)}
          className="w-full flex items-center gap-1 py-0.5 pr-2 text-left hover:bg-[var(--bg-hover)]"
          style={rowStyle}
        >
          {isExpanded
            ? <ChevronDown size={10} style={{ color: 'var(--text-3)' }} />
            : <ChevronRight size={10} style={{ color: 'var(--text-3)' }} />}
          {isExpanded
            ? <FolderOpen size={12} style={{ color: 'var(--accent)' }} />
            : <Folder size={12} style={{ color: 'var(--accent)' }} />}
          <span className="text-[12px] font-mono truncate">{node.name}</span>
          <GitBadge status={node.git_status} />
        </button>

        {isExpanded && !node.has_more && node.children.map((c) => (
          <TreeRow
            key={c.relative_path}
            node={c}
            depth={depth + 1}
            expanded={expanded}
            activeTab={activeTab}
            onToggle={onToggle}
            onOpen={onOpen}
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
      </div>
    )
  }

  return (
    <button
      onClick={() => onOpen(node.relative_path)}
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

// ─── Git badge ────────────────────────────────────────────────────────────────

function GitBadge({ status }: { status: GitStatus | null }) {
  if (!status || status === 'clean' || status === 'ignored') return null
  const color = gitStatusColor(status)
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
