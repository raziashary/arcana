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

## S-12 · The front-matter Rev (1.0) contradicts the change log's own newest Rev (2.1) · major
**Where:** front matter `| **Rev** | 1.0 — created 2026-08-31 |` versus §16 Change log's newest row:
> `| 2026-09-03 | 2.1 | **PD0 · The capability contract closed — properly, on the second attempt.** …`
**What it says:** the document is at rev 1.0.
**What is actually true:** rev 2.1, per its own change log and per `READINESS_MATRIX.md` (which the ground truth for this audit records at rev 2.1). The header was never advanced through seven phases of closeouts.
**Confidence:** certain (verified)

## S-13 · §16's change log breaks its own stated ordering, is missing a rev number, and has no rows for P1, P2 or P3 A–E · minor
**Where:** §16 header `*Append-only, newest first.*` then, in order:
> `| 2026-09-03 | 2.1 | … |`
> `| 2026-09-01 | **Re-planned around two rollouts** (`ADR-LOG-RESTRUCT-0024`). … |`
> `| 2026-09-02 | 1.9 | **`P3-F · Memory` built and closed out …** |`
> `| 2026-09-01 | 1.4 | **P0 built and closed out …** |`
**What it says:** newest first, append-only, one rev per row.
**What is actually true:**
1. The 2026-09-01 "Re-planned around two rollouts" row sits **above** the 2026-09-02 row — the ordering claim is false.
2. That same row has **no rev cell**: the bolded change text occupies column 2, so the row has two columns where every other row has three.
3. Revs **1.5, 1.6, 1.7 and 1.8 have no rows at all** — the log jumps 1.4 (P0) straight to 1.9 (P3-F), so the closeouts of **P1, P2 and P3 A–E** are unrecorded in the log that §0.11/§0.14 rely on. Those three phases are complete per §7/§8/§9's checkboxes and per `git log`.
**Confidence:** certain (verified)

## S-14 · §15's risk log leaves risks owned by completed phases unannotated, while comparable rows carry closure notes · major
**Where:** §15. `R-8` and `R-13` carry `✅ **CLOSED 2026-08-31.**`; these do not:
> `| **R-2** | Caching adopted, silently does nothing | Alarm before cache (P2-12) | P2 |`
> `| **R-3** | The port reproduces the sidecar's own defects | Explicit fix-list (P3-43) | P3 |`
> `| **R-4** | macOS verified last, blocks release | macOS CI in **P0** | P0 |`
> `| **R-7** | `Item::Opaque` deferred, adapters rewritten twice | Day-one trait decision (P2-01) | P2 |`
**What it says:** these risks are live and owned by P0/P2/P3.
**What is actually true:** P0, P2 and P3 are all complete. Their mitigations shipped (`P2-12`, `P2-01`, `P3-43` and `P0-20` are all `[x]` in this document). A risk whose owning phase closed is either discharged or must be re-seated; the register says neither.
**Confidence:** certain (verified) that the owning phases are closed; needs a decision on whether each risk is discharged

## S-15 · R-11's mitigation is recorded as belonging to P3, but P3-33 explicitly did not do half of it · major
**Where:** §15:
> `| **R-11** | *(new, R1)* ONNX runtime downloaded at build time breaks offline/reproducible builds | Pin and vendor (P3-33) | P3 |`
**What it says:** P3-33 pins and vendors, and P3 owns the risk.
**What is actually true:** `P3-33` is `[x]` but its own completion note in §9 says:
> `⚠ **"Vendor" is NOT done and is not deferrable by me**: it needs somewhere to host a 30 MB runtime, which is a distribution decision. Recorded as razi's open call before embeddings enter a release build.`
So the risk is half-mitigated and its owner is now an open Razi decision, not a closed phase. §15 records neither.
**Confidence:** certain (verified)

## S-16 · R-12 restates an emit-site count this document itself refuted in P1-06 · major
**Where:** §15:
> `| **R-12** | *(new, R1)* `autodev-core` extraction is larger than scoped — 132 `emit_*` sites, not 62 lines of `AppState` | Sized as its own group (P1-B); event sink injected | P1 |`
**What it says:** there are 132 `emit_*` sites.
**What is actually true:** §7's `P1-06` note in this same document says:
> `*Counts corrected again:* not 132/27/11 and not the 71/24/21 of `P1-CORRECTIONS` either — the live figures were **19 `emit_X(&app, …)` / `emit_X_global(…)` pairs** collapsed to one function each, rewritten across **50 call sites in 21 files**, plus 30 handle-passing argument lines removed. The earlier count included `events.rs`'s own definitions.`
The correction was applied to the task and never propagated to the risk row — `L-C1` ("a derived artifact was not regenerated after its source changed") inside the document that keeps the L-C1 tally.
**Confidence:** certain (verified)

## S-17 · §14.5's provider-retry row repeats the closed GAP-12 and assigns it the wrong phase · minor
**Where:** §14.5:
> `| **Provider retry** *(GAP-12 — `ErrorClass::Retryable` has no reader today)* | A retryable provider error | … | P2 |`
**What it says:** `ErrorClass::Retryable` has no reader today, and the retry policy is a P2 item.
**What is actually true:** it has a reader, and it landed in **P3**, not P2. `autodev-core/src/provider/retry.rs:1`:
> `// The retry policy \`ErrorClass::Retryable\` never had (P3-09, GAP-12).`
`retry.rs:157-169` exercise `decide(ErrorClass::Retryable, n)`; `usage.rs:230` `matches!(self, ErrorClass::Retryable | ErrorClass::Overloaded)`. See also S-10 (GAP-12 row) and §15's `R-13`, which repeats the same "had no reader" phrasing.
**Confidence:** certain (verified)

## S-18 · §14 asserts the end-to-end suite runs on all three platforms; §6 says it has never run an assertion and macOS never can · major
**Where:** §14 Testing strategy table:
> `| End-to-end | WebDriver against the built app | PR, all three platforms |`
**What it says:** e2e runs on PRs across Windows, Linux and macOS.
**What is actually true:** `P0-21` in §6 of this same document is the only unticked P0 task and says:
> `⚠ **PARTIAL.** … It has **never executed an assertion**: `msedgedriver` is not on PATH on the Windows dev machine, which is the documented prerequisite, and `tauri-driver` has no WKWebView backend so macOS can never run it at all. `CODE-PRESENT`, not `SMOKE-PROVEN`.`
and a **Linux** CI job only. §5.6's remainder table also lists `P0-21` as *blocked*. §14 is the one place a reader looks for "what runs where", and it is the one place that says something impossible.
**Confidence:** certain (verified)

## S-19 · Thirteen P4 tasks (P4-32…P4-44) are filed inside §10.5.6, the PD6 brand-guideline section · major
**Where:** the heading
> `### P4-F · Assignment actually uses affinity *(from the pre-P8 fleet audit, `_verify/A15`)*`
sits between PD6's `**Smoke — SD6**` block (`SD6-01`, `SD6-02`) and `### PD6-B · The failure this phase exists to avoid *(AAE post-mortem)*`, i.e. **inside** `## §10.5.6 · PD6 · Brand guideline`.
**What it says:** structurally, that `P4-32`…`P4-44` belong to PD6. §10.5.6's own heading level (`##`) makes `P4-F` (`###`) its child.
**What is actually true:** they are P4 (Fleet) tasks; §10 is the P4 chapter and `P4-A`…`P4-E` live there at lines under `# §10 · Phase P4 — The Fleet`. Any per-phase count derived by walking headings will attribute 13 fleet tasks to the brand-guideline phase, and `PD6-10` (which follows under `PD6-B`) is separated from its own phase body by them.
**Confidence:** certain (verified)

## S-20 · "degraded" survives as a live outcome name in three open tasks, after PD0-04 retired it · major
**Where:** three places outside strike-through and outside historical quotation:
1. §10.5.0 `PD0-07` — a **razi-ruled note written 2026-09-03**, in an `[x]` task:
> `So: adapt stays deterministic (PD4-02), the **PA** chooses parameters and adjudicates a `degraded` result, and nothing in Rollout 1 depends on a working fleet.`
2. §10.5.2 `PD2-14`:
> `- [ ] **PD2-14** **Fidelity travels with the result.** Every ingest and every export returns its score; a caller can refuse a degraded conversion.`
3. §11.3 `P7-08`:
> `- [ ] **P7-08** A degraded result (`PD0-04`) is distinguishable from a failure in the UI. → `SMOKE-PROVEN``
**What it says:** `degraded` is an outcome a caller/PA/UI can receive and act on. `P7-08` additionally **cites `PD0-04`** as the definition.
**What is actually true:** `PD0-04` in this same document explicitly retired it:
> `⚠ **Not `refused`/`failed`/`degraded`** — a "succeeded with loss" result cannot cross AutoDesign's seam at all, since `lifecycle.ts:80-85` rejects any result carrying both output and an error. … `complete` · `partial` · `held`, with all three invariants enforced by the decoder rather than documented.`
So `P7-08` points at a section that says the opposite of what `P7-08` assumes, and `PD0-07`'s ruling text names an outcome the contract cannot emit.
⚠ **Explicitly NOT flagged:** `P3-32` and `S3-05` also use "degraded" (*"recall … reports itself degraded"*), but that is a different sense — spec `03 §4.4`'s *"degradation, not failure"* for the memory backend, not the contract outcome triple. Those read as correct.
**Confidence:** certain (verified) for P7-08 and PD2-14; likely for PD0-07 (a ruling paraphrase, so it may be intentional wording that still needs re-pointing)

## S-21 · §11.1's "measured, not estimated" P8 footprint no longer matches the tree · major
**Where:** §11.1, under the preamble `its numbers below are **measured, not estimated** (`wc -l`, `grep -c` against the tree on 2026-09-01)`:
> `| `src-tauri/src/sidecar/` | `client.rs` 1,604 · `mod.rs` 1,270 · `sse.rs` 201 · `startup_progress.rs` 222 |`
> `| **Total Rust** | **3,867 LOC** |`
> `| Files referencing "sidecar" | **14** |`
**What is actually true (2026-09-03):** `wc -l src-tauri/src/sidecar/*.rs`:
```
  1605 src-tauri/src/sidecar/client.rs
  1271 src-tauri/src/sidecar/mod.rs
   201 src-tauri/src/sidecar/sse.rs
   223 src-tauri/src/sidecar/startup_progress.rs
  3300 total
```
so three of the four per-file figures are off by one and the total Rust is **3,870** (3,300 + `sidecar_commands.rs` 570), not 3,867. `sidecar_commands.rs` at 570 LOC and **17** `#[tauri::command]` are both still exact — verified.
"Files referencing 'sidecar' = 14" is far low: `grep -rli "sidecar" --include=*.rs --include=*.ts --include=*.tsx src-tauri/src src autodev-core/src src-cli/src | wc -l` → **69**. (If the original count was scoped to something narrower, the table does not say so — and P8's exit condition is "by content **and** by address", which is exactly what that number is supposed to bound.)
`.gitmodules` **2 stanzas** — verified correct (`core-orchestrator`, `memory-mcp-server`).
**Confidence:** certain (verified)

## S-22 · §11.1 claims `src-tauri/resources/sidecars/` holds 225 MB; P0 already emptied it · major
**Where:** §11.1's measured-footprint table:
> `| `src-tauri/resources/sidecars/` | **225 MB** (`core-orchestrator`, `memory-mcp-server`) |`
and §1.1's `| Sidecar | `submodules/core-orchestrator` | 225 MB bundled | **DELETE (P8)** |`
**What it says:** a 225 MB bundle resource ships inside `src-tauri/resources/sidecars/`.
**What is actually true:** `src-tauri/resources/` contains exactly one entry, the directory `sidecars`, and it is empty. `READINESS_MATRIX.md`'s own 2026-09-01 rev-1.4 update log records this:
> `two were found to be wrong rather than stale: SO-3 described a bundle resource P0 had already emptied`
The 225 MB that does exist is under `submodules/`, which is a different path and is the one §1.1a and §0.8 warn about. §11.1's table conflates the two.
**Confidence:** certain (verified) that `src-tauri/resources/sidecars/` is empty; the 225 MB figure for `submodules/` is separately plausible

## S-23 · PD0-04's task line still states the retired `refused`/`failed`/`degraded` taxonomy as the deliverable, unstruck · major
**Where:** §10.5.0:
> `- [x] **PD0-04** An **error taxonomy** distinguishing *refused* (bad input), *failed* (we broke), and *degraded* (succeeded with loss — an unresolvable font, a rasterized blend mode). Degraded is the one that matters: it is the case the old converter reported as success.`
**What it says:** the shipped taxonomy is refused / failed / degraded, and `degraded` is the important member.
**What is actually true:** the shipped taxonomy is `complete` · `partial` · `held`, per the same bullet's own parenthetical and its ⚠ amendment. Compare `PD0-06`, where the superseded wording *was* struck: `~~A **live stub** answers every operation over a transport~~ → **an executable contract**`. `PD0-04` got a correction note but the head line was never struck, so a reader scanning task lines reads the retired taxonomy as the deliverable — and `P7-08` downstream cites `PD0-04` for exactly that (see S-20).
**Confidence:** certain (verified)

## S-24 · PD0-06's completion note contains a pre-remediation passage that contradicts the sentence immediately before it · major
**Where:** §10.5.0, inside `PD0-06`, consecutive sentences:
> `That is a smoke driving the real binary, so the class is now earned rather than claimed. **A class is upgraded when the proof lands, never before it.** The register defines `SMOKE-PROVEN` as *"the end-to-end smoke driving the real app"*, and `runConformance` has four call sites, all inside vitest. The suite is not `reference.ts`'s own unit test, which is what the original note leaned on — but it is still started by one, and that is a weaker thing than the class claims.`
**What it says:** the class is earned — and then, in the next two sentences, that `runConformance`'s four call sites are all inside vitest and the evidence is *"a weaker thing than the class claims"*.
**What is actually true:** the paragraph above it already records the remediation (`scripts/smoke-design-sidecar.mjs` spawns the reference implementation as a child process). The trailing sentences are the **pre-remediation** argument, left in place after the correction was written above them — the same duplicated-stale-block failure the PD0 work audit found in the public brief. `scripts/smoke-design-sidecar.mjs` exists in the tree.
**Confidence:** certain (verified)

## S-25 · SD0-01's evidence note says nine cases; the same section's Final state and PD0-06 both say fourteen · minor
**Where:** §10.5.0:
> `*(**`SMOKE-PROVEN` 2026-09-03** — the sidecar spawned as a real child process, nine cases. All four operations settle with exit `0` … All five malformed inputs …)*`
versus, ~15 lines later:
> `**Final state:** 10 gates (`TGD0-01…10`, 6 falsified) · 118 JS tests · 611 Rust tests · `SD0-01` re-runnable as `npm run smoke:design`, **14 spawned cases** …`
and `PD0-06`: `which **spawns** the reference implementation as a child process across 14 cases — four operations, four holds, five malformed requests and a killed child`.
**What is actually true:** 14. `scripts/smoke-design-sidecar.mjs:7` says so in its own header comment: `// (\`W-2\`) found that eight of the nine cases could not be reproduced from this`, i.e. the nine-case version is the superseded one. `READINESS_MATRIX.md:206` also records `14 spawn cases`. SD0-01's note (4 operations + 5 malformed = 9) is the pre-remediation description; the four holds and the killed child are missing from it.
**Confidence:** certain (verified)

## S-26 · §10.5.0 says PD0 lacked a matrix row "until 2026-09-03"; it still has none · major
**Where:** §10.5.0, after the exit line:
> `⚠ **PD0 had no exit line, no gate section and no matrix row** until 2026-09-03 — none of the design phases did …`
**What it says:** the missing matrix row was supplied on 2026-09-03.
**What is actually true:** `READINESS_MATRIX.md` §2's scored areas are still `A0`…`A9` mapped to `P0`…`P9` with **no PD row**:
> `| A8 | **P8 · Submodule deletion** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |`
> `| A9 | **P9 · Delivery** | 4 | ☐ | ☐ | ☐ | 0% | 0.0 |`
and that document's own headline says so: `**PD0 closed on its second attempt** … and scored nowhere. ⚠ The design phases have no scored area`. Only the **exit line** and the **gate section** (`TESTING_GATES.md §6.5`) were supplied. The MDP sentence bundles all three as fixed.
**Confidence:** certain (verified)

## S-27 · PD0-10's task line says the Novelty Ledger holds twenty rows and none for design; PD0-10 is done and it now holds twenty-one, one of them for design · minor
**Where:** §10.5.0:
> `- [x] **PD0-10** **Open the `NOVELTY_LEDGER` row for the contract.** … the register holds **twenty** rows and **none for the design programme**`
**What is actually true:** `NOVELTY_LEDGER.md` holds **21** rows, `N-01`…`N-21`, and `N-21` is the design-contract row this very task opened (`READINESS_MATRIX.md:207` names it: `we inherit their conventions rather than their types (ADR-LOG-RESTRUCT-0034, N-21)`). Command: `grep -oE "\bN-[0-9]+\b" NOVELTY_LEDGER.md | sort -uV` → `N-01 … N-21`.
**Confidence:** certain (verified). Note: this is a task *premise* left in present tense on a completed task — the same shape as `PD0-04` (S-23), and unlike `PD0-06`, which struck its superseded premise.

## S-28 · §10.5.0's exit line names a gate family that does not exist · minor
**Where:** `**Exit.** `SD0` → `TG-PD0` → matrix → progress → no-stale → commit.`
**What is actually true:** PD0's gates are `TGD0-01`…`TGD0-10` (per the same section's Final state line and `TESTING_GATES.md §6.5`). There is no `TG-PD0` family.
**Confidence:** certain (verified)

## S-22 · ⛔ RETRACTED — `src-tauri/resources/sidecars/` is NOT empty
**Retraction, recorded in place rather than deleted (§0.16 "record overturns in place").** My first `ls -la src-tauri/resources/` showed only the top-level entry and I read that as an empty tree. A recursive measurement disproves it:
```
find src-tauri/resources -type f | wc -l   -> 2665
du -sh src-tauri/resources/sidecars        -> 225M
```
So §11.1's `| src-tauri/resources/sidecars/ | **225 MB** | ` is **correct**. The mechanism of my error is exactly `§0.2L-3`: I sampled the directory instead of measuring it.
⚠ **But the measurement produced a different, real finding — see S-22b.**

## S-22b · §0.8 and §1.1a understate the untracked-tree hazard by ~3.5× · major
**Where:** §0.8's known-destructive list:
> `- **`git add -A` / `git add .` would commit 225 MB of untracked Python.** `submodules/` is untracked **and not gitignored** (§1.1a), so a blind stage pulls in the entire sidecar tree.`
and §1.1a:
> `⚠️ **Live hazard, today:** 225 MB sits untracked and **not ignored**, so `git add -A` would commit the entire Python tree.`
and §1.1: `| Sidecar | `submodules/core-orchestrator` | 225 MB bundled | **DELETE (P8)** |`
**What it says:** the untracked `submodules/` tree is 225 MB.
**What is actually true:**
```
du -sh submodules            -> 795M
du -sh submodules/*          -> 507M  submodules/core-orchestrator
                                289M  submodules/memory-mcp-server
```
The 225 MB figure belongs to a **different path** — `src-tauri/resources/sidecars/`, the bundle resource (verified 225M, 2,665 files). §0.8 and §1.1a apply it to `submodules/`, which is **795 MB**. §1.1 additionally attributes the whole 225 MB to `submodules/core-orchestrator` alone.
This matters because §0.8 is, by its own words, *"the section every session and every subagent dispatch must read"*, and the number is the argument for the rule.
**Confidence:** certain (verified)

## S-21b · §11.1's "Files referencing 'sidecar' = 14" — measured breakdown · (supports S-21)
`grep -rli "sidecar" --include=*.rs src-tauri/src | wc -l` → **17**
`grep -rli "sidecar" --include=*.ts --include=*.tsx src | wc -l` → **33**
whole-tree (src-tauri/src, src, autodev-core/src, src-cli/src) → **69**
No scoping of the original 14 reproduces it.
**Confidence:** certain (verified)

## S-29 · §0.8 and §1.1a both say `submodules/` is not gitignored; it is · blocker
**Where:** §1.1a's verified-state table:
> `| Gitignored | **No** |`
§1.1a's hazard note:
> `⚠️ **Live hazard, today:** 225 MB sits untracked and **not ignored**, so `git add -A` would commit the entire Python tree. §0.9's stage-deliberately rule is what prevents it; it is now also named in §0.8.`
§0.8's known-destructive list:
> `- **`git add -A` / `git add .` would commit 225 MB of untracked Python.** `submodules/` is untracked **and not gitignored** (§1.1a), so a blind stage pulls in the entire sidecar tree.`
**What it says:** `submodules/` is not ignored, so `git add -A` would stage it.
**What is actually true:** it is ignored. `git check-ignore -v submodules/core-orchestrator` →
> `.gitignore:110:submodules/	submodules/core-orchestrator`
and `.gitignore:104-110` carries a purpose-written comment:
> `# The retired Python sidecar / submodule, kept on disk as the reference source`
> `# for capabilities still marked PORT. It is a live git repo with its own history`
> `# and its own uncommitted changes, and its removal is P8's job, gated by TG8-02`
> `# ("the deletion phase cannot complete while any PORT capability is unported").`
> `# Ignored here so 28k untracked files cannot be swept into a commit meanwhile.`
> `submodules/`
`git status --porcelain` does not list `submodules/` as untracked.
⚠ This is the highest-consequence finding in this sweep: §1.1a's table is presented as *"verified rather than assumed (§0.0)"*, and it is the input to P8's deletion procedure. Every other row in that table still checks out (see "What I checked and found correct"), so the one wrong row reads as trustworthy.
**Confidence:** certain (verified)

## S-30 · §0.8's graph-v5 migration-repair warning describes a repair completed in P1-11 · major
**Where:** §0.8's known-destructive list, first entry:
> `- **Migration repair of `graph/` v5.** The head migration uses `ADD COLUMN IF NOT EXISTS`, which is not valid SQLite (`G3d F-31`). Repairing it touches a live per-repo graph DB. **Back up `~/.autodev/graphs/*.db` first.**`
**What it says:** the graph store is still at the broken v5 head and its repair is a pending destructive operation.
**What is actually true:** `P1-11` is `[x]` and `LIVE-DB-PROVEN` in §7: *"Live: `5 → 6`, `integrity_check = ok`"*. §1.1a's own "Live schema head" note says *"the graph store is at **v6**"*. `autodev-core/src/graph/migrations.rs:129-132` records it in the past tense:
> `// v5 originally issued six \`ALTER TABLE … ADD COLUMN IF NOT EXISTS\``
> `// \`ADD COLUMN IF NOT EXISTS\` is not valid SQLite, so all six failed — and`
§0.8's own closing rule is *"Removing an entry requires proof it is fixed"* — the proof exists in §7 and the entry was never removed or annotated. Compare the entry two lines below it, which **was** struck and annotated (`~~The `~/.airs/` → `~/.autodev/` consolidation~~ → **DONE in `P1-14`**`).
**Confidence:** certain (verified)

## S-31 · §1.1 calls the 225 MB sidecar tree "bundled"; `bundle.resources` has been empty since P0 · major
**Where:** §1.1:
> `| Sidecar | `submodules/core-orchestrator` | 225 MB bundled | **DELETE (P8)** |`
**What it says:** 225 MB ships inside the application bundle, and it lives at `submodules/core-orchestrator`.
**What is actually true:** two separate errors.
1. Nothing is bundled: `src-tauri/tauri.conf.json:46` is `"resources": [],`. `READINESS_MATRIX.md:133` (`SO-3`) records the fix: *"The 225 MB Python tree is **no longer a declared bundle resource** — `bundle.resources` is `[]`, fixed in P0, and a gate asserts every declared bundle path exists **and is git-tracked**."*
2. The 225 MB is at `src-tauri/resources/sidecars/` (measured: 2,665 files, 225M). `submodules/core-orchestrator` is **507 MB** (see S-22b).
**Confidence:** certain (verified)

## S-32 · §0.16's recurring-class counts do not match the instances they enumerate · major
**Where:** §0.16 "Recurring classes — running count", whose own preamble states the rule:
> `**A defect seen once is an accident. The same shape three times is a property of how I work.** Every new instance increments a row, or opens a new one and says why it is new.`
**What it says:** the Count column equals the number of instances in the Instances column.
**What is actually true** (segments counted by splitting each Instances cell on ` · `, then expanding the cells that bundle several — "three more in P2", "five more in P2", "×3"):

| Row | Claimed count | Instances actually enumerated |
|---|---|---|
| `L-C1` | **6** | 4 (Part 1B · duplicate ADR-02-06 · specs' gate tables · spec `10 §6`) |
| `L-C6` | **13** | 9 segments (10 if the `PD0` fonts entry is read as two assertions) |
| `L-C9` | **13** | 8 segments; 11 after expanding *"three more in P2"* and the P1 pair |
| `L-C12` | **10** | 7 segments (2 + the five enumerated P2 instances) |

`L-C2`, `L-C3`, `L-C4`, `L-C5`, `L-C7`, `L-C8`, `L-C10`, `L-C11`, `L-C13`, `L-C14`, `L-C15` all match exactly — so the four that do not are drift, not a different counting convention.
**Confidence:** likely — the mismatch is certain; whether the fix is "the count is wrong" or "instances were never written down" needs a decision

## S-33 · L-27 and PD0-06 quote recurring-class counts that contradict the table · minor
**Where:** §0.16, inside `L-27`:
> `Related: `L-C6` (count now **9**), and `§0.2i`'s existing rule …`
and inside `PD0-06`'s note (§10.5.0):
> ``L-C12` is the failure mode here and it has **eight** instances in this register`
**What is actually true:** the table says `L-C6` = **13** and `L-C12` = **10**. Three other inline claims exist and disagree with each other and with the table: `P1-21`'s note says `` `L-C6` → 5 ``, `P3-F`'s note and §16's change log both say `` `L-C6` → 11 ``. Four different live values for one counter (5, 9, 11, 13) in one document.
Commands: `grep -n "count now" MASTER_DEVELOPMENT_PLAN.md`; `grep -on "L-C6\` → [0-9]*" MASTER_DEVELOPMENT_PLAN.md`.
**Confidence:** certain (verified)

## S-34 · §3's architecture diagram and §1.3 both say four providers; the set has been three since 2026-09-01 · major
**Where:** §3's ASCII diagram:
> `        4 providers    ToolRegistry   Memory     Fleet         Code graph`
> `        (HTTP)         (in-process)   (SQLite+   (events)      (SQLite)`
and §1.3:
> `| A vendor agent SDK wrapper | No first-party Rust SDK exists for any of **the four**; all 14 cost-relevant features are plain HTTP (ADR-01-01, ADR-01-04) |`
**What it says:** AutoDev has four HTTP providers.
**What is actually true:** three. `ls autodev-core/src/provider/` → `anthropic.rs`, `openai.rs`, `google.rs` and no fourth adapter. §1.2's `GAP-02` in this same document says so: *"**CLOSED-AS-WONTFIX 2026-09-01** (razi-ruled): the provider set is Anthropic, OpenAI, Google"*, and §5.5 says *"**Three providers** conform"*. §4's module tree is also already correct: `provider/{traits,anthropic,openai,google,streaming}.rs`.
**Confidence:** certain (verified)

## S-35 · §5.6's own prose says the remainder is 319; its heading and its table say 312 · minor
**Where:** §5.6, heading `### Recalibrated remainder — 312 open tasks`, then:
> `The remainder grew from 223 tasks to **319**, and `P8` grew from 24 tasks to **65** …`
then the table's last row: `| **Programme** | **312** | **≈37.5 h** | **≈52 h** | |`
**What it says:** 319 in the prose, 312 in the heading and the arithmetic.
**What is actually true:** **312**. The table's columns add up exactly: Rollout 1 = 1 + 65 + 21 + 27 + 15 + 33 + 13 + 10 + 44 = **229**; Rollout 2 = 21 + 28 + 18 + 16 = **83**; total **312** — and every one of those matches the checkbox-derived ground truth. The prose figure 319 is the outlier (it appears to be 312 + PD0's 7-before-recut, or a pre-`PD0`-closure number). §5.6 is the section that exists *because* "the estimates lived nowhere near the plan", so a stale number inside it is the failure it was written to prevent.
**Confidence:** certain (verified)

## S-36 · §5.6 claims all measured throughput is Rust and documentation and that frontend throughput is zero; PD0 was ten completed TypeScript tasks · major
**Where:** §5.6, "Where this model should NOT be trusted":
> `1. **All 183 measured tasks are Rust and documentation.** `P6` and `P7` are 46 tasks of frontend work with **zero** measured throughput behind them.`
and §5.6's measured table, whose rows stop at `P3-F` and total `| **Total** | **183** | **≈49h calendar** | **21h 23m** |`.
**What it says:** every completed task so far is Rust or prose, so there is no frontend data point.
**What is actually true:** `PD0` is complete (10 tasks, `[x]` throughout §10.5.0; §5.6's own remainder table marks it `**done**`) and its implementation is **entirely TypeScript**. `git show --stat 1cb8b9a`:
> `src/design/contract/conformance.ts | 243 ++`, `contract.test.ts | 259 ++`, `decode.ts | 328 ++`, `gates.test.ts | 207 ++`, `index.ts | 76 ++`, `main.ts | 54 ++`, `reference.ts | 190 ++`, `sidecar.ts | 117 ++`, `types.ts | 305 ++`, `version.ts | 63 ++`, `tsconfig.sidecar.json | 35 ++`
Ten TS modules, a Node build target and a vitest suite — 2,269 insertions. `PD0` also does not appear in §5.6's measured table at all, so the 183 / 8.6-tasks-per-hour headline excludes a completed phase.
**Confidence:** certain (verified)

## S-37 · §0.17 attaches the public-facing refresh step to §0.5; the per-phase closeout order is §0.6 · minor
**Where:** §0.17:
> `**Standing rule, razi-ruled 2026-09-01** (*"publish and keep it updated each phase"*): the per-phase closeout in `§0.5` gains one step.`
**What is actually true:** §0.5 is *"Smoke-then-gates is mandatory"*. The per-phase closeout order is **§0.6**:
> `**§0.6 — Per-phase workflow, in this fixed order.** `build → smoke S<p> → gates TG-P<p> → readiness matrix → progress row → no-stale sweep → commit → continue into the next phase.``
and §0.6's list does **not** contain the public-facing step §0.17 says it gains — so following §0.6 literally (which `L-35` says is now the practice: *"§0.6's order is now something I read rather than recall"*) skips §0.17 entirely.
**Confidence:** certain (verified)

## S-38 · §5.6's "Recommended order: PD0 first, then P8" is advice about a completed phase · minor
**Where:** §5.6:
> `**Recommended order: `PD0` first, then `P8`.** One hour unblocks a partner team who are otherwise waiting on a document, and nothing in `PD0` depends on the sidecar being gone.`
and, above it: `⚠ **`PD0` is not blocked by `P8`.** … The public brief's phase table nonetheless listed it **last, after `P4`** … That was a presentation defect and is corrected.`
**What it says:** a forward-looking sequencing recommendation, and a live complaint about the public brief's ordering.
**What is actually true:** PD0 closed on 2026-09-03 (`git log`: `1cb8b9a`, `1311e65 docs(brief): PD0 closed, M1 reached`, `2ee97b1`, `9bc7895`). Both the recommendation and the "one hour" estimate are spent. Whether the brief's ordering complaint still applies needs a read of the current brief, which the closeout commits touched.
**Confidence:** certain (verified) that PD0 is closed; needs a decision on how to restate

## S-39 · §6 asserts seven P0 tasks that have no task line anywhere in the MDP, and P0-26 cross-references one of them · blocker
**Where:** §6's preamble:
> `Three refutation verifiers re-checked all 27 claims: **21 confirmed as written, 5 understated or too narrow, 1 reversed** — `P0-26` would have deleted the config that is the only evidence of a live runtime break in the Logs export. Six tasks were added (`P0-28`…`P0-33`) and one (`P0-13b`) comes from a Razi ruling.`
> `Where a line below disagrees with that file, **that file wins** until this section is regenerated at P0 closeout.`
and §6's `P0-26` line:
> `- [ ] ~~**P0-26** Remove the dead `dialog`/`fs` plugin blocks from `tauri.conf.json`.~~ → **REVERSED, superseded by `P0-31`.**`
and §16's change log rev 1.4:
> `26 of 28 original tasks done, plus `P0-13b` and six added tasks (`P0-28`…`P0-33`).`
**What it says:** `P0-13b` and `P0-28`…`P0-33` are P0 tasks, all complete, and `P0-31` supersedes `P0-26`.
**What is actually true:** none of those seven ids has a task line in the MDP. Every `P0-*` id that appears in the document:
```
grep -oE "\*\*P0-[0-9]+[a-z]?\*\*" MASTER_DEVELOPMENT_PLAN.md | sort -uV
P0-01 … P0-13a … P0-27      (28 ids; no P0-13b, no P0-28…P0-33)
```
They exist only in `_verify/P0-CORRECTIONS.md`:
> `:124  - [x] **P0-28** `download_checksum` / `download_signature` never check HTTP status`
> `:135  - [x] **P0-31** Register `tauri-plugin-dialog` and `tauri-plugin-fs` and grant their`
> `:170  All six added tasks (`P0-28`…`P0-33`) are complete, as is `P0-13b` …`
So §6 **was never regenerated at P0 closeout**, contrary to its own preamble, and `P0-26`'s "superseded by `P0-31`" is a dangling reference within this document. This also means §6's checkbox set cannot be the source of any P0 task total — seven completed tasks are invisible to it.
**Confidence:** certain (verified)

## S-40 · §2's dependency citations have drifted, and one names a CI line with nothing on it · minor
**Where:** §2's ```rust block:
> `rusqlite = { version = "0.31", features = ["bundled"] }   // src-tauri/Cargo.toml:41`
> `keyring = "2"                        // src-tauri/Cargo.toml:44`
> `// Linux needs D-Bus Secret Service; ABSENT in CI (ci.yml:64) → encrypted-file fallback (G-06)`
**What is actually true:**
- `rusqlite` is at `src-tauri/Cargo.toml:43`, not `:41`.
- `keyring` is at `src-tauri/Cargo.toml:46`, not `:44`.
- `ci.yml:64` is `          workspaces: . -> target` — a `Swatinem/rust-cache` input. `grep -in "dbus\|d-bus\|secret-service\|gnome-keyring" .github/workflows/ci.yml` returns **nothing**, so no line in that file evidences the D-Bus claim. (The claim that D-Bus is absent may still be true — absence is why there is nothing to cite — but the citation is dead and §0.2L-1 requires `path:line` to resolve.)
- ⚠ Separately: after `P1-05` both dependencies are also declared in `autodev-core/Cargo.toml` (`:42` rusqlite, `:73` keyring), which is now the crate that owns persistence and the vault. §2 still points only at the desktop shell.
**Confidence:** certain (verified)

## S-41 · P8-26 attributes `platform/command.rs` to P1; P0-23 built it · minor
**Where:** §11.1 `P8-26`:
> `**razi-ruled:** stop rewriting; route through `platform/command.rs` (built in `P1`, still zero callers — this is what `P4-11` exists for)`
**What is actually true:** it is a **P0** deliverable. §6: `- [x] **P0-23** Create `autodev-core/platform/command.rs` — intent (`Build`/`Test`/`Lint`/`Run`) → platform-correct **argv** …`, and §4's own tree tags it `platform/command.rs               [P0]`. `READINESS_MATRIX.md`'s P0 closeout entry says the same: *"**SO-6 stays RED deliberately** — `platform/command.rs` is proven but has no caller"*.
The "still zero callers" half is **correct** — verified: the only reference outside the module is `src-tauri/src/platform.rs:6: pub use autodev_core::platform::*;`, a re-export, not a call.
**Confidence:** certain (verified)

## S-42 · §5.6 claims the MDP's task counts are its own checkbox counts while criticising the brief for recording P0 at 12 · minor
**Where:** §5.6:
> `Task counts are the **MDP's own checkbox counts**, not the brief's table, which was stale for three completed phases (it still recorded P0 at 12 tasks; P0 grew to 27 during its own pre-build refutation).`
**What it says:** the MDP's task counts come from its own checkboxes, and "P0 at 12" is the brief's error.
**What is actually true:** §5.1 of this same document also records `| ✅ | **P0** Stop the bleeding | done | 12 |`. The sentence's own example of a stale figure is live two sections above it.
⚠ **Flagged only for completeness — §5.1's counts are already being corrected by the main session and are explicitly out of this audit's scope.** What is in scope is that §5.6 makes a *claim about the MDP* ("task counts are the MDP's own checkbox counts") that the MDP does not satisfy.
**Confidence:** certain (verified)

## S-43 · §13's `#[deny(clippy::todo, clippy::unimplemented)]` convention is not present in `autodev-core` · minor
**Where:** §13 Coding conventions, final clause:
> `One shared path-normalisation helper. Timestamps through one helper. `#[deny(clippy::todo, clippy::unimplemented)]` in `autodev-core`.`
**What is actually true:** it is nowhere in the crate. `grep -rn "deny(" autodev-core/src/lib.rs autodev-core/Cargo.toml` returns nothing, there is no `[lints]` table in `autodev-core/Cargo.toml`, and `head -12 autodev-core/src/lib.rs` shows only doc comments before the first `use`. The two clauses beside it *are* satisfied (`db/timestamps.rs` per `P1-13`; the path helper per §13's own rule).
**Confidence:** certain (verified) that the attribute is absent; needs a decision on whether §13 is read as prescriptive (an unmet convention) or descriptive (a false statement)

## S-44 · §12's event taxonomy names do not exist in the emitted event set · minor
**Where:** §12:
> `One taxonomy, emitted by `autodev-core`, consumed by the UI and the CLI:`
> ``TurnStarted` · `TextDelta` · `ThinkingDelta` · `ToolCallStarted` · `ToolCallFinished` · `PermissionRequested` · `TurnEnded` · `Error``
**What it says, in the present tense:** these eight names are the emitted taxonomy.
**What is actually true:** `autodev-core/src/events.rs:93-99` declares the shipped agent names, and none of the eight match:
> `pub const EVT_AGENT_TOKEN: &str = "agent:token";`
> `pub const EVT_AGENT_THINKING: &str = "agent:thinking";`
> `pub const EVT_AGENT_TOOL_START: &str = "agent:tool-start";`
> `pub const EVT_AGENT_TOOL_END: &str = "agent:tool-end";`
> `pub const EVT_AGENT_PERMISSION: &str = "agent:permission-request";`
> `pub const EVT_AGENT_DONE: &str = "agent:done";`
> `pub const EVT_AGENT_ERROR: &str = "agent:error";`
There is no `TurnStarted` counterpart at all, and `TurnEnded` — the name §12's next sentence and `P3-05` both hang on — is `agent:done` / `agent:error` in the tree. §12's second sentence names P6 as the phase that enforces emitter/consumer pairing, so the *design* is legitimately forward-looking; the **present-tense "emitted by `autodev-core`"** is what is false.
**Confidence:** certain (verified) that the names differ; needs a decision on whether §12 is design-target or current-state

---

# Summary

| ID | Severity | One line |
|---|---|---|
| **S-1** | blocker | Header still says "AWAITING GO. No code, no migrations, no deletions." after seven phases |
| **S-2** | major | §0.1 states the GO gate as still closed |
| **S-3** | major | §0.2e says the plan holds 154 tasks; it holds 506 |
| **S-4** | major | §0.2j lists four "open" web-answerable items, three in shipped phases, one contradicted by GAP-02 |
| **S-5** | major | §1.1 "What exists" carries pre-restructure LOC/module/store counts for every row and has no autodev-core row |
| **S-6** | major | Tauri command count is 256 in §1.1 and 255 in §4; the tree has 258 |
| **S-7** | major | §4's module tree still lists mcp_server/ as present and awaiting P0 deletion; the directory is gone |
| **S-8** | minor | Sidecar LOC is 3,161 in §4 and 3,867 in §5.3; the tree says 3,300 |
| **S-9** | major | §2 asserts panic = "abort" at Cargo.toml:126-131; it is panic = "unwind" at the workspace root and never took effect where §2 cites it |
| **S-10** | blocker | §1.2 lists eight gaps (03/04/05/06/07/12/14/17) that later phases closed, with no annotation |
| **S-11** | minor | GAP-01's evidence names rmcp, which P0-16 removed |
| **S-12** | major | Front-matter Rev 1.0 vs change log's own newest Rev 2.1 |
| **S-13** | minor | §16 breaks its stated newest-first order, one row has no rev, and revs 1.5-1.8 (P1, P2, P3 A-E closeouts) have no rows |
| **S-14** | major | §15 leaves R-2/R-3/R-4/R-7 unannotated though their owning phases closed |
| **S-15** | major | R-11's mitigation "pin and vendor (P3-33)" — P3-33's own note says vendoring is not done and is Razi's open call |
| **S-16** | major | R-12 repeats "132 emit_* sites", refuted by P1-06 (19 pairs / 50 call sites) |
| **S-17** | minor | §14.5's retry row repeats closed GAP-12 and files it under P2; retry.rs says P3-09 |
| **S-18** | major | §14 says e2e runs on all three platforms; P0-21 says it has never run an assertion and macOS never can |
| **S-19** | major | P4-32...P4-44 are filed inside §10.5.6 (PD6), not under §10 (P4) |
| **S-20** | major | "degraded" survives as a live outcome in PD0-07, PD2-14 and P7-08 — and P7-08 cites PD0-04, which retired it |
| **S-21** | major | §11.1's "measured, not estimated" P8 footprint: three per-file LOC off by one, total 3,867 -> 3,870, "14 files referencing sidecar" -> 69 |
| **S-21b** | — | supporting measurement for S-21 |
| **S-22** | RETRACTED | src-tauri/resources/sidecars/ is not empty; §11.1's 225 MB is correct |
| **S-22b** | major | §0.8/§1.1a/§1.1 apply the 225 MB bundle figure to submodules/, which is 795 MB |
| **S-23** | major | PD0-04's task line still states refused/failed/degraded as the deliverable, unstruck |
| **S-24** | major | PD0-06's note ends with the pre-remediation argument that contradicts the sentence before it |
| **S-25** | minor | SD0-01 says nine smoke cases; Final state and PD0-06 say fourteen |
| **S-26** | major | §10.5.0 says PD0's matrix row was supplied on 2026-09-03; READINESS_MATRIX.md still has none |
| **S-27** | minor | PD0-10 says the Novelty Ledger holds twenty rows and none for design; it holds 21, incl. N-21 |
| **S-28** | minor | §10.5.0's exit line names gate family TG-PD0; the gates are TGD0-* |
| **S-29** | blocker | §0.8 and §1.1a say submodules/ is not gitignored; .gitignore:110 ignores it |
| **S-30** | major | §0.8's graph-v5 migration-repair warning describes a repair completed in P1-11 (store is at v6) |
| **S-31** | major | §1.1 calls the sidecar tree "225 MB bundled"; tauri.conf.json:46 is "resources": [] |
| **S-32** | major | §0.16's recurring-class counts (L-C1 6, L-C6 13, L-C9 13, L-C12 10) exceed the instances they enumerate |
| **S-33** | minor | Four live values for the L-C6 counter (5, 9, 11, 13) and two for L-C12 (8, 10) in one document |
| **S-34** | major | §3's diagram and §1.3 say four providers; the set has been three since 2026-09-01 |
| **S-35** | minor | §5.6's prose says the remainder is 319; its heading and arithmetic say 312 |
| **S-36** | major | §5.6 says all 183 measured tasks are Rust/docs and frontend throughput is zero; PD0 was 10 completed TypeScript tasks, absent from the measured table |
| **S-37** | minor | §0.17 attaches the public-facing refresh to §0.5; the closeout order is §0.6, and §0.6 does not carry the step |
| **S-38** | minor | §5.6's "Recommended order: PD0 first, then P8" is advice about a phase that closed |
| **S-39** | blocker | §6 asserts P0-13b and P0-28...P0-33 exist and are done; none has a task line in the MDP, and P0-26 cross-references the missing P0-31 |
| **S-40** | minor | §2's Cargo.toml:41/:44 are off by two, and ci.yml:64 contains nothing about D-Bus |
| **S-41** | minor | P8-26 attributes platform/command.rs to P1; P0-23 built it (the "zero callers" half is correct) |
| **S-42** | minor | §5.6 claims task counts are the MDP's own checkbox counts while its example of the brief's staleness is live in §5.1 |
| **S-43** | minor | §13's deny(clippy::todo, clippy::unimplemented) is absent from autodev-core |
| **S-44** | minor | §12's eight event names (TurnStarted...Error) do not match any emitted constant in events.rs |

**Counts:** 4 blocker (S-1, S-10, S-29, S-39) · 22 major · 17 minor · 1 retracted.

**The two to fix first**, on consequence rather than count:
- **S-29** — §0.8 is, by its own words, *"the section every session and every subagent dispatch must read"*, and it justifies a safety rule with a fact that is no longer true (`.gitignore:110` already prevents the hazard it names). A rule whose stated reason is false is the fastest kind to get ignored.
- **S-39** — seven completed P0 tasks have no line in the plan, and one existing line points at a task id the document never defines. Any count, audit or regeneration walking §6's checkboxes walks an incomplete set.

---

# What I checked and found correct

Recorded because a sweep that only reports problems gives no signal about coverage.

**§1.1a — the submodule verified-state table.** Every row except `Gitignored` (S-29) re-verified:
- `git ls-files -s submodules/` -> empty (0 gitlinks in the index) OK
- `git ls-tree HEAD submodules/` -> empty OK
- `git log --all -- submodules/` -> nothing OK
- `.gitmodules` tracked, **2 stanzas**, declaring `submodules/core-orchestrator` and `submodules/memory-mcp-server` OK
- `.git/modules/submodules/` present, holding both object stores OK

**§0.8's other known-destructive entries.** All still true in the tree:
- `git_discard` with an empty file list force-reverts the worktree — `autodev-core/src/git/commit.rs:131-142`: `CheckoutBuilder::new()` + `opts.force()` + a `for f in files` loop that adds no path when `files` is empty, then `repo.checkout_head` OK
- `delete_file` calls `remove_dir_all` despite a docstring saying "empty directory" — `src-tauri/src/commands/ide_commands.rs:135` docstring "Delete the file (or empty directory) at path" and `:141` `std::fs::remove_dir_all(&abs)` OK
- Plugin uninstall runs `rustup self uninstall -y` — `autodev-core/src/plugin/builtins.rs:74` OK
- `ensure_default_recipes` overwrites on every launch — `autodev-core/src/recipes/defaults.rs:45-64` calls `save_recipe` unconditionally for all nine defaults OK
- The `~/.airs` -> `~/.autodev` entry is correctly struck and annotated "DONE in P1-14" OK

**Retired terminology — "live stub".** Exactly **one** occurrence remains in the MDP, legitimately preserved inside strike-through in `PD0-06`. `grep -rn "live stub|live-stub" docs/` shows the only other live-text hits are `ADR_LOG.md:1714` (a historical quotation of the task wording) and the audit reports that found the problem. The six survivors `A17-PD0-work-audit.md` `W-11` listed — including in `AUTODEV-PROGRAMME-BRIEF.md` and `.html` — are **gone**. This one is genuinely fixed.

**Open-task premises about the codebase.** Checked the "zero callers" / path:line class in `P4-F` and `P8-0`, since the brief flagged these as months-old claims. They hold:
- `P4-33`: `select_worker` requires `static_conf > 0.85` at `autodev-core/src/routing/scorer.rs:135` OK; the highest declared pattern confidence is exactly `0.85` at `routing/patterns.rs:27` and `:46` OK — so the `method: "static"` branch is still unreachable
- `P4-42`: `list_stale_worker_messages` has zero callers — only its definition at `autodev-core/src/db/tasks.rs:943` OK
- `P4-39`: `unblock_ready_tasks` has exactly one caller, `src-tauri/src/commands/task_commands.rs:73` OK
- `P4-36`: `WorkersDb::heartbeat` has zero callers **in the app** — its only non-test caller is `src-cli/src/commands/worker.rs:187` (airsctl) OK as worded
- `P8-26`: `rewrite_for_windows` emits `Select-Object -First`/`-Last`/`Select-String` at `bash.rs:533-545` OK, and `bash.rs:116` is `Command::new("cmd")` followed by `cmd.args(["/C", &command])` OK
- `P8-26`'s "platform/command.rs still zero callers" OK — the only reference outside the module is a `pub use` re-export at `src-tauri/src/platform.rs:6`. Only the P1 attribution is wrong (S-41)

**Cross-document claims that resolve.**
- `P1-22`: `TG8-02` `[governance]` is present in `TESTING_GATES.md` under `## 11. P8 — Submodule deletion` OK, and it does read the grounding PORT verdicts as its checklist OK
- `P3-05`: `TG3-01` is still `- [ ]` with `Evidence: _pending_` at `TESTING_GATES.md:221-222` OK
- §0.3's "see the vision Part 10" -> `00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:1335` `# PART 10 · Amendment log` OK
- §10.5's grounding path `C:\AIRS Foundry\apps\psdconversions` exists, and its `index.html` is **exactly 3,202 lines** as claimed OK
- Gate total: `TESTING_GATES.md:8` reads "125 gates across P0-P9 and PD0" and `:54` "Total | 125", matching §16's "115 -> 125" OK — the MDP carries no stale 115 outside its historical change-log rows
- §5.6's remainder arithmetic: R1 229 + R2 83 = **312**, and every per-phase open figure matches the checkbox-derived ground truth (P8 65, PD1 21, PD2 27, PD3 15, PD4 33, PD5 13, PD6 10, P4 44, P5 21, P6 28, P7 18, P9 16) OK — only the prose "319" is wrong (S-35)
- §5.6's measured table sums correctly: 26 + 22 + 4 + 74 + 39 + 18 = **183** OK (the defect is what it omits — S-36)
- §16's PD0 change-log row is accurate in substance: 45 findings, 24 area / 21 work, gates 8 -> 10, `TGD0-09`/`TGD0-10`, `L-35`...`L-38` — all corroborated by `READINESS_MATRIX.md:206` and `TESTING_GATES.md:10` OK
- The public brief is **current**: `AUTODEV-PROGRAMME-BRIEF.md:9` reads "P0-P3 and PD0 (the contract) complete ... P8 next · overall readiness 34% · 312 open tasks", and `:92` lists PD0 at 10 tasks, 10 gates, 14 spawned cases OK — §0.17's rule was honoured at the last closeout
- `_verify/PD0-FINDINGS-LEDGER.py` exists, as §10.5.0's closure note claims OK

**§12's other claim.** *"agent:thinking is emitted on every thinking delta today and has no listener anywhere — reasoning tokens stream into a black hole."* Still true: `events.rs:94` declares it, `events.rs:243` emits it (`emit_serialized(EVT_AGENT_THINKING, &payload)`), `engine.rs:1066` handles `StreamChunk::ThinkingDelta`, and no listener for `agent:thinking` exists anywhere under `src/` OK

**§0.16's lesson numbering.** `L-1`...`L-38` all present, no gaps, no duplicates (extracted with a regex over the lesson headings), matching the stated `L-38` ceiling. `W-3` and `W-4` are the only `W-` lessons OK. The *class* counts are the problem (S-32), not the lesson ids.

**§2's remaining substrate claims.** `tauri = { version = "2.11", ... }` at `src-tauri/Cargo.toml:21` OK; exactly two registered plugins (`tauri-plugin-shell` `:22`, `tauri-plugin-dialog` `:23`) OK; `reqwest 0.12` with `["json","stream"]` `:49` OK; `git2 = "0.19"` `:82`, `tree-sitter = "0.22"` `:99`, `petgraph = "0.6"` `:100`, `notify = "6"` `:105` OK; `sqlite-vec = "0.1.9"` at `autodev-core/Cargo.toml:48` OK.

**§11.1's other P8 figures.** `sidecar_commands.rs` is **570 LOC** with **17** `#[tauri::command]` — both exact OK. `.gitmodules` **2** stanzas OK. `src-tauri/resources/sidecars/` **225 MB** OK (see the S-22 retraction).

---

## Adjacent findings (outside the MDP — reported because §0.3 spans the family)

Not counted in the summary; these are in sibling documents, not `MASTER_DEVELOPMENT_PLAN.md`.

- **`ADR_LOG.md` header is stale by sixteen entries.** `:8` "Rev | 1.0 — 2026-08-31" and `:9` "Entries | **19** — ADR-LOG-RESTRUCT-0001 ... 0019, spanning P0 and P1". The log actually runs to **ADR-LOG-RESTRUCT-0035** and spans P0-P3-F and PD0.
- **`READINESS_MATRIX.md` header Rev is stale.** `:7` "Rev | 1.0 — 2026-08-31" while its own update log's newest row is "2026-09-03 | 2.1".
- **`READINESS_MATRIX.md` §1 "Agent correctness" still says memory is unbuilt** — "Now AMBER because: P3-F · Memory is 15 tasks untouched, so recall is unbuilt". P3-F closed 2026-09-02, and the same document's overall row says so.
- **`NOVELTY_LEDGER.md` header Rev is stale** ("1.0 — 2026-08-31") though `N-21` was added by `PD0-10` on 2026-09-03.

*(All four are the same shape as S-12: a front-matter Rev that no closeout advanced.)*

---

*End of A18. Read-only sweep; no file outside this report was modified.*
