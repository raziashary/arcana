// Graph Tauri commands — expose graph operations to frontend

use tauri::{AppHandle, Emitter};
use crate::errors::Result;
use crate::graph::store::GraphStore;
use crate::graph::cluster::{run_clustering, Community};
use crate::graph::analyze::{god_nodes, surprising_connections, suggest_questions};
use crate::graph::report::{generate_report, export_report_markdown};
use crate::graph::export::{export_json, export_html, export_svg};
use crate::graph::registry::{RepoRegistry, RepoEntry, merge_all_repos, MergeStats};
use crate::graph::incremental::UpdateStats;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStats {
    pub nodes_extracted: usize,
    pub edges_extracted: usize,
    pub communities_detected: usize,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_communities: usize,
    pub average_degree: f32,
    pub graph_density: f32,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub language: String,
    pub community: Option<i64>,
    pub community_name: Option<String>,
    pub file_path: String,
    pub size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphVisualizationData {
    pub nodes: Vec<VisualizationNode>,
    pub edges: Vec<VisualizationEdge>,
    pub communities: Vec<Community>,
}

// ============================================================================
// BUILD & UPDATE COMMANDS
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildProgressPayload {
    pub phase: String,
    pub message: String,
    pub percent: u8,
}

#[tauri::command]
pub async fn build_graph(app: AppHandle, repo_path: Option<String>) -> Result<BuildStats> {
    // Validate repo_path — must be provided and not "."
    let repo_path = repo_path
        .and_then(|p| if p.is_empty() || p == "." { None } else { Some(p) })
        .ok_or_else(|| anyhow::anyhow!(
            "❌ Workspace directory not set or invalid. Please go to Settings → Workspace → Pick Directory"
        ))?;
    
    // Verify the path exists
    let repo_dir = Path::new(&repo_path);
    if !repo_dir.exists() {
        return Err(anyhow::anyhow!(
            "❌ Workspace directory does not exist: {}",
            repo_path
        ).into());
    }
    if !repo_dir.is_dir() {
        return Err(anyhow::anyhow!(
            "❌ Workspace path is not a directory: {}",
            repo_path
        ).into());
    }
    
    tracing::info!("[GraphBuild] Starting graph build for repo: {}", repo_path);
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "init".to_string(),
        message: format!("Opening database for {}...", repo_path),
        percent: 5,
    });
    
    // Database goes in user's home .autodev directory, NOT repo root
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let autodev_dir = home_dir.join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    tracing::debug!("[GraphBuild] Creating .autodev directory at: {:?}", autodev_dir);
    std::fs::create_dir_all(&autodev_dir)?;
    tracing::info!("[GraphBuild] .autodev directory ready");
    
    let db_path = autodev_dir.join("graph.db");
    tracing::debug!("[GraphBuild] Database path: {:?}", db_path);
    
    let start = std::time::Instant::now();
    tracing::info!("[GraphBuild] Opening GraphStore...");
    let store = GraphStore::open(&db_path)?;
    tracing::info!("[GraphBuild] GraphStore opened successfully ({}ms)", start.elapsed().as_millis());
    
    // STEP 1: Detect code files
    tracing::info!("[GraphBuild] Detecting code files in: {}", repo_path);
    let detect_start = std::time::Instant::now();
    let detector = crate::graph::detect::CodeDetector::new(repo_dir);
    let code_files = detector.detect_files()?;
    tracing::info!("[GraphBuild] Detected {} code files ({}ms)", code_files.len(), detect_start.elapsed().as_millis());
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "detecting".to_string(),
        message: format!("Found {} code files", code_files.len()),
        percent: 15,
    });
    
    if code_files.is_empty() {
        tracing::warn!("[GraphBuild] No code files found in repository: {}", repo_path);
        let _ = app.emit("graph:build_progress", BuildProgressPayload {
            phase: "done".to_string(),
            message: "No code files found".to_string(),
            percent: 100,
        });
        let _ = app.emit("graph:updated", ());
        return Ok(BuildStats {
            nodes_extracted: 0,
            edges_extracted: 0,
            communities_detected: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }
    
    // STEP 2: Extract code from all files
    tracing::info!("[GraphBuild] Extracting code from {} files...", code_files.len());
    let extract_start = std::time::Instant::now();
    let extractions = crate::graph::extract::extract_all(&code_files);
    let mut extraction_results = Vec::new();
    let mut extraction_errors = 0;
    
    for (file_path, result) in extractions {
        match result {
            Ok(extraction) => {
                extraction_results.push(extraction);
            }
            Err(e) => {
                tracing::warn!("[GraphBuild] Failed to extract {}: {}", file_path.display(), e);
                extraction_errors += 1;
            }
        }
    }
    
    tracing::info!("[GraphBuild] Extracted {} files ({} errors) in {}ms", 
        extraction_results.len(), extraction_errors, extract_start.elapsed().as_millis());
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "extracting".to_string(),
        message: format!("Extracted {} files ({} errors)", extraction_results.len(), extraction_errors),
        percent: 30,
    });
    
    // STEP 3: Build graph (upsert nodes/edges to database)
    tracing::info!("[GraphBuild] Building graph from {} extractions...", extraction_results.len());
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "building".to_string(),
        message: "Building graph nodes and edges...".to_string(),
        percent: 40,
    });
    let build_start = std::time::Instant::now();
    let build_stats = crate::graph::build::build(extraction_results, &store, None, None)?;
    tracing::info!("[GraphBuild] Graph built: {} nodes, {} edges in {}ms", 
        build_stats.nodes_upserted, build_stats.edges_upserted, build_start.elapsed().as_millis());
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "building".to_string(),
        message: format!("Upserted {} nodes, {} edges", build_stats.nodes_upserted, build_stats.edges_upserted),
        percent: 60,
    });
    
    // STEP 4: Run clustering
    tracing::info!("[GraphBuild] Running clustering...");
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "clustering".to_string(),
        message: "Detecting communities...".to_string(),
        percent: 75,
    });
    let cluster_start = std::time::Instant::now();
    let communities = run_clustering(&store)?;
    tracing::info!("[GraphBuild] Clustering complete: {} communities ({}ms)", communities.len(), cluster_start.elapsed().as_millis());
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "clustering".to_string(),
        message: format!("Found {} communities", communities.len()),
        percent: 90,
    });
    
    // Emit event to frontend
    tracing::debug!("[GraphBuild] Emitting graph:updated event");
    let _ = app.emit("graph:updated", ());
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "done".to_string(),
        message: "Graph build complete".to_string(),
        percent: 100,
    });
    
    let total_elapsed = start.elapsed().as_millis() as u64;
    tracing::info!("[GraphBuild] ✅ Graph build complete in {}ms", total_elapsed);
    
    Ok(BuildStats {
        nodes_extracted: build_stats.nodes_upserted,
        edges_extracted: build_stats.edges_upserted,
        communities_detected: communities.len(),
        elapsed_ms: total_elapsed,
    })
}

#[tauri::command]
pub async fn update_graph(app: AppHandle, repo_path: Option<String>) -> Result<UpdateStats> {
    let start = std::time::Instant::now();
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "init".to_string(),
        message: "Checking workspace...".to_string(),
        percent: 5,
    });
    
    // Validate repo_path — must be provided and not "."
    let repo_path = repo_path
        .and_then(|p| if p.is_empty() || p == "." { None } else { Some(p) })
        .ok_or_else(|| anyhow::anyhow!(
            "❌ Workspace directory not set or invalid. Please go to Settings → Workspace → Pick Directory"
        ))?;
    
    // Verify the path exists
    let repo_dir = Path::new(&repo_path);
    if !repo_dir.exists() || !repo_dir.is_dir() {
        return Err(anyhow::anyhow!(
            "❌ Workspace directory does not exist or is invalid: {}",
            repo_path
        ).into());
    }
    
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "detecting".to_string(),
        message: "Finding changed files...".to_string(),
        percent: 15,
    });
    
    // Database goes in user's home .autodev directory, NOT repo root
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let autodev_dir = home_dir.join(".autodev");
    std::fs::create_dir_all(&autodev_dir)?;
    let db_path = autodev_dir.join("graph.db");
    let store = GraphStore::open(&db_path)?;
    
    // Perform incremental update
    let changed_files = crate::graph::incremental::get_git_changed_files(repo_dir, "HEAD~1")?;
    
    if changed_files.is_empty() {
        tracing::info!("[UpdateGraph] No changed files detected");
        let _ = app.emit("graph:build_progress", BuildProgressPayload {
            phase: "done".to_string(),
            message: "No changed files".to_string(),
            percent: 100,
        });
        let _ = app.emit("graph:updated", ());
        return Ok(UpdateStats {
            files_changed: 0,
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
            elapsed_ms: 0,
        });
    }
    
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "building".to_string(),
        message: format!("Updating {} changed files...", changed_files.len()),
        percent: 40,
    });
    
    let stats = crate::graph::incremental::incremental_update(&store, &changed_files, None, None)?;
    
    let _ = app.emit("graph:build_progress", BuildProgressPayload {
        phase: "done".to_string(),
        message: format!("Updated: {} added, {} removed", stats.nodes_added + stats.edges_added, stats.nodes_removed + stats.edges_removed),
        percent: 100,
    });
    let _ = app.emit("graph:updated", ());
    
    let mut stats = stats;
    stats.elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(stats)
}

// ============================================================================
// STATS & INFO COMMANDS
// ============================================================================

#[tauri::command]
pub async fn get_graph_stats(repo_path: Option<String>) -> Result<GraphStats> {
    let _repo_path = repo_path.unwrap_or_else(|| ".".to_string());
    tracing::info!("[GraphStats] Getting graph stats");
    
    // Database is in user's home .autodev directory, NOT repo root
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let autodev_dir = home_dir.join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    tracing::debug!("[GraphStats] Database path: {:?}", db_path);
    
    let store = GraphStore::open(&db_path)?;
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    tracing::info!("[GraphStats] Loaded {} nodes and {} edges", all_nodes.len(), all_edges.len());
    
    let total_nodes = all_nodes.len();
    let total_edges = all_edges.len();
    let average_degree = if total_nodes > 0 {
        (total_edges as f32 * 2.0) / total_nodes as f32
    } else {
        0.0
    };
    
    let max_possible_edges = if total_nodes > 1 {
        total_nodes * (total_nodes - 1) / 2
    } else {
        0
    };
    let graph_density = if max_possible_edges > 0 {
        total_edges as f32 / max_possible_edges as f32
    } else {
        0.0
    };
    
    // Count communities from node community fields (already persisted by run_clustering
    // during build). Avoids re-running the expensive clustering algorithm.
    let total_communities = all_nodes
        .iter()
        .filter_map(|n| n.community)
        .collect::<std::collections::HashSet<i32>>()
        .len();
    
    tracing::info!("[GraphStats] ✅ Stats complete: {} nodes, {} edges, {} communities", total_nodes, total_edges, total_communities);
    
    Ok(GraphStats {
        total_nodes,
        total_edges,
        total_communities,
        average_degree,
        graph_density,
        last_updated: None,
    })
}

#[tauri::command]
pub async fn get_graph_communities(repo_path: Option<String>) -> Result<Vec<Community>> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    let communities = run_clustering(&store)?;
    
    Ok(communities)
}

#[tauri::command]
pub async fn get_graph_report(repo_path: Option<String>) -> Result<String> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    let report = generate_report(&store)?;
    let markdown = export_report_markdown(&report);
    
    Ok(markdown)
}

// ============================================================================
// EXPORT COMMANDS
// ============================================================================

#[tauri::command]
pub async fn export_graph(
    repo_path: Option<String>,
    format: String,
    output_path: Option<String>,
) -> Result<()> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    let out_path = output_path.unwrap_or_else(|| format!("graph.{}", format));
    
    let store = GraphStore::open(&db_path)?;
    let communities = run_clustering(&store)?;
    
    match format.as_str() {
        "json" => export_json(&store, Path::new(&out_path))?,
        "html" => export_html(&store, &communities, Path::new(&out_path))?,
        "svg" => export_svg(&store, &communities, Path::new(&out_path))?,
        _ => return Err(crate::errors::AutoDevError::Agent(format!("Unknown format: {}", format))),
    }
    
    Ok(())
}

// ============================================================================
// VISUALIZATION DATA COMMANDS
// ============================================================================

#[tauri::command]
pub async fn get_graph_nodes_for_viz(
    repo_path: Option<String>,
    max_nodes: u32,
) -> Result<Vec<serde_json::Value>> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    let all_nodes = store.get_all_nodes_for_graph(max_nodes as i64, 0)?;
    
    let nodes: Vec<_> = all_nodes
        .iter()
        .map(|n| serde_json::json!({
            "id": n.qualified_name,
            "label": n.name,
            "kind": n.kind,
            "file": n.file_path,
            "line": n.line_start,
            "degree": 0 // Will be calculated by frontend
        }))
        .collect();
    
    Ok(nodes)
}

#[tauri::command]
pub async fn get_graph_edges_for_viz(repo_path: Option<String>) -> Result<Vec<serde_json::Value>> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    let edges: Vec<_> = all_edges
        .iter()
        .map(|e| serde_json::json!({
            "source": e.source_qualified,
            "target": e.target_qualified,
            "kind": e.kind,
            "weight": e.weight,
            "confidence": e.confidence_tier
        }))
        .collect();
    
    Ok(edges)
}

// ============================================================================
// ANALYSIS COMMANDS
// ============================================================================

#[tauri::command]
pub async fn get_god_nodes(repo_path: Option<String>, top_n: Option<u32>) -> Result<Vec<serde_json::Value>> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    let nodes = god_nodes(&store, top_n.unwrap_or(10) as usize)?;
    
    let result: Vec<_> = nodes
        .iter()
        .map(|n| serde_json::json!({
            "name": n.label,
            "qualified_name": n.qualified_name,
            "degree": n.degree,
            "file": n.file_path
        }))
        .collect();
    
    Ok(result)
}

#[tauri::command]
pub async fn get_surprising_connections(
    repo_path: Option<String>,
    top_n: Option<u32>,
) -> Result<Vec<serde_json::Value>> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    let connections = surprising_connections(&store, top_n.unwrap_or(5) as usize)?;
    
    let result: Vec<_> = connections
        .iter()
        .map(|c| serde_json::json!({
            "source": c.source,
            "target": c.target,
            "score": c.score,
            "why": c.why
        }))
        .collect();
    
    Ok(result)
}

#[tauri::command]
pub async fn get_suggested_questions(repo_path: Option<String>) -> Result<Vec<serde_json::Value>> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let autodev_dir = Path::new(&path).join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    let questions = suggest_questions(&store, 5)?;
    
    let result: Vec<_> = questions
        .iter()
        .map(|q| serde_json::json!({
            "question": q.question,
            "type": q.question_type,
            "why": q.why
        }))
        .collect();
    
    Ok(result)
}

// ============================================================================
// REGISTRY COMMANDS (Phase 6.16)
// ============================================================================

#[tauri::command]
pub async fn register_repo(path: String, alias: Option<String>) -> Result<RepoEntry> {
    let registry = RepoRegistry::load()?;
    registry.register(Path::new(&path), alias.as_deref())
}

#[tauri::command]
pub async fn unregister_repo(path: String) -> Result<()> {
    let registry = RepoRegistry::load()?;
    registry.unregister(Path::new(&path))
}

#[tauri::command]
pub async fn list_repos() -> Result<Vec<RepoEntry>> {
    let registry = RepoRegistry::load()?;
    registry.list()
}

#[tauri::command]
pub async fn merge_all_repos_cmd(output_path: String) -> Result<MergeStats> {
    let registry = RepoRegistry::load()?;
    merge_all_repos(&registry, Path::new(&output_path))
}

// ============================================================================
// DELETE GRAPH COMMAND
// ============================================================================

#[tauri::command]
pub async fn delete_graph(repo_path: Option<String>) -> Result<()> {
    let path = repo_path.unwrap_or_else(|| ".".to_string());
    let db_path = Path::new(&path).join(".autodev").join("graph.db");
    
    if db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }
    
    Ok(())
}

// ============================================================================
// VISUALIZATION COMMAND
// ============================================================================

#[tauri::command]
pub async fn get_graph_visualization_data() -> Result<GraphVisualizationData> {
    tracing::info!("[GraphViz] Fetching visualization data");
    
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let db_path = home_dir.join(".autodev").join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    
    // Fetch all nodes
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    tracing::info!("[GraphViz] Fetched {} nodes", all_nodes.len());
    
    // Fetch all edges
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    tracing::info!("[GraphViz] Fetched {} edges", all_edges.len());
    
    // Reconstruct communities from node community fields (already written by run_clustering
    // during build/update). This avoids re-running the expensive clustering algorithm on
    // every visualization fetch, which was causing a 2+ second delay.
    let mut community_map: std::collections::HashMap<i32, (String, Vec<String>)> = std::collections::HashMap::new();
    for node in &all_nodes {
        if let Some(comm_id) = node.community {
            let entry = community_map.entry(comm_id).or_insert_with(|| {
                (node.community_name.clone().unwrap_or_else(|| "unnamed".to_string()), Vec::new())
            });
            entry.1.push(node.qualified_name.clone());
        }
    }
    
    // Build edge lookup set for cohesion computation
    let edge_pairs: std::collections::HashSet<(String, String)> = all_edges
        .iter()
        .map(|e| {
            let a = e.source_qualified.clone();
            let b = e.target_qualified.clone();
            if a < b { (a, b) } else { (b, a) }
        })
        .collect();
    
    let communities: Vec<Community> = community_map
        .into_iter()
        .map(|(id, (name, members))| {
            let member_set: std::collections::HashSet<_> = members.iter().cloned().collect();
            let intra_edges = edge_pairs
                .iter()
                .filter(|(a, b)| member_set.contains(a) && member_set.contains(b))
                .count();
            let n = members.len() as f32;
            let max_edges = (n * (n - 1.0) / 2.0).max(1.0) as usize;
            let cohesion = intra_edges as f32 / max_edges as f32;
            Community { id, name, members, cohesion_score: cohesion }
        })
        .collect();
    tracing::info!("[GraphViz] Reconstructed {} communities from DB", communities.len());
    
    // Filter out external module:: nodes — they clutter the visualization
    // and are not part of the user's codebase.
    let project_nodes: Vec<_> = all_nodes
        .iter()
        .filter(|node| !node.qualified_name.starts_with("module::"))
        .collect();
    
    // Build set of project node IDs so we can filter edges too
    let project_node_ids: std::collections::HashSet<String> = project_nodes
        .iter()
        .map(|n| n.qualified_name.clone())
        .collect();
    
    // Convert nodes to visualization format
    let viz_nodes: Vec<VisualizationNode> = project_nodes
        .iter()
        .map(|node| {
            // Size based on node kind (files are larger)
            let size = if node.kind.contains("File") { 15.0 } else { 8.0 };
            
            VisualizationNode {
                id: node.qualified_name.clone(),
                label: node.name.clone(),
                kind: node.kind.clone(),
                language: node.language.clone(),
                community: node.community.map(|c| c as i64),
                community_name: node.community_name.clone(),
                file_path: node.file_path.clone(),
                size,
            }
        })
        .collect();
    
    // Convert edges to visualization format — only include edges where BOTH
    // source and target are project nodes (not filtered-out module:: nodes)
    let viz_edges: Vec<VisualizationEdge> = all_edges
        .iter()
        .filter(|edge| {
            project_node_ids.contains(&edge.source_qualified) &&
            project_node_ids.contains(&edge.target_qualified)
        })
        .map(|edge| VisualizationEdge {
            source: edge.source_qualified.clone(),
            target: edge.target_qualified.clone(),
            kind: edge.kind.clone(),
            weight: edge.weight,
        })
        .collect();
    
    tracing::info!("[GraphViz] ✅ Visualization data ready: {} nodes, {} edges, {} communities",
        viz_nodes.len(), viz_edges.len(), communities.len());
    
    Ok(GraphVisualizationData {
        nodes: viz_nodes,
        edges: viz_edges,
        communities,
    })
}

// ============================================================================
// IDE NAVIGATION COMMAND
// ============================================================================

#[tauri::command]
pub async fn open_in_ide(app: AppHandle, file: String, line: u32) -> Result<()> {
    // Emit event to IDE tab
    let _ = app.emit("ide:navigate", serde_json::json!({
        "file": file,
        "line": line
    }));
    
    Ok(())
}
