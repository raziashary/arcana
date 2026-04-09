import { useState } from "react";
import { GrepOutput } from "../../utils/toolOutputParser";

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
              <button
                onClick={() => toggleFile(fileResult.file)}
                className="w-full text-left p-2 flex items-center gap-2 transition-colors hover:bg-opacity-10"
                style={{ backgroundColor: isExpanded ? "var(--accent-color)10" : "transparent" }}
              >
                <span>{isExpanded ? "▼" : "▶"}</span>
                <span className="font-mono text-sm" style={{ color: "var(--accent-color)" }}>
                  {fileResult.file}
                </span>
                <span className="text-xs ml-auto" style={{ color: "var(--text-tertiary)" }}>
                  {fileResult.matches.length} matches
                </span>
              </button>

              {/* Match List */}
              {isExpanded && (
                <div className="border-t" style={{ borderColor: "var(--border-primary)" }}>
                  {fileResult.matches.map((match, idx) => (
                    <div
                      key={idx}
                      className="p-2 border-b last:border-b-0"
                      style={{ borderColor: "var(--border-secondary)", backgroundColor: "#00000020" }}
                    >
                      <div className="text-xs mb-1" style={{ color: "var(--text-tertiary)" }}>
                        Line {match.line_number}
                      </div>
                      <code className="block font-mono text-sm" style={{ color: "var(--text-primary)" }}>
                        {highlightMatch(match.line_content, match.match_positions)}
                      </code>
                    </div>
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
