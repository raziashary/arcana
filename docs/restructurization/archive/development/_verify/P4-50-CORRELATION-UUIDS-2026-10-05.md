# P4-50 correlation UUID preservation — 2026-10-05

Owner ADR-0302 approves a narrow exception to structured-value redaction. The
real JSONL writer preserves canonical lowercase hyphenated UUIDs only in
`correlation_id`, `fleet_correlation_id`, `trace_id`, `run_id`, `session_id`,
`call_id`, and `tool_call_id`, and only on Arcana telemetry targets. The same
rule covers event fields and current/parent span records. Unknown fields,
noncanonical IDs, sensitive strings and foreign targets retain redaction.

These seven names have production producers and correlation-reader consumers.
No speculative parent-ID field was added. Existing registered operation/event
label preservation remains intact. This does not make arbitrary telemetry
content safe to record or close the full observability gate.

## Verified Windows behavior

- `cargo test -p arcana-core --lib --locked structured_writer_preserves_only_canonical_correlation_uuids_for_arcana_telemetry`: 1 passed.
- `cargo test -p arcanactl --test startup_retry_logs --locked`: 1 passed.

The integration uses the process-global production logger and actual CLI export
subprocess. Its reader verifies that all seven independently generated UUIDs
survive persisted JSONL redaction and export. The same test still proves one
permanent worker-startup event with actual durable run/status readback.

Implementation and exact execution were delegated to GPT-6 Luna High; main
reviewed the diff. Only logging.rs and the existing CLI integration target were
changed for this unit. No provider, live database, listener or Calendar access.
P4-50/TG4-24 remain open for the full PA/Fleet/UI, reconciliation and platform
workflow evidence. Linux follow-up and remaining units are tracked separately.
