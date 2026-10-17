# Arcana — Project Instructions

Read-onboarding for any agent working in this repository. The global rules in
`~/.config/opencode/AGENTS.md` apply here too; this file carries the Arcana-specific
layer: what governs work, what proves it, and what is currently red.

## What governs work

`docs/restructurization/` is the active programme. The documents are the contract:

| Document | Role |
|---|---|
| `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md` | **The anchor.** Atomic checkboxed tasks by phase (P0–P9). `§0` rules are always-on. |
| `docs/restructurization/development/TESTING_GATES.md` | Named automated gate tests (`TG-P<p>-<nn>`) per phase — the definition of done. |
| `docs/restructurization/development/ADR_LOG.md` | Runtime decision journal. Any build-time decision is logged **in the same step as the decision**. |
| `docs/restructurization/development/_verify/` | Evidence receipts. A claim without a receipt here is not proven. |
| `src-tauri/src/ci_gate.rs` | The codified gate tests (TG-P* family, run via `cargo test`). |

Rules that bite (see MDP `§0` for the full register):

- **The shipped codebase is the anchor point.** The plan bends to code, never the reverse.
- **Re-read the plan section before you build.** If faithful implementation still fails, the docs are wrong — fix the docs, not around them.
- **No stale docs.** A change that makes any doc line stale fixes that line in the same step.
- **Adversarially verify before building a phase** (§0.2m): dispatch refutation-briefed subagents, then re-verify every load-bearing result in the main session.
- **Two audit subagents after every phase** (§0.2a), read-only, self-contained briefs, and every finding is reported to the owner — not summarised away.
- **Every task names its proof** (§0.2e): an evidence class + the gate or smoke check that proves it. "It compiles" is not proof.
- **Never stop a phase midway** (§0.2d): ask in-flight and continue from the answer.
- **Continuation is the default** (§0.7): a phase boundary is a checkpoint — commit, update trackers — never an exit.
- **Search order on a blocker** (§0.2j): codebase → web → ask. Escalate a *decision*, never a *fact*.
- **Live-data discipline** (§0.8): every stateful resource is irreplaceable until the owner says otherwise. Back up before schema-mutating or destructive operations — including ones reachable through tests. Suites run **without** live provider credentials by default.
- **Stage by explicit path — never `git add -A`** (§0.8/§0.9). `.env` and `target/` must never be staged.

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
npm run tauri dev   # dev server (kill it when done — §0.10)
```

CI (`.github/workflows/ci.yml`) splits into `rust-checks`, `frontend-checks`,
`cli-checks`, `build-smoke`, and `e2e-smoke` (Linux only). Run the owning check
locally before claiming a gate.

## Status claims

- **A recorded phase closure is not proof the current workflows pass.** Checkboxes are history; re-run the owning gate before relying on any claim.
- Concrete verified-broken-as-of-2026-10-17 local state (fix on contact):
  - `cargo fmt --check`: drift in `arcana-core/src/platform/process_sandbox/linux_smoke/staging.rs`, `src-tauri/src/commands/{settings,updater,vault}_commands.rs`.
  - `npm run lint`: `e2e/**` and `scripts/**` lack node/browser globals in `eslint.config.js` (`no-undef` on the receipt tooling).
  - `cargo clippy -D warnings`: `needless_borrow` in `src-shared/src/db/analytics.rs`. CI clippy is warning-only, so this is stricter than CI.
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
