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

### Phase 1 — Validation tools (immediate wins, lowest risk)

- [ ] **T1.1** — `src-tauri/src/agent/builtin_tools/write_file.rs` — call `validate::tier2_imports` after every `write_file` on `.js/.jsx/.ts/.tsx/.py` files, in addition to the current `tier1_syntax`. Append the result to the tool response (same format as `format_inline()`). Cost: one extra in-process Rust walk per write. No subprocess.

- [ ] **T1.2** — `src-tauri/src/agent/builtin_tools/validate.rs` — add `tier1_syntax` coverage for `.ts/.tsx` via `tsc --noEmit --allowJs --isolatedModules` when `tsc` is on PATH; degrade to a warning when absent (mirroring the existing Python/Node fallback). Currently TS is silently skipped, which is why the modal and hook never get parsed.

- [ ] **T1.3** — `src-tauri/src/agent/builtin_tools/validate.rs` — add a new `tier2_5_exports` pass that, given a JS/TS file, parses its `export` symbols and verifies every sibling `import` from that file matches a real export. Pure Rust regex pass over the file tree, no subprocess. Catches `isDeleting`-not-returned, empty-`module.exports`, prop-type drift.

- [ ] **T1.4** — `src-tauri/src/agent/builtin_tools/check_file.rs` — flip the default `checks` from `["syntax", "imports"]` to `["syntax", "imports", "exports"]`. Make `types` opt-out via a `--no-types` flag rather than opt-in for any file with a `tsconfig.json` nearby.

### Phase 2 — Worker recipe hardening

- [ ] **T2.1** — `src-tauri/src/recipes/defaults.rs` (worker_frontend_recipe, ~line 594) — **fix the scaffold**: write `index.html` to the project root, not `public/`. Vite's default expects root-level `index.html`. The current recipe is the bug factory.

- [ ] **T2.2** — `src-tauri/src/recipes/defaults.rs` (both worker recipes) — replace the prose "MANDATORY before any status update" with an explicit precondition check the worker must `bash`: `bash('npm run build 2>&1 | tee build.log; echo "EXIT:$?"')` and **echo the EXIT line back in the same turn** as the `update_task_status(qc_review)` call. The supervisor recipe can then refuse the transition when EXIT != 0.

- [ ] **T2.3** — `src-tauri/src/recipes/defaults.rs` — add a "smoke" sub-step to both worker recipes: after `npm run build` succeeds, run a 5-second `bash('timeout 5 npm run dev > smoke.log 2>&1; grep -E "(error|EADDR|cannot find)" smoke.log || echo SMOKE_OK')` and require `SMOKE_OK` before transitioning to `qc_review`.

- [ ] **T2.4** — `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` — when target status is `qc_review`, reject the transition if the agent has not invoked `bash` with a recognised build command (`npm run build` / `cargo build` / `uv run pytest` / `go test`) within the last N tool calls of this session. Return a clear error: `cannot move to qc_review without recorded successful build`.

### Phase 3 — Cross-worker handoff

- [ ] **T3.1** — `src-tauri/src/agent/builtin_tools/` (new file `write_interface_spec.rs`) — tool the backend worker must call before `qc_review` on any task containing the word `route`, `endpoint`, `API`, or `handler` in its acceptance criteria. Writes `<workspace>/INTERFACE.md` with one row per route: METHOD, PATH, request shape, response shape (JSON example).

- [ ] **T3.2** — `src-tauri/src/recipes/defaults.rs` (worker_frontend_recipe Step 3) — insert "**Step 3.0**: If `INTERFACE.md` exists in the workspace, read it before writing any API consumer (`apiClient`, `fetch`, `useEvents`-style hooks). Match request URLs and response shapes against it exactly." Make this a discovery-budget-exempt read.

- [ ] **T3.3** — `src-tauri/src/agent/builtin_tools/validate.rs` (new `tier2_6_api_contract`) — given a frontend file that calls `apiClient.<verb>` or `fetch(`, extract the URL strings and verify each appears in `INTERFACE.md`. Pure Rust pass.

- [ ] **T3.4** — `src-tauri/src/agent/builtin_tools/analyze_diff_tool.rs` — when both `INTERFACE.md` and frontend consumer changes are in the diff, add a structured `interface_drift` field to the analyzer's output. The QC agent can then surface it as a hard-fail signal rather than free-text.

### Phase 4 — Fleet smoke test (forward-looking, 1–2 weeks)

- [ ] **T4.1** — `src-tauri/src/recipes/defaults.rs` (new `fleet_smoke_recipe`) — a recipe that runs after all worker tasks of an epic reach `qc_review`. Steps: install (`npm install` + `uv sync`), build (`npm run build`), start the backend (background), start the frontend dev server (background), curl one endpoint round-trip, screenshot/HTML-snapshot the root URL, look for an HTML element matching a fingerprint string set by the supervisor (e.g. `"Add Event"`). Pass/fail wired into the epic's task graph.

- [ ] **T4.2** — `src-tauri/src/agent/builtin_tools/` (new `e2e_probe.rs`) — bash wrapper that takes `{start_cmds, probe_url, expected_text, timeout_s}` and returns structured pass/fail. Built so the smoke recipe can call it without composing brittle inline bash.

- [ ] **T4.3** — `src-tauri/src/recipes/defaults.rs` (qc_recipe) — require the QC agent to invoke `e2e_probe` (or read its persisted result) before approving any epic that contains both a backend and a frontend worker task.

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
