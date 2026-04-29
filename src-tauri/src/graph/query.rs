// Query layer — surgical graph queries for targeted context

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::store::GraphStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub nodes: Vec<QueryNode>,
    pub edges: Vec<QueryEdge>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryNode {
    pub qualified_name: String,
    pub name: String,
    pub file_path: String,
    pub kind: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub confidence: f32,
}

pub fn query_by_name(store: &GraphStore, pattern: &str) -> Result<QueryResult> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    let matching_nodes: Vec<_> = all_nodes
        .iter()
        .filter(|n| n.name.contains(pattern) || n.qualified_name.contains(pattern))
        .collect();
    
    let matching_qnames: HashSet<_> = matching_nodes
        .iter()
        .map(|n| n.qualified_name.clone())
        .collect();
    
    let matching_edges: Vec<_> = all_edges
        .iter()
        .filter(|e| matching_qnames.contains(&e.source_qualified) || matching_qnames.contains(&e.target_qualified))
        .collect();
    
    let nodes: Vec<QueryNode> = matching_nodes
        .iter()
        .map(|n| QueryNode {
            qualified_name: n.qualified_name.clone(),
            name: n.name.clone(),
            file_path: n.file_path.clone(),
            kind: n.kind.clone(),
            language: n.language.clone(),
        })
        .collect();
    
    let edges = matching_edges
        .into_iter()
        .map(|e| QueryEdge {
            source: e.source_qualified.clone(),
            target: e.target_qualified.clone(),
            kind: e.kind.clone(),
            confidence: e.confidence,
        })
        .collect();
    
    let summary = format!("Found {} nodes matching '{}'", matching_nodes.len(), pattern);
    
    Ok(QueryResult {
        nodes,
        edges,
        summary,
    })
}

pub fn query_dependents(store: &GraphStore, qualified_name: &str) -> Result<QueryResult> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    let mut result_qnames = HashSet::new();
    result_qnames.insert(qualified_name.to_string());
    
    // BFS to find all dependents
    let mut queue = vec![qualified_name.to_string()];
    let mut visited = HashSet::new();
    
    while let Some(current) = queue.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());
        
        for edge in &all_edges {
            if edge.target_qualified == current && (edge.kind == "Calls" || edge.kind == "ImportsFrom") {
                result_qnames.insert(edge.source_qualified.clone());
                queue.push(edge.source_qualified.clone());
            }
        }
    }
    
    let node_map: HashMap<String, _> = all_nodes
        .iter()
        .map(|n| (n.qualified_name.clone(), n.clone()))
        .collect();
    
    let nodes = result_qnames
        .iter()
        .filter_map(|qname| {
            node_map.get(qname).map(|n| QueryNode {
                qualified_name: n.qualified_name.clone(),
                name: n.name.clone(),
                file_path: n.file_path.clone(),
                kind: n.kind.clone(),
                language: n.language.clone(),
            })
        })
        .collect();
    
    let edges = all_edges
        .iter()
        .filter(|e| result_qnames.contains(&e.source_qualified) && result_qnames.contains(&e.target_qualified))
        .map(|e| QueryEdge {
            source: e.source_qualified.clone(),
            target: e.target_qualified.clone(),
            kind: e.kind.clone(),
            confidence: e.confidence,
        })
        .collect();
    
    let summary = format!("Found {} dependents of '{}'", result_qnames.len() - 1, qualified_name);
    
    Ok(QueryResult {
        nodes,
        edges,
        summary,
    })
}

pub fn query_dependencies(store: &GraphStore, qualified_name: &str) -> Result<QueryResult> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    let mut result_qnames = HashSet::new();
    result_qnames.insert(qualified_name.to_string());
    
    // BFS to find all dependencies
    let mut queue = vec![qualified_name.to_string()];
    let mut visited = HashSet::new();
    
    while let Some(current) = queue.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());
        
        for edge in &all_edges {
            if edge.source_qualified == current && (edge.kind == "Calls" || edge.kind == "ImportsFrom") {
                result_qnames.insert(edge.target_qualified.clone());
                queue.push(edge.target_qualified.clone());
            }
        }
    }
    
    let node_map: HashMap<String, _> = all_nodes
        .iter()
        .map(|n| (n.qualified_name.clone(), n.clone()))
        .collect();
    
    let nodes = result_qnames
        .iter()
        .filter_map(|qname| {
            node_map.get(qname).map(|n| QueryNode {
                qualified_name: n.qualified_name.clone(),
                name: n.name.clone(),
                file_path: n.file_path.clone(),
                kind: n.kind.clone(),
                language: n.language.clone(),
            })
        })
        .collect();
    
    let edges = all_edges
        .iter()
        .filter(|e| result_qnames.contains(&e.source_qualified) && result_qnames.contains(&e.target_qualified))
        .map(|e| QueryEdge {
            source: e.source_qualified.clone(),
            target: e.target_qualified.clone(),
            kind: e.kind.clone(),
            confidence: e.confidence,
        })
        .collect();
    
    let summary = format!("Found {} dependencies of '{}'", result_qnames.len() - 1, qualified_name);
    
    Ok(QueryResult {
        nodes,
        edges,
        summary,
    })
}

pub fn query_by_community(store: &GraphStore, community_id: i32) -> Result<QueryResult> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    // Filter nodes whose `community` matches the requested id
    let community_nodes: Vec<_> = all_nodes
        .iter()
        .filter(|n| n.community == Some(community_id))
        .collect();
    
    let community_qnames: HashSet<_> = community_nodes
        .iter()
        .map(|n| n.qualified_name.clone())
        .collect();
    
    let nodes = community_nodes
        .into_iter()
        .map(|n| QueryNode {
            qualified_name: n.qualified_name.clone(),
            name: n.name.clone(),
            file_path: n.file_path.clone(),
            kind: n.kind.clone(),
            language: n.language.clone(),
        })
        .collect();
    
    let edges = all_edges
        .iter()
        .filter(|e| community_qnames.contains(&e.source_qualified) && community_qnames.contains(&e.target_qualified))
        .map(|e| QueryEdge {
            source: e.source_qualified.clone(),
            target: e.target_qualified.clone(),
            kind: e.kind.clone(),
            confidence: e.confidence,
        })
        .collect();
    
    let summary = format!("Found {} nodes in community", community_qnames.len());
    
    Ok(QueryResult {
        nodes,
        edges,
        summary,
    })
}

pub struct QueryEngine;

impl QueryEngine {
    pub fn query_graph(_query: &str) -> Result<()> {
        // TODO: Implement query engine
        Ok(())
    }
}
