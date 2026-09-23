# Arcana codebase map (verified 2026-09-23)

```mermaid
flowchart LR
    UI[React tabs and Zustand stores\nsrc/] -->|Tauri invoke and events| Host[Tauri commands and lifecycle\nsrc-tauri/]
    Host --> Engine[AgentEngine and tools\narcana-core/src/agent]
    Host --> Fleet[Recipes, scheduler, task board\narcana-core/src/recipes + db]
    Engine --> Providers[Anthropic/OpenAI/Google adapters\narcana-core/src/provider]
    Engine --> Memory[Memory and context\narcana-core/src/memory]
    Engine --> Data[SQLite sessions/tasks/config\narcana-core/src/db + session]
    Fleet --> Engine
    CLI[arcanactl\nsrc-cli/] --> Fleet
    CLI --> Data
    Host --> Vault[Keychain or encrypted fallback\narcana-core/src/vault]
```

The Python orchestrator/memory sidecars are absent after P8; the PD0 Node
reference was retired under ADR-0043. The graph implementation is
in `arcana-core/src/graph`, with its eventual context read path still part of
the open P5 proof work.
