# A1 — P0 Code Audit (adversarial, post-phase)

Auditor: fresh-context adversarial reviewer. Did not see the implementation reasoning.
Repo: `C:\AIRS Foundry\autodev\autodev`
Scope commits (oldest first): 5fca2bb, 2945ab6, ea55b36, 670e561, a478543 (fmt only, ignored), 03334ab, 0241fc6, 4b17c6f, 5d5d6ed

STATUS: COMPLETE.

## Verdict table

| # | Area | Verdict | One line |
|---|------|---------|----------|
| 1 | Overclaims | **BROKEN** | "root-cause fix for BUG-COMPILE" has zero callers (F-23); "the six surviving :8767 references were stale comments" — one renders in the UI (F-24); "dump.rdb deleted" — still in history (F-14); "clean/both crates" ignores the third workspace member, which is unformatted (F-15, F-13). |
| 2 | Regressions (gates) | **CONCERN** | Every commanded gate passes locally (check/clippy/test 73/fmt src-tauri+src-cli/tsc/vitest 6/eslint 0 errors) — but `cargo fmt --all` FAILS on `src-shared`, and CI has no job for that crate at all (F-15). |
| 3 | Dead code / orphans | **CONCERN** | Two `invoke()` calls to non-existent commands, both `.catch`-swallowed (F-16); 46 registered commands with no consumer including two P0 shipped this phase (F-17); `Recipe.agent_role` now write-only (F-12); six stale comment sites naming deleted modules (F-06, F-25). |
| 4 | Identity change | **BROKEN** | Depth is incremented twice per generation (F-21); the qc_review exemption list contradicts the shipped supervisor recipe, breaking `request_review` (F-22); QC annotations silently re-attributed to "user" (F-04); subagent inboxes unaddressable (F-05). |
| 5 | Native subagent runner | **BROKEN** | No `std::sync::Mutex` across `.await` (clean), but: registry + pool-permit leak for un-awaited spawns (F-02); `SubagentResult` accounting fields are all zero and a live consumer silently no-ops (F-01); `truncate()` byte-slice panic in the event pump (F-03); `terminate()` cannot stop an awaited subagent (F-07). |
| 6 | Tests that cannot fail | **CONCERN** | `tool_trait.rs`'s headline assertion is now type-impossible, i.e. vacuous (F-09); `ci_gate` misses the idiomatic re-break of each invariant it guards and tests the index instead of history (F-18, F-14); `reachability_gate`'s module name-set is global so any stem collision hides an orphan (F-20); `vault_commands` is the strongest of the four but scoped to one file plus two names (F-19). |
| 7 | Anything else wrong | **BROKEN** | The whole recipe fleet is still latched behind the Python sidecar P0 made inert (F-10); the scheduler still fires a duplicate paid "Mode 2 catch-up" tick (F-11). |

## Findings

### F-01 — Subagent cost/token accounting is dead, and a live consumer silently no-ops (CONFIRMED, HIGH)

`src-tauri/src/subagent/native_runner.rs:100-116` and `:127-135` hardcode every accounting field:

```rust
cost_usd: 0.0,
tokens_in: 0,
tokens_out: 0,
files_modified: Vec::new(),
```

The in-file comment is honest ("reported as zero rather than invented"), but the commit message
for `ea55b36` never says that a shipped feature stops working as a result. `spawn_qc_review.rs:218-229`
is a live consumer:

```rust
let cost = result.cost_usd;
// 1. Attribute cost to the task.
if cost > 0.0 {
    ... db.add_qc_cost(task_id_for_bg, cost) ...
}
```

`cost_usd` is now *always* 0.0, so `add_qc_cost` can never be called again — per-task QC cost
attribution is permanently dead, with no error, no warning, and no log line. This is the exact
"silent degradation" class the restructure claims to be removing. Same for `files_modified`,
which is returned to the spawning model as an empty list rather than "unknown".

### F-02 — Registry entries and pool permits leak for any spawn that is never awaited (CONFIRMED, HIGH)

`src-tauri/src/subagent/manager.rs:293-307`, `prune_terminal`, only evicts entries whose
`terminated_at` is `Some`. `terminated_at` is set in exactly one place — `finalise()`
(`manager.rs:281-289`) — which is called from exactly one place: `await_result()`
(`manager.rs:157,161,165`). `pump_events` updates `entry.handle.status` to Completed/Failed
(`manager.rs:264-266`) but never sets `terminated_at`.

So a subagent that finishes but is never `await_result`ed stays in the registry **forever**, and
because `ManagerEntry._permit: PoolPermit` (`manager.rs:42`) is a drop-guard held by the entry, its
**pool slot is never released**. The claimed protection — `manager.rs:118-119` "Prune stale
terminal entries before inserting the new one so the registry doesn't grow unboundedly over a long
session" — does not hold for that path.

Reachable today: `commands/subagent_commands.rs:68` spawns and `:80` awaits in what appear to be
two separate IPC commands, so a frontend that spawns and never awaits leaks a permit per spawn.
`terminate()` does remove the entry, but nothing calls it automatically.

### F-03 — `truncate()` panics on a multi-byte boundary, inside the event pump task (CONFIRMED, MEDIUM-HIGH)

`src-tauri/src/subagent/manager.rs:352-358`:

```rust
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}
```

`s.len()` is bytes; `&s[..max]` panics if byte `max` is not a char boundary. Called from
`short_label` → `SubagentEvent::Error` (`manager.rs:348`), which runs inside the detached
`pump_events` tokio task (`manager.rs:114-116`). A panic there kills the pump silently: the
registry mirror stops updating and `subagent:event` stops being emitted to the UI for that
subagent, with no error surfaced. This codebase's own error strings are full of `—` (U+2014,
3 bytes) — e.g. `native_runner.rs:170` `"No active provider set — cannot spawn a subagent"` —
so the hazard is not theoretical. Mitigating: `SubagentEvent::Error` is currently only produced
by `sidecar/sse.rs:142`, which is on the now-inert sidecar path, so the panic is latent rather
than live.

### F-04 — `annotate_file_tool` silently re-attributes QC annotations to "user" (CONFIRMED, MEDIUM)

`src-tauri/src/agent/builtin_tools/annotate_file_tool.rs:83-88`:

```rust
let source = match ctx.agent_role.as_str() {
    "qc-agent" => "qc_agent",
    r if r.starts_with("worker") => "worker",
    _ => "user",
};
```

Under the old MCP path the reviewer's `__agent_role` was the recipe's declared
`agent_role: Some("qc-agent")` (`recipes/defaults.rs:1310, 1827`) → `"qc_agent"`. Under the new
identity, a QC reviewer subagent's role is `AgentIdentity{role: config.parent_slug}.child(subagent_id)`
(`native_runner.rs:186-190`), i.e. the generated slug `subagent-qc-dispatcher-{millis}-{seq}`
(`subagent/slug.rs:43`). That matches neither arm, so every annotation a QC reviewer writes is now
stored with `source = "user"`. `ea55b36` lists the consequences it considered deliberate
(qc_review gate widening, supervisor messaging) — this one is not among them.

Note the same construction means the child's `role` is a *unique per-spawn slug*, not a role. Any
consumer that compares roles for equality (`send_worker_message`'s `from == to`,
`acknowledge_worker_message`'s `msg.to_slug != caller`, `list_worker_messages`' inbox lookup)
sees an identity that no other agent can ever address. See F-05.

### F-05 — Subagents cannot receive worker messages; their inbox address is unaddressable (CONFIRMED, MEDIUM)

`list_worker_messages.rs:63` uses `ctx.agent_role` directly as the inbox key
(`db.list_worker_messages_for(&to_slug, ...)`), and `acknowledge_worker_message.rs:55,77` requires
`msg.to_slug == ctx.agent_role`. For any subagent the role is the ephemeral generated slug
`subagent-…-{millis}-{seq}`, which no sender can know or type. So the three worker-messaging tools
are formally "enabled" for subagents (the `is_empty()` guard passes) but functionally inert for
them. The commit frames the identity change as *restoring* these tools; for the subagent path it
restores only the send half.

### F-06 — Stale doc comments describing deleted behaviour in `subagent/manager.rs` (CONFIRMED, LOW-MEDIUM)

The file still documents the deleted `bridge.rs`/HTTP lifecycle as current:

- `manager.rs:5` "Forward **bridge** events to:"
- `manager.rs:12-13` "Concurrency: a tokio::Mutex around the registry HashMap. All operations are short — we never hold the lock across **HTTP calls**." (there are no HTTP calls)
- `manager.rs:76-77` "Reserve a slot before generating the slug so depth/per-parent rejections happen before any **HTTP work**."
- `manager.rs:99` "Internal channel: **bridge** → manager → (event bus + last_event)"
- `manager.rs:204-205` "Graceful termination: cancels the bridge task, **then ensures core-orchestrator has the agent deleted**." — the body no longer does the second clause at all; the comment at `:223-225` correctly says it does not. The docstring and the code disagree in the same function.
- `manager.rs:250` "consumes the **bridge's** event channel"; `:278` "after the **bridge** task joins"
- `subagent/slug.rs:8-9` cites "the AgentCreate body in `bridge.rs`" — a deleted file.
- `subagent/slug.rs:28` "sanitised … to avoid breaking **core-orchestrator's** agent_id slugification rules" — no longer a constraint.

### F-07 — `terminate()`/`abort()` cannot stop an already-awaited subagent (CONFIRMED, MEDIUM)

`manager.rs:206-230` takes the `JoinHandle` out of the entry (`entry.task.take()`) and aborts it.
But `await_result` (`manager.rs:149-152`) has *already* `take()`n it for any subagent someone is
waiting on — which is every subagent spawned by `spawn_subagent`, `spawn_parallel_subagents`, and
`spawn_qc_review`. For those, `terminate(force=true)` finds `None`, aborts nothing, marks the entry
Terminated and removes it — while the child engine keeps running to completion and keeps spending
tokens. The doc comment claims it "cancels the … task". SUSPECTED-to-CONFIRMED: confirmed by the
control flow; not reproduced at runtime.

### F-08 — Registry-insert race drops the first lifecycle events (CONFIRMED, LOW)

`manager.rs:106-132`: the runner task and the pump are both `tokio::spawn`ed *before*
`self.registry.lock().await.insert(...)`, and `prune_terminal().await` sits between them.
`native_runner::run` emits `Status{Spawning}` as its very first action (`native_runner.rs:68-75`).
If the pump wins the race, `reg.get_mut(&id)` returns `None` and the event is dropped from the
registry mirror — so `last_event`/`handle.status` can stay at their initial values while the
subagent is actually running. Cosmetic, but it is a mirror the Monitor UI reads.

### F-09 — The identity test's headline assertion is now vacuous (CONFIRMED, MEDIUM)

`src-tauri/src/agent/tool_trait.rs:180-184`:

```rust
assert!(
    !source.contains("agent_role:  None") && !source.contains("agent_role: None"),
    ...
);
```

`ToolContext.agent_role` is a `String` (`tool_trait.rs:62`). `agent_role: None` **cannot compile**
any more, so this assertion can never fail — it re-checks something the type system already
forbids. A change that *should* fail it — e.g. `agent_role: String::new()` or
`agent_role: "unknown".to_string()` at a `ToolContext` site — passes it untouched.

The companion assertion at `:191-197` is also weak:

```rust
let from_identity = source.matches("identity.role").count();
assert!(from_identity >= sites, ...);
```

It counts the *substring* `identity.role` anywhere in the file, comments included, and compares it
to a count of `ToolContext {`. Adding a fourth `ToolContext` site with a hardcoded role passes as
long as the file mentions `identity.role` four times anywhere. The only assertion in the test that
can still fail on a real regression is the `depth: 0,` one — and that one is over-broad in the
other direction: any unrelated struct literal in `tool_executor.rs` with a field `depth: 0,` fails
the test for the wrong reason.

Scope gap: the test reads **only** `src/agent/tool_executor.rs`. Nothing asserts anything about the
identity constructed in `native_runner.rs:186-190` or `recipes/runner.rs:781`, which is where the
role values that F-04/F-05 turn on are actually decided.


### F-10 — The whole recipe fleet is still latched behind the Python sidecar that P0 made inert (CONFIRMED, HIGH)

`src-tauri/src/recipes/runner.rs:101-109` — the first statement of `run_recipe`:

```rust
if !self.orch_client.has_ever_been_healthy() {
    debug!("[RecipeRunner] holding tick for '{}' — orch not yet confirmed healthy (fleet-ready latch closed)", worker_slug);
    self.set_worker_status(worker_slug, "idle");
    return Ok(());
}
```

`has_ever_been_healthy()` (`sidecar/client.rs:211-213`) reads a latch set only by
`record_health_success()` (`:218-223`), which fires only on a real 2xx from the Python
core-orchestrator. Twenty lines later the same function hardcodes

```rust
let orch_healthy = false;          // runner.rs:132
```

with the comment "Mode 2 is INERT from P0 … Every worker therefore runs on the in-process Rust
engine (Mode 1), which needs no sidecar and no Python."

Those two statements are inconsistent. Mode 1 needs no sidecar, but **no tick ever reaches Mode 1
until the sidecar has answered a health probe.** On a machine with no Python — the exact
configuration `native_runner.rs:15-16` names as required by "the headline acceptance test" — every
recipe tick returns `Ok(())` after setting the worker to "idle", forever, with only a `debug!` line.
The fleet is silently dead and reports itself idle rather than blocked.

The latch predates P0; what P0 changed is its meaning. It used to guard the path the tick was
about to take. It now guards a path (`if orch_healthy`, `runner.rs:220`) that is statically
unreachable. This is the same defect class the phase claims to be eliminating — a guard whose
premise no longer exists — and no commit message mentions it.

Related: `runner.rs:131` `let _orch_reachable = self.orch_client.is_healthy().await;` still pays a
network round-trip per tick whose only remaining effect is flipping the latch above.

### F-11 — Mode 2 is "inert" but the scheduler still fires a second full tick for it (CONFIRMED, MEDIUM)

`recipes/scheduler.rs:180-198` still runs the "MODE 2 CATCH-UP TICK": if the startup tick ran while
`!orch_ready`, it waits up to 90 s for the orchestrator and then calls `runner.run_recipe(...)`
**again**:

```rust
if !orch_ready {
    if wait_for_orchestrator_ready(&runner, 90).await {
        info!("[RecipeScheduler] Orchestrator now ready — firing Mode 2 catch-up tick for '{}'", slug_owned);
        if let Err(e) = runner.run_recipe(&recipe, &slug_owned).await { ... }
    }
}
```

With `orch_healthy` hardcoded false, that second call is not a Mode 2 tick — it is a duplicate
Mode 1 run, i.e. a second paid LLM invocation of the same recipe. The comment justifying it
("agent YAML creation only happens in Mode 2 via ensure_agent") describes behaviour that can no
longer occur. `ea55b36` says Mode 2's control flow is "left in place, unreachable"; the scheduler's
Mode-2 scaffolding is *not* unreachable — it still costs money.

### F-12 — `Recipe.agent_role` is now a write-only field (CONFIRMED, LOW-MEDIUM)

`recipes/types.rs:54` declares `pub agent_role: Option<String>`, and `recipes/defaults.rs` sets it
at nine sites (`:217, 381, 638, 858, 973, 1151, 1310, 1827, 1907`). A repo-wide search for reads
(`grep -rn "\.agent_role" src-tauri/src` excluding `ctx.agent_role`) returns **zero**. The identity
that now reaches tools is `AgentIdentity::worker(worker_slug)` (`recipes/runner.rs:781`) where
`worker_slug` is the scheduler's slug, not `recipe.agent_role`. So the field the recipes declare as
the agent's role no longer influences the agent's role. Two parallel notions of "role" now exist and
nothing reconciles them. Not caught by dead-code lints because it is a `pub` serde struct field.

### F-13 — `cargo clippy --workspace --all-targets` is NOT "clean" as four commit messages claim (CONFIRMED, LOW)

Every P0 commit message states "cargo clippy --workspace --all-targets clean". It exits 0, but it
emits warnings, and no `-D warnings` is applied:

- `autodev` (bin): 1 warning
- `airsctl`: 10 warnings — `exit_with`, `partial_success`, `json_string`, `SUCCESS`, `ERROR`,
  `PARTIAL_SUCCESS`, `print_json` all `never used` (`src-cli/src/noninteractive.rs:38,39,40,45,58,69`,
  `src-cli/src/output.rs:68`), plus three `manual_is_multiple_of` at `src-cli/src/commands/logs.rs:315`
- `autodev` (lib) under `cargo check`: **34 warnings**, including `escape_html` never used
  (`src-tauri/src/graph/export.rs:36`)

"Clean" is a stronger claim than "exit code 0". With 34 lib warnings standing, a newly-orphaned
function introduced by a future change would be indistinguishable from the existing noise — which
is precisely the failure mode `reachability_gate.rs` was written to prevent.

### F-14 — `dump.rdb` is still in git history; the TG0-04 check tests a weaker claim than the one it states (CONFIRMED, HIGH)

`src-tauri/src/ci_gate.rs:205-208` states the invariant in its own words:

```rust
// The invariant is "never COMMITTED", not "never exists". ...
// What must never happen is it entering history or a shipped artifact.
```

The check that follows (`:209-224`) runs `git ls-files --error-unmatch <path>` — which inspects the
**current index only**. It cannot see history. And history is not clean:

```
$ git log --all --oneline --diff-filter=A -- "**/dump.rdb" "dump.rdb"
8841bbb refinements
```

`dump.rdb` was added in `8841bbb` and remains in every clone's object store. `0241fc6`'s claim —
"dump.rdb (175 KB committed Redis snapshot) **deleted** and `*.rdb` gitignored" — is true of the
working tree and index, not of the repository. The 175 KB still ships to everyone who clones, and
the gate written to catch exactly this reports green.

This is the L-C6 class ("a check written from the CLAIM rather than the INVARIANT") that `5d5d6ed`
records as a *learning from this phase* — committed in `4b17c6f`, the same commit as the check.

A change that *should* fail this gate but would not: committing `src-tauri/.env` or `src/.env`
(the check tests only the literal root paths `.env`, `dump.rdb`, `src-tauri/dump.rdb`), or
committing `.env.production` (not in the list). And `.gitignore` satisfaction is a substring test
(`:229`) — a `.gitignore` containing only `!.env.example` would pass the `.env` assertion.

### F-15 — `cargo fmt --check` fails on `src-shared`, and CI never checks that crate at all (CONFIRMED, HIGH)

The workspace has **three** members (`Cargo.toml:2-6`: `src-tauri`, `src-cli`, `src-shared`). Every
P0 commit message says "cargo fmt --check clean **both crates**", and `a478543` says it applied
"cargo fmt across **both** Rust crates".

```
$ cargo fmt --all -- --check
Diff in ...\src-shared\src\config.rs:1: ...
exit 1
```

`src-shared` is unformatted (aligned struct fields, out-of-order imports in `src-shared/src/config.rs`
and elsewhere). More seriously, `.github/workflows/ci.yml` has **no job for `src-shared` at all**:

- `ci.yml:67` `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `ci.yml:138` `cargo fmt --manifest-path src-cli/Cargo.toml -- --check`
- clippy (`:71`, `:142`) and tests (`:79`, `:148`) are scoped the same way

So `src-shared/src/{config,errors,lib,db/*}.rs` is never fmt-checked, never clippy-gated on its own,
and any `#[test]` placed in it **never runs in CI**. `670e561`'s headline — "make every gate capable
of failing" — leaves one third of the workspace outside every gate. The `ci_gate` test
(`ci_gate.rs:126-138`) only asserts the literal strings "cargo fmt", "cargo clippy", "cargo test"
appear *somewhere* in `ci.yml`; it never checks coverage, so it cannot catch this.

### F-16 — Two frontend `invoke()` calls name commands that do not exist, both silently swallowed (CONFIRMED, MEDIUM)

- `src/components/autodev/AgentLiveCard.tsx:110` — `invoke('navigate_to_logs', { filterAgent: agentId }).catch(() => {})`
- `src/components/WorkspaceMismatchBanner.tsx:85` — `invoke('open_workspace_picker').catch(() => {`

Neither name appears in `register_commands` (`src-tauri/src/lib.rs:198+`) nor anywhere in
`src-tauri/src`. Both fail at runtime and both discard the rejection into an empty `.catch`. This is
the identical pattern `2945ab6` singles out as the reason it replaced `get_github_pat` rather than
deleting it ("its `.catch` swallowed the failure … the exact class of silent-degradation defect this
restructure exists to remove") — and two live instances of that class went untouched in the same
phase. `tsc`, `eslint` and every P0 gate are blind to them.

### F-17 — 46 registered Tauri commands have zero frontend consumers, including two shipped by P0 itself (CONFIRMED, MEDIUM)

Cross-referencing the 257 names in `register_commands` (`src-tauri/src/lib.rs:198+`) against every
quoted occurrence in `src/` and `e2e/` leaves 46 with no consumer. Two of them were **added by
`2945ab6` in this phase** and are listed in its commit message as the new vault surface:

- `has_provider_key` — zero references in `src/`
- `list_vault_entries` — zero references in `src/`

The commit describes `list_vault_entries` at length (retired-provider probing, orphan marking, the
keyring enumeration limitation) as if it were a feature users get. Nothing calls it. The orphaned-key
surfacing it exists to provide does not exist for a user. `set_provider_key`, `set_github_pat`,
`has_github_pat`, `delete_provider_key`, `delete_github_pat` *are* wired
(`src/stores/settingsStore.ts:114,120`, `src/tabs/Settings.tsx:466,480,497`).

Also in the list: `create_checkpoint`, `compare_checkpoints`. `reachability_gate.rs:40-46` justifies
keeping `db/checkpoints.rs` unreachable on the grounds that `git/checkpoint.rs` is "the **live,
shipped**" implementation "wired to five Tauri commands". Registered is not shipped — no UI calls
them, so undo is not reachable by a user through either implementation. The reason text overstates
the state of the surviving module.

Full list: analyze_diff, analyze_pr_diff, break_down_epic, check_autodev_instructions_exists,
cmd_set_task_budget, compare_checkpoints, create_checkpoint, create_phased_task,
find_symbol_references, force_release_task, get_autodev_instructions_char_count,
get_daily_budget_summary, get_file_metadata, get_graph_communities, get_graph_edges_for_viz,
get_graph_nodes_for_viz, get_orphaned_tasks, get_plugin_config, get_recipe, get_session,
get_system_logs, get_user_preferences, get_worker_analytics, get_worker_by_slug, git_blame,
git_clone, git_init, git_worktree_add, git_worktree_remove, has_provider_key, import_skill,
list_installed_plugins, list_repos, list_skills, list_vault_entries, merge_all_repos_cmd,
register_repo, resolve_symbol, restart_sidecar, save_memory_md, search_file_contents,
search_procedural, search_semantic, set_plugin_config, suggest_worker_for_task,
summarize_session_cmd, unregister_repo.

### F-18 — `ci_gate` TG0-02: the idiomatic way to re-break each invariant passes the gate (CONFIRMED, MEDIUM)

`ci_gate.rs:96-115` — "build smokes are not gated to pushes on main" requires ONE trimmed line to
contain all of `github.event_name`, `push`, and `refs/heads/main`:

```rust
let gates_to_main_push = trimmed.starts_with("if:")
    && trimmed.contains("github.event_name")
    && trimmed.contains("push")
    && trimmed.contains("refs/heads/main");
```

Re-gating the build smokes with the most common form — `if: github.ref == 'refs/heads/main'` — has
no `github.event_name`, so it passes. So does any multi-line `if: >` block, and so does moving the
gate to a `push`-only `on:` trigger for that workflow. The check tests one spelling of the defect.

`ci_gate.rs:75` — `trimmed.starts_with("continue-on-error") && trimmed.contains("true")` misses
YAML 1.1's other true spellings (`continue-on-error: True`, `: yes`, `: on`) and
`continue-on-error: ${{ ... }}`. `:78` — `trimmed.ends_with("|| true")` misses `|| true # why`,
`|| :`, `|| exit 0`, `set +e`, and `if: always()` on a following step.

`ci_gate.rs:126-138` — every required check is a bare `source.contains(...)` against the whole file
text, so a **commented-out** step satisfies it (comment lines are not stripped here, unlike in the
loops above). And `:147-148`:

```rust
let lint_is_tsc = package_json.contains("\"lint\": \"tsc")
    || package_json.contains("\"lint\": \"tsc --noEmit\"");
```

Setting `"lint": "npm run typecheck"` restores the exact defect the test was written for — CI running
the typecheck twice and calling one of them ESLint — and passes.

`ci_gate.rs:178-192` (TG0-03 CSP) matches forbidden **literal hostnames**. A connect-src of
`'self' https://*.anthropic.com` contains none of them and passes, while granting the renderer
direct provider access — the precise thing INV-12 forbids.

### F-19 — `vault_commands` INV-12 gate is enforced for one file plus two names (CONFIRMED, MEDIUM)

`commands/vault_commands.rs:240-319` is the strongest of the four source-scanning tests: it has a
real non-vacuity guard (`assert!(scanned > 20)`, and 34 files are present), it truncates each file
at `#[cfg(test)]` so it cannot match its own literals, and the closed-set `assert_eq!` on the seven
vault commands does fail on any addition or rename inside `vault_commands.rs`.

But the general rule it claims to enforce — "no `#[tauri::command]` that reads the vault may return
a String-bearing type" — is enforced **only inside `vault_commands.rs`** (`:266-268` skips every
other file). Across the rest of `src/commands`, only two literal names are banned (`:258`):

```rust
for banned in ["fn get_provider_key", "fn get_github_pat"] {
```

A change that *should* fail this and does not: add
`pub fn get_anthropic_key() -> Result<Option<String>>` reading `VaultManager::get` to
`settings_commands.rs`. Different name, different file — every assertion passes and plaintext key
material crosses the IPC boundary again. Also: `std::fs::read_dir` is non-recursive, so a command
file placed in a subdirectory of `src/commands` is not scanned at all; and `pub async fn` headers
are invisible to the name collector at `:276` (`header.split("pub fn ")`), though that direction
fails loudly rather than silently.

### F-20 — Reachability gate: the module name-set is global, so any name collision hides an orphan (CONFIRMED, MEDIUM)

`reachability_gate.rs:75-104` builds ONE flat `HashSet<String>` of every module name declared
anywhere in the crate, then at `:132` accepts a file if `declared.contains(stem)` — with no regard
for **where** the declaration is:

```rust
let stem = file.file_stem()...;
if declared.contains(stem) { continue; }
```

So an orphaned `src/anything/types.rs`, `client.rs`, `manager.rs`, `runner.rs`, `health.rs`,
`commands.rs` or `utils.rs` is silently accepted, because some *other* directory declares a module
of that name. `670e561` claims "Proved non-vacuous: planting an orphan .rs file makes the gate fail
with that filename" — true only for a stem that is unique in the crate. The file's own caveat
(`:72-74`) admits it "cannot prove a declared module is itself reachable"; it does not admit the
collision hole, which is the far more likely one.

Two smaller holes: the parser at `:88-94` accepts any non-`//` line containing `" mod "` and ending
in `;` (a block-comment body or a string literal qualifies), and `:129` unconditionally skips any
file whose stem is `build`, anywhere under `src/`.

Scope: the gate walks `src-tauri/src` only (`:112`). `src-cli/src` and `src-shared/src` are never
scanned, and per F-15 `src-shared` has no CI job either.

### F-21 — Subagent depth is incremented TWICE per generation (CONFIRMED, HIGH)

Every spawn tool already increments before handing the config to the manager:

- `spawn_subagent.rs:158` — `depth: ctx.depth + 1, // K.11 — increment nesting depth`
- `spawn_parallel_subagents.rs:174` — `depth: ctx.depth + 1,`
- `spawn_qc_review.rs:176` — `depth: ctx.depth + 1,`

`native_runner.rs:186-190` then treats that already-incremented value as the **parent's** depth and
increments it again:

```rust
// The child's identity is derived from the parent's, so its depth is one
// greater by construction rather than by the model's say-so.
let identity = AgentIdentity {
    role: config.parent_slug.clone(),
    depth: config.depth,          // <-- this is ALREADY the child's depth
}
.child(subagent_id.to_string());  // <-- +1 again
```

`AgentIdentity::child` is `depth: self.depth + 1` (`tool_trait.rs:101-106`). So a subagent of a
depth-0 agent runs with `ctx.depth == 2`, contradicting the contract stated at `tool_trait.rs:53-55`
("0 = top-level agent, n+1 = subagent spawned by an agent at depth n") and the explicit intent at
`spawn_qc_review.rs:78-79`:

```rust
// K.11 — depth guard. qc-dispatcher runs at depth 0 → spawned reviewers
// at depth 1. Anyone else calling this tool is misbehaving.
```

Consequences, all silent:
1. Two numbers now disagree about the same subagent. `SubagentPool::acquire` is called with
   `config.depth` (`manager.rs:78`) and `SubagentSummary.depth` reports `config.depth`
   (`manager.rs:198`) — the correct value — while the child agent's own `ToolContext.depth` is one
   higher. The Monitor UI and the guard describe different trees.
2. `MAX_NESTING_DEPTH = 3` (`pool.rs:32`) and the tools' local `MAX_SUBAGENT_DEPTH = 3`
   (`spawn_subagent.rs:85`, `spawn_parallel_subagents.rs:89`) are reached after roughly half the
   intended generations, because the guard compares against a depth that grows twice per level.

This is in the commit that claims "Depth is now Rust-authored and unforgeable" and "one level
deeper, by construction". It is unforgeable, and it is wrong. No test covers it: the only depth
tests (`tool_trait.rs:144-155`) exercise `AgentIdentity::child` in isolation, never the
spawn-tool → manager → runner path where the second increment happens.

### F-22 — P0 broke the shipped supervisor recipe's `request_review` path; the commit's stated reason it is safe is false (CONFIRMED, HIGH)

`ea55b36` widened the qc_review build-evidence gate from never-firing to firing for every caller
except QC reviewers, and justified the blast radius in its own message and in the code comment
(`update_task_status_tool.rs:130-133`):

```rust
//   • Other agents reading qc_review via supervisor reconciliation
//     touch task status with non-qc_review targets only.
```

The shipped supervisor recipe says otherwise. `recipes/defaults.rs:237`:

```
- **request_review** → If triggering task is `in_progress`, call `update_task_status(id, 'qc_review')`.
  Then `resolve_worker_request(id, action='completed', note='Promoted to qc_review')`.
```

The supervisor's identity is `AgentIdentity::supervisor()` → role `"supervisor"`
(`commands/agent_commands.rs:158`, `tool_trait.rs:85-90`), which satisfies neither exemption at
`update_task_status_tool.rs:145-146` (`role.starts_with("qc-") || role.starts_with("subagent-qc-dispatcher")`).
The supervisor never runs a build, so `build_evidence::most_recent_build(&ctx.session_id)` returns
`None` for its session and the transition is refused with "Cannot move task to qc_review without
recorded build evidence."

Net effect: a worker asks the supervisor for review, the supervisor tries to promote the task, and
the promotion now always fails. The exemption list was written from an assumption about the
recipes rather than from their text.

### F-23 — BUG-COMPILE is not fixed; `platform/command.rs` has zero callers, and P0 simultaneously made the gate it feeds mandatory (CONFIRMED, HIGH)

`0241fc6` opens: "platform/command.rs is the root-cause fix for BUG-COMPILE." Present tense, and the
rest of the message describes the fix as applied. It is not applied:

```
$ grep -rn "platform::command\|use crate::platform" src-tauri/src --include=*.rs | grep -v "^src-tauri/src/platform/"
(no output)
```

`platform/mod.rs` declares `pub mod command;` and nothing else in the crate references it.
`agent/builtin_tools/bash.rs:87-105` still does exactly what the commit diagnoses:

```rust
let command = if cfg!(target_os = "windows") {
    let rewritten = rewrite_for_windows(&command, &ctx.working_dir);
    ...
};
let shell_cmd = if cfg!(target_os = "windows") {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", &command]);
```

and `bash.rs:123` records `build_evidence::record(&ctx.session_id, &command, exit_code)` where
`exit_code` is `output.status.code()` of the whole `cmd /C` line (`bash.rs:210`) — the exit status
of the *last* element of the recipe's `npm run build 2>&1 | tee build.log; echo "EXIT:$?"`, not of
the build.

`5d5d6ed` does record this honestly ("SO-6 deliberately stays RED: platform/command.rs is
unit-proven but nothing calls it"). `0241fc6`'s own message does not — it reads as a completed fix.

The composition is the real problem. `ea55b36` (two commits earlier) turned the qc_review
build-evidence gate from *never firing* into *firing for every non-QC caller*, and that gate is fed
by the exit code `bash.rs` records. P0 therefore made a mandatory gate depend on a measurement it
diagnosed as wrong and then did not wire the fix in. On Windows the recorded exit code does not
correspond to the build's outcome in either direction.

### F-24 — `:8767` is still rendered in the UI; S0-04's "PROVEN" is false for one of its six references (CONFIRMED, MEDIUM)

`4b17c6f` claims: "S0-04 PROVEN — nothing binds :8767 and no code can; **the six surviving
references were stale comments** and are now historical notes."

There are exactly six surviving references. Five are comments. The sixth is live JSX
(`src/components/sidecar/McpToolBrowser.tsx:70`):

```tsx
({tools.length} total, streamed on <span style={{ color: 'var(--text-2)' }}>:8767/mcp</span>)
```

That string renders to the user in the MCP Tool Browser and advertises an endpoint that was deleted
in `ea55b36` — the same commit that lists `McpToolBrowser` among the "four frontend surfaces" it
repaired. The other five (`main.rs:107`, `observability/error_kinds.rs:26`, `recipes/runner.rs:15`,
`sidecar/mod.rs:11`, `src/components/sidecar/SidecarPanel.tsx:250`) are comments as claimed.

### F-25 — Orphaned comment fragments left by the `ea55b36` deletions (CONFIRMED, LOW)

- `src/components/sidecar/SidecarPanel.tsx:116` — a truncated sentence with no code after it:
  `// Subscribe to MCP port-conflict state for the banner below. The store`
- `src/components/sidecar/SidecarPanel.tsx:250-253` — a JSX comment block describing the removed
  port-conflict banner ("Only renders when the preferred port (8767) couldn't be bound…"), with no
  banner beneath it.
- `src-tauri/src/agent/builtin_tools/path_lock.rs:40` — `// * mcp_server::tool_adapter — Mode 2 entry
  point uses the same registry`, naming a deleted module as a live consumer.
- `src-tauri/src/sidecar/client.rs:984` and `:1442` — cite `subagent::bridge::parse_event_frame` /
  `subagent::bridge::parse_sse_stream`; the module is deleted (the helpers moved to `sidecar/sse.rs`).
- `src-tauri/src/subagent/pool.rs:5-6` — "prevents one rogue parent from monopolising
  **core-orchestrator's** worker pool"; the pool now caps in-process tokio tasks.

### F-26 — `PoolPermit::drop`'s comment describes code that is not there, and the code can panic (CONFIRMED, LOW-MEDIUM)

`src-tauri/src/subagent/pool.rs:56-72`:

```rust
// Synchronous decrement — uses the std-Mutex-equivalent via blocking_lock
// alternative is fine here because the lock is held for microseconds.
tokio::task::spawn(async move { ... });
```

No `blocking_lock` appears; the body spawns a detached task. `tokio::task::spawn` **panics** when
called outside a runtime context, so dropping a `PoolPermit` on a non-runtime thread — process
teardown, a `block_in_place` context, a test without `#[tokio::test]` — aborts. Pre-existing, not
introduced by P0, but it sits directly on the path F-02 describes.

### F-27 — Smaller claim/evidence gaps (CONFIRMED, LOW)

- `0241fc6`: "**Two** of the ten tests assert that no resolved command anywhere contains a pipe,
  redirect, separator or `$`." One does — `platform/command.rs:247-278`. The other nine are
  `npm_build_is_run_build_not_a_shell_line`, `npm_test_does_not_take_a_run_prefix`,
  `cargo_lint_is_clippy`, `python_has_no_invented_build_step`,
  `only_the_npm_family_needs_a_windows_shim`, `windows_resolves_the_npm_shim_when_one_exists`,
  `non_shim_tools_are_never_rewritten`, `detection_prefers_the_lockfile_that_is_present`,
  `display_is_for_humans_only` — none of which asserts the metacharacter rule. (The count of ten
  tests is correct.) That one test also checks only `cmd.args`, never `cmd.program`.
- `5fca2bb`: "all **four** clients set `https_only`." There are three `reqwest::Client::builder()`
  sites (`check.rs:86`, `download.rs:68`, `download.rs:212`), all with `.https_only(true)`;
  `sidecar_client()` is one builder shared by the checksum and signature downloads. The invariant
  holds; the count does not.
- `5fca2bb`: "a release build without a key **fails to compile**." The const-assert exists
  (`verify.rs:29-36`) and is gated on `#[cfg(not(debug_assertions))]`, but CI's only build is
  `npm run tauri -- build --debug --no-bundle` (`ci.yml:204`, `:251`), where `debug_assertions` is
  on. Nothing in CI ever compiles the branch that would fail, so the claim is unproven by the gates.
- `check.rs:78-82`: beta/nightly are implemented as `url.replace("releases/latest", "releases?per_page=5")`.
  If `AUTODEV_UPDATE_URL` is overridden to a manifest that does not contain that literal substring,
  the replace is a no-op and both channels silently fall back to the stable endpoint.
- `subagent/native_runner.rs:100-116` — `run()` returns `Ok(...)` on every path, converting failures
  into a `SubagentResult` with `status: Failed`. `manager.rs:160-163`'s `Ok(Err(e))` arm is therefore
  unreachable. `spawn_subagent.rs:173-181` does check `result.status`, so this is not currently a
  swallowed error, but any future caller that only checks the `Result` will treat a failed subagent
  as a success.
- `native_runner.rs:166` — `deps.state.lock().unwrap()` on a `std::sync::Mutex`. The guard is scoped
  to a block and dropped before the first `.await`, so there is **no** lock-held-across-await
  (checked: the only other `std::sync::Mutex` uses in the run path are `recipes/runner.rs:182`
  and `:184`, also block-scoped). The `.unwrap()` is a poison-panic inside a `tokio::spawn`ed task,
  which would kill the subagent silently rather than report.

## Gate results

Run on Windows 11, this working tree, at audit time. Literal outcomes:

| Command | Exit | Result |
|---|---|---|
| `cargo check --workspace --all-targets` | 0 | **PASS** — 34 warnings in `autodev` (lib), 1 in `autodev` (bin), 7 in `airsctl`. Includes `escape_html` never used (`src-tauri/src/graph/export.rs:36`) and seven never-used items in `src-cli/src/noninteractive.rs` / `output.rs`. |
| `cargo clippy --workspace --all-targets` | 0 | **PASS** — not clean: 1 warning in `autodev` (bin), 10 in `airsctl` (7 dead-code + 3 `manual_is_multiple_of` at `src-cli/src/commands/logs.rs:315`). No `-D warnings` anywhere, so "clippy clean" in four commit messages means "exit 0". |
| `cargo test --all-targets --manifest-path src-tauri/Cargo.toml` | 0 | **PASS** — `73 passed; 0 failed; 0 ignored`, plus a `0 tests` main.rs target. Matches `4b17c6f`'s claim of 73. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 0 | **PASS** |
| `cargo fmt --manifest-path src-cli/Cargo.toml -- --check` | 0 | **PASS** |
| `cargo fmt --manifest-path src-shared/Cargo.toml -- --check` | **1** | **FAIL** — `src-shared/src/config.rs` and others. Not run by CI, not covered by any commit's "both crates". |
| `cargo fmt --all -- --check` (workspace) | **1** | **FAIL** — same cause. |
| `npx tsc --noEmit` | 0 | **PASS** — no output. |
| `npm test` (`vitest run`) | 0 | **PASS** — 1 file, 6 tests passed, 25.25 s. Sole test file is `src/stores/toastStore.test.ts`. |
| `npx eslint .` | 0 | **PASS** — `44 problems (0 errors, 44 warnings)`. Warnings are `no-explicit-any` / `react-hooks/exhaustive-deps` as designed. `eslint.config.js` ignores `src-tauri/**`, `src-cli/**`, `submodules/**`, `installers/**`, `*.config.js`, `*.config.ts`. |

Not run, per the constraints: `npm run tauri build`, `npm run test:e2e`.

Cross-check of the gate counts in the commit messages: 47 → 53 → 56 → 58 → 68 → 73 is consistent
with the current 73. The `--lib` vs `--all-targets` distinction matters: `670e561` switched CI to
`--all-targets`, and `ci_gate.rs` / `reachability_gate.rs` tests only exist because of that.

## What I could not verify

- **Runtime behaviour of anything.** No Tauri build, no app launch, no e2e. Every finding is from
  source, `git`, and the gate commands above. F-02 (permit leak), F-03 (`truncate` panic), F-07
  (`terminate` no-op), F-10 (fleet latch), F-21 (double depth), F-22 (supervisor `request_review`)
  are confirmed by control flow, not reproduced.
- **Whether the core-orchestrator sidecar is auto-started on this machine**, which determines how
  often F-10's latch actually closes in practice. `sidecar/mod.rs:527` and `:645` still spawn
  processes; I did not trace which, or whether one of them satisfies `has_ever_been_healthy()`.
- **Whether F-16's two missing commands (`navigate_to_logs`, `open_workspace_picker`) predate P0.**
  They are live orphans now; I did not bisect their origin.
- **The 46-command orphan list (F-17) is a textual cross-reference**, not a call-graph. A command
  invoked through a computed name (`invoke(cmdName)`) would appear as an orphan falsely. I spot-
  checked the vault entries by hand and they are genuinely unreferenced; the rest are unverified
  individually. My first pass over-reported (multi-line `invoke(` calls were missed by the regex);
  the figures above come from the corrected pass that searches for each name as a quoted literal
  anywhere in `src/` and `e2e/`.
- **Whether the recorded exit code in `bash.rs` is wrong in the direction `0241fc6` describes**
  (successful build recorded as failed) or the opposite. Either way it is the exit status of
  `cmd /C <whole line>` rather than of the build; I did not execute a recipe build line to
  determine which.
- **`git log --all` history claims** are limited to what this clone contains; I ran only read-only
  git commands and did not fetch.
- **The ADR log and TESTING_GATES/READINESS_MATRIX documents** — I audited code against commit
  messages, not the tracker documents. `5d5d6ed` self-reports several of the gaps above (SO-6 RED,
  TG0-03/04/05 PARTIAL); where the tracker is honest and the commit message is not, I scored the
  commit message.
- **Whether any of the four source-scanning tests would fail against the pre-P0 tree.** `5fca2bb`
  claims its 14 updater tests "all fail against the previous code"; I did not check out the parent
  commit to confirm, as that would require writing to the working tree.
