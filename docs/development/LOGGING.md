# AutoDev logging

Doc 08 Phase 3. This file documents the host's logging configuration:
where the files live, how rotation works, and how long retention is.

## Where the logs live

`~/.autodev/logs/autodev-<YYYY-MM-DD>.log`

Created by `tracing-appender` (see `src-tauri/src/logging.rs`). On
non-Windows systems `~` resolves to `$HOME`; on Windows it resolves to
`%USERPROFILE%`.

## Rotation

Daily, at UTC midnight. The appender creates a fresh file per calendar day
and appends to today's file for every restart on the same day, so a single
day's activity is always in one file regardless of how many times the
host restarts.

Behind the scenes: `tracing_appender::rolling::RollingFileAppender` with
`Rotation::DAILY`, filename prefix `autodev-`, suffix `.log`.

## Retention

Default: **14 days**. Files older than the cap are pruned exactly once at
host startup (see `src-tauri/src/observability/log_retention.rs::prune`).

Override via the `AUTODEV_LOG_RETENTION_DAYS` environment variable:
- `AUTODEV_LOG_RETENTION_DAYS=7`  → keep one week.
- `AUTODEV_LOG_RETENTION_DAYS=30` → keep one month.
- `AUTODEV_LOG_RETENTION_DAYS=0`  → disable pruning entirely. Useful when
  an external rotation policy (e.g. logrotate on Linux, a Windows scheduled
  task) is in charge.

The prune emits exactly one info line per host lifetime:

```
[log_retention] policy=14d → dropped 3 file(s) reclaiming 12345678 bytes (0 error(s))
```

If a file's modification time can't be read, the file is skipped and
contributes to the `error(s)` count — never deleted.

## Levels

- **ERROR** — recoverable failures the user / operator should see. Always
  carry a `kind=…` field (see `LOG_ERROR_KINDS.md`).
- **WARN** — degraded state but still functioning. Health probe failures
  below threshold, non-fatal validation issues, etc.
- **INFO** — lifecycle events: process start, recipe load, worker tick
  complete. One line per logical event; avoid spam.
- **DEBUG** — verbose internals: SSE frame parsing, tool dispatch, etc.
  Hidden by default; enable with `RUST_LOG=autodev_lib=debug`.

`RUST_LOG` is honoured at startup; default filter is `autodev_lib=debug,info`
(see `logging.rs::init`).

## Console output

The console layer prints to stderr alongside the file appender so the dev
console always shows live logs. Console writes are compact (single-line)
and colour-coded by level when stderr is a TTY.

## Ops hooks

When the host's running, the `~/.autodev/logs/` directory can be safely:

- `tail -f`'d (the appender opens files in append mode + the non_blocking
  background writer flushes lazily; readers see new lines within ~50 ms).
- archived externally (no exclusive locks on yesterday's file).
- pruned externally (the host's own prune is idempotent — it skips files
  not matching `autodev-*.log` so external rotation can coexist).

When the host's NOT running, the directory is fully owned by the operator;
no recovery / journaling files are written here.
