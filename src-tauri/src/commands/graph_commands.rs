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
    
    if code_files.is_empty() {
        tracing::warn!("[GraphBuild] No code files found in repository: {}", repo_path);
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
    
    // STEP 3: Build graph (upsert nodes/edges to database)
    tracing::info!("[GraphBuild] Building graph from {} extractions...", extraction_results.len());
    let build_start = std::time::Instant::now();
    let build_stats = crate::graph::build::build(extraction_results, &store, None, None)?;
    tracing::info!("[GraphBuild] Graph built: {} nodes, {} edges in {}ms", 
        build_stats.nodes_upserted, build_stats.edges_upserted, build_start.elapsed().as_millis());
    
    // STEP 4: Run clustering
    tracing::info!("[GraphBuild] Running clustering...");
    let cluster_start = std::time::Instant::now();
    let communities = run_clustering(&store)?;
    tracing::info!("[GraphBuild] Clustering complete: {} communities ({}ms)", communities.len(), cluster_start.elapsed().as_millis());
    
    // Emit event to frontend
    tracing::debug!("[GraphBuild] Emitting graph:updated event");
    let _ = app.emit("graph:updated", ());
    
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
    
    // Database goes in user's home .autodev directory, NOT repo root
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let autodev_dir = home_dir.join(".autodev");
    
    // Create .autodev directory if it doesn't exist
    std::fs::create_dir_all(&autodev_dir)?;
    
    let db_path = autodev_dir.join("graph.db");
    
    let store = GraphStore::open(&db_path)?;
    
    // Perform incremental update
    let changed_files = crate::graph::incremental::get_git_changed_files(repo_dir, "HEAD~1")?;
    
    if changed_files.is_empty() {
        tracing::info!("[UpdateGraph] No changed files detected");
        return Ok(UpdateStats {
            files_changed: 0,
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
            elapsed_ms: 0,
        });
    }
    
    let stats = crate::graph::incremental::incremental_update(&store, &changed_files, None, None)?;
    
    let _ = app.emit("graph:updated", ());
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
    
    let communities = run_clustering(&store)?;
    
    tracing::info!("[GraphStats] ✅ Stats complete: {} nodes, {} edges, {} communities", total_nodes, total_edges, communities.len());
    
    Ok(GraphStats {
        total_nodes,
        total_edges,
        total_communities: communities.len(),
        average_degree,
        graph_density,
        last_updated: Some(chrono::Local::now().to_rfc3339()),
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
    
    // Fetch communities
    let communities = run_clustering(&store)?;
    tracing::info!("[GraphViz] Fetched {} communities", communities.len());
    
    // Convert nodes to visualization format
    let viz_nodes: Vec<VisualizationNode> = all_nodes
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
    
    // Convert edges to visualization format
    let viz_edges: Vec<VisualizationEdge> = all_edges
        .iter()
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
