# P4-57 procedural Memory chronology correction — 2026-10-10

## Actual defect and correction

The production SQLite search ordered `updated_at` as text before applying `LIMIT`.
A canonical `2026-10-10T09:00:00.000Z` therefore sorted above a legacy SQLite
`2026-10-10 12:00:00` even though noon is later. The shared row parser accepts both
encodings, so this was a selection error rather than a parse failure.

The isolated regression writes topics through `ProceduralStore::save_topic`,
retains canonical, legacy and offset-bearing historical update times, closes the
direct database handle, then calls the actual `ProceduralStore::search` reader.
Before the correction, Windows failed 1/1 in 0.25 seconds by selecting the canonical
09:00 topic at limit 1. The reader now uses the existing Memory instant-order
pattern: `julianday(updated_at) DESC, id DESC`. The regression proves noon wins,
23:00+14:00 ties with 09:00Z and uses stable ID order, returned values parse,
creation times retain production millisecond precision, and keyword/zero-limit
behavior is preserved.

This is within the owner-approved timestamp-reader corrections (ADR0302).
No dependency, schema, migration, public API or index changes were made.
The keyword search already scans substring matches and sorts; no performance
improvement is claimed. Independent GPT-6 Luna High review found no scoped defect.

## Execution and cleanup

`cargo test --offline -p arcana-core --lib memory::procedural::tests:: --locked -- --test-threads=1`

- Windows: 6 passed, 0 failed; 0.46 seconds.
- Ubuntu WSL: 6 passed, 0 failed; 0.55 seconds.
- `cargo check --offline -p arcana-core -p arcanactl --locked`: passes (16.91 seconds).
- Formatting, whitespace and scoped staged credential checks pass.
- Actual production write/search APIs are exercised in temporary Arcana profiles;
  no live keys, user databases, provider calls, servers or listeners are used.
- Ignored receipts: `target/p4-ci-checks/p4-procedural-suite-{windows,linux}-2026-10-10.log`.

## Remaining gate scope

The Memory/config timestamp inventory is finite and distinguishes temporal
consumers, returned fields, null-state predicates and stored-only timestamps.
This correction resolves its demonstrated procedural ordering defect. It is not
annual property proof at every pair, and does not close P4-57/TG4-19.
The separate known tool-error privacy regression and sandbox/desktop/recovery
dependencies remain open. No paid Fleet or Calendar run occurred.
