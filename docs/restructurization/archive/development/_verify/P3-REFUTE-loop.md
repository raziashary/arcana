# P3 Claims Refutation — Fault Isolation (P3-A) and the Agent Loop (P3-B)

**Verifier:** adversarial subagent, read-only.
**Repo:** `C:\AIRS Foundry\autodev\autodev`
**Mandate:** attempt to refute each claim. Default to REFUTED when evidence is ambiguous.

_Status: COMPLETE._

---

## P3-01 · `[profile.release]` currently sets `panic = "abort"`, and it should become `"unwind"`
**Verdict:** REFUTED (the literal text exists, but the setting is INERT — and the plan's fix, applied as written, would be a no-op)

**Evidence:**

The block does exist, but only in a **non-root workspace member**:

`src-tauri/Cargo.toml:117-122`
```toml
[profile.release]
panic = "abort"
codegen-units = 1
lto = true
opt-level = "s"
strip = true
```

The workspace root has **no `[profile]` block at all** — `Cargo.toml:1-8` is the entire file:
```toml
[workspace]
members = [
    "autodev-core",
    "src-tauri",
    "src-cli",
    "src-shared",
]
resolver = "2"
```

`src-tauri` is a listed workspace member, so Cargo discards its profile. Confirmed by execution — `cargo metadata --no-deps --format-version 1` (stderr):
```
warning: profiles for the non root package will be ignored, specify profiles at the workspace root:
package:   C:\AIRS Foundry\autodev\autodev\src-tauri\Cargo.toml
workspace: C:\AIRS Foundry\autodev\autodev\Cargo.toml
```

Grep confirms `[profile` appears exactly once in the whole repo's manifests:
```
./src-tauri/Cargo.toml:117:[profile.release]
```

**Reasoning:** The claim asserts a *current effective state* ("currently sets"). It does not. Cargo ignores profiles declared in non-root workspace members and says so loudly. The effective release profile is Cargo's built-in default, whose `panic` is already `"unwind"`. So the stated defect (unwinding disabled) does not exist, and the stated goal (unwinding enabled) is already true today.

Note the collateral: `lto = true`, `codegen-units = 1`, `opt-level = "s"` and `strip = true` are **also** inert. Release builds are currently `opt-level = 3`, no LTO, 16 codegen units, unstripped — i.e. bigger binaries and no LTO, silently, for however long this has been the case. That is a separate real defect the plan does not mention (see "Claims I would add").

Also note `src-tauri/Cargo.toml:88` carries a stale comment:
```
# Task D3: Keep 0.3.4; panic prevention via D1 (sanitized OLLAMA_HOST) + D2 (catch_unwind)
```
referencing a `catch_unwind` that does not exist anywhere in the repo (see P3-02).

**If the fix were applied as written:** Editing `src-tauri/Cargo.toml:118` to `panic = "unwind"` is a **pure no-op**. Cargo already ignores the block; the binary is already unwinding. It would also leave the real bug (an entire silently-discarded release profile) in place, while creating the false impression the profile is now correct. The correct fix is to **move the whole `[profile.release]` block to the workspace root `Cargo.toml`** and set `panic = "unwind"` there — at which point `lto`/`opt-level = "s"`/`strip` suddenly become live for the first time, which is a behaviour and build-time change that must be validated, not assumed.

---

## P3-02 · No agent turn or subagent currently runs inside `catch_unwind`
**Verdict:** CONFIRMED (literally true) — but the framing is misleading: the Mode 1 path already has *runtime-level* panic isolation via `tokio::spawn`, which `catch_unwind` would duplicate rather than add.

**Evidence:** Repo-wide search across `autodev-core/`, `src-tauri/`, `src-cli/`, `src-shared/` for `catch_unwind`, `AssertUnwindSafe`, `resume_unwind`, `panic::set_hook` returns exactly **one** hit, and it is not `catch_unwind`:

`src-tauri/src/main.rs:97-101`
```rust
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        eprintln!("[PANIC] {msg}");
        tracing::error!("[PANIC] {msg}");
    }));
```

The stale comment at `src-tauri/Cargo.toml:88` claims otherwise:
```
# Task D3: Keep 0.3.4; panic prevention via D1 (sanitized OLLAMA_HOST) + D2 (catch_unwind)
```
There is no `catch_unwind`. That comment is a doc-vs-code lie (Rule 5 bug), and it sits above a *removed* Ollama dependency — the crate line it annotates is gone.

**What already isolates panics, unnamed by the claim:**

`src-tauri/src/commands/agent_commands.rs:134` — the entire Mode 1 turn runs in a detached tokio task:
```rust
    tokio::spawn(async move {
```
`autodev-core/src/agent/tool_executor.rs:314` — concurrent read-only tools each run in their own task, and the join site **already handles the panic**:
```rust
                    Err(e) => {
                        error!("Read-only tool task panicked: {}", e);
                        // Placeholder will be replaced; we need to find which one failed
                    }
```
`autodev-core/src/subagent/native_runner.rs:220-234` — subagents run under a `timeout(...)` wrapper.

**Reasoning:** The literal statement holds. But its implied premise — that without `catch_unwind` a panic is unhandled — is false for the tokio-spawned paths, because `panic = "abort"` is inert (P3-01) so unwinding is live and tokio converts a task panic into a `JoinError`.

**If the fix were applied as written:** Wrapping `engine.run(...)` in `catch_unwind` at `agent_commands.rs:134` would be **largely redundant** — the enclosing `tokio::spawn` already catches it. Worse, `catch_unwind` around an `async` block does not do what it looks like: it catches only panics raised while the future is polled at that point, and `AgentEngine::run` is not `UnwindSafe` (it holds `Arc<Mutex<AppState>>`), so the fix requires `AssertUnwindSafe` — asserting exactly the property that is false here (see P3-03). The genuine gap is not "no `catch_unwind`", it is the two things a `catch_unwind` alone would not fix: the poisoned `Mutex<AppState>` and the never-cleared `is_running` flag.

---

## P3-03 · A panicking worker currently takes down the whole app rather than failing just that turn
**Verdict:** REFUTED as stated — TRUE for a different and worse reason.

**Evidence:**

*(1) The process does not abort.* `panic = "abort"` is discarded by Cargo (P3-01, proven by the `cargo metadata` warning), so builds unwind. Every agent path is inside a `tokio::spawn`, and tokio converts a task panic into a dropped `JoinHandle` — the process survives.

*(2) But the shared state mutex is poisoned, and 93 of 98 lock sites `.unwrap()` on it.* The engine holds `Arc<Mutex<AppState>>` and locks it inside the loop:

`autodev-core/src/agent/engine.rs:124` and `engine.rs:384`
```rust
        if self.state.lock().unwrap().is_cancelled(session_id) {
```
Measured counts across `src-tauri/src` + `autodev-core/src`:
- `.lock().unwrap()` total: **98**
- sites that survive poisoning (`unwrap_or_else(|e| e.into_inner())`): **5**, and only in `autodev-core/src/cache/monitor.rs` (4) and `autodev-core/src/db/test_support.rs` (1).

So a panic anywhere while `AppState` is held poisons the mutex permanently, and every subsequent `state.lock().unwrap()` — in agent commands, in the engine, in Tauri handlers — panics in turn. That is not "just that turn": it is a **permanent, restart-only app-wide failure**, and it presents as a cascade of unrelated panics rather than as the original fault.

*(3) The `is_running` flag leaks.* `agent_commands.rs:171-177`:
```rust
        // Clear running flag
        let mut s = state_clone.lock().unwrap();
        if let Some(agent) = s.agents.get_mut(&session_id_c) {
            agent.is_running = false;
            agent.cancel_flag = false;
        }
```
This is *after* the `.await`. A panic inside `engine.run` skips it entirely, so the session stays `is_running = true` forever and the user cannot start another turn in it. And this line is itself a `.lock().unwrap()` — so on a poisoned mutex it panics too.

**Reasoning:** The claim's mechanism ("takes down the whole app") is wrong — the process survives. The claim's *severity* is understated: mutex poisoning plus a leaked `is_running` flag is worse than a clean crash, because a crash is at least attributable and clears on restart, whereas a poisoned mutex leaves an app that is running, looks alive, and panics on every subsequent interaction with confusing stack traces.

**If the fix were applied as written:** Adding `catch_unwind` around the turn stops nothing that `tokio::spawn` does not already stop, and does **not** un-poison the mutex — `catch_unwind` returning `Err` still leaves `Mutex<AppState>` poisoned. The fix that actually addresses the observed behaviour is: (a) replace `.lock().unwrap()` with a poison-tolerant helper across all 98 sites, and (b) move the `is_running` cleanup into a guard/`Drop` so it runs on unwind. Neither is in the plan.

---

## P3-04 · Three identical byte-slice truncation panics at `bash.rs:134`, `remember_this.rs:94`, `annotate_file_tool.rs:99`; correct impl already at `validate.rs:846-853`
**Verdict:** PARTIALLY CONFIRMED — the panics are **real and reproduced**, but **every line number in the claim is wrong**, they are **not identical** (two share a shape, the third does not), and the "already-correct implementation" is **not at the cited lines** (it is 99 lines later) — while two *actual* reusable helpers exist elsewhere that the claim never mentions.

### The three panic sites — real, but at different lines

**(a) `remember_this.rs:100-104`** — claim said line 94 (line 94 is `Some(&ctx.working_dir),`, an argument to `SemanticStore::store_fact`):
```rust
        let display = if fact.len() > 120 {
            &fact[..120]
        } else {
            &fact
        };
```
`fact` is arbitrary model-supplied text. Byte 120 need not be a char boundary. **Panics.**

**(b) `annotate_file_tool.rs:109`** — claim said line 99 (line 99 is `task_id: None,`, a struct field):
```rust
                    if note.len() > 80 { &note[..80] } else { &note }
```
`note` is arbitrary model-supplied text. **Panics.**

**(c) `bash.rs:159-172`** — claim said line 134 (line 134 is `Ok(Err(e)) => ToolResult::error(format!("Failed to spawn command: {e}")),`):
```rust
                if output.len() > MAX_OUTPUT_BYTES {
                    let tail_start = output.len() - MAX_OUTPUT_BYTES;
                    // Walk forward to a newline boundary to avoid cutting mid-line.
                    let boundary = output[tail_start..]
                        .find('\n')
                        .map(|i| tail_start + i + 1)
                        .unwrap_or(tail_start);
                    output = format!(
                        "[⚠ Output truncated — showing last {} bytes of {} bytes total]\n{}",
                        output.len() - boundary,
                        output.len(),
                        &output[boundary..]
                    );
                }
```
with `bash.rs:13`: `const MAX_OUTPUT_BYTES: usize = 50_000;`

The panic here is on **line 162**, `output[tail_start..]` — a *tail* offset, computed by subtraction, not a fixed head constant. This is a **different shape** from (a) and (b): it slices the suffix, the `.unwrap_or(tail_start)` fallback on line 165 is a second unsafe index, and only `&output[boundary..]` on line 170 is safe (and only in the branch where a `\n` was found — which line 162 must survive to reach). A fix written for the `&x[..N]` shape does not cover this site.

This is the **most likely** of the three to fire in practice: `output` is compiler/test output, and `rustc`, `cargo` and `tsc` all emit multibyte box-drawing characters (`┌ │ └ ─ →`) in diagnostics.

### Proof by execution (isolated, no repo files touched)

The three expressions were transcribed verbatim into a standalone program (`<scratchpad>/panicproof.rs`) and run under `catch_unwind`:
```
fact.len()=123 is_char_boundary(120)=false
  panicked: byte index 120 is not a char boundary; it is inside 'é' (bytes 119..121)
remember_this[..120] => PANIC
note.len()=83 is_char_boundary(80)=false
  panicked: byte index 80 is not a char boundary; it is inside 'é' (bytes 79..81)
annotate[..80]     => PANIC
out.len()=19 tail_start=9 is_char_boundary=false
  panicked: byte index 9 is not a char boundary; it is inside '│' (bytes 7..10)
bash tail_start    => PANIC
validate impl      => ok, out="x││|end=7"
validate ascii max=10 => "aaaaaaaaa|end=9"
```
All three panic. The `validate.rs` shape does not.

### The "correct implementation" is not where the claim says

`validate.rs:846-853` is **mypy/pyright plumbing**, not a truncation helper:
```rust
    if pyright_available {
        run_pyright(path, working_dir).await
    } else {
        run_mypy(path, working_dir).await
    }
}

async fn run_mypy(path: &Path, working_dir: &str) -> LintResult {
```

The real safe truncation is at **`validate.rs:945-957`**:
```rust
            const MAX_CHECKER_BYTES: usize = 16_000;
            let capped = if combined.len() > MAX_CHECKER_BYTES {
                let safe_end = combined
                    .char_indices()
                    .map(|(i, _)| i)
                    .take_while(|&i| i < MAX_CHECKER_BYTES)
                    .last()
                    .unwrap_or(MAX_CHECKER_BYTES);
                format!(
                    "{}\n[... output truncated — {} bytes total, showing first {}. \
                     Fix the errors above, then re-run to see remaining issues.]",
                    &combined[..safe_end],
                    combined.len(),
                    safe_end,
                )
```

It is panic-safe (verified above), but it is **not a helper** — it is an inline block with a function-local `const`, not callable from anywhere else. And it has two defects of its own:
- **Off-by-one:** `.take_while(i < MAX).last()` returns the *start* index of the last char beginning before MAX, so it always drops one more char than necessary. Measured above: max=10 on pure ASCII keeps 9 bytes, not 10.
- **Unreachable-but-unsafe fallback:** `.unwrap_or(MAX_CHECKER_BYTES)` would itself be a raw byte index into `combined`, i.e. the very bug being avoided. It is unreachable today (`.last()` is `Some` whenever `combined.len() > 16_000`), but it is the wrong default to write.

**The claim omits the two genuinely reusable, genuinely correct helpers** that already exist — duplicated verbatim in two modules:

`autodev-core/src/memory/recall.rs:183-192` and `autodev-core/src/memory/summarizer.rs:148-157` (byte-identical):
```rust
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}
```
Both are private (`fn`, not `pub fn`), so neither is reachable from `agent/builtin_tools/`. This is the implementation that should be lifted into a shared util — not the `validate.rs` inline block.

**Reasoning:** The defect is real and reproducible; the plan's bookkeeping is not. All four line numbers are stale by 8–28 lines, "three identical" is false (2+1), and the designated reference implementation is both mislocated and inferior to two existing helpers the plan does not know about.

**If the fix were applied as written:** An implementer who navigates to `bash.rs:134`, `remember_this.rs:94` and `annotate_file_tool.rs:99` finds unrelated code at all three and must re-derive the sites — the plan actively misdirects. An implementer who copies "the correct implementation" from `validate.rs:846-853` copies a `pyright`/`mypy` dispatcher. If they instead copy the block at 945-957, they propagate the off-by-one and the unsafe `unwrap_or` into three more call sites. And a fix templated on `&x[..N]` leaves **`bash.rs:162` still panicking**, because that site slices a computed *tail*, not a fixed head — this is the site most likely to fire in the field.

---

## P3-05 · Four paths in the agent loop return without emitting a terminal event
**Verdict:** OVERSTATED — it is **three**, not four, unless `agent:budget-exceeded` is counted as terminal (it is not, at the consumer). The *effect* the claim describes is real and confirmed at the frontend.

**Evidence — the complete exit inventory of `AgentEngine::run` (`autodev-core/src/agent/engine.rs`):**

| line | exit | terminal event emitted? |
|---|---|---|
| 127 | `return Err` — cancelled before start | **none** — no `emit_agent_error`, no `emit_agent_done`, no `record_spend` |
| 327 | `return Err(BudgetExceeded)` | `emit_budget_exceeded` (312) + `record_spend` (326). **No `agent:done` / `agent:error`.** |
| 333 | `break` — `turn >= MAX_TOOL_TURNS` | falls through to `emit_agent_done` (804) |
| 385 | `break` — cancelled mid-turn | falls through to `emit_agent_done` (804) |
| 578 | `return Err(e)` — stream error | **`emit_agent_error` (571)** yes + `record_spend` (577) |
| 597 | `break` — no tool calls (normal finish) | falls through to `emit_agent_done` (804) |
| 627 | `return Err(e)` — tool-batch failure | `record_spend` (626) only. **none** |

Verbatim, the two clean misses:

`engine.rs:126-129`
```rust
        if self.state.lock().unwrap().is_cancelled(session_id) {
            return Err(AutoDevError::Agent(
                "Agent was cancelled before start".into(),
            ));
        }
```

`engine.rs:623-629`
```rust
                match tool_executor.execute_batch(tool_batch).await {
                    Ok(r) => r,
                    Err(e) => {
                        self.record_spend(session_id, model, &total_usage);
                        return Err(e);
                    }
                }
```

**The caller does not compensate.** `src-tauri/src/commands/agent_commands.rs:219-221`:
```rust
            Err(e) => {
                tracing::error!("Agent error for session {session_id_c}: {e}");
            }
```
That is the entire error handling for the Mode 1 turn. No event is emitted on any `Err` from `run`.

**The consequence is confirmed at the consumer.** `agent:done` and `agent:error` are the **only** two events that clear the streaming flag. `src/stores/agentStore.ts:247` (done) and `agentStore.ts:259` (error) both end with `isStreaming: false`; nothing else in `agentStore.ts` sets it false. `agent:budget-exceeded` is listened for only in `src/components/budget/BudgetBar.tsx:81` and `src/stores/taskStore.ts:263` — neither touches `agentStore`. So a run that exits at line 127, 327 or 627 leaves the chat spinner stuck on forever.

**Reasoning:** Three, not four. The claim most likely counted `engine.rs:578` (stream error) as a miss — it is not; it emits `agent:error` at line 571. A plan that says "find four" invites an implementer either to stop early having found three, or to manufacture a fourth.

Note also that line 385 (cancel mid-turn) emits `agent:done` — a *cancelled* run reports success with a cost figure. That is a distinct defect the claim does not raise.

**If the fix were applied as written:** Adding `emit_agent_error` at the three real sites does fix the stuck spinner. But an implementer hunting a fourth will most likely land on `engine.rs:578`, which already emits — producing a **duplicate** `agent:error`. In the current store that is near-harmless (`isStreaming: false` twice; the text mutation at `agentStore.ts:255-257` is guarded on `!last.content` so the second is dropped) but it is a latent inconsistency. The real omission from the fix list is `agent_commands.rs:219`, the one place that sees *every* failure — including ones `run` cannot report on, such as a panic.

---

## P3-06 · The code persists only `history.last()` rather than the whole `Vec<Item>` with usage
**Verdict:** CONFIRMED — and understated. It is worse than "only the last message": on two of the three normal exit paths it persists **nothing at all**, and it drops tool_use blocks and all but one text block from the message it does persist.

**Evidence:** `src-tauri/src/commands/agent_commands.rs:181-215` is the *only* place an assistant message is persisted. `AgentEngine` never calls `append_message` for assistant messages — grep of `autodev-core/src/agent/engine.rs` for `append_message` returns zero hits; the engine's `session_mgr` is used solely to hand a `SessionManager` to the `ToolExecutor` at `engine.rs:278`, which persists tool messages only.

```rust
                if let Some(last_msg) = updated_history.last() {
                    if last_msg.role == "assistant" {
                        let mut assistant_msg = Message::new_assistant(&session_id_c);
                        // Extract text content from content blocks
                        for block in &last_msg.content {
                            if let crate::provider::types::MessageContent::Text { text } = block {
                                assistant_msg.content = text.clone();
                            }
                        }
```

Four separate losses in ten lines:

1. **Only the last message.** `updated_history.last()`. Every assistant turn before the final one is discarded.
2. **On two exit paths, nothing is persisted at all.** The `break`s at `engine.rs:333` (`turn >= MAX_TOOL_TURNS`) and `engine.rs:385` (cancelled mid-turn) are both reached at the *top* of the loop, after `engine.rs:687-690` pushed the tool-results message:
   ```rust
            history.push(RequestMessage {
                role: "user".to_string(),
                content: tool_results,
            });
   ```
   So `history.last()` is a **user** message, `last_msg.role == "assistant"` is false, and the whole block is skipped. A run that hits the 20-turn ceiling saves no assistant output whatsoever.
3. **Assignment, not accumulation.** `assistant_msg.content = text.clone()` inside a loop over blocks. A response with two text blocks persists only the second. (Multi-block text responses are ordinary — `assemble_in_wire_order` at `engine.rs:1062` exists precisely because they interleave with tool calls.)
4. **`MessageContent::ToolUse` blocks are silently dropped.** Only `Text` is matched. The persisted transcript therefore cannot be replayed: `build_history` at `agent_commands.rs:261-279` reconstructs every stored message as a single `Text` block, so tool calls vanish from the reloaded conversation entirely.

**The "with usage" half is also confirmed.** `Message` has the fields and they are never written. `autodev-core/src/session/types.rs:67-82`:
```rust
    pub fn new_assistant(session_id: &str) -> Self {
        Self {
            ...
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: 0.0,
```
Nothing between `Message::new_assistant` and `append_message` sets any of the three. Per-message usage is structurally present and permanently zero. The run-level totals do reach `record_spend` and analytics (`engine.rs:767`, `engine.rs:790-800`), so spend is not lost — but it is not attributable to a message.

**Nomenclature note:** the claim says `Vec<Item>`. There is no `Item` type in this repo; the type is `Vec<RequestMessage>` (`autodev-core/src/provider/types.rs`). `Item` is OpenAI Responses-API vocabulary, which suggests this claim was written against a different design and never re-grounded.

**If the fix were applied as written:** Persisting the whole `Vec<RequestMessage>` fixes losses 1 and 3 but, on its own, **breaks the reload path**. `build_history` (`agent_commands.rs:261-279`) flattens every stored message to one `Text` block and drops `MessageRole::Tool` rows entirely:
```rust
                MessageRole::Tool => return None, // Tool results folded into user messages
```
Persisting assistant messages that contain `tool_use` blocks without simultaneously teaching `build_history` to round-trip `tool_use`/`tool_result` pairs would reconstruct assistant turns whose tool calls have no matching results — a hard 400 from Anthropic on the next send. The fix is only safe if `Message`, the sessions schema, and `build_history` are changed together. The plan mentions none of that.

---

## P3-07 · The code can append an assistant message with empty content, and doing so "poisons every later send in that session"
**Verdict:** CONFIRMED — both halves. A partial fix has already landed and **cites this very claim**, but it closes the narrower hole while making the wider one more likely, and it does not touch the persisted path at all.

**Evidence — half one: it can happen.** `engine.rs:588-592` pushes unconditionally:
```rust
            // Add assistant message to history
            history.push(RequestMessage {
                role: "assistant".to_string(),
                content: response_content.clone(),
            });
```
`response_content` is whatever `stream_response` returned. `stream_response` builds it from nothing but `assemble_in_wire_order`:
```rust
        content_blocks.extend(assemble_in_wire_order(text_by_block, tool_use_map));

        Ok(content_blocks)
```
(`engine.rs:918-920`; `content_blocks` is initialised empty at `engine.rs:855` and never appended to elsewhere.) A stream that carries only `ThinkingDelta`s, or only empty text, yields an **empty `Vec`**. There is no `is_empty()` guard anywhere between the two.

**A partial fix has already landed, and names this claim.** `engine.rs:1069-1075`:
```rust
    for (idx, text) in text_by_block {
        // An empty text block is dropped rather than emitted: an empty content
        // block poisons every later send in the session (P3-07's defect).
        if !text.is_empty() {
            ordered.insert(idx, MessageContent::Text { text });
        }
    }
```
This removes empty *blocks* — which strictly **increases** the chance `assemble_in_wire_order` returns an empty *Vec*, i.e. it converts the "empty text block" failure into the "empty content array" failure. The guard that would catch that (at `engine.rs:589`) was not added.

**Evidence — half two: the consequence, in-run.** `autodev-core/src/provider/anthropic.rs:96-124` serializes with no empty guard:
```rust
                let content: Vec<serde_json::Value> = m
                    .content
                    .iter()
                    .map(|c| match c {
                        MessageContent::Text { text } => json!({ "type": "text", "text": text }),
                        ...
                    })
                    .collect();
                json!({ "role": m.role, "content": content })
```
An empty `Vec` produces `"content": []`, which the Messages API rejects. `history` is cloned into every subsequent request (`engine.rs:392`, `messages: history.clone()`), so once one empty assistant message is in it, **every remaining turn of that run fails**.

**Evidence — half two, the part not yet fixed: it survives to the database.** There is no empty guard on the persistence path either.
- `agent_commands.rs:184` — `Message::new_assistant` starts with `content: String::new()`; the `for` loop finds no `Text` block, so it stays empty.
- `agent_commands.rs:213` — `session_mgr_clone.append_message(&assistant_msg)` is called regardless.
- `autodev-core/src/session/manager.rs:60-62` — `append_message` is a bare pass-through with no validation:
  ```rust
      pub fn append_message(&self, msg: &Message) -> Result<()> {
          self.storage.lock().unwrap().insert_message(msg)
      }
  ```
- On the **next** `send_message`, `build_history` (`agent_commands.rs:271-276`) reconstructs it as an empty *text block*:
  ```rust
              Some(RequestMessage {
                  role: role.to_string(),
                  content: vec![MessageContent::Text {
                      text: msg.content.clone(),
                  }],
              })
  ```
  which `anthropic.rs:100` serializes as `{"type":"text","text":""}` — also rejected.

So the claim's exact words — "poisons every later send in that session" — describe the **persisted** path, and that path is untouched by the fix already in the tree. The empty row is durable: it survives app restart, and there is no code path that removes or repairs it.

**If the fix were applied as written:** A guard at `engine.rs:589` (skip the push when `response_content.is_empty()`) stops the in-run failure and stops new empty rows being created. It does **not** repair sessions already poisoned, and it does not add the defence-in-depth guard at `append_message`/`build_history` that would make an already-stored empty row survivable. Any fix that stops at the engine leaves existing broken sessions permanently unusable with no diagnostic beyond a raw provider 400.

---

## P3-08 · No `CancellationToken` threaded into the provider call, the tool executor, or pending-permission waits, and these lack timeouts
**Verdict:** CONFIRMED for cancellation; **PARTIALLY CONFIRMED** for timeouts — two of the three named sites have no timeout, but the claim is wrong that all three do, and it misses that a timeout *does* exist one level up for subagents.

**Evidence — no `CancellationToken`, and none is possible today.** Repo-wide grep for `CancellationToken`, `tokio_util`, `tokio-util` across `autodev-core/`, `src-tauri/`, `src-cli/`, `src-shared/` (both `.rs` and `Cargo.toml`) returns **zero hits**. `tokio-util` is not a declared dependency of any crate, so the type is not merely unused — it is unavailable.

**What cancellation actually is today:** a boolean polled at exactly two points in `AgentEngine::run`, both *between* turns:
- `engine.rs:126` — before the first turn
- `engine.rs:383` — at the top of each subsequent turn
```rust
            if self.state.lock().unwrap().is_cancelled(session_id) {
```
Nothing checks it during a provider stream, during `execute_batch`, or during a permission wait. Cancelling mid-stream therefore does nothing until the current turn completes — including the whole billed provider call.

**Timeouts, site by site:**

| site | timeout? |
|---|---|
| provider call | **NONE.** `anthropic.rs:32`, `openai.rs:30`, `google.rs:29` all construct `client: Client::new()`. `reqwest::Client::new()` sets no request timeout and no connect timeout. A hung TLS handshake or a stalled SSE stream blocks forever. |
| tool executor (batch/framework) | **NONE** at the executor level — `tool_executor.rs` contains no `timeout(` call. |
| tool executor (individual tools) | **PARTIAL — the claim is wrong here.** `bash.rs:124-128` wraps the command: `timeout(Duration::from_secs(timeout_secs), run_command(shell_cmd, &ctx.working_dir))`. So shell tools are bounded; other tools are not. |
| pending-permission wait | **NONE.** `tool_executor.rs:567`: `match rx.await {` — a bare `tokio::sync::oneshot` receive with no timeout, no cancellation, no `select!`. If the dialog is dismissed without a response, the frontend reloads, or `respond_to_permission` is never called, the turn hangs forever holding a slot. |

**What the claim misses:** a timeout *does* exist for subagents, one level above these sites — `autodev-core/src/subagent/native_runner.rs:220-234`:
```rust
    let history = match timeout(
        Duration::from_secs(config.timeout_secs.max(1) as u64),
        run_future,
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            return Err(AutoDevError::Agent(format!(
                "Subagent exceeded its {}s timeout",
                config.timeout_secs
            )))
        }
    };
```
The **Mode 1 interactive path has no such wrapper** — `agent_commands.rs:134` is a bare `tokio::spawn` with no timeout. So the asymmetry is: an autonomous subagent (which never prompts for permission, `PermissionMode::Autonomous` at `native_runner.rs:197`) is time-bounded, while the interactive path (the only one that *can* block on a permission dialog) is not. That is exactly backwards.

**If the fix were applied as written:** Threading a `CancellationToken` requires first adding `tokio-util` as a workspace dependency — the plan does not say so, and it is a new-dependency decision (Rule 4: flag and propose). Threading it through the provider call also requires changing the `Provider` trait signature (`autodev-core/src/provider/traits.rs`), which all three adapters, `CappedProvider`, and the conformance test suite implement — a shipped-contract change, not a local edit. The *cheap* fixes the plan does not name are strictly better first moves: a `reqwest::ClientBuilder` timeout at the three `Client::new()` sites, and a `tokio::time::timeout` around `rx.await` at `tool_executor.rs:567`. Both are one-line, need no new dependency, and close the two hangs that can actually occur today.

---

## P3-09 · `ErrorClass::Retryable` exists but there is no retry policy implemented for it (GAP-12)
**Verdict:** CONFIRMED — and the codebase says so itself, in two places, in comments dated the day before this verification.

**Evidence — the enum exists.** `autodev-core/src/provider/usage.rs:213-231`:
```rust
/// How an error should be treated, per spec `02 §3`.
///
/// ⚠ `QuotaExhausted` is **explicitly non-retryable**. A spend-cap 429 carries
/// no `retry-after` and never succeeds on retry; treating it as `Retryable`
/// turns a hard stop into a spend loop against a cap that already fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClass {
    Retryable,
    /// Anthropic returns **529** for this, not 503.
    Overloaded,
    QuotaExhausted,
    Deferred,
    Fatal,
}

impl ErrorClass {
    pub fn is_retryable(&self) -> bool {
        matches!(self, ErrorClass::Retryable | ErrorClass::Overloaded)
    }
```

**Evidence — no retry policy.** Grep for `ErrorClass` across `autodev-core/src`, `src-tauri/src`, `src-cli`, `src-shared` returns 6 production call sites, all identical in shape, all in the three adapters — and **`is_retryable()` has zero production callers**; its only references are the assertions at `usage.rs:544-547`, `747`, `765`. There is no `RetryPolicy`, no backoff, no attempt counter, no `tokio::time::sleep` in `autodev-core/src/provider/`.

**Worse: the classification is computed and then thrown away as a type.** `autodev-core/src/provider/anthropic.rs:230-238` (the same shape at `anthropic.rs:265`, `openai.rs:290`/`324`, `google.rs:235`/`275`):
```rust
            let class = crate::provider::usage::ErrorClass::classify(
                "anthropic",
                status.as_u16(),
                &text,
            );
            return Err(AutoDevError::Provider(format!(
                "Anthropic {status} [{class:?}]: {text}"
            )));
```
`AutoDevError::Provider` is `Provider(String)` (`autodev-core/src/errors.rs:14-15`). The `ErrorClass` is `Debug`-formatted into a message and discarded. **Any future retry loop must recover it by string-matching `[Retryable]` out of an error message** — the exact re-derivation the classifier was written to prevent.

**The code agrees with the claim, twice.** `usage.rs:243-247`:
```
    /// No retry loop exists yet, so nothing was *looping* on the wrong
    /// classification — but the first one written would have had to invent
    /// this mapping from scratch, at the moment when getting it wrong costs
    /// money.
```
`src-tauri/src/ci_gate.rs:1821-1823`:
```
    /// Nothing was looping on the wrong answer, because no retry logic exists
    /// yet — which is exactly why this gate matters now.
```

**A related weakness in the gate that guards this.** `ci_gate.rs:1837-1838`:
```rust
            let blocks = text.matches("if !status.is_success()").count();
            let classified = text.matches("ErrorClass::classify").count();
```
This is string counting, not parsing. It cannot tell a classification inside an error path from one in a comment or a test, it cannot tell whether the classified value is *used*, and `classified < blocks` passes when both are equal even if the classify calls and the error blocks are in unrelated functions. It is green today for the right reason, but it would stay green if someone kept the `classify` calls and deleted the error paths, or vice versa.

**If the fix were applied as written:** Adding a retry loop keyed on `ErrorClass` cannot be done at the call sites as they stand — the type does not cross the error boundary. The prerequisite edit is to `AutoDevError`: either a new variant carrying `ErrorClass`, or a field on `Provider`. That is a change to a shipped error contract consumed by every Tauri command's IPC serialization (`errors.rs:65-72` serializes with `self.to_string()`), so changing the variant shape changes what the frontend receives. The plan treats this as "implement a retry policy"; it is really "widen the error type first, then implement a retry policy", and the first half is the part that needs a decision.

---

# Verdict summary

| claim | verdict |
|---|---|
| P3-01 `panic = "abort"` set, should be `"unwind"` | **REFUTED** — the block is in a non-root workspace member and is discarded by Cargo. Unwinding is already in effect; the stated fix is a no-op. |
| P3-02 no `catch_unwind` anywhere | **CONFIRMED** literally; misleading — `tokio::spawn` already isolates the spawned paths. |
| P3-03 a panicking worker takes down the app | **REFUTED as stated** — the process survives. True for a worse reason: `Mutex<AppState>` poisoning (93/98 lock sites `.unwrap()`) plus a leaked `is_running` flag. |
| P3-04 three identical panics + correct impl at `validate.rs:846-853` | **PARTIALLY CONFIRMED** — all three panics reproduce, but all four line numbers are wrong, they are not identical (2+1), and the reference implementation is at 945-957 and is inferior to two existing helpers the plan never mentions. |
| P3-05 four paths return without a terminal event | **OVERSTATED** — three, not four. Consequence (stuck spinner) confirmed at the consumer. |
| P3-06 persists only `history.last()` | **CONFIRMED**, and understated — two exit paths persist nothing at all; tool_use blocks and all but one text block are dropped; per-message usage fields exist and are never written. |
| P3-07 empty assistant content poisons later sends | **CONFIRMED** both halves. A partial fix landed citing this claim; it closes the narrow hole, widens the broad one, and does not touch the persisted path. |
| P3-08 no `CancellationToken`, no timeouts | **CONFIRMED** for cancellation (`tokio-util` is not even a dependency); **PARTIALLY** for timeouts — `bash.rs` tools *are* bounded, subagents *are* bounded, the provider call and permission wait are not. |
| P3-09 `ErrorClass::Retryable` with no retry policy | **CONFIRMED**, and the code says so in two comments. Worse: the classification is `Debug`-formatted into a `String` and discarded as a type. |

Two of nine claims are wrong as written (P3-01, P3-03). Two are miscounted or mislocated in ways that will misdirect an implementer (P3-04, P3-05). Five are substantively correct, three of those understated.

---

# What I could not determine

**1. Whether the empty-content path (P3-07) actually fires against any shipped provider.**
I proved it is *reachable* in the code and that nothing guards it, but not that any of Anthropic, OpenAI or Google ever emits a stream with zero text blocks and zero tool-use blocks. There is no harness for this: `autodev-core/src/provider/conformance_tests.rs` drives `build_body` and inspects the outgoing JSON; it never drives `stream_response` from a synthetic chunk sequence. A grep of all four test files for `content: vec![]` / `content: Vec::new()` returns nothing — **no test anywhere constructs an assistant message with empty content**.
*Measurement that would settle it:* add a test that feeds `stream_response` a chunk sequence of `[ThinkingDelta, MessageDelta{stop_reason:"end_turn"}, Done]` and asserts the returned `Vec<MessageContent>` is empty; then feed that empty message to each adapter's `build_body` and assert on the serialized `content` field. Both are offline and cost nothing.

**2. Whether the mutex-poisoning cascade in P3-03 is reachable, or only theoretical.**
Poisoning requires a panic *while a guard is alive*. In `engine.rs:126` and `:383` the guard is a temporary that drops at the end of the statement, so only a panic inside `AppState::is_cancelled` itself would poison — narrow. But there are 17 named guards held across multiple statements (`agent_commands.rs:33/62/110/172/232`, `settings_commands.rs:98/138/147/156/162/171`, `capped.rs:71`, `recipes/runner.rs:456`, and others), and I did not audit each for a fallible operation inside the guard's scope.
*Measurement:* for each named-guard site, check whether any indexing, `unwrap`, `expect`, or arithmetic occurs between the `lock()` and the guard's drop. A single one makes the cascade reachable.

**3. The shipped cost of the inert release profile.**
`lto`, `opt-level = "s"`, `codegen-units = 1` and `strip` are all discarded along with `panic`. I did not measure what that costs.
*Measurement:* `cargo build --release` as-is, record binary size and build time; move the block to the workspace root; rebuild; compare. This is the number that decides whether P3-01's real fix (relocating the block) is a size win worth the build-time cost, or a regression.

**4. ~~Whether `cargo check --workspace --all-targets` is green.~~ — RESOLVED.**
Run to completion: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 2m 10s`, **exit code 0**. Warnings only (13 in `autodev` lib, e.g. `unused_mut` at `src-tauri/src/commands/vault_commands.rs:248`, `dead_code` on `redis_process` at `src-tauri/src/sidecar/mod.rs:431`). So every defect in this report is a **runtime** defect against a tree that compiles clean — none of them will surface as a build failure, which is precisely why they have survived.

---

# Claims I would add

These are defects in the same area that the plan does not mention. Ordered by severity.

**A-01 — A panicking read-only tool orphans its `tool_use` block and 400s the next request.** (severity: high)
`tool_executor.rs:288-292` pre-fills every result slot with an **empty** tool_use_id:
```rust
        results.resize(
            read_tools.len() + write_tools.len(),
            (String::new(), ToolResult::error("placeholder")),
        );
```
On a panic, `tool_executor.rs:389-392` logs and leaves the placeholder in place — the comment admits it is unfinished:
```rust
                    Err(e) => {
                        error!("Read-only tool task panicked: {}", e);
                        // Placeholder will be replaced; we need to find which one failed
                    }
```
Downstream in `engine.rs:657` that placeholder is inserted under key `""`; `engine.rs:663` `by_id.remove(id)` then misses the real id, so **no `tool_result` is emitted for that `tool_use`**, and `engine.rs:676-683` appends a `tool_result` with an empty `tool_use_id`. The next request therefore carries an unanswered `tool_use` — a hard 400 from Anthropic. And because `by_id` is a `HashMap<String, _>`, **two or more panicking read-only tools collide on the `""` key and all but one are silently dropped**. Concurrency is up to 10 (`tool_executor.rs:24`), so this is not exotic. This is the concrete, already-reachable version of P3-07's "poisons the session", and it is exactly the failure a `catch_unwind` fix would *not* address.

**A-02 — 98 `.lock().unwrap()` sites, 5 poison-tolerant.** (severity: high)
See P3-03. Only `autodev-core/src/cache/monitor.rs` (4 sites) and `autodev-core/src/db/test_support.rs` (1) use `unwrap_or_else(|e| e.into_inner())`. Every other lock turns one panic into a permanent app-wide failure.

**A-03 — `is_running` is never cleared on a panicking turn.** (severity: high)
`agent_commands.rs:171-177` clears the flag *after* the `.await`. A panic inside `engine.run` skips it and the session is permanently unusable. Belongs in a `Drop` guard.

**A-04 — The entire `[profile.release]` block is inert, not just `panic`.** (severity: medium-high)
`lto = true`, `codegen-units = 1`, `opt-level = "s"` and `strip = true` at `src-tauri/Cargo.toml:119-122` are discarded with the same Cargo warning. Every shipped release build is un-LTO'd, `opt-level = 3` rather than `"s"`, 16 codegen units, and unstripped — including debug symbols in the shipped binary.

**A-05 — All three provider clients have no timeout at all.** (severity: medium-high)
`anthropic.rs:32`, `openai.rs:30`, `google.rs:29` are all `client: Client::new()`. `reqwest::Client::new()` sets neither a request nor a connect timeout. Combined with cancellation being polled only between turns (`engine.rs:126`, `:383`), a stalled SSE stream hangs the turn forever with no user-reachable escape — and the Mode 1 path has no outer timeout either (`agent_commands.rs:134` is a bare `tokio::spawn`). This is a two-line fix that the P3-08 `CancellationToken` work would otherwise defer behind a trait-signature change.

**A-06 — `build_history` cannot round-trip a tool-using conversation.** (severity: medium-high)
`agent_commands.rs:261-279` drops every tool row and flattens everything else to one text block:
```rust
                MessageRole::Tool => return None, // Tool results folded into user messages
```
The comment is false — nothing folds them in. `ToolExecutor` faithfully persists tool messages (`tool_executor.rs:384`), and `build_history` then discards them on reload. So a resumed session presents the model with a history that omits every tool call and every tool result it made. This must be fixed *with* P3-06, not after it: persisting the full `Vec<RequestMessage>` without fixing `build_history` produces assistant messages with `tool_use` blocks and no matching results.

**A-07 — A cancelled run reports success.** (severity: medium)
The `break` at `engine.rs:385` (cancel detected mid-run) falls through to `emit_agent_done` at `engine.rs:804`, which carries `tokens_in`/`tokens_out`/`cost_usd`. The frontend (`agentStore.ts:234-248`) cannot distinguish it from a completed turn. Same for `MAX_TOOL_TURNS` exhaustion at `engine.rs:331-333`, whose only signal is an `error!` log line — the user sees a normal completion of a run that was cut off mid-task, and (per P3-06) nothing is persisted from it.

**A-08 — `validate.rs:945-957` has an off-by-one and an unsafe fallback.** (severity: low, but it is the block P3-04 designates as the reference)
`.take_while(|&i| i < MAX).last()` returns the *start* index of the last char beginning before MAX, so it always drops one char more than needed (measured: max=10 on ASCII keeps 9 bytes). And `.unwrap_or(MAX_CHECKER_BYTES)` would be a raw byte index — the very bug being avoided. Unreachable today, wrong to copy.

**A-09 — `truncate` is duplicated verbatim in two modules and private in both.** (severity: low)
`autodev-core/src/memory/recall.rs:183-192` and `autodev-core/src/memory/summarizer.rs:148-157` are byte-identical and correct. Neither is `pub`. The P3-04 fix should lift one of these into a shared util and delete the other, rather than writing a fourth implementation.

**A-10 — `ci_gate.rs:1837-1838` counts strings instead of parsing.** (severity: low)
```rust
            let blocks = text.matches("if !status.is_success()").count();
            let classified = text.matches("ErrorClass::classify").count();
```
Cannot distinguish a classify call in an error path from one in a comment or a test, and cannot tell whether the classified value is used at all — which is precisely the state the code is in today (P3-09: the value is `Debug`-formatted into a string and discarded). The gate is green while the property it names is only half-true.

**A-11 — Stale comment at `src-tauri/Cargo.toml:88`.** (severity: low, but it is why P3-02 may have been written)
```
# Task D3: Keep 0.3.4; panic prevention via D1 (sanitized OLLAMA_HOST) + D2 (catch_unwind)
```
There is no `catch_unwind` in the repo, and the Ollama dependency this annotates has been removed. A comment that claims a defence which does not exist is a Rule 5 bug.

---

_End of report._
