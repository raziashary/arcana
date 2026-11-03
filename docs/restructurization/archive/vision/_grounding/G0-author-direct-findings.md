# G0 — Author's Direct Grounding Findings

**Author:** Claude (main session, as plan integrator)
**Date:** 2026-08-31
**Method:** First-hand read. Not delegated. Every claim below carries a `path` citation.
**Status:** WORKING — feeds `00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md`

> Evidence classes used throughout: `CODE-PRESENT` (verified by reading shipped
> code/config), `DOC-CLAIM` (a document asserts it; unverified against code),
> `GAP` (specified but absent), `BOUNDARY` (deliberately out of scope).

---

## 1 · Repository shape (CODE-PRESENT)

Verified at `C:\AIRS Foundry\autodev\autodev`.

| Surface | Path | Size | Note |
|---|---|---|---|
| Frontend | `src/` | ~35,143 LOC TS/TSX | React 19 + Zustand 5 + Vite 6 |
| Tauri backend | `src-tauri/src/` | ~53,387 LOC Rust | 33 modules |
| CLI (`airsctl`) | `src-cli/src/` | ~3,508 LOC Rust | **Not previously accounted for** |
| Shared crate | `src-shared/src/` | ~1,073 LOC Rust | config + db (tasks/workers/plans/analytics) |
| Python sidecar | `submodules/core-orchestrator` | large | RETIRE |
| Memory MCP | `submodules/memory-mcp-server` | — | RETIRE |

**Cargo workspace** (`Cargo.toml`) has **four** members: `src-tauri`, `src-cli`,
`src-shared` (+ root). The restructure scope stated as "three main modules"
therefore misses two Rust crates. `src-cli` and `src-shared` must be explicitly
in or out of scope — they are not neutral. → **ADR candidate.**

**Frontend stack** (`package.json`) — `CODE-PRESENT`:
- React 19.2, Zustand 5, Vite 6, TypeScript 5.7
- **Tailwind CSS 3.4.17** present as a devDependency, with `tailwind.config.js`
  and `postcss.config.js` at root — so the UI refactor against `ui-anchor.md`
  must decide Tailwind's fate (the anchor specifies a bespoke `.airs-*` class
  vocabulary and CSS-variable tokens, not utility classes). → **ADR candidate.**
- `three` 0.184 + `@react-three/fiber` + `drei` + `postprocessing` → this is
  **Graph City** (reported broken).
- `d3` + `d3-hierarchy` + `recharts` → 2D graph + charts.
- `monaco-editor` + `@monaco-editor/react` → IDE tab and the AUTODEV.md editor.

---

## 2 · Testing and CI: the false-green problem (CODE-PRESENT — HIGH severity)

This is a *culture* finding, and it is upstream of the owner's bug #1
("compiling of the code does not happen"). The project's own gates do not fail.

| # | Finding | Evidence |
|---|---|---|
| G0-01 | **No frontend test framework exists at all.** No vitest, jest, playwright, or testing-library in `package.json`. No `test` script. | `package.json` `scripts` + `devDependencies` |
| G0-02 | **`npm run lint` is not a linter.** It is aliased to `tsc --noEmit` — identical to `npm run typecheck`. There is no ESLint in the project. | `package.json`: `"lint": "tsc --noEmit"` |
| G0-03 | **CI's "ESLint" step runs no ESLint.** `ci.yml` `frontend-checks` runs `npm run typecheck` then `npm run lint` — i.e. typecheck twice, labelled as two different gates. | `.github/workflows/ci.yml` job `frontend-checks` |
| G0-04 | **Rust tests cannot fail CI.** Both the `src-tauri` and `src-cli` test steps carry `continue-on-error: true`, with a comment conceding clippy is "the real compilation gate". | `.github/workflows/ci.yml` jobs `rust-checks`, `cli-checks` |
| G0-05 | **`cargo fmt --check` cannot fail CI** either (`continue-on-error: true`, both crates). | same |
| G0-06 | **No macOS job in `ci.yml`.** Build smokes cover Windows and Linux only, despite macOS being a shipping target. | `.github/workflows/ci.yml` — `build-smoke-windows`, `build-smoke-linux`, no macOS equivalent |
| G0-07 | **Build smokes never run on pull requests.** Both are gated `if: github.event_name == 'push' && github.ref == 'refs/heads/main'` — so a PR can merge without either platform ever being compiled as a Tauri app. | same |

**Net:** the only gate in CI that can actually fail is `cargo clippy` (both
crates) and `tsc --noEmit`. Everything presented as a test gate is decorative.
A restructure that does not fix this will not be able to *prove* it fixed
anything else.

---

## 3 · Bundling and distribution (CODE-PRESENT)

### 3.1 The bundle ships the Python sidecar — including its `.env` and `.git`

`src-tauri/tauri.conf.json` declares:

```json
"resources": ["resources/sidecars"]
```

and `src-tauri/resources/sidecars/core-orchestrator/` is **a full copy of the
Python source tree — 225 MB** — containing `.env`, `.git/`, `.pytest_cache/`,
`api_gateway_data.sqlite`, `logs/`, and `Dockerfile`.

Verified: `du -sh src-tauri/resources/sidecars/` → `225M`.

Git is *not* the exposure — `.gitignore:6` ignores `src-tauri/resources/sidecars/`
and `git ls-files` returns 0 tracked files there. The exposure is the **installer**:
`tauri build` copies declared resources into the shipped artifact regardless of
gitignore.

This produces a fork, and **both branches are defects**:
- **Local build** → a 225 MB installer shipping a `.env` and a `.git` history to
  end users. *(Severity: CRITICAL — secret disclosure in a distributed artifact.)*
- **CI build** (clean checkout, directory absent) → the declared resource is
  missing, so any runtime path expecting the bundled sidecar fails.

Deleting the sidecar resolves both. **This is the single strongest concrete
argument for the restructure's core thesis.**

### 3.2 Bundle targets are Windows-only

`tauri.conf.json` → `"bundle": { "targets": ["nsis"] }`.

NSIS is a Windows installer format. `dmg`, `deb`, `appimage`, and `rpm` config
blocks all exist in the same file and in `installers/{dmg,wix,deb,rpm,appimage}/`,
and `.github/workflows/build-release.yml` has a `build-macos` job (`macos-14`,
targets `aarch64-apple-darwin,x86_64-apple-darwin`) — but the default bundle
target list produces none of them.

Only `scripts/build-linux.sh:59-63` overrides this, constructing a `--bundles`
list for appimage/deb. `scripts/build-macos.sh` delegates to
`installers/dmg/create-dmg.sh`; `scripts/build-windows.ps1` delegates to
`installers/wix/build-msi.ps1`. So the packaging chain is more built out than
`tauri.conf.json` alone suggests, but the config default is a cross-platform trap.

### 3.3 A Windows-only GPU flag is set for all platforms — likely implicated in Graph City

`tauri.conf.json` → `app.windows[0].additionalBrowserArgs`:

```
--enable-gpu --use-angle=d3d11 --enable-features=Vulkan
```

`d3d11` is Direct3D 11 — **Windows-only**. This ANGLE backend does not exist on
macOS (Metal) or Linux (GL/Vulkan). Passing it on those platforms is at best
ignored and at plausible worst breaks WebGL context creation.

Graph City is `three.js`/WebGL and is reported broken. This flag is a concrete,
testable hypothesis for a class of its failures — and independently a
cross-platform defect. `SUSPECTED` as a Graph City cause; `CONFIRMED` as a
platform-portability defect.

### 3.4 Neither platform is code-signed

- `bundle.macOS.signingIdentity: null` → macOS Gatekeeper blocks the app.
- `bundle.windows.certificateThumbprint: null` → Windows SmartScreen warns.

`hardenedRuntime: true` with no signing identity is incoherent — hardened
runtime is a property of a signed binary. Shipping to macOS users requires a
paid Apple Developer certificate and notarization. This is a **procurement**
dependency, not just a code one, and it gates the stated end goal. → **ADR candidate.**

### 3.5 The updater is built but not configured

`src-tauri/src/updater/` exists (`CODE-PRESENT`), and `build-release.yml` sets
`AUTODEV_UPDATE_PUBKEY` from repo secrets. But `tauri.conf.json` has **no
`updater` entry** in its `plugins` block (only `shell`, `dialog`, `fs`).

Classic writer-with-no-reader shape: the machinery exists, the release pipeline
feeds it a key, and the app never declares it. `SUSPECTED` — needs the Rust
updater module read to confirm whether it uses the Tauri updater plugin or a
hand-rolled path.

### 3.6 CSP does not permit provider APIs

`app.security.csp` `connect-src` allows: `ipc:`, `http://ipc.localhost`,
`https://api.github.com`, `https://releases.airs.dev`.

It does **not** allow `api.anthropic.com`, `api.openai.com`,
`generativelanguage.googleapis.com`, or `api.x.ai`. This is *correct and
desirable* under the restructure — all provider traffic must originate in Rust,
where CSP does not apply — but it must be stated as an explicit invariant so
nobody "fixes" it later by widening the CSP.

→ **INV candidate: no provider API is ever reachable from the webview.**
Keeping keys out of the renderer is the security property this buys.

### 3.7 `build/` and `sidecars/` are empty

The master plan (`docs/autodev-vision/autodev-master-plan.md` §4.1) specifies
`build/bundle-core.sh`, `build/bundle-memory-mcp.sh`, `build/fetch-redis.sh` and
a `sidecars/` output directory. Both directories exist and are **empty**.

`GAP` — the PyInstaller bundling strategy the master plan is built on was never
implemented. The sidecar was instead shipped as a raw 225 MB source copy (§3.1).
This is further evidence the sidecar architecture never reached a shippable
state and should be retired rather than repaired.

---

## 4 · Master-plan reconciliation: what is now SUPERSEDED

`docs/autodev-vision/autodev-master-plan.md` (Draft v1.0, April 2026) is the
project's vision document. The restructure overturns significant parts of it.
Per the skill's prime rules, these are enumerated rather than silently dropped,
and the precursor is flagged **SUPERSEDED IN PART**.

| § | What the master plan says | Status after restructure |
|---|---|---|
| §4.1 Sidecar Architecture | Bundle core-orchestrator + memory-mcp-server + Redis as managed sidecars; Tauri manages their lifecycle; 7-step startup sequence | **SUPERSEDED.** All three sidecars are deleted. |
| §4.1 Keep-vs-Strip list | "Keep the intelligence": 21 FSMs, A2A, SSE engine, MCP client manager, 3-tier memory, context scoring, synopsis evolution | **REINTERPRETED.** These are kept as *semantics to port into Rust*, not as a Python process to bundle. |
| §4.1 Dual-Mode Execution | Mode 1 = Rust-native PA (lightweight, "intentionally simple"); Mode 2 = Worker agents via core-orchestrator LivingAgent | **SUPERSEDED.** Collapses to a single Rust-native mode. The doc's "Zero duplication" rationale inverts: one runtime, not two. |
| §4.1 Git Submodule Strategy | `.gitmodules` tracks both repos; `git submodule update --remote` | **SUPERSEDED.** Both submodule entries are removed. |
| §4.1 Tool Flow | AutoDev tools passed to Python with a `__autodev` suffix and **"executed locally in core-orchestrator (not proxied back to Tauri)"** | **SUPERSEDED — and a prime suspect for bug #7.** If worker file writes execute in the sidecar's process and working directory rather than the user's workspace, produced code lands where the user cannot use it. Cross-check against `docs/endtoendrun/14may2026/07-workspace-escape-rootcause.md`. |
| §6.3 PA Memory storage | "Vector embeddings for semantic search (via core-orchestrator memory-mcp-server)"; "Core orchestrator ContextService provides memory endpoints"; "Sidecar bridge enables Rust → Python memory communication" | **SUPERSEDED.** Embeddings and vector search must be solved in-house. → **ADR: what provides embeddings and vector search in Rust?** |
| §7 Agent engine | "**TypeScript** (compiled to WASM or Node sidecar)" | **SUPERSEDED.** Native Rust. |
| §7 MCP client | "TypeScript MCP SDK (`@modelcontextprotocol/sdk`)" | **SUPERSEDED.** Rust (`rmcp`) per §4.1's own Mode-1 description — the master plan contradicts itself here. |
| §7 UI styling | "**Tailwind CSS**" | **CONTESTED.** `ui-anchor.md` specifies a bespoke token + `.airs-*` class system. → **ADR.** |
| §4/§6 Provider list | Anthropic, OpenAI, Gemini, **Ollama**, Azure | **SUPERSEDED.** Exactly four: Anthropic, OpenAI, Gemini, **Grok**. Ollama and Azure are removed; Grok was never in the plan. |
| §6.8 Model Router examples | `claude-opus-4-6`, `claude-sonnet-4-6`, `claude-haiku-4-5-20251001` | **STALE.** Model IDs need re-grounding against current provider catalogues (see G6). |

### 4.1 Internal contradictions already present in the master plan

These predate the restructure and must be resolved by ADR rather than inherited:

1. **`~/.airs/` vs `~/.autodev/`.** §8 "File & Config Structure" specifies
   `~/.airs/config.yaml`, `~/.airs/skills/`, `~/.airs/recipes/`, `~/.airs/sessions/`.
   §4.1 and §6.3 specify `~/.autodev/tasks.db`, `~/.autodev/workers.db`,
   `~/.autodev/recipes/`, `~/.autodev/memory.db`. Two different application data
   roots. Must be resolved to one.
2. **`CLAUDE.md` vs `AUTODEV.md`.** §6.2 is emphatic — *"Not CLAUDE.md. Not
   AGENTS.md. AUTODEV.md."* — yet §8's config tree lists `~/.airs/CLAUDE.md`
   ("applied to all sessions") and a per-repo `CLAUDE.md` "(unchanged)". §10's
   build prompt also says "CLAUDE.md injection". The proprietary-instruction
   thesis and the file layout disagree.
3. **Compaction vs memory.** §6.1 Agent Engine specifies "auto-compaction at 80%
   context fill". §6.3 PA Memory opens with "**Replaces message compaction.**"
   These are different context strategies. `src-tauri/src/agent/compaction.rs`
   and `src-tauri/src/memory/` both exist — so the code may well have shipped
   both, or neither working. Directly relevant to bug #2 (token blowup).
4. **Internal board vs GitHub Issues.** §4.1 argues at length for an internal
   SQLite task board and against GitHub Issues (rate limits, latency, offline).
   §9 "GitHub Issues as the Command Layer" then describes GitHub Issues as the
   command layer with a live kanban board. Two task-source-of-truth models.
5. **`.airs/skills/` vs `.claude/skills/`** (§6.4, "both paths supported") sits
   oddly beside the AUTODEV.md branding thesis.
6. **Storage root for procedural memory.** §6.3 says `~/.autodev/memory/MEMORY.md`
   + SQLite mirror; §8's tree has no memory directory at all.

---

## 5 · The two unscoped Rust crates: `src-cli` and `src-shared` (CODE-PRESENT)

Nobody scoped these. They are not neutral to the restructure.

### 5.1 `airsctl` agent commands die with the sidecar — CONFIRMED, CRITICAL

`src-cli` is the `airsctl` headless CLI (`src-cli/Cargo.toml` → `[[bin]] name = "airsctl"`).
Its entire agent capability is an HTTP client for the Python orchestrator:

| Command | Endpoint | Evidence |
|---|---|---|
| `agent run` | `POST {orchestrator_url}/api/agents/invoke`, SSE-streamed | `src-cli/src/commands/agent.rs:138`, `:191` |
| `agent list` | `GET {orchestrator_url}/api/agents?limit=` | `src-cli/src/commands/agent.rs:267` |
| `agent show` | `GET {orchestrator_url}/api/agents/{id}` | `src-cli/src/commands/agent.rs:305` |
| `agent stop` | `POST {orchestrator_url}/api/agents/{id}/stop` | `src-cli/src/commands/agent.rs:328` |
| `agent logs` | `GET {orchestrator_url}/api/agents/{id}/...` | `src-cli/src/commands/agent.rs:349` |

`Cargo.toml` is explicit: `# HTTP (for agent run — calls core-orchestrator)`.

**Consequence:** deleting the sidecar breaks every `airsctl agent` subcommand.
The CLI must be re-pointed at the in-house Rust runtime — which, since it is a
separate *process* from the Tauri app, is a real architectural question, not a
find-and-replace. → **ADR: how does `airsctl` reach the agent runtime once the
runtime is a library inside the desktop app?** (Options: extract the runtime into
`src-shared`/a new `autodev-core` crate both binaries link; have `airsctl` drive
the desktop app over IPC; or accept `airsctl` as a thin board/config client only.)

### 5.2 Three mutually inconsistent port/URL stories — CONFIRMED

| Source | Orchestrator location | Evidence |
|---|---|---|
| Shipped code default | `http://127.0.0.1:4000` | `src-shared/src/config.rs:89` |
| Master plan §4.1 | `localhost:8765` (memory MCP on `8766`) | `docs/autodev-vision/autodev-master-plan.md` §4.1 |
| CLI health probe | Redis `6379`, **Postgres `5432`**, MCP `3100` | `src-cli/src/commands/system.rs:65-67` |

Also: the master plan documents the invoke path as `/api/v1/agent/invoke`; the
code calls `/api/agents/invoke` (`src-cli/src/commands/agent.rs:138`). Doc/code drift.

The Postgres probe is notable — the master plan's storage story is SQLite +
Redis, with no Postgres anywhere. Either the probe is vestigial or the sidecar
expects Postgres. Either way it is a documented-vs-shipped divergence.

### 5.3 A user-facing error message names the wrong technology — CONFIRMED, LOW

`src-cli/src/commands/agent.rs:162-164` tells the user:

> `Cannot reach core-orchestrator at {}. Start it with: cargo run --bin core-orchestrator`

`core-orchestrator` is a **Python** service. `cargo run --bin core-orchestrator`
cannot start it and no such Cargo binary exists. Dies with the sidecar anyway,
but it is a marker of the doc/code drift running through this crate.

### 5.4 Duplicated, drifted schema between `src-shared` and `src-tauri` — CONFIRMED, MEDIUM

`src-shared/src/db/tasks.rs:1-2` states: *"Tasks database — mirrors the schema in
`src-tauri/src/db/tasks.rs`. Both binaries open the same `~/.autodev/tasks.db` file."*

Both resolve the same path — `src-shared/src/db/mod.rs:9-17` and
`src-tauri/src/db/mod.rs:20-27` are byte-for-byte equivalent implementations of
`autodev_dir()`. So there is **no split-brain on the database file**. Good.

But the two schema definitions have **already drifted**. The `src-tauri` `tasks`
table carries four columns and one index that `src-shared` does not:

| Column / index | `src-tauri` | `src-shared` |
|---|---|---|
| `rejection_reason TEXT` | ✅ | ❌ |
| `retry_count INTEGER NOT NULL DEFAULT 0` | ✅ | ❌ |
| `claimed_by_pid INTEGER` | ✅ (via ALTER) | ❌ |
| `claim_host TEXT` | ✅ (via ALTER) | ❌ |
| `idx_tasks_created_at` | ✅ | ❌ |

**Severity is contained, and I initially over-called it.** `src-tauri/src/db/tasks.rs:365-377`
carries additive `ALTER TABLE … ADD COLUMN` migrations for exactly these four
columns, explicitly commented *"Add columns to existing DBs that predate these
migrations."* So a `tasks.db` created by `airsctl` is repaired when the desktop
app next opens it. It is **not** an order-dependent corruption.

What remains genuinely wrong:
1. **`src-shared` has no migration path of its own** — only `CREATE TABLE IF NOT
   EXISTS`. The CLI's `Task` struct cannot represent `rejection_reason`,
   `retry_count`, `claimed_by_pid`, or `claim_host`, so `airsctl` structurally
   cannot report why QC rejected a task or how many times it has been retried.
2. **Two hand-maintained definitions of one table, only one with migrations,** is
   a standing drift hazard. The `src-shared` header comment already asserts a
   parity that does not hold — a docstring/code disagreement of the kind Rule 5
   classes as a bug.
3. `autodev_dir()` is duplicated verbatim across the two crates, differing only
   in error type. Same drift shape, lower stakes.

→ **ADR: one schema owner.** The restructure should give the task/worker/plan
schema a single definition (most likely in a shared core crate) that both
binaries link, rather than two mirrors maintained by hand.

### 5.5 `claimed_by_pid` / `claim_host` — a task-claiming mechanism exists

`src-tauri/src/db/tasks.rs:374-377` adds `claimed_by_pid` and `claim_host`. This
is process-level task claiming, distinct from the master plan's git-lock-file
design (§4.1 "Conflict Resolution via Git Locking"). Another shipped-vs-documented
divergence, and directly in the path of bug #3 (workers not assigned work
properly) — the assignment/claim mechanism needs a full trace. Handed to the
platform bug audit.

### 5.6 The CLI bypasses the Key Vault entirely — CONFIRMED, HIGH

Master plan §6.9 promises: *"All API keys encrypted at rest using OS keychain …
Keys never written to plain text config files."*

The **desktop app honours this**: `src-tauri/Cargo.toml:44` pulls `keyring = "2"`
and `src-tauri/src/vault/keychain.rs:4,35,46` implements the OS-keychain vault.
`CODE-PRESENT` — this part of the vision shipped.

**`airsctl` does not.** `keyring` appears in no other crate's manifest (verified
across all three `Cargo.toml` files). The CLI sources its key from, in order:

1. `--api-key` **as a global command-line flag** (`src-cli/src/main.rs:30-32`)
2. env `AIRSDEV_API_KEY`, falling back to `ANTHROPIC_API_KEY` (`src-cli/src/main.rs:147-150`)
3. `~/.airs/config.toml` → `provider.api_key` (`src-shared/src/config.rs:32,142`)

Three defects follow:

- **(a) Plaintext key in a config file** — exactly what §6.9 forbids. Partially
  mitigated: `src-shared/src/config.rs:218` carries `// never emit api_key —
  security`, so `config get`/`list` will not echo it back. The at-rest exposure
  stands.
- **(b) A key passed as a CLI argument** leaks into shell history and into the
  process table (`ps`, Task Manager) for any user on the machine. A flag is the
  wrong channel for a credential.
- **(c) Provider-asymmetric env fallback.** The only fallback variable is
  `ANTHROPIC_API_KEY` — hardcoding one vendor into a tool whose entire premise is
  model-agnosticism. After the restructure there are four providers and no
  `OPENAI_API_KEY` / `GEMINI_API_KEY` / `XAI_API_KEY` path at all.

→ **The restructure must give `airsctl` the same vault the app uses**, and retire
the flag. Couples to the ADR in §5.1 (how `airsctl` reaches the runtime).

### 5.7 Stale provider and model references in the CLI surface — LOW

- `src-cli/src/main.rs:34` — `--provider` help text reads *"e.g. anthropic,
  openai, **ollama**"*. Ollama is retired; Grok is absent.
- `src-cli/src/main.rs:38` — `--model` help text reads *"e.g. claude-sonnet-4-6"*.
  Needs re-grounding against the current catalogue (see G6).

These are user-facing help strings, so they are part of the product surface, not
just comments.

### 5.8 Silent enum coercion — LOW

`src-shared/src/db/tasks.rs:20` — `TaskPriority::from_str` maps every unrecognised
string to `P2` via the `_` arm. A typo'd or future priority silently becomes
"medium" rather than erroring. Correct for `"p2"` by coincidence of the fallback,
not by intent.

---

## 6 · Relative-path defects: output escaping the workspace (CODE-PRESENT)

Two confirmed instances of state being written relative to the **process working
directory** rather than the user's workspace or the app data root. This is the
mechanical shape of the owner's bug #7 ("code may be created but the user cannot
use the end product") and is worth a dedicated hunt.

1. `src-tauri/src/agent/builtin_tools/graph_tools.rs:176` —
   `std::path::Path::new(".autodev/GRAPH_REPORT.md")`. A bare relative path. Where
   this lands depends entirely on the process CWD at call time.
2. **Observed artifact:** `src-tauri/.autodev/graph.db` exists in the source tree.
   The intended location is `~/.autodev/graphs/<12-char-hash>.db`
   (`src-tauri/src/commands/graph_commands.rs:69,80`). A graph database
   materialising inside `src-tauri/` is direct evidence that some graph path
   resolves against CWD instead of the app data root — and plausibly that the
   graph a worker builds is not the graph the app later reads. Bug #4 territory.

→ **INV candidate: no agent-produced artifact is ever written to a path resolved
against the process CWD.** Every write resolves against either the session's
`working_dir` or the app data root, explicitly.

---

## 7 · Open items this file hands to the plan

**ADR candidates raised here:**
- Scope of `src-cli` and `src-shared` in the restructure
- Tailwind: keep, or replace with the anchor's token/class system
- Embeddings + vector search provider for in-house memory
- Application data root: `~/.airs/` vs `~/.autodev/`
- Instruction file identity: AUTODEV.md as sole truth
- Task source of truth: internal SQLite board vs GitHub Issues
- Context strategy: compaction, memory, or a defined composition of both
- macOS/Windows code signing and notarization (procurement-gated)
- Test strategy: what framework, and which gates become blocking

**Invariant candidates raised here:**
- INV: no provider API is reachable from the webview; all provider traffic
  originates in Rust and all keys stay out of the renderer
- INV: no shipped artifact contains a `.env`, a `.git` directory, or any
  credential material
- INV: a gate that cannot fail is not a gate — no `continue-on-error` on any
  correctness check

---

## Amendment log

| Date | Change |
|---|---|
| 2026-08-31 | Created. Direct author findings on repo shape, CI false-green, bundling/distribution, and master-plan reconciliation. |
