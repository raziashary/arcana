// Changes layer — diff and change detection for incremental updates

use super::store::GraphStore;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedNode {
    pub qualified_name: String,
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub risk_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeAnalysis {
    pub changed_nodes: Vec<ChangedNode>,
    pub affected_nodes: Vec<String>,
    pub test_coverage_gaps: Vec<String>,
    pub total_risk_score: f32,
}

/// Parse a unified `git diff` output into a map of `file_path -> [(start_line, end_line), ...]`
/// using the `+++ b/<path>` header to attribute hunks to files, and `@@` hunk headers for ranges.
pub fn parse_git_diff_ranges(diff_output: &str) -> HashMap<String, Vec<(u32, u32)>> {
    let mut ranges: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut current_file: Option<String> = None;

    for line in diff_output.lines() {
        // Track the current file from `+++ b/<path>` header
        if let Some(rest) = line.strip_prefix("+++ ") {
            // Skip /dev/null (deletions)
            if rest == "/dev/null" {
                current_file = None;
                continue;
            }
            // Strip leading `b/` if present
            let path = rest.strip_prefix("b/").unwrap_or(rest).to_string();
            current_file = Some(path);
            continue;
        }

        // Parse hunk header: @@ -10,5 +20,7 @@
        if line.starts_with("@@") {
            let Some(file) = current_file.clone() else {
                continue;
            };

            // Find the `+start,count` portion
            if let Some(plus_idx) = line.find('+') {
                let after_plus = &line[plus_idx + 1..];
                let end_idx = after_plus.find(' ').unwrap_or(after_plus.len());
                let range_part = &after_plus[..end_idx];

                let parts: Vec<&str> = range_part.split(',').collect();
                if let Ok(start) = parts[0].parse::<u32>() {
                    let count = if parts.len() > 1 {
                        parts[1].parse::<u32>().unwrap_or(1)
                    } else {
                        1
                    };
                    let end = start + count.saturating_sub(1).max(0);
                    ranges
                        .entry(file)
                        .or_default()
                        .push((start, end.max(start)));
                }
            }
        }
    }

    ranges
}

pub fn find_changed_nodes(
    store: &GraphStore,
    file_path: &str,
    line_ranges: &[(u32, u32)],
) -> Result<Vec<ChangedNode>> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let mut changed = Vec::new();

    for node in all_nodes {
        if node.file_path == file_path {
            for (start, end) in line_ranges {
                let node_start = node.line_start as u32;
                let node_end = node.line_end as u32;
                if node_start >= *start && node_end <= *end {
                    changed.push(ChangedNode {
                        qualified_name: node.qualified_name,
                        file_path: node.file_path,
                        line_start: node_start,
                        line_end: node_end,
                        risk_score: 0.5, // TODO: compute actual risk
                    });
                    break;
                }
            }
        }
    }

    Ok(changed)
}

pub fn score_change_risk(store: &GraphStore, changed_nodes: &[ChangedNode]) -> Result<f32> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let mut total_risk = 0.0;

    for node in changed_nodes {
        // Count incoming edges (callers)
        let callers = all_edges
            .iter()
            .filter(|e| e.target_qualified == node.qualified_name && e.kind == "Calls")
            .count();

        // Count tests
        let tests = all_edges
            .iter()
            .filter(|e| e.source_qualified == node.qualified_name && e.kind == "TestedBy")
            .count();

        // Risk = callers / (tests + 1)
        let risk = callers as f32 / (tests as f32 + 1.0);
        total_risk += risk;
    }

    Ok(total_risk / changed_nodes.len().max(1) as f32)
}

/// Run `git diff <base_ref>` against the working tree, map hunks to graph nodes
/// (changed_nodes), then compute the impact radius (affected_nodes, test gaps, risk).
pub fn detect_changes(
    store: &GraphStore,
    repo_root: &str,
    base_ref: &str,
) -> Result<ChangeAnalysis> {
    // Validate base_ref to prevent shell injection (same rules as incremental.rs)
    if !regex::Regex::new(r"^[A-Za-z0-9_.~^/@{}\-]+$")
        .unwrap()
        .is_match(base_ref)
    {
        return Err(anyhow::anyhow!("Invalid git ref: {}", base_ref).into());
    }

    // STEP 1: Run `git diff` to get a unified diff with hunk headers
    let output = Command::new("git")
        .args(["diff", "--unified=0", base_ref])
        .current_dir(Path::new(repo_root))
        .output()?;

    if !output.status.success() {
        tracing::warn!(
            "[detect_changes] git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(ChangeAnalysis {
            changed_nodes: Vec::new(),
            affected_nodes: Vec::new(),
            test_coverage_gaps: Vec::new(),
            total_risk_score: 0.0,
        });
    }

    let diff = String::from_utf8_lossy(&output.stdout);
    let ranges_by_file = parse_git_diff_ranges(&diff);

    if ranges_by_file.is_empty() {
        return Ok(ChangeAnalysis {
            changed_nodes: Vec::new(),
            affected_nodes: Vec::new(),
            test_coverage_gaps: Vec::new(),
            total_risk_score: 0.0,
        });
    }

    // STEP 2: For each changed file, find graph nodes that overlap with the diff hunks.
    // The DB stores absolute paths; the diff yields repo-relative paths. Try both.
    let mut all_changed_nodes: Vec<ChangedNode> = Vec::new();
    let repo_root_path = Path::new(repo_root);

    for (relative_path, ranges) in &ranges_by_file {
        // Try absolute path first (matches what `extract` stores), then relative.
        let abs_path = repo_root_path
            .join(relative_path)
            .to_string_lossy()
            .to_string();

        let mut found = find_changed_nodes(store, &abs_path, ranges)?;
        if found.is_empty() {
            found = find_changed_nodes(store, relative_path, ranges)?;
        }
        all_changed_nodes.extend(found);
    }

    // STEP 3: Compute affected nodes via reverse-edge BFS (callers/dependents).
    // Reuse the existing `get_impact_radius` analyzer to walk the graph.
    let mut affected_set: HashSet<String> = HashSet::new();
    let mut tests_to_run: HashSet<String> = HashSet::new();
    let mut coverage_gaps: HashSet<String> = HashSet::new();

    for node in &all_changed_nodes {
        let radius = super::analyze::get_impact_radius(store, &node.qualified_name, 5, 200)?;
        for an in &radius.affected_nodes {
            affected_set.insert(an.qualified_name.clone());
        }
        for t in radius.tests_to_run {
            tests_to_run.insert(t);
        }
        for g in radius.test_coverage_gaps {
            coverage_gaps.insert(g);
        }
    }

    // STEP 4: Risk score = average risk across changed nodes (callers / (tests + 1))
    let total_risk_score = score_change_risk(store, &all_changed_nodes)?;

    let _ = tests_to_run; // ChangeAnalysis doesn't have a `tests_to_run` field; kept for future use

    Ok(ChangeAnalysis {
        changed_nodes: all_changed_nodes,
        affected_nodes: affected_set.into_iter().collect(),
        test_coverage_gaps: coverage_gaps.into_iter().collect(),
        total_risk_score,
    })
}

pub struct ChangeDetector;

impl ChangeDetector {
    /// Convenience wrapper that runs `detect_changes` against HEAD~1.
    pub fn detect_changes(store: &GraphStore, repo_root: &str) -> Result<ChangeAnalysis> {
        detect_changes(store, repo_root, "HEAD~1")
    }
}
