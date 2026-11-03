# 06 — Tools, permissions and sandboxing

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-5, INV-6, INV-9), §Part 1B (M-08, M-09, M-13, M-26), §Part 3.6, §Part 5 (S3, S4) |
| **Locks** | Hands **ADR-02-02**, **ADR-03-03**, **ADR-04-01** to `00 §Part 8` |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G10` (2,532 ln, **113 findings**) · `G2a §6` · `G9` (fleet tools) · `G3b` (path handling) |

---

## What this is

44 tool modules — the agent's entire capability to affect the world — and the permission model that is supposed to gate them.

**The headline finding is not that the permission check is missing. It is that a different check has been silently doing the job, and it is the wrong one.**

`Tool::requires_permission()` is never called anywhere (`G2a T47`). What actually gates auto-approval is **`is_read_only()`** — a flag whose own documentation describes it as a *concurrency* hint for the path lock (`tool_executor.rs:213,264`, `G10-111`). So the rule in force is: **any tool declaring itself read-only is never prompted, in any permission mode.** Two tools declare it wrongly, and both act on the world.

**The subtle failures this subsystem is prone to:**

1. **A flag repurposed as a security boundary.** `is_read_only` was written to answer "may this run concurrently?" and is answering "may this run unprompted?"
2. **A jail that four tools use and six bypass.** The barrier itself is sound; the coverage is not.
3. **A lock keyed on a string.** `C:\a\b.rs` and `c:\a\b.rs` take two different mutexes for one file.
4. **A gate satisfied by a substring.** `git commit -m "cargo build fixes"` records passing build evidence.

---

## 1. The permission model

### 1.1 What is actually in force

| Intended | Real |
|---|---|
| `Tool::requires_permission()` decides prompting | **Never called anywhere** |
| `PermissionMode` (Autonomous / Smart / Manual / ChatOnly) varies behaviour | `agent_commands.rs:142` passes `ManualApproval` **unconditionally** — the PA-mode dropdown changes nothing |
| A read-only tool is safe to run unprompted | `is_read_only()` is a **concurrency hint** (`G10-111`), and it is the auto-approval key |

Two tools declare `is_read_only = true` and are therefore never prompted, while doing this:

- **`analyze_diff`** — writes to the database *and* **uploads the full diff of any repository to a third-party LLM** (`G10-57`, `G10-56`). Under BYOK that is the user's own key and the user's own code leaving the machine, with no prompt.
- **`check_file`** — executes external checkers, including one it finds inside the workspace (§3.4).

### 1.2 The design

**One flag, one meaning, one gate.**

```rust
enum ToolEffect {
    Read,                      // observes; no side effect anywhere
    Write { paths: Vec<..> },  // mutates the workspace
    Execute,                   // runs a subprocess
    Network { host: String },  // leaves the machine
    Board,                     // mutates task/fleet state
}
```

- **`ToolEffect` decides prompting.** `is_read_only` reverts to what it was written for — concurrency — and never gates anything.
- **`PermissionMode` is honoured**, so the PA-mode dropdown means something.
- **`Network` is its own effect.** A tool that sends the user's code off the machine is not "read-only" under any reading, and BYOK makes that the user's own data and budget.
- The gate runs in **one place**, on the one execution path (M-08) — today `ToolExecutor::execute()`, the only fully-correct path, has **zero callers**.

---

## 2. The registration matrix

Full matrix in `G10 §1.2`. What the plan needs from it:

- Four registration entry points exist; **`graph_tools` and `graph_prompts` are in none of them** because their modules are never declared (spec `05 §1.1`).
- Delegate mode is documented as "zero file tools, only `create_task` + read-only MCP tools". `G10 §1.4` checks that against the code — any divergence is a product-promise defect, not a config detail.
- The registry is **frozen after construction** (`register(&mut self)`, then `Arc::new`), so there is no dynamic registration path. This is what makes ADR-05-06 an agent-core question rather than a feature.

---

## 3. The safety helpers

Everything else leans on these four. Verdicts from `G10 §2.8`.

### 3.1 `path_jail.rs` — sound barrier, incomplete coverage

**As an escape barrier it is correct**: `..`, symlinks, junctions, case, `\\?\`, and shared-prefix siblings are all handled properly for the four tools that call it (`write_file:70`, `edit_file:90`, `append_file:73`, `bash:62,287,296`).

**The failure is coverage.** Six tools bypass it entirely and are auto-approved: `read_file`, `grep`, `glob`, `list_directory` — **plus `check_file` and `analyze_diff`**, both of which *act* on the path rather than merely reading it. The sibling audit's list of four was an undercount.

**Two normalisation steps corrupt rather than reject** — the worse class, because rejection is visible and corruption is not:
- `..` in a non-existent tail is **silently deleted and the path rewritten** (`G10-87`, Linux/macOS).
- `\\?\UNC\…` is mangled into a relative `UNC\…` **and then returned as the write target** (`G10-88`).

**The rule: a path that cannot be normalised is refused, never rewritten.** And every tool that takes a path goes through the jail — reading a file outside the workspace is an exfiltration primitive, not a harmless read.

### 3.2 `path_lock.rs` — the key defeats the purpose

**The lock key is a raw path string.** `lock_key:77-84` normalises only `.` and `..`; it never canonicalises and never case-folds. So `C:\a\b.rs` and `c:\a\b.rs` are **two mutexes for one file** — on Windows *and* macOS. This is the same defect found in checkpoint retention (`G3b GI-28`) and the graph DB path key (spec `05 §2.2`); **three subsystems, one missing shared helper.**

It gets worse across callers: `write_file:83` locks the **pre-jail** path while `edit_file:108` locks the **canonical** one (`G10-09`), and the MCP adapter locks the **raw argument string** (`G10-92`). The module header's promise at `:40-41` that "PA-mode and Mode 2 callers contend on the same key" is therefore **false**.

**And the lock is held across an unbounded await**: `write_file:150` → `validate.rs:269` spawns `tsc` with no timeout, so **one hung child process is a permanent, process-wide write outage** (`G10-10`). That is INV-9 exactly.

No deadlock *cycle* is reachable (one lock per site, self-deadlock avoided by a name-matched exemption list) — but the adapter's own lock block is dead code: no registered tool satisfies all three of its conditions (`G10-93`).

### 3.3 `build_evidence.rs` — a hard gate satisfied by a substring

`classify()` is `str::contains` over the lowercased **raw command line**, with no tokenisation. It is honest in its own comments about being directional — and `update_task_status_tool.rs:141` uses it as a **hard gate** (`G10-96`).

**False positives** — evidence recorded for something that is not a build:
- `git commit -m "cargo build fixes"` → records passing Cargo/Node evidence
- `cat pytest.ini` → matches `pytest`
- `mongo test` → matches `go test`
- `npm install`, `npm ci`, `uv sync`, `pip install -r` → dependency installs counted as builds

**False negatives** — a real build not recorded:
- **No `make`, `cmake`, `gradle`, `mvn`, `dotnet` or `bazel` at all**
- `npm i`, `cargo nextest`, `python -m unittest`, `./build.sh` unmatched
- a build run through `check_file` records nothing
- a **timed-out** build records nothing (`G10-25`)
- `npm run dev` is documented as Node evidence and is absent from `node_hits`

**This is why M-13 moves verification into Rust.** A gate whose input is a fuzzy match over a string the model composed is not a gate. When Rust owns the argv, "did a build pass" stops being an inference and becomes an exit code.

### 3.4 `validate.rs` — advisory dressed as verification

Eight validators, four tiers, and:

- **Zero timeouts.** Every subprocess is `.output().await`; a grep for `timeout` across all 1,086 lines returns nothing (`G10-102`). Three of them run under a held path lock (§3.2).
- **A missing binary produces a *warning*, and `is_clean():35-37` ignores warnings** — so `check_file.rs:123` prints **"✓ No syntax errors" for a file that was never checked.** On Windows this is the *default* state, because `Command::new("tsc")` cannot resolve npm's `.cmd` shim (`G10-16`).
- **`resolve_python:880-915` runs `<workspace>/.venv/Scripts/python.exe`** — a binary the **workspace ships** — from a tool that is read-only, unjailed and auto-approved (`G10-86`). Cloning a hostile repository and opening a `.py` file is enough.
- **Nothing blocks on any validator result.** Every one is advisory text appended to an already-successful write.

**The rules:** every subprocess has a timeout · a missing tool is an **error**, never a silent pass · executables are resolved from a trusted location, never from the workspace under inspection · and if a validator is advisory, it must not be phrased as a verdict.

---

## 4. Tool descriptions are a defect surface

**33 of 38 tool descriptions are defective. Five are clean** (`read_file`, `get_epic_progress`, `get_ready_tasks_in_phase`, `suggest_worker`, `search_memory`). Six module doc-comments are additionally false (`G10 §5.3`, `§5.4`).

The `description()` string is the model's *only* guide to a tool. A wrong one does not crash — it produces confidently wrong behaviour, which is far harder to trace back. It is a code defect and it belongs in the register, not in a style guide.

**The rule: a tool's description is part of its contract, and it is reviewed and tested like one** (`G-TOOLDESC`).

---

## 5. Cross-cutting

**Three identical byte-slice panics** — `bash.rs:134-141`, `remember_this.rs:94`, `annotate_file_tool.rs:99` — truncating on a byte index inside a UTF-8 character. **The correct implementation already exists in the same codebase** at `validate.rs:846-853`. One shared helper, three sites.

**`e2e_probe`** (`G10-73`, CRITICAL) executes arbitrary shell with the jail, the redirect scanner, evidence recording and output capture **all absent**, while declaring `requires_permission() = false`.

**`spawn_qc_review`** (`G10-67`) records a $0.10 provisional charge **before eight distinct error returns** and never reconciles it. 50 dispatcher ticks against an already-claimed task trip the $5 hourly ceiling and **block all QC for an hour at zero real spend** — a self-inflicted denial of service on the fleet's own review capacity.

**`write_interface_spec`** (`G10-79/80`) destroys `INTERFACE.md` in default mode with no checkpoint, and its `merge` is an unlocked read-modify-write that degrades to overwrite on a read error.

**Output caps** and their behaviour at the cap, plus the full cross-platform table, are in `G10 §3` and `§6`.

---

## 6. Determinism balance

Entirely `DET`. Tools execute effects and return results; **no tool makes a model call except `analyze_diff`**, which is why its unprompted, unjailed, network-egress status is the sharpest finding in this spec.

---

## 7. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-PERM-GATE** | Every tool passes exactly one permission check, keyed on `ToolEffect`; `is_read_only` gates nothing | G10-111 |
| **G-PERM-MODE** | Each `PermissionMode` measurably changes prompting behaviour | The unconditional `ManualApproval` |
| **G-PERM-NETWORK** | ⚠ **CORRECTED 2026-08-31.** ~~Any tool that sends workspace content off the machine prompts, in every mode~~ — that collided with spec `04`'s `G-QC-ANALYZE`, which requires `analyze_diff` to run in **unattended** QC. Two decided specs, opposite requirements. **Resolution: consent is a property of the workspace + destination, not of each call.** No workspace content leaves the machine without a **recorded, revocable consent** for that workspace and that provider; unattended work runs under that standing consent, and its absence blocks egress rather than prompting a machine nobody is watching | `analyze_diff`; and the contradiction the review caught |
| **G-JAIL-COVERAGE** | Static check: every tool taking a path calls the jail | The six bypassing tools |
| **G-JAIL-REFUSE** | An unnormalisable path is **refused**, never rewritten; property-tested over `..`, symlinks, junctions, UNC, case | G10-87, G10-88 |
| **G-LOCK-KEY** | All spellings of one path take the **same** mutex, across PA and MCP callers | G10-09, G10-92 |
| **G-LOCK-BOUND** | No lock is held across an unbounded await; a hung child cannot block other writes | G10-10 |
| **G-SUBPROC-TIMEOUT** | Every spawned process has a timeout and is killed on drop | G10-102 |
| **G-VALIDATOR-HONEST** | A missing validator binary is an **error**; "no syntax errors" is never printed for an unchecked file | G10-16 |
| **G-NO-WORKSPACE-EXEC** | No executable is resolved from the workspace under inspection | G10-86 |
| **G-EVIDENCE-EXACT** | Build evidence comes from a Rust-executed command's exit code; a command that merely *mentions* a build records nothing | G10-96, and the whole of §3.3 |
| **G-UTF8-TRUNC** | Truncation is char-boundary-safe at every site | The three panics |
| **G-TOOLDESC** | Every description states what the tool does, its constraints, and nothing it does not do | The 33 defective descriptions |
| **G-PROVISIONAL-COST** | A provisional charge is reconciled or released on **every** exit path | G10-67 |

---

> No ADR decision is recorded in this spec. It hands **ADR-02-02**, **ADR-03-03** and **ADR-04-01** to `00 §Part 8`.
