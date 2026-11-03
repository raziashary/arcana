# P4-55 / TG4-29 retention closeout — 2026-10-04

The shared retention policy is proven through file fixtures, production readers, and the real desktop entry point. This closes P4-55/TG4-29 only; core P4 remains active.

## Production startup

`npm run tauri dev` launched Arcana in an isolated profile:
`C:\Users\razia\AppData\Local\Temp\arcana-p4-startup-fixed-9d832a2b2397421f87dcd51902b1f0d8`.

The canonical log directory initially contained eight readable and eight JSONL files dated September 1–8, plus `foreign.log` and `arcana.invalid.log`. With `RUST_LOG=info`, the production logger recorded the startup prune summary: 16 files removed, 288 bytes reclaimed, zero cleanup failures. Both unrecognized files survived. The readable stream grew to 96,882 bytes and the structured stream to 712,522 bytes; all seven seeded workers recorded idle dispatches. No migration error or provider-call event was observed. The earlier `RUST_LOG=warn` launch deliberately filtered INFO readable records; it is not the evidence for the readable summary.

`arcanactl logs export` read the desktop-created current readable file and exported all 96,866 characters through the production export path. The native app/dev session was stopped; no Arcana process or listener on port 1420 remained. The disposable profile preserves the original logs, full export, and `startup-retention-proof.json`. No owner Calendar workspace or live API was accessed.

## Automated contract

- Shared retention suite: 7/7 on Windows and Ubuntu WSL. Covers recognized names, seven-day/five-file caps, malformed/foreign preservation, active-date precedence, open append handle continuity, Unix symlink refusal and cleanup failure summary/detail.
- Tauri `retention_keeps_the_active_date_discoverable_and_fully_exportable`: production pruner → Logs discovery → full 3,005-line export, 1/1 on Windows.
- Cleanup failure isolation is exercised by the public startup pruner under a captured subscriber; it emits failures and returns normally. The desktop smoke independently proves logger-before-pruner ordering and startup survival.

## Startup dependency fixed

The real smoke originally exposed concurrent TasksDb migration races. ADR-LOG-RESTRUCT-0276 records owner approval and the fix: per-rung SQLite `BEGIN IMMEDIATE` with version recheck; transactional legacy-column repair; bounded retry of WAL lock failures; unique automatic backup names, with extended Windows paths for deeply nested snapshots. Explicit backup names continue refusing overwrite. Concurrent seven-opener tests exercise both journal modes, preserve the pre-destructive row in exactly one snapshot, and run the production TasksDb opener.

Migration tests pass 18/18 on Windows and Ubuntu WSL. Backup tests pass 6/6 on both. The Windows DB regression suite passes 213/213, including the legacy fingerprint-evidence upgrade. `cargo check --workspace --locked` passes. Existing unrelated Windows Clippy sandbox failures remain a core P4 blocker; this closeout does not waive them.

## Observability follow-up

The desktop JSONL smoke exposed long fixed operation/event names being replaced by the generic structured-value secret heuristic. Short dispatch names and their worker/outcome fields remain readable, which is the startup evidence above. P4-50 owns fixing the label loss; the owner was asked about exact registered label preservation. It is not hidden by the P4-55 closeout.
