# 09 — Delivery: platform, packaging, signing, update, and the submodule excision

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-3, INV-6, INV-11), §Part 1B (M-26, M-27, M-29), §Part 6 (P8, P9) |
| **Locks** | Hands **ADR-03-04**, **ADR-04-01**, **ADR-04-02**, **ADR-04-05** to `00 §Part 8` |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G0 §2, §3` · `G1 §C.2, §F` · `G3c` (IN-01…IN-31, P-01…P-35) · `G3a` (SK-50, P-01…P-23) · `G3b`/`G3d` portability tables |

---

## What this is

Everything between "the code is correct" and "a person on Windows, Linux or macOS is using it." Today that gap is where the product actually fails: the stated goal is three platforms, and **only one is bundled, none is signed, the updater installs unverified binaries, and on Windows no fleet task can complete at all.**

**The subtle failures this subsystem is prone to:**

1. **A platform that is configured but never built.** `tauri.conf.json` has complete dmg, deb, appimage and rpm blocks — and `targets: ["nsis"]`.
2. **Shell syntax as a cross-platform assumption.** POSIX operators in a prompt are invisible to a Rust compiler and fatal on Windows.
3. **Verification that is skippable by omission.** "No signature supplied" treated as "signature valid" is not a weak check; it is no check.
4. **A success return from a function that did nothing.** The Linux installer writes a script and returns `success: true`; nothing ever executes it.

---

## 1. Cross-platform command execution (M-26, ADR-04-01)

**This is the root class behind BUG-COMPILE and a long tail of others.**

`bash.rs:89-97` runs `cmd /C` on Windows. `bash.rs:404-437` rewrites pipes into **PowerShell** cmdlets — which cmd.exe does not have. Meanwhile every worker recipe mandates `<cmd> 2>&1 | tee build.log; echo "EXIT:$?"` (`SK-50`), the QC recipe hardcodes `/tmp` (`SK-51`), and the `browser-test` skill uses `$(date +%s)` (`SK-70`). Hooks have a single non-OS-specific `command` field, so **one hook definition cannot be portable** (`SK-14`, P-07). The plugin catalogue assumes `apt-get`, `brew`, `winget` and `python3` — the last of which on Windows is a Store-redirect stub (`SK-97`).

**The design:** callers express **intent**, not syntax.

```rust
enum BuildIntent { Build, Test, Lint, Run { target } }
fn resolve(intent, ecosystem, platform) -> Command   // argv, not a shell string
```

- Structured operations (build, test, probe, validate) go through the abstraction. Rust owns the argv; the model never authors it.
- Genuinely free-form commands (a user's hook, a plugin) keep a **per-OS variant field** — the current single-field design is unfixable by any amount of care.
- **`.cmd` shim resolution on Windows.** `Command::new("tsc")` cannot resolve `tsc.cmd`, so validation silently degrades to a warning claiming the tool is not installed (`G3d`). Resolution must try the platform's real extension set.
- No hardcoded `/tmp` — the platform temp dir, always.

---

## 2. Packaging (M-27, ADR-04-02)

### 2.1 What ships

| Issue | Today | Target |
|---|---|---|
| Bundle targets | `["nsis"]` — **Windows only**, despite complete dmg/deb/appimage/rpm blocks and a macOS release job | Per-platform targets, all three produced in CI |
| Sidecar resource | `resources/sidecars` — **225 MB** of Python | Removed (§4). **The single largest build-level win of the restructure** |
| GPU flag | `--use-angle=d3d11` passed on **every** platform | Windows-conditional. d3d11 is Direct3D; it does not exist on macOS or Linux, and is a plausible contributor to Graph City's WebGL failures |
| Linux deps | `keyring` needs `libsecret`/KWallet at runtime; **not in the deb `depends` list** | Declared, or the vault fails on a clean install |
| Dead config | `plugins.dialog` and `plugins.fs` declared; neither plugin is in `Cargo.toml` nor registered | Removed |

### 2.2 Signing (ADR-03-04 — procurement-gated)

`signingIdentity: null` and `certificateThumbprint: null`. macOS Gatekeeper blocks the app; Windows SmartScreen warns. `hardenedRuntime: true` with no signing identity is incoherent — hardened runtime is a property of a signed binary.

**This has a lead time.** An Apple Developer certificate and a Windows code-signing certificate must be procured before P9, which is why ADR-03-04 is flagged for an early answer.

---

## 3. The updater (M-27, INV-11)

**Today the updater will install a binary it has not verified.** The chain:

```
release published without .sig / .sha256 sidecar files
  → check.rs sets signature = "" and checksum = ""
  → verify.rs:90  `if !sig_url.is_empty()`   → Ed25519 branch skipped entirely
  → verify.rs:18  empty checksum             → returns Ok(())
  → verify_update returns Ok
  → stage = Ready → user clicks Install → the binary runs
```

`updater_commands.rs:111` widens it further: `download_checksum(...).unwrap_or_default()` converts a **transient network failure** into an empty string, silently disabling checksum verification for an otherwise-signed release.

And the trust root does not exist: `EMBEDDED_PUBLIC_KEY` is base64 of the literal string `undefined-public-key-for-development-only-placeholder` — 53 bytes, not 32. **There is no configuration in which a genuine signature verifies successfully**: debug builds skip, release builds reject every signature.

### 3.1 The rules

1. **Absence of a signature is a hard failure.** Never a pass.
2. **The trust root is a real Ed25519 key**, private half in CI secrets only.
3. **Verification happens at install time**, re-checked — not read from a mutable in-memory `stage: Ready`.
4. **HTTPS is enforced** on the update URL and every asset URL. `AUTODEV_UPDATE_URL` is currently read from the environment with no scheme validation.
5. **Filenames are sanitised.** `url.split('/').last()` permits `..\` traversal on Windows.

### 3.2 Install must actually install

| Platform | Today | Required |
|---|---|---|
| **Linux** | `schedule_linux` writes a script and returns `success: true`. **Nothing ever executes it.** The user restarts, nothing installs, the update is re-offered every 4 hours forever | A real install path |
| **Windows** | `RunOnce` fires at next **logon**, not app restart — while the message says "Restart AutoDev to apply" and the code then calls `app_handle.restart()` | Mechanism and message must agree |
| **macOS** | A `RunAtLoad` LaunchAgent that is never `launchctl load`ed and never cleaned up; it points at a self-deleted script and errors at every subsequent login | A real install path with cleanup |

### 3.3 Delta patching

Non-functional in every direction: `bzip2_decompress` is a passthrough stub (the crate is genuinely absent from `Cargo.toml`) while the doc claims three streams are decompressed; control values are decoded as two's complement when bsdiff4 uses sign-magnitude; `new_size` is an unbounded attacker-controlled allocation; the length guard is bypassable by `usize` overflow; and `create_patch` emits `ctrl_len = 0`, so nothing it produces can ever be applied.

**Recommendation: delete the delta path.** Full-installer updates are correct today and the delta path has never worked. Re-add it later with a tested implementation if bandwidth justifies it.

---

## 4. The submodule excision (M-29, P8, ADR-04-05)

**Its own phase** (Razi, 2026-08-31).

**Entry gate:** all nine ported capabilities (`G1 §C.4`) pass their tests with the sidecar **not running**.

**What is removed:**

| Item | Size |
|---|---|
| `src-tauri/src/sidecar/` | 3,161 LOC |
| 29 sidecar-dependent Tauri commands | 18 in `sidecar_commands.rs`, all 7 subagent commands, 2 worker commands, `get_startup_phase`, 2 Playwright |
| `resources/sidecars` | **225 MB** |
| `submodules/core-orchestrator`, `submodules/memory-mcp-server` | both `.gitmodules` entries |
| The MCP HTTP server (ADR-03-02) | **~705 LOC** — `server.rs`, `tool_adapter.rs`, **`port_status.rs`**, parts of `mod.rs`, `list_mcp_tools`, `get_mcp_port_status`, and the `main.rs` wiring. ⚠ **CORRECTED 2026-08-31:** an earlier revision said `port_status.rs` survives for external MCP. It does not — `port_status.rs:17` imports `MCP_SERVER_PORT`/`current_mcp_port` from `server.rs`, so it is own-port diagnostics and **keeping it breaks the build** (`G12`). Only `health_monitor.rs` (400 LOC) survives, and per ADR-05-06 it should be rewritten rather than ported |
| Dependencies | `ollama-rs`, `dotenvy`, `sysinfo`, possibly `rmcp` + `axum` |
| Artifacts | `src-tauri/dump.rdb` (175 KB committed Redis snapshot); the Memurai starter; both Redis setup docs |

**The coupled deletion:** `subagent/bridge.rs`'s `find_frame_end`/`parse_event_frame` are `pub(crate)` and reused by `sidecar::client::invoke_worker_streaming`. Those two removals **cannot be sequenced independently**.

**Exit gate:** the tree contains no Python; the installer contains no `.py`; the app is fully functional on all three platforms.

*(The credential concern originally attached to this phase is closed — Razi removed `core-orchestrator/.env` on 2026-08-31 and confirms it was never committed. INV-3 and INV-12 are the recurrence guard.)*

---

## 5. CI (M-28, ADR-04-03)

**Today nothing in CI can fail except `cargo clippy` and `tsc`** (`G0 §2`):

- Rust tests and `cargo fmt` are `continue-on-error` in **both** crates
- There is **no frontend test framework at all** — no vitest, jest, playwright, and no `test` script
- `npm run lint` is aliased to `tsc --noEmit`, so CI's "ESLint" step **runs typecheck twice under two names**. There is no ESLint in the project
- Build smokes are gated `if: push && ref == main` — **a PR can merge without either platform ever being compiled**
- **There is no macOS job at all**

Target: every correctness check blocking; a frontend test framework; real linting; all three platforms built **on PR**; and the end-to-end smoke of spec `10`.

---

## 6. Test gates

| Gate | Asserts | Catches |
|---|---|---|
| **G-PLATFORM-TRIPLE** | The end-to-end smoke passes on Windows, Linux and macOS | BUG-COMPILE and the whole portability class |
| **G-BUNDLE** | Each platform produces its installer, and the installer contains **no `.py` and no `.env`** | `targets: ["nsis"]`; INV-3 |
| **G-SIGNED** | Every released artifact is signed and notarised where applicable | The null signing identities |
| **G-UPDATE-REFUSE** | An update with a **missing** signature is **refused**; with a bad signature, refused; with a good signature, installed | IN-01…IN-05 |
| **G-UPDATE-INSTALLS** | On each platform, an update actually replaces the binary and the app restarts on the new version | The Linux no-op; the Windows RunOnce mismatch; the macOS orphan plist |
| **G-CMD-INTENT** | Build/test/lint/run resolve to a correct argv on all three platforms; `.cmd` shims resolve on Windows | SK-50, the validator degradation |
| **G-NO-POSIX** | Static check: no prompt, recipe or skill text contains POSIX-only shell syntax or `/tmp` | SK-50, SK-51, SK-70 |
| **G-NOSIDECAR** | The full suite passes with the sidecar not running — **the P8 entry gate** | Residual coupling |
| **G-CI-BLOCKING** | Static check: no `continue-on-error` on any correctness job | INV-4 |
| **G-CLEAN-INSTALL** | On a machine with no Rust, Node, Python or `uv`, the installer produces a working app | The `uv`-from-PATH assumption |

---

> No ADR decision is recorded in this spec. It hands **ADR-03-04**, **ADR-04-01**, **ADR-04-02** and **ADR-04-05** to `00 §Part 8`.
