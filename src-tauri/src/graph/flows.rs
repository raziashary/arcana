// Flows layer — execution flow detection and data flow analysis

use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::store::GraphStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    pub qualified_name: String,
    pub label: String,
    pub file_path: String,
    pub entry_type: String, // "main", "handler", "api_endpoint", "test", "cli_command"
    pub downstream_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPath {
    pub entry_point: String,
    pub path: Vec<String>,
    pub depth: usize,
    pub has_loop: bool,
    pub has_conditional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlow {
    pub source_node: String,
    pub sink_node: String,
    pub path: Vec<String>,
    pub data_type: String,
}

pub fn detect_entry_points(store: &GraphStore) -> Result<Vec<EntryPoint>> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for edge in &all_edges {
        *in_degree.entry(edge.target_qualified.clone()).or_insert(0) += 1;
    }
    
    let mut entry_points = Vec::new();
    
    for node in all_nodes {
        let entry_type = if node.name == "main" {
            "main"
        } else if node.name.contains("handler") || node.name.contains("Handler") {
            "handler"
        } else if node.name.contains("endpoint") || node.name.contains("route") {
            "api_endpoint"
        } else if node.is_test {
            "test"
        } else if node.name.contains("cli") || node.name.contains("command") {
            "cli_command"
        } else {
            continue;
        };
        
        // Count downstream nodes
        let mut downstream = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(node.qualified_name.clone());
        
        while let Some(current) = queue.pop_front() {
            if downstream.contains(&current) {
                continue;
            }
            downstream.insert(current.clone());
            
            for edge in &all_edges {
                if edge.source_qualified == current && edge.kind == "Calls" {
                    queue.push_back(edge.target_qualified.clone());
                }
            }
        }
        
        entry_points.push(EntryPoint {
            qualified_name: node.qualified_name,
            label: node.name,
            file_path: node.file_path,
            entry_type: entry_type.to_string(),
            downstream_count: downstream.len(),
        });
    }
    
    entry_points.sort_by(|a, b| b.downstream_count.cmp(&a.downstream_count));
    Ok(entry_points)
}

pub fn trace_execution_path(
    store: &GraphStore,
    entry_point: &str,
    max_depth: usize,
) -> Result<ExecutionPath> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    let mut path = vec![entry_point.to_string()];
    let mut queue = VecDeque::new();
    queue.push_back((entry_point.to_string(), 0));
    let mut visited = HashSet::new();
    let mut has_loop = false;
    let mut has_conditional = false;
    
    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            break;
        }
        
        if visited.contains(&current) {
            has_loop = true;
            continue;
        }
        visited.insert(current.clone());
        
        for edge in &all_edges {
            if edge.source_qualified == current && edge.kind == "Calls" {
                path.push(edge.target_qualified.clone());
                
                // Detect conditionals (heuristic: multiple outgoing calls)
                let outgoing = all_edges
                    .iter()
                    .filter(|e| e.source_qualified == current && e.kind == "Calls")
                    .count();
                if outgoing > 1 {
                    has_conditional = true;
                }
                
                queue.push_back((edge.target_qualified.clone(), depth + 1));
            }
        }
    }
    
    Ok(ExecutionPath {
        entry_point: entry_point.to_string(),
        path,
        depth: max_depth,
        has_loop,
        has_conditional,
    })
}

pub fn analyze_data_flows(
    store: &GraphStore,
    max_hops: usize,
) -> Result<Vec<DataFlow>> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let mut data_flows = Vec::new();
    
    // Simple heuristic: find paths from imports to calls
    for edge in &all_edges {
        if edge.kind == "ImportsFrom" {
            let mut path = vec![edge.source_qualified.clone(), edge.target_qualified.clone()];
            let mut queue = VecDeque::new();
            queue.push_back((edge.target_qualified.clone(), 1));
            let mut visited = HashSet::new();
            visited.insert(edge.source_qualified.clone());
            visited.insert(edge.target_qualified.clone());
            
            while let Some((current, hops)) = queue.pop_front() {
                if hops >= max_hops {
                    break;
                }
                
                for next_edge in &all_edges {
                    if next_edge.source_qualified == current && !visited.contains(&next_edge.target_qualified) {
                        visited.insert(next_edge.target_qualified.clone());
                        path.push(next_edge.target_qualified.clone());
                        queue.push_back((next_edge.target_qualified.clone(), hops + 1));
                    }
                }
            }
            
            if path.len() > 2 {
                data_flows.push(DataFlow {
                    source_node: edge.source_qualified.clone(),
                    sink_node: path.last().unwrap().clone(),
                    path,
                    data_type: "unknown".to_string(),
                });
            }
        }
    }
    
    Ok(data_flows)
}

pub struct FlowAnalyzer;

impl FlowAnalyzer {
    pub fn analyze_flows(_graph_data: &str) -> Result<()> {
        // TODO: Implement flow analysis
        Ok(())
    }
}
