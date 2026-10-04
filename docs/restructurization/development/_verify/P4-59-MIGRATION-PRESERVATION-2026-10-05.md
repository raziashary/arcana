# P4-59 populated migration preservation — 2026-10-05

This proves the populated-upgrade clause of TG4-21, not full retention correctness.

The analytics fixture creates the actual baseline migration in an owned temporary file, populates all five original tables, reopens through AnalyticsDb::new_at, and compares every existing column value. Production daily-cost and token readers consume the preserved rows. The config fixture starts from the actual pre-retention ladder, populates hook_logs, reopens through HooksDb::open_at, and reads the row through get_logs and get_logs_since. Migration backups stay in disposable fixture directories; no user database is accessed.

Exact check: `cargo test -p arcana-core --lib populated_pre_retention_ --locked -- --test-threads=1`. Windows: 2/2, 0.23s (`target/p4-retention-upgrade.txt`). Ubuntu WSL: 2/2, 0.55s (`target/p4-retention-upgrade-linux.txt`). Workspace check passes; TypeScript check passes. No servers or provider calls.

P4-59/TG4-21 remain open: advancing-cutoff regressions reproduce lost daily cost totals, and CLI Fleet disables the maintenance job. Those production changes await owner disposition. Active-writer retention steps separately pass on all three CI hosts at 0926c6f (run 37226796142); that does not resolve these correctness/lifecycle gaps.
