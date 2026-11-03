# TG4-24 / P4-50 — telemetry persistence failure injected through a real Fleet run — 2026-10-20

Closes the gate's injection clause for **persistence**: *"Inject telemetry persistence and export
failures and prove core task execution remains safe and its actual result is preserved."* Previous
evidence was producer-level (a writer that errors inside one test process); this is a real CLI Fleet
subprocess running the shipped degradation path.

## Injection method — filesystem, no test seam

`IsolatedProfile` already redirects `USERPROFILE`/`HOME` to a temporary profile. The test makes
`<profile>/.arcana/logs` a **file** containing `not a directory`, so `resolve_log_dir`'s
`create_dir_all` fails, `init_with_filter` returns console-only guards, and no writer of either
stream exists — exactly what a broken profile does in production. No `#[cfg(test)]` hook, no
injected trait object, no env-var switch in production code.

## What the run proves

`src-cli/tests/fleet_offline.rs::offline_fleet_cli_completes_when_telemetry_cannot_persist`, against
the real offline Fleet scenario (production scheduler, supervisor, worker, sandboxed Cargo build):

| Claim | Assertion |
|---|---|
| Core execution is unaffected | CLI exits successfully with an empty provider-credential environment |
| The work still happened | Epic `Done`, exactly one child `Done`, `assigned_worker == worker-backend` |
| The workspace is not disturbed | `changed_workspace_inputs` is empty |
| No partial telemetry | No `*.log` and no `arcana-events*` entry in `~/.arcana` |
| The failed sink is not half-replaced | The blocker is still a file and still reads `not a directory` |

## Falsification

Deleting the blocker line (so the sink is writable) makes the run create both the readable log and
the `arcana-events` JSONL, and the "no partial stream" assertion fails. Reverted. That proves the
injection is what the assertion measures — the test is not passing because the CLI ignores its home
directory or writes elsewhere.

## Export failures

Deliberately **not** injected through a Fleet run: a Fleet run performs no export. Export is a
user-initiated read of persisted files, and its failure contract is owned by
`write_log_export` — `log_export_failure_is_reported_without_creating_partial_output` proves no
partial output is left behind and the error is returned, and the Logs tab renders the failure
instead of swallowing it. Inventing a Fleet-time export would test an operation that does not exist.

## Test runs (re-run in the main session)

- `cargo test -p arcanactl --features offline-fleet-test --test fleet_offline --locked --
  --test-threads=1` → **7/7 passed** (41.2 s), including the new case.
- Focused rerun of the new case → 1/1 (19 s), twice: once as written, once with the blocker removed
  to confirm the failure.

## Deliberately not claimed

- Not a live-provider run. It does not close P4-49 (owner desktop acceptance) or the live milestone.
- The process-local health flag (`file_logging_unavailable`) cannot be read from another process by
  design, so this run proves isolation and artifact preservation, not cross-process flag readback —
  that is covered by `observability::logging` and `commands::log_commands` from the previous unit.
- `--quiet` suppresses the console layer, so the test does not assert a degradation banner in CLI
  output; it asserts the absence of any stream that would have been fabricated.

## Gate mapping

- **TG4-24** — persistence-failure injection is closed end to end; export-failure refusal remains
  proven at its owning command layer. The gate stays open for offline CLI memory/supervisor-lifecycle
  readback and the owner-authorized live-provider milestone.
- **P4-50** — stays `PARTIAL`; evidence block updated.
