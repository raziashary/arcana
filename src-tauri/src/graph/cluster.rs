// Clustering layer — Community detection using modularity optimization

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::store::GraphStore;

const EDGE_WEIGHTS: &[(&str, f32)] = &[
    ("CALLS", 1.0),
    ("IMPORTS_FROM", 0.5),
    ("INHERITS", 0.8),
    ("IMPLEMENTS", 0.7),
    ("CONTAINS", 0.3),
    ("TESTED_BY", 0.4),
    ("DEPENDS_ON", 0.6),
    ("REFERENCES", 0.4),
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

// ─── Private helpers for community detection ─────────────────────────────────

/// Build adjacency excluding `module::` stub nodes and edges below `min_weight`.
fn build_filtered_adjacency(
    adjacency: &HashMap<String, Vec<(String, f32)>>,
    min_weight: f32,
) -> HashMap<String, Vec<(String, f32)>> {
    adjacency
        .iter()
        .filter(|(k, _)| !k.starts_with("module::"))
        .map(|(k, neighbors)| {
            let filtered: Vec<(String, f32)> = neighbors
                .iter()
                .filter(|(t, w)| !t.starts_with("module::") && *w >= min_weight)
                .cloned()
                .collect();
            (k.clone(), filtered)
        })
        .collect()
}

/// BFS connected components over an adjacency map, returning sorted deterministic results.
fn connected_components_bfs(
    adjacency: &HashMap<String, Vec<(String, f32)>>,
) -> Vec<Vec<String>> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut components: Vec<Vec<String>> = Vec::new();
    let mut nodes: Vec<&String> = adjacency.keys().collect();
    nodes.sort();

    for start in nodes {
        if visited.contains(start) {
            continue;
        }
        let mut component: Vec<String> = Vec::new();
        let mut stack = vec![start.clone()];
        while let Some(current) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            component.push(current.clone());
            if let Some(neighbors) = adjacency.get(&current) {
                for (neighbor, _) in neighbors {
                    if !visited.contains(neighbor) {
                        stack.push(neighbor.clone());
                    }
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components
}

/// Extract a sub-adjacency from `adjacency` containing only `members` nodes and edges ≥ `min_weight`.
fn subgraph_filtered(
    adjacency: &HashMap<String, Vec<(String, f32)>>,
    members: &[String],
    min_weight: f32,
) -> HashMap<String, Vec<(String, f32)>> {
    let member_set: HashSet<&String> = members.iter().collect();
    members
        .iter()
        .map(|node| {
            let neighbors = adjacency
                .get(node)
                .map(|ns| {
                    ns.iter()
                        .filter(|(t, w)| member_set.contains(t) && *w >= min_weight)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            (node.clone(), neighbors)
        })
        .collect()
}

/// Assign orphan nodes (id == -2) to the best neighboring community by weighted vote.
/// Truly isolated nodes with no reachable community get a new unique community each.
fn reassign_orphans(
    assignments: &mut HashMap<String, i32>,
    code_adj: &HashMap<String, Vec<(String, f32)>>,
    next_id: &mut i32,
) {
    let mut orphans: Vec<String> = assignments
        .iter()
        .filter(|(_, &id)| id == -2)
        .map(|(n, _)| n.clone())
        .collect();
    orphans.sort();

    for node in &orphans {
        let mut comm_weight: HashMap<i32, f32> = HashMap::new();
        if let Some(neighbors) = code_adj.get(node) {
            for (neighbor, weight) in neighbors {
                if let Some(&comm_id) = assignments.get(neighbor) {
                    if comm_id >= 0 {
                        *comm_weight.entry(comm_id).or_insert(0.0) += weight;
                    }
                }
            }
        }
        let best = comm_weight
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, _)| id);

        if let Some(comm_id) = best {
            assignments.insert(node.clone(), comm_id);
        } else {
            assignments.insert(node.clone(), *next_id);
            *next_id += 1;
        }
    }
}

/// Assign `module::` stub nodes to the community of their most-frequent importer.
/// Stubs with no known importers get a new unique community each.
fn assign_module_nodes(
    full_adjacency: &HashMap<String, Vec<(String, f32)>>,
    assignments: &mut HashMap<String, i32>,
    next_id: &mut i32,
) {
    let mut module_nodes: Vec<String> = full_adjacency
        .keys()
        .filter(|k| k.starts_with("module::"))
        .cloned()
        .collect();
    module_nodes.sort();

    for node in &module_nodes {
        let mut comm_votes: HashMap<i32, usize> = HashMap::new();
        if let Some(neighbors) = full_adjacency.get(node) {
            for (neighbor, _) in neighbors {
                if let Some(&comm_id) = assignments.get(neighbor) {
                    if comm_id >= 0 {
                        *comm_votes.entry(comm_id).or_insert(0) += 1;
                    }
                }
            }
        }
        let assigned = comm_votes
            .iter()
            .max_by_key(|(_, &v)| v)
            .map(|(&id, _)| id)
            .unwrap_or_else(|| {
                let id = *next_id;
                *next_id += 1;
                id
            });
        assignments.insert(node.clone(), assigned);
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Multi-phase modularity-aware community detection.
///
/// **Phase 1 — Strong-edge connected components:**  
/// Build adjacency with edges ≥ 0.45 weight (retains IMPORTS_FROM/0.5, CALLS/1.0,
/// INHERITS/0.8, IMPLEMENTS/0.7, DEPENDS_ON/0.6; drops CONTAINS/0.3 and REFERENCES/0.4).
/// All `module::` stub nodes are excluded to prevent external imports bridging
/// unrelated files into a single mega-component.
///
/// **Phase 2 — Oversized-component splitting:**  
/// Any component > max(15, 20% of non-module nodes) is re-clustered on its own
/// subgraph using only high-coupling edges (≥ 0.65: CALLS, INHERITS, IMPLEMENTS).
///
/// **Phase 3 — Orphan re-attachment:**  
/// Nodes isolated in Phase 1/2 (degree 0 after filtering) are attached to the
/// best neighbor community by weighted vote in the full non-module adjacency
/// (which includes CONTAINS edges so functions attach to their parent file's community).
///
/// **Phase 4 — Module stub placement:**  
/// External `module::` nodes are placed in the community of their most common
/// importer so they appear within the correct community hull in the visualisation.
pub fn detect_communities(
    adjacency: &HashMap<String, Vec<(String, f32)>>,
) -> HashMap<String, i32> {
    let total_code_nodes = adjacency
        .keys()
        .filter(|k| !k.starts_with("module::"))
        .count();

    // Phase 1: connected components on strong-signal edges only.
    // Threshold 0.45 cuts CONTAINS(0.3) and REFERENCES(0.4) so parent→child
    // containment can't collapse the entire graph into one component.
    let strong_adj = build_filtered_adjacency(adjacency, 0.45);
    let components = connected_components_bfs(&strong_adj);

    let oversized_threshold = ((total_code_nodes as f32 * 0.20) as usize).max(15);
    let mut assignments: HashMap<String, i32> = HashMap::new();
    let mut next_id = 0i32;

    for component in &components {
        if component.len() > oversized_threshold {
            // Phase 2: re-split using only high-coupling edges.
            // Threshold 0.65 keeps CALLS(1.0), INHERITS(0.8), IMPLEMENTS(0.7).
            let sub_adj = subgraph_filtered(adjacency, component, 0.65);
            let sub_components = connected_components_bfs(&sub_adj);
            for sub in &sub_components {
                if sub.len() >= 2 {
                    for node in sub {
                        assignments.insert(node.clone(), next_id);
                    }
                    next_id += 1;
                } else {
                    // Single-node residual — mark for Phase 3 orphan re-attachment
                    assignments.insert(sub[0].clone(), -2);
                }
            }
        } else if component.len() >= 2 {
            for node in component {
                assignments.insert(node.clone(), next_id);
            }
            next_id += 1;
        } else {
            // Single-node component — mark for Phase 3
            assignments.insert(component[0].clone(), -2);
        }
    }

    // Phase 3: re-attach orphans via the full non-module adjacency (includes CONTAINS).
    let code_full_adj = build_filtered_adjacency(adjacency, 0.0);
    reassign_orphans(&mut assignments, &code_full_adj, &mut next_id);

    // Phase 4: place module:: stubs in the community of their most frequent importer.
    assign_module_nodes(adjacency, &mut assignments, &mut next_id);

    assignments
}

pub fn cohesion_score(
    _store: &GraphStore,
    member_qualified_names: &[String],
    all_edges: &[crate::graph::store::GraphEdge],
) -> Result<f32> {
    if member_qualified_names.len() < 2 {
        return Ok(1.0);
    }
    
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
    _store: &GraphStore,
    members: &[String],
    all_nodes: &[crate::graph::store::GraphNode],
) -> Result<String> {
    if members.is_empty() {
        return Ok("unnamed".to_string());
    }
    
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
    // detect_communities handles multi-phase splitting and orphan re-attachment internally
    let communities = detect_communities(&adjacency);
    
    let mut community_map: HashMap<i32, Vec<String>> = HashMap::new();
    for (node, comm_id) in &communities {
        community_map
            .entry(*comm_id)
            .or_insert_with(Vec::new)
            .push(node.clone());
    }
    
    // Pre-fetch all nodes and edges ONCE to avoid N+1 queries
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    
    // Merge tiny communities (< 3 members) into a single "miscellaneous" group
    const MIN_COMMUNITY_SIZE: usize = 3;
    let mut misc_members: Vec<String> = Vec::new();
    let mut significant_communities: Vec<(i32, Vec<String>)> = Vec::new();
    
    for (comm_id, members) in community_map {
        if members.len() < MIN_COMMUNITY_SIZE {
            misc_members.extend(members);
        } else {
            significant_communities.push((comm_id, members));
        }
    }
    
    if !misc_members.is_empty() {
        significant_communities.push((-1, misc_members)); // -1 = miscellaneous
    }
    
    let mut result = Vec::new();
    let mut batch_assignments: Vec<(String, i32, String)> = Vec::new();
    
    for (comm_id, members) in significant_communities {
        let name = if comm_id == -1 {
            "miscellaneous".to_string()
        } else {
            name_community(store, &members, &all_nodes)?
        };
        let cohesion = cohesion_score(store, &members, &all_edges)?;
        
        // Collect for batch update instead of N+1 individual writes
        for member in &members {
            batch_assignments.push((member.clone(), comm_id, name.clone()));
        }
        
        result.push(Community {
            id: comm_id,
            name,
            members,
            cohesion_score: cohesion,
        });
    }
    
    // Single batch update — ~100x faster than looped set_community
    store.set_communities_batch(&batch_assignments)?;
    
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
