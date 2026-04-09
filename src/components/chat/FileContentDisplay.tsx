import { Highlight, themes } from "prism-react-renderer";
import { FileContent, detectLanguageFromPath } from "../../utils/toolOutputParser";

interface FileContentDisplayProps {
  data: FileContent;
}

export function FileContentDisplay({ data }: FileContentDisplayProps) {
  const language = data.language || detectLanguageFromPath(data.path);

  const handleCopy = () => {
    navigator.clipboard.writeText(data.content);
  };

  return (
    <div className="rounded" style={{ backgroundColor: "var(--surface-bg)", border: "1px solid var(--border-primary)" }}>
      {/* Header */}
      <div className="flex items-center justify-between p-2 border-b" style={{ borderColor: "var(--border-primary)" }}>
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm" style={{ color: "var(--accent-color)" }}>
            {data.path}
          </span>
          {data.start_line && data.end_line && (
            <span className="text-xs" style={{ color: "var(--text-tertiary)" }}>
              :{data.start_line}-{data.end_line}
            </span>
          )}
        </div>
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
