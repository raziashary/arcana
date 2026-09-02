# AutoDev Restructure — Runtime ADR Log

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
