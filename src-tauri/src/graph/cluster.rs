// Clustering layer — Community detection using modularity optimization

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::store::GraphStore;

const EDGE_WEIGHTS: &[(&str, f32)] = &[
    ("Calls", 1.0),
    ("ImportsFrom", 0.5),
    ("Inherits", 0.8),
    ("Implements", 0.7),
    ("Contains", 0.3),
    ("TestedBy", 0.4),
    ("DependsOn", 0.6),
    ("References", 0.4),
];

const COMMON_WORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "is", "are", "was", "were", "be", "been",
    "being", "have", "has", "had", "do", "does", "did", "will", "would", "could",
    "should", "may", "might", "must", "can", "get", "set", "make", "take", "use",
    "impl", "fn", "pub", "mod", "struct", "enum", "trait", "type", "const", "let",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    pub id: i32,
    pub name: String,
    pub members: Vec<String>,
    pub cohesion_score: f32,
}

pub fn build_adjacency(
    store: &GraphStore,
) -> Result<HashMap<String, Vec<(String, f32)>>> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let mut adjacency: HashMap<String, Vec<(String, f32)>> = HashMap::new();
    
    for edge in all_edges {
        let weight = EDGE_WEIGHTS
            .iter()
            .find(|(k, _)| k == &edge.kind)
            .map(|(_, w)| *w)
            .unwrap_or(0.5);
        
        adjacency
            .entry(edge.source_qualified.clone())
            .or_insert_with(Vec::new)
            .push((edge.target_qualified.clone(), weight));
        
        adjacency
            .entry(edge.target_qualified.clone())
            .or_insert_with(Vec::new)
            .push((edge.source_qualified.clone(), weight));
    }
    
    Ok(adjacency)
}

pub fn detect_communities(
    adjacency: &HashMap<String, Vec<(String, f32)>>,
) -> HashMap<String, i32> {
    let mut communities: HashMap<String, i32> = HashMap::new();
    let mut community_id = 0i32;
    let mut visited = HashSet::new();
    
    // Simple greedy clustering: BFS-based community detection
    for node in adjacency.keys() {
        if visited.contains(node) {
            continue;
        }
        
        let mut community_members = Vec::new();
        let mut queue = vec![node.clone()];
        
        while let Some(current) = queue.pop() {
            if visited.contains(&current) {
                continue;
            }
            
            visited.insert(current.clone());
            community_members.push(current.clone());
            communities.insert(current.clone(), community_id);
            
            if let Some(neighbors) = adjacency.get(&current) {
                for (neighbor, _weight) in neighbors {
                    if !visited.contains(neighbor) {
                        queue.push(neighbor.clone());
                    }
                }
            }
        }
        
        if !community_members.is_empty() {
            community_id += 1;
        }
    }
    
    communities
}

pub fn split_oversized(
    adjacency: &HashMap<String, Vec<(String, f32)>>,
    communities: &HashMap<String, i32>,
    threshold_fraction: f32,
) -> HashMap<String, i32> {
    let total_nodes = adjacency.len() as f32;
    let threshold_size = (total_nodes * threshold_fraction) as usize;
    
    let mut result = communities.clone();
    let mut community_sizes: HashMap<i32, usize> = HashMap::new();
    
    for (_, &comm_id) in communities {
        *community_sizes.entry(comm_id).or_insert(0) += 1;
    }
    
    for (comm_id, size) in community_sizes {
        if size > threshold_size && size >= 10 {
            // Re-cluster this community in isolation
            let members: Vec<String> = communities
                .iter()
                .filter(|(_, &c)| c == comm_id)
                .map(|(n, _)| n.clone())
                .collect();
            
            let sub_adjacency: HashMap<String, Vec<(String, f32)>> = adjacency
                .iter()
                .filter(|(k, _)| members.contains(k))
                .map(|(k, v)| {
                    let filtered: Vec<(String, f32)> = v
                        .iter()
                        .filter(|(target, _)| members.contains(target))
                        .cloned()
                        .collect();
                    (k.clone(), filtered)
                })
                .collect();
            
            let sub_communities = detect_communities(&sub_adjacency);
            let mut new_comm_id = comm_id + 1000;
            
            for (node, _) in sub_communities {
                result.insert(node, new_comm_id);
                new_comm_id += 1;
            }
        }
    }
    
    result
}

pub fn cohesion_score(
    store: &GraphStore,
    member_qualified_names: &[String],
) -> Result<f32> {
    if member_qualified_names.len() < 2 {
        return Ok(1.0);
    }
    
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let member_set: HashSet<_> = member_qualified_names.iter().cloned().collect();
    
    let intra_edges = all_edges
        .iter()
        .filter(|e| {
            member_set.contains(&e.source_qualified) && member_set.contains(&e.target_qualified)
        })
        .count();
    
    let n = member_qualified_names.len() as f32;
    let max_edges = (n * (n - 1.0) / 2.0) as usize;
    
    Ok(if max_edges > 0 {
        intra_edges as f32 / max_edges as f32
    } else {
        0.0
    })
}

pub fn name_community(
    store: &GraphStore,
    members: &[String],
) -> Result<String> {
    if members.is_empty() {
        return Ok("unnamed".to_string());
    }
    
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let member_set: HashSet<_> = members.iter().cloned().collect();
    
    let member_nodes: Vec<_> = all_nodes
        .iter()
        .filter(|n| member_set.contains(&n.qualified_name))
        .collect();
    
    // Find most common directory prefix
    let mut dir_counts: HashMap<String, usize> = HashMap::new();
    for node in &member_nodes {
        if let Some(idx) = node.file_path.rfind('/') {
            let dir = node.file_path[..idx].to_string();
            *dir_counts.entry(dir).or_insert(0) += 1;
        }
    }
    
    let prefix = dir_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(dir, _)| {
            dir.split('/').last().unwrap_or("module").to_string()
        })
        .unwrap_or_else(|| "module".to_string());
    
    // Find most common class (if > 40% of members)
    let mut class_counts: HashMap<String, usize> = HashMap::new();
    for node in &member_nodes {
        if let Some(parent) = &node.parent_name {
            *class_counts.entry(parent.clone()).or_insert(0) += 1;
        }
    }
    
    if let Some((class_name, count)) = class_counts.iter().max_by_key(|(_, c)| *c) {
        if *count as f32 / members.len() as f32 > 0.4 {
            let class_short = class_name.split("::").last().unwrap_or(class_name);
            return Ok(format!("{}-{}", prefix, class_short.to_lowercase()));
        }
    }
    
    // Find most frequent non-trivial keyword
    let mut keyword_counts: HashMap<String, usize> = HashMap::new();
    for node in &member_nodes {
        let words: Vec<&str> = node.name.split(|c: char| !c.is_alphanumeric()).collect();
        for word in words {
            if !word.is_empty() && !COMMON_WORDS.contains(&word) && word.len() > 2 {
                *keyword_counts.entry(word.to_lowercase()).or_insert(0) += 1;
            }
        }
    }
    
    let keyword = keyword_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(kw, _)| kw.clone())
        .unwrap_or_else(|| "core".to_string());
    
    Ok(format!("{}-{}", prefix, keyword))
}

pub fn run_clustering(store: &GraphStore) -> Result<Vec<Community>> {
    let adjacency = build_adjacency(store)?;
    let communities = detect_communities(&adjacency);
    let communities = split_oversized(&adjacency, &communities, 0.25);
    
    let mut community_map: HashMap<i32, Vec<String>> = HashMap::new();
    for (node, comm_id) in &communities {
        community_map
            .entry(*comm_id)
            .or_insert_with(Vec::new)
            .push(node.clone());
    }
    
    let mut result = Vec::new();
    for (comm_id, members) in community_map {
        let name = name_community(store, &members)?;
        let cohesion = cohesion_score(store, &members)?;
        
        for member in &members {
            store.set_community(member, comm_id, &name)?;
        }
        
        result.push(Community {
            id: comm_id,
            name,
            members,
            cohesion_score: cohesion,
        });
    }
    
    result.sort_by(|a, b| b.members.len().cmp(&a.members.len()));
    Ok(result)
}

pub struct Clusterer;

impl Clusterer {
    pub fn cluster_graph() -> Result<()> {
        // TODO: Implement Leiden clustering
        Ok(())
    }
}
