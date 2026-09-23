# Arcana — Programme Brief

**Current direction (2026-09-23).** Arcana is Razi's personal multi-agent
coding desktop. The app uses Tauri v2, a React/TypeScript renderer and a shared
Rust agent engine. Chat, worker recipes and subagents are meant to run through
the same native tool loop, backed by local SQLite state. The former AutoDesign
PSD/PDF programme is retired.

The completed P8 work removed the Python orchestrator and memory sidecars.
The PD0 Node design reference was also removed from the product and CI after
the owner approved retirement. These recorded closures still have proof gaps:
a live desktop coding turn for P8, the P0 e2e harness, and P3 memory gates.

The next development sequence is **P8 proof/cleanup → P4 fleet → P5 code
graph/context → P6 coding UI → P7 experience**. P9 installation and release
work, plus additional features, follow later. A phase checkbox is not a claim
that its app workflow has been verified. The [current readiness map](../development/READINESS_MATRIX.md)
names the missing proof for each area.

For local development, only a Gemini provider key is planned now. Enter it in
the app's provider Settings; the desktop keeps credentials in its vault and
does not read the repository `.env`. The ignored `.env` uses `GOOGLE_API_KEY`
only for opt-in, potentially billable live-provider tests. No Python service
is required for the desktop runtime.

The [Arcana coding scope](../development/ARCANA_CODING_SCOPE.md),
[master development plan](../development/MASTER_DEVELOPMENT_PLAN.md), and
[testing gates](../development/TESTING_GATES.md) govern the current work.
The [former design programme](../development/HISTORICAL_DESIGN_PROGRAMME.md)
and [original AutoDev brief](AUTODEV-PROGRAMME-BRIEF.md) remain as dated
historical records.
