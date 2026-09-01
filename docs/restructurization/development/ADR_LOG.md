# AutoDev Restructure — Runtime ADR Log

> **Append-only build-time decision journal.** A reversal is a new `SUPERSEDED-BY` entry, never an edit. Retractions of one's own findings are entries too — record the **mechanism of the error**, not just the correction.

| | |
|---|---|
| **Parents** | `MASTER_DEVELOPMENT_PLAN.md` §0.11 · `../vision/00-…-MASTER-PLAN.md` Part 8 |
| **Rev** | 1.0 — 2026-08-31 |
| **Entries** | **0** — no build-time decisions yet; awaiting GO |

---

# §0 · Register relationships

Three registers, three jobs. Confusing them is how a decision gets made twice with different answers.

| Register | Holds | Written when |
|---|---|---|
| `../vision/00-…-MASTER-PLAN.md` **Part 8** | **Pre-build truth** — the 51 ratified architectural decisions | During planning, before GO. **Closed.** |
| **`ADR_LOG.md`** (this) | **Build-time decisions** — everything decided while implementing | Continuously, during the build |
| House `DECISIONS.md` | The **ratified register** — promoted, durable decisions | At phase closeout, by promotion |

**The promotion flow:**

```
a decision is made during a task
   ↓  immediately (§0.11 — an acted-on-but-unlogged decision is a no-stale violation)
ADR_LOG entry, status DECIDED
   ↓  at phase closeout, if ratifiable
house DECISIONS.md, with a back-reference to the log ID
   ↓
ADR_LOG entry status → PROMOTED
```

**Phase-0 duty:** mirror the ratified pre-build decisions from vision Part 8 into the house `DECISIONS.md` as the starting register, each with a back-reference to its ADR id. This log stays empty until the first build-time decision.

---

# §1 · ID namespace

**`ADR-LOG-RESTRUCT-NNNN`**, numbered from 0001, **namespaced from day one**.

Two logs sharing one bare numbering space has already caused cross-pillar collisions in this house. More pointedly, **this very programme produced an ID collision during planning** — two different decisions were both numbered `ADR-02-06`, and both were answered before it was caught. The pillar prefix and the never-renumber rule exist because of exactly that.

**Never renumber history.** If an entry is wrong, supersede it.

---

# §2 · Authorship and tiering

Razi may rule at any time. Claude logs every decision under one of three tiers:

| Tier | Means | Requires |
|---|---|---|
| **`decided-independently`** | A clear precedent exists in the shipped codebase or the vision suite | Log it; proceed |
| **`flagged-and-proposed`** | A new dependency, pattern, or scope change — **or anything that narrows a §0 rule** | Log it, propose, and **wait**. Narrowing a §0 rule is always escalated, without exception |
| **`razi-ruled`** | Razi decided | Log it verbatim, then reword into the polished decision voice preserving intent |

**The escalation rule that matters most:** if implementing a spec faithfully would require narrowing an invariant or a §0 rule, that is never a quiet judgement call. It is `flagged-and-proposed`, and it stops.

---

# §3 · Entry format

Copy this block for every entry.

```markdown
## ADR-LOG-RESTRUCT-NNNN — <short title>

**Status:** PROPOSED | DECIDED | SUPERSEDED-BY ADR-LOG-RESTRUCT-NNNN | PROMOTED
**Author:** <who>
**Tier:** decided-independently | flagged-and-proposed | razi-ruled

**What.** <The choice, precisely. One or two sentences. No hedging.>

**Why.** <What breaks or is worse without it. Cite the grounding fact —
a file:line, a gate, a measured number. "It seemed cleaner" is not a why.>

**When.** <date · phase P<n> · triggering task P<n>-<nn>>

**Where.** <exact repo paths touched>

**How.** <The implementation, and the gate/test/evidence that proves it.
Name the evidence class.>

**Links.** <vision ADR ids · spec sections · TG gate ids · grounding report ids>
```

---

# §4 · Entries

## ADR-LOG-RESTRUCT-0001 — Subagents move to an in-process native runner in P0, not P3

**Status:** DECIDED
**Author:** Razi
**Tier:** razi-ruled

**What.** `P0-13`'s deletion of `src-tauri/src/mcp_server/` proceeds, and `subagent/bridge.rs` is replaced in the same phase by a native in-process runner that executes a subagent on a nested `AgentEngine`. The sidecar round trip for subagents is retired in P0 rather than P3.

**Why.** Grounding at P0 start found the plan had mis-modelled this dependency. Subagents do not run on the Rust engine at all: `subagent/bridge.rs:1-10,48-100` drives a create → invoke(SSE) → delete lifecycle against `core-orchestrator` via `crate::sidecar::client::OrchestratorClient`, and `mcp_server/` is the **inbound** surface those sidecar-hosted agents call back through to reach AutoDev's tools. The shipped recipes mandate that path: `recipes/defaults.rs:1826` instructs `spawn_qc_review(task_id=<task.id>)` and `:1841` says "always use `spawn_qc_review`"; five worker recipes name `spawn_subagent`/`spawn_parallel_subagents` as their delegation model. Deleting `mcp_server/` without a replacement would therefore have left the mandated QC path dead from P0 to P3. Two further facts made the native runner the stronger option: the depth guard today is **model-self-reported** — `subagent/bridge.rs:86-92` injects "Include `__depth: {depth}` in every tool call" into the prompt and `mcp_server/tool_adapter.rs:224-228` reads it back, with `bridge.rs:78` stating outright that without it "the tool_adapter always sees depth=0" — so a model that omits or lies about `__depth` defeats `MAX_NESTING_DEPTH`; and the sidecar path needs Python + `uv` + the `core-orchestrator` submodule, which is in a broken half-state (`MDP §1.1a`), so subagents are already non-functional for any real install.

**When.** 2026-08-31 · phase P0 · triggering task `P0-13` / `P0-13a`

**Where.** `src-tauri/src/subagent/bridge.rs` (replaced) · `src-tauri/src/subagent/{manager,types}.rs` · `src-tauri/src/agent/builtin_tools/{spawn_subagent,spawn_parallel_subagents,spawn_qc_review}.rs` · `src-tauri/src/agent/{engine,tool_executor}.rs` · `src-tauri/src/agent/tool_trait.rs`

**How.** New `subagent/native_runner.rs` constructs an `AgentEngine` for the child with the parent's `working_dir`, the child's role and `depth = parent.depth + 1`, runs it to completion and returns a `SubagentResult`. `depth` and `agent_role` become real fields threaded `AgentEngine::new → ToolExecutor::new → ToolContext`, so both are Rust-authored and unforgeable by the model. Evidence: `SMOKE-PROVEN` via `S0-06` (an ACP tool called on the native path resolves a real `agent_role`) and `GATE-PROVEN` via the P0 reachability/guard gates; the depth guard gets a test that nests to the limit and asserts refusal.

**Links.** vision `01-AGENT-RUNTIME` · `04-THE-FLEET` · spec `10 §4` "Guard-the-guard" · `_verify/V2-P0BC-vault-mcp.md` findings 1–3 · MDP `§6 P0-13`, `P0-13a`

---

## ADR-LOG-RESTRUCT-0002 — The update trust root becomes compile-time; the runtime env override is deleted

**Status:** DECIDED
**Author:** Claude
**Tier:** flagged-and-proposed

**What.** `P0-01` is widened. Replacing the placeholder constant is not sufficient: the public key is made a **compile-time** value via `option_env!("AUTODEV_UPDATE_PUBKEY")`, and the runtime `std::env::var("AUTODEV_UPDATE_PUBKEY")` lookup is deleted outright. A release build with no key configured fails to compile rather than shipping a permissive binary.

**Why.** `updater/verify.rs:46-47` reads the key with `std::env::var` at **runtime**, while the module header at `verify.rs:3-4` claims it is "embedded at compile time from AUTODEV_UPDATE_PUBKEY env var". Three consequences, all live: (1) anything that can set an environment variable for the AutoDev process substitutes its own update-signing trust root — and `main.rs:28` runs `dotenvy` walking up from the CWD, so **dropping a `.env` file beside the binary is sufficient**; (2) the CI secret does nothing — `.github/workflows/build-release.yml:34` sets `AUTODEV_UPDATE_PUBKEY` as a build-time job env var and `src-tauri/build.rs` emits no `cargo:rustc-env`, so the key configured in CI has zero effect on the shipped binary, a misconception `installers/wix/build-msi.ps1:133-135` repeats; (3) `P0-01` as originally written would have swapped the constant and left all of this standing. A docstring promising one behaviour while the code does another is a bug, not a style issue (house rule 5).

**When.** 2026-08-31 · phase P0 · triggering task `P0-01`

**Where.** `src-tauri/src/updater/verify.rs` · `src-tauri/build.rs` · `.github/workflows/build-release.yml` · `installers/wix/build-msi.ps1`

**How.** `build.rs` gains `cargo:rerun-if-env-changed=AUTODEV_UPDATE_PUBKEY`; `verify.rs` resolves the key from `option_env!` only, with `compile_error!` on a release build when it is absent. Evidence: `UNIT-PROVEN` — a test asserts a signature made by a non-matching key is rejected, and `GATE-PROVEN` that no `std::env::var` reference to the pubkey survives.

**Links.** spec `09 §3.1` · `_verify/V1-P0A-updater.md` C1 · MDP `§6 P0-01`

---

## ADR-LOG-RESTRUCT-0003 — Hard-failing on an unsigned update requires shipping release signing in the same phase

**Status:** DECIDED
**Author:** Claude
**Tier:** flagged-and-proposed

**What.** `P0-02` (empty signature URL is a hard failure) ships together with a release-signing step that produces a `.sig` asset per artifact. The verification half is not landed alone.

**Why.** No `.sig` asset is produced anywhere in the repo — zero hits across `.github/`, `scripts/` and `installers/` — so `sig_url` is **always** empty and the Ed25519 path has never executed in production. `P0-02` landed by itself would therefore reject 100% of releases: a correct security posture applied to an artifact pipeline that cannot satisfy it, which reads to a user as "updates are broken". This is the house rule that a load-bearing claim needs a check that can fail, applied in reverse — a check that always fails is equally useless.

**When.** 2026-08-31 · phase P0 · triggering task `P0-02`

**Where.** `.github/workflows/build-release.yml` · `src-tauri/src/updater/verify.rs` · `docs/` (key-generation runbook)

**How.** The release workflow signs each built artifact with the Ed25519 private half held in CI secrets and uploads `<artifact>.sig` alongside it. **Razi's action, not Claude's:** generating the keypair and setting the `AUTODEV_UPDATE_PRIVKEY` / `AUTODEV_UPDATE_PUBKEY` repository secrets — Claude does not generate, handle or enter the private key. A runbook with the exact commands and secret names ships with the task. Evidence: `UNIT-PROVEN` for the verification half now; the signing half is `CODE-PRESENT` until a release runs, and is recorded as such rather than claimed green.

**Links.** spec `09 §3.1` · `_verify/V1-P0A-updater.md` §"What the claims miss" · MDP `§6 P0-02`

---

## ADR-LOG-RESTRUCT-0004 — `filename_from_url` is replaced by an allow-list, not a `..` filter

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** `P0-07` sanitises by **deriving** a safe filename rather than by rejecting bad ones: parse the URL, take the last path segment, percent-decode it, strip any directory component, and accept only a bounded character allow-list with a known extension — falling back to a deterministic generated name.

**Why.** The task as written named `..` traversal, which understates the hole. `download.rs:184-186` is `url.split('/').last()`, operating on the **raw** URL string before any normalisation, and its result is passed to `download_dir()?.join(...)` at `download.rs:58`. A final segment that is an absolute Windows path (`C:\...\x.exe`) makes `PathBuf::join` **discard the base directory entirely** — a write-anywhere primitive, not merely an escape. `\` is not a separator to `split('/')`, query strings survive into the filename (illegal `?` on Windows), and `unwrap_or("autodev-update")` is unreachable because `str::split` always yields at least one item, so a URL ending in `/` yields `""` and makes `dest` the directory itself. A blocklist for `..` fixes none of the other four.

**When.** 2026-08-31 · phase P0 · triggering task `P0-07`

**Where.** `src-tauri/src/updater/download.rs`

**How.** Rewritten helper plus a unit test table covering `..\`, an absolute path, a query string, a trailing slash and a percent-encoded separator; each must land inside the updates directory or be refused. Evidence: `UNIT-PROVEN`.

**Links.** `_verify/V1-P0A-updater.md` C7 · MDP `§6 P0-07`

---

## ADR-LOG-RESTRUCT-0005 — `get_github_pat` is replaced by `has_github_pat`, not merely deleted

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** `P0-09` deletes `get_provider_key` outright, but `get_github_pat` is **replaced** in the same commit by a `has_github_pat` command returning a boolean.

**Why.** The task assumed both commands were unused. `get_provider_key` is (zero call sites), but `get_github_pat` has one: `src/tabs/Settings.tsx:462`, where it is used purely as a presence probe to render a badge — and its `.catch` swallows the failure. Deleting it therefore does not error; it makes the PAT badge read "none" **forever, silently**, which is precisely the class of defect this restructure exists to remove. The vault's asymmetric-IPC invariant (INV-12, inbound only) is satisfied by returning a boolean rather than the secret.

**When.** 2026-08-31 · phase P0 · triggering task `P0-09`

**Where.** `src-tauri/src/commands/vault_commands.rs` · `src-tauri/src/lib.rs` (command registration) · `src/tabs/Settings.tsx:462`

**How.** `has_github_pat` calls the same `VaultManager` lookup and returns `Ok(bool)`; `Settings.tsx` consumes the boolean and its `.catch` is replaced by an explicit error state. Evidence: `SMOKE-PROVEN` via `S0-03` (badge reflects a stored PAT, and an induced vault error surfaces rather than reading "none").

**Links.** vision `06-TOOLS-AND-PERMISSIONS` INV-12 · `_verify/V2-P0BC-vault-mcp.md` C2 · MDP `§6 P0-09`

---

## ADR-LOG-RESTRUCT-0006 — The MCP deletion set is widened to its true dependency closure

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** `P0-13`'s deletion set is corrected from three files to the full closure: the three files **plus** the seven Rust call sites that import from `crate::mcp_server`, the four frontend surfaces that render MCP state, and the hardcoded status at `sidecar/mod.rs:439`. `socket2` joins `rmcp`/`axum` in `P0-16`.

**Why.** The set as written does not compile. `V2` inventoried the dependents: `commands/sidecar_commands.rs:129,148-149`; `sidecar/client.rs:1273` (`register_mcp_server`), `:1394,:1399` (`mcp_server_healthy`); `recipes/runner.rs:215,310,329` — all importing `current_mcp_port`, `is_ready`, `McpPortStatus` or `port_snapshot`. Four frontend surfaces break **silently** because TypeScript still compiles: `components/sidecar/McpToolBrowser.tsx:31`, `stores/sidecarStore.ts:154-160,309`, `components/sidecar/SidecarPanel.tsx:118-128,265-306`. Separately `sidecar/mod.rs:439` hardcodes `mcp_status = Running`, so the panel would show a green light for a deleted service — a writer with no reader, inverted. `socket2` (`Cargo.toml:58`) is used only by `mcp_server/server.rs:19` and so is as removable as `rmcp` and `axum`.

**When.** 2026-08-31 · phase P0 · triggering task `P0-13`, `P0-16`

**Where.** `src-tauri/src/mcp_server/` · `src-tauri/src/commands/sidecar_commands.rs` · `src-tauri/src/sidecar/{client,mod}.rs` · `src-tauri/src/recipes/runner.rs` · `src-tauri/src/main.rs` · `src/components/sidecar/{McpToolBrowser,SidecarPanel}.tsx` · `src/stores/sidecarStore.ts` · `src-tauri/Cargo.toml`

**How.** `health_monitor.rs` is relocated first (`P0-14`), then the closure is removed in one commit so the tree never rests uncompilable. Evidence: `GATE-PROVEN` — `cargo check --workspace --all-targets` and `tsc --noEmit` both clean, plus a grep gate asserting zero surviving references to `mcp_server`, `rmcp`, `axum` or `socket2`.

**Links.** `_verify/V2-P0BC-vault-mcp.md` C5 (REFUTED-incomplete), findings 3–4 · MDP `§6 P0-13`, `P0-14`, `P0-16`

---

---

## ADR-LOG-RESTRUCT-0007 — `ToolContext.agent_role` becomes non-optional, which widens who may use the ACP

**Status:** DECIDED
**Author:** Claude
**Tier:** flagged-and-proposed

**What.** `ToolContext.agent_role` changes from `Option<String>` to `String`, and `depth` is threaded from the spawn tree. Two behaviours change as a direct consequence: the interactive assistant can now use the worker↔worker messaging tools, and the `qc_review` build-evidence gate now applies to every caller rather than none.

**Why.** An absent identity was never a neutral default — it silently disabled capabilities. Seven tools read `agent_role`; three of them (`send_worker_message`, `list_worker_messages`, `acknowledge_worker_message`) hard-refuse without it, `claim_task_tool.rs:85` and `request_supervisor_action.rs:102` fall back to attributing work to the literal string `"unknown"` (breaking orphan detection), and `update_task_status_tool.rs:133` read `ctx.agent_role.is_some()` as the *condition for applying* the build-evidence gate. Since all three in-process construction sites hardcoded `None`, that gate could never fire outside the MCP path — a guard that stops firing without ever erroring, which is exactly the `10 §4` "guard-the-guard" failure class. Making the field non-optional removes the possibility of that state existing at all, rather than adding a check for it.

The two widenings are deliberate and are the point of the change, not side effects:
- The old refusal in `send_worker_message` was a proxy for "has an identity", which interactive callers lacked. They no longer lack one, and the interactive assistant is `"supervisor"` — the role the shipped recipes already assign it (`recipes/defaults.rs:217`) and the one that dispatches to workers. `"anon"` remains impossible.
- The `qc_review` gate applying to everyone is the restoration promised by `P0-13a`. QC reviewers stay exempt because they review someone else's build and never run one.

**When.** 2026-09-01 · phase P0 · triggering task `P0-13a`

**Where.** `src-tauri/src/agent/tool_trait.rs` · `src-tauri/src/agent/tool_executor.rs` · `src-tauri/src/agent/engine.rs` · `src-tauri/src/commands/agent_commands.rs` · `src-tauri/src/recipes/runner.rs` · `src-tauri/src/agent/builtin_tools/{send_worker_message,list_worker_messages,acknowledge_worker_message,claim_task_tool,request_supervisor_action,annotate_file_tool,update_task_status_tool}.rs`

**How.** `AgentIdentity { role, depth }` is constructed at the two engine sites — `AgentIdentity::supervisor()` for the PA, `AgentIdentity::worker(worker_slug)` for a fleet worker — and threaded `AgentEngine::new → ToolExecutor::new → ToolContext`. Evidence: `UNIT-PROVEN` — `identity_tests` asserts depth semantics and, by reading `tool_executor.rs`, that no construction site hardcodes `agent_role: None` or `depth: 0`. That source-scan is the only possible form: the values are struct literals with no runtime surface to interrogate.

**Links.** spec `10 §4` "Guard-the-guard" · `_verify/V2-P0BC-vault-mcp.md` findings 1–2 · ADR-LOG-RESTRUCT-0001 · MDP `§6 P0-13a`

---

## ADR-LOG-RESTRUCT-0008 — Mode 2 is inert from P0; its code is deleted in P8

**Status:** DECIDED
**Author:** Claude
**Tier:** flagged-and-proposed

**What.** `RecipeRunner`'s Mode 2 branch is made unreachable in P0 by forcing its gate to `false`. Every fleet worker runs on the in-process Rust engine (Mode 1). The ~350 lines of Mode 2 control flow are removed with the rest of the sidecar in P8, not now.

> ⚠ **PARTIALLY RETRACTED 2026-09-01 — see ADR-LOG-RESTRUCT-0009.** The decision stands; one of its stated consequences was false. ~~"Routing to Mode 1 keeps workers functional with no sidecar and no Python."~~ A latch twenty lines earlier in the same function held every tick until the orchestrator had been healthy at least once, so with no Python the fleet did not fall back to Mode 1 — it did not run at all.

**Why.** A Mode 2 worker is a `core-orchestrator` LivingAgent that reaches AutoDev's tools by calling back into the in-process MCP server. P0 deletes that server, so a Mode 2 worker would run with **zero tools** while still consuming tokens. The pre-existing pre-flight at `runner.rs` already treated an unreachable MCP server as "abort the tick", so leaving the branch live would mean every Mode 2 tick aborting after paying for `ensure_agent` — worse than not entering it. Routing to Mode 1 keeps workers functional with no sidecar and no Python, which is what the headline acceptance test requires.

Deleting the branch in this commit was considered and rejected: it is ~350 lines of deeply nested control flow (its own closing braces are comment-annotated), and unpicking it in the same change that removes the MCP server would put the build at risk for work P8 already owns. The gate is a single line with the reasoning attached, so the dead branch cannot be mistaken for a live path.

**When.** 2026-09-01 · phase P0 · triggering task `P0-13`

**Where.** `src-tauri/src/recipes/runner.rs`

**How.** `let orch_healthy = false;` with the original probe preserved as `_orch_reachable` so the health call still runs for logging. Evidence: `CODE-PRESENT` now; `SMOKE-PROVEN` at `S0` when a worker tick is observed taking the Mode 1 path. The P8 deletion is tracked as a task, not an intention.

**Links.** MDP `§6 P0-C` sequencing hazard (Razi-ruled) · ADR-03-02 · ADR-LOG-RESTRUCT-0006

---

## ADR-LOG-RESTRUCT-0009 — Correcting ADR-0008: making Mode 2 inert did not make Mode 1 run

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** The fleet-ready latch in `RecipeRunner` is removed. `ADR-LOG-RESTRUCT-0008` is partially retracted: its decision (Mode 2 inert in P0, deleted in P8) stands, but its claim that *"every worker therefore runs on the in-process Rust engine (Mode 1), which needs no sidecar and no Python"* was **false when written**.

**Why — the mechanism of my error, not just the correction.** I set `orch_healthy = false` to make the Mode 2 branch unreachable and verified that the code below that line routes to Mode 1. I did not read what ran *above* it. `runner.rs` opened with

```rust
if !self.orch_client.has_ever_been_healthy() {
    self.set_worker_status(worker_slug, "idle");
    return Ok(());
}
```

a latch whose only purpose was to stop a cold start spamming Mode 2 `ensure_agent` warnings. On a machine with no Python the orchestrator never becomes healthy, the latch never opens, and **every worker tick returns early reporting "idle"**. The fleet was not falling back — it was silently dead, and reporting itself idle while dead. My own comment asserting the opposite sat twenty lines below the latch, in the same function, in the same commit.

This is `L-C4` — a local change made without re-checking the code path I was changing. It is the second instance, and both were the same shape: I verified the consequence I intended and not the surrounding control flow.

**When.** 2026-09-01 · phase P0 · post-phase audit finding `F-10`

**Where.** `src-tauri/src/recipes/runner.rs`

**How.** The latch is deleted, with its history in the replacing comment so it is not reinstated by someone reading only the cold-start rationale. Evidence: `CODE-PRESENT`; a worker tick reaching Mode 1 with no sidecar running is `S0` work for P1, since P0 has no fleet smoke.

**Links.** `_verify/A1-P0-code-audit.md` F-10 · supersedes the consequence clause of ADR-LOG-RESTRUCT-0008 · `MDP §0.16 L-C4`


# §5 · Standing entries carried from planning

Not build-time decisions — but recorded here because a future session will otherwise rediscover them the hard way. Each is already ratified in vision Part 8; these are the **operational consequences** that bite during implementation.

| # | Carried fact | Consequence during the build |
|---|---|---|
| C-1 | `subagent/bridge.rs`'s SSE frame parsers are `pub(crate)` and **shared with** `sidecar/client.rs` | Those two deletions **cannot be sequenced independently**. Attempting one alone breaks the build. |
| C-2 | ACP sender identity is set in **exactly one place** — the MCP adapter deleted in P0 | `ToolContext.agent_role` must be truthful **before** P4 relies on messaging, or a preserved capability dies silently. |
| C-3 | `graph/` migration head **v5 is a broken statement** (`ADD COLUMN IF NOT EXISTS`) | It must be repaired before v6 is added, and the repair touches live per-repo graph DBs — back up first (§0.8). |
| C-4 | Four of seven surviving MCP items live inside **sidecar-named files** | They must be relocated **by address**, or sidecar code survives P8 by filename alone. |
| C-5 | The correct UTF-8-safe truncation **already exists** at `validate.rs:846-853` | Three sites panic on byte-slice truncation; the fix is to use the existing helper, not to write a fourth implementation. |
| C-6 | `is_read_only()` is documented as a **concurrency hint** and is currently the auto-approval key | Repurposing it back to concurrency and introducing `ToolEffect` must happen together, or a window opens where nothing gates prompting. |
| C-7 | Three subsystems share one missing path-normalisation helper — path lock, checkpoint retention, graph DB key | Fix once, in `autodev-core`, and route all three through it. Fixing them separately guarantees they drift again. |

---

# §6 · Log discipline

- **Immediate.** Logged in the same step as the decision, not at wrap-up.
- **Append-only.** Never edit a past entry; supersede it.
- **Retractions are entries.** When a finding or claim of mine proves wrong, the entry records **how the error happened**, not merely that it was corrected. Planning already produced six such corrections — including one where I silently narrowed Razi's ADR-02-02 answer instead of flagging that it conflicted with the doctrine. That kind of thing is logged, in the open, with the mechanism named.
- **A decision with no `Why` citing a grounding fact is not a decision.** It is a preference, and it gets challenged at closeout.

| Date | Rev | Change |
|---|---|---|
| 2026-08-31 | 1.0 | Created. Register relationships, namespaced ID scheme (`ADR-LOG-RESTRUCT-NNNN`), tiering, entry template. Zero entries — build not started. Seven standing facts carried from planning that would otherwise be rediscovered destructively. |
