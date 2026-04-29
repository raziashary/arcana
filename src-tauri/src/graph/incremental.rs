// Incremental layer — incremental graph updates

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::store::GraphStore;
use super::extract;
use super::build;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStats {
    pub files_changed: usize,
    pub nodes_added: usize,
    pub nodes_removed: usize,
    pub edges_added: usize,
    pub edges_removed: usize,
    pub elapsed_ms: u64,
}

pub fn get_git_changed_files(repo_root: &Path, base_ref: &str) -> Result<Vec<PathBuf>> {
    // Validate base_ref to prevent shell injection
    if !regex::Regex::new(r"^[A-Za-z0-9_.~^/@{}\-]+$")
        .unwrap()
        .is_match(base_ref)
    {
        return Err(anyhow::anyhow!("Invalid git ref: {}", base_ref).into());
    }
    
    let output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg(base_ref)
        .current_dir(repo_root)
        .output()?;
    
    if !output.status.success() {
        tracing::warn!("[IncrementalUpdate] git diff failed, returning empty list");
        return Ok(Vec::new());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<PathBuf> = stdout
        .lines()
        .map(|line| repo_root.join(line))
        .collect();
    
    tracing::info!("[IncrementalUpdate] Found {} changed files via git", files.len());
    Ok(files)
}

pub fn find_changed_by_hash(store: &GraphStore, files: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut changed = Vec::new();
    
    for file_path in files {
        if let Ok(new_hash) = extract::hash_file(file_path) {
            let old_hash = store.get_file_hash(&file_path.to_string_lossy());
            match old_hash {
                Ok(Some(old_hash_str)) => {
                    if old_hash_str != new_hash {
                        changed.push(file_path.clone());
                        tracing::debug!("[IncrementalUpdate] File changed: {:?}", file_path);
                    }
                }
                _ => {
                    // File not in database yet, treat as new
                    changed.push(file_path.clone());
                }
            }
        }
    }
    
    tracing::info!("[IncrementalUpdate] {} files have hash changes", changed.len());
    Ok(changed)
}

pub fn find_dependents(store: &GraphStore, changed_qualified_names: &[String]) -> Result<Vec<PathBuf>> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let mut dependent_files = HashSet::new();
    
    for changed_name in changed_qualified_names {
        for edge in &all_edges {
            if edge.target_qualified == *changed_name {
                dependent_files.insert(edge.file_path.clone());
                tracing::debug!("[IncrementalUpdate] {} depends on {}", edge.source_qualified, changed_name);
            }
        }
    }
    
    let result: Vec<PathBuf> = dependent_files
        .into_iter()
        .map(PathBuf::from)
        .collect();
    
    tracing::info!("[IncrementalUpdate] Found {} dependent files", result.len());
    Ok(result)
}

pub fn incremental_update(
    store: &GraphStore,
    changed_files: &[PathBuf],
    agent_author: Option<&str>,
    task_id: Option<i64>,
) -> Result<UpdateStats> {
    let start = std::time::Instant::now();
    
    if changed_files.is_empty() {
        return Ok(UpdateStats {
            files_changed: 0,
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }
    
    tracing::info!("[IncrementalUpdate] Updating {} changed files", changed_files.len());
    
    // STEP 1: Delete old nodes/edges for changed files
    let mut nodes_removed = 0;
    let mut edges_removed = 0;
    
    for file_path in changed_files {
        let file_path_str = file_path.to_string_lossy();
        nodes_removed += store.delete_file_nodes(&file_path_str)?;
        edges_removed += store.delete_file_edges(&file_path_str)?;
    }
    
    tracing::info!("[IncrementalUpdate] Deleted {} nodes, {} edges", nodes_removed, edges_removed);
    
    // STEP 2: Extract changed files in parallel
    tracing::info!("[IncrementalUpdate] Extracting {} changed files", changed_files.len());
    let extractions = extract::extract_all(changed_files);
    let mut extraction_results = Vec::new();
    let mut changed_qualified_names = Vec::new();
    
    for (_, result) in extractions {
        if let Ok(extraction) = result {
            for node in &extraction.nodes {
                changed_qualified_names.push(node.qualified_name.clone());
            }
            extraction_results.push(extraction);
        }
    }
    
    tracing::info!("[IncrementalUpdate] Extracted {} files, {} nodes", extraction_results.len(), changed_qualified_names.len());
    
    // STEP 3: Find dependent files (one hop)
    let dependent_files = find_dependents(store, &changed_qualified_names)?;
    let mut all_files_to_extract = changed_files.to_vec();
    all_files_to_extract.extend(dependent_files);
    all_files_to_extract.sort();
    all_files_to_extract.dedup();
    
    // STEP 4: Build graph (upsert new nodes/edges)
    let build_stats = build::build(extraction_results, store, agent_author, task_id)?;
    
    // STEP 5: Rebuild FTS5 index
    store.rebuild_fts_index()?;
    
    let elapsed_ms = start.elapsed().as_millis() as u64;
    
    tracing::info!("[IncrementalUpdate] ✅ Incremental update complete: {} nodes added, {} edges added in {}ms",
        build_stats.nodes_upserted, build_stats.edges_upserted, elapsed_ms);
    
    Ok(UpdateStats {
        files_changed: changed_files.len(),
        nodes_added: build_stats.nodes_upserted,
        nodes_removed,
        edges_added: build_stats.edges_upserted,
        edges_removed,
        elapsed_ms,
    })
}
