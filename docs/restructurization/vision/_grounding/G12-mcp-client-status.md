# G12 — MCP Client Status Audit

**Question asked (verbatim):** *"MCP client we keep in case we want to extend by letting the user add their own MCP server later on **and it is functional as of now right, check.**"*

**Method:** read-only. Every claim below is anchored to `path:line` and was verified by reading the function body, not the docstring. Where a docstring and the body disagree, the body is reported and the docstring is flagged.

---

## 1. Executive verdict

### Is the MCP client functional today?

## **NO.** There is no MCP client. Not a broken one — an absent one.

This is not "partially wired" or "wired but untested". There is **zero outbound MCP code in the repository**. Specifically, none of the four things that constitute an MCP client exist anywhere in `src-tauri/src`:

| MCP client capability | Exists? | Evidence |
|---|---|---|
| Outbound transport (HTTP-streamable client or stdio child process) | **NO** | `src-tauri/Cargo.toml:50` — `rmcp` is compiled with features `["server", "transport-streamable-http-server"]`. No `client` feature, no client transport. The protocol library is physically incapable of dialling out as built. |
| `initialize` handshake / protocol-version negotiation | **NO** | No occurrence of `ClientHandler`, `serve_client`, `rmcp::service`, `McpClient`, `mcp_client`, or `connect_mcp` anywhere under `src-tauri/src` (repo-wide grep). |
| `tools/list` against a remote server | **NO** | The only `list_tools` in the tree is `src-tauri/src/mcp_server/tool_adapter.rs:126` — an impl of rmcp's **`ServerHandler`**, i.e. AutoDev *answering*. `src-tauri/src/commands/sidecar_commands.rs:313` (`list_mcp_tools`) reads AutoDev's **own** in-process `ToolRegistry`; its own docstring at `:310` says it exists so you can inspect capabilities "without connecting an external MCP client". |
| `tools/call` dispatch to a remote server | **NO** | Same — `tool_adapter.rs:165` is the server-side handler. |

**What actually exists and is being mistaken for a client:**

1. `src-tauri/src/mcp_server/health_monitor.rs` (400 LOC) — an **HTTP `/health` liveness pinger**. `:190-209` builds `format!("{}/health", endpoint)` and does `http_client.get(&url).send()`. No JSON-RPC body, no session, no handshake. It treats HTTP 404 as success (`:199`), so it reports "Connected" for any URL that resolves. It cannot discover or invoke a single tool.
2. `src-tauri/src/commands/github_mcp_commands.rs` (84 LOC) — a **SQLite key/value config store**. Two commands, `get_github_mcp_config` (`:49`) and `set_github_mcp_config` (`:67`), read and write one JSON blob under key `github_mcp_config` in `app_settings` in `config.db`. No network, no process spawn, no protocol.
3. `src/tabs/Settings.tsx:587-660` — a **form** that writes into (2).

The UI text at `src/tabs/Settings.tsx:653-654` states: *"Connects to the GitHub MCP server (`@modelcontextprotocol/server-github`) using your stored PAT. Enables workers to list repos, create PRs, and review code."* A grep of the whole Rust tree for `server-github` / `modelcontextprotocol` returns **zero hits**. Nothing spawns that process. No worker ever receives a GitHub tool. **The settings card is a UI shell over a config write. Enabling it changes nothing except a row in `config.db` and the appearance of one "github-mcp" badge in the health panel.**

### The structural blocker, stated plainly

Even if a client were written tomorrow, discovered tools could not reach an agent. `src-tauri/src/agent/tool_registry.rs` has exactly one mutator, `register(&mut self, …)` at `:18`. Every registry in the app is built once and then frozen:

- `src-tauri/src/agent/engine.rs:59-61` — `AgentEngine::new` creates a local `ToolRegistry`, calls `builtin_tools::register_for_mode`, and moves it into the struct as a plain (non-`Arc`, non-`Mutex`, non-`RwLock`) field. Nothing can add to it afterwards.
- `src-tauri/src/main.rs:108-110` — the MCP-server registry, then `Arc::new(...)`. `Arc<ToolRegistry>` has no interior mutability, so `register` is unreachable from that point on.
- `src-tauri/src/mcp_server/server.rs:150-152` — same pattern again.

There is **no dynamic tool-registration path of any kind**. This corroborates the sibling finding (G10) that the entire tool surface originates from `agent/builtin_tools/mod.rs`.

### Does keeping the client depend on keeping the MCP *server*?

**Structurally, no. Practically, the "client" being kept is 484 LOC that does not do the job.**

- `health_monitor.rs` imports nothing from `server.rs` or `tool_adapter.rs`. It is self-contained (`reqwest` + `tokio` + `tauri`). It survives server deletion untouched.
- `github_mcp_commands.rs` likewise imports only `rusqlite`, `crate::errors`, `crate::db`. Survives.
- **`port_status.rs` does NOT survive.** The task brief lists it as external-MCP infrastructure; that is incorrect. `src-tauri/src/mcp_server/port_status.rs:17` is `use super::server::{MCP_SERVER_PORT, current_mcp_port};` and the whole file (`:19-41`, `:43-54`) is diagnostics for **AutoDev's own** port-8767 bind and its 8768/8769/8770 fallback chain. It is server code and dies with the server.

So the honest arithmetic: of the 1,189 LOC across `mcp_server/*` + `github_mcp_commands.rs`, the part that survives a server deletion and is arguably "the MCP client we keep" is **484 LOC — 400 of health-pinging and 84 of config storage — and neither line of it speaks MCP.**

### What is missing (see §7 for the scoped answer)

Everything on the wire: a client transport, the `initialize` handshake, `tools/list`, `tools/call`, a dynamic registration path into `ToolRegistry`, a per-server tool namespace, an approval/permission story for third-party tools, and credential injection. The config schema and the health-badge UI are the only pieces that already exist, and the config schema is GitHub-shaped (`org`/`repos`), not generic.

---

## 2. What exists — code map

### 2.1 Rust

| File | LOC | Role | Client? |
|---|---|---|---|
| `src-tauri/src/mcp_server/mod.rs` | 19 | Module decls + re-exports `current_mcp_port`, `is_ready`, `port_snapshot` (`:18-19`) | No |
| `src-tauri/src/mcp_server/server.rs` | 228 | **Server.** Binds 127.0.0.1:8767 w/ fallback (`:145` `start_mcp_server`), builds worker-scoped registry (`:150-152`) | No — server |
| `src-tauri/src/mcp_server/tool_adapter.rs` | 286 | **Server.** `ServerHandler` impl: `list_tools` (`:126`), `tools/call` (`:165`) | No — server |
| `src-tauri/src/mcp_server/port_status.rs` | 172 | **Server diagnostics.** `use super::server::{MCP_SERVER_PORT, current_mcp_port}` (`:17`); PID-on-port lookup (`:59`) for AutoDev's own port | No — server |
| `src-tauri/src/mcp_server/health_monitor.rs` | 400 | HTTP `/health` pinger for endpoints registered by name | **No — not MCP** |
| `src-tauri/src/commands/github_mcp_commands.rs` | 84 | Config store (`app_settings.github_mcp_config`) | **No — storage only** |
| `src-tauri/src/commands/sidecar_commands.rs:299-325` | 27 | `list_mcp_tools` — lists AutoDev's own tools | No |
| `src-tauri/src/commands/sidecar_commands.rs:579-599` | 21 | `get_mcp_health`, `force_reconnect_mcp` — thin wrappers over the pinger | No |
| `src-tauri/src/sidecar/client.rs:1258-~1360` | ~100 | `register_mcp_server` — POSTs AutoDev's own MCP URL to the orchestrator's `/api/mcp/servers` | No — sidecar |
| `src-tauri/src/sidecar/client.rs:1375-~1415` | ~40 | `mcp_server_healthy` — TCP probe of AutoDev's own port | No — sidecar |

`src-tauri/src/recipes/defaults.rs:1000-1082`, `:1464-1466`, `:1739-1741` contain `tools/list` / `tools/call` strings. These are **prompt/recipe text templates** instructing a worker agent how to build an MCP server. Not executable code. Ruled out.

### 2.2 Frontend

| File | LOC | Role |
|---|---|---|
| `src/components/sidecar/McpHealthPanel.tsx` | 230 | Renders health badges; `invoke('force_reconnect_mcp', …)` at `:171`. Mounted at `src/components/sidecar/SidecarPanel.tsx:480`. |
| `src/components/sidecar/McpToolBrowser.tsx` | 183 | `invoke<McpToolInfo[]>('list_mcp_tools')` at `:31` — displays **AutoDev's own** tools. Mounted at `src/tabs/Settings.tsx:55` under a `"MCP Tools"` header (`:54`). |
| `src/tabs/Settings.tsx:578-660` | ~82 | `GitHubMCPCard` — the only "add a server" affordance in the app. Mounted at `:83`. |
| `src/stores/sidecarStore.ts:203-309` | ~106 | Listeners for all 7 `mcp:*` events. Confirmed live (matches G3c's finding — no orphan events). |

### 2.3 Registration path — the only one

`src-tauri/src/main.rs:250-263` is the **sole** `register_server` call site in the tree:
```rust
if let Ok(gh_cfg) = ...::get_github_mcp_config() {
    if gh_cfg.enabled {
        if let Some(url) = &gh_cfg.server_url {
            monitor_ref.register_server("github-mcp", url).await;     // :257
        }
        if gh_cfg.server_url.is_none() {
            monitor_ref.register_server("github-mcp", "stdio://github-mcp").await;  // :261
        }
    }
}
```
`unregister_server` (`health_monitor.rs:111`) has **zero call sites** — its docstring at `:110` says it is called "when user removes it from Settings", and no such path exists.

---

## 3. The end-to-end trace, hop by hop

Target flow: *configure → connect → discover → register → agent calls → result returns.*

| # | Hop | Status | Evidence |
|---|---|---|---|
| 1 | **User configures a server** | ⚠️ PARTIAL | `src/tabs/Settings.tsx:587` `GitHubMCPCard` → `invoke('set_github_mcp_config', …)` at `:603`. Works, but is **GitHub-specific**: fields are `enabled`/`org`/`repos`/`server_url` (`github_mcp_commands.rs:14-19`). There is no "add arbitrary MCP server" form, no name field, no command/args for stdio, no env/headers, no list — exactly **one** hardcoded slot. |
| 2 | **Persisted** | ✅ | `github_mcp_commands.rs:78-82` → `app_settings` table in `~/.autodev/config.db` (`:26`). Single row, key `github_mcp_config` (`:23`). |
| 3 | **Read at startup** | ✅ | `main.rs:251`. |
| 4 | **Connect (MCP `initialize` handshake)** | ❌ **BREAK #1** | **Does not exist.** `main.rs:257/261` calls `register_server`, which (`health_monitor.rs:92-107`) only inserts a `McpServerHealth` struct into a `HashMap`. Nothing opens a connection. For the stdio path it inserts the literal string `"stdio://github-mcp"` and marks it `NotMonitored` (`:94`) — no process is spawned, nowhere in the tree (grep for `server-github` / `modelcontextprotocol`: zero Rust hits). |
| 5 | **Discover tools (`tools/list`)** | ❌ **BREAK #2** | **Does not exist.** No outbound `tools/list` anywhere. |
| 6 | **Register discovered tools into `ToolRegistry`** | ❌ **BREAK #3 — structural** | **Does not exist and cannot without a refactor.** `agent/tool_registry.rs:18` `register` needs `&mut self`; registries are frozen after construction at `engine.rs:59`, `main.rs:108-110`, `server.rs:150-152`. `to_definitions()` (`tool_registry.rs:31`) is what reaches the LLM, and it can only ever emit built-ins. |
| 7 | **Agent calls the tool** | ❌ | Unreachable — the LLM is never told the tool exists. |
| 8 | **Result returns** | ❌ | Unreachable. |

**The trace breaks at hop 4 of 8 and never recovers.** Hops 1–3 are a settings form writing to SQLite. Everything after is absent.

The only thing that *does* happen downstream is hop 4′: the pinger's 30-second loop (`health_monitor.rs:150-172`) GETs `{server_url}/health`, and `McpHealthPanel` renders a badge. That is a green light on a wire that is not connected to anything.

---

## 4. Configuration and UI surface

**How a user adds an MCP server today:** they cannot, in the general case. Settings → the `GitHubMCPCard` (`src/tabs/Settings.tsx:83`) exposes:
- an Enable/Disable toggle (`:640`)
- GitHub org/username (`:663`)
- allowed repos, newline-separated (`:672+`)
- `server_url` is in the `GhMcpConfig` interface (`:583`) and is sent on save (`:607` `serverUrl: cfg.server_url || null`) — but **there is no input control bound to it**. `cfg.server_url` is only ever set by the initial `get_github_mcp_config` load at `:595`. A user cannot type a URL. The HTTP branch at `main.rs:257` is therefore **unreachable from the UI**; every real user lands on the `stdio://github-mcp` branch at `:261`, which is `NotMonitored` forever.

**Backing Tauri commands (all exist in Rust, all registered):**
- `get_github_mcp_config` / `set_github_mcp_config` — `lib.rs:67`, `:241-242`
- `get_mcp_health` / `force_reconnect_mcp` — `lib.rs:87`, `:287`
- `list_mcp_tools` — `lib.rs:78`
- `get_mcp_port_status` — `lib.rs:78`, `:272`

**Persistence:** `~/.autodev/config.db`, table `app_settings`, key `github_mcp_config` (`github_mcp_commands.rs:23-42`). The PAT is *not* here — the docstring at `:3` says it lives in the OS keychain via `vault_commands`. Nothing reads the PAT for MCP purposes.

**Naming trap:** the Settings section header at `src/tabs/Settings.tsx:54` reads **"MCP Tools"** and renders `McpToolBrowser`, which lists AutoDev's *own* built-ins. A user will read that panel as "tools from my MCP servers". It is the opposite.

---

## 5. Survives / dies boundary for sidecar deletion

The requirement is that **not one line of sidecar code survives**. Here is the exact cut.

### (a) Client-ish code that SURVIVES (no server or sidecar dependency)

| File / range | LOC | Imports from server/sidecar? | Note |
|---|---|---|---|
| `src-tauri/src/mcp_server/health_monitor.rs` (whole) | 400 | **None.** `reqwest`, `tokio`, `tauri`, `serde`, `crate::events` only | Compiles standalone; must be **moved out of `mcp_server/`** (e.g. to `mcp_client/health.rs`) since that module dies |
| `src-tauri/src/commands/github_mcp_commands.rs` (whole) | 84 | None | Pure config store |
| `src-tauri/src/commands/sidecar_commands.rs:579-599` (`get_mcp_health`, `force_reconnect_mcp`) | 21 | None — but the **file** is sidecar-named and sidecar-scoped | **Must be relocated**, not kept in place, or sidecar code survives by address |
| `src/components/sidecar/McpHealthPanel.tsx` | 230 | Mounted at `SidecarPanel.tsx:480` | **Must be re-parented** out of `components/sidecar/` |
| `src/stores/sidecarStore.ts:203-309` (7 `mcp:*` listeners) | ~106 | Lives in the **sidecar store** | **Must be extracted** to its own store |
| `src-tauri/src/events.rs:74-79` (`EVT_MCP_HEALTH_UPDATE` … `EVT_MCP_REQUIRES_RESTART`) | 6 | None | Keep |
| `src/tabs/Settings.tsx:578-660` (`GitHubMCPCard`) | ~82 | None | Keep / generalise |

**Warning:** four of the seven surviving items physically live inside `sidecar`-named files or directories. "Keep the client, delete the sidecar" is a **file-move task**, not a delete task, or the no-sidecar-line requirement is violated on a technicality.

### (b) Server code that DIES

| File / range | LOC |
|---|---|
| `src-tauri/src/mcp_server/server.rs` | 228 |
| `src-tauri/src/mcp_server/tool_adapter.rs` | 286 |
| `src-tauri/src/mcp_server/port_status.rs` | 172 — server-port diagnostics, `:17` imports `super::server` |
| `src-tauri/src/mcp_server/mod.rs:11-12`, `:18-19` | 4 |
| `src-tauri/src/commands/sidecar_commands.rs:299-325` (`list_mcp_tools` + `McpToolInfo`) | 27 |
| `src-tauri/src/commands/sidecar_commands.rs:148-150` (`get_mcp_port_status`) | 3 |
| `src-tauri/src/main.rs:108-110` (MCP-server registry construction) | 3 |
| `src-tauri/src/main.rs:239-247` (`start_mcp_server` spawn) | 9 |
| `src-tauri/src/events.rs:84` (`EVT_MCP_PORT_CONFLICT`) | 1 |
| `src/components/sidecar/McpToolBrowser.tsx` + `Settings.tsx:21`, `:54-55` | 186 |
| `src/stores/sidecarStore.ts:309+` (`mcp:port-conflict` listener), `:45` | ~15 |
| `Cargo.toml:50` (`rmcp` server features) | 1 — **replace, don't delete**, if a client is ever built |

### (c) Sidecar-registration code that DIES

| File / range | LOC | What it does |
|---|---|---|
| `src-tauri/src/sidecar/client.rs:1258-~1360` (`register_mcp_server`) | ~100 | `POST {orchestrator}/api/mcp/servers` (`:1271`) with `{"id":"autodev-mcp-server", …, "url": "http://127.0.0.1:{port}/mcp"}` (`:1274-1287`); 409-conflict → `PUT /api/mcp/servers/autodev-mcp-server` (`:1331+`). This tells the **Python orchestrator** how to reach AutoDev. Pure sidecar. |
| `src-tauri/src/main.rs:405-410` | 6 | The single call site of the above, inside the orchestrator health-poll loop |
| `src-tauri/src/sidecar/client.rs:1375-~1415` (`mcp_server_healthy`) | ~40 | TCP probe of AutoDev's own MCP port to gate "Mode 2" worker dispatch |
| `src-tauri/src/recipes/runner.rs:303-330` | ~28 | Aborts a worker tick when the in-process MCP server is unreachable; logs `current_mcp_port()` at `:310`, `:329` |
| `src-tauri/src/commands/sidecar_commands.rs:121-131` | 11 | `ServiceStatus` row `"autodev-mcp-server"` incl. `port: current_mcp_port()` (`:129`) |

**Boundary summary:** deleting the MCP server + sidecar removes roughly **705 LOC of Rust and ~200 LOC of TSX**, and leaves **484 LOC of Rust + ~312 LOC of TSX** that is *called* the MCP client but performs no MCP.

---

## 6. Findings table

| ID | file:line | Sev | What it does | Why it is wrong | Concrete failure scenario | Status |
|---|---|---|---|---|---|---|
| **MC-01** | `src-tauri/Cargo.toml:50` | **CRITICAL** | `rmcp` with features `["server","transport-streamable-http-server"]` | No `client` feature and no client transport ⇒ the MCP library cannot dial out. The "MCP client we keep" has no protocol implementation to keep. | Owner defers "let users add MCP servers" believing 80% is built. It is 0% built; the deferral is priced wrong by an entire feature. | **CONFIRMED** |
| **MC-02** | `src/tabs/Settings.tsx:653-654` | **CRITICAL** | UI states it "Connects to the GitHub MCP server (`@modelcontextprotocol/server-github`) using your stored PAT. Enables workers to list repos, create PRs, and review code." | Nothing spawns that process (grep `server-github`/`modelcontextprotocol` over `src-tauri/src`: **zero hits**), no PAT is read for MCP, no tool reaches any worker. The sentence is false on all three counts. | User enables GitHub MCP, stores a PAT, sees an ENABLED badge, and asks a worker to open a PR. The worker has no such tool and hallucinates or fails. The user believes the feature is broken; it was never built. | **CONFIRMED** |
| **MC-03** | `src-tauri/src/agent/tool_registry.rs:18` + `agent/engine.rs:59-61`, `main.rs:108-110`, `mcp_server/server.rs:150-152` | **CRITICAL** | `register(&mut self, …)`; every registry frozen behind a move or `Arc` immediately after construction | No interior mutability ⇒ **no dynamic tool registration exists at any point in the process lifetime**. Even a perfect MCP client could not surface a discovered tool to an agent. | A client is added later; tools are discovered and displayed; agents still cannot call them. The last hop requires refactoring `ToolRegistry` to `RwLock`/`DashMap` and re-plumbing `engine.rs`. | **CONFIRMED** |
| **MC-04** | `src-tauri/src/mcp_server/health_monitor.rs:190-209` | **HIGH** | Health check = `GET {endpoint}/health`, 404 counted as success (`:199`) | This is not an MCP probe. Any live HTTP host returns "Connected". An MCP server that is up but has a broken `/mcp` endpoint, wrong protocol version, or invalid auth reports perfectly healthy. | User's MCP endpoint rotates its token. `/health` still 200s. Panel shows Connected. Every (future) tool call 401s with no diagnostic. | **CONFIRMED** |
| **MC-05** | `src/tabs/Settings.tsx:583`, `:607` vs `main.rs:257` | **HIGH** | `server_url` is in the config type and sent on save, but **no input control is bound to it** | The HTTP transport branch (`main.rs:257`) is unreachable from the UI. Every user falls to `main.rs:261` → `"stdio://github-mcp"` → `NotMonitored` (`health_monitor.rs:94`). | User enables GitHub MCP. The health panel shows a permanent grey "not monitored" badge with no explanation and no way to supply a URL. | **CONFIRMED** |
| **MC-06** | `src-tauri/src/mcp_server/health_monitor.rs:111` | **MEDIUM** | `unregister_server` — docstring says "when user removes it from Settings" | **Zero call sites** in the tree. Disabling GitHub MCP in Settings does not unregister it; only an app restart clears it (the map is in-memory, populated once at `main.rs:251-263`). | User disables GitHub MCP and clicks save. The badge stays until restart. Looks like the setting did not take. | **CONFIRMED** |
| **MC-07** | `src/tabs/Settings.tsx:54-55` + `src/components/sidecar/McpToolBrowser.tsx:31` | **MEDIUM** | Settings section titled **"MCP Tools"** renders `list_mcp_tools`, which lists AutoDev's **own** built-ins | Semantic inversion. Sits directly above the GitHub MCP card, so a user reads it as "tools my MCP servers provide". | User adds a server, opens "MCP Tools", sees ~30 unchanged built-ins, concludes discovery silently failed. | **CONFIRMED** |
| **MC-08** | `src-tauri/src/mcp_server/port_status.rs:17` | **MEDIUM** | `use super::server::{MCP_SERVER_PORT, current_mcp_port}` — entire file is own-port diagnostics | Filed under "external MCP infrastructure" in the deletion brief; it is not. Keeping it "for the client" retains a hard compile dependency on `server.rs`, blocking the server deletion. | Deletion PR keeps `port_status.rs`, the build breaks on the missing `super::server`, and someone resurrects part of `server.rs` to fix it. | **CONFIRMED** |
| **MC-09** | `src/components/sidecar/McpHealthPanel.tsx` (mounted `SidecarPanel.tsx:480`); `src/stores/sidecarStore.ts:203-309`; `src-tauri/src/commands/sidecar_commands.rs:579-599` | **MEDIUM** | The entire surviving "client" UI + command surface lives inside sidecar-named files/dirs | Violates "not one line of sidecar code survives" by address even when the logic is sidecar-free. | Sidecar deletion either drags the health panel out with it (client UI vanishes) or leaves `sidecar_commands.rs` / `sidecarStore.ts` / `components/sidecar/` alive. Both outcomes fail the requirement. | **CONFIRMED** |
| **MC-10** | `src-tauri/src/commands/github_mcp_commands.rs:14-19` | **MEDIUM** | Config schema is `{enabled, org, repos, server_url}` — a single global record under one key (`:23`) | Cannot represent N servers, a server name, a stdio command + args, env vars, or headers. The stated goal ("user adds their own MCP server") needs a list of heterogeneous server definitions. | Any real implementation discards this schema and migrates the table. The existing config is not a head start. | **CONFIRMED** |
| **MC-11** | `src-tauri/src/mcp_server/health_monitor.rs:3-5` | **LOW** | Header comment: "handles user-configured *external* MCP servers only" | Reinforces the false belief that this file is a client. It monitors *endpoints*, and the only endpoint it ever gets is one hardcoded name from `main.rs:257/261`. | Future reader budgets "MCP client: mostly done, 400 LOC exists". | **CONFIRMED** |
| — | `health_monitor.rs` IN-38/39/40/41/42/43/44/45 | — | Timeout defeated by `.unwrap_or_default()`; `Disconnected` state dead end; hardcoded `"attempt": 1`; dead `app` param; serial pings vs fixed interval; stdio healthy-by-assumption; `starts_with("http")` transport sniff; in-process server unmonitored | Already documented | See `_grounding/G3c-bugs-updater-mcp-vault-state-analytics-obs.md:57-64` | **CONFIRMED (G3c)** |

**Note on G3c:** those eight defects are all real, and all in `health_monitor.rs`. In light of this audit they are **defects in a file that should probably not survive in its current form** — fixing them makes a `/health` pinger correct, which is not the capability the owner wants. See §7.

---

## 7. The smallest honest scope to make it real

The capability the owner described — *"a user adds their own MCP server and its tools become available to agents"* — requires the following. Nothing here is optional; each item is a break in the §3 trace.

**A. Protocol (does not exist — ~0 reusable LOC)**
1. `Cargo.toml:50` — add `rmcp` features `client` + `transport-streamable-http-client` and, for stdio servers, `transport-child-process`. Server features can be dropped in the same edit.
2. New `mcp_client/connection.rs`: open transport, run `initialize` (protocol version + capabilities), hold the session, `tools/list`, `tools/call`, shutdown. One `McpConnection` per configured server, in a `HashMap<String, McpConnection>` behind an `RwLock`.
3. Reconnect on transport error, backed by real MCP state — not by a `/health` GET.

**B. Dynamic tool registration (the structural blocker — MC-03)**
4. Make `ToolRegistry` mutable at runtime: change `tools: HashMap<…>` (`tool_registry.rs:10`) to `RwLock<HashMap<…>>` (or `DashMap`), and `register`/`unregister` to `&self`.
5. Re-plumb `AgentEngine` (`engine.rs:59-61`) to take a shared `Arc<ToolRegistry>` rather than owning a private one, so a connection established after engine construction is visible to it.
6. New `McpProxyTool` implementing the existing `Tool` trait: `name()` returns a namespaced id (e.g. `github__create_pr`) to prevent collisions with built-ins; `input_schema()` returns the server's advertised schema verbatim; `execute()` forwards to `tools/call` on the owning connection. This is the single adapter that makes the whole thing work.

**C. Configuration (existing schema is not reusable — MC-10)**
7. Replace the single `github_mcp_config` key with a proper table: `id, name, enabled, transport ('http'|'stdio'), url, command, args, env, headers`. Migration from the one existing key is trivial (one row or none).
8. Commands: `list_mcp_servers`, `add_mcp_server`, `update_mcp_server`, `remove_mcp_server`, `test_mcp_connection`. `remove_*` must actually call `unregister_server` — fixing MC-06 by construction.
9. Credentials go to the OS keychain via the existing `vault_commands`, referenced by key from the server row — never inline in `config.db`.

**D. Safety (new — no precedent in the codebase)**
10. Third-party tools execute arbitrary remote code on the user's behalf. They must route through the existing permission-mode gate, and should default to per-tool approval on first use. There is no existing pattern for "untrusted tool" — this is a Rule-4 architectural decision, not an implementation detail.

**E. UI**
11. Replace `GitHubMCPCard` (`Settings.tsx:578-660`) with a server **list** + add/edit form (name, transport, URL or command+args, env/headers, enable toggle) and a per-server discovered-tools list with counts.
12. Rename the "MCP Tools" section (`Settings.tsx:54`) to something like "Built-in Tools" to end the MC-07 inversion.
13. Re-parent `McpHealthPanel` out of `components/sidecar/` and split the `mcp:*` listeners out of `sidecarStore.ts` into `mcpStore.ts` — required anyway by the no-sidecar-line rule (MC-09).

**F. What to do with the 484 surviving LOC**
- `health_monitor.rs` (400): **do not port as-is.** Its ping model (`/health` GET, 404-is-success) is wrong for MCP (MC-04) and carries eight known defects (G3c IN-38…IN-45). The correct health signal for an MCP server is "the session is alive and `tools/list` succeeds", which only exists once (A) is built. Keep the `McpServerHealth` shape, the event names (`events.rs:74-79`), and the frontend listeners — they are a usable contract. Rewrite the probe body against the real connection. Realistically ~150 LOC survives, ~250 is replaced.
- `github_mcp_commands.rs` (84): superseded by (C.7-8). Keep the `open_config_db` / `app_settings` pattern (`:25-42`); discard `GitHubMcpConfig`.

**Honest sizing.** Items A + B are the feature; C + D + E are what make it usable and safe. This is a **new subsystem**, not a repair. The existing 484 LOC contributes a config-storage idiom, an event contract, and a health-badge UI — call it 15-20% of the surface, and 0% of the protocol. Item B additionally touches `agent/engine.rs` and `agent/tool_registry.rs`, which are core-path files shared by every agent in the app; that is a Rule-4 "stop and ask" change, not a drop-in.

**Recommendation for the vision doc.** The line "we keep the MCP client for future extension" is not currently true of anything in the repo. The accurate statement is: *"AutoDev has no MCP client. It has an MCP server (being removed), an HTTP health-pinger, and a GitHub settings form that stores config nothing reads. Letting users add their own MCP servers is a greenfield feature whose hardest prerequisite — runtime tool registration — does not exist and requires changing `ToolRegistry` and `AgentEngine`."* Anything softer will cause the work to be under-scoped later.

---

## Appendix — raw evidence log

### E1 — `rmcp` crate is compiled SERVER-ONLY
`src-tauri/Cargo.toml:50`
```
rmcp = { version = "0.16", features = ["server", "transport-streamable-http-server"] }
```
No `client` feature, no `transport-streamable-http-client`, no `transport-child-process`. CONFIRMED.

### E2 — The only `tools/list` / `tools/call` implementation is SERVER-side
`src-tauri/src/mcp_server/tool_adapter.rs:126` (`async fn list_tools`) and `:165` implement rmcp's `ServerHandler`. Repo-wide grep for `ClientHandler`, `serve_client`, `rmcp::service`, `McpClient`, `mcp_client`, `connect_mcp` over `src-tauri/src/**/*.rs`: zero hits outside server code. Other `tools/list` matches are in `recipes/defaults.rs` (recipe text templates). CONFIRMED.

### E3 — `list_mcp_tools` is NOT a client discovery call
`src-tauri/src/commands/sidecar_commands.rs:313-325` iterates `registry.all()` — AutoDev's own `ToolRegistry`. Docstring `:308-310`: queries "directly from the in-process ToolRegistry to avoid a JSON-RPC round-trip", for inspection "without connecting an external MCP client". CONFIRMED.

### E4 — `health_monitor.rs` performs a plain HTTP GET, not an MCP connection
`src-tauri/src/mcp_server/health_monitor.rs:190-209`: `format!("{}/health", endpoint)` then `http_client.get(&url).send()`. No handshake, no JSON-RPC, no session id, no protocol version. 404 explicitly treated as success (`:199`). CONFIRMED.

### E5 — `github_mcp_commands.rs` is a config store only
84 lines. `get_github_mcp_config` (`:49`), `set_github_mcp_config` (`:67`). Read/write one JSON blob under key `github_mcp_config` in `app_settings` (`:25-42`). No network, no spawn. CONFIRMED.

### E6 — the only `register_server` call site
`src-tauri/src/main.rs:250-263`. Sole call site. `unregister_server` (`health_monitor.rs:111`) has zero call sites. CONFIRMED.

### E7 — nothing spawns the GitHub MCP process
Grep for `server-github` and `modelcontextprotocol` over `src-tauri/src/**/*.rs`: zero hits. All `npx` hits are Playwright (`commands/browser_commands.rs:32,49,78,100`) or recipe/skill prompt text (`recipes/defaults.rs`, `skills/defaults.rs`). CONFIRMED.

### E8 — `ToolRegistry` is frozen after construction
`agent/tool_registry.rs:10` `tools: HashMap<String, Arc<dyn Tool>>` (bare, no lock); `:18` `register(&mut self, …)`. Construction sites: `agent/engine.rs:59-61` (moved into struct), `main.rs:108-110` (`Arc::new`), `mcp_server/server.rs:150-152` (`Arc::new`). `commands/graph_commands.rs:604` `registry.register(...)` is a **`RepoRegistry`** (`:604` `RepoRegistry::load()`), unrelated. CONFIRMED.

### E9 — `port_status.rs` is server code
`src-tauri/src/mcp_server/port_status.rs:17` `use super::server::{MCP_SERVER_PORT, current_mcp_port};`. Struct doc `:19-21`: "the MCP server's port binding state … emitted via `mcp:port-conflict`". Fields `preferred_port` "always 8767 in the current build" (`:24`), fallbacks 8768/8769/8770 (`:27`). CONFIRMED.

### E10 — sidecar MCP registration
`src-tauri/src/sidecar/client.rs:1262` `register_mcp_server`; `:1271` `POST {base_url}/api/mcp/servers`; `:1274-1287` body `{"id":"autodev-mcp-server","transport":"http","http":{"url":"http://127.0.0.1:{port}/mcp"}}`; 409 → `PUT /api/mcp/servers/autodev-mcp-server` (`:1331`). Sole call site `main.rs:406`. Also `client.rs:1388` `mcp_server_healthy` TCP probe, used by `recipes/runner.rs:303-330`. All sidecar. CONFIRMED.

### E11 — frontend event listeners are live
`src/stores/sidecarStore.ts:203, 212, 227, 244, 261, 285, 309` listen to all 7 `mcp:*` events declared at `src-tauri/src/events.rs:74-79, 84`. `McpHealthPanel` mounted at `src/components/sidecar/SidecarPanel.tsx:480`; `McpToolBrowser` at `src/tabs/Settings.tsx:55`. No orphan events (matches G3c). CONFIRMED.
