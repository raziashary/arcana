# TG4-24 / P4-50 producer→reader→UI event families + token reconciliation — 2026-10-18

Five focused tests added to `src-tauri/src/commands/log_commands.rs` (test module only,
strictly additive), extending the three previously proven producers to the remaining event
families the gate names; each drives a real producer then the actual Tauri reader commands:

1. `production_provider_call_retry_and_failure_events_persist_and_read_through_tauri_logs` —
   provider call / retry (success → retrying → stopped/QuotaExhausted) outcomes emitted through
   the production operation-timing layer read back with correct classifications and bounded,
   argument-free payloads.
2. `production_database_statement_events_persist_and_read_through_tauri_logs` — a real SQLite
   profiler event from the production `TasksDb` read back with normalized database identity and
   no SQL text/values. **Granularity contract kept honest:** the legacy `sqlite3_profile` hook
   has whole-second elapsed resolution (verified in the bundled amalgamation; the producer
   already documents `duration_observed` and the shipped core test asserts presence, not
   non-zero). The UI-level assertion therefore checks exact bucket accounting
   (`duration_count + duration_zero_count == count`, matching persisted nonzero durations) rather
   than over-claiming nonzero per-statement durations. An in-flight flake found while writing the
   test was root-caused to this granularity and fixed by asserting the real contract.
3. `production_worker_tick_lifecycle_events_persist_and_read_through_tauri_logs` — worker-run
   lifecycle (begin tick → phase update → progress record) emitted through the production
   RunsDb operation timers and read back.
4. `production_trace_correlation_links_parent_and_children_through_tauri_reader` — a parent
   operation plus child operations sharing `trace_id` persist and are correlated by the reader
   (the frontend tree build is covered by the existing `Logs.test.tsx` claims).
5. `jsonl_provider_tokens_reconcile_with_analytics_db_session_totals` — known token categories
   written through the JSONL producer and the production AnalyticsDb token writer reconcile:
   JSONL-metrics totals equal the persisted `get_token_usage_for_session` readback for the same
   session.

Validated: `cargo test -p arcana --lib --locked -- <five test names> --test-threads=1` → 5/5,
exit 0 (2026-10-18), rerun ×4; full focused module `commands::log_commands` → 40/40; `cargo
clippy -p arcana --lib` clean in this file; `rustfmt` applied and clean.

Still open for TG4-24/P4-50: telemetry-failure injection through the real Fleet path, trace
durations aggregated in the view, the remaining cost-measurement groups, the offline CLI memory/
supervisor-lifecycle readback, and the owner-authorized live-provider milestone.
