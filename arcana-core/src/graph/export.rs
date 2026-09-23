// Export layer — graph export formats (JSON, GraphML, etc.)

use super::cluster::Community;
use super::store::GraphStore;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

const COMMUNITY_COLORS: &[&str] = &[
    "#4E79A7", "#F28E2B", "#E15759", "#76B7B2", "#59A14F", "#EDC948", "#B07AA1", "#FF9DA7",
    "#9C755F", "#BAB0AC",
];

const D3_SRI: &str = "sha384-OvI8Cgmozvjm0cvNQQv2aZwKlnugZ3+fy+TDo3rvKX3d5NZJ6sSMEbXlRfMq/wP";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportNode {
    pub id: String,
    pub label: String,
    pub source_file: String,
    pub source_location: String,
    pub kind: String,
    pub community: Option<i32>,
    pub community_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub confidence_tier: String,
    pub weight: f32,
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('`', "&#96;")
}

pub fn export_json(store: &GraphStore, output_path: &Path) -> Result<()> {
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

    let export_nodes: Vec<ExportNode> = all_nodes
        .iter()
        .map(|n| ExportNode {
            id: n.qualified_name.clone(),
            label: n.name.clone(),
            source_file: n.file_path.clone(),
            source_location: format!("L{}", n.line_start),
            kind: n.kind.clone(),
            community: None,
            community_name: None,
        })
        .collect();

    let export_edges: Vec<ExportEdge> = all_edges
        .iter()
        .map(|e| ExportEdge {
            source: e.source_qualified.clone(),
            target: e.target_qualified.clone(),
            kind: e.kind.clone(),
            confidence_tier: e.confidence_tier.clone(),
            weight: e.weight,
        })
        .collect();

    let export_data = serde_json::json!({
        "nodes": export_nodes,
        "edges": export_edges,
    });

    let json_str = serde_json::to_string_pretty(&export_data)?;
    std::fs::write(output_path, json_str)?;
    Ok(())
}

pub fn export_html(
    store: &GraphStore,
    _communities: &[Community],
    output_path: &Path,
) -> Result<()> {
    let all_nodes = store.get_all_nodes_for_graph(5000, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

    let nodes_json = serde_json::to_string(&all_nodes)?;
    let edges_json = serde_json::to_string(&all_edges)?;
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html>\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<title>Arcana Graph</title>\n");
    html.push_str("<style>\n");
    html.push_str(
        "body { margin: 0; padding: 0; background: #000; color: #EEE; font-family: sans-serif; }\n",
    );
    html.push_str("#container { display: flex; height: 100vh; }\n");
    html.push_str("#canvas { flex: 1; }\n");
    html.push_str("#sidebar { width: 360px; background: #111; border-left: 1px solid #333; overflow-y: auto; padding: 16px; }\n");
    html.push_str("svg { display: block; width: 100%; height: 100%; }\n");
    html.push_str("circle { cursor: pointer; }\n");
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<div id=\"container\">\n");
    html.push_str("<svg id=\"canvas\"></svg>\n");
    html.push_str("<div id=\"sidebar\">\n");
    html.push_str(&format!("<div>Nodes: {}</div>\n", all_nodes.len()));
    html.push_str(&format!("<div>Edges: {}</div>\n", all_edges.len()));
    html.push_str(&format!("<div>Updated: {}</div>\n", timestamp));
    html.push_str("<div id=\"nodeInfo\" style=\"display: none; margin-top: 16px;\"></div>\n");
    html.push_str("</div>\n</div>\n");
    html.push_str(
        "<script src=\"https://d3js.org/d3.v7.min.js\" crossorigin=\"anonymous\"></script>\n",
    );
    html.push_str("<script>\n");
    html.push_str(&format!("const nodes = {};\n", nodes_json));
    html.push_str(&format!("const edges = {};\n", edges_json));
    html.push_str("const svg = d3.select('#canvas');\n");
    html.push_str("const simulation = d3.forceSimulation(nodes)\n");
    html.push_str("  .force('link', d3.forceLink(edges).id(d => d.qualified_name).distance(30))\n");
    html.push_str("  .force('charge', d3.forceManyBody().strength(-300))\n");
    html.push_str("  .force('center', d3.forceCenter(window.innerWidth * 0.6 / 2, window.innerHeight / 2));\n");
    html.push_str("</script>\n</body>\n</html>");

    std::fs::write(output_path, html)?;
    Ok(())
}

pub fn export_svg(
    store: &GraphStore,
    _communities: &[Community],
    output_path: &Path,
) -> Result<()> {
    let all_nodes = store.get_all_nodes_for_graph(5000, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

    let width = 1200;
    let height = 800;

    let mut svg = String::new();
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"800\" viewBox=\"0 0 1200 800\">\n");
    svg.push_str(
        "  <defs><style>.node { fill: #4E79A7; stroke: #666; stroke-width: 1; }</style></defs>\n",
    );
    svg.push_str("  <rect width=\"1200\" height=\"800\" fill=\"#000\"/>\n");

    // Simple circular layout per community
    let nodes_per_circle = (all_nodes.len() as f32).sqrt().ceil() as usize;
    let radius = 200.0;

    for (idx, node) in all_nodes.iter().enumerate() {
        let angle = (idx as f32 / all_nodes.len() as f32) * std::f32::consts::TAU;
        let x = 600.0 + radius * angle.cos();
        let y = 400.0 + radius * angle.sin();
        svg.push_str(&format!(
            "  <circle class=\"node\" cx=\"{}\" cy=\"{}\" r=\"4\"/>\n",
            x, y
        ));
    }

    // Add edges
    let node_positions: std::collections::HashMap<_, _> = all_nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| {
            let angle = (idx as f32 / all_nodes.len() as f32) * std::f32::consts::TAU;
            let x = 600.0 + radius * angle.cos();
            let y = 400.0 + radius * angle.sin();
            (node.qualified_name.clone(), (x, y))
        })
        .collect();

    for edge in &all_edges {
        if let (Some((x1, y1)), Some((x2, y2))) = (
            node_positions.get(&edge.source_qualified),
            node_positions.get(&edge.target_qualified),
        ) {
            svg.push_str(&format!("  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#666\" stroke-width=\"0.5\"/>\n", x1, y1, x2, y2));
        }
    }

    svg.push_str("</svg>");
    std::fs::write(output_path, svg)?;
    Ok(())
}

pub struct Exporter;

impl Exporter {
    pub fn export_graph(_graph_data: &str) -> Result<()> {
        // TODO: Implement graph export
        Ok(())
    }
}
