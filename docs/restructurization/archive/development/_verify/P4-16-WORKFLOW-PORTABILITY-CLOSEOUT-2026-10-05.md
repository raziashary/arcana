# P4-16 / TG4-17 closeout — 2026-10-05

P4-16 and TG4-17 are proven. CI [run 37226796142](https://github.com/raziashary/arcana/actions/runs/37226796142), revision 0926c6f, executes the six filters defined in `.github/workflows/ci.yml` under fail-fast Bash. All pass on Windows (job 111509360829), macOS (111509360862) and Linux (111509360867): 2 shared validator checks, 7 production portability checks, 1 shipped recipe guard, 1 persisted nine-recipe corpus, 3 saved skill loader checks, 2 shipped skill checks including the seventeen-skill production corpus. Total 16 per host.

These tests drive persisted recipe parse/save/read, skill load/import, hook save/read/runtime and shipped interactive guidance. Invalid legacy or expanded hook commands are refused before spawning. Portable guidance loads; prohibited executable syntax and hardcoded temporary paths are rejected; explanatory prose and portable redirection remain allowed. Owner-approved custom-artifact coverage and previously audited tee/quoted-operator defects are included.

Authenticated logs are retained in ignored target/ci-111509360829-log.txt, target/ci-111509360862-log.txt and target/ci-111509360867-log.txt. CI workspace lint/tests, frontend and CLI pass, and Linux app build/launch passes. Windows and macOS jobs later fail the separate command corpus: missing experimental Windows runtime DLL and intentional macOS fail-closed execution. Those failures do not change workflow-validation results and keep TG4-18/security tasks open. No provider, user Calendar workspace or production database is used.

Current tally: 38/59 P4 tasks, 15/33 TG4 gates. Core P4 remains open, including owner Calendar acceptance.
