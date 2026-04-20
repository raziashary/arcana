import { Highlight, themes } from "prism-react-renderer";
import { ExternalLink } from "lucide-react";
import { FileContent, detectLanguageFromPath } from "../../utils/toolOutputParser";
import { openInIde } from "../../utils/openInIde";

interface FileContentDisplayProps {
  data: FileContent;
}

export function FileContentDisplay({ data }: FileContentDisplayProps) {
  const language = data.language || detectLanguageFromPath(data.path);

  const handleCopy = () => {
    navigator.clipboard.writeText(data.content);
  };

  const handleOpenInIde = () => openInIde(data.path, data.start_line ?? undefined);

  return (
    <div className="rounded" style={{ backgroundColor: "var(--surface-bg)", border: "1px solid var(--border-primary)" }}>
      {/* Header */}
      <div className="flex items-center justify-between p-2 border-b" style={{ borderColor: "var(--border-primary)" }}>
        <div className="flex items-center gap-2 min-w-0">
          <button
            onClick={handleOpenInIde}
            className="font-mono text-sm truncate hover:underline"
            style={{ color: "var(--accent-color)", background: "transparent", border: "none", padding: 0, cursor: "pointer" }}
            title="Open in IDE"
          >
            {data.path}
          </button>
          {data.start_line && data.end_line && (
            <span className="text-xs" style={{ color: "var(--text-tertiary)" }}>
              :{data.start_line}-{data.end_line}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <button
            onClick={handleOpenInIde}
            className="flex items-center gap-1 px-2 py-1 text-xs rounded transition-colors"
            style={{
              backgroundColor: "var(--accent-color)20",
              color: "var(--accent-color)",
              border: "1px solid var(--accent-color)40",
            }}
            title="Open in IDE tab"
          >
            <ExternalLink size={11} /> Open in IDE
          </button>
          <button
            onClick={handleCopy}
            className="px-2 py-1 text-xs rounded transition-colors"
            style={{
              backgroundColor: "var(--accent-color)20",
              color: "var(--accent-color)",
              border: "1px solid var(--accent-color)40",
            }}
          >
            Copy
          </button>
        </div>
      </div>

      {/* Code Block */}
      <div className="overflow-x-auto">
        <Highlight theme={themes.vsDark} code={data.content} language={language as any}>
          {({ style, tokens, getLineProps, getTokenProps }) => (
            <pre style={{ ...style, margin: 0, padding: "12px", fontSize: "13px", lineHeight: "1.6" }}>
              {tokens.map((line, i) => {
                const lineNumber = (data.start_line || 1) + i;
                return (
                  <div key={i} {...getLineProps({ line })} style={{ display: "flex" }}>
                    <span
                      style={{
                        userSelect: "none",
                        paddingRight: "16px",
                        color: "var(--text-tertiary)",
                        minWidth: "40px",
                        textAlign: "right",
                      }}
                    >
                      {lineNumber}
                    </span>
                    <span style={{ flex: 1 }}>
                      {line.map((token, key) => (
                        <span key={key} {...getTokenProps({ token })} />
                      ))}
                    </span>
                  </div>
                );
              })}
            </pre>
          )}
        </Highlight>
      </div>
    </div>
  );
}
