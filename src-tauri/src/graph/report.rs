// Report layer — graph analysis report generation

use super::analyze::{god_nodes, suggest_questions, surprising_connections};
use super::cluster::run_clustering;
use super::flows::detect_entry_points;
use super::store::GraphStore;
use crate::errors::Result;
use chrono;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphReport {
    pub title: String,
    pub generated_at: String,
    pub summary: String,
    pub statistics: ReportStatistics,
    pub sections: Vec<ReportSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportStatistics {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub total_communities: usize,
    pub average_degree: f32,
    pub graph_density: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSection {
    pub title: String,
    pub content: String,
    pub items: Vec<String>,
}

pub fn generate_report(store: &GraphStore) -> Result<GraphReport> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

    let total_nodes = all_nodes.len();
    let total_edges = all_edges.len();
    let average_degree = if total_nodes > 0 {
        (total_edges as f32 * 2.0) / total_nodes as f32
    } else {
        0.0
    };

    let max_possible_edges = total_nodes * (total_nodes - 1) / 2;
    let graph_density = if max_possible_edges > 0 {
        total_edges as f32 / max_possible_edges as f32
    } else {
        0.0
    };

    // Get communities
    let communities = run_clustering(store)?;
    let total_communities = communities.len();

    let mut sections = Vec::new();

    // Section 1: God Nodes
    let god = god_nodes(store, 10)?;
    let god_items: Vec<String> = god
        .iter()
        .map(|n| format!("{} (degree: {}, file: {})", n.label, n.degree, n.file_path))
        .collect();
    sections.push(ReportSection {
        title: "Top 10 Most Connected Nodes (God Nodes)".to_string(),
        content: "These nodes have the highest number of connections and are critical to the system architecture.".to_string(),
        items: god_items,
    });

    // Section 2: Communities
    let community_items: Vec<String> = communities
        .iter()
        .take(10)
        .map(|c| {
            format!(
                "{} ({} members, cohesion: {:.2})",
                c.name,
                c.members.len(),
                c.cohesion_score
            )
        })
        .collect();
    sections.push(ReportSection {
        title: "Detected Communities".to_string(),
        content:
            "The codebase is organized into these logical communities based on connection patterns."
                .to_string(),
        items: community_items,
    });

    // Section 3: Surprising Connections
    let surprising = surprising_connections(store, 5)?;
    let surprising_items: Vec<String> = surprising
        .iter()
        .map(|c| {
            format!(
                "{} → {} (score: {:.2}, reason: {})",
                c.source,
                c.target,
                c.score,
                c.why.join(", ")
            )
        })
        .collect();
    sections.push(ReportSection {
        title: "Surprising Cross-Module Connections".to_string(),
        content: "These unexpected relationships may indicate architectural issues or opportunities for refactoring.".to_string(),
        items: surprising_items,
    });

    // Section 4: Entry Points
    let entry_points = detect_entry_points(store)?;
    let entry_items: Vec<String> = entry_points
        .iter()
        .take(10)
        .map(|e| {
            format!(
                "{} ({}, {} downstream nodes)",
                e.label, e.entry_type, e.downstream_count
            )
        })
        .collect();
    sections.push(ReportSection {
        title: "System Entry Points".to_string(),
        content: "These are the main entry points into the system (main functions, handlers, API endpoints, tests).".to_string(),
        items: entry_items,
    });

    // Section 5: Suggested Questions
    let questions = suggest_questions(store, 5)?;
    let question_items: Vec<String> = questions
        .iter()
        .map(|q| format!("{}: {}", q.question_type, q.question))
        .collect();
    sections.push(ReportSection {
        title: "Suggested Investigation Questions".to_string(),
        content:
            "These questions highlight areas that may need further investigation or clarification."
                .to_string(),
        items: question_items,
    });

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let summary = format!(
        "Graph Analysis Report: {} nodes, {} edges, {} communities, avg degree {:.2}, density {:.4}",
        total_nodes, total_edges, total_communities, average_degree, graph_density
    );

    Ok(GraphReport {
        title: "Code Knowledge Graph Analysis Report".to_string(),
        generated_at: now,
        summary,
        statistics: ReportStatistics {
            total_nodes,
            total_edges,
            total_communities,
            average_degree,
            graph_density,
        },
        sections,
    })
}

pub fn export_report_markdown(report: &GraphReport) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", report.title));
    md.push_str(&format!("**Generated:** {}\n\n", report.generated_at));
    md.push_str(&format!("## Summary\n\n{}\n\n", report.summary));

    md.push_str("## Statistics\n\n");
    md.push_str(&format!(
        "- **Total Nodes:** {}\n",
        report.statistics.total_nodes
    ));
    md.push_str(&format!(
        "- **Total Edges:** {}\n",
        report.statistics.total_edges
    ));
    md.push_str(&format!(
        "- **Communities:** {}\n",
        report.statistics.total_communities
    ));
    md.push_str(&format!(
        "- **Average Degree:** {:.2}\n",
        report.statistics.average_degree
    ));
    md.push_str(&format!(
        "- **Graph Density:** {:.4}\n\n",
        report.statistics.graph_density
    ));

    for section in &report.sections {
        md.push_str(&format!("## {}\n\n", section.title));
        md.push_str(&format!("{}\n\n", section.content));
        for item in &section.items {
            md.push_str(&format!("- {}\n", item));
        }
        md.push_str("\n");
    }

    md
}
