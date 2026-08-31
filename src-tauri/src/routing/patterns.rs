// Static keyword-based routing patterns — fast path checked before any DB lookup.
// Keyword matches are case-insensitive. A pattern fires when ≥2 keywords match.

pub struct StaticPattern {
    pub keywords: &'static [&'static str],
    pub worker_slug: &'static str,
    pub confidence: f32,
}

pub const STATIC_PATTERNS: &[StaticPattern] = &[
    StaticPattern {
        keywords: &[
            "api",
            "endpoint",
            "route",
            "fastapi",
            "pydantic",
            "schema",
            "backend",
            "database",
            "migration",
            "sqlalchemy",
            "orm",
            "crud",
        ],
        worker_slug: "worker-backend",
        confidence: 0.85,
    },
    StaticPattern {
        keywords: &[
            "react",
            "component",
            "ui",
            "css",
            "tailwind",
            "frontend",
            "button",
            "modal",
            "store",
            "zustand",
            "hook",
            "tsx",
            "jsx",
        ],
        worker_slug: "worker-frontend",
        confidence: 0.85,
    },
    StaticPattern {
        keywords: &[
            "docker",
            "ci",
            "github actions",
            "deploy",
            "infrastructure",
            "workflow",
            "platform",
            "devops",
            "kubernetes",
            "helm",
            "pipeline",
        ],
        worker_slug: "worker-platform",
        confidence: 0.80,
    },
    StaticPattern {
        keywords: &[
            "mcp",
            "protocol",
            "tool",
            "server",
            "stdio",
            "transport",
            "plugin",
            "extension",
            "connector",
            "rmcp",
        ],
        worker_slug: "worker-mcp",
        confidence: 0.80,
    },
    StaticPattern {
        keywords: &[
            "dead code",
            "refactor",
            "cleanup",
            "dependency",
            "unused",
            "deprecate",
            "lint",
            "housekeeping",
            "remove",
            "prune",
        ],
        worker_slug: "worker-cleanup",
        confidence: 0.75,
    },
];

/// Returns `(worker_slug, confidence)` for the best-matching static pattern,
/// or `None` if no pattern reaches the threshold of ≥2 keyword matches.
pub fn match_static(task_title: &str, task_description: &str) -> Option<(&'static str, f32)> {
    let haystack = format!("{} {}", task_title, task_description).to_lowercase();

    let mut best_worker: Option<(&'static str, f32, usize)> = None;

    for pattern in STATIC_PATTERNS {
        let hits = pattern
            .keywords
            .iter()
            .filter(|kw| haystack.contains(**kw))
            .count();

        if hits >= 2 {
            let qualifies = best_worker.map_or(true, |(_, _, prev_hits)| hits > prev_hits);
            if qualifies {
                best_worker = Some((pattern.worker_slug, pattern.confidence, hits));
            }
        }
    }

    best_worker.map(|(slug, conf, _)| (slug, conf))
}
