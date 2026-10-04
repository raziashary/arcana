# Arcana logging

Doc 08 Phase 3. This file documents the host's logging configuration:
where the files live, how rotation works, and how long retention is.

## Where the logs live

`~/.arcana/logs/arcana.<YYYY-MM-DD>.log` and
`~/.arcana/logs/arcana-events.<YYYY-MM-DD>.jsonl`

Created by `tracing-appender` (see `src-tauri/src/logging.rs`). On
non-Windows systems `~` resolves to `$HOME`; on Windows it resolves to
`%USERPROFILE%`.

## Rotation

Daily, at UTC midnight. The appender creates a fresh file per calendar day
and appends to today's file for every restart on the same day, so a single
day's activity is always in one file regardless of how many times the
host restarts.

Behind the scenes: `tracing_appender::rolling::RollingFileAppender` with
`Rotation::DAILY`, with separate readable and structured telemetry streams.

## Retention

Retention is fixed at a maximum of **five files per stream and seven days**.
The host prunes only recognized Arcana daily filenames at startup
(`arcana_core::observability::log_retention::prune`). Unrelated files, malformed
names, and symlinks are ignored. Cleanup failures are logged and do not block
startup.

The prune emits exactly one info line per host lifetime:

```
[log_retention] policy=5-files/7-days; removed 3 file(s), reclaimed 12345678 bytes, 0 cleanup failure(s)
```

Files are aged using their validated date in the filename, so touching an old
file cannot extend its retention. The current day remains eligible for live
reads and full-date export.

## Levels

- **ERROR** — recoverable failures the user / operator should see. Always
  carry a `kind=…` field (see `LOG_ERROR_KINDS.md`).
- **WARN** — degraded state but still functioning. Health probe failures
  below threshold, non-fatal validation issues, etc.
- **INFO** — lifecycle events: process start, recipe load, worker tick
  complete. One line per logical event; avoid spam.
- **DEBUG** — verbose internals: provider frame parsing, tool dispatch, etc.
  Hidden by default; enable with `RUST_LOG=arcana_lib=debug`.

`RUST_LOG` is honoured at startup; default filter is `arcana_lib=debug,info`
(see `logging.rs::init`).

## Console output

The console layer prints to stderr alongside the file appender so the dev
console always shows live logs. Console writes are compact (single-line)
and colour-coded by level when stderr is a TTY.

## Ops hooks

When the host's running, the `~/.arcana/logs/` directory can be safely:

- `tail -f`'d (the appender opens files in append mode + the non_blocking
  background writer flushes lazily; readers see new lines within ~50 ms).
- archived externally (no exclusive locks on yesterday's file).
- pruned externally (the host's own prune is idempotent and restricted to
  recognized Arcana daily filenames).

When the host's NOT running, the directory is fully owned by the operator;
no recovery / journaling files are written here.
