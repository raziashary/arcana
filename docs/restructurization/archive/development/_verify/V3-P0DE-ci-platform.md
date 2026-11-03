# V3-P0DE — CI & Platform Claims: Adversarial Verification

**Stance:** refute-by-default. **Read-only pass** — no cargo, no npm, no builds, no servers run.
**Repo:** `C:\AIRS Foundry\autodev\autodev` (Tauri v2 desktop app; Cargo workspace `src-tauri` / `src-cli` / `src-shared`; Vite + React + TS frontend in `src/`).
**Pass date:** 2026-08-31

> Everything below is evidenced by quoted source at a real `path:line`. Nothing was inferred from prose or docstrings.
>
> **One retraction is recorded in this report.** An earlier draft of §3 concluded from a doc comment that the orphaned graph modules were excluded deliberately. That was wrong and is struck in place, with the mechanism of the error and the contradicting evidence, under "⚠️ RETRACTION" in §3. The C6 *verdict* is unaffected; only the recommended response to it changed.

---

## 1. Verdict table

| # | Claim (abridged) | Verdict |
|---|---|---|
| **C1** | 4× `continue-on-error: true` on `cargo test` + `cargo fmt` in `rust-checks` and `cli-checks`; Rust test failures cannot fail CI | **CONFIRMED** |
| **C2** | `lint` === `typecheck` === `tsc --noEmit`; no ESLint anywhere; CI's "ESLint" step typechecks twice | **CONFIRMED** |
| **C3** | No frontend test framework at all; no `test` script | **CONFIRMED** |
| **C4** | `ci.yml` has no macOS job | **CONFIRMED as to `ci.yml`** — but the implied "no macOS coverage" is **REFUTED** (see detail) |
| **C5** | Both build smokes gated to push-on-main; a PR can merge without either platform being compiled | **PARTIALLY CONFIRMED / OVERSTATED** — gating confirmed, "not compiled" is false |
| **C6** | Exactly three unreachable `.rs` files under `src-tauri/src/` | **CONFIRMED — exactly three, exhaustively verified across all 3 crates** |
| **C7** | `additionalBrowserArgs` sets `--use-angle=d3d11` unconditionally; d3d11 is Windows-only | **CONFIRMED (literally) / OVERSTATED (impact)** |
| **C8** | `bundle.targets: ["nsis"]` despite full linux + macOS blocks | **CONFIRMED (literally) / REFUTED (implication)** — every release path overrides it |
| **C9** | `plugins.dialog` / `plugins.fs` declared but crates absent + unregistered; "dead config" | **CONFIRMED and UNDERSTATED** — it is not dead, it is a **live runtime break** |
| **C10** | `dump.rdb` is a committed Redis snapshot; nothing in Rust reads Redis; deleting it is safe | **PARTIALLY CONFIRMED** — facts hold, "deleting is safe" is **not established** |

**Score: 4 clean CONFIRMED, 1 confirmed-but-understated, 5 requiring qualification.**

---

## 2. Per-claim detail

### C1 — CONFIRMED

Four `continue-on-error: true`, exactly where claimed. `.github/workflows/ci.yml`:

```yaml
53:      - name: Format check
56:        continue-on-error: true
57:        run: cargo fmt --manifest-path src-tauri/Cargo.toml -- --check

65:      - name: Tests
68:        continue-on-error: true
69:        run: |
70:          cargo test \
71:            --manifest-path src-tauri/Cargo.toml \
72:            --lib \
73:            -- --test-threads=4

123:      - name: Format check
126:        continue-on-error: true
127:        run: cargo fmt --manifest-path src-cli/Cargo.toml -- --check

135:      - name: Tests
138:        continue-on-error: true
139:        run: |
140:          cargo test \
141:            --manifest-path src-cli/Cargo.toml \
142:            -- --test-threads=4
```

`grep -c "continue-on-error" ci.yml` = 4. No fifth. **A failing Rust test cannot fail CI.**

**Two things the claim misses, both load-bearing for the phase:**

1. **`--lib` scoping (ci.yml:72).** The `rust-checks` test step runs the **lib target only**. Integration tests in a `src-tauri/tests/` dir would not run even after the flag is flipped. (No such dir exists today — §5 — so this is a forward-looking trap, not a present gap.) `cli-checks` has no `--lib`, so it is unscoped.
2. **Test *compilation* is already gated.** `cargo clippy --all-targets` (ci.yml:59-63, 129-133) has **no** `continue-on-error`, and `--all-targets` includes test targets. So today a test that *fails to compile* already breaks CI; only a test that compiles and then *fails* is swallowed. Removing `continue-on-error` widens the gate from "compiles" to "passes" — a smaller delta than the claim implies.
3. Clippy runs **without** `-D warnings` (ci.yml:61-63), and there is no `#![deny(warnings)]` in `src-tauri/src/lib.rs` and no `[lints]` table in any `Cargo.toml`. Clippy findings are advisory.

### C2 — CONFIRMED

`package.json:9-10`:
```json
"typecheck": "tsc --noEmit",
"lint": "tsc --noEmit"
```
Byte-identical. No `eslint` entry in `package.json` dependencies or devDependencies. No `.eslintrc*`, no `eslint.config.*` anywhere in the repo. `ci.yml:93-97` therefore runs `tsc --noEmit` twice under two different step names:
```yaml
93:      - name: TypeScript check
94:        run: npm run typecheck
96:      - name: ESLint
97:        run: npm run lint
```
The job is named "Frontend lint & typecheck" (ci.yml:78) — the name is false advertising. **Zero lint coverage on the frontend, and wasted CI minutes.**

### C3 — CONFIRMED

No `vitest`, `jest`, `playwright`, `@testing-library/*` in `package.json`. No `test` script (`package.json:5-11` has exactly `dev`, `build`, `tauri`, `typecheck`, `lint`). No `vitest.config.*` / `jest.config.*` / `playwright.config.*` at the repo root. `find src -name "*.test.*" -o -name "*.spec.*"` returns nothing. **Zero frontend test files, zero frontend test infrastructure.**

### C4 — CONFIRMED as to `ci.yml`; the broader implication is REFUTED

`ci.yml` runners are, exhaustively: `ubuntu-22.04` (:26), `ubuntu-latest` (:79), `ubuntu-22.04` (:103), `windows-latest` (:149), `ubuntu-22.04` (:180). **No macOS, no matrix.** Confirmed.

**But macOS coverage exists in the other workflow.** `.github/workflows/build-release.yml`:
```yaml
161:  build-macos:
162:    name: Build macOS DMG
164:    runs-on: macos-14   # Apple Silicon runner
174:          targets: aarch64-apple-darwin,x86_64-apple-darwin
```
plus `build-windows` (`windows-latest`, MSI, :102-104) and `build-linux` (`ubuntu-22.04`, AppImage+deb, :242-244), all gating `publish` (`needs: [metadata, build-windows, build-macos, build-linux]`, :298). So macOS is built — **on release only, never on a PR or a push**. State the gap precisely: *macOS breakage is discovered at release time, not at merge time.* That is a real gap, but it is not "no macOS job".

### C5 — PARTIALLY CONFIRMED / OVERSTATED

The gating is exactly as claimed — `ci.yml:150` and `ci.yml:181`, identical string:
```yaml
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
```
On a `pull_request` event both smokes are skipped. Confirmed.

**"Without either platform being compiled" is false.** `rust-checks` and `cli-checks` run on every PR, and both run `cargo clippy --all-targets` with no `continue-on-error` — that is a full type-check and borrow-check of every target in both crates on Linux. What a PR does *not* get is: (a) the Windows MSVC toolchain, (b) the frontend `vite build`, (c) linking an actual binary, (d) `tauri build`'s config validation and bundling. A Windows-only `#[cfg]` break, a broken `tauri.conf.json`, or a Vite build failure would sail through — but "not compiled" overstates it.

Also note the `needs:` on both smokes (`[rust-checks, frontend-checks]`, :148/:179) is redundant given the `if:` — they can only run on a main push anyway.

**Undetermined:** whether GitHub branch protection makes any check required. Not visible from the repo. See §6.

### C6 — CONFIRMED. Exactly three. See §3 for the exhaustive walk.

### C7 — CONFIRMED literally; OVERSTATED as a portability defect

`src-tauri/tauri.conf.json:24`:
```json
"additionalBrowserArgs": "--enable-gpu --use-angle=d3d11 --enable-features=Vulkan"
```
Inside `app.windows[0]`, with no platform conditional anywhere in the file. `d3d11` is Direct3D 11 — Windows-only. Confirmed.

**Qualification:** `additionalBrowserArgs` is a **WebView2-only** option in Tauri v2. WebView2 is itself Windows-only; on Linux (WebKitGTK) and macOS (WKWebView) the field is inert, not harmful. So this is dead config off-Windows rather than a build or runtime breaker. **The genuinely odd part the claim misses:** the same string asks for `--use-angle=d3d11` *and* `--enable-features=Vulkan` — two different graphics backends requested simultaneously. That is worth a look on its own merits.

### C8 — CONFIRMED literally; the implication is REFUTED

`src-tauri/tauri.conf.json:33` — `"targets": ["nsis"]`. And the other blocks do exist in the same file:
- `bundle.linux` — `deb` (:49-53, with `depends`) and `appimage` (:54-56) — lines 48-57.
- `bundle.macOS` — entitlements, hardenedRuntime, minimumSystemVersion, full `dmg` window geometry — lines 66-90.
- `bundle.icon` even includes `icons/icon.icns` (:38), the macOS icon.

**But no release path reads `targets`.** Every packaging path passes its own bundle selection:
- Linux — `scripts/build-linux.sh:60-63`:
  ```sh
  [[ "$BUILD_APPIMAGE" == "true" ]] && BUNDLES="${BUNDLES:+$BUNDLES,}appimage"
  [[ "$BUILD_DEB"      == "true" ]] && BUNDLES="${BUNDLES:+$BUNDLES,}deb"
  npm run tauri -- build --bundles "$BUNDLES"
  ```
  `--bundles` overrides `bundle.targets` outright.
- Windows — `build-release.yml:141-145` calls `scripts/build-windows.ps1`, a wrapper around `installers/wix/build-msi.ps1`, producing an **MSI via WiX**. The `nsis` target is not used by the Windows release path either.
- macOS — `scripts/build-macos.sh:31` calls `installers/dmg/create-dmg.sh`.

So `targets: ["nsis"]` governs only a bare local `tauri build`. It is a misleading default worth fixing, but it does **not** make the app Windows-only, and the linux/macOS blocks are **not** orphaned — `build-linux.sh` genuinely consumes `bundle.linux.deb.depends`, and the DMG path consumes `bundle.macOS`.

### C9 — CONFIRMED, and materially UNDERSTATED

Every part of the claim holds:
- `tauri.conf.json:110-116` declares `"dialog": {}` (:114) and `"fs": {}` (:115).
- `src-tauri/Cargo.toml:19-22` — the only Tauri plugin dependency is `tauri-plugin-shell = "2"` (:22). No `tauri-plugin-dialog`, no `tauri-plugin-fs`.
- `src-tauri/src/main.rs:175-176` — the builder chain registers exactly one plugin:
  ```rust
  tauri::Builder::default()
      .plugin(tauri_plugin_shell::init())
  ```
- `src-tauri/capabilities/default.json` grants `core:*` and `shell:allow-open` only — **no `dialog:*`, no `fs:*` permission**.

**"They are dead config" is wrong, and this is the most consequential correction in this pass.** The frontend imports and *calls* both plugins:

`src/tabs/Logs.tsx:13-14`
```ts
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'
```
with four live call sites — `save({...})` at `Logs.tsx:311` and `:350`, `writeTextFile(filePath, ...)` at `:319` and `:358`. Both JS packages are real dependencies (`package.json:18-19`), so this **typechecks and builds clean** and CI stays green. It fails only at runtime, when a user clicks export in the Logs tab: the JS shim invokes a plugin command that no Rust-side plugin registered and that no capability permits.

This is a textbook *inert-but-green* defect: the config is not dead, it is a **broken feature that CI cannot see** — precisely because there is no frontend test (C3) and no PR-time build smoke (C5). The fix is not "delete the config"; it is add `tauri-plugin-dialog` + `tauri-plugin-fs` to `Cargo.toml`, `.plugin(...)` both in `main.rs`, and add the `dialog:`/`fs:` permissions to `capabilities/default.json` — **or** remove the Logs.tsx export feature. That is a decision, not a cleanup.

### C10 — PARTIALLY CONFIRMED; the safety conclusion is not established

Confirmed facts:
- `src-tauri/dump.rdb` exists, **175,217 bytes**, and is **tracked by git** (`git ls-files` lists it). It is the only `*.rdb` in the repo. `.gitignore` does not mention `rdb` or `dump`.
- It is a genuine Redis snapshot — first 16 bytes are `52 45 44 49 53 30 30 31 31` → `REDIS0011` (RDB format v11).
- **No `redis` crate in any `Cargo.toml`, and none in `Cargo.lock`** (`grep '^name = "redis"' Cargo.lock` → no match). Confirmed.

**Where the claim goes wrong:**

1. **Rust is not uninvolved with Redis.** `src-tauri/src/sidecar/mod.rs` has `redis_process: Mutex<Option<Child>>` (:423) and `redis_status` (:424), probes liveness, and on Windows **starts a Redis-compatible server**: `try_start_memurai_service()` (:760) runs `net start Memurai` / `sc start Memurai` (:767-768) and falls back to spawning `C:\Program Files\Memurai\memurai.exe` (:804, :823). The header comment at :417-423 says AutoDev "does NOT spawn or stop Redis" and then, four lines later, documents the spawn fallback — **the comment contradicts the code it sits on.** A child Redis inheriting the app's cwd (`src-tauri/` under `tauri dev`) writing its RDB to cwd is the most likely provenance of this exact file.
2. **A real Redis client exists in-tree.** The `core-orchestrator` Python sidecar uses Redis throughout — `api_gateway/app.py`, `modules/agents/service.py`, `modules/mcp/router.py`, `modules/mcp/transaction.py`, `system/optimized_stores.py`, `check_redis.ps1`, `config/memory_config.yaml` — and `.env:82` sets `REDIS_URL=redis://127.0.0.1:6379/0`. "Nothing reads Redis" is true only if narrowed to *Rust*.
3. **The file holds agent memory, not scratch.** Printable runs inside it include `procedural:supervisor`, `[MEMORY_TYPE: memory:procedural]`, supervisor tick-protocol prompt text, task UUIDs, and timestamps around `2026-05-12T08:22:19`. It is a snapshot of the agent procedural-memory / warm tier.

**Net:** deleting it is *probably* harmless — `sidecar/mod.rs:419-420` documents Redis as OPTIONAL, its absence disabling "the warm memory tier and agent card store" without blocking startup — but that is a different argument than the one offered, and the evidence given ("no redis crate in Rust") does not support the conclusion. Under the standing live-data rule this is user-decision territory: a 175 KB committed blob of agent memory. **Recommendation:** do not delete silently. Add `dump.rdb` to `.gitignore` and `git rm --cached` it (removing it from the working tree is a separate call the user should make), and fix the cwd so a spawned Memurai stops dropping RDBs into `src-tauri/`.

---

## 3. The complete unreachable-file list, per crate

**Method.** A transitive `mod`-graph walk from each crate's roots, not a grep. Comments (line and block) were stripped before parsing, so a commented-out `mod x;` correctly does **not** count as a declaration. `#[path = "..."]` attributes and inline `mod x { ... }` blocks were handled. Child resolution tried both `dir/x.rs` and `dir/x/mod.rs`. Roots were taken from the manifests:
- `src-tauri/Cargo.toml:8-14` — `[lib] name = "autodev_lib"` (default path `src/lib.rs`) and `[[bin]] path = "src/main.rs"`. Both walked.
- `src-cli/Cargo.toml:8-10` — `[[bin]] path = "src/main.rs"`; `src/lib.rs` also walked (cargo auto-detects it).
- `src-shared/Cargo.toml` — no `[lib]`/`[[bin]]` sections; default `src/lib.rs` walked.

No `include!` macro and no `#[path]` attribute exists anywhere in the three crates (verified by grep), so file inclusion happens only through `mod`.

### Results

| Crate | Total `.rs` under `src/` | Reachable | **Unreachable** |
|---|---|---|---|
| `src-tauri` | 237 | 234 | **3** |
| `src-cli` | 14 | 14 | **0** |
| `src-shared` | 8 | 8 | **0** |
| **Total** | **259** | **256** | **3** |

**The complete unreachable list — `src-tauri` (3):**

1. `C:\AIRS Foundry\autodev\autodev\src-tauri\src\agent\builtin_tools\graph_tools.rs` (840 lines)
2. `C:\AIRS Foundry\autodev\autodev\src-tauri\src\agent\builtin_tools\graph_prompts.rs` (78 lines)
3. `C:\AIRS Foundry\autodev\autodev\src-tauri\src\db\checkpoints.rs` (280 lines)

**`src-cli` — none. `src-shared` — none.**

Also verified: **zero** "declared but file missing" edges in any crate — every `mod X;` in all three crates resolves to a real file. And `src-tauri/build.rs` is the only `.rs` outside a `src/` tree; it is a build script, correctly not part of the module graph.

### Corroborating evidence for each of the three

`src-tauri/src/agent/builtin_tools/mod.rs` declares 42 sibling modules (`pub mod read_file;` :6 … `pub mod acknowledge_worker_message;` :72). Neither `graph_tools` nor `graph_prompts` appears.

`src-tauri/src/db/mod.rs:10-15` declares exactly six:
```rust
pub mod tasks;
pub mod autodev_instructions;
pub mod model_configs;
pub mod plans;
pub mod workers;
pub mod annotations;
```
`checkpoints` is absent.

A raw ripgrep for `mod\s+(graph_tools|graph_prompts|checkpoints)` across every `.rs` file in the repo returns **no matches at all** — so there is not even a commented-out declaration to un-comment. These were never wired, not un-wired.

#### ⚠️ RETRACTION — correcting an earlier statement in this same report

An earlier draft of this section concluded, from `src-tauri/src/mcp_server/server.rs:144`, that the graph pair's exclusion "looks deliberate" and should be treated as "a decision to reopen, not an oversight to correct." **That was wrong, and I am striking it.** The mechanism of the error: I read a doc comment and inferred intent from it without checking the surrounding scope or the plan document — exactly the "read past the signature / a docstring is not evidence" failure.

The full context of that comment shows it is scoped to a **different concern**. `server.rs:126-144` is a list headed `NOT exposed (intentional):` enumerating tools deliberately withheld from `register_for_mcp_worker` — the *fleet-worker* registry — alongside `create_epic_tool`, `search_memory`, and `validate`, all of which **are** declared and compiled. The graph entry reads "PA-only analysis", which asserts the graph tools *should* be available to the PA. It explains why fleet workers don't get them. It says nothing about compilation, and it is not a decision to leave 918 lines out of the binary.

**The actual evidence points the opposite way — this is a false completion.** `docs/development/DEVELOPMENT_PLAN.md` (§6.12) carries:
```
- [x] **Register all graph tools in `src-tauri/src/agent/builtin_tools/mod.rs`** (COMPLETE):
  - Add `pub mod graph_tools;`
  - In `ToolRegistry::new()`: register all tools from `graph_tools.rs` alongside existing tools
  ...
- [x] **Add 5 MCP prompts** in a new `src-tauri/src/agent/builtin_tools/graph_prompts.rs` (COMPLETE):
...
**6.12 Success Criteria:** ✅ COMPLETE
> Worker agent recipe receives `tools/list` from `autodev-mcp-server` → sees `get_graph_minimal_context` + all 22 graph tools → …
```
The literal instruction "Add `pub mod graph_tools;`" is checked off as **COMPLETE**, and the success criterion asserting workers see all 22 graph tools is marked ✅. **None of it happened**: no declaration, no registration, and the file does not compile (§4c). The plan document is stale and asserts a capability the binary does not have. P1 should treat this as an oversight to correct *and* fix the plan's checkboxes — not as a design decision to reopen.

**Two of the three were already known; one is new.** `docs/restructurization/vision/_grounding/G1-rust-architecture-map.md:56` and `G2a-bugs-agent-engine.md:47` (ticket **T32**, CRITICAL, marked CONFIRMED) both already document `graph_tools.rs` + `graph_prompts.rs` as undeclared — "918 LOC of dead files". Neither grounding doc identifies **`db/checkpoints.rs`** as undeclared; G1 in fact treats it as *live* (see §4b). That third file is a new finding relative to the existing grounding set.

### Guard-test warning

If the P1 reachability guard is written as "every `.rs` under `src-tauri/src/` must be declared", it must **exclude `src-tauri/build.rs`** (outside `src/`, so fine) and must not be extended to `src-cli`/`src-shared` expecting failures — both are already clean, so the guard passes there vacuously today and will only ever fire on regressions. The guard must also strip comments before matching, or a future commented-out `mod` will read as a false pass.

---

## 4. Compilation-blocker assessment for the three undeclared files

### 4a. `graph_prompts.rs` — **COMPILES CLEAN. Genuinely a one-line change.**

78 lines. Sole import is `use serde_json::json;` (:3). Six `pub fn`s returning `serde_json::Value` built entirely from `json!` string literals. **Zero references to any project type, function, or trait.** `serde_json` is a real dependency (`src-tauri/Cargo.toml:26`). Adding `pub mod graph_prompts;` compiles with no errors and no warnings (all items are `pub` in a lib crate, so no `dead_code`).

### 4b. `db/checkpoints.rs` — **COMPILES CLEAN (one clippy warning). One-line change.**

280 lines. Imports at :7-10:
```rust
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use crate::errors::{AutoDevError, Result};
use super::autodev_dir;
```
All four resolve:
- `rusqlite = { version = "0.31", features = ["bundled"] }` — `src-tauri/Cargo.toml:41`.
- `serde` with `derive` — `Cargo.toml:25`.
- `AutoDevError::Agent(String)` — `src-tauri/src/errors.rs:19`; `Result` alias at `errors.rs:62`.
- `super::autodev_dir` — `src-tauri/src/db/mod.rs:21`, `pub fn autodev_dir() -> Result<PathBuf>`. Correct parent, correct signature.

`?` on every `rusqlite` call works: `errors.rs:47` has `Database(#[from] rusqlite::Error)`. `params![...]` receives `String`/`i64` only — all `ToSql`. `row_to_checkpoint` (:254) is a plain `fn(&Row) -> rusqlite::Result<FileCheckpoint>`, correctly shaped for both `query_row` and `query_map`. Its `#[cfg(test)] mod tests` (:265-279) uses only `OperationType`, which derives `PartialEq` (:14) — the `assert_eq!`s compile.

**One warning only:** the inherent `pub fn from_str(s: &str) -> Self` (:31) trips `clippy::should_implement_trait`. Warn-level; clippy runs without `-D warnings` (ci.yml:61-63) and there is no `deny(warnings)` in `lib.rs`, so it does not fail anything.

**Caveat 1 — test count.** Declaring this file adds **1 new test** (`test_operation_type_conversion`, :269). Pure-logic, no I/O, will pass. But "declare the modules" and "make `cargo test` blocking" interact — sequence the declare *before* the flag flip so the count is known.

**Caveat 2 — declaring this is a DESIGN DECISION, not a cleanup. Do not do it reflexively.** `db/checkpoints.rs` is a dead **duplicate of a live, shipped feature**:

| | dead | **live** |
|---|---|---|
| file | `src-tauri/src/db/checkpoints.rs` | `src-tauri/src/git/checkpoint.rs` |
| declared? | **no** | **yes** — `src-tauri/src/git/mod.rs:31` |
| table | `file_checkpoints` (:80) | `checkpoints` (:33) |
| database | `~/.autodev/checkpoints.db` (:67) | `~/.autodev/config.db` |
| reached by | nothing | `create_checkpoint`/`restore_checkpoint`/`list_checkpoints`/`delete_checkpoint`/`compare_checkpoints` Tauri commands (`git/git_commands.rs:176-196`), plus `auto_checkpoint_before_write()` (`git/checkpoint.rs:245`) |

Declaring the dead one would stand up a **second checkpoint implementation and a second database file** competing with the shipped one. That needs the user's judgment, not a one-line commit.

**This also resolves an open question in an existing repo doc — and shows that doc is wrong.** `G1-rust-architecture-map.md:540` flags `file_checkpoints`/`checkpoints` as "two tables, similar names, different files — **UNVERIFIED** whether this is intentional". Resolved: there is only **one** live implementation, because `db/checkpoints.rs` is never compiled. Consequently `G1:89`, `G1:527` (which lists `~/.autodev/checkpoints.db` as database **#9**, owner `db/checkpoints.rs:67`) and `G1:540` are **factually wrong** — that database cannot exist, because no compiled code opens it. Whoever owns the grounding set should correct those three rows.

### 4c. `graph_tools.rs` — **DOES NOT COMPILE. 2 distinct hard errors (6 diagnostics). NOT a one-line change.**

Verified independently at source, not taken on report.

**Error 1 — `Option<GraphNode>` treated as `GraphNode`. E0609 ×5.**

`src-tauri/src/agent/builtin_tools/graph_tools.rs:272`:
```rust
let node = store.get_node(&qualified_name)?;
```
`src-tauri/src/graph/store.rs:307`:
```rust
pub fn get_node(&self, qualified_name: &str) -> Result<Option<GraphNode>> {
```
So after `?`, `node: Option<GraphNode>`. Then `graph_tools.rs:296-300` dereferences it as a struct:
```rust
"qualified_name": node.qualified_name,
"name": node.name,
"kind": node.kind,
"file": node.file_path,
"line": node.line_start
```
Five `no field ... on type Option<GraphNode>` errors. Fix: `.ok_or_else(|| AutoDevError::Agent(...))?` or `if let Some(node)`.

**Error 2 — nonexistent field `confidence_tier` on `SuggestedQuestion`. E0609.**

`graph_tools.rs:696`:
```rust
"confidence": q.confidence_tier
```
`src-tauri/src/graph/analyze.rs:31-35`:
```rust
pub struct SuggestedQuestion {
    pub question_type: String,
    pub question: String,
    pub why: String,
}
```
No such field. (`confidence_tier` exists on `GraphEdge`, `store.rs:36-48` — the author mixed up the types.) Fix: use `q.why` or drop the key.

**Everything else in the file checks out**, which is why the two errors are the whole bill:
- All 8 imported paths exist and are `pub`: `query_by_name`/`query_dependents`/`query_dependencies` (`graph/query.rs:32,81,144`), `god_nodes`/`surprising_connections`/`suggest_questions`/`get_impact_radius`/`graph_diff` (`graph/analyze.rs:80,120,201,278,456`), `detect_entry_points` (`graph/flows.rs:34`), `detect_changes` (`graph/changes.rs:134`), `GraphStore` (`graph/store.rs`).
- All call-site arities match the definitions (checked each of the 13 call sites individually).
- Every `GraphStore` method called exists with matching arity: `get_all_nodes_for_graph(i64,i64)` :513, `get_all_edges_for_graph(i64,i64)` :551, `get_edges_for_node(&str)` :344, `get_node(&str)` :307.
- Every field access on `GodNode`, `SurprisingConnection`, `ImpactRadius`, `GraphDiff`, `ChangeAnalysis`, `QueryResult`, `GraphNode`, `GraphEdge` matches its struct — except Error 2.
- `GraphNode`/`GraphEdge` both derive `Clone` + `Serialize`, so the `.clone()` calls and `json!` uses are fine.

**Warnings, not errors:** `graph_tools.rs:8` `use crate::graph::flows::detect_entry_points;` is **unused** — that name appears nowhere else in the file. Plus `clippy::map_clone` at :368 and several `partial_cmp(..).unwrap()` (:44, :243, :369, :610-614) that panic on NaN at runtime.

**Effort — to make it COMPILE:** roughly **a one-line declaration plus a ~4-line repair** — unwrap the Option at :272, swap `q.confidence_tier` → `q.why` at :696, delete the unused import at :8. Not a day.

**Effort — to make it WORK: much larger, and out of scope for a P1 declaration pass.** The 22 tool structs are `pub struct X;` with *inherent* `name()`/`execute()` associated functions. They do **not** implement the `Tool` trait, and nothing registers them in `ToolRegistry`. So declaring the module gets you 840 compiled-but-unreachable lines and a green build — the agent still has zero graph access. `DEVELOPMENT_PLAN.md` §6.12 already claims this registration is `[x] COMPLETE` (see §3), so the plan will not tell you the work is outstanding. **Declaring the module must not be recorded as closing T32** (`G2a-bugs-agent-engine.md:47`), whose stated impact is "The agent has **no** graph access at all" — that remains true after the declaration. Closing T32 requires trait impls plus registration, which is a separate task.

---

## 5. Existing test inventory

**Rust — 34 test functions in 11 files, all in `src-tauri`. `src-cli` and `src-shared` have zero.**

No `tests/` directory exists in any of the three crates (the two `tests/` dirs found under `src-tauri/resources/sidecars/{core-orchestrator,memory-mcp-server}/` are **Python sidecar** trees, not Rust integration tests — they are vendored resources, not part of any cargo target). No `benches/`.

| File (`src-tauri/src/…`) | `#[test]` / `#[tokio::test]` |
|---|---|
| `agent/builtin_tools/path_jail.rs` | 7 |
| `agent/builtin_tools/build_evidence.rs` | 6 |
| `subagent/bridge.rs` | 5 |
| `subagent/slug.rs` | 4 |
| `agent/builtin_tools/path_lock.rs` | 3 |
| `observability/log_retention.rs` | 2 |
| `observability/stray_files.rs` | 2 |
| `subagent/pool.rs` | 2 |
| `agent/builtin_tools/e2e_probe.rs` | 1 |
| `observability/error_kinds.rs` | 1 |
| `db/checkpoints.rs` | 1 ← **orphaned; not compiled, not run today** |
| **Total** | **34** |

**Currently compiled and executed by `cargo test --lib`: 33** (all except the orphaned `checkpoints.rs` one).

A false positive worth recording so nobody re-counts it: `src-tauri/src/graph/extract.rs:711` matches a naive `#[test]` grep but is a **string literal inside the extractor**, not a test:
```rust
let is_test = is_test_file || window.contains("#[test]") || name.starts_with("test_");
```
That file has no `#[cfg(test)]` module. Any tooling that counts tests by grep will overcount by one here.

**Implication for flipping `continue-on-error`:**
- **`cli-checks` is free.** `src-cli` has zero tests; `cargo test` there is a trivial pass. Remove `continue-on-error: true` at `ci.yml:138` with no risk.
- **`rust-checks` is 33 tests of unknown current status.** The step comment claims they need "a display/keychain not present in CI" (ci.yml:66) — but looking at what the tests actually cover (path-jail string logic, slug generation, log retention, subagent pool accounting), most are pure-logic and that justification looks like a stale rationalization rather than a measured finding. **It is a claim, not a measurement, and I could not measure it** (see §6). Do not flip that flag on the strength of the comment in either direction.

**Frontend — zero.** No framework, no config, no test files (C3).

---

## 6. What I could not determine

Bounded by the read-only / no-build constraint, or by information not present in the repo:

1. **Whether the 33 Rust tests currently pass.** I did not run `cargo test`. The phase must run it once before removing `continue-on-error` at `ci.yml:68`. The in-file justification (ci.yml:66-67) is unverified prose.
2. **Whether `npm run build` succeeds.** Static inspection found **no blocker** (details below), but `tsc` under `"strict": true` + `"noUnusedLocals": true` + `"noUnusedParameters": true` (`tsconfig.json`) can only be settled by running it. The static picture is clean:
   - Every bare specifier imported anywhere in `src/` resolves to a declared dependency. The one apparent exception, `postprocessing`, is a **false positive from a comment** — `src/components/city/CityPostProcessing.tsx:3` reads `// Do NOT import from 'postprocessing' directly (not guaranteed to be installed as…)`. There is no actual import of it.
   - The `@/` path alias is defined on **both** sides — `tsconfig.json` `"baseUrl": "."` + `"paths": {"@/*": ["src/*"]}`, and `vite.config.ts:9-11` `alias: { '@': path.resolve(__dirname, './src') }`. Its only two uses (`UpdateDialog.tsx:6`, `UpdateToast.tsx:6`) target `@/stores/updateStore`, and `src/stores/updateStore.ts` exists.
   - `"types": ["node"]` is backed by `@types/node` (`package.json:40`).
3. **Whether any CI check is *required* by branch protection.** Not visible in-repo. C5's "a PR can merge" assumes merges are possible without green checks — plausible but unverified. Worth confirming in GitHub settings before designing around it.
4. **Whether a spawned Memurai actually writes to `src-tauri/`.** `sidecar/mod.rs:804-823` spawns `memurai.exe` but I did not trace whether a `current_dir` is set on that specific `Command` (the two `cmd.current_dir(cwd)` calls at :490 and :597 belong to the Python sidecar spawns). Provenance of `dump.rdb` is a strong inference, not a proof.
5. **Whether `dump.rdb`'s contents are reproducible.** It holds agent procedural memory. Whether that memory can be regenerated is a product question I cannot answer from the repo.
6. **Whether `'__APP_VERSION__'` is correctly substituted.** `vite.config.ts:17` defines it, but `src/components/settings/UpdateSettings.tsx:9` uses it as a **quoted string literal** (`const VERSION = '__APP_VERSION__'`). Whether Vite's `define` replaces inside string literals is version-dependent; I did not verify it, and it is not a build blocker either way. Flagged only so it is not mistaken for a platform issue later.

---

## 7. What the claims MISS

Ordered by consequence.

1. **C9 is a live bug, not dead config — and it is the single most important finding in this pass.** `src/tabs/Logs.tsx:311/319/350/358` calls `save()` and `writeTextFile()` from plugins that have no Rust crate, no `.plugin()` registration, and no capability permission. The Logs-tab export is broken at runtime while CI is fully green. Treating C9 as a config-cleanup ticket would *delete the evidence of a broken feature*. This needs a decision (wire the plugins vs. remove the feature), not a tidy-up.

2. **C9 + C3 + C5 are the same hole seen three ways.** No frontend tests, no PR-time build, no runtime smoke — so a whole class of "compiles and typechecks but does not work" defects is structurally invisible. C9 is the proof that the class is already populated. Fixing C1 (blocking `cargo test`) does nothing for it.

3. **`cargo test --lib` at `ci.yml:72` will silently ignore future integration tests.** If P1 adds the reachability guard as `src-tauri/tests/module_reachability.rs`, `--lib` means **it will never run in CI** — while the step still reports green. That is precisely the guard-the-guard failure the phase is trying to prevent. Either drop `--lib` or place the guard in a `#[cfg(test)]` module inside the lib.

4. **The `cli-checks` cache key looks wrong.** `ci.yml:120-121` sets `workspaces: src-cli -> src-cli/target`, but `src-cli` is a **workspace member** (root `Cargo.toml:2-6`), so its build artifacts land in the *root* `target/` unless overridden. Compare `rust-checks` (:50-51), which uses `. -> src-tauri/target` — also suspect for the same reason. Worth checking whether either cache is actually hitting; if not, both Rust jobs are recompiling from scratch every run.

5. **The `needs:` on both build smokes is dead weight.** `ci.yml:148` and `:179` declare `needs: [rust-checks, frontend-checks]`, but the `if:` already restricts them to main pushes. Harmless, but it makes the graph read as though PRs gate on the smokes.

6. **`sidecar/mod.rs:417-423` contains a comment that contradicts its own code** — "it does NOT spawn or stop Redis" sits four lines above the documented spawn fallback and the `redis_process: Mutex<Option<Child>>` field that holds the spawned child. A docstring that promises one behavior while the code does another is a bug in its own right.

7. **`tauri.conf.json:24` requests two graphics backends at once** — `--use-angle=d3d11` and `--enable-features=Vulkan` in the same argument string. Independent of the portability question in C7, that pair deserves scrutiny.

8. **`dump.rdb` is not gitignored.** Beyond the one committed file, nothing prevents the next `tauri dev` session from dropping a fresh 175 KB snapshot into `src-tauri/` and someone committing it again. The durable fix is the `.gitignore` entry, not the deletion.

9. **`DEVELOPMENT_PLAN.md` §6.12 is a false completion.** Two `[x] COMPLETE` items and one `✅ COMPLETE` success criterion assert that `pub mod graph_tools;` was added and all 22 graph tools registered and visible over MCP `tools/list`. None of it is true (§3, §4c). This is worse than the orphaned files themselves: **the plan document actively hides the gap**, so anyone auditing by reading the plan concludes the feature ships. Whatever process marked §6.12 complete did so without a check that could fail. The checkboxes need reverting in the same step as any P1 fix.

10. **Declaring `db/checkpoints.rs` would fork a shipped feature.** It duplicates the live `git/checkpoint.rs` (declared at `git/mod.rs:31`, wired to five Tauri commands). Wiring the dead one creates a second checkpoint store and a second database. C6 frames all three files as one homogeneous "declare them" batch; they are not — `graph_prompts.rs` is a free one-liner, `graph_tools.rs` needs a small repair, and `checkpoints.rs` needs a **decision**.

11. **The existing grounding docs are partly wrong and should be corrected alongside this work.** `G1-rust-architecture-map.md:89/:527/:540` treats `db/checkpoints.rs` as a live database owner and lists `~/.autodev/checkpoints.db` as database #9; that database cannot exist. G1/G2a correctly flag the two graph files (T32) but miss the third orphan entirely.
