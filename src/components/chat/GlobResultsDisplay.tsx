import { useState } from "react";
import { formatDistanceToNow } from "date-fns";
import { GlobOutput, formatFileSize } from "../../utils/toolOutputParser";

interface GlobResultsDisplayProps {
  data: GlobOutput;
}

type SortKey = "name" | "size" | "modified";

export function GlobResultsDisplay({ data }: GlobResultsDisplayProps) {
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDesc, setSortDesc] = useState(false);
  const [filter, setFilter] = useState("");

  const handleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDesc(!sortDesc);
    } else {
      setSortKey(key);
      setSortDesc(false);
    }
  };

  const sortedAndFiltered = [...data.files]
    .filter((file) => {
      if (!filter) return true;
      return file.path.toLowerCase().includes(filter.toLowerCase());
    })
    .sort((a, b) => {
      let comparison = 0;
      if (sortKey === "name") {
        comparison = a.path.localeCompare(b.path);
      } else if (sortKey === "size") {
        comparison = a.size_bytes - b.size_bytes;
      } else if (sortKey === "modified") {
        comparison = a.modified_at.localeCompare(b.modified_at);
      }
      return sortDesc ? -comparison : comparison;
    });

  return (
    <div className="rounded p-3" style={{ backgroundColor: "var(--surface-bg)", border: "1px solid var(--border-primary)" }}>
      {/* Summary */}
      <div className="mb-3" style={{ color: "var(--text-secondary)" }}>
        <div className="font-semibold mb-1">Pattern: <code style={{ backgroundColor: "#ffffff10", padding: "2px 6px", borderRadius: "3px" }}>{data.pattern}</code></div>
        <div className="text-sm">
          Found <strong>{data.total_files}</strong> files
        </div>
      </div>

      {/* Filter */}
      <div className="mb-2">
        <input
          type="text"
          placeholder="Filter by name..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="w-full px-3 py-1 rounded text-sm"
          style={{
            backgroundColor: "var(--input-bg)",
            border: "1px solid var(--border-primary)",
            color: "var(--text-primary)",
          }}
        />
      </div>

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr style={{ borderBottom: "1px solid var(--border-primary)" }}>
              <th
                className="text-left p-2 cursor-pointer select-none"
                onClick={() => handleSort("name")}
                style={{ color: "var(--text-secondary)" }}
              >
                Name {sortKey === "name" && (sortDesc ? "↓" : "↑")}
              </th>
              <th
                className="text-right p-2 cursor-pointer select-none"
                onClick={() => handleSort("size")}
                style={{ color: "var(--text-secondary)" }}
              >
                Size {sortKey === "size" && (sortDesc ? "↓" : "↑")}
              </th>
              <th
                className="text-right p-2 cursor-pointer select-none"
                onClick={() => handleSort("modified")}
                style={{ color: "var(--text-secondary)" }}
              >
                Modified {sortKey === "modified" && (sortDesc ? "↓" : "↑")}
              </th>
            </tr>
          </thead>
          <tbody>
            {sortedAndFiltered.map((file, idx) => (
              <tr
                key={idx}
                className="border-b transition-colors hover:bg-opacity-5"
                style={{ borderColor: "var(--border-secondary)" }}
              >
                <td className="p-2">
                  <div className="flex items-center gap-2">
                    <span>{file.is_directory ? "📁" : "📄"}</span>
                    <span className="font-mono text-xs" style={{ color: "var(--accent-color)" }}>
                      {file.path}
                    </span>
                  </div>
                </td>
                <td className="p-2 text-right font-mono text-xs" style={{ color: "var(--text-tertiary)" }}>
                  {file.is_directory ? "—" : formatFileSize(file.size_bytes)}
                </td>
                <td className="p-2 text-right text-xs" style={{ color: "var(--text-tertiary)" }}>
                  {file.modified_at !== "unknown"
                    ? formatDistanceToNow(new Date(file.modified_at), { addSuffix: true })
                    : "unknown"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {sortedAndFiltered.length === 0 && (
        <div className="text-center py-4 text-sm" style={{ color: "var(--text-tertiary)" }}>
          No files match the filter
        </div>
      )}
    </div>
  );
}
