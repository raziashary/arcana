# A18 — MASTER_DEVELOPMENT_PLAN.md staleness audit

Scope: everything in MDP EXCEPT §5.1/§5.2/§5.5 task counts (in hand by main session).
Mode: read-only. Findings appended as discovered.

## S-1 · The document header still says the programme is awaiting GO and no code may be written · blocker
**Where:** `MASTER_DEVELOPMENT_PLAN.md` — the front-matter table:
> `| **Status** | ⛔ **AWAITING GO. No code, no migrations, no deletions.** |`
and the row above it: `| **Rev** | 1.0 — created 2026-08-31 |`
**What it says:** the plan is unstarted and no code, migration or deletion is authorised.
**What is actually true:** P0, P1, P2, P3 A–E, P3-F and PD0 are complete and committed. `git log --oneline -25` shows `1cb8b9a feat(PD0): the design capability contract, exercised across a real process` and `dfd1876 feat(memory): P3-F complete …`. Migrations have landed (§1.1a's own "Live schema head" note says "**Superseded by P1 (2026-09-01)** … graph store is at **v6**") and deletions have happened (`src-tauri/src/mcp_server/` no longer exists: `ls src-tauri/src/mcp_server/` → `No such file or directory`).
**Confidence:** certain (verified)

## S-2 · §0.1 states the GO gate as still closed · major
**Where:** `MASTER_DEVELOPMENT_PLAN.md` §0:
> `**§0.1 — The GO gate.** No development until Razi's explicit GO. Rev 1.0 is a plan, not a mandate.`
**What it says:** development has not been authorised.
**What is actually true:** GO was given and seven phases have been built. This is the same falsity as S-1 restated as a standing rule, which is worse: §0 is the section every session and every subagent dispatch is told to read (§0.8's own words).
**Confidence:** certain (verified)

## S-3 · §0.2e says the plan holds 154 tasks · major
**Where:** `MASTER_DEVELOPMENT_PLAN.md` §0.2e, the italic amendment:
> `*Adapted rather than adopted literally: the source rule requires a dedicated smoke check **and** gate per task. Here that would inflate 154 tasks into 154 gates …*`
**What it says:** the task total is 154.
**What is actually true:** the checkbox-derived total is **506** (312 open), per the phase-by-phase counts supplied as ground truth for this audit and consistent with the MDP's own per-phase checkbox blocks. The figure 154 predates PD0–PD6, P8's expansion and the P3 split. (Independent corroboration: `grep -c "^- \[" MASTER_DEVELOPMENT_PLAN.md` — see S-3a below.)
**Confidence:** certain (verified)

## S-4 · §0.2j lists four "known web-answerable open items" of which three are in shipped phases · major
**Where:** `MASTER_DEVELOPMENT_PLAN.md` §0.2j:
> `*Known web-answerable open items in this programme:* whether `ort`'s ONNX runtime can be vendored offline rather than fetched at build time (`P3-33`) · xAI's finish-reason taxonomy, which `G6 §5.5` records as **undocumented** and which `TG2-10` depends on · Tauri v2 capability semantics for the narrowed permission set (`P0-26`) · whether `keyring` 2.x exposes a usable fallback on headless Linux or whether the encrypted-file path must be written from scratch (`P2-24`).`
**What it says:** these four questions are open.
**What is actually true:**
- `P3-33` closed in P3-F. The MDP's own §0.16 `W-3` records the answer: *"`P3-33` said to disable `ort`'s default features, and `fastembed` already does"*, and `L-29`'s corollary records the `ORT_STRATEGY=system` finding. `autodev-core/Cargo.toml:52-68` carries the razi-ruled pin `fastembed = { version = "=6.0.2", optional = true }`.
- `P0-26` closed in P0 (phase complete).
- `P2-24` closed in P2 (phase complete).
- The xAI item is dead by a different route: `§1.2 GAP-02` on the same page says xAI is **CLOSED-AS-WONTFIX 2026-09-01 (razi-ruled)** — so `TG2-10`'s stated dependency on an xAI taxonomy is itself stale here.
**Confidence:** certain (verified) for P3-33 and the xAI contradiction; likely for P0-26/P2-24 (asserted from phase-complete status rather than task-level re-read)

## S-5 · §1.1 "What exists" table carries pre-restructure sizes for every row · major
**Where:** MDP §1.1:
> `| Tauri backend | src-tauri/src/ | 53,387 LOC, 32 modules | 256 Tauri commands, all registered |`
> `| Frontend | src/ | 39,474 LOC | 9 tabs, 20 component dirs, 28 stores |`
> `| CLI | src-cli/src/ | 3,508 LOC | airsctl; agent commands are 100% sidecar HTTP |`
**What it says:** backend 53,387 LOC / 32 modules / 256 commands; frontend 39,474 LOC / 20 component dirs / 28 stores; CLI 3,508 LOC.
**What is actually true (measured 2026-09-03):**
- src-tauri/src/ is **20,534 LOC across 84 .rs files** (find src-tauri/src -name "*.rs" -exec cat {} + | wc -l -> 20534). P1-05 moved 182 files into autodev-core, now **63,382 LOC over 219 files**. The 53,387 figure describes a tree that no longer exists, and the table has **no autodev-core row at all**, even though the Shared row in the same table was updated for P1-09.
- Tauri commands: 258, not 256 (see S-6).
- src/ is **40,088 LOC**; ls src/components/ | wc -l -> **25** (not 20); ls src/stores/ | wc -l -> **29** (not 28).
- src-cli/src/ is **4,019 LOC**, not 3,508.
**Confidence:** certain (verified)

## S-6 · The Tauri command count contradicts itself between §1.1 and §4, and neither matches the tree · major
**Where:** §1.1 says `256 Tauri commands, all registered`; §4's module tree says:
> `  commands/                         [existing]  255 #[tauri::command]`
**What it says:** two different counts, 256 and 255, in one document.
**What is actually true:** **258**.
Command: `grep -rc "#\[tauri::command\]" --include=*.rs src-tauri/src/ | awk -F: '{s+=$2} END {print s}'` -> 258.
This is L-19 ("two lists that must agree will disagree") inside the plan that records L-19.
**Confidence:** certain (verified)

## S-7 · §4's module tree still lists mcp_server/ as a present directory awaiting P0 deletion · major
**Where:** §4:
> `  mcp_server/{server,tool_adapter,port_status}.rs [P0 DELETE — 705 LOC]`
> `  mcp_server/health_monitor.rs      [P0 MOVE out of the sidecar-named tree]`
**What it says:** src-tauri/src/mcp_server/ exists and holds four files, two of them pending P0 work.
**What is actually true:** the directory does not exist — `ls src-tauri/src/mcp_server/` -> `No such file or directory`. P0-13 and P0-14 are both [x] in §6 of this same document; the health monitor moved to `src-tauri/src/mcp/`, confirmed by `ls src-tauri/src/`. The §4 tree also has no `mcp/` entry for the destination.
**Confidence:** certain (verified)

## S-8 · §4's sidecar LOC and §5.3's sidecar LOC disagree, and neither matches the tree · minor
**Where:** §4: `  sidecar/                          [P8 DELETE — 3,161 LOC]`
and §5.3: `carrying 3,867 lines of dead sidecar, 17 dead commands and a 225 MB bundle resource.`
**What it says:** 3,161 lines in one place, 3,867 in another.
**What is actually true:** `wc -l src-tauri/src/sidecar/*.rs` -> **3300 total** across client.rs, mod.rs, sse.rs, startup_progress.rs. Neither figure is current, and the two disagree with each other.
**Confidence:** certain (verified)

## S-9 · §2's substrate block asserts a release profile that was removed and inverted · major
**Where:** §2, inside the ```rust block:
> `// Release profile — CHANGES IN P3 (M-41)`
> `[profile.release] panic = "abort"    // Cargo.toml:126-131  → becomes "unwind"`
**What it says:** `panic = "abort"` is in effect today at Cargo.toml:126-131 and will change in P3.
**What is actually true:** it changed, and it was never in effect. src-tauri/Cargo.toml:117-121 now reads:
> `# ⚠ The [profile.release] block that lived here was **moved to the workspace`
> `# root** in P3-01. Cargo ignores [profile.*] in a non-root member and warns`
> `# on every invocation, so none of it — including panic = "abort" — was ever in`
> `# effect. Do not re-add it here; see the root Cargo.toml.`
and the workspace-root Cargo.toml:36-37 reads `[profile.release]` / `panic = "unwind"`, with :24 recording `#   * panic = "abort" was **never applied** — unwinding has been on all along.`
So the citation `Cargo.toml:126-131` no longer resolves to a profile block, the value is wrong, the tense is wrong, and the "was never in effect" correction never reached §2.
**Confidence:** certain (verified)

## S-10 · §1.2's gap register lists eight gaps that later phases closed, with no annotation · blocker
**Where:** §1.2. These rows carry **no** strike-through and **no** CLOSED annotation, unlike GAP-02, GAP-11, GAP-15 and GAP-16 in the same table which do:
> `| GAP-03 | Any **prompt caching** on any provider | G2a T1 |`
> `| GAP-04 | Any **embeddings or vector search** — zero hits repo-wide | G1 §C.4 P3 |`
> `| GAP-05 | Any **frontend test framework** — no vitest, jest, playwright, no test script | G0 §2 |`
> `| GAP-06 | Any **ESLint** — npm run lint is tsc --noEmit | G0 §2 |`
> `| GAP-07 | Any **macOS CI job** | G0 §2 |`
> `| GAP-12 | Any **retry policy** — ErrorClass::Retryable has no reader | R3 |`
> `| GAP-14 | Any **spend ceiling or kill switch** | R3 |`
> `| GAP-17 | Any **fault isolation** — panic = "abort" + in-process agents | R1 B-04 |`
**What is actually true:**
- **GAP-03** closed in P2. autodev-core/src/cache/mod.rs, cache/monitor.rs, provider/anthropic.rs, provider/streaming.rs, provider/usage.rs all reference cache_creation_input_tokens / cache_read_input_tokens / cache_control. §0.16 L-16 in this same document analyses an observed cache *hit* in p2_16.
- **GAP-04** "zero hits repo-wide" is false: autodev-core/src/memory/embed.rs exists; autodev-core/Cargo.toml:48 `sqlite-vec  = "0.1.9"` and :68 `fastembed = { version = "=6.0.2", optional = true }`. §11's P3-32 is [x].
- **GAP-05** closed by P0-19 ([x] in §6). package.json:14 `"test": "vitest run"`, :71 `"vitest": "^4.1.11"`.
- **GAP-06** closed by P0-18 ([x]). package.json:11 `"lint": "eslint ."`, :61 `"eslint": "^9.39.5"`; eslint.config.js exists at repo root.
- **GAP-07** closed by P0-20 ([x]). .github/workflows/ci.yml:217 `os: macos-14   # Apple Silicon`; build-release.yml:161-164 `build-macos: … runs-on: macos-14`.
- **GAP-12** closed in P3. autodev-core/src/provider/retry.rs:1 opens `// The retry policy \`ErrorClass::Retryable\` never had (P3-09, GAP-12).` — the file that closes the gap names the gap.
- **GAP-14** closed in P2. autodev-core/src/budget/daily.rs, budget/mod.rs, agent/builtin_tools/set_task_budget_tool.rs exist; §0.16 L-21 narrates the daily ceiling being built and then wired.
- **GAP-17** closed in P3-01: root Cargo.toml:37 `panic = "unwind"` (see S-9).
**Confidence:** certain (verified)

## S-11 · GAP-01's evidence ("rmcp is compiled server-features-only") no longer describes the tree · minor
**Where:** §1.2:
> `| GAP-01 | Any MCP **client** — rmcp is compiled server-features-only and cannot dial out | G12 |`
**What it says:** rmcp is a compiled dependency with server features only.
**What is actually true:** rmcp is not a dependency at all. `grep -rn "rmcp\|axum" src-tauri/Cargo.toml autodev-core/Cargo.toml` returns nothing — P0-16 ("Drop rmcp and axum from Cargo.toml") is [x]. The *gap* (no MCP client) is still real; the *evidence sentence* is false.
**Confidence:** certain (verified)
