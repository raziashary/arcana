import { useState } from "react";
import { ExternalLink } from "lucide-react";
import { GrepOutput } from "../../utils/toolOutputParser";
import { openInIde } from "../../utils/openInIde";

interface GrepResultsDisplayProps {
  data: GrepOutput;
}

export function GrepResultsDisplay({ data }: GrepResultsDisplayProps) {
  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set());

  const toggleFile = (file: string) => {
    setExpandedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(file)) {
        next.delete(file);
      } else {
        next.add(file);
      }
      return next;
    });
  };

  const highlightMatch = (content: string, positions: [number, number][]) => {
    if (positions.length === 0) return content;
    const [start, end] = positions[0];
    return (
      <>
        {content.slice(0, start)}
        <span style={{ backgroundColor: "#FFD70050", fontWeight: 600 }}>
          {content.slice(start, end)}
        </span>
        {content.slice(end)}
      </>
    );
  };

  return (
    <div className="rounded p-3" style={{ backgroundColor: "var(--surface-bg)", border: "1px solid var(--border-primary)" }}>
      {/* Summary */}
      <div className="mb-2" style={{ color: "var(--text-secondary)" }}>
        <div className="font-semibold mb-1">Pattern: <code style={{ backgroundColor: "#ffffff10", padding: "2px 6px", borderRadius: "3px" }}>{data.pattern}</code></div>
        <div className="text-sm">
          Found <strong>{data.total_matches}</strong> matches in <strong>{data.files_matched}</strong> files
        </div>
      </div>

      {/* File Results */}
      <div className="space-y-2">
        {data.results.map((fileResult) => {
          const isExpanded = expandedFiles.has(fileResult.file);
          return (
            <div key={fileResult.file} className="border rounded" style={{ borderColor: "var(--border-primary)" }}>
              {/* File Header */}
              <div
                className="flex items-center"
                style={{ backgroundColor: isExpanded ? "var(--accent-color)10" : "transparent" }}
              >
                <button
                  onClick={() => toggleFile(fileResult.file)}
                  className="flex-1 text-left p-2 flex items-center gap-2 transition-colors hover:bg-opacity-10 min-w-0"
                  style={{ background: "transparent" }}
                >
                  <span>{isExpanded ? "▼" : "▶"}</span>
                  <span className="font-mono text-sm truncate" style={{ color: "var(--accent-color)" }}>
                    {fileResult.file}
                  </span>
                  <span className="text-xs ml-auto shrink-0" style={{ color: "var(--text-tertiary)" }}>
                    {fileResult.matches.length} matches
                  </span>
                </button>
                <button
                  onClick={() => openInIde(fileResult.file, fileResult.matches[0]?.line_number)}
                  className="p-2 shrink-0 hover:bg-opacity-20"
                  style={{ color: "var(--accent-color)", background: "transparent", border: "none", cursor: "pointer" }}
                  title="Open in IDE"
                  aria-label={`Open ${fileResult.file} in IDE`}
                >
                  <ExternalLink size={12} />
                </button>
              </div>

              {/* Match List */}
              {isExpanded && (
                <div className="border-t" style={{ borderColor: "var(--border-primary)" }}>
                  {fileResult.matches.map((match, idx) => (
                    <button
                      key={idx}
                      onClick={() => openInIde(fileResult.file, match.line_number)}
                      className="w-full text-left p-2 border-b last:border-b-0 hover:brightness-125 transition"
                      style={{
                        borderColor: "var(--border-secondary)",
                        backgroundColor: "#00000020",
                        background: "transparent",
                        cursor: "pointer",
                      }}
                      title="Open at this line in IDE"
                    >
                      <div className="text-xs mb-1" style={{ color: "var(--text-tertiary)" }}>
                        Line {match.line_number}
                      </div>
                      <code className="block font-mono text-sm" style={{ color: "var(--text-primary)" }}>
                        {highlightMatch(match.line_content, match.match_positions)}
                      </code>
                    </button>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
