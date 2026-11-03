# P4-51 real read-file OS cause repair — 2026-10-10

## Reproduced failure and isolated repair

The real registered `read_file` handler reflected an oversized requested path
before its `std::fs::read_to_string` OS explanation. The shared 512-character error
bound cut off the explanation and retained a private input prefix; later exact
argument removal could not match the complete input. Windows and Linux had both
failed the actual next-model-request privacy assertion (1.06s/1.27s).

The smallest producer repair reports `Cannot read file: {e}` from the actual IO
error. It preserves the existing error envelope, bound, redaction and executor
correlation, and does not echo the supplied path. This is the isolated Rule5 repair
recommended by the independent review, within the owner-selected safe diagnostic
scope; it does not modify the shared sanitizer or its shipped output schema.

The real 47-registration matrix still verifies each failed result reaches the next
`AgentEngine` model request with bounded/versioned failed JSON and actual run/call
correlation. Its `read_file` case confirms an actual raw OS failure before dispatch,
requires private-prefix omission, and now positively requires the same observed
OS error code in the model-visible cause. A generic or empty error would fail.

## Verification

`cargo test --offline -p arcana-core --lib builtin_handler_failures_reach_model_as_correlated_private_error_results --locked -- --test-threads=1`

- Windows: 1 passed, 0 failed; 5.98 seconds.
- Ubuntu WSL: 1 passed, 0 failed; 6.79 seconds.
- Surrounding `agent::builtin_tools::read_file::` bounds tests: Windows 2/2
  (0.00s), Linux 2/2 (0.01s).
- Core/CLI `cargo check --offline -p arcana-core -p arcanactl --locked`: passes
  (36.67s); formatting, diff and scoped credential checks pass.
- Independent GPT-6 Luna High review reports no defect in this producer repair
  or the consumer assertion; see `P4-51-READ-FILE-CAUSE-REVIEW-2026-10-10.md`.
- Disposable homes/workspaces and scripted providers only. Credentials removed
  per command; no paid requests, user databases or Calendar execution. Test
  processes exited; no server/listener was started by this selected matrix.
- Ignored logs: `target/p4-ci-checks/p4-read-os-private-{windows,linux}-2026-10-10.log`
  and `p4-read-file-suite-{windows,linux}-2026-10-10.log`.

## Limits and remaining decisions

This proves the actual `read_to_string` error branch for the oversized path. It
does not repair every possible partial argument reflection in other producers or
path-bearing file rejection messages. Shared sanitizer hardening remains a
distinct pending Rule4 decision. Successful registration coverage remains 46/47;
confined smoke execution, provider cancellation and broader boundary proof keep
P4-51/TG4-25 open. Overall tally remains 41/59 P4 and 19/33 TG4.
