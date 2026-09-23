// Registry layer — multi-repo graph management

use super::store::GraphStore;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use crate::state::lock::LockTolerant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub path: String,
    pub alias: Option<String>,
    pub db_path: String,
    pub last_updated: Option<u64>,
    pub node_count: Option<u32>,
    pub edge_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeStats {
    pub repos_merged: usize,
    pub nodes_merged: usize,
    pub edges_merged: usize,
    pub cross_repo_edges: usize,
}

pub struct RepoRegistry {
    db_path: PathBuf,
    store_cache: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<GraphStore>>>>>,
}

impl RepoRegistry {
    pub fn load() -> Result<Self> {
        let registry_path = dirs::home_dir()
            .ok_or_else(|| crate::errors::ArcanaError::Agent("Cannot find home dir".to_string()))?
            .join(".arcana")
            .join("registry.json");

        // Create if doesn't exist
        if !registry_path.exists() {
            std::fs::create_dir_all(registry_path.parent().unwrap())?;
            let empty_registry = serde_json::json!({ "repos": [] });
            std::fs::write(
                &registry_path,
                serde_json::to_string_pretty(&empty_registry)?,
            )?;
        }

        Ok(RepoRegistry {
            db_path: registry_path,
            store_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn register(&self, path: &Path, alias: Option<&str>) -> Result<RepoEntry> {
        // Validate path
        if !path.exists() {
            return Err(crate::errors::ArcanaError::Agent(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        let db_path = path.join(".arcana").join("graph.db");

        let mut registry: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&self.db_path)?)?;

        let entry = RepoEntry {
            path: path.to_string_lossy().to_string(),
            alias: alias.map(|s| s.to_string()),
            db_path: db_path.to_string_lossy().to_string(),
            last_updated: None,
            node_count: None,
            edge_count: None,
        };

        if let Some(repos) = registry["repos"].as_array_mut() {
            // Check if already registered
            if !repos.iter().any(|r| r["path"] == entry.path) {
                repos.push(serde_json::to_value(&entry)?);
            }
        }

        std::fs::write(&self.db_path, serde_json::to_string_pretty(&registry)?)?;
        Ok(entry)
    }

    pub fn unregister(&self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let mut registry: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&self.db_path)?)?;

        if let Some(repos) = registry["repos"].as_array_mut() {
            repos.retain(|r| r["path"] != path_str);
        }

        std::fs::write(&self.db_path, serde_json::to_string_pretty(&registry)?)?;

        // Remove from cache
        let mut cache = self.store_cache.lock_tolerant();
        cache.remove(path);

        Ok(())
    }

    pub fn list(&self) -> Result<Vec<RepoEntry>> {
        let registry: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&self.db_path)?)?;

        let entries = registry["repos"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|r| serde_json::from_value(r.clone()).ok())
            .collect();

        Ok(entries)
    }

    pub fn get_store(&self, path: &Path) -> Result<Arc<Mutex<GraphStore>>> {
        let mut cache = self.store_cache.lock_tolerant();

        if let Some(store) = cache.get(path) {
            return Ok(store.clone());
        }

        let db_path = path.join(".arcana").join("graph.db");
        let store = Arc::new(Mutex::new(GraphStore::open(&db_path)?));
        cache.insert(path.to_path_buf(), store.clone());

        // Limit cache to 10 entries (LRU)
        if cache.len() > 10 {
            if let Some(oldest_key) = cache.keys().next().cloned() {
                cache.remove(&oldest_key);
            }
        }

        Ok(store)
    }
}

pub fn merge_graphs(stores: &[&GraphStore], output_path: &Path) -> Result<()> {
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();

    for store in stores {
        let nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
        let edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

        all_nodes.extend(nodes);
        all_edges.extend(edges);
    }

    let export_data = serde_json::json!({
        "nodes": all_nodes,
        "edges": all_edges,
    });

    let json_str = serde_json::to_string_pretty(&export_data)?;
    std::fs::write(output_path, json_str)?;
    Ok(())
}

pub fn merge_all_repos(registry: &RepoRegistry, output_path: &Path) -> Result<MergeStats> {
    let entries = registry.list()?;
    let mut all_nodes: Vec<_> = Vec::new();
    let mut all_edges: Vec<_> = Vec::new();

    for entry in &entries {
        let path = PathBuf::from(&entry.path);
        if let Ok(store_arc) = registry.get_store(&path) {
            let store = store_arc.lock_tolerant();
            let nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
            let edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

            all_nodes.extend(nodes);
            all_edges.extend(edges);
        }
    }

    let export_data = serde_json::json!({
        "nodes": all_nodes,
        "edges": all_edges,
    });

    let json_str = serde_json::to_string_pretty(&export_data)?;
    std::fs::write(output_path, json_str)?;

    Ok(MergeStats {
        repos_merged: entries.len(),
        nodes_merged: all_nodes.len(),
        edges_merged: all_edges.len(),
        cross_repo_edges: 0, // TODO: count cross-repo edges
    })
}

pub struct GraphRegistry;

impl GraphRegistry {
    pub fn manage_repos(_repo_path: &str) -> Result<()> {
        // TODO: Implement multi-repo registry
        Ok(())
    }
}
