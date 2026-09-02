# Razi's notes — my findings (main-session, read from the code)

Notes 1, 2, 4, 7 investigated directly. Notes 1-EXTRA, 3, 5, 6 are with subagents.

---

## Note 1 · Do all workers share one base system prompt?

**Answer: no — and the real problem is the opposite of the one you feared.**

### What actually happens

`recipes/types.rs:81` — a `Recipe` carries `instructions: String`, commented
*"system prompt injected into agent"*. `recipes/runner.rs:849` passes it to the
engine as `system_prompt_override`. And `agent/engine.rs:208-227` treats an
override as a **total replacement**:

```rust
Some(override_str) => (
    override_str.to_string(),
    context_builder.memory_block(working_dir, user_message),
),
None => context_builder.build_system_prompt_parts(self.pa_mode, working_dir, user_message)
```

So the prompt each kind of agent actually receives:

| Agent | Stable prompt | Gets AUTODEV.md? |
|---|---|---|
| Interactive PA (Mode 1) | `BASE_SYSTEM_PROMPT` — 7,257 chars, 7 sections | **yes** |
| PA · Review | `REVIEW_SYSTEM_PROMPT` — 1,398 | yes |
| PA · Delegate | `DELEGATE_SYSTEM_PROMPT` — 1,788 | yes |
| **Fleet worker (any recipe)** | **only `recipe.instructions`** + `universal_env_bootstrap()` | **NO** |
| Subagent with an explicit prompt | only that prompt | **NO** |
| Subagent without one | `BASE_SYSTEM_PROMPT` | yes |

The five worker recipes are **strongly differentiated** — 5,443 to 12,483 chars
each, with genuinely distinct execution protocols, domain rules and rejection
criteria. `worker-cleanup` for instance carries a 5-criterion "Demonstrably
Unused Standard" and a mandatory `analyze_diff` functionality check that the
base prompt has no equivalent of.

**So specialisation is real and it lives in the recipe.** A backend worker and a
frontend worker do not behave the same.

### The actual defect: nothing universal is universal

Because the recipe *replaces* rather than *extends*, two things that should
reach every agent reach only the interactive PA.

**(a) The seven prohibitions in `## What You Must Never Do` are invisible to the
entire fleet.** Measured across the five worker recipes:

| Base prohibition | Reaches a worker? |
|---|---|
| Never commit secrets / API keys | 4 of 5 — **`worker-cleanup` has none** |
| Never push directly to `main` — branch + PR | **0 of 5** |
| Never modify files outside your domain | 2 of 5 |
| Never mark complete without running tests | 0 of 5 *(they have their own gates instead)* |
| Never create a PR without linking the Issue | **0 of 5** |
| Never `git push --force` | **0 of 5 — the string does not appear anywhere in `defaults.rs`** |
| Never skip a failing test | 0 of 5 |

⚠ I first ran this as a keyword probe and it reported `worker-cleanup` as
missing all seven. Reading the file corrected that: it is heavily guarded, just
*domain-specifically*. The table above is what survived reading.

**The concrete consequence.** All five workers are instructed to commit like
this — `defaults.rs:462, 721, 953, 1073, 1253`:

```
bash('git add -A && git commit -m "feat(<scope>): <one-line task summary>"')
```

No branch is created first. On a fresh clone that is `main`. It is `git add -A`,
which is exactly what "never commit secrets" exists to guard, and
`worker-cleanup` — the one told to `add -A` while deleting things — is also the
one recipe with no secrets rule. Nothing pushes, so it stays local; that is the
only reason this is contained.

**(b) `AUTODEV.md` never reaches a worker.** The user's global and per-repo
instructions are appended inside `build_system_prompt_parts`, which the override
branch skips entirely. So the file a user writes to teach AutoDev their project's
conventions is read by the interactive assistant and **by none of the agents that
actually do the work**.

### What I recommend discussing

The choice is not "shared vs per-worker" — it is **what the shared part should
contain**. Three options:

1. **Universal preamble + recipe body.** Split the base prompt into a small
   invariant core (identity, the prohibitions, AUTODEV.md) and the PA-specific
   remainder. Every agent gets core + its own instructions. Recipes stay fully
   specialised; the safety floor stops being optional.
2. **Leave replacement, duplicate the rules into all nine recipes.** Cheap now,
   and it is nine copies to keep in step — the shape that produced this gap.
3. **Status quo.** Only defensible if the fleet is not meant to touch git, which
   contradicts the recipes as written.

**My recommendation: option 1**, and it is small — the invariant core is the
7 prohibitions plus the AUTODEV.md block, both of which already exist as text.
The gate is "no agent runs without the core", which is falsifiable.

**Related question this raises for you:** should `git add -A` and unbranched
commits be in the recipes at all? That is a rule I would normally not decide
unilaterally, and it is one line in five files.

---

## Note 2 · Procedural memory at tool execution

**Current state:** `agent/tool_executor.rs` does not consult memory at all. The
only occurrence of the word is a comment about redaction (`:142`). So this is a
new capability, not a repair.

**Your constraint is the right one** — "ensure it does not poison the context."
Three concrete poisoning risks, all of which have already bitten this codebase:

1. **Volume.** Recall on every tool call, with the fleet running many calls per
   tick, multiplies prompt size by the number of calls. `TG3-08` already asks
   for bounded input-token growth.
2. **Injection.** `P3-F` closed a hole where recalled memory could author the
   system prompt. Injecting memory into a *tool* path re-opens that surface at a
   place with no equivalent flattening today.
3. **Determinism.** Tool behaviour that varies with what the agent happens to
   remember is not reproducible, which breaks the replay evidence class.

**Shape I would propose** (for your ruling, not decided): procedural memory is
retrieved **once per turn**, not per call; it is scoped to the tool names the
model actually invoked; it is rendered through the same flattener `recall.rs`
now uses; and it is *advisory text in the turn's context*, never an input to the
tool's arguments. Plus a gate that the tool's own behaviour is unchanged by the
presence or absence of memory.

**Where it belongs:** `P3-F` is closed, and reopening a closed phase to append
tasks is the thing `§1` forbids. Procedural memory helps a repeating agent most,
so **P4 (fleet)** is the natural home — a worker that has run this build before
is exactly the beneficiary. Needs your call on placement.

---

## Note 4 · Who executes the design work — PA or fleet?

**You are right that it is undefined. I checked and it is not a matter of
interpretation:** across the whole of `PD1`–`PD6` in the MDP, the words
*personal assistant*, *PA*, *fleet*, *worker* and *supervisor* appear **zero
times**. The phases define capabilities and say nothing about who drives them.

What *is* decided:

- `PD0-01` freezes four verbs: `ingest` · `export` · `adapt` · `change`.
- `PD0-03` makes them **async jobs** with terminal statuses and events.
- `PD4-02` is explicit: **"No model call inside the tool"**, enforced as a gate.

So `adapt` is deterministic by decision. That resolves part of it — adaptation
is a *tool*, not an agent task. But three things remain open:

1. **Who chooses the parameters** (target size, ratio policy, safe areas) when
   AutoDesign does not supply them?
2. **Who handles `degraded`** (`PD0-04`) — an unresolvable font, a rasterised
   blend mode? That is judgement, and judgement means an agent.
3. **`PD6` (brand guideline) is not deterministic** — "ingest a brand guide and
   apply it as a batch of changes" is interpretation. Nothing says who does it.

⚠ **And there is a sequencing problem in the plan.** `P4` (fleet) is scheduled
**after** `PD6`. So through all of Rollout 1 — including handover — there is no
working fleet. If the answer to "who executes" is "the fleet", the plan has the
dependency backwards.

**My reading:** Rollout 1 should answer the contract with **deterministic tools
plus the PA for the judgement cases**, and the fleet becomes an *additional*
consumer in Rollout 2 rather than a prerequisite. But this is a genuine
architectural decision and it is yours. It also needs a `PD0` task, because the
contract should state the execution model rather than leave AutoDesign to infer
it.

---

## Note 7 · Should adaptation be exposed in AutoDev's own UI?

**Already in the plan, and in the right rollout.** Two tasks exist:

- `P6-25` — *"The design surfaces (`PD1`–`PD6`) get UI — the standalone product
  needs what AutoDesign supplied in R1."*
- `P6-28` — *"Adaptation and change tools are drivable from the UI."*

That matches your instinct and puts it in **Rollout 2**, which is right: in
Rollout 1 AutoDesign *is* the front end, so building a second one would be
duplicated work against a partner product.

**Where I think the plan is wrong: it is under-scoped.** Two tasks for what you
describe — "its own panel with its own canvas like Photoshop" — is not credible.
A canvas editor is layer tree, selection, transform handles, zoom/pan, snapping,
undo/redo, a properties panel, and rendering the IR faithfully at interactive
speed. That is a phase, not two checkboxes.

**For discussion:** either scope `P6-25`/`P6-28` up into a proper task group
(my estimate: 15–20 tasks), or split the design UI into its own phase. I would
also ask whether the canvas needs to be *editable* in R2 or whether
**view + adapt + export** is enough for the first standalone release — editing
is where most of the cost is, and AutoDesign already does it.

---

## What I have NOT done

No plan changes, no architecture changes. Every item above is a proposal for
the discussion, per your "let's discuss when I wake up."

---

## Addendum to Note 1 — a consequence worth its own line

`worker-cleanup`'s Phase 5 is a **mandatory** functionality-preservation check:
it reads the current branch, then calls `analyze_diff(base_branch="main",
head_branch=<current>)` and refuses to mark the task done unless the risk level
is low/medium and the classification is housekeeping-shaped.

That check **assumes the worker is on a feature branch**, and no recipe anywhere
creates one — every worker runs `git add -A && git commit` directly. On a fresh
clone the current branch *is* `main`, so the call becomes
`analyze_diff(base="main", head="main")` — a diff of a branch against itself.

`analyze_diff_tool.rs:110` generates the patch via `git::diff::branch_diff` and
hands it to an LLM analyzer, so I cannot assert the exact verdict without a run.
But the structural point does not need one: **the one hard safety gate in the
cleanup worker is being asked to judge an empty patch in the default case.**

Whichever way the prompt question is decided, "create a branch before the first
commit" is a one-line fix in five recipes and should not wait for it.
