import { useState } from "react";
import { formatDistanceToNow } from "date-fns";
import { useSessionStore } from "../../stores/sessionStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { parseDbTimestamp } from "../../utils/dbTime"

export function SessionBar() {
  const { sessions, activeSessionId, createSession, switchSession, deleteSession, renameSession } = useSessionStore();
  const workspaceDir   = useWorkspaceStore(s => s.workspaceDir);
  const activeProvider = useSettingsStore(s => s.activeProvider);
  const activeModel    = useSettingsStore(s => s.activeModel);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");

  const handleNewSession = async () => {
    // Pass the configured workspace dir so Mode 1 engine uses the right root.
    // Fall back to '' only if nothing has been configured yet (Rust will then
    // use the app's cwd, which is harmless for a first-run blank state).
    const dir      = workspaceDir || '';
    const provider = activeProvider ?? 'anthropic';
    const model    = activeModel    ?? 'claude-sonnet-4-5';
    const newId    = await createSession(dir, provider, model);
    switchSession(newId);
  };

  const handleDeleteSession = (id: string) => {
    if (sessions.length <= 1) {
      alert("Cannot delete the last session");
      return;
    }
    if (confirm("Delete this session?")) {
      deleteSession(id);
    }
  };

  const handleRename = (id: string, title: string) => {
    setEditingId(id);
    setEditTitle(title);
  };

  const handleSaveRename = async (id: string) => {
    if (editTitle.trim()) {
      await renameSession(id, editTitle.trim());
    }
    setEditingId(null);
    setEditTitle("");
  };

  const handleCancelRename = () => {
    setEditingId(null);
    setEditTitle("");
  };

  return (
    <div
      className="flex flex-col h-full border-r"
      style={{
        width: "200px",
        borderColor: "var(--border-primary)",
        backgroundColor: "var(--surface-bg)",
      }}
    >
      {/* New Session Button */}
      <div className="p-2 border-b" style={{ borderColor: "var(--border-1)" }}>
        <button
          onClick={handleNewSession}
          className="w-full py-2 px-3 rounded transition-colors"
          style={{
            backgroundColor: "#1a73e8",
            color: "#ffffff",
            fontWeight: 500,
            border: "none",
            cursor: "pointer",
          }}
        >
          + New Session
        </button>
      </div>

      {/* Session List */}
      <div className="flex-1 overflow-y-auto">
        {sessions.map((session) => {
          const isActive = session.id === activeSessionId;
          const relativeTime = formatDistanceToNow(parseDbTimestamp(session.created_at) ?? new Date(0), {
            addSuffix: true,
          });

          return (
            <div
              key={session.id}
              onClick={() => switchSession(session.id)}
              className="p-3 border-b cursor-pointer transition-colors relative group"
              style={{
                borderColor: "var(--border-1)",
                backgroundColor: isActive ? "#1a73e820" : "transparent",
                borderLeft: isActive ? "3px solid #1a73e8" : "3px solid transparent",
              }}
            >
              {editingId === session.id ? (
                <input
                  type="text"
                  title="Rename session"
                  value={editTitle}
                  onChange={(e) => setEditTitle(e.target.value)}
                  onBlur={() => handleSaveRename(session.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSaveRename(session.id);
                    if (e.key === "Escape") handleCancelRename();
                  }}
                  autoFocus
                  className="text-sm font-medium w-full bg-transparent border-b outline-none"
                  style={{ color: "#ffffff", borderColor: "#1a73e8" }}
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <div
                  className="text-sm font-medium truncate"
                  style={{ color: "#ffffff" }}
                  onDoubleClick={(e) => {
                    e.stopPropagation();
                    handleRename(session.id, session.title || `Session ${session.id.slice(0, 6)}`);
                  }}
                >
                  {session.title || `Session ${session.id.slice(0, 6)}`}
                </div>
              )}
              <div className="text-xs mt-1" style={{ color: "#9ca3af" }}>
                {relativeTime}
              </div>
              <div className="text-xs mt-1" style={{ color: "#6b7280" }}>
                ✓ {session.message_count || 0} msgs
              </div>

              {/* Delete button (shows on hover) */}
              {!isActive && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDeleteSession(session.id);
                  }}
                  className="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity"
                  style={{
                    color: "var(--error-color)",
                    fontSize: "12px",
                  }}
                  title="Delete session"
                >
                  ⋮
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
