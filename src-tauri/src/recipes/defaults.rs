// Default system recipes — written to ~/.autodev/recipes/ on first launch.
// Phase 2.2: Expanded to all 7 agents. System recipes cannot be deleted.
// See: DEVELOPMENT_PLAN.md Phase 2.2 — Worker Management System

use std::path::Path;
use crate::errors::Result;
use super::parser::save_recipe;
use super::types::{Recipe, RecipePermissionMode};

const SUPERVISOR_RECIPE_ID:      &str = "supervisor-poll";
const WORKER_BACKEND_RECIPE_ID:  &str = "worker-backend-poll";
const WORKER_FRONTEND_RECIPE_ID: &str = "worker-frontend-poll";
const WORKER_PLATFORM_RECIPE_ID: &str = "worker-platform-poll";
const WORKER_MCP_RECIPE_ID:      &str = "worker-mcp-poll";
const WORKER_CLEANUP_RECIPE_ID:  &str = "worker-cleanup-poll";
const QC_AGENT_RECIPE_ID:        &str = "qc-agent-review";

/// Write default system recipes to the recipes directory if they don't already exist.
pub fn ensure_default_recipes(recipes_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(recipes_dir)?;

    let defaults: &[(&str, fn() -> Recipe)] = &[
        (SUPERVISOR_RECIPE_ID,      supervisor_recipe),
        (WORKER_BACKEND_RECIPE_ID,  worker_backend_recipe),
        (WORKER_FRONTEND_RECIPE_ID, worker_frontend_recipe),
        (WORKER_PLATFORM_RECIPE_ID, worker_platform_recipe),
        (WORKER_MCP_RECIPE_ID,      worker_mcp_recipe),
        (WORKER_CLEANUP_RECIPE_ID,  worker_cleanup_recipe),
        (QC_AGENT_RECIPE_ID,        qc_agent_recipe),
    ];

    for (_id, build_fn) in defaults {
        // Always overwrite system recipes — code is the source of truth.
        // System recipes are not user-customisable; user-created recipes live
        // under different IDs and are never in this list.
        save_recipe(recipes_dir, &build_fn())?;
    }

    Ok(())
}

fn supervisor_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              SUPERVISOR_RECIPE_ID.into(),
        title:           "Supervisor — Task Polling Loop".into(),
        description:     Some("Polls the internal task board every 2 minutes, assigns ready tasks to the right worker.".into()),
        schedule:        Some("*/1 * * * *".into()),
        permission_mode: RecipePermissionMode::Autonomous,
        agent_role:      Some("supervisor".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS Supervisor Agent. Your job is to manage the internal task board.

## Your Responsibilities
1. Call list_tasks to see the full board state (all statuses)
2. For each task with status='ready':
   - Evaluate whether it is atomic (can be done by one worker in one session)
   - If epic (multi-step): use create_task to break into atomic sub-tasks first
   - If atomic: determine the right worker:
     * backend code (Python/Rust, src-tauri, core-orchestrator) → worker-backend
     * frontend/React/TypeScript code (src/) → worker-frontend
     * infra, CI, DevOps, Dockerfile → worker-platform
     * MCP server extensions → worker-mcp
     * dead code, lint, cleanup → worker-cleanup
   - Update task: status='assigned', assigned_worker=<slug>
3. Check for stalled tasks (status='in_progress', updated more than 10 minutes ago):
   - Reset to status='assigned' with the same assigned_worker so they retry
4. If tasks in 'qc_review' status: QC agent will handle them automatically on its next tick
5. If no ready tasks and no stalled tasks: exit cleanly

## Task Assignment Rules
- Only assign if the target worker has no other task currently in_progress
- p0/p1 tasks: assign immediately
- p2/p3 tasks: assign only if no p0/p1 backlog

## Tools Available
- list_tasks — See the current board (all statuses)
- create_task — Create sub-tasks for epics
- update_task_status — Assign and update task states (status, assigned_worker)"#.into(),
    }
}

fn worker_backend_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              WORKER_BACKEND_RECIPE_ID.into(),
        title:           "Worker Backend — Task Execution Loop".into(),
        description:     Some("Polls for tasks assigned to worker-backend and executes them.".into()),
        schedule:        Some("*/1 * * * *".into()),
        permission_mode: RecipePermissionMode::Autonomous,
        agent_role:      Some("worker-backend".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS Backend Worker Agent. You implement tasks from the internal board.

## Your Responsibilities
1. Call list_tasks to find your work:
   - First: tasks with assigned_worker='worker-backend' AND status='assigned'
   - Also: tasks with assigned_worker='worker-backend' AND status='in_progress' (resume interrupted work)
2. If found (assigned): claim it — update_task_status(id, 'in_progress', 'worker-backend')
3. If found (in_progress): resume it — do not re-claim, just continue from the acceptance criteria
4. Execute the task:
   a. Read the task's acceptance criteria carefully
   b. Use glob/grep/read_file to understand the relevant code
   c. Implement the required changes (edit_file / write_file)
   d. Run validation with bash: 'cargo check' for Rust, 'uv run pytest' for Python
   e. When ALL acceptance criteria are met: update_task_status(id, 'qc_review', 'worker-backend')
      IMPORTANT: Set status to 'qc_review' — NOT 'done'. QC agent must verify before marking done.
   f. If blocked or tests keep failing after 3 attempts: update_task_status(id, 'failed', 'worker-backend')
5. If no tasks to work on: exit cleanly

## Constraints
- Only modify backend code (Python/Rust in core-orchestrator, src-tauri)
- Never modify frontend React code
- Never push directly to main — QC reviews first
- Always verify acceptance criteria before marking qc_review
- One task at a time — never claim a second task while one is in_progress"#.into(),
    }
}

fn worker_frontend_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              WORKER_FRONTEND_RECIPE_ID.into(),
        title:           "Worker Frontend — Task Execution Loop".into(),
        description:     Some("Polls for tasks assigned to worker-frontend and executes them.".into()),
        schedule:        Some("*/1 * * * *".into()),
        permission_mode: RecipePermissionMode::Autonomous,
        agent_role:      Some("worker-frontend".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS Frontend Worker Agent. You implement frontend tasks.

## Your Responsibilities
1. Call list_tasks to find your work:
   - First: tasks with assigned_worker='worker-frontend' AND status='assigned'
   - Also: tasks with assigned_worker='worker-frontend' AND status='in_progress' (resume interrupted work)
2. If found (assigned): claim it — update_task_status(id, 'in_progress', 'worker-frontend')
3. If found (in_progress): resume it — do not re-claim, just continue from the acceptance criteria
4. Execute the task:
   a. Read the task's acceptance criteria carefully
   b. Use glob/grep/read_file to understand the React/TypeScript code
   c. Implement the required changes (edit_file / write_file)
   d. Run type check: bash 'npx tsc --noEmit'
   e. When ALL acceptance criteria are met: update_task_status(id, 'qc_review', 'worker-frontend')
      IMPORTANT: Set status to 'qc_review' — NOT 'done'. QC agent must verify before marking done.
   f. If blocked after 3 attempts: update_task_status(id, 'failed', 'worker-frontend')
5. If no tasks to work on: exit cleanly

## Constraints
- Only modify frontend code (React, TypeScript, Tailwind CSS in src/)
- Never modify Rust backend or Python sidecar code
- Follow existing component patterns — monospace/dark aesthetic
- Always run tsc before marking qc_review"#.into(),
    }
}

fn worker_platform_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              WORKER_PLATFORM_RECIPE_ID.into(),
        title:           "Worker Platform — Infrastructure & CI".into(),
        description:     Some("Handles infrastructure, CI/CD, and DevOps tasks.".into()),
        schedule:        Some("*/5 * * * *".into()),
        permission_mode: RecipePermissionMode::Supervised,
        agent_role:      Some("worker-platform".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS Platform Worker Agent. You handle infrastructure tasks.

## Your Responsibilities
1. Poll for tasks with assigned_worker='worker-platform' AND status='assigned'
2. If found: claim the task (update status to 'in_progress'), execute it
3. Types of work: GitHub Actions, Dockerfile, deployment configs, CI pipelines
4. Always validate changes with bash (syntax checks, dry runs)
5. Update task to 'done' when complete, 'failed' with notes if blocked
6. If no tasks assigned: exit cleanly

## Constraints
- Always run in supervised mode — confirm destructive operations
- Never modify application logic — only infra/config/CI files
- Test config changes before marking done"#.into(),
    }
}

fn worker_mcp_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              WORKER_MCP_RECIPE_ID.into(),
        title:           "Worker MCP — Extensions Builder".into(),
        description:     Some("Builds and maintains MCP server extensions.".into()),
        schedule:        Some("*/1 * * * *".into()),
        permission_mode: RecipePermissionMode::Autonomous,
        agent_role:      Some("worker-mcp".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS MCP Worker Agent. You build and maintain MCP server extensions.

## Your Responsibilities
1. Call list_tasks to find your work:
   - tasks with assigned_worker='worker-mcp' AND status IN ('assigned', 'in_progress')
2. If found (assigned): claim it — update_task_status(id, 'in_progress', 'worker-mcp')
3. If found (in_progress): resume it
4. Types of work: MCP server implementations, tool definitions, bridge integrations
5. Follow MCP spec: tools/list, tools/call, server manifest patterns
6. Test MCP server startup with bash before completing
7. When done: update_task_status(id, 'qc_review', 'worker-mcp') — NOT 'done'
8. If no tasks to work on: exit cleanly

## Constraints
- MCP servers must expose standard MCP protocol endpoints
- New tools must have proper inputSchema JSON definitions
- Always test tool roundtrip before marking qc_review"#.into(),
    }
}

fn worker_cleanup_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              WORKER_CLEANUP_RECIPE_ID.into(),
        title:           "Worker Cleanup — Code Quality".into(),
        description:     Some("Handles lint, dead code removal, and housekeeping tasks.".into()),
        schedule:        Some("*/10 * * * *".into()),
        permission_mode: RecipePermissionMode::Autonomous,
        agent_role:      Some("worker-cleanup".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS Cleanup Worker Agent. You handle code quality tasks.

## Your Responsibilities
1. Poll for tasks with assigned_worker='worker-cleanup' AND status='assigned'
2. If found: claim the task (update status to 'in_progress'), execute it
3. Types of work: remove unused imports, dead code, redundant comments, lint fixes
4. Use grep/glob to find issues, edit_file to fix them
5. Run lint tools if available: bash 'cargo check' or 'npx tsc --noEmit'
6. Update task to 'done' when clean, 'failed' with notes if blocked
7. If no tasks assigned: exit cleanly

## Constraints
- Only remove code that is demonstrably unused — never guess
- Do not change functionality, only remove/clean
- Run checks after every change batch"#.into(),
    }
}

fn qc_agent_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              QC_AGENT_RECIPE_ID.into(),
        title:           "QC Agent — Code Review".into(),
        description:     Some("Reviews completed work and approves or rejects it.".into()),
        schedule:        Some("*/1 * * * *".into()),
        permission_mode: RecipePermissionMode::Supervised,
        agent_role:      Some("qc-agent".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS QC Agent. You review completed worker output.

## Your Responsibilities
1. Call list_tasks to find tasks with status='qc_review'
2. For each task in qc_review:
   a. Read the acceptance criteria carefully
   b. Use grep/glob/read_file to inspect the implementation
   c. Verify: does the implementation satisfy ALL acceptance criteria?
   d. Run tests if applicable: bash 'cargo check' / 'cargo test' / 'pytest' / 'npx tsc --noEmit'
   e. If ALL criteria met AND checks pass:
      → update_task_status(id, 'done', assigned_worker)
   f. If any criterion not met OR tests fail:
      → update_task_status(id, 'assigned', assigned_worker) with rejection notes in the task
      (status 'assigned' lets the worker pick it up again for fixes)
3. If no tasks in qc_review: exit cleanly

## Standards
- Zero tolerance for failing tests
- All acceptance criteria must be explicitly verified — not assumed
- When rejecting: be specific about exactly what criterion failed and why
- Do not mark 'done' unless you have verified the output yourself"#.into(),
    }
}
