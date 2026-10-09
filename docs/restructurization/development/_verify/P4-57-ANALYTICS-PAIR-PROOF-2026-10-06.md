# P4-57 AnalyticsDb timestamp-pair proof — 2026-10-06

Scope: existing production token, API-failure, system-event and optional worker
heartbeat writers/readers. No runtime, schema or dependency changes. Each test
uses a disposable AnalyticsDb and calls its public writer and reader methods.

| Pair | Behavior checked |
|---|---|
| `insert_token_usage` → `get_token_usage_by_date` | Canonical stored UTC and exact represented instant across 4,400 samples; grouping uses the host's local civil date; legacy SQLite leap-day row remains readable. |
| `insert_worker_stats` → `get_worker_stats` | Canonical optional heartbeat and exact readback across the same corpus; legacy heartbeat remains readable. |
| Worker rollup → `get_worker_stats_range` | A corpus heartbeat survives monthly storage and the combined history reader. This is one monthly sample, not full-year coverage of every range combination. |
| `insert_api_failure` → `get_api_failures_recent` | 366 daily samples plus eight DST edges, canonical storage, exact readback, and a legacy leap-day row. |
| `insert_system_event` → `get_system_events_recent` | Same daily/DST/legacy checks; total 375 rows stays below the reader's 500-row cap. |

The 4,400-sample corpus covers every day of leap year 2024 at four wall-clock
times and three offsets, plus forward/backward DST edges in New York and
Sydney. These are explicit offset-bearing instants, not a claim that the host
timezone database was switched. The recent readers use their existing DB-level
hours parameter to include historical data; no injected clock or new API is
needed.

Exact command on Windows and Ubuntu WSL:

```text
cargo test --offline -p arcana-core --lib analytics_timestamp_pairs_ --locked -- --test-threads=1
```

Results: Windows 2/2, 13.74 seconds; Ubuntu WSL 2/2, 102.35 seconds.
Compilation time is excluded. Formatting and diff checks pass. These are
production writer/reader tests, not live Fleet or desktop acceptance.

P4-57/TG4-19 remain open for the complete database-pair inventory and platform
properties. P4-59's selected retention/consumer suite separately passes on
Windows: core retention 23/23, worker-stat 5/5, shared analytics 4/4, CLI export
1/1, analytics failure Logs reader 1/1, hook failure Logs reader 1/1. The CI
matrix now selects all six groups on Windows/Linux/macOS; remote current-tree
results remain pending. No checklist tally changes.


## 2026-10-09 stronger historical reader proof

The original monthly-worker sample is superseded by the complete annual heartbeat/count readback, Windows1/1 and Linux1/1. See `P4-57-ANNUAL-WORKER-ROLLUP-2026-10-09.md`. Other timestamp-pair and platform limits above remain.
