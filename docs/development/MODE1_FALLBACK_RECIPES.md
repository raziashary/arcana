# Mode 1 Fallback Recipes

Doc 05 Phase 2. When core-orchestrator (the Python sidecar) is unhealthy
and the watchdog circuit breaker opens, the recipe runner can fall back to
the Rust `AgentEngine` provider if — and only if — the recipe's
`fallback_policy` says so.

## The two policies

Defined in `src-tauri/src/recipes/types.rs::FallbackPolicy`:

- **`mode2_only`** (default) — refuse to run the tick. The recipe runner
  logs an explicit `fallback skipped per policy` line and emits a
  `worker:tick-end` with outcome `fallback_skipped_policy`. The Live
  Monitor card paints the worker amber (`fallback_idle`) so the user can
  tell at a glance that this worker is intentionally parked. Suitable for
  recipes that depend on MCP tools (worker-backend, worker-frontend,
  worker-platform, worker-mcp, supervisor, qc-agent / qc-dispatcher).
- **`mode1_capable`** — proceed to the Rust `AgentEngine` if a local LLM
  provider is configured (anthropic / openai / google / ollama). Only
  suitable for recipes whose work fits the Mode 1 surface area: no MCP
  tools, file ops only, lint / dead-code removal / docs. The canonical
  Mode1Capable recipe is **worker-cleanup**.

## Why most recipes are `mode2_only`

The 14 May calendar run revealed why "fall back to Mode 1 with no MCP
server" is worse than "stay idle":

- Without MCP, builtin tools resolve paths against
  `std::env::current_dir()` instead of the configured workspace —
  triggering the Doc 07 workspace-escape bug class.
- Without MCP, the worker has no `update_task_status`, `analyze_diff`,
  `annotate_file`, etc. The recipe instructions reference these and the
  agent loops on unmet preconditions.
- Without MCP, the supervisor's routing matrix can't decompose work
  because `create_task` is recipe-driven through MCP.

In short: Mode 1 is the fallback for **the agent runtime**, not for the
**recipe content**. A recipe written assuming MCP tools exist will
misbehave under Mode 1 even if the LLM provider is configured.

## When to flag a recipe as `mode1_capable`

Use this checklist before flipping the policy on a recipe:

1. The recipe's tool list (in the prose Tools section) references ONLY
   tools available in the Rust ToolRegistry. The MCP-only set
   (analyze_diff, annotate_file, write_interface_spec, e2e_probe,
   spawn_qc_review, …) MUST NOT appear.
2. The recipe does NOT call `update_task_status` to a status that depends
   on a sibling worker reading the change.
3. The recipe is idempotent: a half-finished tick can be safely re-run on
   the next cron tick without producing duplicate output (Mode 1 has no
   session memory in the LivingAgent sense).
4. The recipe's worst-case wall-clock fits within the Mode 1 timeout
   policy (currently ~5 min).

If a recipe passes all four, declare it in `defaults.rs`:

```rust
Recipe {
    …
    fallback_policy: super::types::FallbackPolicy::Mode1Capable,
    …
}
```

For YAML-defined recipes outside `defaults.rs`, add the field to the
front-matter:

```yaml
version: "1.0.0"
id: "my-recipe"
…
fallback_policy: "mode1_capable"
```

The serde-default ensures existing YAML files without the field continue
to deserialize as `mode2_only` — safe by default.

## Observability

Mode 1 transitions appear in the log with `mode = "mode1"` and either
`mode1_capable = true` (provider configured, Rust engine actually
running) or `mode1_capable = false` (no provider, worker stays idle).
A `grep 'mode1_capable=true'` over a run shows which workers actually
exercised the fallback path.

The Live Monitor surfaces this distinctly: workers in `fallback_idle` are
amber, workers in `mode1_active` are also amber but populate the activity
card body as they run. This is the visual cue the user needs to tell
"orch is down but cleanup is still working" apart from "the entire fleet
is parked".

## Related docs

- `docs/endtoendrun/14may2026/05-mode1-fallback-check.md` — the
  observability audit that motivated this taxonomy.
- `docs/development/LOG_ERROR_KINDS.md` — the `kind=` field used in
  the watchdog circuit-breaker log line.
