// Structural risk scorer — fast, no LLM call.
// Computes a 0.0–1.0 risk score from parsed diff metadata alone.

use serde::{Deserialize, Serialize};
use super::parser::ParsedDiff;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralRisk {
    pub score:   f32,
    pub factors: Vec<RiskFactor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub name:         String,
    pub contribution: f32,
    pub detail:       String,
}

const CRITICAL_PATH_SEGMENTS: &[&str] = &[
    "auth", "security", "migration", "db/", "schema",
];

const CONFIG_EXTENSIONS: &[&str] = &[
    "env", "toml", "yaml", "yml", "json",
];

const CONFIG_FILENAMES: &[&str] = &[
    "Dockerfile", ".env", "docker-compose",
];

pub fn is_critical_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    CRITICAL_PATH_SEGMENTS.iter().any(|seg| lower.contains(seg))
}

pub fn compute_structural_risk(diff: &ParsedDiff) -> StructuralRisk {
    let mut factors = Vec::new();
    let mut score = 0.0f32;

    // 1. High churn (weight 0.25): total lines changed > 500
    let total_changed = diff.total_lines_added + diff.total_lines_deleted;
    if total_changed > 500 {
        score += 0.25;
        factors.push(RiskFactor {
            name:         "high_churn".into(),
            contribution: 0.25,
            detail:       format!("{total_changed} total lines changed (threshold: 500)"),
        });
    }

    // 2. Critical file paths (weight 0.30)
    let critical: Vec<&str> = diff.files.iter()
        .filter(|f| is_critical_path(&f.path))
        .map(|f| f.path.as_str())
        .collect();
    if !critical.is_empty() {
        score += 0.30;
        factors.push(RiskFactor {
            name:         "critical_file_paths".into(),
            contribution: 0.30,
            detail:       format!("Critical paths changed: {}", critical.join(", ")),
        });
    }

    // 3. File count breadth (weight 0.15): >10 files
    if diff.file_count > 10 {
        score += 0.15;
        factors.push(RiskFactor {
            name:         "file_count_breadth".into(),
            contribution: 0.15,
            detail:       format!("{} files changed (threshold: 10)", diff.file_count),
        });
    }

    // 4. Binary/config changes (weight 0.15)
    let has_config = diff.files.iter().any(|f| {
        let ext = f.extension.to_lowercase();
        let fname = std::path::Path::new(&f.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        CONFIG_EXTENSIONS.contains(&ext.as_str())
            || CONFIG_FILENAMES.iter().any(|name| fname.contains(name))
    });
    if has_config {
        score += 0.15;
        let config_files: Vec<&str> = diff.files.iter()
            .filter(|f| {
                let ext = f.extension.to_lowercase();
                let fname = std::path::Path::new(&f.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                CONFIG_EXTENSIONS.contains(&ext.as_str())
                    || CONFIG_FILENAMES.iter().any(|name| fname.contains(name))
            })
            .map(|f| f.path.as_str())
            .collect();
        factors.push(RiskFactor {
            name:         "config_changes".into(),
            contribution: 0.15,
            detail:       format!("Config/env files changed: {}", config_files.join(", ")),
        });
    }

    // 5. Deletions-heavy (weight 0.15): deletion-to-addition ratio > 2:1
    if diff.total_lines_added > 0
        && diff.total_lines_deleted > diff.total_lines_added * 2
    {
        score += 0.15;
        factors.push(RiskFactor {
            name:         "deletions_heavy".into(),
            contribution: 0.15,
            detail:       format!(
                "Deletion ratio {:.1}:1 ({} deleted vs {} added)",
                diff.total_lines_deleted as f32 / diff.total_lines_added.max(1) as f32,
                diff.total_lines_deleted,
                diff.total_lines_added,
            ),
        });
    } else if diff.total_lines_added == 0 && diff.total_lines_deleted > 0 {
        // Pure deletion with no additions — also high risk
        score += 0.15;
        factors.push(RiskFactor {
            name:         "deletions_heavy".into(),
            contribution: 0.15,
            detail:       format!("Pure deletion: {} lines removed, none added", diff.total_lines_deleted),
        });
    }

    StructuralRisk {
        score: score.min(1.0),
        factors,
    }
}
