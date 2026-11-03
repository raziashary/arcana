# P4-53 Query Registry Snapshot Recovery Report — 2026-10-13

## Scope and authority

This recovery report covers only the query registry and inventory consequences of the approved P4 snapshot unit. The governing source is `MASTER_DEVELOPMENT_PLAN.md` §0. The catalog is `_verify/P4-58-QUERY-INVENTORY.md`; the enforcing scanner is `arcana-core/tests/p4_query_registry.rs`.

## Progress

- [x] Read MDP §0 before reviewing query implementation.
- [x] Created this report before continuing investigation and append findings incrementally.
- [x] Compared `runs.rs` and `hooks/db.rs` SQL owners with the registry.
- [x] Recomputed normalized SHA-256 fingerprints for changed and new in-scope SQL.
- [x] Updated the inventory while retaining fingerprints for unchanged SQL under changed owners.
- [x] Ran the exact registry command after root granted build permission and credential variables were removed for that process.

## Registry test result

Command: `cargo test --offline -p arcana-core --test p4_query_registry --locked -- --test-threads=1`

The crate built with pre-existing warnings. Six of eight tests passed; `every_discovered_reader_owner_has_a_proof_reference_or_reasoned_exclusion` and `production_readers_are_registered_in_the_p4_query_inventory` failed before comparing inventory entries. Both report unresolved dynamic SQL: `core_db::src/db/test_clock.rs::install uses .query_row with unresolved SQL argument`. Therefore the edited owner/fingerprint set remains unverified by the AST registry.

The scanner failure is caused by `test_clock.rs::install` constructing `SELECT {name}({placeholders})`, where `name` iterates over `strftime` and `datetime`, then delegating the call to SQLite's built-in function behavior for test-clock scopes. The scanner currently enumerates every Rust file beneath `src/db`, and `src/db/mod.rs` declares `test_clock` without a scanner-visible `cfg(test)`. This is a scanner-scope/resolution blocker, not a missing query inventory row. No scanner or source changes were authorized in this subtask, and I did not rerun the build.

## Query and owner changes

- `RunsDb::record_operation_intent` is now a no-SQL forwarding wrapper. The unchanged unresolved-task selector belongs to `RunsDb::record_operation_intent_with_snapshot`; retain `98f74c2375d82b4c16c8b964c0e1fe31d345641a7ebe12e3dd5d961dffd9577d` and its existing populated task-index/guard proof.
- `RunsDb::record_operation_outcome` is now a no-SQL forwarding wrapper. The unchanged existence read belongs to `RunsDb::record_operation_outcome_with_snapshot`; retain `abcf90800874a9a1e365127d79107afb3691fcb6429249fb8ef53831e8392d07`.
- The outcome insert-select now persists `snapshot_json`; its fingerprint is `384cea995ce03680d31d489bd1047a04e14f8e548f34408fde4194131827e775`. The conditional snapshot task-id read is new, fingerprint `cd5fbdeecc441be9751f97179e0345af95b4c8d3a5b776c4342d85e823609c63`.
- The reconciliation insert-select moved from the public coordinator into private `append_supervisor_reconciliation`, while its SQL remains identical. Retain `c407e112ce2117535496c8f6b877832bb1b8e0d93359c66c8ddadf8448d42567` under the new owner and the existing populated guard-plan proof.
- `OPERATION_JOURNAL_PAGE_SQL` now selects the intent snapshot and linked outcome snapshot. Its fingerprint is `80237c595a9e645309ec095253f39d8bbd36d658cb70c3bc600fc5c45bc8679e`.
- `HooksDb::has_enabled_hooks` adds `SELECT EXISTS(SELECT 1 FROM hooks WHERE enabled IS NOT 0)`, normalized fingerprint `f1c642d278c613dd19f9038001f25809cb6169a8875c4a4c36c971642821ec43`. It is outside current P4-58 scanner roots (`src/db`, `src/analytics`, shared analytics), so I did not add it to the exact registry. `enabled_hook_presence_does_not_decode_or_hide_malformed_rows` proves fail-closed presence semantics, not a dashboard/dispatch index plan.

Fingerprints follow `query_fingerprint` in the test: split whitespace, join with single spaces, lowercase ASCII, then SHA-256 over UTF-8. These locally computed values still require registry confirmation after the scanner blocker is resolved.

## Snapshot proof alignment and remaining proof

Root reports that `host_verifies_registered_write_after_lost_acknowledgement` now proves a before-snapshot is read back after database reopen. After-snapshot readback from a second real normal write remains pending. The existing populated `operation_journal_query_plans_use_populated_indexes_and_preserve_guards` test covers the page indexes and primary-key task lookup; it does not, by itself, prove after-snapshot production readback.

The inventory's page-query row records the populated plan and existing page behavior while naming the new before-snapshot reopen proof and leaving after-snapshot readback open. No P4 task or gate is closed by this report.

## Changed queries and proof needed

- Resolve or explicitly bound the `test_clock.rs::install` scanner case, then rerun only the authorized exact registry command. The current run did not validate inventory fingerprints.
- Confirm the unchanged unresolved-task selector is still discovered under `record_operation_intent_with_snapshot` and keep its existing fingerprint.
- Confirm the snapshot-aware outcome insert-select and conditional task-id lookup under `record_operation_outcome_with_snapshot`.
- Confirm the unchanged reconciliation insert-select under `append_supervisor_reconciliation`.
- Confirm the updated page SQL fingerprint and prove before-snapshot readback after reopen; keep after-snapshot readback pending until the planned second normal-write test passes.
- Keep `has_enabled_hooks` outside the current registry scope unless that scanner scope is changed; do not invent dashboard/dispatch plan evidence for it.

## Root final registry receipt

The first run6/8 stopped at the explicitly cfg(test) clock fixture's dynamic delegated date-function SQL. Root corrected the scanner to honor file-level cfg(test), marked that fixture explicitly, and added a negative runtime-SQL regression. The next8/9 run caught a missing owner-evidence row for the relocated private append; add that actual owner with the existing populated guard-plan proof. Final9/9 passes4.42s. Actual acknowledged write and lost-ack write now prove both snapshot shapes through reopened public readers (registered recovery3/3); populated journal plans pass in the RunsDb29/29 suite. This supersedes pending readback/registry notes above. Hook presence remains an explicitly documented non-dashboard store-presence reader outside current registry roots; its malformed-enabled-row regression and actual verifier consumer both pass. No task/gate closes.
