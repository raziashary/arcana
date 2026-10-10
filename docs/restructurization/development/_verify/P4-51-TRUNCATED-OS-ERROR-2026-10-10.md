# P4-51 real OS error reflection — 2026-10-10

Actual read_file failure driven by the existing all47-registration AgentEngine fixture uses a1024-character oversized filename with a private marker. A separate std::fs::read_to_string fails with a real raw OS error. The real model's next request receives the bounded failed envelope, but its cause contains the private filename prefix.

Linux exact matrix fails1/1 at “read_file must omit planted-private-os-path.” The handler cause truncates before the executor's exact full-argument removal, so the full private string is no longer present for removal. This demonstrates a production privacy defect, not an oversized planted diagnostic that no handler reads.

Shared-contract direction is pending under Rule4. Recommended correction: preserve existing exact argument removal and mask a reflected prefix that reaches the truncated cause boundary. Test Unicode/bounds/useful non-sensitive text plus real model readback on Windows/Linux. No production fix or green matrix is claimed. Ignored receipt: target/p4-ci-checks/p4-os-error-consumer-linux-2026-10-10.log. No secrets/user databases/provider requests were used.

Windows reproduction also fails1/1 (1.06s) on the identical private-prefix assertion; Linux fails1/1 (1.27s). Independent Luna High source review confirms the truncation-before-full-match mechanism, and notes that masking the prefix alone cannot restore OS explanation already truncated away. Recommended verification must also preserve a useful cause, Unicode safety and existing bounds. See `P4-51-TRUNCATED-ARGUMENT-REVIEW-2026-10-10.md`. Both owned test processes have exited.
