# P4-57 isolated writer clocks — 2026-10-12

Owner approved ADR0370 through ADR0375. This unit introduces only the shared test fixture needed to exercise generated-now production writers. It does not claim annual coverage or close P4-57/TG4-19.

The fixture must isolate threads, restore nested clock overrides on drop, leave normal builds unchanged, and delegate SQLite date-function behavior to an unmodified in-memory SQLite connection after replacing the time-value `now` argument. Historical/offset inputs and date modifiers must retain SQLite's own behavior. Runtime connection setup installs the fixture only in unit-test builds when a clock scope is active. Checkpoint timestamps keep their existing integer-millisecond shape.

## Real schema validation correction

The actual annual task/run writer test initially refuses TasksDb migration10: overriding julianday as a non-deterministic callback prevents the real expression index from being created. Keep numeric conversion/index functions entirely built-in; intercept only the text date/time functions used by generated-now writers. Do not label the callback deterministic or weaken the schema test. This fixes the fixture owner rather than bypassing migration/index policy. The failed attempt is not passing annual evidence.

Source inventory also finds a WorkersDb expression index on date(created_at). Date/time/numeric conversion functions therefore retain their built-in implementations; only canonical strftime and legacy datetime now writers are intercepted. Queries that calculate age using other built-in now functions are outside this fixture; their temporal-reader tests must supply historical values directly rather than claim this scope controls them.

## Accepted current-source proof

On the final current source, credential-stripped `cargo test --offline -p arcana-core --lib db::test_clock::tests --locked -- --nocapture --test-threads=1` passes3/3 (24.99s test execution,40.12s build). The annual test exercises all4400 instants from the existing shared leap-year/offset/DST corpus. Actual TasksDb create/update and RunsDb begin/finish writers generate timestamps; production readers reopen both stores and compare exact canonical text and parsed instants. This proves four generated-now field pairs, not all task/run lifecycle fields. The existing analytics corpus generator is moved verbatim into the test-only fixture and reused, not duplicated or altered.

Nested scope restoration, thread isolation, dynamic connection scope changes, historical offsets/modifiers, NULL, numeric conversions and a strftime format literally equal to now are checked. Numeric/date/time conversion functions remain unmodified for expression indexes and do not inherit the synthetic now. Checkpoint integer milliseconds gain the fixture hook but annual checkpoint execution is not yet proved. The initializer attaches text-function overrides only to unit-test connections opened while a scope is active; ordinary builds have no fixture. The accepted real schema run includes normal migration/index creation and does not relabel the callbacks deterministic. Earlier failed drafts are excluded from passing evidence.

Windows normal core/CLI check, workspace formatting and diff checks pass; existing warnings remain. No provider, Vault, Calendar, server/listener or user database was used. Other pairs, legacy chronology tests and cross-platform annual closure remain P4-57/TG4-19 work; no checklist status changes.
