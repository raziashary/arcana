# P4-56 / TG4-30 — the pinned-Ruff install is behind the task-claim boundary — 2026-10-20

Closes one named TG4-30 bypass: a worker process effect that could begin without a task claim.

## Finding

`ProjectCommandTool::execute` handled `intent: install, toolchain: python, package: ruff` by returning
`install_pinned_ruff_for_host()` immediately, **before** `require_current_task_claim()` (which sat
further down, before the sandboxed project command). That branch is not a pure refusal: on Windows it
performs a network download and writes into Arcana's verified Python tool cache
(`platform::python_runtime::install_ruff`). A worker with no claim, a stale claim, or a claim owned by
another worker could therefore start a host mutation.

TG4-30 states the boundary directly: "worker file/process writes cannot begin without a task claim and
attributable recovery boundary". This was the recorded residue from the P4-19/20 work.

## Fix

The claim is now required at the effect boundary, inside the ruff branch:

```rust
if intent == CommandIntent::Install
    && input["toolchain"].as_str() == Some("python")
    && input["package"].as_str() == Some("ruff")
{
    if let Err(message) = ctx.require_current_task_claim() {
        return ToolResult::error(message);
    }
    return install_pinned_ruff_for_host().await;
}
```

The existing check before the sandboxed project command is unchanged. The plain install path remains a
pure refusal (`sandbox_violation` / network / deny) and keeps its tested envelope.

### Why the check was not hoisted to the tool entry

Hoisting was tried first. It inverted error precedence for pure refusals: `dependency_install_fails_closed_until_locked_staging_exists`
and `project_install_cannot_select_an_arbitrary_python_package` assert the install denial envelope for
a caller whose task row does not exist, and hoisting made both return "the claimed task no longer
exists". That is a behavior change beyond the defect, and a claim in front of a pure refusal proves
nothing. The fix is therefore scoped to the branch that can have an effect.

`require_current_task_claim` still returns `Ok` for the `supervisor` role by design, so Supervisor and
IDE paths are unaffected.

## Test added

`project_command_process_is_refused_before_a_task_claim` now also drives the ruff install from an
unclaimed worker (`task_id: None`, role `worker-backend`) and asserts the claim refusal ("Worker
mutations require a task claim") rather than a tool-operation or installer failure.

## Falsification (each reverted)

| Probe | Result |
|---|---|
| Restore the pre-fix ordering (check only after the ruff branch), installer replaced by a guard returning a distinct message | Failed: `installer reached before the claim boundary` — the unclaimed request reached the installer. |
| Keep the ordering, remove the branch's claim check, same guard | Failed with the same message — the new assertion is what catches it. |

Both probes replaced the installer before running, because a falsified run would otherwise download and
write to the real tool cache.

## Validation (Windows)

- `project_command_process_is_refused_before_a_task_claim` → **1/1** (0.3 s).
- `agent::builtin_tools::project_command::tests` → **37 passed / 1 failed / 2 ignored** (547 s,
  `--test-threads=4`); the failure is the documented `checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher`
  Python-staging failure under parallel load.
- Same test serially → **1/1** (110.9 s), confirming the documented non-regression rather than a
  regression from this change.
- `cargo check -p arcana-core` clean; rustfmt clean.

## Deliberately not claimed

- The pinned installer's own download verification was not audited here.
- P4-56 stays `PARTIAL`: the wider failure-containment review (tool error reflection, retry
  amplification, cancellation races, checkpoint path/task substitution, stale reassignment, sandbox
  escape, telemetry leakage, retention path traversal) remains open under TG4-30.
- No live provider, no user database, no network (the installer never ran in any probe).
