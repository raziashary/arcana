# P4-61 Probe Enforcement Fresh Review — 2026-10-09

Status: read-only review complete. Scope was only the current `e2e_probe.rs` diff against ADR0339/0344/0349/0350. No source changes, tests, builds, network, process launches, environment/.env/Vault/user DB access, or Git mutations were performed.

## Progress log

- Report created before source investigation, per coordinator instruction.

### Evidence checked

- Read accepted ADR-LOG-RESTRUCT-0339, -0344, -0349, and -0350. They require an explicit per-call confined loopback capability, per-platform proof of listener scope/process-tree cleanup, aggregate CPU/memory/process quotas, a bounded private workspace/disk quota and conflict-safe copy-back; absent proof stays fail-closed. ADR0350 specifically removes the unrestricted launcher, retains registration/schema and requires a real registered-executor refusal with no created marker.
- Read the complete current `e2e_probe.rs` diff and surviving tests. The production `execute` path validates argument shape, loopback URL syntax, and selected workspace, then returns `ToolResult::sandbox_violation("loopback_smoke", "e2e_probe", false, fixed_guidance)` unconditionally. Child process/HTTP imports and run helpers are removed from production. `registered_smoke_refuses_unproved_capability_without_launch_or_http` invokes the actual FastDevelopment registration via `ToolExecutor`, supplies valid probe fields plus command/source/key/prompt canaries, asserts the structured denial and output bound, verifies no marker, checks an owned nonblocking loopback listener received no connection, then closes/rebinds its owned port.
- Schema property names/types/required fields remain compatible; only `start_cmds`' description changed to selected-platform-shell wording and the top-level tool description now says the capability is unavailable. No execution success/result payload is produced by this diff.

## Findings

1. **Stale redirect claims overstate the surviving tests** — `arcana-core/src/agent/builtin_tools/e2e_probe.rs:311-343`. The `f19_tests` comments say the redirect policy reuses `validate_probe_url`, that off-loopback redirect targets are refused, and that loopback redirects “must still be followed.” The diff removed `http_probe` and its reqwest redirect policy; these tests now exercise only the URL syntax validator and cannot establish redirect behavior. Rewrite the comments and test names to claim only off-loopback URL syntax rejection / loopback syntax acceptance. If a runner later adds HTTP redirects, test that actual client policy then.

2. **Input-schema prose still describes a smoke run that cannot occur** — `e2e_probe.rs:76-92`. The model-facing property descriptions say commands launch, `probe_url` is GETed after boot, `expected_text` determines PASS, and boot/total timeouts govern the run. The top-level tool description correctly says all valid calls are currently refused, but the detailed schema still presents active execution behavior. Preserve property names, types, required fields, and bounds for compatibility while clarifying the property descriptions that the implementation currently fails closed before launch/request.

## Enforcement and compatibility check

No actionable enforcement defect found in this diff: production `execute` reaches an unconditional structured `sandbox_violation` after its argument/URL/workspace checks (around lines 102-141); the removed process/HTTP launch path has no replacement fallback. The real-registered ToolExecutor test supplies valid fields, asserts `sandbox_violation` and its sandbox denial fields, output bound and canary omission, no marker, no accepted loopback connection, and successful rebind of the owned port (around lines 157-232). Structural schema compatibility is retained; the only schema edit observed is descriptive text for `start_cmds`.

No tests/builds or other prohibited actions were performed. No gate was marked done.

Coordinator disposition: both stale claims corrected. Redirect fixtures now assert only URL syntax; schema fields explicitly say no launch or HTTP while unavailable. The enforcement source is unchanged by these wording fixes.
