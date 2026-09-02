// Doc 09 T3.1 — write_interface_spec.
//
// Tool the backend worker MUST call before qc_review on any task whose
// acceptance criteria mention `route`, `endpoint`, `API`, or `handler`.
// Writes `<workspace>/INTERFACE.md` with one row per route:
//
//   ### GET /api/events
//   **Request:** query string `?from=ISO&to=ISO` (optional)
//   **Response (200):** `{ "events": [{ id, title, startTime, endTime, … }, …] }`
//   **Errors:** `404 { "error": "…" }`, `500 { "error": "…" }`
//
// The frontend worker's Step 3.0 reads this file before writing any
// consumer (apiClient / fetch / useEvents hook). The `tier2_6_api_contract`
// validation pass cross-checks URL strings against the file at write time,
// so a drifted URL surfaces immediately rather than at qc_review.
//
// Modes:
//   • mode='overwrite' (default) — replace any existing INTERFACE.md.
//   • mode='merge'                — append new routes; preserve existing.
//
// The tool is intentionally tolerant of free-form `notes` so the LLM can
// inline auth, rate limits, and CORS comments — those land in the
// "Conventions" section the dispatcher renders at the top.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct WriteInterfaceSpecTool;

#[derive(Debug, Deserialize)]
struct RouteSpec {
    method: String,
    path: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    request_shape: Option<Value>,
    #[serde(default)]
    response_shape: Option<Value>,
    #[serde(default)]
    status_codes: Option<Vec<u16>>,
    #[serde(default)]
    auth_required: Option<bool>,
    #[serde(default)]
    notes: Option<String>,
}

#[async_trait]
impl Tool for WriteInterfaceSpecTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Write
    }

    fn name(&self) -> &str {
        "write_interface_spec"
    }

    fn description(&self) -> &str {
        "Write or merge the workspace's INTERFACE.md contract file. \
         REQUIRED before qc_review on any task that produces or modifies \
         HTTP routes. One entry per route with method, path, request shape \
         (JSON example), response shape (JSON example), HTTP status codes, \
         and optional auth/notes. The frontend worker reads this file as \
         its single source of truth for API consumers — fields not declared \
         here will be flagged by the api_contract check on the next write."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "routes": {
                    "type": "array",
                    "description": "List of route specifications. At least one.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "method":         { "type": "string", "description": "HTTP verb — GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS." },
                            "path":           { "type": "string", "description": "Path (e.g. /api/events or /api/events/:id)." },
                            "description":    { "type": "string", "description": "One-line summary of what the route does." },
                            "request_shape":  { "description": "JSON example of the request body (or query/path parameters as a JSON object). Omit for GETs without a body." },
                            "response_shape": { "description": "JSON example of the 200 response body." },
                            "status_codes":   { "type": "array", "items": { "type": "integer" }, "description": "Non-200 HTTP codes the route can return." },
                            "auth_required":  { "type": "boolean", "description": "True if this route requires authentication." },
                            "notes":          { "type": "string", "description": "Free-form notes — rate limits, CORS, headers, etc." }
                        },
                        "required": ["method", "path"]
                    }
                },
                "mode": {
                    "type": "string",
                    "enum": ["overwrite", "merge"],
                    "description": "overwrite (default) replaces INTERFACE.md; merge appends new routes while preserving existing ones.",
                    "default": "overwrite"
                },
                "conventions": {
                    "type": "string",
                    "description": "Optional free-text section rendered at the top of the file. Use for shared base URLs, auth headers, error envelope shape, etc."
                }
            },
            "required": ["routes"]
        })
    }


    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let routes_val = match input.get("routes") {
            Some(v) if v.is_array() => v,
            _ => return ToolResult::error("'routes' is required and must be an array"),
        };

        let routes: Vec<RouteSpec> = match serde_json::from_value(routes_val.clone()) {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(&format!(
                    "Could not parse 'routes': {e}. Each entry needs at least 'method' and 'path'."
                ))
            }
        };
        if routes.is_empty() {
            return ToolResult::error("'routes' must contain at least one entry");
        }

        let mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("overwrite")
            .to_lowercase();
        if !matches!(mode.as_str(), "overwrite" | "merge") {
            return ToolResult::error("'mode' must be 'overwrite' or 'merge'");
        }

        let conventions = input
            .get("conventions")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // ── Build the markdown body for the new routes ────────────────────────
        let mut new_body = String::new();
        for r in &routes {
            new_body.push_str(&format_route(r));
            new_body.push('\n');
        }

        // ── Locate INTERFACE.md at the workspace root ─────────────────────────
        let path = PathBuf::from(&ctx.working_dir).join("INTERFACE.md");

        // Read existing file when in merge mode.
        let existing = if mode == "merge" && path.exists() {
            std::fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let final_body = if mode == "merge" && !existing.is_empty() {
            // Append the new routes after the existing content. We don't try
            // to deduplicate — duplicates are visible to the worker as side-
            // by-side entries and the supervisor's next decompose pass can
            // collapse them. Naïve append is robust under partial updates.
            let sep = if existing.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{existing}{sep}{new_body}")
        } else {
            let mut out = String::new();
            out.push_str("# Fleet API Interface\n\n");
            out.push_str(
                "Single source of truth for routes exposed by backend workers. \
                 Frontend consumers (`apiClient`, `fetch`, `useXxx` hooks) MUST \
                 match every URL string against an entry in this file. The \
                 `api_contract` validation pass (Doc 09 T3.3) enforces this at \
                 write time.\n\n",
            );
            if !conventions.is_empty() {
                out.push_str("## Conventions\n\n");
                out.push_str(&conventions);
                out.push_str("\n\n");
            }
            out.push_str("## Routes\n\n");
            out.push_str(&new_body);
            out
        };

        // ── Write the file ────────────────────────────────────────────────────
        if let Err(e) = std::fs::write(&path, &final_body) {
            return ToolResult::error(&format!(
                "Could not write INTERFACE.md to '{}': {e}",
                path.display()
            ));
        }

        ToolResult::success(format!(
            "Wrote {} route(s) to '{}' (mode={mode}).\n\
             Frontend consumers can now safely read this file for the API contract.",
            routes.len(),
            path.display()
        ))
    }
}

fn format_route(r: &RouteSpec) -> String {
    let mut s = format!("### {} {}\n", r.method.to_uppercase(), r.path);
    if let Some(d) = &r.description {
        if !d.trim().is_empty() {
            s.push_str(&format!("{}\n", d.trim()));
        }
    }
    if let Some(req) = &r.request_shape {
        s.push_str("**Request:**\n```json\n");
        s.push_str(&serde_json::to_string_pretty(req).unwrap_or_else(|_| req.to_string()));
        s.push_str("\n```\n");
    }
    if let Some(resp) = &r.response_shape {
        s.push_str("**Response (200):**\n```json\n");
        s.push_str(&serde_json::to_string_pretty(resp).unwrap_or_else(|_| resp.to_string()));
        s.push_str("\n```\n");
    }
    if let Some(codes) = &r.status_codes {
        if !codes.is_empty() {
            let strs: Vec<String> = codes.iter().map(|c| c.to_string()).collect();
            s.push_str(&format!("**Other status codes:** {}\n", strs.join(", ")));
        }
    }
    if let Some(true) = r.auth_required {
        s.push_str("**Auth:** required\n");
    } else if let Some(false) = r.auth_required {
        s.push_str("**Auth:** none\n");
    }
    if let Some(n) = &r.notes {
        if !n.trim().is_empty() {
            s.push_str(&format!("**Notes:** {}\n", n.trim()));
        }
    }
    s
}
