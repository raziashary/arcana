# P4-43 — production idle and intentional provider-call bounds

Status: **P4-43 proven**, 2026-10-04. TG4-24 remains open.

## Contract and real consumers

The shipped RecipeScheduler starts Supervisor, QC and coding workers. Its startup,
event-kick and fixed five-minute recovery loops use the production TasksDb work
gate before entering RecipeRunner. An empty task/request/message queue must not
reach a provider. Real engine run caps bound intentional work.

## Behavior proof

- `production_startup_idle_workers_skip_provider_and_record_gate` starts the
  persisted Supervisor, QC and backend recipes, advances Tokio's clock 310 seconds
  through the actual cron sleep, and requires at least two idle work-check records
  per worker (startup plus recovery). Total physical scripted-provider entries: 0.
  Each record has worker/database identity, outcome and measured duration. All
  schedule handles are stopped and removed. Windows 1/1 (0.43s); Ubuntu WSL 1/1
  (0.97s). These are test wall times, not live provider latency estimates.
- `production_idle_worker_kick_skips_provider_and_records_idle_gate` proves zero
  provider entries and no run handle for the production idle QC event kick;
  Windows/Linux 1/1 each (prior checkpoint).
- `scheduler_kick_runs_persistent_qc_inbox_through_recipe_runner_and_acknowledges`
  runs the shipped QC recipe, handles two durable messages and acknowledges both,
  with exactly five scripted provider entries; Windows/Linux 1/1 each (prior
  checkpoint).
- `production_supervisor_and_qc_provider_turns_stop_at_run_caps` runs both saved
  recipes through RecipeRunner and AgentEngine with nonempty persisted inbox work
  and a provider deliberately continuing to request read tools. The existing
  pre-dispatch breaker increments the pending turn, then rejects at the configured
  10/20 thresholds: Supervisor dispatches 9 requests and rejects turn 10; QC
  dispatches 19 and rejects turn 20. Both return `BUDGET_EXCEEDED_TURNS`, leave
  scripted responses unconsumed and keep the original message unacknowledged.
  Reported fixture usage is supplied so unknown-usage token reservations do not
  obscure the turn boundary. Windows 1/1 (2.09s), Ubuntu WSL 1/1 (5.05s).
- Real isolated `npm run tauri dev` also starts all seven seeded workers and
  persists seven idle work checks with zero provider-call events. Profile:
  `%TEMP%/arcana-p4-label-smoke-c8b06395222847c590127c5f59cdf430`.
  The JSON summary and both log streams remain there. App and port 1420 are stopped.

## Validation and limits

Exact commands use `cargo test -p arcana-core --lib <test-name> --locked --
--test-threads=1`. Workspace check and format check pass. No API credential or
Calendar directory was used. The caps' existing inclusive pending-turn semantics
were measured, not changed. P4-43 closes only its scheduler call-count contract;
successful live CLI Fleet coding, Logs UI readback, recovery-through-completion,
platform sandbox execution and the owner's Calendar acceptance remain open.
