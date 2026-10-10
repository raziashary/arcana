# P4-57 procedural timestamp order review — 2026-10-10

## Scope and progress

- Independent read-only review of the `ProceduralStore::search` ordering change and its regression test in `arcana-core/src/memory/procedural.rs`.
- Assess SQLite instant-order semantics against the shared timestamp parser's accepted canonical, legacy, and offset forms; assess fixture validity and scope.
- Coordinator supplied the prior Windows failure and exact test pass context; no tests/builds will be run here.
- Findings will be appended after reading the production query and regression body.

## Review findings

- No defect found in the scoped ordering change or fixture. `ProceduralStore::search` now applies `ORDER BY julianday(updated_at) DESC, id DESC` before `LIMIT` (`procedural.rs:217-223`), so selection and tie-breaking occur in SQL before DTO parsing. This matches the existing instant-order reader patterns in episodic history/summaries (`episodic.rs:170-196`) and semantic recency (`semantic.rs:14-31`), and the owner decision recorded in `ADR_LOG.md` at the 2026-10-10 correction entry around line 7405.
- The fixture begins with three production `save_topic` writes in an isolated Arcana home, changes only their persisted `updated_at` values to intentionally represent three historical encodings, closes the direct DB handle, and calls the actual production search API (which reopens the configured memory DB) (`procedural.rs:316-363`). The values are coherent: canonical 09:00Z, offset-less SQLite noon interpreted as UTC, and 23:00+14:00 (also 09:00Z). Thus the legacy noon row must lead at limit 1; the two 09:00Z rows are tied and should follow descending ID. The result assertions test the top limited row, equal-instant tie, order, keyword filtering, and zero limit. Creation timestamps remain those written by `save_topic`; the loop checks their millisecond precision.
- SQLite `julianday` and `parse_db_timestamp` align for the three fixture forms: SQLite time values with an explicit `+HH:MM` offset are normalized to UTC for Julian-day comparison; the parser converts RFC3339 offsets to UTC, and interprets the accepted offset-less SQLite shape as UTC (`timestamps.rs:4-7,45-68`). The DTO assertions then read timestamps through `parse_row_timestamp` (`procedural.rs:240-248`). This is consistent for the forms exercised; the review does not establish equivalence for every syntactically accepted RFC3339 offset or malformed input.
- No dependency, schema, API, or unrelated ordering change is present in the reviewed code slice. The test does not claim an index or performance improvement; its key evidence is corrected instant order before the one-row limit. Coordinator-provided Windows pre-fix failure and post-fix test context were not independently rerun.

## Limits

- Read-only review of the production search query, the new focused regression, the shared timestamp parser, and cited existing ordering patterns. No builds/tests/network/user DB access/Git operations/source edits.
