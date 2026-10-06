# P4-57 Session and Hook Timestamp Pairs — 2026-10-06

Status: root validated on Windows and Ubuntu WSL. This proof covers selected production session/message pairs and hook execution-log readers. No tests or Git operations were run by the delegated worker; main-session results are recorded below.

## Evidence log

- Report created before source investigation, per task instructions.
- MDP P4-57 (`MASTER_DEVELOPMENT_PLAN.md:1535`) requires the P4 production write/read pairs across databases to use the shared UTC timestamp contract, keep old persisted rows readable, and prove a full year including leap day, local midnight, and DST transitions in both hemispheres. TG4-19 (`TESTING_GATES.md:460-461`) makes this a cross-platform property test for every actual write/read pair; current gate notes earlier AnalyticsDb and RunsDb results but say remaining pairs and platform properties keep it open.

## Source findings

- `SessionStorage::upsert_session` writes `sessions.created_at` and `updated_at` with `format_utc_timestamp`; `list_sessions` and `get_session_with_messages` read both through `parse_row_timestamp`. `insert_message` writes `messages.created_at` canonically; `get_session_with_messages` calls `load_messages`, which reads it with `parse_row_timestamp` and orders by `julianday`. `upsert_session` and `insert_message` accept caller-supplied `DateTime<Utc>`, so full-corpus writer/reader tests are representable without production changes. A temp-file-backed connection plus `migrate()` keeps the fixture isolated.
- Hook `HooksDb::insert_log` omits `executed_at` from its INSERT, so the production config schema supplies the SQLite UTC default. Although `HookExecutionLog` contains `executed_at` and `HookExecutor::execute` fills it, that supplied value is discarded by the persistence writer. `get_logs` and `get_logs_since` return the stored timestamp as a raw string; the latter filters via SQLite `julianday`. There is no supported explicit-time writer API, so a historical synthetic corpus can prove these readers and compatibility, but cannot prove production writer behavior for each synthetic instant without changing the contract. The real writer can only be tested at its current SQLite-generated time.
- Session already has a single-date canonical/legacy compatibility test; HooksDb has current-time insert/read coverage and config migration coverage for old rows. New proofs will remain separately named and tempfile-backed.

## Implemented proof design

- `analytics_timestamp_pairs_session_and_message_writers_read_full_leap_year_and_dst` uses a temporary file and the production `SessionStorage::migrate`, `upsert_session`, `insert_message`, and `get_session_with_messages` paths. It covers 366 local midnights at UTC−12, UTC, and UTC+14 (1,098 instants, including leap day), plus eight explicit US/Sydney DST transition instants. For each of 1,106 timestamps, it checks stored fixed-millisecond `Z` values and compares the production reader result to the represented `DateTime<Utc>`. It then reads offset-less SQLite and offset-bearing legacy values for sessions/messages.
- `analytics_timestamp_pairs_hook_writer_and_readers_cover_current_and_legacy_year_rows` uses `HooksDb::open_at` on a temporary config DB. It seeds a bounded 1,106-row historical corpus (canonical and offset-less SQLite strings) to exercise the real `get_logs_since` year/cutoff reader, then exercises real `insert_log` → `get_logs` → `get_logs_since`. The real writer output is checked as the canonical SQLite-generated timestamp. Its supplied `HookExecutionLog.executed_at` is deliberately a year-independent old value to make the current discard behavior observable. Historical synthetic rows are reader fixtures only, not production-writer proof.
- Corpus cap is strictly below 1,200 rows per module; session executes 1,106 production session writes, 1,106 production message writes, and 1,106 combined production reads. Hook history is one transaction with 1,106 synthetic rows, followed by bounded production reads and one actual production write. All data is confined to temporary files.

## Pair-to-proof matrix and remaining limit

| Pair/field | Production writer | Intended reader | Proof |
| --- | --- | --- | --- |
| Session `created_at`, `updated_at` | `SessionStorage::upsert_session` | `get_session_with_messages`; `list_sessions` also parses these fields | Full 2024 corpus, canonical storage, production readback, plus legacy readback. The new corpus uses ID lookup because `list_sessions` intentionally limits output to 50; the existing list-order test separately covers that consumer. |
| Message `created_at` | `SessionStorage::insert_message` | `get_session_with_messages` → `load_messages` | Full 2024 corpus, canonical storage and production readback, plus legacy readback. |
| Hook log `executed_at` | `HooksDb::insert_log` leaves this field to the schema's SQLite default | `get_logs`; `get_logs_since` | Production insert/read proves current SQLite-generated canonical storage. Historical-year compatibility and cutoffs are reader-only direct fixtures. |
| Hook model `HookExecutionLog.executed_at` input | Present on the input model but omitted from the INSERT column list | Returned model contains the stored DB value | Current implementation discards the supplied value. If the gate means this input instant must be preserved, the existing production API cannot demonstrate or satisfy that requirement; resolving that contract would be outside this test-only ownership. |

No production/API decision was made. The only potential Rule-4 item is whether hook execution time means the executor-provided timestamp or the SQLite insertion time. Source currently implements the latter; the test records that behavior and leaves the semantic choice to the owning decision-maker. No tests were run per coordinator instruction.

Exact test filters for root validation:

- `cargo test -p arcana-core analytics_timestamp_pairs_session_and_message_writers_read_full_leap_year_and_dst`
- `cargo test -p arcana-core analytics_timestamp_pairs_hook_writer_and_readers_cover_current_and_legacy_year_rows`

## Main-session validation

`cargo test --offline -p arcana-core --lib analytics_timestamp_pairs_ --locked -- --test-threads=1` passes 4/4 on Windows (59.67s) and Ubuntu WSL (130.50s), excluding compilation. This selection contains the two new tests and the two earlier AnalyticsDb timestamp-pair tests. The session test exercises all 1,106 production write/read inputs; the hook test proves mixed-format historical readers and one current production insert/read. It does **not** prove that HooksDb preserves its supplied execution timestamp or that its real writer accepts the historical corpus. Owner disposition of that shipped-writer behavior is pending. No P4-57/TG4-19 closure or tally change follows.
