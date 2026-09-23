# Arcana

Arcana is a personal desktop workspace for a multi-agent software development team. It combines a React/TypeScript interface with a Tauri/Rust host and a shared native agent engine. The intended workflow is to plan coding tasks, assign them to workers, review their work, and keep the task board, memory, and code graph available to later turns.

This repository is under active restructuring. The Python orchestrator and memory sidecars were removed in P8, and the former PD0 design reference sidecar has been retired. P4–P7 coding work and several earlier proof gates remain open. The current scope and evidence are tracked in [Arcana coding scope](docs/restructurization/development/ARCANA_CODING_SCOPE.md), [goal](docs/restructurization/development/GOAL.md), and the [master development plan](docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md). P9 packaging and further features come later.

## Codebase

- `src/`: React UI, stores, chat, task and worker views, and settings.
- `src-tauri/`: Tauri commands, app lifecycle, vault integration, and updater.
- `arcana-core/`: native agent engine, providers, recipes, tasks, memory, graph, SQLite, and tools.
- `src-cli/`: `arcanactl`, sharing the Rust core.
- `src-shared/`: shared Rust types and data access.

The original product description is preserved in [development](docs/development/). The restructuring records are in [restructurization](docs/restructurization/); their older AutoDev and AutoDesign passages are historical unless superseded by the current Arcana scope.

## Local development

Install Rust, Node.js/npm, a TypeScript toolchain through npm, and the platform prerequisites for Tauri v2. On Windows this includes C++ build tools, the Windows SDK, and WebView2. From the repository root:

```powershell
npm ci
npm run typecheck
npm run lint
npm test
npm run build
cargo check --workspace --locked
npm run tauri dev
```

Python is no longer required for the desktop runtime. Some legacy planning or verification scripts still use it.

For Gemini in the desktop app, enter the key in **Settings → Providers**. The desktop stores provider credentials in its vault; it does not load `.env`. The ignored `.env` and [`.env.example`](.env.example) are only for explicitly enabled, potentially billable live-provider tests, which expect `GOOGLE_API_KEY`. No other environment value is required for ordinary local development. Arcana has no published update feed yet; checks are inactive unless `ARCANA_UPDATE_URL` is set to an HTTPS release endpoint.

## Planning state

The current preparation verifies the Arcana rename and P8 cleanup before work proceeds through P4 (fleet), P5 (graph/context), P6 (desktop experience), and P7 (stabilization). The [readiness matrix](docs/restructurization/development/READINESS_MATRIX.md) and [test gates](docs/restructurization/development/TESTING_GATES.md) contain historical checkpoints and open evidence; a recorded phase closure is not, by itself, proof that every current workflow passes.
