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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConfidenceTier {
    Extracted,
    Inferred,
    Ambiguous,
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
const EXTRACTOR_VERSION: u32 = 2;

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
    let cache_dir = dirs::home_dir().unwrap().join(".autodev/graph_cache");
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

pub fn extract(path: &Path) -> Result<ExtractionResult> {
    let file_hash = hash_file(path)?;
    
    // Check cache
    if let Some(cached) = load_cached(path, &file_hash) {
        return Ok(cached);
    }
    
    let source = fs::read(path)?;
    let language = detect_language(path)?;
    
    let result = match language.as_str() {
        "python" => extract_python(path, &source, &file_hash)?,
        "javascript" | "typescript" => extract_js_ts(path, &source, &file_hash)?,
        "rust" => extract_rust(path, &source, &file_hash)?,
        "go" => extract_go(path, &source, &file_hash)?,
        "java" => extract_java(path, &source, &file_hash)?,
        "cpp" | "c" => extract_c_cpp(path, &source, &file_hash)?,
        "ruby" => extract_ruby(path, &source, &file_hash)?,
        "php" => extract_php(path, &source, &file_hash)?,
        "kotlin" => extract_kotlin(path, &source, &file_hash)?,
        "swift" => extract_swift(path, &source, &file_hash)?,
        "bash" => extract_bash(path, &source, &file_hash)?,
        _ => ExtractionResult::default(),
    };
    
    // Cache result
    let _ = save_cached(path, &file_hash, &result);
    
    Ok(result)
}

pub fn extract_all(paths: &[PathBuf]) -> Vec<(PathBuf, Result<ExtractionResult>)> {
    paths
        .par_iter()
        .map(|p| (p.clone(), extract(p)))
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

fn extract_python(path: &Path, source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
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
    
    // 5. Extract `from X import Y` statements
    for cap in from_import_re.captures_iter(source_str) {
        let module = cap.get(1).unwrap().as_str();
        let imported = cap.get(2).unwrap().as_str();
        let line = line_of(cap.get(0).unwrap().start());
        let target = format!("module::{}", module);
        
        result.edges.push(EdgeInfo {
            kind: EdgeKind::ImportsFrom,
            source_qualified: file_path.clone(),
            target_qualified: target.clone(),
            file_path: file_path.clone(),
            line,
            confidence: 0.8,
            confidence_tier: ConfidenceTier::Inferred,
            weight: 1.0,
            relation: Some(format!("from {} import {}", module, imported.trim())),
        });
        
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
    
    let _ = module_name; // suppress unused warning
    Ok(result)
}

fn extract_js_ts(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let language = if path.extension().map(|e| e.to_str()) == Some(Some("ts")) || 
                      path.extension().map(|e| e.to_str()) == Some(Some("tsx")) {
        "typescript"
    } else {
        "javascript"
    };
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: language.to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_rust(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "rust".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_go(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "go".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_java(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "java".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_c_cpp(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    let language = if path.extension().map(|e| e.to_str()) == Some(Some("c")) {
        "c"
    } else {
        "cpp"
    };
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: language.to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_ruby(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "ruby".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_php(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "php".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_kotlin(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "kotlin".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_swift(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "swift".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}

fn extract_bash(path: &Path, _source: &[u8], file_hash: &str) -> Result<ExtractionResult> {
    let mut result = ExtractionResult::default();
    let file_path = path.to_string_lossy().to_string();
    
    result.nodes.push(NodeInfo {
        kind: NodeKind::File,
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        qualified_name: file_path.clone(),
        file_path: file_path.clone(),
        line_start: 1,
        line_end: 1,
        language: "bash".to_string(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: file_hash.to_string(),
    });
    
    Ok(result)
}
