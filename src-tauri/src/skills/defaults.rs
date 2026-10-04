// Default built-in skills — written to ~/.arcana/skills/ on first launch.
// Code is the source of truth: builtin skills are always overwritten.
// Phase 3.1

use crate::db::arcana_dir;
use crate::errors::Result;
use std::path::Path;

/// Write all default skills to disk. Always overwrites builtin skills.
pub fn ensure_default_skills() -> Result<()> {
    let base = arcana_dir()?.join("skills");

    // Category directories
    for cat in &["coding", "debugging", "devops", "fleet", "custom"] {
        std::fs::create_dir_all(base.join(cat))?;
    }

    // ── Coding ──────────────────────────────────────────────────────────────
    write(
        &base.join("coding").join("feature-implementation.md"),
        FEATURE_IMPLEMENTATION,
    )?;
    write(&base.join("coding").join("api-endpoint.md"), API_ENDPOINT)?;
    write(&base.join("coding").join("refactoring.md"), REFACTORING)?;
    write(&base.join("coding").join("code-review.md"), CODE_REVIEW)?;
    write(&base.join("coding").join("test-addition.md"), TEST_ADDITION)?;
    write(
        &base.join("coding").join("database-migration.md"),
        DATABASE_MIGRATION,
    )?;

    // ── Debugging ───────────────────────────────────────────────────────────
    write(
        &base.join("debugging").join("error-investigation.md"),
        ERROR_INVESTIGATION,
    )?;
    write(
        &base.join("debugging").join("performance-optimization.md"),
        PERFORMANCE_OPTIMIZATION,
    )?;
    write(
        &base.join("debugging").join("race-condition-debug.md"),
        RACE_CONDITION_DEBUG,
    )?;

    // ── DevOps ──────────────────────────────────────────────────────────────
    write(&base.join("devops").join("docker-setup.md"), DOCKER_SETUP)?;
    write(
        &base.join("devops").join("ci-pipeline-fix.md"),
        CI_PIPELINE_FIX,
    )?;
    write(
        &base.join("devops").join("dependency-update.md"),
        DEPENDENCY_UPDATE,
    )?;

    // ── Fleet ───────────────────────────────────────────────────────────────
    write(
        &base.join("fleet").join("worker-task-execution.md"),
        WORKER_TASK_EXECUTION,
    )?;
    write(
        &base.join("fleet").join("supervisor-orchestration.md"),
        SUPERVISOR_ORCHESTRATION,
    )?;
    write(&base.join("fleet").join("qc-review.md"), QC_REVIEW)?;
    write(
        &base.join("fleet").join("epic-breakdown.md"),
        EPIC_BREAKDOWN,
    )?;

    // ── DevOps (continued) — Ruflo Feature 7 ────────────────────────────────
    write(&base.join("devops").join("browser-test.md"), BROWSER_TEST)?;

    Ok(())
}

fn write(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// CODING SKILLS
// ─────────────────────────────────────────────────────────────────────────────

const FEATURE_IMPLEMENTATION: &str = r#"---
name: feature-implementation
description: Standard 5-step workflow for implementing a new feature end-to-end
category: coding
parameters:
  - name: feature_name
    type: string
    required: true
    description: Name of the feature to implement
  - name: target_module
    type: string
    required: true
    description: Module or directory where the feature belongs
  - name: include_tests
    type: boolean
    required: false
    default: true
    description: Whether to write unit/integration tests
  - name: acceptance_criteria
    type: string
    required: false
    default: "All edge cases handled, tests pass, no regressions"
    description: Acceptance criteria for the feature
version: "1.0.0"
author: "Arcana"
tags: [backend, frontend, api, feature]
---

# Implement: {{feature_name}}

## Target Module
`{{target_module}}`

## Step 1 — Understand Context
1. Use `glob` and `grep` to locate existing code in `{{target_module}}`
2. Read the 2-3 most relevant files to understand patterns and conventions
3. Identify entry points, data models, and interfaces that will be touched

## Step 2 — Design
1. Sketch the data flow and API surface for `{{feature_name}}`
2. Identify all files that need to be created or modified
3. Check for existing utilities you can reuse — do not duplicate code

## Step 3 — Implement
1. Create or modify files in `{{target_module}}`
2. Follow existing code style exactly (indentation, naming, comments)
3. Implement incrementally: core logic first, then edge cases

## Step 4 — Test
{{#if include_tests}}
1. Write unit tests covering the happy path and primary edge cases
2. Run tests: verify they pass
3. Run type check / lint if applicable
{{else}}
1. Manually trace through the logic for correctness
2. Run existing tests to confirm no regressions
{{/if}}

## Step 5 — Verify
Acceptance criteria: {{acceptance_criteria}}

Confirm every criterion is satisfied before marking complete.
"#;

const API_ENDPOINT: &str = r#"---
name: api-endpoint
description: Design, implement, test and document a new API endpoint
category: coding
parameters:
  - name: method
    type: enum
    values: [GET, POST, PUT, PATCH, DELETE]
    required: true
    description: HTTP method
  - name: path
    type: string
    required: true
    description: URL path e.g. /api/users/{id}
  - name: description
    type: string
    required: true
    description: What this endpoint does
  - name: auth_required
    type: boolean
    required: false
    default: true
    description: Whether authentication is required
version: "1.0.0"
author: "Arcana"
tags: [api, backend, rest]
---

# New API Endpoint: {{method}} {{path}}

**Purpose**: {{description}}
**Auth required**: {{auth_required}}

## Step 1 — Design
1. Define request schema (query params / body)
2. Define response schema (success + error shapes)
3. Identify what DB queries or services are needed

## Step 2 — Implement
1. Add route handler for `{{method}} {{path}}`
2. Implement request validation (return 422 for invalid input)
3. Implement business logic
4. Return consistent JSON response shape

## Step 3 — Auth
{{#if auth_required}}
1. Verify authentication middleware is applied
2. Add authorization check (permissions / roles)
{{else}}
1. Confirm endpoint is correctly marked as public
{{/if}}

## Step 4 — Test
1. Write test for 200 success case
2. Write test for 400/422 invalid input
3. Write test for 401/403 if auth is required
4. Run full test suite — zero regressions

## Step 5 — Document
Update API docs or OpenAPI spec with the new endpoint.
"#;

const REFACTORING: &str = r#"---
name: refactoring
description: Safely refactor existing code - analyze, plan, execute, verify
category: coding
parameters:
  - name: target
    type: string
    required: true
    description: File, function, or module to refactor
  - name: goal
    type: string
    required: true
    description: What the refactoring achieves (e.g., "extract service layer", "reduce duplication")
version: "1.0.0"
author: "Arcana"
tags: [refactoring, cleanup, architecture]
---

# Refactor: {{target}}

**Goal**: {{goal}}

## Step 1 — Analyze
1. Read `{{target}}` in full
2. Use `grep` to find all callers / references
3. Identify what can change without breaking the public contract

## Step 2 — Plan
1. List every file that will be modified
2. Define the new structure / interface
3. Identify risk points (public APIs, serialisation formats, DB schemas)

## Step 3 — Execute
1. Make changes in smallest safe increments
2. Keep existing tests green after each increment
3. Do not change behaviour — only structure

## Step 4 — Verify
1. Run full test suite
2. Run type checker / linter
3. Re-read diff and check for any unintended changes
4. Confirm `{{goal}}` is achieved
"#;

const CODE_REVIEW: &str = r#"---
name: code-review
description: Structured code review workflow - read, analyze, comment, summarize
category: coding
parameters:
  - name: target
    type: string
    required: true
    description: File(s) or PR to review
  - name: focus
    type: string
    required: false
    default: "correctness, security, performance, style"
    description: Specific aspects to focus on
version: "1.0.0"
author: "Arcana"
tags: [review, quality, security]
---

# Code Review: {{target}}

**Focus areas**: {{focus}}

## Step 1 — Read
1. Read `{{target}}` completely without judging
2. Understand the intent and context

## Step 2 — Analyze
Check for:
- **Correctness**: Logic errors, off-by-one, null handling, error propagation
- **Security**: Input validation, SQL injection, auth bypass, secret exposure
- **Performance**: N+1 queries, unnecessary allocations, blocking calls
- **Style**: Naming clarity, function length, duplication, comment accuracy

## Step 3 — Comment
For each issue found:
- Cite the exact line(s)
- Explain the problem clearly
- Suggest a concrete fix

## Step 4 — Summarize
Provide:
- Overall assessment (approve / request changes)
- Critical issues (must fix before merge)
- Non-blocking suggestions (optional improvements)
"#;

const TEST_ADDITION: &str = r#"---
name: test-addition
description: Identify untested code and write comprehensive tests
category: coding
parameters:
  - name: target
    type: string
    required: true
    description: File or module to add tests for
  - name: test_framework
    type: string
    required: false
    default: "auto-detect"
    description: Testing framework to use (pytest, jest, cargo test, etc.)
version: "1.0.0"
author: "Arcana"
tags: [testing, quality, coverage]
---

# Add Tests: {{target}}

**Framework**: {{test_framework}}

## Step 1 — Identify
1. Read `{{target}}` and list all public functions/methods
2. Check existing test files for coverage gaps
3. Prioritise: happy path, edge cases, error cases

## Step 2 — Write
For each untested function:
1. Happy path test (expected inputs → expected output)
2. Edge case test (empty, null, boundary values)
3. Error case test (invalid inputs, failure paths)

## Step 3 — Run
1. Execute tests with `{{test_framework}}`
2. All new tests must pass
3. No regressions in existing tests

## Step 4 — Fix
If tests reveal bugs:
1. Fix the bug in the source
2. Re-run until green
"#;

const DATABASE_MIGRATION: &str = r#"---
name: database-migration
description: Design and implement a database schema migration safely
category: coding
parameters:
  - name: migration_name
    type: string
    required: true
    description: Descriptive name for this migration
  - name: db_type
    type: enum
    values: [sqlite, postgres, mysql]
    required: false
    default: sqlite
    description: Database type
  - name: reversible
    type: boolean
    required: false
    default: true
    description: Whether to implement a rollback/down migration
version: "1.0.0"
author: "Arcana"
tags: [database, migration, schema]
---

# Database Migration: {{migration_name}}

**DB**: {{db_type}} | **Reversible**: {{reversible}}

## Step 1 — Design
1. Define the exact schema change (ADD COLUMN, CREATE TABLE, DROP INDEX, etc.)
2. Check for data that needs to be migrated or backfilled
3. Assess impact on existing queries and indexes

## Step 2 — Write Migration
1. Write the UP migration (forward change)
{{#if reversible}}
2. Write the DOWN migration (rollback)
{{/if}}
3. Add migration file with timestamp prefix for ordering

## Step 3 — Test
1. Run UP migration on a test database
2. Verify schema looks correct
3. Run existing queries — confirm no breakage
{{#if reversible}}
4. Run DOWN migration — confirm rollback is clean
{{/if}}

## Step 4 — Deploy
1. Run migration in staging first
2. Verify app behaviour
3. Only then run in production
"#;

// ─────────────────────────────────────────────────────────────────────────────
// DEBUGGING SKILLS
// ─────────────────────────────────────────────────────────────────────────────

const ERROR_INVESTIGATION: &str = r#"---
name: error-investigation
description: Systematic investigation - logs, reproduction, root cause, fix
category: debugging
parameters:
  - name: error_description
    type: string
    required: true
    description: Description of the error or symptom
  - name: affected_component
    type: string
    required: false
    default: "unknown"
    description: Component or service exhibiting the error
version: "1.0.0"
author: "Arcana"
tags: [debugging, investigation, logs]
---

# Investigate Error: {{error_description}}

**Component**: {{affected_component}}

## Step 1 — Gather Evidence
1. Read relevant log files (use `grep` for the error message)
2. Identify the exact stack trace and error type
3. Note timestamps, frequency, and conditions

## Step 2 — Reproduce
1. Find the minimal code path that triggers the error
2. Confirm reproduction is reliable
3. Identify what inputs or state cause it

## Step 3 — Root Cause
1. Trace execution backward from the error
2. Identify the deepest incorrect assumption or logic error
3. Distinguish root cause from symptoms

## Step 4 — Fix
1. Implement the minimal fix at the root cause (not a symptom workaround)
2. Add a regression test that catches this exact error
3. Verify fix eliminates the error reproduction

## Step 5 — Document
Note what was wrong, how it was found, and how it was fixed (brief comment in code or commit message).
"#;

const PERFORMANCE_OPTIMIZATION: &str = r#"---
name: performance-optimization
description: Profile, find bottleneck, optimize, validate
category: debugging
parameters:
  - name: target
    type: string
    required: true
    description: Function, endpoint, or operation to optimize
  - name: current_metric
    type: string
    required: false
    default: "unknown"
    description: Current performance metric (e.g., "500ms p99")
  - name: target_metric
    type: string
    required: false
    default: "50% improvement"
    description: Target performance goal
version: "1.0.0"
author: "Arcana"
tags: [performance, profiling, optimization]
---

# Performance Optimization: {{target}}

**Current**: {{current_metric}} → **Target**: {{target_metric}}

## Step 1 — Profile
1. Add timing instrumentation to `{{target}}`
2. Run with realistic load to identify where time is spent
3. Focus on the top 1-2 hotspots

## Step 2 — Identify Bottleneck
Common culprits:
- N+1 database queries
- Missing indexes
- Unnecessary serialization/deserialization
- Blocking I/O on async path
- Redundant computation in loops

## Step 3 — Optimize
1. Target only the confirmed bottleneck
2. Implement the fix (batching, caching, indexing, parallelism)
3. Do not over-engineer — simplest solution first

## Step 4 — Validate
1. Re-measure with same load profile
2. Confirm `{{target_metric}}` is achieved
3. Run full test suite — no regressions
4. Monitor in production for unexpected side effects
"#;

const RACE_CONDITION_DEBUG: &str = r#"---
name: race-condition-debug
description: Identify and fix race conditions - pattern recognition, lock analysis, stress test
category: debugging
parameters:
  - name: target
    type: string
    required: true
    description: Component or function with suspected race condition
version: "1.0.0"
author: "Arcana"
tags: [concurrency, race-condition, debugging, async]
---

# Race Condition Debug: {{target}}

## Step 1 — Pattern Recognition
Read `{{target}}` and look for:
- Shared mutable state accessed from multiple threads/tasks
- Missing locks around read-modify-write sequences
- `async` functions with `.await` points between reads and writes
- Non-atomic check-then-act sequences

## Step 2 — Lock Analysis
1. Map every shared variable and its current synchronisation primitive
2. Identify unprotected windows between lock release and re-acquire
3. Check for deadlock potential (lock ordering)

## Step 3 — Fix
1. Protect shared state with correct primitive (Mutex, RwLock, Atomic, channel)
2. Minimise lock scope — hold only for the critical section
3. Consider lock-free alternatives where appropriate

## Step 4 — Stress Test
1. Write a test that hammers the fixed code concurrently (100+ iterations)
2. Run under thread sanitizer if available
3. Confirm no data races under load
"#;

// ─────────────────────────────────────────────────────────────────────────────
// DEVOPS SKILLS
// ─────────────────────────────────────────────────────────────────────────────

const DOCKER_SETUP: &str = r#"---
name: docker-setup
description: Containerize a service - Dockerfile, Compose, test, push
category: devops
parameters:
  - name: service_name
    type: string
    required: true
    description: Name of the service to containerize
  - name: base_image
    type: string
    required: false
    default: "auto-detect"
    description: Base Docker image (e.g., python:3.11-slim, node:20-alpine)
  - name: port
    type: string
    required: false
    default: "8000"
    description: Port the service listens on
version: "1.0.0"
author: "Arcana"
tags: [docker, devops, containers, deployment]
---

# Dockerize: {{service_name}}

**Base image**: {{base_image}} | **Port**: {{port}}

## Step 1 — Analyse Service
1. Read the project's existing start command and dependencies
2. Identify runtime dependencies vs build-time dependencies
3. Note any environment variables required

## Step 2 — Write Dockerfile
1. Use `{{base_image}}` as base
2. Copy only what is needed (use .dockerignore)
3. Install dependencies in a separate layer for caching
4. Run as non-root user
5. `EXPOSE {{port}}`

## Step 3 — Write docker-compose.yml
1. Define `{{service_name}}` service
2. Mount volumes for development hot-reload if needed
3. Define environment variables
4. Add health check

## Step 4 — Test
1. `docker build -t {{service_name}}:local .`
2. `docker run -p {{port}}:{{port}} {{service_name}}:local`
3. Hit the health endpoint — confirm 200 OK
4. Run tests inside container: `docker exec ... pytest` or equivalent

## Step 5 — Optimise
1. Check image size — use multi-stage build if > 500MB
2. Ensure build cache is utilised (copy requirements first, then code)
"#;

const CI_PIPELINE_FIX: &str = r#"---
name: ci-pipeline-fix
description: Diagnose failing CI pipeline, patch it, verify it passes
category: devops
parameters:
  - name: pipeline_file
    type: string
    required: true
    description: Path to CI config file (e.g., .github/workflows/ci.yml)
  - name: failure_description
    type: string
    required: true
    description: What is failing in the pipeline
version: "1.0.0"
author: "Arcana"
tags: [ci, github-actions, devops]
---

# Fix CI Pipeline: {{pipeline_file}}

**Failure**: {{failure_description}}

## Step 1 — Diagnose
1. Read `{{pipeline_file}}` fully
2. Identify the failing step from the failure description
3. Check for: missing env vars, wrong working directory, version mismatches, caching issues

## Step 2 — Fix
1. Correct the identified issue
2. Do not add workarounds — fix the root cause
3. Ensure the fix does not break other jobs in the pipeline

## Step 3 — Verify Locally (where possible)
1. Use `act` or equivalent to run the job locally
2. Or trace through the logic manually

## Step 4 — Commit and Push
1. Push fix to trigger the pipeline
2. Confirm the previously failing job is now green
3. Confirm no other jobs regressed
"#;

const DEPENDENCY_UPDATE: &str = r#"---
name: dependency-update
description: Safely update dependencies - check, update, test, commit
category: devops
parameters:
  - name: package_manager
    type: enum
    values: [cargo, pip, npm, yarn, poetry, uv]
    required: true
    description: Package manager to use
  - name: scope
    type: enum
    values: [patch, minor, major, specific]
    required: false
    default: minor
    description: How aggressively to update (semver level)
  - name: target_package
    type: string
    required: false
    default: ""
    description: Specific package to update (leave empty for all)
version: "1.0.0"
author: "Arcana"
tags: [dependencies, security, maintenance]
---

# Update Dependencies

**Manager**: {{package_manager}} | **Scope**: {{scope}}
{{#if target_package}}**Package**: {{target_package}}{{/if}}

## Step 1 — Check Outdated
Run the appropriate outdated check:
- cargo: `cargo outdated`
- pip/uv: `uv pip list --outdated`
- npm: `npm outdated`

## Step 2 — Update
{{#if target_package}}
Update only `{{target_package}}`:
- cargo: `cargo update {{target_package}}`
- npm: `npm update {{target_package}}`
{{else}}
Update all to `{{scope}}` level:
- cargo: `cargo update` (respects semver in Cargo.toml)
- uv: `uv pip install --upgrade -r requirements.txt`
- npm: `npm update` (or `npm install <pkg>@latest` for major)
{{/if}}

## Step 3 — Test
1. Run full test suite — ALL tests must pass
2. Check for deprecation warnings in build output
3. Scan CHANGELOG of major updates for breaking changes

## Step 4 — Commit
1. Commit only the lock file / manifest changes
2. Reference any CVEs fixed in the commit message
"#;

// ─────────────────────────────────────────────────────────────────────────────
// FLEET SKILLS  ⭐ Critical for Phase 2.2+
// ─────────────────────────────────────────────────────────────────────────────

const WORKER_TASK_EXECUTION: &str = r#"---
name: worker-task-execution
description: Standard Fleet worker pattern - claim, analyze, execute, report
category: fleet
parameters:
  - name: worker_slug
    type: string
    required: true
    description: Slug of this worker (e.g., worker-backend)
  - name: task_type
    type: string
    required: false
    default: "any"
    description: Type of tasks this worker handles
version: "1.0.0"
author: "Arcana"
tags: [fleet, worker, task, automation]
---

# Worker Task Execution Pattern: {{worker_slug}}

## Step 1 — Poll
1. Call `list_tasks` to find tasks:
   - `assigned_worker='{{worker_slug}}'` AND `status='assigned'`
   - `assigned_worker='{{worker_slug}}'` AND `status='in_progress'` (resume interrupted work)
2. If no tasks → exit cleanly (next tick will poll again)

## Step 2 — Claim (for assigned tasks only)
1. Call `update_task_status(task_id, 'in_progress', '{{worker_slug}}')`
2. Do not re-claim tasks already `in_progress` — just resume

## Step 3 — Analyze
1. Read the task's `acceptance_criteria` carefully
2. Use `glob`, `grep`, `read_file` to understand the codebase context
3. Identify every file that needs to change

## Step 4 — Execute
1. Implement the changes incrementally
2. Run validation after each significant change:
   - Rust: `cargo check`
   - Python: `uv run pytest`
   - TypeScript: `npx tsc --noEmit`
3. If blocked after 3 attempts → `update_task_status(task_id, 'failed', '{{worker_slug}}')`

## Step 5 — Report
1. Verify ALL acceptance criteria are satisfied
2. Call `update_task_status(task_id, 'qc_review', '{{worker_slug}}')`
   **IMPORTANT: Always set `qc_review`, never `done`. QC agent reviews first.**
"#;

const SUPERVISOR_ORCHESTRATION: &str = r#"---
name: supervisor-orchestration
description: Supervisor fleet orchestration - poll, assign, monitor, recover stalled tasks
category: fleet
parameters:
  - name: poll_interval
    type: string
    required: false
    default: "1 minute"
    description: How often the supervisor polls
version: "1.0.0"
author: "Arcana"
tags: [fleet, supervisor, orchestration, task-management]
---

# Supervisor Orchestration Pattern

**Polling interval**: {{poll_interval}}

## Step 1 — Full Board Snapshot
Call `list_tasks` (all statuses) to see the complete board state.

## Step 2 — Handle `ready` Tasks
For each task with `status='ready'`:
1. **Epic check**: If the task is too large for one worker session, use `create_task` to break it into atomic sub-tasks. Set original task to `assigned` after decomposition.
2. **Worker assignment**:
   - Python/Rust backend → `worker-backend`
   - React/TypeScript UI → `worker-frontend`
   - Infra/CI/DevOps → `worker-platform`
   - MCP servers → `worker-mcp`
   - Dead code / lint → `worker-cleanup`
3. Call `update_task_status(id, 'assigned', <worker_slug>)`

## Step 3 — Assignment Rules
- Only assign if target worker has NO other `in_progress` task
- p0/p1 tasks: assign immediately
- p2/p3 tasks: only if no p0/p1 backlog

## Step 4 — Stall Recovery
For tasks with `status='in_progress'` updated > 10 minutes ago:
1. Call `update_task_status(id, 'assigned', same_worker)` to reset for retry

## Step 5 — Exit
If no `ready` tasks and no stalled tasks → exit cleanly.
QC handles `qc_review` tasks automatically.
"#;

const QC_REVIEW: &str = r#"---
name: qc-review
description: QC agent review pattern - fetch, verify criteria, run tests, approve or reject
category: fleet
parameters:
  - name: strictness
    type: enum
    values: [strict, standard, lenient]
    required: false
    default: strict
    description: Review strictness level
version: "1.0.0"
author: "Arcana"
tags: [fleet, qc, review, quality]
---

# QC Review Pattern

**Strictness**: {{strictness}}

## Step 1 — Find Work
Call `list_tasks` → filter `status='qc_review'`.
If none → exit cleanly.

## Step 2 — Review Each Task
For each task in `qc_review`:

### 2a — Read Criteria
Read the task's `acceptance_criteria` word for word.

### 2b — Inspect Implementation
Use `grep`, `glob`, `read_file` to examine every change the worker made.

### 2c — Run Checks
- Rust: `cargo check` and `cargo test`
- Python: `uv run pytest`
- TypeScript: `npx tsc --noEmit`

### 2d — Decision

**APPROVE** if ALL of the following:
- Every acceptance criterion is explicitly satisfied
- All checks pass (zero failures)
- No obvious regressions introduced

→ Call `update_task_status(id, 'done', assigned_worker)`

**REJECT** if ANY criterion is not met or checks fail:
- Be specific about exactly what failed
→ Call `update_task_status(id, 'assigned', assigned_worker)` so worker retries

## QC Standards ({{strictness}})
- **strict**: Zero tolerance for failing tests or partial criteria
- **standard**: Critical criteria must pass; minor polish can be skipped
- **lenient**: Core functionality must work; style issues are advisory
"#;

const EPIC_BREAKDOWN: &str = r#"---
name: epic-breakdown
description: Decompose an epic task into atomic worker-sized sub-tasks
category: fleet
parameters:
  - name: epic_title
    type: string
    required: true
    description: Title of the epic to break down
  - name: target_worker_types
    type: string
    required: false
    default: "backend, frontend, platform"
    description: Worker types involved in this epic
version: "1.0.0"
author: "Arcana"
tags: [fleet, supervisor, epic, task-breakdown, planning]
---

# Epic Breakdown: {{epic_title}}

**Worker types involved**: {{target_worker_types}}

## Step 1 — Analyse Epic
1. Read the epic's full description and acceptance criteria
2. Identify natural boundaries (backend API, frontend UI, database, tests, docs)
3. Map each boundary to a worker type

## Step 2 — Decompose Rules
Each sub-task MUST be:
- **Atomic**: Completable by one worker in one session (~30-60 min of work)
- **Independent**: Can be assigned and started without waiting for others (where possible)
- **Verifiable**: Has clear acceptance criteria that QC can check

## Step 3 — Create Sub-Tasks
For each identified sub-task, call `create_task` with:
- Clear title describing the single unit of work
- Specific acceptance criteria (not "implement X" but "function Y returns Z when input is W")
- Correct worker type assignment

## Step 4 — Order Dependencies
If tasks have dependencies (backend API before frontend can call it):
1. Assign dependent tasks with lower priority initially
2. Supervisor will assign them after dependencies reach `qc_review`

## Step 5 — Update Epic
Update original epic task status to indicate it has been decomposed.
"#;

// ─────────────────────────────────────────────────────────────────────────────
// DEVOPS SKILLS (continued) — Ruflo Feature 7
// ─────────────────────────────────────────────────────────────────────────────

const BROWSER_TEST: &str = r#"---
name: browser-test
description: Run browser automation tests using Playwright. Use for visual regression testing, E2E tests on frontend PRs, and verifying UI after deployments.
category: devops
parameters:
  - name: url
    type: string
    required: false
    default: "http://localhost:3000"
    description: URL to navigate to or run tests against
  - name: test_file
    type: string
    required: false
    default: ""
    description: Specific Playwright test file or directory to run (leave empty for all tests)
  - name: screenshot
    type: boolean
    required: false
    default: false
    description: Take a full-page screenshot of the URL before running tests
version: "1.0.0"
author: "Arcana"
tags: [testing, playwright, browser, e2e, qc, visual-regression]
requires_capability: [playwright]
---

# Browser Test Skill

You have access to Playwright for browser automation via the `bash` tool.

## Setup Check
First verify Playwright is installed:
```bash
npx playwright --version
```
If Playwright is missing, report that prerequisite and ask before installing it.

## Screenshot (if screenshot = true)
Take a full-page screenshot of {{url}}:
```bash
npx playwright screenshot --full-page {{url}} arcana-screenshot.png
```
The output path is relative to the project workspace. Check that the filename
is unused before running; if it already exists, choose another workspace-relative
filename. Report the saved path in your response.

## Visual Regression / E2E Tests
{{#if test_file}}
Run the specified test file or directory:
```bash
npx playwright test {{test_file}} --project=chromium
```
{{else}}
Run all Playwright tests in the project:
```bash
npx playwright test --project=chromium
```
{{/if}}

## Common Patterns
- Navigate and screenshot: confirms basic rendering is not broken
- Form fill and submit: verifies critical user flows
- Assert page title and key content: catches regressions in rendered output

## Output Format
Always return:
1. Screenshot path (if taken)
2. Test pass/fail summary (X passed, Y failed)
3. For each failure: the test name, selector used, expected value, and actual value
4. Overall verdict: PASS or FAIL
"#;

#[cfg(test)]
mod tg4_17_tests {
    use super::*;

    #[test]
    fn built_in_skill_templates_do_not_embed_posix_paths_or_shell_expansion() {
        let templates = [
            FEATURE_IMPLEMENTATION,
            API_ENDPOINT,
            REFACTORING,
            CODE_REVIEW,
            TEST_ADDITION,
            DATABASE_MIGRATION,
            ERROR_INVESTIGATION,
            PERFORMANCE_OPTIMIZATION,
            RACE_CONDITION_DEBUG,
            DOCKER_SETUP,
            CI_PIPELINE_FIX,
            DEPENDENCY_UPDATE,
            WORKER_TASK_EXECUTION,
            SUPERVISOR_ORCHESTRATION,
            QC_REVIEW,
            EPIC_BREAKDOWN,
            BROWSER_TEST,
        ];
        for template in templates {
            assert!(
                !template.contains("/tmp/"),
                "built-in skill contains a hardcoded POSIX temporary path"
            );
            assert!(
                !template.contains("$(") && !template.contains("$?"),
                "built-in skill contains POSIX shell substitution or exit syntax"
            );
            assert!(
                !template.contains("2>&1") && !template.contains("| tee "),
                "built-in skill contains a shell redirection or pipeline"
            );
            let mut in_shell_block = false;
            for line in template.lines() {
                let line = line.trim();
                if line == "```bash" || line == "```sh" {
                    in_shell_block = true;
                } else if in_shell_block && line == "```" {
                    in_shell_block = false;
                } else if in_shell_block {
                    for fragment in ["&&", ";", "`", "$?", "$(", "2>&1", "| tee ", "&"] {
                        assert!(
                            !line.contains(fragment),
                            "built-in skill embeds non-portable shell command fragment {fragment:?}: {line}"
                        );
                    }
                }
            }
        }
    }
}
