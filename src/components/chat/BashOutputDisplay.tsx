import { useState } from "react";
import Ansi from "ansi-to-react";
import { BashOutput } from "../../utils/toolOutputParser";

interface BashOutputDisplayProps {
  data: BashOutput;
}

export function BashOutputDisplay({ data }: BashOutputDisplayProps) {
  const [showStderr, setShowStderr] = useState(false);

  const hasStdout = data.stdout && data.stdout.trim().length > 0;
  const hasStderr = data.stderr && data.stderr.trim().length > 0;
  const maxLines = 48;
  const stdoutLines = data.stdout.split("\n");
  const truncatedStdout = stdoutLines.length > maxLines 
    ? stdoutLines.slice(0, maxLines).join("\n") + `\n\n... (truncated ${stdoutLines.length - maxLines} lines)`
    : data.stdout;

  return (
    <div className="rounded" style={{ backgroundColor: "var(--surface-bg)", border: "1px solid var(--border-primary)" }}>
      {/* Header */}
      <div className="p-2 border-b" style={{ borderColor: "var(--border-primary)" }}>
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold" style={{ color: "var(--text-secondary)" }}>
            Command:
          </span>
          <code className="font-mono text-sm" style={{ color: "var(--accent-color)" }}>
            {data.command}
          </code>
        </div>
        <div className="flex items-center gap-3 mt-1">
          <span
            className="px-2 py-0.5 rounded text-xs font-semibold"
            style={{
              backgroundColor: data.exit_code === 0 ? "#00FF8820" : "#FF006620",
              color: data.exit_code === 0 ? "#00FF88" : "#FF0066",
            }}
          >
            Exit {data.exit_code}
          </span>
          {data.execution_time_ms && (
            <span className="text-xs" style={{ color: "var(--text-tertiary)" }}>
              {data.execution_time_ms}ms
            </span>
          )}
        </div>
      </div>

      {/* Stdout */}
      {hasStdout && (
        <div className="p-3 border-b" style={{ borderColor: "var(--border-secondary)" }}>
          <div className="text-xs mb-2 font-semibold" style={{ color: "var(--text-secondary)" }}>
            Output:
          </div>
          <pre
            className="overflow-x-auto"
            style={{
              backgroundColor: "#00000020",
              padding: "8px",
              borderRadius: "4px",
              fontSize: "13px",
              lineHeight: "1.5",
              color: "var(--text-primary)",
              fontFamily: "'Courier New', monospace",
            }}
          >
            <Ansi>{truncatedStdout}</Ansi>
          </pre>
        </div>
      )}

      {/* Stderr */}
      {hasStderr && (
        <div className="p-3">
          <button
            onClick={() => setShowStderr(!showStderr)}
            className="text-xs font-semibold mb-2 flex items-center gap-1 transition-colors"
            style={{ color: "#FF0066" }}
          >
            <span>{showStderr ? "▼" : "▶"}</span>
            Error Output ({data.stderr.split("\n").length} lines)
          </button>
          {showStderr && (
            <pre
              className="overflow-x-auto"
              style={{
                backgroundColor: "#FF006610",
                padding: "8px",
                borderRadius: "4px",
                fontSize: "13px",
                lineHeight: "1.5",
                color: "#FF0066",
                fontFamily: "'Courier New', monospace",
                border: "1px solid #FF006640",
              }}
            >
              <Ansi>{data.stderr}</Ansi>
            </pre>
          )}
        </div>
      )}

      {!hasStdout && !hasStderr && (
        <div className="p-3 text-sm text-center" style={{ color: "var(--text-tertiary)" }}>
          No output
        </div>
      )}
    </div>
  );
}
