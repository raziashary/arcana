// Sidebar — right sidebar showing MCP servers, sidecar status, and AUTODEV.md editor.
// Phase 2.1: Added AUTODEV.md card with edit buttons.
// Design from autodevmockup.html .sidebar

import AutodevMdCard from '../autodev/AutodevMdCard'

export default function Sidebar() {
  return (
    <div
      className="shrink-0 flex flex-col gap-3.5 p-3"
      style={{
        width:        '176px',
        borderLeft:   '1px solid var(--border-1)',
        background:   'var(--bg-surface)',
      }}
    >
      <SidebarSection title="MCP Servers">
        <SidebarItem label="github"  active />
        <SidebarItem label="memory"  active />
        <SidebarItem label="context" active={false} />
      </SidebarSection>

      <SidebarSection title="Sidecars">
        <SidebarItem label="core-orchestrator" active={false} />
        <SidebarItem label="memory-mcp"        active={false} />
      </SidebarSection>

      <SidebarSection title="Session">
        <SidebarItem label="PA mode" active />
      </SidebarSection>

      {/* AUTODEV.md editor card */}
      <AutodevMdCard />
    </div>
  )
}

function SidebarSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div
        className="font-mono mb-1.5"
        style={{
          fontSize:      '10px',
          color:         'var(--text-4)',
          letterSpacing: '0.08em',
          textTransform: 'uppercase',
        }}
      >
        {title}
      </div>
      {children}
    </div>
  )
}

function SidebarItem({ label, active }: { label: string; active: boolean }) {
  return (
    <div
      className="flex items-center gap-1.5 py-1"
      style={{
        fontSize:   '11px',
        color:      'var(--text-2)',
        fontFamily: 'var(--font-mono)',
      }}
    >
      <div
        className="shrink-0 rounded-full"
        style={{
          width:      '5px',
          height:     '5px',
          background: active ? 'var(--accent)' : '#2a2a2a',
        }}
      />
      <span className="truncate">{label}</span>
    </div>
  )
}
