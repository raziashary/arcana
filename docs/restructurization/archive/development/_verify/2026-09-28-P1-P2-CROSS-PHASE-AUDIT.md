# P1/P2 Cross-Phase Delivery Audit

Date: 2026-09-28
Scope: Audit P1/P2 claims against the authoritative Arcana plan, testing gates, readiness matrix, existing cross-phase evidence, and current production source. Read-only outside this report. No tests, app, services, live DB, providers, secrets, or Git operations.

## Findings

### P1: compiled graph surface has no agent consumer — CONFIRMED

- **Behavior and consumer:** the 27 graph tool structs expose helper methods, but none implements `Tool`; `register_for_mode` registers no graph tool. The `AgentEngine` registry therefore cannot expose them to a coding turn. This is code presence without a production consumer.
- **Evidence:** `arcana-core/src/agent/builtin_tools/graph_tools.rs` contains plain structs and associated `execute` functions; `arcana-core/src/agent/builtin_tools/mod.rs::register_for_mode` has no graph registration; `arcana-core/src/agent/tool_registry.rs::ToolRegistry` is the registry used by the engine. `src-tauri/src/reachability_gate.rs` explicitly documents this boundary.
- **Gate/task:** P1-01 and TG1-01 are checkboxed historically, but the registration clause is explicitly unmet; the gate does not prove graph capability. P5-20 is the delivery task. Existing cross-phase audit assigns P5 as owner.
- **Owner decision:** none found; defer delivery to P5 as already assigned.
- **Recommended disposition:** keep TG1-01 open for registration/consumer behavior; label module reachability as a separate satisfied structural clause. Do not count graph tools as a current product feature.

### P1: migration, package extraction, and schema ownership — PARTIAL Arcana readiness

- **Behavior and consumer:** current Arcana source has the shared Rust core, common `db::migrate::run_ladder`, and `config_db` ownership. The current source path points to `~/.arcana`; the P1 live-data evidence cited in historical records exercised the former AutoDev data root. That historical live run does not establish the state of a clean Arcana install or an Arcana user's files.
- **Evidence:** current source lives under `arcana-core/src/db/`; `arcana-core/src/db/model_configs.rs::open` delegates to `config_db::open`; current working memory explicitly says Arcana uses fresh state and leaves the old AutoDev state untouched. `TESTING_GATES.md` TG1-02/03/05/06 still describe old package names and 10/11 AutoDev database/live-data evidence.
- **Gate/task:** P1-05/P1-09/P1-10/P1-12 and TG1-02…06 are historical checked claims. TG1-01 also uses a source-name scan with known limitations; its own comment says it is not a true module-graph traversal. The readiness row already says post-rename P1 runtime/current-path proof needs reconciliation.
- **Owner decision:** none needed for the observed distinction; the current scope says old data remains untouched.
- **Recommended disposition:** retain historical task checkboxes with dated provenance, but do not use old live-data evidence as current Arcana runtime proof. Add/keep an Arcana-specific gate that opens current shipped paths against isolated Arcana-root fixtures; preserve the graph registration clause as open.

### P1: run ledger — PARTIAL production attribution

- **Behavior and consumer:** current app startup and CLI entry points create and close durable `app`/`cli` rows; the DB also supports readback. Current source does not assign `run_id` to agent turns, fleet ticks, writes, task transitions, or cost records, so it cannot yet answer which run caused those acts.
- **Evidence:** `arcana-core/src/db/runs.rs::RunsDb::{start,finish,get,list}`; `src-tauri/src/main.rs` records launch and shutdown; `src-cli/src/main.rs` wraps invocations. The `runs.rs` module comment explicitly says later acts have no `run_id` column.
- **Gate/task:** P1-18 is checkboxed and S1-01 historical evidence covers process runs; later agent/fleet attribution was expressly deferred to their owning phases. No source evidence supports calling the ledger full action provenance.
- **Owner decision:** none surfaced; phases owning the downstream tables/acts own adding associations.
- **Recommended disposition:** preserve P1-18 as a ledger-foundation task, but describe product readiness as app/CLI run capture only until agent/fleet/task/write/cost consumers read or carry the ID.

### P2: active provider selection and model-specific settings — PARTIAL end-to-end proof

- **Behavior and consumer:** active provider/model values are persisted by `set_active_provider`, hydrated at desktop startup, read back by Settings, and consumed by desktop and recipe paths. Per-model config is read by `AgentEngine::run` and its fields are copied into `CompletionRequest`; provider adapters serialize the corresponding fields into their outbound bodies. This is current production wiring, not an unused writer.
- **Evidence:** `src-tauri/src/commands/settings_commands.rs::{set_active_provider,hydrate_active_provider,get_active_provider}`; `src-tauri/src/main.rs` invokes hydration; `src/stores/settingsStore.ts::loadProviders`; `arcana-core/src/agent/engine.rs` loads `ModelConfigsDb` then builds request fields; `src-tauri/src/commands/model_config_commands.rs` and `src/stores/modelConfigStore.ts`; `arcana-core/src/provider/{anthropic,openai,google}.rs` request body builders. The provider selection rejects an unknown model or provider/model mismatch during hydration.
- **Gate/task:** TG2-12 and TG2-13 remain unchecked. TG-P2-08's comments still say “Nothing read any of it” and claim literal `None` fields, despite current engine reads and request construction. Its current source-text assertion is structural and does not exercise the Settings → persisted value → outbound wire path.
- **Owner decision:** none identified; P2 owns closing the behavioral proof.
- **Recommended disposition:** mark the old gate comments as stale, preserve TG2-12/13 open until a production-path test or desktop smoke proves restart hydration and visible active model, and proves saved settings in the adapter's recorded outbound request body.

### P2: adapter capability, generated arguments, prefix stability, and opaque replay — PARTIAL

- **Behavior and consumer:** adapter request builders/parsers and the engine are real consumers. Existing fixture tests cover representative request/body and stream cases. The required breadth is still absent: TG2-01 is not exhaustive per provider × model, TG2-03 does not property-test generated nested argument trees, and TG2-06 lacks the specified repeated/cross-process byte proof. Opaque data is captured and replayed by Google, but the session storage model persists a different UI `Message` shape without the opaque tool-call token; there is no production persistence/readback path for `RequestMessage` history visible in these modules.
- **Evidence:** `arcana-core/src/provider/conformance_tests.rs`; adapter builders/parsers in `arcana-core/src/provider/{anthropic,openai,google}.rs`; `arcana-core/src/provider/cache/prefix.rs`; `arcana-core/src/provider/types.rs::MessageContent::ToolUse`; `arcana-core/src/session/types.rs::Message`; `arcana-core/src/session/storage.rs::{insert_message,load_messages}`. `Message` has no `opaque` member and storage serializes only `tool_input`, name/id, and displayed content. Existing provider tests prove Google writes `thoughtSignature` back to the wire, not that the provider history survives a session reopen.
- **Gate/task:** TG2-01, TG2-03, TG2-06, and TG2-08 are unchecked in `TESTING_GATES.md`; the 2026-09-27 cross-phase audit already assigns P2 ownership. TG2-01's retry-policy subclause is now source-consumed: `engine.rs::stream_response` calls `provider::retry::with_retry`, and `retry.rs::decide` enforces retry class, backoff, and attempt limit. However, the sharpened failure-analytics clause remains unproven: `AnalyticsCollector::record_api_failure` has no production caller in current source.
- **Owner decision:** none identified for these proof gaps; P2 owns provider contracts and persistence, while any schema change must follow the current project decision process.
- **Recommended disposition:** keep these gates open; specifically do not call TG2-08 complete based on a request-body test. Add a persisted opaque-token producer/readback/replay proof before closing it. Treat TG2-01's retry half as source-confirmed, while retaining the failure record/dashboard reader clause as open.

### P2: analytics “today” mixes local date with UTC timestamps — CONFIRMED correctness gap

- **Behavior and consumer:** token usage is stamped with `Utc::now()`, then “today” queries compare SQLite `DATE(timestamp)` (UTC date for the stored `+00:00` timestamp) to `Local::now().date()` (user-local date). Near the local midnight boundary, dashboard activity and daily usage can be placed on the wrong day. The daily rollup likewise requests a local “yesterday” date against UTC-based `DATE(timestamp)` rows.
- **Evidence:** `arcana-core/src/analytics/collector.rs::record_token_usage` writes `Utc::now()`; `arcana-core/src/analytics/types.rs::TokenUsageRecord.timestamp` is `DateTime<Utc>`; `arcana-core/src/analytics/db.rs::{get_today_activity,get_token_usage_by_date}` uses `DATE(timestamp)`; `arcana-core/src/analytics/aggregator.rs::run_daily_rollup` derives dates with `Local::now()`; the consumer is `analytics::queries::get_dashboard_overview`.
- **Gate/task:** TG2-15 explicitly requires local-day behavior at UTC−12/0/+14 and a completed sub-hour rollup, and remains unchecked. P2-30 owns immediate/hourly rollup cadence; P2-15 owns provider cache minimums, so neither historical task line covers this timezone grouping defect. TG2-15 is the active proof contract.
- **Owner decision:** none needed to establish this defect; timezone semantics are already explicit in TG2-15.
- **Recommended disposition:** leave TG2-15 open and correct the query boundary to match the user's local day, then prove boundary rows through the real dashboard/rollup readers.

#### Coordinator disposition (2026-09-28)

The date defect was confirmed in current source and corrected: all `token_usage`
date readers now use SQLite's `localtime` conversion before `DATE()` grouping,
matching the local dates chosen by the dashboard and rollup. An isolated DB
test inserts rows immediately before and after local midnight and exercises
date lookup, dashboard activity counts, and date-range totals. This fixes the
observed host-local behavior. TG2-15 remains open for the full UTC−12/0/+14 and
DST matrix plus a measured sub-hour rollup; do not infer those from the current
host test.

### P2: daily spend ceiling is wired but cannot yet claim a strict hard stop — PARTIAL with fail-open paths

- **Behavior and consumer:** `CappedProvider` is used at the normal provider factory and direct recipe, subagent, recipe-command, and diff provider construction points. Before each request it reads the cap and current spend; on exceed it refuses the next call, pauses persisted workers, and emits a budget event. Settings/dashboard expose and display the same configured cap. However, this does not interrupt provider calls already in flight, nor atomically reserve spend across simultaneous calls; multiple calls can pass the same pre-call check before any completed-call cost updates `AppState`. There are also fail-open reads: `DailyCap::load` turns settings DB errors into `enabled=false`, and `AppState::persist_today` logs and suppresses a persistence failure, after which startup can restore an understated total. These paths weaken a strict ceiling claim.
- **Evidence:** `arcana-core/src/provider/factory.rs::build`; `arcana-core/src/provider/capped.rs::{wrap,wrap_global,check,complete,stream}`; direct wrappers in `arcana-core/src/recipes/runner.rs`, `arcana-core/src/subagent/native_runner.rs`, `src-tauri/src/commands/{recipe_commands,diff_commands}.rs`; `arcana-core/src/budget/daily.rs::{DailyCap::load,pause_fleet}`; `arcana-core/src/state/app_state.rs::{add_cost,persist_today,hydrate_today}`; UI consumers in `src/tabs/Settings.tsx::DailyCapCard` and `src/stores/dashboardStore.ts::loadDailyCap`.
- **Gate/task:** TG2-17 remains unchecked and requires provider-layer enforcement, visible pause, global immediate kill, and bounded broadcasts. P2-21/22 and M-42/G-04 are the source tasks; P6 owns the visible kill-switch surface. Current code has a daily cap and fleet pause, but no evidence of immediate cancellation for in-flight agent/worker work or strict concurrent overspend prevention.
- **Owner decision:** current plan already allocates the visible UI surface to P6; the strict enforcement boundary and fail-open handling remain P2 work, with no additional owner ruling found.
- **Recommended disposition:** keep TG2-17 open and describe current behavior as “blocks later requests and pauses workers after recorded spend reaches the configured cap.” Do not claim an immediate/strict ceiling until state-read/persistence failures, in-flight cancellation, and concurrent request admission are closed and behavior-tested.

### P2: provider-removal gate is stale in wording, not a fifth-provider product path — PARTIAL

- **Behavior and consumer:** the catalogue and provider factory expose exactly Anthropic, Google, and OpenAI; ~~`list_providers` uses the supported set.~~ At audit time it was a separate hardcoded vector; it now derives from `factory::supported_ids()` and has a consumer regression test. `factory::build` refuses `ollama` and `xai`. A few retired-provider references remain intentionally so an old stored key can be shown as orphaned and deleted, and in rejection tests/comments. Therefore “only three supported providers” is confirmed, while “no reference to Ollama/Azure anywhere in source/docs” is refuted as a useful product invariant.
- **Evidence:** `arcana-core/src/provider/factory.rs::{build,supported_ids}` and its tests; `arcana-core/src/provider/catalogue.rs::CATALOGUE`; `src-tauri/src/commands/settings_commands.rs::list_providers`; retained secret visibility is described in `TESTING_GATES.md` TG2-14.
- **Gate/task:** ~~TG2-14 remains unchecked, but the gate itself now documents that its zero-reference formulation is unimplemented and that xAI was explicitly ruled out. Its old literal absence clause conflicts with preserving cleanup access to retired keys.~~ Coordinator correction: TG2-14 now tests the supported-provider contract, and the focused factory suite passed on 2026-09-28; see the disposition below.
- **Owner decision:** none needed for current supported-provider count. Coordinator correction: the Settings consumer was wired to the factory-supported set and now has a regression test.
- **Recommended disposition:** ~~replace the zero-string scan with a supported-provider/catalogue/factory/settings contract plus a negative test that retired IDs cannot build or be selected, while preserving the ability to find/delete legacy credentials. Keep TG2-14 open until that gate exists.~~ Implemented on 2026-09-28: TG2-14 now checks the three-provider contract, factory construction/rejection behavior, and the Settings list's use of the factory set; retired-key discovery remains supported.

### P2: checked vault gate includes an unproven Linux fallback clause — PARTIAL

- **Behavior and consumer:** current code provides the encrypted-file fallback when no OS keychain is reachable and exposes the active backend to Settings. The required no-D-Bus Linux production path is not exercised by a current CI job according to the gate and readiness records, so this platform/security claim is source-present but not platform-proven.
- **Evidence:** `arcana-core/src/vault/fallback.rs`, `arcana-core/src/vault/keychain.rs`, `src-tauri/src/commands/vault_commands.rs::vault_backend_info`; `TESTING_GATES.md` TG2-11 explicitly says no D-Bus-less CI job covers fallback and carries it to P3; `READINESS_MATRIX.md` Security posture repeats that limitation.
- **Gate/task:** TG2-11 is nevertheless checkboxed `[x]`/GREEN. Its stated pass condition includes the missing CI path. The historical TG-P2-07 no-env/argv source gate proves credential provenance, but it does not exercise the Linux fallback.
- **Owner decision:** P3 is the named owner for the carried platform proof; no new decision needed.
- **Recommended disposition:** change TG2-11 to partial/open until a D-Bus-less CI or equivalent isolated platform test exercises fallback creation, permissions, backend reporting, and key readback. Keep the separate no-env/argv proof as satisfied.

### P1/P2 historical smokes — PARTIAL current Arcana readiness

- **Behavior and consumer:** P1/P2 implementation claims originated in the AutoDev product/runtime. Current Arcana has one strong replacement smoke: the 2026-09-24 Windows desktop Gemini 3.7 Flash turn read `package.json` through two real `read_file` tool calls and returned the correct answer. This establishes an Arcana provider→engine→tool→UI path for that scenario, not the full P1/P2 matrix, restart path, cache behavior, ceilings, analytics, or all adapters.
- **Evidence:** `_verify/P8-ARCANA-CLOSEOUT-2026-09-24.md` records the current Arcana desktop turn and build; it explicitly distinguishes it from old AutoDev baseline. The same report says P0/P3 specification proof gaps remain. `TESTING_GATES.md` S1/S2 rows and detailed P1/P2 evidence largely point at old `autodev-core`/`airsctl`/historical live provider runs; the current CLI is `arcana-core`/`arcanactl` and the current data root is `~/.arcana`.
- **Gate/task:** S1-01…S1-04 and S2-01…S2-12 remain historical checkboxes. Current Arcana proof covers only a narrow subset of these outcomes. TG1-02/03 and TG2-01…17 are the more granular gates; open clauses listed above remain open regardless of old smoke marks.
- **Owner decision:** no scope change needed; the working memory already says not to claim a current Arcana readiness percentage from historical weighted snapshots.
- **Recommended disposition:** retain historical evidence with its date and original product label. For readiness, count only current Arcana proofs against each real user-visible outcome; do not carry forward a phase-wide “closed” claim solely from old S1/S2 smokes.

## Product readiness, separated from code presence

P1/P2 are not complete current Arcana product phases just because their source modules compile or old checkboxes are marked. Current source confirms a native shared core, current Arcana database owners/migrations, provider adapters, active-model persistence, Settings model-config reads, provider request construction, tool redaction at `ToolExecutor::execute_batch`, and a daily-cap decorator. These are real production paths with consumers. Product-level readiness is still partial because graph tools are not callable by an agent; P1 live-data and many S1/S2 records refer to former AutoDev roots/runtime; agent/fleet activities are not attributed to run IDs; adapter matrix, cross-process cache stability, provider-history opaque-token persistence, restart/UI-to-wire proofs, Linux fallback CI, failure analytics, and strict daily-ceiling/kill-switch behavior remain unproven or incomplete. The TG2-15 timezone grouping defect was corrected in the current tree; complete multi-timezone/DST and cadence proof remains open.

### Evidence register summary

| Area | Status | Current production consumer | Gate/task disposition |
|---|---|---|---|
| Core extraction/schema owner | PARTIAL (current source present; historical live-root evidence is stale for Arcana) | `AgentEngine`, desktop, CLI, shared DB openers | Keep TG1-02…06 as historical until Arcana-root current-path proof; don't equate structural checks with live-data proof |
| Graph agent tools | CONFIRMED unavailable to agent | No consumer; absent from `register_for_mode` | TG1-01 registration open; P5-20 owns delivery |
| Tool/log redaction | CONFIRMED wired at current production boundaries, with stated shape limits | `ToolExecutor::execute_batch`, execution registry, logging/storage consumers | Historical S1-04 alone is insufficient; Arcana source path exists, current audit does not rerun it |
| Run ledger | PARTIAL | Tauri app startup/shutdown and CLI process | P1-18 foundation checked; agent/fleet/action association remains future work |
| Provider settings | PARTIAL end-to-end evidence | Settings stores → persisted state/config → AgentEngine request → adapter body | TG2-12/13 open; TG-P2-08 source comments are stale |
| Provider conformance/cache/replay | PARTIAL | Adapters and engine | TG2-01/03/06/08 remain open at stated missing clauses |
| Vault fallback | PARTIAL | `VaultManager` and backend-info UI | ~~TG2-11 is overstated green~~ Coordinator correction: TG2-11 is now explicitly unchecked/PARTIAL; D-Bus-less runtime proof remains open under P3 |
| Local-day analytics | PARTIAL; host-local query defect corrected with an isolated boundary test | Dashboard overview and daily rollup | TG2-15 open for UTC−12/0/+14, DST, and sub-hour cadence proof |
| Daily spend ceiling | PARTIAL, fail-open/race/in-flight gaps | `CappedProvider`, recipe/subagent/diff callers, cap Settings UI | TG2-17 open; current behavior is a next-call guard plus worker pause, not an immediate strict stop |
| Provider removal | Supported-set behavior proven | Catalogue/factory/Settings support exactly three providers; retired keys remain discoverable | TG2-14 corrected to supported-set contract; four factory tests and one Settings-consumer test passed on 2026-09-28 |

## Final progress

- [x] Report initialized before investigation.
- [x] Reconcile P1/P2 task and gate claims with authoritative documents.
- [x] Trace material claims through production source to actual consumers.
- [x] Record stale-evidence and readiness separation.
- [x] Finalize audit conclusions and product-readiness assessment.

## Coordinator gate corrections (2026-09-28)

The cross-phase findings were applied to the governing gate register. TG2-11 is
now unchecked/PARTIAL because its stated Linux no-D-Bus vault behavior has no
isolated runtime or CI proof; source presence and `0600` at creation are not
substitutes for backend/key readback behavior. TG2-14 now tests the actual
supported-provider contract rather than demanding zero legacy strings: the
provider factory unit suite proves the exact Anthropic/Google/OpenAI set, that
all supported IDs build, and that xAI/Ollama/empty IDs are rejected. That suite
passed on 2026-09-28. The P2 readiness row and evidence summary now preserve
TG2-11 debt and record TG2-14's corrected gate.

Consumer follow-up (2026-09-28): independent review caught that Settings still
used a separate hardcoded three-provider vector, so the four factory tests alone
did not prove the selectable list. `settings_commands::list_providers` now derives
IDs from `factory::supported_ids()` and maps each supported ID to its Settings
label, failing closed if a new supported ID has no label. The focused
`settings_provider_list_covers_the_factory_supported_set` test passes. The
initial audit statement that the Settings list already used the factory set is
struck through above as a correction.

### Current-source reconciliation (2026-10-03)

The dated findings above are retained as historical audit evidence. Two
statements were superseded by later implementation and must not be repeated as
the current source state:

- **P1 run attribution is partial, not absent.** `RecipeRunner::run_recipe`
  creates a worker tick in `RunsDb`; `AgentEngine` carries its run ID into
  `ToolExecutor`; `TasksDb` persists it for selected status transitions; and
  `worker_run_state` / `worker_run_events` store task-attributed worker
  progress. The current remaining gap is broader action provenance, including
  provider-cost records and all file mutations. P1-18 remains the historical
  ledger-foundation task; this observation does not close P4 recovery or
  observability gates.
- **P2 daily reservation has replaced the old check-then-record path.**
  `daily::reserve_provider_call_estimated` reserves through an immediate
  SQLite transaction and reads cap state fallibly; unknown/malformed enabled
  cap state refuses dispatch. `CappedProvider` invokes the reservation before
  request dispatch. The reservation regression covers concurrent holds and
  settlement behavior. TG2-17/P2-21 remain open for real unary/stream wrapper
  success/error/drop integration, restart proof, estimate-bound evidence,
  kill-switch cancellation, bounded broadcasts, and complete production
  consumer coverage.

The related P3 finding remains current: TG3-17 degradation is visible through
prompt recall, but `search_all` and `SearchMemoryTool` still reduce the result
to entries and render an empty-results message without the degradation state.
P3 owns that memory-tool contract; recent P4 memory fallback tests do not close
it.
