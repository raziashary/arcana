// Default system recipes — written to ~/.autodev/recipes/ on first launch.
// These are the hardcoded Phase 2.1 recipes for Supervisor + Worker Backend.
// System recipes have is_system=true and cannot be deleted by users.

use std::path::Path;
use crate::errors::Result;
use super::parser::save_recipe;
use super::types::{Recipe, RecipePermissionMode};

const SUPERVISOR_RECIPE_ID:      &str = "supervisor-poll";
const WORKER_BACKEND_RECIPE_ID:  &str = "worker-backend-poll";

/// Write default system recipes to the recipes directory if they don't already exist.
pub fn ensure_default_recipes(recipes_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(recipes_dir)?;

    let supervisor_path = recipes_dir.join(format!("{SUPERVISOR_RECIPE_ID}.yaml"));
    if !supervisor_path.exists() {
        save_recipe(recipes_dir, &supervisor_recipe())?;
    }

    let worker_path = recipes_dir.join(format!("{WORKER_BACKEND_RECIPE_ID}.yaml"));
    if !worker_path.exists() {
        save_recipe(recipes_dir, &worker_backend_recipe())?;
    }

    Ok(())
}

fn supervisor_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              SUPERVISOR_RECIPE_ID.into(),
        title:           "Supervisor — Task Polling Loop".into(),
        description:     Some("Polls the internal task board every 2 minutes, assigns ready tasks to workers.".into()),
        schedule:        Some("*/2 * * * *".into()),
        permission_mode: RecipePermissionMode::Autonomous,
        agent_role:      Some("supervisor".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS Supervisor Agent. Your job is to manage the internal task board.

## Your Responsibilities
1. Poll the internal AutoDev board for tasks with status='ready'
2. For each ready task:
   - Evaluate whether it is atomic (can be done by one worker in one session)
   - If atomic: assign to 'worker-backend', update status to 'assigned'
   - If epic (multi-step): break into sub-tasks first, then assign
3. Check for stalled tasks (in_progress > 30min): escalate to failed
4. Check for tasks completed by workers (done): no action needed
5. If no ready tasks: exit cleanly

## Task Assignment Rules
- p0/p1 tasks: assign immediately
- p2/p3 tasks: assign if no p0/p1 backlog
- Never assign to a worker that already has an in_progress task

## Tools
Use create_task to create sub-tasks when breaking down epics.
Use update_task_status to assign and update task states.
Use list_tasks to see current board state."#.into(),
    }
}

fn worker_backend_recipe() -> Recipe {
    Recipe {
        version:         "1.0.0".into(),
        id:              WORKER_BACKEND_RECIPE_ID.into(),
        title:           "Worker Backend — Task Execution Loop".into(),
        description:     Some("Polls for tasks assigned to worker-backend and executes them.".into()),
        schedule:        Some("*/2 * * * *".into()),
        permission_mode: RecipePermissionMode::Autonomous,
        agent_role:      Some("worker-backend".into()),
        model:           None,
        is_system:       true,
        instructions:    r#"You are the AIRS Backend Worker Agent. You implement tasks from the internal board.

## Your Responsibilities
1. Poll for tasks with assigned_worker='worker-backend' AND status='assigned'
2. If found: claim the task (update status to 'in_progress'), execute it
3. Execution steps:
   a. Read the task's acceptance criteria carefully
   b. Use glob/grep/read_file to understand the relevant code
   c. Implement the changes (edit_file / write_file)
   d. Run tests with bash (uv run pytest / pnpm test)
   e. If tests pass: update task status to 'done'
   f. If tests fail: fix until passing or update status to 'failed' with error notes
4. If no tasks assigned: exit cleanly

## Constraints
- Only modify backend code (Python/Rust in core-orchestrator, src-tauri)
- Never modify frontend React code
- Never push directly to main — changes are reviewed separately
- Always run tests before marking done
- Never mark done if you haven't verified the acceptance criteria"#.into(),
    }
}
