# 00 — AutoDev Restructure: Master Plan

| | |
|---|---|
| **Owner** | Razi (Programming Lead, AIRS Foundry) |
| **Author** | Claude (planning session) |
| **Created** | 2026-08-31 |
| **Rev** | 0.1 — DRAFT, ADRs UNANSWERED |
| **Status** | **PLANNING ONLY. NO CODE, NO MIGRATIONS, NO DELETIONS UNTIL RAZI'S EXPLICIT GO.** |
| **Supersedes** | `docs/autodev-vision/autodev-master-plan.md` **IN PART** — see Part 1E |

---

## Mandate

Handed over by Razi on 2026-08-31, verbatim in intent:

> Restructure AutoDev across three surfaces. **(1)** Refactor the UI against `docs/restructurization/ui-anchor.md`. **(2)** Sweep `src-tauri` for gaps, bugs and improvements, refactor it, and make it absorb all agent capability in-house. **(3)** Retire the `core-orchestrator` and `memory-mcp-server` submodules — and the sidecar architecture — completely.
>
> Agents move in-house into Rust, built to the semantics of the vendor Agents SDKs, following the way `LivingAgent` works in core-orchestrator and aligning the Episodic / Semantic / Procedural memory model with those SDKs.

**Answered before planning began:**

| # | Question | Razi's answer |
|---|---|---|
| 1 | Deliverable shape | The full suite: Master Plan + deep specs `01–NN` + ADR ledger |
| 2 | Agent runtime | **Native Rust, SDK-shaped** — *and* adopt SDK-native / server-side features wherever they genuinely help |
| 3 | Providers | **Anthropic, OpenAI, Gemini, Grok. Exactly these.** Ollama retired |
| 4 | Bug scope | The nine named bugs are not the full set — sweep end to end, line by line |
| 5 | End state | A properly working, installable AutoDev that **Windows, Linux and macOS** users can program with |

**Amended by Razi, 2026-08-31 (second pass):**

| # | Direction | Effect on this plan |
|---|---|---|
| 6 | `core-orchestrator/.env` **removed**; it was never committed anywhere | Closes the live-credential concern. INV-3/INV-12 remain as the recurrence guard; ADR-04-05 amended |
| 7 | Submodule deletion is **its own phase** of the programme | M-29 promoted to a gated phase; **ADR-04-05** rewritten around it |
| 8 | **BYOK — Bring Your Own Key.** The user enters their key in the app UI; no keys ship with AutoDev | New **M-30**, new **INV-12**, **ADR-03-01** reframed, new **ADR-03-05** |
| 9 | Subagent capability **must land natively in `src-tauri`** regardless | **ADR-02-03 answered** |
| 10 | A repo `.env` will exist for **development-time LLM testing**, excluded from the runtime app | **INV-12** and **ADR-03-05** — a dev credential must never become a runtime fallback |
| 11 | **Refine the user experience** — smooth and intuitive | New workstream **W6** (M-35…M-39), new **ADR-04-06**, grounding workstream `G11` |
| 12 | The worker-side capabilities — escalation, dynamic routing, worker↔worker chat and more — **are good and stay**; sweep them | New **M-31…M-34**, new **ADR-02-06**, grounding workstream `G9` |
| 13 | Sweep **every tool** handed to workers and to the PA in dev mode | Grounding workstream `G10` — 44 tool modules, per-consumer registration matrix |

## How to read this document

- **Part 1** is the thesis, the invariants, and the reconciliation against the old master plan. Read it first; everything else assumes it.
- **Part 1B** is the mechanics inventory (`M-01…M-NN`) — the atomic units of work, each owning an ADR and a spec.
- **Parts 2–5** are design: doctrine, component mechanics, data model, seams.
- **Part 6** is the build sequence; **Part 7** is how we prove it.
- **Part 8 is the ADR ledger — the only place decisions are recorded.** Every entry ends in a `▶ RAZI'S DECISION:` line. Spec docs `01–NN` reference ADRs; they never record decisions.
- **Parts 9–10** are open questions, risks, and the append-only amendment log.

**Evidence classes**, used inline on load-bearing statements: `CODE-PRESENT` (verified by reading shipped code) · `DOC-CLAIM` (a document asserts it; unverified against code) · `GAP` (specified but absent) · `BOUNDARY` (deliberately out of scope) · `MEASURED` (a number produced by running something).

---

## Grounding sources

Every input, its contribution, and its authority. Files marked STALE must not be built on.

| Source | Contribution | Authority | Freshness |
|---|---|---|---|
| Razi's mandate (2026-08-31, this session) | Scope, the five answered decisions | **DOCTRINE** | Authoritative |
| `docs/restructurization/ui-anchor.md` | The locked UI design system | **DOCTRINE** | Authoritative |
| `_grounding/G0-author-direct-findings.md` | Repo shape, CI, bundling, `src-cli`/`src-shared`, master-plan reconciliation | `CODE-PRESENT` | Authoritative |
| `_grounding/G1-rust-architecture-map.md` | 32 modules, 256 commands, **the 9 capabilities to port**, sidecar coupling inventory | `CODE-PRESENT` | Authoritative |
| `_grounding/G2b-bugs-provider-routing-memory.md` | 62 findings; provider capability matrix; caching verdict; cost-math audit | `CODE-PRESENT` | Authoritative |
| `_grounding/G3a/b/c/d-bugs-*.md` | 312+ findings across the platform modules | `CODE-PRESENT` | Authoritative |
| `_grounding/G4-livingagent-memory-semantics.md` | LivingAgent loop §1.1–1.15 | `CODE-PRESENT` | **PARTIAL — truncated at §1.16** |
| `_grounding/G4b-sidecar-subsystems-api-infra.md` | Port/delete verdicts, HTTP surface (9 of ~129 consumed), infra deps | `CODE-PRESENT` | Authoritative |
| `_grounding/G4c-memory-tiers-port-spec.md` | The three-tier memory reimplementation spec, with formulas | `CODE-PRESENT` | Authoritative |
| `_grounding/G5a/G5b/G5c-frontend-*.md` | Chat, Graph City, Settings/stores/contract drift | `CODE-PRESENT` | Authoritative |
| `_grounding/G6-agents-sdk-research.md` | Vendor agent-SDK semantics and server-side features | External docs, verified 2026-08-31 | Authoritative |
| `_grounding/G7-docs-corpus.md` | Doc register, documented contradictions, known-issue ledger | Mixed | See its own register |
| `docs/autodev-vision/autodev-master-plan.md` | The original product vision | **VISION** | **SUPERSEDED IN PART** (Part 1E) |
| `docs/development/DEVELOPMENT_PLAN.md` | Phase history and status | STATUS | Partially stale — see G7 |
| `docs/reference/autodev-graph-master-spec.md` | Graph subsystem design | SPEC | See G7 |
| `docs/development/PA_MEMORY_SYSTEM_SPEC.md` | Intended AutoDev-side memory design | SPEC | See G7 |
| `docs/endtoendrun/**` | Real end-to-end run post-mortems | **POST-MORTEM** | High value; see G7 |

---

# PART 1 · Concepts & first principles

## 1.1 Thesis

**AutoDev today is two products fighting each other: a complete Rust agent runtime, and a Python service that does the same job worse — and the seam between them is where the product fails.** The restructure deletes the Python half, promotes the Rust half to the whole product, and pays down the debt that accumulated because neither half was ever finished.

## 1.2 Why the gap is real — grounded, not aspirational

Four measured facts justify the whole programme.

**(a) The sidecar's surface is 7% consumed.** The orchestrator exposes ~129 HTTP endpoints. The Rust app calls **nine**, and only three carry real weight: `POST /invoke`, `POST /invoke/stream`, `GET /api/llm/models` (`G4b §B.2`, `CODE-PRESENT`). The other ~120 endpoints — memory, analytics, collaboration, orchestration, files, human-input, the MCP wizard — have **zero** callers from either Rust or TypeScript. We are shipping, bundling, health-checking, watchdogging and log-routing a 225 MB Python service to reach three endpoints.

**(b) The round-trip is a circle.** The AutoDev MCP server **already is Rust** (`src-tauri/src/mcp_server/`, 1,105 LOC). Today a worker's tool call travels: Rust tool registry → in-process HTTP MCP server → Python `MCPBridge` → litellm → the model → back. `mcp_server/` exists *solely* so orchestrator LivingAgents can call Rust tools over HTTP (`G1 §C.3`). **Deleting the sidecar collapses this to a direct function call.**

**(c) Only 13 files actually couple to the sidecar.** 61 files mention it; 13 have real coupling; **9 capabilities must be ported** (`G1 §C.4`). The blast radius is far smaller than the 53,387 Rust LOC suggests.

**(d) The bundle is the argument.** `tauri.conf.json` declares `resources/sidecars` — a 225 MB copy of the Python tree **including its `.env` and `.git`** — as a bundle resource (`G0 §3.1`, `CODE-PRESENT`). Gitignore does not protect a built artifact. A locally-built installer ships live AWS keys, provider keys, and a Fernet vault key to end users; a CI-built one ships a missing resource and fails at runtime. **Both branches are defects, and both are cured by the same deletion.**

## 1.3 The governing metaphor, mapped to mechanics

The product metaphor is *one app, any model, full autonomy* — a **single process that is the whole team**. Mapped to operators:

| Concept | Operator | Persisted evidence | Test gate |
|---|---|---|---|
| "One app" | Every agent turn executes in-process in Rust; **zero child processes** for agent work | No `uv`/`python` spawn in `sidecar/`; `submodules/` absent | `G-NOSIDECAR`: grep for sidecar spawn returns 0; installer contains no `.py` |
| "Any model" | One `Provider` trait, four implementations, runtime-switchable from the UI | `model_configs` row + a session recording which model actually served it | `G-SWITCH`: change model mid-session; next turn uses the new one, no restart |
| "Full autonomy" | A worker turn produces a **build-verified** artifact in the user's workspace | `build_evidence` row keyed to task + a file on disk under `working_dir` | `G-COMPILE`: a task cannot reach `qc_review` without a passing build on all three OSes |
| "Remembers" | Episodic/Semantic/Procedural recall is injected into the prompt and **provably read** | Recall trace: query → selected chunks → prompt bytes | `G-MEMREAD`: assert the prompt contains a memory known to be recalled |
| "Affordable" | Prompt caching on by default; context edited, then compacted | `cache_read_input_tokens > 0` across repeated turns | `G-CACHE`: a 5-turn session shows a non-zero cache-read on turns 2+ |

## 1.4 Hard invariants

Violating any of these silently corrupts the system. Each is testable, and each is grounded in a defect we actually found.

| ID | Invariant | Why — the defect it prevents |
|---|---|---|
| **INV-1** | **No agent work happens in a foreign runtime or language** (`G-02`). One agent runtime, written in Rust, linked by every binary that runs agent work. No sidecar, no child interpreter, no localhost agent API — and equally no second implementation of the runtime in another language, embedded or otherwise. *Restated in P1 (`P1-08`): the original wording, "outside the AutoDev process", was process-scoped, and an embedded interpreter would have satisfied it while breaking the premise.* | The entire premise. Today `subagent/bridge.rs` is 100% sidecar (`G2b`), so deleting it without a port kills every subagent. **P1 evidence:** `autodev-core` holds the runtime and is linked by both `src-tauri` and `airsctl`; `cargo tree` reports **zero** Tauri/GTK/webkit edges for core and the CLI against 28 for the desktop crate, and `src-cli/tests/core_runtime.rs` constructs an `AgentEngine` in a process with no UI framework. Not yet satisfied: `main.rs` still spawns `memory-mcp-server` and `core-orchestrator` (P8). |
| **INV-2** | **No provider API is reachable from the webview, and no key ever crosses the IPC boundary *outbound*.** Inbound is the BYOK entry path and is permitted; the same value must never come back out. | `get_provider_key`/`get_github_pat` return raw secrets to the renderer today, and CSP permits `connect-src api.github.com` — an allowed exfiltration path (`G3c IN-47`). |
| **INV-3** | **No shipped artifact contains a `.env`, a `.git`, or any credential.** | `G0 §3.1`. *(The `core-orchestrator/.env` that prompted this has been removed by Razi, 2026-08-31; the invariant is what stops it recurring.)* |
| **INV-12** | **Credential provenance is BYOK and single-sourced.** Every provider key originates from the user through the app UI and lives only in the OS keychain. **No key ships with the app. A development `.env` is a build-time-only convenience and is never a runtime fallback** — the runtime reads the keychain or fails loudly. | *Presence of a capability is not intent to use it.* A dev `.env` silently satisfying a runtime lookup is exactly how a test credential reaches production. Today `airsctl` already violates the spirit of this by reading a key from a CLI flag and a plaintext TOML (`G0 §5.6`). |
| **INV-4** | **A gate that cannot fail is not a gate.** No `continue-on-error` on any correctness check. | Today only `cargo clippy` and `tsc` can fail CI (`G0 §2`). |
| **INV-5** | **No agent-produced artifact is written to a path resolved against the process CWD.** Every write resolves against the session's `working_dir` or the app data root, explicitly. | `graph_tools.rs:176` writes a bare `.autodev/GRAPH_REPORT.md`; a stray `src-tauri/.autodev/graph.db` exists on disk (`G0 §6`). |
| **INV-6** | **Every agent-facing shell command is expressed platform-neutrally, or has a per-OS variant.** No POSIX-only syntax in any prompt, recipe, or skill. | `SK-50`: every worker recipe mandates `tee`/`$?`/`;` while the bash tool runs `cmd /C` on Windows ⇒ **no Windows task can reach QC.** |
| **INV-7** | **Any measurement the user sees is derived from provider-reported values, not estimated** — and cache-read, cache-write, input and output tokens are accounted separately. | Streaming input tokens are 0 for all three cloud providers (`G2b P-30`); the cost fallback `_ => (1.0, 3.0)` catches 7 of 11 models (`G2b C-02`). |
| **INV-8** | **A writer must have a proven reader.** Anything persisted must have a test that reads it back through the intended consumer. | `record_api_failure` has zero callers while the dashboard reads the table (`G3c IN-53`); the graph is built and (pending G2a) may have no context consumer. |
| **INV-9** | **Every blocking wait has a timeout and a cancellation path.** | A permission request whose listener died blocks the engine forever on a oneshot with no timeout (`G5a §8`). |
| **INV-10** | **One definition per schema.** No hand-maintained mirror of a table in a second crate. | `src-shared` and `src-tauri` both define `tasks`, and they have already drifted (`G0 §5.4`). |
| **INV-11** | **The updater installs nothing it has not cryptographically verified.** Absence of a signature is a failure, never a pass. | `verify.rs:90` skips verification entirely on an empty sig URL (`G3c IN-01`). |

## 1.5 Non-goals — BOUNDARY

- **Not** rewriting the Rust modules that already work. `graph/` core, `git/`, `ide/`, `skills/`, `db/` stay; they get bug fixes, not rewrites.
- **Not** preserving the sidecar's server-product surface: A2A over HTTP, the MCP wizard, S3/files, multi-tenant auth, collaboration, the Adobe compiler. All die (`G4b`).
- **Not** shipping a hosted/multi-user AutoDev. Single user, single machine, local-first.
- **Not** adopting a vendor's *hosted agent runtime* (Anthropic Managed Agents, OpenAI's hosted agent state) as the execution substrate — that would re-create the sidecar in someone else's cloud and is Claude-only or OpenAI-only. Server-side *features* (caching, context editing, hosted tools) are adopted per ADR-005.
- **Not** solving multi-machine fleet coordination. The old 8-PC fleet model is gone; one app, one machine.
- **Not** re-litigating the product vision. `docs/autodev-vision/autodev-master-plan.md` remains the product truth except where Part 1E supersedes it.

## 1.6 Worked example — one task, end to end, after the restructure

Grounding the abstraction in a concrete trace. Numbers are targets, not measurements.

> Razi, in Delegate mode: *"Add a /health endpoint to the orchestrator repo."*
>
> 1. **PA turn** (in-process). System prompt assembled: base + AUTODEV.md + memory recall. **Cached prefix** — turn 1 writes it at 1.25×, every later turn reads at 0.1× (`G6 §1.C.1`).
> 2. PA calls `create_task`. A row lands in `~/.autodev/tasks.db` with acceptance criteria. **No HTTP.**
> 3. **Supervisor** wakes on the task-created event (not a 2-minute poll), decomposes if needed, assigns to `worker-backend`, and **kicks it directly** — an in-process channel send, not a cron tick.
> 4. **Worker turn** (in-process `AgentEngine`). Tools resolve through `ToolRegistry` **directly** — no MCP HTTP round-trip. Every write lands under the session's `working_dir` (INV-5).
> 5. **Build verification** runs as a **Rust-native step**, not an LLM-issued shell string: the ecosystem is detected, the build command is executed by Rust with a platform-correct invocation, and the result is written as a `build_evidence` row (INV-6, and the fix for SK-50).
> 6. Worker requests `qc_review`. The guard reads `build_evidence` — present and passing, so the transition is allowed.
> 7. **QC subagent** spawns in-process from the same `AgentEngine`, reviews the diff, approves.
> 8. Supervisor closes the task. The **Live Monitor** showed every step, because tool-start/tool-result events were emitted on a channel the UI is subscribed to for the session's whole life — not only while a send is in flight (fixes `G5a §1`).
> 9. Razi opens the IDE tab and **runs the thing**. The artifact is in his workspace, at a path he chose.

Steps 4, 5, 6 and 9 are precisely where the product fails today.

## 1.7 Reconciliation — what the old master plan gets wrong now

`docs/autodev-vision/autodev-master-plan.md` (Draft v1.0, April 2026) is flagged **SUPERSEDED IN PART**. Full table in `G0 §4`. The load-bearing reversals:

| § | The old plan says | Status |
|---|---|---|
| §4.1 Sidecar Architecture | Bundle core-orchestrator + memory-mcp + Redis as managed sidecars | **SUPERSEDED.** All three deleted. |
| §4.1 Dual-Mode Execution | Mode 1 = lightweight Rust PA; Mode 2 = Python LivingAgent for workers | **SUPERSEDED.** Collapses to one Rust runtime. The "zero duplication" rationale inverts. |
| §4.1 Tool Flow | AutoDev tools "executed locally in core-orchestrator (not proxied back to Tauri)" | **SUPERSEDED — and a prime suspect for BUG-DELIVER.** If worker writes execute in the sidecar's CWD, produced code lands where the user cannot use it. |
| §6.3 Memory storage | Embeddings "via core-orchestrator memory-mcp-server" | **SUPERSEDED.** No embeddings exist anywhere in the Rust crate (`G1 §C.4 P3`). Must be solved in-house or dropped → **ADR-007**. |
| §7 Agent engine | "TypeScript, compiled to WASM or Node sidecar" | **SUPERSEDED.** Native Rust. |
| §7 MCP client | "TypeScript MCP SDK" | **SUPERSEDED.** Rust `rmcp`. The old plan contradicts itself here. |
| §7 UI styling | "Tailwind CSS" | **CONTESTED** → **ADR-010**. |
| Provider list | Anthropic, OpenAI, Gemini, **Ollama**, Azure | **SUPERSEDED.** Exactly four; Ollama and Azure out, **Grok in** (and Grok does not exist in code today). |
| §6.8 Model Router | Role + priority + budget → model | **`GAP`.** Does not exist; model is one global UI dropdown (`G2b R-01`). |

### Contradictions inside the old plan, to be resolved by ADR — not inherited
1. `~/.airs/` (§8) vs `~/.autodev/` (§4.1, §6.3) — **two data roots, and both exist in shipped code** → **ADR-009**.
2. `CLAUDE.md` (§8, §10) vs the emphatic "Not CLAUDE.md. AUTODEV.md." (§6.2) → **ADR-011**.
3. Auto-compaction at 80% (§6.1) vs "Replaces message compaction" (§6.3) → **ADR-006**.
4. Internal SQLite board (§4.1) vs "GitHub Issues as the Command Layer" (§9) → **ADR-012**.
5. `.airs/skills/` vs `.claude/skills/` (§6.4) — both "supported", neither branded.
6. Procedural memory at `~/.autodev/memory/MEMORY.md` (§6.3) vs a §8 tree with no memory directory at all.

---

# PART 1B · Capability & mechanics inventory

**40 mechanics across six workstreams.** Each is an atomic unit of work with an owning ADR and an owning spec doc. *(Was "29 across five" in rev 0.2, before W6 Experience, the fleet-coordination mechanics, M-30 BYOK and M-40 redaction were added. Corrected 2026-08-31 after the review caught the stale count.)*

**Balance notation** — where the work happens: `DET` deterministic Rust · `MATH` a formula, no model · `EMB` embedding/vector · `ORGAN` a model call.

## W1 — The Runtime

| ID | Fixes | Mechanism | Balance | In → Out | Module | ADR | Spec |
|---|---|---|---|---|---|---|---|
| **M-01** | Sidecar dependency for worker turns (`G1 P1`) | In-process fleet-agent turn: `AgentEngine` gains persona, role, and a fleet-agent wrapper | DET + ORGAN | task + worker config → turn result + events | `agent/` | 01-01 | 01 |
| **M-02** | Subagents die with the sidecar (`G2b`); pool permit leak deadlocks after 16 (`S-03`) | In-process subagent runner on tokio tasks; semaphore permit released by `Drop`, not by an explicit call | DET | spawn request → stream + result | `subagent/` | 02-03 | 01 |
| **M-03** | 3 of 4 providers unusable; **Grok absent**; `max_tokens` dropped; Gemini tool args discarded (`G2b P-01…P-30`) | Provider trait v2, normalized to OpenAI-shaped tool calls with per-provider adapters | DET | request → response/stream | `provider/` | 01-04 | 02 |
| **M-04** | **BUG-TOKENS (a)** — zero prompt caching anywhere (`G2a T1`) | Cache breakpoints over a byte-stable `tools → system → messages` prefix; cache-read/write accounted separately | DET | prefix → cached prefix | `provider/`, `agent/context.rs` | 01-05 | 02 |
| **M-05** | **BUG-TOKENS (b,c,d)** — full-history resend; compaction unreachable *and* discarded (`G2a T2`) | Layered context strategy: cached prefix → live history → `clear_tool_uses` → compaction backstop | DET + ORGAN | history + budget → sent context | `agent/context.rs`, `agent/compaction.rs` | 01-06 | 03 |
| **M-06** | Input tokens 0 for all 3 cloud providers streaming; price fallback catches 7/11 models; "today" never resets (`G2b P-30/C-02`, `G3c IN-50`) | Provider-reported usage only; 4-way token split; price table with **no silent fallback** | MATH | usage → cost rows | `cost/`, `provider/streaming.rs` | 02-05 | 02 |
| **M-07** | **BUG-MODELSWITCH** — choice dies on restart; `ModelConfigsDb` written, never read (`G2a`, `G5c F01`) | Persist active provider/model; hydrate on startup; wire temperature/top_p/reasoning/thinking into `CompletionRequest` | DET | UI selection → persisted + applied | `state/`, `db/model_configs.rs`, `provider/` | 01-04 | 02 |
| **M-08** | `ToolExecutor::execute()` — the only correct path — has **zero callers** (`G2a`) | Collapse to one execution path; delete the divergent ones | DET | tool call → result | `agent/tool_executor.rs` | 02-03 | 06 |
| **M-09** | `Tool::requires_permission()` never called; `ToolContext.depth`/`agent_role` hardcoded ⇒ recursion guard inert, build-evidence gate skipped (`G2a T47/T48/T50`) | Populate `ToolContext` truthfully; make the permission check the single gate | DET | tool call + context → allow/deny | `agent/permission.rs` | 02-03 | 06 |
| **M-10** | mem0/Qdrant dependency; `semantic.rs` is a SQL `LIKE`; no embeddings anywhere (`G1 P3`) | ⚠ **RESTATED after ADR-05-01…05-05** *(was: "lexical retrieval; seam left for a vector index" — stale from before the memory anchor was adopted)*. Three tiers in-house over **SQLite + `sqlite-vec` + FTS5**, hybrid retrieval with reranking, scopes, provenance, and a gateway that decides what is stored. **The review flags this as five mechanics wearing one ID** — it is decomposed in the MDP rather than left as a single row | MATH + EMB | query → recalled chunks | `memory/` | 01-06…01-08, 05-01…05-05 | 03 |
| **M-30** | **BYOK.** No key ships with the app; today `airsctl` takes a key as a CLI flag and from plaintext TOML (`G0 §5.6`), and the vault is readable from the webview (`IN-47`) | User enters the key in the UI → validated against the provider → stored in the OS keychain → read **Rust-side at point of use only**. Dev `.env` is build-time only, never a runtime fallback (INV-12) | DET | user input → validated, keychain-stored credential | `vault/`, `src/components/settings/`, `commands/vault_commands.rs` | 03-01, 03-05 | 02 |
| **M-40** | ⚠ **ADDED 2026-08-31 by the adversarial review — a capability grounding ruled PORT that this plan had dropped.** `G4b §A.14` is explicit: *"126 lines, directly reusable — the `SENSITIVE_KEYS` list is the spec. PORT."* The vault half of that verdict became ADR-03-01; **the redaction half was lost.** Verified: `redact`/`scrub`/`sanitiz` appear **zero times** across the plan and all ten specs. Meanwhile `G4:486` records unredacted tool arguments landing in procedural memory verbatim — which, now that memory is persistent, means **permanently** | Port `redact_dict` + its 20-entry `SENSITIVE_KEYS`; apply at every boundary where content leaves the process or enters durable storage: prompts, logs, memory writes, analytics, and any egress | DET | content → redacted content | new `security/`, `memory/`, `logging.rs`, `provider/` | 06-02 | 03, 06 |

| **M-41** | ⚠ **ADDED by the review (G-01).** `panic = "abort"` + in-process workers ⇒ **one panic kills the app and every session**. The sidecar was also a fault boundary, and INV-1 removed it without replacing what it did | Release profile → `panic = "unwind"`; every agent turn and subagent runs inside `catch_unwind`; a panicking worker fails **that turn**, reports it, and the app survives | DET | panic → contained turn failure | `Cargo.toml`, `agent/engine.rs`, `subagent/` | G-01 | 01 |
| **M-42** | ⚠ **ADDED by the review (G-04).** Nothing stops spending: per-task budget inert, no daily ceiling, no off switch; a broadcast fires one LLM tick per worker with no cap. Under BYOK the runaway bill is the user's | A user-set **daily ceiling enforced in the provider layer** — the one place every call passes — plus a **visible global kill switch**. Exceeding the cap pauses the fleet and surfaces it | DET | spend → enforced ceiling | `provider/`, `budget/`, `src/` | G-04 | 02 |
| **M-43** | ⚠ **ADDED by the review (G-08).** Undo does not exist — `db/checkpoints.rs` is not compiled — and `auto_checkpoint_before_write` returns `()`, so a failed checkpoint is invisible and the write proceeds | Compile the checkpoint module; a **failed checkpoint blocks the write**; every agent write stays inside the resolved workspace; **and** each task gets its own worker branch for coarse rollback. The two must never disagree about current state | DET | agent write → reversible agent write | `db/checkpoints.rs`, `git/`, `agent/builtin_tools/` | G-08 | 04, 09 |

> **This raises the ported-capability count from nine to ten**, and adds a P8 gate: the deletion phase cannot complete while a capability grounding marked PORT remains unported. That is the check which would have caught this without a review.

## W2 — The Fleet

| ID | Fixes | Mechanism | Balance | In → Out | Module | ADR | Spec |
|---|---|---|---|---|---|---|---|
| **M-11** | **BUG-ASSIGN (a)** — dead-latch on `fallback_idle`; a DB blip kills the schedule forever (`SK-01/SK-02`) | Event-driven dispatch; worker state as an exhaustively-matched Rust enum; cron demoted to a reconciler | DET | task-assigned event → worker kick | `recipes/scheduler.rs` | 02-01 | 04 |
| **M-12** | **BUG-ASSIGN (b,c)** — task theft (claim ignores `assigned_worker`); one-task guard fails open (`G3d F-55`) | Atomic conditional claim including the owner; guard inside the same statement | DET | claim request → claimed \| rejected | `db/tasks.rs` | 02-01 | 04 |
| **M-13** | **BUG-COMPILE** — evidence in a 15-min in-memory map; PowerShell/cmd mismatch records passing builds as failed; no Rust-native compile (`G3d F-02/F-03/F-05`) | Rust detects the ecosystem, runs the build via argv it controls, reads the exit code directly, persists `build_evidence`. **Per ADR-02-02: deterministic is the main path; an LLM debug step engages only after 3 consecutive failures** — the model diagnoses, it never reports the verdict | DET (+ ORGAN on the 3rd failure) | workspace → persisted evidence row | new `verify/`, `db/` | 02-02 | 04 |
| **M-14** | **BUG-DELIVER** — `load_workspace_dir()` returns `""` on error; 3 divergent fallbacks; output lands in `~/.autodev/agents/` (`G3d F-DEL`) | One workspace resolver that **fails loudly**; every write resolves against it (INV-5) | DET | session → resolved workspace root | `commands/workspace_commands.rs` | 02-02 | 04 |
| **M-15** | **BUG-DELIVER (2)** — no run/launch/handoff step; `open_in_ide` has zero listeners (`G3d`) | A real handoff: reveal, open in IDE tab, and run the artifact | DET | completed task → user-runnable artifact | `commands/`, `src/tabs/IDE.tsx` | 02-02 | 04 |
| **M-16** | Orphan TTL can never fire; QC-age alert always 0 s — SQLite `datetime('now')` parsed as RFC3339 at 2+ sites (`GI-01`, `F-45/F-45b`) | One timestamp discipline across every DB write and read | DET | stale claim → released task | `orphan/`, `db/`, `commands/task_commands.rs` | 02-01 | 08 |

### W2 — Fleet coordination capabilities (**preserve and complete — these are wanted features**)

Razi's direction, 2026-08-31: the worker-side coordination capabilities are **good and stay**. They are audited in `G9` and treated as port-and-fix, never as deletion candidates.

| ID | Capability | Preserve because | Known risk to verify | Module | ADR | Spec |
|---|---|---|---|---|---|---|
| **M-31** | **Supervisor escalation queue** — a worker raises a blocker; the Supervisor resolves it | The only path by which a stuck worker gets unstuck without human intervention | Is the queue actually drained? Does an unresolved request expire, block, or leak? | `request_supervisor_action.rs`, `list/resolve_worker_requests_tool.rs`, `worker_requests` | 02-06 | 04 |
| **M-32** | **Worker↔worker messaging (ACP)** — structured inbox over the `worker_messages` table | Lets a frontend worker ask a backend worker for a contract instead of guessing | `ToolContext.agent_role` is hardcoded `None` at all three construction sites (`G2a T49`) — sender identity and self-message guards may be inert. Loop/storm risk unassessed | `send/list/acknowledge_worker_message.rs` | 02-06 | 04 |
| **M-33** | **Dynamic routing & worker affinity** — historical affinity informs assignment | Real cost and quality lever; the fleet's own learning loop | Task theft overwrites `assigned_worker` while `routing_history` keeps the old name — **affinity learns from corrupted data** (`G3d F-55`) | `routing/`, `suggest_worker_tool.rs`, `routing_history` | 02-04 | 04 |
| **M-34** | **Epic / phase decomposition** — epics fan out into phase-gated tasks | How large work becomes assignable work | Epics are inserted `status='ready'` and `assign_task` never filters `task_type='epic'` — an epic may be assignable to a worker as if it were a task (`G3d`) | `create_epic_tool.rs`, `create_phased_task_tool.rs`, `get_epic_progress_tool.rs`, `get_ready_tasks_in_phase_tool.rs`, `plan/` | 02-01 | 04 |

## W3 — The Graph

| ID | Fixes | Mechanism | Balance | In → Out | Module | ADR | Spec |
|---|---|---|---|---|---|---|---|
| **M-17** | **BUG-GRAPH (a)** — `graph_tools.rs` + `graph_prompts.rs` (918 LOC) have no `mod` declaration and **are never compiled** (`G2a T32`) | Declare the modules; register the tools; add a guard test that every source file is reachable from the crate root | DET | — → graph tools exist in the binary | `agent/builtin_tools/mod.rs` | — | 05 |
| **M-18** | **BUG-GRAPH (b)** — nothing reads the graph for context scoping (`G2a`) | Graph-scoped context selection feeding M-05 | MATH | task + graph → relevant file set | `agent/context.rs`, `graph/query.rs` | 01-06 | 05 |
| **M-19** | Two competing DB path schemes; registry created but never populated; `INSERT OR REPLACE` + cascade silently deleting edges; incremental drops cross-file edges and never re-clusters (`F-21/F-22/F-23`) | One canonical, normalized DB path; non-destructive upsert; incremental re-clustering | DET | repo → correct graph | `graph/` | 01-09 | 05 |

## W4 — The UI

| ID | Fixes | Mechanism | Balance | In → Out | Module | ADR | Spec |
|---|---|---|---|---|---|---|---|
| **M-20** | Anchor not implemented: **7px appears zero times** (12+ radii in use); Urbanist/Manrope/Roboto Mono **zero occurrences** (IBM Plex ships); 1 of 34 `.airs-*` classes exists, in a dead file; two conflicting dark palettes; ~22 undefined CSS vars (`G5c F11/F12`) | Implement `ui-anchor.md` as the single token + class system; delete both dark palettes | DET | anchor → implemented design system | `src/styles/`, all components | 01-10 | 07 |
| **M-21** | **BUG-CHAT** + Live Monitor staleness — listeners live only inside `handleSend`/a mounted tab and die on switch (`G5a §1`, `G5c F07`) | Session-lifetime subscription owned above the tab boundary, with backfill on remount | DET | session → uninterrupted event stream | `src/stores/`, `src/App.tsx` | — | 07 |
| **M-22** | Contract drift: 2 missing commands, **47 dead registered commands (18%)**, 10 dead listeners, 3 dead events incl. `plan:changed` with 16 emit sites and no listener (`G5c`) | Reconcile both directions; a build-time check that every `invoke`/`listen` name resolves | DET | — | `src/`, `src-tauri/src/lib.rs` | — | 07 |
| **M-23** | **BUG-SETTINGS** — no persistence; writes with no reader; unwired controls; failed saves show green (`G5c F01/F02/F03/F06`) | Persist and hydrate every setting; delete or wire every orphaned control; surface save failures | DET | UI change → persisted + applied | `src/components/settings/`, `state/`, `runtime_config.rs` | 01-09 | 07 |
| **M-24** | **BUG-CITY** — CSP blocks drei/troika CDN fonts so every `<Suspense>` hangs; tab never auto-loads; flat skyline; community `-1`; per-mousemove geometry churn (`G5b`) | Self-host fonts; load on `workspaceDir`; send `line_start`/`line_end` and `confidence_tier`; fix the `-1` bucket; memoize | DET | graph → rendered city | `src/components/city/`, `commands/graph_commands.rs` | — | 07 |
| **M-25** | **~3,700 LOC unreachable** — the memory, plugin and plan directories and their 3 stores cannot be reached from any route (`G5c F08`) | Decide per surface: route it or delete it | DET | — | `src/components/{memory,plugin,plan}/` | 03-03 | 07 |

## W5 — Delivery

| ID | Fixes | Mechanism | Balance | In → Out | Module | ADR | Spec |
|---|---|---|---|---|---|---|---|
| **M-26** | POSIX syntax under `cmd /C`; `/tmp` hardcoded; hooks have one non-OS-specific command field; `.cmd` shims unresolvable (`SK-50`, `P-01…P-23`) | A `Command` abstraction: callers express intent, Rust picks the platform-correct invocation | DET | intent → platform-correct argv | new `platform/`, `agent/builtin_tools/bash.rs` | 04-01 | 09 |
| **M-27** | Updater installs unverified binaries; placeholder public key; Linux install is a no-op returning success; `targets: ["nsis"]`; unsigned on macOS and Windows (`IN-01…IN-05`, `IN-13`, `G0 §3`) | Real trust root, signature required; per-platform bundle targets; working install on all three | DET | release → verified install | `updater/`, `tauri.conf.json` | 03-04, 04-02 | 09 |
| **M-28** | Nothing in CI can fail but clippy and tsc; no frontend test framework; smokes skip PRs and macOS (`G0 §2`) | Blocking gates; frontend units; an end-to-end smoke that drives one real task to a runnable artifact | DET | commit → pass/fail | `.github/workflows/`, new test suites | 04-03 | 10 |
| **M-29** | The sidecar itself: 225 MB bundled with `.env` and `.git`; 29 sidecar-dependent commands; 3,161 LOC in `sidecar/` (`G0 §3.1`, `G3d`, `G1 §A.23`) | Excise in dependency order after M-01/M-02 land; rotate credentials first | DET | — | `sidecar/`, `submodules/`, `tauri.conf.json` | 04-05 | 09 |

## W6 — Experience

Razi's direction, 2026-08-31: *"refine the user experience as well, making everything as smooth as possible for the user and intuitive."* This is a first-class workstream, not polish applied at the end — and BYOK (M-30) makes the first-run flow a **primary product surface**. Audited in `G11`.

| ID | Fixes | Mechanism | Balance | In → Out | Module | ADR | Spec |
|---|---|---|---|---|---|---|---|
| **M-35** | **No onboarding path.** With BYOK the user *must* supply a key before anything works, and today the only route is several clicks deep in Settings; opening Chat first yields `"No active provider set"` surfaced as a blank bubble and a destroyed message (`G5a §7`) | A first-run flow: workspace selection → provider choice → key entry → **live key validation** → a first successful turn | DET | launch → working agent | `src/components/startup/`, settings | 03-05, 04-06 | 07 |
| **M-36** | **Failures are invisible or unactionable.** Toast system exists and is bypassed; `invoke` calls without `.catch`; a failed save shows green "Saved!" (`G5c`); errors discarded unless the last message happens to be an empty assistant bubble (`G5a §7`) | One error contract: every failure surfaces with a cause and a next action; retry affordances where retry is meaningful | DET | failure → actionable message | `src/stores/toastStore.ts`, all callers | 04-06 | 07 |
| **M-37** | **No honest progress.** Long operations (graph build 30–90 s, worker ticks, clone) with no progress or cancellation; the build overlay is hidden entirely in city mode (`G5b B11`) | Real progress from real events; cancellation where the backend supports it; empty states that say what to do next | DET | long op → visible, cancellable progress | `src/`, `events.rs` | 04-06 | 07 |
| **M-38** | **Destructive actions without confirmation** — a one-click trash icon calls `git_discard` and permanently destroys uncommitted work (`G3b GI-07`); `delete_file` runs `remove_dir_all` while its doc says "empty directory" (`GI-05`) | Confirmation proportional to blast radius; undo where feasible (note `db/checkpoints.rs` is **not compiled**, so undo does not currently exist — `G3d F-44`) | DET | destructive intent → confirmed, reversible action | `src/components/git/`, `ide/` | 04-06 | 07 |
| **M-39** | **Concept and surface incoherence** — ~3,700 LOC of memory/plugin/plan UI unreachable from any route (`G5c F08`); inconsistent vocabulary; the app never explains PA modes, Supervisor/Worker/QC, recipes, skills or hooks in-product | One vocabulary; every shipped surface reachable or deleted; in-product explanation of the agent model | DET | — | `src/` | 03-03, 04-06 | 07 |

## Model-call accounting — the honest roll-up

Where a model actually fires, after the restructure:

| Family | Model calls per unit | Note |
|---|---|---|
| PA turn | 1 per user message + 1 per tool round-trip (≤ `MAX_TOOL_TURNS`) | The product's core loop |
| Worker turn | same, per task attempt | |
| QC review | 1 subagent turn + 1 diff classification | The classification call is currently capped at `max_tokens: 512`, which **guarantees truncation** and discards the free deterministic risk score (`GI-94/GI-95`) |
| Compaction | 1 extra full-context call **when triggered** | Today it fires ~never, and the result is discarded (`G2a`). Under M-05 it fires rarely and is used |
| Memory: episodic write | **0** | Verbatim, `infer=False` |
| Memory: semantic extract | 1 per user message, fire-and-forget | → **ADR-01-07** may remove this entirely |
| Memory: recall | **0** | Lexical + `MATH` scoring |
| Build verification | **0** | `DET` — this is the point of M-13 |
| Graph build & scoping | **0** | `DET` + `MATH` |
| Task assignment | **0** | `DET` — supervisor decomposition is a separate ORGAN call |

**Nine of thirteen families are model-free.** The restructure moves build verification and graph scoping *out* of the model's hands, which is both cheaper and — as BUG-COMPILE demonstrates — more correct.

---

# PART 1C · The pipeline in plain terms

**The analogy.** Today AutoDev is a workshop where the foreman phones an outside contractor for every job, the contractor works in his own garage, and the finished cabinets are left in a shed nobody has the key to. The restructure fires the contractor, moves the tools inside, and puts the cabinets in the room where the customer is standing.

**The flow, after:**

```
 user ──▶ PA (in-process)
            │  creates task
            ▼
       tasks.db ──event──▶ Supervisor ──assign+kick──▶ Worker (in-process)
                                                          │
                        ┌─────────────────────────────────┤
                        ▼                                 ▼
                  ToolRegistry (direct call)      Rust build verifier  ──▶ build_evidence
                        │                                 │
                        ▼                                 ▼
                 user's workspace ◀───────────── qc_review gate (reads evidence)
                        │                                 │
                        │                                 ▼
                        │                          QC subagent (in-process)
                        ▼
                   IDE tab: reveal · open · RUN
```

**Per subsystem — what it eats, what it produces, which spec:**

| Subsystem | Eats | Produces | Spec |
|---|---|---|---|
| Agent runtime | messages + tools + context | a turn: text, tool calls, usage | `01` |
| Provider layer | a normalized request | a normalized response/stream + exact usage | `02` |
| Context & memory | history + query + budget | the exact bytes sent, with a cached prefix | `03` |
| Fleet | tasks + worker config | claimed work, build evidence, delivered artifacts | `04` |
| Code graph | a repo | a queryable graph, and a relevant-file set for context | `05` |
| Tools & permissions | a tool call + truthful context | an effect, gated | `06` |
| UI | events + commands | the anchor's design system, live | `07` |
| Data model | — | 10 SQLite DBs, one schema owner each | `08` |
| Delivery | a commit | a signed, verified installer on three OSes | `09` |
| Proof | the whole system | gates that can actually fail | `10` |

---

# PART 2 · The deterministic ↔ organ doctrine

## 2.1 The rule

**A model may fire only where judgement is genuinely required. Everything checkable is checked deterministically.**

This is not a stylistic preference. BUG-COMPILE exists precisely because a language model was made responsible for authoring a shell command whose exact byte-level syntax gated the entire task pipeline — and on Windows that syntax was wrong, silently, for every task ever run.

## 2.2 Where a model may fire

| Permitted | Why |
|---|---|
| Deciding *what* to do (planning, decomposition, next action) | Genuine judgement |
| Writing and editing code | The product |
| Reviewing a diff for quality | Genuine judgement |
| Summarising for compaction | Lossy by nature; no deterministic equivalent |
| Extracting facts for semantic memory | Genuine judgement (and see ADR-01-07) |

| Forbidden | Replaced by | Defect it prevents |
|---|---|---|
| Authoring the build/test command | `DET` ecosystem detection + Rust-controlled argv (M-13) | BUG-COMPILE |
| Reporting whether a build passed | The process exit code, read by Rust | Successful builds recorded as failed |
| Choosing where a file is written | The workspace resolver (M-14) | BUG-DELIVER |
| Deciding whether a task may advance state | The `build_evidence` gate, read by Rust | The gate being skippable |
| Counting tokens or computing cost | Provider-reported usage (M-06) | Fabricated dashboard numbers |
| Selecting which files are relevant | Graph query + scoring (M-18) | Full-history resend |

## 2.3 The airlock laws

Any future model call obeys all four:

1. **Bounded input.** Never the whole history by default — a selected, budgeted context (M-05).
2. **Typed output.** Structured output or a schema-validated parse. Never a free-text response that a downstream `if` branches on. *(Today `diff_analysis` parses free-form JSON at `max_tokens: 512`, which guarantees truncation and then discards the deterministic risk score it already computed — `GI-94/GI-95`.)*
3. **A deterministic fallback.** If the call fails, times out, or returns unparseable output, the system degrades to a `DET` answer and says so. Never a hard abort that throws away work already done.
4. **Accounted.** Every call reports real usage into M-06. No estimates.

## 2.4 The balance, by workstream

| Workstream | Mechanics | DET | MATH | ORGAN | EMB |
|---|---|---|---|---|---|
| W1 Runtime | 11 | 8 | 2 | 1 (shared) | 0–1 (ADR-01-07) |
| W2 Fleet + coordination | 10 | 9 | 1 | 0 | 0 |
| W3 Graph | 3 | 2 | 1 | 0 | 0 |
| W4 UI | 6 | 6 | 0 | 0 | 0 |
| W5 Delivery | 4 | 4 | 0 | 0 | 0 |
| W6 Experience | 5 | 5 | 0 | 0 | 0 |

**34 of 39 mechanics are deterministic.** The restructure is overwhelmingly an engineering-correctness and experience programme, not an AI programme — which is the honest read of a bug register in which almost every entry is a silently-swallowed error, an unreachable guard, or a writer with no reader.

# PART 3 · Component mechanics

At the depth needed to decide the ADRs. Exhaustive treatment moves to specs `01–10`.

## 3.1 The agent runtime (`01`)

**Central loop.** Unchanged in shape from today's `AgentEngine`, which `G1 §D.1` assesses as *complete for the single-agent case*: assemble context → call provider → if tool calls, execute and append results → repeat until no tool calls or `MAX_TOOL_TURNS`. What changes is everything around it.

**Core DTOs** (frozen, `deny_unknown_fields`):
- `Turn { session_id, items: Vec<Item>, usage: Usage, stop: StopReason }`
- `Item::{ User, Assistant, ToolCall { id, name, args: Value }, ToolResult { id, content, is_error }, Opaque { provider, kind, bytes } }`
- `Usage { input, cached_read, cache_write, output, reasoning, tool }` — **six fields, not two.** Today's `ModelInfo` carries only in/out prices, which is why caching cannot be represented at all (`G2b`).

**`Item::Opaque` is load-bearing and must exist from day one.** Four providers carry unreplayable reasoning state under different names — Anthropic thinking `signature`, OpenAI `encrypted_content`, Gemini thought signatures, xAI `reasoning_content` — and dropping it breaks turn continuity *and* is xAI's single largest cause of cache misses (`G6 §5.1`). Retrofitting it means touching every adapter again.

**Invariants.** Every exit path emits a terminal event (today `engine.rs:299`, `:164`, `:92` and `agent_commands.rs:134-137` all return without one, which is why the UI spins forever — `G5a §1`). Every blocking wait has a timeout (INV-9). `ToolContext` is populated truthfully — `depth` and `agent_role` are hardcoded at all three construction sites today, which silently disables the subagent recursion guard, the ACP identity checks, and the `qc_review` build-evidence gate (`G2a T48/T49/T50`).

## 3.2 Providers (`02`)

**One trait, four adapters, normalized to the OpenAI-shaped tool-call wire format** — the design LiteLLM already proved in the sidecar (`G4b §A.1`). The trait is sketched in ADR-01-04.

**The divergence the abstraction must absorb** (`G6 §5.2`): caching is *explicit* on Anthropic (≤4 breakpoints, per-model minimums), *fully automatic* on xAI (steered by an `x-grok-conv-id` header), and different again on the other two — hence `CachePolicy` has four variants. Tool-call ids differ (OpenAI has two, only `call_id` returns; Gemini renames `id`→`call_id`). Anthropic requires **all** parallel `tool_result`s in a single user message or it silently stops calling tools in parallel. Truncation is signalled as `max_tokens` / `MAX_TOKENS` / `length` depending on vendor, and xAI's finish-reason set is undocumented. Rate-limit headers exist only on Anthropic (RFC 3339) and OpenAI (duration strings).

**Capabilities are keyed by model, not provider.** The four vendors vary per model, not per vendor — so `capabilities(&ModelId)` is the only correct shape.

## 3.3 Context and memory (`03`)

**The composition, in precedence order** (ADR-01-06):

1. **Stable cached prefix** — `tools → system → AUTODEV.md → recalled memory`. Built by **one** builder with ordered maps, because Anthropic requires a 100% byte match and non-deterministic JSON key order defeats caching silently (`G6 §5.5`). Today the system prompt is rebuilt per run and is **not byte-stable** (`G2a Q4`), so caching would not work even if it were wired.
2. **Live history** — not `history.clone()` every turn (`engine.rs:196`).
3. **`clear_tool_uses`** above a threshold tuned so `clear_at_least` exceeds the cache-write it forces.
4. **Compaction** as a long-session backstop only.

**The memory tiers** are specified formula-by-formula in `G4c`. The one decision that changes behaviour rather than porting it is competitive-vs-quota budgeting (ADR-01-08).

**A defect the port must not reproduce:** `_get_episodic_key` is `f"episodic:{user_id}:{session_id}"` with the `query` argument unused despite a docstring claiming it hashes the query — so two different questions in one session share a cache entry (`G4c §5.4.2`). And semantic recall ignores its `query` parameter entirely, making it query-independent (`§5.4.1`).

## 3.4 The fleet (`04`)

**Worker state is a Rust enum matched exhaustively.** The current dead-latch (`SK-01`) exists because a `match` on a `&str` status had a catch-all arm that silently skipped ticks for two states the runner could actually write. A compiler-enforced exhaustive match makes that class structurally impossible.

**The claim is one atomic statement** including `assigned_worker`, not a `SELECT COUNT` guard followed by an `UPDATE` — today the guard fails open via `.unwrap_or(0)` and the claim ignores ownership, permitting task theft (`G3d F-55`).

**Build verification is deterministic** (M-13, ADR-02-02): Rust detects the ecosystem, runs the build with an argv it controls, reads the exit code, and persists `build_evidence`. Today evidence is a process-global `HashMap` with a 15-minute `Instant` TTL, never persisted, written by a heuristic that classifies a *failed* `tee` pipeline as a failed build (`G3d F-02/F-03/F-05`).

**Coordination capabilities** (M-31…M-34) are audited in `G9` and preserved per ADR-02-06.

## 3.5 The code graph (`05`)

**First, make it exist in the binary.** `graph_tools.rs` (840 LOC) and `graph_prompts.rs` (78 LOC) have no `mod` declaration (`G2a T32`). A guard test must assert every `.rs` file under `src/` is reachable from the crate root — the same defect independently hides `db/checkpoints.rs`, which is why undo/rollback is not in the shipped app (`G3d F-44`).

**Then give it a reader.** Graph-scoped file selection feeds the context builder (M-18). This is the "workers not using it for token management" half of BUG-GRAPH.

**Correctness fixes:** one canonical normalized DB path (today two schemes, and the path is hashed verbatim so `C:\p`, `C:/p`, `c:\p` and a trailing slash each produce a different database); non-destructive upsert (today `INSERT OR REPLACE` plus `ON DELETE CASCADE` silently deletes edges graph-wide); incremental update must re-cluster and must not drop cross-file edges (`G3d F-21/F-22/F-23`).

## 3.6 Tools and permissions (`06`)

**One execution path.** `ToolExecutor::execute()` — the only fully-correct one — has zero callers today (`G2a`). **One gate**: `Tool::requires_permission()` is never called anywhere (`T47`), so the permission model is decorative. Detailed per-tool mechanics come from `G10`.

## 3.7 The UI (`07`)

**The anchor is implemented, not approximated.** Current distance: 7px appears **zero** times against 12+ radii in use; Urbanist/Manrope/Roboto Mono have **zero occurrences** (IBM Plex ships instead); 1 of 34 `.airs-*` classes exists, in a dead file; two conflicting dark palettes are live while the anchor mandates light-only; ~22 CSS custom properties are referenced and never defined (`G5c F11/F12`).

**Event subscription is owned above the tab boundary** so it survives tab switches — the single mechanism behind both BUG-CHAT and Live Monitor staleness (M-21).

---

# PART 4 · Data model

## 4.1 What exists — the verified live state

**11 SQLite databases** (`G1 §E.1`, `CODE-PRESENT`). The migration head is meaningful in exactly one of them.

| # | File | Owner | Tables |
|---|---|---|---|
| 1 | **`~/.airs/autodev.db`** | `session/storage.rs:293` | `sessions`, `messages` |
| 2 | `~/.autodev/tasks.db` | `db/tasks.rs:287` | `tasks`, `worker_requests`, `worker_messages`, `routing_history` |
| 3 | `~/.autodev/config.db` | **six modules** | `autodev_instructions`, `model_configs`, `hooks`, `hook_logs`, `app_settings`, `checkpoints` |
| 4 | `~/.autodev/workers.db` | `db/workers.rs:111` | `workers` (+ 7 seeded fleet workers) |
| 5 | `~/.autodev/plans.db` | `db/plans.rs:205` | `plans`, `plan_items`, `plan_item_dependencies`, `plan_executions` |
| 6 | `~/.autodev/memory.db` | `memory/db.rs:122` | `episodic_memories`, `semantic_memories`, `fact_history`, `procedural_memories` |
| 7 | `~/.autodev/analytics.db` | `analytics/db.rs:23` | `token_usage`, `daily_costs`, `worker_stats`, `api_failures`, `system_events` |
| 8 | `~/.autodev/annotations.db` | `db/annotations.rs:51` | `annotations` |
| 9 | `~/.autodev/checkpoints.db` | `db/checkpoints.rs:67` | `file_checkpoints` — **module not compiled** |
| 10 | `~/.autodev/graphs/<12-char-sha>.db` | `graph/store.rs:66` | `nodes`, `edges`, `communities`, `flows`, `snapshots`, `analysis_cache`, `repos`, `nodes_fts` |
| 11 | `<repo>/.autodev/graph.db` | `graph/registry.rs:62,123` | a **second, competing** graph location |

## 4.2 The reconciliations the substrate forces

| # | Divergence | Resolution |
|---|---|---|
| **R1** | **Sessions live under `~/.airs/`, everything else under `~/.autodev/`** — a *third* data root beyond the two the old plan already contradicted itself about | ADR-01-09. This is now the strongest evidence for a single root plus a one-time migration |
| **R2** | **`app_settings` has three independent creators** — `commands/workspace_commands.rs:23`, `commands/github_mcp_commands.rs:30`, `ide/workspace.rs:35` — each doing its own `CREATE TABLE IF NOT EXISTS` | One owner (INV-10) |
| **R3** | **Two checkpoint systems**: `checkpoints` in `config.db` (`git/checkpoint.rs`) and `file_checkpoints` in `checkpoints.db` (`db/checkpoints.rs`, **not compiled**) | Pick one. ⚠ ~~the uncompiled one is why undo does not exist~~ — **corrected 2026-09-01: undo ships** via `git/checkpoint.rs` (`git/mod.rs:21`, 5 commands, UI-wired, auto-snapshot on every agent write). `db/checkpoints.rs` is uncompiled and has **zero callers**, so it creates no second database today |
| **R4** | **Two graph DB locations** (#10 vs #11); the registry DB is created but never populated, so `merge_all_repos_cmd` always merges empty stores and returns `Ok` | One canonical, normalized path (M-19) |
| **R5** | **`src-shared` mirrors the `tasks` schema by hand and has already drifted** — it lacks `rejection_reason`, `retry_count`, `claimed_by_pid`, `claim_host` and one index (`G0 §5.4`) | One schema owner (INV-10, ADR-01-03) |
| **R6** | **`analytics/queries.rs:29` queries `hourly_costs`, a table `migrate()` never creates**, and masks the failure as `-1` (`G3c IN-54`) | Delete or create it |
| **R7** | **Timestamp format is inconsistent** — SQLite `datetime('now')` written, RFC 3339 parsed, at three known sites, disabling the orphan TTL and the QC-age alert (`GI-01`, `F-45/F-45b`) | One format, one helper, everywhere (M-16) |

## 4.3 Migration discipline

**Today: there is essentially none.** Only `graph/` has a real ladder (`PRAGMA user_version`, 5 migrations, head **v5**). Every other database uses idempotent `CREATE TABLE IF NOT EXISTS` on each open — **no schema-version table, no down-migration, and no backup path anywhere in the crate**. `updater/persistence.rs` creates a restore point for `config.db` and `analytics.db` only, and only during an update.

Worse, the one ladder that exists is broken at its head: **migration v5 uses `ADD COLUMN IF NOT EXISTS`, which is not valid SQLite** — a syntax error that is silently swallowed and can never retry (`G3d F-31`).

**The discipline going forward:**
1. Every database gets `PRAGMA user_version` and a numbered ladder. New migrations are appended after the **verified live head** — v5 for `graph/`, v0 for everything else, and **v5 must be repaired before v6 is added**.
2. `journal_mode=WAL` and a `busy_timeout` on every connection. Today only `memory.db` sets WAL; `analytics.db` sets `foreign_keys=ON` on a schema with no foreign keys, and no `busy_timeout` anywhere — so a concurrent writer gets an immediate `SQLITE_BUSY` (`G3c IN-65`).
3. **A backup taken before any destructive migration.** There is no backup path today.
4. Indexes on every hot query column, and **no function wrapping an indexed column in a `WHERE`** — `DATE(timestamp)` currently defeats `idx_token_usage_timestamp` on every dashboard query (`IN-63`).
5. Retention: `token_usage`, `system_events`, `api_failures`, `worker_stats` and `hook_logs` all grow unbounded with no pruning and no `VACUUM` (`IN-64`, `SK-23`).

---

# PART 5 · Seams, boundaries & disciplines

**S1 · The webview seam.** Commands and events only. Keys cross **inbound only** (INV-2, BYOK). No provider API is reachable from the renderer. Every `invoke` name and every `listen` name resolves at build time — today 2 frontend calls target commands that do not exist, 47 registered commands (18%) are never called, 10 listeners are bound to events never emitted, and 3 events are emitted with no listener, including `plan:changed` with **16 emit sites and no listener** (`G5c`).

**S2 · The provider seam.** One trait; adapters own all vendor-specific JSON. No vendor type escapes into the engine, with the single deliberate exception of `Item::Opaque`, whose bytes the engine carries without interpreting.

**S3 · The tool seam.** `ToolRegistry` is the only way to reach an effect. After the restructure agents call it **directly, in-process** — no HTTP round-trip (the current MCP loop exists only to serve the sidecar, ADR-03-02). Every tool call passes exactly one permission gate with a truthful `ToolContext`.

**S4 · The filesystem seam.** One workspace resolver that **fails loudly** (M-14). Every write resolves against the session `working_dir` or the app data root (INV-5). `path_jail` is the enforcement point and must actually be reachable from every write path — today the read tools bypass it entirely and are auto-approved (`G2a T12/T13`).

**S5 · The persistence seam.** One owner per table (INV-10). No hand-maintained schema mirrors.

**S6 · The process seam.** After M-29 there is exactly one AutoDev process. No child interpreters for agent work (INV-1). The only permitted subprocess spawns are build/test/validate commands, invoked through the platform `Command` abstraction (M-26).

**Naming locks:** `AUTODEV.md` (never CLAUDE.md/AGENTS.md, ADR-01-11) · one data root (ADR-01-09) · `.airs-*` for the UI class vocabulary (ADR-01-10) · `worker` for a fleet agent and `subagent` for an ephemeral child, used consistently in code, UI and docs.

---

# PART 6 · Build sequence

Slices, their exit gates, and their dependencies. **The atomic task breakdown moves to the MDP once the ADRs are answered** — this is the shape, not the schedule.

| # | Slice | Contains | Exit gate | Depends on |
|---|---|---|---|---|
| **P0** | **Stop the bleeding** | Repair the updater trust root (M-27 partial); asymmetric vault (M-30/ADR-03-01); make existing CI gates blocking (M-28 partial); delete `dump.rdb` | An unsigned/unverified update **cannot** install; no command returns a key; a failing test fails CI | — |
| **P1** | **Make the invisible visible** | Declare the undeclared modules (M-17 + `db/checkpoints.rs`); the reachability guard test; fix the workspace resolver to fail loudly (M-14) | `graph_tools`, `graph_prompts`, `checkpoints` are in the binary; a guard test proves every source file is reachable; no agent write can land outside a resolved workspace | P0 |
| **P2** | **The provider layer** | Trait v2 with `Item::Opaque` and 6-field `Usage` (M-03); **Grok adapter**; prompt caching + the cache-hit alarm (M-04); usage/cost rebuild (M-06); model persistence (M-07); BYOK lifecycle (M-30) | All four providers pass a conformance suite (streaming, tools, parallel tools, structured output, caching, truncation detection); a 5-turn session shows non-zero cache reads from turn 2; unknown model = explicit error | P1, ADR-01-04/05, 02-05, 03-05 |
| **P3** | **The runtime** | Context strategy (M-05); one tool path (M-08); truthful `ToolContext` + a live permission gate (M-09); in-process subagents (M-02); memory in-house (M-10) | A long session's context stays bounded and measurably cached; the recursion guard, ACP identity and build-evidence gate all demonstrably fire; subagents run with no sidecar | P2, ADR-01-06/07/08, 02-03 |
| **P4** | **The fleet** | Event dispatch + exhaustive state enum (M-11); atomic claim (M-12); **Rust-native build verification** (M-13); delivery + handoff (M-15); reconciliation timestamps (M-16); coordination capabilities (M-31…M-34) | A task runs end to end on **all three OSes** and produces a build-verified artifact in the user's chosen workspace; no worker can steal another's task; a stranded task is actually released | P3, ADR-02-01/02/06, 04-01 |
| **P5** | **The graph** | Canonical path + non-destructive upsert + incremental correctness (M-19); graph-scoped context (M-18) | Incremental update preserves cross-file edges and re-clusters; context selection provably narrows the prompt | P3 |
| **P6** | **The UI** | Anchor implementation (M-20); session-lifetime events (M-21); contract reconciliation (M-22); Settings persistence (M-23); Graph City repair (M-24); surface reachability (M-25) | Anchor conformance test passes; no event is lost across a tab switch; every setting survives restart; zero unreachable shipped surfaces | P2, ADR-01-10, 03-03 |
| **P7** | **Experience** | First-run/BYOK flow (M-35); error contract (M-36); progress + cancellation (M-37); destructive-action safety (M-38); coherence (M-39) | **A new user reaches a working agent turn without documentation** | P6, ADR-04-06 |
| **P8** | **SUBMODULE DELETION** *(its own phase, per Razi 2026-08-31)* | Remove `sidecar/` (3,161 LOC), the 29 sidecar-dependent commands, `resources/sidecars`, both `.gitmodules` entries, the MCP HTTP server (ADR-03-02), and the now-dead deps (`ollama-rs`, `dotenvy`, `sysinfo`, possibly `rmcp`/`axum`) | **Entry gate:** all nine ported capabilities pass a test with the sidecar **not running**. **Exit gate:** the tree contains no Python, the installer contains no `.py`, and the app is fully functional | P4, ADR-04-05 |
| **P9** | **Delivery** | Per-platform bundle targets + signing (M-27); the platform `Command` abstraction hardened (M-26); the end-to-end smoke in CI on three OSes (M-28) | A signed, verified installer for Windows, Linux and macOS, each of which installs and runs a task to completion | P8, ADR-03-04, 04-02/03 |

**Two sequencing constraints worth stating explicitly:**
- **P8 cannot start before P4 finishes** — the sidecar is the reference oracle while porting, and its SSE frame parsers are shared between `subagent/bridge.rs` and `sidecar/client.rs`, so those deletions are coupled.
- **M-12 (atomic claim) must land before affinity learning is re-enabled** (M-33), or routing continues to train on corrupted attributions.

---

# PART 7 · Proof & eval

## 7.1 Evidence classes

A claim is only "done" when it carries evidence of the right class. `DOC-CLAIM` never counts.

| Class | Means |
|---|---|
| `CODE-PRESENT` | The code exists and was read |
| `TEST-GREEN` | A test that **can fail** passes |
| `MEASURED` | A number produced by running something |
| `PROVEN-READ` | A test asserts the intended consumer reads it back (INV-8) |
| `PLATFORM-TRIPLE` | Verified on Windows, Linux **and** macOS |

## 7.2 The headline acceptance test

> **A user installs AutoDev on a clean machine of each of the three OSes, enters their own API key in the UI, points it at a repo, types one sentence, and receives a build-verified change in that repo which they can then run — without reading documentation, and without any Python on the machine.**

Every part of that sentence maps to a mechanic: *clean machine* → M-27/M-29 · *enters their own key* → M-30/M-35 · *points at a repo* → M-14 · *one sentence* → M-01 · *build-verified* → M-13 · *in that repo* → M-14/INV-5 · *which they can run* → M-15 · *without documentation* → M-35…M-39 · *no Python* → M-29.

## 7.3 The product proof

Beyond the acceptance test, three things must be demonstrably true:
1. **Cost is honest** — a session's reported cost matches the provider's own billing for that session, within rounding (`MEASURED`).
2. **Context is bounded** — a 30-turn session's input tokens do not grow linearly with turn count (`MEASURED`).
3. **The fleet recovers** — kill a worker mid-task; the task is released and re-run without human intervention (`TEST-GREEN`).

## 7.4 Test families

These harvest 1:1 into `TESTING_GATES.md` under the `dev-anchor-set` skill.

| Family | Asserts | Catches |
|---|---|---|
| **Reachability** | Every `.rs` under `src/` is reachable from the crate root; every `invoke`/`listen` name resolves | `graph_tools`, `db/checkpoints`, the 47 dead commands, the 3 dead events |
| **Provider conformance** | Each of the four: streaming, tools, parallel tools, structured output, caching, usage, truncation detection, error classification | Gemini discarding tool args; `max_tokens` dropped; input tokens reported as 0 |
| **Cache efficacy** | A repeated-prefix session shows non-zero `cache_read` from turn 2, on every provider that supports it | Silent cache misses below the per-model minimum; non-deterministic key order |
| **Reader-exists** (INV-8) | For every persisted artifact, a test reads it back through the intended consumer | `record_api_failure` with no writer; the graph with no reader; `ModelConfigsDb` with no reader; budget config with no reader |
| **Guard-the-guard** | Every safety gate is proven to actually fire | `requires_permission()` never called; the recursion guard inert; the build-evidence gate skipped; the Fix-8c sentinel unreachable |
| **Platform triple** | The end-to-end smoke, on all three OSes | BUG-COMPILE, and the whole `P-01…P-35` portability class |
| **Destructive-safety** | Every destructive path confirms, and is reversible where claimed | One-click `git_discard`; `delete_file` running `remove_dir_all` |
| **Anchor conformance** | Radii, fonts, tokens, class vocabulary, light-only, banned motion | The 12-radii/zero-7px, IBM-Plex, two-dark-palettes state |
| **Migration** | Every ladder applies cleanly from empty and from the live head; a backup exists before any destructive step | The `ADD COLUMN IF NOT EXISTS` v5 syntax error |
| **No-sidecar** | The full suite with the sidecar absent — **the P8 entry gate** | Any residual coupling |

## 7.5 The standing rule

**A gate that cannot fail is not a gate** (INV-4). No `continue-on-error` on any correctness check. Today that single rule would flip most of the project's CI from decorative to real: Rust tests and `cargo fmt` are non-blocking, there is no frontend test framework at all, `npm run lint` runs `tsc` twice under two names, and build smokes skip both pull requests and macOS entirely (`G0 §2`).

# PART 8 · THE ADR LEDGER

**Status: UNANSWERED (0 / 25).** This is the only place decisions are recorded. Spec docs `01–NN` reference these; they never record decision fields.

Some ADRs are marked **BLOCKS CODE** — no implementation may begin on the affected slice until they carry a decision.

---

## Group A — Cross-cutting architecture

### ADR-01-01 — The agent runtime substrate
**The call:** Does the agent loop run as native Rust, or does the app host a vendor SDK?
**Why it matters:** No vendor ships a first-party Rust agent SDK. Hosting the real SDKs means embedding a Node/Python runtime — a sidecar by another name, which is the thing being retired. Native Rust means reimplementing loop semantics against REST.
**Options:** **(A)** Native Rust, conforming to SDK semantics, plus adoption of server-side features over plain HTTP. **(B)** Embedded Node runtime hosting the vendor SDKs verbatim. **(C)** Hybrid.
**Couples with:** ADR-01-05, ADR-01-04, INV-1.
**My recommendation:** **(A)**, and this is already Razi's answer.
**Read first:** `G6` Parts 1–2; `G1 §C.4`.
**▶ RAZI'S DECISION:** **(A) Native Rust, SDK-shaped — and adopt SDK-native/server-side features as far as they help.** *Answered 2026-08-31 at mandate time. Rationale: a second process is the defect being removed, and every server-side feature worth having (caching, context editing, compaction, hosted tools) is plain HTTP and therefore reachable from `reqwest` without the vendor SDK.*

### ADR-01-02 — How `airsctl` reaches the agent runtime — **BLOCKS CODE**
**The call:** `airsctl`'s entire `agent` command group is an HTTP client for the Python orchestrator (`G0 §5.1`). When the sidecar dies, so does it. What replaces the transport?
**Why it matters:** `airsctl` is a **separate process** from the Tauri app. "Move agents in-house" makes the runtime a library inside the desktop binary — which a separate CLI process cannot call. Get this wrong and either the CLI dies or we re-create a local daemon.
**Options:** **(A)** Extract the runtime into a shared `autodev-core` crate that both binaries link; the CLI runs its own in-process agent. **(B)** `airsctl` drives the running desktop app over local IPC. **(C)** Demote `airsctl` to a board/config client only — no agent execution. **(D)** Retire `airsctl`.
**Couples with:** ADR-01-03, INV-1, INV-10.
**My recommendation:** **(A)**. It is the only option that keeps headless CI/automation working (a stated product requirement in the old plan §3) without a daemon, and it forces the runtime to be a clean library rather than something entangled with Tauri state — which is good architecture independently. Cost: a real refactor, since `AgentEngine` currently reaches into `AppState`.
**Read first:** `G0 §5.1`; `src-cli/src/commands/agent.rs`; `G1 §D.1`.
**▶ RAZI'S DECISION:** Do as per your recommendation, since we are now moving the agent in house inside src/tauri which means that the `airsctl` also become an in process agent house itself.

### ADR-01-03 — Scope of `src-cli` and `src-shared`
**The call:** The mandate named three modules. The Cargo workspace has **four** crates. Are `src-cli` (3,508 LOC) and `src-shared` (1,073 LOC) in scope?
**Why it matters:** They are not neutral. `src-shared` hand-mirrors the `tasks` schema and has **already drifted** from `src-tauri` (`G0 §5.4`), violating INV-10. `src-cli` bypasses the OS keychain and takes an API key as a CLI flag (`G0 §5.6`).
**Options:** **(A)** Full scope — refactor both, one schema owner. **(B)** `src-shared` in scope (schema unification), `src-cli` deferred. **(C)** Both out of scope; accept the drift.
**Couples with:** ADR-01-02, ADR-01-09, INV-10.
**My recommendation:** **(A)**. The schema mirror is an active correctness hazard, and the CLI's key handling contradicts the Key Vault promise the app otherwise keeps.
**Read first:** `G0 §5`.
**▶ RAZI'S DECISION:** so the `sec-cli` is the one that makes the AutoDev can run as an MCP server, or in a way the entire app can be run from the cli. `src-shared` is the one commands that is used in multiple src, so yes we do a full scope refacot one schema owner in the best architecture possible.

### ADR-01-04 — The provider abstraction — **BLOCKS CODE**
**The call:** What shape is the `Provider` trait, and how does it absorb the four providers' divergence?
**Why it matters:** The current trait is broken in ways that make three of four providers unusable for agent work: `max_tokens` is silently dropped by OpenAI and Google; Gemini tool-call arguments are **discarded** (`google.rs:313` `let _args = ...`); parallel tool calls take first-only on OpenAI and Google; structured output exists nowhere; **Grok does not exist at all** and the provider factory falls through to a silent `return` inside `tokio::spawn`, leaving a phantom running agent (`G2b P-01…P-30`).
**Options:** **(A)** Rewrite the trait around a normalized OpenAI-shaped tool-call wire format (what LiteLLM does), with per-provider adapters for the divergences. **(B)** Keep the current trait, fix the bugs in place. **(C)** Adopt a Rust multi-provider crate (`rig`, `genai`, `async-openai`) and wrap it.
**Couples with:** ADR-01-05, ADR-01-06, ADR-02-04.
**My recommendation:** **(A) — hand-rolled adapters on `reqwest` + `serde` + SSE behind one trait**, using `async-openai` tactically inside the OpenAI *and* xAI adapters (xAI is OpenAI-wire-compatible with a base-URL override).

`G6 Part 5` settles the crate question: **no crate implements compaction, context editing, or per-model cache minimums.** `genai` (0.7.0-beta.20) covers part of caching; `rig` 0.42.0 has `CacheControl` for Anthropic only. Since all 14 cost-relevant features are plain HTTP, a crate buys little and costs a lot — any multi-provider crate must flatten the divergence table, **and the flattening loses opaque reasoning state, which is invisible in the types and expensive on the invoice.**

Three decisions the trait must make on day one, per `G6 §5.1`:
- **`Item::Opaque { provider, kind, bytes }` from the start.** Four distinct kinds of provider state (Anthropic thinking `signature`, OpenAI `encrypted_content`, Gemini thought signatures, xAI `reasoning_content`) are unreplayable if dropped — and dropping `reasoning_content` is xAI's **number-one cause of cache misses**. Retrofitting this later means re-touching every adapter.
- **Tool-call `args` is `serde_json::Value`, never `String`.**
- **`capabilities()` is keyed by *model*, not provider** — the four providers vary per model, not per vendor.

Sketch: `complete()`, `stream() -> BoxStream<TurnEvent>`, `count_input_tokens()`, `apply_cache_policy()`, `compact()`, `read_rate_limit()`, `classify_error()`; with `CachePolicy::{Off, Automatic{affinity_key}, Explicit{breakpoints, ttl}, Handle{id}}` — the four-variant shape is forced by the fact that **Anthropic is explicit, xAI is fully automatic, and the others differ again**.
**Read first:** `G2b` provider capability matrix; `G6 §5.1` (trait), `§5.2` (divergence table), `§5.4` (crate assessment).
**▶ RAZI'S DECISION:** So we do A as per your recommendation of: `hand-rolled adapters on `reqwest` + `serde` + SSE behind one trait**, using `async-openai` tactically inside the OpenAI *and* xAI adapters (xAI is OpenAI-wire-compatible with a base-URL override).`. In a way creating a single caller system like how LiteLLM does it, with all inputs are there but dropped per provider, for example input schema A is supported in OpenAI but not in anthropic, so we drop A the moment we change Openao -> Anthropic. In a way a single class of multiverse caller for multiple provider rather than a single provider a single class caller which then will create multiple singletons. So ensure the LLM caller is a singleton re-used for multiple provider callers.

### ADR-01-05 — Which server-side features to adopt — **BLOCKS CODE**
**The call:** Razi has said "adopt SDK-native features as much as we can." Which, concretely, and in what order?
**Why it matters:** These are the largest cost levers available, and some carry traps that make them net-negative if adopted naively. Anthropic cache reads are **0.1× base input** and writes 1.25× (5-min) / 2.0× (1-hour) — break-even after a single read (`G6 §1.C.1`). But prefixes below the per-model minimum (512–4,096 tokens) **silently do not cache and return no error**, and any byte change in `tools → system → messages` invalidates that level onward. Compaction changes the billing shape: `usage.iterations[]` appears and top-level `input_tokens` reflects only non-compaction iterations, so naive cost tracking under-reports (`G6 §1.C.5`).
**Options:** The ranked adoption set from `G6 §5.3`. **All 14 rows are plain HTTP — nothing here requires a vendor SDK.** Highest value first:

| # | Feature | Benefit | Coverage |
|---|---|---|---|
| 1 | **Prompt caching + shared-prefix hygiene** | ~90% off the repeated prefix (xAI 75–85%); Anthropic cache reads also don't count against ITPM | **All four.** The largest lever available |
| 2 | **Server-side compaction** | Bounds input growth; xAI's `dropped_message_count` makes the saving measurable | Anthropic, OpenAI, xAI. **Google has none** → client-side fallback |
| 3 | **Anthropic context editing (`clear_tool_uses`)** | Kills the resent-file-dump failure mode directly | Anthropic only; **no equivalent elsewhere** |
| 4 | **Explicit reasoning effort on every request** | xAI defaults to `high`, Gemini to `medium` — **silence is the expensive choice** | All four |
| 5 | **Cheap-tier routing** | `gemini-2.5-flash-lite` $0.10/$0.40 vs Opus 5 $5/$25 — **25–50×** | xAI has no sub-$1 tier |
| 6 | **Server-side token counting**, pre-dispatch | Budget checks before spending; Gemini even breaks out `total_tool_use_tokens` | All four |
| 7 | Server-side state **as a cache-hit feature only** | Gemini + xAI. **OpenAI bills prior tokens regardless** — no saving there | Selective |
| 8 | Route hosted web search to **xAI at $5/1k** | vs Anthropic $10, Google $14 | Cheapest of the four |
| 9–11 | Hosted code execution · Batch at 50% · Files | Batch: Anthropic/OpenAI/Google. **Skip xAI's 20%** | Selective |
| 12–14 | **SKIP** hosted RAG · managed agent runtimes · xAI multi-agent | Re-hosts our runtime in a vendor cloud; per-vendor | — |

**Couples with:** ADR-01-04, ADR-01-06, INV-7.
**My recommendation:** rows 1–4 mandatory in the first build slice; 5–8 in the second; 9–11 opportunistic; 12–14 rejected.

**Two traps that make naive adoption net-negative, and must be designed for, not discovered:**
- **Silent cache misses.** No provider errors when a prefix falls below its minimum (512–4,096 tokens, per *model*); only the usage field reveals it. **Build the cache-hit alarm before the cache.**
- **Non-deterministic JSON key order defeats caching entirely.** Anthropic requires a 100% byte match. One shared prefix builder, `BTreeMap`/`preserve_order`, no exceptions.

**Read first:** `G6 §1.C`, `§2.C`, `§3.C`, `§4.C`, and `§5.3` (the shortlist) + `§5.5` (traps).
**▶ RAZI'S DECISION:** We do all 1 - 8 now. Now for memory, remember i told you we will use the concept of `core-orchestrator` LivingAgent memory of procedural, episodic, semantic. I was thinking we can go either two routes here:
1. We use these providers Agent SDK memory
2. We use an slite or something to store everything and create a semantic fetching system, like how a usual vector RAG works properly. I have created an anchor for this in: C:\AIRS Foundry\autodev\autodev\docs\restructurization\memory-anchor.md. Read it properly.

For me i prefer going to route 2 so we have full control over everything on memory side, and the Personal Assistant already have that schematics and just need to be improved, so the in-house LLM/Agents later on will have the same memory, evolved like the one in the anchor: C:\AIRS Foundry\autodev\autodev\docs\restructurization\memory-anchor.md

### ADR-01-06 — The context strategy — **BLOCKS CODE**
**The call:** Four mechanisms can shrink or shape context — prompt caching, provider context-editing, provider compaction, and our own memory recall. What is their composition and precedence?
**Why it matters:** The old plan contradicts itself (auto-compaction at 80% in §6.1 vs "Replaces message compaction" in §6.3), and the code shipped both `agent/compaction.rs` and `memory/`. The mechanisms interact: **clearing tool results invalidates the cached prefix and forces a cache write** (`G6 §1.C.5`) — so a naive "clear early, clear often" policy can cost more than it saves. Order matters: cache first, then clear, then compact.
**Options:** **(A)** Memory-first: recall replaces history; compaction is a fallback. **(B)** Provider-first: lean on context-editing + compaction; memory supplies only cross-session facts. **(C)** Layered with an explicit precedence and a budget per layer.
**Couples with:** ADR-01-05, ADR-01-07, ADR-01-08.
**My recommendation:** **(C)**, with the precedence: stable cached prefix (system + tools + AUTODEV.md + recalled memory) → live history → `clear_tool_uses` above a threshold tuned so `clear_at_least` exceeds the cache-write cost → compaction only as a long-session backstop. **This ADR is the direct answer to BUG-TOKENS and should not be decided until `G2a` reports the current mechanism.**
**Read first:** `G6 §1.C.1`, `§1.C.5`; `G2a` when it lands; `G4c §4.4`.
**▶ RAZI'S DECISION:** We combine both A and C, so always memory first and then if compaction is needed then we do C.

### ADR-01-07 — Embeddings and semantic recall — **BLOCKS CODE**
**The call:** Semantic memory currently has no embeddings. Do we build vector search in Rust, or accept lexical retrieval?
**Why it matters:** This is a genuine capability decision, not a port detail. `grep -rni "embedding|vector|cosine|faiss|hnsw"` across the Rust crate returns **zero hits** (`G1 §C.4 P3`), and `memory/semantic.rs:166-179` is a SQL `LIKE '%q%'` substring match. Meanwhile the two submodules **disagree with each other**: `core-orchestrator` wires no vector store at all and retrieves via BM25 (`G4b §C.4`), while `memory-mcp-server`'s config declares Qdrant with `text-embedding-3-small` at 1536 dims (`G4c §0`). **There is no existing index to migrate either way.**
**Options:** **(A)** Lexical only — port BM25 + the scoring heuristics; matches what demonstrably runs today; zero new dependencies. **(B)** Local embeddings — `fastembed-rs` + `sqlite-vec`/`usearch`; no API cost, no network, but a model download and a real implementation. **(C)** Provider embeddings — call the provider's embeddings endpoint, store vectors in SQLite; simple, but adds per-write API cost and a network dependency to memory. **(D)** Defer: ship (A), design the schema so (B) can be added without migration.
**Couples with:** ADR-01-06, ADR-01-08, ADR-01-09.
**My recommendation:** **(D)**. The honest finding is that semantic vector recall has **never actually run** in this product; adopting it now is new-feature work wearing a port's clothes. Ship lexical, keep the seam, and let real usage justify the index.
**Read first:** `G4c §0`, `§6`; `G4b §C.4`; `G1 §C.4 P3`.
**▶ RAZI'S DECISION:** I have a guide and an anchor for this, so for te embeddings and semantic recall or the memory architecture we use this: C:\AIRS Foundry\autodev\autodev\docs\restructurization\memory-anchor.md

> **⚠ MY RECOMMENDATION IS OVERTURNED, and correctly so.** I recommended **(D)** — lexical-first, defer vectors — on the grounds that semantic vector recall has never actually run in this product. Razi has adopted `memory-anchor.md`, which specifies **hybrid retrieval (dense vector + BM25 + metadata filtering) with reranking** as the architecture. That is a deliberate capability decision, not an oversight, and it supersedes (D).
>
> Recorded plainly rather than quietly dropped. The substrate and embedding-source questions this opens are **ADR-05-01** and **ADR-05-02**; the scope question is **ADR-05-05**. My (D) reasoning survives only as the phasing argument inside ADR-05-05 — build the fundamentals first — which is also the anchor's own §22 position.

### ADR-01-08 — Memory token budgeting: competitive or per-tier
**The call:** The sidecar budgets the three tiers **competitively** — all chunks are unified into one pool and ranked against each other by a two-stage utility function; a tier's share is emergent from `type_bonus`, `authority` and `session_anchor` (`G4c §4.4`). Do we reproduce that, or assign per-tier quotas?
**Why it matters:** **Any port that assigns fixed per-tier token quotas is changing the behaviour, not porting it.** The competitive model is the sophisticated part of the design and is directly relevant to BUG-TOKENS. It is also considerably more code: two scoring stages, nine weight factors, seven archetype presets, and a greedy marginal-utility loop with a redundancy penalty.
**Options:** **(A)** Port the competitive model faithfully (formulas are fully specified in `G4c §4.4`). **(B)** Per-tier quotas — far simpler, behaviourally different. **(C)** Phase it: ship (B) behind the same interface, upgrade to (A) later.
**Couples with:** ADR-01-06, ADR-01-07.
**My recommendation:** **(C)**. The competitive model's value is unproven *in this product* (the recall path it feeds has its own defects — the episodic cache key ignores the query, semantic recall ignores the query entirely). Ship the simple version, keep the interface, and port the scoring once memory is provably read (INV-8).
**Read first:** `G4c §4.4` (the full formula set), `§5.4`, `§6`.
**▶ RAZI'S DECISION:** We do as per your recommendation, we first ship B in the early phases say before Phase 3. Then based on the testing, usage, and stuff we then ship A later on.

### ADR-01-09 — The application data root
**The call:** `~/.airs/` or `~/.autodev/`?
**Why it matters:** **Both exist in shipped code today.** `src-shared/src/config.rs:13` reads `~/.airs/config.toml`; `src-tauri/src/db/mod.rs:26` and `src-shared/src/db/mod.rs:9` both resolve `~/.autodev/`; `plugin/registry.rs` and the fleet secret live under `~/.airs/` (`G3a SK-101`). A user backing up "their AutoDev data" copies one and silently loses the other.
**Options:** **(A)** `~/.autodev/` everywhere, with a one-time migration from `~/.airs/`. **(B)** `~/.airs/` everywhere. **(C)** Platform-idiomatic dirs (`%APPDATA%`, `~/Library/Application Support`, `$XDG_DATA_HOME`) via the `directories` crate.
**Couples with:** ADR-01-03, ADR-01-11.
**My recommendation:** **(A)** for the app data root, because the majority of shipped code and every database already uses it and the product is named AutoDev. Note **(C)** is the *correct* answer for a shipping desktop app on three OSes and would also fix the `"."` fallbacks found in the updater (`G3c P-10/P-11/P-12`) — but it is a bigger migration. Recommend (A) now, (C) as a named follow-up.
**Read first:** `G0 §4.1`, `G3a SK-101`, `G3c P-10…P-12`.
**▶ RAZI'S DECISION:** We do as per your recommendation

### ADR-01-10 — Tailwind: keep or remove — **BLOCKS CODE** (UI slice)
**The call:** `ui-anchor.md` specifies a bespoke token + `.airs-*` class system. Tailwind 3.4.17 is installed and configured. Do they coexist?
**Why it matters:** They currently fight. `tailwind.config.js` defines a dark "Winter is Coming" palette (`accent: #82AAFF`, `bg-card: #01111D`) while `globals.css` defines a **different** dark palette (`accent: #00FF88`, `bg-card: #1e1e2e`) — so any component mixing Tailwind utilities with `var(--…)` gets two clashing themes on one element (`G5a §9`). **The anchor mandates light mode only**, so both palettes are removal targets. Separately, `prose prose-invert` are no-op classes because `@tailwindcss/typography` is not installed — which is why GFM tables render unstyled.
**Options:** **(A)** Remove Tailwind; pure CSS with the anchor's tokens and `.airs-*` vocabulary. **(B)** Keep Tailwind, reconfigure its theme to *be* the anchor tokens, ban raw `var(--…)` inline styles. **(C)** Keep both with a hard rule about which layer owns what.
**Couples with:** ADR-04-01 (the UI refactor slice).
**My recommendation:** **(B)** if the frontend already leans on Tailwind utilities heavily, **(A)** if it does not — `G5c` is measuring exactly this (files using Tailwind utilities vs `var(--…)` inline styles). Decide when that count lands. **(C)** is what exists today and is the cause of the defect.
**Read first:** `ui-anchor.md`; `G5a §9`; `G5c` design-system section.
**▶ RAZI'S DECISION:** Just ensure the UI is properly following the anchor, so we do as per your recommendation

### ADR-01-11 — Instruction-file identity
**The call:** Is AUTODEV.md the sole instruction system, and where does it live?
**Why it matters:** §6.2 of the old plan is emphatic — *"Not CLAUDE.md. Not AGENTS.md. AUTODEV.md."* — while §8's config tree lists `~/.airs/CLAUDE.md` and a per-repo `CLAUDE.md` "(unchanged)", and §10's build prompt says "CLAUDE.md injection". Shipped code reads `~/.autodev/config/autodev_global.md` and `{repo}/.autodev/autodev.md` (`agent/context.rs:42,54`) — so the code already chose, and the doc is stale.
**Options:** **(A)** AUTODEV.md only, two tiers (global + repo), SQLite-backed with optional FS sync, as §6.2 describes. **(B)** AUTODEV.md plus read-only compatibility with `CLAUDE.md`/`AGENTS.md` if present.
**Couples with:** ADR-01-09.
**My recommendation:** **(A)**, and fold the stale §8/§10 references in the old plan. **(B)** is tempting for interop but dilutes the proprietary-instruction thesis the product is built on.
**Read first:** old plan §6.2 vs §8/§10; `agent/context.rs:34-110`.
**▶ RAZI'S DECISION:** We do A completely and do not do B at all

### ADR-01-12 — Task source of truth
**The call:** Internal SQLite board, or GitHub Issues?
**Why it matters:** The old plan argues at length for the internal board (§4.1: 10–100× faster, no rate limits, offline) and then describes GitHub Issues as "the Command Layer" with a live kanban (§9). Shipped code has `~/.autodev/tasks.db` and a task board UI.
**Options:** **(A)** Internal board is truth; GitHub is an optional export/PR target. **(B)** GitHub is truth. **(C)** Bidirectional sync.
**Couples with:** ADR-02-01.
**My recommendation:** **(A)**. It matches shipped code, the single-machine model, and the offline requirement. **(C)** is a distributed-systems problem nobody asked for.
**Read first:** old plan §4.1 vs §9; `db/tasks.rs`.
**▶ RAZI'S DECISION:** We do A here, internal board is truth, github is an optional one.

---

## Group B — Fleet & execution

### ADR-02-01 — Worker dispatch: polling or events — **BLOCKS CODE**
**The call:** Workers currently run on a 2-minute cron. Keep polling, or move to event-driven dispatch?
**Why it matters:** The cron design has produced two dead-latch defects. `scheduler.rs:227` only fires for status `idle`/`active`/`error`; `run_recipe` writes `fallback_idle` and `mode1_idle_no_provider`, which match neither — so **a worker that ever enters either state never ticks again, even after recovery** (`G3a SK-01`). And a transient SQLite error collapses to `None`, which matches the paused/deleted arm and **permanently kills the schedule task** (`SK-02`). Both are BUG-ASSIGN. Polling also does 4–6 full task-board reads per worker per tick (`SK-33`).
**Options:** **(A)** Event-driven: a task-assigned event kicks the worker directly; keep a slow cron purely as a reconciliation sweep. **(B)** Keep cron, fix the state machine. **(C)** Hybrid with an explicit, exhaustively-matched worker state enum.
**Couples with:** ADR-02-02, INV-9.
**My recommendation:** **(A)+(C)**: event-driven dispatch, with the worker state modelled as a Rust enum that the compiler forces us to match exhaustively — which makes SK-01 structurally impossible rather than fixed-by-vigilance. Keep a low-frequency reconciler for stranded work.
**Read first:** `G3a SK-01, SK-02, SK-33, SK-41…SK-46`; `G3d` when it lands.
**▶ RAZI'S DECISION:** We do A + C as per your recommendation while also fixing the worker and supervisor states as well.

### ADR-02-02 — Build verification: deterministic or model-authored — **BLOCKS CODE**
**The call:** Who runs the build — Rust, or the LLM via a shell string?
**Why it matters:** **This is BUG-COMPILE.** Today every worker recipe instructs the model to run `bash('<cmd> 2>&1 | tee build.log; echo "EXIT:$?"')`. The bash tool runs `cmd /C` on Windows, where `tee`, `$?` and `;` do not exist. `update_task_status` then hard-rejects `qc_review` without recorded build evidence — so **on Windows no fleet task can ever reach QC**; it loops build_failed → ready → build_failed until it exhausts its retry budget (`G3a SK-50`). Related: the Fix-8c sentinel guard that was written to catch this is **unreachable by construction** (`SK-27`), and `Ecosystem::install_cmd()`/`validate_cmd()` are interpolated into a log note and never executed (`SK-26`).
**Options:** **(A)** Rust-native: detect the ecosystem, run the build with `std::process::Command` and an argv the Rust side controls, capture the exit code directly, write `build_evidence`. The model never authors the build command. **(B)** Keep model-authored but supply per-OS command templates. **(C)** Keep model-authored, add a cross-platform shell shim.
**Couples with:** ADR-04-01, INV-6, INV-8.
**My recommendation:** **(A)**, unreservedly. The exit code of a build is a fact, not a judgement — there is no reason a language model should be in that path. This also makes the gate honest: build evidence becomes something Rust observed, not something the model reported.
**Read first:** `G3a SK-26, SK-27, SK-50`; `agent/builtin_tools/bash.rs:89-92`; `G3d` when it lands.
**▶ RAZI'S DECISION:** We do A properly, and only use the LLM if and only if multiple failures occur so the LLM step in as a debug mechanism, so imagine A combined with B but A is the main path, and only do B after say three consecutive failures.

### ADR-02-03 — In-process subagents — **BLOCKS CODE**
**The call:** How do subagents execute once the sidecar is gone?
**Why it matters:** `subagent/bridge.rs` is **100% sidecar** — `create_ephemeral_agent`, `invoke_agent_stream_raw`, `delete_agent` all HTTP, with no in-process path at all. Deleting the sidecar kills `spawn_subagent`, `spawn_parallel_subagents`, `spawn_qc_review`, and Delegate mode outright (`G2b`). Worse, `bridge.rs`'s SSE frame parsers are `pub(crate)` and reused by `sidecar::client::invoke_worker_streaming`, so **the two deletions must be planned together.** Separately, the pool leaks permits: `terminated_at` is set only in `await_result`, so the fire-and-forget Tauri path leaks a permit each time and **after 16 spawns all spawning deadlocks silently** (`G2b S-03`).
**Options:** **(A)** Subagents are `AgentEngine` instances on tokio tasks, with a semaphore-bounded pool and RAII cleanup. **(B)** Subagents share the parent's engine with a scoped context.
**Couples with:** ADR-01-02, ADR-02-01, INV-9.
**My recommendation:** **(A)**, with the permit released by `Drop` rather than by an explicit call — the current bug exists precisely because release is on a path that is not always taken.
**Read first:** `G2b` subagent section; `G1 §C.4 P2`.
**▶ RAZI'S DECISION:** **Subagent capability is in scope and must land natively inside `src-tauri` regardless of what else changes.** *Answered 2026-08-31. Rationale: subagents are load-bearing for the fleet — QC fan-out, parallel work streams, and Delegate mode all depend on them — and they are today the single most sidecar-entangled subsystem in the codebase (`subagent/bridge.rs` is 100% HTTP with no local path). The remaining open question is only the mechanism, for which (A) stands as the recommendation.*  For the mechanism we use A as per your recommendation (READ THIS).

### ADR-02-06 — Scope of the fleet coordination capabilities
**The call:** Escalation, worker↔worker messaging (ACP), dynamic routing/affinity, and epic/phase decomposition all exist and all have defects. Are they ported as-is, completed, or reduced?
**Why it matters:** Razi has stated these are **good capabilities that stay** (2026-08-31), so deletion is off the table — but "ported as-is" would carry known corruption forward. Routing affinity currently **learns from corrupted data** because the task-claim statement never checks `assigned_worker`, so a stolen task leaves `routing_history` attributing the work to the wrong worker (`G3d F-55`). ACP sender identity may be inert because `ToolContext.agent_role` is hardcoded `None` (`G2a T49`). Epics may be assignable to workers as if they were tasks. And it is not yet established whether the escalation queue is ever actually drained.
**Options:** **(A)** Port all four, fix the identified defects, and add the missing reader-proofs (INV-8) before declaring any of them done. **(B)** Port escalation + routing now; defer ACP and epic decomposition to a later phase. **(C)** Port as-is, fix later.
**Couples with:** M-31…M-34, ADR-02-01, INV-8.
**My recommendation:** **(A)**, with one sequencing constraint: **fix the atomic claim (M-12) before turning affinity learning back on**, or the model keeps training on wrong attributions. `G9` is auditing all four now; this ADR should be answered once it lands.
**Read first:** `G9` (in flight); `G3d F-55`; `G2a T49`.
**▶ RAZI'S DECISION:** I want all of those capabilities in the AutoDev application, so we first refine everything on that side which means we do Option A properly and do as per your recommendation.

### ADR-02-04 — The Model Router: build or drop
**The call:** The old plan §6.8 specifies routing by role + priority + budget. It does not exist — model selection is one global UI dropdown (`G2b R-01`).
**Why it matters:** It is a real cost lever (Haiku for cleanup, Opus for supervision), and the product's cost story assumes it. But it is unbuilt, and building it now competes with fixing what is broken.
**Options:** **(A)** Build it as specified. **(B)** Per-worker model only (already in `workers.db`), no priority/budget dimension. **(C)** Drop it; one model, chosen in the UI.
**Couples with:** ADR-01-04, ADR-02-05.
**My recommendation:** **(B)** for the restructure. Per-worker model assignment already exists in the schema and delivers most of the cost benefit; priority- and budget-driven downgrade is a second-order optimisation that should wait until cost accounting is trustworthy (it currently is not — see ADR-02-05).
**Read first:** old plan §6.8; `G2b R-01`; `db/workers.rs`.
**▶ RAZI'S DECISION:** We do as per your recommendation.

### ADR-02-05 — Cost and budget: rebuild the accounting — **BLOCKS CODE**
**The call:** Cost tracking is wrong in several independent ways. What is the corrected model?
**Why it matters:** Every number the user sees is currently unreliable. **(i)** Streaming has no `message_start` handler, so Anthropic `input_tokens` is always 0 — and with the OpenAI/Google equivalents, **input tokens are 0 for all three cloud providers in streaming** (`G2b P-30`). **(ii)** The price table's `_ => (1.0, 3.0)` fallback catches **7 of 11 selectable models**: Gemini 2.5 Flash billed 13× high, free Ollama billed $1/1M, GPT-4.1 under-billed 5× (`C-02`). **(iii)** `cost_today_usd` is never reset — "today" means "since process start" (`G3c IN-50`). **(iv)** The daily rollup sleeps an hour before its first run and only aggregates *yesterday*, so a desktop app used in sub-hour sessions **never completes a single rollup** (`IN-56`). **(v)** `ModelInfo` has only in/out prices — the type cannot represent cache-read/cache-write pricing at all, which blocks ADR-01-05.
**Options:** **(A)** Rebuild: provider-reported usage only, a four-way token split (in / out / cache-read / cache-write), a per-model price table with **no silent fallback** (unknown model = explicit error), and real date handling. **(B)** Patch the known bugs.
**Couples with:** ADR-01-04, ADR-01-05, INV-7.
**My recommendation:** **(A)**. The type itself cannot express what caching needs, so this is not patchable. And an unknown model must fail loudly — a wrong cost number is worse than no cost number.
**Read first:** `G2b P-30, C-02`; `G3c IN-50, IN-56, IN-59…IN-63`.
**▶ RAZI'S DECISION:** We do option A as per your recommendation

### ADR-02-08 — Provider-hosted tools
*(Renumbered from ADR-02-06 on 2026-08-31 — an ID collision with the fleet-coordination ADR above. Both were answered; no decision was lost. See Part 10.)*
**The call:** Do we let agents use provider-hosted tools (web search, code execution, file search)?
**Why it matters:** They are cheap capability with no implementation cost, but they execute **outside** our sandbox and permission model, and they are per-provider, so behaviour differs by which model is serving. Provider-side code execution in particular sits oddly beside a product whose whole point is executing code on the user's machine.
**Options:** **(A)** Adopt web search only. **(B)** Adopt web search + file search; never hosted code execution. **(C)** Adopt none; all tools are ours. **(D)** Adopt all, gated per-worker.
**Couples with:** ADR-01-05, ADR-03-02.
**My recommendation:** **(B)**. Web search is a genuine gap (the sidecar had Tavily; the Rust side has nothing) and file search is useful. Hosted code execution conflicts with the product's identity and with the permission model.
**Read first:** `G6 §1.C.3`, `§2.C.3`.
**▶ RAZI'S DECISION:** We do B as per your recommendation

### ADR-02-07 — Worker persona storage
**The call:** Worker persona and system prompt currently live in YAML **inside the submodule tree** at `{orch}/config/agents/{slug}.yaml`, read and written from Rust (`commands/worker_commands.rs:74,181`). Where do they go?
**Why it matters:** The submodule is being deleted, so these files vanish. `workers.db` exists and seeds 7 fleet workers, but carries no persona.
**Options:** **(A)** Fold persona into `workers.db`. **(B)** Persona lives in the recipe YAML under `~/.autodev/recipes/`. **(C)** Split: identity in the DB, prompt in the recipe.
**Couples with:** ADR-01-11, ADR-01-12.
**My recommendation:** **(C)** — the DB owns identity/model/state (queryable, UI-editable), the recipe owns the prompt (versionable, diffable, shareable). But note `recipes/defaults.rs:45` **overwrites all system recipes on every launch**, silently reverting user edits (`SK-49`) — that must be fixed as part of this, or personas will be un-editable.
**Read first:** `commands/worker_commands.rs:74,181`; `G3a SK-49`; `G1 §C.4 P6`.
**▶ RAZI'S DECISION:** This is an architectural confusion for me, as i prefer to put everything in a single source of truth like how B described it, but is C better in terms of usage and scalability? Explore this and decide based on that consideration of single source of truth and scalability.

---

## Group C — Security

### ADR-03-01 — Secrets and the IPC boundary under BYOK — **BLOCKS CODE**
**The call:** Under BYOK the user types a key into the UI, so there **must** be an inbound path from the webview to the keychain. Is there ever an *outbound* one?
**Why it matters:** `get_provider_key` and `get_github_pat` return raw secrets to the renderer today (`G3c IN-47`). Anything executing in the frontend — a compromised npm dependency, or agent-generated content rendered into the app — can read them, and the CSP permits `connect-src https://api.github.com`, so **exfiltration to a plausible destination is allowed by policy**. Separately `airsctl` accepts an API key as a **command-line flag**, leaking into shell history and the process table (`G0 §5.6`).
**Options:** **(A)** Asymmetric: inbound `set_provider_key` only; **no command ever returns a key**. The UI sees presence/absence, a masked last-4, and a validation result. **(B)** Keep read commands, add a confirmation. **(C)** Status quo.
**Couples with:** ADR-01-03, ADR-03-05, M-30, INV-2, INV-12.
**My recommendation:** **(A)**. BYOK makes this sharper, not looser: the user's own key is the *only* key in the system, so losing it is losing everything they configured. Asymmetry costs nothing — the UI has no legitimate need to read a key back, only to prove one is present and works. Remove the `--api-key` flag in favour of the keychain.
**Read first:** `G3c IN-47…IN-49`; `G0 §5.6`; `G11` BYOK flow.
**▶ RAZI'S DECISION:** We do as per your recommendation

### ADR-03-05 — BYOK key lifecycle and the development credential path — **BLOCKS CODE**
**The call:** Where does a provider key come from at runtime, what validates it, and how do we test against real LLMs during development without creating a runtime backdoor?
**Why it matters:** Razi's direction (2026-08-31): keys are **Bring Your Own Key** — entered by the user in the UI, never shipped. He will also keep a `.env` in the repo for testing real LLM calls during development, excluded from the runtime app. **The danger is precisely the seam between those two facts.** A dev `.env` that silently satisfies a runtime lookup is how a test credential reaches a shipped build — the "presence of a capability is not intent to use it" failure. This is not hypothetical here: `airsctl` today falls back to `ANTHROPIC_API_KEY` from the environment (`G0 §5.6`).
**The sub-decisions:**
1. **Runtime source** — keychain only, or keychain with an env fallback?
2. **Validation** — does the app verify a key works before saving it (a cheap `models` call), or accept it blindly and fail at first use?
3. **Dev path** — how does a developer run against real providers? A cargo feature? A debug-build-only reader? A test harness that never links into the app?
4. **Per-provider vs single** — one key per provider, all four independently configurable, with the UI showing which are live.
5. **Rotation and removal** — the vault has no `list`/enumerate today, so keys for removed providers linger forever (`G3c IN-49`).
**Options:** **(A)** Keychain-only at runtime; the dev `.env` is read **exclusively by an integration-test harness** that is not part of the app binary; validation on save; per-provider. **(B)** Keychain with an env fallback gated on `#[cfg(debug_assertions)]`. **(C)** Keychain with an env fallback gated on a runtime flag.
**Couples with:** ADR-03-01, M-30, M-35, INV-12.
**My recommendation:** **(A)**. Options (B) and (C) both leave a code path in the source whose only purpose is to bypass the credential store — (B) is safer than (C) but still means the shipped and tested binaries differ in exactly the area that matters. A test harness that reads `.env` and drives the provider adapters directly gives the same developer convenience with no runtime surface at all, and it doubles as the provider conformance suite the restructure needs anyway (spec `10`).
**Read first:** `G0 §5.6`; `G3c IN-47…IN-49`; `G11` BYOK flow.
**▶ RAZI'S DECISION:** We do as per your recommendation, just tell me if i need to create a .env or not as i feel it is needed for testing on our side. And the sub-decisions from me if not covered in A:

- Runtime source: Keychain only
- Validation: The app needs to validate the key the moment a user enters it, say a 200 test of streaming or non-streaming with a simple check like hi or something that is fast
- Dev path: the dev `.env` is read **exclusively by an integration-test harness** that is not part of the app binary
- per-provider vs single: per provider key that needs to be tested by the validator
- rotation and removal: do this properly

### ADR-03-02 — The in-process MCP HTTP server — **BLOCKS CODE**
**The call:** Once agents run in-process, does the MCP HTTP server survive?
**Why it matters:** It exists **solely** so orchestrator LivingAgents could call Rust tools over HTTP (`G1 §C.3`). With agents in-process they call `ToolRegistry` directly. Meanwhile it is a serious hole: `/mcp` is mounted with **no auth and no `Origin` validation** on a comment that "localhost-only" is sufficient, and the registry it serves includes `BashTool`, `WriteFileTool`, `EditFileTool` and `SpawnSubagentTool`; the caller also supplies `__working_dir`, choosing the jail root (`G3c IN-06/IN-07`). Any local process, or a web page via DNS rebinding, gets arbitrary command execution.
**Options:** **(A)** Delete the HTTP server; keep ~~`health_monitor.rs` + `port_status.rs` (572 LOC) which serve *external* MCP servers~~ **⚠ CORRECTED 2026-08-31 — `health_monitor.rs` (400 LOC) alone survives. `port_status.rs` (172 LOC) is NOT external-MCP infrastructure**: `port_status.rs:17` is `use super::server::{MCP_SERVER_PORT, current_mcp_port}` — it is own-port diagnostics for 8767 and **dies with the server; keeping it breaks the build** (`G12`). The original claim came from `G1 §C.3` and I propagated it without checking the import. **(B)** Keep it for third-party MCP clients, add Origin validation + a session token + a fixed working dir.
**Couples with:** ADR-02-03, INV-1, INV-2.
**My recommendation:** **(A)**. Nothing else needs it, and its safest configuration is not existing. If exposing AutoDev's tools to third-party clients is later wanted, that is a new feature with its own threat model.
**Read first:** `G3c IN-06, IN-07, IN-32…IN-46`; `G1 §C.3`.
**▶ RAZI'S DECISION:** We do option A for now, we delete it first now, as you said, in the event we need it we build it once all the entire restructure is done rather than in phase of restructure.

### ADR-03-03 — The plugin system — **BLOCKS CODE**
**The call:** Keep, sandbox, or remove?
**Why it matters:** As built it is **arbitrary code execution by design**. `install_plugin_from_yaml` takes a manifest from the frontend and passes `commands.install.<os>` straight to `cmd /C` / `sh -c` with the user's full privileges — no allowlist, no signature, no confirmation, no timeout (`G3a SK-90`). The install directory is an unsanitised `join` on the plugin name, so `name: "../../Documents"` writes there and a later uninstall `remove_dir_all`s it (`SK-83`). The "fleet secret" is generated by an LCG seeded from `unix_now() ^ pid`, making every setup token forgeable (`SK-76`). And `worker-backend`'s uninstall command is `rustup self uninstall -y` — uninstalling an AutoDev *plugin* removes the user's system Rust toolchain, which `update()` also triggers with no rollback (`SK-99`, `SK-85`).
**Options:** **(A)** Remove the plugin system from the restructure scope; ship without it. **(B)** Keep, with a signed-manifest requirement, a path allowlist, an explicit user confirmation showing the exact command, and timeouts. **(C)** Reduce to a declarative environment-check tool that reports what is missing and never installs anything.
**Couples with:** ADR-04-02.
**My recommendation:** **(C)**, with **(A)** as an acceptable answer. The plugin system's actual job is "make sure this worker PC has rustc/node/uv" — that is a *verification* problem, and turning it into an installer is what created every one of these defects. Telling the user "Rust is missing, here is the command" is nearly all the value at none of the risk.
**Read first:** `G3a SK-76…SK-102`.
**▶ RAZI'S DECISION:** We do as per your recommendation.

### ADR-03-04 — Updater trust root and code signing — **BLOCKS CODE** (release slice) · procurement-gated
**The call:** What is the update trust root, and do we buy signing certificates?
**Why it matters:** The updater is currently an RCE path: `verify.rs:90` skips the entire Ed25519 branch when the signature URL is empty; `verify.rs:18` treats an absent checksum as a pass; a network failure fetching the checksum silently downgrades to no verification; and the embedded public key is the literal string `undefined-public-key-for-development-only-placeholder` (53 bytes, not 32) — so **no configuration exists in which a genuine signature verifies** (`G3c IN-01…IN-05`). Separately `signingIdentity: null` and `certificateThumbprint: null` mean macOS Gatekeeper blocks the app and Windows SmartScreen warns.
**Options:** **(A)** Real Ed25519 key with the private half in CI secrets; **absence of a signature is a hard failure**; plus an Apple Developer certificate (~$99/yr) and a Windows code-signing certificate. **(B)** Fix verification only; ship unsigned. **(C)** Disable auto-update entirely for now; manual download.
**Couples with:** ADR-04-03, INV-11.
**My recommendation:** **(A)** for the end state, **(C)** as the interim if certificates are not yet procured. **What is not acceptable is shipping the current state** — an updater that installs unverified binaries is worse than no updater. This has a **procurement dependency and a lead time**, so it needs an answer early.
**Read first:** `G3c IN-01…IN-05, IN-19, IN-21`; `G0 §3.4`.
**▶ RAZI'S DECISION:** We do as per your recommendation

---

## Group D — Platform & delivery

### ADR-04-01 — Cross-platform command execution — **BLOCKS CODE**
**The call:** How is any shell command an agent runs made platform-correct?
**Why it matters:** This is the root class behind BUG-COMPILE and a long tail of others. `bash.rs` runs `cmd /C` on Windows while every recipe and several skills prescribe POSIX syntax (`SK-50`, `SK-70`, `SK-51`). `/tmp` is hardcoded in the QC recipe and the browser-test skill. Hooks have a single non-OS-specific `command` field, so one hook definition **cannot** be portable (`SK-14`, P-07). The plugin catalogue assumes `apt-get`, `brew`, `winget`, and `python3`.
**Options:** **(A)** A `Command` abstraction in Rust: callers express intent (build / test / run), Rust picks the platform-correct invocation; raw shell strings become a last resort with a required per-OS variant. **(B)** Bundle a POSIX shell (busybox/git-bash) on Windows and standardise on sh. **(C)** Per-OS variant fields everywhere, authored by hand.
**Couples with:** ADR-02-02, INV-6.
**My recommendation:** **(A)** for everything structured (builds, tests, probes — which is where the damage is), with **(C)** for genuinely free-form hook/plugin commands. **(B)** is superficially attractive but ships a shell and still leaves `/tmp` and path semantics wrong.
**Read first:** `G3a SK-50, SK-51, SK-70, P-01…P-23`; `G3b` portability table; `G3c` portability table.
**▶ RAZI'S DECISION:** We do as per your recommendation, love it combining everything.

### ADR-04-02 — Bundle targets and the sidecar resource — **BLOCKS CODE**
**The call:** What does the installer contain, and for which platforms does it build by default?
**Why it matters:** `bundle.targets` is `["nsis"]` — **Windows only** — despite dmg/deb/rpm/appimage config and an existing macOS release job (`G0 §3.2`). `resources/sidecars` ships 225 MB of Python including `.env` and `.git` (`G0 §3.1`). And `additionalBrowserArgs` passes `--use-angle=d3d11` — a **Direct3D-only flag** — on every platform (`G0 §3.3`), which is a portability defect and a plausible contributor to Graph City's WebGL failures.
**Options:** **(A)** Remove the sidecar resource, set per-platform targets explicitly, make the GPU flag conditional on Windows. **(B)** As (A), plus move all three platform builds into blocking CI.
**Couples with:** ADR-03-04, ADR-04-03, INV-3.
**My recommendation:** **(B)**. "Works on three OSes" is a stated goal, and an unbuilt platform is an unmet goal — it has to be a gate, not a script someone remembers to run.
**Read first:** `G0 §3`.
**▶ RAZI'S DECISION:** We do as per your recommendation

### ADR-04-03 — Test strategy and which gates block — **BLOCKS CODE**
**The call:** What tests exist, and which of them can fail the build?
**Why it matters:** Today **nothing in CI can fail except `cargo clippy` and `tsc`.** Rust tests and `cargo fmt` are `continue-on-error`; there is **no frontend test framework at all**; `npm run lint` is `tsc --noEmit` wearing an ESLint label; build smokes skip PRs entirely and skip macOS entirely (`G0 §2`). A restructure that does not fix this cannot *prove* it fixed anything else — which makes this the gate on every other claim in this plan.
**Options:** **(A)** Rust tests blocking; add Vitest for frontend units; add ESLint for real; add a Playwright/WebDriver smoke that launches the app and drives one end-to-end task; all three platforms build on PR. **(B)** Make existing tests blocking, defer new frameworks. **(C)** Status quo plus discipline.
**Couples with:** every other ADR, INV-4, INV-8.
**My recommendation:** **(A)**, sequenced: make what exists blocking first (cheap, immediate), then add the frontend framework, then the end-to-end smoke. The end-to-end smoke is the one that matters most — it is the only thing that would have caught BUG-COMPILE, BUG-DELIVER and BUG-CHAT, all of which pass every check the project currently runs.
**Read first:** `G0 §2`; `.github/workflows/ci.yml`.
**▶ RAZI'S DECISION:** We do option A as per your recommendation

### ADR-04-04 — Redis and the warm tier
**The call:** Does anything replace the Redis warm cache?
**Why it matters:** The sidecar hard-fails without Redis in some paths, ships a Memurai service starter for Windows, and stores session history (7-day TTL), continuity packets, MCP prompts, agent cards and Fernet-encrypted secrets there. **But no `redis` crate is in `Cargo.toml`** and no Rust code reads Redis — the only contact is a raw-TCP liveness PING (`G1 §C.4`, `G4b §C.1`). A stray `src-tauri/dump.rdb` (175 KB) is committed alongside source.
**Options:** **(A)** Nothing replaces it — the hot LRU plus SQLite is the whole story for a single-process desktop app. **(B)** A local KV (`redb`/`sled`) for the TTL'd data.
**Couples with:** ADR-01-08, ADR-03-01.
**My recommendation:** **(A)**. A second cache tier exists to share state between processes; after the restructure there is one process. Delete `dump.rdb`, the Memurai starter, and both Redis setup docs.
**Read first:** `G4b §C.1`; `G1 §C.4`; `G3c IN-87`.
**▶ RAZI'S DECISION:** Look what i am worried here is that Memurai can only works on windows not on macbook and linux, so just research this properly and you call the shots. Hell we can even migrate to sqlite as well properly for memory as per defined in the anchor of memory: C:\AIRS Foundry\autodev\autodev\docs\restructurization\memory-anchor.md

### ADR-04-05 — Submodule deletion: its own phase, and its ordering
**The call:** Submodule removal is a **named phase of the programme** (Razi, 2026-08-31), not a cleanup step. Where does that phase sit, and what gates entry to it?
**Why it matters:** `subagent/bridge.rs`'s SSE frame parsers are `pub(crate)` and **shared with** `sidecar::client::invoke_worker_streaming` (`G2b`), so those two deletions are coupled and cannot be sequenced independently. Deleting too early strands nine capabilities (`G1 §C.4`); deleting too late leaves a 225 MB bundle resource and 29 sidecar-dependent commands (`G3d`) in every build. *(The credential concern that originally attached to this ADR is closed: Razi removed `core-orchestrator/.env` on 2026-08-31 and confirms it was never committed. INV-3 and INV-12 are what stop it recurring.)*
**Options:** **(A)** Port first behind a flag, run both paths, then a dedicated deletion phase. **(B)** Delete first, rebuild against the gap. **(C)** Port and delete per capability, in dependency order.
**Couples with:** ADR-02-03, ADR-03-02, M-29.
**My recommendation:** **(A)**, with the deletion phase gated on a single explicit criterion: **every one of the nine ported capabilities has a passing test that exercises it with the sidecar not running.** Keeping the sidecar as a reference oracle during the port is nearly free; the deletion then becomes a config change, a `git rm`, a `.gitmodules` edit, and the removal of `sidecar/` (3,161 LOC) plus the 29 dependent commands.
**Read first:** `G1 §C.4`; `G2b` subagent section; `G3d` command inventory.
**▶ RAZI'S DECISION:** We do as per your recomendation, just ensure at the end the sidecar and submodule is deleted and not a single code of it survives or exists.

### ADR-04-06 — The scope and standard of the experience workstream
**The call:** Razi has added *"refine the user experience, making everything as smooth as possible for the user and intuitive"* as a goal. What is in scope, and what is the bar?
**Why it matters:** Without a definition this expands without limit or gets dropped under pressure. It also now has a hard dependency: **BYOK makes the first-run flow load-bearing** — under BYOK an unconfigured app does nothing at all, and today the only route to entering a key is several clicks deep in Settings, while opening Chat first produces a blank bubble and destroys the typed message (`G5a §7`). Separately ~3,700 LOC of built UI is unreachable from any route (`G5c F08`), which is both a UX problem and a scope question.
**Options:** **(A)** Scope to five outcomes — first-run to first successful turn; every failure actionable; honest progress and cancellation; destructive actions confirmed; every shipped surface reachable or deleted. Bar: a new user reaches a working agent turn without documentation. **(B)** Fix only the defects that block a task, defer the rest. **(C)** Full redesign alongside the anchor implementation.
**Couples with:** M-35…M-39, ADR-01-10, ADR-03-03, ADR-03-05.
**My recommendation:** **(A)**. It is testable — "a new user reaches a working agent turn without documentation" is an acceptance test, not a sentiment — and it composes with the anchor implementation (M-20) rather than competing with it. **(C)** would swallow the restructure.
**Read first:** `G11` (in flight); `G5c F08`; `G5a §7`.
**▶ RAZI'S DECISION:** We do as per your recommendation, while restructuring you will traverse the code which means at runtime you can find broken user experience as well to be refined.

---

---

## Group E — Raised by the answering pass (2026-08-31)

Razi answered all 29 ADRs on 2026-08-31 and introduced a new reference: `docs/restructurization/memory-anchor.md`. Reading his answers against that anchor and against the shipped platform opens five genuinely new decisions and closes four he delegated to me.

### ADR-05-01 — The memory substrate on a desktop — **BLOCKS CODE**
**The call:** The memory anchor §21 recommends **PostgreSQL + pgvector** as the initial architecture. AutoDev is a single-user desktop app that ships as an installer. What is the substrate?
**Why it matters — this is a direct conflict between two governing documents.** The entire restructure exists to **delete a bundled server process** (INV-1). Bundling PostgreSQL would recreate precisely the defect being removed — a second process, a port, a lifecycle, a watchdog, a platform-specific install story, and 200 MB of installer. The anchor is written as general architecture guidance, not as a desktop constraint; its §21 explicitly says *"The architecture should remain modular enough that the retrieval backend can later be replaced."* Razi anticipated this himself in ADR-04-04: *"we can even migrate to sqlite as well properly for memory as per defined in the anchor."*

**The desktop-correct instantiation of the anchor is available and complete** (`CODE-PRESENT` / verified 2026-08-31):

| Anchor requirement (§9) | Desktop instantiation | Evidence |
|---|---|---|
| Structured metadata + relational queries | **SQLite**, already statically linked | `src-tauri/Cargo.toml:41` — `rusqlite 0.31` with `features = ["bundled"]`; no system dependency |
| Dense vector search | **`sqlite-vec`** — the Rust crate embeds the C source and **statically links at build time** via `cc`. No extension loading, no runtime dependency | [sqlite-vec Rust](https://alexgarcia.xyz/sqlite-vec/rust.html) |
| Keyword / BM25 search | **FTS5**, built into SQLite | — |
| Metadata filtering | Plain SQL columns | — |

That is the anchor's exact trifecta — *"Dense Vector Search + Keyword/BM25 Search + Metadata Filtering"* — **in one file, with no server, no port, and no second process.**

**Options:** **(A)** SQLite + `sqlite-vec` + FTS5, single file, statically linked. **(B)** Bundle PostgreSQL + pgvector as the anchor literally specifies. **(C)** SQLite now, with a backend seam so (B) is possible for a future hosted/team edition.
**Couples with:** ADR-01-07, ADR-04-04, ADR-05-02, INV-1.
**My recommendation:** **(A), implemented as (C)** — the anchor's *principles* adopted in full, its *storage recommendation* re-instantiated for the platform, and the retrieval backend kept behind a trait exactly as the anchor asks. **(B) is not viable**: it violates INV-1 and would be the third time this product bundled a server it did not need.
**Read first:** `memory-anchor.md` §21–22; `G1 §F.2`; ADR-04-04.
**▶ RAZI'S DECISION:** **(A), implemented as (C) — SQLite + `sqlite-vec` + FTS5, behind a retrieval-backend trait.** *Answered 2026-08-31. The anchor's principles are adopted in full; its storage recommendation is re-instantiated for the platform. `rusqlite` is already statically bundled and `sqlite-vec` compiles into the binary, so the anchor's dense-vector + BM25 + metadata trifecta ships as one file with no server, no port and no second process — satisfying INV-1. The backend stays behind a trait exactly as `memory-anchor.md` §21 asks, leaving PostgreSQL viable for any future hosted edition.*

### ADR-05-02 — The embedding source under BYOK — **BLOCKS CODE**
**The call:** Where do embedding vectors come from — a local model, or a provider API?
**Why it matters — BYOK and provider embeddings do not compose.** **Anthropic ships no embeddings endpoint at all**; it recommends Voyage AI, a separate vendor with a separate key ([Claude docs](https://platform.claude.com/docs/en/build-with-claude/embeddings)). So under BYOK with our four providers:
- a user who configures **only Anthropic** — a completely reasonable choice — would have **no embeddings at all**, and semantic memory would silently degrade or fail;
- making it work would require a **fifth API key** from a vendor not in the four Razi named;
- and every memory write would incur an API call and a network round-trip, making memory unavailable offline.

**The local option is mature:** `fastembed-rs` runs local ONNX inference, is **synchronous with no Tokio**, downloads its model **once and then runs offline**, and ships both dense (BGE-small-en-v1.5) and **sparse SPLADE** embeddings — the latter being directly useful for the anchor's hybrid retrieval ([fastembed-rs](https://github.com/anush008/fastembed-rs)).

**Options:** **(A)** Local embeddings via `fastembed-rs`. **(B)** Provider embeddings (OpenAI/Gemini only, with Anthropic-only users degraded to lexical). **(C)** Local by default, provider optionally.
**Honest cost of (A):** a one-time model download (~130 MB for BGE-small) on first use, which lands in the first-run experience (M-35) and must be handled as a visible, resumable, skippable step — not a silent stall.
**Couples with:** ADR-05-01, ADR-03-05 (BYOK), M-35, INV-12.
**My recommendation:** **(A)**. It is the only option that keeps memory working for all four providers, works offline, costs nothing per write, and adds no key. It also keeps the user's own API budget for inference rather than bookkeeping.
**Read first:** `memory-anchor.md` §9; the Anthropic embeddings note above.
**▶ RAZI'S DECISION:** ~~**(B) — provider embeddings via OpenAI `text-embedding-3-small`.** *Answered 2026-08-31, against my recommendation.*~~ **RETRACTED by Razi on 2026-08-31, same day, before any downstream work depended on it.**

**▶ RAZI'S DECISION (final):** **(A) — local embeddings via `fastembed-rs`, model BGE-small-en-v1.5 (~130 MB).** *Answered 2026-08-31. Rationale: it removes the BYOK coupling entirely — no user is forced to hold an OpenAI key to get semantic memory on a Claude-only or Gemini-only setup — and memory writes stop depending on the network or on the user's inference budget.*

The struck answer is kept visible rather than deleted, per the append-only rule.

**What the retraction changes — one of these is a correction to a number I had already propagated:**

| | Under the struck OpenAI answer | Under the final local answer |
|---|---|---|
| **Vector dimension** | 1536 | **⚠ 384** — BGE-small-en-v1.5 is a 384-dim model. The `sqlite-vec` column width changes accordingly, and **the "matches the retired memory-mcp-server's 1536" continuity note no longer applies** (it was never load-bearing — `G4b §C.4` establishes there is no existing index to migrate). Corrected here and in spec `03 §4.4`. |
| **Required keys** | OpenAI mandatory for semantic memory | **None.** Works on any provider, or none |
| **Network** | Every memory write | **First-run download only**, then fully offline |
| **Per-write cost** | Small, billed to the user's key | **Zero** |
| **First-run** | Nothing extra | **A ~130 MB model download** that must be visible, resumable and skippable (M-35) — skipping degrades to lexical rather than blocking |

**Three consequences that remain design constraints:**

1. **The model download is now a first-run surface.** It is the one heavyweight step in an otherwise light onboarding, and under ADR-04-06's standard it must show progress, survive interruption, and let the user proceed without it.
2. **Degradation still matters, for a different reason.** Before the model is present — during download, or if the user skipped it — retrieval falls back to **FTS5/BM25 + metadata filtering**, two of the anchor's three legs (§9). Memory works, less well, and says so.
3. **`fastembed` is synchronous and Tokio-free.** Inference must run on a blocking thread pool (`spawn_blocking`), never on the async runtime, or it stalls every other task on that worker. Embedding stays **asynchronous with respect to a turn** — a write enqueues and embeds behind it.

**One honest limitation to note:** BGE-small-en-v1.5 is English-centric. Recall quality on non-English content will be lower than a multilingual model would give. The retrieval backend is behind a trait (ADR-05-01) and the model is a config value, so this is a swap, not a redesign, if it ever bites.

### ADR-05-03 — The scope model, and "working memory" as a fourth type
**The call:** The anchor specifies memory **scopes** (Global → user/org/agent · Project → architecture/conventions/dependencies/decisions · Task → findings/decisions/failures/progress) and a **fourth memory type, "working memory"** (§4.4), which the current three-tier model does not have. Do we adopt both?
**Why it matters:** The anchor is emphatic that *"Scope is critical because it prevents irrelevant memories from contaminating retrieval"* — and contamination is a defect we already found: sidecar episodic recall **never forwards `agent_id`** despite a documented defence, so recall is unscoped by agent (`G4c §5.4.5`), and procedural recall has no type filter at all, so cross-tier bleed is by construction. Scope is the structural fix for a class of defect we have already observed.

Working memory is different: the anchor says it *"should generally remain temporary and highly task-focused"* and *"does not necessarily belong in long-term memory"*. That is arguably **not a memory tier at all** — it is the agent's live context plus the task row, both of which already exist.
**Options:** **(A)** Adopt scopes as first-class columns; treat working memory as the existing session context + task record, not a persisted tier. **(B)** Adopt both as four persisted tiers. **(C)** Scopes only, defer the question.
**Couples with:** ADR-01-08, ADR-05-04, INV-8.
**My recommendation:** **(A)**. Scopes earn their place immediately — they fix an observed contamination defect and they are cheap (columns + a filter). A fourth persisted tier for data that is by definition transient would add a lifecycle to manage with no reader that needs it.
**Read first:** `memory-anchor.md` §4.4, §7; `G4c §5.4.4-5`.
**▶ RAZI'S DECISION:** **(C) — adopt scopes now; leave "working memory" formally open.** *Answered 2026-08-31. Scopes become first-class columns and a mandatory retrieval filter, because they are cheap and they structurally close an observed contamination defect (recall never forwards `agent_id` despite a documented defence for it, `G4c §5.4.5`). Whether working memory becomes a persisted fourth tier is deferred, not dropped.*

**Deferred-item discipline:** the schema is designed so a fourth tier can be added **without a migration** — the tier discriminator is a value, not a table. This costs nothing now and means the deferral cannot become a trap later. Tracked as **OQ-7** (Part 9) so it does not drift into "never decided".

### ADR-05-04 — The Memory Gateway and its write-permission boundary
**The call:** The anchor §16 puts all memory access behind a **gateway** (validation, dedup, embedding, ranking, consolidation, lifecycle), and §17 proposes a **permission boundary**: a read-only agent gets `search_memory` / `get_memory` / `list_memories`, while *"a trusted memory-management component"* additionally gets `store_memory` / `update_memory` / `delete_memory`. Do we adopt that split?
**Why it matters:** Today the agent has `remember_this` — an **unrestricted write** into its own long-term memory. Combined with the anchor's §18 selectivity criteria (importance, novelty, duplicate check, confidence) being absent, an agent can write whatever it likes, as often as it likes, with no evaluation gate. That is how a memory store fills with duplicates and low-value observations — which the anchor then needs consolidation (§12) to clean up. The gateway is the place where the anchor's own lifecycle (§5: candidate → evaluate → store/discard) is enforced.
**Options:** **(A)** Adopt the split: agents get read + *propose*; a deterministic gateway evaluates candidates and decides what is stored. **(B)** Agents write directly through a validating gateway. **(C)** Status quo.
**Couples with:** ADR-05-03, the det↔organ doctrine (Part 2), INV-8.
**My recommendation:** **(A)**. It puts the evaluation gate somewhere deterministic and testable, and it matches Part 2's doctrine — the *judgement* of what was learned is an ORGAN call; the *decision to persist* is `DET` policy over importance, novelty and duplication.
**Read first:** `memory-anchor.md` §5, §16, §17, §18.
**▶ RAZI'S DECISION:** **(A) — agents read and propose; a deterministic gateway decides what is stored.** *Answered 2026-08-31. The agent's tool surface becomes `search_memory` / `get_memory` / `list_memories` plus a `propose_memory`; `remember_this`'s unrestricted write is removed. The gateway applies the anchor's §18 evaluation — importance, novelty, duplicate check, confidence — before anything is persisted, and that evaluation is `DET` policy, not a model call. This is the anchor's §17 permission boundary and it matches Part 2's doctrine: judging what was learned is an ORGAN act; deciding to persist is not.*

### ADR-05-05 — How much of the anchor lands inside the restructure
**The call:** The anchor is a **six-phase programme** (§22: basic → retrieval → quality → consolidation → autonomous → advanced) with its own evaluation suite (§23: Recall@5/10, MRR, NDCG, plus agent-level metrics). That is materially more than porting the sidecar's three tiers. What is in scope for *this* restructure?
**Why it matters:** This is the scope question that decides whether the restructure ships. The anchor itself is disciplined about this — *"Only after the fundamentals work should the system explore..."* (§22 Phase 6) — so phasing it is faithful to the document, not a dilution of it. But left unstated, "use the anchor" silently expands the memory workstream from a port into a research programme, and R-1 (the register swallowing the restructure) becomes real.
**Options:** **(A)** Anchor Phases 1–3 in the restructure (basic memory, hybrid retrieval + reranking, quality: importance/confidence/recency/dedup); Phases 4–6 after. **(B)** Phases 1–4, adding consolidation and contradiction handling. **(C)** All six.
**Couples with:** ADR-01-08, ADR-05-01…05-04, R-1.
**My recommendation:** **(A)**, with one addition from Phase 4 pulled forward: **provenance** (§14). It is cheap — a source column — and it is what makes contradictions *resolvable later* rather than requiring a migration to add. Everything else in Phases 4–6 is deferred with a named follow-up.

Note this composes with Razi's ADR-01-08 answer (ship simple per-tier budgeting first, competitive ranking later): the anchor's §10 reranker **is** the competitive model, so the two decisions agree — simple first, the anchor's scoring in Phase 2.
**Read first:** `memory-anchor.md` §22, §23; ADR-01-08.
**▶ RAZI'S DECISION:** **(A) — anchor Phases 1–3, plus provenance pulled forward from Phase 4.** *Answered 2026-08-31. In scope: basic memory (store/search/get/list), hybrid retrieval with reranking, and quality (importance, confidence, recency, dedup) — plus `source` provenance, because it is one column and it is what makes contradictions resolvable later without a migration. Phases 4–6 (consolidation, contradiction resolution, autonomous memory creation, knowledge graphs, multi-hop retrieval) are deferred with a named follow-up. This composes with ADR-01-08: simple per-tier budgeting ships first, and the anchor's §10 reranker — which is the competitive model — arrives in Phase 2.*

**The anchor's evaluation suite (§23) is in scope for what ships.** Recall@5/@10 and MRR over a fixed memory corpus become gates in spec `10`. The agent-level metric the anchor calls out — *"number of rediscovered facts"* — is the honest measure of whether memory is doing anything, and it is worth one dashboard line.

### ADR-05-06 — The MCP client: build it, or defer it — **BLOCKS CODE** (agent core)
**The call:** OQ-6 was answered "keep the MCP client" on the premise that one exists and works. **It does not exist** (`G12`, evidence in Part 9 §OQ-6). So the real question is: build it now, or defer it?
**Why it matters — and why this is a stop-and-ask, not a scoping detail.** Making third-party MCP tools reachable by an agent is not "wire up a client". It requires a change to the **agent core path**:

> **`agent/tool_registry.rs:18` is `register(&mut self)`, and every registry in the app is frozen immediately after construction** — `engine.rs:59-61` moves it into the struct; `main.rs:108-110` and `server.rs:150-152` wrap it in `Arc::new` with no interior mutability. **There is no dynamic tool-registration path anywhere.** Even a flawless MCP client could not surface a tool to an agent.

Making the registry runtime-mutable means an `RwLock`, re-plumbing `AgentEngine` to a shared `Arc`, and introducing a namespaced `McpProxyTool`. That touches the file the entire runtime rests on, during a restructure whose own thesis is that the runtime is the one part that currently works (`G1 §D.1`).

There is also **no permission story for untrusted third-party tools** — no precedent exists in the codebase, and `Tool::requires_permission()` is never called even for our own tools (`G2a T47`). Admitting arbitrary third-party tools before the permission model is real would be adding an attack surface to a system whose existing one we are in the middle of closing (ADR-03-02).

**Honest assessment of what survives to help:** 484 Rust LOC (`health_monitor.rs` 400, `github_mcp_commands.rs` 84) contribute a config idiom, an event contract and a health badge — roughly **15–20% of the surface and 0% of the protocol**. And `health_monitor.rs` should be **rewritten, not ported**: its probe model is wrong for MCP (a `GET /health` that counts 404 as success) and it carries the eight defects in `G3c IN-38…IN-45`.

**Options:** **(A)** Defer — remove the false UI claim at `Settings.tsx:653`, keep the config surface dormant, and build the client as a post-restructure feature with its own threat model. **(B)** Build it inside the restructure, accepting the registry refactor and a new permission model. **(C)** Build only the registry mutability now (so the seam exists), defer the client.
**Couples with:** ADR-03-02, ADR-03-03, M-08, M-09, INV-2.
**My recommendation:** **(A)**, with one thing done immediately regardless: **delete the false claim in the UI.** Telling a user the app "connects to the GitHub MCP server" when nothing spawns it is the same class of defect as the hooks that show ENABLED and protect nothing — and under ADR-04-06's standard, a control that lies is worse than an absent one.

**(C)** is a defensible middle path but I would not take it: an unused `RwLock` on the hot tool path is a cost paid every turn for a capability nobody can reach yet, and the seam is easy to add later precisely *because* the registry is small.

**A trap for the "not one line of sidecar code survives" rule:** 4 of the 7 surviving MCP items live inside **sidecar-named files and directories** — `commands/sidecar_commands.rs:579-599`, `src/components/sidecar/McpHealthPanel.tsx`, `src/stores/sidecarStore.ts:203-309`. **Whatever is kept must be moved out by address**, or sidecar code survives P8 by filename alone.
**Read first:** `G12` (full report); `G3c IN-38…IN-45`; `agent/tool_registry.rs:18`.
**▶ RAZI'S DECISION:** **(A) — defer the client; delete the false claim immediately.** *Answered 2026-08-31. `Settings.tsx:653`'s statement that AutoDev "Connects to the GitHub MCP server (`@modelcontextprotocol/server-github`)" is removed in the UI slice (P6), because a control that lies is worse than an absent one — the same standard ADR-04-06 applies to the hooks that show ENABLED and protect nothing. The config surface may remain dormant. `ToolRegistry` stays immutable, so no cost is paid on the hot tool path for an unreachable capability. The client is built after the restructure, with its own threat model and a permission story for untrusted third-party tools — which does not exist today, since `Tool::requires_permission()` is never called even for our own.*

**Carried into P8 as a hard requirement:** of the seven MCP items that survive, **four live inside sidecar-named files and directories** — `commands/sidecar_commands.rs:579-599`, `src/components/sidecar/McpHealthPanel.tsx`, `src/stores/sidecarStore.ts:203-309`. They must be **relocated by address**, or sidecar code survives the deletion phase by filename alone, violating Razi's "not a single line survives" condition on ADR-04-05.

---

## Group F — Decisions Razi delegated to me, now made

Recorded here as decisions, with the reasoning, per Razi's instruction to call them.

### ADR-02-07 — Worker persona storage — **DECIDED: (C)**
Razi: *"I prefer to put everything in a single source of truth like how B described it, but is C better in terms of usage and scalability? Explore this and decide."*

**The finding that decides it: these are two kinds of data with different lifecycles, and forcing them into one store makes one of them wrong.**

Worker **state** — status, current task, last heartbeat, claiming PID — changes every few seconds and must be *queryable* ("which workers are idle?"). Worker **prompt** changes rarely and its whole value is that it can be diffed, versioned and shared.

Under **(B)** (everything in recipe YAML): rendering the worker list means parsing N YAML files; every status change means **rewriting a versioned file**, which destroys the diffability that was the reason to choose YAML. Scalability is worse on both axes, and the "single source of truth" benefit is illusory because it forces mutable runtime state into a version-controlled artifact.

Under **(C)**: `workers.db` owns identity, model and state; the recipe file owns the prompt; `workers.db` already carries a `recipe_path` **reference**, not a copy.

**Razi's underlying concern is right, and (C) satisfies it — the rule is one owner per *fact*, not one store per subsystem.** That is exactly INV-10, and the defects we found (`app_settings` with three creators, the `src-shared` schema mirror) are violations of *that* rule, not arguments for a single store. Neither store duplicates the other; no fact lives in two places.

**Binding condition:** `recipes/defaults.rs:45` overwrites all system recipes on every launch, silently reverting user edits (`SK-49`). **Personas are un-editable until that is fixed**, so it is part of this decision, not a separate defect.

### ADR-04-04 — Redis and the warm tier — **DECIDED: (A), delete entirely**
Razi: *"Memurai only works on Windows not on macbook and linux, so research this properly and you call the shots."*

**Razi's concern is correct and it strengthens the case.** Memurai is a commercial Redis-for-Windows port; there is no equivalent bundled story for macOS or Linux that does not mean shipping a server process — which violates INV-1, the invariant the whole restructure exists to establish.

But the decisive evidence is that **Redis is already not a dependency of the Rust application**: there is no `redis` crate in `src-tauri/Cargo.toml`, and no Rust code reads or writes Redis. The only two touchpoints are a raw-TCP `PING` liveness probe and a Windows-service starter — both of which exist solely to babysit the *Python* sidecar (`G1 §C.4`, `G4b §C.1`).

A warm tier exists to share state **between processes**. After P8 there is one process. And with ADR-05-01 putting memory in SQLite, there is nothing left for it to hold.

**Deleted:** the `dump.rdb` committed snapshot (175 KB), the Memurai starter, the TCP probe, and both Redis setup documents. **Nothing replaces it.**

### OQ-5 — Retention — **DECIDED**
Razi: *"you decide based on the restructure, drop whatever is not needed."*

| Table | Policy |
|---|---|
| `token_usage` | 90 days raw, then rolled into `daily_costs` and pruned |
| `daily_costs` | **Indefinite** — one row per day is trivially small and it is the cost history |
| `worker_stats` | 90 days raw, then rolled up |
| `system_events` | 30 days |
| `api_failures` | 30 days — **but wire the writer or drop the table.** It has zero writers today (`IN-53`) while the dashboard reads it. Under BYOK it becomes genuinely valuable (rate limits, invalid keys, quota exhaustion are now the *user's* problem to see), so the recommendation is **wire it** |
| `hook_logs` | 7 days **or** 10,000 rows, whichever comes first |
| Memory tables | **No automatic deletion.** The anchor §11 is explicit: *"Old memories should generally be deprioritized before they are destroyed."* Decay affects ranking, not existence; consolidation (Phase 4) removes duplicates |

Plus a scheduled `VACUUM`, which exists nowhere today.

### ADR-03-05 — "Do I need to create a `.env`?" — **YES**
Razi's sub-answers (keychain-only runtime · validate on entry with a fast live call · dev `.env` read only by the test harness · per-provider keys · proper rotation) are all consistent with option (A) and are adopted as written.

**Yes, create one** — at the repo root, which is **already gitignored** (`.gitignore:54`, verified). It is read **only** by the integration-test harness, never by the app binary, and `G-KEY-NOENV` (spec `02 §11`) enforces that: with the keychain empty and every provider variable set, the app must **refuse to run a turn**.

```
ANTHROPIC_API_KEY=…
OPENAI_API_KEY=…
GEMINI_API_KEY=…
XAI_API_KEY=…
```

Your validation sub-answer is right and slightly cheaper than you proposed: rather than a streaming "hi", the cheapest sound check is the provider's **models-list or token-count endpoint** — it proves the key, costs no completion tokens, and returns in well under a second. Where a provider lacks one, a 1-token completion is the fallback.

---

---

## Group G — Post-review (adversarial pre-dev gap review, 2026-08-31)

Three fresh-context reviewers read the suite against the shipped code: **R1 buildability** (23 findings), **R2 contradictions** (47), **R3 gaps & policy** (34). **104 findings, 39 proposed ADRs**, consolidated below. Reports in `_review/`.

The review overturned several of the plan's own claims. Those are corrected in place, per the append-only rule.

### G-01 — Fault isolation after the process boundary is removed — **ANSWERED**
**The call:** `[profile.release] panic = "abort"` (`Cargo.toml:126-131`) plus in-process workers and subagents means **one panic kills the app and every session**. Ten specs never mention fault isolation (`R1 B-04`).
**Why it matters:** I framed the sidecar purely as a defect. It was also a **process boundary**, and INV-1 removes it without replacing what it did. The audit catalogued many reachable panics — three byte-slice truncations, poisoned mutexes in Tauri commands, `.unwrap()` on user input. Today one kills a Python process; after the restructure it kills AutoDev.
**▶ RAZI'S DECISION:** **Catch-unwind per agent turn.** *Answered 2026-08-31. Razi first asked whether deleting the sidecar and "that panic" would suffice; clarified that `panic = "abort"` is a compiler setting in our own `Cargo.toml`, not sidecar code, and that panics cannot be eliminated as a class — only specific ones fixed. Decision: release profile moves to `panic = "unwind"`, and each agent turn and subagent runs inside `catch_unwind`, so a panicking worker fails **that turn** and reports it while the app and other sessions survive. New mechanic **M-41**.*

### G-02 — INV-1 restated under `airsctl` — **ANSWERED**
**The call:** ADR-01-02 says `airsctl` "becomes an in-process agent house itself" — a second process running agents. INV-1 says no agent work outside the AutoDev process; S6 says "exactly one AutoDev process"; and spec `08`'s `G-CONCURRENT` requires *two* processes to write the same database successfully (`R2 X-05/X-13/X-14`).
**▶ RAZI'S DECISION:** **Restate the invariant.** *Answered 2026-08-31.*

> **INV-1 (restated):** *No agent work happens in a **foreign runtime or language**. Every agent turn executes in Rust, in a binary built from this workspace, linking the shared agent-core crate.*

That is the property the sidecar actually violated. Both the desktop app and `airsctl` satisfy it. **S6 is amended** to "exactly one *agent runtime*, linked by both binaries" — and `G-CONCURRENT` is no longer in tension, it is the requirement.

### G-03 — Memory has a writer in Phases 1–3 — **ANSWERED**
**The call:** ADR-05-04 adopts anchor §18, titled *"Autonomous Memory Creation"*; ADR-05-05 defers Phases 4–6, which explicitly includes autonomous memory creation. With `remember_this` removed, this decides whether anything writes memory at all early on (`R2 X-03`).
**▶ RAZI'S DECISION:** **Agent proposes, gateway evaluates — shipping in Phases 1–3.** *Answered 2026-08-31. The agent explicitly proposes a memory at the end of a task; the deterministic gateway applies importance, novelty, duplication and confidence before storing. What defers to Phase 5 is the agent doing this **unprompted and continuously**. Memory therefore has a writer, and a gate, from day one.*

### G-04 — A spend ceiling that actually stops — **ANSWERED**
**The call:** Nothing stops spending. Per-task budget is inert (`with_task_budget` has zero callers), ADR-02-04 dropped the budget dimension, no daily ceiling, no off switch. A worker broadcast fires a full LLM tick per worker with no per-tick cap (`R3`, `G9 F-BUD-04`). Under BYOK the runaway bill is the user's.
**▶ RAZI'S DECISION:** **Hard daily cap plus a kill switch.** *Answered 2026-08-31. A user-set daily ceiling enforced in the provider layer — the one place every call already passes — plus a visible global stop that halts all agent work immediately. Exceeding the cap **pauses the fleet and surfaces it**, never fails silently. Per-task budgets become advisory on top of a ceiling that actually stops things. New mechanic **M-42**.*

### G-05 — The build-escalation contract — **ANSWERED**
**The call:** ADR-02-02 was answered "only do (B) after three consecutive failures" — but **(B) literally means "keep the model authoring the build command"**, which Part 2.2 forbids absolutely and which is the direct cause of BUG-COMPILE (`R2 X-07`).
**⚠ My own conduct, recorded:** when writing M-13 and spec `04` I silently narrowed (B) to "the model diagnoses only". That was **reinterpreting the owner's answer instead of flagging the conflict**, and the review was right to catch it.
**▶ RAZI'S DECISION:** **Model diagnoses; Rust still owns and runs the command.** *Answered 2026-08-31. After three consecutive failures the model reads the build output and proposes a cause. Rust still composes the argv and still reads the exit code. **The verdict is never the model's.** A literal reading of (B) would have re-created BUG-COMPILE on the retry path, on the exact machine class where the original bug lives.*

### G-06 — The Linux credential store — **ANSWERED**
**The call:** BYOK is keychain-only, but `keyring 2.3.3` needs a D-Bus Secret Service on Linux, which the repo's own `ci.yml:64` admits is absent. So headless Linux has nowhere to put a key — **and the end-to-end test that defines "done" cannot execute** (`R1 B-10`, `R3`).
**▶ RAZI'S DECISION:** **Encrypted file fallback on Linux.** *Answered 2026-08-31. Where no Secret Service is available, fall back to an encrypted file under the app data root with `0600` permissions, and **tell the user plainly which store is in use**. CI uses the same path, so the shipped and tested binaries do not diverge — which is what ADR-03-05 rejected the debug-flag option for. INV-12 is amended to name the weaker case rather than pretend it does not exist.*

### G-07 — Memory privacy: inspect, export, forget — **ANSWERED**
**The call:** Memory is permanent with no way to see, export or delete it, and the gateway's storage criteria do not include **sensitivity** — so a secret read from a file can be stored forever (`R3`).
**▶ RAZI'S DECISION:** **All of it: inspect, export, forget — plus redaction on the write path.** *Answered 2026-08-31. A memory browser showing what is stored, per-entry and per-project deletion, JSON export, and **M-40's ported redaction running before anything is persisted** so secrets do not enter the store in the first place. Much of the UI already exists inside the ~3,700 unreachable LOC (spec `07 §6`) — this routes and finishes it rather than building new.*

### G-08 — Autonomy limits and reversibility — **ANSWERED**
**The call:** Workers run unattended, execute shell commands and write files, with no blast-radius limit — and ~~**undo does not exist**: `db/checkpoints.rs` is not compiled into the binary (`G3d F-44`), and~~ — **corrected 2026-09-01: undo DOES exist** (`git/checkpoint.rs`, wired end to end). The half of `G3d F-44` that stands is the silent-failure half: `auto_checkpoint_before_write` returns `()` and logs at `debug!` level only, so a failed checkpoint is invisible and the overwrite proceeds anyway.
**▶ RAZI'S DECISION:** **Both — checkpoints and a worker branch per task.** *Answered 2026-08-31. Compile the checkpoint module so per-write undo genuinely exists; make a **failed checkpoint block the write** rather than proceed silently; keep every agent write inside the resolved workspace (INV-5); **and** give each task its own worker branch for coarse rollback. New mechanic **M-43**.*
**Binding condition:** the two systems must never disagree about current state. Checkpoints are per-write and short-lived; the branch is the durable record. Spec `04` and spec `09` carry the reconciliation rule, and the two competing checkpoint tables (`R3` of Part 4) collapse to one as part of this.

---

### Decided by me under the review's findings

These needed an owner but not a fork — the plan's own prior decisions already determine them.

**G-09 — The egress consent model.** `G-PERM-NETWORK` ("prompts in every mode") collided with `G-QC-ANALYZE` (must run unattended) — two decided specs, opposite requirements (`R3`). **Resolved: consent is a property of the workspace + destination, not of each call.** The user authorises "this workspace may send content to this provider" once, revocably; unattended work runs under that standing consent, and its **absence blocks egress** rather than prompting an unattended machine. Already folded into both gates.

**G-10 — Hosted tools: web search yes, file search no.** ADR-01-05 rows 12–14 **SKIP hosted RAG**; ADR-02-08 adopted "web search + file search". **Provider file search *is* hosted RAG** (`R2 X-02`), and it ships user code to a vendor against §1.5's local-first non-goal. **File search is dropped.** Web search stays, and **OQ-4's "Google" wins over ADR-01-05 row 8's xAI** — the later, more specific answer governs. Noted honestly: Google is the option the plan itself prices at ~2.8× xAI ($14 vs $5 per 1k).

**G-11 — `ToolEffect` gets an owner.** Spec `06 §1.2` introduced a `ToolEffect` enum replacing the permission model, owned by no ADR — a spec recording a decision, which the disclaimer forbids (`R2 X-43`). **Adopted here as the decided permission model**, so it has a home in Part 8.

**G-12 — Scope lives in the `vec0` schema, not beside it.** `vec0` KNN can only filter on **declared metadata / partition-key columns**; the plan's "metadata filtering → plain SQL columns" plus a *mandatory* scope filter makes scope a **post-filter over the top-k**, so a selective scope returns few or zero memories (`R1 B-01`). ADR-05-01, 05-03 and 05-05 are individually fine and **jointly unimplementable** as written. **Resolved: scope is a declared `vec0` partition key**, so filtering happens inside the KNN rather than after it.

**G-13 — ⚠ The reranker equivalence claim was false.** ADR-05-05 asserted *"the anchor's §10 reranker **is** the competitive model, so the two decisions agree."* The review checked: **6 of the anchor's 8 terms and 7 of the ported model's 9 are disjoint** (`R2 X-04`). Because I declared them identical, **the Phase-2 reranker had no work item and no schema columns.** Corrected: they are two different scoring models, the anchor's is the target, and it gets its own mechanic and its own columns.

**G-14 — `fastembed`'s real cost, and its CWD violation.** `fastembed 6.0.2` pulls `ort 2.0.0-rc.13`, whose default features **download ONNX Runtime from a third-party CDN at build time** — breaking offline builds, adding a possible signed native library to the bundle (absent from M-27), and reaching `airsctl` through workspace feature unification (`R1 B-02/B-03`). Its cache also defaults to `.fastembed_cache` **relative to process CWD** — a direct **INV-5 violation on the newest subsystem**, and on a macOS `.app` the CWD is `/`. **Resolved: pin and vendor the ONNX runtime rather than fetching at build time, and set the cache path explicitly under the app data root.** Razi should know the local-embedding cost is higher than the "~130 MB model" I originally quoted.

**G-15 — The unauthenticated `/mcp` server is deleted in P0, not P8.** ADR-03-02's own words are *"we delete it first now"*, but the build sequence scheduled it in P8 (`R2 G-09`). An unauthenticated local RCE surface does not wait eight slices. **Moved to P0.**

**G-16 — `ui-anchor.md` vs self-hosted fonts.** The anchor says fonts are *"loaded from Google Fonts"*; spec `07` requires **self-hosting** because the CSP block on the font CDN is what makes Graph City hang, and `G-CITY` requires offline rendering (`R2 X-25`). **Self-hosting wins**, and unlike the memory-anchor §21 divergence this one was unacknowledged — it is acknowledged now.

---

### Corrections and fixes carried out, not requiring a decision

| # | Finding | Action |
|---|---|---|
| 1 | **M-40 redaction was missing** — `G4b §A.14` ruled it **PORT** and the plan dropped it; `redact`/`scrub`/`sanitiz` appeared **zero times** across plan and specs | Added as **M-40**; ported capabilities go **nine → ten**; new P8 gate: the deletion phase cannot complete while a capability grounding marked PORT remains unported |
| 2 | **Part 1B was last regenerated at rev 0.3** — none of ADR-05-01…05-06 owned a mechanic, so an MDP harvested today would omit **the entire memory programme** (`R2 X-18`) | Part 1B regenerated; M-10 restated; counts corrected |
| 3 | Stale counts — "29 mechanics across five workstreams" (actually 40 across six) | Corrected |
| 4 | **"Structurally impossible" was an overclaim** — `workers.status` is unconstrained `TEXT` and `src-shared` writes arbitrary strings from a second process, so the enum **relocates** the dead-latch to `FromStr` (`R1 B-11`) | Corrected in spec `04`, with the missing half added: unknown status is a loud error, never a silent default, plus a `CHECK` constraint |
| 5 | **The cache-hit formula was Anthropic-only** — OpenAI and Gemini report cached tokens *inside* the prompt total, so the plan's arithmetic over-reports cost on three of four providers (`R1 B-09`) | Corrected in spec `02`; restated provider-neutrally |
| 6 | `G-PERM-NETWORK` × `G-QC-ANALYZE` gate collision | Resolved via G-09; both gates rewritten |
| 7 | **`G1 §C.3` still carries the retracted "`port_status.rs` should be KEPT" claim** while listed Authoritative (`R2 X-32`) | Struck in the grounding report with a pointer to `G12 MC-08` |
| 8 | **No Part-6 slice schedules `autodev-core` / `src-cli` / `src-shared`** — two BLOCKS-CODE ADRs with no work (`R1 B-05/B-06`) | Core-crate extraction added to the build sequence |
| 9 | **P8's entry gate is unmeetable** — `G1 §C.4`'s nine capabilities include P4 ("10 provider families" vs a 4-provider mandate) and P7 (startup FSM, deleted by OQ-3). "Sidecar not running" is also weaker than "sidecar renamed" — persona YAML still reads `{orch}/config/agents/*.yaml` (`R1 B-08`) | Gate restated against the corrected capability list, and strengthened to **sidecar absent, not merely stopped** |
| 10 | **P4's exit gate cannot run** — three-OS end-to-end, but spec `10` puts the harness in P9 and macOS CI in a P0 that does not contain it (`R2 X-21/22`) | Harness and macOS CI moved into P0 |
| 11 | **`src-shared` duplicates four tables, not one** — `tasks`, `workers`, `plans`, `plan_steps`; and `plans.db` has **two incompatible designs** (`plan_steps` vs `plan_items`) (`R1 B-07`) | Spec `08`'s reconciliation R5 widened |
| 12 | Specs recording decisions despite the disclaimer — spec `07 §1.2` resolved ADR-01-10's open A/B, spec `09 §3.3` disposed of delta patching (`R2 X-42/44`) | Both lifted into Part 8 |
| 13 | **No cadence anywhere** — `interval`/`cadence`/`backoff`/`max_retries` appear **zero times in all 13 documents**; eleven mechanisms specify *what* and never *when*. Sharpest: **`ErrorClass::Retryable` is defined with no retry policy** — a writer with no reader, inside INV-8's home spec (`R3`) | An **operational cadence register** becomes a required MDP artifact |

**Kept intact by the review, explicitly:** INV-8 reader-exists · guard-the-guard · ADR-02-02(A) · the SQLite substrate choice (`sqlite-vec` and `rusqlite` **do** compose — no duplicate-symbol conflict; the suspicion in my own brief was disproven) · P8 as its own phase · the append-only ledger · BYOK asymmetry · both local-RCE removals · the disciplined MCP-client deferral. **R1 re-verified 19 sampled `CODE-PRESENT` claims and all held** — the diagnosis is sound; the forward mechanisms were where it failed.

---

> ## ✅ LEDGER COMPLETE — 51 ADRs, 0 open
>
> **29 answered by Razi in the first pass · 4 decided under delegation (Group F) · 6 answered by Razi (Group E) · 8 answered by Razi + 8 decided under the review's findings (Group G).**
>
> All Part-9 open questions are answered. OQ-6 was re-opened as ADR-05-06 after its premise proved false, and is now answered. **OQ-7** (working memory as a fourth tier) is explicitly deferred with a schema shape that makes the deferral safe.
>
> **Claims overturned and recorded in place, not deleted:**
> - ADR-01-07 — my lexical-first recommendation, overturned by the owner adopting `memory-anchor.md`
> - ADR-05-02 — OpenAI embeddings, **retracted by the owner the same day** in favour of local `fastembed`; the struck answer remains visible and it corrected a dimension I had already propagated (1536 → 384)
> - ADR-03-02 / spec `09` — my claim that `port_status.rs` survives the MCP-server deletion; it does not, and keeping it breaks the build
> - ADR-05-05 (G-13) — my claim that the anchor's §10 reranker *is* the ported competitive model; **6 of 8 terms are disjoint**, and the false equivalence had left the Phase-2 reranker with no work item
> - Spec `02` (G-14 fix) — my cache-hit formula, which was Anthropic-only
> - Spec `04` (fix 4) — "structurally impossible", which was an overclaim
> - **ADR-02-02 (G-05)** — where I silently narrowed the owner's answer instead of flagging that it conflicted with Part 2.2. The review caught it; the owner has now decided it explicitly.
>
> ## ✅ Exit criteria for `vision-master-planning` are MET
>
> Every Part-8 ADR and Part-9 OQ carries a decision · design-changing answers folded through the suite · the stale precursor flagged SUPERSEDED IN PART with a wrong-vs-ratified table (Part 1.7) · the live migration head verified (`graph/` v5, and its head is a broken statement that must be repaired before v6) · the amendment log records the answering and review passes · **the adversarial pre-dev gap review is complete and its findings are folded**.
>
> **Next: hand off to `dev-anchor-set` → `docs/restructurization/development`.**

# PART 9 · Open questions & risks

## 9.1 Open questions

Answered on the same `▶` pattern as the ADRs. These are narrower — they do not fork the architecture, but they must not be left to drift.

**OQ-1 — Does `airsctl` remain a product commitment?** The old plan makes it central (headless CI, worker provisioning). Nothing in the current mandate mentions it, and ADR-01-02 shows it is a real refactor.
**▶ RAZI'S DECISION:** remain it as a product commitment as per stated in the ADRs above

**OQ-2 — What happens to the seven seeded fleet workers?** `db/workers.rs:185` seeds Backend, Frontend, Platform, MCP Builder, Cleanup, Supervisor and QC. Shipped default, user-editable, or a starting template?
**▶ RAZI'S DECISION:** Ship those as a default team and user editable. 

**OQ-3 — Is Demo Mode kept?** `runtime_config.skip_sidecars` and the scripted startup FSM (`main.rs:283-347`) exist to demo without sidecars. After P8 there are no sidecars to skip, but the scripted-progress UX may still be wanted.
**▶ RAZI'S DECISION:** Remove demo mode completely

**OQ-4 — Web search: adopt, and via which provider?** The sidecar had Tavily; the Rust side has nothing. `G6` finds xAI hosts it cheapest ($5/1k vs Anthropic $10, Google $14).
**▶ RAZI'S DECISION:** Google.

**OQ-5 — Retention policy for the unbounded tables.** `token_usage`, `system_events`, `api_failures`, `worker_stats`, `hook_logs` grow forever. What is kept, for how long?
**▶ RAZI'S DECISION:** this one you decide based on the restructure, drop whatever is not needed.

**OQ-6 — Does AutoDev keep its own MCP *client*?** ADR-03-02 removes the MCP *server*. The client (GitHub MCP etc.) is a separate question, and ~~`health_monitor.rs` + `port_status.rs` (572 LOC) exist to serve it~~ **⚠ CORRECTED — see ADR-05-06.**
**▶ RAZI'S DECISION:** Yes, mcp client we keep in case we want to extend by letting the user add their own MCP server later on and it is functional as of now right, check.

> **⚠ CHECKED — THE PREMISE IS FALSE. There is no MCP client. Not broken: absent.** (`G12`, all CONFIRMED.)
>
> - **`src-tauri/Cargo.toml:50`** — `rmcp` features are `["server", "transport-streamable-http-server"]`. **No client feature, no client transport. The library physically cannot dial out.**
> - Repo-wide grep for `ClientHandler`, `serve_client`, `rmcp::service`, `McpClient`, `connect_mcp` across `src-tauri/src`: **zero hits**. The only `list_tools`/`tools/call` is an rmcp **ServerHandler** impl.
> - **`Settings.tsx:653` tells the user it "Connects to the GitHub MCP server (`@modelcontextprotocol/server-github`)". That sentence is false** — grep for `server-github` / `modelcontextprotocol` in Rust returns zero hits. Nothing spawns it. `commands/github_mcp_commands.rs` (84 LOC) is a SQLite key/value write with no network and no process spawn.
> - `health_monitor.rs:190-209` is a plain `GET {endpoint}/health` that **counts a 404 as success** — it would report "Connected" for `example.com`. It is not speaking MCP.
> - The end-to-end trace breaks at **hop 4 of 8**: configure ✅ → persist ✅ → read at startup ✅ → **connect ❌** → discover ❌ → register ❌ → call ❌ → return ❌.
>
> **Because the decision was made on a false premise, it is re-opened as ADR-05-06.** "Keep it" is not an available option — there is nothing to keep.

**OQ-7 — Does "working memory" become a persisted fourth tier?** Opened by ADR-05-03, which adopted the anchor's scopes and deferred this. The anchor (§4.4) says working memory *"should generally remain temporary"* and *"does not necessarily belong in long-term memory"*; today the live session context plus the task row already serve the purpose. **Deferred safely:** the memory schema uses a tier *discriminator value*, not a table per tier, so a fourth tier can be added later without a migration.
**▶ RAZI'S DECISION:** _deferred by ADR-05-03; revisit after Phase 3 memory is in real use_

## 9.2 Risk log

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| **R-1** | **The bug register is large enough to swallow the restructure.** ~415 defects invites fixing everything and shipping nothing | High | High | Slices P0–P9 are gated on *outcomes*, not defect counts. A defect not on a slice's path is logged, not fixed. **The register is a map, not a backlog** |
| **R-2** | **Caching is adopted and silently does nothing.** No provider errors below the per-model minimum; one volatile byte defeats the prefix | High | High | Build the cache-hit alarm **before** the cache (ADR-01-05). One prefix builder, ordered maps. `MEASURED` gate in P2 |
| **R-3** | **The port reproduces the sidecar's own defects** — the memory spec being ported contains a query-independent semantic recall, a cache key that ignores the query, and an always-throwing HITL writer | Medium | Medium | `G4c §6` lists them explicitly as fix-don't-reproduce; spec `03` carries that list forward |
| **R-4** | **macOS is verified last and blocks release.** No macOS CI exists today; signing is procurement-gated | Medium | High | ADR-03-04 answered early; macOS enters CI in **P0**, not P9 |
| **R-5** | **BYOK turns first-run into a wall.** An unconfigured app does nothing at all | Medium | High | M-35 is a gated slice with an acceptance test, not polish (ADR-04-06) |
| **R-6** | **Deleting the sidecar strands a capability nobody enumerated.** Nine are known; the tenth is the danger | Medium | High | P8's entry gate is a full suite run with the sidecar **not running** — the only honest test |
| **R-7** | **`Item::Opaque` is deferred and every adapter gets rewritten twice** | Medium | Medium | Named as a day-one trait decision in ADR-01-04 |
| **R-8** | **Fixing the fleet re-enables a broken feedback loop.** Affinity trained on stolen-task attributions gets worse once dispatch works | Medium | Medium | Explicit sequencing constraint in Part 6: M-12 before M-33. Consider discarding existing `routing_history` |
| **R-9** | **Rate limits and cost during development.** The programme is itself LLM-heavy — this planning run already hit an account limit | High | Low | Dev `.env` + a conformance harness (ADR-03-05) rather than ad-hoc live calls; batch APIs where applicable |
| **R-10** | **Windows stays the only truly tested platform**, because it is the development machine | High | High | `PLATFORM-TRIPLE` is an evidence class; P4's exit gate names all three OSes |

**"Decide later" never means "never decided":** every OQ carries a `▶` line, and every risk carries a mitigation that lands in a named slice.

# PART 10 · Amendment log

| Date | Rev | Change |
|---|---|---|
| 2026-08-31 | 0.1 | Created. Mandate, grounding sources, Part 1 (thesis, evidence, metaphor, 11 invariants, non-goals, worked example, reconciliation). Parts 1B–9 skeletoned pending the four in-flight grounding workstreams. |
| 2026-08-31 | 0.2 | Grounding complete — 16 workstreams, all nine named bugs traced, ~415 defects catalogued. Wrote Part 1B (29 mechanics), 1C, Part 2 (det↔organ doctrine), and Part 8 (25-ADR ledger). |
| 2026-08-31 | 0.3 | **Razi's second-pass direction folded through** (mandate items 6–13): `.env` removed → ADR-04-05 amended · submodule deletion promoted to its own gated phase **P8** · **BYOK** → new **M-30**, **INV-12**, **ADR-03-05**, ADR-03-01 reframed · **ADR-02-03 answered** (subagents land natively in `src-tauri`) · dev-`.env`-is-never-a-runtime-fallback → INV-12 · new **W6 Experience** workstream (M-35…M-39) + **ADR-04-06** · fleet coordination capabilities preserved and completed → **M-31…M-34** + **ADR-02-06** · tool-surface sweep commissioned. Ledger now **29 ADRs**; mechanics now **39**. Three new grounding workstreams dispatched (`G9`, `G10`, `G11`). |
| 2026-08-31 | 0.4 | Wrote Parts 3–7 and Part 9. Data model grounded against `G1 §E`: **11 databases, not 10**, and a **third data root** discovered (`~/.airs/autodev.db` holds sessions) — strengthening ADR-01-09. Seven schema reconciliations (R1–R7); migration discipline (only `graph/` has a ladder, and its head **v5 is a swallowed syntax error**); the nine-slice build sequence with **P8 as the gated deletion phase**; the headline acceptance test; ten test families; six OQs and ten risks. |
| 2026-08-31 | 0.9 | **Adversarial pre-dev gap review complete and folded — Group G added.** Three fresh-context reviewers (`R1` buildability 23 · `R2` contradictions 47 · `R3` gaps & policy 34) produced **104 findings and 39 proposed ADRs**, consolidated into **16 Group-G entries: 8 answered by Razi, 8 decided under the review's findings**. Ledger now **51 ADRs, 0 open**. Three new mechanics: **M-41** fault isolation, **M-42** spend ceiling, **M-43** reversibility. **Six of the plan's own claims were overturned and corrected in place**, including one where I had silently narrowed the owner's ADR-02-02 answer instead of flagging its conflict with Part 2.2 — the review caught it and the owner has now decided it explicitly. Thirteen further corrections applied without needing a decision, the most consequential being that **Part 1B had not been regenerated since rev 0.3, so none of ADR-05-01…05-06 owned a mechanic** — an MDP harvested before this fix would have omitted the entire memory programme. The review also **disproved the suspicion in my own brief**: `sqlite-vec` and `rusqlite` do compose, so ADR-05-01's substrate stands. **Exit criteria for `vision-master-planning` are met.** |
| 2026-08-31 | 0.8 | **ADR-05-02 retracted and re-answered by Razi the same day**, before any downstream work depended on it: embeddings move from OpenAI `text-embedding-3-small` to **local `fastembed-rs` / BGE-small-en-v1.5**. The struck answer is kept visible per the append-only rule. **This corrects a number I had already propagated: the vector dimension is 384, not 1536**, so the `sqlite-vec` column width changes and the "matches the retired memory-mcp-server" continuity note is withdrawn (it was never load-bearing — there is no index to migrate). Net effect: no provider key is required for semantic memory, memory writes need no network and cost nothing, and the new constraint is a one-time ~130 MB first-run download that must be visible, resumable and skippable. Two new constraints recorded: `fastembed` is synchronous and must run on `spawn_blocking`; and BGE-small is English-centric, which is a config-level swap if it ever bites. |
| 2026-08-31 | 0.7 | **Group E answered — the ADR ledger is complete at 35, zero open.** Memory substrate: **SQLite + `sqlite-vec` + FTS5** behind a backend trait (ADR-05-01). Embeddings: **OpenAI `text-embedding-3-small`**, overturning my local-embeddings recommendation (ADR-05-02) — three consequences recorded as design constraints: OpenAI becomes a required key for semantic memory and first-run must say so; retrieval **degrades to FTS5/BM25 rather than failing** when no OpenAI key is present; and embedding must be async and retryable so a network hiccup cannot stall a turn. Vector width settled at **1536**, matching the retired memory-mcp-server. Scope: anchor **Phases 1–3 + provenance** (ADR-05-05); **scopes adopted, working memory deferred** to the new **OQ-7** with a migration-free schema shape (ADR-05-03); **agent proposes, deterministic gateway decides** — `remember_this`'s unrestricted write is removed (ADR-05-04); **MCP client deferred, false UI claim deleted** (ADR-05-06). |
| 2026-08-31 | 0.6 | **`G12` answered OQ-6's "check": there is no MCP client — not broken, absent.** `rmcp` is compiled with server features only and physically cannot dial out; the trace breaks at hop 4 of 8; and `Settings.tsx:653` tells the user it connects to the GitHub MCP server, which is false. OQ-6 is re-opened as **ADR-05-06**, which also surfaces a structural blocker: `ToolRegistry` is frozen after construction with no dynamic registration path anywhere, so admitting third-party tools means changing the agent core path. **Two of my own claims corrected in place:** `port_status.rs` does *not* survive the MCP-server deletion (it imports from `server.rs`; keeping it breaks the build) — corrected in ADR-03-02 and spec `09`; and the survivor set is 484 LOC, not 572. Also recorded: 4 of 7 surviving MCP items live inside sidecar-named files, so they must be **moved by address** or sidecar code survives P8 by filename. |
| 2026-08-31 | 0.5 | **Razi answered all 29 ADRs and all 6 OQs**, and introduced `memory-anchor.md` as a governing reference. Changes: **(a) a duplicate `ADR-02-06` I had introduced was found and corrected** — the provider-hosted-tools entry is renumbered **ADR-02-08**; both entries were answered, so no decision was lost. **(b)** New **Group E** — five ADRs opened by reading the anchor against the platform: the substrate conflict (the anchor specifies PostgreSQL + pgvector; the platform is a desktop app mid-way through deleting a bundled server), the embedding source under BYOK (**Anthropic ships no embeddings endpoint**), the scope model and "working memory", the Memory Gateway's write-permission boundary, and how much of the anchor's six-phase programme lands inside the restructure. **(c)** New **Group F** — the four decisions Razi delegated, now made with reasoning: worker persona storage **(C)**, Redis **deleted entirely**, retention policy per table, and yes-create-a-`.env`. **(d) ADR-01-07's recommendation is recorded as overturned** by the owner's adoption of the anchor. **(e)** ADR-02-02 amended per Razi: deterministic build verification is the main path, with an LLM debug step only after **three consecutive failures**. Ledger now **34 ADRs — 29 answered, 4 delegated-and-decided, 5 open**. |
