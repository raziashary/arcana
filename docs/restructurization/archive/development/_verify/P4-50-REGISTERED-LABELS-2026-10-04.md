# P4-50 — registered telemetry-label preservation

Status: scoped fix proven, 2026-10-04. **P4-50/TG4-24 remain open.**
Owner decision: preserve only exact registered telemetry labels (ADR-0278).

## Failure and correction

The production redactor classified alphanumeric/underscore/hyphen strings of
32+ characters as possible keys. Desktop JSONL consequently replaced fixed
`scheduler_recoverable_work_check` and `database_schema_migration_ladder`
operation labels with `[REDACTED_KEY]`. Earlier capture-layer tests did not use
that writer and missed the loss.

The shared JSONL writer now runs existing generic redaction first, then restores
only exact registered `event_type` and `operation` values on Arcana telemetry
records, including their current span and parent-span list. The source-controlled
private catalogue contains 50 event labels and 287 operation labels. Unknown
strings, foreign targets and sensitive fields remain under existing redaction.
The test-only syn inventory checks literal producers, fixed timing-helper calls
and the analytics grouping match against the sorted unique catalogue. It does
not infer arbitrary dynamic labels or register model-controlled content.

## Real producer and consumer proof

- `cargo test -p arcana-core --lib observability:: --locked -- --test-threads=1`:
  Windows 27/27; Ubuntu WSL 28/28 (includes Unix-specific retention test).
  The production nonblocking JSONL writer test checks long event/operation names,
  current/parent span identity, credential fields, unknown secret-like labels and
  foreign targets. The source inventory test passes on both hosts.
- `cargo test -p arcanactl --test consent_cli
  cli_structured_writer_preserves_registered_labels_through_full_export --locked
  -- --exact --test-threads=1`: Windows 1/1 (1.18s), Ubuntu WSL 1/1 (0.49s).
  The actual CLI process initializes its production logger and WorkersDb;
  a second CLI process exports the full JSONL stream. Its consumer sees the
  measured successful workers migration and initialization labels intact.
- Real `npm run tauri dev`, isolated profile
  `%TEMP%/arcana-p4-label-smoke-c8b06395222847c590127c5f59cdf430`: 1,385 JSONL
  events, 61 identifiable migration spans, 15 worker initialization spans,
  seven idle recoverable-work checks (one per seeded worker), zero provider-call
  events. Original readable/JSONL files and `telemetry-label-smoke-summary.json`
  remain there. Arcana exited and port 1420 has zero listeners.
- Workspace check and format/diff checks pass. No live provider credential or
  Calendar directory was used.

## Limits

This proves registered identity survives the real shared writer and CLI reader;
it is not a live Fleet-to-Logs UI completion proof. Generic UUID-shaped
session/run/call/trace fields are still redacted pending the owner's separate
correlation-field decision. Broader producer coverage, retention, measurements,
strict process sandbox corpus and successful CLI Fleet remain core P4 work.
