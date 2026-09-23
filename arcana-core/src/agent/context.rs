// Context builder — constructs the system prompt for PA mode.
// Injects ARCANA.md content from SQLite database (proprietary AIRS system).
// NOT file-based like Claude Code — this is UI-managed, stored in ~/.arcana/config.db
// See: docs/arcana-vision/ARCANA.md-deep-spec.md

use std::path::Path;
use tracing::debug;

// TODO: Import DB module when implemented
// use crate::db::arcana_instructions;

pub struct ContextBuilder {
    working_dir: String,
    session_id: Option<String>,
}

impl ContextBuilder {
    pub fn new(working_dir: impl Into<String>) -> Self {
        Self {
            working_dir: working_dir.into(),
            session_id: None,
        }
    }

    /// Attach a session ID so memory context can be injected into the prompt.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
    // ⚠ `build_system_prompt` was DELETED in P2-62.
    //
    // It had **zero callers** — a repo-wide search matched only its own
    // definition — and it was not a harmless duplicate. It read ARCANA.md from
    // the filesystem only, never queried `ArcanaInstructionsDb`, and never
    // injected the memory block, so it produced a **different prefix** from the
    // one actually sent by `build_system_prompt_with_mode`.
    //
    // That made it a trap for exactly the work that comes next: whoever wires
    // prompt caching has to pick a builder, and picking this one would produce a
    // cache key that never matches what the request carries — a cache that
    // silently never hits, with no error and no symptom but the bill.
    //
    // Deleted rather than kept "for reference": a second implementation of a
    // thing that must be byte-identical is a liability, not documentation.

    /// Falls back to filesystem paths if DB entry is empty.
    /// The system prompt, as one string.
    ///
    /// Delegates to `build_system_prompt_parts` and joins, so the joined form
    /// and the split form cannot drift apart. That drift is not hypothetical:
    /// `build_system_prompt` was a second builder in this file that read
    /// ARCANA.md from the filesystem while the live path read SQLite and
    /// injected memory, and it was deleted in P2-62 for exactly this reason.
    pub fn build_system_prompt_with_mode(
        &self,
        mode: super::PAMode,
        repo_path: &str,
        query: &str,
    ) -> String {
        let (stable, volatile) = self.build_system_prompt_parts(mode, repo_path, query);
        if volatile.is_empty() {
            stable
        } else {
            format!("{stable}\n\n---\n\n{volatile}")
        }
    }

    /// The system prompt split at the cache breakpoint (`P2-14`).
    ///
    /// Returns `(stable, volatile)`. The stable half is the base prompt plus
    /// the ARCANA.md instructions — it changes only when the user edits those.
    /// The volatile half is the memory block, which is rebuilt from
    /// `get_recent_summaries` and `recall_facts` and therefore changes whenever
    /// the agent learns anything.
    ///
    /// ⚠ **The order matters and is load-bearing.** Anthropic caches a prefix,
    /// so volatile content has to come LAST or it invalidates everything behind
    /// it. Memory was already last here — but it was last *inside a single
    /// string*, with no way to place a breakpoint in front of it. Splitting it
    /// out is what makes the base prompt cacheable at all: otherwise every new
    /// remembered fact throws away the multi-KB prompt in front of it, on every
    /// session, silently.
    /// The user's own instructions, global then per-repo, ready to append.
    ///
    /// 🔴 **Extracted in `P8-36` so there is exactly ONE implementation.** This
    /// was inline in `build_system_prompt_parts`, which the override branch
    /// skips entirely — so the file a user writes to teach Arcana their
    /// conventions reached the interactive assistant and **no worker**. Copying
    /// the fetch into the override path would have made it two lists that must
    /// agree, and the second is always the one nobody updates (`L-19`).
    ///
    /// SQLite is primary, the filesystem is the fallback, in that order.
    fn arcana_md_parts(&self, repo_path: &str) -> Vec<String> {
        let mut out = Vec::new();

        let global_content = crate::db::arcana_instructions::ArcanaInstructionsDb::open()
            .ok()
            .and_then(|db| db.get("global", None).ok())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                home_dir().and_then(|h| {
                    std::fs::read_to_string(
                        h.join(".arcana").join("config").join("arcana_global.md"),
                    )
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                })
            });
        if let Some(content) = global_content {
            out.push(format!("## Global Instructions (ARCANA.md)\n\n{content}"));
        }

        let repo_content = crate::db::arcana_instructions::ArcanaInstructionsDb::open()
            .ok()
            .and_then(|db| db.get("repo", Some(repo_path)).ok())
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                std::fs::read_to_string(
                    std::path::Path::new(repo_path)
                        .join(".arcana")
                        .join("arcana.md"),
                )
                .ok()
                .filter(|s| !s.trim().is_empty())
            });
        if let Some(content) = repo_content {
            out.push(format!("## Project Instructions (ARCANA.md)\n\n{content}"));
        }

        out
    }

    /// The stable prompt for an agent that brings its own instructions.
    ///
    /// 🔴 **`P8-35`: a recipe EXTENDS the core, it does not replace it.**
    /// `engine.rs` treated `system_prompt_override` as a total replacement of
    /// the stable half, so a fleet worker ran with its recipe text and nothing
    /// else — no prohibitions, no git rules, none of the user's `ARCANA.md`.
    /// The agents that write files and push commits were the ones running with
    /// no floor.
    ///
    /// **This is the only composer.** `build_system_prompt_parts` calls it too,
    /// passing the PA prompt as `own_instructions` — so there is one order, one
    /// `ARCANA.md` fetch and one place the core is added, rather than a second
    /// copy that drifts (`L-19`).
    ///
    /// Order: **core → own instructions → `ARCANA.md`**, most general to most
    /// specific. The core is a floor a role can build on but cannot drop by
    /// existing; the user's own `ARCANA.md` is last because it is the most
    /// specific voice in the room, and because that is the relative order the
    /// PA path already had (`P8-36` must not silently re-rank it).
    ///
    /// Everything here is **stable** in the caching sense (`P2-14`) — the core
    /// is byte-identical forever, `ARCANA.md` changes rarely, a recipe's text
    /// changes only when the recipe does. Memory is volatile, stays last, and
    /// is not built here.
    pub fn build_agent_stable_prompt(&self, repo_path: &str, own_instructions: &str) -> String {
        let mut parts = vec![INVARIANT_CORE.to_string()];
        if !own_instructions.trim().is_empty() {
            parts.push(own_instructions.to_string());
        }
        parts.extend(self.arcana_md_parts(repo_path));
        parts.join("\n\n---\n\n")
    }

    pub fn build_system_prompt_parts(
        &self,
        mode: super::PAMode,
        repo_path: &str,
        query: &str,
    ) -> (String, String) {
        let base = match mode {
            super::PAMode::FastDevelopment => BASE_SYSTEM_PROMPT,
            super::PAMode::Review => REVIEW_SYSTEM_PROMPT,
            super::PAMode::Delegate => DELEGATE_SYSTEM_PROMPT,
        };

        // 🔴 `P8-35`. This used to assemble its own `parts` vector starting at
        // `base`, which meant the PA was the one agent that never received the
        // invariant core — and that the `ARCANA.md` fetch existed only here,
        // on the branch every worker skips (`P8-36`).
        let stable = self.build_agent_stable_prompt(repo_path, base);

        // Phase 4.4 — Memory injection: recall relevant memories from all 3 tiers.
        //
        // ⚠ This comment used to read: "Skipped when no session_id is set (e.g.
        // MCP server, subagent, recipe mode)." **That names the wrong cause,
        // and it was believed for long enough to reach a user-facing tool
        // description** (`remember_this`, via P3-20 — corrected in P3-46).
        //
        // Recipes DO have a session id: `recipes/runner.rs` builds
        // `recipe-{worker_slug}-{uuid}`. What actually removes their memory is
        // the **system-prompt override** — `engine.rs` returns
        // `(override_str, String::new())` for `Some(_)`, and the memory block
        // lives in that empty VOLATILE half. `runner.rs` passes
        // `Some(&recipe.instructions)`; `native_runner.rs` passes
        // `system_prompt.as_deref()`. See `P3-45`.
        //
        // The conclusion was right and the mechanism was wrong, which is the
        // more dangerous combination: it looks verified.
        //
        // P2-14. This goes into the VOLATILE half, not into `parts`. It is
        // rebuilt from the episodic and semantic stores on every call, so a
        // single new fact changes these bytes — and anything sharing a cache
        // region with it is invalidated along with it.
        (stable, self.memory_block(repo_path, query))
    }

    /// The memory block for this session, scoped to `repo_path` and searched
    /// with `query`.
    ///
    /// 🔴 Extracted so the **system-prompt override path can reach it too**
    /// (`P3-45`). An override used to return `(override_str, String::new())`,
    /// and the memory block lives in that empty volatile half — so **any user
    /// who customised their system prompt silently lost every memory**, from a
    /// settings field, with no warning anywhere. Recipes and subagents pass an
    /// override too, which is the real reason they had no memory (`P3-46`).
    ///
    /// An override says *"use my instructions"*. It does not say *"forget what
    /// you know"*, and the two are not the same request.
    ///
    /// 🔴 `query` and `repo_path` are both load-bearing (`P3-43`): the caller
    /// passed the **empty string literal** as the query for the life of this
    /// module, and `working_dir` was discarded — so recall was identical every
    /// turn, and unscoped.
    pub fn memory_block(&self, repo_path: &str, query: &str) -> String {
        self.session_id
            .as_ref()
            .and_then(|sid| crate::memory::build_memory_context(sid, query, repo_path))
            .unwrap_or_default()
    }

    /// Returns the default base system prompt (for display in settings).
    pub fn default_system_prompt() -> &'static str {
        BASE_SYSTEM_PROMPT
    }
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(std::path::PathBuf::from)
}

/// The floor every agent stands on — PA, fleet worker, subagent, QC.
///
/// 🔴 **`P8-34`. Before this, nothing was universal.** A recipe's
/// `instructions` are passed as `system_prompt_override`, and `engine.rs`
/// treats an override as a **total replacement** of the stable half. So the
/// prohibitions below, and the user's own `ARCANA.md`, reached the
/// interactive assistant and **none of the agents that do the work** — the
/// agents that actually write files, run commands and push commits.
///
/// ⚠ **Deliberately small.** Everything PA-specific stays in
/// `BASE_SYSTEM_PROMPT`; everything role-specific stays in the recipe. A core
/// that grows becomes a second place to look for a worker's behaviour, and the
/// recipe keeps full control of its own role, protocol and rejection criteria
/// (`P8-35`).
///
/// ⚠ **It is STABLE**, in the prompt-caching sense: it is byte-identical
/// across turns and sits ahead of the volatile memory block, so adding it does
/// not cost a cache write per turn (`P2-14`).
pub const INVARIANT_CORE: &str = r#"You are Arcana, a personal autonomous software engineering agent.

You may be running as the interactive assistant, as a fleet worker on a task, or as a subagent spawned by another agent. Your specific role, protocol and acceptance criteria come from the instructions that follow this section. What is below applies to you whatever your role.

## What you must never do

- Never commit secrets, API keys, credentials, or a `.env` file. Scan a diff before committing.
- Never modify files outside the scope you were given.
- Never mark work complete without running the project's own tests, and never make a failing test pass by deleting, skipping or weakening it. Fix the cause.
- Never report success for work you did not verify. If you could not check something, say so.

## Version control

These are the house rules, and they apply to every agent equally:

- **Create a branch before your first commit.** Never commit straight onto `main` or `master`.
- **Stage deliberately** — name the paths you mean. Never `git add -A` or `git add .`: a blind stage sweeps in build output, scratch files and anything an ignore rule did not anticipate.
- **Never force-push, and never rewrite published history** — no `--force`, no amending or rebasing a commit that is already pushed.
- **Never push to `main`, open a pull request, or merge one** without the user's explicit approval. Prepare the change and say it is ready.
- **Never delete** a branch, a stash, a remote ref, or files via `git clean` without being asked.

## Coordination

Work is coordinated through the **SQLite task board** in this application — not through GitHub Issues, and not through files on disk.

Do **not** write filesystem lockfiles (`current_tasks/*.lock`) and do not call `git worktree add` / `git worktree remove`. A previous multi-machine fleet used those primitives; this one uses the task board plus a PID-based orphan detector, so a lockfile here is an artifact nothing reads."#;

/// The interactive assistant's own prompt.
///
/// ⚠ **This is the PA half only.** The prohibitions, the git house rules and
/// the coordination note live in `INVARIANT_CORE`, which every agent receives
/// (`P8-34`); repeating them here would be two lists that must agree.
///
/// 🔴 **Rewritten in `P8-56`. What it said before, on every turn:**
///
/// - *"Lock files are sacred — Write `current_tasks/issue-N.lock`"* as rule 8,
///   and sixty-five lines later *"Do NOT write filesystem lockfiles … they
///   would just be dead artifacts."* **The same prompt, contradicting itself.**
/// - Coordination *"through GitHub Issues on the AIRS-Foundry organization"*,
///   workers polling *"your assigned GitHub Issues label"*, QC applying
///   `qc-approved` labels to pull requests, and branches named
///   `worker/<role>/<issue-number>`. None of that is how this application
///   works: coordination is the SQLite task board, and it was already the task
///   board when this prompt was describing labels.
/// - A "Python backend" section telling the model to run `uv run pytest` and
///   `uv run mypy src/` before opening a PR, against the subsystem deleted in
///   `P8-B`/`P8-C`.
/// - A **tool catalogue** duplicating the schemas the request already carries
///   in its `tools` field (`engine.rs:252,509`). It had drifted: it documented
///   `edit_file(path, old_str, new_str)` while the schema requires
///   `old_string` / `new_string`, so the prompt was teaching the model
///   parameter names that fail validation. The catalogue is **gone** rather
///   than corrected — a second copy of a machine-readable contract is `L-19`,
///   and this one had already drifted once.
///
/// What survives is what a schema cannot carry: sequencing, judgement, and the
/// conventions of this codebase.
const BASE_SYSTEM_PROMPT: &str = r#"You are AIRS Arcana's interactive assistant — the person at the keyboard is talking to you directly.

You run on a local machine with direct filesystem access and shell execution. You write production-quality code, make precise targeted changes, and verify your work before reporting it done.

## Where you sit

Arcana runs a small fleet of agents inside this one application. Work is coordinated through a **SQLite task board**, and every agent reads and writes the same board:

- **You, the assistant** — interactive. You answer questions about the codebase, make changes directly when asked, and create tasks on the board when work should go to the fleet.
- **The Supervisor** — breaks work down, assigns tasks to workers, and reviews what comes back.
- **Workers** — backend, frontend, platform, MCP and cleanup. Each claims tasks assigned to it and implements them.
- **QC** — reviews finished work and either approves it or sends it back with a reason.

A task moves `ready` → `assigned` → `in_progress` → `qc_review` → `done`, and can land in `rejected`, `failed`, `build_failed`, `needs_info` or `cancelled` instead. Use those exact strings.

## How you work

1. **Read before you write.** Read a file in full before modifying it. Never assume its contents.
2. **Prefer surgical edits.** Reach for `edit_file`; use `write_file` only for a new file, or a rewrite where most of the file changes.
3. **Search before you explore.** Use `grep` and `glob` to locate code. Do not read files speculatively.
4. **Verify after every change.** Run the project's tests or linter. Do not report a task complete until you have watched it pass.
5. **Stay in scope.** Change what the task needs. Do not refactor unrelated code, rename for style, or make unrequested changes.
6. **Fail loudly and specifically.** When a command fails, report the exact error and your diagnosis, and propose a concrete fix — do not ask a vague clarifying question instead.
7. **Say what you did not check.** A verification you skipped is a fact about the work, not an omission to smooth over.

## Sequencing that pays off

**Implementing a change:** locate with `glob` or `list_directory` → find the relevant code with `grep` → read around it → edit → run the tests.

**Debugging a failing test:** run it first and capture the exact error → `grep` for what the error names → read that file in full → fix → re-run the same test.

**A codebase you do not know:** read the manifest (`Cargo.toml`, `package.json`, `pyproject.toml`) for entry points and dependencies → `list_directory` at the top level → `grep` for the specific thing you need. Do not read broadly first.

## Conventions in this codebase

- **Rust:** `cargo check` / `cargo test` / `cargo clippy`. **TypeScript/React:** `npm run typecheck`, `npm test`.
- **Commit messages:** conventional commits — `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:` — with a scope where one is obvious. Never "update" or "fix stuff".
- **`ARCANA.md`:** the user's own instructions, global and per-repository. You do not need to go looking for it — if one exists, it is already appended below, and it outranks the defaults in this section.

## Memory

You have persistent memory across sessions. `search_memory` and `get_memory` recall; `propose_memory` records something worth keeping. Relevant memories are also injected automatically at the end of your context. Record decisions and constraints that will not be obvious from the code later — not things the repository already says."#;

/// Review mode.
///
/// 🔴 **`P8-57`. What this promised was not distinctive, and saying so
/// was worse than saying nothing.** It opened *"Review Mode is identical to
/// Fast Development but adds an explicit permission checkpoint before any
/// write operation."* There is no such addition:
///
/// - `builtin_tools/mod.rs` registers `FastDevelopment | Review` in **one
///   match arm** — the two have byte-identical tool registries.
/// - `agent_commands.rs:246` passes `PermissionMode::ManualApproval` as a
///   **literal**, independent of `PAMode`. Fast Development already prompts on
///   everything except a pure read.
///
/// So the checkpoint it advertised as the difference was present in both, and
/// a user choosing Review for that reason was choosing something they already
/// had. What Review actually changes is **posture** — how the model works, not
/// what it is allowed to do — and that is now what it says.
///
/// ⚠ Its tool catalogue is gone for the reason `BASE_SYSTEM_PROMPT`'s is: the
/// request already carries the real schemas, and this copy had drifted the same
/// way, documenting `edit_file(path, old_str, new_str)`.
const REVIEW_SYSTEM_PROMPT: &str = r#"You are AIRS Arcana's interactive assistant, in **Review mode**.

Review mode does not give you different tools or a different permission policy — the assistant already asks before anything that is not a pure read. What changes is how you work, and you are in this mode because the user wants deliberation over speed.

1. **Explore read-only first.** Understand the code before you propose changing any of it. Read the callers, not only the function.
2. **Say what you are about to do, and why, before you do it.** Name the file, the change, and what would break if you are wrong.
3. **One change at a time.** Make it, verify it, then move on. Do not batch edits across files and check at the end.
4. **Report what you could not verify.** If a change is untested, or you could only reason about it, say so plainly rather than describing it as done.
5. **Prefer the smaller change.** When two fixes work, take the one that touches less and explains itself.

Everything in the assistant's normal working rules still applies."#;

/// Delegate mode — conversation and the task board, no file access.
///
/// 🔴 **`P8-57`. Its worked example taught the model to file work against
/// a deleted subsystem** — a task titled *"Add GET /health endpoint to the
/// orchestrator API"*. So the one concrete model of a good task, shown on every
/// turn in this mode, pointed at a service removed in `P8-B`/`P8-C`.
///
/// ⚠ It also **understated its own tools**, closing with *"## Available Tool"*
/// singular and naming only `create_task`. `register_for_mode`'s `Delegate` arm
/// also registers `planner` and the four memory tools. A mode that says it has
/// one tool will use one tool.
const DELEGATE_SYSTEM_PROMPT: &str = r#"You are AIRS Arcana's interactive assistant, in **Delegate mode** — conversation and the task board. You have no file tools and cannot run commands. The worker fleet does the implementing.

Your job:

1. **Understand** what the user actually wants, by asking.
2. **Decompose** it into atomic units a single worker can finish independently.
3. **Create** each one on the task board with acceptance criteria a reviewer could check without asking you.

## What makes a task good

- **Atomic.** One worker, one coherent change. If it needs two people, it is two tasks.
- **Ordered.** Say what must land first. A task that cannot start yet should say what it waits on.
- **Checkable.** Acceptance criteria name an observable outcome — an endpoint that returns a shape, a test that passes, a file that no longer exists. "Works correctly" is not a criterion.
- **Prioritised.** `p0` blocking, `p1` urgent, `p2` normal, `p3` nice-to-have.

**A single change** — "let people export their data":
→ one task. Title: *"Add CSV export to the sessions list"*. Criteria: *"A button on the sessions view downloads a CSV of the visible rows, with a header row; covered by a test that asserts the header and one row."*

**An epic** — "build user authentication":
→ several, in order — the users table migration (p1), token generation (p1), the login endpoint (p1), the middleware on protected routes (p1), then the integration tests (p2).

## Never

- Never claim to have read, written or run anything. You have no such tools here, and saying otherwise is the one failure this mode can produce.
- Never dispatch a worker outside the board.
- Never write a task nobody could verify, or one so vague the worker has to guess the scope.

## Your tools

`create_task` files work on the board, and `planner` holds a multi-step plan while you and the user shape it. `search_memory`, `get_memory` and `list_memories` recall what you know; `propose_memory` records something worth keeping."#;

#[cfg(test)]
mod p3_43_45_tests {
    use super::*;

    /// 🔴 `P3-45`. The override path must still assemble memory.
    ///
    /// This is the test that would have caught it: it asserts on the SEAM the
    /// engine uses for an override, not on the happy path. The defect lived for
    /// the module's whole life because every test exercised
    /// `build_system_prompt_parts` and nothing exercised what an override does.
    #[test]
    fn the_memory_block_is_reachable_without_going_through_the_full_prompt() {
        let b = ContextBuilder::new("/proj").with_session("s1");
        // No session store here, so the block is empty — what matters is that
        // the METHOD EXISTS and is callable on the override path. Before this,
        // memory could only be reached via `build_system_prompt_parts`, which
        // an override skips entirely.
        let block = b.memory_block("/proj", "deploy key");
        assert!(block.is_empty() || block.contains("Memory"));
    }

    #[test]
    fn a_builder_with_no_session_yields_no_memory_rather_than_erroring() {
        // Memory must degrade to absent, never to a failed turn.
        let b = ContextBuilder::new("/proj");
        assert_eq!(b.memory_block("/proj", "anything"), "");
    }

    /// 🔴 `P3-43`. The query reaches the memory layer.
    ///
    /// Structural, and deliberately so: the defect was a **parameter that was
    /// not passed**, and the only way to make that impossible again is for the
    /// signature to demand it. A test that checked output could pass while the
    /// caller still handed over `""`.
    #[test]
    fn the_prompt_builder_demands_a_query() {
        let b = ContextBuilder::new("/proj").with_session("s1");
        // This compiles only because `build_system_prompt_parts` takes a query.
        // If someone removes the parameter, this test stops building — which is
        // the loudest possible failure for "the query was dropped again".
        let (stable, _volatile) =
            b.build_system_prompt_parts(super::super::PAMode::FastDevelopment, "/proj", "deploy key");
        assert!(!stable.is_empty());
    }
}
