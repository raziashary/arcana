# AutoDev Restructure — Runtime ADR Log

> **Current direction, 2026-09-23:** Arcana is a personal coding platform.
> See `ARCANA_CODING_SCOPE.md`. The entry count in the original header below
> is a dated snapshot, not a live total; this append-only log continues past
> `0019`.

> **Append-only build-time decision journal.** A reversal is a new `SUPERSEDED-BY` entry, never an edit. Retractions of one's own findings are entries too — record the **mechanism of the error**, not just the correction.

| | |
|---|---|
| **Parents** | `MASTER_DEVELOPMENT_PLAN.md` §0.11 · `../vision/00-…-MASTER-PLAN.md` Part 8 |
| **Rev** | 1.0 — 2026-08-31 |
| **Entries** | **19** — `ADR-LOG-RESTRUCT-0001` … `0019`, spanning P0 and P1 |

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


---

## ADR-LOG-RESTRUCT-0010 — Retraction: "undo does not exist in the shipped binary" is false

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** The claim that AutoDev has no undo is retracted. It was false when written and has been struck in place in all five locations that carried it. The *other half* of the same grounding finding — that a failed checkpoint is silently swallowed — stands and is preserved.

**Why — the mechanism of the error.** Grounding `G3d F-44` recorded that `db/checkpoints.rs` is not compiled, and concluded from that alone that undo is absent. I adopted the conclusion without checking the load-bearing detail, and propagated it into **my own gate code** (`reachability_gate.rs:8`), two gate rows (`TESTING_GATES.md:110`, `:412`), and two master-plan rows (`00 §R3`, `§1175`).

Undo ships. `pub mod checkpoint;` at `git/mod.rs:21`; five commands registered at `lib.rs:386-390`; `src/stores/gitStore.ts:238` calls `list_checkpoints`; and `auto_checkpoint_before_write` runs before every agent write (`edit_file.rs:177`, `write_file.rs:124`). `db/checkpoints.rs` is genuinely uncompiled but has **zero callers**, so it creates no second database today.

The sharpest detail: `reachability_gate.rs` **contradicted itself**. Its header said undo does not exist; its own `KNOWN_UNREACHABLE` entry twenty lines below correctly described `git/checkpoint.rs` as "the live, shipped" implementation. Both were written by me in the same commit.

**What survives.** `auto_checkpoint_before_write` returns `()` and logs failure at `debug!` level only (`git/checkpoint.rs:256-260`); the caller ignores it. A checkpoint that fails is invisible — in production, `debug!` is typically filtered out entirely — and the overwrite proceeds regardless. That is a real defect and remains what `TG5-xx` must prove.

**When.** 2026-09-01 · phase P1 · pre-build verification (`§0.2m`), triggered by `P1-02`

**Where.** `src-tauri/src/reachability_gate.rs` · `docs/restructurization/development/TESTING_GATES.md` · `docs/restructurization/vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md`

**How.** Struck in place with the correction beside it, never deleted (§0.11). Evidence: `CODE-PRESENT` — the declaration, the five command registrations, the UI call site and the two auto-snapshot call sites are all cited above and were verified in the main session, not taken from the subagent report (§0.2a).

**Links.** `_verify/V4-P1AB-core-extraction.md` C4 · `G3d F-44` · `MDP §0.16 L-C3` (4th instance) · `L-1` (a verdict with an "and" in it is a checklist)


---

## ADR-LOG-RESTRUCT-0011 — `autodev-core` is extracted in full; the four crate cycles are broken

**Status:** DECIDED
**Author:** Razi
**Tier:** razi-ruled

**What.** `P1-05` does the complete extraction: break all four crate cycles, then move `agent/` into `autodev-core` alongside `provider/`, `memory/` and `platform/`. The narrower option — move only the three clean directories and leave `agent/` behind — was offered and rejected.

**Why.** The task as written was refuted. Tauri coupling, the assumed blocker, is six lines in two files; `provider/`, `memory/` and `platform/` have none. What actually blocks the move is four cycles Cargo rejects outright: `agent`↔`recipes` (13/9), `agent`↔`subagent` (5/11), `agent`↔`commands` (1/38, via `agent/builtin_tools/analyze_diff_tool.rs:92`), and `agent`→`diff_analysis`→`provider`. `agent/` also holds 114 references to 17 modules that stay behind. Razi chose the full job so P1's stated goal — *both binaries link the core* — is actually delivered rather than partially.

**When.** 2026-09-01 · phase P1 · `P1-05`

**Where.** new `autodev-core/` · `src-tauri/src/{agent,provider,memory,platform}/` · `src-tauri/src/{recipes,subagent,commands,diff_analysis}/`

**How.** Cycles are broken before the move: invert `agent`→`commands` behind a callback, and split the `agent`↔`recipes` and `agent`↔`subagent` edges behind traits owned by core. `security/` is **not** part of the move — it does not exist until `P1-20` creates it. Evidence: `CODE-PRESENT` for the move, `GATE-PROVEN` by `cargo check --workspace` and by `S1-02` (`airsctl --version` linking core).

**Links.** `_verify/V4-P1AB-core-extraction.md` C5 · `_verify/P1-CORRECTIONS.md §A` · MDP `§7 P1-05`

---

## ADR-LOG-RESTRUCT-0012 — `db/checkpoints.rs` is deleted, not declared

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** `P1-02` is reversed. `src-tauri/src/db/checkpoints.rs` is deleted rather than declared, and `P1-16` becomes "confirm the single owner and remove the dead implementation".

**Why.** The task's justification — *"the undo feature is currently absent from the shipped app"* — is false, and is retracted in `ADR-LOG-RESTRUCT-0010`. Undo ships through `git/checkpoint.rs`.

`db/checkpoints.rs` is uncompiled and has **zero callers**, so `checkpoints.db` does not exist on any machine. Declaring it would *create* the second checkpoint store that `P1-16` then has to merge — manufacturing a data-migration problem two tasks before solving it. As things stand `P1-16` is a one-way delete of dead code, and **that window closes the moment `P1-02` lands as written**.

Decided independently rather than escalated because §0.12 governs: faithful implementation of a doc whose premise is false is not faithfulness, and the cheaper, less destructive path is the one with codebase precedent (`patch.rs` was deleted the same way in P0-08).

**When.** 2026-09-01 · phase P1 · `P1-02`, `P1-16`

**Where.** `src-tauri/src/db/checkpoints.rs` · `src-tauri/src/reachability_gate.rs` (its `KNOWN_UNREACHABLE` entry)

**How.** Delete the file and its exception-list entry in one commit; the reachability gate then passes with one fewer exception, which is the direction that list is only ever allowed to move. Evidence: `CODE-PRESENT`, plus `GATE-PROVEN` by the reachability gate.

**Links.** `_verify/V4-P1AB-core-extraction.md` C4 · `ADR-LOG-RESTRUCT-0010` · MDP `§7 P1-02`

---

## ADR-LOG-RESTRUCT-0013 — The graph migration repair is a new `v6`, not an edit to `v5`

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** `P1-11` is implemented as a new `migration_v6`. `migration_v5` is left as history.

**Why.** Editing v5 in place is a no-op. Every existing graph database is already stamped `user_version = 5`, and `graph/migrations.rs:37` gates on `if current < 5`, so corrected v5 code is never invoked on any machine that has run the app.

The reason it is stamped is the more serious finding: all six statements in v5 are `let _ = …`, so the invalid `ADD COLUMN IF NOT EXISTS` failed silently, the migration stamped `user_version = 5`, committed, and logged "✅ complete". An inert-but-green migration inside the migration system — the same defect class this programme exists to remove, in the component whose entire job is correctness under change.

Fixing v5 would satisfy a reviewer reading the diff and change nothing on disk. That is the outcome §0.2e exists to forbid.

**When.** 2026-09-01 · phase P1 · `P1-11`

**Where.** `src-tauri/src/graph/migrations.rs`

**How.** `migration_v6` adds, idempotently, the columns v5 failed to add — checking `PRAGMA table_info` rather than relying on `IF NOT EXISTS`, which SQLite does not support for `ADD COLUMN`. The `let _ =` swallowing is replaced with propagated errors so a future failure cannot stamp a version it did not achieve. Evidence: `LIVE-DB-PROVEN` against a copy of the real graph DB, never the original.

**Links.** `_verify/V5-P1CD-schema-redaction.md` C3 · `_verify/P1-CORRECTIONS.md §B` · MDP `§7 P1-11` · §0.8 named-destructive list

---

## ADR-LOG-RESTRUCT-0014 — `P1-19` runs first; a backup helper is a precondition, not a deliverable

**Status:** DECIDED
**Author:** Claude
**Tier:** decided-independently

**What.** The backup helper moves from last in `P1-C` to first in the phase, before any task that mutates a database.

**Why.** `P1-11` and `P1-14` both carry "⚠ back up first" in the plan and both are listed *before* the helper that would let them. There is no SQLite backup capability in the tree: no `VACUUM INTO`, no `Connection::backup`, and rusqlite's `backup` feature is not enabled. The two existing helpers do not qualify — `backup_recipe_file` is YAML-only, and `create_restore_point` writes a size/mtime manifest that nothing reads, which is a writer with no reader (INV-8).

The ordering also has teeth: `~/.airs/autodev.db` carries a **4.1 MB uncheckpointed WAL**, so a helper built on `fs::copy` would silently discard newer data than the file it copied.

**When.** 2026-09-01 · phase P1 · `P1-19`

**Where.** `src-tauri/src/db/backup.rs`

**How.** `VACUUM INTO`, which produces a consistent snapshot including WAL content and does not mutate the source, rather than a file copy. Wired into every destructive path per GAP-11. Evidence: `UNIT-PROVEN` — including a test that writes to a WAL-mode database without checkpointing and asserts the backup contains the uncheckpointed rows, because that is the failure a copy-based helper would have.

**Links.** `_verify/V5-P1CD-schema-redaction.md` C9 · §0.8 · MDP `§7 P1-19`


## ADR-LOG-RESTRUCT-0015 — Core owns the infrastructure the engine owns; trait seams go only where behaviour forks

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled` (conditional: *"check first if its possible and clean to do full trait seams, if not and risky then we do the hybrid"* — the condition was checked and failed; see **Why**)

**What.** `autodev-core` takes the agent runtime **and the infrastructure that runtime genuinely owns**: `db/`, `graph/`, `git/`, `routing/`, `analytics/`, `hooks/`, `recipes/`, `subagent/`, `agent/`, plus the already-moved `provider/`, `memory/`, `platform/`, `session/`, `state/`, `vault/`, `budget/`, `cost/`, `plugin/`, `observability/`, `diff_analysis/`, `errors/`, `events/`.

`src-tauri` keeps only what is genuinely host-bound: `commands/` (255 `#[tauri::command]`), `updater/`, `mcp/`, `sidecar/`, `skills/watcher.rs`, `main.rs`/`lib.rs`, and the CI gates.

Trait seams are introduced at **exactly two** places, being the only two where desktop and CLI behaviour actually differ:
- `events::EventSink` — the desktop emits to a window; `airsctl` logs or discards.
- an approval seam — the desktop asks a human (`agent/tool_executor.rs:505` emits a permission request and waits); a CLI must resolve by policy.

**Why.** Full trait inversion was measured before being rejected, not assumed away. `agent/`'s 63 cross-boundary references collapse to 20 distinct symbols, but behind them sit **91 distinct methods across 10 traits** — `TaskStore` 28, `PlanStore` 13, `GraphAccess` 12, `WorkerStore` 8, `GitOps` 8, `HookRegistry` 7, `Analytics` 6, `Annotations` 4, `Instructions` 3, `Router` 2.

Three findings make that unclean rather than merely large:

1. **Every trait would have exactly one implementation.** No polymorphism is gained; it is a header file for SQL.
2. **The domain types cross into core regardless.** Trait signatures reference `Task`, `TaskType`, `WorkerRequest`, `PlanItem`, so only the SQL stays behind — and separating a repository's types from its queries across crates is the exact mechanism that already broke `airsctl`: `src-shared/src/db/workers.rs` selects `last_seen` and `notes`, while the live `~/.autodev/workers.db` written by `src-tauri` has neither column (`id, name, slug, worker_type, status, model, primary_repos, polling_interval, recipe_id, is_system_defined, description, created_at, updated_at`). Every `airsctl` worker list/get/heartbeat call errors today. `P1-09` exists to repair that; a blanket trait split would rebuild the same fault line.
3. **These are synchronous `rusqlite` calls behind an async engine.** Routing them through trait objects across await points forces `async_trait` and `Send + Sync` bounds on all 91.

Timing is decisive: MDP `§4` itself tags `agent/` **"[P1 move, P3 rework]"**. Authoring ten traits is a rework, so P3 would write them a second time.

The criterion is stated as *"does the engine own this, or does the host?"* — **not** *"does it import `tauri`"*, which is mechanical and accidental. Both happen to give the same answer here, which is why the placement is principled rather than a coincidence of imports.

**When.** 2026-09-01 · P1 · group `P1-B`, task `P1-05`/`P1-06`

**Where.** `autodev-core/src/` (destination) · `src-tauri/src/` (shims) · `autodev-core/src/events.rs` (`EventSink`) · MDP `§4` module tree (corrected by this entry)

**How.** `git mv` per module with a `pub use autodev_core::<m>::*;` re-export shim left behind, so no import path in 255 commands is rewritten. Proven by `cargo check --workspace` and `cargo test --workspace --all-targets` by **exit code** at each step. Evidence: `UNIT-PROVEN` + `GATE-PROVEN` (`TG-P1` crate-boundary gate asserts core contains no `tauri` reference).

**Supersedes in part.** `MDP §4`'s module tree, which places `db · graph · git · recipes · hooks` in `src-tauri` while `§3`'s diagram and `INV-1` place them under core. §4 is the stale line and is corrected in the same step (§0 no-stale).

**Links.** `ADR-LOG-RESTRUCT-0011` (the full-extraction ruling this implements) · `P1-CORRECTIONS.md` §A · MDP `§3`, `§4`, `INV-1` · `P1-09`

## ADR-LOG-RESTRUCT-0016 — `airsctl` adopts core's task/worker/plan store; four decisions the swap forced

**Status.** DECIDED
**Author.** Claude · **Tier.** `decided-independently` (each choice below is the additive or non-destructive option; the one alternative that would have removed a shipped command was rejected for that reason)

**What.** `src-shared/src/db/{tasks,workers,plans}.rs` are deleted. `airsctl` now reads and writes those tables through `autodev_core::db`, the same code the desktop app runs. Four decisions were forced by the swap:

1. **`last_seen` is added to core's `workers` schema** rather than removing `airsctl worker heartbeat`. Additive-by-default (§0); removing a shipped command is a contract change that would need Razi.
2. **`unregister` maps to a new `deactivate` (soft), not to core's `delete` (hard).** Core's `delete` removes the row and refuses system-defined workers. `airsctl worker unregister` has always been reversible, and silently turning it into permanent deletion of live rows is not a consolidation, it is data loss.
3. **`task claim` walks `assign → claim` through `claim_task_with_pid`.** Core models the handoff in two steps and refuses to claim anything not already `assigned`; `src-shared` went straight from `ready` to `in_progress`. The CLI keeps its one-command behaviour and gains what the two-step path carries.
4. **`src-shared/src/db/analytics.rs` stays.** It is not one of the four tables `P1-09` names, and it contains **zero** `CREATE`/`INSERT`/`UPDATE`/`DELETE` statements — a pure reader cannot drift a schema. Consolidating it means porting aggregation semantics that decide the numbers users are shown (`cost_summary(window)` vs `get_cost_analytics(start, end)`), which is a separate task with its own risk.

**Why.** The mirrors were not a latent risk, they were a live outage. `src-shared/src/db/workers.rs` selected `last_seen` and `notes`; the live `~/.autodev/workers.db`, written by `src-tauri`, had neither. Its own `CREATE TABLE IF NOT EXISTS workers (… last_seen …)` was a **silent no-op** against the already-existing table, so the columns were never added and `airsctl worker list`, `status` and `heartbeat` all failed with *no such column: last_seen* — from the day they were written. Proven by executing the statements against a copy of the live file before changing anything.

Decision 3 was **found by testing, not by reading**: the first mapping called `claim_task_with_pid` directly on a `ready` task and every claim failed. The behaviour it buys is not cosmetic — `claimed_by_pid` and `claim_host` are what orphan detection uses to release stale tasks, so a CLI-claimed task under the old path could never be detected as orphaned, and one worker could hold several tasks while the app assumed one.

**When.** 2026-09-01 · P1 · group `P1-C`, task `P1-09`

**Where.** `autodev-core/src/db/workers.rs` (`last_seen`, `add_missing_columns`, `heartbeat`, `deactivate`, `open_at`, `DEFAULT_MODEL`) · `src-cli/src/commands/{worker,task,plan}.rs` · `src-cli/src/repl.rs` · `src-shared/src/db/` (three files deleted)

**How.** `add_missing_columns` checks `PRAGMA table_info` and issues `ALTER TABLE`, because **`ADD COLUMN IF NOT EXISTS` is not valid SQLite** — the graph migration used it, wrapped every statement in `let _ = …`, and still stamped `user_version = 5` and logged success (`ADR-LOG-RESTRUCT-0013`). Nothing here is discarded: a column that cannot be added fails `open()`.

Evidence — `LIVE-DB-PROVEN`:
- Backup first (§0.8): `VACUUM INTO` to `~/autodev-backups/p1-09-preflight`, `integrity_check = ok`, 7 workers and 220 `routing_history` rows recorded.
- Proven on an exact copy of the live file before touching the original: `last_seen` added, **7 rows preserved**, `integrity_check = ok`, all seven slugs intact.
- Then on the live database: same result, and `airsctl worker list` exits **0** where it had always errored. `heartbeat` persists a timestamp that `status` reads back — the write is proven read, not merely accepted. An unknown slug exits **1** rather than silently doing nothing.
- `task claim` records `claimed_by_pid=9392`, `claim_host=LAPTOP-3K58S1BQ`; a second claim by the same worker exits **1**.
- `UNIT-PROVEN`: 6 tests in `db::workers`, including one that rebuilds the live schema **without** `last_seen` and asserts the fixture really lacks the column before migrating it, one asserting the migration is idempotent, one asserting `deactivate` leaves the row in the table, and one that inserts a row without a model to pin `DEFAULT_MODEL` to the schema default so the two cannot drift.

**Known behaviour change, recorded rather than hidden.** Core's `get_by_slug` filters `status != 'deleted'`; `src-shared`'s did not. `airsctl worker status <deactivated-slug>` now reports not-found instead of showing the row. There is no regression from a working state — the command errored outright before this change — and it makes `list` and `status` agree.

**Links.** `ADR-LOG-RESTRUCT-0015` · `P1-CORRECTIONS.md` §B (`P1-09`) · MDP `§7 P1-09` · `P1-15` (`app_settings`, whose single owner `db/app_settings.rs` was created in `P1-B`)

## ADR-LOG-RESTRUCT-0017 — Retraction: graph v5 did not lose any columns; `migration_v6` is defensive, not corrective

**Status.** DECIDED · **partially supersedes `ADR-LOG-RESTRUCT-0013`**
**Author.** Claude · **Tier.** `decided-independently` (a retraction of my own finding, per §0's retraction discipline)

**What is retracted.** `ADR-LOG-RESTRUCT-0013` and `P1-CORRECTIONS.md §B` state that graph `migration_v5` **failed to add** `agent_author`, `task_id`, `worker_locked` and `worktree`, and that `P1-11` must write a `v6` to *repair* that loss. The premise is **false**. No column was ever missing.

**What is true, and stands.** The mechanism described in `0013` is real and unchanged: v5's six statements use `ALTER TABLE … ADD COLUMN IF NOT EXISTS`, which **is not valid SQLite**; all six were wrapped in `let _ = …`; the migration therefore stamped `user_version = 5`, committed, and logged *"✅ migration_v5 complete"* over six failures. That is exactly the inert-but-green class this programme exists to remove, and it is still a defect.

**What was wrong.** The six statements were also **redundant**. `graph/schema.rs`'s `CREATE_NODES` already declares `agent_author`, `task_id`, `worker_locked` and `worktree` (lines 57–60); `CREATE_EDGES` declares `agent_author` and `task_id` (78–79); and `ALL_INDEXES` already contains `idx_nodes_task` and `idx_nodes_author`. Every database created by `migration_v1` has had all six columns from the start.

**The mechanism of my error.** I reasoned from the migration file alone: invalid SQL plus a discarded result implies the columns are absent. That inference is sound in isolation and I did not check it against anything. Two pieces of evidence were available and unused:

1. **The live database.** `~/.autodev/graphs/d0ef18f6ad8b.db` carries all six columns and both indexes at `user_version = 5`.
2. **The file's own code, four lines below the failures.** v5's two `CREATE INDEX … ON nodes(agent_author)` / `ON nodes(task_id)` statements use `?`, not `let _ =`. Had the columns been absent, **those** would have failed and propagated — the migration would have errored rather than logging success. Its own success was evidence against my reading, in the same function.

This is `§0.16 L-C2` (*read past the signature*) in a new place: I read a migration's statements and inferred a schema, instead of reading the schema. The rule earned again: **a claim about state is verified against the state.**

**What `migration_v6` therefore is.** Defensive, not corrective. On every database that exists today it is a no-op that advances the version — verified. It exists for a database created by a build whose `CREATE_NODES` predated those columns and which was then "migrated" by the broken v5: for that database the columns *are* genuinely absent, and nothing else would ever add them, because `CREATE TABLE IF NOT EXISTS` does not alter an existing table. Since such a database cannot be ruled out on someone else's machine, v6 ships.

Unlike v5, v6 checks `PRAGMA table_info` before each `ALTER` (via `db::migrate::ensure_columns`) and propagates failure instead of discarding it. v5's six dead statements are **deleted** rather than repaired, with the reason recorded in place.

**When.** 2026-09-01 · P1 · group `P1-C`, task `P1-11`

**Where.** `autodev-core/src/graph/migrations.rs` · `autodev-core/src/graph/schema.rs` (the evidence) · `autodev-core/src/graph/store.rs` (`schema_version`)

**How.** Evidence — `LIVE-DB-PROVEN` + `UNIT-PROVEN`:
- Backed up first to `~/autodev-backups/p1-11-preflight` (`integrity_check = ok`).
- Four tests, including one that builds a `nodes`/`edges` pair **without** the columns, stamps `user_version = 5`, asserts the fixture really lacks them, and then requires v6 to add all six while preserving an existing row — the database v6 exists for. A second test rewinds a complete schema to v5 and asserts v6 advances it **without changing a single column**, which is what would fail with *duplicate column name* had v6 been written the naive way.
- Live: proven on a copy first, then applied — `5 → 6`, `integrity_check = ok`, column list and row counts **identical** to the backup.

**Links.** `ADR-LOG-RESTRUCT-0013` (superseded in part) · `P1-CORRECTIONS.md §B` (corrected) · `§0.16 L-C2` · MDP `§7 P1-11`

## ADR-LOG-RESTRUCT-0019 — Redaction is applied where the tool actually runs; egress is covered through logs and analytics, not by rewriting the request

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled` (he chose *"full remediation including over-redaction"* when the audit reported the boundary was not on the production path)

**What.**

1. **Redaction moves onto the production path.** It is applied at **every** `tool.execute` call site — `ToolExecutor::execute` and both branches of `execute_batch` — rather than at one downstream point, because there is no single downstream point: each reaches the conversation, the `messages` table and the UI event independently.
2. **Two more boundaries are wired:** the log appender (`src-tauri/src/logging.rs`, via a `RedactingWriter` around the file appender) and analytics (`record_api_failure`, `record_system_event`).
3. **Egress is deliberately NOT redacted at the request body.** Its durable-storage risk is covered by the two above.
4. **Key matching becomes segment-aware.** The Python source's raw substring test is not portable as written.
5. **One `PIIRedactor` pattern stays narrowed, and now says so.**

**Why.**

`ToolExecutor::execute` had **no production caller.** The agent loop calls `execute_batch` (`agent/engine.rs:314`), which redacted nothing — raw tool input and output reached the model, the database and the renderer, and subagents inherited the same path. Both proofs were green: `S1-04` drove the dead method, and `TG-P1-06` was a substring check for `security::redaction` over the file, which matched the dead code. **Three independent post-phase auditors found it**, and the third refuted the claim that rested on it.

That is `L-C12`'s **second** instance in one phase and `L-C9`'s fourth. Per `§0.2n` a finding whose class is already in the register is worse than a novel one, and this one is worse still: I wrote *"compiling is not wiring"* three times in this phase, about `graph_tools.rs`, and then shipped exactly that.

**On egress (3).** M-40 names five boundaries and *egress* is one. Redacting the outbound provider request would strip the content the model must reason about — a config file the user asked about, a stack trace they pasted — breaking the product to protect a channel the user deliberately chose to use. What actually reaches durable storage from that path is what we **record** about a call: `record_api_failure` (an auth failure routinely echoes the request) and the log appender. Both are now redacted. The claim "every boundary" is therefore true of durable storage and explicitly not true of the wire, which is the honest formulation.

**On segment matching (4).** `any(sk in key_lower for sk in SENSITIVE_KEYS)` ported literally redacts `author` (contains `auth`), `passed` (contains `pass`) and `prompt_tokens` (contains `token`). In JSON that also changed the **type** — a numeric token count became the string `"[REDACTED]"` — and in free text `let auth = build_auth(cfg);` became `let auth=[REDACTED];` in the transcript the model reads. The set of sensitive names is unchanged; only the matching is corrected.

**On the narrowing (5).** `PIIRedactor`'s phone rule makes separators optional, so it matches any bare ten-digit number. `1777963265` is a Unix timestamp and this tree's checkpoint filenames are built from them. A separator is required. The card and `api_key`-prefix patterns, also narrowed by the first port, are **restored** — and the fact that two of the three narrowings had been *undocumented* is itself the `L-1` failure that lost redaction in the first place.

**When.** 2026-09-01 · P1 · post-phase audit remediation

**Where.** `autodev-core/src/agent/tool_executor.rs` · `autodev-core/src/security/redaction.rs` · `autodev-core/src/analytics/collector.rs` · `src-tauri/src/logging.rs` · `src-tauri/src/ci_gate.rs` · `autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs`

**How.** `TG-P1-06` now checks the **shape of the call** — every `tool.execute` must be wrapped in `redact_result` — so a fourth branch added later fails the gate rather than passing on a substring. `S1-04` drives `execute_batch`, covers both branches and a mixed batch, and was **falsified**: removing the fix fails all three tests with *"THE SECRET REACHED THE PROMPT"*. 19 of the area auditor's 40 bypass attempts are closed, with each shape pinned by a test.

⚠ **One regression the fix introduced, caught by a test rather than by reading.** `\s*` around the assignment separator crosses a newline, so a harmless `config:` ending one line swallowed the `AWS_SECRET_ACCESS_KEY=…` line below it as its value, found `config` not sensitive, returned the match unchanged — and consumed the real assignment. The secret survived. Now `[ \t]*`, pinned by `a_harmless_key_does_not_swallow_the_secret_on_the_following_line`.

**Links.** `_verify/P1-AUDIT-TRIAGE.md` · `_verify/A4-P1-area-audit.md` `A-01`…`A-07` · `_verify/A3-P1-work-audit.md` `W-10`/`W-11` · `_verify/A6-P1-claims-refutation.md` claims 8 and 11 · `M-40` · `§0.2n`

## ADR-LOG-RESTRUCT-0020 — `config.db`'s ladder owns all six of its tables

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled` (*"fold checkpoints + hooks into the ladder"*)

**What.** `git/checkpoint.rs` and `hooks/db.rs` stop issuing their own `CREATE TABLE`. `db/config_db.rs` owns all six tables in the file — `app_settings`, `autodev_instructions`, `model_configs`, `checkpoints`, `hooks`, `hook_logs` — adopting the last three at **v2**. `TG-P1-04` reads `config_db::TABLES` so the gate cannot drift from the owner.

**Why.** `P1-15` said `config.db` had three tables from three creators. It has **six from five modules**. Both extra writers predate P1 (`git show eab345c^` confirms), so this was never a regression — but the ladder was declared the file's schema owner while half its contents sat outside it, and `INV-10` was scored **GREEN** on that count.

The mechanism is `L-C6`: I searched for `CREATE TABLE … app_settings` — the **claim** — instead of for `config.db` — the **invariant**. My own justification for creating `config_db.rs` was *"`user_version` belongs to the **file**, not a table"*, and I then put three of six tables under it. The argument condemned the implementation and I did not notice.

**When.** 2026-09-01 · P1 · post-phase audit remediation
**Where.** `autodev-core/src/db/config_db.rs` · `autodev-core/src/git/checkpoint.rs` · `autodev-core/src/hooks/db.rs` · `src-tauri/src/ci_gate.rs`

**How.** A `v2` rung rather than an edit to `v1`: every live `config.db` is already stamped 1, so a v1 edit would never run. The statements are `IF NOT EXISTS` and the tables already exist, so on every database that exists today v2 creates nothing and only advances the version — which is the point. Evidence `LIVE-DB-PROVEN`: proven on a copy, then applied — `uv 1 → 2`, `integrity_check = ok`, **all 108 rows of real undo history preserved**, backed up to `~/autodev-backups/p1-audit-configdb` first.

**Links.** `_verify/A5-P1-doc-staleness.md` `D-27` · `_verify/A3-P1-work-audit.md` `W-07` · `INV-10` · `ADR-LOG-RESTRUCT-0015`

## ADR-LOG-RESTRUCT-0021 — Tool arguments are redacted on every reporting path; the permission dialog is the one deliberate exception

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled` (*"A-08 pull forward"*)

**What.**

1. All four `ToolCallContext` hook payloads — PreToolUse and PostToolUse, in both `execute` and `execute_batch` — carry `reported`, the redacted arguments, instead of the raw input.
2. `execute_batch`'s **write branch** gains the redacted copy it never had: its `emit_tool_start` and its `save_tool_message` were still passing raw input.
3. The read branch's already-redacted value is renamed from `input` to `reported` at the point it is consumed.
4. `request_permission` keeps the **raw** input, and now says why in the code.
5. A second gate, `tg_p1_06_raw_tool_input_reaches_only_the_permission_dialog`, pins the input side.

**Why.**

A-08 was triaged to P3 on the reasoning that redacting a hook's input changes what a hook can act on. That reasoning was wrong in a specific way: a hook still receives the tool name and the argument *names*, so every dispatch decision it can make today it can still make. What it loses is the *values* — and hooks are user-authored shell, so a credential in that payload reaches whatever that shell does with it, outside anything this codebase controls.

**The pull-forward is what found the real defect (2).** The A-01 remediation wrapped every `tool.execute` in `redact_result` and I recorded the boundary as closed. It was closed on the **result** side only. `execute_batch`'s write branch — the path every file edit, every command, every write tool takes — was still handing the renderer and the `messages` table the raw arguments. A credential passed *to* a tool is exactly as durable as one returned *from* it.

`TG-P1-06` could not have caught it. I wrote that gate from the claim I had just implemented — *"every tool execution redacts its result"* — rather than from the invariant it exists to protect: *no secret reaches a durable or outbound surface*. This is `L-C6`'s **fifth** instance, and the second time in one phase that a gate was green over an unprotected path.

**On the exception (4).** The permission dialog is the one place raw arguments still go, and that is correct. The user is being asked to approve *this exact call*; a redacted prompt asks them to approve `curl -H "Authorization: [REDACTED]"`, which tells them nothing about where their credential is about to be sent. The dialog is a **control over** exfiltration, so blinding it weakens the control rather than the leak. The recipient is the user, on their own machine, looking at their own secret, and nothing persists the payload — every path that outlives the dialog is redacted.

**When.** 2026-09-01 · P1 · post-phase audit remediation (Razi's ruling)

**Where.** `autodev-core/src/agent/tool_executor.rs` · `src-tauri/src/ci_gate.rs`

**How.** The new gate walks the production half of `tool_executor.rs`, attributes every raw-input site to its enclosing function, and asserts there is exactly one and that it is `request_permission`. **Falsified**: reverting a single hook site to `input.clone()` fails it with *"found 2: execute_batch():452 … | request_permission():562 …"* — naming the function and line. Its detector has its own test, so the gate cannot pass by being blind.

**Links.** `_verify/P1-AUDIT-TRIAGE.md` `A-08`/`A-08a`/`A-08b` · `ADR-LOG-RESTRUCT-0019` · `M-40` · `§0.16` `L-C6`

## ADR-LOG-RESTRUCT-0022 — The self-serve claim is one atomic statement; the supervisor's two-step handoff is untouched

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled` (*"A-22 ship the changes"*)

**What.** `TasksDb::claim_next_ready(worker, pid)` — a single guarded `UPDATE` that takes the oldest `ready` task, records `claimed_by_pid` and `claim_host`, and enforces the one-in-progress-per-worker rule **inside the statement** via a subquery. `airsctl task claim` calls it; the CLI's read-then-`assign`-then-claim sequence and its compensating revert are both deleted. `claim_task_with_pid` is unchanged.

**Why.**

Core models the handoff in two steps because the supervisor decides *who* gets a task before that worker picks it up. A worker pulling from the queue has no such decider, so the CLI faked the first step — writing `assigned`, then claiming. Three round trips, two windows. A worker that lost the race left the row `assigned` with **no PID**, which orphan detection is blind to; nothing released it until the 4-hour TTL.

The compensating revert I shipped first narrowed that window and did not close it, and could itself fail — the error path said as much. The audit called it a workaround and it was one.

**A guard read before the write is not a guard.** The one-task-per-worker check was a separate `SELECT` in `claim_task_with_pid`; moved into the claim statement it cannot be true when read and false when used. The queue policy — oldest first — had to move into core with it, because a policy applied in Rust over a list read a round trip earlier cannot be part of the same atomic write.

**When.** 2026-09-01 · P1 · post-phase audit remediation (Razi's ruling)

**Where.** `autodev-core/src/db/tasks.rs` · `src-cli/src/commands/task.rs`

**How.** A single `UPDATE` holds SQLite's write lock for its whole duration, so a concurrent claimer either waits and re-evaluates the subselect — seeing the row is no longer `ready` and taking the next one — or finds the queue empty. Evidence is **contention, not inspection**: 8 threads, 8 connections, one real file, released together on a `Barrier` → 8 distinct tasks claimed, 0 rows left in `assigned`, every claim carrying a pid and a worker. **Falsified:** restoring the select-then-update shape fails the test with all eight workers claiming task #1. Two further tests cover the `ready`-only guard and the per-worker guard. `SMOKE-PROVEN` end to end through `airsctl` against a throwaway data root: oldest-first claim, the guard's refusal, a second worker taking the next task, and a clean *"No ready tasks available."* on an empty queue.

**Links.** `_verify/P1-AUDIT-TRIAGE.md` `A-22` · `_verify/A4-P1-area-audit.md` · `P1-09`

## ADR-LOG-RESTRUCT-0023 — P3 absorbs design ingestion; the layer IR is the agent-facing format, not the HTML

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled`

**What.**

1. **P3 gains `P3-G`** (`P3-45`—`P3-54`): the PSD converter is vendored into AutoDev, the **layer IR** is built, and three kinds of adaptation run on it — reformat across aspect ratios, rebrand, content swap.
2. **The IR is the agent-facing format. HTML is one serializer.**
3. **P2 narrows** to what P3-G needs: Anthropic conforming (`P2-01`—`04`, `P2-17`) and honest cost with a real ceiling (`P2-18`, `21`, `22`).
4. **Deferred with owners:** P3-F memory (14 tasks), P5 Graph, three adapters, P3-D subagents, P2 polish, P4 Fleet.
5. **AutoDev publishes a capability contract and stops there.** It does **not** implement `source.psd.reference` or `integration.invoke`.

**Why.**

Razi: *"the ending is wiring AutoDev to AutoDesign — a design can be translated into HTML and CSS which in reality is lines of code or what AutoDev can do."*

**On (2), which is the load-bearing decision.** The spike converted two real PSDs successfully — and produced **17.41 MB** and **25.27 MB** of HTML, because every layer is an embedded base64 PNG. That cannot enter a model's context, so an agent cannot adapt it. The converter's own architecture document already stated the answer before we started: *"the in-memory layer array is the universal intermediate representation — everything downstream is just a different serialization."* The IR is a few KB. Handing the model the HTML instead makes the feature impossible rather than merely slow, which is why `P3-46` is ordered before every adaptation task.

**On (5).** AutoDesign's capability manifest has exactly **two** capabilities `available`. `source.psd.reference` is **planned** at G3B and `integration.invoke` — the capability that would invoke AutoDev at all — is **deferred** at **G6**, annotated `future_forge_extension`. The binding constraint on the wired outcome is therefore AutoDesign's roadmap, not AutoDev's, and Razi ruled that AutoDesign is owned elsewhere. Building to a published seam lets both halves proceed in parallel; building past it would be building against a schedule we do not control.

**This also preserves P8.** The two apps talk over AutoDesign's existing loopback server, so AutoDev never contains Forge, and *"not one line of sidecar code survives"* stands unchanged. Embedding Forge was the only version that broke it.

**On the estimate, stated rather than absorbed.** Razi accepted *"~8—9 days"* against a figure priced on **reformat only**. He then confirmed adaptation is **three** kinds. The number is now **~11—12 days**. An estimate that cannot express having grown is the same defect class as `L-C9` — a verification whose shape cannot express failure.

**When.** 2026-09-01 · P1 closeout → P2/P3 re-scope

**Where.** `MASTER_DEVELOPMENT_PLAN.md` §5 phase table, §9 P3-G, S3-06—S3-10 · `_verify/AUTODESIGN-INTEGRATION-RESEARCH.md`

**How.** Conversion is already `SPIKE-PROVEN`: two real PSDs (31.8 MB / 41.9 MB, 37 layers), both orientations, verified **visually** rather than from a log. ⚠ The first spike used 1080×1080 — the size the refinements notes say already works — so the portrait case was run **specifically to check** rather than assumed; it renders correctly and that documented bug is already fixed. Two defects were found and are now tasks (`P3-47`, `P3-48`).

**Links.** `_verify/AUTODESIGN-INTEGRATION-RESEARCH.md` · `P3-45`—`P3-54` · `S3-06`—`S3-10` · `ADR-LOG-RESTRUCT-0019`

## ADR-LOG-RESTRUCT-0024 — AutoDev ships twice; the design work becomes seven phases; every phase now has tasks

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled` (third-pass direction, 2026-09-01)

**What.**

1. **Two rollouts.** R1 = *"the backend is ready, wire it to AutoDesign."* R2 = *"the entire application is ready for usage."* R1 ships first and is a **complete product with a different consumer**, not a subset of R2.
2. **The design work is promoted from `P3-G` (10 tasks) to `PD0`—`PD6` (88 tasks).**
3. **`P8` moves into R1, before the handover.**
4. **`P5`—`P9` get full task breakdowns now** (107 tasks) rather than at phase start.
5. **Providers become three** — Anthropic, OpenAI, Google, with **Gemini primary**. `P2-07` struck, `GAP-02` closed WONTFIX.
6. **`P3-F` memory returns to the critical path**; its deferral is withdrawn.
7. **Nothing is "deferred."** Every phase has a slot in R1 or R2.

**Why.**

**On (1).** Razi: *"The first one need to come first, the second one is the second rollout."* The consumer differs, so the definition of done differs. In R1 AutoDesign supplies the UI, the file, the user and the Apply/Reject surface; AutoDev supplies capability. Treating R1 as "R2 minus the UI" would have sequenced UI work before capability work and delivered neither.

**On (2).** The scope Razi added does not fit a lettered sub-section: pixel-level fidelity **detection with refinement tooling**, PDF ingestion beside PSD, export to three formats including back to PSD, adaptation as a deterministic tool, a change system, and brand-guide ingestion. Sizing it honestly gives 88 tasks. Leaving it as a sub-section would have hidden a phase-sized body of work inside a phase.

**On (3), which is the sequencing change with real evidence behind it.** Razi: *"we dont want them to have an unused sidecar on the backend of AutoDesign."* Measured, not assumed: **3,867 LOC** of sidecar Rust, **17** Tauri commands, **225 MB** of bundle resources, **2** `.gitmodules` stanzas, **14** referencing files. It is affordable now because `P1` already built the seam — `autodev-core` declares `OrchestratorProbe` and ships **`NoOrchestrator`**, so the excision is a seam swap rather than a port — and `runtime_config.skip_sidecars` already provides a tested path that never starts them. Its one real dependency is `P3-F`, which replaces `memory-mcp-server`.

**On (4).** `§11` previously argued that writing task lists early guarantees staleness. That reasoning was sound and its guard survives: `§0.12` still requires re-reading the specs at each phase start. What it did not price was the cost of the *absence*: two rollouts cannot be sequenced against phases whose contents nobody has written, and Razi could not be told when to hand over.

**On the doctrine in `PD4`.** *"The entire tool does the calculation and stuff and the LLM/AutoDev fleet here is only the executor of that tool."* Adaptation geometry is **deterministic code**. The fleet chooses when to adapt and to what; it never computes the layout. `PD4-02` gates the absence of a model call inside the tool, so this is a checked property rather than an intention.

**On `PD5`'s grounding.** Razi: *"if we can build it from scratch then we can tune and reverse engineer it."* Verified in the source: the v4.7 pipeline constructs shape layers (`vectorMask` Bezier knots + `vectorFill`), editable type layers and pixel layers from nothing, then writes them with `writePsd`. **Construction reversed is mutation** — so changes are not new machinery, they are the existing machinery run backwards.

**On `PD3-06`'s boundary.** Razi asked for semantic TypeScript/React *"where HTML and CSS cannot handle it with the code limitations."* Rather than leave that to taste, the eligible set is pinned to the source's own fidelity table — its **⚠ Approximate** and **❌ Not Supported** rows (Dissolve, Subtract, Divide, Hard Mix, bevel/emboss/satin, pattern overlays, adjustment layers). A component is written only for a row on that list. Without that anchor "semantic components" expands until it means rewriting the design by hand.

**When.** 2026-09-01 · after P1 closeout, before P2

**Where.** `MASTER_DEVELOPMENT_PLAN.md` `§5` (rewritten), `§10.5` (new, PD0—PD6), `§11` (rewritten, P5—P9) · `vision/00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md` (directions 14—25)

**How.** Grounded in the source repository, read rather than recalled: `psdconversions/index.html` (3,202 lines) and its four architecture documents — the four-phase operator-list pipeline, per-draw-op CTM tracking, the ten text-merge guards, the seven ingestion guards, the fidelity classification table, and the PSD construction path. The sidecar numbers are `wc -l` and `grep -c` against the tree on 2026-09-01. The velocity figures are measured from this repository's own commit timestamps: **P1 built 22 tasks in ~2 h 50 m and its post-phase audit cost a further ~3 h 30 m**, which is why the estimates below carry the audit rather than omitting it.

**Links.** `_verify/AUTODESIGN-INTEGRATION-RESEARCH.md` · `ADR-LOG-RESTRUCT-0023` (superseded in scope by this entry) · `PD0`—`PD6` · `P8`

## ADR-LOG-RESTRUCT-0025 — P2's pre-build verification corrected the plan in five places before any code

**Status.** DECIDED
**Author.** Claude · **Tier.** `decided-independently` (`§0.2m` requires the plan be corrected before the first line of code; none of these narrows a `§0` rule or changes a shipped contract)

**What.** Before writing P2, its load-bearing claims were checked against the code and against
spec `02`. Five corrections landed in the plan first.

1. **`P2-02` reverted to four `CachePolicy` variants.** My own edit of 2026-09-01 was wrong.
2. **`P2-29`—`P2-33` added** — five defects spec `02` names that the task list had dropped.
3. **`S2-06`/`S2-07` added** — smokes for the two new defects that undermine existing tasks.
4. **The `§8` goal line** still read *"Four conforming providers"* after xAI was struck.
5. **`P2-06`'s citation corrected** — the defect is at `google.rs:341`, not `:313`, and it is **streaming-only**.

**Why.**

**On (1), which is a `§0.13` dual-anchor conflict against my own work.** When Razi struck xAI I
rewrote `P2-02` as *"three variants — Anthropic explicit, OpenAI automatic, Google differs"*,
mapping variants one-to-one onto providers. Spec `02 §2` does not do that: `Off` /
`Automatic { affinity_key }` / `Explicit { breakpoints, ttl }` / `Handle { id }` are **policy
shapes**, and providers share them. Striking xAI removes a *user* of `Automatic`, not the
variant — and OpenAI's caching is automatic too, so the variant survives on its own merits.
Reducing shapes to a per-provider tally is precisely the *"collapsing this to a boolean"* the
spec warns *"loses the feature on at least two providers."* The count was a coincidence of
arithmetic, not a reading of the design.

**On (2), which is `L-C3` caught before the build rather than after it.** Two of the five do
not merely add work — they **invalidate a task already written**:

- **`cost_today_usd` is never reset (`IN-50`)**, so *"today"* means *"since process start"*.
  `P2-21` specifies a **daily** spend ceiling. A ceiling over a counter that resets on restart
  is a per-process ceiling: restart twice, spend three times the cap. `P2-21` could have been
  built, tested, and demonstrated passing while being wrong in exactly the way that costs
  money.
- **The daily rollup sleeps an hour before its first run and aggregates only *yesterday*
  (`IN-56`)**, so a desktop app used in sub-hour sessions never completes one. The ceiling
  reads what the rollup writes.

The other three are `P2-31` (18 hardcoded model strings, and `analyze_diff_tool.rs:219-222`
falling back to `claude-sonnet-4-6`, an id in no catalogue), `P2-32` (`airsctl` takes a key as
a **CLI flag** — leaking into shell history *and the process table* — and falls back to
`ANTHROPIC_API_KEY`), and `P2-33` (the vault's missing `enumerate`, `IN-49`).

**`P2-32` is a standing contradiction, not a gap.** `INV-12` and the `G-KEY-NOENV` gate
require that with an empty keychain and every provider env var set, the app **refuses to run
a turn**. `airsctl` currently does the opposite. The gate and the code cannot both stand, and
the code is what changes.

**On (5).** The plan cites `google.rs:313` `let _args = ...`. The file moved into
`autodev-core` during P1 and the line drifted to **341**. More importantly the description is
imprecise: the **non-streaming** path at `:402` handles arguments correctly, so the defect is
**streaming-only**. That changes the fix — `StreamChunk::ToolUseStart` carries no input field,
so the arguments must follow as a `ToolInputDelta`, which the enum already supports. Read as a
one-line fix it would have been mis-scoped.

**When.** 2026-09-02 · P2 · before the first line of code

**Where.** `MASTER_DEVELOPMENT_PLAN.md` `§8` (goal, `P2-02`, `P2-06`, new `P2-E`, `S2-06`/`S2-07`)

**How.** Claims were checked against the tree, verbatim with `path:line`, per `§0.2L-1`:
`max_tokens` appears **only** in `anthropic.rs:134` — OpenAI and Google never send it
(`P2-08` CONFIRMED). `cost/tracker.rs:61` reads `_ => (1.0, 3.0), // Safe default`
(`P2-18` CONFIRMED). `google.rs:341` discards `_args` in the streaming path while `:402` uses
them (`P2-06` CONFIRMED, corrected). Three read-only refutation agents were dispatched in
parallel over caching, cost and BYOK, each carrying `§0.2i`'s incremental-write clause
verbatim and instructed not to read `.env`.

⚠ **`§0.2m` earned its place again.** The rule exists because this plan's own pre-GO review
found six errors in it. This pass found five more, one of which was **mine, made yesterday**,
and two of which would have produced a spend ceiling that passes its own test and does not
cap spending.

**Links.** spec `02 §2`, `§4.2`, `§5`, `§7`, `§11` · `_verify/P2-REFUTE-*.md` · `L-C3` · `L-4` · `INV-12`

## ADR-LOG-RESTRUCT-0026 — P2 absorbs every audit finding; adapters get fixtures in CI and a live smoke behind a flag

**Status.** DECIDED
**Author.** Claude · **Tier.** `razi-ruled`

**What.**

1. **All ~30 pre-build audit findings enter P2**, not a deferral register. `P2-34`—`P2-75` in four new groups: **F** adapter conformance, **G** cost and analytics integrity, **H** caching prerequisites, **I** test infrastructure. P2 goes from 32 tasks to **74**, and from ~11 h to an honest **~20 h+**.
2. **Adapters are proven by fixtures in CI plus a live smoke behind a cargo feature** (`P2-67`—`P2-69`).

**Why.**

Razi, asked directly: *"everything the audits found"* and *"fixtures + a live smoke behind a flag."*

**On (1).** Three read-only refutation agents (§0.2m) returned findings the written tasks did not cover, and three of them **invalidated tasks as written** rather than merely adding work:

- **`with_task_budget` has zero callers.** `engine.rs:164-180`, `escalate_budget_exceeded`, `TaskBudget::default_worker` and `BudgetExceededReason` are unreachable — ~100 lines of enforcement with no entry point. **There is no enforced spend limit at any scope.** `P2-21` could have been built, tested and demonstrated green while capping nothing.
- **The byte-stability premise is refuted.** All four causes the plan names — timestamps, paths, counts, HashMap order — are **absent** from the system prompt. `to_definitions()` already sorts (`tool_registry.rs:43`) and `serde_json` is already a `BTreeMap`. Within a `run()` the prefix is byte-identical **by construction**. `TG2-06` as written would have **passed on day one and proved nothing** — the programme's own inert-but-green failure.
- **"Handle `message_start`" fixes zero of three providers.** OpenAI never *asks* for usage (`stream_options` absent tree-wide) and separately discards the usage frame on empty `choices`; Google's handler is blocked by two earlier returns.

And one that decides the phase's own success: **every Gemini tool call executes with empty input** (`google.rs:341` discards `_args`, no `ToolInputDelta` is ever emitted). Gemini is the **primary** provider for R1.

**Absorbing rather than deferring is the more expensive choice and the right one here.** These are not adjacent defects; they are the provider and cost layers P2 exists to make correct. A deferral register for *"the price table is wrong for 9 of 12 models"* would mean closing a phase whose headline is *"cost is honest"* over a known-fabricated price.

**On (2).** Measured: **1,733 lines of adapter code with zero `#[test]` of any kind.** Every defect in `P2-F` survived because nothing could catch it. Fixtures run in CI with no credentials — §0.5: *a gate skipped in CI is not green* — and the live smoke catches vendor wire drift that fixtures freeze and hide. Credentials are set **per-command, never session-wide** (§0.8).

**A correction to my own recent work, recorded rather than quietly fixed.** `P2-11` called Ollama removal *"clean"* and *"dead weight"*. Measured: **537** LOC not 506, **31** Rust references across **10** files not *"10 sites in 8 files"*, and `ollama-rs` in **two** manifests. It is a **live, user-facing provider** — enumerated at `settings_commands.rs:28-33`, rendered by `Settings.tsx:67-68`, reached by five dispatch switches. Deletion is a user-visible feature removal, which Razi has ruled, but it is not a cleanup. And the tree now **contradicts itself**: `PROBED_PROVIDERS` marks Ollama retired while `settings_commands.rs` presents it as live — half of that introduced by me on 2026-09-01 (`P2-75`).

**When.** 2026-09-02 · P2 · after §0.2m, before the feature build

**Where.** `MASTER_DEVELOPMENT_PLAN.md` §8 groups F—I · `_verify/P2-REFUTE-{caching,cost,byok-adapters}.md`

**How.** Each task carries the measured `path:line` from the report that found it, quoted verbatim per §0.2L-1. `P2-H` is ordered **before** `P2-B` deliberately: a messages-level invalidator looks exactly like *"caching doesn't work"* while the prefix is perfectly stable, which is the hardest possible bug to attribute.

⚠ **The dispatch itself had a defect, recorded not hidden.** All three agents carried §0.2i's incremental-write clause and were dispatched as **read-only** agent types with no write tool, so none could write its report. The reports survived only because they fitted in their completion notifications. That is `L-C8` — a recorded lesson not applied — and the rule it yields is that an agent required to write a file must be given a tool that can write one.

**Links.** `ADR-LOG-RESTRUCT-0025` · `_verify/P2-REFUTE-*.md` · `L-C8` · `L-C9` · `L-C12`

## ADR-LOG-RESTRUCT-0027 — two P3 gates had no owning task; Razi ruled both into the phase

**Status.** DECIDED
**Author.** Claude (found) · Razi (ruled) · **Tier.** `razi-ruled` — both are capability calls
that change the phase's scope, so `§0.15` puts them with the overseer, and `§0.2j` sends them
immediately rather than at closeout.

**What.** Reading spec `06` and `TESTING_GATES.md` against `§9`'s task list, before any P3 code,
found **two gates that exist with no task that would build them**. Razi ruled both into P3:

1. **`TG3-37` — the provisional-charge leak.** Fixed in P3, in full. New task **`P3-55`**.
2. **`TG3-26` — egress consent.** The **prompting half** only, folded into `P3-11`'s
   `ToolEffect::Network`. New task **`P3-56`**; the consent store is recorded against P4 rather
   than left orphaned.

**Why.**

**On the orphaned gates as a class.** `§0.2e` says every task names its proof. This is the
inverse failure and it had no rule watching for it: **a proof with no task**. `TG3-37` and
`TG3-26` are written, numbered, and were going to be scored against a phase that builds
neither — so they could only ever have gone red at closeout, or, worse, been quietly marked
not-applicable. `L-C1`'s shape (a derived artifact not regenerated after its source changed) is
adjacent but not the same: nothing here went stale, the two documents were **never reconciled in
that direction**. The MDP was harvested from the specs, and the gate register was written from
them separately; a gate the task harvest missed is invisible from the MDP side.

**On `TG3-37` specifically — this is a live denial of service, not an accounting nit.**
`spawn_qc_review` (`G10-67`) records a **$0.10 provisional charge before eight distinct error
returns** and never reconciles it. The dispatcher ticks against already-claimed tasks; 50 ticks
trip the **$5 hourly ceiling** and **block all QC review for an hour at zero real spend**. The
fleet disables its own review capacity, and the ledger says it was busy. It is also exactly the
shape of `P3-05` — *every exit path must do the thing* — which is why it sits well in this
phase rather than in P4: the same `Drop`-guard reasoning covers both.

**On `TG3-26` — why the prompting half is the right cut.** The gate as written wants a
consent **store**: per workspace, per destination, recorded, revocable, with unattended work
running under a standing consent whose absence blocks egress. That is a feature with a UI, and
P3 has no UI slice. But the hole it protects against is live today and needs no store to close:
`analyze_diff` declares `is_read_only = true`, is therefore **never prompted in any mode**, and
**uploads the full diff of any repository to a third-party LLM** — under BYOK, that is the
user's own code leaving the machine on the user's own key, with no prompt. `P3-11` already
introduces `Network` as a distinct effect; making it prompt closes the exfiltration path using
machinery the phase builds anyway. Razi's ruling takes that and leaves the store for P4 **with
an owner**, which is the part that stops it drifting back to orphaned.

**When.** 2026-09-02 · phase P3, before the first line of code · triggered by the `§0.12`
re-read and the `§0.2m` pre-build pass.

**Where.** `MASTER_DEVELOPMENT_PLAN.md §9` (`P3-55`, `P3-56` appended — **not** renumbered,
per `§1`) · `TESTING_GATES.md` (`TG3-26`, `TG3-37` gain their owning task) ·
`autodev-core/src/agent/builtin_tools/spawn_qc_review.rs` · the `ToolEffect` work in `P3-11`.

**How.** `P3-55` is proven by `TG3-37`, parameterised over all eight error returns. `P3-56` is
proven by `TG3-24`'s effect-keyed check plus a case asserting a `Network` tool prompts where an
equivalent `Read` tool does not.

⚠ **Sequencing carried from `C-6`.** `P3-56` cannot land before `P3-11`: reverting
`is_read_only` to a concurrency hint and introducing `ToolEffect` must happen in the same
change, or a window opens in which **nothing** gates prompting.

**Links.** `TG3-26` · `TG3-37` · spec `06 §1.2`, `§5` · `G10-67` · `C-6` · `§0.2e` · `§0.2j`

## ADR-LOG-RESTRUCT-0028 — P3's pre-build refutation: two headline claims false, five tasks already done, C-5 retracted

**Status.** DECIDED
**Author.** Claude · **Tier.** `decided-independently` (`§0.2m` requires the plan be corrected
before the first line of code; none of this narrows a `§0` rule or changes a shipped contract)

**What.** Three adversarial agents were dispatched against P3 A–E's load-bearing claims before
any code, per `§0.2m` and Razi's explicit instruction. Every load-bearing result below was
**re-verified in the main session** before it changed a row. The plan, and one standing fact in
`§5` of this log, were wrong in the following ways.

### 1. 🔴 `P3-01` was a no-op, and it hid four more

`[profile.release]` lives at `src-tauri/Cargo.toml:117-122` — a **non-root workspace member**.
Cargo discards it, and says so:

> `warning: profiles for the non root package will be ignored, specify profiles at the workspace root:`

The root `Cargo.toml` has **no `[profile]` block at all**. So `panic = "abort"` was **never in
effect**; unwinding is already on; and editing that line — the phase's headline fault-isolation
task — would have changed nothing while appearing to close `M-41`. `lto = true`,
`codegen-units = 1`, `opt-level = "s"` and `strip = true` are inert for the same reason, so
every release build ships un-LTO'd and unstripped.

**This is the `§0.2m` rule paying for itself in a single finding.** A verification that runs
after the build measures the build; this one measured the plan and found its headline task
inert.

### 2. 🔴 `P3-03`'s mechanism was wrong, so its fix would have been wrong

The process does **not** die on a panic: every agent path is inside `tokio::spawn` and
unwinding is live. What happens is **`Mutex<AppState>` poisoning** — **98 `.lock().unwrap()`
sites against 5 poison-tolerant** — so one panic becomes a cascade of unrelated panics at every
later lock: a restart-only failure presenting as something else entirely. `catch_unwind` alone
would not have fixed it.

### 3. Five tasks were already done, and one was done by me last phase

`P3-23` (depth threading and caps), `P3-26` (`context_window`, done in `P2-57`), `P3-27`
(compacted history used, and its usage folded in by `P2-66`) are struck. `P3-24` and `P3-25`
are half-done. **A task list that says "build this" for work already shipped is not merely
wasteful — it invites a second implementation beside the first**, which is `L-C12`'s exact
soil.

### 4. Two counts were wrong in a way that would have caused the defect they warn about

`P3-05` says four silent exits; there are **three** (`engine.rs:127`, `:327`, `:627`) —
`:578` already emits. Hunting a fourth lands there and produces a **double emission**, which
`TG3-01` fails exactly as loudly as zero. `P3-04` says three byte-slice panics; there are
**four** sites, they are **not identical**, and every line number in the task is wrong.

### 5. 🔴 Retraction — standing fact `C-5` is false

`C-5` states: *"The correct UTF-8-safe truncation **already exists** at `validate.rs:846-853`."*
It does not. **`846-853` is pyright/mypy dispatch plumbing.** The truncation helper is at
**`945-957`**, it truncates one character early, and it carries an `unwrap_or(MAX_CHECKER_BYTES)`
fallback that would be the exact unsafe byte-slice if it were reachable (it is not, because the
length guard makes `char_indices` non-empty — dead defensive code, but a trap for the next
reader). Two **genuinely** correct helpers exist that no document mentions, duplicated verbatim
at `memory/recall.rs:183-192` and `memory/summarizer.rs:148-157`.

**How the error happened, per `§6`:** the citation was carried from a grounding report into the
spec, into the MDP, and into this log's standing facts **without anyone opening the file** — four
documents agreeing because they share one unchecked source. That is `L-C1`'s shape (a derived
artifact never reconciled with its source) and it is why `§0.2L-1` requires quoting verbatim
with `path:line`.

### 6. My own P2 gate asserts on a field production never reads

`catalogue.rs:377-383` — the test written to prove `P2-57`'s fix — asserts on
`ModelCapabilities::context_window`, and production reads `ModelInfo::context_window`. **Two
different fields**, agreeing only because both derive from the same `CatalogueEntry`. A
regression in `models_for()` alone leaves the test green and Gemini compacting at 200k again.
`L-C6`, sixth instance, **inside the test written to stop `L-18` recurring** — logged as
`P3-59`.

**Why.** `§0.2m`: *a verification that runs after the build measures the build; one that runs
before it measures the plan.* Six of the twenty-two A–E claims were false as written, five were
already satisfied, and two carried counts whose correction prevents the defect the task exists
to fix.

**When.** 2026-09-02 · phase P3, before the first line of code.

**Where.** `MASTER_DEVELOPMENT_PLAN.md §9` (P3-01, 03, 04, 05, 06, 07, 08, 09, 22, 24, 25, 28,
29 corrected; P3-23, 26, 27 struck as done; P3-57…P3-63 appended) · `§5 C-5` retracted below ·
`_verify/P3-REFUTE-{loop,context,tools}.md`.

**How.** Each corrected task carries its own evidence class. The refutation reports hold the
verbatim quotes; this entry holds the decisions.

**Links.** `ADR-LOG-RESTRUCT-0027` · `§0.2m` · `§0.2L-1` · `L-C1` · `L-C6` · `L-C12` · `C-5`

# §5 · Standing entries carried from planning

Not build-time decisions — but recorded here because a future session will otherwise rediscover them the hard way. Each is already ratified in vision Part 8; these are the **operational consequences** that bite during implementation.

| # | Carried fact | Consequence during the build |
|---|---|---|
| C-1 | `subagent/bridge.rs`'s SSE frame parsers are `pub(crate)` and **shared with** `sidecar/client.rs` | Those two deletions **cannot be sequenced independently**. Attempting one alone breaks the build. |
| C-2 | ACP sender identity is set in **exactly one place** — the MCP adapter deleted in P0 | `ToolContext.agent_role` must be truthful **before** P4 relies on messaging, or a preserved capability dies silently. |
| C-3 | ~~`graph/` migration head **v5 is a broken statement**~~ → **RESOLVED in `P1-11`.** v5's six dead statements were removed and `migration_v6` added; the live graph store is at v6. ⚠ And the *premise* was wrong: the columns were never missing — `CREATE_NODES` declared them from v1, so v6 is **defensive**, not corrective (`ADR-LOG-RESTRUCT-0017`). | Historical. |
| C-4 | Four of seven surviving MCP items live inside **sidecar-named files** | They must be relocated **by address**, or sidecar code survives P8 by filename alone. |
| C-5 | ~~The correct UTF-8-safe truncation **already exists** at `validate.rs:846-853`~~ → 🔴 **RETRACTED 2026-09-02, `ADR-LOG-RESTRUCT-0028`.** `846-853` is pyright/mypy plumbing. The helper is at **`945-957`** and is itself flawed (off-by-one; an unreachable-but-unsafe `unwrap_or` fallback). Two genuinely correct helpers, unmentioned anywhere, are duplicated at `memory/recall.rs:183-192` and `memory/summarizer.rs:148-157`. | There are **four** panicking sites, not three, and they are **not identical** — `bash.rs:162` slices a *computed tail*, so a fix templated on `&x[..N]` leaves it panicking. The task is one shared helper replacing four sites **and** two duplicates. |
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

---

## ADR-LOG-RESTRUCT-0029 — P3 A–E build-time decisions: two Razi rulings, four judgement calls, two gates left red

**Status.** DECIDED
**Author.** Claude · **Tier.** mixed — two `razi-ruled`, four `decided-independently`,
two `flagged-and-proposed` (recorded here and surfaced in chat, since Razi is reading
from a phone and cannot open files)

**When.** 2026-09-02 · Phase P3 A–E · triggered by tasks `P3-16` … `P3-68`

---

### razi-ruled · 1. A checker that could not run is not a tool error (`P3-18`)

**What.** `check_file` returns SUCCESS when a validator binary is missing or times
out, carrying a prominent `NOT CHECKED` line, rather than returning an error.

**Why.** The task text says a missing binary "is an **error**". Taken literally that
turns a missing *optional* dependency into a failed tool call the model cannot repair
by retrying — on a machine without `tsc`, every `.ts` check would fail forever. The
defect being fixed is the **false claim**, not the absent tool: `is_clean()` returned
`true` for a file nothing had examined, and `check_file` printed `✓ No syntax errors.`
Razi ruled for the narrower reading.

**Where.** `autodev-core/src/agent/builtin_tools/check_file.rs`,
`…/validate.rs` (`SEVERITY_UNAVAILABLE`, `LintResult::ran`)

**How proven.** `p3_17_18_tests` (7), `TG-P3-18`, falsified.

---

### razi-ruled · 2. Wire `permission_mode`, and flip the cron recipes (`P3-65`)

**What.** `runner.rs` builds the engine from `PermissionMode::from(&recipe.permission_mode)`;
`worker-platform`, `qc-agent` and `qc-dispatcher` now declare `Autonomous`;
`fleet-smoke` keeps `Supervised` and now genuinely receives it.

**Why.** The field was written by nine recipes and read by nothing. Honouring it
naively would stall the fleet — three of the four `Supervised` recipes run on cron
with nobody present to answer a 300s prompt. Razi chose to honour the field *and*
correct the declarations, over deleting the control or leaving it decorative.

**Where.** `autodev-core/src/recipes/{runner,types,defaults}.rs`

**How proven.** `TG-P3-65` (falsified three ways) + 6 unit tests.

---

### decided-independently · 3. `scheduled + supervised` is refused, not downgraded

**What.** `run_recipe` returns an error naming the recipe, its schedule and the fix,
rather than silently running Autonomous or blocking on a prompt.

**Why.** `TG-P3-65` covers the nine shipped recipes; **users author their own**, and
`RecipeEditor.tsx` puts `permission_mode` and `schedule` on the same form — the
invalid pair is two clicks away where no source gate can reach. Downgrading silently
would re-create the exact `P3-65` defect as a "convenience"; honouring it would block
every Execute/Network/Board tool for 300s and then deny. Both do something other than
what was asked. This says so instead.

**How proven.** `p3_65_runner_tests` (3), including both no-fire halves.

---

### decided-independently · 4. `P3-61` fixed at the chokepoint, not per caller

**What.** `pick_servable_model` lives in `native_runner`, which every subagent passes
through, rather than being repeated at `spawn_subagent`, `spawn_parallel_subagents`
and `spawn_qc_review`.

**Why.** The task names two callers with the defect. A per-caller fix leaves the
*next* caller broken, and this defect has already appeared twice independently. A test
covers the QC id explicitly so the second case is not merely assumed to be covered.

---

### decided-independently · 5. `A3` stays at 0% in the readiness matrix

**What.** P3 A–E is complete — 53 of 68 tasks, 19 gates, 520 tests — and the scored
area does not move.

**Why.** `A3` is scored per *phase* and `P3-F · Memory` is 15 tasks untouched, so the
Build row is genuinely unmet. `§0` says no partial credit. Recording A–E against a row
spanning A–G is the arithmetic dishonesty that rule exists to prevent. The **safety
overlay** is where the change was recorded instead, because that is where the signal
actually is.

---

### flagged-and-proposed · 6. `TG3-08` and `TG3-10` left RED with the gap stated

**What.** Two spec gates in `TESTING_GATES.md` are annotated and left unchecked.

**Why.** `TG3-08` wants a regression slope over turns 10–30 against a recorded
baseline, asserted as a number; `S3-01` proves the *ordering* that bounds growth,
which is weaker, and checking a gate against a property it merely resembles is how a
green row stops meaning anything. **`TG3-10`'s premise is refuted by measurement**: it
requires clearing to fire only when the saving exceeds "the forced cache re-write it
causes", and there is no such cost — `anthropic.rs` sets a breakpoint on the system
block only, so nothing in the message region is cached. The gate must be re-pointed,
or the premise restored by adding a message-region breakpoint, before it can pass.

**Needs Razi.** Whether to re-point `TG3-10` or to add a message-region cache
breakpoint (which would make the original trade-off real, and require re-deriving
`CLEAR_TRIGGER_FRACTION`).

---

### flagged-and-proposed · 7. Two of my own gates were green and wrong

**What.** Recorded as `L-25` and `L-26` in the MDP `§0.16` register; `L-C9` incremented
to 9.

**Why it is here and not only there.** Both were caught by falsification, not by
review, and both came from *fixing* something: `TG-P3-20` went blind because I widened
its search to silence a false positive, and `TG-P3-55`'s falsification proved nothing
because two of three planted defects did not compile — `cargo test` exited 101 on a
**build error** that looked exactly like a gate firing. A third variant: `shutil.copy2`
preserves mtime, so cargo can reuse the artifact built from a planted file *after* the
restore, which can report a false green about whether the restore happened.

**How applied.** Every falsification after that point asserts both that the run failed
**and** that it failed for the stated reason, and restores are verified by reading the
file rather than by re-running the gate.

---

## ADR-LOG-RESTRUCT-0030 — P3 A–E's post-phase audits: nine of my own claims were false, and the fleet cannot run

**Status.** DECIDED
**Author.** Claude · **Tier.** `decided-independently` for the remediation;
`flagged-and-proposed` for `P4-31` and the residual triage
**When.** 2026-09-02 · after P3 A–E built green (43 tasks, 19 gates, 520 tests)

**What.** Two adversarial agents were run per `§0.2a` over disjoint briefs — one to
**refute the completion claims** (`_verify/A9-P3-claims-refutation.md`), one to hunt
**defects nobody had fixed** (`_verify/A10-P3-gap-hunt.md`). Both were told to refute
rather than confirm, and neither had seen the reasoning behind the work. 47 findings:
2 critical, 16 high, 19 medium, 9 low, 1 unresolved question.

**Why it matters that these were found by an audit and not by me.** Every one of the
nine false claims below was written by someone (me) who had just read the code, run the
tests, and watched the gates go green. Four of them had a gate specifically written to
protect them, and the gate was green.

### The nine claims of mine that were false

| Task | The claim | What was true |
|---|---|---|
| `P3-58` | the depth boundary moved | `d >= 3` and `d + 1 > 3` are **one predicate**; the fix was a no-op and the task's premise was also wrong |
| `P3-24` | cancellation reaches the children | `terminate` **skipped the abort** for any awaited subagent and returned `Ok` — i.e. for every in-flight one |
| `P3-62` | a timed-out subagent reports its spend | it reported **$0.00**; `timeout` drops the future before any exit path runs |
| `P3-64` | every tool passes exactly one gate | **two** gates, the second with a fresh call id |
| `P3-08` | the cancel arm is wired | `with_state` had **zero callers**, so the arm could never fire |
| `P3-68` | `probe_url` is confined to loopback | confined the **first request only**; redirects were followed |
| `P3-05` | `GATE-PROVEN` | the named gate **did not exist**; emptying the guard's `Drop` broke no test |
| `P3-16` | one shared helper, three consumers | **one** consumer; the others do not exist yet |
| `P3-63` | zero means the worker default | that branch is **unreachable** (harmless: both constants are 20) |

### The two live security holes neither task list named

1. **A subagent inherited `Autonomous` regardless of its parent.** One `spawn_subagent`
   call escaped `ManualApproval` entirely, with `bash` and the full registry, on the
   user's own key. The justifying comment — *"a subagent has no user to prompt"* — was
   contradicted two lines below by `deps.pending` being passed straight in.
2. **`e2e_probe` spawned model-chosen shell without the credential scrub `bash` applies**,
   so its children inherited every provider key. `bash`'s own comment records that ITS
   first version of that fix landed in one branch only, "which is its own small lesson
   about two-branch fixes" — this was the same lesson one tool over.

### `P4-31`, which is bigger than anything in P3

**Eight of the nine shipped recipes cannot run.** All are `fallback_policy: Mode2Only`
against a Mode 2 inert since P0, so `run_recipe` logs `fallback_skipped_policy` and
returns `Ok(())` every tick, forever. The fleet reports itself **idle rather than
broken**. Filed rather than fixed: which recipes should be `Mode1Capable` — or whether
`Mode2Only` should mean anything now — is a dispatch decision.

**Needs Razi.** That decision, and the residual triage (33 findings, none critical,
listed in the two `_verify` reports with severity and reproduction).

### How the gates failed, which is the transferable part

Four green gates sat over live defects, and the pattern is one thing: **each asserted
the shape of the change rather than the property the change existed to establish.**
`TG-P3-11` asserted `is_read_only` was absent — true, and silent about a second prompt.
`TG-P3-58` asserted a predicate rewrite that was a tautology. Recorded as `L-27`, with
the practical tell: *if the assertion would still pass after a **different** fix for the
same defect, it is keyed to the diff.* A second shape, same entry: a gate that
**enumerates** its subjects finds only the ones somebody already listed — `TG-P3-58`
checked two files and missed the third guard, written in the very spelling its own
failure message called a defect.

**How proven.** Every remediation carries a test or gate, each falsified with a
**compiling** planted defect and the failure reason read rather than assumed (`L-26`).
534 tests, 22 `TG-P3` gates.

---

## ADR-LOG-RESTRUCT-0031 — P3-F's pre-build refutation: the architecture held, the dependency story did not, and three defects had no task

**Status.** DECIDED
**Author.** Claude · **Tier.** `decided-independently` for the plan corrections
(`§0.2m` requires the plan be corrected before the first line of code);
**`razi-ruled`** for the platform boundary
**When.** 2026-09-02 · before any P3-F code

**What.** Two adversarial agents were dispatched against P3-F's load-bearing
claims per `§0.2m` — one against the **external dependencies**, one against the
**state of this repository** — both briefed to refute and to default to
"refuted" on ambiguity. **Every load-bearing result was re-verified in the main
session before it changed a row**, and two of the agents' own claims did not
survive that.

### 1. The architecture held — and it was tested, not read

`sqlite-vec`'s entire `[dependencies]` is `cc`. It does **not** depend on
`libsqlite3-sys`, so there is nothing to conflict with our pinned
`rusqlite 0.31`; `loadable_extension` never enters the picture, so it never
meets `bundled`. A scratch crate on the repo's exact pins built on Windows MSVC.
**FTS5 is already present** in the bundled SQLite 3.45.0 — measured here by
creating a virtual table and matching against it.

**`vec0` partition keys are a genuine pre-filter**, proven with a *discriminating*
fixture: `k=2 AND scope='a'` surfaced a row at distance 1.4142 that a post-filter
over a global top-k could never return. Limits measured rather than assumed:
**at most 4 partition columns, TEXT/INTEGER only**.

*A refutation brief that only ever confirms is a rubber stamp. This one confirmed
the part that mattered and broke the part nobody had checked.*

### 2. The dependency story was wrong in a way that would have shipped as a fix

**`P3-33` prescribed disabling `ort`'s default features.** `fastembed` already
depends on `ort` with `default-features = false`, and `ort` is not a direct
dependency of ours — **so the prescribed fix would have changed nothing while
looking like a fix.** The download flag lives on `fastembed`, not `ort`. This is
the `L-C12` shape one level out: not a helper wired to nothing, but a *mitigation*
applied to the wrong crate.

**`P3-34` was understated.** `with_cache_dir` is not authoritative — `HF_HOME`
overrides it, so any machine with Python ML tooling installed silently relocates
the 133 MB model outside the app data root, which is the exact INV-5 violation the
task exists to close.

### 3. razi-ruled — the platform boundary

Our macOS DMG is a **universal binary** and CI targets both Apple architectures.
**Microsoft publishes no x86_64 macOS ONNX Runtime in any release from v1.25.0 to
v1.29.0** — verified across eight releases, `osx-arm64` only. A universal binary
needs both slices to link, so adding `fastembed` unconditionally **fails the macOS
DMG outright**, and vendoring cannot supply what upstream does not publish.

**Ruling: the embedding backend sits behind a Cargo feature that is OFF for
macOS.** Windows and Linux get semantic recall; macOS gets FTS5/BM25 and reports
itself degraded — which is the architecture `03 §4.4` already specifies
("degradation, not failure"), not a workaround. No platform is dropped.

### 4. Three defects with no task — now `P3-45`, `P3-46`, `P3-47`

* **`P3-45`** — `system_prompt_override` returns an empty *volatile* half, and the
  memory block lives there. It is a user-facing settings field: **any user who
  customises their system prompt loses all recall, silently.**
* **`P3-46`** — recipes/subagents/MCP get no memory, and the **recorded cause is
  wrong**: recipes *do* carry session ids. The override is the mechanism. ⚠ I
  propagated that wrong cause into a **user-facing tool description** in `P3-20`.
  `L-C3`, and the source was a comment (`L-22`).
* **`P3-47`** — a fleet worker can write into the user's memory at
  **`confidence = 1.0`, `source_type = "user_explicit"`**, and `recall_facts` has
  **no scope predicate**, so it lands at the *top* of the user's "what I know
  about you" block. ⚠ **Dormant until `P4-31` made the fleet run — earlier the
  same day, by me, after the audit that would have caught it.**

### 5. Two agent claims that did NOT survive re-verification

* *"The Python sidecar is still present and wired — 323 `.py` files."* **13,665**
  files, and `git ls-files "*.py"` returns **0** — every one is untracked, the P8
  deletion backlog. `MEMORY_URL` is a constant plus one `/health` probe; **the
  memory path is pure Rust and never traverses it.** OVERSTATED.
* *"0 of the 7 defects exist."* True of the list as literally worded, and it
  buries the finding: the operative defect — query-independent recall — **does
  ship**, in all three tiers, and is worse than the list describes.

### 6. Live data, before anything

`~/.autodev/memory.db`: `user_version 1`, integrity ok, **39 episodic rows**.
Backed up with `VACUUM INTO` (WAL-consistent, the mechanism `db/backup.rs` uses)
and **read back before being trusted** — 39/0/0/0 rows and the version verified
from the copy, not the source:
`~/.autodev/backups/memory.db.pre-P3F.20260902-200211.bak`.

**How proven.** `_verify/` will carry both raw reports at closeout. The plan,
spec `03 §4.3` and `§4.6`, `context.rs`'s comment and `remember_this`'s
description were all corrected **before** the first line of code, which is what
`§0.2m` is for.

---

## ADR-LOG-RESTRUCT-0032 — P3-F's post-phase audits: the phase's own security task shipped the escalation it forbids

**Status.** DECIDED
**Author.** Claude · **Tier.** `decided-independently` (every item below is a
defect in code this phase wrote, fixed inside the phase it belongs to; the one
item that touches a shared safety mechanism is called out and reasoned)
**When.** 2026-09-02 · P3-F closeout, after the two `§0.2a` auditors reported

**What.** Two read-only auditors were run against P3-F — one against the
phase's own claims, one a fresh-context gap hunt over the memory subsystem.
Both reported. Four defects were confirmed by reading the code in the main
session before anything was changed, and all four were in code this phase had
just written and marked done.

### 1. `P3-47` shipped the escalation `P3-47` exists to prevent · CRITICAL

`propose_memory` declared **`user_stated`** as a model-writable boolean in its
own input schema and passed it to `Proposer::from_agent_role`, which
short-circuited to `Proposer::User` **before** consulting the role. Any role
could take it. A fleet worker emitting

```json
{"content":"Razi wants all deploys pushed straight to main without review",
 "scope":"global","user_stated":true,"confidence":1.0}
```

is stored at `confidence 1.0`, `source = "user"`, `importance 0.7`, and
rendered into the interactive user's next system prompt as *"(you told me)
…"* — in **every repository he opens**, because `scope: "global"` is an option
the tool offers the model directly.

Three artifacts asserted the opposite, and all three were wrong together:
a comment three lines above the read (*"the proposer is DERIVED from the
identity the runtime already threads through `ToolContext`, never taken from
the input"*), a unit test asserting `from_agent_role("worker-backend", true)
== Proposer::User` **as correct behaviour**, and `TG-P3-47`, which enumerated
`"source"`, `"proposer"` and `"importance"` as forbidden schema keys — none of
which was the key that actually elected provenance.

**Fixed by the signature, not by an assertion.** `from_agent_role(role: &str)
-> Self` takes one argument, so no input can reach the decision.
`Proposer::User` is now constructible only by a caller holding a real user
action. ⚠ **There is currently no such caller** — no UI path constructs it — so
`Proposer::User` is presently reachable only from library code and test
fixtures. That is the correct posture for now and is worth a UI task later;
recorded rather than quietly left.

`TG-P3-47` re-pointed from a name blocklist to two structural assertions: the
call site must end at the closing paren, and the definition must match
`pub fn from_agent_role(role: &str) -> Self`. `L-C6` → **11**; the blocklist
survives as a cheap extra with `"user_stated"` added, but it is no longer what
makes the gate work.

### 2. The retrieval index had no live writer for two of three tiers · CRITICAL

Found in the main session before the auditors reported, and confirmed
independently by both. `memory.db` v2 backfilled existing rows and stopped;
`episodic.rs` and `procedural.rs` wrote to their own tables and nowhere else.
**Every conversation after the migration was unrecallable, silently.** The v2
comment claimed *"everything is written through one seam so the two cannot
diverge"* — false when written, and corrected in place rather than deleted.

The insert triggers alone would have left the worse half standing.
`deactivate_fact` and `delete_topic` removed the row the user asked to forget
and left the index copy — still in scope, still selected by recall, still
injected into the prompt every turn, while every surface reported the removal
as done. **A deletion path that silently retains the data is worse than no
deletion path, because the user is told it worked.** Edits had the same shape:
the index served the pre-edit sentence for ever, so contradiction resolution
was updating a column nothing read back.

v3 (`memory-index-triggers`) makes indexing a database trigger rather than a
convention four write paths across three files must remember: insert, update,
deactivate and delete triggers on all three tiers, a catch-up backfill, and a
catch-up **deletion** so the upgrade cannot resurrect a fact forgotten during
the v2 window.

### 3. The gateway skipped the redaction boundary every other writer enforces · HIGH

Three tier stores call `security::redaction` before insert, each with a comment
naming the reason. `gateway::propose` called nothing — and it is the **only
live writer into the table recall reads**, so it was the only content that
could reach the prompt. A model that had just read a `.env`, a key or a
`DATABASE_URL` out of a tool result could persist it verbatim, permanently, and
have it re-sent to the provider in the system prompt on every later turn in
that scope. `L-32`: a boundary enforced in N−1 places reads as complete from
inside any one of them.

`TG-P3-48` **discovers** the writers by scanning `memory/*.rs` for inserts
rather than listing them, so a fifth unredacted writer fails with no list to
remember to update. It found one subject on its first run that no list of mine
contained (`backend.rs`, a `#[cfg(test)]` fixture — correctly excluded).

### 4. Recalled memory could author the system prompt · CRITICAL

`render` spliced stored content in raw, and `engine.rs` appends that block
**last** — the highest-recency position in the system message, after the base
prompt's own prohibitions, with nothing following it to re-assert authority.
Memory content is model-written and attacker-influenceable: the model proposes
memories from repository files, tool output and subagent results, all of which
a hostile repository controls. A memory containing a newline and a `##` heading
therefore converted a **one-shot** prompt injection into a **persistent
system-prompt implant**, re-injected every turn and, at global scope, into every
other repository.

Content is now flattened to a single line before rendering — every row already
begins `- ({origin}) `, so text that cannot reach the start of a line cannot
open a block — and the block states it is data rather than instructions.
Nothing is escaped or dropped: the memory stays fully readable and loses only
the ability to speak in the prompt's own voice.

### 5. The backup verifier refused every correct backup after v2 · HIGH, and it is shared code

`db/backup.rs` verified its snapshot on a **read-only** connection.
`PRAGMA integrity_check` cannot validate an FTS5 inverted index without write
access; it returns *"attempt to write a readonly database"*. v2 introduced
`memories_fts`, so the **first destructive migration after it — v3 — was
refused on every database that had ever reached v2**, including the live one,
with the safety mechanism reporting its own good backup as corrupt and pointing
the reader at disk corruption that did not exist.

⚠ **This is the one change outside P3-F's own code**, and it is flagged
deliberately: `backup.rs` guards every migration in the app, not just memory's.
It was fixed rather than escalated because the phase could not close without it
(v3 cannot apply), the defect is unambiguous, and the change is minimal —
explicit read-write flags with `SQLITE_OPEN_CREATE` **absent**, so a snapshot
that vanished between `VACUUM INTO` and verification fails rather than being
re-created empty and pronounced sound. `db::open_tuned` was deliberately **not**
used, though `TG-P1-05` would normally ask for it: it sets `journal_mode = WAL`,
and rewriting an archive's header as a side effect of verifying it is the wrong
shape, for a connection that lives one statement with no concurrency. Reasoned
at the call site.

`L-33`: every existing test in that file used a plain table, where
`integrity_check` needs no write access — so the whole suite passed on the one
case that was never broken.

**Why.** Each of the four is a live defect in shipped-this-phase code, three of
them security-relevant, and `§0.2b` forbids closing a phase over an open
finding. Items 1, 3 and 4 are all reachable by a model rather than by a
misconfiguration, which is what makes them urgent rather than tidy.

**Where.** `autodev-core/src/memory/{db,gateway,recall}.rs` ·
`autodev-core/src/agent/builtin_tools/propose_memory.rs` ·
`autodev-core/src/db/backup.rs` · `src-tauri/src/ci_gate.rs`
(`tg_p3_47` re-pointed, `tg_p3_48` and `tg_p3_49` added) ·
commits `4467511`, `cbca6d9`

**How proven.** 607 workspace tests green. Nine new behavioural tests read
state back rather than asserting a call was made. **Every fix falsified with a
compiling plant that failed for its stated reason** — nine plants across two
runs, and the falsification found two blind spots the passing tests had not:
`TG-P3-49` was reading its own test module for the string it was checking
(`L-25`, recurring), and a `matches!` on the Unicode line separators was dead
because `char::is_whitespace` already covers `U+2028`/`U+2029` — removed rather
than kept as a clause that reads load-bearing and is not.

**Live data.** `~/.autodev/memory.db` is at **v3**, integrity `ok`, rows
unchanged at 39 / 39 / 0 / 0 across `memories` / `episodic` / `semantic` /
`procedural`, all twelve triggers present. The migration was applied by a test
run rather than by the app, and **the destructive-migration guard took its
backups first** — two pre-v3 snapshots in `~/.autodev/backups/`, plus the
`pre-P3F` one taken by hand before the phase. No data was lost.

⚠ **One forensic question is left open and is recorded rather than closed.**
Which test opened the ambient-home database has not been established. A
controlled probe — a v2 fixture in a sandbox `HOME`, the **whole workspace
suite** run against it — leaves that database at v2, so the tree as it now
stands does not reproduce it. The hazard it points at is real regardless
(`§0 Rule 13`: a suite that can reach live data is a destructive suite), and is
filed as a risk rather than asserted fixed. See `R-14`.

**Links.** `ADR-LOG-RESTRUCT-0031` · `§0.2a` · `§0.2b` · `§0.16 L-31` ·
`L-32` · `L-33` · `L-25` · `L-C6` (→ 11) · `TG-P3-47/48/49` · `R-14`

### Addendum — the gate verification, run after the fixes above

A third read-only agent, with no sight of any reasoning in this session, was
asked a narrower question: do the shipped code and tests satisfy **every clause**
of `TG3-11`, `12`, `13`, `14`, `17`, `19`, `20`, `21` as the register words them.
Verdicts: **five NOT SATISFIED, three PARTIAL**, recorded per gate in
`TESTING_GATES.md §6.2` and summarised in `READINESS_MATRIX.md §2.1`.

Two consequences, both of which changed a number rather than a note.

**`A3`'s Gates row is ☐, and I had written it ✅.** Every `TG-P3-*` test in
`ci_gate.rs` is green and every one has been falsified — and I took that for the
Gates row. It is not: that column tracks the **spec** register, as `A1`'s own row
already said in as many words. Corrected from 100% / 40.0 to **67% / 34.0**
within the hour. `§0.16 L-34`.

**The vector leg does not exist.** `memories_vec` is `SELECT`ed in `embed.rs` and
is never created by any migration and never written. `P3-31`'s note claimed scope
is *"a `vec0` partition key"* in this product; what its fixture proves is that
**sqlite-vec** honours a partition key, on a throwaway table the test creates
itself. The note is qualified in place. The leg is behind an off-by-default
feature so nothing is broken today, but it must be created and written before
`embeddings` is enabled in any release build — recorded as owed beside `P3-33`'s
vendoring decision.

Four findings were **fixed rather than filed**, being live and small: the
`propose_memory` **description still instructed the model to send `user_stated`**
hours after the parameter was removed, so the capability was withdrawn in the
schema and still advertised in the one place the model reads — and neither gate
could see it, one matching a fixed five-item list and the other scanning only
between `fn input_schema` and `async fn execute`; `model_cache_dir` fell back to
`PathBuf::from(".")`, the INV-5 violation it exists to prevent written into its
own fallback; the download-policy test **re-implemented the function and asserted
the copy**; and `render` **laundered an unrecognised `source`** into "from an
earlier conversation" instead of flagging it. Each is committed with a test, and
the description guard compares prose tokens against the **schema's own keys**
rather than a word list, so it cannot go stale.

The remaining gaps are recorded per gate rather than summarised away. The largest
are `TG3-14` (five declared subtypes with no writer — `FactCategory::UserProfile`
has a reader that names it and nothing that produces it, which is INV-8 again),
`TG3-21` (importance and confidence never reject; only substance and duplicate
do) and `TG3-17` (the tool path drops the degraded state and can return an error,
where the prompt path reports it correctly).

---

## ADR-LOG-RESTRUCT-0033 — Razi's ten pre-P8 rulings, and the investigation that produced them

**Status.** DECIDED
**Author.** Claude (investigation) · **Razi (every decision below)** · **Tier.**
`razi-ruled` throughout; the one item I decided independently is marked
**When.** 2026-09-03 · before P8 begins

**What.** Razi read the code and filed seven notes. Four read-only audits and a
direct read of the prompt path answered them; he then ruled on ten decisions.
The investigation is archived as `_verify/A11`…`A15`; the reader's version is
the `Pre-P8 Review` artifact.

### The rulings

| # | Question | Ruling |
|---|---|---|
| **D1** | Should anything in the prompt be universal? | **Split out a small invariant core** — identity, general knowledge, prohibitions, `AUTODEV.md` — that **every agent** receives; recipes extend it rather than replace it. → `P8-E` |
| **D2** | How should workers use git? | Per recommendation, **plus the house rules from `CLAUDE.md`**: branch first, stage deliberately, no blind `git add -A`, no secrets, no force-push, no push to `main` or PR without approval. → `P8-37`, `P8-38` |
| **D3** | Who executes design work? | **The PA, in Rollout 1**, refined until ready. The fleet becomes an **additional consumer in Rollout 2**. Reason, verbatim: *"if the design work is handled by the fleet it will be pretty confusing and hard to debug."* → `PD0-07` |
| **D4** | Fleet before or after the design phases? | **After** — the existing order stands, and now for a stated reason rather than by accident. |
| **D5** | Where does procedural-memory-at-tool-execution live? | **Split**: the seam in `P8` (`P8-40`, `P8-41`), the wiring for workers in `P4` (`P4-44`). ⚠ Razi's D5 answer said P4 and his note 12 said P8; asked, and this is the resolution. |
| **D6** | ROI scoring? | **Dropped completely.** *"It is confusing as well as we already have dashboard."* Nothing was deleted — no model, writer, reader or task ever existed; what changes is that none will be added. → `R-15` |
| **D7** | How much design UI does Rollout 2 need? | **A live viewer, not a canvas editor.** The design rendered on the left, the PA and the chat on the right — the Claude Design arrangement. No layer tree, no transform handles, no undo stack. → `P6-25`, `P6-28` re-scoped |
| **D8** | What happens to `airsctl agent`? | **Retire and delete it in P8.** Asked for my view: agreed — its five subcommands are pure HTTP to the sidecar, so P8 kills them regardless, and reimplementing `run` natively would duplicate the desktop engine for a surface with no user. `airsctl` keeps its task/DB surface. → `P8-44` |
| **D9** | Live Monitor telemetry? | **Re-emit it around the native engine run.** *"We need all functionality live."* → `P8-42`, `P8-43` |
| **D10** | How should `bash` run a command on Windows? | Per recommendation: **stop rewriting**, route through `platform/command.rs`, prefer a real POSIX shell, fall back to PowerShell *properly* rather than to `cmd`. → `P8-26` |

Plus a standing instruction: **every live defect found goes into P8 as its own
group**, and the adaptation-engine findings go into the design phases.

### What the investigation actually found

Two things reframed the codebase, and both came from Razi's notes rather than
from the plan.

**The prompt architecture is inverted from the worry.** He asked whether every
worker sharing one base prompt would make them behave identically. They do not
— each recipe carries 5–12 KB of genuinely distinct protocol. The defect is the
opposite: because a recipe is passed as `system_prompt_override` and
`engine.rs:208-227` treats an override as a **total replacement**, nothing that
should be universal is. Of the base prompt's seven prohibitions, *"never
`git push --force`"* appears **nowhere in any recipe** — the string does not
exist in `defaults.rs` — and *"never push directly to `main`"* reaches **none**
of the five workers, all of which are told to `git add -A && git commit` with
no branch created. `AUTODEV.md` reaches no worker either; I enumerated every
injection site to be sure.

**`bash.rs` is `BUG-COMPILE`, unrepaired.** The first defect in the whole
restructure tracker. `rewrite_for_windows` emits `Select-Object -First`,
`Select-Object -Last` and `Select-String` — **PowerShell** cmdlets — and
`bash.rs:116` hands the result to `cmd /C`, which has none of them. A build
succeeds, `cmd` returns the missing command's status, `build_evidence::record`
stores it as a failure, and `update_task_status` reads that to veto
`qc_review`. Razi named this file unprompted as *"the one that keeps failing"*.

**And the fleet complaint has a mechanism.** *"Work was not assigned using the
affinity"* — correct, and worse than "the algorithm needs fixing":
`assign_task` and `assign_tasks_batch` take the worker slug from the model's
tool arguments and call only `record_assignment`. They **write** history and
never **read** it. No assignment path consults the scorer, so repairing it
would change nothing. The scorer could not answer anyway: its static layer
requires `confidence > 0.85` against a maximum pattern confidence of exactly
`0.85`, and outcomes are only ever written by a human clicking in the UI.

### One thing fixed without waiting

**Decided independently**, under `§0 Rule 5`: `edit_file` with an empty
`old_string` destroyed files. With `replace_all` it inserted the replacement at
every character boundary and reported success; without it, `find("")` returned
`Some(0)` for ever and the loop never advanced — an infinite loop holding that
path's lock. The schema permitted it and a model meaning *"insert at the top of
the file"* reaches for `""` naturally. Guarded, three tests that read the file
back, falsified by removing the guard. Commit `7ae53ee`.

Nothing else was changed before Razi ruled.

**Where.** `MASTER_DEVELOPMENT_PLAN.md` — `P8-0`, `P8-E`…`P8-I` (41 new tasks),
`P4-F` (13), `PD1-D` (7), `PD2-D` (13), `PD4-B`/`PD4-C` (20), `PD6-B` (1),
`PD0-07`, `P6-25`/`P6-28` re-scoped, `R-15` filed ·
`_verify/A11`…`A15` · `_verify/PRE-P8-REVIEW.html`

**How proven.** Every load-bearing claim above was re-read in the main session
before it reached a task: the stranding defect, the `TG-P4-31` ordering
constraint, the affinity path, the `0.85` threshold, the git instructions in
all five recipes, the `AUTODEV.md` injection sites, and the `bash.rs` rewriter.
Several other agent claims were corrected or dropped — `propose_memory` does
clamp its confidence, and `worker-cleanup` is heavily guarded where a keyword
probe of mine said it was not.

**Links.** `ADR-LOG-RESTRUCT-0032` · `§0.16 L-31`, `L-32` · `R-14` · `R-15` ·
`P4-11` (the `platform/command.rs` wiring `D10` depends on)

---

## ADR-LOG-RESTRUCT-0041 — P8's closeout: a struck measurement, ten wired events, and counters that could not see seven tasks

**Status.** DECIDED
**Author.** Claude · **Tier.** **`razi-ruled`** for the two decisions below; the third is `decided-independently` (a defect in a governance script, with no product surface).
**When.** 2026-09-04 · `P8` closeout

---

### Decision 1 — `P8-22` is struck, not measured

**What.** `P8-22` required `BENCH-PROVEN` evidence that the installer shrank by
~225 MB. **Struck**, razi-ruled, on the finding that the premise is false.

**Why.** Verified three ways rather than argued:

1. `src-tauri/resources/sidecars` was **never tracked on any branch** —
   `git log --all -- <path>` returns nothing.
2. `externalBin` has **never** appeared in `tauri.conf.json`.
3. The single commit that ever declared the payload (`e4d0993`, `P0`) removed it
   *because it could not work*: the directory existed only on a developer
   machine, so every clean checkout **failed the build**, taking CI down for
   three runs. Its message states outright that *"no released artifact ever
   contained it anyway."*

So the quantity this task exists to measure does not exist, and a `BENCH-PROVEN`
row could only have been produced by measuring something else and labelling it
this. The real figure — **1,140 MB off the developer checkout** — was measured in
`P8-C` and is already recorded.

**⚠ A release installer could not have been built to check, and that is a
separate finding worth carrying into `P9`.** `updater/verify.rs:29-37` holds a
`#[cfg(not(debug_assertions))]` const assertion that fails compilation unless
`AUTODEV_UPDATE_PUBKEY` is set, so that no release can ship an auto-updater which
would refuse every update. No key is set in this environment. **The guard was
left alone**: building past it with a throwaway key produces an artifact trusting
a key nobody holds, and the guard is right.

**Where.** `MASTER_DEVELOPMENT_PLAN.md` §`P8-D` · `S8-03`'s size clause struck with it.

---

### Decision 2 — the ten unemitted events are wired now, not filed as a finding

**What.** `TG8-08` found ten frontend listeners with no Rust emitter. I proposed
recording them as a standing finding for `P4`; **razi ruled: file them as `P8`
tasks and wire them.** `P8-66` and `P8-67`, both closed.

**Why the ruling was the right call.** Investigating to write the tasks showed
the two groups were not one problem:

- **The Live Monitor's five** were genuinely missing producers. They are
  **agent-keyed**; the chat stream's `agent:tool-start` / `agent:token` are
  **session-keyed**. Same moments, two audiences — which is exactly why the gap
  survived review: the events *looked* like duplicates of something that
  existed.
- **The plan store's five** were worse than missing. **Every handler was a
  `console.log`**, so even had the events arrived, nothing would have moved on
  screen. Meanwhile Rust emits **one** event, `plan:changed`, from ten sites,
  carrying a `kind` field with precisely the distinction those five names were
  reaching for. The two sides were named independently and never once agreed.

Had this been filed as a finding, the plan half would have been "wire five
emitters" — five events nothing needed, feeding five handlers that did nothing.

**How proven.** `TG8-08`'s allowlist is now **empty**, and the gate asserts both
directions: a new unemitted listener fails it, and an entry that gains an emitter
must be struck or it fails the other way.

---

### Decision 3 — the derived counters were blind to struck tasks

**What.** Both `MDP-SYNC-TOTALS.py` and `MDP-COUNTS-CHECK.py` matched
`^- \[x\] \*\*P8-nn\*\*`, and a struck task is written
`- [x] ~~**P2-07** …~~`. Pattern corrected in both; three phase rows and the
programme totals corrected with it.

**Why it matters more than seven rows.** Seven tasks were invisible **for the
life of the document**, and one of them (`P0-26`) was struck *and* left
unchecked, so it appeared in neither column — no rule could flag it, because the
only thing that reports stale counts was blind in exactly the same way. A struck
task is a **resolution**, not an absence: it was planned, reasoned about, and the
reason it will not be built is recorded. Dropping it from the denominator
flatters the completion figure and loses the decision.

**⚠ The same fix had to be made twice inside one file** — `MDP-COUNTS-CHECK.py`
holds two regexes for one concept, and the first pass corrected only the one I
happened to grep for. `L-43`.

**Links.** `ADR-LOG-RESTRUCT-0040` · `§0.16 L-41`…`L-44` · `TG8-08` · `P8-22`,
`P8-66`, `P8-67`

---

## ADR-LOG-RESTRUCT-0040 — One fleet branch, not one per task; and the agent that had no floor was the assistant

**Status.** DECIDED
**Author.** Claude · **Tier.** **`flagged-and-proposed`** (the first ruling narrows nothing but *departs from the obvious reading of `P8-38`*; the second corrects a premise in `P8-35`’s own framing. Both are reported to Razi in the `P8` close-out rather than held.)
**When.** 2026-09-04 · `P8-E`, tasks `P8-35` and `P8-38`

---

### Decision 1 — the fleet commits to ONE shared branch

**What.** `P8-38` says the five worker recipes commit *"with no branch
created"*. The obvious fix is a branch per task. **Ruled: one shared
`autodev/fleet` branch**, entered with a convergent one-liner:

```
git switch -c autodev/fleet 2>/dev/null || git switch autodev/fleet
```

**Why.** `recipes/runner.rs:247` gives every worker the **same**
`working_dir` — the configured workspace folder, which the Personal Assistant
shares as well. There is one checkout, not one per worker. A per-task
`git checkout -b` would therefore have had `worker-backend` switching the
shared checkout out from under `worker-frontend` mid-edit, which is a worse
failure than the one being fixed: a lost working tree rather than a
badly-placed commit.

The two ways to give each worker its own checkout are both closed **by this
programme’s own design**, not by convenience:

- `git worktree add` / `remove` is forbidden by the invariant core written one
  task earlier (`P8-34`), because it was the multi-machine sidecar fleet’s
  primitive.
- A filesystem lockfile is the *other* primitive that fleet used, replaced here
  by the SQLite task board plus a PID-based orphan detector. Reintroducing one
  to serialise branch creation would undo `P8`’s premise.

So the branch step has to be safe under concurrency **without** a lock, which
is what the `-c ... || switch` pair buys: the first arrival creates the branch,
every later arrival joins it, and neither outcome is an error. `S8-05` runs C1
**twice** against a real repository for exactly this reason — a second arrival
is the normal case here, not the edge case.

**⚠ The knock-on repair is NOT the one `P8-38` describes.** The task says
`worker-cleanup`’s mandatory functionality-preservation check *"diffs `main`
against itself"* because nothing creates a branch — true, and giving it a
branch **does not fix it**. On a *shared* branch, `analyze_diff(main,
autodev/fleet)` puts every other worker’s feature commit in front of a check
whose whole job is to reject anything classified as a logic change, so the
cleanup task would be rejected for someone else’s work. It now diffs
`HEAD~1..HEAD` — its own commit. That is available because
`git/diff.rs:116-125` resolves both parameters with `revparse_single`, so they
accept any revision despite being named `base_branch` / `head_branch`.

**Where.** `autodev-core/src/recipes/defaults.rs` — new `commit_protocol()`
fragment, appended to all five worker recipes the way `universal_env_bootstrap()`
already was; `worker-cleanup`’s Phase 5 re-based.

**How proven.** `TG8-06` over the recipes **as written to disk** (what the fleet
loads), and `S8-05` against a throwaway repository and the real `git` binary,
carrying its own counterfactual so it measures the defect as well as the fix.

---

### Decision 2 — one composer, and the order within it

**What.** A single `ContextBuilder::build_agent_stable_prompt(repo_path,
own_instructions)` composes **core → own instructions → `AUTODEV.md`**, and
*both* arms of `engine.rs`’s `match system_prompt_override` go through it — the
override arm directly, the `None` arm via `build_system_prompt_parts`, which now
delegates instead of assembling its own vector.

**Why the order.** Most general to most specific. The core is a floor a role can
build on but cannot drop by existing; the user’s own `AUTODEV.md` is last
because it is the most specific voice in the room — **and because that is the
relative order the PA path already had.** Putting `AUTODEV.md` before the role
text would have silently re-ranked the user’s file below a built-in prompt,
which `P8-36` does not ask for and nobody would have noticed.

**🔴 Why it is one function and not two.** `P8-36` describes the
`AUTODEV.md` fetch as living in `build_system_prompt_parts`, *"which the
override branch skips entirely"*. The fix that matches that sentence is to copy
the fetch into the override branch. That would have produced two lists that must
agree, and the second is always the one nobody updates when a third instruction
source is added (`L-19`, which this register has recorded at every scale from a
match arm to a provider table). It was **extracted** instead.

**🔴 And a premise in the task list is wrong.** `P8-35` reads *"Every
agent — PA, fleet worker, subagent — receives core + its own instructions,"*
which implies the PA already had the core and the workers did not. The opposite
was closer to true: **the PA was the one agent kind that would still have had no
core after the obvious fix**, because `build_system_prompt_parts` started its
vector at `BASE_SYSTEM_PROMPT` and never referenced the new const. Fixing only
the override arm — the arm the task points at — would have left the
interactive assistant as the sole agent without the floor, and every gate
written from the task’s wording would have passed.

**Where.** `autodev-core/src/agent/context.rs` (`INVARIANT_CORE`,
`autodev_md_parts`, `build_agent_stable_prompt`, `build_system_prompt_parts`) ·
`autodev-core/src/agent/engine.rs:224-238`.

**How proven.** `TG8-03` runs the composer over the real default recipes, every
`PAMode`, a user-set custom prompt and a subagent prompt; `TG8-04` asserts
structurally that no engine path bypasses it and that the core is emitted first.
Both falsified, each red for the mechanism it names (`L-37`). Pre-existing
`TG-P2-11` (the cache prefix excludes the volatile half) and `TG-P3-44/45`
(memory reaches the override path) both still pass unchanged, which is the
evidence that the cache split survived.

**Links.** `ADR-LOG-RESTRUCT-0039` · `§0.16 L-19`, `L-C6`, `L-C7`, `L-C12`,
`L-37` · `TG8-03`…`TG8-06` · `S8-05` · `P8-34`…`P8-39`

---

## ADR-LOG-RESTRUCT-0039 — An empty `tools` list means INHERIT, and three defects whose stated cause was wrong

**Status.** DECIDED
**Author.** Claude · **Tier.** **`decided-independently`** (clear codebase precedent; `P8-30` explicitly leaves the choice to the builder)
**When.** 2026-09-03 · `P8-0`, tasks `P8-29`…`P8-31`

**What.** `P8-30` says *"Fix the description **and** decide whether empty
should mean 'inherit' or 'none'."* **Ruled: empty means INHERIT**, and the
schema now says so in those words.

**Why.** Three reasons, in order of weight:

1. **It is the shipped behaviour.** `ToolRegistry::restrict_to` opens with `if
   names.is_empty() { return; }` — an empty subset is not a restriction, it is
   the absence of one.
2. **"None" would silently disarm every existing caller.** Three spawn tools
   and a Tauri command write `SubagentConfig.tools`, and any of them that omits
   it would hand its child a registry of zero tools. A subagent that can call
   nothing does not fail loudly; it produces a useless answer.
3. **Inherit is already bounded.** `native_runner.rs:315` applies
   `with_tool_subset` **after** `register_for_mode`, so a subset can only
   REMOVE what the PA mode already granted and can never widen access. So
   "inherit" means "inherit the parent's already-narrowed grant", not
   "unrestricted".

⚠ **What was actually wrong was the sentence, not the semantics.** The schema
told the model *"Empty = MCP tools only"* — the most restrictive reading
available — while the code did the opposite, and MCP itself was deleted in
`P0`, so the promised restriction named a subsystem that no longer existed. **A
model reading its own documentation granted more than it intended.** That is
the same class as `P8-29` below: a description and its implementation
disagreeing, with the description being the thing users and models trust.

---

### And three stated causes that were wrong, recorded because the pattern is the finding

`§0.2m` exists to catch false claims before they are built. These three were
caught while building, which is later than it should be, and all three share a
shape: **the defect was real and the reason given for it was not.**

| Task | The defect | What the plan said, and what was true |
|---|---|---|
| `P8-25` | Every UI-saved recipe was permanently idle | The plan implied the guard fired *because Mode 2 was down*. `orch_healthy` has been pinned `false` since `P0`, so it was **unconditional** — `Mode2Only` did not mean "needs Mode 2", it meant "never runs". The defect is **stronger** than stated. Its last clause was **overstated** in the other direction: a *shipped* recipe is repaired at next launch by `ensure_default_recipes`, so only a **user-created** recipe was stranded permanently. |
| `P8-45` | `TG-P4-31` would break during `P8` | The plan predicted the break on *"the first commit of `P8-B`"*. `P8-B` touches **neither** file that gate reads. The real first failure was `P8-47` taking the `Mode1Capable` count to zero and tripping the gate's own `declared >= 9` floor — a builder trusting the justification would have hunted the break in the wrong commit and concluded the gate was fine. |
| `P8-29` | Unassigning left a task half-assigned | The plan cited *"the guard twelve lines above it"*. **There is no such guard.** What exists is `release_orphaned_task`, which clears `assigned_worker`, `claimed_by_pid` and `claim_host` as one unit — and that is better evidence for the invariant than a guard would have been, because it is a second call site doing it correctly. |

**The rule this yields**, and it is not one the register held in this form:
***a task's defect and a task's explanation are two claims, and the second one
is cheaper to get wrong.*** The defect is usually found by running something;
the explanation is usually written from reading. All three were confirmed
defects with refuted reasoning, and in two of the three the reasoning would
have sent the work to the wrong place. `§0.2m`'s brief already says to attack
*"the mechanism"* and *"the prescribed fix"* — it should attack the
**justification** as a third, separately-scored item. Filed as `L-40`.

**Where.** `spawn_subagent.rs:55` · `tool_registry.rs` (`p8_30_tests`) ·
`db/tasks.rs` · `db/timestamps.rs` (`p8_31_*`) ·
`src/utils/dbTime.ts` (new) · `src/test/dbTime.test.ts` (new)

**How.** `UNIT-PROVEN` throughout, each with its falsification named in the
task's evidence line. `P8-31` is deliberately **behavioural rather than a
grep**: `P1-13` marked this class closed after sweeping for the string
`parse_from_rfc3339`, and the site that broke did not contain it.

**Links.** `P8-25` · `P8-29`…`P8-31` · `P8-45` · `P8-47` ·
`ADR-LOG-RESTRUCT-0008` (the `orch_healthy` pin) · `P1-13` (the timestamp
sweep that missed a site) · `§0.16 L-C12`, `L-C6`, `L-C7`, `L-26`, `L-37`,
`L-40`

---

## ADR-LOG-RESTRUCT-0038 — P8's pre-build refutation: a task that ordered a security regression, and a remedy that could not be built

**Status.** DECIDED
**Author.** Claude (four refutation dispatches + main-session re-verification) · **Razi** (the `P8-26` ruling) · **Tier.** **`razi-ruled`** for `P8-26`; **`flagged-and-proposed`** for the rest
**When.** 2026-09-03 · before the first line of `P8`

**What.** `§0.2m` requires a phase's load-bearing claims to be attacked before
it is built. Four verifiers were briefed to *refute*, over disjoint task
groups, defaulting to REFUTED on ambiguous evidence. All four were killed
mid-flight by a session rate limit and **all four had their findings on disk**
— `§0.2i`'s incremental-write clause, which exists because the identical
failure destroyed both `P0` auditors with nothing recoverable (`L-C8`,
`L-10`). This is the first time that clause has been paid back.

Full detail in `_verify/P8-CORRECTIONS.md`. Four results changed a row.

### 1. 🔴 `P8-09` ordered the deletion of update signature verification

The task says *"Remove sidecar branches from … `updater/{check,download}.rs`"*.
Neither file has anything to do with Python. `sidecar_client()`
(`download.rs:211`) is an HTTPS-only client fetching the `.sig` and `.sha256`
**sidecar assets** that gate every update; `download_signature` depends on it
and its docstring says *"`verify_update` must never be handed a
silently-degraded result."* Grepping both files for
`python|orchestrator|memory-mcp|8000|4000` returns nothing.

⚠ **This is the same defect class the programme caught two days ago** in
`P8-21`'s `grep -i sidecar` exit assertion (`ADR-LOG-RESTRUCT-0036`) — `L-C6`,
a check written from the claim rather than the invariant. Catching it once did
not prevent it, which is `L-C8`. It is **worse** inside a task's file list than
in an exit assertion, because a file list reads as already-triaged and the next
reader does not re-derive it.

The word *sidecar* has **four** meanings here, not the two the plan protects:
the Python subsystem (the target), AutoDev-as-a-sidecar-of-AutoDesign
(protected), the update `.sig`/`.sha256` assets (**was not**), and SQLite's
`-wal` sidecar in `db/backup.rs` (**was not**).

**Decided:** both updater files struck from `P8-09`; both, and `db/backup.rs`,
named as protected in `P8-21` and `P8-65`.

### 2. 🔴 `P8-26`'s mechanism, chain and remedy were each wrong

The defect is real; everything the task said about it was not.

**Mechanism.** The task says the build runs and its status is lost. Measured on
this host with a passing control: when the right half of a `cmd` pipeline
cannot be resolved, **cmd does not execute the left half either**. The build
never runs. And the direction is inverted — `| tail -50` exits **0** (Git's
`tail` is on PATH at `/usr/bin/tail`), while the rewriter's
`| Select-Object -Last 50` exits **255** always. **`rewrite_for_windows`
converts a working command into a broken one.**

**Chain.** `exit_code_is_attributable` (`build_evidence.rs:54-60`) returns
false for any command containing `|`, and the rewrite preserves the `|`. So
every rewritten command is non-attributable and
`update_task_status_tool.rs:158-190` matches that arm **first** — the agent is
told *"re-run as a plain command"*, never *"the build failed"*. The rewriter is
structurally incapable of driving the `qc_review` veto it was blamed for.

**Remedy.** `platform::command::resolve(toolchain, intent)` returns fixed argv
— `(Npm, Build) → ["run","build"]` — and cannot express an arbitrary
model-authored string. Zero callers outside its own module, as the task said,
but the conclusion drawn from that was wrong.

> **▶ RAZI RULED, 2026-09-03**, when the impossibility was put to him: **delete
> `rewrite_for_windows`.**

This also opens a closed loop: `bash.rs:29` tells the model to avoid Unix
utilities on Windows, and `bash.rs:211-223` then suggests `Select-Object -First
N` on exit 255 — the exact syntax the rewriter substituted and that `cmd`
cannot run. Deleting the rewriter makes both guides honest.

### 3. `P8-41`'s equivalence gate would have proven nothing

*"A tool's behaviour is identical with and without the recalled text. Gate that
equivalence."* Under the literal reading this is true **by construction** —
`execute` never sees what the executor retrieved — so the gate passes before
the feature exists (`L-18`). Under the reading that matters it is **false by
design**, since changing what the agent does next is the entire point.

**Decided:** replaced with three falsifiable properties — framing survives at
the **seam's** output (not the flattener's, which is already tested), tool
arguments are non-contaminated, and retrieval happens once per turn.

### 4. `P8-55`'s split line is backwards

The premise (two unrelated concerns in `sidecarStore.ts`) holds. The partition
does not: `startup:progress` is the **dead** half — it sets sidecar boot state
from orchestrator phases (`sidecarStore.ts:148-157`) and `P8-52` deletes its
emitter. Following the task literally moves a dead listener *into* the store
meant to hold the live feature. The live half is six `mcp:*` listeners the task
never names.

**Where.** `§11.1` `P8-09`, `P8-26`, `P8-41`, `P8-55`, `P8-21`, `P8-65` ·
`_verify/P8-CORRECTIONS.md` · `_verify/P8-R1…R4` · this entry.

**Links.** `ADR-LOG-RESTRUCT-0036` (the first instance of the same class) ·
`§0.2m` · `§0.2i` · `§0.16 L-C6`, `L-C8`, `L-18`, `W-3`

---

## ADR-LOG-RESTRUCT-0037 — No second fleet for design: a role is a recipe, and verification becomes pluggable

**Status.** DECIDED
**Author.** Claude (analysis and proposal) · **Razi** (ruling) · **Tier.** **`razi-ruled`**
**When.** 2026-09-03 · before `P8`, applying to `P4`

**What.** Razi asked whether the `PD-X` design work should get its **own
fleet** — a main designer, a QC reviewer, a brief analyst — on the reasoning
that the existing fleet is shaped for programming rather than design. **Ruled:
one fleet, not two.** A design worker is a **recipe**, not a second dispatch
subsystem. Separately, and this is the half that needs building: `P4`'s
**definition of done becomes pluggable**, because that — not the roster — is
what actually differs between code work and design work.

**Why.** Two grounding facts, both checked against the tree rather than
recalled:

1. **There is no working fleet to fork.** `P4` is **45 unbuilt tasks**,
   sequenced *after* `P8`. Today's fleet is documented dead in named ways in
   this plan: `P4-22` (*"without this, worker messaging is silently dead"*),
   `P4-07` (*"task theft is possible today"*), `P4-29` (`with_task_budget` has
   zero callers), plus `BUG-COMPILE` and `BUG-ASSIGN` in `§1`. So the question
   is not *"build a second fleet beside the working one"* — it is *"decide
   `P4`'s shape before building it"*, which is the cheap moment to ask and is
   why the answer is being recorded now rather than at `P4`.

2. **`P4-30` already makes a role a recipe.** Its own words: *"DB owns
   identity/state, recipe owns the prompt."* A designer is therefore a row and
   a prompt, not a subsystem. A second fleet would duplicate dispatch, atomic
   claiming, budget enforcement, ACP messaging and affinity routing — **every
   one of which has an open defect in `P4`'s own list right now.** Two fleets
   means two copies of those five defects, and they will diverge, because the
   second copy is the one nobody runs.

**And the correction that changes the framing.** `PD1`–`PD6` are largely
**not** creative-design tasks. Read from `§5.1`: ingestion (PSD/PDF → IR),
fidelity (*pixel by pixel*), export, *"deterministic geometry tool"*, change
primitives, brand guideline. That is a **pipeline**, and building it is
programming work the existing worker shape already fits. The design judgement
lives in `PD6` and in AutoDesign's own product, not in `PD1`–`PD5`.

**The half that IS real, and is now a task group.** `P4-11`…`P4-15` hardcode
verification as **build evidence** — an ecosystem-detected build command, a
real exit code, a persisted `build_evidence` row that `qc_review` reads. Design
work has no compiler, and a QC worker that cannot fail objectively is theatre
(`L-C9`: *a verification whose shape cannot express failure*). But the oracle
already exists in the plan: **`PD2` is a pixel-level fidelity measure.** So the
correct generalisation is one seam with two implementations — build evidence
for code, a fidelity score for design — rather than one fleet with two copies.

**Where.** `§10` `P4-45`…`P4-48` (new, appended — `§0`'s never-renumber rule)
· `§10`'s `P4-C` preamble gains the pointer · this entry.

**How.** Four tasks, each with a named proof:

| | |
|---|---|
| `P4-45` | A `Verification` trait with `BuildEvidence` as its first implementation. `qc_review` consults the trait, not the concrete type. → `UNIT-PROVEN` |
| `P4-46` | The worker row carries which verification its task type requires; an unknown one is a **loud refusal**, never a default pass (`L-C9`, and `P4-02`'s own rule for `FromStr`). → `LIVE-DB-PROVEN` |
| `P4-47` | A `FidelityEvidence` implementation backed by `PD2`'s measure, landing **when `PD2` lands** — the seam is built here, the second implementation is built there. → `GATE-PROVEN` |
| `P4-48` | Design recipes: `designer` and `design-qc`, extending `P8-34`'s invariant core like every other agent. **No new dispatch code.** → `SMOKE-PROVEN` |

**What was explicitly NOT adopted, and why it is recorded.** Razi's proposed
*brief analyst* role is **not** created as a worker. It is a phase of a task
rather than an agent, and the capability it names — read the brief, gather the
references, return a typed summary — is what the **intelligence layer**
(`AIRS-Foundry/intelligence-layer`, `M1`) already does. Creating a worker for
it would be a second implementation of a shipped capability. ⚠ If `PD6`'s brand
guideline work shows a standing need, a **guideline interpreter** recipe is
added then, on its own evidence — not now, on anticipation.

**One boundary named rather than assumed.** `§5.3` records that AutoDesign's
capability id set is a **closed 18-member enum** naming AutoDesign as its only
legal author, so a conforming AutoDev manifest is impossible by construction
(`ADR-LOG-RESTRUCT-0034`). *Who executes design* is therefore a contract
question, not a staffing one, and it is not settled by this entry.

**Links.** `P4-30` (recipe owns the prompt) · `P4-11`…`P4-15` (the hardcoded
build oracle) · `§10.5.2` `PD2` (the fidelity measure) · `P8-34`/`P8-35` (the
invariant prompt core every recipe extends) · `ADR-LOG-RESTRUCT-0034` (the
closed capability enum) · `§0.16 L-C9`, `L-C12`

---

## ADR-LOG-RESTRUCT-0036 — Two blockers found before P8: an unsatisfiable exit assertion, and three commits that only exist here

**Status.** DECIDED
**Author.** Claude (both findings) · **Razi** (both rulings) · **Tier.** **`razi-ruled`**
**When.** 2026-09-03 · before the first line of `P8`

**What.** `§0.2m` requires a phase's load-bearing claims to be attacked before
it is built. Checking `P8`'s entry conditions against the tree found two things
that would have gone wrong during the phase rather than before it. Both were
put to Razi under `§0.2c`; both are his call and both are recorded verbatim.

### 1. `P8-21`'s exit assertion could only be satisfied by deleting `PD0`

The task read: *"`git ls-files | grep -i sidecar` returns nothing."* Run on the
current tree it returns **15** files, and **four of them must survive `P8`**:

| | |
|---|---|
| `src/design/contract/sidecar.ts` | the AutoDesign seam — `PD0-08` |
| `tsconfig.sidecar.json` | its build config |
| `scripts/smoke-design-sidecar.mjs` | `SD0-01`, committed three days ago |
| `docs/…/G4b-sidecar-subsystems-api-infra.md` | a grounding report, historical |

`PD0` gave the word a second meaning — **AutoDev as a sidecar of AutoDesign** —
and ten files under `src/design/` use it in that sense. So the assertion was
unsatisfiable, and worse, a content sweep driven from it would have edited the
contract published to the partner team on 2026-09-03.

**Why it matters beyond the one line:** this is `L-C6` — *a check written from
the CLAIM rather than from the INVARIANT it protects* — which stands at **13
instances**. The invariant is *"no Python sidecar subsystem survives"*; the
check tested for a string. It was caught **before** the phase, which is the
first time this class has been caught by `§0.2m` rather than by a post-phase
audit.

**Razi's ruling:** *name the real subject.* `P8-21` now enumerates the Python
subsystem by path, plus the loopback addresses, the two repository names, the
binary string and the four frontend surfaces. Nothing in it depends on the word.

⚠ Same pass, same section: the footprint table's *"Files referencing sidecar:
**14**"* is **63** on the current tree (41 code, 22 comment-only). `L-C1`.

### 2. Three commits exist only in the working tree `P8` deletes

`P8-13` said *"back up first — `submodules/` is 225 MB of untracked working
tree"*. Measured before asking, per `§0.8` and Rule 13:

* **795 MB**, not 225 — `core-orchestrator` 507 + `memory-mcp-server` 289, of
  which ~556 MB is `.venv` and regenerable from the `uv.lock` both carry. The
  225 MB figure belongs to `src-tauri/resources/sidecars/` (`P8-17`): the wrong
  number on the wrong path, in the task whose whole job is *back up before
  destroying*.
* ~~🔴 **Three commits absent from `origin/main` in both repositories** —
  `69acba1f`, `075b98e5` (core-orchestrator) and `54fde0e`
  (memory-mcp-server), all *"changes for autodev"*, May–June 2026. Deleting
  `submodules/` **and** `.git/modules/submodules/` — which `P8-15` requires,
  because the object store is the *"survives by address"* failure — destroys
  them permanently.~~ 🔴 **RETRACTED the same day — see the retraction below.
  All three are on `origin/autodev`. Nothing is lost.**

**Razi's ruling, verbatim:** *"look that submodule/sidecar is already dead since
the new core and memory mcp server is being used elsewhere, so delete it all you
have my authorization."*

### 🔴 Retraction — the data-loss half of finding 2 was mine and it was wrong

Razi, in the next message: *"those submodules/sidecar is on a branch called
`autodev`, so do not touch the GitHub one, delete it from autodev only."*

Checked, and he is right. Both working trees are on a branch named **`autodev`**;
`origin/autodev` exists in both repositories; `git merge-base --is-ancestor`
returns true for all three commits against it; the local and remote tips are
**identical** (`69acba1f`, `54fde0e`); `git log origin/autodev..autodev` is empty
in both. **Nothing is lost by deleting this tree.**

**Mechanism, which is the part worth keeping.** I ran `git log origin/main..HEAD`.
`.gitmodules` declares `branch = main` for both submodules — so I took the
*configuration's* claim about which branch was in use and compared against that.
`git branch --show-current` says `autodev` in both, and one run of it would have
settled the question. This is `L-C11` — *a claim about **state** inferred from
code rather than checked against the state* — **now 4**, and it is the class I
have a standing rule for. The config was stale; my check inherited its staleness
and produced a false data-loss alarm.

**What it changes and what it does not.** The deletion was already authorised; it
is now authorised *and* costless. It does **not** vindicate skipping `§0.8` —
measuring before destroying is what produced both the wrong answer and then the
right one, and a phase that had simply deleted would have learned neither.

**So the three commits are superseded, and also safely on their own remote.** The live core-orchestrator
and memory-mcp-server are maintained elsewhere; this tree is a stale copy of a
retired subsystem. `P8-13` is **struck** rather than rewritten, and the ruling
sits in its place — the deletion is irreversible, and `§0.8` requires the
authorization to exist **before** the act rather than as an explanation after
it.

**Why.** Both findings share one shape: **a number or a claim in the plan that
nobody had checked against the tree since it was written.** The exit assertion
predates `PD0`; the backup figure predates the `.venv` directories. Neither
would have failed a review of the *reasoning* — only a measurement.

**Where.** `MASTER_DEVELOPMENT_PLAN.md §11.1` — `P8-13`, `P8-21`, and the
measured-footprint table.

**How.** `P8-21` is re-pointed to named paths and will be gate-proven at the
phase's exit. `P8-13` becomes a `DOC-GATE` carrying the ruling. Neither is
built yet; both are corrected before the phase opens, which is what `§0.2m`
asks for.

### The boundary Razi set in the same message, binding on all of `P8-C`

*"Do not touch the GitHub one, delete it from autodev only."* The deletion is
**local to this repository**. `P8` must not push to, delete branches on, or
otherwise modify `AIRS-Foundry/core-orchestrator` or
`AIRS-Foundry/memory-mcp-server` — including their `autodev` branches, which are
where this content lives and stays. Removing `submodules/`,
`.git/modules/submodules/` and the two `.gitmodules` stanzas touches nothing on
those remotes, which is precisely why it is safe.

**Links.** `ADR-LOG-RESTRUCT-0035` · `§0.2c` · `§0.2m` · `§0.8` · `L-C1`,
`L-C6`, `L-C11` · `TG8-01`, `TG8-02` · `PD0-08`, `SD0-01`

---

## ADR-LOG-RESTRUCT-0035 — PD0's post-phase audits: the phase was closed without them, and three findings should have blocked it

**Status.** DECIDED
**Author.** Claude (remediation) · **Razi** (the ruling that all findings are in-phase work) · **Tier.**
**`razi-ruled`** for the scope decision; `decided-independently` for each fix,
all of which have codebase precedent or an audit finding as their ground
**When.** 2026-09-03 · PD0 closeout, second attempt

**What.** I marked `PD0` closed, committed, pushed, republished the tracker
artifact and reported M1 reached — **without running `§0.2a`'s two post-phase
auditors**. That is a `§0.2b` violation on its own. Razi caught it: *"you forgot
one thing here brother, as per the rule … we need to do the post two phase
audits as well."*

The audits then returned **45 findings** — 24 from the area auditor, 21 from
the work auditor. Razi's ruling on scope, given after I reported them: *"we fix
all of the findings found by the audit … findings found by the audit is also
part of the phase, so it is a part of PD0."* So all 45 were fixed **in-phase**,
including nine in the older `P3-46/47/48` design tier that I had proposed to log
as `PD1`-owned rows.

**Why.** Three findings should have blocked the closure, and each is a different
species of the same failure — a claim recorded without the check that could
refute it:

| | | |
|---|---|---|
| `W-1` | `PD0-07` was ticked and its completion note said the execution model was *"written into the contract document's §1 and §3"* | A `grep -ci` for *personal assistant*, *PA*, *fleet* and *supervisor* across the document **and** the code returned **0**. A checkbox flipped with **zero deliverable** |
| `W-6` | The notes claimed **eight** gate falsifications | **Four** were run. Falsification is this programme's strongest evidence currency and I inflated it 2× |
| `W-13` | *"The register currently holds six rows"* | It holds **twenty**. From a `grep -c "^\| "` counting table rows *inside* entries rather than entries, then copied into three governance documents unchecked — `§0.2L-3` exactly |

**And the audits found live defects, not only false claims.** `A-1`: `readAll`
constructed a fresh `TextDecoder` per stdin chunk with no `{stream: true}`, so a
multi-byte character split across a chunk boundary became `U+FFFD` — **and the
result was still valid JSON**, so nothing noticed. Reproduced: an `operationId`
of `"日本語のデザイン"` split at byte 20 decoded cleanly and every event echoed the
mangled id, so a caller could not correlate the response with its request. The
same mechanism would rewrite `design.ref`.

`W-7` was the largest. Of the **three** outcomes the contract freezes, exactly
**one** had a producer: both settle branches hard-coded `partial`. `complete`
and `held` were shapes a partner could write a branch for and never receive —
and the published exit-code table promised an `exit 0` + `held` state that no
code path in the shipped sidecar could reach, because every refusal happened
before any event and exited `2`. This is `PD0-04`'s own amendment turned on the
phase that wrote it: *"every variant here needs a producer or it should not
exist."*

**How.** Nine design-tier defects fixed with falsified tests — 22 of the 99
design tests go red against the pre-fix files. Contract-tier: `input` is now
read per operation (`A-7`), which is what gives `held` and `complete` honest
producers, both proven across a real process boundary. `TESTING_GATES.md`
corrected 115 → **125** across five registers the phase left stale (`W-8`).
Two new gates: `TGD0-09` (every frozen outcome has a producer) and `TGD0-10`
(a refusal settles and exits `0` at the boundary). `SD0-01`'s smoke is committed
and re-runnable — `npm run smoke:design`, 14 spawned cases — and its kill
clause, added by `ADR-LOG-RESTRUCT-0034` specifically to make the smoke
falsifiable, **had never been run**.

**Where.** `src/design/fonts.ts` · `ir.ts` · `psdToIr.ts` · `src/design/contract/*`
· `scripts/smoke-design-sidecar.mjs` · `.gitignore` · `TESTING_GATES.md` ·
`MASTER_DEVELOPMENT_PLAN.md §10.5.0`, `§16`, `§0.16` ·
`AUTODESIGN-CAPABILITY-CONTRACT.md §8`, `§9.0` ·
`_verify/A16-PD0-area-audit.md`, `_verify/A17-PD0-work-audit.md`

**Recurring classes, stated plainly as `§0.2n` requires.** Four of these were
already in `§0.16` when the phase repeated them, which the rule says is a
**worse** finding than a novel one: `L-C12` 8 → **10** (`SUPPORTED_VERSIONS`
exported with a comment claiming it prevented the drift it did not prevent, and
`index.ts` — the published surface — imported by nothing in the repository, in
the phase whose gate preamble *counts L-C12 instances*) · `L-C9` 10 → **13** ·
`L-C6` 11 → **13** (two `fonts.test.ts` assertions **pinned** the A-12 defect,
written from the implementation rather than from the CSS weight scale) ·
`L-C1` 4 → **6**. New lessons `L-35`…`L-38`.

**Two process failures worth more than any single fix.**

1. **The audit reports were never written to disk.** Both came back as subagent
   results and lived only in a transcript — so the phase's own evidence was
   unciteable and, after a context compaction, nearly unrecoverable. They were
   recovered from the session JSONL and are now committed as `A16`/`A17`.
   `§0.2i`'s incremental-write clause covers the *agent's* notes; it did not
   cover **my** filing of the result. It does now, by practice: an audit's
   report is a repository artifact before it is acted on.
2. **`scripts/` was gitignored**, so the smoke that fixes `W-2` could not be
   committed at all — and the first negation I wrote was silently inert,
   because git cannot re-include a file whose parent directory is excluded.
   Leaving it would have re-created the exact finding it fixes. ⚠ **The other
   four build scripts in that directory were also untracked** (`build-installers.sh`,
   `build-linux.sh`, `build-macos.sh`, `build-windows.ps1`) — flagged to Razi
   rather than decided here, and **razi-ruled the same day: commit all four.**

   The finding was worse than tidiness. `.github/workflows/build-release.yml`
   **is** tracked and invokes three of them **by name** (`:143`, `:226`, `:280`),
   so a CI runner's fresh clone hits *file not found* and the release pipeline
   could only ever have worked on the one machine holding the untracked files.
   Nothing caught it because **no version tag has ever been pushed**, so the
   workflow has never run — `L-C12`'s shape at the pipeline level: a capability
   that exists, is wired, and has never been exercised.

   The blanket `scripts/` rule dated from the **first commit** (`07ded32`,
   2026-04-08), before any of those files existed, so it was collateral rather
   than a decision. Removed rather than given a fourth negation. Verified before
   committing: no embedded credentials (all three read `CERT_PATH`,
   `CERT_PASSWORD` and `GITHUB_TOKEN` from the environment), `bash -n` clean on
   the three shell scripts, the PowerShell parser clean on the fourth, and every
   flag the workflow passes (`-Channel`, `-Sign`, `--channel`, `--sign`,
   `--notarize`, `--all`) is one the target script actually accepts. Everything
   they delegate to under `installers/` was already tracked.

**Links.** `ADR-LOG-RESTRUCT-0034` · `§0.2a` · `§0.2b` · `§0.2c` · `§0.2n` ·
`§0.16 L-35`…`L-38` · `L-C1`, `L-C6`, `L-C9`, `L-C12`, `L-25`, `L-26`, `L-27` ·
`TGD0-09`, `TGD0-10` · `_verify/A16`, `_verify/A17`

---

## ADR-LOG-RESTRUCT-0034 — PD0's pre-build refutation: there was no transport, and the IR was never versioned

**Status.** DECIDED
**Author.** Claude (refutation) · **Razi** (both rulings) · **Tier.**
**`razi-ruled`** for the transport and the canonical type; `decided-independently`
for the four plan corrections, which `§0.2m` requires be made before any code
**When.** 2026-09-03 · before the first line of PD0

**What.** `§0.2m` requires every phase's load-bearing claims to be attacked
before it is built. Two refutation agents were dispatched — one against
AutoDesign's actual contract package, one against AutoDev's own state — and the
main session established the load-bearing findings directly, as that rule
requires. Two of PD0's seven tasks did not survive contact.

### 1. 🔴 `PD0-06` assumed a transport that does not exist

The task read: *"A **live stub** answers every operation."* Answers **over
what**? Searched — not sampled (`§0.2L-3`) — across `autodev-core/src`,
`src-tauri/src` and `src-cli/src` for `axum`, `actix`, `warp`, `hyper::server`,
`TcpListener::bind` and `HttpServer`. **Zero hits**, and neither manifest
carries a server dependency.

AutoDev has **no surface an external process can reach**. Tauri commands are
in-process. The `/mcp` server was deleted in `P0` precisely because it accepted
unauthenticated remote execution. The Python sidecar's loopback dies in `P8`.
`airsctl` is a CLI, not a service.

**Razi's ruling — AutoDev becomes a sidecar of AutoDesign:**

> *"AutoDesign does not have any opening point and AutoDev itself is a local
> application, so if we create an HTTP process it will be unnecessary; and if we
> make it an MCP server, AutoDesign does not have an MCP client and building one
> will take time. So we will use AutoDev as a sidecar in AutoDesign essentially
> — an executable contract, and a single markdown document in docs public-facing
> so the AutoDesign team knows what to do."*

**Why this is the right shape and not merely the cheap one.** AutoDesign
**already spawns a sidecar of its own**, so this reuses a pattern that team
already operates instead of asking them to build a client they do not have. It
adds no listening socket, which matters in a codebase whose last one was an RCE.
And it does not wait on `integration.invoke`, which is at **G6, deferred** on
their roadmap.

`PD0-06` is re-pointed to an **executable contract** — a reference
implementation plus a conformance suite both teams can run. `PD0-08` defines the
invocation model; `PD0-09` is the public document.

### 2. 🔴 The IR is not versioned, and `PD0-02` is the task that says it is

`src/design/ir.ts:155`:

```ts
export interface DesignDocument {
  width: number; height: number; sourceName: string
  layers: DesignLayer[]; fonts: FontRequirement[]; assetIds: string[]
}
```

No `version`, no `schemaVersion`. `PD0-02` reads *"published as a **versioned**
type"* — the type exists; the versioning does not. That is now the task's actual
content, together with a check that **refuses** an unknown version rather than
parsing it optimistically.

### 3. The IR lives in the frontend, and ownership needed a ruling

`src/design/` is TypeScript, in the Tauri frontend; nothing in `autodev-core`
mirrors it. **Razi's ruling: TypeScript is canonical**, and `PD1` mirrors it
into Rust behind a conformance test pinning the two.

⚠ The drift risk is not hypothetical — `P1` deleted `src-shared`'s mirrored
`tasks`/`workers`/`plans` tables because two definitions of one schema had
diverged, with 22, 19 and 23 write statements between them. The pinning test is
what stops that shape recurring, and it is `PD1`'s to write.

### 4. Three corrections that did not need a ruling

- **`PD0-03`'s job semantics** are re-expressed for a child process: a
  long-running invocation streaming progress and terminal status over stdout,
  not an HTTP job resource that is polled. The statuses are unchanged; the
  carrier is. ⚠ Its *"25–35 s on a 30–45 MB input"* is inherited from the spike
  and I have not re-measured it; the async decision holds regardless, because a
  30–45 MB PSD parse is not a request-response shape at any plausible number.
- **`SD0-01` was sharpened so it can fail.** *"Receives well-formed responses"*
  is satisfiable by a function returning a constant — `L-C9`. It now also
  requires a malformed request to be **refused**, an unknown schema version to
  be **rejected rather than parsed**, and a killed child to leave **no partial
  output claiming success**.
- **`PD0-10` opens the `NOVELTY_LEDGER` row.** The register holds **twenty** rows and — ⚠ this entry first said *six*, corrected
2026-09-03 by the work audit —
  **none for the design programme**, though `PD0` adopts an external contract
  shape, `PD1` is grounded in an external converter, and `PD2`/`PD4` now carry
  ~33 tasks derived from an external adaptation engine. The anchor-set rule is
  that no externally-inspired mechanic becomes product truth without a row.

### What the refutation did NOT overturn

**Razi's 2026-09-01 ownership ruling holds and answers the obvious objection.**
AutoDesign's `integration.invoke` sits at G6, deferred — so they cannot call
AutoDev today. That does not block PD0, because the contract is *the*
deliverable: *"without it the parallel build has nothing to agree on"*
(`AUTODESIGN-INTEGRATION-RESEARCH §7.2`). What it does correct is the milestone
wording I wrote on 2026-09-03, which said AutoDesign *"can wire and test against
the stub"*. They cannot. M1 delivers **an agreed, exercisable seam**.

**Why.** `§0.2m` exists because this plan's own pre-GO review found six errors
in it. This pass found two more before they became code — a task that could not
be built as written, and a task whose stated deliverable did not exist.

**Where.** `MASTER_DEVELOPMENT_PLAN.md §10.5.0` (banner, `PD0-02`/`03`/`06`
amended, `PD0-08`/`09`/`10` added, `SD0-01` sharpened) ·
`_verify/PD0-CORRECTIONS.md`

**How proven.** Every finding above was established in the main session by
reading the code and searching for strings, per `§0.2m`'s re-verification
requirement — not adopted from an agent.

**Links.** `ADR-LOG-RESTRUCT-0033` · `§0.2m` · `§0.2L-3` · `L-C9` · `L-C12` ·
`AUTODESIGN-INTEGRATION-RESEARCH §3`, `§7.2`

## ADR-LOG-RESTRUCT-0042 — Arcana becomes a personal coding platform

**Status.** DECIDED for product scope; compatibility disposition pending.
**Author.** Razi (owner direction, 2026-09-23).
**Supersedes.** The AutoDesign-first R1/R2 sequencing and the planned PD1–PD6
design capabilities in the MDP §5 and §10.5. Historical PD evidence remains.

**What.** Razi renamed the project from AutoDev to Arcana, made it his personal
project, and removed the PD-X design tasks from the intended product. The
remaining target is a multi-agent coding platform built around the native Rust
runtime, fleet, memory, graph, desktop UI, and delivery phases. The current
scope and checkable outcomes are in `ARCANA_CODING_SCOPE.md`.

**Why.** This is the owner's new product direction, given after P8. The old
AutoDesign handover is no longer the sequence that determines when the coding
platform is ready. It cannot be treated as a search-and-replace: `PD0` already
published a runnable protocol, and `~/.autodev`, the keyring service, installer
identifier, and CLI names are live identity and state boundaries.

**Where/how.** The active-scope banner in the MDP and vision points to the
new scope document. Design tasks remain visible as historical rows until
their task and gate disposition is recorded. No PD0 contract consumer or local
state has been removed or moved by this decision. The owner will decide the
retirement policy for that published contract and whether existing local data
must migrate before those implementation units change.

**Evidence.** User direction in this task; `src/design/contract/main.ts` and
`reference.ts` for PD0; `autodev-core/src/db/mod.rs` and
`vault/keychain.rs` for persistent state; `_verify/ARCANA-RECON-PLAN.md` for
the phase audit. This is a scope decision, not a claim of a green build.

## ADR-LOG-RESTRUCT-0043 — Retire PD0 and start Arcana with fresh local state

**Status.** DECIDED. **Author.** Razi (owner answers, 2026-09-23).
**Supersedes.** ADR-0042's pending compatibility dispositions and the PD0
published-interface retention requirement for the current product.

**What.** Retire the PD0 Node design protocol and reference process as part of
the coding-only project. Arcana starts with fresh local tasks, sessions, and
provider vault entries. The old AutoDev root and OS keyring entries are left
undisturbed; no data copy or deletion is required.

**Why.** Razi confirmed there is no established AutoDev user data to carry
forward and explicitly chose to retire the design interface. Keeping it in
CI would preserve a product contract outside Arcana's new scope.

**Where/how.** Remove `src/design`, its sidecar build/smoke, npm/CI hooks, and
its design-only dependency. The P8 exit gate must still protect updater
signature assets and SQLite backup code. Rename root/vault/installation
identity as a separate unit. `.env` remains a live-test input only; the
desktop key is entered through Settings. Historical PD0 evidence remains in
the original documents.

**Proof.** TypeScript/Rust checks, design-path absence scan, and a normal app
launch. No retirement-complete claim is made by this decision entry.

## ADR-LOG-RESTRUCT-0044 — Require an explicit Arcana update feed

**Status.** DECIDED for the current personal build (2026-09-23).

**What/why.** The renamed updater must not query the former AIRS Foundry release
repository or assume a new Arcana release location. Until Arcana has a verified
published feed, update checks return no update. An explicit HTTPS
`ARCANA_UPDATE_URL` enables the existing updater path. This leaves signature and
checksum verification intact and prevents a background request to an unrelated
release source on every launch.

**Where/proof.** `src-tauri/src/updater/check.rs`; Rust check and updater tests
are part of the current rename validation. P9 must choose and prove a release
source before enabling automatic updates for distributed builds.

## ADR-LOG-RESTRUCT-0045 — Use a neutral local app identifier

**Status.** DECIDED for the local Arcana build (2026-09-23).

**What/why.** The Tauri bundle and macOS updater agent use
`dev.arcana.desktop` instead of the former organization-owned identifier.
This is a fresh install identity, consistent with the owner's decision not to
migrate AutoDev state. It makes no claim to an unconfirmed web domain or
release repository. If distribution later requires a different signing or
store identifier, P9 must choose it before publishing installers.

**Where/proof.** `src-tauri/tauri.conf.json`,
`src-tauri/src/updater/install.rs`, and the WiX file-association fragment;
current check/build and packaging smoke remain the validation boundary.

## ADR-LOG-RESTRUCT-0046 — Archive retired design tasks outside the active plan

**Status.** DECIDED by Razi's 2026-09-23 direction for this development-suite
revision.

**What/why.** The MDP now schedules one Arcana coding sequence: reconcile P8
proof, then P4–P7; P9 release work follows later. The former §5 two-rollout
sequence, PD0–PD6 task chapter, design-only P3/P4/P6/P9 rows, and PD0 gate
section are preserved verbatim in `HISTORICAL_DESIGN_PROGRAMME.md`, outside
the active checklists. This removes withdrawn work from active counts without
erasing its dated evidence. Coding build evidence, security and lifecycle
gates retain their obligations.

**Where/how.** `MASTER_DEVELOPMENT_PLAN.md` §5 and phase task lists,
`TESTING_GATES.md` phase index, `ARCANA_CODING_SCOPE.md`, and the public brief.
The active MDP has ~~376 explicitly checkboxed P-numbered rows, 253 checked and
123 open~~ **corrected by ADR-0047** at this edit; the active gate register has 126 P0–P9 rows. These are
counts of written rows, not a readiness or pass claim. The source `origin`
remote is `raziashary/arcana`, so worker setup links now use that owner; the
updater feed still requires explicit configuration until P9 release proof.

**Proof required.** Count scripts/searches, link and source scans, Rust and
TypeScript validation, runnable smoke, and review of the historical extraction.

## ADR-LOG-RESTRUCT-0047 — Correct the Arcana active task count

**Status.** CORRECTION to ADR-0046, 2026-09-23.

**What went wrong.** The first count pattern required `**P...` immediately
after a checkbox. Seven struck-through completed tasks use `~~**P...` and were
invisible to that pattern. It reported 376/253/123, seven short in both total
and checked. This repeats the exact counter defect documented in the former
MDP count checker, which I had read but had not applied to my own count.

**Correction/proof.** The active MDP has **383** explicitly checkboxed P-numbered
rows, **260 checked and 123 open**. The rewritten
`_verify/MDP-COUNTS-CHECK.py` includes struck rows, checks unique IDs, checks
the MDP header and §5.1 totals, checks 126 active gate rows, and requires the
historical archive. No readiness score follows from these counts.

## ADR-LOG-RESTRUCT-0048 — Keep Vite out of Cargo's build tree

**Status.** DECIDED from the 2026-09-23 Windows desktop smoke.

**What/why.** `npx tauri dev --no-watch` launched Vite, then Vite's Windows
watcher tried to watch `target/debug/deps/arcana.exe` while Cargo linked it.
Node raised `EBUSY` and Tauri exited before the app could start. Vite already
ignores `src-tauri`; add `**/target/**` to the same watcher exclusion because
Cargo artifacts are never frontend source. This keeps the desktop dev path
from contending with its own linker.

**Where/proof.** `vite.config.ts`; rerun Tauri dev smoke, observe the Arcana
process and stop it cleanly with port 1420 freed. This entry does not claim a
provider-backed turn or S8-02 completion.

## ADR-LOG-RESTRUCT-0049 — Regenerate the updater interop vector after the rename

**Status.** CORRECTION found by the P8 full-suite run, 2026-09-23.

**What went wrong.** The Arcana rename changed the fixed payload in
`a_signature_made_by_openssl_verifies_here` but retained the Ed25519 public key
and signature generated for the old bytes. The verifier correctly rejected the
pair; the first all-targets run stopped at that test (107 other desktop-library
tests passed), so it did not establish TG8-01.

**Correction/proof.** Generate a fresh throwaway Ed25519 key with the Git for
Windows OpenSSL binary, sign the new fixed payload with `pkeyutl -sign -rawin`,
embed only its public key and detached signature, and discard the temporary
private key. Re-run the interop test and full suite; the tampered-payload
negative control must still fail verification.

## ADR-LOG-RESTRUCT-0050 — Exercise the worker commit recipe in its real shell

**Status.** CORRECTION found by the isolated P8 suite, 2026-09-23.

**What went wrong.** `S8-05` ran the recipe's branch command through `sh` on
every platform, although the production `bash` tool uses `cmd /C` on Windows.
This laptop has Git but no `sh` on PATH, so the smoke failed before exercising
the branch protocol. The recipe also carried a POSIX `2>/dev/null` redirect
that is not portable to the production Windows shell.

**Correction/proof.** Keep the convergent `git switch -c … || git switch …`
branch step without a shell-specific redirect, and execute the smoke through
the same platform shell as the production tool. Run it twice against a
throwaway repository and verify the original trunk stays in place. This is a
test/recipe contract correction, not a reason to add `sh` to Arcana's Windows
runtime requirements.

## ADR-LOG-RESTRUCT-0051 — Ignore the complete repository-local Arcana store

**Status.** DECIDED during P8 verification, 2026-09-23.

**What/why.** The previous ignore covered only `.arcana/sessions/` and
`.arcana/logs/`. An isolated suite home placed under `.arcana/verify/` exposed
that its local databases and Windows cache files would otherwise appear as
untracked source. The entire repository-local `.arcana/` tree is runtime/test
state, not application source, so the ignore now covers it. No tracked file
resides there; the real user vault and home store remain outside the repository.

**Where/proof.** `.gitignore`; `git status` must show only deliberate source
edits, while the isolated suite continues to use an external test home.

## ADR-LOG-RESTRUCT-0052 — Send Gemini tool schemas in its JSON Schema field

**Status.** CORRECTION from the first live Arcana Gemini turn, 2026-09-23.

**What went wrong.** The native Google adapter put every shared tool JSON
Schema under `functionDeclarations[].parameters`. Gemini rejected the request
before generating any token because `annotate_file` declares
`additionalProperties: false`, which that restricted Schema field does not
recognize. Provider connection validation passed because it makes no tool call.
The desktop displayed the provider's 400 error instead of a coding result.

**Correction/proof.** Google's [FunctionDeclaration API reference]
(https://ai.google.dev/api/generate-content#functiondeclaration) defines
`parametersJsonSchema` for a full JSON Schema and makes it mutually exclusive
with `parameters`. Use that field without mutating the shared tool definition;
assert the outbound body shape in a conformance test, then repeat the live
desktop turn including a tool call and a visible answer. A successful key
check alone is not proof of tool-capable Gemini operation.

## ADR-LOG-RESTRUCT-0053 — Reopen P8-01 when port names lacked behavior proof

**Status.** CORRECTION to the 2026-09-03 P8-01/TG8-02 claim, 2026-09-24.

**What went wrong.** `_verify/P8-BASELINE.md` concluded that all ten G4b
PORT/PARTIAL rows had named, tested Rust destinations by locating related
symbols and tests. That instrument did not exercise each row's actual
behavior. A fresh audit found no replay of Anthropic signed thinking across a
tool follow-up, no in-flight execution query registry, no 600-second/500-cap
session reap, no prior-to-next task-state validation, and no full proof for
the prompt, intelligence, bridge, and correlated bus clauses. The streaming
row likewise proves SSE parsing, not its specified three-mode detector.
Redaction has stronger behavioral proof. The baseline's `SATISFIABLE` result
was therefore a false green, not a current acceptance result.

**Decision/proof.** Reopen P8-01 and leave TG8-02 unchecked. Keep all ten rows
in the audit population: none is wholly design-only. Record precise
destinations, tests and missing behavior in
`_verify/P8-ARCANA-CLOSEOUT-2026-09-24.md`. The owner subsequently ruled
to retire design-only clauses while implementing and proving all retained
coding behavior (ADR-0056). Only then restore this gate's check.

## ADR-LOG-RESTRUCT-0054 — Repoint the staleness checker to Arcana's active tree

**Status.** CORRECTION during the 2026-09-24 P8 no-stale sweep.

**What went wrong.** `_verify/MDP-STALENESS-CHECK.py` still opened
`autodev-core/src/provider`, although that tree became `arcana-core`, and
compared P4-F with a PD0 heading moved to the historical archive. Both checks
raised rather than checking the current tree. Its `S-7` also invoked Unix
`test -d` through `cmd.exe` on Windows; the command failed, which accidentally
made the absence check pass without examining the directory.

**Correction/proof.** Point the provider check at `arcana-core`, bound P4-F
between the active §10 and §11 headings, and check MCP directory presence and
absence with `os.path`. The Python checker now reports 21 checks, 0 open and
passes syntax compilation on Windows. No historical PD0 text was rewritten.

## ADR-LOG-RESTRUCT-0055 — Apply P8's sidecar-absent suite to built gates

**Status.** OWNER-RULED 2026-09-24.

**Conflict.** `TESTING_GATES.md` §11's old P8 preamble required the entire
TG0–TG7 set during P8, but the active MDP explicitly schedules P4–P7 after
P8. Several of those future gates have no implementation yet. A review caught
our first TG8-01 green mark using only the current Windows suite against that
literal wording; it was retracted while the owner decided scope.

**Ruling and application.** The owner chose **"Apply built gates only"**:
P8's sidecar-absent regression run covers all tests and gates that are already
built and runnable in the current CI suite. Future P4–P7 gates become binding
in their own phases. This makes the P8 prerequisite executable without
silently treating unbuilt future gates as passed. The separate incomplete
P0/P3 specification proofs stay open in their own registers and readiness
rows. Record the 652-test isolated Rust suite and the frontend checks as
TG8-01's current Windows proof, and leave platform-triple release proof to P9.

## ADR-LOG-RESTRUCT-0056 — Retain coding ports and repair the workspace format gate

**Status.** OWNER-RULED 2026-09-24.

**Scope.** The owner chose to retire only Adobe/design clauses from G4b's ten
PORT/PARTIAL rows and **implement and prove the coding-relevant behavior** in
Arcana. No entire row is dropped. TG8-02 stays open until the retained behavior
has named Rust destinations and tests that fail when the behavior is removed.
This is a product-scope decision, not a declaration that the old baseline's
symbol matches were adequate.

**Format gate.** The same ruling authorized a mechanical workspace-wide
`cargo fmt --all` pass for pre-existing formatting debt, followed by
`cargo fmt --all -- --check` and proportionate tests. Keep formatting changes
separate from behavior edits in review and a logical commit where possible.

## ADR-LOG-RESTRUCT-0057 — Enforce task lifecycle at the database boundary in P8

**Status.** OWNER-RULED 2026-09-24; implementation pending.

**Conflict.** G4b §A.5 retains coding lifecycle guards as part of the sidecar
port. Today `TasksDb::update_status_with_reason` accepts any supplied
transition; Tauri, CLI and recipes can bypass the agent tool's narrower
checks. `TaskStatus` parsing silently maps unknown strings to `Ready`. The
worker runner also writes `idle_no_provider`, which `WorkerStatus` fails to
represent. ~~`P4-02` owns strict parsing of unknown task statuses.~~ **This
was false: P4-02 explicitly concerns `workers.status`; see ADR-0058.**

**Ruling.** The owner chose **"Enforce transitions in P8"** for the task
database. Derive legal moves and recovery exceptions from all current task
writers, including direct SQL paths, and test direct DB, UI/CLI and tool paths.
P4-02's worker parser ownership remains with P4; P8 can represent the real
`idle_no_provider` state without claiming the future strict-parser task. The
exact task transition graph remains a pre-build verification subject, not an
assumed policy.

## ADR-LOG-RESTRUCT-0058 — Correct the worker/task status ownership claim

**Status.** CORRECTION from the P8 §0.2m pre-build audit, 2026-09-24.

**Mechanism.** ADR-0057 and the draft port workplan said P4-02 owned strict
parsing of unknown **task** statuses. Its actual MDP task says
`workers.status`; the draft conflated two distinct database contracts and
would have silently moved a P4 task into P8. The audit also found
`TasksDb::unblock_ready_tasks` writes status by direct SQL twice, bypassing
`update_status_with_reason`; one write ignores the SQL result. A guard in
that method alone would not enforce the task invariant.

**Correction.** Keep P4-02's worker-parser acceptance in P4. P8's owner ruling
applies to task lifecycle enforcement; its implementation must cover all
direct and indirect task status writers or enforce at the database boundary.
Test the unblock/recovery path as a negative control. Reconcile `idle_no_provider`
with the worker enum as a current bug without declaring P4-02 complete.

## ADR-LOG-RESTRUCT-0059 — Port the MCP round trip as direct built-in dispatch

**Status.** DECIDED from P8 §0.2m source review, 2026-09-24.

**What the bridge served.** G4b A.12/C.7 describes `MCPBridge` converting
the *existing Rust in-process MCP server's 26 built-in tools* into Python
`ToolSpec`s, then calling back over HTTP. P8 removed that redundant
Rust→Python→Rust round trip. The current core `ToolRegistry` already produces
tool definitions and dispatches the built-ins directly from `AgentEngine`.
Configured external MCP servers currently have health/config paths but no
agent tool-discovery/call consumer; adding a generic external client here
would create a new product/trust contract rather than port the old bridge.

**Proof obligation.** TG8-02's bridge row requires a behavioral test from
built-in `ToolRegistry` definition through a model-facing request and a real
tool result consumed by the engine, plus the separate DELEGATE parser and
depth/cycle behavior. Keep external MCP discovery/call as a separately named
future feature, not a false claim that health/config already supplies tools.

## ADR-LOG-RESTRUCT-0060 — Prefer integrated native ports, prove equivalence when necessary

**Status.** OWNER-RULED 2026-09-24 after the P8 §0.2m pre-build refutation.

**Choice.** The owner wants a full AutoDev→Arcana conversion and prefers to
recreate and **integrate** the old coding mechanisms, including runtime/tool
registries, delegation and internal events, rather than declaring them
irrelevant because the current tree has no reader. Where literal recreation
is not possible or would violate the native trust/permission architecture,
prove the native equivalent on a real coding path and record the reason.

**Guardrails.** A new registry, bus, prompt source or decision type is not
accepted because it exists: a production consumer must read it and a test
must fail if that consumption is removed. Do not revive the unauthenticated
hosted MCP server, a prose delegation permission bypass, old sidecar
startup phases, Adobe/design features or Redis/A2A transport solely for
parity. Port the coding semantics into Rust and keep Arcana's current
security boundaries. TG8-02 remains open until every retained row meets
this standard.

## ADR-LOG-RESTRUCT-0061 — Manual task changes obey the shared lifecycle

**Status.** OWNER-RULED 2026-09-24.

**Choice.** Tauri UI and CLI manual task-status actions must follow the **same
legal transitions** as agent/recipe actions. There is no special manual
override that bypasses the task database guard. The current free-form status
command therefore may return an invalid-transition error for a jump it used
to accept. This is intentional and must be represented clearly to callers.

**Proof.** P8-69 must test valid workflow/recovery edges and invalid jumps
through the shared DB method and each direct SQL writer, then drive one UI/CLI
path to show it cannot bypass the rule. Do not use the unrelated P4-02 worker
parser task as evidence of task lifecycle completion.

## ADR-LOG-RESTRUCT-0062 — Represent native runtime isolation as leases

**Status.** DECIDED from G4b A.4 and P8 §0.2m evidence, 2026-09-24;
implementation pending.

**Why this shape.** The old Python sidecar retained one `AgentRuntime` object
per `(agent_id, session_id)`, with 600-second idle reap and a 500-entry cap.
Arcana already creates a fresh `AgentEngine` for each interactive turn,
recipe tick and subagent run; recipe/subagent sessions are intentionally
ephemeral. Retaining those engines for ten minutes would add a new lifetime,
hold provider state and conflict with that native design.

**Native port.** Track lightweight `(agent_id, session_id)` admission leases,
not retained engines. All three production entry paths must acquire a lease;
an active pair cannot be admitted twice, idle entries are reaped after 600
seconds on a 60-second interval, and the registry refuses entry 501 while
preserving persisted session rows. A guard releases activity on every normal,
error and panic exit. Tests with a controlled clock prove isolation, cap and
reap; an entry-path test must prove a real caller consults the registry.
This is the allowed native equivalent under ADR-0060, not a literal Python
object-lifetime transplant.

## ADR-LOG-RESTRUCT-0063 — Keep provider reasoning private across the native stream

**Status.** DECIDED from the current provider catalogue and stream contracts,
2026-09-24.

**Choice.** Preserve three distinct input modes in Arcana's existing stream
path: provider-native reasoning, provider-native thinking, and bounded
incremental `<thinking>` fallback for models whose catalogue entry does not
advertise native reasoning. Private segments feed only the dedicated thinking
event and never ordinary chat text or provider-visible history. Provider
capability metadata selects the mode; model-name heuristics do not.

**Proof.** Test native OpenAI/Gemini/Anthropic chunks, split and incomplete XML
delimiters, and the engine path that routes each segment. Anthropic signed
block replay is covered separately by P8-68.

## ADR-LOG-RESTRUCT-0064 — Use the live worker database as prompt and dispatch authority

**Status.** DECIDED from current WorkersDb and tool validation paths,
2026-09-24.

**Choice.** Delegate prompt construction and assignment/recipient validation
share the current non-deleted worker roster from `WorkersDb`. The roster is
placed only in the volatile prompt tail, so roster changes do not stale the
cached stable prompt prefix. If the database cannot be read, the composer does
not invent an authoritative roster. Tool-result honesty remains in the shared
invariant core used by all agent kinds.

**Proof.** Database tests cover custom and deleted workers; composer tests
cover the volatile live-roster fragment and invariant core; tool validation
tests demonstrate the same database roster is accepted by the real dispatch
surface.

## ADR-LOG-RESTRUCT-0065 — Correlate in-flight executions through typed native events

**Status.** DECIDED from the current execute_batch, Tauri event, and chat
reconnect consumers, 2026-09-24.

**Choice.** Keep existing public Tauri event names and payloads while adding a
typed in-process event channel for internal execution tracking. `execute_batch`
publishes start/finish events and owns an RAII execution guard; the registry
subscribes to finish events and exposes only redacted active-call snapshots to
the Tauri command. The chat store hydrates that snapshot after reconnect and
guards against a late snapshot resurrecting a tool already completed in the
live stream. The Arcana built-in tool registry and structured subagent tool
remain the dispatch and delegation authorities; no prose delegation parser
or external MCP client is added without a real supported caller.

**Proof.** Run a blocked real tool through `execute_batch`, query it from the
registry reader, then observe completion cleanup; test the frontend late
snapshot race and redacted command payload. Existing pool and native-runner
tests prove depth and permission behavior. Retry decisions and provider
routing continue through their current typed results and real engine
consumers; a second generic decision engine would duplicate authority.

## ADR-LOG-RESTRUCT-0066 — Align P8-02 smoke with the coding-only product scope

**Status.** OWNER-RULED by the user's 2026-09-23 coding-only direction;
correction recorded 2026-09-24.

**Choice.** Retire P8-02's legacy design-conversion clause with the PD-X
programme. The active smoke is that Arcana starts, completes a native
coding-agent turn, and has no application-owned Python sidecar process. Keep
the 2026-09-03 `skip_sidecars` implementation note only as historical evidence;
that setting has since been removed because no sidecar remains to skip.

**Why.** Requiring a design conversion contradicted the owner's decision to
make Arcana fully coding-focused and retire PD-X. S8-02 already demonstrates
the retained coding behavior through the live Gemini desktop path and process
observation.

**Proof.** `MASTER_DEVELOPMENT_PLAN.md` P8-02 now states the coding-only
outcome; `S8-02` and `_verify/P8-ARCANA-CLOSEOUT-2026-09-24.md` carry the
recorded live-turn and process-list evidence.

## ADR-LOG-RESTRUCT-0067 — Keep the shared fleet branch and prove reversibility with task checkpoints

**Status.** OWNER-RULED by the user's 2026-09-26 decision; the shared
implementation is present and consumer proof remains in progress under
TG3-26/TG4-16.

**Choice.** Keep the shared `arcana/fleet` branch and shared checkout required
by the shipped fleet protocol. Revise P4-20: do not create or switch to a
branch per task. Prove per-task reversibility through checkpoint ownership and
restore behavior instead.

**Why.** Fleet workers operate against the same configured workspace. A
per-task branch switch would change shared Git state underneath concurrent
workers and contradict ADR-0040 and the P8 worker protocol. The user explicitly
ruled that the shared branch governs and P4-20 must prove per-task
reversibility through checkpoints.

**Proof required.** The P4 task and TG4-23 must verify that each task's writes
are associated with that task, a failed checkpoint prevents the write, and
restoring one task's checkpoint does not revert another task's writes. Keep
the single-checkpoint-store requirement from TG1-05. Do not treat a session ID
as a task ID unless the production path proves that relationship.

## ADR-LOG-RESTRUCT-0068 — Carry task budgets and consent through native fleet execution

**Status.** OWNER-RULED by the user's 2026-09-26 decisions; implementation
in progress as of 2026-09-26.

**Choices.** Retain P4-29's saved per-task advisory budget alongside the
existing worker-run cap. Bring the complete workspace-egress consent contract
into P4 so QC can obey TG3-26/TG4-16: consent is recorded per workspace and
destination, can be revoked, authorizes only that destination, and absent or
revoked consent blocks egress without prompting unattended workers. Prove
S4-03 with cancellation or panic of the in-process worker run, followed by
automatic claim recovery and re-dispatch; Arcana workers are Tokio tasks, not
separate operating-system processes.

**Why.** A worker-level cap is not the product's persisted per-task budget
behavior. The existing TG4-16 contract depends on a production consent store
and reader that source inspection did not find; leaving it blocked on P3 would
prevent the user-requested P4 closeout. The consent mechanism therefore needs
its full shared persistence, decision, revocation, and enforcement path in P4,
not a QC-only exception. For S4-03, process-level restart is not what the
current in-process runner can claim; exercise its actual cancellation and
recovery seam instead.

**Proof required.** Per-task budget settings load for the claimed task, enforce
the advisory breaker, and surface the result without weakening P2's hard cap.
Consent tests cover absent, granted, destination-mismatched, revoked, and
unattended calls; every workspace-content egress consumer reads the same
persisted decision and records an auditable outcome. Worker-fault tests cancel
and panic a real scheduler run, then observe claim release and subsequent
re-dispatch without an operator action.

## ADR-LOG-RESTRUCT-0069 — Keep task-board commits atomic at their behavioral boundary

**Status.** DECIDED from the live task tool contract and current P4 source audit,
2026-09-26.

**Choice.** Dependency unblocking and the corresponding epic-progress update
are one transaction. Batch assignment retains its existing per-entry
partial-success contract; each entry atomically commits its status, assigned
worker, and routing-history row, while failures for other entries remain
independent. Claim metadata is set only by a successful atomic claim and is
cleared when a task leaves a claim-bearing state or loses its owner. A budget
overrun remains held for explicit budget/user resolution; it must not
auto-resume merely because dependency-blocked work shares `needs_info`.

**Why.** The batch tool reports independent per-entry outcomes today, so
whole-batch rollback would replace its user-visible contract. By contrast,
unblocking dependents and advancing their epic is one logical board mutation.
The current lifecycle has both automated dependency waits and budget stops in
`needs_info`; only dependency completion is an automatic resume condition.
Claim PID/host data identifies a live in-process claim and cannot outlive that
claim into reassignment.

**Proof required.** Crash-inject every commit boundary in isolated task DBs;
prove dependent-ready changes and epic advancement roll back together, while
successful batch entries commit independently and failed entries leave no
partial owner/history. Test budget-held and dependency-blocked `needs_info`
separately, and exercise every task status writer to prove stale claim metadata
cannot release a reassigned task.

## ADR-LOG-RESTRUCT-0070 — Deliver worker messages at least once until acknowledged

**Status.** OWNER-RULED by the user's 2026-09-26 decision; implementation
in progress as of 2026-09-26.

**Choice.** Worker messages are durably delivered at least once until their
acknowledgement is committed. A stale/unacknowledged message is recoverable;
the consumer uses the stable message ID to make repeated handling idempotent.
Do not claim exactly-once delivery or exactly-once external effects.

**Why.** The current reader changes `pending` to `read` before a separate
acknowledgement. A consumer crash between those steps can require replay, so
exactly-once delivery is not the behavior this persistence model can guarantee.
The user selected at-least-once-until-ack with idempotent handling.

**Proof required.** Deliver, crash the consumer before ack, recover and redeliver
the same message ID, demonstrate idempotent handling, then ack and prove no
further delivery. Also prove acknowledged messages are never replayed.

## ADR-LOG-RESTRUCT-0071 — Keep migration tests out of the live backup directory

**Status.** DECIDED, 2026-09-26.

**Choice.** P4 migration behavior tests use isolated in-memory SQLite fixtures
and run the production migration ladder against a legacy schema there. Do not
use `open_at` with a file-backed fixture while its destructive-migration path
unconditionally writes safety snapshots to `%USERPROFILE%\\.arcana\\backups`.
Do not remove any files produced by an earlier test without explicit owner
approval. This proof does not assert backup integration.

**Why.** A focused P4 test attempt showed that a temporary database file still
triggered the production backup destination in the user's Arcana directory;
parallel tests also collided on the backup filename. The test must not
silently mutate user state or race over snapshots. The existing migrator's
in-memory path skips backup for a database with no file path while applying
the same SQLite DDL and migration steps.

**Proof required.** The P4 test creates the pre-migration schema in memory,
applies the actual worker migration ladder, and verifies accepted/rejected
statuses and resulting schema. Record backup-path integration as untested;
retain existing backup files and report the test side effect discovered during
the first attempt.

## ADR-LOG-RESTRUCT-0072 — Preserve the existing routing-history state as found

**Status.** DECIDED from a read-only inspection of the Arcana task database,
2026-09-26.

**Choice.** Preserve the existing `~/.arcana/tasks.db` without modification.
Its `routing_history` table is absent, so there are no existing routing rows
to repair or remove. P4 may add the missing table through the normal migration
ladder; this is a schema repair, not a cleanup of prior history.

**Why.** P4-24a required a read-only inspection before deciding whether old
attributions needed cleanup. The actual Arcana file passed `PRAGMA
integrity_check` and contains the expected task/worker tables, but no
`routing_history`. The source also contains reader/writer methods without a
table-creation statement, so routing persistence is not currently initialized
by the shipped task database schema.

**Proof.** On 2026-09-26 a Python 3.14 SQLite connection opened the actual file
with `mode=ro` and `PRAGMA query_only=ON`. Integrity returned `ok`; a catalog
query found no `routing_history` table. No data was read beyond schema and
integrity metadata, and no database write, migration, or backup was performed.

## ADR-LOG-RESTRUCT-0073 — Scope persisted workspace consent to Arcana-mediated egress

**Status.** OWNER-RULED by the user's 2026-09-26 decision; implementation
pending.

**Choice.** Persisted, revocable workspace consent gates Arcana-mediated
provider requests and diff-analysis requests. A grant is scoped to the
canonical workspace and the destination (provider/service). Missing,
mismatched, or revoked consent refuses unattended egress; the decision and
outcome are auditable. Arbitrary network activity inside free-form shell
commands is outside this app-mediated consent contract because Arcana cannot
reliably infer its destination. That requires separate OS-level network
control and is not represented as covered by TG3-26.

**Why.** The owner selected “App-mediated egress.” Provider and diff calls
have a known destination and pass through Arcana-owned code. A shell command
can contact destinations chosen dynamically by its own program or script, so a
workspace/provider grant must not be described as containing that path.

**Proof required.** Exercise grant, absent, destination mismatch, revoke, and
unattended behavior through the persisted production reader. Prove every
Arcana-mediated provider and diff consumer reads the same decision and records
an auditable outcome. Document shell network egress as outside this consent
boundary.

**Implementation progress (2026-09-28).** The shared consent tables, grant /
revoke commands, provider wrapper, QC deterministic fallback, direct Tauri diff
wrappers, and Settings controls are present. QC now checks consent before
provider-key lookup; the missing/mismatched-consent path returns deterministic
structural analysis without credentials. Focused consent-provider and
structural-fallback tests pass. `analyze_diff_tool_executes_repository_diff_without_consent_or_provider_key`
now drives `AnalyzeDiffTool::execute` over a real isolated repository diff,
verifies deterministic-only output without a provider key, and reads back the
denied audit row. TG3-26/TG4-16 remain open for the Tauri PR-diff invocation
and full allowed/revoked desktop-consumer proof.

## ADR-LOG-RESTRUCT-0074 — Make the coding harness and built-in team the product distinction

**Status.** OWNER-RULED by the user's 2026-09-27 direction.

**Choice.** Arcana's product goal is a capable coding harness that works side
by side with models. Arcana owns deterministic, observable, and safe mechanics
where the rules can be specified, including project/tool discovery, command
construction, permission checks, task lifecycle, evidence, and recovery. Models
own code generation and judgment where intelligence is needed. Arcana also
ships a usable built-in development-team workflow so a user can start without
authoring agent files, subagents, skills, or tool wiring; recipes remain the
customization mechanism.

**Why.** The owner wants Arcana to be meaningfully different from a model
wrapper and from tools that require hours of personal workflow configuration.
Unused registered tools and prompt-only safety instructions do not satisfy
that goal.

**Proof required.** Audit P1, P2, P3, P4, and P8 against production consumers
and behavior evidence. For each deterministic capability, identify its source
contract, actual caller, fail-closed behavior, and tests; prove a fresh project
can complete a coding task through built-in defaults. Record P5–P7 and other
later-phase findings with their owners rather than silently closing their
gates early.

## ADR-LOG-RESTRUCT-0075 — Enforce fleet commit safety in the Arcana host

**Status.** OWNER-RULED by the user's 2026-09-27 instruction to close P8
properly after the audit identified prompt-only commit safety.

**Choice.** Keep the shared `arcana/fleet` branch selected for the fleet, but
make staging and committing host-owned. Workers call the registered
`git_commit` tool with explicit file paths and a Conventional Commit subject
plus a descriptive body. The host refuses `main`/`master`, pre-staged unrelated
changes, paths outside the repository, directories, `.env` files, and staged
diffs redacted by Arcana's credential scanner. Bash rejects direct `git add`
and `git commit`; branch inspection and switching remain available.

**Why.** The previous recipe protocol depended on model compliance. P8-38's
smoke drove raw Git commands itself, so it proved the script worked but did not
prove the production tool path could stop an agent from bypassing it. The owner
directed Arcana to close this host-enforcement gap with a real gate and smoke.

**Proof required.** Behavior-focused negative cases for trunk, pre-staged
changes, unsafe paths, credentials, and incomplete messages; positive isolated
repository commit with only named files; Bash refusal and production registry
registration; recipe consumer and end-to-end smoke. Keep P8 open until TG8-13
and S8-06 pass on the current Arcana source.

**Implementation evidence (2026-09-27; strengthened 2026-09-28).** P8-74, TG8-13, and S8-06 passed on
the current tree. The Git tool refuses paths and diffs according to the
redaction scanner's recognized credential patterns. The Bash guard inspects
textual command segments, including covered shell wrappers, and is not a
general-purpose process sandbox; encoded or otherwise obfuscated executable
code remains outside that heuristic. The supported worker path uses the host
tool, and the gate/smoke evidence is in `TESTING_GATES.md` and the P4 session
checkpoint.

The 2026-09-28 closeout strengthening protects a branch literally named
`trunk` in addition to `main` and `master`, serializes the full shared-index
transaction among in-process workers, validates every path-array element,
scans the exact staged blobs (including binary content), and requires the
documented blank line before the commit body. It exercises host-tool trunk
refusal in S8-06 before proving the allowed named-file commit on
`arcana/fleet`. The current focused unit suite has ten passing tests. Bash
protection covers common textual Git index/ref mutation commands; it remains a
heuristic rather than a sandbox against arbitrary executable payloads.

## ADR-LOG-RESTRUCT-0083 — Serialize shared-index commits and scan staged blobs

**Status.** Accepted on 2026-09-28 as P8 closeout hardening under ADR-0075.

**Choice.** `git_commit` acquires Arcana's process-wide path lock for the
repository index before reopening Git state and holds it through staging,
validation, commit, and rollback. It also holds sorted per-file locks through
staging. Secret scanning reads the exact blob IDs from the staged index rather
than reopening mutable worktree paths.

**Why.** Multiple recipe workers run in-process against one checkout. Without
serialization, a failed commit could roll back another worker's staged state;
reading the worktree after staging could inspect bytes different from those in
the commit tree.

**Proof.** `concurrent_commits_serialize_index_staging_and_refusal_rollback`
starts two valid commits and a secret-bearing refusal concurrently and checks
both valid commits survive, the secret is absent, and the final index is clean.
`commit_refuses_recognizable_credentials_embedded_in_binary_file` verifies
binary content is checked from the staged object. TG8-13 and S8-06 pass on the
current tree. This lock is process-local; the product's in-process fleet is the
scope of the guarantee.

## ADR-LOG-RESTRUCT-0084 — Launch fixed Windows package-manager shims via cmd

**Status.** Accepted on 2026-09-28 under P4-11/TG4-18.

**Choice.** On Windows, `run_resolved_command` launches resolved `.cmd` and
`.bat` package-manager shims through `cmd.exe /d /s /c`, preserving the quoted
shim path. The only arguments placed in that bounded shell invocation are
fixed argv generated by the `CommandIntent` resolver; model-authored shell
text is never used for these arguments. Other executables continue through
direct process creation.

**Why.** The Windows resolver can correctly locate `npm.cmd`, `pnpm.cmd`, or
`yarn.cmd`, but `CreateProcessW` cannot execute a batch shim directly. The
first production-launcher fixture exposed incorrect `/S /C` quoting for paths
with spaces, so the invocation now has an explicit outer quote pair.

**Proof.** `windows_batch_package_manager_shim_runs_with_fixed_argv` writes a
temporary `npm.cmd` under a path containing spaces, verifies it receives the
fixed `run build` arguments, and observes its exit code 17 through the same
launcher used by `ProjectCommandTool`. This proves the Windows shim launch
primitive; the three-platform project corpora remain open under TG4-18.

## ADR-LOG-RESTRUCT-0076 — Retire the obsolete prose delegation proof

**Status.** Accepted as a documentation reconciliation on 2026-09-27.

**Choice.** The separate prose `DELEGATE` parser requirement in ADR-0059 is
retired. Structured delegation is the supported contract: built-in tools
create subagents, the engine executes their typed calls, and the parent
permission mode is inherited. P8's retained proof is the structured production
path and its behavior tests; no prose parser or parallel delegation contract
will be recreated.

**Why.** ADR-0060/0065 and P8-73 chose structured tool calls and retired prose
delegation. Keeping ADR-0059's earlier parser test as an active obligation
conflicts with that later contract and would reward a second, unused parser.

**Proof required.** Keep the structured tool registry, permission checks,
subagent pool, and native execution behavior tested at their consumers. The
retired parser requirement must not be used to claim missing P8 work.

## ADR-LOG-RESTRUCT-0077 — Resolve one canonical project workspace

**Status.** Accepted on 2026-09-28 as the P4-17 implementation of the existing
workspace invariant.

**Choice.** Interactive sessions, fleet recipes, IDE file access, diff analysis,
and app-mediated workspace consent resolve the configured or explicit project
root through `platform::workspace::resolve_workspace_dir`. The resolver accepts
only existing absolute directories, returns the canonical path, and fails when
the setting is unset or invalid. No execution path falls back to process CWD or
an Arcana-owned per-agent directory.

**Why.** A single project root keeps all agents and app-mediated workspace
operations scoped to the user's selected project. Silent fallback makes
misconfiguration look like success while writes land in an unrelated directory.

**Proof required.** Keep negative tests for unset, relative, missing, and
non-directory paths; prove the production session/recipe consumers reject
fallback and the Tauri IDE/diff/consent callers use the resolver. File-write
  confinement and reveal/open/run handoff are separate P4-18/P4-21 obligations.

## ADR-LOG-RESTRUCT-0078 — Keep escalation visible without blocking fleet dispatch

**Status.** Accepted on 2026-09-28 as the P4-24 liveness implementation.

**Choice.** Pending worker requests are recoverable Supervisor work, so they
keep the Supervisor eligible for an event/recovery tick. Resolving a request
does not short-circuit same-tick READY-task dispatch. If a request remains
unresolved after actionable board work is handled, the Supervisor may stop
idle; the pending request remains visible and keeps the next recovery tick
eligible.

**Why.** The previous recipe stopped after handling any request and the
scheduler did not consider requests when deciding whether an idle Supervisor
had work. A pending blocker could therefore suppress both immediate dispatch
and future wake-up when it was the only Supervisor work item.

**Proof.** `a_pending_escalation_keeps_supervisor_dispatch_available` leaves a
request pending with no ready tasks, verifies the Supervisor work predicate,
then runs the shipped batch tool and reads an unrelated task back as assigned.
`shipped_supervisor_creates_visible_epic_and_phased_child_through_agent_engine`
also runs the shipped instructions through AgentEngine, leaves the request
pending, and reads back an unrelated assignment. A targeted startup migration
updates the early-stop clause while preserving customized recipe text.

## ADR-LOG-RESTRUCT-0079 — Attribute routing outcomes to one observed assignment

**Status.** Accepted on 2026-09-28 as the P4-25 correctness implementation.

**Choice.** A completion/reassignment outcome belongs to the newest pending
routing row for the worker recorded on the task before the status transition.
The status change and outcome write commit in one transaction that compares the
observed status and owner before applying the transition; a stale completion
after reassignment is refused. Older pending rows for prior attempts remain
unchanged. A manually assigned task with no affinity row can still complete; it
simply produces no routing observation. Tauri and worker-tool outcomes use the
same transaction.

**Why.** Updating every pending row for a task can train affinity on stale or
wrong worker assignments. The task's pre-transition owner is the authoritative
worker identity after owner-checked claim.

**Proof.** `autonomous_completion_closes_only_the_current_owners_routing_row`
uses the real status tool and checks both the selected row and task status,
including completion of an untracked manual assignment.
`stale_task_owner_cannot_overwrite_reassignment_or_claim_its_outcome` proves a
delayed completion cannot change the reassigned task or attribute success to
the old worker.

## ADR-LOG-RESTRUCT-0080 — Compare-and-set automatic and manual orphan release

**Status.** Accepted on 2026-09-28 for P4-40; the owner requires stale-safe
compare-and-set for both automatic recovery and manual Tauri force-release.

**Choice.** Automatic reconciliation releases a task only when its status,
owner, PID, host and update timestamp still match the diagnosed row snapshot.
Manual Tauri force-release compares the same observed fields against the UI's
expected snapshot and calls the same conditional DB writer. A mismatch refuses
release so a replacement worker keeps its claim.

**Why.** Process liveness is checked after candidate selection. Without a
compare-and-set, a new worker can acquire the task between those operations and
then be released using the previous worker's stale PID result.

**Proof.** `stale_orphan_snapshot_cannot_release_a_newer_worker_claim` captures
the old claim, releases and reassigns the task, then proves the delayed recovery
write leaves the new owner and PID intact. The Tauri command test
`manual_release_command_rejects_a_stale_claim_snapshot` invokes manual release
with that prior snapshot and proves the replacement worker remains in progress.
SQLite migration 6 enforces claim metadata invariants. A fresh writer audit and
an injected race between Tauri preflight and CAS remain open.

## ADR-LOG-RESTRUCT-0081 — Check QC consent before provider construction

**Status.** Accepted on 2026-09-28 as a P4 cross-gate correctness fix for
TG3-26/TG4-16 under the owner-approved app-mediated egress boundary
(ADR-0073).

**Choice.** `AnalyzeDiffTool` checks persisted workspace-and-destination consent
before resolving a provider key or constructing the provider. Without matching
consent it returns deterministic structural analysis; with consent, the shared
provider wrapper rechecks authorization immediately before the outbound call.

**Why.** Constructing the analyzer before checking consent made the local,
deterministic QC fallback depend on a configured provider key, even though that
fallback is specifically the behavior for absent or mismatched consent. The
provider wrapper remains the final egress boundary in case consent is revoked
between the QC decision and the request.

**Proof.** `qc_diff_without_matching_consent_uses_structural_review_without_provider_key`
asserts that a mismatched grant returns `deterministic_only` without vault
configuration. `provider_reader_blocks_without_grant_and_rechecks_revocation`
asserts that the production wrapper blocks and rechecks outbound provider
calls. The Tauri direct-diff path now checks and audits consent before provider
construction; its mismatched-consent test proves the deterministic fallback
works without a key. Full `AnalyzeDiffTool` Git-diff and Tauri PR-diff behavior
remain open under TG3-26/TG4-16.

## ADR-LOG-RESTRUCT-0082 — Discover nested project manifests deterministically

**Status.** Accepted on 2026-09-28 for the P4-11 project-command path, following
the owner's validated-selector and mixed-project fallback direction.

**Choice.** `detect_toolchains` inspects manifests from the selected directory
through workspace ancestors and recursively beneath the selected directory.
It prunes generated/dependency trees, does not follow directory symlinks, and
fails clearly at its 20,000-directory bound. Matching Cargo workspace members
collapse to their workspace root so Cargo does not repeat the same workspace
operation. A toolchain selector runs every detected project for that ecosystem;
without a selector, each detected project runs separately. Dependency install
remains an explicit intent.

**Why.** Ancestor-only detection handled a selected nested package and a
mixed-language root, but missed sibling projects when the workspace root was
selected. Running only the first match for an ecosystem also silently omitted
other projects in a monorepo.

**Proof.** `workspace_discovery_finds_nested_packages_and_prunes_generated_trees`
detects separate pnpm and yarn projects, collapses a declared Cargo member to
the workspace root, and ignores `node_modules`. The shipped project-command
tests assert a selector retains all matching nested projects, Arcana's mixed
Rust/TypeScript root resolves separately, and real spawned process exits are
reported. Three-OS runtime proof remains open under P4-11/TG4-18.

## ADR-LOG-RESTRUCT-0085 — Group analytics timestamps by the user's local day

**Status.** Accepted on 2026-09-28 as a correctness repair from the P1/P2
cross-phase audit; TG2-15 remains open.

**Choice.** Every `token_usage` calendar-date reader applies SQLite's
`'localtime'` modifier before `DATE()` grouping/range filtering. Stored values
remain UTC RFC3339 instants; dashboard activity, date queries, and rollup input
are grouped by the machine's local calendar day.

**Why.** `AnalyticsCollector` writes UTC instants, while the caller supplies
`Local::now()` dates. `DATE(timestamp)` interprets a UTC timestamp as a UTC
calendar date, which can place activity near local midnight on the wrong day.
Using the local-time conversion at each reader keeps query semantics aligned
with the dates chosen by the dashboard and daily rollup without changing stored
timestamps or existing database rows.

**Proof.** `utc_instants_are_grouped_by_the_users_local_calendar_day` writes
isolated analytics rows one second before and one minute after local midnight,
then checks date lookup, today's dashboard counters, and date-range totals.
TG2-15's property matrix across UTC−12/0/+14, DST transitions, and measured
rollup cadence is still required before that gate can close.

## ADR-LOG-RESTRUCT-0086 — Retract stale P8 protected-alias audit claim

**Status.** Correction recorded on 2026-09-28 after checking the live source.

**Correction.** The earlier statement in `_verify/2026-09-28-P8-CLOSEOUT-AUDIT.md`
that the shipped worker Commit Protocol and `git_commit` tool description omitted
`trunk` was incorrect for the current tree. Both already list `main`, `master`,
and `trunk`; the report's coordinator closeout failed to refresh that stale
finding. The audit paragraph is struck through and the current recipe consumer
assertion now guards against recurrence.

**Evidence.** `git_commit.rs` description and exact branch check, shared Commit
Protocol in `recipes/defaults.rs`, and TG8-13's shipped-recipe assertion.

## ADR-LOG-RESTRUCT-0087 — Derive Settings providers from the factory set

**Status.** Accepted on 2026-09-28 as a cross-phase P2 gate correction.

**Choice.** The Settings `list_providers` command obtains selectable provider IDs
from `provider::factory::supported_ids()` and maps each ID to a visible label.
If a supported provider has no Settings label, listing fails instead of silently
omitting it. TG2-14 proves the exact supported/buildable factory set and this
Settings consumer mapping; retired IDs may remain discoverable for legacy-key
cleanup while remaining unbuildable.

**Why.** The previous hardcoded Settings vector could drift from the factory:
a provider could become selectable while provider construction rejected it, or
a new supported provider could remain invisible. A zero-reference string sweep
would also remove intentional rejection and legacy-secret cleanup paths.

**Proof.** `settings_provider_list_covers_the_factory_supported_set` asserts the
Settings label mapping covers the sorted factory set. The four provider factory
tests prove exact IDs, successful construction of supported IDs, and rejection
of unknown/retired IDs. All five focused tests passed on 2026-09-28.

## ADR-LOG-RESTRUCT-0088 — Use the workspace jail for IDE file mutations

**Status.** Accepted on 2026-09-28 as a focused path-safety repair discovered
during the P4 write-path audit.

**Choice.** Tauri IDE create, delete, rename, and move operations resolve source
and destination paths through `path_jail::validate_workspace_path`. Validate
the final move destination as well as its directory before mutating files.

**Why.** The previous helper only normalized `..` lexically, and rename/move
destinations bypassed it. That allowed malformed or symlinked paths to escape
the configured workspace. The canonical jail is already the shared Rust rule
for agent file writes, so using it here closes the same boundary for the IDE
without adding a second path policy.

**Proof.** IDE path safety tests refuse traversal, accept new and existing
workspace paths, and on Unix refuse a symlink escape. The focused Windows test
command passed. P4-18 still governs agent/process mutation paths and remains
open.

## ADR-LOG-RESTRUCT-0089 — Run project-command corpora on the platform matrix

**Status.** Accepted on 2026-09-28 to make TG4-18's existing platform-triple
requirement executable in CI.

**Choice.** Keep the checked-in Rust/Node/Python fixtures under `arcana-core`
tests and run their supported build/test/lint/run intents through the
production resolver and process launcher in the existing Windows/Linux/macOS
CI build matrix. CI installs the Python test/lint tools required by those
fixtures; runtime product flows continue to require explicit install intent.

**Why.** TG4-18 already requires these three project corpora on every OS, but
the corpora were absent and the matrix only built Arcana. Running focused
behavior tests in that matrix makes OS-specific command and shim failures
blocking without adding a separate CI architecture.

**Proof.** The exact corpus test passes on this Windows host, including real
Rust, npm, and Python commands through Arcana's launcher. Linux/macOS execution
remains pending CI, and TG4-18 remains open until those results and worker-path
evidence are recorded.

## ADR-LOG-RESTRUCT-0090 — Validate persisted custom shell guidance

**Status.** Accepted on 2026-09-28.

**Choice.** Apply the platform portability validator to every persisted recipe
and skill when parsed/loaded, and to shell hooks when saved and again before
execution. This includes user-authored content; invalid existing content is
reported and excluded from active execution.

**Why.** A check limited to shipped defaults leaves user-customized fleet
instructions and legacy hook rows able to reintroduce POSIX-only commands.
Validation at the production reader blocks old saved data as well as new writes.

**Proof.** Focused tests exercise custom recipe parse/save and skill loader
paths, plus a legacy-hook execution refusal before spawning. TG4-17 remains open
until the full corpus and shipped-content suites are rerun.

## ADR-LOG-RESTRUCT-0091 — Contain and approve free-form Bash writes

**Status.** Accepted on 2026-09-28.

**Choice.** P4-18 must add OS-level write containment for autonomous processes
and require an approval step for free-form Bash commands that may write. The
existing Arcana-managed artifact path checks remain in place.

**Why.** Restricting writes only to built-in artifact tools would remove useful
project workflows. Process containment and command approval preserve general
Bash capability while establishing a host-owned boundary.

**Proof.** `autonomous_bash_requires_permission_before_shell_execution` drives
an Autonomous-mode Bash write through `ToolExecutor::execute_batch`, observes
the actual pending permission, denies it, verifies the returned tool error, and
proves the marker file was never created and the permission entry was drained.
This closes the per-call approval behavior proof; OS sandbox implementation,
capability probes, and process-path workspace proofs remain outstanding.

## ADR-LOG-RESTRUCT-0092 — Enforce QC build evidence through TasksDb

**Status.** Accepted on 2026-09-28.

**Choice.** SQLite enforces the latest task-scoped, attributable, passing
non-smoke build evidence on every transition into `qc_review`. The shared
`TasksDb` transition API keeps the same validation for a useful error message;
the trigger closes direct-SQL bypasses as defense in depth.

**Why.** The owner selected the SQLite transition boundary so no writer can
bypass the rule. Keeping the shared API check gives normal callers actionable
errors before SQLite rejects the write.

**Proof.** `shared_task_status_api_rejects_qc_review_without_latest_attributable_passing_build`
rejects missing, unattributable, and latest-failing evidence through both
TasksDb and direct SQL, and accepts the latest attributable pass.
`stranded_task_reconciliation_cannot_bypass_qc_build_evidence` exercises the
real RecipeRunner reconciler with refusal and passing cases;
`tauri_status_command_cannot_promote_without_build_evidence` proves the UI
command surfaces the shared DB refusal. P4-14 is closed. TG4-07 remains open
for real process-exit coverage and cross-platform worker proof.

## ADR-LOG-RESTRUCT-0093 — Define the build failure streak

**Status.** Accepted on 2026-09-28.

**Choice.** P4-15 counts every observed nonzero exit from each Rust-spawned
build command toward the three-failure diagnostic threshold, including multiple
or concurrent failed commands. Any observed success resets the consecutive
streak. Spawn failures and timeouts remain separately surfaced and do not
invent an exit result.

**Why.** The threshold must reflect the process result Arcana actually
observed, not an inferred task failure or an unstarted process.

**Proof.** The failure-streak tests must cover consecutive nonzero exits,
success reset, spawn failure, and timeout behavior.

## ADR-LOG-RESTRUCT-0094 — Retain Bash; require native QC evidence

**Status.** Accepted on 2026-09-28.

**Choice.** Keep general Bash available to workers. Only successful
Rust-owned `project_command` build evidence can authorize `qc_review`.

**Why.** Bash remains useful for project operations outside Arcana's supported
deterministic command intents, but model-authored shell output must not certify
build success.

**Proof.** Tests must show Bash execution alone cannot create acceptable QC
evidence, while the production project-command path can.

## ADR-LOG-RESTRUCT-0095 — Reconcile ACP state before replay

**Status.** Accepted on 2026-09-28.

**Choice.** Use the existing message/task bounds for ACP storm control. Before
retrying a replayed message, reconcile current state and its prior effect so
delivery remains at-least-once until acknowledgement without duplicating a
completed effect.

**Why.** A stable message ID identifies replay, while reconciliation protects
the consumer's domain state from repeating already-completed work.

**Proof.** The production restart/recovery test must replay the same ID and
demonstrate one domain effect before acknowledgement; current bounds and their
enforcement remain part of TG4-10 evidence.

## ADR-LOG-RESTRUCT-0096 — Keep explicit manual release; CAS automatic release

**Status.** Accepted on 2026-09-28.

**Choice.** Both automatic orphan recovery and the explicit UI/manual release
must compare-and-set against the observed task status, owner, PID, host, and
updated timestamp. SQLite triggers enforce the claim metadata invariant for all
writers.

**Why.** Any delayed release based only on task ID can clear a replacement
claim. Manual intent does not make a stale UI snapshot safe, and direct SQL
writers must not be able to manufacture or retain mismatched claim metadata.

**Proof.** Tests must race a replacement claim against both release paths,
prove stale observations fail, prove a current snapshot releases, and exercise
the SQLite triggers against direct SQL.

## ADR-LOG-RESTRUCT-0097 — Associate checkpoints with task ownership

**Status.** Accepted on 2026-09-28.

**Choice.** Add a nullable `task_id` to the existing config.db checkpoint
records. Preserve session and reason metadata; worker snapshots record the
claimed task ID while interactive session checkpoints continue to use NULL.

**Why.** A shared fleet branch needs per-task reversibility without inventing a
second checkpoint store or replacing useful session history.

**Proof.** The additive migration and production write/list path must show a
task checkpoint remains associated after reopening config.db, while legacy
session-only rows remain readable.

## ADR-LOG-RESTRUCT-0098 — Activate task budgets after atomic claim

**Status.** Accepted on 2026-09-28.

**Choice.** After a successful claim, load saved task caps and usage into the
current AgentEngine run. Enforce the stricter of the task caps and the existing
worker run cap, then persist cumulative usage from subsequent turns.

**Why.** The engine is constructed before the model selects and claims a task;
binding budgets earlier would either enforce the wrong task's saved cap or
require rebuilding the live engine.

**Proof.** A production run test must claim a budgeted task, demonstrate its cap
blocks the next provider turn at the configured limit, and verify usage updates
the claimed task row.

## ADR-LOG-RESTRUCT-0099 — Make P4-32 routing native and auditable

**Status.** Accepted on 2026-09-28.

**Choice.** Rust chooses among runnable workers using task signals and routing
history. Persist explicit assignment overrides and their reason in
`routing_history`; exclude overridden outcomes from automatic learning.
Runnable states are `idle`, `active`, `idle_no_provider`, and `error`.

**Why.** Worker selection is deterministic orchestration that should not be
delegated to model prose. Explicit operator decisions remain auditable without
teaching the router that an exception was a normal match.

**Proof.** Migration 8 stores automatic/override provenance and reasons.
`automatic_batch_dispatch_uses_recorded_outcomes_without_model_worker_choice`
drives the real batch tool with six persisted automatic outcomes and verifies
the selected worker changes; `autonomous_completion_closes_only_the_current_owners_routing_row`
proves the status-tool outcome belongs to the actual prior owner;
`routing_overrides_are_audited_and_never_enter_automatic_learning` proves
reasons persist and overrides do not train the scorer; and
`automatic_dispatch_uses_outcomes_and_only_runnable_workers` proves only
runnable implementation workers enter selection. All focused tests pass.

## ADR-LOG-RESTRUCT-0100 — Require a real desktop-to-fleet coding smoke

**Status.** Accepted on 2026-09-28.

**Choice.** P4 cannot close until Arcana is launched and the user-facing PA chat
delegates a coding task to the fleet that creates a small Google-Calendar-like
application under `C:\Playground`, with frontend, backend, and working wiring.

**Why.** Unit and scripted tests cannot establish that the shipped desktop
workflow can delegate real project work and produce an integrated runnable
artifact.

**Proof.** Preserve the actual task, worker execution/evidence, project files,
and a successful runnable smoke check; inspect the resulting frontend and
backend behavior before closing P4. The isolated output folder is
`C:\Playground\arcana-calendar`; other Playground contents remain untouched.

## ADR-LOG-RESTRUCT-0101 — Keep free-form shell consent per invocation

**Status.** Accepted on 2026-09-28 under P4-18/TG4-09.

**Choice.** A session-wide `AllowAlways` grant never bypasses permission for a
`FreeformShell` call. Other tool effects keep the existing session allowance.

**Why.** Bash's effect is specifically defined as requiring review for each
model-authored command because the tool name cannot bound its side effects.
The standing-allow set previously ran before that effect policy and silently
converted an “always allow” response into persistent approval for all later
commands.

**Proof.** `bash_standing_allowance_does_not_skip_per_call_approval` seeds the
standing allowance, drives an autonomous Bash call through
`ToolExecutor::execute_batch`, observes its pending approval, denies it, and
verifies that no marker file was created. The OS process sandbox is a separate
open P4-18 requirement.

## ADR-LOG-RESTRUCT-0103 — Validate executable shell guidance, not prose mentions

**Status.** Accepted on 2026-09-28 under P4-16/TG4-17.

**Choice.** Apply platform checks to executable shell fences, extracted inline
`bash('...')` examples, and saved hook commands. Allow standard file-descriptor
redirection such as `2>&1`, which works in both supported Windows CMD and
POSIX shell execution paths.

**Why.** The first real desktop start refused saved recipes because a global
substring scan treated explanatory text such as “never use `source .venv`” as
an instruction, and classified portable redirection as POSIX-only. Scanning
actual command examples preserves validation while avoiding false refusals.

**Proof.** `guidance_checks_executable_examples_without_rejecting_explanatory_prose`
proves explanatory mentions and portable redirection are accepted, while
fenced or inline executable `/tmp`, `source .venv`, and shell-chaining
examples are refused. Hook tests cover the same redirection and operator
boundary. The desktop reload started four workers and refused three legacy
saved recipes whose actual commands use `;` or `&`; nonportable commands remain
intentionally rejected.

## ADR-LOG-RESTRUCT-0102 — Start the runtime reaper inside Tauri's async runtime

**Status.** Accepted on 2026-09-28 under P4-36.

**Choice.** Tauri's synchronous setup callback enters `tauri::async_runtime`
before calling `runtime_registry::spawn_reaper`.

**Why.** The first real desktop launch panicked because the core reaper calls
`tokio::spawn` but the setup callback is not itself running in a Tokio context.
That prevented Arcana from reaching fleet startup.

**Proof.** After the change, `npx tauri dev` reaches `Running target\\debug\\arcana.exe`,
initializes analytics, and keeps the `Arcana` process/window alive without the
runtime-reactor panic. Worker startup still reports portability failures for
legacy saved recipe content; full desktop fleet smoke remains open.

## ADR-LOG-RESTRUCT-0104 — Make the calendar acceptance observable end to end

**Status.** Recorded on 2026-09-28 as an operationalization of the owner's
P4-49 acceptance metric.

**Choice.** The fleet-built calendar must support creating an event, reading it
back after refresh from backend persistence, editing it, and deleting it. The
acceptance record must identify the frontend request, backend response/storage
path, returned UI state, task/build evidence, and preserve a before/after
fingerprint of `C:\Playground` with every non-target entry unchanged.

**Why.** A page that launches or a frontend-only mock does not prove the
owner-requested frontend, backend, and wiring work together. A small CRUD
scenario supplies an observable and repeatable end-to-end test without
prescribing the implementation stack.

**Proof.** Submit through the Windows desktop PA chat and observe the fleet
create, verify, launch, and inspect the artifact in
`C:\Playground\arcana-calendar`; record event create/read-after-refresh/edit/
delete behavior and the backend round trip. CLI-only, manual, unit-only, or
mock-only results do not satisfy P4-49.

## ADR-LOG-RESTRUCT-0105 — Use Gemini 3.8 Flash for PA and fleet defaults

**Status.** Accepted on 2026-09-28 at the owner's direction.

**Choice.** New PA sessions, newly seeded workers, built-in fleet workers,
QC subagents, and the app's initial Google selection use `gemini-3.8-flash`.
Migrate only the known shipped worker defaults and prior Google default;
preserve custom worker models and explicit alternate PA selections.

**Why.** The owner specified Gemini 3.8 Flash as Arcana's PA and fleet model,
and Arcana's initial supported provider setup is Google/Gemini. Keep this
selection in source and saved configuration rather than relying on a prompt.

**Pricing and model facts.** Google's official model page lists the ID,
1,048,576-token input context, 65,536-token output limit, and thinking levels
`low`, `medium`, and `high`. Google's pricing page lists standard input/output
at $0.75/$3.75 per million through 2026-12-31 and $1.50/$7.50 from 2027-01-01;
Arcana consumes cached-read counts but not cache-write counts from Google's
usage response, so the price row keeps cache writes at zero.
Sources: https://ai.google.dev/gemini-api/docs/models/gemini-3.8-flash and
https://ai.google.dev/gemini-api/docs/pricing.

**Proof.** The Rust/provider catalogue and TypeScript picker agree; the model
has a dated standard-price row; worker migration tests cover all shipped seed
rows and preserve a custom worker model. Live provider access still requires a
key stored in Arcana Settings; `.env` presence alone does not enable desktop
use.

## ADR-LOG-RESTRUCT-0106 — Keep fleet run evidence usable during a busy smoke

**Status.** Accepted on 2026-09-28 under the owner's calendar-run observability
request.

**Choice.** Logs tab refreshes only the newest 3,000 lines, counts appended
lines incrementally, reports refresh latency and file size, prevents overlapping
polls, surfaces read/export errors, and retains a separate complete-file export.
Project commands record task-scoped intent, toolchain, exit code, duration, and
captured output byte counts; worker recipe ticks record worker, session,
outcome, and duration. Never log command arguments or provider secrets.

**Why.** The owner will run the PA-to-fleet calendar acceptance and return its
logs. Complete records and basic duration signals make early failures
diagnosable and give later speed work an observable baseline.

**Proof.** Parser and full-log/tail tests verify namespaced Rust targets, the
bounded live tail, append-aware line counts, and full-file readback. The real
desktop Logs tab remains un-smoked in this environment; the owner-run PA
acceptance will validate the displayed/exported logs.

## ADR-LOG-RESTRUCT-0107 — Make P4 observability usable for fleet acceptance

**Status.** Accepted on 2026-09-28 at the owner's direction; task P4-50 and
TG4-24 are added to the active development suite.

**Choice.** P4 observability has four user-expandable UI sections: readable
Logs, JSON-expandable Structured Events, correlated parent/child Traces with
operation and sub-operation durations, and Metrics. The production path must
cover PA/fleet and worker/task lifecycle, provider/model calls and reported
tokens, tool outcomes, Rust-owned project commands/builds, graph operations,
and MCP connector operations. Records should preserve run/task/session/worker
correlation where known, distinguish success/failure/timeout/cancellation/retry,
show safe result summaries, and exclude credentials, prompts, source contents,
command arguments, and complete model responses. Filter, bounded live reads,
complete selected-run/date export, and measured producer/UI costs are part of
the contract. P4-50 must choose persistence and propagation boundaries from the
existing Rust event, tracing, run, and analytics paths before implementation.

**Why.** Plain log lines and isolated live events do not show what happened
across a PA-to-fleet workflow, how nested operations relate, where time was
spent, or whether provider/tool calls returned expected results. The owner
needs the complete view before performing the final P4-49 calendar acceptance;
the current Logs panel and selected duration breadcrumbs are only partial
starting points.

**Proof.** TG4-24 requires fixture-driven producer-to-persistence-to-UI
readback, correlation and token/result reconciliation, privacy assertions,
expandable JSON and trace trees, four-section behavior, complete export, and a
recorded performance baseline. No implementation is claimed by this decision.

## ADR-LOG-RESTRUCT-0108 — Instrument Arcana locally with the Rust tracing stack

**Status.** Accepted on 2026-09-28 after official documentation research,
under the owner's expansion of P4-50 to memory, SQLite, and CLI preflight.

**Choice.** Keep `tracing` as Arcana's instrumentation API and extend the
existing `tracing-subscriber` with newline-delimited JSON for structured event
capture. Enable rusqlite's SQLite profile callback on production connections
and record only the database category, safe statement class/fingerprint,
duration, and outcome—never SQL text or bound values. Persist telemetry in
Arcana's local data root and derive in-app metrics from those structured,
correlated operation records and the existing provider usage records. Do not
add an OpenTelemetry exporter or collector runtime to this desktop milestone;
keep export compatibility as a later option.

**Why.** Official Rust documentation describes `tracing` as context-aware and
structured, documents async-safe future instrumentation, and provides a JSON
formatter that emits newline-delimited structured events with span context.
OpenTelemetry is a suitable interoperability direction, but the current Rust
documentation marks its traces, metrics, and logs components Beta, while OTLP
export targets a collector (HTTP defaults to localhost:4318). Arcana needs a
local, self-contained view for the calendar run. Rusqlite's `trace` feature
provides SQLite statement-profile durations, which closes a direct visibility
gap without exposing SQL or parameters. The `metrics` crate is a facade whose
recorder/exporter must still be supplied; Arcana's persisted operation records
are already the correct source for per-run UI counts and latency.

**Proof.** Implement and test producer → local persistence → bounded Tauri
reader → four-panel UI, including SQLite statement timing, memory lifecycle,
provider usage/call outcomes, safe JSON, export, and CLI preflight. Research
sources: [`tracing::Span`](https://docs.rs/tracing/latest/tracing/struct.Span.html),
[`tracing-subscriber` JSON](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/format/struct.Json.html),
[rusqlite trace profiling](https://docs.rs/rusqlite/0.31.0/rusqlite/trace/index.html),
[OpenTelemetry Rust status](https://opentelemetry.io/docs/languages/rust/),
and [OTLP exporter configuration](https://docs.rs/opentelemetry-otlp/latest/opentelemetry_otlp/).
Implementation and evidence remain pending.

## ADR-LOG-RESTRUCT-0109 — Keep telemetry safe, correlated, and incrementally readable

**Status.** Accepted on 2026-09-28 during P4-50 implementation. Its
preflight-only CLI limitation is superseded by ADR-LOG-RESTRUCT-0122 after the
owner expanded P4-50 to require an actual production Fleet run from the CLI;
the telemetry schema, privacy, and incremental-read choices remain in force.

**Choice.** Emit typed operational events to a dedicated telemetry target while
keeping the existing human-readable logs. Database profiles persist only the
database alias, fixed statement category, outcome, and elapsed nanoseconds;
SQLite's expanded SQL is discarded because it may contain bound values.
Provider/tool/memory/graph/command events carry only identifiers, outcomes,
counts, token categories, and byte counts. Fleet execution spans provide parent
context where available. Metrics aggregate appended JSONL incrementally and
retain incomplete trailing lines until the next refresh. CLI validation remains
a safe capability preflight: this CLI exposes worker administration but no
production fleet-run command, so it cannot substitute for PA-to-fleet evidence.

**Why.** The telemetry needs enough structure to diagnose fleet latency without
recording prompts, memory, command arguments, tool output, SQL, or credentials.
Re-reading an ever-growing daily file on each panel refresh would add recurring
I/O to the observed system; caching by file size and ingesting only appended
complete records gives bounded incremental refresh work while preserving export
of the full date. The shipped CLI has no runner entry point, and inventing one
would change this phase's runtime contract rather than test the existing path.

**Proof.** Focused producer-to-JSON SQLite test passes with a parameterized
statement and asserts that SQL and the bound value are absent. Metrics fixtures
cover grouped counts, failures, malformed records, partial lines, and append
refresh. Remaining whole-workflow and performance baseline proof belongs to
TG4-24 and the owner-run P4-49 calendar acceptance.

## ADR-LOG-RESTRUCT-0110 — Bound P4 failure retries, retention, and worker authority

**Status.** Accepted on 2026-09-28 while expanding P4 from the owner's new failure-containment goal.

**Choice.** Keep provider stream retries governed by the shipped P3-09 policy (three total attempts, 500 ms then 1 s, no current `Retry-After` consumption), and define worker/agent startup recovery as a separate capped Fibonacci-plus-jitter policy. Startup retries must stop before provider/tool work or other non-idempotent side effects begin; cancellation, task claim, task budget, and daily budget remain single-run scoped. Retain no more than five Arcana log files per stream and no files older than seven days. Enforce task ownership and Supervisor-only routing at Rust production boundaries. A worker may only update its own claimed task; assignment/reassignment remains Supervisor-owned. Any resumed write or command must first reconcile task-owned checkpoint/workspace state.

**Why.** Provider retry is an existing P2 contract and must not be silently widened while adding a distinct worker-start retry. Bounded retention gives the local desktop telemetry a fixed file-count and age ceiling. The P4 security audit found that autonomous workers could mutate another worker's task and that checkpoint coverage could omit pre-claim and subprocess writes; model instructions alone cannot protect those state transitions.

**Proof.** TG4-26 covers startup retry caps, jitter bounds, cancellation, and side-effect safety; TG4-29 covers retention and symlink refusal; TG4-30 covers cross-worker task mutation and security boundaries. P4-53/54 additionally require task-attributed checkpoint recovery and Supervisor-visible reconciliation. Current findings are not closed by this decision; implementation and gates remain required.

## ADR-LOG-RESTRUCT-0111 — Merge permission masks in the AppContainer ACL lease

**Status.** Accepted on 2026-09-29 during P4 security hardening.

**Choice.** Cache AppContainer ACL grants by canonical path and inheritance, but
retain the granted permission mask and widen an existing ACE when a later
consumer requests additional rights. Never let a narrower earlier grant
suppress a later workspace or toolchain grant; do not create duplicate ACEs for
rights already covered.

**Why.** The workspace-scoped sandbox lease intentionally reuses its AppContainer
identity and ACL grants across child launches. The prior cache treated equal
path/inheritance as a complete match and ignored the mask, so a read-only grant
could make a later write-capable request appear successful without applying
the requested rights. This violates the lease's access contract.

**Proof.** `cargo test -p arcana-core --lib
appcontainer_acl_cache_upgrades_permissions_for_an_existing_path --locked`
passes on Windows (1 test). It exercises both read-to-workspace-rights and the
Rustup-shaped traverse-to-read/execute upgrade, and `AccessPolicy` drops the
temporary profile and grants during test cleanup. End-to-end sandbox
performance remains separately unverified.

## ADR-LOG-RESTRUCT-0112 — Keep the canonical active log during same-date pruning

**Status.** Accepted on 2026-09-29 during P4-55 retention hardening.

**Choice.** When recognized legacy and current log names have the same date,
sort the canonical current-day readable and structured files ahead of aliases
before applying the per-stream file cap. Use a stable filename tie-break for
the remaining same-date files.

**Why.** Retention runs after the rolling writers open the current files. A
deterministic preference makes active-file preservation explicit even when a
same-date alias is encountered, and avoids relying on directory enumeration
order. The canonical current names are `arcana.YYYY-MM-DD.log` and
`arcana-events.YYYY-MM-DD.jsonl`.

**Proof.** `policy_keeps_current_files_when_legacy_names_share_the_active_date`
passes on Windows using all supported legacy/current naming variants and a
test-only cap of one; both canonical active names are retained. The production
five-file/seven-day corpus, live writer/export check, and cleanup-failure
startup proof remain open under TG4-29.

## ADR-LOG-RESTRUCT-0113 — Heartbeats update liveness without undoing lifecycle state

**Status.** Accepted on 2026-09-29 during P4-54 recovery hardening.

**Choice.** A worker heartbeat persists `last_seen` but preserves `working`,
`paused`, and `error` status. Newly created and existing idle workers may still
become `active` when a heartbeat arrives. A heartbeat must never clear the scheduler's
persisted marker for an in-progress run or implicitly unpause/recover a worker.

**Why.** Interrupted-run reconciliation relies on the persisted `working`
status. The previous heartbeat SQL replaced every non-deleted status with
`active`, so a heartbeat could hide an interrupted run and could resume a
paused worker without an explicit lifecycle action. Existing created-to-active
behavior is retained.

**Proof.** The focused DB test `a_heartbeat_preserves_working_paused_and_error_lifecycle_states`
asserts status readback and `last_seen` persistence for protected lifecycle
states. The existing `a_heartbeat_is_recorded_and_readable` test preserves the
created-to-active contract. The 2026-09-29 P4-54 implementation now emits
15-second RecipeRunner heartbeats, persists fleet-run/progress/interruption
state, and exposes it through a Supervisor-only reader; see ADR-0115. Two-worker
resume/reassignment and duplicate-effect prevention remain open.

## ADR-LOG-RESTRUCT-0114 — Retry worker setup before entering the agent run

**Status.** Accepted on 2026-09-29 while implementing P4-52.

**Choice.** Retry only RecipeRunner's local preparation boundary: canonical
workspace resolution, provider-key retrieval, provider construction, and
AgentEngine construction. Use at most four attempts, Fibonacci delays from a
200 ms base with bounded jitter, a 2.2 second per-delay cap, and a 2 second
total elapsed cap. Treat missing configuration and unsupported providers as
permanent; retry transient I/O, SQLite busy/locked, keychain errors, and a
startup panic. Emit structured attempt/outcome/delay telemetry without error
text. Keep the ephemeral session ID and runtime lease outside the retry loop,
and call `engine.run` only after preparation succeeds.

**Why.** Retrying after provider/tool work begins can duplicate paid calls or
mutations. Keeping `engine.run` outside the retry boundary gives startup
recovery a clear no-replay limit while leaving task-budget and claim behavior
owned by one actual engine run.

**Proof.** `cargo test -p arcana-core --lib startup_retry_tests --locked`
passes six tests, and `cargo test -p arcana-core --lib recipes::runner::
--locked -- --test-threads=1` passes all 13 RecipeRunner tests on Windows.
The broader group also verifies missing workspace still fails closed before
provider selection. The tests do not yet inject a failure after a production
side effect or prove task claim/budget continuity; TG4-26 remains open for
those proofs.

## ADR-LOG-RESTRUCT-0115 — Reuse the run ledger for Supervisor-visible fleet health

**Status.** Accepted on 2026-09-29 during P4-54.

**Choice.** Keep worker health and operation history in the existing `runs.db`
ledger instead of adding a second worker-health database. Each RecipeRunner
tick creates one fleet run plus a worker-state row, records bounded safe tool
operation labels in a capped history table, and updates heartbeat/run phases.
Scheduler recovery closes those rows only after task claims have been released.
Expose the latest roster/run/progress/checkpoint summary through a Supervisor-
only built-in tool; do not let the health reader perform reassignment itself.
Enforce one active run per worker in SQLite, require stale recovery to prove
the owner PID is dead on the same host, and release only claims matching that
run's PID and host. A checkpoint-store read failure is an explicit `unknown`
state that blocks automated reassignment decisions.

**Why.** `runs.db` already owns durable run identity and process-recovery
semantics. A second store would split lifecycle evidence and create another
cross-database consistency problem. The read-only Supervisor boundary keeps
health evidence separate from the existing validated routing and claim APIs.
Progress labels intentionally exclude prompts, tool arguments, paths, and
outputs; recent events are capped at 500 per run and the reader returns at most
ten per worker.

**Proof.** The schema and ledger round-trip tests, the Supervisor-only tool
readback/refusal/unknown-checkpoint tests, current-process recovery tests, the
shipped Supervisor engine test, and the complete RecipeRunner test group pass
on Windows. Two-worker restart/reassignment, resume after reconciliation, and
exactly-once side-effect behavior remain open under TG4-27/28.

## ADR-LOG-RESTRUCT-0116 — Do not overstate the independent daily spend ceiling

**Status.** Accepted on 2026-09-29 during cross-phase audit reconciliation.

**Choice.** Keep P4-29 focused on activating the saved per-task budget beside
the worker-run cap. Describe P2's daily ceiling as a separate contract and
leave its hard-stop claim unverified until TG2-17 closes; keep TG4-15 open for
the interaction proof.

**Why.** The P1/P2 cross-phase audit still records concurrency and persistence
gaps in the daily-ceiling path. Calling that boundary an independently enforced
hard stop from P4-29 would claim more than the current P2 evidence supports.

**Proof.** The existing `claimed_task_budget_activates_before_the_next_provider_request`
test proves the saved task cap enters AgentEngine after claim and blocks the
next provider request. It does not prove interaction with the daily ceiling;
that distinction is now explicit in P4-29 and TG4-15.

## ADR-LOG-RESTRUCT-0117 — Keep source gates aligned with delegated production paths

**Status.** Accepted on 2026-09-29 during the full-workspace test triage.

**Choice.** Repair the failing source gates to inspect the current production
delegation boundaries: both `execute_batch` partitions must use
`execute_handler`, whose successful result is redacted; worker status and budget
checks must scan past early test-only helper items instead of truncating at the
first `#[cfg(test)]` marker. Preserve the underlying redaction, worker-status,
and autonomous-budget requirements.

**Why.** The 2026-09-29 workspace attempt reached the desktop library target:
132 of its 135 tests passed before the three source-gate failures stopped that
target.
The three failures were caused by source detectors that could not express the
current code shape, not by failing runtime behavior. Production source confirms
the executor helper redacts successful results, the runner writes the expected
status variants after its early test helper, and both worker and subagent
constructors receive budgets. Leaving those detectors stale would block useful
verification while misreporting behavior.

**Proof.** `cargo fmt --all -- --check` passed. Focused `tg8_07`, `tg_p1_06`,
and `tg_p2_03` tests passed. A fresh full workspace run remains pending; no
phase or cross-phase gate is closed by this correction.

## ADR-LOG-RESTRUCT-0118 — Preserve observability correlation and producer contracts

**Status.** Accepted on 2026-09-29 during P4-50 consumer reconciliation.

**Choice.** Aggregate tool-result byte metrics from the production
`output_bytes` field while retaining compatibility with existing
`result_bytes` records. Group trace views by their strongest available stable
correlation identifier (`trace_id`, run, task, session, or call) and allow the
operator to filter by that identifier. Time the cross-tier Memory search and
aggregate stats APIs using fixed labels and count-only result metadata; report
semantic extraction attempts, successful writes, and failures separately.
Install SQLite profiling before connection setup pragmas. MCP observability
must identify the connector and classify reconnect outcomes without logging
endpoint URLs or raw request errors.

**Why.** The independent P4-50 source review found that the Logs metric reader
did not consume the tool executor's byte field, trace paths merged events by
span name across distinct runs, Memory APIs were missing aggregate timers, and
semantic extraction marked failed writes as successful. It also found the
WAL setup pragma ran before SQLite profiling and MCP registration logs exposed
the configured endpoint.

**Proof.** Logs TypeScript checking and focused UI tests pass (5/5) for trace
separation/filtering. Rust tests for metrics aggregation, Memory timings/write
outcomes, production SQLite setup profiling, and MCP outcomes remain pending;
these changes do not close TG4-24 or P4-50.

## ADR-LOG-RESTRUCT-0119 — Keep recovery guidance outside bounded tool-error causes

**Status.** Accepted on 2026-09-29 during P4 failure-path verification.

**Choice.** Preserve the concise, redacted error cause within its 512-character
bound, and provide caller-specific recovery instructions through the separate
bounded `next_step` field. `project_command` uses this field for the third
consecutive observed nonzero build exit, retaining the native-command-only
diagnosis rule even when compiler output fills the cause.

**Why.** The real Windows `project_command` test observed three failed Cargo
build exits, but `ToolResult::error` truncated the serialized cause before the
diagnostic suffix, so the model never received the recovery instruction. This
is a producer-to-agent contract defect, not a missing failure counter.

**Proof.** `bounded_error_keeps_actionable_next_step_separate_from_long_cause`
and the production `third_observed_build_failure_exposes_diagnostic_guidance_on_the_tool_path`
test pass. The latter ran three real Windows-isolated Cargo builds and verified
the diagnostic in parsed JSON `next_step` despite a long compiler cause. This
correction alone does not close TG4-25 or P4-51.

## ADR-LOG-RESTRUCT-0120 — Quarantine interrupted task claims before retry

**Status.** Accepted on 2026-09-29 during P4-53 recovery audit.

**Choice.** When the exact owning worker process is interrupted, move its
in-progress task to `needs_info`, clear only the matching PID/host claim, and
preserve the assigned worker and a `[worker recovery]` reason. Wake the
Supervisor to inspect the task and available health/checkpoint evidence. Do not
return the task to automatic `ready` dispatch until uncertain side effects have
been reconciled. If the current evidence cannot establish safety, leave it held
for PA review.

**Why.** The scheduler previously cleared the exact claim and returned the task
to `ready` immediately after timeout, panic, or cancellation. The next
Supervisor tick could replay a file, command, or other effect whose completion
was not acknowledged. The existing run ledger keeps bounded operation labels,
but it does not yet provide a durable mutation cursor or effect-intent ledger;
requeueing before those records exist is unsafe.

**Proof.** `p4_interrupted_run_quarantines_exact_claim_for_reconciliation`,
`interrupted_worker_run_recovers_claim_and_persisted_worker_status`,
`recovery_work_check_recognizes_supervisor_and_assigned_worker_tasks`,
`panic_and_timeout_inside_guarded_runs_recover_state_and_keep_dispatch_eligible`,
`recovered_supervisor_state_reaches_a_real_recipe_runner_on_the_next_tick`, and
`shipped_supervisor_creates_visible_epic_and_phased_child_through_agent_engine`
pass on 2026-09-29. These prove containment and Supervisor visibility, not
resumable execution or side-effect reconciliation; P4-53 and TG4-27 remain
open until a lost-acknowledgement mutation can be reconciled and resumed without
duplication.

## ADR-LOG-RESTRUCT-0121 — Separate sandbox launch measurement from setup hypotheses

**Status.** Accepted on 2026-09-29 during P4-50 Windows launcher triage.

**Choice.** Record the bounded-output process test as a 430.42-second
end-to-end test measurement. Do not attribute that duration to ACL propagation,
child startup, or pipe capture without stage-level timings. Compare cold launch
with repeated launches under one retained workspace lease before changing ACL
scope or cache lifetime.

**Why.** The current test performs one PowerShell launch and validates draining
and bounded capture for both streams. The code uses a workspace-scoped lease in
production, but this isolated test does not measure a multi-launch retained
lease or separate ACL API timing. A Luna High performance review found ACL
inheritance over toolchain/PATH roots plausible but unmeasured; narrowing grants
could break Cargo, build scripts, linkers, or command shims.

**Proof.** `child_output_is_drained_but_captured_with_a_bounded_tail` passed
1/1 in 430.42 seconds on 2026-09-29. This is a single cold-path observation,
not a comparable baseline and not a measured ACL stage. The staged cold/warm
benchmark remains open; no sandbox permission was weakened.

## ADR-LOG-RESTRUCT-0122 — Share native runtime initialization across desktop and CLI

**Status.** Accepted on 2026-09-29 during P4-50 CLI execution work, following
the owner's explicit selection of a shared host initializer.

**Choice.** Move host-independent Arcana runtime composition into a reusable
initializer in `arcana-core`. Both Tauri and `arcanactl` will use it for shared
state, provider/model hydration, sessions and permissions, subagent and
analytics globals, and the Fleet runner/scheduler. Each host continues to own
its event sink, logging subscriber/output policy, UI-only services, and process
lifetime. Both hosts use the shared redacted readable/JSONL writer while
selecting their own filters. A bounded CLI run disables the desktop's periodic
analytics rollup, and its scheduler exposes a shutdown path that aborts and
joins scheduled loops and one-off event kicks. The CLI Fleet command executes
through the shared production runner/scheduler and provides a task-terminal
completion condition plus a deadline.

**Why.** P4-50/TG4-24 now requires a real production Fleet run without Tauri.
Desktop setup currently owns several mandatory process globals, while the CLI
only installs a null event sink and console tracing. Duplicating those globals
in the CLI would let the two hosts drift and could skip required telemetry or
cost accounting. The owner chose a shared initializer after reviewing this
cross-host ownership change.

**Proof.** Desktop and CLI both compile with the shared initializer; the core
initializer selection tests pass (4/4), structured writer/redaction tests pass
(2/2), and the Tauri log metric aggregation test passes. Focused Logs UI tests
pass (6/6) and TypeScript typecheck passes. The real production CLI Fleet run,
persisted event readback, injected recovery, and end-to-end shutdown proof are
still pending; this ADR does not close P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0123 — Match personal-access-token markers as whole segments

**Status.** Accepted on 2026-09-29 during the production CLI smoke.

**Choice.** Detect `PAT` as a complete environment-variable name segment rather
than using the substring check `_PAT`.

**Why.** The actual CLI startup reported removal of `CODEX_MCP_NODE_PATH`,
`CODEX_TECTONIC_PATH`, and other host path variables because `_PAT` also
matches the prefix of `_PATH`. Those variables are not credentials; removing
them can break tool discovery and made the startup report inaccurate. The
segment match retains `GITHUB_PAT`/`GITLAB_PAT` protection while leaving path
variables intact.

**Proof.** Current CLI smoke exposed four false positives. Added classifier
tests for the exact host variables and positive PAT names; `cargo test -p
arcana-core --lib security::env_scrub::tests --locked` is the required focused
recheck. Secret-shaped variables remain scrubbed, and the CLI smoke is isolated
under a temporary profile/workspace.

**Proof update (2026-09-29).** The focused env scrub suite passes 5/5,
including all four host variables and positive PAT names.

## ADR-LOG-RESTRUCT-0124 — Make persisted workspace consent manageable from CLI

**Status.** Accepted on 2026-09-29 after the owner selected grant/revoke/status
commands for CLI workspace consent.

**Choice.** Add explicit `arcanactl consent grant|revoke|status` commands,
requiring both a workspace path and provider. Resolve the workspace to its
canonical path and use the same `WorkspaceEgressConsentDb` used by desktop
Settings and provider wrappers. `fleet run` requires an explicit working
directory and checks persisted consent before creating a task or starting
workers.

**Why.** A production CLI Fleet smoke correctly failed closed when it attempted
Gemini egress without a matching persisted consent row, but the CLI had no way
to manage that grant. An explicit workspace/provider command surface preserves
the existing consent boundary; an implicit grant based on key presence or a
Fleet invocation would weaken it.

**Proof.** CLI workspace/provider validation test passes. A prior isolated
production CLI call was denied before provider egress and wrote a `denied`
audit row for the canonical workspace. The isolated subprocess test `cargo test -p arcanactl --test consent_cli --locked` passes persisted grant/status/revoke across separate CLI invocations. The denied live CLI attempt recorded a content-free audit row. After consent was granted, Gemini 3.8 Flash reached the Supervisor but exhausted the run budget before epic decomposition; successful Fleet completion/readback remains open.


## ADR-LOG-RESTRUCT-0125 — Bound CLI Fleet Supervisor budget overrides

**Status.** Accepted on 2026-09-29 after the owner selected an explicit per-run
CLI budget override.

**Choice.** `arcanactl fleet run` may override the Supervisor's token, USD, and
turn caps for that invocation. Unspecified axes retain the existing Supervisor
defaults (50,000 tokens, $0.25, 10 turns); the input is validated against the
existing hard ceilings (1,000,000 tokens, $100, 100 turns). The shared runtime
passes the override only to Supervisor engine runs. Worker run caps, saved task
budgets, and the independent daily spend ceiling remain in force.

**Why.** An isolated real Gemini Fleet attempt reached the production
Supervisor but hit its default budget before it decomposed the epic. Raising a
global default would remove the per-run choice and silently increase spend. A
CLI-only bounded override makes the increased allowance explicit while
preserving existing caps elsewhere.

**Proof.** The CLI validates axis-specific overrides and retains defaults for
unspecified fields; focused unit tests cover valid, partial, and over-ceiling
inputs. The RecipeRunner budget selection test proves the override applies to
the Supervisor and does not widen worker caps. A successful provider-backed
Fleet run and its persisted cost readback remain pending.

## ADR-LOG-RESTRUCT-0126 — Bootstrap newly seeded CLI workers before routing

**Status.** Accepted on 2026-09-29 during the isolated production CLI Fleet
smoke.

**Choice.** The CLI Fleet host may start recipe-bound workers in `created`
state. `RecipeScheduler::start_worker` first transitions those workers to
`idle`; task assignment and subsequent worker admission continue to use only
the runnable states (`idle`, `active`, `idle_no_provider`, and `error`).

**Why.** A fresh `WorkersDb` seeds recipe-bound system workers in `created`.
The first isolated `fleet run` correctly excluded them from routing but also
treated them as ineligible for startup, so no worker could ever become
runnable. Desktop startup already starts recipe-bound Created workers. The CLI
needs the same explicit bootstrap step without making `created` itself a
routing state.

**Proof.** The isolated CLI command reached this fail-fast condition before
creating the epic or starting workers. A focused CLI unit test now asserts
Created is startable for bootstrap but remains unroutable; the full CLI rerun
and worker/task readback are pending.

## ADR-LOG-RESTRUCT-0127 — Use the repository `.env` key for Gemini live tests

**Status.** Accepted on 2026-09-29 after the owner clarified that the new
Gemini key in `.env` should be used for current and future tests and authorized
replacing the saved vault value.

**Choice.** Live Gemini provider tests read `GOOGLE_API_KEY` directly from the
repository `.env` and do not fall back to ambient environment variables. The
saved Arcana Google credential was synchronized from that same value so CLI
and desktop production-host runs use the current key. Test output must never
include the secret.

**Why.** A previous CLI provider attempt used the older saved Windows
credential and therefore was not evidence about the user's newly added `.env`
key. The owner explicitly approved updating the vault; keeping the live test
reader pinned to `.env` prevents this drift from recurring.

**Proof.** A temporary sync helper stored the `.env` key through
`VaultManager::store`, read it back for an in-memory equality check, zeroized
both buffers, printed no key material, and was removed. The feature-gated live
models-endpoint test passed using the `.env` reader. The focused Gemini 3.8
stream-signature replay test currently receives repeated Google 503 Overloaded
responses; signature replay remains unproven until the provider accepts the
request.

## ADR-LOG-RESTRUCT-0128 — Canonical UTC timestamp format for new persistence

**Status.** Accepted on 2026-09-29 during P4-57/TG4-19 source inventory.

**Choice.** New persisted timestamps use UTC RFC 3339 with fixed millisecond
precision and a `Z` suffix (`YYYY-MM-DDTHH:mm:ss.sssZ`). Rust writers use one
shared formatter; SQLite defaults, triggers, and direct SQL writers emit the
same representation. Readers use the shared timestamp parser and continue to
accept the older RFC 3339 offsets and offset-less SQLite UTC rows. Do not
rewrite historical row contents solely to normalize their spelling.

**Why.** The P4-57 inventory found writer splits in task/routing, worker/run,
analytics, and configuration data; several readers bypass the shared parser;
and analytics performs string comparisons between RFC 3339 values and
space-separated SQLite cutoffs. Task elapsed displays and FleetChat also hand
offset-less values directly to JavaScript `Date`. A single UTC representation
makes new writes unambiguous, while dual-format readers preserve existing data.

**Proof.** Partial implementation is underway. The shared Rust helper's
leap-year, UTC−12/UTC+0/UTC+14, northern/southern DST-edge, and Rust-vs-SQLite
shape tests pass. Memory/session writers use the shared formatter, and task,
FleetChat, hook, and date-bucket UI cases pass focused tests in
America/New_York and Australia/Sydney. Production DB defaults/triggers,
analytics time-window query behavior, and every database write/read pair still
need focused verification before TG4-19 can close.

## ADR-LOG-RESTRUCT-0129 — Traverse exact ancestors for external toolchains

**Status.** Accepted on 2026-10-01 after the production Windows
`project_command` build test failed inside AppContainer.

**Choice.** When a child needs a read/execute toolchain root outside the
selected workspace, Arcana grants non-inheriting `FILE_TRAVERSE` on only that
root's exact ancestors that are outside the workspace, excluding filesystem
roots and OS-managed directories. Paths within `USERPROFILE` remain bounded to
that profile. The toolchain root itself retains the existing inheritable
read/execute grant; ancestor traversal never grants reading, listing, or
writing.

**Why.** The fixture built successfully outside AppContainer, but the real
production launcher failed when Cargo invoked `link.exe` from `C:\PCI
Geomatics`. The launcher granted read/execute at PATH roots and ancestor
traversal only beneath `USERPROFILE`, so the external linker directory could
not be reached. The fix preserves the user's OS-write-sandbox decision while
granting the minimum ancestor capability needed to execute an already-selected
toolchain binary.

**Proof.** `external_toolchain_traversal_stays_on_the_selected_path_and_profile_paths_stay_scoped`
passes its positive/negative path-shape assertions. The failed production
`project_command` test took 229.40 seconds and confirms the host fixture builds
outside the sandbox. Re-run the same production test and the checked-in
ecosystem corpus before treating external toolchain execution or TG4-09 as
proven. Keep the old abandoned temporary fixtures and user-profile AppContainer
ACEs untouched until each is confidently attributed to a completed or
interrupted test run.

## ADR-LOG-RESTRUCT-0130 — Keep P4 observability local-first on `tracing`

**Status.** Accepted on 2026-10-01 after reviewing the P4 observability
implementation and official Rust observability documentation.

**Choice.** Keep Arcana's in-process instrumentation on the existing `tracing`,
`tracing-subscriber`, and `tracing-appender` stack, with privacy-safe JSONL as
the durable source consumed by Arcana's Logs UI. Do not add OpenTelemetry SDK
or OTLP exporter dependencies for P4. Reconsider an OTLP bridge only when an
external collector/export destination is an explicit product requirement and
its egress, failure-isolation, and local-retention behavior have an owner.

**Why.** P4 requires a local four-panel Logs workflow and export that works
without a collector, account, or network. The current `tracing` stack already
provides structured spans/events, JSON formatting, filtering, and rolling
writers used by that reader. Official OpenTelemetry Rust documentation
currently labels traces, metrics, and logs Beta; the OTLP exporter sends data
to a configured collector/backend. Adding it now would create another output
path and egress/configuration surface without replacing the app-specific JSONL
reader or satisfying a stated destination requirement. Sources reviewed:
[OpenTelemetry Rust status](https://opentelemetry.io/docs/languages/rust/) and
[OpenTelemetry OTLP exporter](https://docs.rs/opentelemetry-otlp/latest/opentelemetry_otlp/).

**Proof.** Current `tracing-subscriber` supports per-layer filtering and JSON
output, and `tracing-appender` supplies the existing rolling local sink.
Focused structured-writer, fast-telemetry-filter, SQLite-profiling, and
compaction-call tests pass on 2026-10-01. End-to-end producer-to-UI proof,
telemetry failure isolation, and cost measurements remain open under P4-50 /
TG4-24.

## ADR-LOG-RESTRUCT-0131 — Grant path metadata for sandbox canonicalization

**Status.** Accepted on 2026-10-01 after reproducing the production Clippy
failure with a Rust compiler wrapper inside the Windows AppContainer.

**Choice.** Grant non-inheriting `FILE_READ_ATTRIBUTES | FILE_TRAVERSE` on the
exact ancestor directories required to resolve the selected workspace and
selected toolchain paths. Keep read/write/execute access on the workspace and
the existing narrowly selected toolchain roots. Ancestors do not receive file
contents, directory-listing, or write access through this grant.

**Why.** The production `cargo clippy` command failed with Windows
`ERROR_ACCESS_DENIED` while canonicalizing `CLIPPY_CONF_DIR` and its config
file, even though a sandboxed `cmd.exe type` could read the file. A Rust
compiler wrapper inside the same AppContainer reproduced the denial on both
paths. Moving the fallback config from `.arcana-cache` to the workspace root
did not fix it and was reverted. The Windows workspace traversal grant only
contained `FILE_TRAVERSE`; canonicalization also requires metadata access on
ancestor directories. The added ACE is non-inheriting, limited to the exact
path chain, and does not permit reading ancestor directory contents.

**Proof.** The strengthened `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox`
test records config-directory and config-file canonicalization from a
Rustc-wrapper process, then runs the actual fixed-argv Clippy intent. The
updated Windows ACL scope test asserts the ancestor mask includes only path
metadata rights. Both checks must pass before P4-11/TG4-18 can advance; Linux
and macOS corpus results remain separate requirements.

## ADR-LOG-RESTRUCT-0132 — Extend metadata grants through the selected path chain

**Status.** Decision recorded on 2026-10-01; production proof pending.

**Correction to ADR-LOG-RESTRUCT-0131.** The first metadata grant did not
restore Clippy. A sandboxed Rustc wrapper still received `Access is denied`
from `canonicalize()` for both the config directory and config file, while its
`metadata()` call succeeded. Moving the fallback config to the workspace root
also failed. The initial ancestor grant stopped at the current user profile,
so it did not yet cover the full exact path chain (for example,
`C:\Users\razia\...` also traverses `C:\Users`).

**Choice.** Apply the same non-inheriting traverse-plus-metadata ACE to each
non-OS-managed ancestor on the selected workspace or toolchain path, ending
before the volume root. Keep the grant limited to path metadata; do not grant
directory listing, file content, or write access to those ancestors.

**Why.** The user-selected workspace may be outside the user profile, such as
`C:\Playground\arcana-calendar`, and the sandbox must resolve that path for
Rust tools without granting it content access to unrelated directories. The
profile-only rule was too narrow for Windows path canonicalization.

**Proof.** The Windows scope test now checks the exact parent chain for both
the Rust toolchain and `C:\Playground\arcana-calendar`. The production Clippy
probe asserts Rust-level path metadata/readback and runs Clippy through the
AppContainer. Keep this decision and P4-11/TG4-18 open unless those Windows
tests pass; Linux/macOS remain separate.

## ADR-LOG-RESTRUCT-0133 — Deny autonomous child network and external project reads

**Status.** Accepted on 2026-10-01 by the owner for P4-61.

**Choice.** Sandboxed autonomous commands have no network access and cannot
read arbitrary files outside their selected workspace. The host may expose
only exact selected toolchain/runtime files or directories required to launch
the chosen compiler/interpreter. Locked project dependency sources needed for
the command are staged under the workspace before child launch. A missing
dependency fails closed and requires a separate explicit host-controlled
install/consent action; the child is never granted network access to fetch it.

**Why.** The owner expanded P4-18 after the cross-phase security audit found
that build/test children could inspect unrelated files and use outbound
network. Preserving broad Cargo registry/git cache access would retain that
external-read path. Platform-specific write-only confinement also did not
enforce the requested read/network boundary.

**Proof.** P4-61 / TG4-31 must demonstrate workspace read/write and exact
runtime operation, external user-file read denial, network-denial against a
controlled local endpoint, dependency staging from a locked fixture, and
fail-closed behavior when a locked dependency is unavailable on Windows,
Linux, and macOS. The real P4 launcher corpus must continue to pass under this
contract. No P4 security task is closed by this decision record.

## ADR-LOG-RESTRUCT-0134 — Correct the prior worker-timeout baseline and measure command stages

**Status.** Accepted on 2026-10-01 as a correction and P4-62 implementation record.

**Correction.** The P4-06 notes and TG4-02 reference a 600-second production
push timeout. Inspection of the committed scheduler at `HEAD` shows that push
and cron paths directly awaited `runner.run_recipe` without a production
timeout wrapper. The 600-second statement is retracted as a source claim;
the existing timeout-helper tests do not prove that production deadline.

**Choice.** `project_command` uses a one-hour default and a two-hour per-call
maximum; worker scheduler paths use a two-hour hard deadline. Keep the CLI
epic deadline finite. Time command preparation, child process wait, output
drain, and evidence persistence as separate correlated events. Use virtual
time for a long-running scheduler/command-future test and a short real child
test for process-tree cancellation instead of waiting eleven wall-clock
minutes.

**Why.** The owner asked to increase the capability's time after receiving a
source-independent analysis. Bounded multi-hour values allow long project
verification while preserving an explicit escape deadline. The analysis's
544-second versus 0.81-second ratio lacks matched command, cache, hardware,
platform, and timing boundaries and is not evidence of a compiler slowdown.
Existing 430.42-second and 399.87-second Windows observations are end-to-end
test durations without stage attribution.

**Proof.** The timeout clamp test passes for the one-hour default, one-second
override, two-hour maximum, and over-limit clamping. The production scheduler
constant and wrapper are set to two hours. Remaining evidence is P4-62/TG4-32:
exercise production timeout and reconciliation, cancel a real process tree,
run a paused-clock scenario beyond ten minutes, record the new correlated
stage timings, and compare equivalent cold/warm fixtures. The baseline
correction does not close that gate.

## ADR-LOG-RESTRUCT-0135 — Require per-call host approval for dependency installation

**Status.** Accepted on 2026-10-01 by the owner.

**Choice.** `project_command(intent="install")` requires an interactive,
per-call host approval in every permission mode, including Autonomous. It
cannot be authorized by a standing tool allowance. A headless host with no
approval responder fails closed; no child network capability is granted to
build/test commands.

**Why.** A worker-authored install request is not host consent. Package
installation can fetch untrusted code and execute package lifecycle/build
scripts, so it needs a visible one-call approval even when ordinary autonomous
project verification is allowed.

**Proof.** The approval path uses the shared typed effect policy and existing
permission event, carries the exact install intent to the host UI, and hides
the “Allow Always” option. Focused tests must prove Autonomous mode prompts
despite a saved `project_command` allowance and that denial prevents the
install handler from running. A separate host-mediated staging path and the
cross-platform read/network sandbox remain required by P4-61/TG4-31; approval
alone does not close them.

## ADR-LOG-RESTRUCT-0136 — Deny installs until locked dependency staging is implemented

**Status.** Adopted on 2026-10-01 to enforce the already accepted P4-61
security contract while staging is incomplete.

**Choice.** After the per-call host approval, `project_command(intent="install")`
still returns a structured network-capability denial before process launch
until Arcana can stage only locked dependencies through a host-controlled
path. Approval does not authorize a network-capable untrusted child.

**Why.** Linux currently applies write-only Landlock rules, macOS still allows
default reads/network, and Windows has not yet proved exact runtime/dependency
staging. Launching an install process on any of those platforms would violate
the accepted no-child-network policy or rely on unproved confinement.

**Proof.** A focused tool test parses the structured denial and verifies the
capability, deny policy, safe command identity, non-retryable state, and
host-staging next step. The headless CLI and desktop permission tests remain
separate; cross-platform staging and isolation proof is still required before
this guard can be removed.

## ADR-LOG-RESTRUCT-0137 — Return source-free structured project diagnostics

**Status.** Accepted on 2026-10-02 by the owner.

**Choice.** Failed `project_command` calls return bounded diagnostic codes and
workspace-relative file/line/column locations when these can be extracted
safely. They do not forward compiler messages, source snippets, raw commands,
or absolute paths to the model-facing error envelope. Unknown formats fall back
to a generic stable diagnostic code.

**Why.** The Supervisor and worker need actionable failure information without
turning untrusted compiler output into an unrestricted prompt/source disclosure
path. Raw output can contain source text, credentials, or attacker-controlled
instructions.

**Proof.** Focused parser and ToolExecutor tests must prove useful compiler
codes/locations survive while planted source, credentials, and outside-workspace
paths do not. This does not claim compiler-message fidelity; maintainers should
extend only the source-free parsers needed by supported toolchains.

## ADR-LOG-RESTRUCT-0138 — Grant selected Windows config ancestor traversal only

**Status.** Accepted on 2026-10-02 by the owner.

**Choice.** For a validated Clippy configuration directory inside the selected
workspace, Windows may add temporary, non-inheriting `FILE_TRAVERSE` and
`FILE_READ_ATTRIBUTES` grants to that directory and its in-workspace ancestors.
Exact file grants remain in place. Grants are revoked with the AppContainer
lease; no listing, content-read, write, or execute capability is added by this
ancestor allowance.

**Why.** Clippy reads a selected workspace config but fails while canonicalizing
its path inside AppContainer. The narrow path-metadata grant permits that
operation without widening reads to sibling files or unrelated user paths.

**Proof.** The Windows tests inspect exact ACL masks and inheritance, run the
Clippy corpus, and use an AppContainer probe to deny synthetic sibling reads
and loopback access while retaining workspace access. Linux/macOS remain
separate target-platform proof.

## ADR-LOG-RESTRUCT-0139 — Define deterministic Python build evidence

**Status.** Accepted on 2026-10-02 by the owner.

**Choice.** Python `project_command(intent="build")` runs
`python -m compileall -q` over the selected project, directing bytecode into
Arcana's workspace-local cache. Python test and lint intents remain separate;
passing tests provide additional verification confidence but do not replace
the deterministic build evidence needed for QC.

**Why.** Python has no universal package build command, while P4's QC contract
requires a Rust-owned observed build result for each supported ecosystem. A
standard-library syntax compilation gives Python projects a bounded,
dependency-free build operation with an observable exit code.

**Proof.** Add production launcher fixtures for successful and failing
compilation, confirm cache output stays within the selected workspace, and
prove only the Rust-owned observed build result reaches task-scoped QC evidence.

## ADR-LOG-RESTRUCT-0140 — Retain provider replay tokens across repeated stream updates

**Status.** Accepted on 2026-10-02 as an engine-level preservation rule.

**Choice.** When a streamed tool-call part is updated more than once, retain a
provider-issued opaque replay token already observed for that same part if a
later update omits it. A later non-empty token replaces the previous value;
tokens are never logged or interpreted.

**Why.** The Google adapter maps repeated updates to the same response-part
index into the engine's tool-call accumulator. Replacing that accumulator with
an unsigned update can discard metadata required to replay the call. Preserving
provider-issued metadata is safe because the token is opaque and is required to
be echoed unchanged.

**Proof.** `repeated_gemini_stream_part_does_not_erase_its_thought_signature`
parses repeated Gemini SSE parts, passes them through the production engine
stream assembler, then verifies the real Gemini request builder carries the
signature into the assistant part alongside its function response. The live
CLI retry after this fix ended on provider `503 UNAVAILABLE` before tool
execution, so the networked confirmation remains pending.

## ADR-LOG-RESTRUCT-0141 — Correct the Python build command description

**Status.** Correction recorded on 2026-10-02.

**Correction.** ADR-LOG-RESTRUCT-0139 omitted two execution details. The live
resolver runs `python -S -m compileall -q -x <generated-tree-exclusion-regex>
.`, where `-S` disables site initialization and the workspace-local bytecode
prefix is set by the platform launcher. Python Test remains the separate
`python -m pytest` intent. The prior ADR remains correct about the selected
stdlib build/test split and is corrected here rather than rewritten.

**Proof.** `python_build_is_standard_library_compilation_and_keeps_tests_separate`
asserts the exact argv and `project_command` sets `PYTHONPYCACHEPREFIX` beneath
the workspace cache directory.

## ADR-LOG-RESTRUCT-0142 — [RETRACTED] Resolve selected Windows paths through exact ancestors

**Status.** Retracted on 2026-10-02 after the isolated ACL test demonstrated
unacceptable cost and cleanup risk.

**Retracted choice.** ~~A selected workspace-contained Clippy configuration may receive
temporary `FILE_TRAVERSE | FILE_READ_ATTRIBUTES` grants on each existing
directory in the exact absolute path chain, including parents above the
workspace. Grants do not inherit and add no listing, content read, write, or
execute capability. Exact config-directory/file grants remain workspace-bound.~~

**Why this was retracted.** Restricting traversal to the workspace root did not let Windows
canonicalize the absolute config path; the operation still traverses parent
components before reaching the selected file. The prior production Clippy test
proved readable bytes were insufficient. Exact path metadata access preserves
the approved least-privilege contract while allowing canonicalization to be
tested in theory, but the experiment below showed it is not operationally safe
on this host.

**Failed proof.** `selected_path_ancestors_receive_only_non_inheriting_traversal_grants`
ran for over 60 seconds at high CPU while applying grants above the workspace.
The interrupted test required explicit cleanup of its unique ACEs and
AppContainer profile. No grant remains, but that cost and interruption risk
make this path unsuitable.

## ADR-LOG-RESTRUCT-0143 — Keep Windows traversal grants inside the workspace

**Status.** Accepted as the operational rollback on 2026-10-02.

**Choice.** Keep Windows config ancestor traversal limited to the selected
workspace tree. Do not modify ACLs on protected user-profile or system
ancestors to force Clippy path canonicalization. Clippy remains blocked until
a path-resolution design can pass without expensive outer-ancestor DACL writes.

**Why.** The broader experiment sustained high CPU, touched AppData/Temp
ancestor ACLs, and was interrupted before normal RAII cleanup. It is less safe
and less reliable than leaving Clippy unproven while preserving the narrow
workspace security boundary.

**Proof.** The unique AppContainer SID was removed from the test workspace,
Temp, Local, AppData, and profile paths; Windows `DeleteAppContainerProfile`
returned success and its mapping is absent. No matching ACE remains in the
exact inspected paths. Do not rerun the broader ACL test.

## ADR-LOG-RESTRUCT-0144 — Keep absent task IDs out of correlation buckets

**Status.** Accepted on 2026-10-02 as a telemetry serialization correction.

**Choice.** Serialize an absent task correlation as an empty string in
structured telemetry, and treat it as unmatched by task-ID searches. Keep
present task IDs as canonical decimal strings. Apply the same representation
to operation timing, provider/engine events, and project-command lifecycle
events.

**Why.** A synthetic task ID of `0` makes unrelated taskless PA/project-command
operations appear to share one task in trace grouping and correlation reads.
The structured event contract already uses empty strings for unavailable
correlation fields; the shared helper now makes task IDs follow that convention.

**Proof.** Focused tests verify timer serialization and the Logs matcher for
absent versus real task IDs. Current production project-command events use the
shared helper, and spawn-error events carry their tool-call ID. Metrics,
structured-stream listing, and quiet-filter regressions are also covered by
focused CLI/Tauri tests. This does not prove the complete PA/Fleet producer to
Logs UI path; P4-50 remains open.

## ADR-LOG-RESTRUCT-0145 — Test worker deadlines with paused Tokio time

**Status.** Accepted on 2026-10-02 as a focused test-infrastructure choice.

**Choice.** Enable Tokio's existing `test-util` feature only for `arcana-core`
tests and use its paused clock to exercise the production worker timeout
wrapper without waiting in real time.

**Why.** P4-62 explicitly requires proof that an operation longer than ten
minutes completes before the configured two-hour worker cap. Real sleeps would
make this regression slow and unreliable; the production code already uses
Tokio time, so its test clock proves the same timeout boundary deterministically.

**Proof.** `production_worker_deadline_allows_work_past_ten_minutes` calls the
production `run_worker_future` with `PUSH_RUN_TIMEOUT`, advances a simulated
10m+1s operation, and asserts successful completion and the 7200-second cap.
Process-tree cancellation/reconciliation and stage-timing performance evidence
remain open.

## ADR-LOG-RESTRUCT-0146 — Normalize Fleet correlation and structured log discovery

**Status.** Accepted on 2026-10-02 as a Logs consumer consistency fix.

**Choice.** Treat `correlation_id` and `fleet_correlation_id` as one Fleet
trace group; discover desktop log dates from valid readable `.log` and
structured `.jsonl` files; and include the backend `failure` outcome in the
Metrics failed-outcomes headline.

**Why.** The CLI Fleet lifecycle and workers emit different field names for
one UUID, so grouping by field name split one run into separate trace roots.
A structured-only day was also inaccessible from the desktop date selector,
and the failure headline omitted a backend-supported outcome. Normalizing
these consumer contracts keeps a single Fleet view and reconciled metrics.

**Proof.** Logs UI tests cover CLI/worker group unification and `failure`
headline totals; a Tauri command helper test covers JSONL-only date discovery.
The focused UI suite passes 9/9 and TypeScript typecheck passes. Production
producer-to-UI proof and statement-level SQLite outcome instrumentation remain
open under P4-50.
## ADR-LOG-RESTRUCT-0147 — Guarantee the P4-51 suggested-action field

**Status.** Accepted on 2026-10-02 during P4-51 contract audit.

**Context.** P4-51 and TG4-25 require every production tool failure envelope to
include an actionable `suggested_action`. Generic `ToolResult::error` values
carry an actionable `next_step`, but `with_execution_context` did not project
that value into the required field. Only handlers that already emitted
`suggested_action` satisfied the full envelope contract.

**Decision.** At the shared executor enrichment boundary, preserve an existing
`suggested_action`; otherwise copy the already-redacted, bounded `next_step`;
if neither is present, use the existing conservative default recovery guidance.
Keep `next_step` for compatibility. Do not infer that an operation is safe to
retry from the presence of guidance.

**Reason.** A single result boundary covers generic, categorized, permission,
timeout, panic, and sandbox failures before they reach the model, persistence,
or UI. This additive field implements the named plan contract without changing
the result's failure state or weakening retry safety.

**Proof.** `cargo test -p arcana-core --lib executor_context_ --locked` passes
five focused tests. `executor_context_promotes_generic_next_step_to_suggested_action`
asserts generic `next_step` becomes `suggested_action`, while
`executor_context_enriches_failures_without_changing_successes` verifies the
sandbox handler-authored action remains intact. Full TG4-25 coverage remains
open.

## ADR-LOG-RESTRUCT-0148 — Retract an unsafe assumption in the PR-diff consent test

**Status.** Correction recorded on 2026-10-02; the keychain-backed test path was
removed before accepting evidence.

**Retracted claim.** An isolated `HOME`/`USERPROFILE` made the matching-consent
test provider-key-free and prevented network access. This was false on Windows:
home isolation does not isolate the OS credential manager. The first test run
failed its expected missing-key assertion after granting consent to a temporary
workspace. It may have retrieved the stored Google key and issued one provider
request with the synthetic test diff; local output does not establish whether
the request reached Google or was billed. No repeated attempt was made.

**Correction.** The PR-diff helper now accepts a private post-consent analyzer
callback. Production still builds the capped `ConsentProvider`, which rechecks
persisted consent immediately before egress. The regression supplies a local
structural callback and proves matching consent reaches that callback; it never
reads the vault or constructs a live provider. The unconsented test asserts the
callback is not invoked.

**Proof.** `cargo test -p arcana --lib commands::diff_commands::tests --locked`
passes all three tests after the correction. This proves the Tauri PR-diff
decision and audit callback path, not a live provider request or the desktop UI
workflow; P4-60/TG4-16 remain open.

## ADR-LOG-RESTRUCT-0149 — Correct the P4-55 desktop-retention status

**Status.** Correction recorded on 2026-10-02 after checking the source entry
point.

**Retracted claim.** The P4-55 checkpoint and follow-on notes said desktop
startup integration was still open. This claim was based on carrying forward
the checkpoint without re-reading the Tauri bootstrap.

**Correction.** `src-tauri/src/main.rs` calls `arcana_lib::logging::init()`
before resolving the canonical `.arcana/logs` path and calling
`arcana_lib::observability::log_retention::prune()`. Cleanup is best-effort and
occurs after logging, so a cleanup failure is written to the new daily log and
does not block startup. The P4-55 and TG4-29 records now reflect this existing
production wiring. Isolated execution of desktop startup ordering and
cross-platform symlink behavior remain unproven.

**Proof.** Source bodies at the Tauri entry point show the call ordering; the
core retention suite and captured cleanup-failure test pass. This corrects the
integration status but does not close P4-55/TG4-29.

## ADR-LOG-RESTRUCT-0150 — Persist task assignment before worker dispatch

**Status.** Applied on 2026-10-02 to close the production ordering defect
identified in the P4 recovery audit.

**Decision.** The Tauri task-status command persists the status and owner
transition before issuing a synchronous scheduler kick. ACP results describe
inbox persistence and pending acknowledgement; a stored message is not reported
as consumed merely because Arcana requested a worker tick.

**Reason.** `RecipeScheduler::kick_worker` checks recoverable work immediately.
Kicking before the task row changes can observe stale state and defer work to
the five-minute reconciliation sweep. ACP messages are delivered at-least-once
until acknowledgement and may remain queued when no recipe consumer is active.

**Proof.** `ui_assignment_persists_before_recipe_runner_dispatch` observes the
assigned task during the kick and reaches the real `RecipeRunner` no-provider
boundary using an isolated workspace and no provider configuration. The
production `send_worker_message` ToolExecutor test persists a message to an
unbound worker, reads the stable inbox row, and proves acknowledgement removes
it from replay. Full ACP interruption-side-effect idempotency and the live
Calendar Fleet run remain open.

## ADR-LOG-RESTRUCT-0151 — Gate query-plan assertions on representative rows

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** The worker claim and latest worker-tick SQL are named constants
shared by their production methods and query-plan regressions. Tests populate
representative rows and run `ANALYZE` before asserting indexed access. For task
claim lookup, either the status or assignment index is valid; the test records
SQLite's observed choice instead of imposing an unmeasured composite index.

**Reason.** Index selection depends on data distribution. A test should reject
a table scan while allowing the planner to choose among applicable indexes;
adding a composite index requires a workload measurement that demonstrates a
benefit. The complete production query inventory remains a P4-58 obligation.

**Proof.** The populated daily-cost fixture shows the date primary-key index;
task fixtures show `idx_tasks_status` for both claim lookup and orphan-candidate
selection; the fleet-run fixture selects `idx_worker_run_state_worker` for
latest worker state. Each test rejects a primary-table scan. These are four
query proofs, not complete gate evidence. The full working list is
`_verify/P4-58-QUERY-INVENTORY.md`.

## ADR-LOG-RESTRUCT-0152 — Index worker requests by status and observed time

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** Add the additive tasks database migration
`idx_worker_requests_status_created_at` on `(status, julianday(created_at)
DESC, id DESC)` and make the status-filtered production query a shared SQL
constant used by its populated-fixture plan regression.

**Reason.** With 160 representative worker requests and `ANALYZE`, SQLite chose
`SCAN worker_requests` for the production pending-status query and sorted it in
a temporary B-tree. A temporary candidate expression index changed the same
query plan to an indexed search and removed the temporary sort while preserving
the existing chronological ordering semantics. The migration is justified by
that measured plan change rather than index presence alone.

**Proof.** `worker_request_status_filter_uses_an_index_with_populated_rows`
passes against the migrated isolated database and requires the new index with
no request-table scan. The P4-58 inventory remains incomplete.

## ADR-LOG-RESTRUCT-0153 — Keep worker-stat SQL parameters numbered

**Status.** Applied on 2026-10-02 while adding P4-58 analytics query-plan proof.

**Decision.** Number all three worker-stat range query binds explicitly as
`?1`, `?2`, and `?3` across the outer aggregate and latest-heartbeat subquery.

**Reason.** The query-plan regression surfaced SQLite assigning anonymous
placeholders after the highest numbered placeholder in the nested query. That
made the production statement require six parameters although the real reader
supplies exactly worker ID, start date, and end date.

**Proof.** The parameter-count failure was reproduced in the isolated populated
fixture. After the SQL fix, the focused analytics plan test is rerun with the
same three values used by the production caller.

## ADR-LOG-RESTRUCT-0154 — Preserve analytics timestamp semantics with indexed predicates

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** Analytics local-day readers use UTC lexical bounds for canonical
RFC3339 rows and legacy SQLite space-separated rows, with an explicit separator
predicate on each branch so a canonical `T` row cannot also match the legacy
space-bound branch. Recent failure and system-event readers split valid recent
rows from malformed non-null timestamps with `UNION ALL`; additive
`julianday(timestamp)` expression indexes serve both branches while preserving
strict-reader errors for malformed data.

**Reason.** The existing `DATE(timestamp, 'localtime')` and
`julianday(timestamp)` predicates caused populated analytics reads to scan.
An initial two-format range rewrite also double-counted canonical rows against
the space-separated range because `T` sorts after a space. A local-midnight
behavior test exposed that defect. The separator predicate restores one-format,
one-branch matching without discarding legacy rows. Splitting recent valid and
malformed paths allows SQLite to seek the expression index rather than scan the
whole table for an `OR` predicate.

**Proof.** `analytics::db::local_day_tests` verifies local-day boundaries,
legacy/canonical cutoff behavior, and filtered system-event results.
`dashboard_live_analytics_plans_classify_scans_and_worker_index_use` populates
and analyzes token usage, worker stats, API failures, and system events, then
asserts production query plans use their selected indexes. The focused
analytics database suite passes. Full TG4-20 inventory and enforcement against
unregistered readers remain open.

## ADR-LOG-RESTRUCT-0155 — Index ready dispatch and recent automatic routing history

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** Add a partial `idx_tasks_ready_dispatch` index for ready non-epic
tasks ordered by creation time and ID, and use a shared `CLAIM_NEXT_READY_SQL`
constant for atomic dispatch and its plan regression. Add a routing-history
expression index on automatic-assignment provenance plus `julianday(assigned_at)`
for the bounded newest-first history reader. Keep the existing worker/type
index for affinity aggregation; do not add a redundant covering index because
the measured production plan already groups in index order without a temporary
sort.

**Reason.** Populated pre-index dispatch plans used the status index but needed
a temporary chronological sort. The partial index narrows the relevant ready
non-epic set and provides its ordering while retaining the same single-statement
active-claim guard. Recent history's `julianday(assigned_at)` reader could not
use its existing plain timestamp index. Affinity grouping already selected
`idx_routing_history_worker_type` with no temporary B-tree, so another index
would increase write/storage cost without demonstrated plan improvement.

**Proof.** `ready_task_claim_uses_dispatch_and_active_claim_indexes` compares
the populated baseline and candidate plans, then verifies the oldest ready
task is claimed and a second active claim is rejected. The routing history and
affinity regression seeds automatic and override rows, compares the baseline
with the migrated plans, proves automatic history uses the measured expression
index, and confirms affinity stays on the existing worker/type index. The
focused tests pass; TG4-20 still has remaining query families and registry
enforcement open.

## ADR-LOG-RESTRUCT-0156 — Index recent and stale ACP message readers

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** Extract shared SQL for the recent and stale ACP readers. Express
stale age as `julianday(created_at) <= julianday('now', ?1)` so it matches a
partial index over pending/read messages, and add measured expression indexes
for global recent ordering and stale-age selection. Keep recipient/status and
task-chat indexes for the other ACP production readers.

**Reason.** The previous stale predicate wrapped its column in `datetime()`
while sorting by `julianday()`, and the recent reader had only an unaligned
plain timestamp index. Populated baseline plans showed a table scan and/or
temporary ordering work. The new expression indexes align with the actual
filter and order clauses and preserve the existing SQLite UTC datetime
semantics.

**Proof.** `worker_message_inbox_and_counts_use_recipient_or_task_indexes`
compares baseline and candidate plans after populating 360 timestamped ACP
messages and running `ANALYZE`. Recipient/status, task, recent, stale and count
queries select their intended indexes. The production recent/stale readers
return bounded, status-appropriate results. The test passes; query inventory
registration remains incomplete outside this ACP family.

## ADR-LOG-RESTRUCT-0157 — Retain existing claim indexes for recovery selection

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** Share the exact-owner interrupted-claim selection SQL with its
populated plan regression and retain the existing task status/assignment
indexes. The assigned-task PID claim remains a primary-key update with its
active-owner guard; no additional composite claim index is added.

**Reason.** The task recovery fixture shows the exact worker/PID/host selection
uses an existing status or worker index and preserves nonmatching claims. The
assigned claim point-reads by integer primary key. Adding a new index before a
measured plan or observed latency gap would add write cost without reducing a
base-table scan.

**Proof.** `interrupted_claim_reconciliation_uses_owner_filter_index` seeds
matching and mismatched claims, checks the indexed plan, holds only the exact
owner IDs, and confirms other claims remain active. The extended ready-claim
plan test confirms assigned claims point-read the task and transition through
the production method. Remaining diagnostics/request inventory and new-reader
registration keep TG4-20 open.

## ADR-LOG-RESTRUCT-0158 — Index the dashboard workers-added-today query

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** Share the three worker-summary SQL statements used by
`AnalyticsQueries::get_dashboard_summary`. Add migration 5's expression index
`idx_workers_created_date` over `date(created_at)` for the workers-added-today
reader. Classify exact total and active worker counts as intentional full-roster
count exceptions.

**Reason.** The populated 1,000-row fixture showed the date-wrapped reader used
the existing status index only as a covering scan and visited the roster. The
date expression index lets SQLite seek the day directly. Total and active
counts must inspect every current worker row because their status predicates
define the exact roster totals; a secondary index would still require a broad
count and would add write cost without demonstrated benefit.

**Proof.** `dashboard_worker_summary_uses_date_index_and_preserves_counts`
compares the production-shaped date query before and after adding the candidate
index, checks both exact-count plans as registered scan exceptions, and
verifies 800 non-deleted, 600 active, and 500 added-today results from 1,000
rows. The focused test passes. P4-58/TG4-20 remain open pending mechanically
enforceable query registration and final inventory audit.

## ADR-LOG-RESTRUCT-0159 — Index epic and phase task-progress readers

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work.

**Decision.** Share production SQL for `TasksDb::get_epic_tasks` and
`get_ready_tasks_in_phase`; add migration 14 indexes for epic ordering and
ready tasks within an epic phase.

**Why.** The remaining-reader audit found both fleet progress readers absent
from the query inventory. A populated 5,000-task plan fixture reproduced a
full scan plus temporary sort for epic progress, and a status-index search plus
temporary sort for ready phase work. The measured indexes allow SQLite to seek
by epic (and phase) in the requested `phase_order, id` order. Adding indexes
without representative populations or production-shared SQL would not prove
the workload path.

**When.** 2026-10-02 · phase P4 · triggering task `P4-58` / gate `TG4-20`

**Where.** `arcana-core/src/db/tasks.rs` ·
`docs/restructurization/development/_verify/P4-58-QUERY-INVENTORY.md` ·
`MASTER_DEVELOPMENT_PLAN.md` · `TESTING_GATES.md` · `READINESS_MATRIX.md`

**How.** Migration 14 creates `idx_tasks_epic_phase_order` and
`idx_tasks_ready_phase_order`. `epic_progress_readers_use_measured_indexes_with_populated_tasks`
compares pre/post `EXPLAIN QUERY PLAN` over 5,000 inserted tasks, asserts the
production SQL selects the intended indexes without table scans or temporary
ordering sorts, and verifies production reader results/order. The focused test
passes. P4-58/TG4-20 remain open for the remaining scoped reader audit and an
enforceable query registry.

**Links.** MDP P4-58 · `TG4-20` · `_verify/P4-58-QUERY-INVENTORY.md`

## ADR-LOG-RESTRUCT-0160 — Grant exact external ancestors for selected config canonicalization

**Status.** Owner-approved on 2026-10-02; implementation and Windows proof in progress.

**Supersedes.** ADR-LOG-RESTRUCT-0142 and ADR-LOG-RESTRUCT-0143's rollback boundary, following the owner's later approval to grant exact ancestor traversal for the selected config path.

**Choice.** For a config file already validated inside the selected workspace, temporarily grant the AppContainer SID only non-inheriting `FILE_TRAVERSE | FILE_READ_ATTRIBUTES` on the existing directories in that exact path's ancestor chain, including ancestors above the workspace and excluding the volume root. Keep content read/execute grants confined to the selected config directory/file. Revoke the exact grants when the workspace AppContainer lease ends. Do not grant listing, file content, write, or execute on external ancestors.

**Why.** The production Clippy failure remains after the config bytes became readable: its child process cannot canonicalize the selected absolute config path because Windows denies metadata/traversal on parent directories. A workspace-only chain does not satisfy canonicalization for an absolute path. The owner explicitly selected the exact-ancestor grant after the earlier experiment was rolled back; the scope remains limited to the selected path and metadata rights.

**Proof.** The Windows regression must verify the grant set exactly follows the selected path through the volume root's parent, carries no inheritable bits or extra rights, and is revoked after lease teardown. An AppContainer probe must canonicalize the config directory/file and execute the production Clippy intent while continuing to deny sibling content reads and local TCP. Measure this focused test and stop/clean its child process and unique SID on every exit path. Linux/macOS remain separate requirements.

**When / where.** 2026-10-02 · P4-11/P4-61 · `arcana-core/src/platform/process_sandbox/windows.rs` · `TG4-18` · `TG4-31`.

## ADR-LOG-RESTRUCT-0161 — Index omitted fleet and dashboard query readers

**Status.** Applied on 2026-10-02 as measured P4-58/TG4-20 work.

**Decision.** Share production SQL and add populated plan/result regressions for the daily task-budget summary, stranded QC-reviewer claims, and ordered worker roster used by routing and assignment. Add migration 15's `date(created_at)` task index, migration 16's partial QC-claim index, and workers migration 6's partial active-roster ordering index.

**Why.** A fresh consumer audit found these P2 readers absent from the existing fleet/dashboard inventory. The daily budget reader wrapped its timestamp in `date()`, QC reconciliation filtered sparse non-null claims without an index, and the full roster query paid a temporary ordering sort. Each candidate was compared against a populated baseline before adoption; exact full-roster and aggregate count semantics remain explicit.

**Proof.** `daily_budget_summary_uses_measured_date_index_and_preserves_utc_day_totals` passes with 2,000 task rows, an observed baseline scan, indexed date seek, and exact production totals. `qc_claim_reconciliation_uses_partial_index_with_populated_tasks` passes with 2,000 rows, baseline scan, indexed sparse-claim seek, and all claim/reviewer pairs read back. `worker_roster_uses_measured_order_index_and_returns_full_non_deleted_roster` passes with 2,000 workers, baseline temporary sort, index-ordered production plan, and exact filtered/order behavior. The fresh source audit continues for any remaining readers; enforceable inventory registration remains open.

**Where.** `arcana-core/src/db/tasks.rs` · `arcana-core/src/db/workers.rs` · `_verify/P4-58-QUERY-INVENTORY.md` · `MASTER_DEVELOPMENT_PLAN.md` · `TESTING_GATES.md` · `READINESS_MATRIX.md`.

## ADR-LOG-RESTRUCT-0162 — Retain existing unblock and lookup indexes

**Status.** Applied on 2026-10-02 as incremental P4-58/TG4-20 work; the listed focused regressions pass.

**Decision.** Add production-shared SQL and populated query-plan/result coverage for task budget status, task unblocking/blocker point reads, epic completion aggregation, daily-cost point lookup, and worker lookup by slug. Keep `TasksDb::unblock_ready_tasks` on the existing task-status index; do not add a partial index unless the populated planner selects it over the existing index. Use the existing epic index, daily-cost date primary key, and unique worker-slug index. Read yesterday's dashboard daily-cost row once and reuse it for both cost and token totals.

**Why.** A fresh reader audit found production consumers not represented in the P4-58 inventory. Initial plan comparison selected `idx_tasks_status` for sparse blocked-task selection and did not select the proposed partial covering index, so that migration was removed. Worker slug and daily cost are already indexed by schema. The dashboard summary previously issued the same daily-cost point query twice for one date.

**Proof.** `cargo test -p arcana-core task_budget_and_unblock_readers_use_existing_indexes_with_populated_tasks --lib`, `cargo test -p arcana-core worker_roster_uses_measured_order_index_and_returns_full_non_deleted_roster --lib`, and `cargo test -p arcana-core dashboard_daily_cost_range_uses_primary_key_index_with_populated_rows --lib` each pass 1/1. The task regression checks primary-key and epic indexes, the existing status index, exact budget results, and actual dependent-task release. The worker regression checks the unique slug plan and production readback. The daily-cost regression checks the date point and range plans and production point reader.

**Where.** `arcana-core/src/db/tasks.rs` · `arcana-core/src/db/workers.rs` · `arcana-core/src/analytics/db.rs` · `arcana-core/src/analytics/queries.rs` · `_verify/P4-58-QUERY-INVENTORY.md`.

## ADR-LOG-RESTRUCT-0163 — Treat AppContainer DOS canonicalization as a platform boundary

**Status.** Open for owner direction on the Windows sandbox architecture; no rights were widened.

**Decision.** Do not attempt to solve Rust/Clippy `canonicalize()` failures by adding broader filesystem ACL grants. The owner-approved exact ancestor traversal/read-attributes grants remain the maximum approved ACL scope. The production Clippy regression confirms metadata/config reads work while `canonicalize()` is denied. Windows Rust build/test corpus proof remains open until Arcana either evaluates a different OS sandbox that preserves external-read/network denial or keeps incompatible toolchains fail-closed under AppContainer.

**Why.** The failure is consistent with Windows AppContainer's DOS-volume namespace restriction, not simply a missing filesystem ACE. Rust documents Windows `std::fs::canonicalize` as using `CreateFile` and `GetFinalPathNameByHandle`; a current Microsoft STL AppContainer report reproduces `ERROR_ACCESS_DENIED` for DOS-volume resolution even with broad filesystem ACLs and describes NT-volume fallback as a workaround: https://github.com/microsoft/STL/issues/6286. Rust API documentation: https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html. A separate Cargo workspace `target` creation denial also remains to be isolated; the namespace finding does not explain that write failure.

**Proof.** `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox` failed after 78.29 seconds with `canonicalize()` denied for both config directory/file, metadata/content reads succeeding, and Cargo failing to create `workspace\target`. `selected_path_ancestors_receive_only_non_inheriting_traversal_grants` passes for the exact granted chain. No wider ACL experiment was run.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs` · `arcana-core/src/agent/builtin_tools/project_command.rs` · `P4-11` · `P4-61` · `TG4-18` · `TG4-31` · `_verify/P4-SESSION-CHECKPOINT-2026-10-02.md`.

## ADR-LOG-RESTRUCT-0164 — Preserve monthly worker-stat coverage in every reader

**Status.** Applied on 2026-10-02; focused core and shared-reader tests pass. P4-59/TG4-21 remain open pending the full retention/export lifecycle proof.

**Decision.** Keep the exact `covered_start` and `covered_end` for each monthly worker-stat bucket and expose those bounds plus `granularity` through the Tauri analytics response, CLI table, and JSON export. A mixed aggregate reports a monthly granularity when any contributing record is monthly.

**Why.** Monthly totals cannot honestly answer a partial-month query at daily precision. Showing the dates represented by each rollup lets the operator see the actual evidence interval, while recent raw rows retain day-level detail.

**Proof.** `worker_stats_accumulate_roll_up_atomically_and_read_with_coverage` passes with same-day tick accumulation, idempotent rollup, mixed daily/monthly aggregation, and exact bounds. `cargo test -p arcana-shared worker_stats --lib --locked` passes both monthly composition and legacy-schema reader tests. The Tauri response and CLI now carry/display coverage. Build checks are recorded in the active P4 checkpoint.

**Where.** `arcana-core/src/analytics/db.rs` · `src-shared/src/db/analytics.rs` · `src-tauri/src/commands/analytics_commands.rs` · `src-cli/src/commands/analytics.rs` · P4-59 · TG4-21.

## ADR-LOG-RESTRUCT-0165 — Reserve a conservative daily spend maximum before provider dispatch

**Status.** Implementation in progress on 2026-10-02; focused reservation tests pass. P2-21/TG2-17 remain open pending provider-boundary, failure, restart, and production-consumer evidence.

**Decision.** When the persisted daily cap is enabled, atomically reserve a conservative maximum in config.db before each provider request. Compute it from the selected model's full context window and output/thinking budget at the highest published input/output rates. Fail closed on unknown model/pricing, malformed cap state, or persistence error. Reconcile against reported usage; retain the full hold when a dispatched call has no reliable usage. Disabled caps do not incur model lookup or estimate work.

**Why.** A read-then-call counter allows concurrent workers to overspend and previously swallowed settings failures. An immediate SQLite transaction includes outstanding holds before a request is sent. Holding the maximum after ambiguous dispatch favors the user's ceiling over reclaiming possibly spent capacity.

**Proof.** Focused tests cover atomic concurrent reservations, denial at the ceiling, actual-cost reconciliation, unknown-usage retention, malformed cap rejection, the disabled setting, and re-enabling after uncapped same-day spend. The provider wrapper and production failure/restart paths still need end-to-end regressions; do not claim TG2-17 closed.

**Where.** `arcana-core/src/budget/daily.rs` · `arcana-core/src/provider/capped.rs` · `arcana-core/src/db/config_db.rs` · P2-21 · TG2-17.

## ADR-LOG-RESTRUCT-0166 — Record logical database outcomes after the operation returns

**Status.** Applied in part on 2026-10-02; comprehensive P4-50/TG4-24 coverage remains open.

**Decision.** Keep the rusqlite profile callback's statement event explicitly labeled `duration_observed`; it cannot report success or failure. Record outcome at a logical operation boundary after the Rust operation returns. Such events may include only fixed operation names, logical database identity, bounded result counts, duration, and a stable outcome.

**Why.** Reporting a profiled SQL statement as successful would be unsupported. A result-aware operation event preserves an honest distinction between statement timing and a DB-backed operation's known result, without capturing SQL or bound values.

**Proof.** `OperationTimer`'s captured JSON regression verifies database identity, duration, success outcome, and a zero result count. Worker-stat rollup/range, graph build plus node/edge/FTS/flush stages, and memory operation timers now emit logical database identities and result-aware outcomes. Graph producer, memory recall classification, and Logs fallback metric tests pass. This is not yet an audit of every production DB/Memory/graph operation or the Logs UI reader path.

**Where.** `arcana-core/src/observability/timing.rs` · `arcana-core/src/analytics/db.rs` · `arcana-core/src/analytics/aggregator.rs` · P4-50 · TG4-24.

## ADR-LOG-RESTRUCT-0167 — Fail closed when scoped SQLite reader owners are not inventoried

**Status.** Initial owner-registration implementation applied on 2026-10-02 and strengthened by ADR-0168; TG4-20 remains open until discovered in-scope readers have populated plan/result proofs or explicit bounded exclusions.

**Decision.** Use a `syn`-based integration test over the six P4 fleet/dashboard SQLite modules to discover production read owners, resolve SQL constants and local bindings, ignore test-only code, and fail when a discovered owner is absent from the P4-58 inventory. Replace the three internal token-group SQL interpolations with fixed allowlisted SQL constants so the source audit can identify the actual query forms.

**Why.** An inventory that exists only as prose can drift while production queries change. AST discovery makes new reader owners visible in CI; the inventory then assigns plan evidence or a bounded scope decision. Source registration is explicitly separate from populated `EXPLAIN QUERY PLAN` and result behavior proof.

**Proof.** The initial `cargo test -p arcana-core --test p4_query_registry --locked` passed 2/2 after listing newly discovered owners. Its owner-only coverage was insufficient for additions within an existing owner; ADR-0168 records the exact-fingerprint extension. The inventory continues to mark remaining plan/scope work open, so TG4-20 does not close on the registry test alone.

**Where.** `arcana-core/tests/p4_query_registry.rs` · `arcana-core/src/analytics/db.rs` · `_verify/P4-58-QUERY-INVENTORY.md` · P4-58 · TG4-20.

## ADR-LOG-RESTRUCT-0168 — Fingerprint every statically resolved production query

**Status.** Applied on 2026-10-02; exact-query registry and focused production-reader tests pass. TG4-20 remains open for missing plan/result proof and scope decisions.

**Decision.** Extend ADR-0167's AST inventory guard from owner presence to exact normalized SQL fingerprints. Every statically resolved `SELECT`/`WITH` read in the six scoped production modules must match a checked-in fingerprint; any new query or query-text change fails the gate until registered. Keep plan/result evidence and exclusions on the owner inventory rows, and do not treat the fingerprint table as plan proof.

**Why.** Owner-only registration allows a new query inside an already-listed method to bypass the registry. Exact fingerprints close that gap without pretending that source registration proves planner behavior or consumer results.

**Proof.** `cargo test -p arcana-core --test p4_query_registry --locked` passes 2/2 with 79 production query fingerprints. The inventory now records passing populated-plan/result evidence for `RunsDb::latest_worker_run_owner`, `RunsDb::latest_worker_ticks`, token aggregates/top sessions, and the exact `TasksDb::get_blocked_tasks` reader. Remaining open inventory rows continue to block TG4-20.

**Where.** `arcana-core/tests/p4_query_registry.rs` · `_verify/P4-58-QUERY-INVENTORY.md` · `arcana-core/src/db/runs.rs` · `arcana-core/src/analytics/db.rs` · `arcana-core/src/db/tasks.rs` · P4-58 · TG4-20.

## ADR-LOG-RESTRUCT-0169 — Scope P4-58 to production fleet/dashboard dispatch readers

**Status.** Applied on 2026-10-02 after production-caller audit and isolated plan/result tests; TG4-20 remains open for other in-scope rows.

**Decision.** Treat `PlansDb::claim_next_item` and `check_dependencies` as in-scope because the production planner invokes them for plan dispatch and dependency readiness. Exclude plan presentation/readback helpers with bounded reasons where their only consumers are planner detail/list/update flows, or where no production consumer exists. Exclude `RunsDb::get` and `recent` because the Rust production tree has no callers; fleet recovery uses dedicated readers. Include the worker point lookup used by worker status/model/recipe commands, but exclude slug uniqueness validation from dispatch/routing query evidence.

**Why.** TG4-20 covers fleet/dashboard, dispatch, stale-recovery, task-health, and observability reads. A source inventory should not silently expand to unrelated planner presentation or admin validation paths, and public helper existence alone is not evidence of production use. Dispatch dependencies remain in scope regardless of which UI invokes them.

**Proof.** Focused populated plan/result tests pass for `PlansDb::claim_next_item`, `PlansDb::check_dependencies`, and `WorkersDb::get_by_id`. Source audit identifies planner production consumers in `arcana-core/src/agent/builtin_tools/planner.rs` and worker/admin callers in `src-tauri/src/commands/worker_commands.rs`; no Rust production callers exist for `RunsDb::get`, `RunsDb::recent`, or `PlansDb::plan_exists`. Scope entries are documented in `_verify/P4-58-QUERY-INVENTORY.md`.

**Where.** `arcana-core/src/db/plans.rs` · `arcana-core/src/db/workers.rs` · `arcana-core/src/agent/builtin_tools/planner.rs` · `src-tauri/src/commands/worker_commands.rs` · `_verify/P4-58-QUERY-INVENTORY.md` · P4-58 · TG4-20.

## ADR-LOG-RESTRUCT-0170 — Build a separately sandboxed macOS XPC runner

**Status.** Owner-approved on 2026-10-02; implementation and macOS verification remain open under P4-61/TG4-31.

**Decision.** Build a separately signed App Sandbox XPC service for macOS project-command execution. The XPC service must have its own minimal entitlements, deny network, avoid broad user-file access, and be signed independently before the containing Arcana app. Keep all hosts fail-closed unless the real toolchain and descendant process tree prove the selected-workspace, exact-runtime, no-external-read, no-network, cancellation, and output contract.

**Why.** The current `sandbox-exec` path is default-allow and cannot establish the required isolation boundary. Apple documents XPC services as independently sandboxed components. However, dynamically granted bookmark access is not inherited by arbitrary child processes; service-level access alone is not proof that compilers and their descendants can access only the selected workspace. The owner later approved a sandbox-owned staging/copy-back fallback under ADR-LOG-RESTRUCT-0171; that approval does not replace path, symlink, and writeback validation.

**Proof.** Apple references: [XPC services](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingXPCServices.html), [App Sandbox entitlements](https://developer.apple.com/library/archive/documentation/Miscellaneous/Reference/EntitlementKeyReference/Chapters/EnablingAppSandbox.html), and [nested code signing](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/). The inspected repository has no existing XPC service, helper bundle, or signing integration. A Luna architecture review mapped the `process_sandbox` integration and Tauri packaging points and identified the descendant-access limitation. No runner code or macOS verification is claimed yet.

**Where.** `arcana-core/src/platform/process_sandbox/` · `src-tauri/` · `installers/dmg/` · `.github/workflows/ci.yml` · P4-61 · TG4-31.

## ADR-LOG-RESTRUCT-0171 — Authorize sandbox-owned workspace staging on macOS

**Status.** Owner-approved on 2026-10-02; implementation remains open under P4-61/TG4-31.

**Decision.** If an App Sandbox XPC child cannot directly access the selected workspace under the least-privilege profile, Arcana may stage a validated workspace snapshot into a sandbox-owned directory and return only bounded, validated changes to paths beneath the selected workspace. Canonicalize the selected root before execution; reject symlinks, special files, traversal, retargeting, and workspace identity changes at staging and copy-back. Copy back only a bounded write set derived from the staged before/after snapshot, verify destination components without following links, and apply file replacements atomically. Never copy arbitrary untrusted paths or overwrite paths outside the original selected workspace.

**Why.** The owner authorized staged execution/copy-back to address macOS's non-inheritance of dynamic bookmark access. The strict read/network denial contract remains in force, and an untrusted child must not use copy-back to write outside the selected workspace or replace protected paths.

**Proof.** This is an owner-authorized design constraint, not implemented behavior. Direct workspace access and descendant behavior remain untested on macOS. Staging/copy-back must add negative tests for traversal, symlink retargeting, unrelated paths, changed workspace identity, and disallowed output, plus positive persistence of approved build outputs. The former write-only `sandbox-exec` launch is now removed from the production path and returns an unsupported error until XPC is packaged. This decision does not close TG4-31.

**Where.** `arcana-core/src/platform/process_sandbox/` · macOS XPC service/package · P4-61 · TG4-31.

## ADR-LOG-RESTRUCT-0172 — Index bounded worker-progress retention by run and event ID

**Status.** Applied on 2026-10-02; focused plan/result test passes. TG4-20 remains open for other embedded lifecycle statements and final audit.

**Decision.** Add runs database migration 3 with `idx_worker_run_events_run_id_id` on `(run_id, id DESC)` for the `record_worker_progress` retention subquery that keeps the newest 500 events per run.

**Why.** The populated baseline plan scanned the full `worker_run_events` table despite the outer delete's run filter. A run-scoped descending index lets the embedded top-500 selector seek and walk only one run's rows. This index is scoped to bounded observability retention and cannot alter other runs' history.

**Proof.** `worker_progress_retention_uses_run_index_and_keeps_latest_500` compares baseline and candidate plans on populated target/unrelated runs, verifies the production candidate uses the new index with no base-table scan, and reads back exactly 500 target events while preserving all 24 unrelated events. The isolated DB migration applies automatically in the fixture.

**Where.** `arcana-core/src/db/runs.rs` · `_verify/P4-58-QUERY-INVENTORY.md` · P4-58 · TG4-20.

## ADR-LOG-RESTRUCT-0173 — Keep Windows Cargo outputs and cleanup inside the workspace cache

**Status.** Applied on Windows; the production Rust corpus passes Build and Test but remains blocked at Clippy config canonicalization under P4-11/TG4-18 and P4-61/TG4-31.

**Decision.** Set sandboxed child processes' `CARGO_TARGET_DIR` to the pre-created selected-workspace `.arcana-cache/cargo-target`, disable Cargo incremental compilation for those child runs, and include the Windows `DELETE` right only in the selected workspace's inherited write capability.

**Why.** The Windows production launcher first failed creating a project-local target directory, then failed moving/removing temporary object files under an explicitly workspace-scoped target directory. Redirecting outputs into Arcana's existing workspace cache avoids the first failure; disabling incremental state and granting delete only inside the same selected workspace supports Cargo's required temporary-file cleanup without extending file access outside the workspace. The direct build/test corpus reached Clippy after these changes. A volume-root ACL extension was denied by the host and was reverted; no root permission is part of this decision.

**Proof.** `checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher` now completes the dependency-free Rust Build and Test intents before failing at Lint because Clippy cannot canonicalize its readable config (`Access is denied`). The focused production Clippy test confirms the earlier target-directory creation failure is gone. This is partial Windows launcher evidence; it is not TG4-31 proof, and Python/Node production and external-file/network tests remain open.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs` · `arcana-core/src/agent/builtin_tools/project_command.rs` · P4-11 · P4-61 · TG4-18 · TG4-31.

## ADR-LOG-RESTRUCT-0174 — Keep Windows project execution fail-closed on unsupported Home edition

**Status.** Applied as the current host capability decision on 2026-10-02; P4-11/P4-61 remain open for a supported, verified Windows boundary.

**Decision.** Do not use Windows Sandbox as an alternate runner on this host. Keep AppContainer-backed process execution fail-closed for operations that cannot satisfy the required workspace/toolchain policy. Re-evaluate the VM-backed option only on a supported Windows edition with an isolated staging/copy-back and output-capture design.

**Why.** This machine reports Windows Home Single Language, while Microsoft documents Windows Sandbox as unsupported on Windows Home. The Windows Sandbox CLI is also documented as lacking process stdout/stderr capture and requiring an active user session for `ExistingLogin`; mapped-folder writes persist to the host. Although a staged workspace plus guest-side log/result files may be viable on supported editions, it is not an available or verified alternate on this machine.

**Proof.** Local host inspection reports Windows Home Single Language, build 26200, with no `wsb.exe` or `WindowsSandbox.exe` on PATH; querying the optional feature requires elevation and was not attempted with elevation. Microsoft references: [Windows Sandbox overview and supported editions](https://learn.microsoft.com/en-us/windows/security/threat-protection/windows-sandbox/windows-sandbox-overview), [Windows Sandbox CLI and its process-I/O/session limits](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-cli), and [Sandbox mapped-folder persistence](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file).

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs` · `docs/restructurization/development/TESTING_GATES.md` · P4-11 · P4-61 · TG4-18 · TG4-31.

## ADR-LOG-RESTRUCT-0175 — Disable the macOS write-only sandbox wrapper until XPC is packaged

**Status.** Applied on 2026-10-02; macOS project-command launch now fails closed. TG4-31 remains open pending the owner-approved signed XPC/staging implementation and macOS verification.

**Decision.** Remove the production `sandbox-exec` wrapper from `process_sandbox::spawn`. Return an `Unsupported` error on macOS until the separately signed App Sandbox XPC runner is integrated.

**Why.** The previous profile started with `(allow default)` and restricted writes only. It did not deny unrelated reads or network, so it could not satisfy P4-61's untrusted-code contract. Leaving it active would report a sandboxed launch while violating the required capability boundary. A secure refusal is preferable until the approved XPC architecture has a real consumer.

**Proof.** The macOS-only regression asserts that production `spawn` returns `Unsupported` before creating a child or workspace output. This target test has not run on the Windows host; `cargo check -p arcana-core --lib --locked` and the macOS target CI/build still need to validate cfg-specific compilation. The sandbox-owned staged workspace/copy-back policy is detailed in ADR-0171 and remains unimplemented.

**Where.** `arcana-core/src/platform/process_sandbox/mod.rs` · `installers/dmg/` · P4-61 · TG4-31.

## ADR-LOG-RESTRUCT-0176 — Bind QC build evidence to workspace content

**Status.** Implemented on 2026-10-02; P4-14/TG4-07 remain open for the complete cross-platform production corpus.

**Decision.** Bind each observed Rust-owned Build exit to the canonical selected workspace path and a deterministic SHA-256 fingerprint captured before the child starts. Require the latest attributable passing evidence to match the current workspace fingerprint both in the shared QC reader and in SQLite's `qc_review` trigger. Fingerprint failures reject QC. Timeouts and spawn errors retain their separate outcomes and do not fabricate an exit row or affect the observed-nonzero failure streak. The fingerprint skips the generated directories `.git`, `.arcana-cache`, `.arcana-tmp`, `.mypy_cache`, `.next`, `.pytest_cache`, `.ruff_cache`, `.turbo`, `.venv`, `__pycache__`, `coverage`, `dist`, `node_modules`, `target`, and `venv`; it hashes remaining paths, entry types, symlink target text, file bytes, and lengths.

**Why.** The cross-phase audit found that a passing build could authorize QC after source edits followed by a timed-out command, because timeout results were not persisted as exit evidence and the trigger compared only the old exit code. Workspace binding makes any lasting change to included project inputs stale the old result while preserving the rule that only observed nonzero process exits affect build-failure diagnosis. Generated output trees are excluded so ordinary compiler output does not invalidate the input state that was verified.

**Proof.** `workspace_fingerprint_tracks_workspace_inputs_but_ignores_generated_output` proves content sensitivity and generated-output exclusion. `shipped_project_command_persists_spawned_build_results_and_qc_reads_them` exercises real Cargo Build evidence, then pass → workspace mutation → bounded timeout → API/direct-SQL QC rejection, verifies the timeout created no synthetic exit, restores the exact workspace, and confirms the existing pass is valid again. `shared_task_status_api_rejects_qc_review_without_latest_attributable_passing_build` covers missing, unattributable, failed, and passing states. Focused tests pass; broader `arcana-core` verification remains open.

**Where.** `arcana-core/src/platform/workspace_fingerprint.rs` · `arcana-core/src/db/tasks.rs` · `arcana-core/src/agent/builtin_tools/project_command.rs` · P4-14 · TG4-07 · TG4-20.

## ADR-LOG-RESTRUCT-0177 — Keep the Clippy config path absolute under AppContainer

**Status.** Rejected experiment on 2026-10-02; no production behavior changed.

**Decision.** Do not replace the canonical absolute `CLIPPY_CONF_DIR` with a workspace-relative value as a workaround for the Windows AppContainer canonicalization failure.

**Why.** The hypothesis was that resolving `.arcana-cache` from the selected project working directory would avoid the failing absolute-path lookup while keeping the same workspace boundary. The production child still failed with `Access is denied` when canonicalizing both `.arcana-cache` and `.clippy.toml`; changing path spelling therefore does not fix the sandbox's path-handle restriction and would leave an unproven environment contract.

**Proof.** The path-shape unit test passed with the temporary relative value. The production `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox` test ran for 74.53 seconds and failed with the same Windows error even though metadata and file-byte reads passed. The experiment was reverted. The corrected full `cargo test -p arcana-core --lib --locked` suite remains 804 passed / 2 failed, both at this boundary. Windows Sandbox is unavailable on this Windows 10 Home host per ADR-0174; an alternate production boundary remains unresolved.

**Where.** `arcana-core/src/agent/builtin_tools/project_command.rs` · `arcana-core/src/platform/process_sandbox/windows.rs` · P4-11 · P4-61 · TG4-18 · TG4-31.

## ADR-LOG-RESTRUCT-0178 — Stage the selected Windows Python runtime for sandboxed verification

**Status.** Implemented and production-tested for the selected PATH `python.exe` on this Windows host. P4-11/P4-61 remain open for fallback selection, lint availability, broader external-file proof, and the cross-platform corpus.

**Decision.** Copy the selected interpreter's standard library, DLL runtime, and the bounded pytest import closure into an Arcana-owned `%USERPROFILE%\\.arcana\\python-runtimes` cache. Run it with isolated startup flags and a generated `._pth` search path; expose only that exact runtime tree to AppContainer with inherited read/execute rights. Remove ambient `PYTHONHOME`, `PYTHONPATH`, `PYTHONUSERBASE`, and `PYTHONSTARTUP`, disable third-party pytest plugin autoload, and keep generated Python bytecode under the selected workspace. Reject symlinks/special files and verify staged content before reuse. A staging error remains fail-closed; fallback to a user-owned install is unavailable until there is an explicit installation selector.

**Why.** The selected host interpreter's `Lib` is a junction into the user profile, and pytest is in user-site packages. Granting those trees directly would expose broad user files, while ambient `PYTHONHOME` could select an arbitrary read root. Copying the standard runtime plus pytest's import closure gives Python production commands a precise, read-only capability without granting the source installation to untrusted build/test code.

**Proof.** Host isolation probe: staged Python 3.14.7 reports its own `sys.prefix`, imports staged pytest 9.1.1, and compiles the fixture. `staged_runtime_is_readable_but_not_writable_and_cannot_read_its_sibling` passes through the production AppContainer launcher with pytest import, synthetic sibling-secret denial, runtime-write denial, and controlled loopback TCP denial. `production_python_build_and_tests_use_the_staged_runtime` passes Build (`compileall`) and Test (`pytest`, 1 passed) through the production launcher. Cold and verified-cache test runs measured 42.61s and 9.14s; the latter reused the Arcana-owned runtime cache. The first run exposed host-probe-generated `.pyc` drift; disposable bytecode is excluded from the immutable-code fingerprint, cache verification rejects symlinks before ignoring generated bytecode, and the host probe disables bytecode writes.

**Where.** `arcana-core/src/platform/python_runtime.rs` · `arcana-core/src/platform/process_sandbox/{mod.rs,windows.rs}` · `arcana-core/src/agent/builtin_tools/project_command.rs` · P4-11 · P4-61 · TG4-07 · TG4-31.

## ADR-LOG-RESTRUCT-0179 — Verify the Windows Python executable before host inspection

**Status.** Applied on 2026-10-02; Windows compile and focused sandbox verification are running. P4-11/P4-61 remain open for full runtime trust, Ruff installation/staging, and platform corpus proof.

**Decision.** Before running a PATH-selected Python executable for metadata inspection, require Windows Authenticode verification through `WinVerifyTrustEx` with UI disabled and cache-only certificate retrieval. Require the executable to report a prefix that contains its standard `DLLs` tree and either an in-prefix `Lib` or the exact versioned per-user Python `Lib` junction already used by this machine. Fingerprint source runtime bytes, exclude disposable bytecode, and revalidate Arcana's staged runtime on process-cache reuse. Run all staged Python probes and commands with `-B` to prevent bytecode from mutating the verified runtime.

**Why.** The independent Luna High review found host execution of the first PATH interpreter occurred before sandboxing, cache identity used only metadata, and canonicalizing `Lib` followed any junction. The owner selected signed standard installations. Verifying the executable before any host launch and constraining the one supported user-install junction closes those trust gaps while preserving this PC's valid Python layout. Content-based keys and per-use staged-cache checks prevent metadata-preserving source changes or later cache mutation from silently reusing a changed runtime.

**Proof.** `stdlib_junction_must_resolve_inside_prefix_or_exact_user_runtime` rejects unrelated external targets. `staged_runtime_is_readable_but_not_writable_and_cannot_read_its_sibling` exercises signed interpreter selection, runtime integrity, staged pytest import, sibling-file denial, runtime-write denial, and loopback network denial through the production sandbox. Production Build/Test proof is `production_python_build_and_tests_use_the_staged_runtime`. The current host has Python 3.14.7 signed by PSF and no installed Ruff; the per-call install and staged Ruff closure still need an implemented host-controlled path and real proof.

**Where.** `arcana-core/src/platform/python_runtime.rs` · `arcana-core/src/platform/process_sandbox/` · `arcana-core/src/agent/builtin_tools/project_command.rs` · P4-11 · P4-61 · TG4-18 · TG4-31.

## ADR-LOG-RESTRUCT-0180 — Discover scoped query sources from directories

**Status.** Applied on 2026-10-02; expanded registration and source-discovery regressions pass. TG4-20 remains open for populated plan/result evidence and the remaining scoped audit.

**Decision.** The P4-58 AST registry discovers Rust modules beneath `arcana-core/src/db`, `arcana-core/src/analytics`, and `src-shared/src/db` at test time instead of naming each production file with `include_str!`. Enumeration and file-read errors fail the test rather than silently dropping a module. Existing settings, model-configuration, annotation, and schema-helper modules outside the owner-approved P4-58 scope remain excluded by an explicit path list. The query inventory names the rationale for every excluded module, and a regression requires those exclusions to stay documented.

**Why.** A seven-file literal list allowed a new production reader module to bypass registration if maintainers forgot to update the test. Directory discovery makes new modules in the scoped roots visible by default while preserving the owner's explicit scope boundaries. It improves source-registration completeness only; an exact fingerprint does not replace populated production-result and query-plan proof.

**Proof.** `scoped_source_discovery_includes_new_production_database_modules`, `query_inventory_documents_each_source_scope_exception`, and `production_readers_are_registered_in_the_p4_query_inventory` pass in `cargo test -p arcana-core --test p4_query_registry --locked -- --nocapture` (4/4). `cargo fmt --all -- --check` and `git diff --check` pass.

**Where.** `arcana-core/tests/p4_query_registry.rs` · `_verify/P4-58-QUERY-INVENTORY.md` · P4-58 · TG4-20.

## ADR-LOG-RESTRUCT-0181 — Stage Ruff as an explicitly approved Arcana tool

**Status.** Implemented for Windows x86-64 on 2026-10-02; the signed-Python archive-extraction regression passes. The real desktop approval-to-lint path remains unproven, and unsupported host architectures fail closed.

**Decision.** Keep ordinary `project_command(intent="install")` for project dependencies fail-closed. Add only the exact `intent="install", toolchain="python", package="ruff"` host-tool request. It requires the existing one-call `HostApproval` gate in every permission mode and cannot be standing-allowed; headless execution refuses before entering the handler. After approval, the Arcana host downloads Ruff 0.16.8 from the fixed PyPI file URL, enforces a 12 MiB response limit, verifies the published wheel SHA-256 before opening it, and uses the already-signature-verified standard Python's `zipfile` module to extract only the exact `ruff-0.16.8.data/scripts/ruff.exe` member. It verifies the executable SHA-256 and rechecks a local manifest and binary digest before each lint launch. The staged executable is stored under Arcana-owned tool cache and is the only extra read-only path passed to the sandboxed Python lint process; child networking remains denied.

**Why.** The owner approved a pinned, host-approved Ruff install and stage because the selected Python runtime has no Ruff module. Generic project dependency installation still has no locked staging/copy path, and build/test code must stay offline. A fixed wheel/hash and exact-member extraction avoids adding a new Rust archive dependency or exposing the full wheel to the child process.

**Proof.** PyPI's 0.16.8 release metadata provides the Windows x86-64 wheel URL and SHA-256; Ruff's official installation documentation confirms the supported PyPI wheel distribution. `ruff_wheel_extractor_writes_only_the_exact_expected_executable` creates a synthetic wheel with signed host Python, extracts only the allowed member, and proves a traversal member is ignored. The real Tauri permission card flow, actual pinned download/staging via a granted call, and production sandboxed lint remain to be exercised. Do not claim Python Lint or TG4-31 complete.

**Where.** `arcana-core/src/platform/python_runtime.rs` · `arcana-core/src/agent/builtin_tools/project_command.rs` · P4-11 · P4-61 · TG4-18 · TG4-31.

### 2026-10-02 implementation correction

An independent GPT-6 Luna High review found the initial staging location and
grant shape did not satisfy the existing Windows sandbox contract: the
AppContainer accepts read-only grants only for directories beneath
`python-runtimes`, while the first Ruff implementation staged under
`python-tools` and passed a file path. Ruff is now staged in its own directory
under `python-runtimes` and that directory is passed to the existing
read/execute-tree grant. The downloaded executable is 26,408,448 bytes, so the
archive member bound is 32 MiB; the extraction regression now proves a 17 MiB
member is accepted while the exact-member/traversal checks remain in place.
The real approved download and sandboxed lint run remain open; this correction
does not close TG4-31.

## ADR-LOG-RESTRUCT-0182 — Scope the Python process cache to the Arcana profile

**Status.** Applied on 2026-10-02; the isolated-profile AppContainer test and
the next full core-suite rerun pass this cache/profile boundary.

**Decision.** Reuse the in-process staged Python runtime only when its verified
root is beneath the active profile's canonical `python-runtimes` directory.
When the active Arcana profile changes, discard the in-memory cache entry and
stage or verify the runtime under that profile before launching a child. Keep
profile-redirecting production tests under the shared `HomeGuard` lock and use
temporary profiles so they cannot race one another or leave test runtimes in
the user's normal Arcana data directory.

**Why.** The full parallel Rust suite exposed a mismatch: one test changed the
process-global profile while another reused a staged runtime from the previous
profile. The Windows sandbox correctly rejected that outside-root path. The
cache key therefore needs to include the Arcana-owned cache root, and tests
that alter process-global profile selection need to serialize on the existing
crate-wide guard.

**Proof.** `staged_runtime_is_readable_but_not_writable_and_cannot_read_its_sibling`
passes under an isolated temporary Arcana profile. The production AppContainer
test grants a second staged directory under the same profile runtime root and
verifies the child can read that explicit grant while sibling reads, runtime
writes, and loopback network remain denied. The previous full-suite failure
occurred when profile-global environment state changed concurrently. The
2026-10-02 full suite passes this cache/profile boundary; its only failures are
the separate Clippy config path canonicalization tests (809 passed / 2 failed).

**Where.** `arcana-core/src/platform/python_runtime.rs` ·
`arcana-core/src/agent/builtin_tools/project_command.rs` · P4-11 · P4-61 ·
TG4-18 · TG4-31.

## ADR-LOG-RESTRUCT-0183 — Make startup-cancellation proof clock-deterministic

**Status.** Applied on 2026-10-02; focused cancellation test passes.

**Decision.** Keep production startup retry delays on the existing bounded
full-jitter policy, but route delay selection through a private internal
closure so the cancellation test can inject a deterministic duration. Use
Tokio's paused clock and cancel after the first retry-scheduled notification.

**Why.** A full parallel core run exposed that the prior 20 ms wall-clock
timeout could lose the race to a randomly short full-jitter delay under load,
causing a second setup attempt before the assertion. The test was timing the
host scheduler instead of proving cancellation during backoff. This change
leaves production delay selection and limits unchanged while exercising the
same retry loop at a deterministic boundary.

**Proof.**
`cargo test -p arcana-core --lib cancellation_during_backoff_stops_before_another_attempt --locked -- --nocapture`
passes with one setup attempt and an aborted retry future during a one-second
paused-clock backoff. The 2026-10-02 full core suite also passes this test; its
only failures are the two Clippy config canonicalization tests.

**Where.** `arcana-core/src/recipes/runner.rs` · P4-52 · TG4-26.

## ADR-LOG-RESTRUCT-0184 — Start the command timeout after sandboxed child startup

**Status.** Applied on 2026-10-02; focused and full-suite production timeout
proofs pass.

**Decision.** In the nested-process cancellation regression, poll the same
production `project_command` future until the sandboxed descendant writes its
startup marker, with a separate bounded ten-second startup deadline. Only
then apply the selected 300 ms execution timeout and drop the command future
before checking that the descendant stopped.

**Why.** The full parallel suite exposed that timing out the whole launcher at
300 ms could expire during AppContainer setup under load, before the child had
started. The resulting test failed on a missing marker without exercising
process-tree cancellation. Separating bounded startup from execution tests the
intended behavior: a started command times out and its nested process tree is
terminated.

**Proof.**
`cargo test -p arcana-core --lib production_command_timeout_stops_nested_process_writes --locked -- --nocapture`
passes. The test observes the descendant startup marker within the separate
ten-second startup bound, applies the 300 ms execution timeout, drops the
production command future, sees no further heartbeat growth, and verifies no
late marker. The 2026-10-02 full core suite also passes this test; its only
failures are the two Clippy config canonicalization tests.

**Where.** `arcana-core/src/agent/builtin_tools/project_command.rs` · P4-62 ·
TG4-32.

## ADR-LOG-RESTRUCT-0185 — Propagate worker run identity into task status telemetry

**Status.** Applied on 2026-10-02; focused status capture and query registry tests pass.

**Decision.** Carry the existing optional worker `run_id` through `ToolContext` and the task-status tool into sibling run-aware `TasksDb` transition methods. Keep the existing public transition signatures as compatibility wrappers that emit the same events without a run ID when none is available.

**Why.** The worker `ToolExecutor` already owns the current run ID, but task-status telemetry lost it when constructing `ToolContext`. Adding it at that boundary provides task/run correlation where known without guessing IDs or changing existing non-worker callers. The SQL statements are unchanged; the P4-58 registry follows the new methods that now own those statements.

**Proof.** `cargo test -p arcana-core --lib worker_run_id_reaches_read_and_write_tool_contexts --locked -- --nocapture --test-threads=1` proves the executor places the same run ID into both read and write tool contexts. The production consumer test `cargo test -p arcana-core --lib worker_status_tool_carries_run_id_into_committed_database_event --locked -- --nocapture --test-threads=1` invokes `UpdateTaskStatusTool`, commits a worker status change, and asserts the captured task, worker, run ID, and success fields. `cargo test -p arcana-core --lib task_claim_and_status_emit_safe_result_events --locked -- --nocapture --test-threads=1` separately covers transition result classification. `cargo test -p arcana-core --test p4_query_registry --locked -- --nocapture` passes 8/8 after its owner fingerprints were updated. `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and `git diff --check` pass.

**Where.** `arcana-core/src/agent/tool_trait.rs`, `tool_executor.rs`, `builtin_tools/update_task_status_tool.rs`, `db/tasks.rs`, and `_verify/P4-58-QUERY-INVENTORY.md` · P4-50 · TG4-24.

## ADR-LOG-RESTRUCT-0186 — Keep SQLite statement profiling at native millisecond resolution

**Status.** Owner-selected on 2026-10-03.

**Decision.** Keep the bundled SQLite profile callback at its native millisecond granularity. Measure logical database operations and their parent/child work with the existing nanosecond `OperationTimer` spans; do not add a higher-resolution callback per connection.

**Why.** The recorded 20,000-query comparison showed that callback-only overhead was small in that fixture, while the formatter-to-sink path dominated and the callback produced only two nonzero millisecond samples. A second callback prototype is not justified by that evidence. Operation spans supply sub-millisecond timing at the meaningful database/API boundary without overstating per-statement precision.

**Proof and limit.** The diagnostic fixture recorded 10.77 ms without profiling, 14.28 ms with the bundled callback, and 483.36 ms with JSON formatting to a sink. These figures are synthetic and do not establish Fleet or disk-logging cost. TG4-24 still requires representative producer, persistence, refresh, export, privacy, failure-isolation, and CLI-to-Logs evidence.

**Where.** `arcana-core/src/observability/sqlite.rs`, `timing.rs`, and P4-50/TG4-24 · owner direction 2026-10-03.

## ADR-LOG-RESTRUCT-0187 — Persist an explicit user-owned Python fallback

**Status.** Implemented on 2026-10-03; production fallback-consumption proof remains open.

**Decision.** Keep the preferred signed Python staging path first. If it fails, use a fallback only when the host selected a signed `python.exe` beneath the current user's `%LOCALAPPDATA%\\Programs\\Python` tree in Settings. Persist that selection in the existing `config.db` `app_settings` key/value store. Do not allow workers to choose or override the host interpreter path.

**Why.** The P4-11/P4-61 contract requires staged exact runtime access and an explicit user-owned fallback, while the current PC's machine-level interpreter may not permit the sandbox to traverse or grant its source installation. Reusing the existing settings store and Tauri file picker keeps selection host-controlled and persistent; canonical containment and Authenticode checks make selection fail closed outside a signed per-user install.

**Proof and limit.** The Windows runtime tests pass 5/5, including user-root/name validation, cache invalidation when the fallback changes, and existing staged-runtime sibling-read/network denial. `Settings.test.tsx` passes 2/2 and proves the selector calls the registered read/set/clear command contract. `cargo check -p arcana --locked`, `npm run typecheck`, format, and diff checks pass. The current user-owned Python directory has no `python.exe`, so this host cannot prove the fallback execution branch. A controlled host with a valid signed per-user installation must exercise staging through `project_command` before P4-11/P4-61 close.

**Where.** `arcana-core/src/platform/python_runtime.rs`, `src-tauri/src/commands/settings_commands.rs`, `src-tauri/src/lib.rs`, and `src/tabs/Settings.tsx` · P4-11 · P4-61 · TG4-18 · TG4-31 · owner direction 2026-10-01/02.

## ADR-LOG-RESTRUCT-0188 — Correct the matched Python launcher fixture inputs

**Status.** Corrected on 2026-10-03 after independent GPT-6 Luna High review.

**Correction.** The first P4-62 cold/warm measurement copied an existing
`tests/__pycache__` from the fixture into sample zero, then removed it only
after that sample. Cold and warm project inputs were therefore not guaranteed
to match. Retract those original measurements; the plan and gate preserve the
old numbers struck through as a correction trail. The measurement test now
removes the fixture-owned bytecode directory immediately after copying and
before the cold sample, and cleans generated bytecode between every run.

**Proof.** The corrected ignored production-path test passed one cold plus five
warm runs on the same profile/workspace. Corrected cold runtime/sandbox/process/
drain was 36,386/1,030/435/<1ms; warm medians were 3,147/13/394/<1ms; warm
total range was 3,552–3,603ms. The result is fixture-specific, not an
application performance claim. The independent review and the exact command
are recorded in the P4-62 checkpoint. P4-62 remains open for other gate clauses.

**Where.** `arcana-core/src/agent/builtin_tools/project_command.rs` · P4-62 · TG4-32.

## ADR-LOG-RESTRUCT-0189 — Revalidate QC workspace fingerprints around a build batch

**Status.** Implemented on 2026-10-03; full concurrency-race closure remains unproven.

**Decision.** Capture one baseline workspace fingerprint for the selected build batch and recompute after each command outcome. If any pre/post sample differs or cannot be computed, persist observed exits without a fingerprint so neither the shared QC API nor the SQLite trigger can use them. Fingerprint regular files through an open handle and verify the path still names that same file before and after reading (Windows volume/file index; Unix device/inode).

**Why.** A fingerprint taken only before compilation could remain attached to a successful exit even if project code or another writer changed inputs while the command ran. Keeping the workspace hash as the QC binding requires checking that it still describes the workspace around the observed build.

**Proof and limit.** `workspace_fingerprint_comparison_rejects_changed_or_unavailable_samples` passes for unchanged, changed, and unavailable samples. The existing production ProjectCommand/QC regression still passes, and the fingerprint implementation now checks open-handle identity. A build-script mutation attempt through the production Windows sandbox returned a failed Cargo exit (101), so it did not prove a successful command racing a workspace write; that attempt is not counted as proof. Directory membership and a mutation restored between the pre/post samples remain possible race windows. TG4-07 stays open for a production race test and cross-platform corpus proof.

**Where.** `arcana-core/src/agent/builtin_tools/project_command.rs`, `arcana-core/src/platform/workspace_fingerprint.rs`, P4-14, TG4-07.

## ADR-LOG-RESTRUCT-0190 — Reconcile interrupted worker effects before redispatch

**Status.** Accepted from the owner's P4-53 continuity contract; aligned on 2026-10-03.

**Decision.** P4-36/S4-03 recover interrupted in-process runs by releasing only exact-owner claims, persisting the interruption, and making uncertain work visible to the Supervisor without restarting Arcana. Do not automatically redispatch a task whose prior side effects are uncertain. Resume/reassign only after persisted operation and repository state establish that replay is safe; otherwise keep the task held for explicit reconciliation.

**Why.** The owner expanded P4-53 to require reconciliation of each side effect before resuming and explicitly prohibited inferring that an unacknowledged effect did not occur. An unconditional automatic retry would contradict that safety contract and could duplicate mutations.

**Proof and limit.** Focused production timeout/quarantine and Supervisor recovery tests prove exact-owner claim holding and Supervisor visibility. Durable operation outcome/cursor, repository snapshot, and safe post-reconciliation continuation are not implemented, so P4-36/S4-03 remain open. This decision narrows automatic replay to cases proven safe; it does not waive the original recovery proof.

**Where.** `arcana-core/src/recipes/scheduler.rs`, `arcana-core/src/db/runs.rs`, P4-36, P4-53, S4-03, TG4-27.

## ADR-LOG-RESTRUCT-0191 — Preserve repeated trace span instances in Logs

**Status.** Accepted on 2026-10-03 while closing the P4-50 Logs review finding.

**Decision.** The Logs trace tree keys a span by its explicit producer span ID
when one is present. Arcana's current `tracing_subscriber` JSON formatter does
not emit span IDs, so the UI uses a record-local span identity in that case;
separate telemetry records with repeated span names therefore remain separate
visible operation nodes instead of merging their durations.

**Why.** The previous name-only child map conflated two same-named operations
within the same run. The current JSON contract does not provide enough identity
to prove that same-named spans in different records are the same live span, so
record-local identities preserve each observed operation and its timing rather
than suggesting unsupported cross-record identity.

**Proof and limit.** A Logs UI regression supplies two same-named `worker_run`
records under one run and proves the 1 ms and 2 ms events render under distinct
nodes. `npm test -- --run src/tabs/Logs.test.tsx` passes 17/17. Producers that
need cross-record span reconstruction should emit a stable explicit identity in
a future schema change; this decision does not alter telemetry event storage.

**Where.** `src/tabs/Logs.tsx`, `src/tabs/Logs.test.tsx`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0192 — Correlate startup retry events to the durable run

**Status.** Accepted on 2026-10-03 during the P4-52 retry audit.

**Decision.** Pass the worker run ID, allocated before startup preparation, into
the bounded retry helper and include it on retry-scheduled, recovered,
exhausted/permanent-failure, and retry-phase persistence events.

**Why.** Retry attempts occur within one durable worker run. Without its ID,
structured events cannot be joined reliably to the run row and worker health
context, especially when one worker retries several times in a day.

**Proof and limit.** The captured production retry-event regression verifies
that the scheduled first attempt and recovered second attempt both carry the
same run ID and correct attempt/outcome fields. The test
`startup_retry_events_include_the_durable_worker_run_id` passes 1/1. Persisted
Logs-reader readback and task-budget continuity remain unproven; P4-52/TG4-26
remain partial.

**Where.** `arcana-core/src/recipes/runner.rs`, P4-52/TG4-26.

## ADR-LOG-RESTRUCT-0193 — Report partial Memory reads as degraded operations

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 Memory
producer audit.

**Decision.** Memory lookup/list tools and the recall access-history write emit
fixed-label, result-aware operation events with existing run/task/call
correlation when available. A list or procedural search that encounters
malformed persisted data keeps valid results but reports a degraded outcome.
`list_memories` bases `truncated` on the raw SQL lookahead (including malformed
rows) and reports `malformed_count`; procedural search separates truly skipped
rows from rows retained with empty tags in `malformed_tag_sets`. Not-found
`get_memory` operations record an observed zero result with `not_found` outcome.
Failure to persist access history remains non-fatal to the primary recall,
while its own operation reports degradation.

**Why.** SQLite statement callbacks expose statement identity and elapsed time,
not the result or the partial row-decoding state. Treating surviving rows as an
unqualified success hid Memory data quality and secondary-write failures from
the Supervisor and Logs view. Fixed labels and counts preserve the privacy
boundary: memory contents, IDs, SQL, and bound values stay out of telemetry.

**Proof and limit.** Isolated production-entry tests exercise get/list Memory
tools, malformed list lookahead, procedural row and tag degradation, and
successful/failed access-history updates. A production detailed-search path
with an injected history-write failure still returns the retrieved result.
Tests assert observed-zero semantics, outcome/count, partial-result state, and
privacy. They do not complete coverage of every P4 DB or Memory producer;
TG4-24 remains open.

**Where.** `arcana-core/src/agent/builtin_tools/get_memory.rs`,
`list_memories.rs`, `arcana-core/src/memory/procedural.rs`,
`recall.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0194 — Mark operation result counts as observed or unknown

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 observability
audit.

**Decision.** Every operation timing event retains numeric `result_count` and
`result_bytes` fields for compatibility and adds `result_count_observed` and
`result_bytes_observed` booleans. The booleans are true only when a producer
explicitly measured a value, including a measured zero.

**Why.** A default numeric zero is ambiguous: it may mean an observed empty
result or an operation that never reports a result count. The expanded JSON
view needs to preserve that distinction so the Supervisor and operator do not
infer behavior from a telemetry default.

**Proof and limit.** The captured-operation tests prove explicit zero is
observed and omitted counts/bytes are marked unknown. Existing aggregate sums
remain numerically compatible; the flags do not add new frontend totals. Full
producer coverage and live Fleet telemetry remain open under TG4-24.

**Where.** `arcana-core/src/observability/timing.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0195 — Report result-aware operations for config.db consumers

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 database
producer audit.

**Decision.** The app-settings, model-config, and Arcana-instructions
repositories emit fixed-label operation events with the logical `config`
database identity, measured row/result counts, and explicit success/error
outcomes. Event fields never include setting keys or values, model IDs,
repository paths, or instruction content. The SQLite callback continues to
report per-statement duration at its native millisecond granularity; these
logical operations provide result semantics and nanosecond duration.

**Why.** The callback does not report whether a query found a row, how many
rows a write affected, or whether it failed. These repositories own frequently
used provider/workspace settings and agent instructions, so their outcomes
must be visible without exposing the values that drive the behavior.

**Proof and limit.** Isolated config.db tests read/write/delete settings,
exercise model-config lookup/upsert/delete, and exercise instruction
read/write/exists/length/delete. An injected settings trigger failure produces
an error event without reflecting its SQL error or input values. The focused
test groups pass 6/6, 1/1, and 1/1. This closes only these config.db producer
paths; full database producer and Fleet-to-Logs coverage remain open under
TG4-24.

**Where.** `arcana-core/src/db/app_settings.rs`,
`arcana-core/src/db/model_configs.rs`,
`arcana-core/src/db/arcana_instructions.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0196 — Keep consent and malformed Memory results observable safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
review.

**Decision.** Workspace egress-consent DB operations emit fixed operation
labels, the `config` database identity, observed result counts, and `allowed`
or `denied` outcomes. They do not copy workspace paths or provider
destinations into telemetry. Detailed Memory search carries the production
tool's task/call/run/role correlation. An invalid persisted timestamp keeps
the search result available with a current-time compatibility value, but
marks the result degraded and logs only a fixed warning, never the raw stored
value or parser error.

**Why.** Consent is a production gate on app-mediated provider egress and its
decision latency/outcome must be diagnosable. Memory search also needs the
correlation already available to its caller, while malformed persisted values
must not become arbitrary log content or appear as a healthy result.

**Proof and limit.** Isolated production-method capture proves consent grant,
revoke, read, allowed and denied outcomes, observed counts, and omission of
workspace/provider identifiers (3 consent tests pass). Memory tests prove the
production search preserves caller correlation, labels malformed timestamps
degraded, and excludes both the malformed value and content from telemetry
(79 Memory tests pass). These paths do not close other DB/Memory producer
coverage or TG4-24's end-to-end Fleet-to-Logs, failure-isolation, and cost
requirements.

**Where.** `arcana-core/src/egress_consent.rs`,
`arcana-core/src/memory/recall.rs`,
`arcana-core/src/agent/builtin_tools/search_memory.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0197 — Add result-aware timings to daily-cost analytics

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Daily-cost insert, point-read, and range-read operations emit
fixed-label `analytics` DB timing events with explicit result counts and
success/error outcomes. The event excludes cost values, breakdown contents,
dates, and SQL error details.

**Why.** These paths feed the operator-facing spend dashboard and exports. A
statement-duration callback alone cannot distinguish a missing daily record,
a populated range, or a failed writer; operation events must report those
results without copying financial/provider labels into structured telemetry.

**Proof and limit.** The isolated production-method test covers absent and
present reads, a two-row range, successful inserts, and a trigger-injected
insert error; it verifies result counts and privacy (1/1). Other analytics
families and end-to-end Fleet/Logs behavior remain open under TG4-24.

**Where.** `arcana-core/src/analytics/db.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0203 — Observe task-budget persistence without budget values

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Task-budget set, usage update, exceeded-state write, per-task
status read, and daily summary read emit fixed-label `tasks` operation events
with task correlation, row/result counts, and outcomes. Limits, spend/token
values, and free-form exceeded reasons stay out of telemetry.

**Why.** P4-29 and P2 daily-cap behavior need observable persistence and reader
outcomes, but logging budget values or user-provided reasons would turn timing
events into a second financial/project-data store.

**Proof and limit.** The isolated TasksDb test checks budget persistence and
daily-summary readback, a trigger-injected usage-write failure, and value/reason
privacy (1/1). This does not prove the budget interaction contract or close
TG4-24.

**Where.** `arcana-core/src/db/tasks.rs`, P4-29/P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0198 — Time high-fan-out GraphStore readers

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Graph file listing, paged node/edge reads, node search, and graph
statistics emit fixed-label `graph` DB operation timings with explicit result
counts and success/error outcomes. Node names, paths, search text, and DB paths
are excluded from telemetry.

**Why.** Graph build stages already had timing, but the UI and agent graph
consumers also spend time on independent database reads. Without logical
events, search and rendering latency could not be separated from graph-build
latency, and direct inputs/rows cannot be copied into the log safely.

**Proof and limit.** A seeded production GraphStore test exercises populated
file/node/search/stats reads plus an empty edge page and verifies counts and
privacy (1/1). Mutation methods and other database families remain open; this
does not close TG4-24.

**Where.** `arcana-core/src/graph/store.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0199 — Time primary task-board and claim reads

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** TasksDb creation, task-ID lookup, worker claimed-task lookup,
board list, and worker/status list emit fixed-label `tasks` operation events
with explicit counts and success/error outcomes. Missing ID lookup is
`not_found`; no claimed task is `empty`. Task title, description, status
filter, and other task text remain out of event fields; task/worker correlation
uses only the existing typed IDs and worker identity fields.

**Why.** These readers are on the Plan Board, Supervisor and worker dispatch
paths. SQLite statement durations alone do not show whether a board returned
rows, a task ID was missing, or a worker had no claim.

**Proof and limit.** The isolated migrated TasksDb test exercises production
create/read/list methods, empty and populated claim reads, successful task
claim, and task-title exclusion (1/1). Build evidence, message inbox, budget,
epic/QC methods and the complete Fleet-to-Logs path remain open under TG4-24.

**Where.** `arcana-core/src/db/tasks.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0200 — Observe the ACP inbox without recording message bodies

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Worker message insert/read, inbox, stale/task/recent reads,
unread/unacknowledged counts, and acknowledgement emit fixed-label `tasks`
database operation timings with result counts and outcomes. Duplicate or
missing acknowledgement is `not_found`. Message bodies and message payloads
are never copied into event fields.

**Why.** ACP intentionally delivers messages at least once until acknowledgement
and the Supervisor/worker UI must diagnose delays and replay state. The
statement callback alone cannot distinguish empty inboxes, unacknowledged
replays, or duplicate acknowledgements; recording the body would expose
arbitrary project/user data.

**Proof and limit.** A production TasksDb test exercises insert/read, inbox
delivery, unread/unacknowledged counts, task/stale listing, successful ack,
duplicate ack classification, and body privacy (1/1). This does not prove
scheduler retry end-to-end or close TG4-24.

**Where.** `arcana-core/src/db/tasks.rs`, P4-23/P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0201 — Measure Supervisor worker-request operations

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Worker-request create/read/list/update operations emit fixed-label
`tasks` DB timings with observed counts and outcomes. Available task and worker
identities are attached through the existing typed correlation fields;
request payload and resolution note are excluded.

**Why.** Worker-to-Supervisor escalation is part of Fleet dispatch and recovery.
Without operation results, the Supervisor cannot distinguish an empty queue,
a persisted pending request, and a successfully resolved request. Payloads can
contain project data, so the event records lifecycle results only.

**Proof and limit.** An isolated production-method capture covers creation,
readback, pending/completed queue reads, resolution, counts, and payload/note
privacy (1/1). Other task database families and TG4-24's full reader/UI and
failure-isolation requirements remain open.

**Where.** `arcana-core/src/db/tasks.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0202 — Time legacy analytics failure/event repositories safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** API-failure and system-event insert/read operations emit fixed-
label `analytics` DB timing with explicit counts and success/error outcomes.
Provider IDs, error text/type/request IDs, event message/component/type, and
metadata stay out of these operation events.

**Why.** These records contain arbitrary provider and project diagnostics and
feed analytics views. The operation span needs to show write failures and
reader volume while preserving the source record as the only place where
diagnostic details are stored and access-controlled.

**Proof and limit.** An isolated AnalyticsDb test covers insert and filtered
readback for both tables, an injected SQL failure, counts, outcomes, and
privacy markers (1/1). It does not complete remaining AnalyticsDb coverage or
TG4-24 end-to-end observability.

**Where.** `arcana-core/src/analytics/db.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0204 — Observe build evidence and routing history safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Task-scoped build-evidence batches/readback and routing-history
insert, update, list, and aggregate methods emit fixed-label `tasks` operation
timings with available typed task/worker/session correlation, measured counts,
and error/not-found outcomes. Commands, workspace paths/fingerprints, titles,
and arbitrary outcome values remain excluded.

**Why.** These records authorize QC and feed automatic worker-affinity
decisions. Their duration and persistence result should be observable without
copying project or task content into telemetry.

**Proof and limit.** Production-method tests cover build evidence insert/read,
rollback-on-trigger-error, routing assignment/outcome/history/statistic
readback, missing outcome targets, and content/SQL-error privacy (1/1 each).
This does not resolve the pending P4-14 batch-completeness contract or close
TG4-24.

**Where.** `arcana-core/src/db/tasks.rs`, P4-14/P4-50/TG4-07/TG4-24.

## ADR-LOG-RESTRUCT-0205 — Observe hook persistence without hook content

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** HooksDb configuration CRUD, hook-type reads, execution-log
insert, and execution-log reads emit fixed-label `config` operation timings
with result counts and success/error/not-found outcomes. Hook identifiers,
names, actions, shell commands, working directories, output and error text
remain excluded.

**Why.** Hook lookup and log persistence are part of relevant agent tool/loop
paths, and the UI reads those persisted outcomes. Timing should be available
when optional hook logging fails without exposing executable content or tool
results.

**Proof and limit.** An isolated production-config DB test covers hook
insert/list/read/toggle/delete and execution-log write/readback plus a
trigger-injected logging failure and privacy markers (1/1). This does not prove
logging failure isolation in a live AgentEngine path or close TG4-24.

**Where.** `arcana-core/src/hooks/db.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0206 — Observe run lifecycle and recovery ledger operations

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Generic RunsDb begin/finish/read/recent/open operations and
worker-tick interruption/dead-run reaping emit fixed-label `runs` operation
timings with result counts and run/worker correlation where an existing record
supplies a trusted identifier. A missing run lookup does not echo its input as
a correlation key. Actor, run detail, failure code, and database path remain
excluded.

**Why.** These APIs support CLI/desktop lifecycle and scheduler recovery, and
their outcomes are needed to distinguish empty, interrupted, reaped and failed
run paths without duplicating user-provided run details into telemetry.

**Proof and limit.** `run_ledger_lifecycle_and_recovery_emit_private_result_spans`
passes 1/1 against an isolated migrated DB, including begin/read/finish,
not-found and duplicate finish, worker-tick interruption, dead-process reaping,
and an injected insert failure. This is producer proof only; it does not close
TG4-24 or prove recovery event readback in a live Fleet run.

**Where.** `arcana-core/src/db/runs.rs`, P4-36/P4-50/P4-53/TG4-24.

## ADR-LOG-RESTRUCT-0207 — Observe worker registry persistence safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Worker registry lookup/create/update/soft-delete and slug
existence operations emit fixed-label `workers` DB timings with observed result
counts and not-found/error outcomes. A stored worker slug may be used as the
stable worker correlation; names, models, repository paths, recipes and
descriptions remain excluded.

**Why.** Worker setup and lifecycle feed Fleet dispatch, settings, CLI, and
recovery. Safe operation timings make those paths diagnosable while preserving
private configuration and repository information.

**Proof and limit.** `worker_registry_crud_operations_emit_private_result_spans`
passes 1/1 against the migrated isolated worker DB, covering create/read/model
and repository update/recipe bind/slug checks/soft-delete and an injected
create failure. This does not close TG4-24 or prove full desktop/CLI producer
readback.

**Where.** `arcana-core/src/db/workers.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0206 — Observe run lifecycle and recovery ledger operations

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Generic RunsDb begin/finish/read/recent/open operations and
worker-tick interruption/dead-run reaping emit fixed-label `runs` operation
timings with result counts and run/worker correlation where an existing record
supplies a trusted identifier. A missing run lookup does not echo its input as
a correlation key. Actor, run detail, failure code, and database path remain
excluded.

**Why.** These APIs support CLI/desktop lifecycle and scheduler recovery, and
their outcomes are needed to distinguish empty, interrupted, reaped and failed
run paths without duplicating user-provided run details into telemetry.

**Proof and limit.** `run_ledger_lifecycle_and_recovery_emit_private_result_spans`
passes 1/1 against an isolated migrated DB, including begin/read/finish,
not-found and duplicate finish, worker-tick interruption, dead-process reaping,
and an injected insert failure. This is producer proof only; it does not close
TG4-24 or prove recovery event readback in a live Fleet run.

**Where.** `arcana-core/src/db/runs.rs`, P4-36/P4-50/P4-53/TG4-24.

## ADR-LOG-RESTRUCT-0207 — Observe worker registry persistence safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer
audit.

**Decision.** Worker registry lookup/create/update/soft-delete and slug
existence operations emit fixed-label `workers` DB timings with observed result
counts and not-found/error outcomes. A stored worker slug may be used as the
stable worker correlation; names, models, repository paths, recipes and
descriptions remain excluded.

**Why.** Worker setup and lifecycle feed Fleet dispatch, settings, CLI, and
recovery. Safe operation timings make those paths diagnosable while preserving
private configuration and repository information.

**Proof and limit.** `worker_registry_crud_operations_emit_private_result_spans`
passes 1/1 against the migrated isolated worker DB, covering create/read/model
and repository update/recipe bind/slug checks/soft-delete and an injected
create failure. This does not close TG4-24 or prove full desktop/CLI producer
readback.

**Where.** `arcana-core/src/db/workers.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0208 — Instrument graph and session persistence without payloads

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Reuse the existing `OperationTimer` for fixed-label spans across public `GraphStore` and `SessionStorage` DB operations. Emit only the logical database identity, duration, measured result count/bytes, and existing safe task/session correlation. Keep graph names, source paths, query text, snapshot JSON, session titles, provider/model IDs, message/tool payloads, and DB filesystem paths out of telemetry. Remove the pre-existing GraphStore open diagnostics that rendered its filesystem path in ordinary logs.

**Why.** SQLite statement callback samples retain native millisecond precision; logical operation spans expose nanosecond elapsed time across connection setup, transactions, row mapping, and returned payload handling. Structured payload capture would create unnecessary privacy exposure and unbounded cardinality.

**Proof and limit.** `graph::store::timestamp_tests` passes 3/3, including operation coverage, persisted reads, not-found results, and a trigger-injected batch write failure. `session::storage::tests` passes 3/3, including CRUD/readback, schema migration/version, missing rows, and a trigger-injected update failure. Capture tests assert private graph/session content, paths, and SQL failure details are absent. This does not close TG4-24 or prove a live Fleet-to-Logs path.

**Where.** `arcana-core/src/graph/store.rs`, `arcana-core/src/session/storage.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0209 — Observe QC claim lifecycle without reviewer payloads

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Instrument TasksDb QC claim try/release/upgrade/list and cost-add operations with fixed-label `tasks` spans, task ID correlation, measured row counts, and explicit not-claimed/not-found/skipped/error outcomes. Do not emit reviewer/subagent IDs, task content, cost values, or SQL error details.

**Why.** QC anti-double-spawn claims and reconciliation are production Fleet control paths. Their latency and conflict/recovery outcomes need visibility while reviewer handles and cost payloads remain private.

**Proof and limit.** `qc_claim_and_cost_operations_emit_private_result_spans` passes 1/1 against a migrated isolated TasksDb, covering duplicate claim refusal, upgrade/list/release, missing rows, zero-cost no-op, and trigger-injected update failure. This adds producer evidence but does not close TG4-24 or prove full Fleet-to-Logs consumption.

**Where.** `arcana-core/src/db/tasks.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0210 — Observe epic and phase task persistence safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Instrument epic creation, phased-task creation, epic task listing, phase summaries, and phase-ready task selection with fixed-label `tasks` spans. Emit the inserted task's integer ID only after it is read back successfully; do not emit epic identifiers, phase names, titles, criteria, milestones, descriptions, or creator values.

**Why.** PA and Supervisor planning workflows create and inspect phased work through TasksDb. The fixed labels and result counts expose persistence latency and empty/failure outcomes without making user planning content part of logs.

**Proof and limit.** `epic_phase_operations_emit_private_result_spans` passes 1/1 against a migrated isolated TasksDb, including create/readback, phase summary/ready selection, empty epic results, and a trigger-injected create failure. This does not close TG4-24 or prove live Fleet-to-Logs readback.

**Where.** `arcana-core/src/db/tasks.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0211 — Observe task retry and destructive board operations safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Instrument TasksDb retry increment/count read, diff-analysis persistence, single-task deletion, and clear-board operations with fixed-label result spans. The clear-board path retains backup-before-delete and emits a structured backup event containing only a fixed operation, reason, and byte count; do not log the backup path, diff payload, retry values, or SQL error details.

**Why.** Retry and diff persistence are part of worker diagnosis, while deletion and clear-board are operationally significant. Their outcomes and duration should be visible without turning user data or filesystem paths into telemetry. The clear-board event must also preserve backup-first behavior while replacing the prior path-bearing log.

**Proof and limit.** `task_retry_diff_and_delete_operations_emit_private_result_spans` and `clearing_the_board_backs_up_first_and_the_backup_holds_the_tasks` pass 1/1 each against isolated production TasksDb methods, including injected SQL failures, result readback, backup-first behavior, and privacy assertions. This does not close TG4-24 or prove live Fleet-to-Logs consumption.

**Where.** `arcana-core/src/db/tasks.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0212 — Attribute token analytics subquery latency

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Add fixed operation labels and observed result counts for token usage grouped by model, provider, and agent type, plus the top-session reader. Keep the parent date-range operation and emit no grouping values, session identifiers, provider/model labels, or SQL detail.

**Why.** The dashboard range operation runs several sequential aggregate queries. A parent duration alone cannot identify which query dominates latency; child spans make those sub-operations visible without high-cardinality or private fields.

**Proof and limit.** `worker_stats_and_live_token_aggregates_emit_safe_result_spans` passes 1/1 and checks the four child spans, observed counts, and privacy. This does not close TG4-24 or establish end-to-end dashboard cost.

**Where.** `arcana-core/src/analytics/db.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0213 — Measure shared SQLite migration ladders safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Time the shared `run_ladder` operation with a fixed operation label, allowlisted logical database identity, applied-rung count, and result outcome. Unknown database labels map to `other`. Do not include database paths, arbitrary names, migration names, SQL, or failure strings in this operation event.

**Why.** SQLite statement sampling shows individual statements at millisecond precision, but cannot show the complete migration/startup duration or classify a failed/no-op ladder. The logical operation complements statement timings without copying migration input into telemetry.

**Proof and limit.** `migration_ladder_emits_safe_result_span` passes 1/1 for migrations applied, a current schema, an empty ladder, and a failing SQL rung. It verifies outcome/count semantics and path/name/SQL exclusion. This does not close TG4-24 or provide startup cost evidence from a real Fleet run.

**Where.** `arcana-core/src/db/migrate.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0214 — Record spend-cap decisions and orphan scan failures

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Emit fixed-label config-operation outcomes for atomic spend reservation and settlement, including grant/deny/disabled/settled/not-found/error states. Instrument the orphan-candidate selector with an observed row count and distinguish empty from success and error. Never emit reservation IDs, spend values, task content, SQL errors, or paths.

**Why.** A provider cap is a control boundary whose transaction-level outcome is not visible through statement timings. Orphan reconciliation previously could turn a failed candidate scan into an empty result, obscuring a recovery failure from the Supervisor. These events make both decisions visible while keeping private values out of telemetry.

**Proof and limit.** `provider_budget_reservation_emits_decisions_without_private_values` and `orphan_candidate_scan_distinguishes_empty_rows_and_database_failure` pass 1/1 each against isolated databases, including injected insert/query failures, no-op/empty cases, and privacy assertions. This does not close TG4-24 or prove live Fleet-to-Logs readback.

**Where.** `arcana-core/src/budget/daily.rs`, `arcana-core/src/db/tasks.rs`, P2/TG2-17 and P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0215 — Time checkpoint recovery operations safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Add fixed-label operation timings to checkpoint create, restore, list, compare, delete, and automatic pre-write creation. Keep safe session/task correlation; record observed result counts/bytes and distinguish empty/not-found results. Do not log checkpoint IDs, file/snapshot paths, reasons, or file/diff contents.

**Why.** Checkpoints are the durable rollback boundary around agent file mutations. They combine database and filesystem work, so SQLite statement timings cannot represent recovery-operation latency or identify missing/empty results.

**Proof and limit.** `checkpoint_lifecycle_operations_emit_private_result_spans` passes 1/1 over the production API with snapshot comparison, listing, restore, delete and auto-checkpoint behavior. It verifies restored file content and asserts path/content/reason/ID privacy. This does not close TG4-24 or prove worker recovery from the Logs UI.

**Where.** `arcana-core/src/git/checkpoint.rs`, P4-50/TG4-24 and P4-53.

## ADR-LOG-RESTRUCT-0216 — Keep CLI analytics telemetry at the host boundary

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Emit result-aware operation spans from the `arcanactl` analytics command boundary for cost, token, worker-stat and export reads. Keep `arcana-shared` as a dependency-light reader and avoid adding a dependency from it to `arcana-core`. Export records a parent event and fixed child operations; none includes user filter values, row contents, worker IDs, output paths, or error strings.

**Why.** A direct core dependency in the shared query crate would invert its existing host dependency boundary. The production consumer already links both crates, so telemetry belongs at the CLI boundary without coupling the reusable reader layer.

**Proof and limit.** `analytics_query_wrapper_emits_safe_success_empty_and_error_spans` passes 1/1 and verifies result and privacy fields. Existing profiled connection coverage remains separate. This does not close TG4-24 or prove a full CLI Fleet-to-Logs workflow.

**Where.** `src-cli/src/commands/analytics.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0217 — Observe IDE and GitHub MCP configuration reads safely

**Status.** Accepted and implemented on 2026-10-03 during the P4-50 producer audit.

**Decision.** Instrument persisted IDE workspace-state load/save and GitHub MCP connector configuration read/write operations with fixed labels, logical database identity, measured counts, and result outcomes. Malformed IDE state remains a safe empty fallback but is reported as degraded. Do not emit workspace paths, serialized state, connector endpoints, organization/repository identifiers, SQL errors, or configuration payloads.

**Why.** IDE state and connector configuration are user-visible control-plane operations backed by SQLite; statement-level sampling alone does not explain their operation duration or whether the API returned empty, degraded, or failed state.

**Proof and limit.** `workspace_state_operations_emit_safe_result_spans` and `github_mcp_configuration_operations_emit_private_result_spans` pass 1/1 each against isolated storage, covering default/empty, populated, malformed, and injected storage failure states with privacy assertions. This is producer coverage only and does not prove connector invocation telemetry or full TG4-24.

**Where.** `arcana-core/src/ide/workspace.rs`, `src-tauri/src/commands/github_mcp_commands.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0218 — Require complete project-command batches for QC

**Status.** Owner-approved contract; implementation and focused proof added on 2026-10-03. P4-14/TG4-07 remain open for production mixed-project failure coverage and the cross-platform process corpus.

**Decision.** Persist one task-scoped `running` batch record before any selected `project_command` build child starts, then finalize that same record atomically with observed child exits. QC may use build evidence only when the latest batch is finalized, every selected command has an observed successful result, and the batch workspace provenance still matches the selected workspace. A timeout, spawn failure, cancellation, process loss, missing result, or failed command leaves the batch ineligible; a crash before finalization leaves the durable row `running` and blocks any older pass. Preserve actual per-command observations; never synthesize process exit codes. The same `TasksDb` transition contract and SQLite boundary must refuse `qc_review` when the latest batch is incomplete or failing.

**Why.** Per-command passing rows let one successful project in a multi-project selection authorize QC while a sibling command timed out or failed to spawn. One batch-level finalization makes completeness explicit and durable across API and direct-SQL transitions.

**Proof and limit.** Migration 19 adds the durable batch table, observed-count/evidence linkage and batch-aware SQLite QC triggers for both UPDATE and direct INSERT. `project_build_batches_block_partial_and_interrupted_qc_until_complete_pass` proves an observed passing sibling plus a missing child stays blocked through both the shared API and direct SQL; a still-running row survives DB reopen and blocks QC; a complete two-project passing batch with a current fingerprint is accepted. `shipped_project_command_persists_spawned_build_results_and_qc_reads_them` exercises the production command path through pass, workspace mutation, timeout, API/direct-SQL refusal, a fresh passing rerun and acceptance. The direct INSERT guard regression also passes. The production mixed-project timeout/spawn/cancel matrix and cross-platform command corpus remain open; no timeout or spawn exit is fabricated.

**Where.** `arcana-core/src/agent/builtin_tools/project_command.rs`, `arcana-core/src/db/tasks.rs`, P4-14/TG4-07.

## ADR-LOG-RESTRUCT-0219 — Persist operation continuity in a dedicated RunsDb journal

**Status.** Accepted by the owner on 2026-10-03; journal migration, production writer/reader, and cancellation behavior are partially implemented and focused-tested. Recovery/reassignment proof remains pending.

**Decision.** Add a dedicated append-only operation journal table to the existing `runs.db`, linked to durable worker run and task IDs. Keep the bounded `worker_run_events` table as a summary stream. Journal deterministic operation intent before execution, then append observed outcome or `unknown` when acknowledgement is lost; retain a durable operation cursor, safe changed-path references, and reconciliation state needed by replacement/resumed workers. Treat a lost acknowledgement as an uncertain side effect and reconcile against current task/workspace/Git state before replay. Do not store raw tool arguments, source contents, or unbounded provider results.

**Why.** The current recovery ledger can identify interrupted runs but cannot say which mutations completed, where to continue, or which side effects remain uncertain. A separate append-only table avoids rebuilding the bounded summary event schema and preserves audit history.

**Proof and limit.** Migration v4, append-only intent/outcome/reconciliation rows, cursor paging, bounded workspace-relative targets, and the production `ToolExecutor` writer plus Supervisor-only `list_worker_health` reader are implemented. Focused RunsDb paging/migration/bounds tests, producer failure/unknown-outcome tests, Supervisor journal readback tests, and `cancellation_drops_inflight_run_side_effect_and_leaves_unknown_journal_intent` pass. Cancellation drops the in-flight effect and leaves an unresolved intent for the Supervisor. Still required: cancellation or panic after multiple mutations, lost-ack reconciliation against task/workspace/Git state without duplicate effects, replacement-worker continuation, and task-scoped snapshot/restore safety. No automated replay or resume is claimed.

**Where.** `arcana-core/src/db/runs.rs`, P4-36/P4-53/P4-54 and TG4-26/TG4-33.

**2026-10-03 continuation.** A structured `outcome_unknown` marker on tool errors is now consumed by the journal writer; the regression proves it persists `Unknown` and does not retry the effect (1/1). Bash/project-command timeouts and post-dispatch MCP errors set the marker. Supervisor reads now accept independent cursors keyed by worker run; the reader suite passes 6/6, and the focused journal tests pass 6/6. These changes correct outcome attribution and paging only. Actual state reconciliation, replacement-worker continuation, and no-duplicate recovery are still open.

## ADR-LOG-RESTRUCT-0220 — Add consent-gated external MCP invocation to P4

**Status.** Accepted by the owner on 2026-10-03; shared-core tools, consent/audit integration, and local Streamable HTTP protocol proof are partially implemented. Desktop/CLI full consumer and resilience proof remain pending.

**Decision.** Add an external MCP client to the shared Rust runtime so both desktop and CLI Fleet turns can discover and call tools from configured Streamable HTTP connectors. Use the official Rust MCP SDK `rmcp` pinned to version `3.5.0` with only the Streamable HTTP client features needed by the configured endpoint contract; do not launch stdio servers because the current persisted connector configuration has no executable/argument contract. Register bounded, connector-namespaced proxy tools in each agent engine. Immediately before each call, require and durably audit workspace-scoped consent for that connector; fail closed on absent, revoked, or unreadable consent. Preserve existing interactive `Network` approval behavior. Apply finite deadlines and cancellation, never automatically retry an ambiguous call, bound results before any tool-result consumer, and record safe correlated outcome/duration/size telemetry without arguments, results, credentials, endpoint URLs, or raw remote errors.

**Why.** P4-50/TG4-24 explicitly requires MCP call outcomes, but current Arcana code only stores connector settings and performs HTTP health probes. Shared-core ownership makes the production path consistent for Tauri and CLI without duplicating invocation logic.

**Research.** The official Rust SDK documents `rmcp`'s Streamable HTTP client transport, lifecycle client, and cancellation behavior; the current tool specification requires user-visible tool use, validation, timeout and audit handling. Sources: [official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/README.md), [official client example](https://github.com/modelcontextprotocol/rust-sdk/blob/main/examples/clients/src/streamable_http.rs), [MCP tools specification](https://modelcontextprotocol.io/specification/2026-07-28/server/tools), [Streamable HTTP specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http).

**Proof and limit.** Exact `rmcp` 3.5.0 client dependency and Streamable HTTP transport are pinned. Shared-core `mcp_list_tools`/`mcp_call_tool` tools use the configured GitHub HTTP endpoint, per-call workspace egress-consent audit, `Network` permission effects, bounded metadata/arguments/results, no ambiguous automatic replay, fixed-label result telemetry, and explicit stdio refusal. `streamable_http_client_discovers_and_invokes_a_local_mcp_tool` exercises the real Rust SDK client against a local protocol fixture (1/1); endpoint/size policy tests pass (4/4 MCP tests total). `configured_mcp_destination_is_scoped_granted_revoked_and_audited` proves denied/granted/revoked audit readback (1/1). Shared core check passes with `--locked`. Still required: production configured-tool execution through AgentEngine in both desktop and CLI, permission-mode/timeout/cancel/error and oversized-wire response cases, telemetry persistence/Logs readback, and credential/host scope review. Stdio remains fail-closed.

**Where.** `arcana-core/src/agent/engine.rs`, `arcana-core/src/agent/tool_executor.rs`, shared MCP module, consent/runtime wiring, P4-50/TG4-24/TG4-25/TG4-26.

**2026-10-03 continuation.** MCP call telemetry now also records bounded invalid-input, authorization-failure, connection-failure, and result-rejection outcomes, besides timeout, protocol, remote-tool and success results. The local MCP suite passes 6/6, including a captured failure event that excludes remote tool names and private context markers; workspace Rust check passes. Actual configured desktop/CLI AgentEngine invocation and telemetry persistence/Logs readback remain open. The owner has been asked whether to add an RMCP pre-parse body-cap transport and whether GitHub org/repository scope must be enforced by the client; those security-contract choices remain pending.

**2026-10-03 revocation-race proof.** Shared MCP invocation now performs an authorization/audit check immediately before `tools/call`, after the connection handshake. The local real-RMCP fixture allows one call, simulates revocation before a second call, and asserts the second call never reaches the server; focused test 1/1, MCP module 6/6. This closes that race in the helper path only. Configured desktop/CLI AgentEngine exercise, bounded raw response bodies, remote cancellation, connector scope, and telemetry persistence/Logs remain open.
**2026-10-03 persisted revocation regression.** The fixture now uses an isolated real SQLite consent store: it grants, completes an MCP handshake, revokes the persisted grant, and exercises the production authorization gate; the server observes no second `tools/call`. The consent decision and audit record are written in one immediate transaction. Commit is the authorization decision point; a revoke after commit does not retroactively cancel an authorized in-flight request. Focused fixture 1/1, MCP module 6/6, and consent module 4/4 pass. Full desktop/CLI consumer, wire-body cap, remote cancellation, scope, and Logs proof remain open.

**2026-10-04 redirect regression.** `streamable_http_client_does_not_follow_endpoint_redirects` drives Arcana's production `connect_to_endpoint` through a local HTTP fixture that responds with a redirect to a second local listener. The MCP handshake fails and the redirect target receives no connection; all 9 focused MCP tests pass. This proves redirect refusal for the pinned RMCP production adapter without a remote server or credential. Raw response-body limits before parsing, desktop/CLI MCP Logs readback, and the other listed host/security cases remain open.

**2026-10-03 claim-attribution correction.** Worker engines begin with no task ID, while `claim_task` commits ownership before the engine can bind the claimed task into `ToolContext`. The operation-intent writer now derives a positive task ID from the validated-shaped `claim_task.task_id` input for journal attribution, falling back to runtime context where available; unrelated tools continue using only the runtime task ID. If the claim succeeds but the follow-up budget read fails, the tool now marks the result `outcome_unknown` so the journal does not record a definite failure for an already-committed claim. A production RunsDb-backed executor regression verifies the pre-handler claim intent stores the requested task ID (1/1).

**2026-10-03 correction.** ~~This fixes attribution and failure classification only; it does not implement the reconciliation action or unblock/reassignment guard required by P4-53.~~ The later task side-effect recovery continuation added an immediate-transaction unresolved-intent barrier before ToolExecutor dispatch and a unique one-decision-per-intent constraint. Current revalidation passes `unresolved_task_effect_blocks_new_intents_until_reconciled`, `operation_reconciliation_has_one_durable_winner`, and `unresolved_prior_task_effect_blocks_executor_handler_until_reconciled` (1/1 each). This closes the atomic side-effect redispatch bypass. A production Supervisor/host action for inspecting and resolving journal state, filesystem/Git/checkpoint reconciliation, complete mutation capture, and replacement-worker continuation/no-duplicate proof remain open.

## ADR-LOG-RESTRUCT-0221 — Correct the P4 gate count

**Status.** Accepted correction on 2026-10-03.

**Retraction.** ~~TG4-01–32 has 9 passed and 23 open, with TG4-33 also passing.~~

**Correction.** The active `TESTING_GATES.md` checkboxes show **8 passed and 24 open among TG4-01–32**. TG4-33 is a separately listed passing scheduler/QC-worker smoke gate; it is not part of that 32-gate range.

**Why.** The prior recount added the separately listed TG4-33 to the first 32-gate denominator while counting pass states, creating an off-by-one error in the status report. The plan's TG4 range and checked rows were re-counted directly.

**Effect.** No task or gate status changes. P4 remains open; the readiness report and user-facing count use 8/32 plus TG4-33 separately.

**Where.** `docs/restructurization/development/TESTING_GATES.md`, `READINESS_MATRIX.md`, P4 closeout status.

## ADR-LOG-RESTRUCT-0222 — Record cancellation of in-flight MCP operations

**Status.** Accepted and implemented on 2026-10-03; focused MCP telemetry suite passes 8/8.

**Decision.** When the AgentEngine drops an in-flight MCP handler because its run is cancelled, emit one bounded `mcp_call` event with `outcome=cancelled`, safe operation identity, known run/task/call/role correlation, and elapsed nanoseconds. Do not record the endpoint, remote tool name, arguments, response, or cancellation error text. Normal completed outcomes continue to use their existing result event and must not emit a second cancellation record.

**Why.** Arcana treats MCP calls as potentially mutating external side effects and does not retry ambiguous outcomes. A cancellation currently drops the handler future before its normal result instrumentation, leaving the MCP-specific trace without an outcome even though the general tool journal records an unknown effect.

**Proof and limit.** `aborted_mcp_handler_emits_one_safe_cancelled_outcome` aborts the tracked handler future and verifies one cancellation event with known safe correlation and without session/workspace markers. `completed_mcp_handler_does_not_emit_a_second_cancellation_outcome` proves normal completion adds no duplicate cancellation record. The complete focused MCP module passes 8/8. `streamable_http_client_discovers_and_invokes_a_local_mcp_tool` invokes the actual `McpListToolsTool.execute` and `McpCallTool.execute` handlers using isolated production connector/consent databases against a local RMCP endpoint (1/1). This proves shared-core tool-handler execution, not a provider-driven AgentEngine turn. Remote-server cancellation acknowledgement, configured desktop/CLI AgentEngine execution, and persisted Logs readback remain open.

**Where.** `arcana-core/src/agent/builtin_tools/external_mcp.rs`, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0223 — Preserve permission outcomes and filter echoed tool arguments in failures

**Status.** Accepted and implemented for the shared production ToolExecutor; all-builtin and full privacy audit remain open.

**Decision.** Keep explicit user denial, approval timeout, session cancellation, and a dropped approval listener as separate versioned JSON errors (`permission_denied`, `permission_timed_out`, `operation_cancelled`, and `permission_channel_closed`). Every non-allow path remains fail-closed and returns a failed tool result with executor-owned operation/tool and call correlation. Before a handler failure reaches event, persistence, or model consumers, remove exact echoed command, prompt, source, and path argument values from bounded cause text, corrective guidance, and diagnostic path fields; retain safe Arcana-generated command identity and stable correlation fields.

**Why.** Collapsing timeout/cancellation into user denial makes the Supervisor misclassify a transient or terminal run state. Builtin handlers also often format invalid input and OS failures into prose; bounded truncation and credential-pattern redaction alone do not prevent exact prompt/source/command/path echo. The production executor has the raw tool input at the one shared failure boundary, so this filter covers read and write handlers without changing each tool's successful output contract.

**Proof and limit.** `approval_cancellation_is_not_reported_as_user_denial` drives a real ToolExecutor cancellation and proves a correlated cancellation envelope, no handler invocation, and pending-approval cleanup. `approval_timeout_and_closed_listener_have_distinct_decisions` proves the separate timeout and listener outcomes. `executor_context_removes_echoed_command_prompt_source_and_path_arguments` plants all four value types and verifies they are absent while the error and call correlation remain. `agent::tool_trait::tool_result_error_tests` passes 8/8, `agent::tool_executor::p8_execution_registry_tests` passes 15/15, the production AgentEngine unknown-tool consumer test passes 1/1, `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and `git diff --check` pass. This is not the every-builtin/provider error matrix, complete short-argument/OS-error privacy proof, or retry-safety audit required to close P4-51/TG4-25.

**Where.** `arcana-core/src/agent/tool_trait.rs`, `arcana-core/src/agent/tool_executor.rs`, P4-51/TG4-25, GOAL and P4 checkpoint.

## ADR-LOG-RESTRUCT-0224 — Gate task side effects on operation reconciliation

**Status.** Implemented on 2026-10-03; focused RunsDb and production ToolExecutor regressions pass.

**Decision.** Before any run-scoped non-board side effect with a task ID, acquire SQLite's immediate write transaction, check for an earlier unknown operation whose reconciliation remains unresolved or do-not-retry, then append the new intent in that same transaction. If one exists, return a structured `operation_reconciliation_required` result and do not call the handler. A retry-safe or reconciled decision allows the next intent. Enforce one reconciliation row per intent with a partial unique index in RunsDb migration 5. Continue to treat unavailable journal storage as fail-closed.

**Why.** The earlier claim-time check allowed a later path to reach the ToolExecutor without rechecking, and a check performed outside intent insertion could race with another writer. The tool executor's durable intent boundary is immediately before handler dispatch, and an immediate SQLite transaction serializes the check-and-append across connections. A reconciliation decision must also have one durable winner; appending conflicting decisions would make recovery depend on whichever row a reader happened to select last.

**Proof and limit.** `unresolved_prior_task_effect_blocks_executor_handler_until_reconciled` verifies the actual production ToolExecutor withholds a replacement worker's handler, emits a structured recovery code without the private path, and permits the handler after an explicit retry-safe record. `unresolved_task_effect_blocks_new_intents_until_reconciled`, `concurrent_task_intents_allow_at_most_one_unresolved_side_effect`, and `operation_reconciliation_has_one_durable_winner` cover the DB contract and race. The P4-53 gate remains open: no operator/Supervisor reconciliation consumer, repository snapshot comparison, complete changed-path capture, replacement-worker continuation, multi-mutation lost-ack proof, or no-duplicate end-to-end recovery has been established.

## ADR-LOG-RESTRUCT-0225 — Prefer runtime-probed PSEC for strict Windows process confinement

**Status.** Accepted and partially implemented on 2026-10-03 under the owner's prior approval to evaluate and build an alternate Windows sandbox runner. Focused PSEC behavior is proven; the full Windows toolchain/canonicalization contract is not.

**Decision.** Add a runtime-probed Process Security Environment (PSEC) launch path using Microsoft's `CreateProcessSecurityEnvironment` plus the `PROC_THREAD_ATTRIBUTE_SECURITY_ENVIRONMENT` attribute on the existing `CreateProcessW` flow. Keep Arcana's suspended launch, explicit inherited-handle list, stdout/stderr pipes, and kill-on-close Job Object. Express only the selected workspace as read/write, exact staged runtime/toolchain roots as read-only, explicitly deny known external user-data roots, and set network egress default to deny. Select this backend only if the host exposes the required API and reports the required schema/security support; otherwise fail closed. Do not widen AppContainer ACLs or silently use an unrestricted child.

**Why.** Production Windows tests confirmed AppContainer denies canonicalizing an allowed Clippy config path despite exact read/traversal grants. Microsoft's PSEC process security environment attaches to ordinary `CreateProcessW`, preserving the process/pipe lifecycle that `Experimental_CreateProcessInSandbox` cannot currently provide because that API requires `inheritHandles=FALSE`. The current host exposes PSEC create/close/support-query exports. PSEC must still prove sibling/user-data read denial, selected workspace access, network denial, output capture, timeout/cancellation descendant cleanup, and supported runtime/toolchain behavior before production selection.

**Source basis.** Microsoft's `mxc` repository provides the PSEC wire schema at `external/windows-sdk/ProcessSecurityEnvironment.fbs` and uses the same processmodel exports and process attribute in `src/backends/process_container/common/src/secenv.rs`; the repository schema is MIT-licensed. The API is dynamically loaded and required support is checked at runtime. The host currently reports Windows 11 25H2 build 26200.9457 and exports the PSEC APIs, but export presence alone is not capability proof.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs`, P4-11/P4-61/P4-62 and TG4-18/TG4-31.

**2026-10-03 implementation evidence.** PSEC now backs production Windows child creation; old production AppContainer ACL writes were removed. Host support query reports PSEC v1.0 and filesystem-deny flags `0x3`, without v1.1 enumeration-only traversal. Windows-target check passes; focused production tests pass for staged Python Build/Test, selected MSVC Cargo Build, `.cmd` stdout/exit capture, sibling-read/loopback denial, and PSEC serialization. Clippy still fails because OS error 5 is returned when the confined process canonicalizes the selected workspace root. This proves the unresolved issue is parent traversal under the available PSEC contract; do not widen access. The owner subsequently approved guarded Windows workspace staging/copy-back; its implementation and production proof remain open. The latest full core result is 916 tests: 913 passed, 2 failed, 1 ignored.

## ADR-LOG-RESTRUCT-0226 — Name egress controls by destination

**Status.** Implemented and verified on 2026-10-03.

**Decision.** Give each workspace-egress consent button a destination-specific accessible name, so provider and MCP grants remain independently discoverable by assistive technology and reliable in UI tests when both destinations have the same current action.

**Why.** A shared label such as `Allow egress` makes two different external destinations indistinguishable to accessibility queries and keyboard/screen-reader users.

**Proof and limit.** The focused Settings test passes 2/2 and the full frontend suite passes 51/51; `npx tsc --noEmit` and `npm run build` pass. Vite still reports existing mixed dynamic/static import and large bundle-size warnings. This UI evidence does not prove live desktop IPC or persisted consent behavior.

**Where.** `src/tabs/Settings.tsx`, `src/tabs/Settings.test.tsx`, workspace egress consent.

## ADR-LOG-RESTRUCT-0227 — Revalidate the protected commit target after lock waits

**Status.** Implemented on 2026-10-03; P8-74/TG8-13 remain open for stale-claim and production-path proof.

**Decision.** `git_commit` captures the validated branch ref and target commit after acquiring the shared index lock. After all awaited file locks and immediately before creating the commit, it rechecks that HEAD still names that ref and that its target has not moved. The final commit updates only the captured ref; it never resolves a fresh HEAD ref after the host has approved a different branch.

**Why.** Independent Luna High review found that an external Git action could switch HEAD to `main` while the host waited on a worker file lock. The old flow selected the now-current ref after that wait, so an earlier `arcana/fleet` decision could authorize a commit on protected `main`. Capturing identity, refusing when the identity changes, and targeting only the captured ref prevents the reviewed bypass.

**Proof and limit.** `commit_refuses_head_switch_to_protected_branch_while_waiting_for_file_locks` injects a switch from `arcana/fleet` to `main` after file-lock acquisition and proves refusal without moving either branch; all 13 `git_commit::tests` pass. This does not make a multi-process Git lock around HEAD, prove task-claim ownership remains current across the same wait, or exercise a model-produced call through AgentEngine/RecipeRunner/scheduler. TG8-13/P8-74 remain open.

**Where.** `arcana-core/src/agent/builtin_tools/git_commit.rs`, P8-74/TG8-13.

## ADR-LOG-RESTRUCT-0228 — Match the expanded tool-failure envelope at the dispatch boundary

**Status.** Implemented on 2026-10-03; full builtin/provider matrix and privacy audit remain open.

**Decision.** Shared `ToolResult::with_execution_context` now emits the expanded goal contract at the JSON envelope level: `error_code`, `category`, `message`, operation/tool identity, retryability and `retry_after_ms`, cause/action, context, run/agent/task correlation, and timestamp. It retains the established nested `error` object and `code` alias so existing consumers remain compatible.

**Why.** The expanded P4 goal names these fields as the common model/UI/observability contract. Before this change, the executor emitted a nested `error.code` and nested correlation, so a consumer following the documented top-level shape could not read the required fields consistently.

**Proof and limit.** The real shared executor privacy test asserts both top-level contract fields and nested compatibility fields, including explicit null retry delay and correlation IDs; `agent::tool_trait::tool_result_error_tests` passes 8/8. This does not establish every builtin/provider error category, retry semantics, or complete privacy coverage; P4-51/TG4-25 remain open.

**Where.** `arcana-core/src/agent/tool_trait.rs`, P4-51/TG4-25, expanded P4 goal §3.1.

## ADR-LOG-RESTRUCT-0229 — Use guarded Windows workspace staging for PSEC traversal failures

**Status.** Owner-approved on 2026-10-03; implementation and production proof remain open.

**Decision.** When the confined Windows child cannot traverse/canonicalize the selected workspace directly, execute against a guarded stage and copy approved changes back beneath the originally selected workspace only if the stage's own location is proven usable by the selected sandbox backend. Preserve denied external-read/network boundaries; do not solve traversal errors with broad read grants or ACL mutation. Reuse validated path/symlink/workspace-identity protections, reject conflicting host edits, and fail closed when staging or safe copy-back cannot be proven. A plain move to the ordinary OS temporary directory is not sufficient: the 2026-10-03 production Clippy probe failed with the same OS error 5 there.

**Why.** PSEC v1.0 on the current host denies canonicalization of an allowed workspace root/config with OS error 5, preventing supported toolchain operations such as Clippy. The failure is not resolved by reading exact config bytes or by relocating the workspace to an ordinary OS temporary directory. Guarded staging remains the approved direction, but its location/backend must first be shown to support required traversal while retaining untrusted-code isolation.

**Proof and limit.** This records the owner's selected direction, not implemented behavior. The Windows copy/copy-back path must be tested through Rust, Node, and Python launchers, including a sandbox-approved stage location, workspace mutation persistence, unrelated-path and symlink denial, deletion/rename handling, command failure and timeout, descendant cleanup, conflict refusal, and refusal on workspace identity changes. The ordinary-temp experiment reproduced Clippy's denial; the checked-in production Clippy test remains intentionally red acceptance evidence until a usable stage/backend is implemented. P4-11/P4-61/TG4-18/TG4-31 remain open; latest full core result is 913 passed, 2 failed, 1 ignored, with both failures in Clippy canonicalization.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs`, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0230 — Reserve run-token budget before provider dispatch

**Status.** Owner-approved direction on 2026-10-03; reservation design and integration remain open.

**Decision.** Enforce the Supervisor's run-token cap through conservative pre-dispatch reservations. Do not rely only on usage reconciliation after a provider response. Apply the cap across every physical provider attempt, including retries, compaction calls, and child-agent work, and reconcile actual usage against the reservation when returned.

**Why.** Isolated CLI runs exceeded a 100,000-token run cap by reporting 106,667 tokens because accounting occurred after responses. Another provider request before bounded pre-dispatch enforcement could exceed the user's configured cap.

**Proof and limit.** This records the selected policy, not working enforcement. The implementation must define and test a safe request bound, atomic concurrent reservations, failure/unknown-usage settlement, persisted-budget interaction after task claim, retry/compaction coverage, and shared parent/child accounting. No live provider call is authorized by this ADR; defer it until the bound is enforced and focused production-path tests pass.

**Where.** `arcana-core/src/agent/engine.rs`, provider dispatch/retry/compaction paths, P4-15/P4-50, TG4 budget gates.

## ADR-LOG-RESTRUCT-0231 — Time the production Fleet work-availability gate

**Status.** Implemented and focused-tested on 2026-10-03; P4-43 remains open.

**Decision.** Emit fixed-label operation timings at both shared scheduler boundaries: `worker_has_recoverable_work` records the recoverable-work query used by startup, cron, and event-kick callers; `run_if_recoverable_work_with_timeout` records idle, already-running, and completed dispatch decisions plus total run duration. Record the `tasks` database identity for the query, worker slug, and bounded result counts only. Leave read/open and dispatched-run failures at the timer's explicit error default. Do not include task titles, criteria, request payloads, or message content.

**Why.** Empty-work checks are the deterministic gate that prevents unnecessary provider calls. Timing the query and shared dispatch helper makes idle cost, skipped duplicate work, actual dispatched runs, and failures visible without adding model calls or duplicating measurements at each scheduler caller.

**Proof and limit.** `production_work_gate_emits_safe_idle_and_work_timing` proves query outcomes, duration, DB/worker identity, and task-content privacy (1/1). `startup_and_event_dispatch_gate_records_idle_and_completed_runs` proves the shared dispatch helper's idle and dispatched outcomes/durations and that the work closure executes once (1/1). `empty_startup_and_qc_recovery_checks_do_not_poll_the_provider` continues to prove the empty queue does not execute its provider closure. This does not count physical provider requests or measure every real cron/event lifecycle; P4-43 remains open.

**Where.** `arcana-core/src/recipes/scheduler.rs`, P4-43 and P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0232 — Preflight Gemini input tokens before run-budget reservation

**Status.** Owner-approved and implemented on 2026-10-03; local fixtures and live countTokens preflight pass. Successful CLI Fleet completion remains open.

**Decision.** For Gemini generation requests, call the official `models.countTokens` endpoint with the same serialized `GenerateContentRequest` used for generation, then atomically reserve the returned input token count plus that request's `maxOutputTokens` against one shared run ledger before each physical generation attempt. Reconcile with returned usage after completion; retain the full reservation when dispatch occurred but usage is unavailable. Apply this to streaming, unary completion, retries, compaction, and child-agent calls. Fail closed when preflight, model identity, reservation storage, or output bounds cannot be established. The preflight is a separate provider operation: record its duration/outcome and do not count it as a generation attempt or token usage.

**Why.** CLI runs observed 106,667 tokens against a 100,000 run cap because usage was only reconciled after responses. Reserving the full 1,048,576-token Gemini context would reject every call under Arcana's current 1,000,000-token CLI ceiling. Google's current official API contract accepts `generateContentRequest` at `models.countTokens` and returns input `totalTokens`; `maxOutputTokens` bounds generation output. Source: [Gemini token-counting API](https://ai.google.dev/api/tokens) and [GenerateContent API](https://ai.google.dev/api/generate-content).

**Proof and limit.** Local HTTP fixtures prove request-body parity including the required nested model resource, malformed/error denial, safe normalized error summaries, and streaming rejection beyond 16 KiB. A live synthetic request through the current Google key returns 200 with the production-shaped model/system/tools/generation configuration. The production CLI initially exposed a missing `generateContentRequest.model`: the preflight returned 400 even though `countTokens` accepted a simple `contents` request. The production request now sets `model` to `models/{id}` and the CLI proceeds through preflight and generation. Run-token tests prove atomic reservations, no generation dispatch on preflight denial, delayed usage settlement after an empty finish frame, and full reservation retention when usage is unknown. An AgentEngine claim test seeds 2,000 persisted tokens under a 10,000-token task cap and proves that the next 8,192-token output reservation is denied and escalated to `needs_info`. Provider decorators are wired across unary/stream, retries, compaction, and child engines. The isolated CLI Fleet run then stopped safely at its 100,000-token cap before decomposition; no successful coding run is proven.

**Where.** `arcana-core/src/provider/google.rs`, `provider/traits.rs`, run-scoped provider/budget wrapper, `agent/engine.rs`, `agent/compaction.rs`, `subagent/native_runner.rs`, P4-29/P4-50 and the expanded run-token cap contract.

## ADR-LOG-RESTRUCT-0233 — Require a single-use Supervisor approval after build diagnosis

**Status.** Owner-approved and implemented with focused DB, production-tool, scripted Supervisor-recipe, concurrent/reopen, and Windows mixed-project proof on 2026-10-03; supported-platform variants and successful CLI Fleet execution remain open.

**Decision.** After three consecutive observed nonzero Rust-owned build exits, stop automatic build execution and present the worker's safe diagnosis to the Supervisor. A Supervisor may authorize exactly one subsequent `project_command` build batch for that task. Persist the decision with task, assigned worker, Supervisor identity, reason, issue time, and consumed state; consume it atomically before the batch starts. Bind the grant to the exact latest failed/incomplete diagnostic batch reviewed, then recheck that batch identity at approval and consumption. The SQLite insert trigger requires a connection-scoped host authorization function in addition to the Supervisor identity, so direct SQL cannot forge approval by writing the string `supervisor`. Do not infer approval from free-form messages or request-resolution notes. Missing, stale, mismatched, reused, or unreadable approval state remains denied. Passing observed build evidence resets the failure streak; spawn failures and timeouts remain separate.

**Why.** The existing guard correctly blocks a fourth build but has no path for the owner's selected Supervisor decision. One-use durable authorization makes that decision explicit and auditable while preventing workers, repeated requests, or multi-process races from bypassing the cap.

**Proof and limit.** The database test proves observed failure thresholds, forged-SQL and worker self-approval rejection, Supervisor reason/actor audit, diagnostic-batch binding, atomic consumption, and reuse denial. Production ProjectCommandTool tests run three failed Cargo builds, process the typed worker request and Supervisor approval, allow one additional batch, and reject reuse. `shipped_supervisor_recipe_approves_one_diagnosed_build_retry` drives the persisted Supervisor recipe through RecipeRunner/AgentEngine with a scripted provider, then proves exactly one of two concurrent consumers succeeds after reopening TasksDb. `production_mixed_batch_stops_after_third_observed_failure` proves on Windows that the cutoff suppresses the later Python child, then an approved batch runs Cargo and Python, both pass, and the production QC transition succeeds. Cross-platform variants and successful CLI Fleet execution remain open; P4-15/TG4-08 are not closed.

**Where.** `arcana-core/src/db/tasks.rs`, `project_command.rs`, worker-request/Supervisor tools and recipe, P4-15/TG4-08.

## ADR-LOG-RESTRUCT-0234 — Supervisor-authored operation reconciliation

**Status.** Owner-goal implementation in progress, 2026-10-03.

**Decision.** A task-attributed non-Board operation whose durable outcome is unknown stays quarantined until a Supervisor records one append-only reconciliation decision in `runs.db`. The decision names the exact `run_id` and intent cursor, uses one of `reconciled`, `retry_safe`, or `do_not_retry`, records a bounded reason code and the Supervisor actor, and is single-winner. `retry_safe` and `reconciled` permit a later claim; `do_not_retry` remains quarantined. SQLite must reject decision rows without the host authorization function, correct Supervisor identity, valid reason code, an unknown non-Board intent, and no previous reconciliation. Workers cannot reconcile their own effects. The shipped Supervisor flow must inspect the health/journal record and task/workspace/Git evidence before using the tool; this action records the decision but does not claim that Arcana independently verified model-provided reasoning.

**Why.** The existing executor and claim boundary safely hold uncertain work, but there is no production Supervisor tool to release that hold. A durable, exact-intent decision makes the existing recovery contract actionable while preserving the fail-closed default and preventing concurrent or forged approvals.

**Where.** `arcana-core/src/db/runs.rs`, new Supervisor builtin reconciliation tool, shared tool registry, shipped Supervisor recipe, TG4-27/TG4-28.

**2026-10-03 proof update.** `shipped_supervisor_recipe_keeps_uncertain_operation_quarantined` drives the real shipped Supervisor recipe through RecipeRunner/AgentEngine, reads the task board, request queue, inbox, active journal entry and exact held task, then invokes the registered tool. After reopening the stores, it proves `do_not_retry` and its Supervisor/reason audit fields persist while the task remains quarantined and the retry barrier stays active. `reconcile_worker_operation` tests pass 2/2, including an injected SQLite append failure followed by direct-SQL forgery rejection, proving the host authorization guard is reset on failure. The connection-scoped authorization now resets by RAII across ordinary error and unwinding. This proves the conservative hold decision only; verified-complete/safe-retry reconciliation, workspace/Git/checkpoint comparison, complete mutation capture, and replacement-worker continuation/no-duplicate proof remain open.

## ADR-LOG-RESTRUCT-0235 — Preserve guarded workspace changes after stopped project commands

**Status.** Owner-approved on 2026-10-03; Windows staged copy-back implementation and proof remain open.

**Decision.** When P4-61 runs a project command against a guarded workspace stage, copy back conflict-free, validated changes after any command outcome, including nonzero exit, timeout, or cancellation, but only after the full child process tree is confirmed stopped. Preserve the observed process exit or incomplete/timeout classification independently; a copied change never turns failed or incomplete build evidence into a pass. Reject copy-back on changed workspace identity, symlink/reparse escape, or conflicting host edits.

**Why.** A failed build can still produce intentional workspace changes or useful partial progress. Discarding all stage changes on failure would lose work, while copying before descendants stop or overwriting concurrent host edits would create unsafe or surprising mutations. Keeping copy-back and build evidence as separate outcomes preserves both user work and QC correctness.

**Proof and limit.** This is the selected policy, not implemented behavior. Tests must prove copy-back after success, nonzero exit, timeout, and cancellation; process-tree termination precedes copy-back; conflicts and path/identity violations refuse to modify the original workspace; and incomplete commands never authorize QC. P4-11/P4-61/TG4-18/TG4-31 remain open.

**Where.** `arcana-core/src/agent/builtin_tools/project_command.rs`, `platform/process_sandbox`, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0236 — Windows sandbox bridge without inherited handles

**Status.** Owner-approved on 2026-10-04; Windows bridge implementation has focused production-path proof, while full P4-61/TG4-31 proof remains open.

**Decision.** The Windows sandbox runner may add an Arcana-owned bridge execution mode so the experimental OS sandbox API is called with handle inheritance disabled, while the bridge captures bounded stdout/stderr for the validated project command. The bridge must remain inside the same OS-enforced sandbox and process-tree job boundary; it may launch only the already validated executable, argv, cwd, and scrubbed environment supplied by the host. It must not grant network access or broader filesystem paths to make output capture work. Output transport, bridge executable resolution, and packaged CLI/Tauri startup must be tested against missing binary/runtime, launch failure, oversized output, timeout/cancellation, and descendant termination; any incomplete setup fails closed. This approval does not claim the experimental runner is production-ready or authorize any reduced read/network policy.

**Why.** Microsoft documents `Experimental_CreateProcessInSandbox` as experimental and requires `inheritHandles = FALSE`; the current Windows launcher uses inherited standard handles for output pipes. A bridge provides a place to create child pipes without asking the experimental API to inherit host handles, while preserving Arcana's bounded output and process-tree cancellation contracts.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs`, Windows sandbox runtime, CLI and Tauri entrypoints, TG4-31/P4-61. Microsoft API contract: https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox.

**2026-10-04 implementation evidence.** `Experimental_CreateProcessInSandbox` now starts the real `arcanactl.exe` bridge with `inheritHandles = FALSE`; the bridge validates its bounded request, launches the fixed argv with an explicit child-handle list, keeps the command tree in a kill-on-close job, captures 256 KiB tails per stream, and returns bounded result metadata through workspace-local temporary files. The production dispatcher uses this runner on Windows and both CLI/Tauri entrypoints check the private bridge mode before normal startup. The runner checks for Windows 11 build 22000+ and the required processmodel export. On this host, the SBOX AppContainer bridge passes real workspace-write, sibling-read denial, sibling-write denial, loopback denial, and stdout/stderr capture. All 13 focused Windows process-sandbox tests pass, including a direct Clippy-config file-read probe (not production Cargo Clippy), staged CPython, `.cmd`, timeout and descendant cleanup cases. `cargo check -p arcana-core`, `cargo check -p arcanactl`, `cargo check -p arcana --bin arcana`, and `cargo build -p arcanactl` passed during integration. The bridge currently uses the selected original workspace directly; the separately owner-approved guarded stage/copy-back behavior is still missing. Linux Landlock production proof, signed macOS XPC/staging, packaged Tauri runtime smoke, OS-build coverage, dependency-stage proof, and user-owned Python fallback execution remain open. TG4-31/P4-61 remain unchecked.

**2026-10-04 validation correction.** ~~The 13 focused tests prove Clippy config access on the production command path.~~ They prove only a direct config-file read probe; production Cargo Clippy still fails canonicalizing the selected workspace/config path (OS error 5). `cargo fmt --all -- --check`, `cargo check -p arcana-core --locked`, and the 13-test Windows sandbox suite pass. The full core run discovered 956 tests but was interrupted after the staged-Python isolation test remained CPU-bound for more than ten minutes, so no full-suite pass is claimed. A per-launch ancestor-ACL experiment did not fix the path failure and was removed after a 185-second attempt. No P4 task or gate closes from this evidence.

**Host mitigation note.** Setting `disallow_win32k_system_calls` in the SBOX specification makes the Arcana bridge fail during process initialization on this Windows build (`0xC0000142`); the passing runner specification leaves that optional mitigation off. AppContainer path restrictions, explicit default-deny network policy, no inherited host handles, and job-scoped descendant termination remain enabled. This changes only the optional Win32k mitigation and is recorded as a host compatibility limitation, not as a filesystem/network policy relaxation.

## ADR-LOG-RESTRUCT-0237 — Do not stage Windows projects until confined path resolution works

**Status.** Evidence recorded 2026-10-04; guarded stage/copy-back remains approved but is not implementable on the current SBOX path contract without another proven resolution mechanism.

**Finding.** A disposable production SBOX child was given one exact read-only stage root. It could read a staged `.clippy.toml` and obtain directory/file metadata, but Rust `Path::canonicalize()` returned Windows error 5. `GetFinalPathNameByHandleW` with DOS-volume mode returned error 5; NT-volume mode returned `\Device\HarddiskVolume3\...`, which Rust metadata/canonicalization could not reopen (error 3). Prefixing the NT path with `\\?\GLOBALROOT` allowed metadata but canonicalization still returned error 5. The same `GetFinalPathNameByHandleW` failure occurred for an ordinary temporary workspace and reproduces in the real Cargo Clippy production test.

**Decision.** Do not treat workspace relocation, a read-only stage grant, an NT device path, or `GLOBALROOT` as a safe fix. Keep P4-11/P4-61 and TG4-18/TG4-31 open and fail closed for affected Windows commands. Continue evaluating a supported path-resolution/sandbox backend that preserves external-read and network denial before implementing stage/copy-back. Do not weaken the policy with broad filesystem grants or assume copy-back is safe until the child can execute against and resolve the stage.

**Proof and limit.** The direct config read and metadata succeeded in the focused disposable child; DOS final-path resolution and Rust canonicalization did not. `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox` remains a reproduced failing acceptance test. This is a current-host finding, not a claim that every Windows build behaves identically. The exploratory test harness was removed after recording its output; no production code or sandbox policy was relaxed.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs`, `project_command.rs`, ADR-LOG-RESTRUCT-0229/0235, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0238 — Trace Python runtime staging phases

**Status.** Implemented on 2026-10-04; the timing helper and core check pass. The cold Windows isolation test did not return within ten minutes and was stopped; phase-duration evidence from that test is unavailable.

**Decision.** Keep the existing runtime staging and trust/cache behavior, but emit bounded `OperationTimer` events for interpreter validation and inspection, source fingerprinting, interpreter/DLL/stdlib/pytest copying, staged fingerprinting, cached-runtime verification, and runtime probes. Fixed operation names carry duration and success/error only; paths, file contents, and exception strings remain excluded. This decomposes the existing total runtime-preparation duration so slow cold launches can be localized before optimization.

**Why.** A full `arcana-core` test run stalled in the staged-Python isolation case on this Windows host. The staging function previously emitted only one aggregate preparation duration, so it did not distinguish source scans, copies, integrity hashing, cache checks, or the probe.

**Proof and limit.** `python_runtime_phase_timing_records_success_and_failure` verifies the timing helper emits duration and result state without error text; the production isolation test now asserts the expected phase labels. `cargo check -p arcana-core --locked` passes. The isolation test itself remained active for ten minutes, with its isolated runtime tree copied, then was interrupted while no Python child was present; therefore its production phase assertions are unverified, the Windows cold-stage latency is unresolved, and no optimization or gate closure is claimed.

**Where.** `arcana-core/src/platform/python_runtime.rs`, P4-11/P4-62 and TG4-18/TG4-32.

## ADR-LOG-RESTRUCT-0239 — Keep Windows sandbox setup off async executor threads

**Status.** Implemented on 2026-10-04; Windows launcher, cancellation, CLI, Tauri, and production command-timeout checks pass.

**Decision.** Run the synchronous Windows SBOX workspace/profile/process setup on Tokio's blocking pool instead of the async executor. Limit concurrent setup jobs to four with a process-wide semaphore; cancellation while waiting for a permit exits immediately, while cancellation during an OS call detaches only that bounded blocking job. Keep its permit until the OS call returns. If it returns a child after the async caller has timed out, dropping the unobserved result invokes the child's kill-on-close process-tree cleanup. Linux's async child-spawn behavior and macOS's fail-closed path remain unchanged.

**Why.** A cold staged-Python isolation test remained inside host setup for ten minutes before it was stopped. The previous synchronous `process_sandbox::spawn` ran before the command's first `await`, so the worker timeout could not be polled during a slow SBOX call. Offloading setup lets the command deadline and other runtime work progress while bounding concurrent stuck setup calls.

**Proof and limit.** `cancelled_windows_launch_returns_and_drops_a_late_child` proves timeout returns promptly and a value produced after cancellation is dropped. `production_command_timeout_stops_nested_process_writes_and_records_stage_time` passes 1/1 after the async change; the Windows launcher suite passes 13/13. `cargo check -p arcanactl --locked` and `cargo check -p arcana --bin arcana --locked` pass. A blocking OS call itself cannot be forcibly interrupted; it retains one of four slots until it returns. The staged-Python cold isolation test was interrupted at ten minutes before this change and has not yet been rerun, so that stall and phase timing assertions remain open.

**Where.** `arcana-core/src/platform/process_sandbox/mod.rs`, its Bash/ProjectCommand/Python callers, P4-11/P4-62, TG4-18/TG4-32.

## ADR-LOG-RESTRUCT-0240 — Correct the stale daily-spend observability gap report

**Status.** Correction recorded on 2026-10-04 after checking the live source and tests.

**Retracted claim.** ~~The daily-spend reserve and settle transactions have no operation-level observability.~~ This finding came from a stale read-only audit and is false for the current working tree.

**Observed behavior.** `reserve_provider_call_at_estimated` in `arcana-core/src/budget/daily.rs` emits `daily_budget_provider_call_reserve` with `database=config` and `disabled`, `denied`, or `granted` outcomes; early errors default to `error`. `settle_provider_call_at` emits `daily_budget_provider_call_settle` with `settled`, `settled_unknown_usage`, or `not_found`. The existing `provider_budget_reservation_emits_decisions_without_private_values` test verifies grant, denial, disabled cap, injected DB failure, known settlement, duplicate settlement, durations, and identifier/error privacy. No production instrumentation is added for this correction.

**Follow-up verification.** The reservation telemetry test now includes unknown-usage settlement and verifies the `settled_unknown_usage` outcome while confirming private reservation markers remain absent; the focused test passes. No production instrumentation was added for this correction. `orphan_candidate_scan` likewise already has result-aware operation timing and empty/error tests in live `orphan`/`TasksDb` sources, contradicting the stale audit's second finding.

**Where.** `arcana-core/src/budget/daily.rs`, `arcana-core/src/orphan/mod.rs`, `arcana-core/src/db/tasks.rs`, P2-21/TG2-17 and P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0241 — Probe AppContainer-private staging for canonical workspace paths

**Status.** Rejected as a production staging root on 2026-10-04; no production path or readiness change is authorized by this probe.

**Decision.** Do not use the documented AppContainer Local AppData folder as the P4 staging root. A direct SBOX child reproduced the Rust `Path::canonicalize()` failure on its working directory, so this location does not resolve the production Clippy blocker. Retain fail-closed behavior for incompatible Windows toolchains and evaluate a different supported sandbox/path-resolution mechanism before implementing stage manifests or copy-back.

**Why.** The production Clippy probe can read and stat an explicitly granted workspace config, but Rust path canonicalization still fails with OS error 5. Microsoft's AppContainer documentation identifies a per-container Local AppData folder accessible to that AppContainer and exposes `GetAppContainerFolderPath`; this gives a more precise candidate than another ordinary temporary directory. This remains a hypothesis until the SBOX child test proves canonicalization and sibling/network denial at the same boundary.

**Security bounds.** Any future staging backend must stage only the selected project tree, pass only that staged root as writable, preserve default-deny network and the exact runtime allowlist, and never copy through symlinks/reparse points. Production copy-back remains gated on process-tree-stop confirmation, original workspace identity checks, per-path baseline conflict checks, and all-or-nothing preparation of the validated change set. This ADR does not authorize a broader ACL grant or a relaxed policy.

**Sources and proof.** Microsoft documents `GetAppContainerFolderPath` as returning the local app-data directory for the supplied AppContainer SID and documents that an AppContainer can access files created specifically for it: https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getappcontainerfolderpath and https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation. These documents support the probe location only; they do not prove `Experimental_CreateProcessInSandbox` path canonicalization, staging performance, or safe copy-back. P4-11/P4-61/TG4-18/TG4-31 remain open.

**Probe update — 2026-10-04.** The test harness first placed its probe executable inside the writable workspace, which correctly triggered the SBOX rule against overlapping writable and read-only roots; it was moved into its own unique read-only runtime directory. An initial bridge run used a stale CLI executable and is not evidence. After rebuilding the bridge, its child could not read the host-created request file (`stage=request_read os_code=3`). A direct SBOX launch then bypassed bridge transport and ran a tiny Rust probe from the AppContainer-private Local AppData directory; it exited `52`, which the probe maps specifically to `current_dir().canonicalize()` failure. This reproduces the Clippy path-resolution gap independently of bridge I/O. An additional test-only attempt to add non-inheriting traversal/read-attributes ACEs to every ancestor did not complete promptly while changing protected parent ACLs. The test was stopped; its two exact SID grants were removed and verified absent, and both disposable AppContainer profiles were deleted. The ACL-mutation probe was removed from source. No production staging or copy-back behavior has been added; Windows SBOX/Clippy and TG4-31 remain open.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs`, `windows_sbox.rs`, ADR-LOG-RESTRUCT-0229/0235/0237, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0242 — Correct the staged-Python isolation stall diagnosis

**Status.** Test fix verified 2026-10-04; no sandbox policy change.

**Correction.** The earlier claim that the production SBOX launcher stalled before a Python child appeared was wrong. The isolation test had called `read_to_end()` on stdout and stderr before invoking `WindowsSandboxChild::wait()`. This bridge only closes its output streams while `wait()` finalizes the bridge result, so the test deadlocked even though the child had completed.

**Evidence.** The interrupted run's bounded bridge result JSON recorded exit code 0 and the expected isolation output. The test now drains stdout, stderr and `child.wait()` concurrently with `tokio::join!`. The exact staged-runtime sibling-read/runtime-write/staged-tool/loopback/pytest isolation test passes 1/1 in 38.46s. The complete focused Windows sandbox test group passes 16/16 in 96.37s. Two additional launch probes pass from an OS-temp workspace and with an explicitly granted external read root.

**Disposition.** Strike the earlier “no child appeared”/“sandbox launch stalled” claims in the P4 checkpoint, MDP, gates and readiness notes. Keep the full core suite open until rerun. This correction does not resolve the separate production Cargo Clippy path-canonicalization failure (Windows OS error 5), guarded stage/copy-back, locked dependency staging, packaged Tauri smoke, Linux Landlock, or macOS XPC/staging requirements.

**Where.** `arcana-core/src/platform/python_runtime.rs`, `arcana-core/src/platform/process_sandbox/windows.rs`, MDP P4-11/P4-61/P4-62, TG4-18/TG4-31/TG4-32.

## ADR-LOG-RESTRUCT-0243 — Record full core rerun and remove ineffective Clippy config stage

**Status.** Verified 2026-10-04; Windows Clippy gate remains open.

**Decision.** Keep the production sandbox fail-closed. Do not stage Clippy configuration in a separate read-only directory as a workaround; retain the direct failure as evidence until a supported path-resolution solution passes the production command.

**Evidence.** The full `arcana-core` library run completed 962 tests: 959 passed, 2 failed, 1 ignored in 645.51s. Both failures are production Clippy paths (the explicit Clippy test and the project-command ecosystem corpus) returning OS error 5 during canonicalization. A temporary config stage under Arcana's own profile was granted as an explicit read-only SBOX root; a child could read and stat the file but could not canonicalize either it or the directory. That experiment was removed. The other two failures from the first full run were test/metric date defects: the journal fixture attempted to reconcile an already-successful operation (reconciliation is for unknown effects), and the dashboard compared UTC timestamps against the local Bangkok date. The test now uses an unknown outcome; the dashboard compares the worker date against UTC. Both focused tests pass.

**Disposition.** P4 remains 32/59 and TG4 remains 10/33. No task, gate, or readiness score changes. P4-11/P4-61/TG4-18/TG4-31 remain open; full core validation is red until a secure path-resolution backend works.

**Where.** `arcana-core/src/agent/builtin_tools/list_worker_health.rs`, `arcana-core/src/analytics/queries.rs`, `arcana-core/src/agent/builtin_tools/project_command.rs`, P4-11/P4-61, TG4-18/TG4-31.

**Vendor contract note.** Microsoft describes the Windows SBOX API as experimental, and its documentation says explicit `fs_read_write`/`fs_read_only` directory grants apply recursively under AppContainer. On this Windows 11 build 26200, the production child can read and stat the explicitly granted file but Rust `Path::canonicalize()` still returns OS error 5. This is an observed host/API limitation, not a documented guarantee that canonicalization must succeed. Keep the real production regression and fail closed until an implementation passes it without weakening the filesystem or network boundary. [Microsoft SBOX API](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox).

## ADR-LOG-RESTRUCT-0244 — Reject read-only drive-root grant for SBOX path resolution

**Status.** Rejected on 2026-10-04; production sandbox policy restored to exact workspace/runtime roots.

**Retracted hypothesis.** ~~Microsoft's SBOX contract describes a drive-root grant as applying to the root itself rather than recursively to its contents. A read-only `C:\` grant might therefore allow volume-anchor path resolution without allowing sibling-file reads.~~ I incorrectly applied the documented `fs_read_write` drive-root exception to the separate `fs_read_only` field.

**Evidence.** The schema test confirmed the canonical drive-root path was serialized, but the real production `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox` still failed: Cargo Clippy returned Windows OS error 5 finding its configuration file. The granted root did not resolve the production failure. The focused test fails with the same error observed before the experiment.

**Decision.** Remove the unneeded drive-root grant and helper. Do not retain extra volume-root capability without a demonstrated need and production negative tests for outside reads, writes and network. Keep the Clippy regression and sandbox fail-closed; P4-11/P4-61 and TG4-18/TG4-31 remain open.

**Where.** `arcana-core/src/platform/process_sandbox/windows_sbox.rs`, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0245 — Correct the SBOX drive-root permission interpretation

**Status.** Correction recorded on 2026-10-04 after checking Microsoft's current API documentation.

**Retracted claim.** ~~The SBOX read-only `C:\` grant used by ADR-LOG-RESTRUCT-0244 was root-only and did not broaden access to files below the volume root.~~ The drive-root exception applies to `fs_read_write`; the docs state that `fs_read_only` applies recursively below its directory.

**Observed test scope.** The experimental `fs_read_only: ["C:\\"]` policy was present for one run of `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox`. That run executed Cargo Clippy on the test's generated dependency-free probe crate. The production test still failed with OS error 5. No network capability was granted, the SBOX policy is default-deny for egress, and the command did not execute arbitrary application dependencies; however, the test did run with a broader read boundary than intended. No evidence shows that unrelated user-file contents were read or disclosed. The broad grant was removed immediately after the failed production test; the current policy again lists only the selected workspace and exact runtime roots.

**Decision and prevention.** Treat `fs_read_write` and `fs_read_only` as separate policy contracts; never transfer exceptions between fields without a field-specific source and negative production tests. Do not add a drive-root `fs_read_only` grant. Keep the exact-path boundary and Clippy path-canonicalization regression open.

**Sources.** Microsoft's [SBOX API documentation](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox) explicitly scopes the non-recursive drive-root exception to `fs_read_write`; its `fs_read_only` section says the grant applies recursively to files and subdirectories. The local production runner test is `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox`.

**Where.** `arcana-core/src/platform/process_sandbox/windows_sbox.rs`, ADR-LOG-RESTRUCT-0244, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0246 — Exact ancestor traversal grants do not resolve SBOX canonicalization

**Status.** Rejected as a Windows path-resolution fix on 2026-10-04; production sandbox permissions unchanged.

**Question.** Can Arcana grant only `FILE_TRAVERSE | FILE_READ_ATTRIBUTES` with inheritance disabled on accessible ancestors of a selected workspace, allowing Rust `Path::canonicalize()` while sibling reads remain denied?

**Evidence.** A disposable Rust child ran through the production `Experimental_CreateProcessInSandbox` bridge with those exact temporary ACL grants from a user-owned `C:\arcana\target\p4-acl-tests` workspace. The child reported `workspace_resolves=false` while `sibling_read_denied=true`; the focused test failed after 90.07 seconds. A separate probe to add the same non-inheriting ACE directly to `C:\` failed with Windows OS error 5 before changing its DACL, confirming that this process cannot grant access to the volume root. The ACL helper and test-only identity override were removed after the run. `cargo fmt --all -- --check`, `cargo check -p arcana-core --locked`, and `git diff --check` pass after cleanup.

**Decision.** Do not broaden filesystem grants or retain per-launch ancestor ACL mutation for this workaround. It does not resolve canonicalization on this host, adds significant launch/setup cost, and cannot satisfy the production Clippy regression. Keep the Windows SBOX boundary fail-closed and P4-11/P4-61/TG4-18/TG4-31 open. No task or gate is closed.

**Where.** `arcana-core/src/platform/process_sandbox/windows.rs`, P4-11/P4-61, TG4-18/TG4-31, ADR-LOG-RESTRUCT-0245.

## ADR-LOG-RESTRUCT-0247 — Correct stale run-token pre-dispatch gap

**Status.** Correction recorded on 2026-10-04 after tracing the production provider path and running its focused tests.

**Retracted claim.** ~~Run-token usage is still accounted only after the response, so a pre-dispatch run-token bound is an unresolved gap.~~ This claim remained in GOAL.md and the MDP after `RunTokenBudgetProvider` was wired into AgentEngine.

**Observed production path.** `AgentEngine::run` wraps the consented provider in `RunTokenBudgetProvider`. With a finite run limit, the wrapper asks the underlying provider for an input-token upper bound, adds `CompletionRequest.max_tokens`, and atomically reserves that amount in the shared `RunTokenLedger` before calling `complete` or `stream`. Gemini's upper-bound implementation sends the actual built request, including tools and system instruction, to `countTokens`. Failed preflight and denied reservation do not dispatch generation; known usage settles the reservation, while unknown or dropped streams retain the conservative amount.

**Evidence.** `cargo test -p arcana-core --lib provider::run_budget::tests:: --locked -- --test-threads=1` passes 4/4, including dispatch denial and preflight failure; prior recorded isolated CLI evidence shows a later request denied before provider generation at projected 100,266 tokens. `arcanactl` tests pass 15/15 in this continuation.

**Remaining scope.** The historical 106,667-token response against a 100,000 cap occurred on the earlier pre-reservation implementation and is not erased. This correction does not prove successful CLI Fleet completion, provider quota handling, or durable reconciliation of an in-flight unknown generation across process restart; daily USD reservation coverage under TG2-17 also remains open. Do not start another paid Fleet run without a newly bounded decision.

**Where.** `arcana-core/src/provider/run_budget.rs`, `arcana-core/src/budget/run_tokens.rs`, `arcana-core/src/agent/engine.rs`, `docs/restructurization/development/GOAL.md`, P2-21/TG2-17, P4-50/TG4-24.

## ADR-LOG-RESTRUCT-0248 — Cancel in-flight concurrent read tools with their session

**Status.** Implemented and focused-tested on 2026-10-04; broad cancellation gate remains open.

**Decision.** Every concurrently executed `ToolEffect::Read` races its handler future against the existing session cancellation signal. If cancellation wins, drop that read future, return the normal correlated structured `operation_cancelled` failure, and continue the executor's regular end-event and active-tool cleanup path. The write/side-effect partition retains its existing unknown-outcome journal contract.

**Reason.** The parallel read partition previously awaited every spawned handler without observing session cancellation. A stuck read could therefore keep the AgentEngine turn and worker recovery open after the user cancelled the session. Reusing the existing session signal keeps one cancellation source and makes the read partition consistent with the serial operation wait.

**Evidence.** `cancelling_session_drops_inflight_concurrent_read_and_returns_correlated_failure` blocks a real read-tool future, cancels the registered session, and verifies bounded batch completion, future drop, structured category/code and call correlation, and active-registry cleanup. `cargo test -p arcana-core --lib p8_execution_registry_tests --locked -- --test-threads=1` passes 17/17.

**Limit.** This proves one blocked read in the production executor, not every builtin/provider cancellation class, remote cancellation acknowledgement, or the full TG4-25/TG4-27 recovery matrix. No P4 task or gate closes from this evidence alone.

**Where.** `arcana-core/src/agent/tool_executor.rs`, P4-51/P4-53, TG4-25/TG4-27.

## ADR-LOG-RESTRUCT-0249 — Redact readable log records across writer calls

**Status.** Implemented and focused-tested on 2026-10-04; complete telemetry privacy gate remains open.

**Decision.** The readable log sink retains each record until its newline or writer finalization, then applies free-text redaction to the whole record. A record above 256 KiB is omitted and replaced with a fixed marker. `flush()` does not emit an incomplete fragment, so a credential split around flush remains available to the next write for redaction.

**Reason.** Redacting each `Write::write` buffer independently allowed a recognized credential shape to cross the buffer boundary and evade every pattern. Unbounded whole-record buffering would create an avoidable memory risk, so oversized records fail closed.

**Evidence.** `cargo test -p arcana-core --lib observability::logging::tests::readable_writer --locked -- --test-threads=1` passes 3/3, covering ordinary credentials, split writes with an intervening flush, and oversized-record omission.

**Limit.** This proves the sink wrapper cases; complete planted-input coverage across all readable/structured producers and live Logs readback remain open. No P4 task or gate closes from this change alone.

**Where.** `arcana-core/src/observability/logging.rs`, P4-50, TG4-24/TG4-30.

## ADR-LOG-RESTRUCT-0250 — Preserve the active log during count pruning

**Status.** Implemented and focused-tested on 2026-10-04; CLI retention startup remains open.

**Decision.** Within each readable/structured stream, retention sorts the current active filename ahead of date order before applying the per-stream file count. Age pruning still applies to files older than the configured seven-day maximum.

**Reason.** Valid future-dated filenames could previously rank above today's open log and cause count pruning to unlink the appender's active file.

**Evidence.** `cargo test -p arcana-core --lib observability::log_retention::tests --locked -- --test-threads=1` passes 6/6. The new regression keeps today's file open while five future-dated recognized logs are pruned, then appends and reads back the same active path.

**Limit.** The test covers the pure production pruning function. CLI startup still does not invoke the shared retention policy; no P4 gate closes here.

**Where.** `arcana-core/src/observability/log_retention.rs`, P4-50/P4-59, TG4-21/TG4-24.

## ADR-LOG-RESTRUCT-0251 — Apply log retention in the headless CLI host

**Status.** Implemented and production-consumer-tested on 2026-10-04; P4-50/TG4-24 remain open.

**Decision.** Expose one shared `prune_default()` entry point and call it after logger initialization in both Tauri and `arcanactl`. It resolves the same `USERPROFILE`/`HOME/.arcana/logs` location used by Arcana's default profile.

**Reason.** The desktop startup applied the five-file/seven-day policy, but CLI-only use wrote into the same log streams without running that policy.

**Evidence.** `cargo test -p arcanactl --test consent_cli cli_startup_prunes_both_shared_log_streams_and_keeps_today_active --locked -- --exact --test-threads=1` passes 1/1. It starts the built CLI under a temporary profile containing six recognized files per stream, checks that each stream is reduced to five, and verifies today's active paths survive.

**Limit.** This proves CLI startup invokes daily-log cleanup. Required database-table retention, every telemetry producer/reader, live Fleet-to-Logs UI proof and remaining TG4-24 clauses stay open; no gate closes here.

**Where.** `arcana-core/src/observability/log_retention.rs`, `src-cli/src/main.rs`, `src-tauri/src/main.rs`, P4-50, TG4-24.

## ADR-LOG-RESTRUCT-0252 — Record result-aware SQLite setup operation spans

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Emit fixed-label, privacy-safe parent operation timing for generic tuned SQLite connection setup, full workers database migration/repair/seed initialization, and daily-cap save through transaction commit. Mark success only after the complete operation succeeds; early returns and commit failures remain `error`.

**Reason.** Statement callbacks expose SQL timings but cannot attribute final operation success or failure. These setup and persistence flows need a parent result event so the Logs/trace consumers can distinguish successful initialization and committed settings from partial work.

**Evidence.** `cargo test -p arcana-core --lib db::tests::tuned_connection_open_records_success_and_failure_without_paths --locked -- --exact --test-threads=1`, `cargo test -p arcana-core --lib db::workers::tests::worker_database_initializer_span_covers_seed_result_without_paths --locked -- --exact --test-threads=1`, and `cargo test -p arcana-core --lib budget::daily::tests::daily_cap_save_span_reports_committed_value_and_rejects_invalid_input --locked -- --exact --test-threads=1` pass. The tests cover successful and failing real isolated paths, assert result outcomes, prove the saved value is read back through `DailyCap::load`, and verify paths/config values are absent from telemetry.

**Limit.** This does not complete all database operation spans or P4-59 retention. No P4 task or gate closes from the instrumentation slice alone.

**Where.** `arcana-core/src/db/mod.rs`, `arcana-core/src/db/workers.rs`, `arcana-core/src/budget/daily.rs`, P4-50/P4-59, TG4-24.

## ADR-LOG-RESTRUCT-0253 — Include bounded task recovery state in Supervisor health

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Extend `list_worker_health` with each worker's configured model and a task-state summary read through the existing `TasksDb::get_by_id` API. The summary includes only status, retry count, assignee, epic/phase identity, dependency IDs, and budget-exceeded state; it excludes task text and criteria. A database-open/read failure is represented as `unknown` with a safe fixed error class rather than empty or failed-overall health.

**Reason.** The Supervisor's durable health response already joined worker runs but omitted the task retry and epic/phase state it needs before retry/reassignment. The fields already exist in WorkersDb/TasksDb, so the change reuses the authoritative store and adds no schema or parallel lifecycle state.

**Evidence.** `cargo test -p arcana-core --lib agent::builtin_tools::list_worker_health::tests --locked -- --test-threads=1` passes 8/8. The new focused test exercises the production TasksDb reader with an isolated epic/child, two retries, a blocked dependency, and planted private text; another test verifies store errors remain an `unknown` state with no raw SQLite details. The existing full tool test reads the configured model. Together they prove persisted fields while confirming private task prose is omitted.

**Limit.** This does not yet expose provider token aggregates, parent/child worker links, or replacement-worker continuation. No P4 task or gate closes from this slice alone.

**Where.** `arcana-core/src/agent/builtin_tools/list_worker_health.rs`, P4-54, TG4-24.

## ADR-LOG-RESTRUCT-0254 — Read provider usage by worker session in Supervisor health

**Status.** Implemented and focused-tested on 2026-10-04; P4-54 remains open.

**Decision.** Add a fixed indexed analytics query for aggregate request count, prompt/completion/thinking/total tokens, and cost by the durable worker-run session ID. Include only those totals in Supervisor health; omit the session ID and all per-request/provider prompt content. If analytics cannot be opened or queried, report `unknown` with a fixed error class.

**Reason.** P4-54 requires model/token usage in the Supervisor's recovery view. `worker_run_state` already persists the session ID, `token_usage` already indexes that key, and the shared AnalyticsDb owns the usage records; a direct exact-session aggregate therefore avoids a new table, migration, or duplicate accounting path.

**Evidence.** `worker_session_token_usage_uses_session_index_and_returns_persisted_totals` populates 502 sessions, proves the production SQL uses `idx_token_usage_session` without scanning `token_usage`, reads exact totals back through the production method, and verifies a missing session produces a known zero aggregate. `supervisor_reads_roster_and_latest_worker_run_state` inserts production token rows into an isolated home and verifies the real Supervisor tool returns only the run session totals, excluding a larger unrelated session. `session_usage_failure_is_unknown_without_database_details_or_session_id` checks fail-safe output. `cargo test -p arcana-core --lib agent::builtin_tools::list_worker_health::tests --locked -- --test-threads=1` passes 9/9; `cargo test -p arcana-core --lib worker_session_token_usage_uses_session_index_and_returns_persisted_totals --locked -- --test-threads=1` passes 1/1.

**Limit.** The P4-58 registry audit currently reports this new fingerprint plus other unregistered P4-14/P4-53 production reads and one stale entry; the token reader is listed with its populated plan, but the registry gate must be reconciled and rerun before closure. Worker parent/child linkage, failure attribution, and safe recovery continuation remain open. No P4 task or gate closes from this slice alone.

**Where.** `arcana-core/src/analytics/db.rs`, `arcana-core/src/analytics/types.rs`, `arcana-core/src/agent/builtin_tools/list_worker_health.rs`, P4-54, P4-58, TG4-20, TG4-27.

## ADR-LOG-RESTRUCT-0255 — Index task-scoped operation-journal recovery lookups

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Add an additive RunsDb migration with a partial descending `(task_id, id)` index for intent rows with a non-null task ID. The index must support unresolved-side-effect lookup and its atomic intent barrier while preserving append-only journal semantics. Do not treat a full or broad index scan as a bounded exception.

**Reason.** The P4-58 populated fixture showed task recovery filtering 181 intents through `idx_operation_journal_run_cursor` rather than searching by task; this journal has no task_id index and can grow across runs. Its latest-intent ordering is also by descending journal ID. An index shaped for the actual predicate/order avoids scanning the journal as it grows.

**Evidence.** Migration 7 adds the partial `(task_id, id DESC)` index for task-bound intents. `operation_journal_v7_upgrade_and_append_only_guards` creates a simulated v6 database with a persisted intent, opens it through the production migration ladder, verifies v7 and intent readback, then proves the lookup selects `idx_operation_journal_task_cursor`. `operation_journal_query_plans_use_populated_indexes_and_preserve_guards` passes with the new index and asserts both task-scoped recovery readers avoid a journal scan. The full query-registry suite passes 8/8 after registering every current owner/fingerprint pair. These are isolated temporary databases; no user data was modified.

**Limit.** This is a measured index choice for the two task-scoped recovery readers only. P4-58/TG4-20 query-registry evidence is restored; unrelated P4 platform, Fleet, telemetry, and recovery work remains open.

**Where.** `arcana-core/src/db/runs.rs`, `_verify/P4-58-QUERY-INVENTORY.md`, P4-53/P4-58, TG4-20/TG4-27.

## ADR-LOG-RESTRUCT-0256 — Order mixed-format timestamps by represented instant

**Status.** Implemented and focused-tested on 2026-10-04; P4-57/TG4-19 remain open.

**Decision.** Production token-usage history and session-message history sort with SQLite `julianday(timestamp)` so canonical UTC RFC3339 values and legacy offset-less SQLite values are ordered by their represented UTC instant. Add measured expression indexes matching the filtered query shapes: `(session_id, julianday(timestamp) DESC)` for analytics and `(session_id, julianday(created_at) ASC)` for messages. Preserve the original timestamp string for display and avoid rewriting stored history.

**Reason.** The readers already accept legacy timestamp rows, but lexical ordering compares the `T` separator in canonical values against the space in SQLite values before comparing time-of-day. That can reverse records from the same date. The analytics schema already has a global `julianday(timestamp)` index; the filtered reader and per-session message reader need matching composite indexes to retain an indexed plan with chronological ordering.

**Evidence.** The shared CLI analytics production-reader fixture mixes `2026-09-01 12:00:00` and `2026-09-01T09:00:00.000Z`, verifies actual-instant order, and confirms both global and filtered query plans select their expression indexes. The production analytics migration test verifies the new session index is present. The SessionStorage production readback fixture verifies canonical 09:00 precedes legacy 12:00 and `EXPLAIN QUERY PLAN` selects `idx_messages_session_julianday`. Both focused reader tests pass.

**Limit.** This closes two timestamp ordering pairs only. All other database writer/reader pairs, defaults/triggers and the complete TG4-19 inventory remain open. The schema changes are additive indexes; existing values remain unchanged.

**Where.** `src-shared/src/db/analytics.rs`, `arcana-core/src/analytics/db.rs`, `arcana-core/src/session/storage.rs`, P4-57/P4-58, TG4-19/TG4-20.

## ADR-LOG-RESTRUCT-0257 — Bound append-only database retention

**Status.** Implemented and focused-tested on 2026-10-04; P4-59/TG4-21 remain open.

**Decision.** Retain raw `token_usage`, `system_events`, `api_failures`, and `hook_logs` for 90 days. Before deleting old token rows, transactionally refresh their existing daily cost summaries so long-offline periods do not lose historical cost/token totals; keep worker-stat monthly summaries as the other historical aggregate. Do not invent monthly summaries for operational event/failure/hook detail, whose production readers expose recent bounded records. Run cleanup through the shared runtime's owned analytics maintenance job and report cleanup failures through structured logs.

**Reason.** The token usage writer already produces daily cost snapshots consumed by cost-history readers, but the daily job may be offline for longer than 90 days. Refreshing summaries from still-retained raw rows before pruning preserves that history. API-failure and system-event consumers request recent windows, while hook-log consumers read a limited recent list. Keeping raw rows indefinitely is unbounded; adding rollups without a historical aggregate consumer would create a writer-only data path.

**Evidence.** Owner-selected policy: “90-day raw; monthly summaries where useful.” Source review of `AnalyticsQueries`, `AnalyticsReader`, and `HooksDb::get_logs` confirmed the existing historical readers and recent-detail boundaries. Analytics and hook retention suites pass 3/3 each for exact cutoff, invalid-timestamp refusal, and active-writer serialization; the shared maintenance lifecycle passes 1/1 with production reader readback. The real CLI export test passes 1/1, proving expired token detail is removed while its daily cost/token aggregate remains exportable with monthly worker history and today's row.

**Limit.** This policy preserves existing `daily_costs` and `worker_stats_monthly` history; it does not preserve per-request token detail beyond 90 days. Cross-platform writer-contention coverage, full export coverage for all retained stores, and cleanup-failure log/UI readback remain open. No pruning is run against a live user database during implementation.

**Where.** `arcana-core/src/analytics/db.rs`, `arcana-core/src/analytics/aggregator.rs`, `arcana-core/src/hooks/db.rs`, P4-59, TG4-21.

## ADR-LOG-RESTRUCT-0258 — Keep Google provider errors bounded and privacy-safe

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Google generation, streaming, and countTokens failures classify from the bounded response body but return only the HTTP status, existing typed `ErrorClass`, and a validated safe summary. Error bodies above 16 KiB or with read failure produce `provider_error_details_unavailable` and fail closed as fatal.

**Reason.** The real CLI quota failure showed Google response text flowing through `ArcanaError` into RecipeRunner logs, worker tick outcomes, and scheduler system-event messages. Provider response text may echo prompts or other submitted content. The adapter already had an allowlisted status/field summary for countTokens; applying the same boundary to all Google non-success responses preserves quota/retry decisions without propagating the raw body to downstream sinks.

**Evidence.** `cargo test -p arcana-core --lib provider::google::google_tests --locked -- --test-threads=1` passes 19/19, including generation/stream quota classification with planted private response text and an oversized-body fail-closed test. `cargo test -p arcana-core --lib exhausted_provider_failure_is_recorded_without_retrying_or_persisting_body --locked -- --test-threads=1` passes 1/1. `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and `git diff --check` pass. The Google adapter tests use the injected loopback base; `complete` and `stream` now honor `api_base` like `countTokens`.

**Limit.** This decision covers the Google adapter observed in the current Gemini production path. Other providers require separate source-grounded privacy review; it does not close P4-51 or TG4-24.

**Where.** `arcana-core/src/provider/google.rs`, `arcana-core/src/provider/google_tests.rs`, P4-51, TG4-24.

## ADR-LOG-RESTRUCT-0259 — Bound HTTP error bodies across provider adapters

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** All three shipped HTTP provider adapters (Google, OpenAI, Anthropic) consume non-success response bodies through one 16 KiB bounded reader for classification. Preserve the existing typed quota/retry class when a complete bounded body is available; return only a validated Google summary or a static provider-error summary. Oversized or unreadable bodies fail closed as `Fatal`; raw response text is never returned in `ArcanaError`.

**Reason.** OpenAI and Anthropic previously returned the complete provider response body from both unary and streaming failures. Such bodies can echo submitted prompts. Keeping body classification in the adapters preserves the non-retryable quota distinction while preventing those contents from flowing into RecipeRunner, worker outcomes, scheduler events, or logs.

**Evidence.** Local HTTP fixtures drive OpenAI and Anthropic unary and streaming methods with planted private response text and quota error bodies; `cargo test -p arcana-core --lib provider_error_body_is_not_forwarded --locked -- --test-threads=1` passes 2/2. The Google adapter suite passes 19/19, including a bounded oversized-body failure, and the production AgentEngine no-retry/analytics test passes 1/1. `cargo check -p arcana-core --locked`, workspace formatting, and diff checks pass.

**Limit.** This covers HTTP non-success bodies for the three shipped adapters only. Transport failures, malformed successful responses, in-stream provider errors, timeout/cancellation behavior, and the full ToolExecutor failure matrix remain in P4-51/TG4-25; no task or gate closes here.

**Where.** `arcana-core/src/provider/usage.rs`, `arcana-core/src/provider/google.rs`, `arcana-core/src/provider/openai.rs`, `arcana-core/src/provider/anthropic.rs`, their adapter tests, P4-51, TG4-25.

## ADR-LOG-RESTRUCT-0260 — Keep provider transport and stream diagnostics private

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Do not put raw `reqwest::Error`, response-decoder details, or malformed SSE data into provider errors or connection-test logs. Map request/stream transport failures to a bounded provider/stage code while preserving the established `[Retryable]` classification; map malformed successful JSON to a static `[Fatal]` code. SSE parsers continue to fail visibly, but return only a fixed invalid-frame message. Connection-test logs retain the operation/provider label without interpolating the request error.

**Reason.** `reqwest::Error` can retain the request URL, successful-response decode errors can carry request context, and the three SSE parsers previously included the full untrusted data frame in `ArcanaError`. Provider errors flow into model-facing tool results, worker outcomes, and logs, so content must remain bounded and source-free at the adapter boundary.

**Evidence.** Local adapter tests exercise malformed successful responses and unreachable endpoints for Google, OpenAI, and Anthropic; they assert safe stable error codes, preserved retry classification, and omission of planted URL/body markers. Parser regressions for all three providers confirm malformed SSE frames remain errors while private frame content is omitted. Focused results are recorded in the P4-51/TG4-25 progress note.

**Limit.** This closes identified provider-adapter diagnostic channels only. It does not prove every ToolExecutor failure class, provider timeout/cancellation, full telemetry readback, or successful CLI Fleet completion; P4-51/TG4-25 remain open.

**Where.** `arcana-core/src/provider/usage.rs`, `arcana-core/src/provider/google.rs`, `arcana-core/src/provider/openai.rs`, `arcana-core/src/provider/anthropic.rs`, `arcana-core/src/provider/streaming.rs`, provider adapter tests, P4-51, TG4-25.

## ADR-LOG-RESTRUCT-0261 — Export every retained append-only analytics detail

**Status.** Implemented and production-consumer tested on 2026-10-04.

**Decision.** `arcanactl analytics export` includes all raw records still inside the selected 90-day window from `token_usage`, `api_failures`, `system_events`, and `hook_logs`, alongside all daily cost summaries and combined daily/monthly worker history. Do not cap the full-export token rows at 10,000. Read hook logs from the schema-owned config database and emit a timed operation with `database=config`. Invalid timestamps cause export refusal rather than silent omission.

**Reason.** The existing export claimed to include all analytics but capped token usage at 10,000 and omitted two analytics append-only tables and the separately stored hook logs. That left operators without a complete retained-history artifact after the database pruner removed raw rows. The approved policy keeps full raw detail only inside 90 days, while cost and worker summaries preserve useful older history.

**Evidence.** `retained_export_is_complete_and_excludes_rows_past_the_raw_cutoff` exercises more than 10,000 token rows and expired/current event/failure rows through `AnalyticsReader::export_all`; it proves all in-window rows export, older raw detail does not, and malformed timestamps fail visibly. `hook_log_export_reads_complete_retained_rows_and_refuses_invalid_dates` proves hook export uses the cutoff and refuses malformed rows. The real CLI subprocess export fixture reads back retained API/system/hook records, pruned token detail, preserved cost summary, monthly worker coverage, and current-day worker rows. Focused suite names and results are recorded under P4-59/TG4-21.

**Limit.** This closes CLI retained-detail export coverage on Windows. Cross-platform writer contention and the remaining TG4-21 platform proof stay open; this does not close P4-59/TG4-21.

**Where.** `src-shared/src/db/analytics.rs`, `src-cli/src/commands/analytics.rs`, `src-cli/tests/analytics_export.rs`, `arcana-core/src/hooks/db.rs`, P4-59, TG4-21.

## ADR-LOG-RESTRUCT-0262 — Keep Windows project execution fail-closed on Home

**Status.** Evaluation recorded 2026-10-04; no production backend change.

**Decision.** Keep the current experimental Windows process sandbox fail-closed when it cannot canonicalize the selected workspace or toolchain path. Do not grant broader filesystem access or use the original workspace without the approved guarded staging/copy-back boundary. Windows Sandbox is not an available fallback on this host because it runs Windows 11 Home Single Language; evaluate that VM-backed candidate only on a supported Windows edition or dedicated runner.

**Reason.** Repeated exact-path, staged-config, private-stage, and ancestor-traversal probes still fail Rust path canonicalization with OS error 5 while direct file reads may succeed. Microsoft documents Windows Sandbox as unsupported on Home. Its mapped-folder write access persists to the host, so the approved stage/outbox/copy-back protocol would still need implementation and proof even on an eligible edition.

**Evidence.** The current host reports Windows 11 Home Single Language, build 26200. Official requirements list Windows Pro, Enterprise, Pro Education/SE, and Education as supported editions and state Home is unsupported. Configuration supports disabled networking, mapped folders, and logon commands, but writable mapped folders persist changes. The existing Windows bridge and full-core evidence remain as recorded under P4-11/P4-61 and TG4-18/TG4-31; no filesystem/network policy was relaxed during this evaluation.

**Limit.** This does not solve guarded staging/copy-back, locked dependency staging, Windows production Cargo canonicalization, Linux Landlock production proof, or signed macOS XPC/staging. P4-11/P4-61 and TG4-18/TG4-31 remain open.

**Where.** `arcana-core/src/platform/process_sandbox/`, P4-11/P4-61, TG4-18/TG4-31. References: [Windows Sandbox editions](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/), [Windows Sandbox configuration](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file), [Create Process in Sandbox API](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox).

## ADR-LOG-RESTRUCT-0263 — Trace Memory database initialization and migration

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Emit result-aware operation timings around the production Memory database initialization and schema-migration boundaries. Use fixed operation labels and the logical database name `memory`; never include the database path or migration error text in these events.

**Reason.** Memory query and mutation paths already report operation timings, and SQLite profile callbacks report statement categories and durations, but the real `MemoryDb::open()` and schema ladder had no parent result events. Without those events, slow startup and migration failures could not be separated from individual SQL statement timings in Logs.

**Evidence.** `memory_database_initialization_and_migration_emit_result_spans_without_paths` verifies successful initialization and migration events, duration fields, database identity, and path omission. `memory_database_initialization_failure_emits_a_redacted_error_span` forces the production open path to fail before creating the DB and verifies a result-aware error without the path. `cargo test -p arcana-core --lib memory_database_ --locked -- --nocapture --test-threads=1` passes 2/2.

**Limit.** This adds initialization/migration visibility only. It does not prove complete Memory operation or SQLite coverage, live Fleet-to-Logs delivery, or end-to-end observability costs; P4-50/TG4-24 remain open.

**Where.** `arcana-core/src/memory/db.rs`, P4-50, TG4-24.

## ADR-LOG-RESTRUCT-0264 — Order episodic Memory timestamps by instant

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Order session timelines and recent session summaries by SQLite `julianday(created_at)` and a stable ID tie-breaker instead of comparing timestamp text.

**Reason.** Episodic rows include legacy SQLite timestamps with a space separator and newer UTC RFC 3339 timestamps with `T`. Lexical order places the space-form before the `T`-form regardless of the represented instant, so consumers could observe conversation history and summaries out of chronological order.

**Evidence.** `production_timeline_and_summary_readers_order_mixed_timestamp_formats_by_instant` writes the timeline through production episodic writer methods, seeds mixed-format values in the isolated production Memory database, and verifies both production readers return the correct chronological order and parsed instants. `cargo test -p arcana-core --lib production_timeline_and_summary_readers_order_mixed_timestamp_formats_by_instant --locked -- --nocapture --test-threads=1` passes 1/1.

**Limit.** This proves two episodic Memory reader pairs only. It does not close the full timestamp inventory or TG4-19; remaining database defaults, triggers, and production write/read pairs still require proof.

**Where.** `arcana-core/src/memory/episodic.rs`, P4-57, TG4-19.

## ADR-LOG-RESTRUCT-0265 — Verify concurrent provider-consent revocation

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Keep the existing consent contract: a provider request that passed its persisted authorization check may complete in flight, while a new request must re-read consent and be denied after revocation.

**Reason.** Revocation applies before the next request; it cannot retract data already sent to a provider. A deterministic overlapping-call test makes that boundary explicit and guards against stale wrapper state authorizing later requests.

**Evidence.** `concurrent_revocation_blocks_the_next_request_without_rewriting_in_flight_authorization` pauses a fake provider after the production `ConsentProvider` wrapper authorizes and dispatches a request. It revokes the SQLite grant while that request is paused, proves the next wrapper call does not reach the provider, then releases the original request. `cargo test -p arcana-core --lib concurrent_revocation_blocks_the_next_request_without_rewriting_in_flight_authorization --locked -- --nocapture --test-threads=1` passes 1/1.

**Limit.** This proves the Rust provider-wrapper race only. Live desktop IPC, complete cross-consumer allow/revoke readback, and desktop smoke remain open; P4-60/TG4-16 stay open.

**Where.** `arcana-core/src/egress_consent.rs`, P4-60, TG4-16.

## ADR-LOG-RESTRUCT-0266 — Order session lists by timestamp instant

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Sort the production session-list query by `julianday(updated_at) DESC, id DESC` and add a matching expression index through additive session schema migration 3.

**Reason.** `SessionStorage::list_sessions` previously ordered the stored timestamp text directly. Legacy space-separated SQLite timestamps and canonical RFC 3339 `T` timestamps do not sort lexically by represented instant. The repository already uses `julianday` for mixed-format chronological readers; the matching index keeps this bounded 50-row dashboard query on an indexed path and the ID tie-break makes equal instants deterministic.

**Evidence.** `production_session_list_orders_mixed_timestamp_formats_by_instant` inserts through the production session writer, retains a legacy-form timestamp, reads through `list_sessions`, asserts ordering by parsed UTC instant, and verifies `EXPLAIN QUERY PLAN` selects `idx_sessions_updated_julianday`. `cargo test -p arcana-core --lib session::storage::tests:: --locked -- --test-threads=1` passes 4/4, including the existing canonical/legacy reader and malformed-time refusal checks. `cargo fmt --all -- --check` passes.

**Limit.** This closes only the session-list timestamp pair. P4-57/TG4-19 remain open for the full inventory and remaining production writer/readers.

**Where.** `arcana-core/src/session/storage.rs`, P4-57, TG4-19.

## ADR-LOG-RESTRUCT-0267 — Order Memory recency readers by timestamp instant

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Use `julianday(created_at) DESC` with a stable ID tie-break in Memory recency consumers and add a matching expression index through Memory schema migration 4. Parse timestamps as UTC instants before rank tie-breaking.

**Reason.** Memory history can contain both legacy SQLite timestamps and canonical UTC strings. Lexical ordering can place a later legacy instant before an earlier canonical instant. Existing production database readers already use `julianday` and matching indexes for this mixed-format compatibility; rank tie-breaking must use the same instant semantics.

**Evidence.** `recency_fallback_orders_mixed_timestamp_formats_by_instant`, `production_list_tool_orders_mixed_timestamps_by_instant`, and `equal_score_ties_order_mixed_timestamp_formats_by_instant` exercise the Memory fallback, production list tool, and rank tie-break. `cargo test -p arcana-core --lib mixed_timestamp_formats_by_instant --locked -- --test-threads=1` passes 4/4, including the session-list regression; the production list-tool test also passes independently.

**Limit.** This proves selected Memory consumers, not every Memory writer/reader pair. P4-57/TG4-19 remain open for full inventory and cross-platform property evidence.

**Where.** `arcana-core/src/memory`, P4-57, TG4-19.

## ADR-LOG-RESTRUCT-0268 — Use instant ordering for semantic Memory ties

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Keep semantic-fact priority ordered by confidence, then compare `updated_at` as a SQLite instant, with ID as a deterministic final tie-break. Add a partial expression index for active semantic facts.

**Reason.** The production fact-recall and preference commands ordered equal-confidence rows by raw timestamp text. Legacy space-separated SQLite timestamps and canonical UTC strings can sort contrary to the instant they represent. Both commands share one ordering rule and production SQL constants, and the index supports their confidence/recency order.

**Evidence.** `production_semantic_readers_order_mixed_timestamps_by_instant` seeds 5,000 populated semantic rows plus mixed-format preference rows in an isolated production Memory DB. It asserts both production readers return legacy noon before canonical 09:00 and both production SQL plans select `idx_sem_active_confidence_updated_julianday`. `cargo test -p arcana-core --lib production_semantic_readers_order_mixed_timestamps_by_instant --locked -- --nocapture --test-threads=1` passes 1/1.

**Limit.** This closes two Memory ordering readers only. P4-57/TG4-19 remain open for full writer/reader inventory and cross-platform property evidence.

**Where.** `arcana-core/src/memory/semantic.rs`, P4-57, TG4-19.

## ADR-LOG-RESTRUCT-0269 — Fail closed for unsupported host Ruff staging

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Keep the pinned Ruff host installer behind the Windows platform boundary where Arcana has a verified signed Python runtime and staged-wheel implementation. On Linux and macOS, return a structured `unsupported_platform` failure instead of referencing a module that is not compiled or attempting an unverified package installation.

**Reason.** The production Python runtime module is Windows-only, and the current Ruff staging implementation is verified only for Windows x64. ProjectCommand is cross-platform; its unsupported branch must compile and fail closed without allowing a Bash/package-manager bypass.

**Evidence.** Linux Ubuntu WSL test `agent::builtin_tools::project_command::tests::pinned_ruff_install_fails_closed_with_structured_unsupported_platform_result` passes 1/1 and asserts the categorized error, actionable guidance, and absence of HTTP detail. The Linux build also exposed an async Linux sandbox test that called `spawn` without awaiting it; the test now awaits the production launcher and passes 1/1. The Linux retention-filtered core suite passes 19/19, including both production analytics/hook active-writer tests.

**Limit.** This does not implement Ruff staging on Linux/macOS or close cross-platform ProjectCommand/security gates. Linux external-read/network containment, complete ecosystem corpus, full core suite, macOS sandbox, and Windows canonicalization remain open.

**Where.** `arcana-core/src/agent/builtin_tools/project_command.rs`, `arcana-core/src/platform/process_sandbox/linux.rs`, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0270 — Keep absolute SQLite paths out of backup telemetry

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Emit structured backup success events with fixed operation labels, outcome, and byte count. Do not put source or destination filesystem paths in logs. Keep the concrete backup path available on the returned `Backup` value and in operation-specific user-facing error messages where it is needed to recover.

**Reason.** The complete Linux core suite showed that migration and Clear Board backup logs serialize absolute temporary database paths. That violates the existing observability privacy contract and caused three path-redaction regressions. Structured telemetry can show that a recoverable backup was made and its size without exposing a user's home or project path.

**Evidence.** The Linux full core run exposed the three redaction assertions. After the change, the WorkersDb initializer, Memory DB initializer/migration, and Clear Board backup path-redaction tests pass on both Ubuntu WSL and Windows (3/3 per host). The backup API still returns its concrete path to its caller; telemetry carries outcome and byte count only.

**Limit.** This closes only backup success-event path leakage. Other errors that include paths must continue to be reviewed at their consumer boundary; P4-50/TG4-24 remain open.

**Where.** `arcana-core/src/db/backup.rs`, `arcana-core/src/db/migrate.rs`, P4-50, TG4-24.

## ADR-LOG-RESTRUCT-0271 — Keep Linux Cargo caches and targets in the workspace

**Status.** Implemented and production-tested on 2026-10-04.

**Decision.** Set `CARGO_HOME` and `CARGO_TARGET_DIR` to Arcana-managed cache directories under the selected workspace for Linux child processes.

**Reason.** A production Cargo fixture failed because inherited `CARGO_TARGET_DIR` pointed outside the selected workspace, where Landlock correctly denied the build lock write. Rust cache state must be explicit and scoped to the task workspace; this also avoids writing to a shared host cache during autonomous execution.

**Evidence.** `cargo_cache_and_build_output_are_isolated_inside_the_selected_workspace` passes on Linux. The production Rust ProjectCommand build/QC test, mixed-project spawn-failure test, cancellation/descendant cleanup test, and checked-in Rust/Node/Python-build corpus pass on Linux with the staged host toolchains.

**Limit.** This isolates cache writes; it does not stage locked dependencies or constrain filesystem reads/network access. P4-11/P4-61 and TG4-18/TG4-31 remain open.

**Where.** `arcana-core/src/platform/process_sandbox/mod.rs`, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0272 — Allow Linux children to write only to `/dev/null`

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Add a single Landlock write rule for `/dev/null` alongside the selected workspace write rule, and retain its file descriptor until child restriction is installed.

**Reason.** The Linux production fixture uses the host npm launcher, which writes redirections to `/dev/null`; write confinement denied this harmless runtime operation. A precise device exception supports conventional toolchains without opening general external writes.

**Evidence.** `confined_child_can_write_workspace_and_cannot_escape_via_symlink_or_child` passes on Linux with the `/dev/null` redirection, workspace write, and external symlink/descendant denial assertions.

**Limit.** This is only a write exception for one device. It does not implement the approved external-read/network denial contract or complete runtime allowlisting; P4-11/P4-61 and TG4-18/TG4-31 remain open.

**Where.** `arcana-core/src/platform/process_sandbox/linux.rs`, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0273 — Resolve Python through the Unix `python3` convention

**Status.** Implemented and production-tested on 2026-10-04.

**Decision.** On Unix hosts, resolve the Python toolchain to an executable `python3` found on PATH when available, and keep the existing `python` fallback. Preserve Windows' signed/staged Python runtime path.

**Reason.** The Linux production corpus reached Python after Rust and Node execution, then failed to spawn because Ubuntu provides `python3` while Arcana selected the Windows-oriented `python` name. This host convention can be resolved deterministically without shell invocation or automatic dependency installation.

**Evidence.** The checked-in ecosystem corpus passes on Ubuntu WSL with the host's `/usr/bin/python3` Build intent. The test project currently lacks pytest; this dependency is not installed implicitly, and staged Windows-runtime tests remain the pytest execution proof.

**Limit.** This only resolves the interpreter executable. It does not install pytest/Ruff or stage project dependencies; P4-11/P4-61 and TG4-18/TG4-31 remain open.

**Where.** `arcana-core/src/platform/command.rs`, P4-11/P4-61, TG4-18/TG4-31.

## ADR-LOG-RESTRUCT-0274 — Flush appended bytes before reporting success

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Require `append_file` to complete an async flush before it reports a successful append. If either writing or flushing fails after side effects start, return the existing `append_outcome_unknown` result and instruct reconciliation against the checkpoint.

**Reason.** A full Linux core run intermittently observed an empty file after the production tool returned success. The isolated append checkpoint test also reproduced once; awaiting a flush gives the tool a completion boundary before a consumer reads the file and avoids reporting success while buffered work may remain.

**Evidence.** After the change, the production task-checkpoint append and partial-write unknown-outcome tests pass together (2/2) on Ubuntu WSL and Windows. The complete Linux `arcana-core` library suite passes 966/966 with the native Linux toolchain environment.

**Limit.** This does not provide durable-media `fsync`; it ensures the async writer flush completed before success. P4-19/TG4-09 remain open for full artifact handoff and supported-platform proof.

**Where.** `arcana-core/src/agent/builtin_tools/append_file.rs`, P4-19/P4-20, TG4-09.

## ADR-LOG-RESTRUCT-0275 — Bound raw RMCP Streamable HTTP responses

**Status.** Implemented and focused-tested on 2026-10-04.

**Decision.** Route the configured GitHub MCP connector through an Arcana-owned RMCP `StreamableHttpClient` implementation. Reject HTTP JSON bodies above 1 MiB before deserialization and enforce a cumulative 1 MiB raw-byte cap on SSE GET/POST response streams before parsing. Retain the separate 64 KiB serialized result cap before returning MCP results to the model.

**Reason.** The RMCP default reqwest adapter bounds individual SSE events but reads JSON success/error bodies without a byte limit before parsing. A post-parse result cap therefore did not bound transport memory or parser work. RMCP supports a generic client implementation, allowing Arcana to retain its existing reqwest 0.12 version and redirect-denial policy without adding a second reqwest client version.

**Evidence.** Windows and Ubuntu WSL `cargo test -p arcana-core --lib mcp --locked -- --nocapture --test-threads=1` pass 14/14 each. Production-shaped tests reject oversized chunked JSON before parsing and oversized raw SSE data, and existing local RMCP discovery/call, redirect denial, AgentEngine executor, consent revocation, and cancellation-classification tests pass through the bounded backend. `cargo check -p arcana-core --locked`, workspace formatting, and diff checks pass. Direct `http` and `sse-stream` dependencies use already-locked versions; no package version was added.

**Limit.** This proves bounded shared-core transport only. It does not prove remote cancellation acknowledgment, every HTTP/auth/status combination, Tauri and CLI host runtime behavior, MCP event persistence/Logs readback, successful CLI Fleet completion, or full TG4-24. P4-50/TG4-24 remain open.

**Where.** `arcana-core/src/agent/builtin_tools/bounded_mcp_client.rs`, `external_mcp.rs`, P4-50, TG4-24.

## ADR-LOG-RESTRUCT-0276 — Serialize migration version checks and isolate automatic backup names

**Status.** Owner approved, implemented and production-startup-proven on 2026-10-04.

**Decision.** Acquire SQLite `BEGIN IMMEDIATE` before reading `user_version` for each migration rung, recheck the version after every write reservation, and commit schema work and version together. Use the existing rusqlite transaction guard to roll back every error path. Automatic destructive backups carry a UUID suffix; explicitly supplied backup names retain their refusal-to-overwrite contract.

**Reason.** Real isolated desktop startup starts seven workers concurrently. Their stale version reads re-applied trigger migrations, and different startup migrations collided on second-resolution backup names. A process-local mutex cannot cover CLI/desktop processes, whereas SQLite's writer reservation does. The owner explicitly approved this shared migration-boundary change.

**Where.** `arcana-core/src/db/migrate.rs`, `arcana-core/src/db/backup.rs`; core P4 desktop startup dependency.

**Proof required.** Concurrent file-backed ladder and TasksDb openers, destructive snapshot preservation and failure rollback, focused regression suites, and a real isolated desktop startup/retention smoke. No task or gate closes from implementation alone.

**0276 verification.** Migration tests pass 18/18 and backup tests 6/6 on Windows and Ubuntu WSL. Windows DB regression passes 213/213; workspace check passes. First-open WAL conversion also needed a lock-only retry bounded by the existing five-second timeout. Legacy repairs reserve a writer only when no caller transaction exists. Canonical snapshot paths preserve Windows extended-length support after unique names exposed a nested-path regression; the real legacy migration test passes. Isolated desktop startup now starts all seven workers without migration failures; `_verify/P4-55-RETENTION-CLOSEOUT-2026-10-04.md` records the smoke.

## ADR-LOG-RESTRUCT-0277 — Exercise persisted workflow portability in the existing host matrix

**Decision.** Add the existing production recipe, skill and hook portability checks to the three-OS build-smoke job. Use Bash with errexit so a failed early Cargo command fails the step on Windows too. Add a persisted legacy-hook regression that reads through HooksDb and reaches the real HookExecutor refusal boundary.

**Reason.** P4-16/TG4-17 cover saved custom content as well as shipped defaults. Existing Windows proof and Linux core proof are insufficient for the desktop skill reader on Linux/macOS. Reuse the established CI matrix and dependency setup rather than inventing a parallel verification route.

**Proof.** Local Windows recipe/skill/hook filters pass; Linux core filters pass, including the new production legacy-hook refusal. CI workflow correctness gates pass 3/3. Remote Linux/macOS desktop results remain required before closing P4-16/TG4-17.

## ADR-LOG-RESTRUCT-0278 — Preserve exact registered telemetry identifiers at the writer

**Status.** Owner approved and implemented on 2026-10-04; focused Windows/Linux and real desktop/CLI writer proof pass.

**Decision.** Keep generic structured-value/credential redaction unchanged. At the telemetry JSONL writer only, preserve exact fixed operation and event labels from a source-controlled allowlist. Unknown strings and sensitive fields still use the existing redactor. Derive a source coverage test with the existing test-only syn parser so new literal producers cannot silently lose their labels.

**Reason.** The real desktop startup stream replaced `scheduler_recoverable_work_check` and `database_schema_migration_ladder` with `[REDACTED_KEY]`, destroying operation identity. Parent capture tests bypassed the production writer, so they did not catch it. The owner explicitly approved registered-label preservation. A separate decision is pending for canonical UUID correlation fields, also matched by the same heuristic.

**Proof.** Production writer tests preserve registered long labels in event and parent-span fields while redacting unknown secret-like strings, sensitive keys and foreign targets. The syn coverage test includes literal timers, fixed helper-call labels and fixed match selections. Observability suite passes 27/27 Windows and 28/28 Ubuntu WSL. The real CLI writer-to-full-export test passes on both hosts. Real isolated desktop startup persists 1,385 events, including seven identifiable idle worker work checks, 61 migration spans and 15 worker-initializer spans, with zero provider calls; clean shutdown is verified. `_verify/P4-50-REGISTERED-LABELS-2026-10-04.md` records evidence. P4-50 remains open, including the separate UUID-correlation decision.

## ADR-LOG-RESTRUCT-0279 — Compare timestamp offsets without signed-zero identity

**Decision.** Preserve the database timestamp test's exact offset equation, but assert that `correct - naive + offsetMs` is zero. Do not distinguish IEEE signed zero when the CI host is UTC.

**Reason.** Authenticated CI logs show the frontend gate failing on `expected +0 to be -0` under UTC, preventing the dependent P4 host matrix. The prior commit also fails on this test; the date parser itself is unchanged.

**Proof.** Frontend suite passes 53/53 with `TZ=UTC`; the timestamp suite passes 11/11 with `TZ=Asia/Bangkok` and `TZ=America/New_York`. Typecheck passes. This unblocks a prerequisite, not P4-57/TG4-19 closure.

## ADR-LOG-RESTRUCT-0280 — Measure real recovery ticks and existing provider turn caps

**Decision.** Extend P4-43's production startup test with Tokio's existing paused-clock test facility, advancing beyond the actual fixed five-minute cron boundary. Drive the persisted shipped Supervisor/QC recipes through RecipeRunner and the real engine with a deliberately continuing scripted provider, checking the existing 10/20-turn run caps.

**Reason.** Prior helper and startup-only tests could not establish cron request counts or intentional work bounds. These tests use the existing production loops and policies, without a new scheduler abstraction, live credentials or provider requests.

**Proof.** Windows/Linux startup and real cron recovery tests pass with zero provider entries; continuing persisted Supervisor/QC RecipeRunner runs dispatch 9/19 requests and refuse the pending turn at the existing 10/20 thresholds, returning `BUDGET_EXCEEDED_TURNS` with inbox work still present. P4-43 is proven; `_verify/P4-43-CALL-BOUNDS-CLOSEOUT-2026-10-04.md` records exact evidence. The pending-turn semantics are unchanged.

## ADR-LOG-RESTRUCT-0281 — Follow bounded provider classification in the CI source guard

**Decision.** Update TG-P2-06's source guard to check each unsuccessful-status block's existing safe response helper and attached class, then verify the bounded helper's classification chain. Keep all adapter error behavior unchanged.

**Reason.** Authenticated run 37216487226 failed only this desktop source guard (165 passed, one failed): it still counted inline `ErrorClass::classify` calls, which now live behind the approved bounded-body helper. All six complete/stream error blocks use that helper. This stale test prevents the dependent P4 three-platform workflow corpus from executing.

**Proof.** Repaired source guard passes 1/1; full desktop source-gate module passes 77/77. Production Anthropic/OpenAI complete/stream error privacy fixtures pass 2/2; Google quota-class/privacy fixture passes 1/1. Workspace check and formatting pass. Remote CI must rerun; no P2 or P4 gate closes from the source guard alone.

## ADR-LOG-RESTRUCT-0282 — Inject the manual-release race through existing tracing

**Decision.** Test P4-40's late reassignment boundary using a test-only tracing Layer that observes the real `database_task_orphan_release` span entry. A separate isolated TasksDb connection replaces the claim after command preflight but before its compare-and-set. Add direct-SQL invariant falsification and document the production writer inventory.

**Reason.** The existing Tauri regression changes the claim before preflight and cannot prove the later window. The production release already creates the span immediately before its atomic SQL, so the test needs no shipped hook, API or schema change. Keep the owner-approved DB-wide claim invariant and CAS policy unchanged.

**Proof required.** Real Tauri command reaches the post-preflight span, rejects the stale CAS and preserves the replacement owner/PID/host; direct SQL cannot introduce inconsistent claim metadata; production writer and recovery consumers are accounted for. Those checks pass; P4-40 is proven in `_verify/P4-40-CLAIM-SAFETY-CLOSEOUT-2026-10-05.md`. The raw SQL and claim-tool tests pass on Windows/Linux; the late manual command race and CLI smoke pass on Windows. The manual endpoint has no React caller; no manual UI-button proof is claimed.


## ADR-LOG-RESTRUCT-0283 — Follow shared startup spend hydration in the CI guard

**Decision.** Update S2-06's stale desktop source guard to trace desktop and CLI Fleet entry points into `runtime::initialize`, then `hydrate_state` and `AppState::hydrate_today`. Add isolated persisted-counter readback through the actual shared hydration helper, including old-day refusal.

**Why / when / where.** On 2026-10-05 CI run 37218542575 passed desktop unit tests and core library tests, then stopped on the old assertion requiring an inline hydration call in `src-tauri/src/main.rs`. Startup hydration moved to the approved shared host initializer; the runtime still calls it. This is a test ownership correction, not a spending-policy change.

**Proof.** S2-06 integration suite passes 9/9 on Windows/Linux. The shared hydration helper restores the prior instance's persisted cost/token counts and discards an old-day stamp; runtime group passes 5/5 on Linux and within the Windows 11/11 runtime-filter group (which also includes Python runtime tests). Workspace check and format check pass. The remote host matrix remains pending until upstream workspace tests pass.


## ADR-LOG-RESTRUCT-0284 — Prove ACP replay after a completed domain effect

**Decision.** Extend the existing scheduler scripted-provider fixture with an opt-in pause after its response script. Drive the saved Supervisor inbox recipe through the real scheduler, stop it after a task cancellation succeeds but before message acknowledgement, then redispatch via a newly constructed scheduler. The second run reads the same message ID and current task state before acknowledging without repeating the effect.

**Why / when / where.** On 2026-10-05 P4-23/TG4-11 still lack interruption-to-replay side-effect proof despite existing stable-ID inbox and stale-message QC recovery tests. Count actual task-status mutations with an audit trigger in the isolated test database; no production table, runner seam or API change is introduced. Record the provider's actual tool-result inputs to prove reconciliation state reaches the consumer.

**Proof.** Those behaviors pass on Windows/Linux; the captured consumer receives the persisted domain-state result and saved replay policy, and an isolated status-audit trigger counts one effect. Native backend sender → saved QC recipe acknowledgement and stale-message nudge recovery also pass. Details: `_verify/P4-23-ACP-REPLAY-CLOSEOUT-2026-10-05.md`. The fixture was corrected to use the live tool/Usage field names, request a full JSON task result rather than compact output plus hint, and compare distinct durable run IDs because `latest_worker_ticks` intentionally returns one latest row per worker; no production bug was inferred from those test mistakes.


## ADR-LOG-RESTRUCT-0285 — Construct checkpoint-failure fixtures before publishing HOME

**Decision.** Build the isolated unusable config.db directory before entering the process-global test home redirect in both write-file checkpoint refusal fixtures. Do not delete an existing database or weaken the refusal assertion.

**Why / when / where.** On 2026-10-05 CI run 37220225970 passed 983 core library tests but the checkpoint-executor fixture failed while creating its blocker: config.db already existed. Concurrent tests can persist counters into a process-global redirected home. Preparing a previously unknown unique fixture path first makes the injected failure deterministic. My initial guess of another stale structural guard was wrong; the authenticated log identifies fixture setup, not checkpoint production behavior.

**Proof.** Windows checkpoint refusal/regression group passes 12/12. Linux core library passes 985/985 with CI's four-test-thread setting (86.68s), including both refusal fixtures; all core integration targets pass 44/44. An initial Linux run passed 984 tests but its ecosystem corpus could not find npm: this harness lost its temporary Linux runtime and excluded it from PATH. Restoring the already-approved official Node 24.21.0 runtime to a persistent user-owned test-tools directory, verifying its archive against official SHASUMS256, and selecting the Linux-only PATH fixes the harness; no product launcher policy changes. Remote three-platform matrix remains pending.

## ADR-LOG-RESTRUCT-0286 — Exercise startup exhaustion and the post-effect boundary

**Decision.** Extend the existing shipped-worker scripted-provider fixture with a failure immediately after its real host Git commit. Add an exhausted-startup run with an assigned task's persisted budget and a pre-existing daily reservation; verify no provider call, claim, mutation or reservation replacement occurs.

**Why / when / where.** On 2026-10-05 P4-52/TG4-26 still lacks these falsifiable boundaries. Keep production retry policy and stores unchanged. Test the saved recipe, scheduler and RecipeRunner using existing fixture seams, rather than adding a new runtime retry mechanism. The separate P4-53 continuity contract owns post-effect reconciliation.

**Proof.** Shipped-worker startup/commit group passes 3/3 on Windows (4.85s) and Linux (4.67s); bounded retry policy group passes 9/9 on both. Exhausted setup preserves the exact daily hold/spend and saved task budget. Post-commit failure produces one commit, a failed run and needs_info quarantine; another scheduler kick dispatches no request. A new standalone CLI integration target runs a permanent startup failure through RecipeRunner and the real redacting logger, then reads the persisted event through the actual arcanactl export subprocess (Windows 1/1, 1.44s; Linux 1/1, 1.34s). Workspace check and formatting pass. P4-52/TG4-26 remains open for the deadline issue below; no Calendar/live-provider claim.


## ADR-LOG-RESTRUCT-0287 — Expose the synchronous startup deadline limitation

**Finding / decision pending.** P4-52/TG4-26's total elapsed cap is not a hard interrupting deadline. `retry_startup_with_state_and_delay` calls synchronous `prepare()` before checking elapsed time; successful preparation returns without an elapsed check. A blocked SQLite/keychain call can overrun the two-second policy. The existing fast-closure tests prove retry admission and delay bounds, not a hard setup deadline. The plan's total-elapsed claim is overstated and is explicitly flagged rather than silently narrowed.

**When / sources / next.** 2026-10-05, runner retry helper and its RecipeRunner setup closure. Tokio documents that started spawn_blocking work cannot be aborted, so moving the closure to that pool alone would not establish cancellation safety: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html . Asked the owner to choose a cancellable hard-deadline startup refactor or explicit retry-admission/overrun semantics. No runtime or deadline contract change before that answer; continue independent core P4 work.

## ADR-LOG-RESTRUCT-0288 — Prove independent saved-task and daily budget breakers

**Decision.** Exercise both enabled limits together through AgentEngine, the real ClaimTaskTool and CappedProvider. With a permissive daily ceiling, a saved one-turn task cap must stop the second request and persist its breaker. With a denied daily reservation, the provider must not dispatch or claim the assigned task and the saved budget must remain intact; worker pause is read back.

**Why / when / where.** 2026-10-05 P4-29/TG4-15 is open specifically for interaction proof. Reuse the existing scripted provider fixture with catalogue metadata for its explicit Google identity; no production limit, pricing, schema or API changes. The older fixture's message incorrectly calls its token exhaustion a one-turn cap although max_turns is ten; correct that assertion wording and historical evidence wording without altering its behavior.

**Proof.** Engine consumer group passes 10/10 on Windows (4.27s) and Linux (5.70s), including both exact-cost/different-refusal cases and the original saved-token cap. Production readers verify cap/usage, needs_info versus assigned, and worker pause versus no daily pause; zero holds and exact observed daily microdollars are read back. Earlier daily regression group passes 15/15 on both. Workspace check/format pass. P4-29/TG4-15 is proven in `_verify/P4-29-BUDGET-INTERACTION-CLOSEOUT-2026-10-05.md`; TG2-17 remains separate. The first new fixture used obsolete Usage field names; corrected to live input/output before verification, not a production bug.
