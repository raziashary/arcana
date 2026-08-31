# G2a — Bug Audit: Agent Engine, Context, Compaction, Tools

**Status:** COMPLETE — 57 findings (3 CRITICAL, 18 HIGH, 23 MEDIUM, 13 LOW)
**Auditor:** read-only pass over `src-tauri/src/agent/**` + traced consumers
**Date:** 2026-08-31
**Root:** `C:\AIRS Foundry\autodev\autodev`

All paths below are relative to `C:\AIRS Foundry\autodev\autodev\src-tauri\src\` unless stated.

---

## 1. Findings table

| ID | file:line | Sev | Class | What the code does | Why it is wrong | Concrete failure scenario | Status |
|----|-----------|-----|-------|--------------------|-----------------|---------------------------|--------|
| **T1** | `provider/types.rs:49-59`, `provider/anthropic.rs:115-141` | CRITICAL | cost / missing feature | `CompletionRequest` has no cache field; `build_body` emits no `cache_control`; no `anthropic-beta` header. Zero hits for `cache_control`/`cache_creation`/`prompt_cache` tree-wide | No prompt caching on any provider. Every turn re-bills the full system prompt + 38 tool schemas + entire history at full input rate | A 20-turn session bills the ~10 KB system prompt and 38 tool schemas 20×. Typical 5-10x overspend | CONFIRMED |
| **T2** | `agent/engine.rs:196` | CRITICAL | cost / design | `messages: history.clone()` every turn, 2 messages appended per turn (`engine.rs:268-271`, `328-331`), no per-result trimming | Quadratic input cost: turn *k* re-bills turns 1..k-1's tool outputs | 20 turns × 30 KB avg tool output ≈ 200 message-copies ≈ millions of billed input tokens for one user message | CONFIRMED |
| **T3** | `agent/engine.rs:174` | HIGH | hardcoded value | `let model_context = 200_000u32;` — never reads `ModelInfo::context_window` (exists at `provider/anthropic.rs:40`, and in `openai.rs`/`google.rs`/`ollama.rs`) | Wrong for every non-Anthropic model | On a 128 K OpenAI model or a 32 K Ollama model the request overflows and 400s; compaction never fires because 160 K is never reached | CONFIRMED |
| **T4** | `agent/compaction.rs:13,26-28,31-41` + `engine.rs:175-178` | HIGH | inert guard | Compaction fires only when `sum(json_len)/4 > 160_000`, i.e. ~640 KB of history, inside a ≤20-turn loop | Threshold far above what the 20-turn ceiling normally reaches | Sessions run to `MAX_TOOL_TURNS` at full context cost and compaction never runs once | CONFIRMED |
| **T5** | `commands/agent_commands.rs:166-196` vs `agent/engine.rs:402` | HIGH | discarded work | `run()` returns compacted `history`; the caller reads only `updated_history.last()` and saves its text | Compaction output is thrown away; it can never help across user messages | Long session: every user message rebuilds history from DB, so compaction restarts from zero each time | CONFIRMED |
| **T6** | `commands/agent_commands.rs:249` | HIGH | correctness / cost | `MessageRole::Tool => return None` — tool messages dropped when rebuilding history | The model loses every tool observation between user messages | User asks a follow-up; the agent has no memory of the files it read and re-reads them all, paying again | CONFIRMED |
| **T7** | `commands/agent_commands.rs:169-177` | HIGH | data loss | Saves only `updated_history.last()`, and only if `role == "assistant"`; assigns (not appends) each text block | When the loop exits via `MAX_TOOL_TURNS` (`engine.rs:168-171`) or cancel (`engine.rs:188-191`), the last message is the tool-results **user** message → **nothing is saved** | Agent streams a full answer to the UI; on reload the assistant reply is gone from the DB | CONFIRMED |
| **T8** | `agent/context.rs:117-121` | MEDIUM | cache-hostile | Volatile memory block appended to the **end** of the system prompt | Even if caching were added, the system-prompt tail changes every run; the volatile part must go into the first user message instead | Adding `cache_control` naively at the end of `system` would yield ~0% cache hits | CONFIRMED |
| **T9** | `agent/context.rs:86,101` | LOW | efficiency | `AutodevInstructionsDb::open()` called twice per run | Two SQLite opens where one suffices | Minor per-message latency | CONFIRMED |
| **T10** | `agent/engine.rs:198` | MEDIUM | hardcoded value | `max_tokens: 8192` for every model | Some models cap lower (400 error), some allow far more | Ollama model with 4096 max → hard 400 on every request | CONFIRMED |
| **T11** | `agent/builtin_tools/list_directory.rs:98,108-141` | HIGH | unbounded output | `if out.len() >= max_entries { return; }` is checked only on **entry** to `collect_entries`, never inside the `for entry in dir_entries` loop | `max_entries` does not bound a single directory; `truncated = entries.len() == max_entries` (line 61) uses `==` so it reads **false** on overflow | `list_directory` on a 20 000-file directory returns all 20 000 entries as JSON into history and reports `truncated:false` | CONFIRMED |
| **T12** | `agent/builtin_tools/read_file.rs:196-199` | HIGH | security | `resolve_path` has **no path jail** — any absolute path accepted. `is_read_only()==true`, so auto-approved with no prompt in every mode (`tool_executor.rs:264`) | `write_file`/`edit_file`/`append_file`/`bash` all call `validate_workspace_path`; `read_file` does not | A prompt-injected instruction in a repo file makes the agent `read_file("C:/Users/x/.aws/credentials")` unprompted, and the contents land in the LLM request | CONFIRMED |
| **T13** | `agent/builtin_tools/grep.rs:119-124`, `glob.rs:100-105`, `list_directory.rs:45-49` | HIGH | security | Same: absolute `path` accepted, no `validate_workspace_path`, all read-only and `requires_permission()==false` | The read side of the workspace jail does not exist | `grep(pattern="BEGIN PRIVATE KEY", path="C:/Users")` runs unprompted | CONFIRMED |
| **T14** | `agent/builtin_tools/edit_file.rs:149-156` | HIGH | hang / OOM | `while let Some(pos) = content[start..].find(&params.old_string) { … start += pos + params.old_string.len(); }` | With `old_string == ""`, `find("")` returns `Some(0)` and `start` never advances → **infinite loop** pushing into `positions` | LLM emits `edit_file(path, old_string="", new_string="x")`; `matches("")` > 1 so the branch is taken; the tool never returns, the agent hangs, RAM grows until OOM | CONFIRMED |
| **T15** | `agent/builtin_tools/edit_file.rs:164-168` | MEDIUM | corruption | `replace_all=true` with `old_string == ""` → `content.replace("", new)` | Inserts `new_string` between **every character** of the file | Silent whole-file corruption reported as success `"✓ Edited …"` | CONFIRMED |
| **T16** | `agent/builtin_tools/bash.rs:404-437` vs `bash.rs:89-92` | HIGH | portability | `rewrite_for_windows` rewrites Unix pipes into **PowerShell** cmdlets (`Select-Object`, `Select-String`), then the command runs under `cmd /C` | `cmd.exe` does not know `Select-Object`/`Select-String`. The tool description at `bash.rs:23` correctly says "The shell is Windows CMD" — the rewriter targets a different shell | `ls \| head -20` → `ls \| Select-Object -First 20` → cmd: `'Select-Object' is not recognized`. The rewrite makes the failure more confusing, not less | CONFIRMED |
| **T17** | `agent/builtin_tools/bash.rs:432-433` | MEDIUM | dead code / ordering | `replace(" \| grep ", " \| Select-String ")` runs **before** `replace(" \| grep -i ", …)` | The first replace consumes the substring, so the `-i` arm is unreachable | `grep -i` never gets its intended `-Pattern` rewrite | CONFIRMED |
| **T18** | `agent/builtin_tools/bash.rs:131-136` | HIGH | panic | `let tail_start = output.len() - MAX_OUTPUT_BYTES; output[tail_start..]` — byte-index slice of a `String` | Panics if `tail_start` is not a UTF-8 char boundary | A >50 KB build log containing a multi-byte char (cargo's `→`, box drawing, an accented path) at the cut point panics the tool task; the tokio task aborts, no `agent:done` fires, the UI hangs "running" forever | CONFIRMED |
| **T19** | `agent/builtin_tools/bash.rs:387` | MEDIUM | corruption | `out.push(bytes[i] as char)` in `strip_heredoc_bodies` | `u8 as char` is a Latin-1 cast; every multi-byte UTF-8 sequence is mangled | A heredoc containing non-ASCII yields a mojibake rescan string; the redirect jail then scans corrupted text | CONFIRMED |
| **T20** | `agent/builtin_tools/path_jail.rs:110-111` | MEDIUM | panic | Windows arm: `let t_prefix = if t.len() >= w.len() { &t[..w.len()] }` — byte slice of a lowercased `String` | Panics when byte offset `w.len()` falls inside a multi-byte char in `t` | A workspace or target path containing non-ASCII (Cyrillic/accented folder) can panic the jail check inside every write tool | CONFIRMED |
| **T21** | `agent/tool_executor.rs:220-221, 285-302` | HIGH | protocol violation | Results vector pre-filled with `(String::new(), ToolResult::error("placeholder"))`; on a read-tool task panic the `Err` arm only logs (`"Placeholder will be replaced; we need to find which one failed"`) and leaves the placeholder | The placeholder has an **empty `tool_use_id`**, later emitted as a `tool_result` block (`engine.rs:314-325`) | One panicking read tool ⇒ Anthropic `400 tool_use_id … not found`, killing the turn with an opaque provider error | CONFIRMED |
| **T22** | `agent/tool_executor.rs:243-279` | MEDIUM | missing behaviour | The concurrent read-only path fires **no** `PreToolUse`/`PostToolUse`/`OnToolError` hooks, unlike `execute()` (`:119-179`) and the serial write path (`:344-406`) | A `PreToolUse` hook cannot block a read tool; `PostToolUse` consumers never see reads | A hook installed to audit or veto `read_file` is silently bypassed for every read | CONFIRMED |
| **T23** | `agent/tool_executor.rs:33-34, 452` | MEDIUM | docstring vs code | Comment says "Tools auto-approved for this session (AllowAlways responses)", but `ToolExecutor` is constructed inside `run()` (`engine.rs:111-121`) — **fresh per user message** | "Allow always" lasts one user message, not the session | User clicks "Always allow" for `bash`; the very next message prompts again | CONFIRMED |
| **T24** | `agent/tool_executor.rs:110-115` | MEDIUM | stub | `PermissionMode::SmartApprove` arm is `// Phase 2: smart approval logic / For now, auto-approve read-only, prompt otherwise` — identical to `ManualApproval` | A named mode that does not do what it claims | User selects Smart Approve expecting pattern-based auto-approval; gets manual prompts | CONFIRMED |
| **T25** | `agent/tool_executor.rs:448-457` | MEDIUM | missing timeout | `rx.await` on the permission oneshot has **no timeout** | If the UI never answers (window closed, event lost, stale call_id — see `agent_commands.rs:235-237`), the agent task blocks forever | Agent stuck "running"; the `Semaphore` permit and path locks stay held | CONFIRMED |
| **T26** | `agent/tool_executor.rs:239-241` | LOW | dead code | `let _mode`, `let _always_allow`, `let _pending` cloned per read tool, never used | Pointless clones (a `HashSet` clone per tool call) | Minor allocation churn | CONFIRMED |
| **T27** | `cost/tracker.rs:54-55` | MEDIUM | wrong comparison order | `m if m.contains("gpt-4o") => (2.5, 10.0)` precedes `m if m.contains("gpt-4o-mini") => (0.15, 0.6)` | The `gpt-4o-mini` arm is unreachable — `"gpt-4o-mini"` contains `"gpt-4o"` | `gpt-4o-mini` billed at ~16x its real input price in the cost tracker, budget breaker, and analytics | CONFIRMED |
| **T28** | `cost/tracker.rs:57` | MEDIUM | silent default | Unknown models fall through to `(1.0, 3.0) // Safe default` with no warning | Claude 4.6/Opus-4.6, gpt-4.1, gemini-2.5 and every Ollama model are silently mispriced | The Budget Circuit Breaker computes `cost_usd` from this (`engine.rs:219`) and halts — or fails to halt — on fiction | CONFIRMED |
| **T29** | `agent/engine.rs:364-372` | LOW | duplicated inference | Provider re-derived from the model string for analytics although the real provider id was known at `agent_commands.rs:33`; a third copy at `commands/worker_commands.rs:318-338` | Ollama/xAI models record as `"unknown"`; three copies drift | Analytics dashboard under-reports non-Anthropic usage | CONFIRMED |
| **T30** | `commands/settings_commands.rs:86-96`, `state/app_state.rs:14-42`, `main.rs:112` | HIGH | no persistence | `set_active_provider` writes only to in-memory `AppState`; nothing hydrates it at startup | The chosen model is lost on every app restart | After restart, `send_message` returns `"No active model set"` until the frontend re-pushes it | CONFIRMED |
| **T31** | `commands/model_config_commands.rs:12-42` vs `engine.rs:199-202`, `compaction.rs:117-120`, `recipe_commands.rs:248-249`, `diff_analysis/classifier.rs:140-141` | HIGH | writer with no reader | `ModelConfigsDb` persists temperature / top_p / reasoning_effort / thinking; **no code outside `model_config_commands.rs` ever reads it**; every `CompletionRequest` hardcodes those fields to `None` | The entire per-model settings UI is inert | User enables extended thinking or sets reasoning_effort=high; nothing changes in any request | CONFIRMED |
| **T32** | `agent/builtin_tools/graph_tools.rs`, `graph_prompts.rs` | CRITICAL | dead code | Neither file has a `mod` declaration anywhere in `src-tauri` (`agent/builtin_tools/mod.rs:6-72` lists 44 modules, neither present; tree-wide grep for `mod graph_tools` → no match) | 918 lines never compiled, never registered | The agent has **no** graph access at all — the one bridge from the code graph to the agent is not in the binary | CONFIRMED |
| **T33** | `agent/builtin_tools/glob.rs:56` | LOW | docstring lie | Description claims "Automatically respects .gitignore" | The body uses `WalkDir` + a hardcoded `skip_dirs` array (`glob.rs:115`) and never reads `.gitignore` | Agent trusts the claim, globs `**/*` and gets ignored build artifacts | CONFIRMED |
| **T34** | `agent/context.rs:177` (BASE_SYSTEM_PROMPT) vs `agent/builtin_tools/grep.rs` | LOW | docstring lie | The system prompt tells the model "**grep(...)** — Search files using ripgrep" | The implementation is a hand-rolled `WalkDir` + `regex`-crate walk. No ripgrep, no `.gitignore`, and the `regex` crate rejects lookahead/backreferences | Agent writes a PCRE-style pattern and gets `Invalid regex pattern` | CONFIRMED |
| **T35** | `agent/builtin_tools/grep.rs:64,192-195` | LOW | docstring vs code / unbounded | Description claims it "Skips binary files … automatically" | No binary detection; it relies on `read_to_string` failing on invalid UTF-8. A UTF-8-valid blob is searched, and there is **no per-line length cap** | A minified bundle returns a multi-MB single "line" straight into history | CONFIRMED |
| **T36** | `agent/builtin_tools/append_file.rs` (whole file) | MEDIUM | inconsistent behaviour | `append_file` does **not** call `crate::git::checkpoint::auto_checkpoint_before_write` and does **not** fire `PostFileWrite`, unlike `write_file.rs:115-118,122-145` and `edit_file.rs:173-177,183-208` | Appends are not undoable and are invisible to hook consumers | User hits "undo"; the appended change is in no checkpoint | CONFIRMED |
| **T37** | `agent/builtin_tools/path_lock.rs:26-30` | LOW | unbounded growth | "Entries are not GC'd. One entry per path the fleet has ever touched." | Documented, but still an unbounded process-lifetime `HashMap` | A long-lived app touching many files grows monotonically | CONFIRMED (documented) |
| **T38** | `agent/engine.rs:34,168-171` | MEDIUM | hardcoded value / silent stop | `MAX_TOOL_TURNS: usize = 20`, no config surface; the exit path `break`s with only an `error!` log | Non-configurable, and the user is never told the agent stopped early | Complex task halts at turn 20; the UI shows a truncated answer with no explanation (and per T7 nothing is persisted) | CONFIRMED |
| **T39** | `provider/anthropic.rs:31` | MEDIUM | missing timeout / no retry | `client: Client::new()` — reqwest default has **no request timeout**; no retry on 429/5xx anywhere in the provider layer | A hung connection blocks the turn indefinitely; a transient 429 kills the whole run (`engine.rs:253-258`) | Rate-limited mid-session → the run aborts and (per T7) nothing is saved | CONFIRMED |
| **T40** | `agent/engine.rs:217-230` vs `commands/agent_commands.rs:140-147` | MEDIUM | guard never armed | The Budget Circuit Breaker engages only when `with_task_budget()` was called; the interactive path never calls it | Interactive PA sessions have **no** token, cost, or spend ceiling | The very sessions the owner reports ballooning are the ones with the breaker disabled | CONFIRMED |
| **T41** | `agent/engine.rs:149-166` | LOW | off-by-one | `budget_status.turns_used += 1` runs **before** the `turn >= MAX_TOOL_TURNS` check and before any tokens are spent | `max_turns` is enforced one turn early relative to actual LLM calls | A budget of `max_turns=1` halts before making any request | CONFIRMED |
| **T42** | `agent/engine.rs:467-471` | MEDIUM | silent failure | `serde_json::from_str(&json_str).unwrap_or(Value::Object(Map::new()))` — malformed/truncated streamed tool-input JSON silently becomes `{}` | The tool then runs with **no arguments** and reports "Missing required field" instead of "tool input was truncated" | Long tool input split across SSE chunks parses to `{}`; the agent sees a bogus validation error and retries the same call | CONFIRMED |
| **T43** | `agent/engine.rs:464-471` | MEDIUM | nondeterminism | Content blocks rebuilt by iterating `tool_use_map: HashMap`; text is always pushed first regardless of its real position | HashMap iteration order is randomized, so multiple `tool_use` blocks are emitted in arbitrary order | Assistant messages replayed to the provider have a different block order than the model produced; also defeats any future prefix caching of the assistant turn | CONFIRMED |
| **T44** | `agent/context.rs:34-66` | LOW | dead code | `build_system_prompt()` has **zero callers** (verified: only `build_system_prompt_with_mode` at `engine.rs:101` and `default_system_prompt` at `settings_commands.rs:136` are used) | Two divergent prompt builders; the legacy one reads a different global path and skips the DB | Maintenance trap — a fix applied to one is missed in the other | CONFIRMED |
| **T45** | `agent/context.rs:171` | LOW | prompt contradicts the product | BASE_SYSTEM_PROMPT instructs: "**CLAUDE.md files**: if a `CLAUDE.md` exists … read it at the start of every session" | `context.rs:62-63` explicitly states "REMOVED: AGENTS.md fallback — AutoDev is AUTODEV.md only". The prompt sends the model hunting for a deliberately abandoned convention | Wasted `read_file`/`glob` turns; tokens spent every turn carrying the instruction | CONFIRMED |
| **T46** | `agent/context.rs:162` vs `227-231` | LOW | self-contradicting prompt | Rule 8: "**Lock files are sacred** — Write `current_tasks/issue-N.lock` …"; 65 lines later: "Do NOT write filesystem lockfiles (`current_tasks/*.lock`)" | Two directly contradictory instructions in one system prompt | Model behaviour on locking is a coin flip; the contradiction is re-billed every turn | CONFIRMED |
| **T47** | `agent/tool_trait.rs:61` | MEDIUM | dead trait method | `fn requires_permission(&self) -> bool { !self.is_read_only() }` is overridden by 8 tools but **never called** — `tool_executor.rs` branches only on `is_read_only()` (`:104,113,213,331,338`) | Tools that deliberately opt out of prompting (`e2e_probe.rs:89`, `write_interface_spec.rs:98`, both `requires_permission()->false` yet `is_read_only()->false`) still prompt; the whole method is decoration | A recipe worker in ManualApproval mode stalls on a permission prompt for `e2e_probe`, which was explicitly written to not need one | CONFIRMED |
| **T48** | `agent/tool_executor.rs:88,251,315` | HIGH | inert guard | `ToolContext.depth` is hardcoded to `0` at all three construction sites; nothing ever propagates a real depth on the native path | `spawn_subagent.rs:84-90`, `spawn_parallel_subagents.rs:85-92`, `spawn_qc_review.rs:164` all guard on `ctx.depth >= MAX_SUBAGENT_DEPTH (3)`, so on the `AgentEngine` path the check is **always `0 >= 3` = false**. `agent/builtin_tools/mod.rs:92-95` explicitly calls this "the safety net" | A subagent chain driven through `ToolExecutor` never trips the recursion guard; `tool_trait.rs:41-44` documents propagation that does not exist | CONFIRMED (guard inert); recursion depth in practice depends on the orchestrator path — SUSPECTED unbounded |
| **T49** | `agent/tool_executor.rs:89,252,316` | MEDIUM | inert field | `ToolContext.agent_role` is hardcoded to `None` at all three construction sites | Three ACP tools registered in PA mode (`mod.rs:230-234`, comment: "so Razi can manually inspect / send worker messages from the chat tab") refuse exactly that caller: `send_worker_message.rs:129-137`, `list_worker_messages.rs:60-66`, `acknowledge_worker_message.rs:48-55` all return an error when `agent_role` is `None` | Three tool schemas shipped in every request (token cost) for tools that can only ever return "refused" on this path | CONFIRMED |
| **T50** | `agent/builtin_tools/update_task_status_tool.rs:133` | MEDIUM | guard bypassed | The build-evidence gate on `qc_review` transitions is conditioned on `ctx.agent_role.is_some()` | On the native `AgentEngine` path `agent_role` is always `None` (T49), so the gate — added specifically because "the 14 May calendar run shipped 16 integration defects to qc_review" (`:117-119`) — never fires there | A PA/Mode-1 agent moves a task to `qc_review` with no build ever run; the exemption comment calls this deliberate for "raw tool invocation", but it swallows the entire Mode-1 surface | CONFIRMED |
| **T51** | `agent/builtin_tools/analyze_diff_tool.rs:219-222` | HIGH | ignores model selection | Provider and model are read from env vars `AUTODEV_ACTIVE_PROVIDER` / `AUTODEV_ACTIVE_MODEL`, falling back to `"anthropic"` / `"claude-sonnet-4-6"` | A repo-wide grep shows those env vars are **read here and set nowhere** (Rust or TS). So `analyze_diff` always uses hardcoded anthropic + `claude-sonnet-4-6` — an id present in **no** provider catalogue (`provider/anthropic.rs:38,47,56` list `claude-opus-4-5` / `claude-sonnet-4-5` / `claude-haiku-4-5`) | Changing the model in the UI has zero effect on `analyze_diff`; the call is likely rejected by the API as an unknown model | CONFIRMED |
| **T52** | `agent/builtin_tools/analyze_diff_tool.rs:55` | MEDIUM | mislabelled | `is_read_only() -> true` on a tool that makes a **billed LLM call** (`build_analyzer_from_vault` → `DiffAnalyzer`) | Read-only tools are auto-approved with no prompt and run up to 10-at-a-time concurrently (`tool_executor.rs:231,264`) | The model can fire 10 concurrent paid LLM calls with no permission prompt and no budget accounting in the parent's token counters | CONFIRMED |
| **T53** | `agent/builtin_tools/validate.rs:186,221,269,753,816` | HIGH | missing timeout | Five `tokio::process::Command … .output().await` calls with **no timeout**, invoked from `write_file.rs:151,161,172,185` and `edit_file.rs:215` on **every** write | A hung `tsc` / `node` / `pyright` / `py_compile` blocks the tool forever while holding the per-path lock (`path_lock::acquire_for`) | One stuck type-checker deadlocks every subsequent write to that file across the whole process | CONFIRMED |
| **T54** | `agent/builtin_tools/validate.rs:269,753` | HIGH | portability | `Command::new("tsc")` and `Command::new("pyright")` — bare names | On Windows these are npm/pip `.cmd` shims. Rust's `Command` uses `CreateProcess`, which appends only `.exe`, not `PATHEXT`, so the spawn fails even when the tool **is** installed | The `Err(_)` arm (`validate.rs:275-286`) then emits `"Could not run 'tsc' — TypeScript not installed. Install it (npm install -g typescript)"` — a **false diagnostic** shown to the agent on every `.ts`/`.tsx` write on Windows | CONFIRMED |
| **T55** | `agent/builtin_tools/e2e_probe.rs:179-197` | MEDIUM | portability | Spawns start commands via `cmd /C <cmd>` on Windows with `kill_on_drop(true)` | On Windows, killing the `cmd.exe` wrapper does **not** kill its grandchildren (`npm`, `node`, `uvicorn`). There is no job-object or process-group teardown | Every `e2e_probe` run leaks an orphaned dev server holding its port; the next probe then fails on "address in use" | CONFIRMED |
| **T56** | `agent/builtin_tools/list_worker_messages.rs:46-55` | LOW | docstring vs code | The comment reasons at length about marking the tool read-only, then concludes "Erring on the cautious side: mark as not-read-only" and returns `false` | The comment's premise ("the side effect is metadata-only") and its conclusion disagree with each other mid-paragraph; harmless but confusing | Reader-facing only | CONFIRMED |
| **T57** | `agent/builtin_tools/grep.rs:146` vs `list_directory.rs:84-87` vs `glob.rs:115` | LOW | inconsistency | Three different skip-directory lists: grep/glob use `["node_modules",".git","target","dist","build",".next",".cache"]`; list_directory uses `[".git","node_modules","target","__pycache__",".venv","venv","dist","build",".airs"]` | `grep` and `glob` walk into `.venv`, `venv`, and `__pycache__`; `list_directory` walks into `.next` and `.cache` | `grep` on a Python repo searches the whole virtualenv, returning site-packages hits and burning the 100-match cap on noise | CONFIRMED |

---

## 2. BUG-TOKENS — traced mechanism

### Q1. Context assembly — what goes into the prompt each turn?

Assembly happens in two places: `agent/engine.rs::run()` (per user message) and
`agent/context.rs::build_system_prompt_with_mode()` (system prompt only).

Per **user message** (`agent/engine.rs:96-121`):
1. `ContextBuilder::new(working_dir).with_session(session_id)` → system prompt built **once**, before the loop (`engine.rs:98-102`).
2. `tool_defs = self.registry.to_definitions()` → built **once** (`engine.rs:103`).
3. `history` is the caller-supplied prior conversation; the new user message is pushed onto it (`engine.rs:106-109`).

Per **turn inside the loop** (`agent/engine.rs:193-203`):
```rust
let request = CompletionRequest {
    model:    model.to_string(),
    messages: history.clone(),          // ← FULL history, every turn
    system:   Some(system_prompt.clone()),
    tools:    tool_defs.clone(),
    max_tokens: 8192,
    temperature: None, top_p: None,
    reasoning_effort: None, thinking_budget_tokens: None,
};
```
**Yes — the entire conversation is re-sent verbatim on every turn.** `history` grows by two messages per turn: the assistant message (`engine.rs:268-271`) and a user message carrying **all tool results** (`engine.rs:328-331`).

Order of the system prompt (`agent/context.rs:72-124`), joined with `"\n\n---\n\n"`:
1. `BASE_SYSTEM_PROMPT` / `REVIEW_SYSTEM_PROMPT` / `DELEGATE_SYSTEM_PROMPT` (`context.rs:77-81`) — the base prompt is ~240 lines / ~10 KB (`context.rs:139-241`).
2. Global AUTODEV.md — SQLite `autodev_instructions` table, falling back to `~/.autodev/config/autodev_global.md` (`context.rs:86-99`).
3. Repo AUTODEV.md — SQLite, falling back to `<repo>/.autodev/autodev.md` (`context.rs:101-113`).
4. **Memory block** — `crate::memory::build_memory_context(sid, "", repo_path)` (`context.rs:117-121`).

**Full file contents ARE retained in history.** A `read_file` result becomes a `ToolResult` content block appended to history at `engine.rs:314-331` and is never trimmed, summarised, or replaced. There is **no per-result truncation in the engine** — `tool_executor.rs` and `engine.rs` pass `result.output` straight through, only appending a hint (`engine.rs:315-319`). All size limiting lives inside individual tools (see Q6).

### Q2. Compaction — does it fire?

**Trigger** (`agent/engine.rs:173-185`):
```rust
let model_context = 200_000u32;                       // engine.rs:174  HARDCODED
if CompactionEngine::needs_compaction(
    CompactionEngine::estimate_tokens(&history),      // history ONLY
    model_context,
) { ... }
```
**Threshold** (`agent/compaction.rs:13, 26-28`):
```rust
const COMPACTION_THRESHOLD: f32 = 0.80;
estimated_tokens as f32 / context_window as f32 > COMPACTION_THRESHOLD
```
So the comparison is `estimate_tokens(history) > 160_000`. `estimate_tokens` is
`sum(serde_json::to_string(block).len()) / 4` (`compaction.rs:31-41`) — i.e.
**~640,000 characters of JSON-serialised history** must accumulate inside a
single `run()` call before compaction fires.

**Is that reachable?** Only marginally, and the ceiling fights it:
- `MAX_TOOL_TURNS = 20` (`engine.rs:34`) caps the loop at 20 tool turns.
- To hit 640 KB in ≤20 turns you need ~32 KB of tool output per turn *sustained*.
  `bash` caps at 50 KB (`builtin_tools/bash.rs:13`) and `read_file` at 256 KB
  (`builtin_tools/read_file.rs:102`), so it **is** reachable with a few large
  reads — but the common case (grep hits, small edits) never reaches it. The
  loop hits `MAX_TOOL_TURNS` and exits long before compaction triggers.
- **The 200 000 constant is wrong for non-Anthropic models.** `provider/anthropic.rs:40,49,58`
  declare `context_window: 200_000` in `ModelInfo`, and `provider/openai.rs` /
  `provider/google.rs` / `provider/ollama.rs` declare their own — but
  `engine.rs:174` **never reads `ModelInfo`**. On a 128 K OpenAI model or a
  small Ollama model, the request overflows and the provider 400s *before*
  compaction is ever considered. CONFIRMED.

**What it does to history** (`compaction.rs:78-153`): keeps `messages[0]`, makes
a **separate full-context LLM call** on `messages[1..len-10]` to summarise
(`compaction.rs:111-124`), inserts a synthetic assistant summary, and keeps the
last `KEEP_RECENT_MESSAGES = 10`. On LLM failure it falls back to
`compact_truncate` (`compaction.rs:130-133`).

**Is the compacted result used?** *Within* the run, yes — `history` is reassigned
(`engine.rs:180`). **Across user messages, NO.** `engine.rs:402` returns the
compacted history, but the only consumer
(`commands/agent_commands.rs:166-196`) reads **only `updated_history.last()`**
and, if it is an assistant message, saves its text. The compacted structure is
thrown away. On the next user message, history is rebuilt from scratch by
`build_history()` (`commands/agent_commands.rs:243-256`). **Compaction is
per-run-only and its output is discarded.** CONFIRMED.

Two further consequences of `build_history()`:
```rust
MessageRole::Tool => return None,   // agent_commands.rs:249 — tool msgs DROPPED
```
- Tool results never survive into the next user turn. The model loses all its
  tool observations between user messages — a *correctness* defect that also
  causes re-reading the same files (which costs tokens again).
- Assistant messages are stored as flattened text only
  (`agent_commands.rs:171-177`), so `tool_use` blocks are lost too, and only the
  **last** text block survives (`assistant_msg.content = text.clone()` is an
  assignment, not an append).

### Q3. Prompt caching — **ABSENT. Zero breakpoints anywhere.**

```
grep -rniE "cache_control|cache_creation|ephemeral|prompt_cache|cache_read" src-tauri/src/
```
returns **no hit for `cache_control`, `cache_creation`, `prompt_cache`, or
`cache_read` anywhere in the tree.** Every `ephemeral` hit is the unrelated
"ephemeral subagent" lifecycle feature. The single caching-adjacent token is
`cached_tokens` in `sidecar/client.rs:42,899` — a passthrough field in the
Python-sidecar usage dict, not used by the Rust agent path.

Structural proof it cannot be set: `provider/types.rs:49-59` — `CompletionRequest`
has **no field** for cache control, and `provider/anthropic.rs:115-141`
(`build_body`) emits only `model`, `max_tokens`, `messages`, `tools`, `system`,
`temperature`, `top_p`, `thinking`, `stream`. No `anthropic-beta` header is set
either (`provider/anthropic.rs:158,181,231` set only `x-api-key`,
`anthropic-version`, `content-type`).

OpenAI automatic prompt caching would apply to a stable prefix, but it is
defeated by the volatile system prompt (Q4). Gemini explicit context caching is
not used (no `cachedContent` in `provider/google.rs`).

**Verdict: no prompt caching on any provider path. Every turn pays full input
price for the entire system prompt + tool schemas + full history.** This alone is
a ~5-10x cost multiplier on a tool-heavy 20-turn run.

### Q4. System prompt — rebuilt per run, and NOT byte-stable

- Rebuilt **once per `run()`**, i.e. once per user message, not per turn
  (`engine.rs:98-102`). Within a run it is `.clone()`d unchanged each turn
  (`engine.rs:196`), so it is byte-stable *within* a run.
- **Across runs it is NOT stable**, because `context.rs:117-121` appends
  `crate::memory::build_memory_context(sid, "", repo_path)` — a recall over the
  episodic/semantic/procedural memory tiers that changes as the session
  accumulates messages. Every user message therefore mutates the tail of the
  system prompt.
- Because the memory block is appended **last**, the ~10 KB base prompt +
  AUTODEV.md prefix *would* still be a cacheable stable prefix — if any
  `cache_control` breakpoint existed. It does not (Q3).
- No timestamps or directory listings are embedded; the volatility is entirely
  the memory block. It also does file I/O on **every** run
  (`context.rs:92,106-108` `std::fs::read_to_string`, plus two SQLite opens at
  `context.rs:86` and `context.rs:101` — the DB is opened **twice** per run
  rather than once).

### Q5. The graph — see §4. **Nothing reads it for context scoping.**

### Q6. Tool result size caps (all values CONFIRMED by reading the bodies)

| Tool | file:line | Cap |
|---|---|---|
| `bash` | `builtin_tools/bash.rs:13` | `MAX_OUTPUT_BYTES = 50_000` |
| `read_file` | `builtin_tools/read_file.rs:18` | `READ_FILE_MAX_LINES_PER_CALL = 2000` lines |
| `read_file` | `builtin_tools/read_file.rs:102` | `MAX_BYTES = 256_000` whole-file guard |
| `read_file` | `builtin_tools/read_file.rs:13` | `READ_FILE_REQUIRE_RANGE_OVER_LINES = 800` — forces a range arg over 800 lines |
| `send_worker_message` | `builtin_tools/send_worker_message.rs:27` | `MAX_BODY_BYTES = 4096` |
| `validate` (checker output) | `builtin_tools/validate.rs:846` | `MAX_CHECKER_BYTES = 16_000` |
| `grep` | `builtin_tools/grep.rs` | see §6 — **no byte cap constant** |
| `list_directory` | `builtin_tools/list_directory.rs` | see §6 — **no byte cap constant** |
| `glob` | `builtin_tools/glob.rs` | see §6 |
| `graph_tools` | `builtin_tools/graph_tools.rs` | see §6 |

The engine imposes **no cap of its own**: `engine.rs:314-325` copies
`result.output` verbatim into the `ToolResult` block. A single `read_file` can
therefore inject ~64 K tokens into history, and that block is re-sent on every
one of the remaining turns.

### Q7. Token counting

Two independent, inconsistent mechanisms:
1. **`len/4` heuristic** — `CompactionEngine::estimate_tokens` (`compaction.rs:31-41`)
   serialises each content block to JSON and divides char count by 4. This
   over-counts (JSON quoting/escaping) but is the **only** count that gates
   behaviour, and it gates only compaction (`engine.rs:175-178`).
2. **Provider-reported usage** — `StreamChunk::MessageDelta { tokens_in, tokens_out }`
   accumulated at `engine.rs:455-458`. Used for: budget circuit breaker
   (`engine.rs:218`), cost calculation (`engine.rs:219,355`), the analytics DB
   (`engine.rs:376-391`), and the `agent:done` event (`engine.rs:394-400`).

No real tokenizer (no `tiktoken`, no `count_tokens` API call) exists anywhere.
The budget circuit breaker is the only *other* behaviour gate and it only fires
when `with_task_budget()` was called — which `commands/agent_commands.rs:140-147`
**never does**, so interactive PA sessions have **no token or cost ceiling at all**.

### VERDICT — BUG-TOKENS

The blowup is the product of four compounding defects, all CONFIRMED:

1. **No prompt caching at all** (`provider/types.rs:49-59`, `provider/anthropic.rs:115-141`).
   The full ~10 KB base system prompt, the AUTODEV.md blocks, the memory block,
   the full tool-schema array (~40 tools registered in FastDevelopment mode) and
   the entire history are billed at full input rate on **every single turn**.
2. **Quadratic history resend inside a run.** `history.clone()` at `engine.rs:196`
   with two messages appended per turn means turn *k* pays for the outputs of
   turns 1..k-1 again. Over `MAX_TOOL_TURNS = 20` turns the engine bills
   ≈ *T²/2* = ~200 message-copies of tool output. With `read_file` allowed
   256 KB and `bash` 50 KB, one run can bill millions of input tokens.
3. **Compaction is effectively inert.** It needs ~640 KB of JSON history
   (`compaction.rs:13,26-28,31-41` × `engine.rs:174`) inside a 20-turn window,
   and even when it *does* fire it (a) makes an **extra full-context LLM call**
   to summarise (`compaction.rs:124`), and (b) its output is discarded by the
   caller (`commands/agent_commands.rs:166-196`). It cannot help across user
   messages at all.
4. **Wrong hardcoded context window.** `engine.rs:174` uses `200_000` for every
   model even though `ModelInfo::context_window` exists and differs per model.

The relieving factor — `build_history()` dropping tool messages
(`commands/agent_commands.rs:249`) — is itself a correctness bug: it makes the
agent forget everything it read, which drives it to re-read the same files on the
next user message, spending the tokens again from a cold start.

**Fix order (highest ratio first):** (a) add `cache_control: {type:"ephemeral"}`
breakpoints after the system prompt, after the tool array, and at a rolling
history breakpoint — requires adding the field to `CompletionRequest` and
`build_body`; (b) move the volatile memory block *out* of the system prompt into
the first user message so the prefix is byte-stable; (c) read
`ModelInfo::context_window` instead of `engine.rs:174`'s literal; (d) lower the
compaction threshold and make the caller persist the compacted history.

---

## 3. BUG-MODELSWITCH — traced mechanism

### Resolution path

`commands/agent_commands.rs:32-40` — resolved **per request**, at the top of every
`send_message` invocation:
```rust
let (provider_id, model) = {
    let s = state.lock().unwrap();
    let p = s.active_provider.clone().ok_or_else(|| ...)?;
    let m = s.active_model.clone().ok_or_else(|| ...)?;
    (p, m)
};
```
The provider object is then constructed fresh inside the spawned task
(`agent_commands.rs:129-138`) and handed to `AgentEngine::new`
(`agent_commands.rs:140-147`). The model string is passed to `engine.run(...)`
(`agent_commands.rs:149-156`) and used verbatim in the request
(`engine.rs:194`).

**So a model change DOES take effect on the next user message**, for both new and
existing sessions. It does **not** affect a run already in flight — the model and
provider are captured before `tokio::spawn`. That is defensible.

### Which commands change it

- `commands/settings_commands.rs:86-96` — `set_active_provider(provider, model)`;
  sets both fields on `AppState`. Called from the frontend at
  `src/stores/settingsStore.ts:96`.
- `commands/settings_commands.rs:98-107` — `get_active_provider`; read at
  `src/stores/settingsStore.ts:84`.
- Both registered in `lib.rs:194-195`.

There is **no** per-session model override command and no per-session model
column consulted on the PA path.

### DEFECT M1 — the active model is never persisted (CONFIRMED, HIGH)

`AppState` is created as `AppState::new()` at `main.rs:112` with
`active_provider: None, active_model: None` (`state/app_state.rs:14-42`,
`Default` derive). `set_active_provider` writes **in-memory only**
(`settings_commands.rs:92-93`) — no DB write. There is no code path anywhere in
`src-tauri/src` that hydrates `active_provider`/`active_model` at startup
(grep for `active_model` returns only `agent_commands.rs`, `diff_commands.rs`,
`recipe_commands.rs`, `settings_commands.rs`, `recipes/runner.rs`,
`state/app_state.rs`). **Every app restart resets the model to "none", and
`send_message` then hard-errors with `"No active model set"`** unless the
frontend re-pushes it. The user-visible symptom is exactly "changing the model
from the UI doesn't stick".

### DEFECT M2 — per-model settings are written but never read (CONFIRMED, HIGH)

`commands/model_config_commands.rs:12-42` persists `temperature`, `top_p`,
`reasoning_effort`, `thinking_enabled`, `thinking_budget` to
`db::model_configs::ModelConfigsDb`. A grep for `ModelConfigsDb` across the whole
tree returns **only** `commands/model_config_commands.rs` and the `db/mod.rs`
module declaration. Nothing reads it back.

Meanwhile every construction site of `CompletionRequest` hardcodes them to `None`:
- `agent/engine.rs:199-202`
- `agent/compaction.rs:117-120`
- `commands/recipe_commands.rs:248-249`
- `diff_analysis/classifier.rs:140-141`

**Changing temperature / reasoning effort / extended thinking in the UI has zero
effect on any request.** Writer with no reader.

### DEFECT M3 — provider inferred from the model string, twice, inconsistently

- `engine.rs:364-372` re-derives the analytics provider from substring matches on
  the model name (`contains("claude")`, `contains("gpt")`, `starts_with("o1")`,
  `starts_with("o3")`, `contains("gemini")`, else `"unknown"`), even though the
  real provider id was known at `agent_commands.rs:33`. An Ollama model or a
  `grok-*` model records as `"unknown"` in the analytics DB.
- `commands/worker_commands.rs:318-338` has a *third*, differently-written
  copy of the same inference.

### DEFECT M4 — silent cost mispricing for unknown models

`cost/tracker.rs:51-56` matches on substrings; anything unmatched falls through to
a default. `gpt-4o-mini` is unreachable because `contains("gpt-4o")` at line 54
matches first — **`gpt-4o-mini` is billed at `gpt-4o` rates (2.5/10.0 instead of
0.15/0.6), a 16x overstatement.** CONFIRMED (arm-ordering bug).

### Every hardcoded model string (file:line)

| file:line | String | Context |
|---|---|---|
| `agent/builtin_tools/analyze_diff_tool.rs:222` | `"claude-sonnet-4-6"` | `unwrap_or_else` fallback model — **this id does not appear in any provider's `models_static()`** |
| `agent/builtin_tools/spawn_qc_review.rs:146` | `"claude-sonnet-4-5"` | `unwrap_or` fallback model for QC reviewer |
| `recipes/runner.rs:160` | `"claude-sonnet-4-5"` | last-resort fallback after worker.model → recipe.model → active_model |
| `recipes/defaults.rs:1802` | `"claude-haiku-4-5"` | qc-dispatcher recipe model |
| `recipes/defaults.rs:1879` | `"claude-haiku-4-5"` | recipe model |
| `subagent/types.rs:65` | `DEFAULT_MODEL = "claude-haiku-4-5"` | subagent default |
| `db/workers.rs:177` | `"claude-opus-4-5"` | Supervisor seed row |
| `db/workers.rs:184` | `"claude-haiku-4-5"` | QC Agent seed row |
| `db/workers.rs:185-187` | `"claude-sonnet-4-5"` ×3 | Worker Backend/Frontend/Platform seed rows |
| `db/workers.rs:188` | `"claude-opus-4-5"` | Worker MCP seed row |
| `db/workers.rs:189` | `"claude-haiku-4-5"` | Worker Cleanup seed row |
| `provider/anthropic.rs:38,47,56` | `"claude-opus-4-5"`, `"claude-sonnet-4-5"`, `"claude-haiku-4-5"` | static catalogue (legitimate) |
| `provider/anthropic.rs:82-84` | `"claude-4-5-haiku"` etc. | slug normalisation table |
| `provider/openai.rs:36,45,54` | `"gpt-4.1"`, `"gpt-4.1-mini"`, `"gpt-4o"` | static catalogue (legitimate) |
| `provider/google.rs:36,45,54` | `"gemini-2.0-flash-thinking-exp-01-21"`, `"gemini-2.5-pro"`, `"gemini-2.5-flash"` | static catalogue (legitimate) |
| `provider/ollama.rs:416,425,461` | `"llama3.2"`, `"llama3.1"`, `"deepseek-coder"` | static catalogue (legitimate) |
| `sidecar/client.rs:1626-1627` | `"claude-haiku-4-5"` → `"claude-4-5-haiku"` | orchestrator slug map |
| `cost/tracker.rs:51-56` | `claude-opus`/`claude-sonnet`/`claude-haiku`/`gpt-4o`/`gpt-4o-mini`/`gemini-1.5-pro` | pricing table (stale: no gpt-4.1, no gemini-2.5) |
| `engine.rs:174` | `200_000` | hardcoded context window (not a model id, but the same class) |

No `grok-*` string exists anywhere in `src-tauri/src` — xAI is not supported.

### VERDICT — BUG-MODELSWITCH

The switch itself works per-request; the bug the owner is seeing is that the
choice is **volatile (M1: in-memory only, lost on restart)** and that everything
*around* the model — temperature, reasoning effort, thinking budget — is a dead
UI (M2). Fix M1 by persisting `active_provider`/`active_model` to SQLite and
hydrating at `main.rs:112`; fix M2 by reading `ModelConfigsDb` in
`agent_commands.rs` and threading the values into `CompletionRequest`.

---

## 4. Does anything read the graph? — **NO. Writer with no reader for context management.**

`src-tauri/src/graph/` contains 17 modules (`analyze, build, cache, changes,
cluster, detect, export, extract, flows, incremental, migrations, mod, query,
registry, report, schema, store, watcher`).

A tree-wide grep for `crate::graph` / `graph::` outside `graph/` itself returns
**exactly two consumer files**:

1. `agent/builtin_tools/graph_tools.rs:5-9` —
   `GraphStore`, `query_by_name`, `query_dependents`, `query_dependencies`,
   `god_nodes`, `surprising_connections`, `suggest_questions`,
   `get_impact_radius`, `graph_diff`, `detect_entry_points`, `detect_changes`.
   These *look* like LLM-callable tools — **but see the finding below: this file
   is never compiled.**
2. `commands/graph_commands.rs:5-11,137,165,197,278,304` — Tauri commands for the
   graph UI (build, cluster, report, export, incremental update). Frontend-only.

### G-1 (CRITICAL, CONFIRMED) — `graph_tools.rs` and `graph_prompts.rs` are not in the crate

`agent/builtin_tools/mod.rs` declares 44 `pub mod` lines (`mod.rs:6-72`).
**Neither `graph_tools` nor `graph_prompts` is among them**, and a grep for
`mod graph_tools` / `mod graph_prompts` across the entire `src-tauri` tree
returns **no match**. Rust only compiles files reachable from a `mod`
declaration, so:

- `agent/builtin_tools/graph_tools.rs` — **840 lines, never compiled, never
  registered, unreachable at runtime.**
- `agent/builtin_tools/graph_prompts.rs` — **78 lines, same.**

Corroborating comment: `mcp_server/server.rs:144` says
*"graph_tools / graph_prompts — use GraphStore API, not Tool trait; PA-only
analysis"* — i.e. someone knew they were outside the tool path, but they are in
fact outside the **binary**. Neither `register_for_mode()`
(`agent/builtin_tools/mod.rs:174-245`) nor `register_for_mcp_worker()`
(`mod.rs:96-164`) mentions any graph tool.

**Definitive answer to the owner's bug #4: CONFIRMED, and stronger than
suspected.** Nothing in `agent/engine.rs`, `agent/context.rs`,
`agent/compaction.rs`, `agent/tool_executor.rs`, or
`agent/builtin_tools/mod.rs` imports or calls any graph API. `context.rs` has no
`graph` import at all — its only inputs are the three static prompt constants,
`AutodevInstructionsDb`, two filesystem fallbacks, and
`crate::memory::build_memory_context`. And the one file that *did* wire the graph
to the agent was never added to the module tree, so the agent has **no graph
access of any kind** — not automatic, not even opt-in.

**Nothing in `agent/engine.rs`, `agent/context.rs`, `agent/compaction.rs`,
`agent/tool_executor.rs`, or `agent/builtin_tools/mod.rs` imports or calls any
graph API.** `context.rs` has no `graph` import at all — its only inputs are the
three static prompt constants, `AutodevInstructionsDb`, two filesystem fallbacks,
and `crate::memory::build_memory_context`.

**Summary:** The code graph is built, stored, clustered, exported, and rendered
in the UI, but **no code path anywhere reads a graph query API to scope, narrow,
rank, or budget the agent's context** — and the only file that ever tried is
dead. The graph is a pure UI side-artifact. There is no retrieval, no relevance
ranking, no "files related to this task" pre-selection — the agent rediscovers
the codebase turn-by-turn with `grep`/`glob`/`read_file`, paying full input price
each turn (§2), and again from scratch on the next user message because
`build_history()` drops tool results (`commands/agent_commands.rs:249`).

---

## 5. Per-file narrative

### `agent/engine.rs` (537 lines)

Single-struct driver. `run()` is one long function that: resolves the system
prompt once (`:96-102`), pushes the user message (`:106-109`), builds a
`ToolExecutor` (`:111-121`), then loops.

Each loop iteration: fire `AgentLoopStart` hook (`:131-146`) → increment and
check the budget (`:149-166`) → check `MAX_TOOL_TURNS` (`:168-171`) → maybe
compact (`:173-185`) → check cancel (`:188-191`) → build and stream the request
(`:193-214`) → accumulate usage into the budget (`:217-230`) → collect tool calls
(`:262-265`) → append the assistant message (`:268-271`) → execute the batch
(`:298-302`) → append tool results as a `user` message (`:328-331`).

**What is right:** hooks are fired at loop start/end and on error; cancellation
is checked twice per turn; the `std::sync::Mutex` on `AppState` is taken in
scoped blocks (`:92`, `:188`, `:356-359`) and never held across an `.await`
(checked — no lock guard survives into an await point anywhere in this file);
budget escalation (`:480-536`) logs on every DB failure rather than swallowing.

**What is wrong:**
- The four token defects (T1-T4, T10) all live in `:174` and `:193-203`.
- `stream_response` (`:407-474`) rebuilds content blocks from a `HashMap`
  (T43) and silently swallows malformed tool-input JSON (T42).
- `total_tokens_in/out` are `u32` and accumulated without checked arithmetic
  (`:456-457`). At ~4.3 B tokens this wraps; not reachable in practice, but the
  budget is `u64` (`:218`) so the types disagree.
- The provider is re-inferred from the model string for analytics (T29).
- `MAX_TOOL_TURNS` exit is a bare `break` with an `error!` log and no user-facing
  event (T38); combined with T7 the user loses the whole turn.
- `escalate_budget_exceeded` (`:480-536`) is `fn`, not `async fn`, and does
  blocking SQLite work (`TasksDb::open`, `create_worker_request`,
  `mark_task_budget_exceeded`) directly on the tokio worker thread — a blocking
  call inside an async context. LOW/MEDIUM.

### `agent/context.rs` (302 lines)

Two prompt builders (one dead — T44) plus three large `const &str` prompts
occupying lines 139-303. The live builder
`build_system_prompt_with_mode()` (`:72-124`) is a clean four-part concatenation.

**What is wrong:** the DB is opened twice (T9); the memory block is appended last
and is volatile (T8); the base prompt contradicts itself on lockfiles (T46),
tells the model to look for `CLAUDE.md` after the product explicitly dropped that
convention (T45, contradicting `:62-63`), and documents `grep` as ripgrep (T34).
The prompt also documents only 8 tools (`:176-186`) while 38 are actually
registered — the model receives 30 tools it was never told about in prose, and 8
described with parameters that don't match the real schemas (e.g. the prompt says
`grep(pattern, path?, context_lines?)` but the real schema also takes
`case_insensitive`, `glob`, `max_results`).

**Notable:** the `TODO` at `:9-10` and `:31-33` ("Import DB module when
implemented", "Phase 2.5 — Replace filesystem-based injection with SQLite DB")
are stale — the DB path *is* implemented, in the other function (`:86-113`).

### `agent/compaction.rs` (159 lines)

Small and readable. `needs_compaction` (`:26-28`), `estimate_tokens` (`:31-41`),
`compact_truncate` (`:45-74`), `compact_with_summary` (`:78-153`), and a legacy
alias `compact` (`:156-158`).

**What is wrong:**
- The threshold is effectively unreachable in a 20-turn window (T4), and the
  window it is measured against is hardcoded by the caller (T3).
- `estimate_tokens` measures the **JSON-serialised** form, inflating the count by
  quoting and escaping — so the estimate is systematically high relative to real
  tokens, which makes the already-high threshold *behave* slightly lower. It is
  never reconciled against the provider's real `tokens_in`, which the engine
  already has (`engine.rs:456`).
- `compact_with_summary` sends the **entire middle of the conversation** as a
  fresh request (`:94-124`) using the same expensive model the session is on
  (`engine.rs:183`). Compacting therefore costs one extra full-context input
  charge — at the exact moment the context is largest.
- `compact` (`:156-158`) is a legacy alias; grep shows no caller. Dead code.
- `messages[0]` is always preserved (`:57`, `:144`) on the assumption it is
  "context/system instructions" (`:5`). In practice `messages[0]` is whatever the
  DB returned first from `build_history()` — an ordinary user message. The
  docstring's premise does not hold.

### `agent/tool_executor.rs` (459 lines)

Three execution paths that have drifted apart:
1. `execute()` (`:78-194`) — full path with permission check, both hooks,
   persistence, and both events. **Grep shows no caller** — `engine.rs` uses only
   `execute_batch`. Dead code, and it is the *only* path that is fully correct.
2. `execute_batch` read partition (`:224-303`) — concurrent via a
   `Semaphore(10)`. No permission check (by design), **no hooks at all** (T22),
   and a broken panic path (T21).
3. `execute_batch` write partition (`:306-422`) — serial, with permission and
   hooks. Correct, but duplicates ~80 lines of `execute()`.

Other defects: `always_allow` scoped to one user message despite its comment
(T23); `SmartApprove` is a stub (T24); no permission timeout (T25); three unused
clones per read tool (T26); `requires_permission()` never consulted (T47);
`ToolContext.depth` and `.agent_role` hardcoded (T48, T49).

`request_permission` (`:427-458`) correctly scopes the `std::sync::Mutex` guard
in a block (`:434-437`) before awaiting — no lock-across-await bug here. Same for
`path_lock.rs:94-104`, which releases the registry `StdMutex` before
`lock_owned().await`. **No `Mutex`/`RwLock` held across `.await` was found
anywhere in the audited scope.**

---

## 6. Builtin tools inventory

38 tools are registered in `PAMode::FastDevelopment` / `Review`
(`agent/builtin_tools/mod.rs:174-235`); 4 in `Delegate` (`:236-243`);
33 via `register_for_mcp_worker` (`:96-164`).

| Tool name | File | read_only | Registered (Fast/Delegate/MCP) | What it does | Defect |
|---|---|---|---|---|---|
| `read_file` | `read_file.rs` | true | F / – / M | Read a file with line numbers; 800-line no-range refusal, 2000-line hard cap, 256 KB whole-file guard | **T12** no path jail |
| `write_file` | `write_file.rs` | false | F / – / M | Create a file, refuse-on-exists without `overwrite=true`; jail + path lock + checkpoint + 4-tier lint | **T53/T54** lint subprocesses have no timeout and fail on Windows |
| `edit_file` | `edit_file.rs` | false | F / – / M | Exact-string replace with `replace_all`; jail + path lock + checkpoint + tier-1 lint | **T14** infinite loop on empty `old_string`; **T15** corruption on `replace_all` + empty |
| `append_file` | `append_file.rs` | false | F / – / M | Append with trailing-newline probe; jail + path lock | **T36** no checkpoint, no `PostFileWrite` hook |
| `bash` | `bash.rs` | false | F / – / M | Run a shell command; 120 s default timeout, 50 KB tail-truncated output, cwd jail + redirect scan | **T16** PowerShell rewrite fed to `cmd`; **T17** unreachable `-i` arm; **T18** UTF-8 slice panic; **T19** byte-as-char |
| `list_directory` | `list_directory.rs` | true | F / – / M | List entries as JSON, depth 3 when recursive | **T11** `max_entries` does not bound a directory; `truncated` lies; **T13** no jail |
| `grep` | `grep.rs` | true | F / – / M | `WalkDir` + `regex` crate search, 100-match cap, 1 context line | **T13** no jail; **T34/T35** not ripgrep, no binary detection, no per-line cap; **T57** skip-list drift |
| `glob` | `glob.rs` | true | F / – / M | `WalkDir` + `glob::Pattern`, 200-file cap | **T13** no jail; **T33** does not respect `.gitignore` despite the claim |
| `check_file` | `check_file.rs` | true | F / – / M | On-demand lint/type check for one file | Shares **T53/T54** (calls `validate`) |
| `validate` (module, not a Tool) | `validate.rs` | – | – | Tier 1 syntax, Tier 2 imports, Tier 2.5 exports, Tier 2.6 API contract | **T53** no timeouts; **T54** `.cmd` spawn failure on Windows produces a false "not installed" diagnostic |
| `create_task` | `create_task.rs` | false | F / D / M | Create a task on the SQLite board | — |
| `list_tasks` | `list_tasks_tool.rs` | true | F / – / M | List tasks with filters | — |
| `get_task` | `get_task_tool.rs` | true | F / – / M | Fetch one task | — |
| `update_task_status` | `update_task_status_tool.rs` | false | F / – / M | Drive the task lifecycle; build-evidence gate on `qc_review` | **T50** gate skipped whenever `agent_role` is `None` (always, natively) |
| `assign_task` | `assign_task.rs` | false | F / – / M | Supervisor dispatch to a worker slug | — |
| `assign_tasks` | `assign_tasks_batch.rs` | false | F / – / M | Batch dispatch | — |
| `claim_task` | `claim_task_tool.rs` | false | F / – / M | Atomic PID-recording claim | Falls back off `ctx.agent_role` (`:85`), always `None` natively |
| `suggest_worker` | `suggest_worker_tool.rs` | true | F / – / M | Routing hint for the Supervisor | — |
| `planner` | `planner.rs` | false | F / D / M | Plan compile / breakdown / claim / submit | — |
| `request_supervisor_action` | `request_supervisor_action.rs` | false | F / – / M | Worker→Supervisor escalation queue | Reads `ctx.agent_role` (`:102`), always `None` natively |
| `list_worker_requests` | `list_worker_requests_tool.rs` | true | F / – / M | Drain the escalation queue | — |
| `resolve_worker_request` | `resolve_worker_request_tool.rs` | false | F / – / M | Mark an escalation resolved | — |
| `send_worker_message` | `send_worker_message.rs` | false | F / – / M | ACP worker→worker message, 4 KB body cap | **T49** always refuses on the native path |
| `list_worker_messages` | `list_worker_messages.rs` | false | F / – / M | Read the ACP inbox | **T49** always refuses; **T56** self-contradicting comment |
| `acknowledge_worker_message` | `acknowledge_worker_message.rs` | false | F / – / M | Ack an ACP message | **T49** always refuses |
| `spawn_subagent` | `spawn_subagent.rs` | false | F / – / M | Spawn one ephemeral subagent and block on its result | **T48** depth guard inert (`ctx.depth` always 0) |
| `spawn_parallel_subagents` | `spawn_parallel_subagents.rs` | false | F / – / M | Fan out ≤10 subtasks in windows of `max_concurrent` (default 5) | **T48** same |
| `spawn_qc_review` | `spawn_qc_review.rs` | false | F / – / M | Claim a task and spawn a QC reviewer; `CEILING_USD_PER_HOUR=5.0`, `PROVISIONAL_COST_USD=0.10` hardcoded (`:354,358`) | **T48** same; hardcoded model `"claude-sonnet-4-5"` (`:146`) |
| `write_interface_spec` | `write_interface_spec.rs` | false | F / – / M | Backend writes `INTERFACE.md` before `qc_review` | **T47** its `requires_permission()->false` is ignored, so it prompts anyway |
| `e2e_probe` | `e2e_probe.rs` | false | F / – / M | Boot backend+frontend, curl an endpoint, look for a fingerprint. 30 s / 8 s / 10 s timeouts (`:34-36`) | **T47** same; **T55** orphaned grandchildren on Windows |
| `analyze_diff` | `analyze_diff_tool.rs` | **true** | F / – / M | LLM-scored diff classification + interface-drift detection | **T51** env-var model resolution that is never set → hardcoded nonexistent `claude-sonnet-4-6`; **T52** paid LLM call marked read-only |
| `annotate_file` | `annotate_file_tool.rs` | false | F / – / M | Pin review notes to file lines | Reads `ctx.agent_role` (`:77`), always `None` natively |
| `set_task_budget` | `set_task_budget_tool.rs` | false | F / – / M | Set per-task token/cost/turn budget | Budget only takes effect if the runner calls `with_task_budget()` — **T40** |
| `create_epic` | `create_epic_tool.rs` | false | F / – / – | Create a phased epic | — |
| `create_phased_task` | `create_phased_task_tool.rs` | false | F / – / – | Create a task within a phase | — |
| `get_epic_progress` | `get_epic_progress_tool.rs` | true | F / – / M | Epic progress rollup | — |
| `get_ready_tasks_in_phase` | `get_ready_tasks_in_phase_tool.rs` | true | F / – / M | Unblocked tasks in a phase | — |
| `search_memory` | `search_memory.rs` | true | F / D / – | Query the 3-tier memory store | — |
| `remember_this` | `remember_this.rs` | false | F / D / – | Write a semantic memory fact | — |
| **`graph_*` (all)** | `graph_tools.rs` | – | **NOT COMPILED** | Graph query / impact radius / god nodes / entry points / diff | **T32 — no `mod` declaration; 840 lines dead** |
| **`graph_prompts`** | `graph_prompts.rs` | – | **NOT COMPILED** | Graph prompt templates | **T32 — 78 lines dead** |
| _(helpers, not tools)_ | `path_jail.rs`, `path_lock.rs`, `build_evidence.rs` | – | – | Workspace jail, per-path async mutex, build-invocation ring | **T20** UTF-8 slice panic in the Windows jail arm; **T37** lock registry never GC'd |

**Registration asymmetries worth noting:** `create_epic` and `create_phased_task`
are registered in PA modes but **not** in `register_for_mcp_worker`, even though
`get_epic_progress` and `get_ready_tasks_in_phase` are — a Supervisor running
through the MCP surface can read epic progress but cannot create an epic.
Conversely `search_memory` / `remember_this` are absent from the MCP worker set,
so fleet workers have no memory access at all.

---

## 7. Cross-platform portability

| # | Location | Assumption | Windows | Linux/macOS | Severity |
|---|---|---|---|---|---|
| P1 | `agent/builtin_tools/bash.rs:89-97` | `cmd /C` on Windows, `sh -c` elsewhere | Works, but `cmd` is not `sh` — no `&&` semantics parity, no globbing, no pipes-to-Unix-tools | Works | (baseline, known) |
| P2 | `agent/builtin_tools/bash.rs:404-437` | `rewrite_for_windows` emits **PowerShell** cmdlets (`Select-Object`, `Select-String`) | **Broken** — the rewritten command is executed by `cmd /C`, which does not know these cmdlets (T16) | N/A (guarded by `cfg!`) | HIGH |
| P3 | `agent/builtin_tools/bash.rs:409-412` | Strips `cd "<wd>" && ` / `Set-Location "<wd>" ; ` prefixes by exact string match | Only matches the four exact spellings; `cd /d "<wd>" &&`, single quotes, or trailing-slash variants slip through | N/A | LOW |
| P4 | `agent/builtin_tools/bash.rs:131-136` | Byte-slicing the 50 KB output tail | Panics on a non-ASCII char at the cut point (T18) — more likely on Windows where cargo/npm emit box-drawing and `→` | Same risk | HIGH |
| P5 | `agent/builtin_tools/validate.rs:269` (`tsc`), `:753` (`pyright`) | Bare binary names resolved via PATH | **Broken** — npm/pip install these as `.cmd` shims; Rust `Command` appends only `.exe` (T54). The failure is reported as "TypeScript not installed" | Works (real executables) | HIGH |
| P6 | `agent/builtin_tools/validate.rs:885-903` | Venv python discovery | Correct: `\.venv\Scripts\python.exe` etc. under `cfg!(target_os = "windows")` | Correct: `.venv/bin/python3` | OK — this one is done right |
| P7 | `agent/builtin_tools/validate.rs:221` (`node`) | Bare `node` | Works (`node.exe` is a real exe) | Works | OK |
| P8 | `agent/builtin_tools/e2e_probe.rs:179-197` | `cmd /C` + `kill_on_drop(true)` for long-running servers | **Broken teardown** — killing `cmd.exe` orphans `npm`/`node` grandchildren; no job object (T55) | `sh -c` also leaves the child's own children, but process-group kill is at least possible | MEDIUM |
| P9 | `agent/builtin_tools/path_jail.rs:104-121` | Windows arm lowercases and byte-slices; non-Windows uses component-wise `Path::starts_with` | Case-insensitive compare is correct for NTFS; the byte slice can panic on non-ASCII paths (T20) | Correct | MEDIUM |
| P10 | `agent/builtin_tools/path_jail.rs:89-96` | Strips the `\\?\` verbatim prefix after `canonicalize()` | Correct and necessary | No-op | OK |
| P11 | `agent/context.rs:132-137` | `home_dir()` = `USERPROFILE` else `HOME` | Correct | Correct | OK |
| P12 | `agent/builtin_tools/grep.rs:177`, `glob.rs:136`, `check_file.rs:204` | `.replace('\\', "/")` to normalise relative paths for glob matching | Correct — but it also mangles any legitimate backslash in a filename (legal on Linux) | Corrupts Linux filenames containing `\` | LOW |
| P13 | `agent/builtin_tools/list_directory.rs:122-124` | `entry_path.display()` for the relative path — **not** normalised to `/` | Returns `src\foo.rs` here while `grep`/`glob` return `src/foo.rs` | Consistent | LOW (inconsistent path style between tools confuses the agent) |
| P14 | `agent/builtin_tools/path_lock.rs:146,151` (tests) | Hardcoded `/tmp/...` paths | Test-only; the path never touches the filesystem (it is only a HashMap key), so the test passes on Windows | Fine | LOW |
| P15 | `agent/builtin_tools/bash.rs:149` | Windows-only exit-code-255 + `"is not recognized"` suggestion branch | Fires only on Windows; the suggestion recommends PowerShell syntax to a `cmd` shell (compounds T16) | N/A | LOW |
| P16 | `agent/context.rs:166-167` (system prompt) | Prescribes `uv`, `ruff`, `mypy`, `pytest`, `pnpm`, `eslint`, `vitest` unconditionally | Same on all platforms — but hardcoded toolchain assumptions in the prompt apply to every repo the agent opens | Same | LOW |

**The one genuinely load-bearing portability bug is P2/T16:** on Windows the
`bash` tool actively rewrites the model's command into a dialect the shell it
then invokes cannot execute. Either invoke `powershell -Command` (matching the
rewriter) or delete the rewriter (matching the `cmd /C` invocation and the tool
description at `bash.rs:23`). The current combination is the worst of both.

---

## 8. What was NOT found (negative results worth recording)

- **No `Mutex`/`RwLock` held across an `.await`** anywhere in the audited scope.
  `engine.rs:92,188,356`, `tool_executor.rs:434-437`, and `path_lock.rs:94-104`
  all scope their guards correctly.
- **No `todo!()`, `unimplemented!()`, or `panic!()`** in `agent/**` outside test
  modules. The stubs that exist are behavioural (T24 `SmartApprove`), not macro
  stubs.
- **No `grok-*` model string** anywhere in `src-tauri/src` — xAI is unsupported.
- **No retry/backoff layer** in `provider/` — a single 429 or 5xx aborts the run.
- **No tokenizer dependency** (`tiktoken`, `tokenizers`) in the tree; token
  counting is `len/4` plus provider-reported usage only (§2 Q7).
