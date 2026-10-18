# Codebase refactor plan — wave 1 (2026-10-18)

**Status: PROPOSAL. Awaiting the owner's decision. Nothing in this document authorises a
cross-cutting refactor.** The active programme remains
`MASTER_DEVELOPMENT_PLAN.md` (P0–P9) and its §0 rules; this plan is subordinate to it and does
not change any P4 scope, gate, or checkbox. Where this plan touches a shipped contract it says
so and stops at owner judgment.

## 1. Why this exists

The owner's standing complaint is that the codebase accumulated *patch-on-patch* code:
workarounds layered on workarounds rather than defects fixed in the layer that owns them. Six
read-only engineering-quality audits were commissioned against disjoint areas to test that
claim with evidence.

**Method.** Each audit ran read-only (no edits, no commits, no live credentials, no billable
provider calls), returned file:line citations plus CONFIRMED/SUSPECTED verdicts, and was told
not to re-derive findings already confirmed elsewhere. Every load-bearing finding was
re-verified in the main session by execution before it was acted on.

| # | Area | State |
|---|---|---|
| 1 | agent / tool executor / permissions / journal | complete |
| 2 | fleet orchestration / recipes / workers / subagents | complete |
| 3 | persistence / migrations / schemas | complete |
| 4 | provider layer / observability / secrets | complete |
| 5 | platform, sandbox, app shell, hooks, vault, CLI | complete |
| 6 | frontend (React/TS), e2e, tooling, CI | complete |

Wave 1 covers audits 1–4; **wave 2 (§3.7)** folds in audits 5–6, which were
commissioned in parallel. They confirmed the same themes and surfaced two security
boundaries in the hook layer that no gate covers, plus a P0 tooling hole that was
already in `AGENTS.md`'s known-red list.

## 2. Headline finding

The audits independently converged on one signature, repeated across every area: **a single
responsibility implemented more than once, with the copies drifting apart silently.** In three
cases the drift was already live and user-visible:

- **Groq could not run the fleet.** Four `match provider_id` provider constructions existed
  outside `provider/factory.rs`; two had drifted and refused `groq`, which the catalogue,
  Settings and the vault all offer. A Groq user lost fleet workers and subagents completely.
  → **fixed today** (`894bbe0`).
- **A worker was permanently locked out after one successful run.** `kick_worker`'s success
  path suppressed the process-wide `kick_inflight` slot release, contradicting the code's own
  comment. Both dispatch paths consult that set, so the worker never dispatched again for the
  life of the process. → **fixed today** (`a2e7aa8`).
- **`memory.db` and `arcana.db` reported a wrong identity** (`"other"`) in migration telemetry,
  so a reader could not distinguish them from an unknown database. → **fixed today** (`5953f8c`).

The rest of the class is latent but mechanically preventable, which is why several proposed
workstreams below are *guards*, not rewrites.

## 3. Findings by theme

Severity: **S1** = live user-visible defect or security-relevant; **S2** = latent drift that
will become a defect; **S3** = maintainability/dead code.

### Theme A — hand-maintained tables that must be updated by memory (S2)

| Finding | Where | Consequence |
|---|---|---|
| Six hand-maintained tool-name maps, including a 47-name copy in `agent/engine.rs` (`expected_live_names`) | agent audit §3.1 | A tool rename updates some maps and not others; the registry and the copy disagree silently |
| Telemetry label allow-list: 349 entries (55 event types, 294 operations), edited by hand for every new producer | `observability/telemetry_labels.rs` | **Was red on `main`** until seven missing labels were added; a new producer fails the suite until a human adds it |
| 16 `HookType` variants with no firing site | `hooks/types.rs` (agent audit §2.3, §4.4) | The type surface promises behaviour nothing implements |
| `catalogue.rs` ↔ `src/data/models.ts::MODEL_GROUPS` compared by text-scraping | gate `TG-P2-05` | Fields not scraped can drift freely; the test proves the scraper, not the models |
| `TG-P2-04` asserts only that the source *contains* `CappedProvider::wrap` | `src-tauri/src/ci_gate.rs` | The named provider gate cannot catch a new `match provider_id` copy — the hole the Groq drift passed through |
| Provider-id lists without construction: `vault_commands.rs` (`is_supported_provider`), `settings_commands.rs` ×2 | provider audit §4 | Three more places to forget |

### Theme B — one responsibility, several implementations (S2)

| Finding | Where | Note |
|---|---|---|
| `arcana_dir()` implemented twice plus three inline re-derivations | db audit F9 (`arcana-core/src/db/mod.rs`, `src-shared/src/db/mod.rs`, `db/backup.rs`, `memory/db.rs`, `session/storage.rs`, plus 2 in `src-tauri`) | A `.airs` fallback added to one copy desynchronises which root a component reads |
| Four path resolvers | agent audit §3.2 | Same question answered four ways |
| Three retry/backoff authorities with unrelated constants, one a hard-coded `sleep(60s)` that `shutdown()` cannot interrupt | fleet audit A1/A2 (`runner.rs:74-79,99-106`; `scheduler.rs:287-291`; `provider/retry.rs`) | Two policies can disagree about the same failure |
| Four dispatch gates for one concern | fleet audit A6 (`scheduler.rs:469,491,696,704`) | Divergent skip semantics; skips are silent |
| Four-to-five paths that write a worker's task status, several without build evidence | fleet audit A4/A5, §4.3–4.4; db audit F13 | Contends with the P4-14 transition boundary |
| `PRAGMA foreign_keys=ON` re-stated in four openers | db audit F5 | A new opener silently loses FK enforcement |
| Graph store bypasses `migrate::run_ladder` entirely (own version checks, no backup guard, `DROP TABLE` on a rung) | db audit F2 (`graph/migrations.rs:12-50,116`) | The one database the migration engine was built to protect is not using it |
| Graph timestamp triggers created on every open, outside any ladder version | db audit F3 (`graph/store.rs:65-89,122`) | Unversioned, unbacked-up schema mutation |
| Every table has two live schema definitions (baseline + `ADDED_COLUMNS`) | db audit F1 | Pinned by tests, but two hand-maintained column lists per table |
| Budget/spend guards stacked across ≥5 layers | provider audit §4 | Overlap not judgeable: one layer is the owner's frozen WIP |

### Theme C — writes outside the owning boundary, and instrumentation that can lie (S1/S2)

| Finding | Where | Consequence |
|---|---|---|
| `guard_and_validate` looks for the cargo sentinel where sandboxed builds never write, then flips an `in_progress` task to `build_failed` by an out-of-band DB write | fleet audit A4 (`runner.rs:1008-1064`; `hooks/ecosystem.rs:81` vs `process_sandbox/mod.rs:533`) | A successful tick can silently rewrite task state outside the journal, claim and build-evidence paths |
| Hook **infrastructure** failure is reported as a **policy denial** | agent audit §4.1 (`tool_executor.rs:1202-1209` ← `hooks/registry.rs:23,31`) | A DB-open failure reads to the user as "permission denied"; a named P4-51 clause |
| `journal_target_paths` returns empty for both "capture failed" and "genuinely pathless" | agent audit §4.2 | P4-53 clause: the journal cannot distinguish the two |
| Reconciliation envelope omits `outcome_unknown` | agent audit §4.3 | An unknown outcome must be expressed as something it is not |
| `do_not_retry` reconciliation skips `RecoverySnapshot::verify_state` | existing TG4-27 receipt | Safe, but conflict wording is enforced only on continuation paths |
| When the log directory cannot be prepared, logging degrades to console-only with **no writer and no counter** | TG4-24 receipt (`observability/logging.rs:102-111,169-178`) | "No telemetry sink" is indistinguishable from "no telemetry activity" in the Logs UI |
| Error class is encoded into the message text and parsed back out of the `Display` string | provider audit (`agent/engine.rs:1466-1474`) | The typed retry policy is bypassed; changing an adapter's wording silently changes retry behaviour |

**Security note (positive):** audit 4 traced every provider-boundary path and found **no
defect** — bodies are bounded (16 KiB), failures fail closed, keys stay in headers, and no
adapter echoes a provider body into an error or telemetry. Its one warning is that this safety
rests on adapter *authoring discipline*, not on a type.

### Theme D — tests and harnesses that cannot fail (S2)

| Finding | Where | Consequence |
|---|---|---|
| Process-global singletons installed first-wins, second install discarded, and the home they point at is a tempdir that gets deleted | fleet audit A7 (`analytics/mod.rs:136-141`; `runtime.rs:186-189`; `subagent/manager.rs:456-469`; `scheduler.rs:684`) | Order-dependent tests; this is the contamination surface behind the lost-ack trio. `scheduler.rs:1755-1770` already has the guarded pattern to copy |
| Test fixtures hand-write schemas that can drift from the ladder | db audit F10 (`hooks/db.rs:423-426`; `src-shared/src/db/analytics.rs:341-464`) | "A fixture that drifts from the migration proves the fixture, not the product" — the repo's own words |
| Tests share a process-global runtime cache without priming it | fixed today (`project_command` corpus) | Passed only because of test ordering |
| Production struct carries `#[cfg(test)]`/feature seams | fleet audit A8 (`runner.rs:358-414`) | Production shape defined by test needs |

### Theme E — verified dead code (S3)

`RecipeRun`/`RecipeRunStatus` (no construction site anywhere), `SubagentResult.files_modified`
(always empty but surfaced to the model), `cleanup_orphans` (deliberate no-op),
`_session_mgr` (stored, never read), `let _ = worker_opt`, `Tool::is_read_only` (zero call
sites), `hooks/registry.rs::fire_with_logs` (dead), `cost/tracker.rs::format_cost`/`format_tokens`
(no callers), `pricing.rs::is_priced`/`priced_models` (tests only), `mod dirs_next` (removed
today). Note: absence of a static caller is **not** proof of deadness in this repo (registries,
macros and Tauri's invoke surface can reach code dynamically), so each removal needs its own
named proof.

### Theme F — docs and gates that disagree with code (S3)

`db/mod.rs` store map (corrected today), `app_settings.rs` future-tense comment that describes
work already done, `catalogue.rs` "TG-P2-05 fails the build" when it is a `cargo test` failure,
and the project instructions citing `src-shared/src/cost/pricing.rs` where the file is
`arcana-core/src/cost/pricing.rs`.

### 3.7 Wave 2 (audits 5–6) — app shell / hooks / sandbox, and frontend / tooling / CI

**Security (S1, new, no gate covers it).** `hooks/executor.rs` has two actions that
sit outside the controls the rest of the product enforces, and I verified both by
reading the file:

- `HookAction::Http` (`executor.rs:99-147`) builds a `reqwest` client and sends to a
  template-expanded URL with caller-supplied headers and body. There is **no**
  `egress_consent` check anywhere in the hook path, while provider and MCP traffic is
  consent-gated. `substitute()` (`executor.rs:440-463`) expands the whole
  `HookContext`, which for a `PostFileWrite`/`PostToolUse` hook includes the tool
  input — so model-derived content can reach the URL.
  **Verified correction to the audit's wording:** the *body* and *headers* are not
  substituted (only the URL is, plus `Message` in the Log action), so "POST file
  contents" is not the mechanism — template expansion into the URL is.
- `HookAction::Log` (`executor.rs:149-165`) appends to a path from the hook's stored
  config with no workspace jail, while the sibling Shell action validates its command
  and working directory (`executor.rs:56-76`) and runs sandboxed. The `file` field is
  **not** substituted, so the path is user-authored and fixed.

  The severity turns on a trust question I could not settle from the code: hooks are
  authored through the UI (`hook_commands.rs` → `HooksDb::insert`), and I found no
  agent tool that creates one, so this is *user-authored* code rather than a
  model-reachable primitive. That makes it a deliberate-trust model — but the product
  declares an egress-consent model and this path bypasses it, and `TG4-30` enumerates
  containment cases without mentioning hooks. **Owner decision** (see §8.6), with my
  recommendation: route hook HTTP through `egress_consent` under a `hook:<id>`
  destination, and give Log the same jail as Shell (or document hooks explicitly as
  fully-trusted and add the gate to `TG4-30`).

**Tooling (S1, fixed today).** `npm run lint` was red on `main` — 95 `no-undef`
errors in three tracked tooling files — because the ESLint block carrying the globals
matched only `**/*.{ts,tsx}`. The red was load-bearing: `frontend-checks` → `build-smoke`
→ `e2e-smoke`, so **the e2e harness never ran on current main**. Fixed at the config
(`3ce44a5`): each file gets the globals of the context it runs in. The audit also notes
`e2e-smoke` is Linux-only and that `tsconfig.json` excludes `e2e/**`/`scripts/**` from
type-checking.

**Frontend drift (S2/S3).** `DEFAULT_MODEL`'s literal is re-hardcoded in five places
(`Chat.tsx:269`, `settingsStore.ts:88`, `sessionStore.ts:55`, `SessionBar.tsx:21`,
`AnnotationModal.tsx:83`) — the exact defect class `ci_gate.rs` says it already fixed
once. `formatBytes` exists five times with inconsistent output, token K/k casing
disagrees between two components, the cost `toFixed` ladder is byte-identical in
`TabBar.tsx` and `Sidebar.tsx`, and `Chat.tsx:28-46` carries a 17-entry context-window
table of which 16 entries are retired models. Dead UI clusters (plan/memory/plugin
components and stores, city-viz files, four CSS files) and a dead `providerForModel`
export are enumerated in the audit.

**IPC contract drift.** 255 commands registered, ~200 invoked, exactly **one**
invoked-but-unregistered: `navigate_to_logs` (`AgentLiveCard.tsx:110`) — a user-visible
"Open in Logs" affordance that is a logged no-op. The audit explicitly corrected a
prior audit: `src/types/live.ts` payload types are **not** orphans (they are consumed
by `agentLiveStore.ts`). Also: `src-shared/` is a Rust crate, not a shared TS layer.

**Gate weakness confirmed independently.** `TG-P2-05` compares only model *ids*
(`ci_gate.rs:3628-3683`); provider, label, context window, output cap, vision and
reasoning flags in `catalogue.rs` vs `src/data/models.ts` are compared by nothing. Two
audits found this from different directions.

**Doc drift found by audit 5 (not yet fixed — needs care, see §8.7).** Three `P4` rows
carry a proven-sounding token while unchecked (`P4-18` `GATE-PROVEN`,
`P4-51` `UNIT-PROVEN`, `P4-56` `SECURITY-PROVEN` at `MASTER_DEVELOPMENT_PLAN.md:1469`,
`:1541`, `:1548`); `_verify/A1-P0-code-audit.md:363` names a command that no longer
exists (`check_autodev_instructions_exists` → `check_arcana_instructions_exists`); and
`src-tauri/src/db.rs` plus 26 siblings are one-line `pub use arcana_core::*` veneers
with no stated retirement condition.

### 3.8 The project-command / "single strong tool set" thread (grounded 2026-10-18)

**A correction first.** The ruling points at `P4_EXPANDED_GOAL.md` for the project-command
isolation requirement; a search of that file finds **no** sandbox, project-command or tool-surface
text. The requirement lives in MDP **P4-61** (`MASTER_DEVELOPMENT_PLAN.md:1556`: *"Treat project
code, tests, dependencies, build scripts, and all subprocesses as untrusted"*) and in the
`_verify/P4-61-*.md` receipts. The direction itself is the ruling recorded in §8.1.

**What already exists — the half of the mandate that is built.** The model cannot hand
`project_command` a command line. Its schema (`project_command.rs:214-246`) accepts only a declared
intent from a closed set (`install|build|test|lint|run`), an optional toolchain and package, a
timeout and a project path; the argv is constructed by `platform/command.rs::resolve` from
`(Toolchain, CommandIntent)`, never parsed from a string; the call is validated against the
workspace, the detected toolchains and a current task claim (`project_command.rs:287-321`); the
sandbox is entered at `process_sandbox::spawn` (`mod.rs:266`) with per-platform enforcement; and
`intent=install` is classified `ToolEffect::HostApproval` (`:198-204`). So "vetted, intent-based,
sandboxed, approved" is already true of *this* tool.

**What is actually missing, verified:**

1. **Exclusivity — there is a second, untyped path.** `bash.rs` builds `cmd /C` / `sh -c` from raw
   model-supplied text (`bash.rs:119-151`), enters the same sandbox, and is classified
   `ToolEffect::FreeformShell` (`:28`). The "single strong tool set" does not exist while the model
   can also emit a command line.
2. **No capability object.** `ProcessSpec` (`mod.rs:116-125`) carries program/args/cwd/env/
   read-only paths — nothing about network or capabilities. Vetting is by effect class and tool
   *name*, not by what the command is allowed to do.
3. **A bypass.** `run_ide_project` (`project_command.rs:43` → `:116`) calls the launcher directly,
   skipping the task claim and build-evidence check, and is Tauri-exposed
   (`src-tauri/src/commands/ide_commands.rs:32`).
4. **The CLI's consent model is implicit.** `src-cli/src/commands/fleet.rs:88` sets
   `host_approval_available: false`, so `Execute` runs unprompted in `Autonomous`
   (`tool_effect.rs:95`) — a deliberate headless posture that is nowhere declared as one.
5. **Consent is keyed to the tool's name** (`tool_executor.rs:702,1506`), not to a capability, so a
   standing allow on one intent cannot be distinguished from a standing allow on another.
6. **The existing tests pin the internal launcher, not the entry point** — `run_resolved_command*`
   (`project_command.rs:1145`, `:1153`, callers at `:2526`, `:3137`, `:3182`), which is the gap
   `_verify/P4-62-NEXT-PROOF-2026-10-09.md` §5 already records.

**A new workstream (W9) is scoped around this**: one vetted command surface, capability-scoped
consent, and a declared approval model per front end — not a new abstraction layer. That is the
proposal in §8.7.

**What the receipts leave unproven, so nobody reads them as green:** Windows Cargo Clippy still
cannot canonicalize the workspace (`OS error 5`) and locked-dependency staging is absent; on Linux
Landlock confines **writes only, not external reads** (`P4-61-LINUX-NETWORK-BOUNDARY-2026-10-05.md:18`),
cgroup quotas are proved only as a WSL cost-only candidate
(`P4-61-DELEGATED-QUOTA-PROOF-2026-10-12.md:78`), and staged copy-back is test-only with no
production consumer (`P4-61-COPYBACK-APPLY-2026-10-13.md:31,45`); macOS is fail-closed with
`execution_enabled:false` (`P4-61-MACOS-NATIVE-SLICE-2026-10-09.md:15,33`); the loopback smoke
runner is unintegrated; and `WorkspaceWriteLease` serializes managed writers only — not
`Execute`/`FreeformShell`/subprocesses (`P4-61-WRITE-LEASE-REVIEW-2026-10-12.md:30`).

## 4. Already landed (wave 0) — no approval needed, done at the owning layer

Eleven commits, each with its own failing-then-passing evidence: `49e87e0` `eea621e`
`c00a6f0` `ce909be` **`a2e7aa8`** `1e03e08` **`894bbe0`** `5953f8c` `60c5534` `3ce44a5`
`a875007`. Two of those are production defects: the leaked worker slot (`a2e7aa8`) and the
duplicated provider dispatch that locked Groq out of the fleet and subagents (`894bbe0`).
Two clear pre-existing reds: the telemetry-label allow-list on `main`, and `npm run lint`
(`3ce44a5`, 95 → 0 errors), which had been skipping `build-smoke` and `e2e-smoke`. See
`_verify/2026-10-18-RED-BASELINE-AND-AUDIT-FIXES.md` and ADR-LOG-RESTRUCT-0397/0398.

## 5. Proposed workstreams

Each is independently shippable, leaves the tree green, and carries its own proof. "OD" = needs
an owner decision before work starts.

| WS | Scope | Owning layer | Size | Risk | Proof | OD |
|---|---|---|---|---|---|---|
| **W1** | Mechanical guards for the theme-A tables: one tool-name source + scan; hook-firing scan; make `TG-P2-04` fail on any provider construction outside `factory.rs`; derive `is_supported_provider` from `factory::supported_ids()` | `agent/`, `hooks/`, `ci_gate.rs`, `src-tauri/commands` | M | Low (tests only) | New guard tests + existing gate suite | — |
| **W2** | Delete verified-dead code, one unit per commit, each with a no-caller proof that accounts for registries/macros/invoke | theme E list | S–M | Low | `cargo check` + the owning module's tests | — |
| **W3** | Collapse theme-B duplicates onto one owner: single `arcana_dir`; one path resolver; one retry policy (incl. the uninterruptible 60 s scheduler sleep); one dispatch gate; one task-status writer fed by build evidence; one opener helper that sets every pragma | `db/`, `platform/`, `recipes/`, `agent/` | L | Med | Owning module suites + a gate run in the touched area | — |
| **W4** | Theme-C honesty: distinguish hook infrastructure failure from policy denial (P4-51); add `outcome_unknown` and a "capture failed" journal value (P4-53); surface "no telemetry sink"; decide the fate of `guard_and_validate` | `hooks/`, `agent/`, `observability/`, `recipes/` | M | **Med-High** (visible behaviour) | New negative tests + TG4-24/27/28/30 re-runs | **OD** |
| **W5** | Test-harness integrity: adopt the guarded once-per-process singleton pattern; make `install_global` return `Result`; derive fixtures from the real ladders | `analytics/`, `recipes/`, `hooks/`, `src-shared/` | M | Med | The previously order-dependent suites run in one process | — |
| **W6** | Theme-F doc/gate truth, including a generated (not hand-maintained) telemetry label set | docs + `observability/`, `src-tauri/ci_gate.rs` | S–M | Low | Doc no-stale sweep + suite | **OD** for generation |
| **W7** | Wave 2 (audits 5–6): one `DEFAULT_MODEL`/provider default; one formatter set with a pinned K/M casing contract; retire the stale `MODEL_CONTEXT` table and the retired-family branches in `shortModelName`; delete the dead plan/memory/plugin/city clusters and 4 CSS files after a caller re-check; wire or remove `navigate_to_logs` | `src/**` | M | Low–Med | `npx tsc --noEmit`, `npx eslint .`, `npx vitest run`, `npm run build` | — |
| **W8** | Hooks/sandbox: consent-gate `HookAction::Http`, jail `HookAction::Log`, distinguish infra failure from denial in `fire`/`fire_with_logs`, add the hook boundary to `TG4-30` | `hooks/`, `egress_consent`, `ci_gate` | M | **Med-High** | New refusal tests + `TG4-30` update | **approved** |
| **W9** | One vetted command surface (§3.8): make the intent-based tool the model's only way to run a project command; give consent a capability identity instead of a tool name; make `run_ide_project` pass the same claim/evidence check; declare each front end's approval model (desktop consents, CLI headless) instead of implying it | `project_command.rs`, `bash.rs`, `tool_effect.rs`, `tool_executor.rs`, `platform/command.rs`, `src-cli` | **L** | **High** (changes what the model may do) | New refusal tests; `TG4-30`; the intent-based suite re-run through `execute`, not the internal launcher | **OD** (§8.7) |

**Sequencing proposal:** W1 → W2 → W6 (all no-behaviour-change) → W3 → W5 → W7 → W4 → W8.
Do **not** run two workstreams that touch the same files concurrently.

## 6. Guardrails for any refactor

1. One workstream at a time, one commit per logical unit, never two writers in one file.
2. Before and after any workstream, run the owning gate(s) for every area it touches — and re-run
   the P4 gates listed in `TESTING_GATES.md`, because these refactors touch files that open P4
   clauses rely on.
3. No refactor may be used to *infer* a P4 gate green; gates are closed only by their named proof.
4. Receipts under `_verify/`, ADR entries at decision time, and doc lines fixed in the same step
   as the change that staled them (§0 no-stale).
5. Destructive-schema work (graph ladder port, dual-format retirement) requires a backup of any
   live database first, per §0.8 — and an explicit owner decision.
6. The owner's uncommitted WIP files stay frozen until the owner says otherwise.

## 7. Excluded from this plan until the owner rules

`arcana-core/src/platform/process_sandbox/mod.rs`,
`platform/process_sandbox/linux_smoke*`, `provider/run_budget.rs`, `e2e/wdio.conf.ts`, `native/`,
`scripts/build-macos-sandbox.sh`, `docs/restructurization/development/P4_EXPANDED_GOAL.md`, and
the untracked session-checkpoint receipts.

## 8. Owner rulings (2026-10-18) and what remains open

1. **The uncommitted WIP is *in scope*, not frozen.** Ruling: *"they were part of the refactor —
   most of the work there is trying to create a sandboxed system where every project command is
   vetted against a system … so rather than an agent running cmd, it's running a single strong
   tool set."* That is the direction this plan must serve, so the target shape is: `project_command`
   stops being a general command runner and becomes a **vetted tool**, and the sandbox/capability
   system is the thing that vets it. Grounding: `P4_EXPANDED_GOAL.md` and MDP **P4-61** (untrusted
   project code, sandbox, toolchain staging, install approval), with the owner's receipts in the
   untracked `_verify/P4-61-*.md` set. Consequence for this plan: W5/W6 are re-scoped around that
   boundary — a capability the code cannot name is a capability the sandbox cannot deny.
2. **Push: approved and executed.** `6832bc5..4598e1f` is on `main`; `P4_EXPANDED_GOAL.md` §24 also
   gives standing authorization for verified work. Everything pushed was individually validated,
   and no push rewrites history.
3. **All four escalations approved** — now authorized workstream items, not questions:
   (a) **structural `ErrorClass` — DONE** (`ADR-LOG-RESTRUCT-0399`, commit `ad56120`; it closed
   `F-13`, a recorded high-severity finding, in `_verify/A9-P3-claims-refutation.md`);
   (b) generate the telemetry label set instead of hand-maintaining 349 entries; (c) port the graph
   store onto the shared migration ladder and version its triggers, **with a backup guard**;
   (d) decide and act on `guard_and_validate`'s sentinel check.
4. **Priority: continue P4 in parallel** with the no-behaviour-change workstreams.
5. **Hook trust posture: consent-gate them.** Persisted hooks must obey the same consent and
   jailing as provider/MCP traffic and the Shell action, so **W8 is a security fix, not a
   documentation change**.
6. **Still open (§3.7):** may I correct three `P4` status tokens that read `GATE-PROVEN` /
   `UNIT-PROVEN` / `SECURITY-PROVEN` while their rows are unchecked, plus the stale command name in
   `_verify/A1-P0-code-audit.md:363` (no checkbox changes), and may the 27 `src-tauri` `pub use`
   re-export veneers be given a stated retirement condition or removed?
7. **How the freeform path is retired (§3.8, W9).** The mandate is now clear — one vetted tool set —
   but the mechanism decides a capability change, so I have not touched it. The options, in my
   order of preference:
   1. **Consent-gate + capability-scope the freeform shell** (keep it available, but only through an
      explicit approval that names what it will be allowed to touch). Least likely to strand an
      existing workflow; the approval still has to mean something.
   2. **Remove the freeform tool from the model's set entirely** and route every command need
      through the intent-based tool, extending the intent set where something real is missing. This
      is the purest reading of the mandate, and it changes what the agent can do unprompted — hence
      the ask.
   3. **Keep both but make the vetted tool the default and record which path ran**, so the split is
      visible while it is being closed. Weakest, and it leaves the untyped path in the model's
      reach.
   Which one — and if (2), which intents must exist first so no real workflow loses its only route?
