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
- Source points to fresh Arcana state (`~/.arcana`), keyring service,
  application identifier, `.arcana` association, crates/packages and CLI.
  Old AutoDev state remains untouched; the final Windows debug NSIS package
  has been rebuilt and inspected for P8.
- The original weighted readiness snapshots (34.0% and 36.7%) are historical;
  no current Arcana percentage is claimed. P0/P3 gate-proof gaps remain in
  their own registers and were not waived by P8.
- The active MDP §5 now records P8 closed, then schedules P4–P7 in order; P9
  is later.
  Retired design rows and the former two-rollout sequence live in
  `HISTORICAL_DESIGN_PROGRAMME.md`. The 2026-09-24 active register has 389
  explicitly checkboxed rows (266 checked, 123 open), not a readiness score.
  P8-01 and P8-68–73 are checked after TG8-02's consumer-backed port proofs.
- Owner changed target on 2026-09-23 to a personal coding-only Arcana. ADR-0042
  records this; ADR-0043 authorizes PD0 retirement and fresh Arcana state.
- The 2026-09-24 desktop smoke completed a Gemini `read_file` coding turn in
  session `13fdac95-d2e1-4bea-83e3-ba168a9f54f4` with a correct visible
  answer; a four-minute process sample found no app-owned Python process.
  `_verify/P8-ARCANA-CLOSEOUT-2026-09-24.md` is the evidence record. The first
  attempt exposed a Google tool-schema incompatibility, corrected in ADR-0052.
- At startup the environment scrub removed
  `LD_LIBRARY_PATH` and several non-secret path/metadata variable names while
  announcing them as credential-shaped. Audit this filter before P4's
  cross-platform worker-command proof; no behavior claim follows from the
  current source-only security gate.

P8 is closed under ADR-0055's built-gate scope. Final evidence and the
independent Luna audit disposition are in
`_verify/P8-ARCANA-CLOSEOUT-2026-09-24.md`. Next: execute P4, then P5–P7.
P9 delivery and additional features are later scope.
