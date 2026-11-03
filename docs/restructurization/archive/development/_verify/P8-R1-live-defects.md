# P8-R1 — Adversarial verification of P8-0 "live defect" claims

**Verifier role:** adversarial pre-build verifier. Default verdict is REFUTED when evidence is ambiguous.
**Scope:** claims P8-25 .. P8-32 in the P8-0 task group.
**Method:** read-only inspection of the shipped tree at `C:\AIRS Foundry\autodev\autodev`. No writes outside this file. No app runs, no migrations, no DB-touching tests.
**Status:** IN PROGRESS (written incrementally; sections appended as each claim is investigated).

---

## P8-25 — "Every recipe saved in the UI is silently stranded"

**VERDICT: PARTIALLY CONFIRMED — the core mechanism is real and is in fact *worse* than
stated; the final sentence ("editing a shipped recipe strands it too") is OVERSTATED.**

### Every cited fact checks out

`autodev-core/src/recipes/types.rs:64-71`:
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FallbackPolicy {
    #[default]
    Mode2Only,
    Mode1Capable,
}
```

`autodev-core/src/recipes/types.rs:84-88`:
```rust
    // Doc 05 Phase 3 — per-recipe fallback policy. `serde(default)` so
    // existing on-disk YAML without the field deserializes as Mode2Only.
    #[serde(default)]
    pub fallback_policy: FallbackPolicy,
```

`src/components/recipes/RecipeEditor.tsx:139-149` — `buildRecipe()` enumerates ten fields
and omits `fallback_policy`:
```tsx
  const buildRecipe = (): Recipe => ({
    version:         recipe?.version ?? '1.0.0',
    id,
    title,
    description:     description.trim() || null,
    schedule:        schedule.trim()    || null,
    permission_mode: permMode,
    agent_role:      agentRole.trim()   || null,
    model:           model.trim()       || null,
    is_system:       recipe?.is_system  ?? false,
    instructions:    instructions.trim(),
  })
```
The TypeScript `Recipe` interface itself (`src/components/recipes/RecipeEditor.tsx:12-23`)
has no `fallback_policy` member, so the omission is not a slip in one function — the field
does not exist anywhere in `src/`. `rg fallback_policy src/` returns zero hits.

`src-tauri/src/commands/recipe_commands.rs:113-134` — `update_recipe` is a full overwrite:
it backs up, then calls `save_recipe(&dir, &recipe)` with the deserialized payload. And
`autodev-core/src/recipes/parser.rs:72-79`:
```rust
pub fn save_recipe(recipes_dir: &Path, recipe: &Recipe) -> Result<()> {
    let yaml = serde_yaml::to_string(recipe)
        .map_err(|e| AutoDevError::Agent(format!("Failed to serialize recipe: {e}")))?;
    let path = recipes_dir.join(format!("{}.yaml", recipe.id));
    std::fs::write(&path, yaml)?;
    Ok(())
}
```
No read-modify-write, no field merge — `std::fs::write` replaces the file wholesale with
the serialization of a struct whose `fallback_policy` was defaulted to `Mode2Only` during
Tauri argument deserialization.

The guard is where the plan says it is, `autodev-core/src/recipes/runner.rs:717-737`:
```rust
        if matches!(
            recipe.fallback_policy,
            crate::recipes::types::FallbackPolicy::Mode2Only
        ) {
            info!(
                "[RecipeRunner] Worker '{}' Mode 1 fallback skipped per policy \
                 (recipe.fallback_policy = mode2_only) — staying idle until \
                 core-orchestrator is reachable.",
                worker_slug
            );
            emit_worker_tick_end(WorkerTickEndPayload {
                slug: worker_slug.to_string(),
                outcome: "fallback_skipped_policy".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
            self.set_worker_status(worker_slug, "fallback_idle");
            return Ok(());
        }
```
`return Ok(())` — never runs, never errors. Confirmed.

### The plan understates it: the guard is not conditional, it is unconditional

The plan's phrasing implies the guard fires because Mode 2 happens to be down. It is not a
matter of luck. `autodev-core/src/recipes/runner.rs:173-174`:
```rust
        let _orch_reachable = self.orch_client.is_healthy().await;
        let orch_healthy = false;
```
`orch_healthy` is a hardcoded `false` (a P0 decision, ADR-LOG-RESTRUCT-0008, because the
in-process MCP server the LivingAgent needed was deleted). The health probe's result is
discarded into `_orch_reachable`. So `if orch_healthy { … }` at `runner.rs:275` is dead
code, the `else` at `runner.rs:703` always runs, and **every** tick of **every** recipe
reaches line 717. There is no configuration, no environment, no running sidecar under which
a `Mode2Only` recipe executes today. The correct statement of the defect is stronger than
the plan's: `Mode2Only` is not "the policy for recipes that need Mode 2", it is "never run",
full stop — and it is the serde default.

### Where the claim is overstated

"`update_recipe` is a full overwrite, so editing a shipped recipe strands it too."

The overwrite is real, but the stranding of a *shipped* recipe is not durable.
`autodev-core/src/recipes/defaults.rs:43-65`:
```rust
/// Write default system recipes to the recipes directory.
/// Always overwrites — code is the source of truth for system recipes.
pub fn ensure_default_recipes(recipes_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(recipes_dir)?;
    …
    for (_id, build_fn) in defaults {
        save_recipe(recipes_dir, &build_fn())?;
    }
```
and `autodev-core/src/recipes/mod.rs:26-31` (`init()`), called from
`src-tauri/src/main.rs:123`. All nine shipped recipes are rewritten from code on **every
app launch**, and all nine carry `fallback_policy: super::types::FallbackPolicy::Mode1Capable`
(`defaults.rs:232, 408, 677, 916, 1043, 1226, 1399, 1971, 2064`). So a force-edited system
recipe is stranded only for the remainder of the current session and is repaired at the
next start — while the user's edit is silently destroyed, which is a *different* live defect
that P8-0 does not name. Whether the runner re-reads the YAML mid-session or holds a cached
`Recipe` decides whether the strand even lasts that long; I did not chase that (see below).

A **newly created** recipe (any id outside the nine defaults) is not in `ensure_default_recipes`
and is therefore stranded permanently. That is the durable half of the claim, and it holds.

### Remedy attack

No remedy was quoted for P8-25, so there is nothing to attack — but note the shape of the
trap for whoever writes one. Flipping `#[default]` to `Mode1Capable` fixes new recipes and
leaves the ~10-line `Mode2Only` guard as a branch nothing can select from the UI. Adding the
field to `buildRecipe()` fixes the UI and leaves every recipe already written to
`~/.autodev/recipes/*.yaml` with `fallback_policy: mode2_only` on disk, still stranded,
because `#[serde(default)]` only fills a *missing* key — an explicitly-written
`fallback_policy: mode2_only` survives any change to the Rust default. **A fix that does not
also address recipe YAML already on users' disks is not a fix.** Since P8 deletes Mode 2
entirely, deleting `FallbackPolicy` and the `runner.rs:711-737` block outright, plus
`#[serde(default, skip)]`-style tolerance for the stale on-disk key so old YAML still parses,
is the only variant that leaves no stranded state.

### Could not determine

- Whether the running scheduler re-reads recipe YAML per tick or caches a `Recipe` at load —
  this decides whether a UI save takes effect immediately or at next launch. Settles by
  reading `autodev-core/src/recipes/scheduler.rs` for where the `Recipe` handed to
  `run_worker_tick` originates.
- Whether a stranded custom recipe surfaces anywhere in the UI as anything other than an idle
  worker. `set_worker_status(worker_slug, "fallback_idle")` writes a distinct string; whether
  the frontend renders it distinctly is a separate check (`rg fallback_idle src/`).

## P8-26 — "`bash` rewrites POSIX pipes into PowerShell cmdlets and runs them under `cmd`"

**VERDICT: CONFIRMED as a defect, REFUTED as a mechanism. The plan describes the wrong
causal chain, understates the damage, and its named remedy cannot be applied to `bash` at
all.**

### The three cited facts are exact

`autodev-core/src/agent/builtin_tools/bash.rs:533-546`:
```rust
    // `head -N` → `Select-Object -First N` (PowerShell)
    // Handle both `head -20` and `head -n 20` forms.
    out = out.replace(" | head -n ", " | Select-Object -First ");
    out = out.replace(" | head -", " | Select-Object -First ");
    // `tail -N` → `Select-Object -Last N`
    out = out.replace(" | tail -n ", " | Select-Object -Last ");
    out = out.replace(" | tail -", " | Select-Object -Last ");
    // `grep PATTERN` in a pipe → `Select-String -Pattern PATTERN`
    // This is a best-effort rewrite; complex grep flags are not handled.
    out = out.replace(" | grep ", " | Select-String ");
    out = out.replace(" | grep -i ", " | Select-String -Pattern ");
```

`autodev-core/src/agent/builtin_tools/bash.rs:105-118` — the rewritten string shadows the
original and is what reaches `cmd`:
```rust
        let command = if cfg!(target_os = "windows") {
            let rewritten = rewrite_for_windows(&command, &ctx.working_dir);
            …
            rewritten
        } else {
            command
        };

        let mut shell_cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &command]);
```

And `bash.rs:158` records that same rewritten string as evidence:
```rust
                super::build_evidence::record(&ctx.session_id, &command, exit_code);
```

### Measured: the plan says the build runs and its status is lost. It does not run at all.

Run on this Windows 11 host (read-only probes, scratchpad only):

| probe | exit | side effect |
|---|---|---|
| `cmd /c "echo hi \| Select-Object -Last 5"` | **255** | — |
| `cmd /c "echo hi \| tail -50"` | **255** | — |
| `cmd /c "mkdir DIR \| Select-Object -Last 5"` | 255 | **dir NOT created** |
| `cmd /c "mkdir DIR \| more"` (control) | 0 | dir created |
| `cmd /c "SHIM.cmd 2>&1 \| Select-Object -Last 50"` | 255 | **marker NOT written** |
| `cmd /c "SHIM.cmd 2>&1 \| more"` (control) | 0 | marker written |

The last pair is the real case: `npm` on Windows is a `.cmd` shim. When the right half of a
`cmd` pipeline cannot be resolved, **cmd does not execute the left half either**. So
`npm run build 2>&1 | tail -50` on Windows does not produce a build whose status is
discarded — **it produces no build**. The artefact is never written and the tool returns
exit 255 with `'Select-Object' is not recognized`.

(One nuance: an already-resolved external `.exe` on the left *does* run — `xcopy.exe /? > f | Select-String foo`
wrote `f` and still exited 255. The abort applies to internal commands and shim/batch
programs, which is exactly the npm/pnpm/yarn family.)

### The stated chain to the qc_review veto is wrong

> "`build_evidence::record` stores a successful build as failed, and `update_task_status`
> reads that to veto `qc_review`."

`autodev-core/src/agent/builtin_tools/build_evidence.rs:54-60`:
```rust
pub fn exit_code_is_attributable(command: &str) -> bool {
    !command.contains('|')
        && !command.contains(';')
        && !command.contains("&&")
        && !command.contains("||")
        && !command.contains('\n')
}
```
The rewrite replaces the *program name* on the right of the pipe; the `|` itself survives.
So every command the rewriter touches is recorded with `exit_code_attributable: false`
(`build_evidence.rs:176`). In `update_task_status_tool.rs:158-190` the match arms are
ordered:
```rust
                    Some(ev) if !ev.exit_code_attributable => { … "ran as a pipeline …" }
                    Some(ev) if ev.exit_code != 0 => { … "exited with code {}" }
```
The **attributability arm fires first**. The agent is never told "the build failed"; it is
told "re-run the build as a plain command" — correct advice that would be given identically
on Linux where `tail` exists and the build genuinely succeeds. **The Windows rewrite is not
load-bearing for the `qc_review` veto**, and it is structurally incapable of being so: any
command the rewriter modifies contains a `|` and is therefore non-attributable by
construction. A change to the rewriter will not change the `qc_review` behaviour at all.

The real consequence is upstream of the gate: the agent's build never happens, it never gets
the build output it asked for, and it burns turns on a command that cannot work.

### The rewrite is worse than doing nothing

Both `| tail -50` and `| Select-Object -Last 50` fail identically under `cmd /C` on a bare
Windows box (measured above, both 255). But:

1. `bash.rs:29` already tells the model, on Windows, *"The shell is Windows CMD. Avoid
   Unix-only utilities (`head`, `tail`, `jq`, `grep` in pipes)."* The rewriter then converts
   the thing the model was told to avoid into a **different** thing that also does not work,
   so the error names a cmdlet the model never wrote.
2. `bash.rs:211-223` compounds it:
   ```rust
                   if is_error
                       && exit_code == 255
                       && cfg!(target_os = "windows")
                       && output.contains("is not recognized")
                   {
                       let suggestion = format!(
                           "{output}\n\n[SUGGESTION] This command uses a Unix utility not available on Windows. \
                           Try rewriting it with PowerShell-compatible syntax (e.g. `Select-Object -First N` instead of `head -n`, …
   ```
   The exit-code check (`== 255`) does fire — measured 255, not 9009. So on the very failure
   the rewriter caused, the agent is advised to use `Select-Object -First N`: **the exact
   syntax the rewriter already substituted and that `cmd` cannot run.** That is a closed loop
   the model cannot escape by following instructions.
3. On a machine with Git's `usr/bin` on PATH the rewrite is a strict regression: `tail.exe`,
   `head.exe` and `grep.exe` exist there (verified present at
   `C:\Program Files\Git\usr\bin\` on this host, though not on this shell's `PATH`), so the
   *unrewritten* command would have worked under `cmd` and the rewrite breaks it.

### Remedy attack — "route through `platform/command.rs` (built in P0-23; still zero callers)"

Zero callers is correct: searching the tree for `platform::command`, `CommandIntent`,
`ResolvedCommand` and `Toolchain::` returns **no hits outside
`autodev-core/src/platform/command.rs` itself**. `src-tauri/src/platform.rs` is a six-line
re-export (`pub use autodev_core::platform::*;`).

But the remedy is a **category error for `bash`**. `platform::command::resolve` takes
`(Toolchain, CommandIntent)` and returns a fixed argv
(`autodev-core/src/platform/command.rs:112-154`): `Npm + Build → ["run","build"]`, and so
on. It cannot express anything a shell tool is for. `bash` receives an arbitrary
model-authored string; there is no `CommandIntent` for `npm run build 2>&1 | tail -50`, for
`npx tsc --noEmit -p tsconfig.app.json`, or for any of the thousands of commands the model
will actually write. Routing `bash` "through" this module means either (a) discarding every
command that does not pattern-match one of 7 toolchains × 4 intents, or (b) building a
classifier from shell string → intent — the same fragile text matching that
`build_evidence::classify` already does and that P8-28 is a bug report about.

What `platform/command.rs` *is* fit for is callers that already know their intent: the
recipe build step, `validate.rs`, `e2e_probe.rs`, the CI gate. Those should use it. `bash`
needs something different and the plan does not name it: either a real POSIX shell on
Windows (Git-Bash `sh.exe` when present) or dropping `rewrite_for_windows` and letting the
honest `'tail' is not recognized` reach the model. **Deleting `rewrite_for_windows` is
strictly better than keeping it and is a small, contained change; adopting
`platform/command.rs` inside `bash` is not possible.** If P8-0's item is scoped as "make
`bash` use `platform/command.rs`", it will either not land or will land as something else.

### Could not determine

- Whether any shipped recipe's instructions actually emit a `| tail` / `| head` / `| grep`
  build line. The header comment of `platform/command.rs:6-15` asserts the recipes' build
  step is `npm run build 2>&1 | tee build.log; echo "EXIT:$?"` — note `tee`, which the
  rewriter does **not** handle at all, so that particular line is not even a rewrite victim.
  Settles by searching `autodev-core/src/recipes/defaults.rs` for those pipe forms.
- Why `cmd`'s pipeline abort applied to an internal command and a `.cmd` shim but not to a
  resolved `.exe`. It does not change the verdict — npm is a shim — but a regression test
  must pin the shim case, not the `.exe` case.

---

## P8-27 — "`bash` never kills its child"

**VERDICT: CONFIRMED, in every particular.**

`autodev-core/src/agent/builtin_tools/bash.rs:143-152` — the timeout drops the future and
nothing else:
```rust
        let result = timeout(
            Duration::from_secs(timeout_secs),
            run_command(shell_cmd, &ctx.working_dir),
        )
        .await;

        match result {
            Err(_) => ToolResult::error(format!(
                "Command timed out after {timeout_secs}s: {command}"
            )),
```
`autodev-core/src/agent/builtin_tools/bash.rs:254-262` — `run_command` never holds a `Child`
it could kill:
```rust
    cmd.current_dir(&normalized_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().await?;
```
Searching `bash.rs` for `kill_on_drop`, `kill(`, `Child` and `spawn(` returns **zero hits**.
Tokio's default is `kill_on_drop(false)`; dropping the `output()` future orphans the child.

The two comparison sites are exactly as cited.
`autodev-core/src/agent/builtin_tools/validate.rs:277-281`:
```rust
/// ⚠ `kill_on_drop(true)` is load-bearing, not decorative: on timeout the
…
    cmd.kill_on_drop(true);
```
`autodev-core/src/agent/builtin_tools/e2e_probe.rs:418` and `:636` both set it too.

**The invariant is already written down as a gate — and the gate does not cover `bash`.**
`src-tauri/src/ci_gate.rs:3329-3333`:
```rust
        assert!(
            helper_body.contains("kill_on_drop"),
            "without kill_on_drop the timed-out child is orphaned and keeps running"
        );
```
That test (`tg_p3_17_every_validator_subprocess_is_bounded`) reads
`autodev-core/src/agent/builtin_tools/validate.rs` and nothing else. So the project has
already articulated this exact rule, enforced it on one file, and left the file that runs
**model-authored commands** — the highest-risk spawner in the tree — outside the gate. That
is a stronger statement of the defect than the plan makes, and it names the fix's shape:
widen `TG-P3-17` to `bash.rs`, or the same hole reopens.

Two caveats the remedy will have to face, neither of which weakens the verdict:

1. `kill_on_drop(true)` alone is **not sufficient** for the `cargo build` scenario the plan
   names. On Windows the child is `cmd.exe`; killing `cmd` does not kill the `cargo.exe` it
   spawned, and `cargo` is what holds `target/.cargo-lock`. A correct fix needs a Job Object
   (Windows) or a process group plus `kill(-pgid)` (POSIX) — which is what the plan's own
   words ("no process-group kill") imply, but `kill_on_drop` is the only mechanism the two
   cited exemplars use, so copying them reproduces a half-fix that still leaves the lock
   held.
2. `cmd.output()` cannot be combined with an explicit kill; the fix must move to
   `cmd.spawn()` + `wait_with_output()` so a `Child` handle exists at all.

### Could not determine

- Whether a stale `target/.cargo-lock` genuinely causes *every later build in the session* to
  time out. Cargo's lock is released when the holder exits; the plan's scenario needs the
  orphan to keep running, which it does. Settles by measurement, not reading: start a long
  `cargo build` through the tool, let it time out, then time a second `cargo build` in the
  same directory. I did not run it — it is a multi-minute compile and outside a read-only
  budget.
- Whether `bash`'s per-call `timeout_secs` default is short enough for this to bite in
  practice. Settles by reading the `timeout_secs` extraction near `bash.rs:60-100`.

---

## P8-28 — "The build-evidence gate matches command *text*"

**VERDICT: CONFIRMED — and the exploit is simpler than the one the plan names.**

`autodev-core/src/agent/builtin_tools/build_evidence.rs:99-134`:
```rust
pub fn classify(command: &str) -> Option<BuildEvidenceKind> {
    let c = command.to_lowercase();
    …
    let node_hits = [
        "npm run build",
        "npm install",
        …
    ];
    if node_hits.iter().any(|h| c.contains(h)) {
        return Some(BuildEvidenceKind::Node);
    }
```
`contains`, on the raw command string. Nothing inspects the filesystem, a build artefact, a
timestamp, or the exit status *of a build*.

Trace the plan's example, `grep -rn "npm run build" package.json`:
1. `classify` → `c.contains("npm run build")` → `Some(Node)`.
2. `exit_code_is_attributable` (`build_evidence.rs:54-60`) → no `|`, `;`, `&&`, `||`,
   newline → **`true`**.
3. `grep` on a `package.json` that has a build script → exit 0.
4. `record` (`build_evidence.rs:168-184`) stores `{kind: Node, exit_code: 0,
   exit_code_attributable: true}`.
5. `update_task_status_tool.rs:158-190` falls through every rejection arm to
   `Some(_) => { /* ok — proceed */ }`.

Confirmed. But the plan picked a needlessly elaborate example. `bash("echo npm run build")`
does the same with no file, no repo and no grep: it lowercases to a string containing
`npm run build`, carries no shell metacharacter, and exits 0 (`echo` is a `cmd` builtin, so
it works on Windows too — the plan's `grep` does not, see below). The *accidental* forms
matter more than the deliberate ones, because an agent does not have to be adversarial to
hit them: `rg "npm run build" .`, `git log --grep="npm run build"`,
`sed -i 's/npm run build/pnpm build/' package.json` all open the door while building nothing.

Note the asymmetry that makes this severe rather than theoretical: the same gate is
*aggressive* about false negatives (any pipe or `;` rejects, `build_evidence.rs:54-60`) and
*completely permissive* about false positives. An agent rejected twice for piping has a
strong gradient toward any plain command containing the magic substring.

### Remedy attack

No remedy is quoted. The obvious one — tighten `classify` to `starts_with` instead of
`contains` — is a **no-op against the exploit**: `bash("npm run build --help")` and
`bash("npm run build:nothing")` still pass, while `starts_with` breaks the legitimate
`cd sub && npm run build` and `env CI=1 npm run build` forms. Text matching cannot be
repaired by better text matching. The only fix that can falsify is artefact-based: record
the build's *output* (mtime of `dist/` or `target/debug/`, a `tsc` exit) or have the runner
invoke the build itself via `platform::command::resolve` and record from that invocation
rather than from anything the model typed. That last option is where `platform/command.rs`
genuinely belongs — see P8-26.

### Could not determine

- Whether `grep` is resolvable under `cmd /C` on a stock Windows install. On this host
  `Get-Command tail` → NOT FOUND and `cmd /c "echo hi | tail -50"` → 255, so the plan's
  chosen illustration may exit non-zero on Windows and be caught by the `ev.exit_code != 0`
  arm. The defect stands either way; only the illustration is platform-fragile.
