export interface GrepMatch {
  line_number: number;
  line_content: string;
  match_positions: [number, number][];
}

export interface GrepFileResult {
  file: string;
  matches: GrepMatch[];
}

export interface GrepOutput {
  pattern: string;
  total_matches: number;
  files_matched: number;
  results: GrepFileResult[];
}

export interface GlobFileEntry {
  path: string;
  size_bytes: number;
  modified_at: string;
  is_directory: boolean;
}

export interface GlobOutput {
  pattern: string;
  total_files: number;
  files: GlobFileEntry[];
}

export interface BashOutput {
  command: string;
  exit_code: number;
  stdout: string;
  stderr: string;
  execution_time_ms?: number;
}

export interface FileContent {
  path: string;
  start_line?: number;
  end_line?: number;
  content: string;
  language?: string;
}

export type ToolOutputType =
  | { type: "grep"; data: GrepOutput }
  | { type: "glob"; data: GlobOutput }
  | { type: "bash"; data: BashOutput }
  | { type: "file_content"; data: FileContent }
  | { type: "text"; data: string };

export function parseToolOutput(
  toolName: string,
  output: string
): ToolOutputType {
  try {
    switch (toolName) {
      case "grep":
        const grepData = JSON.parse(output) as GrepOutput;
        return { type: "grep", data: grepData };

      case "glob":
        const globData = JSON.parse(output) as GlobOutput;
        return { type: "glob", data: globData };

      case "bash":
        try {
          const bashData = JSON.parse(output) as BashOutput;
          return { type: "bash", data: bashData };
        } catch {
          return { type: "bash", data: { command: "", exit_code: 0, stdout: output, stderr: "" } };
        }

      case "read_file":
        return { type: "file_content", data: { path: "", content: output } };

      default:
        return { type: "text", data: output };
    }
  } catch (error) {
    return { type: "text", data: output };
  }
}

export function detectLanguageFromPath(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase();
  const langMap: Record<string, string> = {
    rs: "rust",
    ts: "typescript",
    tsx: "tsx",
    js: "javascript",
    jsx: "jsx",
    py: "python",
    md: "markdown",
    json: "json",
    toml: "toml",
    yaml: "yaml",
    yml: "yaml",
    sh: "bash",
    css: "css",
    html: "html",
    sql: "sql",
    go: "go",
    java: "java",
    c: "c",
    cpp: "cpp",
    h: "c",
    hpp: "cpp",
  };
  return langMap[ext || ""] || "text";
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
