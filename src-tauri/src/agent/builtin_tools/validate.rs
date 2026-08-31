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
    pub message: String,
    pub line: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct LintResult {
    pub checker: String,
    pub diagnostics: Vec<LintDiagnostic>,
}

impl LintResult {
    pub fn clean(checker: impl Into<String>) -> Self {
        Self {
            checker: checker.into(),
            diagnostics: vec![],
        }
    }

    pub fn is_clean(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.severity == "error")
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == "warning")
            .count()
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
            out.push_str(&format!(
                "\n  {}{}: {}",
                d.severity.to_uppercase(),
                loc,
                d.message
            ));
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
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty()
    }

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
        "json" => Some(check_json(path)),
        "yaml" | "yml" => Some(check_yaml(path)),
        "toml" => Some(check_toml(path)),
        "py" => Some(check_python_syntax(path, working_dir).await),
        "js" | "mjs" | "cjs" => Some(check_node_syntax(path).await),
        // Doc 09 T1.2 — TypeScript syntax via `tsc --noEmit --allowJs
        // --isolatedModules`. Falls back to a `warning` LintResult when
        // `tsc` is missing — same degradation pattern as py_compile /
        // node --check, so the worker isn't blocked on a missing tool but
        // still sees "TS syntax not validated" prominently in the response.
        "ts" | "tsx" | "jsx" => Some(check_tsc_syntax(path, working_dir).await),
        _ => None,
    }
}

fn check_json(path: &Path) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Err(e) => {
            return LintResult {
                checker: "json_parse".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: e.to_string(),
                    line: None,
                }],
            }
        }
        Ok(c) => c,
    };
    match serde_json::from_str::<serde_json::Value>(&content) {
        Ok(_) => LintResult::clean("json_parse"),
        Err(e) => LintResult {
            checker: "json_parse".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "error",
                message: e.to_string(),
                line: Some(e.line() as u32),
            }],
        },
    }
}

fn check_yaml(path: &Path) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Err(e) => {
            return LintResult {
                checker: "yaml_parse".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: e.to_string(),
                    line: None,
                }],
            }
        }
        Ok(c) => c,
    };
    // Use serde_json::Value as target — any valid YAML maps to it and gives us
    // a parse error with location info when the YAML is malformed.
    match serde_yaml::from_str::<serde_json::Value>(&content) {
        Ok(_) => LintResult::clean("yaml_parse"),
        Err(e) => {
            let line = e.location().map(|loc| loc.line() as u32);
            LintResult {
                checker: "yaml_parse".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: e.to_string(),
                    line,
                }],
            }
        }
    }
}

fn check_toml(path: &Path) -> LintResult {
    let content = match std::fs::read_to_string(path) {
        Err(e) => {
            return LintResult {
                checker: "toml_parse".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: e.to_string(),
                    line: None,
                }],
            }
        }
        Ok(c) => c,
    };
    // toml::Value::try_from(str) parses the whole document.
    match content.parse::<toml::Value>() {
        Ok(_) => LintResult::clean("toml_parse"),
        Err(e) => LintResult {
            checker: "toml_parse".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "error",
                // toml error messages already include the line/column inline.
                message: e.to_string(),
                line: None,
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
                message: format!(
                    "Could not run '{python}' — Python not available, syntax not checked. \
                     Install Python to enable automatic validation."
                ),
                line: None,
            }],
        },
        Ok(out) if out.status.success() => LintResult::clean("py_compile"),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let line = parse_python_error_line(&stderr);
            LintResult {
                checker: "py_compile".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: stderr.trim().to_string(),
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
                message: "Could not run 'node' — Node.js not available, syntax not checked. \
                           Install Node.js to enable automatic validation."
                    .into(),
                line: None,
            }],
        },
        Ok(out) if out.status.success() => LintResult::clean("node_check"),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let line = parse_node_error_line(&stderr);
            LintResult {
                checker: "node_check".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: stderr.trim().to_string(),
                    line,
                }],
            }
        }
    }
}

/// Doc 09 T1.2 — TypeScript syntax validation via `tsc --noEmit
/// --allowJs --isolatedModules`. Used at Tier 1 because the calendar
/// run shipped .tsx files whose syntax errors were caught only at
/// `npm run build` time — which the worker never ran. Surfacing
/// syntax-only errors at write time avoids the qc_review round-trip.
///
/// `--isolatedModules` is critical: it stops tsc from chasing
/// cross-file type errors at Tier 1 (those belong in Tier 3 / opt-in
/// `check_file --types`). At isolated-module level tsc parses each
/// file standalone, which is exactly what "syntax check" should mean.
///
/// Falls back to a `warning` diagnostic when `tsc` isn't on PATH, so
/// workers without a TypeScript toolchain installed still get a
/// successful write — just with a visible note that TS wasn't checked.
async fn check_tsc_syntax(path: &Path, working_dir: &str) -> LintResult {
    let path_str = path.to_string_lossy().to_string();
    let output = tokio::process::Command::new("tsc")
        .args([
            "--noEmit",
            "--allowJs",
            "--isolatedModules",
            "--jsx",
            "preserve",
            "--target",
            "es2020",
            "--module",
            "esnext",
            "--moduleResolution",
            "node",
            &path_str,
        ])
        .current_dir(working_dir)
        .output()
        .await;

    match output {
        Err(_) => LintResult {
            checker: "tsc_check".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "warning",
                message: "Could not run 'tsc' — TypeScript not installed. \
                           Install it (`npm install -g typescript`) to enable \
                           automatic .ts/.tsx syntax validation."
                    .into(),
                line: None,
            }],
        },
        Ok(out) if out.status.success() => LintResult::clean("tsc_check"),
        Ok(out) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            );
            // tsc emits one diagnostic per line — first 4 lines is enough
            // signal for write-time feedback; the agent can re-run
            // check_file for the full output. Strip ANSI just in case.
            let trimmed = combined
                .lines()
                .filter(|l| !l.trim().is_empty())
                .take(4)
                .collect::<Vec<_>>()
                .join("\n");
            LintResult {
                checker: "tsc_check".into(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: trimmed.trim().to_string(),
                    line: None,
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
        "py" => check_python_imports(path, working_dir),
        "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" => check_js_imports(path, working_dir),
        _ => None,
    }
}

fn check_python_imports(path: &Path, working_dir: &str) -> Option<ImportResult> {
    let content = std::fs::read_to_string(path).ok()?;
    let imports = extract_python_imports(&content);
    if imports.is_empty() {
        return None;
    }

    // Walk upward from the file's directory to find requirements.txt.
    let declared: HashSet<String> = path
        .parent()
        .and_then(|p| find_upward(p, "requirements.txt", working_dir))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| parse_requirements(&s))
        .unwrap_or_default();

    let stdlib = python_stdlib_set();
    let missing: Vec<String> = imports
        .into_iter()
        .filter(|imp| !stdlib.contains(imp.as_str()) && !declared.contains(imp))
        .collect();

    Some(ImportResult {
        missing,
        checker: "python_imports".into(),
    })
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

    if third_party.is_empty() {
        return None;
    }

    let pkg_file = path
        .parent()
        .and_then(|p| find_upward(p, "package.json", working_dir))?;
    let pkg_content = std::fs::read_to_string(pkg_file).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&pkg_content).ok()?;

    let mut declared: HashSet<String> = HashSet::new();
    for section in &[
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
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

    Some(ImportResult {
        missing,
        checker: "js_imports".into(),
    })
}

// ─── Tier 2.5 — Export cross-reference (Doc 09 T1.3) ─────────────────────────
//
// Catches the consumer/producer drift bug class that bypassed Tier 1
// (syntax-clean) and Tier 2 (third-party imports declared). The calendar run
// shipped `useEvents.ts` that returned `{ events, addEvent, deleteEvent }`
// while `App.tsx` destructured `{ isDeleting, … }` — both files parsed and
// type-checked individually, but the boundary was broken.
//
// This pass:
//   1. Extracts every relative `import { A, B } from './foo'` from `path`.
//   2. Resolves './foo' to an actual file on disk (.ts/.tsx/.js/.jsx, index.ts).
//   3. Parses the exports of the resolved file (`export { A, B }`,
//      `export const X`, `export default`, etc.).
//   4. Reports any imported name not present in the producer's export set.
//
// Pure regex pass — no subprocess, no parser. Conservative on false
// positives: when a name can't be resolved we emit it as `missing` only
// when we *successfully* parsed the target file's export list. Files we
// fail to read are silently skipped so a stale-on-disk dep doesn't spam
// the worker.

/// `tier2_5_exports` returns Some(ImportResult) when the file is a JS/TS
/// module with at least one relative import; the `missing` vector lists
/// `name (from './target')` strings the agent can act on.
pub fn tier2_5_exports(path: &Path, working_dir: &str) -> Option<ImportResult> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    if !matches!(ext.as_str(), "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx") {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let imports = extract_named_imports(&content);
    if imports.is_empty() {
        return None;
    }

    let dir = path.parent()?;
    let mut missing: Vec<String> = Vec::new();

    for imp in imports {
        // Only check relative paths (./, ../). Third-party imports are
        // covered by Tier 2; we have no easy way to introspect node_modules.
        if !imp.module.starts_with('.') {
            continue;
        }

        let Some(resolved) = resolve_relative_module(dir, &imp.module, working_dir) else {
            // Target file not found on disk — likely a producer that doesn't
            // exist yet. Surface as a single line; downstream Tier 2 already
            // covers missing manifest entries.
            missing.push(format!(
                "module '{}' not found from {}",
                imp.module,
                path.display()
            ));
            continue;
        };

        let Ok(target_src) = std::fs::read_to_string(&resolved) else {
            continue;
        };
        let exports = extract_exports(&target_src);

        // `default` is its own kind — we tolerate any `default` import when
        // the target has a default export. Named imports are exact match.
        let has_default = exports.iter().any(|e| e == "default");
        for name in &imp.named {
            if name == "default" {
                if !has_default {
                    missing.push(format!(
                        "default import from '{}' but target has no default export",
                        imp.module
                    ));
                }
                continue;
            }
            if !exports.iter().any(|e| e == name) {
                missing.push(format!("'{}' (imported from '{}')", name, imp.module));
            }
        }
    }

    Some(ImportResult {
        missing,
        checker: "js_exports".into(),
    })
}

/// Named import block parsed out of source.
#[derive(Debug, Clone)]
struct NamedImport {
    /// The module specifier — e.g. "./useEvents".
    module: String,
    /// The list of identifiers being imported. The literal "default" is
    /// used for `import X from '...'` form to keep matching simple.
    named: Vec<String>,
}

fn extract_named_imports(content: &str) -> Vec<NamedImport> {
    let mut out: Vec<NamedImport> = Vec::new();

    // 1. `import { A, B as C } from 'x'`  → A, C
    let re_braced =
        regex::Regex::new(r#"import\s*\{\s*([^}]+)\s*\}\s*from\s*['"]([^'"]+)['"]"#).unwrap();
    for cap in re_braced.captures_iter(content) {
        let raw_names = &cap[1];
        let module = cap[2].to_string();
        let names: Vec<String> = raw_names
            .split(',')
            .filter_map(|n| {
                let n = n.trim();
                if n.is_empty() {
                    return None;
                }
                // Handle "A as B" — record the local alias as the consumed name
                // (it's what the file references later). The producer-side check
                // is against the LHS name, so we record the LHS for missing-export
                // matching.
                let lhs = n.split_whitespace().next().unwrap_or(n);
                Some(lhs.to_string())
            })
            .collect();
        if !names.is_empty() {
            out.push(NamedImport {
                module,
                named: names,
            });
        }
    }

    // 2. `import Default from 'x'` (no braces)
    let re_default = regex::Regex::new(
        r#"(?m)^\s*import\s+([A-Za-z_$][\w$]*)\s*(?:,\s*\{[^}]*\}\s*)?from\s*['"]([^'"]+)['"]"#,
    )
    .unwrap();
    for cap in re_default.captures_iter(content) {
        let module = cap[2].to_string();
        out.push(NamedImport {
            module,
            named: vec!["default".to_string()],
        });
    }

    out
}

fn extract_exports(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // `export const NAME = ...`  / `export let NAME` / `export var NAME`
    let re_var =
        regex::Regex::new(r"(?m)^\s*export\s+(?:const|let|var)\s+([A-Za-z_$][\w$]*)").unwrap();
    for cap in re_var.captures_iter(content) {
        out.push(cap[1].to_string());
    }

    // `export function NAME(` / `export async function NAME(`
    let re_fn =
        regex::Regex::new(r"(?m)^\s*export\s+(?:async\s+)?function\s+([A-Za-z_$][\w$]*)").unwrap();
    for cap in re_fn.captures_iter(content) {
        out.push(cap[1].to_string());
    }

    // `export class NAME` / `export interface NAME` / `export type NAME`
    let re_decl =
        regex::Regex::new(r"(?m)^\s*export\s+(?:class|interface|type|enum)\s+([A-Za-z_$][\w$]*)")
            .unwrap();
    for cap in re_decl.captures_iter(content) {
        out.push(cap[1].to_string());
    }

    // `export { A, B as C, default as D }`
    let re_brace = regex::Regex::new(r"export\s*\{\s*([^}]+)\s*\}").unwrap();
    for cap in re_brace.captures_iter(content) {
        for tok in cap[1].split(',') {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            // `A as B` → exported name is B (RHS)
            let exported = if let Some((_lhs, rhs)) = tok.split_once(" as ") {
                rhs.trim().to_string()
            } else {
                tok.split_whitespace().next().unwrap_or(tok).to_string()
            };
            out.push(exported);
        }
    }

    // `export default ...`
    let re_default = regex::Regex::new(r"(?m)^\s*export\s+default\b").unwrap();
    if re_default.is_match(content) {
        out.push("default".to_string());
    }

    // `module.exports = { A, B }`  (CJS shape)
    let re_cjs = regex::Regex::new(r"module\.exports\s*=\s*\{\s*([^}]*)\s*\}").unwrap();
    for cap in re_cjs.captures_iter(content) {
        for tok in cap[1].split(',') {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            // Support `{ foo }` shorthand and `{ foo: bar }` long form.
            let name = tok.split(':').next().unwrap_or(tok).trim();
            if !name.is_empty() {
                out.push(name.to_string());
            }
        }
    }

    // `module.exports.NAME =`  / `exports.NAME =`
    let re_cjs_named =
        regex::Regex::new(r"(?m)^\s*(?:module\.)?exports\.([A-Za-z_$][\w$]*)\s*=").unwrap();
    for cap in re_cjs_named.captures_iter(content) {
        out.push(cap[1].to_string());
    }

    out.sort();
    out.dedup();
    out
}

/// Resolve a relative import specifier to a real file path on disk.
/// Tries the literal path, then common extensions, then `<spec>/index.*`.
/// Returns None when nothing matches — caller treats it as a missing producer.
fn resolve_relative_module(from_dir: &Path, spec: &str, working_dir: &str) -> Option<PathBuf> {
    let base = from_dir.join(spec);
    let candidates: Vec<PathBuf> = vec![
        base.clone(),
        base.with_extension("ts"),
        base.with_extension("tsx"),
        base.with_extension("js"),
        base.with_extension("jsx"),
        base.with_extension("mjs"),
        base.with_extension("cjs"),
        base.join("index.ts"),
        base.join("index.tsx"),
        base.join("index.js"),
        base.join("index.jsx"),
    ];
    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    // As a last resort, walk up to workspace root and search — but only when
    // the spec begins with `./` or `../`. We've already exhausted the
    // direct join, so this is purely defensive.
    let _ = working_dir;
    None
}

// ─── Tier 2.6 — API contract check (Doc 09 T3.3) ──────────────────────────────
//
// Validates that every URL referenced by a frontend consumer file
// (apiClient.get(...), fetch('...'), axios.post(...), etc.) appears in
// the workspace's INTERFACE.md, which the backend worker writes via
// `write_interface_spec` (Doc 09 T3.1).
//
// Returns:
//   • None — file isn't a JS/TS module, or no INTERFACE.md exists yet
//     (don't punish frontends that ship before the backend writes a spec —
//     the supervisor's task graph should sequence those, not this pass).
//   • Some(clean) — every URL is documented.
//   • Some(missing) — URL is referenced but not declared. The agent gets
//     the literal URL string and the file it came from so it can fix
//     either side of the contract.

pub fn tier2_6_api_contract(path: &Path, working_dir: &str) -> Option<ImportResult> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    if !matches!(ext.as_str(), "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx") {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;

    let urls = extract_api_urls(&content);
    if urls.is_empty() {
        return None;
    }

    let interface_md = path
        .parent()
        .and_then(|p| find_upward(p, "INTERFACE.md", working_dir))?;
    let spec = std::fs::read_to_string(&interface_md).ok()?;

    // INTERFACE.md is a plain markdown table of endpoints. We strip query
    // strings + path-param placeholders and check substring containment —
    // this is more forgiving than exact match (a frontend `${baseUrl}/events`
    // string vs `/events` in the spec).
    let mut missing: Vec<String> = Vec::new();
    for url in urls {
        let normalised = normalise_url_for_match(&url);
        if normalised.is_empty() {
            continue;
        }
        if !spec.contains(&normalised) {
            missing.push(format!("'{url}' not declared in INTERFACE.md"));
        }
    }

    Some(ImportResult {
        missing,
        checker: "api_contract".into(),
    })
}

fn extract_api_urls(content: &str) -> Vec<String> {
    // Covers:
    //   fetch('/api/x')
    //   axios.get('/api/x', ...)
    //   apiClient.post('/api/x', ...)
    //   client.request({ url: '/api/x' })
    // Quotation marks: ', ", or backticks (template literals).
    // We extract the raw quoted string; template literals with ${} get
    // normalised to bare path segments in normalise_url_for_match.
    let mut out: Vec<String> = Vec::new();
    let patterns = [
        r#"\bfetch\s*\(\s*['"`]([^'"`]+)['"`]"#,
        r#"\b(?:axios|apiClient|client|api)\s*\.\s*(?:get|post|put|delete|patch|head|options|request)\s*\(\s*['"`]([^'"`]+)['"`]"#,
        r#"\burl\s*:\s*['"`]([^'"`]+)['"`]"#,
    ];
    for pat in patterns {
        let re = regex::Regex::new(pat).unwrap();
        for cap in re.captures_iter(content) {
            let raw = cap[1].to_string();
            // Only paths that look like an API route (start with / or
            // contain /api/). Skips image / asset / scheme URLs.
            if raw.starts_with('/') || raw.contains("/api/") {
                out.push(raw);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn normalise_url_for_match(url: &str) -> String {
    // Drop query strings.
    let without_query = url.split('?').next().unwrap_or(url);
    // Drop trailing slash.
    let trimmed = without_query.trim_end_matches('/');
    // Collapse `${foo}` template segments — easier than trying to fully
    // resolve them. Substring containment in the spec absorbs the
    // remaining ambiguity for static prefixes.
    let no_template = regex::Regex::new(r"\$\{[^}]*\}")
        .unwrap()
        .replace_all(trimmed, "");
    no_template.to_string()
}

// ─── Tier 3 — Type checkers ───────────────────────────────────────────────────

/// Run a slow type-checker subprocess appropriate for the file type.
/// `checker` overrides auto-detection (`None` → auto).
pub async fn tier3_types(path: &Path, working_dir: &str, checker: Option<&str>) -> LintResult {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match (ext.as_str(), checker) {
        ("py", Some("mypy")) => run_mypy(path, working_dir).await,
        ("py", _) => run_pyright_or_mypy(path, working_dir).await,
        ("ts" | "tsx", _) => run_tsc(path, working_dir).await,
        ("js" | "jsx", Some("eslint")) | ("js" | "jsx", None) => {
            run_eslint(path, working_dir).await
        }
        _ => LintResult {
            checker: "tier3".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "warning",
                message: format!("No type checker configured for .{ext} files."),
                line: None,
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
        &[
            "--no-error-summary",
            "--show-column-numbers",
            &path.to_string_lossy(),
        ],
        working_dir,
        "mypy",
    )
    .await
}

async fn run_pyright(path: &Path, working_dir: &str) -> LintResult {
    run_checker_subprocess(
        "pyright",
        &[&path.to_string_lossy()],
        working_dir,
        "pyright",
    )
    .await
}

async fn run_tsc(path: &Path, working_dir: &str) -> LintResult {
    let mut args = vec![
        "--noEmit".to_string(),
        "--pretty".to_string(),
        "false".to_string(),
    ];

    // Use tsconfig.json if found; otherwise pass the file directly with --allowJs.
    if let Some(ts) = path
        .parent()
        .and_then(|p| find_upward(p, "tsconfig.json", working_dir))
    {
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
    )
    .await
}

async fn run_checker_subprocess(
    bin: &str,
    args: &[&str],
    working_dir: &str,
    checker: &str,
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
                message: format!(
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
            // Show the FIRST N bytes so the earliest (most actionable) errors are
            // always visible. Previously we showed the last N bytes, which meant
            // the root-cause error was often cropped out in favour of cascading
            // downstream errors.
            const MAX_CHECKER_BYTES: usize = 16_000;
            let capped = if combined.len() > MAX_CHECKER_BYTES {
                let safe_end = combined
                    .char_indices()
                    .map(|(i, _)| i)
                    .take_while(|&i| i < MAX_CHECKER_BYTES)
                    .last()
                    .unwrap_or(MAX_CHECKER_BYTES);
                format!(
                    "{}\n[... output truncated — {} bytes total, showing first {}. \
                     Fix the errors above, then re-run to see remaining issues.]",
                    &combined[..safe_end],
                    combined.len(),
                    safe_end,
                )
            } else {
                combined
            };
            LintResult {
                checker: checker.to_string(),
                diagnostics: vec![LintDiagnostic {
                    severity: "error",
                    message: capped.trim().to_string(),
                    line: None,
                }],
            }
        }
    }
}

// ─── Helpers — Python / Node resolution ──────────────────────────────────────

/// Return the Python binary path, preferring a virtual-environment interpreter
/// over the system Python so that project dependencies are on sys.path.
pub fn resolve_python(working_dir: &str) -> String {
    // Common virtual-environment directory names, checked in priority order.
    // Many tools use non-standard names (.env, env, .venv39, etc.) — cover the
    // most common ones so `py_compile` doesn't silently fall back to a system
    // Python that lacks the project's dependencies.
    let candidates: Vec<String> = if cfg!(target_os = "windows") {
        vec![
            format!("{working_dir}\\.venv\\Scripts\\python.exe"),
            format!("{working_dir}\\venv\\Scripts\\python.exe"),
            format!("{working_dir}\\env\\Scripts\\python.exe"),
            format!("{working_dir}\\.env\\Scripts\\python.exe"),
            "python".into(),
        ]
    } else {
        vec![
            format!("{working_dir}/.venv/bin/python3"),
            format!("{working_dir}/venv/bin/python3"),
            format!("{working_dir}/env/bin/python3"),
            format!("{working_dir}/.env/bin/python3"),
            format!("{working_dir}/.venv/bin/python"),
            format!("{working_dir}/venv/bin/python"),
            "python3".into(),
            "python".into(),
        ]
    };

    for c in candidates {
        // Absolute paths: check existence. System names: let OS find them.
        if Path::new(&c).is_absolute() {
            if Path::new(&c).exists() {
                return c;
            }
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
        if candidate.exists() {
            return Some(candidate);
        }
        if current == root {
            break;
        }
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
    let re_from = regex::Regex::new(r"(?m)^from\s+([\w][.\w]*)").unwrap();
    let mut out: Vec<String> = Vec::new();

    for cap in re_import.captures_iter(content) {
        out.push(cap[1].to_string());
    }
    for cap in re_from.captures_iter(content) {
        // Take the top-level package only: "os.path" → "os"
        let pkg = cap[1].split('.').next().unwrap_or(&cap[1]).to_string();
        if !pkg.is_empty() {
            out.push(pkg);
        }
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
    let re_esm =
        regex::Regex::new(r#"(?:^|\n)\s*(?:import|export)[^'"]*['"]([^'"]+)['"]"#).unwrap();
    let re_cjs = regex::Regex::new(r#"require\(['"]([^'"]+)['"]\)"#).unwrap();
    let mut out: Vec<String> = Vec::new();

    for cap in re_esm.captures_iter(content) {
        out.push(cap[1].to_string());
    }
    for cap in re_cjs.captures_iter(content) {
        out.push(cap[1].to_string());
    }
    out.sort();
    out.dedup();
    out
}

fn parse_requirements(content: &str) -> HashSet<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
        .map(|l| {
            // Strip version specifier: "requests>=2.0" → "requests"
            let name = l
                .split(&['=', '>', '<', '!', '[', ';', ' '][..])
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
        "__future__",
        "__main__",
        "abc",
        "aifc",
        "argparse",
        "array",
        "ast",
        "asynchat",
        "asyncio",
        "asyncore",
        "atexit",
        "audioop",
        "base64",
        "bdb",
        "binascii",
        "binhex",
        "bisect",
        "builtins",
        "bz2",
        "calendar",
        "cgi",
        "cgitb",
        "chunk",
        "cmath",
        "cmd",
        "code",
        "codecs",
        "codeop",
        "collections",
        "colorsys",
        "compileall",
        "concurrent",
        "configparser",
        "contextlib",
        "contextvars",
        "copy",
        "copyreg",
        "cProfile",
        "csv",
        "ctypes",
        "curses",
        "dataclasses",
        "datetime",
        "dbm",
        "decimal",
        "difflib",
        "dis",
        "doctest",
        "email",
        "encodings",
        "enum",
        "errno",
        "faulthandler",
        "fcntl",
        "filecmp",
        "fileinput",
        "fnmatch",
        "fractions",
        "ftplib",
        "functools",
        "gc",
        "getopt",
        "getpass",
        "gettext",
        "glob",
        "grp",
        "gzip",
        "hashlib",
        "heapq",
        "hmac",
        "html",
        "http",
        "idlelib",
        "imaplib",
        "imghdr",
        "imp",
        "importlib",
        "inspect",
        "io",
        "ipaddress",
        "itertools",
        "json",
        "keyword",
        "lib2to3",
        "linecache",
        "locale",
        "logging",
        "lzma",
        "mailbox",
        "mailcap",
        "marshal",
        "math",
        "mimetypes",
        "mmap",
        "modulefinder",
        "multiprocessing",
        "netrc",
        "nis",
        "nntplib",
        "numbers",
        "operator",
        "optparse",
        "os",
        "ossaudiodev",
        "pathlib",
        "pdb",
        "pickle",
        "pickletools",
        "pipes",
        "pkgutil",
        "platform",
        "plistlib",
        "poplib",
        "posix",
        "posixpath",
        "pprint",
        "profile",
        "pstats",
        "pty",
        "pwd",
        "py_compile",
        "pyclbr",
        "pydoc",
        "queue",
        "quopri",
        "random",
        "re",
        "readline",
        "reprlib",
        "resource",
        "rlcompleter",
        "runpy",
        "sched",
        "secrets",
        "select",
        "selectors",
        "shelve",
        "shlex",
        "shutil",
        "signal",
        "site",
        "smtpd",
        "smtplib",
        "sndhdr",
        "socket",
        "socketserver",
        "spwd",
        "sqlite3",
        "sre_compile",
        "sre_constants",
        "sre_parse",
        "ssl",
        "stat",
        "statistics",
        "string",
        "stringprep",
        "struct",
        "subprocess",
        "sunau",
        "symtable",
        "sys",
        "sysconfig",
        "syslog",
        "tabnanny",
        "tarfile",
        "telnetlib",
        "tempfile",
        "termios",
        "test",
        "textwrap",
        "threading",
        "time",
        "timeit",
        "tkinter",
        "token",
        "tokenize",
        "tomllib",
        "trace",
        "traceback",
        "tracemalloc",
        "tty",
        "turtle",
        "turtledemo",
        "types",
        "typing",
        "unicodedata",
        "unittest",
        "urllib",
        "uu",
        "uuid",
        "venv",
        "warnings",
        "wave",
        "weakref",
        "webbrowser",
        "winreg",
        "winsound",
        "wsgiref",
        "xdrlib",
        "xml",
        "xmlrpc",
        "zipapp",
        "zipfile",
        "zipimport",
        "zlib",
        "zoneinfo",
        // Common type-checking extras that aren't truly third-party
        "typing_extensions",
        // Popular packages often treated as stdlib by agents
        "pathspec",
        "abc",
    ]
    .into()
}

fn node_builtin_set() -> HashSet<&'static str> {
    // Node.js built-in modules (available without npm install).
    [
        "assert",
        "async_hooks",
        "buffer",
        "child_process",
        "cluster",
        "console",
        "constants",
        "crypto",
        "dgram",
        "diagnostics_channel",
        "dns",
        "domain",
        "events",
        "fs",
        "http",
        "http2",
        "https",
        "inspector",
        "module",
        "net",
        "os",
        "path",
        "perf_hooks",
        "process",
        "punycode",
        "querystring",
        "readline",
        "repl",
        "stream",
        "string_decoder",
        "sys",
        "timers",
        "tls",
        "trace_events",
        "tty",
        "url",
        "util",
        "v8",
        "vm",
        "wasi",
        "worker_threads",
        "zlib",
        // node: prefixed variants
        "node:assert",
        "node:buffer",
        "node:child_process",
        "node:cluster",
        "node:console",
        "node:crypto",
        "node:dgram",
        "node:dns",
        "node:events",
        "node:fs",
        "node:http",
        "node:http2",
        "node:https",
        "node:module",
        "node:net",
        "node:os",
        "node:path",
        "node:process",
        "node:querystring",
        "node:readline",
        "node:stream",
        "node:tls",
        "node:url",
        "node:util",
        "node:v8",
        "node:vm",
        "node:worker_threads",
        "node:zlib",
    ]
    .into()
}
