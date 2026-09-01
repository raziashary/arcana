# P0 — plan corrections from the §0.2m adversarial pass

**Written 2026-08-31, before the first line of P0 code.** Three read-only refutation
verifiers (`V1`, `V2`, `V3`) were briefed to *refute* the P0 task claims and to default
to REFUTED on ambiguity. Every load-bearing result below was then re-verified in the
main session against the code, as §0.2m requires.

This file **amends** `MDP §6`. Where a task line here disagrees with `§6`, this file wins
until `§6` is regenerated at P0 closeout.

**Headline:** of 27 task claims, **21 confirmed as written**, **5 confirmed but
understated or too narrow**, **1 reversed outright** (`P0-26` — deleting that config
would have erased the evidence of a live runtime break). Six new tasks fall out.

---

## A · Corrections that change what gets built

### `P0-01` — WIDENED · ADR-LOG-RESTRUCT-0002
Replacing the constant is not the fix. `verify.rs:46-47` reads the key with
**`std::env::var` at runtime** while `verify.rs:3-4` claims it is compile-time embedded.
Consequences: the trust root is injectable (and `main.rs:28` runs `dotenvy` walking up
from the CWD, so a dropped `.env` suffices); and the CI secret at
`build-release.yml:34` has **zero effect on the shipped binary** because `build.rs` emits
no `cargo:rustc-env` — a misconception `installers/wix/build-msi.ps1:133-135` repeats.
→ `option_env!` only, delete the runtime override, `compile_error!` on a release build
with no key, and fix both downstream docs in the same step.

### `P0-02` — GATED ON SIGNING · ADR-LOG-RESTRUCT-0003
**No `.sig` asset is produced anywhere** (zero hits across `.github/`, `scripts/`,
`installers/`), so `sig_url` is always empty and the Ed25519 path has never run in
production. `P0-02` landed alone rejects **100 % of releases**. It ships together with a
release-signing step. Keypair generation and the repository secrets are **Razi's action**.

### `P0-05` — EXTENDED
Also: `install_update` never checks its caller-supplied `version` against
`progress.version` (`updater_commands.rs:171-197`), so update history can record a
version that was not the one installed. Flagged, not yet scoped: the Windows
`.bat`/RunOnce and the Linux fixed `/tmp` script are a **second** TOCTOU downstream of
`schedule_install`.

### `P0-07` — WIDENED · ADR-LOG-RESTRUCT-0004
`..` traversal is the least of it. An **absolute** final segment makes `PathBuf::join`
discard the base directory entirely — a write-anywhere primitive. Also: `\` is not a
separator to `split('/')`; query strings survive into the filename; and
`unwrap_or("autodev-update")` is unreachable, so a trailing `/` yields `""` and `dest`
becomes the directory itself. → derive by allow-list, do not blocklist.

### `P0-08` — EXTENDED
`patch.rs` deletion confirmed safe (only reference is `pub mod patch;` at `mod.rs:14`).
Also remove the now-orphaned `use_delta` parameter (`download.rs:42-56`), `delta_url` /
`delta_size` (`check.rs:37-39`, always `None`), their **non-optional** TS mirrors
(`updateStore.ts:13,15`), and the stale `mod.rs:7` doc line.

### `P0-09` — CORRECTED · ADR-LOG-RESTRUCT-0005
`get_provider_key` has zero call sites and is deleted. `get_github_pat` has **one**
(`src/tabs/Settings.tsx:462`, a presence probe whose `.catch` swallows failure) — deleting
it makes the PAT badge read "none" **silently, forever**. → replaced by `has_github_pat`
returning a boolean, in the same commit.

### `P0-13` — WIDENED · ADR-LOG-RESTRUCT-0006
The three-file set does not compile. Full closure adds seven Rust sites —
`commands/sidecar_commands.rs:129,148-149`; `sidecar/client.rs:1273,1394,1399`;
`recipes/runner.rs:215,310,329` — plus four frontend surfaces that break **silently**
because TS still compiles: `components/sidecar/McpToolBrowser.tsx:31`,
`stores/sidecarStore.ts:154-160,309`, `components/sidecar/SidecarPanel.tsx:118-128,265-306`.
Also `sidecar/mod.rs:439` hardcodes `mcp_status = Running` — a green light for a deleted
service.

### `P0-13a` → **`P0-13b` added** · ADR-LOG-RESTRUCT-0001 · **razi-ruled**
Subagents never ran on the Rust engine: `subagent/bridge.rs` drives create → invoke(SSE)
→ delete against the Python sidecar, and `mcp_server/` is the inbound surface those agents
call back through. The shipped recipes mandate it (`defaults.rs:1826,1841`). Razi ruled:
**build the native in-process runner now.** `subagent/bridge.rs` is replaced by
`subagent/native_runner.rs` running a nested `AgentEngine`; `depth` and `agent_role`
become Rust-threaded fields, replacing today's **model-self-reported** `__depth`
(`bridge.rs:86-92` asks the model to send it; `bridge.rs:78` admits that without it the
guard sees 0).

### `P0-16` — EXTENDED
`socket2` (`Cargo.toml:58`) is used only by `mcp_server/server.rs:19` — it drops with
`rmcp` and `axum`.

### `P0-20` — NARROWED
macOS is **not** absent from the repo: `build-release.yml:161-164` already runs
`build-macos` on `macos-14`. The gap is `ci.yml` only. (`V3` refuted the implication.)

### `P0-22` — SCOPED
A reachability guard fails immediately on three files, and one of them **does not
compile**: `graph_tools.rs` has 2 errors — `:272` treats an `Option<GraphNode>` as a
`GraphNode` (E0609 ×5) and `:696` reads a non-existent `confidence_tier`. Repair is ~4
lines, but the 22 structs do not implement `Tool` and nothing registers them, so
declaring the module compiles 840 lines of unreachable code. → the P0 gate ships with a
**named, shrinking** exception list; the real repair is P3 (BUG-GRAPH).

### `P0-24` — CORRECTED
`additionalBrowserArgs` is **WebView2-only**, so it is already inert off-Windows and the
"unconditional d3d11" framing is wrong. The real defect is that it requests **d3d11 and
Vulkan simultaneously**.

### `P0-25` — CORRECTED
No release path reads `bundle.targets`: `build-linux.sh:63` passes `--bundles`
explicitly, and Windows ships an **MSI via WiX**, not nsis. Fix for honesty; it is not
load-bearing.

### `P0-26` — **REVERSED**
Do **not** delete the `plugins.dialog` / `plugins.fs` blocks. `src/tabs/Logs.tsx:13-14`
imports `save()` and `writeTextFile()` and calls them at `:311, :319, :350, :358`. The npm
packages are declared so it typechecks and CI is green — but Rust has neither plugin in
`Cargo.toml`, `main.rs:176` registers only `tauri_plugin_shell`, and
`capabilities/default.json` grants no `dialog:` / `fs:` permission. **Logs export fails
when a user clicks it.** Deleting the config erases the evidence of a broken feature.
→ either register the plugins and grant the capabilities, or remove the UI. Not silence.

### `P0-27` — EXTENDED
Deleting `dump.rdb` is safe, but `sidecar/mod.rs:417-423` claims AutoDev "does NOT spawn
Redis" **four lines above the spawn fallback that created the file**. The contradiction is
the bug; fix both.

---

## B · New tasks

- [x] **P0-28** `download_checksum` / `download_signature` never check HTTP status
  (`download.rs:155,168`) — a 404 body becomes the "checksum". → status-check both.
  → `UNIT-PROVEN`
- [x] **P0-29** `platform_key_from_filename` does not match what CI actually builds:
  `build-msi.ps1:221` emits `autodev_<v>_x64_en-US.exe` while the matcher wants
  `_x64.msi` / `_x64-setup.exe`, and Linux emits `_x86_64.deb` against a matcher wanting
  `_amd64.deb`. **Windows and Linux resolve to no asset at all**, so no end-to-end update
  smoke is possible until this is fixed. → `SMOKE-PROVEN`
- [x] **P0-30** `ci.yml:72` runs `cargo test --lib`, so anything in `src-tauri/tests/`
  **never runs while reporting green** — every later phase's integration gate would be
  invisible. → `cargo test --all-targets`. → `GATE-PROVEN`
- [x] **P0-31** Register `tauri-plugin-dialog` and `tauri-plugin-fs` and grant their
  capabilities, so the Logs export in `Logs.tsx` actually works (supersedes the deletion
  in `P0-26`). → `SMOKE-PROVEN`
- [x] **P0-32** Fix the `sidecar/mod.rs:417-423` Redis contradiction alongside `P0-27`.
  → `CODE-PRESENT`
- [x] **P0-33** `check.rs:73-77` builds a *list* endpoint for beta/nightly whose
  `tag_name` is null, so those channels **can never return an update**. → `UNIT-PROVEN`

---

## C · Confirmed exactly as written — no change

`P0-03` · `P0-04` · `P0-06` · `P0-10` · `P0-11` · `P0-12` · `P0-14` · `P0-15` · `P0-17`
(exact lines `ci.yml:56, 68, 126, 138`) · `P0-18` · `P0-19` · `P0-21` · `P0-23`.

Unreachable-file claim confirmed **exhaustively**: exactly 3 in `src-tauri` (237 files),
**0** in `src-cli` (14) and `src-shared` (8); no `include!`, no `#[path]`, no
declared-but-missing modules. Test inventory: 34 `#[test]` in 11 files, 33 of which run
today; `src-cli` and `src-shared` have none, so unblocking `ci.yml:138` is free.

---

## D · Standing facts worth carrying

- `.env` is loaded at startup (`main.rs:28`, `dotenvy` walks **up** from the CWD) — this
  is the delivery vehicle for both updater env overrides, and it constrains INV-12.
- `graph/extract.rs:711` contains `"#[test]"` as a **string literal** — grep-based test
  counters overcount by one.
- Both Rust CI cache keys point outside the workspace `target/`.
- `VaultEntry` (`keychain.rs:10`) has zero constructors and zero consumers.

---

## E · Outcome, recorded 2026-09-01

All six added tasks (`P0-28`…`P0-33`) are complete, as is `P0-13b` (the native
subagent runner, Razi-ruled). `P0-26` stayed reversed and was superseded by
`P0-31`. `P0-21` is the one item that did not reach its named evidence class —
the e2e harness exists and is wired into CI but has never executed an
assertion, because `msedgedriver` is absent on the Windows dev machine.

Two of the five P0 gates that this file's corrections fed into are worth
restating, because they are the ones a reader will assume are green and are
not:

- **`TG0-01`** — the *refuse* half is unit-proven; the *accept* half cannot be
  proven until Razi generates the signing keypair. Until then the updater
  refuses every update, which is correct for INV-11 and still means updates do
  not work.
- **`TG0-05`** — the three-OS CI lanes exist, run on PRs and can fail; the
  end-to-end harness inside them has not run.
