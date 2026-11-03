# Arcana — Project Instructions

Read-onboarding for any agent working in this repository. The global rules in
`~/.config/opencode/AGENTS.md` apply here too; this file carries the Arcana-specific
layer: what governs work, what proves it, and what is currently red.

## What governs work

`docs/restructurization/` holds the plan. Since 2026-11-03 it is one live plan plus an archive:

| Document | Role |
|---|---|
| `docs/restructurization/SHIP_PLAN.md` | **The anchor.** What v1 (Windows-only) means, the nine working rules, the roadmap R0–R4 and the status snapshot. |
| `docs/restructurization/HARDENING_BACKLOG.md` | Work deferred until after v1. |
| `docs/restructurization/DECISIONS.md` | Decisions from 2026-11-03 on. Log a real choice in the same step as the decision. |
| `docs/restructurization/reference/` | UI and memory anchors, code map, engineering learnings. |
| `docs/restructurization/archive/` | Retired master plan, gate register, 412 ADRs, receipts. History, not instructions. |
| `src-tauri/src/ci_gate.rs` | The codified gate tests (TG-P* family, run via `cargo test`). |

Work resumes only when the owner says go. Rules that bite (full text: `SHIP_PLAN.md` §3):

- **The shipped codebase is the anchor point.** The plan bends to code, never the reverse; if faithful work contradicts a doc, fix the doc.
- **No stale docs.** A change that makes any doc line stale fixes that line in the same step.
- **Every task names a proof that can fail.** "It compiles" is not proof; a checkbox is a claim until the owning check is re-run.
- **One independent read-only review at the end of each phase**, plus an independent review before security-sensitive code (vault, sandbox, egress consent, commit safety) lands.
- **A phase boundary is a checkpoint, not an exit.** Commit, update the status snapshot, continue.
- **Live-data discipline:** every stateful resource is irreplaceable until the owner says otherwise. Back up before schema-mutating or destructive operations, including ones reachable through tests. Suites run **without** live provider credentials by default.
- **Stage by explicit path — never `git add -A`.** `.env` and `target/` must never be staged.

## Commands

```bash
# Frontend
npm test            # Vitest unit tests
npm run typecheck   # tsc --noEmit
npm run lint        # ESLint

# Rust (workspace root is the four-crate workspace)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets          # CI runs this WITHOUT -D warnings
cargo test --workspace --all-targets --locked -- --test-threads=4

# App
npm run tauri dev   # dev server (kill it when done and confirm the port is free)
```

CI (`.github/workflows/ci.yml`) splits into `rust-checks`, `frontend-checks`,
`cli-checks`, `build-smoke`, and `e2e-smoke` (Linux only). Run the owning check
locally before claiming a gate.

## Status claims

- **A recorded phase closure is not proof the current workflows pass.** Checkboxes are history; re-run the owning gate before relying on any claim.
- Verified 2026-11-03 after R0 (current status lives in `SHIP_PLAN.md` §5): typecheck, `npm test` (63), `cargo fmt --check`, `cargo clippy` (CI mode; about 400 warnings, a `-D warnings` run still fails on `needless_borrow` in `src-shared/src/db/analytics.rs`) and the `ci_gate` family (80 tests) pass. A full `cargo test --workspace` takes about 35 minutes, mostly the leap-year/DST round-trip tests. The four sandbox/`project_command` tests that once failed under `NoDefaultCurrentDirectoryInExePath=1` now launch `.\name.cmd` and no longer depend on it; `ecosystem_corpora` needs the real home's staged Python runtime. The `fleet_offline` tests run only with `--features offline-fleet-test`, which the `cli-checks` job now enables (CI result for that change not yet seen).
- Known, documented suite failures on Windows: AppContainer/PSEC canonicalization (`OS error 5`) in a small number of sandbox tests. They are recorded platform limits, not regressions — confirm the failure *set* matches the docs before treating anything as new.

## Provider layer conventions

Providers are a four-file contract: `arcana-core/src/provider/{factory,catalogue}.rs`,
the adapter itself, and `src/data/models.ts`. Constraints:

- `catalogue.rs` and `src/data/models.ts::MODEL_GROUPS` must be exactly equal (`TG-P2-05` fails the build otherwise).
- Every offered model must have a verified price in `src-shared/src/cost/pricing.rs`, or be declared in `catalogue.rs::UNPRICED` (unpriced = invisible to the daily spend cap = effectively uncapped).
- Provider construction goes through `provider::factory::build` — never a new match-on-string copy.
- Keys live in the OS keychain via the vault; storage validates against the live provider first. Keys go in **headers**, never URLs; provider error bodies are never echoed raw into errors or telemetry.
- `.env` holds live testing keys (e.g. `GOOGLE_API_KEY`) for gated live smoke tests. Never commit it, never leak it into logs or error strings, and enable live/billable tests explicitly only.

## Commits and pushes

One commit per logical unit, Conventional Commit subject + non-empty body.
The owner (Razi) authorises all pushes; anything touching `main` — including
the first push — needs explicit approval, even mid-task. Never rewrite
published history.
