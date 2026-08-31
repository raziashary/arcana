# 01 — The Agent Runtime

| | |
|---|---|
| **Parent** | `00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md` §Part 1 (INV-1, INV-9), §Part 1B (M-01, M-02, M-08, M-09), §Part 3.1, §Part 5 (S3, S6) |
| **Locks** | Hands **ADR-01-01**, **ADR-01-02**, **ADR-02-03** to `00 §Part 8` |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G1 §A.2, §D.1, §C.4` · `G2a` (57 findings) · `G2b` (subagent section) · `G4 §1.1–1.15` (LivingAgent loop) |

---

## What this is

The loop that turns a user sentence into work: assemble context, call a model, execute the tools it asks for, feed results back, repeat until done. AutoDev already has one — `G1 §D.1` assesses `agent/engine.rs` as **complete for the single-agent case**. This spec is therefore mostly about what surrounds it, and about the four subtle ways it is currently gotten wrong.

**The subtle failures this subsystem is prone to — each maps to a decided ADR:**

1. **A loop that ends without saying so.** Four exit paths in the current engine return without emitting a terminal event, so the UI spins forever while the Rust task is already dead. Termination is not "the loop returned" — it is "the consumer was told".
2. **Guards that are structurally inert.** `ToolContext.depth` and `.agent_role` are hardcoded at all three construction sites, which silently disables the subagent recursion guard, ACP sender identity, and the `qc_review` build-evidence gate. Every one of those *looks* implemented in code review. This is why INV-8's reader-exists discipline exists.
3. **Two execution paths where one is correct.** `ToolExecutor::execute()` is the only fully-correct path and has **zero callers**. Divergent copies drift, and the drift is invisible until a gate that exists on one path is missing on the other.
4. **A blocking wait with no timeout.** The permission oneshot blocks the engine forever if its listener died — and a tab switch is enough to kill the listener.

---

## 1. First principles

**The runtime is a library, not an application service.** It takes a request and a tool registry and produces a turn. It does not know about Tauri, `AppState`, or the webview. This is not architectural purity for its own sake: ADR-01-02 asks whether `airsctl` — a separate process — can run agents, and the answer is only "yes" if the runtime is linkable. It also makes the runtime testable without a UI, which is the precondition for the conformance suites in spec `10`.

**One loop serves every role.** PA, Supervisor, Worker, QC reviewer and subagent are the *same* engine with different personas, tool sets, and permission modes. The sidecar's `LivingAgent` conflated the loop with the persona and grew 21 FSMs around it (`G4`); the Rust engine already keeps them separate and should stay that way.

**Every turn is accounted.** A turn that runs and reports no usage is a turn that will be mispriced (INV-7).

---

## 2. Core DTOs

Frozen; unknown fields rejected.

```rust
pub struct Turn {
    pub session_id: SessionId,
    pub items:      Vec<Item>,
    pub usage:      Usage,
    pub stop:       StopReason,
}

pub enum Item {
    User      { content: Vec<Content> },
    Assistant { content: Vec<Content> },
    ToolCall  { id: ToolCallId, name: String, args: serde_json::Value },
    ToolResult{ id: ToolCallId, content: String, is_error: bool },
    /// Provider reasoning state we carry but never interpret.
    Opaque    { provider: ProviderId, kind: OpaqueKind, bytes: Vec<u8> },
}

pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,        // truncation — MUST be distinguishable
    MaxToolTurns,
    Cancelled,
    Error(ErrorClass),
}
```

**`Item::Opaque` exists from day one.** Four providers carry unreplayable reasoning state under four names (Anthropic thinking `signature`, OpenAI `encrypted_content`, Gemini thought signatures, xAI `reasoning_content`). Dropping it breaks turn continuity *and* is xAI's largest single cause of cache misses (`G6 §5.1`). Retrofitting it means touching every adapter twice.

**`StopReason::MaxTokens` is a distinct variant** because truncation is currently undetectable: the three cloud providers signal it as `max_tokens` / `MAX_TOKENS` / `length`, and xAI's set is undocumented (`G6 §5.5`). A truncated response that looks like a normal completion produces silently incomplete work.

---

## 3. The loop

```
run(request) -> Result<Turn>:
  1. assemble context            (spec 03)
  2. emit TurnStarted
  3. loop up to MAX_TOOL_TURNS:
       a. provider.stream()      (spec 02)
       b. forward deltas as events
       c. if stop != ToolUse: break
       d. execute tool calls     (spec 06)
       e. append results to history
  4. persist the turn
  5. emit TurnEnded { stop, usage }        ← unconditional
```

### 3.1 Termination is a contract

**Step 5 runs on every path, including errors, cancellation and turn-limit exhaustion.** Implemented as a guard object whose `Drop` emits `TurnEnded` if nothing else did — the same reasoning as the subagent permit in §5, and for the same reason: release-on-the-happy-path is the bug.

Today's violations, all `CODE-PRESENT`: `engine.rs:299` (`?` on `execute_batch` returns `Err` with no event), `engine.rs:164` (budget-exceeded emits only a budget event nobody listens for), `engine.rs:92` (pre-loop cancel returns early), `agent_commands.rs:134-137` (unsupported provider returns from inside `tokio::spawn` with **zero** events, leaving a phantom running agent).

### 3.2 Persistence is not "the last message"

`agent_commands.rs:169-177` saves only `updated_history.last()`, and only if it is an assistant message. Consequences: on `MAX_TOOL_TURNS` or cancel the last entry is a tool-results *user* message, so **nothing is persisted and the entire turn evaporates on reload**; every mid-loop assistant narration is lost; and `Message::new_assistant` stores `tokens_in/out = 0`, so reloaded turns show no cost.

Worse, if a run ends on a tool-use-only assistant block, an assistant `Message` with `content: ""` is appended — and `build_history` then emits an empty text block, which Anthropic rejects. **Every subsequent send in that session fails.** That is a plausible mechanism for "chat works in a new session but not an old one".

**The rule: the whole `Vec<Item>` is persisted, item by item, with its usage.**

### 3.3 Cancellation

`cancel_agent` sets a bool checked between turns (`app_state.rs:44-48`, `engine.rs:188`). That is insufficient: it does not interrupt an in-flight provider stream, and it does not drain `PendingPermissions`, so cancelling during an approval wait leaves the engine blocked forever while the UI shows idle.

**The rule: cancellation is a `CancellationToken` threaded into the provider call, the tool executor, and every pending-permission wait. Every one of them has a timeout as well** (INV-9).

---

## 4. `ToolContext` — truthful or not at all

```rust
pub struct ToolContext {
    pub session_id:  SessionId,
    pub working_dir: PathBuf,      // resolved, never "" — spec 04
    pub agent_role:  AgentRole,    // NOT Option
    pub depth:       u8,           // real spawn depth
    pub permission:  PermissionMode,
    pub cancel:      CancellationToken,
}
```

Today `depth` is `0` and `agent_role` is `None` at all three construction sites (`tool_executor.rs:88-89, 251-252, 315-316`). The blast radius:

| Consumer | Believes | Actually |
|---|---|---|
| Subagent recursion guard | depth-limited | **never trips** — unbounded nesting |
| ACP worker messaging | knows the sender | sender identity absent; self-message guards inert |
| `qc_review` build-evidence gate | role-gated | **skipped entirely on the native path** |
| ACP tools | role-appropriate | permanently self-refusing |

**`agent_role` is not `Option`.** If a construction site cannot name the role, that is a bug at the construction site, and the type should force it to be fixed rather than allow `None` to paper over it.

---

## 5. Subagents

**Decided (ADR-02-03): subagent capability lands natively in `src-tauri`.** Today `subagent/bridge.rs` is 100% sidecar — `create_ephemeral_agent`, `invoke_agent_stream_raw` and `delete_agent` are all HTTP with no local path — so deleting the sidecar without this port kills `spawn_subagent`, `spawn_parallel_subagents`, `spawn_qc_review` and Delegate mode outright.

**Design:** a subagent is an `AgentEngine` on a tokio task, with:
- a **semaphore-bounded pool**, permit released by `Drop`. The current leak (`G2b S-03`) exists precisely because `terminated_at` is set only in `await_result`, so the fire-and-forget Tauri path leaks a permit every time — **after 16 spawns all spawning deadlocks silently**;
- **`depth + 1`** from the parent's real `ToolContext`, with a hard cap (the sidecar used `MAX_CALL_DEPTH = 3`, `G4b §A.12`);
- a **timeout** and propagated cancellation;
- events on the same channel the UI already subscribes to, so the Live Monitor sees subagent activity without a special case.

**Coupled deletion warning:** `bridge.rs`'s `find_frame_end`/`parse_event_frame` are `pub(crate)` and reused by `sidecar::client::invoke_worker_streaming`. Those two removals cannot be sequenced independently — noted in `00 §Part 6` as a P8 constraint.

---

## 6. Events

One taxonomy, emitted by the runtime, consumed by the UI (spec `07`) and the CLI.

`TurnStarted` · `TextDelta` · `ThinkingDelta` · `ToolCallStarted` · `ToolCallFinished` · `PermissionRequested` · `TurnEnded` · `Error`

Two current defects this fixes: **`agent:thinking` is emitted on every thinking delta and has no listener anywhere in `src/`** — extended-thinking models stream reasoning into a black hole while the UI shows an empty bubble (`G5a §1`); and five `agent:*` names the frontend listens for are emitted by no Rust code.

**Subscription lifetime belongs to the session, not the tab** (spec `07`, M-21). The runtime's obligation is to emit; the UI's obligation is to stay subscribed.

---

## 7. Determinism balance

| Step | Balance |
|---|---|
| Context assembly | `DET` + `MATH` |
| Provider call | `ORGAN` — the only one |
| Tool dispatch and execution | `DET` |
| Permission decision | `DET` |
| Persistence and usage accounting | `DET` |

**One model call per iteration, and nothing else in this subsystem is an organ.**

---

## 8. Test gates

Harvested 1:1 into `TESTING_GATES.md`.

| Gate | Asserts | Catches |
|---|---|---|
| **G-TERM** | Every exit path emits exactly one `TurnEnded`. Parameterised over: normal, tool-limit, cancel, provider error, unsupported provider, budget exceeded | `engine.rs:299/164/92`, `agent_commands.rs:134-137` |
| **G-PERSIST** | After a turn ending in each `StopReason`, reloading the session reproduces every item with its usage | The last-message-only save; the empty-content poisoning |
| **G-CANCEL** | Cancel during (a) a provider stream, (b) tool execution, (c) a pending permission — all return within the timeout and release every resource | The permission oneshot with no timeout; the undrained `PendingPermissions` |
| **G-CTX-TRUTH** | `ToolContext` fields are non-default at every construction site; a test asserts `agent_role != <unset>` and `depth` reflects real nesting | T48/T49/T50 |
| **G-DEPTH** | A subagent spawning a subagent past the cap is refused | The inert recursion guard |
| **G-POOL** | N fire-and-forget spawns beyond the pool size still permit spawn N+1 after they finish | The permit leak / 16-spawn deadlock |
| **G-ONEPATH** | Static check: exactly one tool-execution entry point has callers | `ToolExecutor::execute()` with zero callers |
| **G-TRUNC** | A response hitting `max_tokens` yields `StopReason::MaxTokens`, on all four providers | Silent truncation |
| **G-EVENTS** | Every emitted event name has a consumer; every consumed name has an emitter | `agent:thinking`; the five phantom `agent:*` |
| **G-NOSIDECAR** | The full runtime suite passes with the sidecar **absent**, not merely stopped | The P8 entry gate |
| **G-PANIC-CONTAINED** | ⚠ **ADDED at rev 0.9 (G-01, M-41).** A panicking tool or subagent fails **that turn only**; the app and every other session survive; the failure is reported, not silent | `panic = "abort"` + in-process agents — one panic killing everything |

---

> No ADR decision is recorded in this spec. It hands **ADR-01-01**, **ADR-01-02** and **ADR-02-03** to `00 §Part 8`.
