# V2-P0BC — Adversarial Verification: Vault + MCP Server Deletion

**Verifier posture:** hostile. Default REFUTED on ambiguity.
**Mode:** read-only. No builds run, no files edited outside this report.
**Repo:** `C:\AIRS Foundry\autodev\autodev` (Cargo workspace: `src-tauri`, `src-cli`, `src-shared`)
**Date:** 2026-08-31

---

## 1. Verdict table

| # | Claim (abbrev.) | Verdict |
|---|---|---|
| **C1** | `vault_commands.rs:12`/`:27` return raw secret to webview | **CONFIRMED** (line numbers exact) |
| **C2** | Deleting both is safe — frontend does not depend on reading a key back | **PARTIALLY CONFIRMED / OVERSTATED** — `get_provider_key` has **0** call sites (safe); `get_github_pat` has **1** live call site (`Settings.tsx:462`) that is a presence probe. Deleting it *without a replacement* breaks the Settings PAT status UI. |
| **C3** | `vault/keychain.rs` has no `list`/enumerate | **CONFIRMED** |
| **C4** | `src-cli/src/main.rs:30-32` global `--api-key`; `:147-150` `ANTHROPIC_API_KEY` fallback | **CONFIRMED** (both line ranges exact) |
| **C5** | Deletion set is exactly {`server.rs`, `tool_adapter.rs`, `port_status.rs`, parts of `mod.rs`, 2 commands, `main.rs` wiring} ≈ 705 LOC | **REFUTED (incomplete)** — LOC arithmetic is fine (~700), but the set **omits 4 out-of-set Rust files and 4 frontend files** that consume the deleted symbols. Deleting exactly this set does **not** compile. |
| **C6** | `health_monitor.rs` survives; imports nothing from `server.rs`/`tool_adapter.rs`/sidecar | **CONFIRMED** |
| **C7** | `port_status.rs:~17` = `use super::server::{MCP_SERVER_PORT, current_mcp_port}`; dies with server | **CONFIRMED** (line 17 exact) |
| **C8** | `agent_role` set in exactly one place (`tool_adapter.rs:280`); 3 sites in `tool_executor.rs` hardcode `None`/`depth: 0` | **CONFIRMED** (struct literals open at `:84`, `:247`, `:311`; the `agent_role: None` lines are `:89`, `:252`, `:316` as claimed) |
| **C9** | All three ACP tools hard-refuse when `agent_role` absent | **CONFIRMED** (`send_worker_message.rs:129-137` exact) |
| **C10** | `rmcp` + `axum` consumed only by the deleted files | **CONFIRMED, and UNDERSTATED** — `socket2` is *also* exclusive to `server.rs:19` and is equally droppable. Claim misses it. |
| **C11** | `Settings.tsx:653` claims GitHub MCP connection; nothing in Rust spawns it | **CONFIRMED** (line 653 exact; zero `server-github` hits in any Rust/Python/TS source outside that string and the planning docs) |

---

## 2. Per-claim detail

### C1 — CONFIRMED

`src-tauri/src/commands/vault_commands.rs` (34 lines total):

```rust
11  #[tauri::command]
12  pub fn get_provider_key(provider: String) -> Result<Option<String>> {
13      VaultManager::get(&provider_key(&provider))
14  }
...
26  #[tauri::command]
27  pub fn get_github_pat() -> Result<Option<String>> {
28      VaultManager::get(&github_pat_key())
29  }
```

`VaultManager::get` (`src-tauri/src/vault/keychain.rs:30-38`) returns `Ok(Some(val))` where `val` is
`entry.get_password()` — the plaintext secret. Both commands are registered in the webview handler:
`src-tauri/src/lib.rs:52-53` (import) and `:202`, `:205` (`generate_handler!` list). So yes: any JS
in the webview can `invoke('get_provider_key', {provider:'anthropic'})` and receive the plaintext key.
**Line numbers as claimed. CONFIRMED.**

### C2 — PARTIALLY CONFIRMED / OVERSTATED

Exhaustive enumeration. Grep over `src/` for all six vault command names returns **five** call sites,
and only two are `get*`:

| Call site | Command | What it does with the value |
|---|---|---|
| `src/tabs/Settings.tsx:462` | `get_github_pat` | `.then(v => setHasPat(v !== null && v !== ''))` — **presence test only**; the string is never stored, rendered, or forwarded. |
| — | `get_provider_key` | **ZERO call sites anywhere in `src/`.** |
| `src/tabs/Settings.tsx:472` | `store_github_pat` | writes |
| `src/tabs/Settings.tsx:486` | `delete_github_pat` | deletes |
| `src/stores/settingsStore.ts:111` | `store_provider_key` | writes |
| `src/stores/settingsStore.ts:116` | `delete_provider_key` | deletes |

```tsx
461  useEffect(() => {
462    invoke<string | null>('get_github_pat')
463      .then(v   => setHasPat(v !== null && v !== ''))
464      .catch(()  => setHasPat(false))
465  }, [])
```

Provider-key presence is *already* surfaced without the secret: `ProviderInfo.has_key: boolean`
(`src/stores/settingsStore.ts:34`), fed by `invoke<ProviderInfo[]>('list_providers')` at
`settingsStore.ts:81`. So `get_provider_key` is genuinely dead on the frontend.

**Why this is OVERSTATED, not CONFIRMED:** the claim as written ("the frontend does not depend on
reading a key back") is *semantically* true — no consumer needs the secret's *value* — but it is
operationally misleading. `Settings.tsx:462` does depend on the *command existing*. Delete
`get_github_pat` with no replacement and the GitHub panel's `hasPat` state silently falls into the
`.catch(() => setHasPat(false))` branch: the UI will permanently report "no PAT stored" even when
one is. That is a real (if cosmetic) regression, and it is a silent one — the `.catch` swallows it.

**Required by the phase, not stated in the claim:** add a `has_github_pat() -> Result<bool>` command
(and, for symmetry, keep provider presence on `list_providers`) and repoint `Settings.tsx:462`
*in the same commit*. If the phase deletes the command without this, the claim is **REFUTED in
practice**.

### C3 — CONFIRMED

`src-tauri/src/vault/keychain.rs` is 57 lines. The complete public surface:

- `VaultManager::store` (`:20`), `VaultManager::get` (`:30`), `VaultManager::delete` (`:41`)
- `provider_key(&str) -> String` (`:52`), `github_pat_key() -> String` (`:56`)
- `pub struct VaultEntry { key, value }` (`:10`)

No `list`, `enumerate`, `keys`, `all`, or iteration of any kind. The underlying `keyring` v2 `Entry`
API is per-(service, username) anyway. So a key written for a provider that is later removed from the
provider list is unreachable through this module — no orphan sweep is possible without adding an
index. **CONFIRMED.**

*Bonus finding (not claimed):* `VaultEntry` (`keychain.rs:10-13`) is re-exported at
`src-tauri/src/vault/mod.rs:8` and **has zero constructors and zero consumers** anywhere in the
workspace. Dead type.

### C4 — CONFIRMED, line numbers exact

```rust
// src-cli/src/main.rs
29      /// API key override — also reads AIRSDEV_API_KEY or ANTHROPIC_API_KEY
30-31   #[arg(long, global = true, env = "AIRSDEV_API_KEY")]
32      pub api_key: Option<String>,
```

(The `#[arg]` attribute is line 31 and the doc comment line 30; the claim's "around lines 30-32" is
correct.) Note `global = true` — the flag is accepted on **every** subcommand, and `env = "AIRSDEV_API_KEY"`
means clap itself reads a second env var before the explicit fallback fires.

```rust
147   // Override api_key: prefer AIRSDEV_API_KEY, fall back to ANTHROPIC_API_KEY
148   let ctx = if ctx.api_key.is_none() {
149       let fallback = std::env::var("ANTHROPIC_API_KEY").ok();
150       OutputContext { api_key: fallback, ..ctx }
151   } else {
152       ctx
153   };
```

Exactly lines 147-150 as claimed. The key lands in `OutputContext.api_key`
(`src-cli/src/output.rs:18`) and is used as a bearer token at `src-cli/src/commands/agent.rs:156`,
`:270`, `:308`, `:331`, `:353`. **CONFIRMED.**

### C5 — REFUTED (the set is incomplete)

**LOC arithmetic is defensible.** `wc -l`:

| File | LOC |
|---|---|
| `src-tauri/src/mcp_server/server.rs` | 228 |
| `src-tauri/src/mcp_server/tool_adapter.rs` | 286 |
| `src-tauri/src/mcp_server/port_status.rs` | 172 |
| **subtotal (whole-file deletes)** | **686** |
| `mcp_server/mod.rs` — lines 11, 12, 14, 16-19 | ~7 |
| `sidecar_commands.rs` — `McpToolInfo` + `list_mcp_tools` (`:301-326`) | ~26 |
| `sidecar_commands.rs` — `get_mcp_port_status` (`:143-149`) | ~7 |
| `lib.rs` — import + handler entries (`:78`, `:272`, `:274`) | ~3 |
| `main.rs` — `:16`, `:105-110`, `:184`, `:236-244` | ~18 |
| **total** | **~747** |

"~705" is in the right neighbourhood; I am not refuting on arithmetic.

**I am refuting on completeness.** `mcp_server/mod.rs:18` re-exports `current_mcp_port` and
`is_ready` from `server.rs`, and `:19` re-exports `McpPortStatus` / `port_snapshot` from
`port_status.rs`. Those re-exports are consumed from **four Rust files outside the stated deletion
set**. Delete exactly the C5 set and `cargo check` fails with (at minimum) six `unresolved import` /
`no function` errors. Full inventory in §3.

### C6 — CONFIRMED

`src-tauri/src/mcp_server/health_monitor.rs` (400 LOC). Every `use` at the top of the file:

```rust
14  use std::collections::HashMap;
15  use std::sync::Arc;
16  use std::time::{Duration, Instant};
18  use serde::{Deserialize, Serialize};
19  use tauri::{AppHandle, Emitter};
20  use tokio::sync::RwLock;
21  use tracing::{debug, warn, info};
```

No `super::`, no `crate::mcp_server::`, no `crate::sidecar::`. The only in-crate references in the
whole file are to `crate::events` (`:167`, `:168`, `:264`, `:276`, `:307`, `:360`, `:390`, `:394`) —
event-name constants defined at `src-tauri/src/events.rs:74-79`, all of which survive. It compiles
standalone once `mod.rs` is reduced to `pub mod health_monitor;`. **CONFIRMED.**

### C7 — CONFIRMED, line number exact

```rust
// src-tauri/src/mcp_server/port_status.rs
17  use super::server::{MCP_SERVER_PORT, current_mcp_port};
```

Both symbols are used at `:46`, `:47`, `:51`, `:136`, `:137`. `MCP_SERVER_PORT` is defined at
`server.rs:30`; `current_mcp_port()` at `server.rs:50` and reads the `BOUND_PORT: AtomicU16`
(`server.rs:41`) that only `bind_with_fallback()` (`server.rs:95`) ever writes. With the server gone
the whole file is meaningless *and* uncompilable. **CONFIRMED.** `port_status.rs` must be deleted,
not kept.

### C8 — CONFIRMED

Struct definition — `src-tauri/src/agent/tool_trait.rs:37-48`:

```rust
37  pub struct ToolContext {
38      pub session_id:  String,
39      pub working_dir: String,
40      pub call_id:     String,
44      pub depth:       u32,
47      pub agent_role:  Option<String>,
48  }
```

`#[derive(Debug, Clone)]` only — **no `Default`**, so every construction is an explicit literal and
the grep for `ToolContext {` is exhaustive. **Exactly four construction sites in the entire
workspace** (none in `src-cli`, none in `src-shared`, none in any `#[cfg(test)]` module):

| Site | `depth` | `agent_role` |
|---|---|---|
| `agent/tool_executor.rs:84` (literal), `:88`/`:89` (fields) | `0` | `None` |
| `agent/tool_executor.rs:247` (literal), `:251`/`:252` | `0` | `None` |
| `agent/tool_executor.rs:311` (literal), `:315`/`:316` | `0` | `None` |
| `mcp_server/tool_adapter.rs:275` (literal), `:279`/`:280` | from `__depth` | from `__agent_role` |

The claim's "around lines 89, 252, 316" points at the `agent_role: None` field lines; the literals
open three-to-five lines earlier. The values come from `tool_adapter.rs:191`
(`args.remove("__agent_role")`) and the matching `__depth` extraction. **CONFIRMED — and this is the
single most consequential fact in the whole set.** See §5.

### C9 — CONFIRMED

All three hard-refuse with `ToolResult::error`, no fallback, no "unknown" default:

```rust
// src-tauri/src/agent/builtin_tools/send_worker_message.rs
129  let from = match ctx.agent_role.as_ref() {
130      Some(r) if !r.trim().is_empty() => r.trim().to_string(),
131      _ => return ToolResult::error(
132          "send_worker_message refused: no __agent_role on the call. The \
133           ACP is fleet-only — PA/interactive callers cannot send \
134           worker messages. If you're a fleet worker, ensure your MCP \
135           client forwards __agent_role."
136      ),
137  };
```

```rust
// src-tauri/src/agent/builtin_tools/list_worker_messages.rs:60-66
60  let to_slug = match ctx.agent_role.as_ref() {
61      Some(r) if !r.trim().is_empty() => r.trim().to_string(),
62      _ => return ToolResult::error(
63          "list_worker_messages refused: no __agent_role on the call. \
64           The ACP is fleet-only — every reader must identify themselves."
65      ),
66  };
```

```rust
// src-tauri/src/agent/builtin_tools/acknowledge_worker_message.rs:48-55
48  let caller = match ctx.agent_role.as_ref() {
49      Some(r) if !r.trim().is_empty() => r.trim().to_string(),
50      _ => return ToolResult::error(
51          "acknowledge_worker_message refused: no __agent_role on the \
52           call. The ACP is fleet-only — every ack must identify the \
53           caller."
54      ),
55  };
```

Line range `129-137` in `send_worker_message.rs` is **exact**. **CONFIRMED.**

### C10 — CONFIRMED, and understated

Every `rmcp` / `axum` / `socket2` reference in `src-tauri/src/`:

| Symbol | Sites | All inside deletion set? |
|---|---|---|
| `rmcp` | `tool_adapter.rs:11`, `:128`; `server.rs:15`, `:194` | Yes |
| `axum` | `server.rs:197`, `:224` | Yes |
| `socket2` | `server.rs:19` | **Yes — claim omits this** |

(`routing/patterns.rs:31` contains the *string literal* `"rmcp"` in a keyword list and
`claim_task_tool.rs:73` mentions rmcp in a comment — neither is a code dependency.)

`src-cli/Cargo.toml` and `src-shared/Cargo.toml` declare **neither** `axum` nor `rmcp` nor `socket2`,
and grep over `src-cli/src` + `src-shared` for all three returns zero hits. So after the deletion,
`src-tauri/Cargo.toml` lines **50** (`rmcp`), **53** (`axum`) and **58** (`socket2`) are all droppable.

⚠️ **Do NOT drop `sysinfo`** (`Cargo.toml:94`). `port_status.rs:123` uses it, but so do
`commands/sidecar_commands.rs:423`, `:437`, `:440` and `orphan/pid_checker.rs:4`, `:10`, `:28`.

### C11 — CONFIRMED, line number exact

```tsx
// src/tabs/Settings.tsx:652-654
652  <div className="font-mono text-xs" style={{ color: 'var(--text-3)' }}>
653    Connects to the GitHub MCP server (<code>@modelcontextprotocol/server-github</code>) using your stored PAT.
654    Enables workers to list repos, create PRs, and review code.
```

A repo-wide grep for `server-github` / `modelcontextprotocol/server` (excluding `node_modules`)
returns hits only in: this line, `docs/reference/rustmcpexplained.md:170` (a tutorial example using
`server-everything`), and the planning docs themselves. **Zero hits in any `.rs`, `.py`, `.json`, or
`.toml` under `src-tauri/`, `src-cli/`, `src-shared/`, `sidecars/`, `submodules/`, `scripts/`.**

The entire GitHub-MCP backend is `src-tauri/src/commands/github_mcp_commands.rs` — 84 lines, two
commands (`get_github_mcp_config:49`, `set_github_mcp_config:67`), both pure SQLite key/value against
`config.db`. No `Command::new`, no `spawn`, no `npx`, no HTTP.

The only runtime consumer is `src-tauri/src/main.rs:253-262`, which — for the stdio case — registers
the literal placeholder string:

```rust
261   monitor_ref.register_server("github-mcp", "stdio://github-mcp").await;
```

i.e. it inserts a display row into the health monitor's `HashMap`. Nothing connects. **The sentence
at Settings.tsx:653 is false as written. CONFIRMED.**

**Escalation the claim does not make:** `github.pat` is **written and never read**. The only Rust
reader of `github_pat_key()` is `get_github_pat` (`vault_commands.rs:27`), whose only caller is the
presence probe at `Settings.tsx:462`. No HTTP client, no MCP client, no sidecar, nothing consumes the
PAT. The app asks the user for a GitHub Personal Access Token, stores it in the OS keychain, and no
code path ever uses it. That is a "prove the reader reads it" failure and arguably a bigger
disclosure issue than the read-back command itself.

---

## 3. BUILD-BREAK INVENTORY (the important section)

### 3a. Per-file dependents of the deletion set

#### `mcp_server/server.rs` — symbols `MCP_SERVER_PORT`, `current_mcp_port`, `is_ready`, `start_mcp_server`

| Dependent | Line | Reference | In deletion set? |
|---|---|---|---|
| `mcp_server/port_status.rs` | 17, 46, 47, 51, 136, 137 | `use super::server::{MCP_SERVER_PORT, current_mcp_port}` | ✅ yes (deleted) |
| `mcp_server/mod.rs` | 18 | `pub use server::{current_mcp_port, is_ready};` | ✅ yes (edited) |
| `src-tauri/src/main.rs` | 16, 241 | `use autodev_lib::mcp_server::server::start_mcp_server;` / call | ✅ yes (edited) |
| **`src-tauri/src/commands/sidecar_commands.rs`** | **129** | `port: crate::mcp_server::current_mcp_port(),` inside `get_full_sidecar_status` | ❌ **NO — BUILD BREAK** |
| **`src-tauri/src/sidecar/client.rs`** | **1273** | `let mcp_port = crate::mcp_server::current_mcp_port();` in `register_mcp_server()` | ❌ **NO — BUILD BREAK** |
| **`src-tauri/src/sidecar/client.rs`** | **1394, 1399** | `crate::mcp_server::is_ready()` / `current_mcp_port()` in `mcp_server_healthy()` | ❌ **NO — BUILD BREAK** |
| **`src-tauri/src/recipes/runner.rs`** | **310, 329** | `crate::mcp_server::current_mcp_port()` | ❌ **NO — BUILD BREAK** |

#### `mcp_server/tool_adapter.rs` — symbol `AutodevMcpServer`

| Dependent | Line | In deletion set? |
|---|---|---|
| `mcp_server/server.rs` | 24, 192 | ✅ yes |
| `agent/builtin_tools/path_lock.rs` | 40 | comment only — no build impact |
| `agent/builtin_tools/mod.rs` | 45, 129 | comments only — but they go **stale** (no-stale rule) |

`tool_adapter.rs` is the sole caller of `register_for_mcp_worker`? No — `server.rs:151` is. Both die.
`register_for_mcp_worker` (`agent/builtin_tools/mod.rs:96`) then has **zero callers**. It is `pub` in
a lib crate so it will not warn, but it becomes ~60 lines of dead registration code.

#### `mcp_server/port_status.rs` — symbols `McpPortStatus`, `snapshot`, `find_pid_on_port`, `is_ghost_pid`

| Dependent | Line | In deletion set? |
|---|---|---|
| `mcp_server/mod.rs` | 19 | ✅ yes |
| `mcp_server/server.rs` | 212 | ✅ yes |
| **`src-tauri/src/commands/sidecar_commands.rs`** | **148, 149** | ❌ **NO — BUILD BREAK** (this *is* the `get_mcp_port_status` command, which C5 does list — but the file it lives in is not named) |
| `src-tauri/src/events.rs` | 83 (doc comment naming the payload type) | ❌ no — **stale comment**, not a break |

**Answer to "is `port_status` used anywhere outside `mcp_server/`?" → YES**, at
`src-tauri/src/commands/sidecar_commands.rs:148-149`, plus a doc reference at `events.rs:83`.

#### Deleted Tauri commands — frontend dependents

| Deleted command | Frontend consumer | Effect |
|---|---|---|
| `list_mcp_tools` | `src/components/sidecar/McpToolBrowser.tsx:31` | The **entire component** loses its only data source. Must be deleted or stubbed. |
| `get_mcp_port_status` | `src/stores/sidecarStore.ts:156` (`loadMcpPortStatus`), state field `:69`, `:77`, `:90`, `:154` | Store action becomes a permanently-rejecting invoke. |
| `mcp:port-conflict` event (`events.rs:84`) | `src/stores/sidecarStore.ts:309` (listener), comment `:45` | Listener for an event nothing emits any more (only emitter was `server.rs:220`). |
| — (downstream) | `src/components/sidecar/SidecarPanel.tsx:118-128, 265-306` | The whole port-conflict banner block (~42 lines of JSX) is dead. |

Tauri `invoke` of a non-registered command **rejects at runtime, not compile time** — TypeScript will
build clean and the failure will only show up in the app. These four frontend files are therefore
*silent* breaks, which is worse than loud ones.

#### `main.rs` wiring that becomes orphaned

`main.rs:184` — `.manage(mcp_registry.clone())` — its **only** consumer is `list_mcp_tools`
(`sidecar_commands.rs:313: registry: State<'_, Arc<ToolRegistry>>`). No other command in
`src-tauri/src/commands/` takes a `State<'_, Arc<ToolRegistry>>`. So `main.rs:108-110` +`:184`
+`:239` all go together, and `builtin_tools::register_all` (`agent/builtin_tools/mod.rs:78`) loses its
**only** call site (`main.rs:109`) — `engine.rs:60` uses `register_for_mode` instead. `register_all`
becomes dead.

### 3b. `mcp_server/mod.rs` — what it re-exports and what survives

```rust
11  pub mod tool_adapter;      // ← DELETE
12  pub mod server;            // ← DELETE
13  pub mod health_monitor;    // ← KEEP (only surviving line)
14  pub mod port_status;       // ← DELETE
16-17  // comment about port-resolution surface  ← DELETE (stale)
18  pub use server::{current_mcp_port, is_ready};              // ← DELETE
19  pub use port_status::{McpPortStatus, snapshot as port_snapshot}; // ← DELETE
```

**Neither re-export survives.** Line 13 is the only survivor. The file's entire header comment
(`:1-9`, describing the MCP bridge and citing "DEVELOPMENT_PLAN.md Phase 3.4") also goes stale.
Consider renaming the module — after the deletion `mcp_server` contains no server, only an external-MCP
health poller; `mcp_health` would be honest. (Flag, not a build issue.)

### 3c. `main.rs` — every MCP reference by line

| Line | Content | Fate |
|---|---|---|
| 16 | `use autodev_lib::mcp_server::server::start_mcp_server;` | **DELETE** |
| 17 | `use autodev_lib::mcp_server::health_monitor::McpHealthMonitor;` | **KEEP** |
| 105-107 | `// Phase 3.4 — start AutoDev MCP Server in-process.` comment | **DELETE** |
| 108 | `let mut mcp_registry = ToolRegistry::new();` | **DELETE** |
| 109 | `builtin_tools::register_all(&mut mcp_registry);` | **DELETE** |
| 110 | `let mcp_registry = Arc::new(mcp_registry);` | **DELETE** |
| 157-159 | Ruflo Feature 6 comment | **KEEP** |
| 160 | `let mcp_health_monitor = Arc::new(McpHealthMonitor::new(30, 3, 10));` | **KEEP** |
| 170 | `let mcp_health_for_setup = mcp_health_monitor.clone();` | **KEEP** |
| 184 | `.manage(mcp_registry.clone())  // …so list_mcp_tools can iterate` | **DELETE** |
| 186 | `.manage(mcp_health_monitor)` | **KEEP** |
| 236-238 | `// Phase 3.4 — start AutoDev MCP Server as an async task.` | **DELETE** |
| 239 | `let mcp_registry_for_server = mcp_registry.clone();` | **DELETE** |
| 240-244 | `tauri::async_runtime::spawn(… start_mcp_server(…) …)` | **DELETE** |
| 246-266 | Health-monitor start block (`register_server` ×2, `start_monitoring`) | **KEEP** |
| 405-409 | `orch_probe.register_mcp_server()` + eprintln pair | **DELETE** (its callee dies — see below) |

### 3d. Does `health_monitor.rs` get registered from anywhere in the deletion set? — **NO. It is safe.**

Its registration is `main.rs:246-266`, which is health-monitor wiring, not MCP-server wiring:

- `main.rs:160` — construct `McpHealthMonitor::new(30, 3, 10)`
- `main.rs:186` — `.manage(mcp_health_monitor)`
- `main.rs:257` / `:261` — `monitor_ref.register_server("github-mcp", …)`
- `main.rs:266` — `monitor_ref.start_monitoring(app_for_mcp)`
- Commands: `sidecar_commands.rs:585` (`get_mcp_health`), `:595` (`force_reconnect_mcp`)
- Frontend: `src/stores/sidecarStore.ts:144`, `src/components/sidecar/McpHealthPanel.tsx:171`

None of that is in the deletion set. **It survives as live code, not dead code.**

**However** — and the phase should know this — `health_monitor.rs` is *already* near-inert for its
stated purpose. `main.rs:261` registers the literal string `"stdio://github-mcp"`, and per C11 no such
process exists. So the monitor is polling/displaying a server that is never spawned. It survives the
deletion structurally, but it is monitoring a fiction. That is a separate defect the MCP-server
deletion neither causes nor fixes.

### 3e. Does anything still need `axum` after the deletion? — **NO**

Whole-workspace check:
- `src-tauri`: `axum` only at `server.rs:197`, `:224` (deleted). Same for `rmcp` and `socket2`.
- `src-cli/Cargo.toml`: no axum/rmcp/socket2 declared; zero source hits.
- `src-shared/Cargo.toml`: no axum/rmcp/socket2 declared; zero source hits.

**`axum` (Cargo.toml:53), `rmcp` (:50) and `socket2` (:58) can all three be dropped.** No HTTP server
remains anywhere in the workspace — `reqwest` (client-side) is what `src-cli` and `src-shared` use.

### 3f. Fix-list the phase must execute for the build to be green

1. `commands/sidecar_commands.rs:129` — remove the `port:` field's call or hardcode/remove the
   `autodev-mcp-server` `ServiceStatus` entry (`:122-131`) entirely, since the service no longer exists.
2. `commands/sidecar_commands.rs:143-149` — delete `get_mcp_port_status`.
3. `commands/sidecar_commands.rs:301-326` — delete `McpToolInfo` + `list_mcp_tools`.
4. `sidecar/client.rs:1262-…` — delete `register_mcp_server()` entirely (it registers a server that
   won't exist), and remove its call site at `main.rs:406`.
5. `sidecar/client.rs:1388-…` — delete `mcp_server_healthy()`, **and** its caller at
   `recipes/runner.rs:215` plus the whole `mcp_up` gate that follows.
6. `recipes/runner.rs:310, 329` — remove the two `current_mcp_port()` calls in the abort/proceed log
   messages (and re-word or delete the surrounding Mode-2 gate — see §5).
7. `sidecar/mod.rs:416, 439, 692-708` — `mcp_status` field, `mark_mcp_running`, `mark_mcp_error`,
   `mcp_status()` now describe a non-existent service. Not a build break (they compile fine) but they
   report `SidecarStatus::Running` forever for a deleted server.
8. `lib.rs:78, 272, 274` — remove `get_mcp_port_status` and `list_mcp_tools` from imports and
   `generate_handler!`.
9. `events.rs:83-84` — `EVT_MCP_PORT_CONFLICT` loses its only emitter (`server.rs:220`). Delete const
   + doc comment.
10. Frontend: delete `src/components/sidecar/McpToolBrowser.tsx` and its mount point; strip
    `mcpPortStatus`/`loadMcpPortStatus` from `src/stores/sidecarStore.ts` (`:45`, `:69`, `:77`, `:90`,
    `:154-160`, `:309`); strip the banner from `src/components/sidecar/SidecarPanel.tsx`
    (`:118-128`, `:265-306`).
11. `Cargo.toml:50, 53, 58` — drop `rmcp`, `axum`, `socket2`. Keep `sysinfo`.
12. Stale comments to fix in the same step: `agent/builtin_tools/mod.rs:45`, `:129`;
    `agent/builtin_tools/path_lock.rs:40`; `agent/builtin_tools/claim_task_tool.rs:71-74`;
    `mcp_server/mod.rs:1-9`; `agent/builtin_tools/request_supervisor_action.rs:2`, `:95-106`;
    `agent/builtin_tools/set_task_budget_tool.rs:7`; `sidecar/client.rs:758`
    (`"mcp_servers": ["autodev-mcp-server"]` — a *live JSON payload*, not a comment; see §5).

---

## 4. What I could not determine

1. **Whether `cargo check` actually produces exactly the errors I predict.** I was instructed not to
   run cargo. The inventory in §3 is derived from exhaustive grep of `use` statements and symbol
   references, which is sound for Rust path resolution but cannot catch a break arising from trait
   coherence, a macro expansion, or a `#[cfg]`-gated path I did not exercise. **The phase must run
   `cargo check -p autodev` after the deletion; treat §3f as a floor, not a ceiling.**
2. **Whether core-orchestrator (the Python sidecar) has its own persisted registration of
   `autodev-mcp-server`.** `sidecar/client.rs:758` sends `"mcp_servers": ["autodev-mcp-server"]` in an
   agent-creation payload and `client.rs:1440-1458` forwards an `mcp_servers` array. If the
   orchestrator has already persisted that registration in its own store, deleting the Rust server
   leaves the sidecar trying to reach `127.0.0.1:8767` on every LivingAgent tick. I could not inspect
   the orchestrator's state (it is a submodule/sidecar, and I did not run it).
3. **Whether the keychain currently holds a `github.pat`.** I did not touch the OS keychain. If one
   exists, the phase's story for it (leave / delete / migrate) is unresolved and — per C3 — there is
   no enumeration API to find it with.
4. **Whether `Settings.tsx:462`'s `hasPat` state has any other consumer I missed** beyond the
   badge in the same component. I read the surrounding block, not the full 700+ line file.
5. **The exact intended scope of "the `main.rs` wiring"** in C5. I have partitioned it in §3c on the
   evidence, but if the phase author intended lines 157-266 wholesale, health_monitor's registration
   dies with it and C6's "survives" becomes false-in-practice. **This needs an explicit answer before
   the delete.**

---

## 5. What the claims MISS

**M1 — The single largest functional consequence is never stated: deleting `tool_adapter.rs` makes
`agent_role` permanently `None` process-wide, which permanently disables three tools and silently
degrades four more.**

C8 and C9 each state half of it; together they imply it, but no claim draws the conclusion. Per C8
there are exactly four `ToolContext` construction sites and only `tool_adapter.rs:275` ever sets
`agent_role` to a real value. Delete it and `ctx.agent_role` is `None` at every remaining call site
forever. Consequences:

| Tool | Line | Behaviour after deletion |
|---|---|---|
| `send_worker_message` | `:129-137` | **Always refuses.** Permanently unusable. |
| `list_worker_messages` | `:60-66` | **Always refuses.** Permanently unusable. |
| `acknowledge_worker_message` | `:48-55` | **Always refuses.** Permanently unusable. |
| `claim_task_tool` | `:85-90` | Falls through to `"unknown"` + a warning. Orphan detection degrades. |
| `request_supervisor_action` | `:102-106` | Records requester as `"unknown"`. |
| `annotate_file_tool` | `:77` | `match ctx.agent_role.as_deref()` — takes the `None` arm always. |
| `update_task_status_tool` | `:133-137` | `if status == "qc_review" && ctx.agent_role.is_some()` — this branch becomes **unreachable**. A QC-gate path silently stops existing. |

That last one is the dangerous kind: a guard that stops firing without erroring. The phase is
deleting an entire inter-agent communication protocol (ACP) and one QC-gate branch as a *side effect*
of removing an HTTP transport, and nothing in the claim set says so. **This needs an explicit
decision, not a silent consequence.**

**M2 — `sidecar/client.rs:758` is a live payload, not a comment.**

```rust
758   "mcp_servers": ["autodev-mcp-server"],
```

This string is sent to core-orchestrator when creating an agent. After the deletion it names a server
that does not exist. Also `subagent/manager.rs:87` → `SubagentConfig::DEFAULT_MCP_SERVER` defaults
every subagent to the same name (`subagent/types.rs:48`, `:209`; `subagent/bridge.rs:63-66, 103`).
Four `builtin_tools` spawn sites pass `mcp_servers: Vec::new()` with the comment
"default = autodev-mcp-server" (`spawn_subagent.rs:148`, `spawn_parallel_subagents.rs:149`,
`spawn_qc_review.rs:162`). **The whole subagent-spawn path assumes the deleted server exists.**
None of this appears in C5's deletion set or in the LOC count.

**M3 — `subagent/bridge.rs:78` documents that `tool_adapter` is the *only* thing that forwards
`__depth`:**

> "Without `__depth` forwarding, the tool_adapter always sees depth=0 and …"

The subagent nesting-depth guard (`MAX_NESTING_DEPTH=3`, per `server.rs:135-136` and
`builtin_tools/mod.rs:129`) is enforced from `ctx.depth`, which after the deletion is hardcoded `0`
at all three surviving sites. **The recursion guard against runaway subagent fan-out stops working.**
This is a safety property, not a feature.

**M4 — `socket2` is a fourth droppable dependency** (`Cargo.toml:58`), used only at `server.rs:19`.
C10 names two.

**M5 — The GitHub PAT is write-only.** Beyond C11's "the string is false": `github.pat` is stored by
`store_github_pat` and read by nothing except a presence probe. The app collects a credential it never
uses. Deleting `get_github_pat` (C2) removes even that reader, at which point the PAT is *provably*
write-only. Either wire it to something or stop asking for it — but per C3 there is no way to
enumerate and clean up PATs already sitting in users' keychains.

**M6 — `VaultEntry` (`vault/keychain.rs:10`, re-exported `vault/mod.rs:8`) has zero constructors and
zero consumers.** Free deletion, not claimed.

**M7 — Frontend breakage is silent.** `invoke()` on an unregistered command rejects at *runtime*.
`tsc --noEmit` and `vite build` will both pass with all four frontend consumers pointing at dead
commands. Rule 7's smoke test — open Settings and the Sidecar Panel in the running app — is the only
thing that catches this. A green build is not evidence here.

**M8 — `sidecar/mod.rs:439` initialises `mcp_status: Mutex::new(SidecarStatus::Running)`** with the
comment "MCP is in-process, starts immediately." After the deletion the Sidecar Panel will report the
AutoDev MCP Server as permanently **Running** — a green light for a service that no longer exists.
Inert-but-green, exactly the failure class the house rules name.
