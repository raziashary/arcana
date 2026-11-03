# P4-51 shared argument-prefix hardening

## Contract and implementation

Owner-approved ADR0359/0360 extends the existing failure boundary in `agent/tool_trait.rs`. An earlier producer can truncate an error before exact argument removal, leaving a private argument prefix. Executor enrichment now normalizes both sides with the existing text policy, removes full or longest reflected prefixes at Unicode character boundaries, and retains the existing twelve-character threshold. Nested arguments are borrowed, sorted longest first and deduplicated; overlapping matches are not replaced twice. Short values retain exact-whole-value behavior.

The versioned JSON shape, 512-character diagnostic bounds, corrective guidance and executor-owned correlation remain unchanged. This cannot recover a useful cause already lost to producer truncation; the previously published `read_file` producer fix remains necessary.

## Observed behavior

Exact command, with GEMINI_API_KEY, GOOGLE_API_KEY, OPENAI_API_KEY, ANTHROPIC_API_KEY and DATABASE_URL absent:

```text
cargo test -p arcana-core --lib agent::tool_trait::tool_result_error_tests --locked -- --test-threads=1
```

- Windows: 11 passed, zero failed; test runtime 0.15 seconds.
- Linux/WSL: 11 passed, zero failed; test runtime 0.57 seconds; process exit zero.
- Receipts: ignored `target/p4-ci-checks/p4-sanitizer-windows-2026-10-12.log` and the corresponding Linux log.

The public executor-context consumer proves truncated lengths 12/64/400/512, Unicode boundaries, nested/overlapping inputs and normalized controls are removed while bounded typed JSON, task/run/call correlation and useful OS causes survive. Negative assertions preserve unrelated diagnostics and eleven-character coincidences. Independent Luna High source review found no actionable sanitizer defect; its separate provider findings are tracked under ADR0361.

No provider requests, user databases, servers or listeners were used. This is a verified shared privacy slice, not closure of P4-51/TG4-25: positive confined smoke execution and the remaining production/error conjunctions are still required. Overall P4 remains 41/59 and TG4 19/33.

Default-feature cargo check --offline -p arcana-core -p arcanactl --locked passed on Windows in 3m26s; existing warnings remain. rustfmt on tool_trait.rs and scoped git diff --check passed.
