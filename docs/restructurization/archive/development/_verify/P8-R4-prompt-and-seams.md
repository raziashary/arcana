# P8-R4 — Adversarial pre-build verification: prompt floor & seams

**Scope:** Groups P8-E (one prompt floor), P8-F (procedural memory seam),
P8-G (Live Monitor), P8-H (retire `airsctl agent`).
**Stance:** refute-first. Default REFUTED where evidence is ambiguous.
**Mode:** read-only. No writes outside this file. No git writes, no app runs,
no migrations, no DB-touching tests.

**Status: IN PROGRESS — appended incrementally.**

---

## GROUP P8-E — one prompt floor for every agent

### E.0 — The framing claim: "override is a total replacement"

**VERDICT: CONFIRMED (line numbers exact).**

`autodev-core/src/agent/engine.rs:208-233` — the cited range `208-227` is
precisely the `match` head through the end of the `Some(_)` arm:

```
208:        let (stable_prompt, volatile_prompt) = match system_prompt_override {
...
224:            Some(override_str) => (
225:                override_str.to_string(),
226:                context_builder.memory_block(working_dir, user_message),
227:            ),
228:            // 🔴 P3-43. `user_message` is the query. It used to be the empty
229:            // string literal, which made recall byte-identical every turn.
230:            None => {
231:                context_builder.build_system_prompt_parts(self.pa_mode, working_dir, user_message)
232:            }
233:        };
```

The `Some` arm calls **only** `memory_block`. It never calls
`build_system_prompt_parts`, so it never reaches `BASE_SYSTEM_PROMPT`,
`REVIEW_SYSTEM_PROMPT`, `DELEGATE_SYSTEM_PROMPT`, the global AUTODEV.md, or
the per-repo AUTODEV.md. Total replacement of everything except memory.

The code itself already states the intended semantics, in the same comment
block P8-E is arguing against:

`autodev-core/src/agent/context.rs:170` — "An override says *"use my
instructions"*. It does not say *"forget what you know"*, and the two are not
the same request."

### E.1 — "seven `## What You Must Never Do` rules"

**VERDICT: CONFIRMED — exactly seven, in `BASE_SYSTEM_PROMPT`.**

`autodev-core/src/agent/context.rs:290-298`:

```
290:## What You Must Never Do
291:
292:- Never commit secrets, API keys, or credentials to any file
293:- Never push directly to `main` — always use a branch + PR
294:- Never modify files outside your assigned domain (e.g., a backend worker must not touch frontend code)
295:- Never mark a task complete without running tests
296:- Never create a PR without linking the originating GitHub Issue
297:- Never use `git push --force` on shared branches
298:- Never ignore a failing test by deleting or skipping it — fix the underlying issue"#;
```

Note there are **two other** `## What You Must Never Do` sections in the same
file, with different content and different counts:
`context.rs:319-322` (REVIEW_SYSTEM_PROMPT, three rules) and
`context.rs:353-357` (DELEGATE_SYSTEM_PROMPT, four rules).
P8-34 says "extract an invariant core from `BASE_SYSTEM_PROMPT`" and does not
say what happens to the other two mode prompts, which today each carry their
own divergent, shorter prohibition list. **A core extracted from BASE only
leaves Review and Delegate mode still carrying weaker prohibitions unless
they are rebased on the core too.** That is a gap in the task text, not a
refutation.

### E.2 — "reach the interactive assistant"

**VERDICT: OVERSTATED.** The interactive assistant reaches them only when the
user has *not* set a custom system prompt — and the same override field is
what the settings UI writes.

`autodev-core/src/state/app_state.rs:56-57`:
```
56:    // Custom system prompt override (if set, replaces BASE_SYSTEM_PROMPT)
57:    pub system_prompt_override: Option<String>,
```

`src-tauri/src/commands/settings_commands.rs:168-173` writes it from the UI.
`src-tauri/src/commands/agent_commands.rs:109-111` reads it and
`:244` passes it into `engine.run(...)` on the **interactive** path.

So the defect P8-E describes is strictly worse than stated: a PA user who
customises their system prompt in Settings *also* loses the seven rules and
both AUTODEV.md blocks. That strengthens P8-35/P8-36 rather than weakening
them, but the task text's premise ("reaches the interactive assistant") is
only conditionally true and should not be used as the baseline for a gate.

---

## GROUP P8-G — restore the Live Monitor

### G.1 — P8-42 "their only emitters are inside the Mode 2 branch"

**VERDICT: CONFIRMED for `worker:tick-start`. PARTIALLY CONFIRMED (one live
emitter exists) for `worker:tick-end`. The *conclusion* — "the Live Monitor
is already permanently blank" — is CONFIRMED in practice, but by a stronger
mechanism than the task states.**

**What I searched (exhaustive, three passes):**
1. `worker:tick` across the whole repo (all file types) — the string literals
   live only in `autodev-core\src\events.rs:106-107` on the Rust side.
2. `EVT_WORKER_TICK_START|EVT_WORKER_TICK_END` — 4 hits, all in `events.rs`
   (2 const defs at `:106-107`, 2 uses at `:303` and `:307` inside the two
   emit helpers).
3. `emit_worker_tick_start|emit_worker_tick_end` — **every Rust call site**:

```
autodev-core\src\recipes\runner.rs:36   (use ...)
autodev-core\src\recipes\runner.rs:287  emit_worker_tick_start
autodev-core\src\recipes\runner.rs:316  emit_worker_tick_end
autodev-core\src\recipes\runner.rs:348  emit_worker_tick_end
autodev-core\src\recipes\runner.rs:391  emit_worker_tick_end
autodev-core\src\recipes\runner.rs:415  emit_worker_tick_end
autodev-core\src\recipes\runner.rs:552  emit_worker_tick_end
autodev-core\src\recipes\runner.rs:634  emit_worker_tick_end
autodev-core\src\recipes\runner.rs:694  emit_worker_tick_end
autodev-core\src\recipes\runner.rs:727  emit_worker_tick_end
```

`runner.rs` is the **sole** emitter file. There is no second emitter anywhere
(no `src-tauri`, no `src-cli`, no test helper).

**Where the Mode 2 branch begins and ends:**
`autodev-core/src/recipes/runner.rs:276` — `if orch_healthy {`
`autodev-core/src/recipes/runner.rs:703` — `} else {`

So call sites **287, 316, 348, 391, 415, 552, 634, 694** are inside the Mode 2
branch. Call site **727 is NOT** — it is after the branch closes, in the
`FallbackPolicy::Mode2Only` guard:

```
717:        if matches!(
718:            recipe.fallback_policy,
719:            crate::recipes::types::FallbackPolicy::Mode2Only
720:        ) {
...
727:            emit_worker_tick_end(WorkerTickEndPayload {
728:                slug: worker_slug.to_string(),
729:                outcome: "fallback_skipped_policy".to_string(),
730:                timestamp: chrono::Utc::now().to_rfc3339(),
731:            });
```

**So "their only emitters are inside the Mode 2 branch" is literally false for
`tick-end`** — one emitter (`:727`) survives P8-B's deletion and would be
orphaned: a `tick-end` with no `tick-start` in front of it.

**Why the conclusion still holds — and why it holds harder than stated.**
The Mode 2 branch is not merely "the branch P8-B deletes"; it is already
statically unreachable:

```
173:        let _orch_reachable = self.orch_client.is_healthy().await;
174:        let orch_healthy = false;
```

`orch_healthy` is a hardcoded `false` literal (`runner.rs:174`), so
`if orch_healthy {` at `:276` never runs. The health probe's result is
discarded into `_orch_reachable`. That is the real reason the Live Monitor is
blank — not the sidecar's absence, but a constant in this function.

And the surviving emitter at `:727` is also unreachable in a default install:
**all nine** default recipes declare `Mode1Capable`, so the `Mode2Only` guard
never fires either —
`autodev-core/src/recipes/defaults.rs:232, 408, 677, 916, 1043, 1226, 1399, 1971, 2064`
all read `fallback_policy: super::types::FallbackPolicy::Mode1Capable,` and
there is **no** `Mode2Only` in `defaults.rs`.

**Net:** today a default fleet emits **zero** tick events of either kind. A
user-authored `Mode2Only` recipe would emit a lone `tick-end`. Both facts
argue for P8-42; neither is what the task text says.

### G.2 — P8-42 cited range `runner.rs:802-889`

**VERDICT: CONFIRMED — exact.**

`:802` is `self.set_worker_status(worker_slug, "working");` (first line of the
native run block after provider setup) and `:889` is the closing `}` of the
outcome `match`:

```
802:        self.set_worker_status(worker_slug, "working");
...
842:        let result = engine
843:            .run(
844:                &session_id,
845:                &working_dir,
846:                &model,
847:                vec![],
848:                RECIPE_TRIGGER,
849:                Some(&recipe.instructions),
850:            )
851:            .await;
...
889:        }
```

There is **no** tick emitter anywhere in 739-889. The native path sets status
only, via `set_worker_status(...)` at `:802`, `:860`, `:878` — which is
`set_worker_status_ex(slug, status, None, None)` (`runner.rs:990-992`), i.e.
**both** `tick_duration_ms` and `last_outcome` are hardcoded `None` on every
native-path call.

### G.3 — P8-43 "no Rust call site ever sets `mode1_active`"

**VERDICT: CONFIRMED.**

**What I searched:** `mode1_active` across the whole repo. Every hit is
TypeScript or documentation — **zero** `.rs` files:

```
src\types\live.ts:55:  | 'fallback_idle' | 'mode1_active'
src\stores\agentLiveStore.ts:404:  e.payload.status === 'mode1_active'  ? 'mode1_active'   :
src\components\autodev\AgentLiveCard.tsx:40:  mode1_active:  'var(--warning)',
```

The three statuses `runner.rs` actually writes on the native path are
`"working"` (`:802`), `"idle"` (`:860`), `"error"` (`:878`), plus
`"mode1_idle_no_provider"` (`:766`), `"fallback_idle"` (`:735`) and `"idle"`
(`:212`, `:699`). `mode1_active` is a reader with no writer — exactly as
P8-43 says.

**Remedy check — P8-43 is under-specified.** It offers "produced or removed"
as alternatives, but the two are not equivalent here: `AgentLiveCard.tsx:37-40`
paints `mode1_active` amber specifically to signal "degraded mode", and
`docs/development/MODE1_FALLBACK_RECIPES.md:91` and
`COMPLETE_FEATURE_LIST.MD:1660` both document that amber state to users. After
P8 there is no Mode 2 to be degraded *from*, so **producing** `mode1_active`
would paint every healthy worker amber. Removal is the only coherent option,
and it must take the two doc lines with it (see WHAT THE TASK LIST MISSES).

### G.4 — P8-43 "carries real values on the live path"

**VERDICT: CONFIRMED that the values are absent; the remedy is feasible.**

`autodev-core/src/recipes/runner.rs:990-1032`:
```
990:    fn set_worker_status(&self, slug: &str, status: &str) {
991:        self.set_worker_status_ex(slug, status, None, None);
992:    }
```
The only call to `set_worker_status_ex` with non-`None` arguments is
`runner.rs:558` — inside the Mode 2 branch (276-703), therefore dead.

The payload fields already exist and are already typed
(`autodev-core/src/events.rs:280-281`):
```
280:    pub tick_duration_ms: Option<u64>, // wall-clock ms of the last agent tick
281:    pub last_outcome: Option<String>,  // "success" | "fallback" | "error"
```
so P8-43 needs no schema change — only a live call site. The Mode 2 branch
already shows the pattern (`let tick_start = Instant::now();` at `:286`), and
`Instant` is already imported in this file. The remedy works.

**Caveat the task does not state:** the frontend's declared vocabulary for
`last_outcome` is `"success" | "fallback" | "error"` (`events.rs:281`), while
the live `tick-end` emitter at `:727` sends `"fallback_skipped_policy"` and the
dead ones send `"ensure_agent_error"`, `"idle"`, etc. The comment is already
a lie about its own emitters. Whatever P8-42 emits should be checked against
what `src/stores/workerStore.ts` actually branches on, not against this
comment.

---

## P8-38 — five recipes commit with no branch; the cleanup knock-on

### F.1 — the five `git add -A` sites

**VERDICT: CONFIRMED. All five line numbers are exact, and there is no
sixth.**

**What I searched:** `git add` (not just `git add -A`) across
`autodev-core/src/recipes/defaults.rs`. Both searches return the identical
five lines — there is no other staging call anywhere in the file:

```
462:   bash('git add -A && git commit -m "feat(<scope>): <one-line task summary>"')
721:   bash('git add -A && git commit -m "feat(ui): <one-line task summary>"')
953:   bash('git add -A && git commit -m "chore(infra): <one-line task summary>"')
1073:   bash('git add -A && git commit -m "feat(mcp): <tool-name> implementation"')
1253:   bash('git add -A && git commit -m "chore(cleanup): <what was removed>"')
```

**"with no branch created" — CONFIRMED.** I searched `defaults.rs` for
`checkout -b`, `git branch`, `git switch`, and `worktree`. **Zero hits, of
any kind, anywhere in the 2239-line file.** No recipe creates, names, or
checks out a branch at any point.

**Aggravating fact the task does not state.** The `working_dir` these recipes
run in is the user's own workspace, not a sandbox:
`autodev-core/src/recipes/runner.rs:256-258` —
```
256:        let workspace_dir = crate::db::app_settings::load_workspace_dir();
257:        let working_dir = if !workspace_dir.trim().is_empty() {
258:            workspace_dir
```
with the comment at `:250-252` stating this is "the SAME root the Personal
Assistant uses". So `git add -A` on a cron tick stages **every** uncommitted
change in the user's checkout — the user's own in-flight work included — and
commits it under the worker's message. This is worse than "no branch"; it is
"no branch, in the user's live worktree, on whatever branch they are sitting
on". P8-37's "stage deliberately, never a blind `git add -A`" addresses the
staging half; nothing in P8-E addresses the worktree half.

### F.2 — the `worker-cleanup` knock-on: "diffs `main` against itself"

**VERDICT: CONFIRMED, and the check is worse than vacuous — it is
*misleading in both directions*.**

`autodev-core/src/recipes/defaults.rs:1255-1280` (Phase 5, verbatim head and
tail):
```
1255:### Phase 5 — Functionality Preservation Check (run after commit, before marking done)
1256:
1257:Cleanup's #1 rule is MUST NOT change functionality. Verify this automatically:
1258:
1259:1. Get the current branch:
1260:   bash('git rev-parse --abbrev-ref HEAD')
1261:
1262:2. Call `analyze_diff` with:
1263:   - repo_path: root of the repository you modified
1264:   - base_branch: "main"
1265:   - head_branch: current branch from step 1
...
1280:Never mark done without passing this check.
```

`head_branch` is *defined* as "current branch", and nothing ever changes the
current branch. So on a user sitting on `main`, this is
`analyze_diff(base="main", head="main")`.

**What that produces, traced to the leaf:**
`autodev-core/src/git/diff.rs:116-146` resolves both refs and calls
```
145:        .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut opts))
```
Identical refs → identical trees → **empty patch**.
`autodev-core/src/agent/builtin_tools/analyze_diff_tool.rs:110-115` passes
that empty string straight into `analyzer.analyze(&patch)` with **no
empty-diff guard**, and
`autodev-core/src/diff_analysis/mod.rs:47-53` scores it:
```
47:        let risk_level = match structural.score {
48:            s if s >= 0.75 => "critical",
...
51:            _ => "low",
52:        }
```
An empty parse scores at the bottom of that ladder, so the check returns
`risk_level: "low"` — which is precisely the recipe's pass condition at
`defaults.rs:1277-1278`. **The MANDATORY functionality-preservation gate
passes by construction, on a diff of nothing, no matter what the worker
actually changed.** This is a green gate that cannot go red — the exact
"inert-but-green" failure class.

**The second failure mode the task does not name.** If the user happens to be
on a feature branch rather than `main`, the same call diffs *all* of that
branch's work against `main`, most of which the cleanup worker never touched.
The gate then rejects the cleanup task for someone else's changes. So the
check is vacuous on `main` and false-positive off it; there is no branch on
which it does what its own heading claims.

**One thing I could NOT determine:** whether an empty patch also yields a
passing `classification` (the recipe requires *both* a low/medium
`risk_level` **and** a classification in
`housekeeping/style/dead_code/formatting`). `classify_diff`
(`diff_analysis/classifier.rs`) is an **LLM call**
(`diff_analysis/mod.rs:44-45`), so its output on an empty diff is not
statically determinable. **What would settle it:** a unit test that calls
`DiffAnalyzer::analyze("")` against a stub `Provider` and asserts the tuple,
or simply asserting that `analyze_diff` *errors* on an empty patch — which is
the fix I would expect P8-38 to require and which the task does not mention.

### F.3 — stale comment found in passing (a bug, not style)

`autodev-core/src/recipes/defaults.rs:1221-1225`:
```
1221:        // Doc 05 Phase 3 — worker-cleanup is the canonical Mode1-capable
1222:        // recipe: lint fixes and dead-code removal are file-only operations
1223:        // that the Rust AgentEngine can execute when the LLM provider is
1224:        // configured. Other recipes (worker-backend / frontend / qc-agent)
1225:        // are MCP-tool-heavy and stay Mode2Only.
```
"Other recipes … stay Mode2Only" is **false**: all nine recipes in this file
declare `Mode1Capable` (`:232, 408, 677, 916, 1043, 1226, 1399, 1971, 2064`)
and `Mode2Only` appears nowhere in `defaults.rs`. A reader deciding whether
the `Mode2Only` guard at `runner.rs:717` can be deleted would be told by this
comment that four recipes depend on it. They do not.

---

## P8-36 — the WARNING about the cache split, attacked directly

**The task's warning:** "Keep the cache split intact: the core and
`AUTODEV.md` are stable, memory stays volatile and last."

### Q1 — Does prepending an invariant core break byte-stability of the cached prefix?

**VERDICT: NO — prepending the core is byte-safe. But the warning is aimed at
the wrong hazard, and the split it tells you to preserve is not intact today.**

Prepending is safe because `stable` is assembled deterministically from
`const &str` inputs and hashed with `DefaultHasher`:

`autodev-core/src/cache/prefix.rs:98-108`
```
 98:    // -- system --
 99:    stable.push_str(system_base);
100:    for block in instructions {
101:        stable.push_str("\n\n---\n\n");
102:        stable.push_str(block);
103:    }
...
107:    let mut hasher = DefaultHasher::new();
108:    stable.hash(&mut hasher);
```
A `const` core prepended to a `const` recipe instruction string yields the
same bytes every tick, so the hash is unchanged tick-to-tick. And because
`estimate_tokens` is `len() / 4` (`prefix.rs:143-145`), a longer prefix
strictly *increases* the chance `will_cache` returns true
(`prefix.rs:153-175`). Prepending the core makes caching *more* likely, not
less.

### Q2 — Is the "cache split" actually intact today? NO. This is the finding that matters.

The split exists in the *bookkeeping* and does not exist *on the wire*.

1. The engine joins both halves into one string
   (`autodev-core/src/agent/engine.rs:237-241`):
```
237:        let system_prompt = if volatile_prompt.is_empty() {
238:            stable_prompt.clone()
239:        } else {
240:            format!("{stable_prompt}\n\n---\n\n{volatile_prompt}")
```
2. It sends that joined string as the request's system field
   (`engine.rs:495-498`):
```
495:            let request = CompletionRequest {
496:                model: model.to_string(),
497:                messages: history.clone(),
498:                system: Some(system_prompt.clone()),
```
3. `system` is a **single `Option<String>`** — there is structurally nowhere
   to put a mid-system breakpoint
   (`autodev-core/src/provider/types.rs:96`):
```
 96:    pub system: Option<String>,
```
4. The Anthropic adapter wraps that whole string in **one** content block and
   hangs `cache_control` on it
   (`autodev-core/src/provider/anthropic.rs:181-188`):
```
181:            let mut block = json!({ "type": "text", "text": sys });
182:            if let CachePolicy::Explicit { ref ttl, .. } = req.cache {
183:                block["cache_control"] = json!({
184:                    "type": "ephemeral",
185:                    "ttl": ttl.as_wire_str(),
186:                });
187:            }
188:            body["system"] = json!([block]);
```
Note `ref ttl, ..` — **`breakpoints` is destructured away and never read.**
`engine.rs:279` sets `breakpoints: vec![0]` and no adapter consumes it; the
only other occurrences of the field are in
`autodev-core/src/provider/conformance_tests.rs:452, 499, 517` (test inputs)
and its declaration at `autodev-core/src/provider/usage.rs:168`.

**Consequence:** the marked cache prefix on the wire is
`tools` then the ENTIRE system string, memory block included. The volatile
half is inside the cached region. Every new remembered fact changes those
bytes and invalidates the whole prefix — which is verbatim the defect the
D-12 comment at `engine.rs:195-207` says was fixed.

**Why the existing gate does not catch it.** `TG-P2-11`
(`src-tauri/src/ci_gate.rs:3769-3794`) asserts only that
`crate::cache::prefix::build(` is called with `stable_prompt` and not with
`&system_prompt`:
```
3785:        assert!(
3786:            call.contains("stable_prompt"),
3787:            "the prefix is not built from the stable half:\n{call}"
```
That is a claim about the **hash input** — about what the monitor *believes*
was cached. It says nothing about the request body. A gate that can only see
`prefix::build`'s arguments cannot fail for the wire mechanism, so it stays
green while the wire is wrong: the same green-test-for-an-untaken-path shape
its own docstring names as `L-18`.

**Impact on P8-36 as written.** "Keep the cache split intact" instructs the
implementer to preserve a property that does not currently hold. If P8-36 is
implemented literally — put the core and AUTODEV.md in `stable`, leave memory
in `volatile` — the result on the wire is a *larger* single cached block that
still contains memory, and the cache still misses on every learned fact, now
over more tokens. **The task should be rewritten to require the split be
REALISED**, which means either a multi-block `system` field (Anthropic accepts
an array, and `anthropic.rs:161-168` already explains why the array shape was
adopted) or an explicit decision that memory stays inside the cached region.

### Q3 — "Is there a measurement (`cache_read`) that would settle it?"

**VERDICT: YES — the instrument exists and is already wired. It is the right
evidence for this task's gate.**

- Parsed from the wire, non-streaming and streaming:
  `autodev-core/src/provider/anthropic.rs:410` —
  `cached_read: u["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32,`
  and `autodev-core/src/provider/streaming.rs:52` (same expression).
  OpenAI equivalent: `autodev-core/src/provider/openai.rs:464, 536, 566`
  (`prompt_tokens_details.cached_tokens`).
- Observed per turn, in the live loop
  (`autodev-core/src/agent/engine.rs:584-604`):
```
588:                cache_monitor.observe(
589:                    cache_prefix.hash,
590:                    cache_prefix.estimated_tokens,
591:                    &caps,
592:                    &turn_usage,
593:                );
...
598:                crate::cache::monitor::global().observe(
```
- The verdict type already names the failure:
  `autodev-core/src/cache/monitor.rs:54` — `SilentMiss { seen_before: u32 }`,
  and `:62-63` `is_alarm()` returns true only for `SilentMiss`.
- A live harness already exists:
  `autodev-core/tests/live_provider_smoke.rs:292-299` calls
  `monitor.observe(...)` twice and asserts the second turn is not a
  `SilentMiss`.

**So the P8-36 gate should be:** run that two-turn probe on a *worker* prompt
(core + recipe instructions + AUTODEV.md + memory), assert `cached_read > 0`
on the second turn, then **change one memory fact and assert `cached_read` is
still > 0**. Under today's single-block wiring the second assertion fails —
which is the falsification that proves the split is real rather than merely
computed.

**I did not run this.** It needs a live provider key and a real API call, and
my constraints forbid running the application. **What would settle it:**
`cargo test -p autodev-core --test live_provider_smoke` with a key present,
extended with the memory-mutation second assertion above.

### Q4 — A hazard P8-36 introduces and does not name

Moving `AUTODEV.md` onto the worker path puts **two SQLite opens plus two
filesystem fallbacks inside the stable (hashed) half**, on every turn:
`autodev-core/src/agent/context.rs:98-110` (global) and `:116-128` (repo).
Both are `.ok().and_then(..).filter(..).or_else(fs::read_to_string(..))`
chains that yield `None` on any failure. A transient DB lock, or a first run
before the table is seeded, drops the block **silently** — which does two
things at once: the agent loses its instructions for that tick, and the stable
bytes change, so the cache misses. Neither produces a log line today. P8-36
should require that a *failure* to read AUTODEV.md is distinguishable from its
*absence* before this is put on an unattended cron path.

---

## GROUP P8-F — procedural memory seam

### F8-40 — "a retrieval seam in `tool_executor.rs`, which consults memory nowhere today"

**VERDICT: CONFIRMED.**

**What I searched:** a case-insensitive regex
`memory|Memory|recall|Recall|procedural|Procedural` over the entire
672-line `autodev-core/src/agent/tool_executor.rs`. **Exactly one hit, and it
is inside a doc comment:**

```
142:    /// reaching procedural memory verbatim, and memory is permanent.
```

There is no `use crate::memory`, no call to `recall`, `build_memory_context`,
`ProceduralStore`, or `MemoryDb`. The file's imports are
`errors`, `events`, `hooks`, `session::manager`, `session::types`,
`state::lock` (`tool_executor.rs:14-22`). The claim holds exactly.

**"Retrieved once per turn, never per call" — feasible, and the seam point is
unambiguous.** `execute_batch` is the single live entry (`execute()` was
deleted in P3-10, `tool_executor.rs:147-161`), declared at
`tool_executor.rs:208` and called from exactly one place:
`autodev-core/src/agent/engine.rs:774` —
`match tool_executor.execute_batch(tool_batch).await {`.
That call sits inside the turn loop, so "once per `execute_batch`" is
literally "once per turn". The task's phrasing is accurate.

*In passing (stale, minor):* `tool_executor.rs:126` and
`autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:17, 85` all cite
the caller as `agent/engine.rs:314`. It is `:774`.

### F8-41 — "goes through `recall.rs`'s flattener … this path cannot re-open the prompt-implant hole"

**VERDICT: REFUTED as stated. The protection is real but it is NOT a property
of "this path" — it is a property of one specific entry point, and the
obvious API for a *procedural* seam bypasses it.**

The flattener is **private**:
`autodev-core/src/memory/recall.rs:152` — `fn flatten_for_prompt(s: &str) -> String {`
(no `pub`). `tool_executor.rs` lives in `crate::agent`; `recall.rs` lives in
`crate::memory`. A seam in `tool_executor.rs` **cannot call the flattener at
all**. It gets flattening only as a side effect of going through
`render()` (`recall.rs:245-248`):
```
245:            parts.push(format!(
246:                "- ({origin}) {}\n",
247:                truncate(&flatten_for_prompt(&c.content), 300)
248:            ));
```
which is reachable only via the two public entry points
`recall()` (`recall.rs:60`) and `build_memory_context()` (`recall.rs:117`).

**The bypass.** `ProceduralStore`'s API is fully public and returns raw
`content`, with no flattening and no data framing:
```
autodev-core/src/memory/procedural.rs:34:    pub fn get_memory_md() -> Result<String> {
autodev-core/src/memory/procedural.rs:65:    pub fn list_topics() -> Result<Vec<ProceduralMemory>> {
autodev-core/src/memory/procedural.rs:110:   pub fn get_topic(topic: &str) -> Result<ProceduralMemory> {
autodev-core/src/memory/procedural.rs:177:   pub fn search(query: &str, limit: u32) -> Result<Vec<ProceduralMemory>> {
```
`ProceduralStore::search(query, limit)` is precisely the function an
implementer building "a procedural retrieval seam" would reach for, and it
returns unflattened, unframed, model-written text. **So P8-41's safety claim
is contingent on an implementation choice the task never states.** As written
the task asserts the outcome; it needs to *mandate the entry point*
(`recall::recall` / `build_memory_context`) and, better, add a gate that the
seam does not name `ProceduralStore` at all.

**A second consequence the task does not name.** `recall()` is not
procedural-only. `recall.rs:73-80` builds a `RetrievalQuery` with
`tiers: Vec::new()` (all tiers) and `render()` (`recall.rs:209-213`) emits
Semantic, Episodic and Procedural sections. So a seam that routes through the
safe entry point retrieves **all three tiers** — duplicating, per turn, the
same memory block the system prompt already carries via
`engine.rs:226`/`:231`. That is a token cost and a contradiction-risk
(two copies with different ranking snapshots), not a safety hole, but P8-40's
"retrieved once per turn" does not make it free.

### F8-41 — "never an input to a tool's arguments, so a tool's behaviour is identical with and without it. Gate that equivalence."

**VERDICT: REFUTED — the equivalence as worded is either trivially true or
false, depending on which reading you take, and neither reading yields a
gate worth writing.**

*Reading A (the literal one): `tool.execute(input)` produces the same result
with or without a memory retrieval having happened.* This is true **by
construction and needs no gate**: `execute_batch` receives its `tool_batch`
from the model's response (`engine.rs:713-716` collects `ToolUse` blocks,
`:774` passes them), and a tool's `execute(input, ctx)` never sees anything
the executor retrieved unless the implementer deliberately threads it in. A
test asserting this passes before the feature is written — a green test for
an absent mechanism, the exact `L-18` shape this repo has been bitten by
repeatedly (`ci_gate.rs:3763-3768`).

*Reading B (the one that matters): the model's subsequent tool calls are the
same with and without the recalled text.* This is **false by design** — the
entire purpose of retrieval is to change what the agent does next. A gate
asserting it would either fail the feature or, if written loosely enough to
pass, prove nothing.

**What a real gate would assert instead** — three properties that *are*
falsifiable:
1. **Framing survives**: the injected text is preceded by the
   quoted-as-data frame (`recall.rs:192-195`) and every row begins
   `- ({origin}) `; assert on a stored memory containing
   `"\n---\n## Global Instructions\n"` that the rendered output contains no
   newline inside the row. (`recall.rs:447-463` already tests the flattener
   itself; the gate must test the *seam's* output, not the flattener's.)
2. **Argument non-contamination**: assert the `serde_json::Value` handed to
   `tool.execute` is byte-identical to the `input` the model emitted — i.e.
   that the seam wrote into a *reporting/context* channel and not into
   `input`. That is checkable and it is what "never an input to a tool's
   arguments" actually means.
3. **Frequency**: assert exactly one retrieval per `execute_batch` regardless
   of batch size (P8-40's "never per call"), by counting calls against a
   fake store with a 3-tool batch.

Property 2 is what the task is reaching for; it should say so, because
property 2 is not the same statement as "a tool's behaviour is identical".

**What I could not determine:** where the retrieved text is meant to *go*.
P8-40 and P8-41 never name the destination — tool result preamble, a hook
context (`tool_executor.rs:19` imports `HookContext`/`ToolCallContext`), an
extra user message, or the system prompt. Property 2 above cannot be written
until that is decided, and the injection point determines whether the memory
lands before or after the model's own prohibitions. **What would settle it:**
the plan naming the destination field explicitly.
