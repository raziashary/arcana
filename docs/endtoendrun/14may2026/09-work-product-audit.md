# AutoDev improvements identified from a bad work product

Audited path: `C:\AIRS Foundry\autodev\testing` (17 files, ~750 LOC, calendar app from the 14 May 2026 fleet run).
Reference: original bug list preserved at the bottom of this doc. The specific calendar fixes are not the point — what follows is what the fleet itself needs.

## TL;DR

The calendar app shipped non-functional. The individual bugs (`/api/api/events`, `response.events || []`, missing React deps, `index.html` in `public/`, dead `controllers/`) are not interesting on their own. They are symptoms of three structural gaps in AutoDev:

1. **Workers can declare a task "done" without ever running the artifact.** `npm run build` would have failed instantly on the missing-deps `package.json`. The recipe demands it. Nothing enforces it.
2. **There is no contract between worker-backend and worker-frontend.** The backend returns a bare array; the frontend reads `response.events`. Each worker did its job in isolation — no shared spec, no handshake, no consumer-side fixture.
3. **`qc_review` runs on text diffs, not on a live system.** `analyze_diff` looks at the patch. It does not start the server, hit the API, or open the page. So 16 integration defects sailed through.

Everything below is a concrete change to a real file. None of it is about making the LLM write smarter code.

## Bug classes observed in the calendar output

- **Missing manifest entries** — `package.json` declares `express`/`cors`/`sqlite3`/`dotenv` only; the entire React/Vite/TS toolchain is absent.
- **Unrunnable scripts** — `"dev": "node server.js"` (no Vite), `"build": "node -c …"` (`-c` is a Bash flag, not a Node flag).
- **Cross-worker contract drift** — frontend posts to `/api/events` while `apiClient` already prefixes `/api`; frontend reads `response.events` while backend returns `[…]`; `id: string` on the FE vs SQLite integer on the BE.
- **Hook contract drift** — `App.tsx` destructures `isDeleting` that `useEvents` does not return; `CalendarMonth` accepts `onMonthChange` in JSX but its prop type doesn't declare it.
- **Dead architectural scaffolding** — `controllers/eventController.js` exports `{}`; `middleware/errorHandler.js` is implemented but never `app.use`'d.
- **Wrong file location** — `index.html` under `public/` instead of project root; Vite 404s on `/`.
- **Workspace escape** — three files written to `src-tauri/` instead of `testing/` (see `07-workspace-escape-rootcause.md`).
- **Zero smoke evidence** — no test, no boundary, no proof the dev server starts.

Every one of these is detectable by something cheaper than an LLM read.

## AutoDev capability gaps

### Gap 1 — `npm run build` is required by the recipe but unenforced

`src-tauri/src/recipes/defaults.rs:564-576` (worker_frontend_recipe) tells the agent in plain English to run `npm run build` and verify `dist/index.html` exists before marking `qc_review`. The agent skipped it. There is no runtime check that the bash command was actually invoked.

Manifested as: `package.json` shipped with zero frontend deps. A single `npm install` would have failed; a single `npm run build` would have failed louder. Neither was run.

### Gap 2 — `write_file` runs Tier 1 only; package consistency is never checked

`validate.rs::tier1_syntax` runs after every `write_file` and catches JSON/YAML/TOML/JS/Python syntax. `tier2_imports` exists and walks `package.json`/`requirements.txt` for missing deps — but it is `check_file`-only (Option B), opt-in. So when the worker wrote `App.tsx` importing `react`, `lucide-react`, etc., nothing flagged that `package.json` did not declare any of them.

Manifested as: zero frontend deps in `package.json` despite ~12 `import` statements demanding them.

### Gap 3 — No cross-file consumer/producer check

The frontend's `useEvents.ts:34` reads `response.events`. The backend's `routes/events.js:107` returns a bare array. Both files individually pass syntax and type checks. The mismatch lives at the boundary, which no current tool inspects. Same story for `isDeleting` (consumed but not produced) and `onMonthChange` (passed but not in the prop type).

Manifested as: the calendar would render empty even with events in the database; the delete spinner would never appear; month navigation never refetches.

### Gap 4 — Workers have no shared interface spec

`worker-backend` builds the API. `worker-frontend` builds the UI. They never exchange a contract artefact. The frontend worker simply guessed the response shape and got it wrong. There is no `INTERFACE.md`, no OpenAPI fragment, no TypeScript `.d.ts` shared between workers — only natural-language task descriptions written by the supervisor.

Manifested as: `/api/api/events` 404 (URL prefix assumed wrong), `response.events` mismatch (shape assumed wrong), `id: string` vs SQLite integer (type assumed wrong).

### Gap 5 — `qc_review` analyzes diffs, not runtime behaviour

`analyze_diff_tool.rs` LLM-scores a `git diff` and returns risk/classification/notes. It never executes the artefact. The calendar would have failed `curl http://localhost:5173/` and `curl -X POST http://localhost:5000/api/events` in the first two seconds — but no QC step ever attempts either.

Manifested as: 16 distinct integration defects passed QC review (or would have, had QC engaged at all — see `06-qc-agent-no-visibility.md`).

### Gap 6 — `bash.rs` doesn't surface non-zero exits as a status signal

`bash.rs` runs commands and returns stdout/stderr/exit-code to the agent text stream. The agent can read the exit code and choose to ignore it. There is no recipe-level enforcement that "if `npm run build` was just run and exit != 0, the next allowed status transition is `build_failed`, not `qc_review`."

Manifested as: nothing prevents the worker from running build, seeing a failure, then calling `update_task_status(qc_review)` anyway — or skipping the build entirely.

### Gap 7 — Recipes describe a scaffold but `glob('vite.config.*')` is the only "check"

`worker_frontend_recipe` (defaults.rs:582) prescribes a scaffold (vite.config.ts, public/index.html, main.tsx, tsconfig.json) and says "Run glob to check. If no result, create these files FIRST." It does not say *where* — `index.html` is supposed to be in `public/` per the recipe, which is wrong for Vite (Vite expects it at the project root). The recipe itself bakes in the bug.

Manifested as: `index.html` shipped under `public/`; Vite would 404 on `/`. The agent followed the recipe faithfully.

### Gap 8 — No "fleet handoff" phase between workers

The fleet has worker recipes and a QC recipe but no inter-worker integration step. Backend finishes → tasks marked `qc_review` → frontend starts. There's no phase where the frontend worker reads the actual backend response shape (e.g. by running it and curling it) before writing its consumer code.

Manifested as: every contract-drift bug in Gap 4.

## Atomic implementation tasks

### Phase 1 — Validation tools — ✅ COMPLETE 14 May 2026

- [x] **T1.1** — `src-tauri/src/agent/builtin_tools/write_file.rs` — post-write now runs Tier 1 syntax + Tier 2 imports + Tier 2.5 exports + Tier 2.6 api_contract back-to-back, with each non-clean result appended to the success message under a single `⚠ Lint feedback:` section. The calendar run's missing-dep / drifted-import classes are caught at the write that introduced them.
- [x] **T1.2** — `src-tauri/src/agent/builtin_tools/validate.rs` `check_tsc_syntax` — `.ts` / `.tsx` / `.jsx` files now hit Tier 1 via `tsc --noEmit --allowJs --isolatedModules --jsx preserve --target es2020 --module esnext --moduleResolution node`. `--isolatedModules` keeps the pass per-file (true syntax check; cross-file type stuff stays in opt-in Tier 3). Graceful degradation: when `tsc` isn't on PATH the worker gets a `warning` diagnostic + a successful write instead of a hard fail.
- [x] **T1.3** — `src-tauri/src/agent/builtin_tools/validate.rs` `tier2_5_exports` — pure Rust regex pass that, for every relative `import { X } from './foo'` in the file just written, resolves `./foo` to a real file (.ts / .tsx / .js / .jsx / index.*) and confirms `X` appears in the target's `export { … }` / `export const X` / `export function X` / `export class|interface|type X` / `module.exports = { X }` / `module.exports.X =` / `export default` set. Reported via `ImportResult` so the formatting matches the existing Tier 2 import check.
- [x] **T1.4** — `src-tauri/src/agent/builtin_tools/check_file.rs` — default `checks` is now `["syntax", "imports", "exports", "contract"]`. Schema enum expanded with the two new tiers; usage hint at "no checkers available" updated to list them. `types` stays opt-in (Tier 3 is genuinely slow).

### Phase 2 — Worker recipe hardening — ✅ COMPLETE 14 May 2026

- [x] **T2.1** — `src-tauri/src/recipes/defaults.rs` `worker_frontend_recipe` Frontend Scaffold section — `index.html` now lives at the PROJECT ROOT, not under `public/`. Recipe carries an inline "Vite's default expects root-level index.html" note + a `Doc 09 T2.1` reference so future audits don't recreate the bug.
- [x] **T2.2** — `src-tauri/src/recipes/defaults.rs` worker-backend AND worker-frontend Build Validation sections — both recipes now mandate the EXIT-echo form: `bash('<cmd> 2>&1 | tee build.log; echo "EXIT:$?"')`. The literal `EXIT:N` line must appear in the same tick as `update_task_status(qc_review)`. The `update_task_status` tool's T2.4 gate rejects qc_review transitions without recorded build evidence — recipe-level prose meets tool-level enforcement.
- [x] **T2.3** — `src-tauri/src/recipes/defaults.rs` worker-backend AND worker-frontend — smoke probe section added: `bash('timeout 5 npm run dev > smoke.log 2>&1; grep -E "(error|EADDR|cannot find|ECONNREFUSED)" smoke.log -i || echo SMOKE_OK')`. The grep is i-flag case-insensitive and covers connection refused too. Both `SMOKE_OK` AND `EXIT:0` are required before qc_review.
- [x] **T2.4** — `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` + new `build_evidence.rs` module — bash records every recognised build/test/smoke invocation in a session-keyed ring (`build_evidence::record` with a 15-min TTL). On `update_task_status(id, 'qc_review', …)` the tool calls `build_evidence::most_recent_build(ctx.session_id)`; rejects with a self-correcting error when no evidence exists OR the most recent invocation exited non-zero. QC reviewers are exempted via `ctx.agent_role` starts-with check so reviewers (who never run builds) can still move tasks to done/rejected. 6 unit tests cover the classifier + round-trip.

### Phase 3 — Cross-worker handoff — ✅ COMPLETE 14 May 2026

- [x] **T3.1** — `src-tauri/src/agent/builtin_tools/write_interface_spec.rs` — new builtin tool. Schema accepts a `routes` array (method, path, description, request_shape, response_shape, status_codes, auth_required, notes) plus an `overwrite|merge` `mode` and optional `conventions` free-text. Output is a structured `INTERFACE.md` with a Conventions header and one `### METHOD /path` section per route. Registered in `mod.rs` for both MCP worker registration and PA-mode registration; worker-backend recipe Step 3.5 mandates calling it before qc_review on routing tasks.
- [x] **T3.2** — `src-tauri/src/recipes/defaults.rs` `worker_frontend_recipe` — new Step 3.0 inserted between Step 3a and Step 3b, marked discovery-budget-exempt. Frontend reads `INTERFACE.md` BEFORE writing any API consumer; the file is described as the single source of truth.
- [x] **T3.3** — `src-tauri/src/agent/builtin_tools/validate.rs` `tier2_6_api_contract` — pure-Rust pass. Extracts URL strings from `fetch('…')` / `axios.get('…')` / `apiClient.post('…')` / `{ url: '…' }` patterns, normalises (drops query strings, collapses `${template}` expressions), and substring-matches each against the workspace's `INTERFACE.md`. Wired into both `write_file` post-write (silent skip when INTERFACE.md isn't there yet) and `check_file` (new `contract` tier in the defaults).
- [x] **T3.4** — `src-tauri/src/agent/builtin_tools/analyze_diff_tool.rs` `detect_interface_drift` — pure regex scan over the unified-diff `+++ b/path` / `--- a/path` markers. Emits a structured `interface_drift: { status, touched_spec, touched_consumer, spec_files, consumer_files }` field injected into the analyzer's JSON output. `status: "consumer_only" | "contract_only" | "both" | "neither"` — the QC reviewer recipe's Phase 4e calls out `consumer_only` / `contract_only` as hard-fail signals.

### Phase 4 — Fleet smoke test — ✅ COMPLETE 14 May 2026

- [x] **T4.1** — `src-tauri/src/recipes/defaults.rs` `fleet_smoke_recipe` — new persistent recipe (haiku, on-demand: `schedule: None`, kicked by the supervisor). Reads the epic's `start_cmds` / `probe_url` / `expected_text` from task metadata, installs dependencies (npm / uv / cargo), runs the build with the T2.2 EXIT-echo form, then calls `e2e_probe`. On success → `update_task_status(epic_id, 'done', 'fleet-smoke')`. On failure → `'rejected'` with the probe's `failure_reason`. Registered in `FLEET_SMOKE_RECIPE_ID` next to the other system recipes.
- [x] **T4.2** — `src-tauri/src/agent/builtin_tools/e2e_probe.rs` — new builtin tool. Spawns each `start_cmd` via `tokio::process::Command` with `kill_on_drop(true)`, waits `boot_wait_s` (default 8s) for ports to bind, issues a `reqwest` GET against `probe_url`, asserts the `expected_text` substring is in the body. Returns a structured `ProbeReport { success, probe_url, expected_text, status_code, response_body_excerpt, failure_reason, started_processes }` serialised into the tool response. Total timeout (default 30s) wraps the entire probe; teardown always runs even on timeout.
- [x] **T4.3** — `src-tauri/src/recipes/defaults.rs` `qc_reviewer_system_prompt` — Phase 4 rewritten to mandate `e2e_probe` BEFORE approval. Discovery steps for `start_cmds`/`probe_url`/`expected_text`, exact invocation, verdict branches with example rejection text. Phase 4e adds explicit guidance: `analyze_diff.interface_drift.status == "consumer_only" | "contract_only"` is a hard-fail signal even when other phases are green.

**Verification (full Phase 4 run):**
- `cargo check` clean (only pre-existing warnings)
- `cargo test --lib agent::builtin_tools::path_jail::tests` → **7 passed, 0 failed**
- `cargo test --lib agent::builtin_tools::build_evidence` → **6 passed, 0 failed** (new)
- `cargo test --lib agent::builtin_tools::e2e_probe` → **1 passed, 0 failed** (new)
- `cargo test --lib subagent` → **11 passed, 0 failed** (regression-free)
- `npx tsc --noEmit` clean (exit 0)

## What NOT to do

- **Do not try to make the LLM write better code per task.** The model wrote a clean Express+SQLite backend; it failed at the boundaries between files and between workers. Tuning prompts to "remember to declare your deps" is a treadmill — every prompt addition trades for another forgotten thing. The fix is detection.
- **Do not add LLM-based contract checking when a regex pass suffices.** `tier2_imports` already proves this works for `package.json`. The same approach scales to exports and interface specs without burning tokens.
- **Do not push smoke testing into QC's natural-language reasoning.** QC reads diffs. Smoke is binary: did the page render the expected text? It belongs in a dedicated recipe phase with a structured pass/fail signal that QC consumes, not generates.
- **Do not block on Phase 4 to ship Phase 1.** Phase 1 tasks (T1.1–T1.4) catch ~7 of the 16 calendar bugs at write time, in-process, with no new subprocesses. They are low-risk wins.

---

## Appendix — Original calendar bug list (preserved for reference)

| # | Location | Bug |
|---|----------|-----|
| 1 | `package.json:15-20` | Zero frontend deps declared. |
| 2 | `package.json:8` | `"dev": "node server.js"` — no Vite. |
| 3 | `package.json:9` | `"build": "node -c …"` — `-c` is a Bash flag. |
| 4 | `AddEventModal.tsx:83` | Double `/api` prefix → 404. |
| 5 | `useEvents.ts:34-39` | Reads `response.events`; backend returns bare array. |
| 6 | `App.tsx:19` | Destructures `isDeleting` the hook never returns. |
| 7 | `CalendarMonth.tsx:11` | Prop type omits `onMonthChange`. |
| 8 | `CalendarMonth.tsx:12` | Duplicate `currentDate` state vs `App`. |
| 9 | `useEvents.ts:31-32` | Date-only string vs `endTime <= ?` boundary bug. |
| 10 | `server.js` | `middleware/errorHandler.js` never `app.use`'d. |
| 11 | `controllers/eventController.js` | `module.exports = {}`. |
| 12 | `public/index.html` | Should be at project root for Vite. |
| 13 | `tsconfig.json` | No `vite/client` types. |
| 14 | `App.tsx:54` | `confirm()` without `window.` prefix. |
| 15 | DB ids | `id: string` FE vs integer BE. |
| 16 | `.env` committed | Listed in `.gitignore` but shipped anyway. |
