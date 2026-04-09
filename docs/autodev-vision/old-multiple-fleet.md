# AIRS Autonomous Agent Development System
### Architecture, Workflow & Implementation Plan

**Document Status:** Draft v1.0  
**Prepared by:** Razi (Programming Lead, AIRS Foundry)  
**Date:** March 2026

---

## Table of Contents

1. [Vision & Overview](#1-vision--overview)
2. [The Org Chart](#2-the-org-chart)
3. [Role Definitions](#3-role-definitions)
4. [Worker Division & Repo Assignments](#4-worker-division--repo-assignments)
5. [GitHub as the Command Board](#5-github-as-the-command-board)
6. [Communication & Dispatch Protocol](#6-communication--dispatch-protocol)
7. [Full Workflow: End-to-End](#7-full-workflow-end-to-end)
8. [Task Lifecycle](#8-task-lifecycle)
9. [Technical Setup Per Machine](#9-technical-setup-per-machine)
10. [QC Protocol](#10-qc-protocol)
11. [Infrastructure Requirements](#11-infrastructure-requirements)
12. [Pricing Breakdown](#12-pricing-breakdown)
13. [Accounts & Tools Required](#13-accounts--tools-required)
14. [Risks & Mitigations](#14-risks--mitigations)
15. [Phased Rollout Plan](#15-phased-rollout-plan)

---

## 1. Vision & Overview

The goal is to operate AIRS's development as an autonomous software factory: a human (Razi) speaks to a personal AI assistant, which translates intent into structured tasks on a shared GitHub board. A Supervisor Agent — acting as a line manager — organizes, breaks down, assigns, tracks, and quality-controls work. Dedicated Worker Agents — each running 24/7 on a dedicated PC — pick up tasks, execute them on their assigned codebases, and submit their work for QC before anything is committed.

No human needs to manually trigger a worker or write a ticket in detail. The entire chain from "spoken idea" to "committed, tested code" is automated.

**What this is NOT:**
- A replacement for human architectural decisions (those still come from Razi through the PA)
- A system that auto-deploys to production without human sign-off
- A fully headless server farm (each PC runs Claude Code Desktop, which provides stability and the full MCP ecosystem)

---

## 2. The Org Chart

```
┌─────────────────────────────────────────────────────────────────┐
│                          RAZI (Boss)                            │
│                    Speaks to Personal Claude                    │
└─────────────────────────┬───────────────────────────────────────┘
                          │  Natural language intent
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                   PERSONAL CLAUDE (PA)                          │
│  Connected to GitHub MCP — creates/labels Issues on the board  │
└─────────────────────────┬───────────────────────────────────────┘
                          │  Structured GitHub Issues
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│               SUPERVISOR AGENT (Line Manager PC)                │
│  Polls board · Breaks epics · Assigns workers · Tracks status  │
│  Pushes tasks anytime · Reviews QC reports · Updates Issues    │
└──────┬──────────┬───────────┬──────────┬───────────┬───────────┘
       │          │           │          │           │
       ▼          ▼           ▼          ▼           ▼
   Worker 1   Worker 2    Worker 3   Worker 4   Worker 5
  (Backend)  (Frontend)  (Platform)  (MCP       (Cleanup &
                                     Builder)   Refactor)
       │          │           │          │           │
       └──────────┴───────────┴──────────┴───────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │      QC AGENT         │
                    │  Reviews PRs · Runs   │
                    │  tests · Approves or  │
                    │  rejects with notes   │
                    └───────────────────────┘
```

**Total machines: 8**
- 1 × Personal Claude (your own PC/laptop — already exists)
- 1 × Supervisor Agent PC
- 5 × Worker Agent PCs
- 1 × QC Agent PC

---

## 3. Role Definitions

### Personal Claude (PA)
- **Lives on:** Your personal machine (claude.ai or Claude Desktop with GitHub MCP connected)
- **Role:** Your interface to the system. You speak naturally, it translates into structured GitHub Issues with proper labels, acceptance criteria, priority, and target worker assignment.
- **Does NOT:** Execute code, touch repos, or make decisions

### Supervisor Agent
- **Lives on:** Dedicated PC, 24/7
- **Role:** Operates like a line manager. Continuously monitors the GitHub Issues board. Responsible for:
  - Breaking down epic/high-level issues into sub-tasks
  - Assigning issues to the correct worker label
  - Pushing urgent tasks to a worker at any time via issue assignment + dispatch signal
  - Reviewing QC Agent reports and deciding pass/fail
  - Updating issue statuses throughout the lifecycle
  - Resolving blockers (e.g., if a worker is stuck, re-scope or re-assign)
- **Polling frequency:** Every 2 minutes (cron job)

### Worker Agents (×5)
- **Lives on:** Dedicated PCs, 24/7
- **Role:** Executes assigned tasks in their designated repos. Workflow per worker:
  1. Poll GitHub Issues for issues assigned to their label
  2. Claim the issue (write a lock file in the repo to prevent double-claiming)
  3. Create a git worktree branch (`worker/<worker-name>/<issue-number>`)
  4. Execute the task with full Claude Code autonomy
  5. Run tests
  6. Open a PR, tag it `qc-review`, update the issue
  7. Notify Supervisor via issue comment
- **Polling frequency:** Every 2 minutes

### QC Agent
- **Lives on:** Dedicated PC, 24/7
- **Role:** Automated code reviewer. When a PR is tagged `qc-review`:
  1. Pull the branch
  2. Review the diff against acceptance criteria in the original issue
  3. Run automated tests
  4. Post a structured QC report as a PR comment
  5. Label: `qc-approved` or `qc-rejected` (with specific notes)
  6. Notify Supervisor
- On approval: Supervisor instructs worker to merge and close the issue
- On rejection: Issue is re-opened and re-assigned to the same worker with QC notes

---

## 4. Worker Division & Repo Assignments

Your original division (cleanup, frontend, new features, memory, new MCP servers) was **role-based** rather than **repo-based**, which creates ambiguity — a "new feature" worker would need to know *which repo* and context switch constantly. The recommendation is to divide by **domain/codebase** instead, with roles being defined by the type of work that naturally belongs to each domain.

### Revised Worker Division

| Worker | Domain | Primary Repos | Secondary Repos | What It Does |
|---|---|---|---|---|
| **Worker 1: Backend** | Core Orchestrator | `core-orchestrator` | `main-infrastructure` | New features, bug fixes, API changes, agent/LLM logic, MCP client updates in the Python backend |
| **Worker 2: Frontend** | UI & Client | `frontend` | — | React components, hooks, SSE handling, UI features, styling, UX improvements |
| **Worker 3: Platform** | MCP Servers & Infra | `memory-mcp-server`, `mcp-server-adobe` | `main-infrastructure` | Maintaining and improving existing MCP servers, Docker Compose changes, infrastructure config |
| **Worker 4: MCP Builder** | New Integrations | New repos (created on demand) | `main-infrastructure` | Building entirely new MCP server repos from scratch (e.g., a new third-party integration). Each task may spawn a new repo |
| **Worker 5: Cleanup & Quality** | Cross-cutting | All repos | — | Dead code removal, refactoring, test coverage, documentation, dependency upgrades, performance optimisation |

**Notes on the division:**
- "New features" is not a standalone worker — new features belong to **Backend** (if server-side) or **Frontend** (if UI). The Supervisor routes accordingly.
- Worker 5 (Cleanup) is the only worker with write access across all repos — its PRs should have slightly stricter QC requirements.
- Worker 4 (MCP Builder) may need to bootstrap new repos entirely; its CLAUDE.md must include instructions for repo creation, initial CI setup, and registration in `mcp_config.yaml`.

---

## 5. GitHub as the Command Board

GitHub Issues is the single source of truth for all tasks. It was chosen because:

- Workers already operate inside git — tasks, branches, PRs, and reviews are all native
- GitHub's official MCP server is natively supported in Claude Code with a simple PAT setup
- File-based locking (writing a lock file to the repo) prevents two workers claiming the same task — the same pattern Anthropic used in their 16-agent C compiler experiment
- Full auditability: every task, assignment, status change, QC result, and merge is visible and timestamped
- No third-party tool to maintain or pay for

### GitHub Organisation Structure

```
AIRS-Foundry (GitHub Org)
├── core-orchestrator          ← Worker 1 primary
├── frontend                   ← Worker 2 primary
├── memory-mcp-server          ← Worker 3 primary
├── mcp-server-adobe           ← Worker 3 primary
├── main-infrastructure        ← Workers 1, 3, 4 secondary
├── airs-task-board            ← NEW: dedicated repo for cross-repo tasks
└── [new-mcp-*]                ← Created by Worker 4 as needed
```

**`airs-task-board` repo** is a new lightweight repo that serves as the cross-repo task board. Issues here are for tasks that span multiple repos or don't belong to a single repo (e.g., cleanup across all repos, architecture-level decisions). All agents have read access to all repos; workers only have write access to their assigned repos.

### Issue Label Taxonomy

```
Priority:        p0-critical · p1-high · p2-medium · p3-low
Worker Target:   worker-backend · worker-frontend · worker-platform
                 worker-mcp-builder · worker-cleanup
Type:            epic · feature · bug · refactor · infra · docs
Status:          status-ready · status-in-progress · status-blocked
                 status-pr-open · qc-review · qc-approved · qc-rejected
Source:          from-razi · from-supervisor
```

### GitHub Plan Required
**GitHub Teams** plan ($4/user/month) — provides private repos, branch protection rules, required PR reviews, and team-level permissions. For 8 accounts (1 PA + 1 Supervisor + 5 Workers + 1 QC), this is $32/month.

Alternatively, **GitHub Enterprise** ($21/user/month) if SOC2 audit logging across GitHub is needed — $168/month for 8 seats.

---

## 6. Communication & Dispatch Protocol

### How the Supervisor Pushes Tasks to a Worker (Anytime)

The Supervisor does not wait for workers to come to it. It can push work at any time via two mechanisms that work together:

**Mechanism 1 — GitHub Issue Assignment (primary)**
The Supervisor creates or updates a GitHub Issue, assigns it the worker's label (`worker-backend`, etc.) and sets status to `status-ready`. The worker's polling loop (running every 2 minutes) picks this up.

**Mechanism 2 — Dispatch Signal File (for urgency)**
Each worker PC watches a shared `dispatch/` directory inside its primary repo. The Supervisor writes a file `dispatch/<worker-name>-<timestamp>.signal` containing the issue number. This is the "tap on the shoulder" — it forces the worker's next poll to be immediate rather than waiting up to 2 minutes.

```
/core-orchestrator
  /dispatch
    worker-backend-20260325-142300.signal   ← Supervisor wrote this
```

The worker's cron script checks for signal files before sleeping, processes them first, and deletes them after acknowledging.

**Mechanism 3 — Direct PR Assignment (for QC)**
When the Supervisor wants QC to review a specific PR, it adds the `qc-review` label. QC polls for this label every 2 minutes.

### Communication Flow Diagram

```
Razi speaks
    │
    ▼
Personal Claude (PA)
    │  Creates GitHub Issue with:
    │  - Title, description, acceptance criteria
    │  - Label: worker-target, priority, type
    │
    ▼
GitHub Issues Board
    │
    ├── Supervisor polls every 2 min ──► Supervisor Agent
    │                                        │
    │                                        │  Breaks epics, assigns,
    │                                        │  updates labels, writes
    │                                        │  dispatch signals if urgent
    │                                        │
    │◄───────────── Issue updated ───────────┘
    │
    ├── Worker polls every 2 min ──────► Worker Agent
    │   (also watches dispatch/ dir)         │
    │                                        │  Claims issue (lock file)
    │                                        │  Creates git worktree branch
    │                                        │  Executes task
    │                                        │  Opens PR → tags qc-review
    │                                        │  Comments on issue
    │◄───────────── PR opened ───────────────┘
    │
    ├── QC polls for qc-review label ──► QC Agent
    │                                        │
    │                                        │  Reviews PR diff
    │                                        │  Runs tests
    │                                        │  Posts QC report
    │                                        │  Labels: qc-approved / qc-rejected
    │◄───────────── PR updated ───────────────┘
    │
    └── Supervisor sees QC result
             │
             ├── Approved → instructs worker to merge, closes issue
             └── Rejected → re-opens issue, re-assigns to worker with notes
```

---

## 7. Full Workflow: End-to-End

**Example scenario:** Razi wants to add a new endpoint to the core orchestrator that returns agent health status.

```
Step 1  │  Razi → Personal Claude (PA)
        │  "Add a /health endpoint to the orchestrator that returns
        │   the status of each agent, their last active time, and
        │   current memory usage. Should be read-only, no auth required."

Step 2  │  PA → GitHub Issues
        │  Creates Issue #142 on core-orchestrator:
        │  Title: "Add GET /agents/health endpoint"
        │  Labels: worker-backend, p2-medium, feature, status-ready
        │  Body: Full description + acceptance criteria

Step 3  │  Supervisor polls, sees Issue #142
        │  Checks if this is atomic or needs breaking down
        │  Decides it's atomic — updates label to confirm assignment
        │  Writes dispatch signal: dispatch/worker-backend-20260325-0930.signal

Step 4  │  Worker 1 (Backend) detects signal
        │  Reads Issue #142
        │  Writes lock file: current_tasks/issue-142.lock
        │  Creates git worktree: worker/backend/issue-142
        │  Executes: implements endpoint, writes tests
        │  Runs test suite
        │  Opens PR #87: "feat: Add GET /agents/health endpoint"
        │  Tags PR: qc-review
        │  Comments on Issue #142: "PR #87 opened for QC review"
        │  Updates label: status-pr-open
        │  Removes lock file

Step 5  │  QC Agent detects qc-review label on PR #87
        │  Pulls branch, reviews diff
        │  Runs: pytest, mypy, lint
        │  Posts QC report on PR:
        │    ✅ Endpoint exists and returns correct schema
        │    ✅ Tests pass (coverage 87%)
        │    ⚠️  Minor: no rate limiting on endpoint
        │  Labels PR: qc-approved (warning noted)
        │  Notifies Supervisor via issue comment

Step 6  │  Supervisor sees qc-approved
        │  Reviews QC report — decides warning is acceptable for now
        │  Creates new Issue #143: "Add rate limiting to /agents/health"
        │  (deferred cleanup task)
        │  Instructs Worker 1: merge PR #87
        │  Closes Issue #142

Step 7  │  Worker 1 merges PR #87
        │  Branch deleted, worktree removed
        │  Issue #142 closed ✅
```

---

## 8. Task Lifecycle

```
                    [Created by PA]
                          │
                          ▼
                    ┌─────────────┐
                    │  BACKLOG    │  status-ready + worker label
                    └──────┬──────┘
                           │ Supervisor picks up
                           ▼
                    ┌─────────────┐
                    │  ASSIGNED   │  Supervisor confirms + may dispatch signal
                    └──────┬──────┘
                           │ Worker claims (lock file)
                           ▼
                    ┌─────────────┐
                    │ IN PROGRESS │  status-in-progress + worktree branch active
                    └──────┬──────┘
                           │ Worker opens PR
                           ▼
                    ┌─────────────┐
                    │ IN REVIEW   │  status-pr-open + qc-review label
                    └──────┬──────┘
                           │ QC result
                    ┌──────┴──────┐
                    ▼             ▼
             ┌──────────┐   ┌──────────┐
             │ APPROVED │   │ REJECTED │
             └────┬─────┘   └────┬─────┘
                  │              │ Re-open issue
                  │              │ Add QC notes
                  │              └── Back to IN PROGRESS
                  ▼
             ┌──────────┐
             │  MERGED  │  Worker merges, branch cleaned up
             └────┬─────┘
                  ▼
             ┌──────────┐
             │  CLOSED  │  Issue closed ✅
             └──────────┘
```

**Blocked tasks:** If a worker cannot complete a task (missing dependency, unclear spec, external blocker), it comments on the issue with a detailed blocker description and labels it `status-blocked`. Supervisor is responsible for resolving blockers — either by updating the spec, breaking the task differently, or assigning to a different worker.

---

## 9. Technical Setup Per Machine

### All Machines (Common)

```
OS:              Ubuntu 24.04 LTS (recommended) or Windows 11
Claude Code:     v2.1.72+ (latest, for scheduled tasks + agent teams)
Claude plan:     Enterprise (single seat per machine)
GitHub:          Personal Access Token (PAT) with repo + issues + PRs scope
MCP configured:  GitHub MCP server (official: github/github-mcp-server)
Git:             Configured with machine-specific identity
                 e.g. git config user.name "AIRS-Worker-Backend"
Auto-start:      Claude Code Desktop launches on boot
Sleep:           Disabled ("Keep computer awake" enabled in settings)
```

### CLAUDE.md (per repo/worker)

Each worker has a `CLAUDE.md` in their primary repo that tells Claude Code exactly:
- What this worker is responsible for
- What labels to look for on GitHub Issues
- How to name branches (`worker/<name>/<issue-number>`)
- Where to write lock files (`current_tasks/`)
- How to write QC-ready PRs (description format, checklist)
- What tests to run before opening a PR
- What NOT to do (e.g., Worker 1 must not touch frontend code)

### Supervisor PC — Additional Setup

```
Cron schedule:   */2 * * * * /home/supervisor/scripts/poll-and-dispatch.sh
GitHub PAT:      Write access to all repos + issues + labels
MCP configured:  GitHub MCP (all repos), Slack MCP (optional alerts to Razi)
CLAUDE.md:       Full org chart, all worker labels, escalation rules,
                 QC pass/fail criteria, how to break down epics
```

### Worker PC — Polling Script Pattern

Each worker runs a headless Claude Code command on cron:

```bash
#!/bin/bash
# /home/worker-backend/scripts/poll.sh

# Check for dispatch signals first
SIGNALS=$(ls /home/worker-backend/core-orchestrator/dispatch/*.signal 2>/dev/null)
if [ -n "$SIGNALS" ]; then
  ISSUE_NUM=$(cat $SIGNALS | head -1)
  claude -p "You are the AIRS Backend Worker. An urgent task has been dispatched:
  Issue #$ISSUE_NUM. Read the issue using GitHub MCP, claim it by writing a
  lock file to current_tasks/issue-$ISSUE_NUM.lock, create a worktree branch
  worker/backend/$ISSUE_NUM, implement the task, run tests, open a PR, and
  comment on the issue. Follow your CLAUDE.md instructions." \
  --dangerously-skip-permissions
  rm -f $SIGNALS
  exit 0
fi

# Regular poll
claude -p "You are the AIRS Backend Worker. Poll GitHub Issues on
core-orchestrator for any issues labelled worker-backend AND status-ready.
If you find one, claim it, implement it, and open a PR. If nothing is ready,
exit cleanly. Follow your CLAUDE.md instructions." \
--dangerously-skip-permissions
```

---

## 10. QC Protocol

The QC Agent runs a structured checklist against every PR before approval:

### QC Checklist (embedded in QC Agent's CLAUDE.md)

```
□ Does the implementation match the acceptance criteria in the original issue?
□ Are new tests written for the changes?
□ Do all existing tests still pass?
□ Is there any obvious regression or breakage in related modules?
□ Does the code follow the style and patterns established in the repo?
□ Are there any hardcoded secrets, credentials, or environment-specific values?
□ Is the PR scoped correctly? (Does it touch only what the issue requires?)
□ Are there any TODO comments that should be new issues instead?
```

### QC Report Format (posted as PR comment)

```markdown
## QC Report — Issue #142 / PR #87

**Result:** ✅ APPROVED / ❌ REJECTED

### Checks
| Check | Result | Notes |
|-------|--------|-------|
| Matches acceptance criteria | ✅ | All 3 criteria met |
| New tests written | ✅ | 4 new test cases |
| All tests pass | ✅ | pytest: 142 passed, 0 failed |
| No regressions | ✅ | |
| Code style | ⚠️ | Minor: missing docstring on health_check() |
| No hardcoded secrets | ✅ | |
| PR scope | ✅ | |

### Summary
Implementation is correct and tests pass. One minor style issue noted.
Recommended: approve with Supervisor noting docstring as a follow-up.

**Reviewed by:** QC Agent  
**Timestamp:** 2026-03-25 09:47 UTC
```

---

## 11. Infrastructure Requirements

### Hardware Per Machine

Lightweight is sufficient. Claude Code's compute is in the cloud — the local machine only runs the CLI client, git, and polling scripts.

| Component | Minimum | Recommended |
|---|---|---|
| CPU | 2-core (any modern) | 4-core |
| RAM | 8 GB | 16 GB |
| Storage | 256 GB SSD | 512 GB SSD |
| Network | Stable broadband | Stable broadband |
| OS | Ubuntu 24.04 LTS | Ubuntu 24.04 LTS |
| Form factor | Mini PC / NUC | Mini PC / NUC |

**Recommended hardware option:** Intel NUC or similar mini PC (~$150–$250 per unit). 8 units ≈ **$1,200–$2,000 one-time hardware cost.**

### Network Requirements

All machines must have:
- Outbound HTTPS to `api.anthropic.com`, `github.com`, `api.githubcopilot.com`
- Static local IPs recommended for predictable SSH access
- No inbound ports required (all communication is outbound polling)

### GitHub Repo Branch Protection Rules

For all repos:
- `main` branch: require PR + QC Agent approval before merge
- Workers push to `worker/<name>/*` branches only
- Supervisor can merge approved PRs
- No force pushes

---

## 12. Pricing Breakdown

### Claude Enterprise Seats

As confirmed: Enterprise seat fee covers access only. All token usage is billed separately at API rates on top of the seat fee.

| Seat | Role | Est. Monthly Token Cost | Notes |
|---|---|---|---|
| Personal Claude | PA (Razi's existing seat) | Low | Already exists |
| Supervisor Agent | 1 seat | Medium-High | Constantly polling + breaking down tasks |
| Worker 1 (Backend) | 1 seat | High | Active coding sessions |
| Worker 2 (Frontend) | 1 seat | High | Active coding sessions |
| Worker 3 (Platform) | 1 seat | Medium | Less frequent, deeper tasks |
| Worker 4 (MCP Builder) | 1 seat | Medium | Episodic, project-based |
| Worker 5 (Cleanup) | 1 seat | Medium | Broad scope, many small PRs |
| QC Agent | 1 seat | Medium | Review sessions, not generation |

**Total new seats required: 7** (Personal Claude seat already exists)

Enterprise seat pricing is custom/negotiated — publicly reported at approximately $60/seat/month at volume, with a minimum of 20 seats and an annual contract. At minimum commitment of 20 seats: **~$1,200/month in seat fees**, with additional API token costs on top.

> **For smaller scale before reaching minimum seat requirements:** Claude Max 20x ($200/month per seat) allows heavy autonomous usage with a flat monthly fee — no per-token surprises. 7 workers × $200 = **$1,400/month flat**, all-inclusive. This is likely the better starting point before going Enterprise.

### GitHub

| Plan | Cost | Notes |
|---|---|---|
| GitHub Teams | $4/user/month × 8 accounts | $32/month |
| GitHub Enterprise | $21/user/month × 8 accounts | $168/month — only if SOC2 audit logging needed |

### Hardware (one-time)

| Item | Qty | Est. Unit Cost | Total |
|---|---|---|---|
| Mini PCs (NUC-class) | 7 new | ~$200 | ~$1,400 |
| Existing personal PC | 1 | — | — |

### Total Monthly Operating Cost Estimate

| Item | Monthly Cost |
|---|---|
| Claude Max 20x (7 worker seats) | ~$1,400 |
| GitHub Teams (8 accounts) | ~$32 |
| Electricity (8 PCs × ~30W avg × 24/7) | ~$25–$50 |
| **Total** | **~$1,460–$1,480/month** |

When ready for Enterprise (custom contract, higher volume, SOC2 compliance):
- Seat fees negotiated; token costs remain variable
- Expect $1,200–$2,000/month seat fees + token overage

---

## 13. Accounts & Tools Required

| Tool | Purpose | Account Type | Est. Cost |
|---|---|---|---|
| Anthropic Claude Enterprise | Claude Code seats for all agents | Enterprise (minimum 20 seats, annual) | Custom |
| GitHub | Repos, Issues board, PR reviews | Teams or Enterprise | $32–$168/month |
| GitHub MCP Server | Claude Code ↔ GitHub native integration | Included with GitHub account | Free |
| Git (all machines) | Version control, worktrees, locking | Open source | Free |

### Optional Enhancements

| Tool | Purpose | Cost |
|---|---|---|
| Slack MCP | Supervisor sends Razi a Slack notification on critical events | Free tier or existing |
| Sentry MCP | QC Agent checks for runtime errors introduced by a PR | From $26/month |
| Linear MCP | If GitHub Issues proves insufficient for complex epics | From $8/user/month |

---

## 14. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Two workers claim the same issue | Medium | High | File-based lock (`current_tasks/issue-N.lock`) — git's sync prevents double-claim |
| Worker gets stuck in a loop | Medium | Medium | Max turn limit in Claude Code + timeout on cron script; Supervisor detects stale in-progress after 2 hours |
| QC Agent approves bad code | Low | High | Branch protection + Razi can always review before merge; QC reports are logged |
| Runaway token cost | Medium | High | Set spend limits per user in Enterprise admin console; monitor daily |
| A PC goes offline | Low | Low | GitHub label stays `status-in-progress` with the lock file; Supervisor detects stale locks after 4 hours and re-opens the issue |
| Merge conflict between workers | Medium | Medium | Each worker uses isolated git worktrees; conflicts surface at PR time and QC flags them |
| Scope creep by a worker | Medium | Medium | CLAUDE.md explicitly constrains scope; QC checks that PR only touches what the issue requires |

---

## 15. Phased Rollout Plan

### Phase 1 — Foundation (Week 1–2)
- Set up GitHub Org with correct repos, branch protection, and label taxonomy
- Configure GitHub MCP on Razi's Personal Claude
- Write master `CLAUDE.md` templates for Supervisor, Workers, and QC
- Set up Supervisor PC with cron + polling script
- Test the PA → GitHub Issues → Supervisor loop manually

### Phase 2 — First Worker (Week 3)
- Deploy Worker 1 (Backend) on its dedicated PC
- Run 3–5 real tasks through the full pipeline manually observed
- Tune CLAUDE.md prompts based on observed behaviour
- Validate lock file mechanism works under concurrent load

### Phase 3 — QC Agent + Full Worker Fleet (Week 4–5)
- Deploy QC Agent
- Deploy Workers 2, 3, 4, 5 one at a time
- Run parallel tasks across multiple workers
- Validate no cross-repo contamination

### Phase 4 — Autonomy (Week 6+)
- Switch to fully unattended operation
- Razi monitors via GitHub dashboard and optional Slack notifications
- Weekly review of QC reports and closed issues
- Iterate on CLAUDE.md quality based on QC rejection rate

---

*This document should be treated as a living spec. As the system evolves, CLAUDE.md files on each machine are the ground truth for agent behaviour — keep them in sync with this document.*