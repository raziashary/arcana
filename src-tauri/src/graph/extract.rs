// Extraction layer — tree-sitter AST parsing and language-specific handlers

use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use crate::errors::Result;
use rayon::prelude::*;

// ── Data Structures ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NodeKind {
    File,
    Class,
    Function,
    Type,
    Test,
    Module,
    Import,
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeKind::File     => "File",
            NodeKind::Class    => "Class",
            NodeKind::Function => "Function",
            NodeKind::Type     => "Type",
            NodeKind::Test     => "Test",
            NodeKind::Module   => "Module",
            NodeKind::Import   => "Import",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum EdgeKind {
    Calls,
    ImportsFrom,
    Imports,
    Inherits,
    Implements,
    Contains,
    TestedBy,
    DependsOn,
    References,
    SemanticallySimilar,
}

// Display produces UPPER_SNAKE_CASE to match the frontend EDGE_COLORS map keys.
impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EdgeKind::Calls               => "CALLS",
            EdgeKind::ImportsFrom         => "IMPORTS_FROM",
            EdgeKind::Imports             => "IMPORTS",
            EdgeKind::Inherits            => "INHERITS",
            EdgeKind::Implements          => "IMPLEMENTS",
            EdgeKind::Contains            => "CONTAINS",
            EdgeKind::TestedBy            => "TESTED_BY",
            EdgeKind::DependsOn           => "DEPENDS_ON",
            EdgeKind::References          => "REFERENCES",
            EdgeKind::SemanticallySimilar => "SEMANTICALLY_SIMILAR",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConfidenceTier {
    Extracted,
    Inferred,
    Ambiguous,
}

impl std::fmt::Display for ConfidenceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ConfidenceTier::Extracted => "EXTRACTED",
            ConfidenceTier::Inferred  => "INFERRED",
            ConfidenceTier::Ambiguous => "AMBIGUOUS",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub kind: NodeKind,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub line_start: i32,
    pub line_end: i32,
    pub language: String,
    pub parent_name: Option<String>,
    pub params: Option<String>,
    pub return_type: Option<String>,
    pub modifiers: Option<String>,
    pub is_test: bool,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeInfo {
    pub kind: EdgeKind,
    pub source_qualified: String,
    pub target_qualified: String,
    pub file_path: String,
    pub line: i32,
    pub confidence: f32,
    pub confidence_tier: ConfidenceTier,
    pub weight: f32,
    pub relation: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub nodes: Vec<NodeInfo>,
    pub edges: Vec<EdgeInfo>,
}

// ── Utility Functions ──────────────────────────────────────────────────────

pub fn make_id(parts: &[&str]) -> String {
    let joined = parts.join("_");
    let normalized = regex::Regex::new(r"[^a-zA-Z0-9]+")
        .unwrap()
        .replace_all(&joined, "_")
        .to_string();
    normalized.to_lowercase().trim_matches('_').to_string()
}

pub fn normalize_id(s: &str) -> String {
    let normalized = regex::Regex::new(r"[^a-zA-Z0-9]+")
        .unwrap()
        .replace_all(s, "_")
        .to_string();
    normalized.to_lowercase().trim_matches('_').to_string()
}

pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let hash = Sha256::digest(&bytes);
    Ok(hex::encode(hash))
}

// ── Cache Functions ───────────────────────────────────────────────────────

// Bump this whenever the extractor logic changes — invalidates all cached entries.
const EXTRACTOR_VERSION: u32 = 5;

pub fn load_cached(file_path: &Path, file_hash: &str) -> Option<ExtractionResult> {
    let cache_dir = dirs::home_dir()?.join(".autodev/graph_cache");
    let cache_file = cache_dir.join(format!("{}.json", hex::encode(Sha256::digest(file_path.to_string_lossy().as_bytes()))));
    
    if let Ok(content) = fs::read_to_string(&cache_file) {
        if let Ok(entry) = serde_json::from_str::<CacheEntry>(&content) {
            if entry.file_hash == file_hash && entry.extractor_version == EXTRACTOR_VERSION {
                return Some(entry.result);
            }
        }
    }
    None
}

pub fn save_cached(file_path: &Path, file_hash: &str, result: &ExtractionResult) -> Result<()> {
    let cache_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home directory for cache"))?
        .join(".autodev/graph_cache");
    fs::create_dir_all(&cache_dir)?;
    
    let cache_file = cache_dir.join(format!("{}.json", hex::encode(Sha256::digest(file_path.to_string_lossy().as_bytes()))));
    let entry = CacheEntry {
        file_hash: file_hash.to_string(),
        extractor_version: EXTRACTOR_VERSION,
        result: result.clone(),
        cached_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    
    fs::write(&cache_file, serde_json::to_string(&entry)?)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    file_hash: String,
    #[serde(default)]
    extractor_version: u32,
    result: ExtractionResult,
    cached_at: u64,
}

// ── Main Extraction Entry Point ────────────────────────────────────────────

pub fn extract(path: &Path, repo_root: &Path) -> Result<ExtractionResult> {
    let file_hash = hash_file(path)?;
    
    // Check cache
    if let Some(cached) = load_cached(path, &file_hash) {
        return Ok(cached);
    }
    
    let source = fs::read(path)?;
    let language = detect_language(path)?;
    
    let result = match language.as_str() {
        "python" => extract_python(path, &source, &file_hash, repo_root)?,
        "javascript" | "typescript" => extract_js_ts(path, &source, &file_hash)?,
        "rust" => extract_rust(path, &source, &file_hash, repo_root)?,
        "go" => extract_go(path, &source, &file_hash, repo_root)?,
        "java" => extract_java(path, &source, &file_hash)?,
        "cpp" | "c" => extract_c_cpp(path, &source, &file_hash)?,
        "ruby" => extract_ruby(path, &source, &file_hash)?,
        "php" => extract_php(path, &source, &file_hash)?,
        "kotlin" => extract_kotlin(path, &source, &file_hash)?,
        "swift" => extract_swift(path, &source, &file_hash)?,
        "bash" => extract_bash(path, &source, &file_hash)?,
        _ => ExtractionResult::default(),
    };
    
    // Cache result — log on failure, never abort extraction
    if let Err(e) = save_cached(path, &file_hash, &result) {
        tracing::warn!("[extract] cache write failed for {}: {}", path.display(), e);
    }
    
    Ok(result)
}

pub fn extract_all(paths: &[PathBuf], repo_root: &Path) -> Vec<(PathBuf, Result<ExtractionResult>)> {
    paths
        .par_iter()
        .map(|p| (p.clone(), extract(p, repo_root)))
        .collect()
}

// ── Language Detection ─────────────────────────────────────────────────────

fn detect_language(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    
    Ok(match ext {
        "py" => "python",
        "js" | "mjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "cpp" | "cc" | "cxx" | "c++" => "cpp",
        "c" | "h" => "c",
        "rb" => "ruby",
        "php" => "php",
        "kt" => "kotlin",
        "swift" => "swift",
        "sh" | "bash" => "bash",
        _ => "unknown",
    }
    .to_string())
}

// ── Language Extractors ────────────────────────────────────────────────────

fn extract_python(path: &Path, source: &[u8], file_hash: &str, repo_root: &Path) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let module_name = file_name.trim_end_matches(".py").to_string();
    let is_test_file = file_name.starts_with("test_") || file_name.ends_with("_test.py");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    
    // 1. File node
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: file_name.clone(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: total_lines,
        language: "python".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: is_test_file,
        file_hash: file_hash.to_string(),
    });
    
    let source_str = match std::str::from_utf8(source) {
        Ok(s) => s,
        Err(_) => return Ok(result),
    };
    
    // Regex patterns (compiled once)
    let class_re = regex::Regex::new(r"(?m)^(\s*)class\s+([A-Za-z_][A-Za-z0-9_]*)\s*[\(:]").unwrap();
    let func_re = regex::Regex::new(r"(?m)^(\s*)(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap();
    let import_re = regex::Regex::new(r"(?m)^\s*import\s+([A-Za-z_][A-Za-z0-9_.]*)").unwrap();
    let from_import_re = regex::Regex::new(r"(?m)^\s*from\s+([A-Za-z_.][A-Za-z0-9_.]*)\s+import\s+(.+)$").unwrap();
    
    // Helper: line number from byte offset
    let line_of = |offset: usize| -> i32 {
        source_str[..offset.min(source_str.len())].matches('\n').count() as i32 + 1
    };
    
    // 2. Extract classes
    for cap in class_re.captures_iter(source_str) {
        let class_name = cap.get(2).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, class_name);
        
        result.nodes.push(NodeInfo {
            kind: NodeKind::Class,
            name: class_name.to_string(),
            qualified_name: qname.clone(),
            file_path: file_path.clone(),
            line_start: line,
            line_end: line,
            language: "python".to_string(),
            parent_name: Some(file_path.clone()),
            params: None,
            return_type: None,
            modifiers: None,
            is_test: is_test_file,
            file_hash: file_hash.to_string(),
        });
        
        // File contains class
        result.edges.push(EdgeInfo {
            kind: EdgeKind::Contains,
            source_qualified: file_path.clone(),
            target_qualified: qname,
            file_path: file_path.clone(),
            line,
            confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted,
            weight: 1.0,
            relation: None,
        });
    }
    
    // 3. Extract functions
    for cap in func_re.captures_iter(source_str) {
        let indent = cap.get(1).unwrap().as_str();
        let func_name = cap.get(2).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, func_name);
        let is_method = !indent.is_empty();
        let is_test = is_test_file || func_name.starts_with("test_");
        
        result.nodes.push(NodeInfo {
            kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: func_name.to_string(),
            qualified_name: qname.clone(),
            file_path: file_path.clone(),
            line_start: line,
            line_end: line,
            language: "python".to_string(),
            parent_name: Some(file_path.clone()),
            params: None,
            return_type: None,
            modifiers: if is_method { Some("method".to_string()) } else { None },
            is_test,
            file_hash: file_hash.to_string(),
        });
        
        // File contains function (only top-level, not methods)
        if !is_method {
            result.edges.push(EdgeInfo {
                kind: EdgeKind::Contains,
                source_qualified: file_path.clone(),
                target_qualified: qname,
                file_path: file_path.clone(),
                line,
                confidence: 1.0,
                confidence_tier: ConfidenceTier::Extracted,
                weight: 1.0,
                relation: None,
            });
        }
    }
    
    // 4. Extract `import X` statements → create Imports edges
    for cap in import_re.captures_iter(source_str) {
        let module = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        // Target is the module name (will be reconciled to file later if matchable)
        let target = format!("module::{}", module);
        
        result.edges.push(EdgeInfo {
            kind: EdgeKind::Imports,
            source_qualified: file_path.clone(),
            target_qualified: target.clone(),
            file_path: file_path.clone(),
            line,
            confidence: 0.7,
            confidence_tier: ConfidenceTier::Inferred,
            weight: 0.8,
            relation: Some(format!("import {}", module)),
        });
        
        // Also create a stub Module node so the edge has a valid target
        result.nodes.push(NodeInfo {
            kind: NodeKind::Module,
            name: module.to_string(),
            qualified_name: target,
            file_path: file_path.clone(),
            line_start: line,
            line_end: line,
            language: "python".to_string(),
            parent_name: None,
            params: None,
            return_type: None,
            modifiers: Some("external".to_string()),
            is_test: false,
            file_hash: file_hash.to_string(),
        });
    }
    
    // 5. Extract `from X import Y` statements — G-6: resolve intra-project relative imports
    //    to real file-node qualified_names instead of always emitting "module::" stubs.
    for cap in from_import_re.captures_iter(source_str) {
        let module = cap.get(1).unwrap().as_str();
        let imported = cap.get(2).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());

        let resolved = resolve_python_import(path, module, repo_root);
        let (target_qname, is_external) = match &resolved {
            Some(abs_path) => (abs_path.to_string_lossy().to_string(), false),
            None => (format!("module::{}", module), true),
        };

        result.edges.push(EdgeInfo {
            kind: EdgeKind::ImportsFrom,
            source_qualified: file_path.clone(),
            target_qualified: target_qname.clone(),
            file_path: file_path.clone(),
            line,
            confidence: if is_external { 0.7 } else { 1.0 },
            confidence_tier: if is_external { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            weight: 1.0,
            relation: Some(format!("from {} import {}", module, imported.trim())),
        });

        if is_external {
            // Only create a stub Module node for packages outside the repo.
            // Internal target File nodes are created when we parse that file itself.
            result.nodes.push(NodeInfo {
                kind: NodeKind::Module,
                name: module.to_string(),
                qualified_name: target_qname,
                file_path: file_path.clone(),
                line_start: line,
                line_end: line,
                language: "python".to_string(),
                parent_name: None,
                params: None,
                return_type: None,
                modifiers: Some("external".to_string()),
                is_test: false,
                file_hash: file_hash.to_string(),
            });
        }
    }

    // 6. CALLS edge detection — G-11: heuristic second pass for intra-file call sites.
    //    Scans source for `name(` tokens where name matches a function defined in this file.
    //    Uses line-number bracketing to attribute each call to its enclosing function
    //    (the last function definition that starts at or before the call site).
    //    Emits one deduplicated CALLS edge per (caller → callee) pair.
    {
        let known_funcs: std::collections::HashMap<String, String> = result
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Test))
            .map(|n| (n.name.clone(), n.qualified_name.clone()))
            .collect();

        if !known_funcs.is_empty() {
            // Sorted (line_start, qualified_name) list for enclosing-function lookup.
            let mut fn_lines: Vec<(i32, String)> = result
                .nodes
                .iter()
                .filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Test))
                .map(|n| (n.line_start, n.qualified_name.clone()))
                .collect();
            fn_lines.sort_by_key(|&(l, _)| l);

            let call_re = regex::Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap();
            // emitted: Set<(caller_qname, callee_qname)> — one edge per unique pair
            let mut emitted: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

            for cap in call_re.captures_iter(source_str) {
                let callee_name = cap.get(1).unwrap().as_str();
                if let Some(callee_qname) = known_funcs.get(callee_name) {
                    let call_line = line_of(cap.get(0).unwrap().start());
                    // Find enclosing function: last entry with line_start ≤ call_line
                    let idx = fn_lines.partition_point(|&(l, _)| l <= call_line);
                    let source_qname: String = if idx > 0 {
                        fn_lines[idx - 1].1.clone()
                    } else {
                        file_path.clone() // call appears before any def — attribute to file
                    };
                    // Skip self-referential edges (function contains its own name)
                    if source_qname == *callee_qname { continue; }
                    let key = (source_qname.clone(), callee_qname.clone());
                    if emitted.insert(key) {
                        result.edges.push(EdgeInfo {
                            kind: EdgeKind::Calls,
                            source_qualified: source_qname,
                            target_qualified: callee_qname.clone(),
                            file_path: file_path.clone(),
                            line: call_line,
                            confidence: 0.8,
                            confidence_tier: ConfidenceTier::Extracted,
                            weight: 1.0,
                            relation: None,
                        });
                    }
                }
            }
        }
    }

    let _ = module_name; // suppress unused warning
    Ok(result)
}

// ── Python Import Resolver (G-6) ───────────────────────────────────────────

// Resolves a Python import module string to an absolute file path within the repo.
// Only handles relative imports (leading dots). Absolute imports cannot be resolved
// without PYTHONPATH knowledge and are left as external "module::" stubs.
fn resolve_python_import(current_file: &Path, module: &str, repo_root: &Path) -> Option<PathBuf> {
    if !module.starts_with('.') {
        return None;
    }

    let dots = module.chars().take_while(|c| *c == '.').count();
    let rest = &module[dots..];

    // First dot = current package dir; each additional dot = one level up
    let mut base = current_file.parent()?;
    for _ in 1..dots {
        base = base.parent()?;
    }

    // Never escape the repo root
    if !base.starts_with(repo_root) {
        return None;
    }

    if rest.is_empty() {
        // `from . import something` — the package __init__.py
        let candidate = base.join("__init__.py");
        if candidate.exists() { return Some(candidate); }
    } else {
        // Build path by treating each dot-segment as a directory component
        let candidate_base: PathBuf = rest.split('.').fold(base.to_path_buf(), |p, seg| p.join(seg));
        // Try as a module file: base/a/b.py
        let candidate_file = candidate_base.with_extension("py");
        if candidate_file.exists() { return Some(candidate_file); }
        // Try as a package: base/a/b/__init__.py
        let candidate_init = candidate_base.join("__init__.py");
        if candidate_init.exists() { return Some(candidate_init); }
    }

    None
}

fn extract_js_ts(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let language = if ext == "ts" || ext == "tsx" { "typescript" } else { "javascript" };
    let is_test_file = file_path.contains(".test.") || file_path.contains(".spec.") || file_path.contains("__tests__");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    // 1. File node
    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: language.to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // 2. Classes
    let class_re = regex::Regex::new(r"(?m)^(?:export\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    for cap in class_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: language.to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: is_test_file, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // 3. Named functions
    let func_re = regex::Regex::new(r"(?m)^(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*[<(]").unwrap();
    for cap in func_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let is_test = is_test_file || name.starts_with("test") || name == "it" || name == "describe";
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: language.to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // 4. Arrow / const functions (top-level)
    let arrow_re = regex::Regex::new(r"(?m)^(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(?:async\s*)?\(").unwrap();
    for cap in arrow_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Function, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: language.to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: Some("arrow".to_string()),
            is_test: is_test_file, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // 5. Import statements → IMPORTS_FROM edges
    let import_re = regex::Regex::new(r#"(?m)^\s*import\s+[^'"]+\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    for cap in import_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let resolved = resolve_js_import(path, spec);
        let (target, is_ext) = match &resolved {
            Some(p) => (p.to_string_lossy().to_string(), false),
            None => (format!("module::{}", spec), true),
        };
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: if is_ext { 0.7 } else { 1.0 },
            confidence_tier: if is_ext { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            weight: 1.0, relation: Some(format!("from '{}'", spec)) });
        if is_ext {
            let mod_name = spec.trim_start_matches('@').split('/').next().unwrap_or(spec);
            result.nodes.push(NodeInfo { kind: NodeKind::Module, name: mod_name.to_string(),
                qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
                language: language.to_string(), parent_name: None, params: None, return_type: None,
                modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
        }
    }

    Ok(result)
}

// JS/TS relative import resolver — probes .ts/.tsx/.js/.jsx and index variants (G-2)
fn resolve_js_import(current_file: &Path, specifier: &str) -> Option<PathBuf> {
    if !specifier.starts_with('.') { return None; }
    let base = current_file.parent()?;
    let raw = base.join(specifier);
    let exts = ["ts", "tsx", "js", "jsx", "mjs"];
    for ext in &exts {
        let c = raw.with_extension(ext);
        if c.exists() { return Some(c); }
    }
    if raw.is_file() { return Some(raw.clone()); }
    for ext in &exts {
        let c = raw.join(format!("index.{}", ext));
        if c.exists() { return Some(c); }
    }
    None
}

fn extract_rust(path: &Path, source: &[u8], file_hash: &str, repo_root: &Path) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let is_test_file = file_path.contains("/tests/") || file_path.ends_with("_test.rs");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "rust".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Functions (pub/pub(crate)/async/etc.)
    let fn_re = regex::Regex::new(r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*[<(]").unwrap();
    for cap in fn_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let window = &source_str[off.saturating_sub(120)..off];
        let is_test = is_test_file || window.contains("#[test]") || name.starts_with("test_");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "rust".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Structs
    let struct_re = regex::Regex::new(r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in struct_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "rust".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: Some("struct".to_string()),
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Enums
    let enum_re = regex::Regex::new(r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in enum_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Type, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "rust".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: Some("enum".to_string()),
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Traits
    let trait_re = regex::Regex::new(r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in trait_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Type, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "rust".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: Some("trait".to_string()),
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // use statements — IMPORTS_FROM edges to intra-project files, or module:: stubs for extern/std.
    // Handles: `use crate::a::b`, `use super::a`, `use self::a`, `use extern_crate::a`.
    // Strips leading `pub ` and trailing `as Alias` and `{grouped, items}`.
    let use_re = regex::Regex::new(r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?use\s+([\w:{}*,\s]+)\s*;").unwrap();
    for cap in use_re.captures_iter(source_str) {
        let raw_path = cap.get(1).unwrap().as_str().trim();
        let line = line_of(cap.get(0).unwrap().start());
        // Expand grouped use: `crate::a::{B, C}` → ["crate::a::B", "crate::a::C"]
        let expanded = expand_rust_use_path(raw_path);
        for use_path in expanded {
            let resolved = resolve_rust_use(path, &use_path, repo_root);
            let (target, is_ext) = match &resolved {
                Some(p) => (p.to_string_lossy().to_string(), false),
                None => (format!("module::{}", use_path), true),
            };
            result.edges.push(EdgeInfo {
                kind: EdgeKind::ImportsFrom,
                source_qualified: file_path.clone(),
                target_qualified: target.clone(),
                file_path: file_path.clone(),
                line,
                confidence: if is_ext { 0.7 } else { 1.0 },
                confidence_tier: if is_ext { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
                weight: 1.0,
                relation: Some(format!("use {}", use_path)),
            });
            if is_ext {
                let mod_name = use_path.split("::").next().unwrap_or(&use_path);
                result.nodes.push(NodeInfo {
                    kind: NodeKind::Module, name: mod_name.to_string(),
                    qualified_name: target, file_path: file_path.clone(),
                    line_start: line, line_end: line, language: "rust".to_string(),
                    parent_name: None, params: None, return_type: None,
                    modifiers: Some("external".to_string()), is_test: false,
                    file_hash: file_hash.to_string(),
                });
            }
        }
    }

    Ok(result)
}

// ── Rust use-path helpers ─────────────────────────────────────────────────────

/// Expands a raw `use` path string into a flat list of qualified paths.
/// Handles grouped imports: `crate::a::{B, C}` → ["crate::a::B", "crate::a::C"].
/// Strips glob (`*`) and `as Alias` suffixes.
fn expand_rust_use_path(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    if let Some(brace_start) = raw.find('{') {
        let prefix = raw[..brace_start].trim_end_matches(':');
        let inner = raw[brace_start + 1..].trim_end_matches('}');
        return inner
            .split(',')
            .map(|item| {
                let item = item.trim().split(" as ").next().unwrap_or("").trim();
                if item.is_empty() || item == "*" {
                    prefix.to_string()
                } else {
                    format!("{}::{}", prefix, item)
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
    }
    // Strip `as Alias`
    let path = raw.split(" as ").next().unwrap_or(raw).trim();
    if path == "*" || path.is_empty() { return vec![]; }
    vec![path.to_string()]
}

/// Tries to resolve a Rust use-path to an absolute `.rs` file within the repo.
/// Maps:
///   `crate::foo::bar`  → `{src_root}/foo/bar.rs` or `{src_root}/foo/bar/mod.rs`
///   `super::foo`       → sibling of `path`'s parent dir
///   `self::foo`        → sibling of `path`
///
/// Standard library (`std`, `core`, `alloc`), `extern crate`, and third-party
/// crate roots are left as `module::` stubs.
fn resolve_rust_use(current_file: &Path, use_path: &str, repo_root: &Path) -> Option<PathBuf> {
    let segments: Vec<&str> = use_path.split("::").collect();
    if segments.is_empty() { return None; }

    // Determine the starting directory based on the path anchor
    let (base_dir, rest_segments) = match segments[0] {
        "crate" => {
            // `crate::` starts from the crate root. Heuristic: look for `src/` under repo_root,
            // or fall back to repo_root itself (for single-file or workspace member layouts).
            let src = repo_root.join("src");
            let crate_root = if src.is_dir() { src } else { repo_root.to_path_buf() };
            (crate_root, &segments[1..])
        }
        "super" => {
            // `super::` → parent package directory of the current file
            let parent = current_file.parent()?.parent()?;
            if !parent.starts_with(repo_root) { return None; }
            (parent.to_path_buf(), &segments[1..])
        }
        "self" => {
            let dir = current_file.parent()?;
            (dir.to_path_buf(), &segments[1..])
        }
        // External crates and std — not resolvable without Cargo metadata
        "std" | "core" | "alloc" | "proc_macro" => return None,
        _ => return None,
    };

    if rest_segments.is_empty() { return None; }

    // Build candidate path: treat every segment but the last as a directory component,
    // last segment as a filename (with .rs or /mod.rs).
    // We walk all intermediate segments as directories, and the final segment as a module.
    let module_path: PathBuf = rest_segments.iter().fold(base_dir, |p, seg| p.join(seg));
    // Try as a .rs file
    let as_file = module_path.with_extension("rs");
    if as_file.exists() && as_file.starts_with(repo_root) { return Some(as_file); }
    // Try as a directory module (mod.rs)
    let as_mod = module_path.join("mod.rs");
    if as_mod.exists() && as_mod.starts_with(repo_root) { return Some(as_mod); }

    None
}

fn extract_go(path: &Path, source: &[u8], file_hash: &str, repo_root: &Path) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let is_test_file = file_path.ends_with("_test.go");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "go".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Functions (top-level and methods with receivers)
    let func_re = regex::Regex::new(r"(?m)^func\s+(?:\([^)]+\)\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*[<(]").unwrap();
    for cap in func_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let is_test = is_test_file && name.starts_with("Test");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "go".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Structs and interfaces
    let type_re = regex::Regex::new(r"(?m)^type\s+([A-Za-z_][A-Za-z0-9_]*)\s+(struct|interface)\b").unwrap();
    for cap in type_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let kind_str = cap.get(2).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo {
            kind: if kind_str == "interface" { NodeKind::Type } else { NodeKind::Class },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "go".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: Some(kind_str.to_string()),
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Import statements — both single-line and block forms.
    //   Single: `import "path/to/pkg"`
    //   Block:  `import (\n    "path/to/pkg"\n    alias "other/pkg"\n)`
    // Relative imports (`./sub`, `../sibling`) are resolved to actual files.
    // Package-path imports (e.g. `github.com/…` or Go stdlib) become module:: stubs.
    let import_spec_re = regex::Regex::new(r#"(?m)"([^"]+)""#).unwrap();
    // Match single-line: import "…"
    let single_import_re = regex::Regex::new(r#"(?m)^import\s+"([^"]+)""#).unwrap();
    // Match block: import ( … ) — capture everything between parens
    let block_import_re = regex::Regex::new(r"(?ms)^import\s*\(([^)]*)\)").unwrap();

    let mut emit_go_import = |spec: &str, line: i32, result: &mut ExtractionResult| {
        let resolved = resolve_go_import(path, spec, repo_root);
        let (target, is_ext) = match &resolved {
            Some(p) => (p.to_string_lossy().to_string(), false),
            None => (format!("module::{}", spec), true),
        };
        result.edges.push(EdgeInfo {
            kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: if is_ext { 0.7 } else { 1.0 },
            confidence_tier: if is_ext { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            weight: 1.0, relation: Some(format!("import \"{}\"", spec)),
        });
        if is_ext {
            let mod_name = spec.split('/').last().unwrap_or(spec);
            result.nodes.push(NodeInfo {
                kind: NodeKind::Module, name: mod_name.to_string(),
                qualified_name: target, file_path: file_path.clone(),
                line_start: line, line_end: line, language: "go".to_string(),
                parent_name: None, params: None, return_type: None,
                modifiers: Some("external".to_string()), is_test: false,
                file_hash: file_hash.to_string(),
            });
        }
    };

    for cap in single_import_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        emit_go_import(spec, line, &mut result);
    }
    for block_cap in block_import_re.captures_iter(source_str) {
        let block_start_line = line_of(block_cap.get(0).unwrap().start());
        let block_body = block_cap.get(1).unwrap().as_str();
        for spec_cap in import_spec_re.captures_iter(block_body) {
            let spec = spec_cap.get(1).unwrap().as_str();
            // Approximate line: block_start_line + newlines before this match
            let inner_line = block_start_line
                + block_body[..spec_cap.get(0).unwrap().start()].matches('\n').count() as i32;
            emit_go_import(spec, inner_line, &mut result);
        }
    }

    Ok(result)
}

// ── Go import resolver ────────────────────────────────────────────────────────

/// Resolves a Go import path to an absolute file within the repo.
/// Only relative imports (`./sub` or `../sibling`) can be resolved without go.mod.
/// All other import paths (stdlib, github.com/…, etc.) return None → module:: stub.
fn resolve_go_import(current_file: &Path, import_path: &str, repo_root: &Path) -> Option<PathBuf> {
    if !import_path.starts_with('.') { return None; }
    let base = current_file.parent()?;
    let target_dir = base.join(import_path);
    // Canonicalize to resolve `../` components
    let canonical = target_dir.canonicalize().ok()?;
    if !canonical.starts_with(repo_root) { return None; }
    // Return the directory itself — the graph will have a File node for each .go file inside.
    // We point at the first .go file we find as a representative, or the dir path if none.
    if canonical.is_dir() {
        // Look for a non-test .go file to use as the target
        if let Ok(entries) = std::fs::read_dir(&canonical) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("go")
                    && !p.file_name().and_then(|n| n.to_str()).unwrap_or("").ends_with("_test.go")
                {
                    return Some(p);
                }
            }
        }
        Some(canonical)
    } else {
        None
    }
}

fn extract_java(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let is_test_file = file_name.starts_with("Test") || file_name.ends_with("Test.java")
        || file_name.ends_with("Tests.java") || file_name.ends_with("Spec.java");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: file_name.clone(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "java".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Classes, interfaces, enums, records, annotations
    let class_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|abstract|final|static)\s+)*(?:class|interface|enum|record|@interface)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in class_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "java".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: is_test_file, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Methods: access modifiers + return type + name(
    let method_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|static|final|abstract|synchronized|default|native|strictfp)\s+)+(?:\S+\s+)+([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap();
    for cap in method_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        if matches!(name, "class" | "interface" | "enum" | "record" | "if" | "while" | "for" | "switch" | "try" | "catch" | "new" | "return") { continue; }
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let window = &source_str[off.saturating_sub(120)..off];
        let is_test = is_test_file || window.contains("@Test") || name.starts_with("test");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "java".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // import statements (package-based; no relative file resolution in Java)
    let import_re = regex::Regex::new(r"(?m)^[ \t]*import\s+(?:static\s+)?([A-Za-z_][A-Za-z0-9_.]+)\s*;").unwrap();
    for cap in import_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let mod_name = spec.split('.').last().unwrap_or(spec);
        let target = format!("module::{}", spec);
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: 0.7, confidence_tier: ConfidenceTier::Inferred,
            weight: 1.0, relation: Some(format!("import {}", spec)) });
        result.nodes.push(NodeInfo { kind: NodeKind::Module, name: mod_name.to_string(),
            qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
            language: "java".to_string(), parent_name: None, params: None, return_type: None,
            modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
    }

    Ok(result)
}

fn extract_c_cpp(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let language = if ext == "c" || ext == "h" { "c" } else { "cpp" };
    let is_test_file = file_path.contains("test") || file_path.contains("spec");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: language.to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Structs and classes (C++)
    let class_re = regex::Regex::new(r"(?m)^[ \t]*(?:class|struct)\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:[:{]|$)").unwrap();
    for cap in class_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        if matches!(name, "alignas" | "alignof" | "decltype" | "noexcept" | "requires") { continue; }
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: language.to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Function definitions: return_type name( where line starts at low indentation
    // Pattern matches: optional qualifiers, return type tokens, then identifier(
    let func_re = regex::Regex::new(r"(?m)^(?:[ \t]{0,2})(?:(?:static|inline|extern|virtual|explicit|constexpr|override|const)\s+)*(?:[A-Za-z_~][A-Za-z0-9_:*& \t]*\s)([A-Za-z_][A-Za-z0-9_:]*)\s*\(").unwrap();
    for cap in func_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        if matches!(name, "if" | "while" | "for" | "switch" | "return" | "else" | "do"
            | "break" | "continue" | "case" | "default" | "goto" | "sizeof"
            | "class" | "struct" | "union" | "namespace" | "template" | "typedef"
            | "using" | "new" | "delete" | "throw" | "try" | "catch"
            | "operator" | "decltype" | "alignof" | "typeid" | "noexcept") { continue; }
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let is_test = is_test_file || name.starts_with("test_") || name.starts_with("Test");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: language.to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // #include "local.h" → resolve relative to current file's directory
    let include_local_re = regex::Regex::new(r#"(?m)^[ \t]*#include\s+"([^"]+)""#).unwrap();
    for cap in include_local_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let resolved = path.parent().map(|p| { let c = p.join(spec); if c.exists() { Some(c) } else { None } }).flatten();
        let (target, is_ext) = match &resolved {
            Some(p) => (p.to_string_lossy().to_string(), false),
            None => (format!("module::{}", spec), true),
        };
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: if is_ext { 0.7 } else { 1.0 },
            confidence_tier: if is_ext { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            weight: 1.0, relation: Some(format!("#include \"{}\"", spec)) });
        if is_ext {
            result.nodes.push(NodeInfo { kind: NodeKind::Module, name: spec.to_string(),
                qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
                language: language.to_string(), parent_name: None, params: None, return_type: None,
                modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
        }
    }

    // #include <system.h> → external module node
    let include_sys_re = regex::Regex::new(r"(?m)^[ \t]*#include\s+<([^>]+)>").unwrap();
    for cap in include_sys_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let target = format!("module::{}", spec);
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: 0.7, confidence_tier: ConfidenceTier::Inferred,
            weight: 1.0, relation: Some(format!("#include <{}>", spec)) });
        result.nodes.push(NodeInfo { kind: NodeKind::Module, name: spec.to_string(),
            qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
            language: language.to_string(), parent_name: None, params: None, return_type: None,
            modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
    }

    Ok(result)
}

fn extract_ruby(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let is_test_file = file_name.ends_with("_test.rb") || file_name.ends_with("_spec.rb")
        || file_path.contains("/spec/") || file_path.contains("/test/");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: file_name.clone(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "ruby".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Modules and classes (Ruby uses CamelCase for both)
    let class_re = regex::Regex::new(r"(?m)^[ \t]*(?:class|module)\s+([A-Z][A-Za-z0-9_:]*)").unwrap();
    for cap in class_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "ruby".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: is_test_file, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Methods: def name / def self.name
    let def_re = regex::Regex::new(r"(?m)^[ \t]*def\s+(?:self\.)?([A-Za-z_][A-Za-z0-9_!?=]*)").unwrap();
    for cap in def_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let is_test = is_test_file || name.starts_with("test_");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "ruby".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // require_relative 'path' → resolve relative to current file
    let req_rel_re = regex::Regex::new(r#"(?m)^[ \t]*require_relative\s+['"]((?:[^'"]+))['"]\'?"#).unwrap();
    for cap in req_rel_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let resolved = path.parent().and_then(|p| {
            let raw = p.join(spec);
            let with_rb = raw.with_extension("rb");
            if with_rb.exists() { Some(with_rb) } else if raw.exists() { Some(raw) } else { None }
        });
        let (target, is_ext) = match &resolved {
            Some(p) => (p.to_string_lossy().to_string(), false),
            None => (format!("module::{}", spec), true),
        };
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: if is_ext { 0.7 } else { 1.0 },
            confidence_tier: if is_ext { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            weight: 1.0, relation: Some(format!("require_relative '{}'", spec)) });
        if is_ext {
            result.nodes.push(NodeInfo { kind: NodeKind::Module, name: spec.to_string(),
                qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
                language: "ruby".to_string(), parent_name: None, params: None, return_type: None,
                modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
        }
    }

    // require 'gem_or_lib' → external module node
    let req_re = regex::Regex::new(r#"(?m)^[ \t]*require\s+['"]((?:[^'"]+))['"]\'?"#).unwrap();
    for cap in req_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let target = format!("module::{}", spec);
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: 0.7, confidence_tier: ConfidenceTier::Inferred,
            weight: 1.0, relation: Some(format!("require '{}'", spec)) });
        result.nodes.push(NodeInfo { kind: NodeKind::Module, name: spec.to_string(),
            qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
            language: "ruby".to_string(), parent_name: None, params: None, return_type: None,
            modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
    }

    Ok(result)
}

fn extract_php(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let is_test_file = file_name.ends_with("Test.php") || file_name.ends_with("Spec.php")
        || file_path.contains("/tests/") || file_path.contains("/Tests/");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: file_name.clone(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "php".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Classes, interfaces, traits, abstract classes, enums
    let class_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:abstract|final|readonly)\s+)?(?:class|interface|trait|enum)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in class_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "php".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: is_test_file, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Functions and methods
    let func_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|static|abstract|final)\s+)*function\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap();
    for cap in func_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let window = &source_str[off.saturating_sub(120)..off];
        let is_test = is_test_file || window.contains("@test") || window.contains("@Test") || name.starts_with("test");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "php".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // use Namespace\Class statements → external module
    let use_re = regex::Regex::new(r"(?m)^[ \t]*use\s+([A-Za-z_\\][A-Za-z0-9_\\]*)").unwrap();
    for cap in use_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let mod_name = spec.split('\\').last().unwrap_or(spec);
        let target = format!("module::{}", spec);
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: 0.7, confidence_tier: ConfidenceTier::Inferred,
            weight: 1.0, relation: Some(format!("use {}", spec)) });
        result.nodes.push(NodeInfo { kind: NodeKind::Module, name: mod_name.to_string(),
            qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
            language: "php".to_string(), parent_name: None, params: None, return_type: None,
            modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
    }

    // require / include with relative paths → resolve to file
    let req_re = regex::Regex::new(r#"(?m)^[ \t]*(?:require|require_once|include|include_once)\s*\(?\s*['"]((?:[^'"]+))['"]\'?"#).unwrap();
    for cap in req_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let resolved = if spec.starts_with('.') {
            path.parent().and_then(|p| { let c = p.join(spec); if c.exists() { Some(c) } else { None } })
        } else { None };
        let (target, is_ext) = match &resolved {
            Some(p) => (p.to_string_lossy().to_string(), false),
            None => (format!("module::{}", spec), true),
        };
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: if is_ext { 0.7 } else { 1.0 },
            confidence_tier: if is_ext { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            weight: 1.0, relation: Some(format!("require '{}'", spec)) });
        if is_ext {
            result.nodes.push(NodeInfo { kind: NodeKind::Module, name: spec.to_string(),
                qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
                language: "php".to_string(), parent_name: None, params: None, return_type: None,
                modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
        }
    }

    Ok(result)
}

fn extract_kotlin(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let is_test_file = file_name.ends_with("Test.kt") || file_name.ends_with("Spec.kt")
        || file_path.contains("/test/") || file_path.contains("/androidTest/");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: file_name.clone(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "kotlin".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Classes, interfaces, objects, data classes, sealed classes, enum classes
    let class_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|internal|abstract|sealed|open|data|inner|companion|annotation|value|external)\s+)*(?:class|interface|object|enum\s+class)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in class_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "kotlin".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: is_test_file, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Functions (including suspend, inline, operator, infix, tailrec)
    let func_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:public|private|protected|internal|override|inline|suspend|operator|infix|tailrec|external|expect|actual)\s+)*fun\s+(?:<[^>]*>\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*[(<]").unwrap();
    for cap in func_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let window = &source_str[off.saturating_sub(120)..off];
        let is_test = is_test_file || window.contains("@Test") || name.starts_with("test");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "kotlin".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // typealias declarations
    let type_re = regex::Regex::new(r"(?m)^[ \t]*typealias\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in type_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Type, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "kotlin".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: Some("typealias".to_string()),
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // import statements (package-based; Kotlin has no relative file imports)
    let import_re = regex::Regex::new(r"(?m)^[ \t]*import\s+([A-Za-z_][A-Za-z0-9_.]*)").unwrap();
    for cap in import_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let mod_name = spec.split('.').last().unwrap_or(spec);
        let target = format!("module::{}", spec);
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: 0.7, confidence_tier: ConfidenceTier::Inferred,
            weight: 1.0, relation: Some(format!("import {}", spec)) });
        result.nodes.push(NodeInfo { kind: NodeKind::Module, name: mod_name.to_string(),
            qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
            language: "kotlin".to_string(), parent_name: None, params: None, return_type: None,
            modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
    }

    Ok(result)
}

fn extract_swift(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let is_test_file = file_name.ends_with("Tests.swift") || file_name.ends_with("Spec.swift")
        || file_path.contains("Tests/") || file_path.contains("/test/");
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: file_name.clone(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "swift".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: is_test_file, file_hash: file_hash.to_string(),
    });

    // Classes, structs, enums, protocols, extensions, actors
    let class_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:public|private|internal|fileprivate|open|final|@frozen)\s+)*(?:class|struct|enum|protocol|extension|actor)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in class_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Class, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "swift".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: is_test_file, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // Functions (func keyword, including async, throws, static, mutating, etc.)
    let func_re = regex::Regex::new(r"(?m)^[ \t]*(?:(?:public|private|internal|fileprivate|open|override|static|class|final|mutating|nonmutating|async|rethrows|@discardableResult)\s+)*func\s+([A-Za-z_][A-Za-z0-9_]*)\s*[<(]").unwrap();
    for cap in func_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let off = cap.get(0).unwrap().start();
        let line = line_of(off);
        let is_test = is_test_file || name.starts_with("test");
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: if is_test { NodeKind::Test } else { NodeKind::Function },
            name: name.to_string(), qualified_name: qname.clone(), file_path: file_path.clone(),
            line_start: line, line_end: line, language: "swift".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // typealias declarations
    let type_re = regex::Regex::new(r"(?m)^[ \t]*typealias\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in type_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Type, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "swift".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: Some("typealias".to_string()),
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // import statements (Swift imports are framework/module level only; no relative file imports)
    let import_re = regex::Regex::new(r"(?m)^[ \t]*import\s+([A-Za-z_][A-Za-z0-9_.]*)").unwrap();
    for cap in import_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let target = format!("module::{}", spec);
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: 0.7, confidence_tier: ConfidenceTier::Inferred,
            weight: 1.0, relation: Some(format!("import {}", spec)) });
        result.nodes.push(NodeInfo { kind: NodeKind::Module, name: spec.to_string(),
            qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
            language: "swift".to_string(), parent_name: None, params: None, return_type: None,
            modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
    }

    Ok(result)
}

fn extract_bash(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let total_lines = source.iter().filter(|&&b| b == b'\n').count() as i32 + 1;
    let source_str = match std::str::from_utf8(source) { Ok(s) => s, Err(_) => return Ok(result) };
    let line_of = |off: usize| source_str[..off.min(source_str.len())].matches('\n').count() as i32 + 1;

    result.nodes.push(NodeInfo {
        kind: NodeKind::File, name: file_name.clone(),
        qualified_name: file_path.clone(), file_path: file_path.clone(),
        line_start: 1, line_end: total_lines, language: "bash".to_string(),
        parent_name: None, params: None, return_type: None, modifiers: None,
        is_test: false, file_hash: file_hash.to_string(),
    });

    // Function definitions: `function foo {`, `foo() {`, `function foo() {`
    let func_re = regex::Regex::new(r"(?m)^[ \t]*(?:function\s+)?([A-Za-z_][A-Za-z0-9_:-]*)\s*\(\s*\)").unwrap();
    for cap in func_re.captures_iter(source_str) {
        let name = cap.get(1).unwrap().as_str();
        if matches!(name, "if" | "while" | "for" | "case" | "select" | "do"
            | "done" | "fi" | "esac" | "then" | "else" | "elif" | "in" | "until") { continue; }
        let line = line_of(cap.get(0).unwrap().start());
        let qname = format!("{}::{}", file_path, name);
        result.nodes.push(NodeInfo { kind: NodeKind::Function, name: name.to_string(), qualified_name: qname.clone(),
            file_path: file_path.clone(), line_start: line, line_end: line, language: "bash".to_string(),
            parent_name: Some(file_path.clone()), params: None, return_type: None, modifiers: None,
            is_test: false, file_hash: file_hash.to_string() });
        result.edges.push(EdgeInfo { kind: EdgeKind::Contains, source_qualified: file_path.clone(),
            target_qualified: qname, file_path: file_path.clone(), line, confidence: 1.0,
            confidence_tier: ConfidenceTier::Extracted, weight: 1.0, relation: None });
    }

    // source / . (dot) sourcing of other scripts → resolve relative to current file
    let source_re = regex::Regex::new(r#"(?m)^[ \t]*(?:source|\.)\s+["']?([^"' \t\n#$\\]+)["']?"#).unwrap();
    for cap in source_re.captures_iter(source_str) {
        let spec = cap.get(1).unwrap().as_str();
        if spec.is_empty() || spec.contains('$') || spec.contains('{') { continue; }
        let line = line_of(cap.get(0).unwrap().start());
        let resolved = path.parent().and_then(|p| { let c = p.join(spec); if c.exists() { Some(c) } else { None } });
        let (target, is_ext) = match &resolved {
            Some(p) => (p.to_string_lossy().to_string(), false),
            None => (format!("module::{}", spec), true),
        };
        result.edges.push(EdgeInfo { kind: EdgeKind::ImportsFrom, source_qualified: file_path.clone(),
            target_qualified: target.clone(), file_path: file_path.clone(), line,
            confidence: if is_ext { 0.7 } else { 1.0 },
            confidence_tier: if is_ext { ConfidenceTier::Inferred } else { ConfidenceTier::Extracted },
            weight: 1.0, relation: Some(format!("source '{}'", spec)) });
        if is_ext {
            result.nodes.push(NodeInfo { kind: NodeKind::Module, name: spec.to_string(),
                qualified_name: target, file_path: file_path.clone(), line_start: line, line_end: line,
                language: "bash".to_string(), parent_name: None, params: None, return_type: None,
                modifiers: Some("external".to_string()), is_test: false, file_hash: file_hash.to_string() });
        }
    }

    Ok(result)
}
