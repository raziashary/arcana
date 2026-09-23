# Arcana working memory (2026-09-23)

Authoritative current scope: [ARCANA_CODING_SCOPE.md](ARCANA_CODING_SCOPE.md).
Historical implementation tasks and invariant rules: `MASTER_DEVELOPMENT_PLAN.md`
§0 and phase sections. Current code and observed behavior override historical
status prose; `_verify/ARCANA-RECON-PLAN.md` and
`_verify/ARCANA-RECON-APP.md` record this reconnaissance.

- Desktop: Tauri v2 (`src-tauri`) with React/TypeScript (`src`), invoking Rust
  commands and listening to events. `arcana-core` owns the native engine,
  providers, task board, memory, recipes, graph, security, and persistence.
- `src-cli`/`arcanactl` shares core but its old HTTP agent verb was retired in P8.
  `src-shared` carries remaining shared types/data access.
- P8 removed Python orchestration and memory sidecars. PD0's separate Node
  protocol reference was retired from source and npm by ADR-0043; its history
  remains in the restructuring documents.
- Provider keys are stored through `VaultManager` in OS keyring or encrypted
  fallback. The desktop does not load `.env`. Only the explicit live-provider
  tests read `GOOGLE_API_KEY` from the repo `.env` for Gemini.
- Source now points to fresh Arcana state (`~/.arcana`), keyring service,
  application identifier, `.arcana` association, crates/packages and CLI.
  Old AutoDev state remains untouched; packaging smoke is still required.
- The MDP recorded P0–P3, PD0, and P8 as closed but original proof gaps remain;
  the matrix contains inconsistent 34.0/36.7% snapshots. Avoid stating overall
  readiness until its active scope and evidence are reconciled.
- The active MDP §5 now schedules P8 proof cleanup, then P4–P7; P9 is later.
  Retired design rows and the former two-rollout sequence live in
  `HISTORICAL_DESIGN_PROGRAMME.md`. The 2026-09-23 active register has 383
  explicitly checkboxed rows (260 checked, 123 open), not a readiness score.
- Owner changed target on 2026-09-23 to a personal coding-only Arcana. ADR-0042
  records this; ADR-0043 authorizes PD0 retirement and fresh Arcana state.
- The 2026-09-23 desktop smoke launched successfully but did not execute a
  provider-backed coding turn. At startup the environment scrub removed
  `LD_LIBRARY_PATH` and several non-secret path/metadata variable names while
  announcing them as credential-shaped. Audit this filter before P4's
  cross-platform worker-command proof; no behavior claim follows from the
  current source-only security gate.

Next: finish rename validation and P8 proof/gate reconciliation, then work
through P4–P7. P9 delivery and additional features are later scope.
