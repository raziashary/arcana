// validate.rs — Shared file-validation module used by write_file, edit_file, and check_file.
//
// Three tiers of checking:
//   Tier 1 — fast syntax: in-process (JSON, YAML, TOML) or lightweight subprocess
//             (py_compile, node --check). Always runs after file writes (Option A).
//   Tier 2 — import cross-reference: pure Rust, no subprocess. Walks the project
//             tree to find requirements.txt / package.json and verifies that every
//             import appears there or in the language stdlib.
//   Tier 3 — slow type checking: mypy / pyright / tsc / eslint via subprocess.
//             On-demand only (Option B, check_file tool).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub severity: &'static str, // "error" | "warning"
    pub message:  String,
    pub line:     Option<u32>,
}

#[derive(Debug, Clone)]
pub struct LintResult {
    pub checker:     String,
    pub diagnostics: Vec<LintDiagnostic>,
}

impl LintResult {
    pub fn clean(checker: impl Into<String>) -> Self {
        Self { checker: checker.into(), diagnostics: vec![] }
    }

    pub fn is_clean(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.severity == "error")
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == "error").count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == "warning").count()
    }

    /// Compact multi-line string appended to tool responses.
    /// Returns empty string when there are no diagnostics (clean file).
    pub fn format_inline(&self) -> String {
        if self.diagnostics.is_empty() {
            return String::new();
        }
        let mut out = format!(
            "[{}] {} error(s), {} warning(s):",
            self.checker,
            self.error_count(),
            self.warning_count()
        );
        for d in &self.diagnostics {
            let loc = d.line.map(|l| format!(" line {l}")).unwrap_or_default();
            out.push_str(&format!("\n  {}{}: {}", d.severity.to_uppercase(), loc, d.message));
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Third-party imports that are not declared in the manifest or stdlib.
    pub missing: Vec<String>,
    pub checker: String,
}

impl ImportResult {
    pub fn is_clean(&self) -> bool { self.missing.is_empty() }

    /// Compact one-liner for tool responses. Returns empty string when clean.
    pub fn format_inline(&self) -> String {
        if self.missing.is_empty() {
            return String::new();
        }
        format!(
            "[{}] {} undeclared import(s): {}",
            self.checker,
            self.missing.len(),
            self.missing.join(", ")
        )
    }
}

// ─── Tier 1 — Syntax validation ───────────────────────────────────────────────

/// Run a fast syntax check appropriate for the file's extension.
/// Returns `None` if the file type is unsupported (no check = no noise).
/// Returns `Some(LintResult)` with `.diagnostics` empty when the file is clean.
pub async fn tier1_syntax(path: &Path, working_dir: &str) -> Option<LintResult> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    match ext.as_str() {
        "json"            => Some(check_json(path)),
        "yaml" | "yml"    => Some(check_yaml(path)),
        "toml"            => Some(check_toml(path)),
        "py"              => Some(check_python_syntax(path, working_dir).await),
        "js" | "mjs" | "cjs" => Some(check_node_syntax(path).await),
        // TypeScript is skipped at Tier 1 — node --check rejects TS syntax.
        // Tier 3 (tsc --noEmit) handles TypeScript type + syntax checking.
        _ => None,
    }
}

fn check_json(path: &Path) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Err(e) => return LintResult {
            checker: "json_parse".into(),
            diagnostics: vec![LintDiagnostic { severity: "error", message: e.to_string(), line: None }],
        },
        Ok(c) => c,
    };
    match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(_)  => LintResult::clean("json_parse"),
        Err(e) => LintResult {
            checker: "json_parse".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "error",
                message:  e.to_string(),
                line:     Some(e.line() as u32),
            }],
        },
    }
}

fn check_yaml(path: &Path) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Err(e) => return LintResult {
            checker: "yaml_parse".into(),
            diagnostics: vec![LintDiagnostic { severity: "error", message: e.to_string(), line: None }],
        },
        Ok(c) => c,
    };
    // Use serde_json::Value as target — any valid YAML maps to it and gives us
    // a parse error with location info when the YAML is malformed.
    match serde_yaml::from_str::<serde_json::Value>(&content) {
        Ok(_)  => LintResult::clean("yaml_parse"),
        Err(e) => {
            let line = e.location().map(|loc| loc.line() as u32);
            LintResult {
                checker: "yaml_parse".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message:  e.to_string(),
                    line,
                }],
            }
        }
    }
}

fn check_toml(path: &Path) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Err(e) => return LintResult {
            checker: "toml_parse".into(),
            diagnostics: vec![LintDiagnostic { severity: "error", message: e.to_string(), line: None }],
        },
        Ok(c) => c,
    };
    // toml::Value::try_from(str) parses the whole document.
    match content.parse::<toml::Value>() {
        Ok(_)  => LintResult::clean("toml_parse"),
        Err(e) => LintResult {
            checker: "toml_parse".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "error",
                // toml error messages already include the line/column inline.
                message:  e.to_string(),
                line:     None,
            }],
        },
    }
}

async fn check_python_syntax(path: &Path, working_dir: &str) -> LintResult {
    let python = resolve_python(working_dir);
    let output = tokio::process::Command::new(&python)
        .args(["-m", "py_compile", &path.to_string_lossy()])
        .output()
        .await;

    match output {
        Err(_) => LintResult {
            checker: "py_compile".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "warning",
                message:  format!(
                    "Could not run '{python}' — Python not available, syntax not checked. \
                     Install Python to enable automatic validation."
                ),
                line: None,
            }],
        },
        Ok(out) if out.status.success() => LintResult::clean("py_compile"),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let line   = parse_python_error_line(&stderr);
            LintResult {
                checker: "py_compile".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message:  stderr.trim().to_string(),
                    line,
                }],
            }
        }
    }
}

async fn check_node_syntax(path: &Path) -> LintResult {
    // `node --check` validates syntax without executing the file.
    let output = tokio::process::Command::new("node")
        .args(["--check", &path.to_string_lossy()])
        .output()
        .await;

    match output {
        Err(_) => LintResult {
            checker: "node_check".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "warning",
                message:  "Could not run 'node' — Node.js not available, syntax not checked. \
                           Install Node.js to enable automatic validation."
                    .into(),
                line: None,
            }],
        },
        Ok(out) if out.status.success() => LintResult::clean("node_check"),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let line   = parse_node_error_line(&stderr);
            LintResult {
                checker: "node_check".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message:  stderr.trim().to_string(),
                    line,
                }],
            }
        }
    }
}

// ─── Tier 2 — Import cross-reference ──────────────────────────────────────────

/// Check that every import in the file is declared in the project manifest or
/// is part of the language's standard library.
/// Returns `None` if the file type is unsupported or no manifest is found.
pub fn tier2_imports(path: &Path, working_dir: &str) -> Option<ImportResult> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    match ext.as_str() {
        "py"  => check_python_imports(path, working_dir),
        "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx"
              => check_js_imports(path, working_dir),
        _     => None,
    }
}

fn check_python_imports(path: &Path, working_dir: &str) -> Option<ImportResult> {
    let content = std::fs::read_to_string(path).ok()?;
    let imports = extract_python_imports(&content);
    if imports.is_empty() { return None; }

    // Walk upward from the file's directory to find requirements.txt.
    let declared: HashSet<String> = path.parent()
        .and_then(|p| find_upward(p, "requirements.txt", working_dir))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| parse_requirements(&s))
        .unwrap_or_default();

    let stdlib = python_stdlib_set();
    let missing: Vec<String> = imports
        .into_iter()
        .filter(|imp| !stdlib.contains(imp.as_str()) && !declared.contains(imp))
        .collect();

    Some(ImportResult { missing, checker: "python_imports".into() })
}

fn check_js_imports(path: &Path, working_dir: &str) -> Option<ImportResult> {
    let content = std::fs::read_to_string(path).ok()?;
    // Extract third-party imports (not relative, not node builtins).
    let third_party: Vec<String> = extract_js_imports(&content)
        .into_iter()
        .filter(|i| !i.starts_with('.') && !i.starts_with('/'))
        .map(|i| {
            // Scope-package: "@scope/name/deep" → "@scope/name"
            // Regular:       "lodash/get"       → "lodash"
            let s = i.as_str();
            if s.starts_with('@') {
                s.splitn(3, '/').take(2).collect::<Vec<_>>().join("/")
            } else {
                s.split('/').next().unwrap_or(s).to_string()
            }
        })
        .collect();

    if third_party.is_empty() { return None; }

    let pkg_file = path.parent()
        .and_then(|p| find_upward(p, "package.json", working_dir))?;
    let pkg_content = std::fs::read_to_string(pkg_file).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&pkg_content).ok()?;

    let mut declared: HashSet<String> = HashSet::new();
    for section in &["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        if let Some(obj) = pkg[section].as_object() {
            declared.extend(obj.keys().cloned());
        }
    }

    // Node built-ins that never appear in package.json.
    let node_builtins = node_builtin_set();

    let missing: Vec<String> = third_party
        .into_iter()
        .filter(|pkg| !declared.contains(pkg) && !node_builtins.contains(pkg.as_str()))
        .collect();

    Some(ImportResult { missing, checker: "js_imports".into() })
}

// ─── Tier 3 — Type checkers ───────────────────────────────────────────────────

/// Run a slow type-checker subprocess appropriate for the file type.
/// `checker` overrides auto-detection (`None` → auto).
pub async fn tier3_types(path: &Path, working_dir: &str, checker: Option<&str>) -> LintResult {
    let ext = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match (ext.as_str(), checker) {
        ("py", Some("mypy"))    => run_mypy(path, working_dir).await,
        ("py", _)               => run_pyright_or_mypy(path, working_dir).await,
        ("ts" | "tsx", _)       => run_tsc(path, working_dir).await,
        ("js" | "jsx", Some("eslint")) | ("js" | "jsx", None)
                                => run_eslint(path, working_dir).await,
        _ => LintResult {
            checker: "tier3".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "warning",
                message:  format!("No type checker configured for .{ext} files."),
                line:     None,
            }],
        },
    }
}

async fn run_pyright_or_mypy(path: &Path, working_dir: &str) -> LintResult {
    // Prefer pyright if available, fall back to mypy.
    let pyright_available = tokio::process::Command::new("pyright")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if pyright_available {
        run_pyright(path, working_dir).await
    } else {
        run_mypy(path, working_dir).await
    }
}

async fn run_mypy(path: &Path, working_dir: &str) -> LintResult {
    run_checker_subprocess(
        "mypy",
        &["--no-error-summary", "--show-column-numbers", &path.to_string_lossy()],
        working_dir,
        "mypy",
    ).await
}

async fn run_pyright(path: &Path, working_dir: &str) -> LintResult {
    run_checker_subprocess(
        "pyright",
        &[&path.to_string_lossy()],
        working_dir,
        "pyright",
    ).await
}

async fn run_tsc(path: &Path, working_dir: &str) -> LintResult {
    let mut args = vec!["--noEmit".to_string(), "--pretty".to_string(), "false".to_string()];

    // Use tsconfig.json if found; otherwise pass the file directly with --allowJs.
    if let Some(ts) = path.parent().and_then(|p| find_upward(p, "tsconfig.json", working_dir)) {
        args.push("--project".to_string());
        args.push(ts.to_string_lossy().to_string());
    } else {
        args.push("--allowJs".to_string());
        args.push(path.to_string_lossy().to_string());
    }

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_checker_subprocess("tsc", &arg_refs, working_dir, "tsc").await
}

async fn run_eslint(path: &Path, working_dir: &str) -> LintResult {
    run_checker_subprocess(
        "eslint",
        &["--format=compact", &path.to_string_lossy()],
        working_dir,
        "eslint",
    ).await
}

async fn run_checker_subprocess(
    bin:        &str,
    args:       &[&str],
    working_dir: &str,
    checker:    &str,
) -> LintResult {
    let result = tokio::process::Command::new(bin)
        .args(args)
        .current_dir(working_dir)
        .output()
        .await;

    match result {
        Err(_) => LintResult {
            checker: checker.to_string(),
            diagnostics: vec![LintDiagnostic {
                severity: "warning",
                message:  format!(
                    "'{bin}' not found — install it to enable type checking. \
                     (e.g. `pip install {bin}` or `npm install -g {bin}`)"
                ),
                line: None,
            }],
        },
        Ok(out) if out.status.success() => LintResult::clean(checker),
        Ok(out) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            );
            // Cap output — type checkers can produce thousands of lines.
            const MAX_CHECKER_BYTES: usize = 4_000;
            let truncated = if combined.len() > MAX_CHECKER_BYTES {
                format!(
                    "[output truncated — showing last {} bytes]\n{}",
                    MAX_CHECKER_BYTES,
                    &combined[combined.len() - MAX_CHECKER_BYTES..]
                )
            } else {
                combined
            };
            LintResult {
                checker: checker.to_string(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message:  truncated.trim().to_string(),
                    line:     None,
                }],
            }
        }
    }
}

// ─── Helpers — Python / Node resolution ──────────────────────────────────────

/// Return the Python binary path, preferring a virtual-environment interpreter
/// over the system Python so that project dependencies are on sys.path.
pub fn resolve_python(working_dir: &str) -> String {
    let candidates: Vec<String> = if cfg!(target_os = "windows") {
        vec![
            format!("{working_dir}\\.venv\\Scripts\\python.exe"),
            format!("{working_dir}\\venv\\Scripts\\python.exe"),
            "python".into(),
        ]
    } else {
        vec![
            format!("{working_dir}/.venv/bin/python3"),
            format!("{working_dir}/venv/bin/python3"),
            "python3".into(),
            "python".into(),
        ]
    };

    for c in candidates {
        // Absolute paths: check existence. System names: let OS find them.
        if Path::new(&c).is_absolute() {
            if Path::new(&c).exists() { return c; }
        } else {
            return c;
        }
    }
    "python3".into()
}

/// Walk from `start` upward toward the filesystem root, but stop at
/// `working_dir`. Returns the first file named `filename` found.
pub fn find_upward(start: &Path, filename: &str, working_dir: &str) -> Option<PathBuf> {
    let root = PathBuf::from(working_dir);
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(filename);
        if candidate.exists() { return Some(candidate); }
        if current == root { break; }
        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => break,
        }
    }
    None
}

// ─── Helpers — Error-line extraction ─────────────────────────────────────────

fn parse_python_error_line(stderr: &str) -> Option<u32> {
    // py_compile writes: `  File "foo.py", line 5`
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("File ") {
            if let Some(pos) = trimmed.find(", line ") {
                return trimmed[pos + 7..].split_whitespace().next()?.parse().ok();
            }
        }
    }
    None
}

fn parse_node_error_line(stderr: &str) -> Option<u32> {
    // node errors: `/path/to/file.js:5\n  ^\nSyntaxError: ...`
    for line in stderr.lines() {
        // Last colon segment is often `column`; second-to-last is `line`.
        // Simplified: grab the number after the first `:NN` we see.
        if let Some(rest) = line.strip_prefix('/').or_else(|| line.strip_prefix('C')) {
            // looks like a file path — grab the line number after the path
            let parts: Vec<&str> = line.rsplitn(3, ':').collect();
            if parts.len() >= 2 {
                if let Ok(n) = parts[parts.len() - 2].trim().parse::<u32>() {
                    return Some(n);
                }
            }
            let _ = rest; // suppress unused warning
        }
    }
    None
}

// ─── Helpers — Import extraction ─────────────────────────────────────────────

fn extract_python_imports(content: &str) -> Vec<String> {
    // Matches: `import os`, `import os.path`, `from os import path`, `from pathlib import Path`
    let re_import = regex::Regex::new(r"(?m)^import\s+([\w]+)").unwrap();
    let re_from   = regex::Regex::new(r"(?m)^from\s+([\w][.\w]*)").unwrap();
    let mut out: Vec<String> = Vec::new();

    for cap in re_import.captures_iter(content) {
        out.push(cap[1].to_string());
    }
    for cap in re_from.captures_iter(content) {
        // Take the top-level package only: "os.path" → "os"
        let pkg = cap[1].split('.').next().unwrap_or(&cap[1]).to_string();
        if !pkg.is_empty() { out.push(pkg); }
    }
    out.sort();
    out.dedup();
    out
}

fn extract_js_imports(content: &str) -> Vec<String> {
    // ESM:     import X from 'pkg'
    //          import { X } from 'pkg'
    //          import 'pkg'
    //          export { X } from 'pkg'
    // CJS:     require('pkg')
    let re_esm = regex::Regex::new(r#"(?:^|\n)\s*(?:import|export)[^'"]*['"]([^'"]+)['"]"#).unwrap();
    let re_cjs = regex::Regex::new(r#"require\(['"]([^'"]+)['"]\)"#).unwrap();
    let mut out: Vec<String> = Vec::new();

    for cap in re_esm.captures_iter(content) { out.push(cap[1].to_string()); }
    for cap in re_cjs.captures_iter(content) { out.push(cap[1].to_string()); }
    out.sort();
    out.dedup();
    out
}

fn parse_requirements(content: &str) -> HashSet<String> {
    content.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
        .map(|l| {
            // Strip version specifier: "requests>=2.0" → "requests"
            let name = l.split(&['=', '>', '<', '!', '[', ';', ' '][..])
                .next()
                .unwrap_or(l)
                .trim()
                .to_lowercase();
            // Normalize: replace hyphens with underscores (pip-normalised name)
            name.replace('-', "_")
        })
        .collect()
}

// ─── Language built-in sets ──────────────────────────────────────────────────

fn python_stdlib_set() -> HashSet<&'static str> {
    // Python 3.10+ stdlib modules (commonly imported subset).
    [
        "__future__", "__main__", "abc", "aifc", "argparse", "array", "ast",
        "asynchat", "asyncio", "asyncore", "atexit", "audioop", "base64",
        "bdb", "binascii", "binhex", "bisect", "builtins", "bz2", "calendar",
        "cgi", "cgitb", "chunk", "cmath", "cmd", "code", "codecs", "codeop",
        "collections", "colorsys", "compileall", "concurrent", "configparser",
        "contextlib", "contextvars", "copy", "copyreg", "cProfile", "csv",
        "ctypes", "curses", "dataclasses", "datetime", "dbm", "decimal",
        "difflib", "dis", "doctest", "email", "encodings", "enum", "errno",
        "faulthandler", "fcntl", "filecmp", "fileinput", "fnmatch", "fractions",
        "ftplib", "functools", "gc", "getopt", "getpass", "gettext", "glob",
        "grp", "gzip", "hashlib", "heapq", "hmac", "html", "http", "idlelib",
        "imaplib", "imghdr", "imp", "importlib", "inspect", "io", "ipaddress",
        "itertools", "json", "keyword", "lib2to3", "linecache", "locale",
        "logging", "lzma", "mailbox", "mailcap", "marshal", "math", "mimetypes",
        "mmap", "modulefinder", "multiprocessing", "netrc", "nis", "nntplib",
        "numbers", "operator", "optparse", "os", "ossaudiodev", "pathlib",
        "pdb", "pickle", "pickletools", "pipes", "pkgutil", "platform",
        "plistlib", "poplib", "posix", "posixpath", "pprint", "profile",
        "pstats", "pty", "pwd", "py_compile", "pyclbr", "pydoc", "queue",
        "quopri", "random", "re", "readline", "reprlib", "resource", "rlcompleter",
        "runpy", "sched", "secrets", "select", "selectors", "shelve", "shlex",
        "shutil", "signal", "site", "smtpd", "smtplib", "sndhdr", "socket",
        "socketserver", "spwd", "sqlite3", "sre_compile", "sre_constants",
        "sre_parse", "ssl", "stat", "statistics", "string", "stringprep",
        "struct", "subprocess", "sunau", "symtable", "sys", "sysconfig",
        "syslog", "tabnanny", "tarfile", "telnetlib", "tempfile", "termios",
        "test", "textwrap", "threading", "time", "timeit", "tkinter",
        "token", "tokenize", "tomllib", "trace", "traceback", "tracemalloc",
        "tty", "turtle", "turtledemo", "types", "typing", "unicodedata",
        "unittest", "urllib", "uu", "uuid", "venv", "warnings", "wave",
        "weakref", "webbrowser", "winreg", "winsound", "wsgiref", "xdrlib",
        "xml", "xmlrpc", "zipapp", "zipfile", "zipimport", "zlib",
        "zoneinfo",
        // Common type-checking extras that aren't truly third-party
        "typing_extensions",
        // Popular packages often treated as stdlib by agents
        "pathspec", "abc",
    ].into()
}

fn node_builtin_set() -> HashSet<&'static str> {
    // Node.js built-in modules (available without npm install).
    [
        "assert", "async_hooks", "buffer", "child_process", "cluster",
        "console", "constants", "crypto", "dgram", "diagnostics_channel",
        "dns", "domain", "events", "fs", "http", "http2", "https", "inspector",
        "module", "net", "os", "path", "perf_hooks", "process", "punycode",
        "querystring", "readline", "repl", "stream", "string_decoder",
        "sys", "timers", "tls", "trace_events", "tty", "url", "util",
        "v8", "vm", "wasi", "worker_threads", "zlib",
        // node: prefixed variants
        "node:assert", "node:buffer", "node:child_process", "node:cluster",
        "node:console", "node:crypto", "node:dgram", "node:dns", "node:events",
        "node:fs", "node:http", "node:http2", "node:https", "node:module",
        "node:net", "node:os", "node:path", "node:process", "node:querystring",
        "node:readline", "node:stream", "node:tls", "node:url", "node:util",
        "node:v8", "node:vm", "node:worker_threads", "node:zlib",
    ].into()
}
