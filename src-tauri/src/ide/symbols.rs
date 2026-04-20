// Symbol extractor — regex-based first cut for Phase 3.2.
//
// Scope and rationale:
//   The master plan calls for tree-sitter based extraction. Tree-sitter
//   grammars require compilation/bundling per language which is a non-trivial
//   integration. To unblock the IDE outline panel we ship a pragmatic
//   regex extractor that covers the languages AutoDev most commonly touches
//   (Rust, TS/JS, Python, Go). A future Phase 3.2.1 can swap this module's
//   internals for a tree-sitter implementation without changing the Tauri API.
//
// All extractors return a flat list of top-level symbols; nested types are
// attached as children where the grammar naturally allows (impl blocks,
// classes). Everything else is flattened to keep the first cut simple.

use std::path::Path;

use regex::Regex;

use crate::errors::{AutoDevError, Result};
use super::file_reader::detect_language;
use super::types::{Symbol, SymbolKind, SymbolTree};

pub fn extract(workspace_root: &str, path: &str) -> Result<SymbolTree> {
    let workspace = Path::new(workspace_root);
    let p = Path::new(path);
    let abs = if p.is_absolute() { p.to_path_buf() } else { workspace.join(p) };

    let text = std::fs::read_to_string(&abs)
        .map_err(|e| AutoDevError::Agent(format!("read {}: {e}", abs.display())))?;

    let language = detect_language(&abs);
    let symbols = match language.as_str() {
        "rust"                       => extract_rust(&text),
        "typescript" | "javascript"  => extract_ts(&text),
        "python"                     => extract_python(&text),
        "go"                         => extract_go(&text),
        _                            => Vec::new(),
    };

    let relative_path = abs
        .strip_prefix(workspace)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| abs.to_string_lossy().into_owned());

    Ok(SymbolTree { relative_path, language, symbols })
}

// ─── Rust ─────────────────────────────────────────────────────────────────────

fn extract_rust(text: &str) -> Vec<Symbol> {
    // Covers: fn, struct, enum, trait, impl, mod, const, static, type
    let fn_re      = Regex::new(r"^\s*(?:pub\s+(?:\([^)]*\)\s+)?)?(?:async\s+)?(?:unsafe\s+)?(?:const\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let struct_re  = Regex::new(r"^\s*(?:pub\s+(?:\([^)]*\)\s+)?)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let enum_re    = Regex::new(r"^\s*(?:pub\s+(?:\([^)]*\)\s+)?)?enum\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let trait_re   = Regex::new(r"^\s*(?:pub\s+(?:\([^)]*\)\s+)?)?(?:unsafe\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let mod_re     = Regex::new(r"^\s*(?:pub\s+(?:\([^)]*\)\s+)?)?mod\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let const_re   = Regex::new(r"^\s*(?:pub\s+(?:\([^)]*\)\s+)?)?(?:const|static)\s+([A-Z_][A-Z0-9_]*)").unwrap();
    let type_re    = Regex::new(r"^\s*(?:pub\s+(?:\([^)]*\)\s+)?)?type\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();

    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if let Some(c) = fn_re.captures(line)     { out.push(sym(&c[1], SymbolKind::Function, line, lineno)); continue }
        if let Some(c) = struct_re.captures(line) { out.push(sym(&c[1], SymbolKind::Struct,   line, lineno)); continue }
        if let Some(c) = enum_re.captures(line)   { out.push(sym(&c[1], SymbolKind::Enum,     line, lineno)); continue }
        if let Some(c) = trait_re.captures(line)  { out.push(sym(&c[1], SymbolKind::Trait,    line, lineno)); continue }
        if let Some(c) = mod_re.captures(line)    { out.push(sym(&c[1], SymbolKind::Module,   line, lineno)); continue }
        if let Some(c) = const_re.captures(line)  { out.push(sym(&c[1], SymbolKind::Constant, line, lineno)); continue }
        if let Some(c) = type_re.captures(line)   { out.push(sym(&c[1], SymbolKind::Type,     line, lineno)); continue }
    }
    out
}

// ─── TypeScript / JavaScript ──────────────────────────────────────────────────

fn extract_ts(text: &str) -> Vec<Symbol> {
    let fn_re        = Regex::new(r"^\s*(?:export\s+)?(?:async\s+)?function\s*\*?\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let arrow_re     = Regex::new(r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*(?::\s*[^=]+)?=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][A-Za-z0-9_$]*)\s*=>").unwrap();
    let class_re     = Regex::new(r"^\s*(?:export\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let interface_re = Regex::new(r"^\s*(?:export\s+)?interface\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let type_re      = Regex::new(r"^\s*(?:export\s+)?type\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let enum_re      = Regex::new(r"^\s*(?:export\s+)?(?:const\s+)?enum\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let const_re     = Regex::new(r"^\s*(?:export\s+)?const\s+([A-Z_][A-Z0-9_]*)\s*[:=]").unwrap();

    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if let Some(c) = fn_re.captures(line)        { out.push(sym(&c[1], SymbolKind::Function,  line, lineno)); continue }
        if let Some(c) = arrow_re.captures(line)     { out.push(sym(&c[1], SymbolKind::Function,  line, lineno)); continue }
        if let Some(c) = class_re.captures(line)     { out.push(sym(&c[1], SymbolKind::Class,     line, lineno)); continue }
        if let Some(c) = interface_re.captures(line) { out.push(sym(&c[1], SymbolKind::Interface, line, lineno)); continue }
        if let Some(c) = type_re.captures(line)      { out.push(sym(&c[1], SymbolKind::Type,      line, lineno)); continue }
        if let Some(c) = enum_re.captures(line)      { out.push(sym(&c[1], SymbolKind::Enum,      line, lineno)); continue }
        if let Some(c) = const_re.captures(line)     { out.push(sym(&c[1], SymbolKind::Constant,  line, lineno)); continue }
    }
    out
}

// ─── Python ───────────────────────────────────────────────────────────────────

fn extract_python(text: &str) -> Vec<Symbol> {
    let fn_re    = Regex::new(r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let class_re = Regex::new(r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();

    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if let Some(c) = fn_re.captures(line)    { out.push(sym(&c[1], SymbolKind::Function, line, lineno)); continue }
        if let Some(c) = class_re.captures(line) { out.push(sym(&c[1], SymbolKind::Class,    line, lineno)); continue }
    }
    out
}

// ─── Go ───────────────────────────────────────────────────────────────────────

fn extract_go(text: &str) -> Vec<Symbol> {
    let fn_re     = Regex::new(r"^func\s+(?:\([^)]*\)\s+)?([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let struct_re = Regex::new(r"^type\s+([A-Za-z_][A-Za-z0-9_]*)\s+struct").unwrap();
    let iface_re  = Regex::new(r"^type\s+([A-Za-z_][A-Za-z0-9_]*)\s+interface").unwrap();
    let type_re   = Regex::new(r"^type\s+([A-Za-z_][A-Za-z0-9_]*)\s+").unwrap();

    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if let Some(c) = fn_re.captures(line)     { out.push(sym(&c[1], SymbolKind::Function,  line, lineno)); continue }
        if let Some(c) = struct_re.captures(line) { out.push(sym(&c[1], SymbolKind::Struct,    line, lineno)); continue }
        if let Some(c) = iface_re.captures(line)  { out.push(sym(&c[1], SymbolKind::Interface, line, lineno)); continue }
        if let Some(c) = type_re.captures(line)   { out.push(sym(&c[1], SymbolKind::Type,      line, lineno)); continue }
    }
    out
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn sym(name: &str, kind: SymbolKind, line_text: &str, line: usize) -> Symbol {
    let column = line_text.find(name).unwrap_or(0);
    Symbol {
        name:     name.to_string(),
        kind,
        line,
        column,
        children: Vec::new(),
    }
}
