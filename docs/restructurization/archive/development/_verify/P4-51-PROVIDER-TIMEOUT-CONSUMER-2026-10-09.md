# P4-51 Provider Timeout Consumer — 2026-10-09

Status: verified Windows/Linux under explicitly configured test timeout; shipped cancellation remains open. Scope is limited to repository reads, `cfg(test)` edits in `arcana-core/src/provider/google_tests.rs`, and this report. No builds/tests/process/network execution, environment/.env/Vault/credential/user DB reads, Git mutations, or gate changes will be performed here.

## Progress log

- Report created before source investigation, per coordinator instruction.

## Findings so far
- The Google test module already owns loopback HTTP fixtures; engine tests already prove actual Google adapter HTTP status and retry/stream failures reach the analytics reader.
- Existing cancellation coverage in `engine.rs` uses a scripted provider, so actual adapter IO timeout/cancellation consumption is still unproven.
- Engine test helpers are private to their module. A Google-test-only fixture can construct the public AgentEngine and reuse the existing isolated analytics setup pattern without changing production code or adding a seam.
- No tests, builds, processes, or network commands have been run.
- The gate's 2026-10-04 entry explicitly leaves provider timeout/cancellation open; the 2026-10-09 matrix notes remaining provider/compiler error conjunctions. Google `stream()` maps send and body transport errors through the bounded `safe_transport_error`; AgentEngine retries stream-establishment errors and writes one aggregate that the existing analytics query reads.
- AgentEngine checks cancellation only after `stream_response` finishes consuming/returning the provider stream; it does not select cancellation against the pending Reqwest request. There is no cancellable adapter request path in the permitted test surface, so a test claiming in-flight cancellation would be inaccurate.
- ~~The first draft relied on a supposed default Reqwest request timeout.~~ Correction: `GoogleProvider` production construction uses `Client::new()` with no request/read timeout. A timeout can be exercised without a production seam by setting the existing private client field to an explicitly timed test client and holding an owned loopback request open. ~~The first draft proposed a deterministic fatal retry response.~~ Correction: the final fixture returns a valid local SSE response so the actual adapter timeout classification is preserved as a resolved `retryable` analytics row; cancellation remains a separate unproven category.

## Authored proof
- Added `provider::google::google_tests::actual_google_stream_timeout_reaches_engine_analytics_reader` in the permitted Google test file. It uses the actual `GoogleProvider` and `AgentEngine`; the fixture configures `GoogleProvider.client` with a 750 ms Reqwest timeout in the child test module, keeps the first established request unanswered until that timeout, then returns a valid local SSE response for the retry. This proves error handling with the explicitly configured test client, not the shipped default. The test requires exactly two accepted requests, one retry, a resolved bounded `google` analytics row with the original `retryable` classification read through `AnalyticsQueries`, and omission of API-key/prompt/system canaries from the analytics message.
- The AnalyticsManager setup follows the existing engine fixture's leaked isolated-home + `OnceCell` pattern; if another test has already installed the global manager, the fixture reuses it.
- Exact parent-run filter: `cargo test -p arcana-core --lib provider::google::google_tests::actual_google_stream_timeout_reaches_engine_analytics_reader --locked -- --exact --test-threads=1`.
- Not run here, per dispatch. Runtime is bounded at 15 seconds; the owned fixture terminates on the retry or after a 10-second retry-accept deadline. This specifically proves an actual client request timeout flowing through retry, AgentEngine failure accounting, and the analytics reader.

## Remaining limit
- It does not prove cancellation interrupts an in-flight HTTP request. The current AgentEngine checks cancellation after `stream_response` finishes consuming the provider stream. The production Google client has no request/read timeout; only this test configures a request timeout, and it does not prove in-flight cancellation. A truthful cancellation proof needs an accepted production cancellation contract or a separately scoped engine/provider seam; do not reinterpret this timeout test as cancellation coverage or close TG4-25 from it.

- Review correction: changed the retry response from a terminal 400 to a valid local SSE completion. This makes the analytics row retain `retryable` with `resolved=true`, directly exposing the timed-out attempt's classification through the production reader; the first draft would have overwritten the final class with `fatal`.



## Retraction and correction (2026-10-09)
- Retract the prior claim that `reqwest::Client::new()` supplies a request timeout. The coordinator verified the pinned Reqwest 0.12.28 source: request and read timeouts default to `None`; `tcp_user_timeout` is not an HTTP response timeout. The prior fixture therefore did not prove a timeout and must not be counted as evidence.
- Corrected test: set the existing private `GoogleProvider.client` field to a test-local `reqwest::Client::builder().timeout(750 ms).build()`, then keep the owned loopback request open until that configured timeout triggers. The test proves timeout classification/analytics handling only under this explicitly configured test client, not the shipped default. In-flight cancellation remains unproven.

- Follow-up correction: made the accepted retry socket blocking explicitly before its bounded request read (for Windows), reduced the engine timeout to 15 seconds, set the fixture retry-accept deadline to 10 seconds, and require exactly one new provider failure row. Exact filter is unchanged; test remains unrun here.


## Coordinator result — 2026-10-10

The surrounding Google adapter suite passes Windows21/21 (3.77s) and Linux21/21 (3.84s), including the actual held-open request/test-configured750ms timeout → establishment retry → successful body → exactly-one resolved retryable analytics row. Owned server requests are bounded and joined; sockets close. No live provider key/request is used. This proves real transport-timeout handling under explicit test configuration, not a shipped request deadline or in-flight cancellation. ADR0353 retracts the erroneous default-timeout premise; the shared cancellation correction awaits owner direction. No task/gate closes. Logs: ignored target/p4-ci-checks/provider-timeout-{windows,linux}-2026-10-09.log.
