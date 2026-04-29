// Graph MCP prompts — 5 workflow prompts for code analysis and review

use serde_json::json;

pub fn review_changes_prompt() -> serde_json::Value {
    json!({
        "name": "review_changes_prompt",
        "description": "Step-by-step code review workflow",
        "messages": [
            {
                "role": "user",
                "content": "You are a code review expert. Follow these steps:\n\n1. Call detect_graph_changes(base=\"HEAD~1\") to identify changed nodes\n2. For each high-risk changed node, call get_impact_radius(qualified_name) to find affected code\n3. Call get_review_context(files) to get minimal source snippets\n4. Call list_tests(qualified_name) to verify test coverage\n5. Provide a risk assessment and approval recommendation\n\nFocus on: correctness, test coverage, architectural impact, and potential side effects."
            }
        ]
    })
}

pub fn architecture_map_prompt() -> serde_json::Value {
    json!({
        "name": "architecture_map_prompt",
        "description": "Understand codebase architecture",
        "messages": [
            {
                "role": "user",
                "content": "You are an architecture analyst. Follow these steps to map the codebase:\n\n1. Call get_graph_architecture_overview() to see all communities\n2. Call list_graph_communities() to get detailed community structure\n3. Call get_god_nodes(top_n=10) to identify critical hub nodes\n4. For each god node, call explain_node(label) to understand its role\n5. Identify architectural patterns, layers, and cross-cutting concerns\n\nProvide a summary of: main components, their responsibilities, dependencies, and architectural risks."
            }
        ]
    })
}

pub fn debug_issue_prompt() -> serde_json::Value {
    json!({
        "name": "debug_issue_prompt",
        "description": "Trace and debug issues",
        "messages": [
            {
                "role": "user",
                "content": "You are a debugging expert. Follow these steps to trace an issue:\n\n1. Call semantic_search_nodes(query, top_k=10) to find relevant code\n2. For each found node, call query_graph(target, pattern=\"callers_of\") to trace callers\n3. Call find_graph_path(source, target) to understand execution flow\n4. Call get_affected_flows(file_path) to see data flow implications\n5. Call list_tests(qualified_name) to check test coverage\n\nProvide: root cause hypothesis, affected code paths, test recommendations, and fix strategy."
            }
        ]
    })
}

pub fn onboard_developer_prompt() -> serde_json::Value {
    json!({
        "name": "onboard_developer_prompt",
        "description": "Guided codebase tour for new developers",
        "messages": [
            {
                "role": "user",
                "content": "You are an onboarding guide. Follow these steps to introduce the codebase:\n\n1. Call get_graph_report() to get the comprehensive analysis\n2. Call list_graph_communities() to explain main components\n3. Call get_god_nodes(top_n=5) to identify critical nodes\n4. For each god node, call explain_node(label) to explain its purpose\n5. Call get_file_summary(file_path) for key files\n\nProvide: codebase overview, key components, important files, entry points, and recommended learning path."
            }
        ]
    })
}

pub fn pre_merge_check_prompt() -> serde_json::Value {
    json!({
        "name": "pre_merge_check_prompt",
        "description": "Pre-merge risk assessment",
        "messages": [
            {
                "role": "user",
                "content": "You are a merge gate reviewer. Follow these steps for pre-merge checks:\n\n1. Call detect_graph_changes(base=\"main\") to identify all changed nodes\n2. Call get_impact_radius(qualified_name, max_depth=3) for each changed node\n3. Call get_flow_criticality() to identify critical flows affected\n4. Call list_tests(qualified_name) to verify test coverage\n5. Assess merge safety based on: blast radius, test coverage, criticality, and risk score\n\nProvide: risk level (green/yellow/red), critical issues, required tests, and merge recommendation."
            }
        ]
    })
}

pub fn all_prompts() -> Vec<serde_json::Value> {
    vec![
        review_changes_prompt(),
        architecture_map_prompt(),
        debug_issue_prompt(),
        onboard_developer_prompt(),
        pre_merge_check_prompt(),
    ]
}
