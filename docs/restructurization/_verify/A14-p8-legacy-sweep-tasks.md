# A14 · P8 legacy sweep — the proposed task list

Read-only sweep, 2026-09-03, for `P8 · Sidecar removal`. The auditor could not
write files, so this records the **actionable core** of its report; the full
narrative is in the session transcript.

⚠ **Verified in the main session before recording:** the `FallbackPolicy`
stranding defect (§Live), the `TG-P4-31` ordering constraint, and the dead
`paMode` argument. The rest is agent-reported and should be re-checked as each
task is picked up.

---

## Two things share the words "Mode 1 / Mode 2" and must not be conflated

| Concept | What it is | Status |
|---|---|---|
| `PAMode` (`FastDevelopment` / `Review` / `Delegate`) | interactive PA prompt + tool-set selector | **fully live — do not sweep** |
| `FallbackPolicy` (`Mode2Only` / `Mode1Capable`) | recipe dispatch policy against the retired sidecar | **inert mechanism, live landmine** |

`PAMode` selects the system prompt (`context.rs:89-93`) and the tool registry
(`builtin_tools/mod.rs:190-284`) on the live path. Deleting it would collapse
three prompts into one and hand the PA file+bash tools in a mode advertised as
having no file access. It is not sidecar residue.

## The live defect this sweep found

**Every user-authored recipe is permanently stranded.** `#[default] Mode2Only`
+ `#[serde(default)]` on `Recipe.fallback_policy` + a `RecipeEditor.tsx`
payload that omits the field ⇒ any recipe created in the UI deserialises as
`Mode2Only`, hits the guard at `runner.rs:717-737`, reports `fallback_idle`,
and never runs. And `update_recipe` is a **full overwrite**, so editing a
*shipped* recipe strands it too — silently undoing the `P4-31` fix.

This is a bug fix, not cleanup, and it does not have to wait for P8.

---

## Proposed tasks

⚠ = behavioural change needing an owner decision. ✅ = pure deletion.

### Phase A — unblock the gates (must be first)

1. ✅ **Re-point `TG-P4-31` before touching the runner.** `ci_gate.rs:2811-2828`
   reads the literal `"let orch_healthy = false"` and calls `panic!` if it is
   absent — deliberately, so reviving Mode 2 must be a visible decision. It
   therefore fails CI on the first deletion commit unless rewritten first.
2. ⚠ **Decide the `FallbackPolicy` disposition** and close the stranding
   defect above. Delete the field and its guard, or flip the default to
   `Mode1Capable` as a hotfix ahead of P8.
3. ✅ **Add a vocabulary gate** so P8 cannot half-land: no file outside
   comments may contain `Mode 2`, `LivingAgent`, `core-orchestrator` or
   `memory-mcp-server`. Tighten to include comments at the end of Phase F.

### Phase B — delete the Mode 2 path in core

4. ✅ Delete `runner.rs:173-174` (the discarded health probe and the
   `orch_healthy` pin) and the unreachable `if orch_healthy` block, lines
   276-702. Re-flatten the control flow.
5. ✅ Delete `runner.rs:108-110` `is_orchestrator_ready` (zero callers), the
   `orch_client` field, and the `RecipeRunner::new` parameter.
6. ✅ Delete `autodev-core/src/recipes/orchestrator.rs` entirely.
7. ⚠ **Restore Live-Monitor telemetry on the surviving path.** Steps 4-6 delete
   the only emitters of `worker:tick-start`, `worker:tick-end` and the only
   populated `tick_duration_ms` / `last_outcome`. Either re-emit around the
   native engine run, or delete the frontend consumers. The UI is *already*
   blank; this decides whether P8 fixes or formalises that.
8. ✅ Delete `register_for_mcp_worker` (`builtin_tools/mod.rs:100-180`, ~80
   lines, zero callers) and the two doc-comments citing it as live.

### Phase C — delete the sidecar host layer

9. ✅ Delete `src-tauri/src/sidecar/` (mod.rs 1271 lines, client.rs 1605,
   sse.rs, startup_progress.rs) and its `pub mod`.
10. ⚠ **Split `sidecar_commands.rs` before deleting it.** `list_mcp_tools`,
    `get_mcp_health` and `force_reconnect_mcp` are **not** sidecar
    functionality — they serve AutoDev's client connections to external MCP
    servers, which is live. Move them to `mcp_commands.rs` keeping the command
    names, then delete the other twelve.
11. ⚠ **Decide the startup overlay's fate** — deleting `startup_progress.rs`
    removes 8 of its 13 phases. Collapse to a native FSM or remove the overlay;
    a mismatch shows a spinner that never advances.
12. ⚠ **Decide Demo Mode's fate** — `runtime_config.rs`'s only field is
    `skip_sidecars`, which becomes meaningless.
13. ✅ Delete the auto-start block and dotenvy loading in `main.rs`
    (20-64, 185-188, 232-233, 247-248, 342-355, 359-363, 373-376, 378-530).
    **Keep `scrub_process_env()`** — see "do not delete".
14. ✅ Delete `EVT_SIDECAR_CRASH` / `SidecarCrashPayload` / `emit_sidecar_crash`
    and `KIND_ORCH_CRASH` / `KIND_WATCHDOG_RESTART` — no emitters remain. Fix
    `error_kinds.rs:15`, which points at a file that does not exist.
15. ⚠ `worker_commands.rs::list_workers` loses both model sources; confirm the
    DB column is populated on every path first or workers render blank.
16. ⚠ `update_worker_model_sync` becomes identical to `update_worker_model` —
    collapse them and update the caller.
17. ✅ Delete `rel_dir_is_sidecar` and the superseded `TG-P4-31`.

### Phase D — frontend

18. ✅ Delete `SidecarPanel.tsx`, `SubmoduleStatusPanel.tsx`,
    `SidecarCrashBanner.tsx` and their mounts.
19. ⚠ **Split `sidecarStore.ts`** — it carries dead sidecar status *and* live
    external-MCP health. Extract `mcpHealthStore.ts`, delete the rest, update
    four consumers.
20. ✅ Delete `SystemHealth.tsx` — no importer, and its content is a
    hardcoded mock reporting 99.8% uptime for a service being deleted.
21. ⚠ Per step 7, keep or delete the Mode-1/2 state in `live.ts`,
    `agentLiveStore.ts`, `AgentLiveCard.tsx`, `workerStore.ts`, `AutoDevPanel.tsx`.
22. ⚠ **Fix the dead `paMode` argument.** `agent_commands.rs:122-127` prefers
    `session.pa_mode`, which is `NOT NULL DEFAULT` and so never empty — making
    the frontend's argument unreachable and "Delegate to Fleet" a no-op.
    Either honour an explicit argument or drop the parameter and have the
    annotation flow call `set_session_pa_mode` first.

### Phase E — packaging, config, CLI

23. ⚠ **Decide `airsctl agent run|list|status|stop|logs`** — five subcommands
    that are pure HTTP to the orchestrator and cannot work after P8. Largest
    behavioural item in the sweep.
24. ✅ Delete `orchestrator_url` / `auto_start_sidecars` from the CLI config
    schema. Sequence after 23.
25. ✅ Delete `installers/wix/bundle-sidecars.ps1` and its call; drop `python3`
    from the deb and rpm manifests; revisit the dmg entitlement comment.
26. ✅ Remove both submodules, `src-tauri/resources/sidecars/` (225 MB,
    untracked), and the empty `sidecars/` and `build/` directories.
27. ✅ Strip `.env` to what the app still reads — after step 13, nothing.
    Cross-check `security/env_scrub.rs` so the scrub does not become a no-op.

### Phase F — prose (last, so the gate can be tightened)

28. ⚠ **Rewrite `BASE_SYSTEM_PROMPT`.** It contradicts itself — rule 8 says
    write `current_tasks/*.lock`, line 284 says never to — and describes
    GitHub-Issues coordination, `worker/<role>/<issue>` branches and a Python
    backend, none of which exist. Changes what the model is told every turn.
29. ⚠ Rewrite `DELEGATE_SYSTEM_PROMPT` — its worked example creates a task
    against the deleted sidecar.
30. ⚠ Reconcile `REVIEW_SYSTEM_PROMPT` with the fact that Review and
    FastDevelopment share both the registry arm and `ManualApproval`, so the
    "explicit permission checkpoint" it promises is not distinctive.
31. ✅ Fix `spawn_subagent.rs:112` and `spawn_qc_review.rs:93`, which tell the
    model to "wait for sidecar startup" — an event that will never happen.
32. ✅ Clean `defaults.rs` recipe instructions (lines 292, 321, 611, 1083,
    1225) and the eight duplicated P4-31 comment blocks.
33. ✅ Clean ~20 stale rationale comments naming deleted mechanisms.
34. ⚠ **Rewrite the in-app Docs tab** — ~106 legacy lines across six files.
    `docsAllD.ts:24-57` documents three mechanisms removed in P0. Shipped
    product documentation; needs an author, not a sweep.
35. ✅ Rewrite the README's dual-mode, Mode 2, architecture, submodule and FAQ
    sections, and drop references to `build/` scripts that do not exist.
36. ✅ Update the Delegate-mode description in `settingsStore.ts` and the
    PA-mode table in the docs to match step 29.
37. ✅ Tighten the step-3 gate to include comments; run it as the P8 exit
    criterion.

---

## Do NOT delete

- **`PAMode`**, and `sessions.pa_mode` — live on the interactive path.
- **`list_mcp_tools` / `get_mcp_health` / `force_reconnect_mcp`** — filed under
  "sidecar" by address only; they serve live external-MCP features.
- **The canonical `ToolRegistry`** built in `main.rs` — its old consumer is
  gone, but `list_mcp_tools` reads it to render the tool browser.
- **`KIND_INVOKE_FAILED`** — still emitted on the live startup-tick path; only
  its rationale comment is stale.
- **`scrub_process_env()`** — removing the dotenv load makes it *more*
  defensible, not redundant: it protects every child process `bash.rs` spawns.
- **`path_lock` / `path_jail` / `tool_effect` guards** — their comments justify
  themselves by naming Mode 2, but they protect current concurrent callers.
- **`git worktree` commands and `plan_items.worktree_branch`** — the
  user-facing git feature, unrelated to the old fleet's lockfile protocol. Do
  not sweep on a name match.
