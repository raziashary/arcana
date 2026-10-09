# P4-51 JSON output redaction — 2026-10-09

Owner-approved ADR0332 fixes the actual production ToolExecutor boundary. Whole-output regex replacement previously changed numeric confidences into invalid JSON (`0.[CARD]`), failing the real Memory and routing readers on Windows/Linux. Those assertions remain unchanged.

Valid JSON output is parsed and redacted recursively using the existing sensitive-key matcher and free-text string policy. Numbers, booleans and null retain their types; sensitive-key values are masked wholesale even when numeric or nested. Plain-text output retains the previous policy. Tool arguments keep their existing stricter long-token policy. No tool schema, dependency, permission or retry policy changes.

Regressions exercise long decimal and integer values, token counters, nested sensitive fields, credential-shaped strings, preserved source/identifiers, JSON scalars, free-text/malformed fallback and idempotence. The real registered Memory matrix proves propose/get/list/search readback; the routing matrix proves Rust-selected runnable worker readback.

Windows: redaction22/22 (0.09s), Memory1/1 (0.60s), routing1/1 (0.76s), executor18/18 (4.17s). The first routing command used a wrong filter and ran zero tests; it is not evidence. The corrected exact routing filter passes. Linux: redaction22/22 (0.18s), Memory1/1 (0.75s), routing1/1 (0.77s), executor18/18 (4.50s). All checks use the unchanged real reader assertions. Full P4-51/TG4-25 remains open for other registrations and all error categories.

Raw safe logs are under target/p4-ci-checks/json-*windows-2026-10-09.log and approved-*linux-2026-10-09.log. No live credentials, paid provider, user database or Calendar action is used.