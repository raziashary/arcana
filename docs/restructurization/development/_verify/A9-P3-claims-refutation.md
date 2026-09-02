# A9 · P3 A–E — claims refutation

> Adversarial audit of every `- [x]` completion note in MDP `P3-A`…`P3-E`, run per `§0.2a` after the phase built green. The agent was instructed to REFUTE, not confirm, and had not seen the reasoning behind any of the work.
>
> **Run** 2026-09-02 · **Working tree** was not clean at read time (a parallel remediation was in flight); line numbers are working-tree numbers.
>
> ⚠ **This is the auditor's raw report, preserved unedited.** Findings acted on are recorded against their task ids in the MDP, and several were **verified in the main session before any change** — one summary claim (that `TG3-01`…`TG3-28` "do not exist") proved to be about them being *unimplemented*, not absent, which the remediation states. An audit is evidence, not a verdict.

---

# Adversarial audit — MDP P3 A–E completion claims

Repo: `C:\AIRS Foundry\autodev\autodev`
Document under audit: `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md`, sections `### P3-A` … `### P3-E`.
Method: for each `- [x]` task, read the completion note, locate the code it names, and try to break the claim.
Rules: read-only; no test suite run; no credentials.

---

## F-1 · P3-04 · OVERSTATED
**Claim:** "⚠ The gate asserts the **invariant** (no raw byte-index slicing of a string anywhere), not that those four sites were fixed. Written the other way it would pass for a fifth site added tomorrow — which is `L-C6`."
**Reality:** `TG-P3-04` (`src-tauri/src/ci_gate.rs:2678-2745`) does *not* assert "no raw byte-index slicing of a string anywhere". Its detector `regex_lite_slice()` (`src-tauri/src/ci_gate.rs:2775-2812`) flags a line only if **both**:
  1. the line contains one of **thirteen hardcoded binding spellings** — `&fact[`, `&note[`, `&output[`, `&command[`, `&text[`, `&s[`, `&content[`, `&combined[`, `&stdout[`, `&stderr[`, `&line[`, `&body[`, `&prompt[` (`ci_gate.rs:2795-2803`, comment: *"The string bindings this codebase slices, named rather than guessed"*), **and**
  2. the *same line* carries a numeric literal index or the text `len()` (`ci_gate.rs:2806-2811`).
  It also scans only `autodev-core/src`, `src-tauri/src`, `src-cli/src` (`ci_gate.rs:2679-2683`) — `src-shared/src` (5 `.rs` files, incl. `db/analytics.rs`) is never scanned.
**Demonstration:** Three separate one-line reintroductions of the exact P3-04 defect that this gate passes:
  - `&summary[..120]` — binding name not in the thirteen ⇒ `touches_a_string == false` ⇒ not flagged.
  - `let cut = fact.len().min(120); ` on one line and `&fact[..cut]` on the next — the slice line has no digit token and no `len()` ⇒ `has_literal_index == false` ⇒ not flagged. This is byte-for-byte the `sidecar/client.rs` defect (`&body[..body.len().min(400)]`) split across two lines.
  - Any of the removed sites re-added under `src-shared/src/` — outside the scanned roots.
**Severity:** medium

## F-2 · P3-04 · OVERSTATED (count)
**Claim:** "**Twelve** unsafe sites, not three: the four corrected ones, `bash.rs:415`, and **seven** in `sidecar/client.rs` the gate found."
**Reality:** The enumeration does not add to twelve without double-counting. `autodev-core/src/security/truncate.rs:8-13` lists the corrected set as exactly four *including* `bash.rs:415` (`remember_this.rs:101`, `annotate_file_tool.rs:109`, `bash.rs:162`, `bash.rs:415`). 4 + 7 = **11 sites**, not 12. Verified against the landing commit `4106d7a`: removed unsafe expressions are `&note[..80]`, `&fact[..120]`, `&command[i..]`, `output[tail_start..]` + `&output[boundary..]` (one site, two slice expressions), and 7 in `sidecar/client.rs`. Twelve is only reachable by counting *slice expressions* (12), while the sentence counts *sites*.
**Demonstration:** `git show 4106d7a -- '*.rs' | grep '^-'` yields 12 slice expressions across 11 distinct call sites; the MDP sentence enumerates 4+1+7 where the "four" already contains the "+1".
**Severity:** low

## F-3 · P3-02 · FALSE (as stated) / gate cannot express failure
**Claim:** "**P3-02** Every agent turn **and subagent** runs inside `catch_unwind`. → `UNIT-PROVEN` (`TG-P3-02`…)"
**Reality:** There is exactly **one** production `catch_unwind` in the whole tree: `src-tauri/src/commands/agent_commands.rs:237`. Subagents do **not** run inside one — `autodev-core/src/subagent/manager.rs:126-130` launches `tokio::spawn(async move { native_runner::run(...) })` with no unwind boundary; a subagent panic surfaces as a `JoinError` at `manager.rs:187`. (`autodev-core/src/agent/terminal_event.rs:169` is the only other occurrence and it is inside that file's own test module.) The gate does not check the subagent path at all — `TG-P3-02` (`src-tauri/src/ci_gate.rs:1347-1370`) reads only `agent_commands.rs`.
**Demonstration (gate cannot fail):** `TG-P3-02`'s containment assertion is `src.contains("catch_unwind")` over the **whole file text, comments included** (`ci_gate.rs:1351`). `agent_commands.rs` mentions `catch_unwind` in prose comments at `:198` and `:223`. Delete the actual wrapper at `:237` and call `engine.run(...).await` directly — the two comments keep `contains("catch_unwind")` true, `impl Drop for RunningGuard` is untouched, `bare_clears` stays at 2, and the gate **passes over the removed containment**.
**Severity:** medium (the `tokio::spawn` boundary does contain the panic in practice, so the app still survives — but the stated proof does not hold and the gate is inert against the defect)

## F-4 · P3-03 · FALSE (count) and incomplete fix
**Claim:** "**103 sites** routed through `lock_tolerant`/`read_tolerant`/`write_tolerant` — 98 mutex + 5 RwLock the first pass missed and the gate caught."
**Reality — the number:** measured on the landing commit `e2997fe`, the diff removes **98** `.lock().unwrap()`, **2** `.read().unwrap()` and **1** `.write().unwrap()` = **101 sites**, and only **3** of them are RwLock, not 5. Confirmed against the pre-image: `git show e2997fe^` contains 98 `.lock().unwrap()` and exactly 3 `.read()/.write().unwrap()` sites (all in `src-tauri/src/skills/registry.rs`). "98 mutex + 5 RwLock = 103" is not reproducible from the code.
**Reality — the fix:** the claim "one panic cannot poison the application" does not hold. **Twenty-two** production sites still propagate poisoning and the gate cannot see any of them, because `TG-P3-03` (`src-tauri/src/ci_gate.rs:1312-1319`) matches the *single-line literal* `.lock().unwrap()` / `.read().unwrap()` / `.write().unwrap()`:
  - **7 multi-line `.lock()` newline `.unwrap()` sites**, all production (no `#[cfg(test)]` exists in any of these files): `autodev-core/src/session/manager.rs:41,49,77` (the session-storage mutex — a panic under it makes `get_session` panic forever), `autodev-core/src/recipes/scheduler.rs:306`, `src-tauri/src/sidecar/client.rs:286,677,1317`.
  - **15 `.lock().map_err(...)?` sites** in `autodev-core/src/analytics/db.rs:137,151,178,231,260,300,342,376,434,474,496,647,663,710,776` — these convert poisoning into a permanent `Err` on every analytics write for the life of the process, which is the same restart-only failure the task describes, merely spelled as an error.
**Demonstration:** re-add any removed site in the multi-line form —
```
self.state
    .lock()
    .unwrap()
```
— and `TG-P3-03` passes; `rustfmt` produces exactly this form whenever the receiver expression is long, which is why 7 of them already exist.
**Severity:** high

## F-5 · P3-05 · FALSE (`GATE-PROVEN` badge) — the named gate does not exist
**Claim:** "**P3-05** … → `GATE-PROVEN` *(A `Drop` guard that **owns** the emission … routing it prevents the double emission `TG3-01` fails as loudly as zero.)*"
**Reality:** **`TG3-01` does not exist.** It is `- [ ]` (unchecked) in `docs/restructurization/development/TESTING_GATES.md:218`, and no test of that name or shape exists in `src-tauri/src/ci_gate.rs`, `autodev-core/tests/`, or anywhere else in the tree. It is nonetheless cited as an active check in **eight** places: `autodev-core/src/agent/engine.rs:170`, `autodev-core/src/agent/terminal_event.rs:1,12,50,132,160,175`, `src-tauri/src/commands/agent_commands.rs:138`. This is the identical failure the repo already recorded once — `TG-P2-05` "was cited twice in `catalogue.rs` and did not exist" (`ci_gate.rs:2818-2820`).
**Demonstration (the deliverable is untested):** the only tests are `terminal_event.rs`'s own `mod tests` (`:108-176`), and the two that cover the actual fix assert nothing about it:
  - `a_silent_exit_still_reports` (`:148-162`) asserts only `!fired` *inside* the scope, then comments *"Observable only through the event bus, which `TG3-01` asserts against"*.
  - `a_panic_unwinding_through_the_guard_still_reports` (`:164-176`) asserts only `result.is_err()`, then *"No assertion on the bus here — that is TG3-01's job."*
  Replace `impl Drop for TerminalEventGuard` (`terminal_event.rs:84-105`) with `fn drop(&mut self) {}` — i.e. delete the entire P3-05 fallback — and **every test in the tree still passes**. Nothing anywhere asserts that a silent exit produces an event.
**Severity:** critical (an evidence-class badge with no evidence behind it; the guard body itself is correct, but no check would notice its removal)

## F-6 · P3-02 / P3-05 · UNPROVEN — a live path still leaks `is_running` with no terminal event
**Claim (P3-02):** "a `Drop`-guarded `is_running` — it was cleared on the line *after* the await, so a panicking turn left the session permanently marked running with nothing saying why. The guard clears during unwinding."
**Reality:** the `RunningGuard` is constructed at `src-tauri/src/commands/agent_commands.rs:213`, **inside** the `tokio::spawn` closure that begins at `:133`. But `is_running: true` is written at `:67`, and `session_mgr.append_message(&user_msg)?` at `:75` — an ordinary fallible SQLite write, still on the synchronous command path — can return `Err` *before* the spawn. On that path the session is left `is_running = true` forever with no `RunningGuard`, no `TerminalEventGuard` (created at `engine.rs:171`, never reached) and no terminal event: exactly the "permanently marked running with nothing saying why" state the task claims to have closed.
**Demonstration:** make `append_message` fail (locked/read-only session DB, disk full). `send_message` returns `Err` to the frontend; `state.agents[session_id].is_running` stays `true`; no `agent:error` is emitted; the session cannot be used again until the app restarts.
**Severity:** medium

## F-7 · P3-08 · FALSE — two of the three named threads were never done, and `GATE-PROVEN` has no gate
**Claim (task text, still marked `- [x]`):** "Thread a `CancellationToken` into **the provider call, the tool executor, and every pending-permission wait**; **every one has a timeout**. → `GATE-PROVEN`"
**Reality:**
  - **No `CancellationToken` exists anywhere in the repository.** `grep -rn "tokio-util\|tokio_util\|CancellationToken"` over `autodev-core`, `src-tauri`, `src-cli` (both `Cargo.toml` and `.rs`) returns **zero** hits. The dependency the note flagged under `§0.15` was never added.
  - **The provider call has no timeout.** `autodev-core/src/provider/anthropic.rs:44` builds `Client::new()` — reqwest's default has *no* request timeout — and no `tokio::time::timeout` appears anywhere in `autodev-core/src/provider/`. A hung provider stream still blocks the turn indefinitely.
  - **The tool executor has no timeout.** The only `Duration` in `autodev-core/src/agent/tool_executor.rs` is `PERMISSION_TIMEOUT` (`:31`) and the 250 ms poll (`:598`). A tool other than `bash` that hangs is unbounded.
  - Only the third thread — the permission wait (`tool_executor.rs:588-624`) — was actually implemented, and it is implemented correctly.
  - **`GATE-PROVEN` has no gate.** The note cites `TG3-03` (`tool_executor.rs:631`); `TG3-03` is `- [ ]` with `Evidence: _pending_` at `docs/restructurization/development/TESTING_GATES.md:225-226`, and no test of that name or shape exists. `autodev-core/src/agent/tool_executor.rs` contains **no `#[cfg(test)]` module at all**, and no test in `autodev-core/tests/` references `request_permission` or `PERMISSION_TIMEOUT`.
**Demonstration:** delete the `_ = tokio::time::sleep(PERMISSION_TIMEOUT)` arm at `tool_executor.rs:615-624` — restoring an unbounded wait — and no test in the tree fails.
**Severity:** high

## F-8 · P3-05 / P3-08 / P3-10 · UNPROVEN — eight gate ids are cited in shipped code and none of them exist
**Claim:** several completion notes carry `GATE-PROVEN` / `CODE-PRESENT` badges anchored to gate ids that the code cites as live checks.
**Reality:** the following gate ids appear in **production source comments** as if they were active checks, and no test by that name or shape exists in `src-tauri/src/ci_gate.rs`, `autodev-core/tests/`, or anywhere else: `TG3-01`, `TG3-03`, `TG3-07`, `TG3-16`, `TG3-25`, `TG3-26`, `TG3-27`, `TG3-28`. Cross-checked against `docs/restructurization/development/TESTING_GATES.md`, `TG3-01` `:218`, `TG3-03` `:225` are still `- [ ]` with `Evidence: _pending_`. (`TG3-07` at `:239` is `- [x]` but its evidence points at `TG-P3-11`, a different check with a different assertion — see F-13.)
**Demonstration:** `grep -rhno "TG3-[0-9]*" --include=*.rs autodev-core/src src-tauri/src src-cli/src | sort -u` yields 8 ids; `grep -rn "    fn tg" src-tauri/src/ci_gate.rs` yields 0 matching any of them. This is the exact recurrence the repo already recorded once for `TG-P2-05` ("cited twice in `catalogue.rs` and did not exist", `ci_gate.rs:2818`).
**Severity:** high

## F-9 · P3-64 / P3-11 · OVERSTATED — one of `TG-P3-11`'s three assertions cannot fail
**Claim:** "→ `GATE-PROVEN` (`TG-P3-11`. Permission is decided **before** the partition…)" and the gate's own comment: "The read partition must not run anything the gate has not seen."
**Reality:** `src-tauri/src/ci_gate.rs:1443-1456` builds `code_only` by truncating each line at its first `//` — i.e. **comments removed** — and then asserts `!code_only.contains("read-only always approved")`. The string `read-only always approved` only ever existed as a *comment* (`// Execute (read-only always approved)`). After comment stripping it can never be present, so this assertion is **true by construction and cannot fail**.
**Demonstration:** re-add the literal defect comment `// Execute (read-only always approved)` above the `tool.execute(...)` call in the read partition — `code_only` still does not contain the phrase, and the assertion passes. The gate's other two assertions (zero `is_read_only` in code; `check_permission` textually before the partition comment) do carry weight, so the overall gate is not inert — but the assertion written *for the read-partition bypass specifically* is decorative.
**Severity:** low

## F-10 · P3-67 · OVERSTATED (count)
**Claim (task text, marked `- [x]`):** "**Re-point the three execution-path gates** so they assert the invariant rather than today's structure."
**Reality:** there were **two**, not three. Commit `f2b5d7b` changes exactly two floors: `sites >= 3` → `sites >= 1` in `autodev-core/src/agent/tool_trait.rs:207` and `total >= 3` → `total >= 1` in `src-tauri/src/ci_gate.rs:821`. The task's third citation (`ci_gate.rs:772-867`, "the sibling checks") is the *same* test as its first citation (`ci_gate.rs:811`) — `tg_p1_06_every_tool_execution_redacts_its_result` spans `:783-828`. The completion note says "Both floors re-pointed", silently contradicting the task's own "three" without striking it.
**Demonstration:** `grep -n ">= 3" src-tauri/src/ci_gate.rs` before the commit returns one execution-path floor; `git show f2b5d7b` shows two `>= 3` → `>= 1` edits total.
**Severity:** low

## F-11 · P3-06 · FALSE — the "with usage" half was never done, and `UNIT-PROVEN` has no unit test
**Claim (task text, marked `- [x]`):** "Persist the **whole** message vector **with usage**, not `history.last()`. → `UNIT-PROVEN`"
**Reality — usage:** the `messages` table has per-row `tokens_in`, `tokens_out`, `cost_usd` columns (`autodev-core/src/session/storage.rs:81,185,196`), and `Message` documents them as *"Token/cost for this message (assistant messages only)"* (`autodev-core/src/session/types.rs:41-44`). The P3-06 persistence block (`src-tauri/src/commands/agent_commands.rs:295-375`) sets **only** `.content` (`:345`) and `.tool_name` (`:351`). `Message::new_assistant` initialises `tokens_in: 0, tokens_out: 0, cost_usd: 0.0` (`types.rs:76-78`) and nothing overwrites them. `grep -rn "\.tokens_in = "` over `src-tauri/src` and `autodev-core/src` returns **zero** production writers. Every persisted assistant row is `0/0/0.0`, forever. The completion note silently drops the words "with usage" from the task it is closing.
**Reality — evidence class:** `src-tauri/src/commands/agent_commands.rs` contains **zero** `#[test]` and **zero** `#[cfg(test)]` (`grep -c` = 0). No test anywhere references P3-06. `UNIT-PROVEN` names a unit test that does not exist.
**Demonstration:** revert the whole `new_assistant` loop back to `updated_history.last()` — no test in the tree fails. Separately, open any session's `messages` table after a paid turn: every assistant row reads `tokens_in=0, tokens_out=0, cost_usd=0.0`.
**Severity:** high

## F-12 · P3-07 · UNPROVEN — `UNIT-PROVEN` with no test on either half
**Claim:** "→ `UNIT-PROVEN` *(Both halves. The live path refuses to append an empty assistant message; the persisted path refuses to write an empty row.)*"
**Reality:** both halves are genuinely implemented — the live guard at `autodev-core/src/agent/engine.rs:700-706` and the persisted guard at `src-tauri/src/commands/agent_commands.rs:338-344`. **Neither is tested.** `engine.rs`'s test module (`:1282-1380`) covers only `assemble_in_wire_order`; its `an_empty_text_block_is_dropped` (`:1360`) exercises the *P2 partial fix* the note explicitly says was insufficient, not the P3-07 guard. `agent_commands.rs` has no tests at all.
**Demonstration:** delete the `if response_content.is_empty() { … break; }` block at `engine.rs:700-706` **and** the `if text.trim().is_empty() && tool_names.is_empty() { continue; }` block at `agent_commands.rs:338-344` — the entire P3-07 deliverable — and every test in the tree still passes.
**Severity:** medium

## F-13 · P3-09 · FALSE — the stated requirement was not met, and the code comment contradicts the code three lines below it
**Claim (task text, marked `- [x]`):** "…any retry loop written today would have to **string-match `[Retryable]` back out of an error message**. The policy needs the classification carried as a *type*, which is `F-5` from the P2 audit, deferred to exactly here. → `UNIT-PROVEN` (`provider/retry.rs`, and **wired**…)"
**Reality:** the classification is still **not** carried as a type, and the wiring does precisely the string-matching the task said it must replace. `autodev-core/src/agent/engine.rs:979-992`:
```rust
|e| {
    // The adapter embedded the class in the message as `[Class]`.
    // Read back until the error type carries it structurally —
    // recorded as `F-5`, and this is its first real consumer.
    let text = e.to_string();
    if text.contains("[QuotaExhausted]") { … } else if text.contains("[Overloaded]") { … }
```
The adapters still flatten it into the message: `autodev-core/src/provider/anthropic.rs:279-281` — `AutoDevError::Provider(format!("Anthropic {status} [{class:?}]: {text}"))` (same at `google.rs:236,276`, `openai.rs:291,325`). `AutoDevError` gained no `ErrorClass` field.
**Demonstration (comment vs code):** the block comment directly above, at `engine.rs:961-965`, states: *"The classification comes from the error the adapter attached (`ErrorClass::classify`), **not from matching its text** — the whole defect `P3-09` fixes is that the classification was computed, stringified and discarded."* The closure at `:983` is `let text = e.to_string(); if text.contains("[QuotaExhausted]")`. The comment and the code four lines below it assert opposite things.
**Second defect — the retry never fires for the commonest retryable failure:** a transport-level failure (connection reset, DNS, TLS, read timeout) becomes `AutoDevError::Http` → `"HTTP error: {reqwest}"` (`autodev-core/src/errors.rs:50-51`), which carries **no `[Class]` tag**, so the closure's `else` branch returns `ErrorClass::Fatal` and `decide()` returns `Stop(NotRetryable)`. The policy can only ever retry an HTTP *response* the adapter classified (5xx / non-quota 429) — never a dropped connection.
**Severity:** high

## F-14 · P3-14 · OVERSTATED — the gate cannot catch the "new tool" case it claims to
**Claim:** "Keyed on *declares a path input and touches the disk*, so a **NEW** tool with a path and no jail call fails the build, which is `TG3-27`'s requirement." (The count itself — "**Seven** tools, not six" — is TRUE: commit `d6403db` adds `validate_workspace_path` to `analyze_diff_tool`, `check_file`, `glob`, `grep`, `list_directory`, `read_file`, `write_interface_spec`.)
**Reality:** `TG-P3-14` (`src-tauri/src/ci_gate.rs:1554-1616`) recognises a path input by four **exact quoted literals** — `"path"`, `"repo_path"`, `"file_path"`, `"directory"` (`:1577-1580`) — and then only asserts `text.contains("validate_workspace_path")` **anywhere in the file** (`:1599`).
**Demonstration (three ways to add an unjailed path tool and stay green):**
  1. Name the parameter anything else. `"dest"`, `"output_path"`, `"target_file"`, `"src"`, `"folder"` all fail `text.contains("\"path\"")` (the literal includes the quotes, so `"output_path"` does **not** match `"path"`), so `declares_path` is false and the tool is skipped before any assertion runs.
  2. Take two paths and jail one. The check is a file-level substring, not per-parameter, so `validate_workspace_path(&input_path, …)` present anywhere satisfies it while `dest_path` goes straight to `std::fs::write`.
  3. Mention it in a comment. `read_source` normalises line endings only; comments are **not** stripped for this check (unlike `TG-P3-03`/`TG-P3-04`), so `// TODO: call validate_workspace_path here` passes the gate.
  Two shipped tools already sit outside the gate on the `touches_disk` exemption while taking a model-named path: `annotate_file_tool` (`"file_path"`, 0 jail calls) and `spawn_qc_review` (`"repo_path"`, 0 jail calls). Both are currently benign (DB key / prompt text respectively), which is exactly why the exemption is invisible.
**Severity:** medium

## F-15 · P3-16 · FALSE — the "one shared helper" has one caller, not three, and the code says three
**Claim (task text, marked `- [x]`):** "Normalise the `path_lock` key … through **one shared helper also used by the graph DB key and checkpoint retention** (a **fourth** instance was found)."
**Reality:** `autodev-core/src/security/path_key.rs` opens with the line *"One canonical key for one path — **used by three subsystems** (P3-16, C-7)"* (`:1`) and names them at `:8-13`: `path_lock`, "the graph DB path key (spec `05 §2.2`)", "checkpoint retention (`G3b GI-28`)". `path_key()` has exactly **one** production caller in the whole tree: `autodev-core/src/agent/builtin_tools/path_lock.rs:92`. `grep -rn "path_key" --include=*.rs autodev-core/src src-tauri/src` returns only that call, the `platform_is_case_insensitive()` use in `path_lock.rs:224`, the `pub mod` line, and the helper's own tests. Neither the graph DB key nor checkpoint retention was converted.
**Demonstration:** delete the graph and checkpoint bullet points from the file header and nothing changes; conversely, `grep -rn "to_lowercase\|canonicalize" autodev-core/src/graph/` shows the graph subsystem still rolling its own normalisation (`graph/extract.rs:133,141,1397`). The helper's own docstring is the only evidence for two of the three claimed consumers.
**Note:** the *other* half of P3-16 — "the race itself is closed by the caller change" — **survives**: `write_file.rs:74,98`, `edit_file.rs:88,106` and `append_file.rs:78,88` all now pass the jailed path to `path_lock::acquire_for`.
**Severity:** medium

## F-16 · P3-18 · OVERSTATED — the gate's key assertion is an exact-source-text match that reformatting defeats
**Claim:** "`is_clean()` is now `self.ran() && no errors`, backed by a third severity `SEVERITY_UNAVAILABLE` … → `UNIT-PROVEN`" and the gate's own docstring: "A checker that did not run must never be reportable as clean."
**Reality (implementation is correct):** `autodev-core/src/agent/builtin_tools/validate.rs:61-63` — `self.ran() && !…severity == "error"` — and `format_inline` renders `NOT CHECKED` (`:95`). That half survives.
**Reality (gate):** `TG-P3-18`'s third assertion (`src-tauri/src/ci_gate.rs:2606-2610`) is
```rust
!text.contains("severity: \"warning\",\n                message: format!(\n                    \"Could not run")
```
— a match on the **exact former source text**, including its exact indentation (16 and 20 spaces) and the exact line breaks rustfmt happened to produce. It encodes the defect's old spelling, not the invariant.
**Demonstration:** reintroduce the defect as a single line — `severity: "warning", message: format!("Could not run {checker}"),` — or with any other indentation, wording, or `.to_string()` instead of `format!`. The literal no longer matches, so the assertion passes; `SEVERITY_UNAVAILABLE` is still present in the file so assertion 2 passes; `is_clean` still contains `self.ran()` so assertion 1 passes. But `ran()` (`validate.rs:43-48`) only looks for `SEVERITY_UNAVAILABLE`, so a missing binary emitting `"warning"` again makes `ran() == true`, `is_clean() == true`, and `check_file.rs:160` prints `"✓ No syntax errors."` for a file nothing looked at — the original defect, verbatim, with the gate green.
**Severity:** medium

## F-17 · P3-17 · OVERSTATED (gate scope)
**Claim:** "The gate asserts the **invariant** (zero `.output()` outside the helper), not the presence of the fix: a planted **sixth** unbounded spawn site fails it."
**Reality:** true only for a plant that uses `.output()`. `TG-P3-17` (`src-tauri/src/ci_gate.rs:2531-2576`) counts `text.matches(".output()")` in `validate.rs` and nothing else. `tokio::process::Command` has three other ways to run a child, none of which the gate sees: `.spawn()`, `.status()`, `.wait_with_output()`. The sibling tool `e2e_probe.rs` already uses the `.spawn()` form (`:402`, `:602`), so the shape is present in the codebase. The gate is also single-file: a new validator subprocess added anywhere but `validate.rs` is invisible to it.
**Demonstration:** add `let child = tokio::process::Command::new("tsc").spawn()?; let out = child.wait_with_output().await?;` as a sixth site in `validate.rs` — `total_output_calls` is still 1, all four assertions pass, and the unbounded spawn ships. (The five-site count itself is TRUE: `run_bounded` is called at `validate.rs:306,343,407,930,1007`.)
**Severity:** medium

## F-18 · P3-20 · OVERSTATED — the gate covers 1 of the 5 defects it was written for
**Claim:** "`TG-P3-20` gates the class both ways and was falsified." (The count is TRUE: commit `37077e2` corrects descriptions in exactly five tool files — `glob`, `remember_this`, `check_file`, `spawn_parallel_subagents`, `assign_tasks_batch` — and there are 38 tools, so "the other 33" is arithmetically consistent.)
**Reality:** `tg_p3_20_a_description_may_not_name_a_mechanism_the_code_lacks` (`src-tauri/src/ci_gate.rs:1709-1789`) is keyed on a **hardcoded five-element list**: `("gitignore"|"PID"|"tears down"|"timeout"|"transaction")` (`:1712-1718`). Of the five real defects the note enumerates, only **`glob`/gitignore** is covered:
  - `remember_this` — "facts appear in every future system prompt" → no keyword.
  - `check_file` — syntax tier "Always runs" → no keyword.
  - `spawn_parallel_subagents` — "floor of 2 where the schema allows 1" → numeric, no keyword; the sibling test `tg_p3_20_enumerated_values_match_the_schema` (`:1798-1884`) only parses the two markers `"Valid statuses:"` and `"Valid workers:"` (`:1842`), never a minimum.
  - `assign_tasks` — unmeasured "~10x faster" → no keyword.
**Demonstration:** restore any of those four sentences verbatim into its tool's `description()` and both `TG-P3-20` tests pass. The `backed` test is also near-vacuous for one of the five keywords it does cover: `("timeout", &["timeout", "Duration"])` (`:1716`) — any tool file that mentions `Duration` anywhere counts as backing a timeout promise.
**Severity:** medium

## F-19 · P3-68 · OVERSTATED — the loopback confinement is bypassed by an HTTP redirect, and by DNS rebinding
**Claim:** "`probe_url` is confined to loopback **by resolution, not spelling** — every address the host resolves to must be loopback, so `127.0.0.1.nip.io` and **any attacker-controlled DNS name pointing at `169.254.169.254`** are both refused… Tested against the real EC2 metadata URL."
**Reality:** `validate_probe_url` (`autodev-core/src/agent/builtin_tools/e2e_probe.rs:60-100`) is correct as far as it goes, and is called before any spawn (`:209-212`). But the fetch that follows does not inherit the confinement:
  1. **Redirects are followed.** `http_probe` (`:482-487`) builds `reqwest::Client::builder().timeout(…).build()` with **no `.redirect(Policy::none())`**. reqwest's default is `Policy::limited(10)`. A URL that passes validation (`http://127.0.0.1:PORT/`) and answers `302 Location: http://169.254.169.254/latest/meta-data/iam/security-credentials/` is followed, and up to 400 characters of the final body are returned to the model at `:457` (`response_body_excerpt: Some(body.chars().take(400).collect())`). The redirecting server is trivially reachable — `start_cmds` is model-supplied arbitrary shell executed by this same tool call, seconds earlier.
  2. **DNS rebinding.** `validate_probe_url` resolves the host at `:84` (`to_socket_addrs`), and `http_probe` resolves it **again independently** at `:489`. A short-TTL attacker-controlled name answering `127.0.0.1` to the first lookup and `169.254.169.254` to the second passes the check and is fetched — which is exactly the "attacker-controlled DNS name" case the claim says is refused.
**Demonstration:** `start_cmds: ["python -c \"import http.server,socketserver;…302 to http://169.254.169.254/…\""]`, `probe_url: "http://127.0.0.1:8099/"`. Validation passes (loopback), the child is started, the GET follows the redirect off-machine, and the metadata body lands in `response_body_excerpt`. `TG-P3-21/68` asserts only that `validate_probe_url` contains `is_loopback` and `to_socket_addrs` and is called before `run_probe` (`ci_gate.rs:1917-1941`) — it never inspects the client's redirect policy.
**Severity:** high

## F-20 · P3-22 · OVERSTATED — the gate finds a third stamping site only if it is spelled identically
**Claim:** "the gate asserts the invariant - *every* site that stamps `terminated_at` releases the permit - and a third site fails it."
**Reality:** `TG-P3-22` (`src-tauri/src/ci_gate.rs:2211-2238`) locates stamping sites with `text.match_indices("terminated_at = Some(Instant::now())")` — an exact literal — and then requires `permit.take()` within the next 220 bytes.
**Demonstration:** a third stamping site written as
```rust
let now = Instant::now();
entry.terminated_at = Some(now);
```
is not matched, `stamps.len()` stays at 2, the loop never examines it, and the new site leaks a permit with the gate green. The same holds for a struct-literal form (`terminated_at: Some(Instant::now())`) — note `manager.rs:148` already uses the struct-literal spelling for the `None` case, so the alternate form is idiomatic here.
**Note:** the *fix* itself survives — `permit: Option<PoolPermit>` (`manager.rs:52`), released at both stamping sites (`:344`, `:377`), prune moved above `acquire` (`:88-100`), and `terminate()` releases by removing the entry (`:245`). P3-24 also survives: `abort_children_of` is called on both cancellation paths (`engine.rs:185`, `:458`) and both spawn tools key on `ctx.session_id` (`spawn_subagent.rs:159`, `spawn_parallel_subagents.rs:162`).
**Severity:** low

## F-21 · P3-58 · FALSE — the "fix" is arithmetically a no-op; the pool's depth branch is still unreachable
**Claim (task):** "**Make the pool's depth branch reachable.** The three builtin-tool guards reject at `>= 3` on the **parent** while `pool.rs` rejects at `> 3` on the **child** — strictly tighter, so the pool's own depth check can never fire from any builtin tool."
**Claim (note):** "Both spawn tools dropped their local `const MAX_SUBAGENT_DEPTH: u32 = 3` and now test `ctx.depth + 1 > subagent::pool::MAX_NESTING_DEPTH` — the CHILD, which is the side the pool rejects on. Previously parent `>= 3` fired one generation before child `> 3` ever could, so the pool branch was unreachable."
**Reality:** for `ctx.depth: u32` and `MAX_NESTING_DEPTH = 3`, **`ctx.depth >= 3` and `ctx.depth + 1 > 3` are the same predicate.** The old guard and the new guard fire at exactly the same parent depth. Nothing about reachability changed:
  - old: `spawn_subagent.rs` `const MAX_SUBAGENT_DEPTH: u32 = 3; if ctx.depth >= MAX_SUBAGENT_DEPTH` (commit `0c760ff`, removed lines).
  - new: `autodev-core/src/agent/builtin_tools/spawn_subagent.rs:99-100` — `let child_depth = ctx.depth + 1; if child_depth > subagent::pool::MAX_NESTING_DEPTH`.
  - pool: `autodev-core/src/subagent/pool.rs:94` — `if depth > MAX_NESTING_DEPTH`, where `depth` is `config.depth = ctx.depth + 1` (`spawn_subagent.rs:189`, `spawn_parallel_subagents.rs:192`).
  The tool guard runs first and rejects at the same threshold, so **`pool.rs:94` is still unreachable from any builtin tool** — the exact condition the task exists to remove. The premise "strictly tighter" / "fires one generation before" is false: neither is tighter; they are equal.
**Demonstration:** substitute any `d: u32`. `d >= 3` ⟺ `d + 1 > 3` ⟺ `d > 2`. Enumerate: d=2 → both false; d=3 → both true. There is no depth at which the old and new guards differ, therefore no depth at which the pool's branch becomes reachable through a tool.
**Third site:** the task says "the **three** builtin-tool guards"; only two were changed. `autodev-core/src/agent/builtin_tools/spawn_qc_review.rs:81` still reads `if ctx.depth >= subagent::pool::MAX_NESTING_DEPTH` — the parent-side spelling the gate's own message calls a defect ("testing the parent instead makes the two guards disagree by exactly one generation", `ci_gate.rs:2385-2388`). The gate's loop covers only `spawn_subagent.rs` and `spawn_parallel_subagents.rs` (`ci_gate.rs:2359`), so it never looks at the third.
**Gate cannot express the failure:** `TG-P3-58/…` asserts only the *presence of the strings* `subagent::pool::MAX_NESTING_DEPTH` and `let child_depth = ctx.depth + 1` (`ci_gate.rs:2377-2389`). It cannot detect that the two boundaries coincide, because they always did.
**Severity:** high (the task's stated deliverable — a reachable pool guard — was not delivered; the real, unclaimed benefit is deduplicating the constant)

## F-22 · P3-60 · OVERSTATED — the fix inverted the meaning of `tools` and left the model-facing description saying the opposite
**Claim:** "An empty list means unrestricted, matching how all four writers use the field; making empty mean 'no tools' would have silently disarmed every existing subagent."
**Reality:** `ToolRegistry::restrict_to` returns early on an empty list (`autodev-core/src/agent/tool_registry.rs:42-45`), so empty = full registry. But `spawn_subagent`'s own schema, which is what the model reads, still says the opposite: `autodev-core/src/agent/builtin_tools/spawn_subagent.rs:55` — `"description": "Tool subset (e.g. [\"read_file\", \"grep\"]). Empty = MCP tools only."` A model that omits `tools` believing it is restricting the child to MCP tools now hands it the **entire** builtin registry. This is precisely the `P3-20` defect class ("a description may not name a mechanism it lacks"), reintroduced by the `P3-60` fix in the same phase — and `TG-P3-20`'s five-keyword list (F-18) has nothing that matches it.
**Demonstration:** `spawn_subagent({"task_description": "...", "tools": []})` — the description promises MCP-only; `restrict_to(&[])` returns without narrowing anything; the child's registry is whatever `register_for_mode` granted.
**Severity:** medium

## F-23 · P3-62 · FALSE — a timed-out subagent still reports $0.00, which is the case the claim names
**Claim:** "The runner reads it **before** unwrapping the timeout outcome, so a subagent that **timed out at 600s still reports what it spent**; reporting zero there would hide precisely the runs the ceiling exists to catch."
**Reality:** `AgentEngine::record_spend` — the only writer of `self.last_run` — is called **only on exit paths inside `run()`**: `autodev-core/src/agent/engine.rs:387` (budget), `:675` (stream error), `:745` (tool error), `:887` (normal exit). It is never called per turn. `tokio::time::timeout` (`autodev-core/src/subagent/native_runner.rs:311-315`) **drops** `run_future` when the deadline fires; a dropped future does not execute the remainder of its body, so none of those four sites runs. `engine.last_run_accounting()` (`:319`) therefore returns `None`, `*accounting` is left at `RunAccounting::default()` (`:118`) = `0.0/0/0`, and the failed `SubagentResult` at `:194-196` reports `cost_usd: 0.0`.
**Demonstration:** set `timeout_secs` low enough that the engine is mid-`stream_response` when it fires. `last_run` is `None`; the result carries `cost_usd: 0.0, tokens_in: 0, tokens_out: 0`. The gate (`ci_gate.rs:2424-2436`) only counts occurrences of the *strings* `cost_usd: 0.0` (must be 0) and `cost_usd: accounting.cost_usd` (must be 2) — it cannot see that `accounting` is still zero on this path.
**Note:** the *non-timeout* half survives — `Ok(Err(e))` from `run()` does reach `record_spend`, so an engine error reports real spend.
**Severity:** high (the QC hourly dollar ceiling is computed from this field, and the 600 s timeout is the most expensive run)

## F-24 · P3-25 / P3-28 · OVERSTATED — the clear layer is silently dropped by two of the three providers, including the primary one
**Claim:** "**The ordering is the deliverable** … `CLEAR_TRIGGER_FRACTION = 0.60` sits below `COMPACTION_THRESHOLD = 0.80`, and that inequality is asserted, so **the cheap server-side layer always gets its chance before the summarisation call**."
**Reality:** the engine attaches `clear_tool_uses` to **every** request unconditionally (`autodev-core/src/agent/engine.rs:486-488`), but only the Anthropic adapter reads it — `autodev-core/src/provider/anthropic.rs:197-212` (body) and `:259,:300` (the beta header). `grep -n "clear_tool_uses" autodev-core/src/provider/google.rs autodev-core/src/provider/openai.rs` returns **nothing**: the field is constructed and discarded. On Google and OpenAI the 60 %/80 % band does nothing at all and compaction at 80 % remains the only layer. The MDP's own §5 records "Providers narrowed to three with **Gemini primary**", so the layer is inert on the default install.
**Demonstration:** run any long Gemini session. `clear_policy(model_context)` returns `Some(..)`, `CompletionRequest.clear_tool_uses` is populated, `GoogleProvider::build_body` never reads it, and the request goes out unchanged. The `BENCH-PROVEN` measurement ("11,310 → 2,750 input tokens") was taken through `AnthropicProvider::complete`, as the note itself states.
**Severity:** medium

## F-25 · P3-59 · OVERSTATED (scope)
**Claim:** "The gate constructs a **real `GoogleProvider`** and calls the real `Provider::models()`, so it **walks every hop the engine walks** rather than reaching into the catalogue."
**Reality:** `TG-P3-59` (`src-tauri/src/ci_gate.rs:2456-2514`) constructs a bare `GoogleProvider`. The engine's provider is a `CappedProvider` wrapper (`autodev-core/src/provider/capped.rs:116-118`), which the gate never traverses — that hop is skipped. The gate also covers **only Google**: `AnthropicProvider::models()` and `OpenAiProvider::models()` are never exercised, so the same `models_for()` regression on either of those two adapters would leave the gate green.
**Demonstration:** make `CappedProvider::models()` return `Vec::new()` — every model falls to the engine's `unwrap_or(200_000)` fallback (`engine.rs:415`) and `TG-P3-59` still passes, because it never constructs a `CappedProvider`.
**Severity:** low

## F-26 · P3-64 · OVERSTATED — two of three dead bindings removed
**Claim:** "The dead `_mode`/`_always_allow` bindings are gone."
**Reality:** true for those two, but the third binding of exactly the same shape survives in the same block: `autodev-core/src/agent/tool_executor.rs:321` — `let _pending = self.pending.clone();`, cloned once per read-only tool per batch and never referenced inside the spawned task (verified: `pending` appears nowhere between `:313` and `:375` except that line). It is underscore-prefixed for the same reason the other two were — to silence the unused warning.
**Severity:** low

## F-27 · P3-63 · OVERSTATED — the "zero means use the worker default" branch cannot be reached
**Claim:** "A caller-set bound now reaches the enforcer; **zero still means 'use the worker default'**."
**Reality:** `autodev-core/src/subagent/native_runner.rs:284-286` guards the wiring with `if config.max_turns > 0`. But `SubagentManager::spawn` fills the default **before** the config is cloned for the runner — `autodev-core/src/subagent/manager.rs:103-105` (`if config.max_turns == 0 { config.max_turns = SubagentConfig::DEFAULT_MAX_TURNS; }`) executes at `:103`, and `let run_cfg = config.clone()` is at `:127`. So `config.max_turns` is never 0 in the runner and the `else` (leave the budget's own default) is dead code — structurally identical to the dead `config.model.is_empty()` fallback that `P3-61`, in this same commit, was written to remove.
**Demonstration:** the only writer that could pass 0 is `manager::spawn`, and it rewrites 0 to `DEFAULT_MAX_TURNS` 24 lines before the clone. There is no input under which `config.max_turns == 0` reaches `native_runner.rs:284`.
**Severity:** low

---

## Summary

**Scope examined:** all **43** `- [x]` tasks in `### P3-A` … `### P3-E` of `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md` (P3-01…P3-29, P3-55…P3-68). Four of the 43 are struck-through (`P3-13` moved to `P0-13a`; `P3-23`, `P3-26`, `P3-27` marked ALREADY-DONE) and were checked only for the residues they carry forward.

**Claims that survived refutation (18 task ids):** `P3-01`, `P3-11`, `P3-12`, `P3-13`✂, `P3-15`, `P3-19`, `P3-21`, `P3-23`✂, `P3-24`, `P3-26`✂, `P3-27`✂, `P3-29`, `P3-55`, `P3-56`, `P3-57`, `P3-61`, `P3-65`, `P3-66`.
Notable strong work: `TG-P3-55` (asserts the ordering that makes the RAII guard load-bearing), `TG-P3-59` (constructs a real provider), `TG-P3-17`'s `.output()` invariant, the `TerminalEventGuard` design, `ProvisionalCharge`, the pre-keyed `tool_use_id` slots, and the eight/seven/five counts in `P3-55`/`P3-14`/`P3-20`, all of which reproduce exactly from the code.

**Claims with at least one finding (25 task ids):** `P3-02`, `P3-03`, `P3-04`, `P3-05`, `P3-06`, `P3-07`, `P3-08`, `P3-09`, `P3-10`, `P3-14`, `P3-16`, `P3-17`, `P3-18`, `P3-20`, `P3-22`, `P3-25`, `P3-28`, `P3-58`, `P3-59`, `P3-60`, `P3-62`, `P3-63`, `P3-64`, `P3-67`, `P3-68`.

**Findings: 27**

| severity | count | ids |
|---|---|---|
| critical | 1 | F-5 |
| high | 8 | F-4, F-7, F-8, F-11, F-13, F-19, F-21, F-23 |
| medium | 11 | F-1, F-3, F-6, F-12, F-14, F-15, F-16, F-17, F-18, F-22, F-24 |
| low | 7 | F-2, F-9, F-10, F-20, F-25, F-26, F-27 |

**The recurring shapes, in order of how often they fired:**

1. **A badge with no evidence behind it** (F-5, F-7, F-8, F-11, F-12). Eight gate ids — `TG3-01`, `TG3-03`, `TG3-07`, `TG3-16`, `TG3-25`, `TG3-26`, `TG3-27`, `TG3-28` — are cited in shipped source comments and **none exists as a test**. `P3-05` and `P3-08` carry `GATE-PROVEN`; `P3-06` and `P3-07` carry `UNIT-PROVEN` over files with zero `#[cfg(test)]`. Deleting the entire `P3-05` and `P3-07` deliverables breaks no test in the tree.
2. **A check whose shape cannot express failure** (F-1, F-3, F-9, F-14, F-16, F-18, F-20, F-21). Four distinct mechanisms: an allowlist of thirteen variable *names* (`TG-P3-04`), a `contains()` over a file that includes its own explanatory comments (`TG-P3-02`), an exact multi-line source-text literal encoding the defect's former formatting (`TG-P3-18`), and an assertion against a comment string after comments have been stripped (`TG-P3-11`, vacuous by construction).
3. **A fix applied at some sites while the same defect stands at others** (F-4, F-15, F-21, F-26). 22 poison-propagating lock sites survive `P3-03`; `path_key` reaches 1 of the 3 subsystems its own header names; `spawn_qc_review` keeps the depth spelling `P3-58` calls a defect; a third `_`-prefixed dead binding survives `P3-64`.
4. **A stated requirement quietly dropped from the completion note** (F-7, F-11, F-13, F-21). `P3-08`'s `CancellationToken` and provider/tool-executor timeouts, `P3-06`'s "with usage", `P3-09`'s "carried as a *type*", `P3-58`'s "make the pool's branch reachable" — each is in the task text, each is unmet, and each note closes the task on a narrower deliverable without striking the original wording.
5. **A comment asserting the opposite of the code beneath it** (F-13, F-22). `engine.rs:961-965` says the class is not matched from text; `engine.rs:983` matches it from text. `spawn_subagent.rs:55` tells the model "Empty = MCP tools only"; `restrict_to(&[])` returns without narrowing anything.
6. **A number that does not reproduce** (F-2, F-4, F-10). "twelve sites" (11), "103 sites — 98 mutex + 5 RwLock" (101, of which 3 are RwLock), "the three execution-path gates" (2).

**The single most consequential finding is F-21** (`P3-58`): `ctx.depth >= 3` and `ctx.depth + 1 > 3` are the same predicate, so the change was semantically a no-op, the premise ("fires one generation earlier") is arithmetically false, and the pool's depth branch — the thing the task exists to make reachable — is still unreachable from every builtin tool.

---

## Provenance note — the tree was not clean when I read it

I made **no** edits; every command I ran was read-only. But `git status` shows three files with **uncommitted working-tree changes** that were already present when this audit began:

- `autodev-core/src/agent/engine.rs` — adds `.with_state(self.state.clone())` under a comment reading *"🔴 G-3 (P3 post-phase gap hunt). `with_state` had **zero callers workspace-wide**, so `self.state` was always `None`"*.
- `autodev-core/src/agent/tool_executor.rs` — removes a **second** `check_permission` call from `execute_batch`'s write partition (*"🔴 G-1 … Every write/execute tool was prompted twice … with a fresh `Uuid::new_v4()` call id the second time"*).
- `src-tauri/src/ci_gate.rs` — adds a "count the gates" block to `tg_p3_11_one_permission_gate_keyed_on_effect`.

Two consequences for this report:

1. **All line numbers above are working-tree line numbers.** `ci_gate.rs` is +36 lines from `HEAD` at offsets past ~1391, and `tool_executor.rs` is +17 past ~437. Citations before those offsets are unaffected.
2. **Two further defects against the audited claims are already recorded in that uncommitted work, and I did not re-derive them.** At the audited commit (`1f16975`), `P3-08`'s note claims the permission wait "races three outcomes … the existing `is_cancelled` flag"; with `with_state` never called, `self.state` was `None`, `.map(...).unwrap_or(false)` was permanently `false`, and **that arm could never fire** — so at HEAD the claim was two-thirds true. And `P3-64`/`TG-P3-11`'s "every tool passes exactly one gate" was false at HEAD: `execute_batch`'s write partition ran a second `check_permission` with a freshly minted `Uuid::new_v4()` call id, so every write/execute tool prompted twice. Both corroborate F-9's conclusion that `TG-P3-11` was written against the shape of the P3-64 fix rather than against its invariant.
