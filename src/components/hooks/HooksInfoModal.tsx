// HooksInfoModal — Educational modal explaining hooks system
// Phase 3.1

import { X } from 'lucide-react'

interface Props {
  onClose: () => void
}

export default function HooksInfoModal({ onClose }: Props) {
  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={e => e.target === e.currentTarget && onClose()}
    >
      <div
        className="rounded-lg w-full max-w-3xl max-h-[90vh] overflow-hidden flex flex-col"
        style={{ background: 'var(--bg-1)', border: '1px solid var(--border)' }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4" style={{ borderBottom: '1px solid var(--border)' }}>
          <div>
            <div className="font-mono text-sm font-semibold" style={{ color: 'var(--text-1)' }}>
              What are Hooks?
            </div>
          </div>
          <button title="Close" onClick={onClose} className="p-1 rounded hover:opacity-70">
            <X size={16} style={{ color: 'var(--text-3)' }} />
          </button>
        </div>

        {/* Content */}
        <div className="overflow-y-auto px-5 py-4 space-y-4 flex-1 text-xs" style={{ color: 'var(--text-2)' }}>
          {/* What is Hooks */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              What are Hooks?
            </h3>
            <p>
              Hooks are event-driven actions that execute automatically at specific lifecycle points in Arcana. They allow you to customize behavior, automate tasks, and integrate with external systems without modifying core code.
            </p>
            <p className="mt-2">
              Think of them like Git hooks (pre-commit, post-commit) but for the entire agent lifecycle: before/after tools run, when files change, when commits happen, when plans execute, etc.
            </p>
          </section>

          {/* When to Use */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              When to Use Hooks
            </h3>
            <ul className="space-y-1 ml-4 list-disc">
              <li><strong>Notifications:</strong> Send Slack/Discord messages when tasks complete</li>
              <li><strong>Auto-formatting:</strong> Run prettier/rustfmt after file writes</li>
              <li><strong>Logging:</strong> Log all tool executions to a custom file</li>
              <li><strong>Blocking:</strong> Prevent dangerous commands (e.g., `rm -rf /`)</li>
              <li><strong>CI/CD:</strong> Trigger GitHub Actions or Jenkins on commits</li>
              <li><strong>Monitoring:</strong> Track agent behavior and performance</li>
            </ul>
          </section>

          {/* Hook Types */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              Hook Types (When They Trigger)
            </h3>
            <div className="space-y-2 ml-4">
              <div>
                <strong>Agent Lifecycle:</strong> session_start, session_end, agent_loop_start, agent_loop_end, on_error
              </div>
              <div>
                <strong>Tool Execution:</strong> pre_tool_use, post_tool_use, on_tool_error, on_permission_request
              </div>
              <div>
                <strong>File System:</strong> pre_file_read, post_file_write, pre_file_delete, on_checkpoint_create, on_checkpoint_restore
              </div>
              <div>
                <strong>Git Operations:</strong> pre_commit, post_commit, pre_push, on_branch_switch, on_merge_conflict
              </div>
              <div>
                <strong>Plan Management:</strong> pre_plan_step, post_plan_step, on_plan_activate, on_plan_complete, on_plan_abandon
              </div>
              <div>
                <strong>Fleet (Workers):</strong> on_task_claim, on_task_complete, on_task_fail, on_qc_review_start, on_qc_approve, on_qc_fail
              </div>
            </div>
          </section>

          {/* How to Create */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              How to Create a Hook
            </h3>
            <ol className="space-y-2 ml-4 list-decimal">
              <li>Click <strong>"+ Add Hook"</strong> button in the Hooks panel</li>
              <li>Fill in the form:
                <ul className="ml-4 mt-1 space-y-1 list-disc">
                  <li><strong>Name:</strong> Unique identifier (e.g., "notify-on-write")</li>
                  <li><strong>Hook Type:</strong> When to trigger (e.g., "post_file_write")</li>
                  <li><strong>Priority:</strong> Execution order (higher = runs first)</li>
                  <li><strong>Conditions:</strong> Filter when to run (optional, e.g., file_pattern: "*.rs")</li>
                  <li><strong>Action:</strong> What to do (shell, http, notification, log, block, emit_event)</li>
                </ul>
              </li>
              <li>Click <strong>"Create Hook"</strong></li>
            </ol>
          </section>

          {/* Action Types */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              Action Types (What Hooks Can Do)
            </h3>
            <div className="space-y-2 ml-4">
              <div>
                <strong>shell:</strong> Execute shell command (e.g., `cargo fmt`, `notify-send`)
              </div>
              <div>
                <strong>http:</strong> Send HTTP request (e.g., POST to Slack webhook)
              </div>
              <div>
                <strong>notification:</strong> Desktop notification popup
              </div>
              <div>
                <strong>log:</strong> Write to custom log file
              </div>
              <div>
                <strong>block:</strong> Prevent action with error message
              </div>
              <div>
                <strong>emit_event:</strong> Send custom Tauri event to frontend
              </div>
            </div>
          </section>

          {/* How to Trigger */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              How Hooks Trigger (Automatic)
            </h3>
            <p>
              Hooks trigger <strong>automatically</strong> when their event occurs. You don't need to do anything special:
            </p>
            <ul className="space-y-1 ml-4 mt-2 list-disc">
              <li>Write a file → <strong>post_file_write</strong> hooks run</li>
              <li>Execute a tool → <strong>pre_tool_use</strong> and <strong>post_tool_use</strong> hooks run</li>
              <li>Create a commit → <strong>pre_commit</strong> and <strong>post_commit</strong> hooks run</li>
              <li>And so on...</li>
            </ul>
            <p className="mt-2">
              The hook system evaluates conditions (if any) and executes matching hooks in priority order.
            </p>
          </section>

          {/* What to Check */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              What to Check / Debug
            </h3>
            <ul className="space-y-1 ml-4 list-disc">
              <li><strong>Hook Status:</strong> Enable/disable toggle on each hook row</li>
              <li><strong>Test Button:</strong> Click ▶ to test a hook with empty context</li>
              <li><strong>Execution Logs:</strong> Click "Logs" button to view all hook executions</li>
              <li><strong>Filter Logs:</strong> Use tabs (All / Executed / Failed / Blocked / Skipped) to find issues</li>
              <li><strong>Expand Log Entry:</strong> Click row to see full output and error details</li>
              <li><strong>Export Logs:</strong> Download logs as JSON for analysis</li>
            </ul>
          </section>

          {/* Example */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              Example: Notify on Rust File Write
            </h3>
            <div
              className="p-3 rounded mt-2 font-mono text-xs"
              style={{ background: 'var(--bg-2)', border: '1px solid var(--border)' }}
            >
              <div><strong>Name:</strong> notify-rust-write</div>
              <div><strong>Hook Type:</strong> post_file_write</div>
              <div><strong>Priority:</strong> 50</div>
              <div><strong>Condition - file_pattern:</strong> *.rs</div>
              <div><strong>Action Type:</strong> notification</div>
              <div><strong>Action - title:</strong> Rust file modified</div>
              <div><strong>Action - body:</strong> File was written</div>
              <div className="mt-2" style={{ color: 'var(--text-3)' }}>
                → Every time a .rs file is written, a desktop notification appears
              </div>
            </div>
          </section>

          {/* Tips */}
          <section>
            <h3 className="font-semibold mb-2" style={{ color: 'var(--text-1)' }}>
              Tips & Best Practices
            </h3>
            <ul className="space-y-1 ml-4 list-disc">
              <li>Use <strong>conditions</strong> to avoid running hooks on every event</li>
              <li>Set <strong>priority</strong> high (1000+) for safety-critical hooks (e.g., blocking dangerous commands)</li>
              <li>Test hooks with the ▶ button before enabling in production</li>
              <li>Check logs regularly to catch failures</li>
              <li>Use <strong>block</strong> action sparingly; prefer logging and notifications</li>
              <li>Variable substitution: Use double braces in action commands</li>
            </ul>
          </section>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end px-5 py-3" style={{ borderTop: '1px solid var(--border)' }}>
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded text-xs font-mono"
            style={{ background: 'var(--accent)', color: '#011627' }}
          >
            Got it
          </button>
        </div>
      </div>
    </div>
  )
}
