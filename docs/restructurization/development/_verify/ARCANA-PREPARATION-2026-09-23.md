# Arcana preparation verification — 2026-09-23

This is a local Windows checkpoint for the rename, PD0 retirement, environment
repair and active-plan conversion. It does not close P4–P7 or their gates.

## Environment and credentials

- Rust `rustc`/`cargo` 1.98.1, Node 24.21, npm 11.19, C++ build tools,
  Windows SDK and WebView2 were detected. `npm ci` completed.
- Python 3.14.7 was split across `C:\Python314` and the per-user installation.
  A restorable backup of files and PATH/registry state was saved under
  `C:\Users\razia\AppData\Local\Arcana\migration-backups\python-20260923`.
  Installer repair attempts returned 1622/1603. A reversible `Lib` junction,
  copied extension DLLs and User PATH adjustment restored `python`, `sqlite3`,
  `ssl` and `pip` in a fresh shell. Python is not an Arcana desktop dependency.
- The ignored repo `.env` contains a nonempty `GOOGLE_API_KEY` assignment,
  normalized to remove whitespace before `=` without changing or displaying
  its value; the live-test parser requires the exact assignment syntax.
  `.env.example` lists
  that one opt-in live-test input. The desktop does not read `.env`; a key must
  be entered separately in Settings → Providers for a real Gemini turn.

## Checks run

| Check | Result |
|---|---|
| `npm run typecheck` | Pass |
| `npm run lint` | Pass, 42 existing warnings |
| `npm test` | 14 tests passed in 2 files |
| `npm run build` | Pass, Vite chunk-size warnings |
| `cargo check --workspace --locked` | Pass, existing Rust warnings |
| `cargo run -p arcanactl -- --help` | Pass; renamed CLI responds |
| `cargo test -p arcana --lib ci_gate::tests --locked` | 75 passed, including TG8-11 |
| `cargo test -p arcana-core --test s8_memory --locked` | 1 passed in sandbox |
| `MDP-COUNTS-CHECK.py` | Pass: 383 active tasks, 260 checked, 123 open; 126 active gates |
| Vite dev HTTP smoke | 200, Arcana title; port 1420 free after stop |
| `npx tauri dev --no-watch` | First attempt exposed Vite watcher `EBUSY` on Cargo's exe; after ignoring `target/`, Arcana window launched and responded |
| `git diff --check` | Pass after EOF cleanup |

`cargo fmt --all -- --check` remains red on widespread pre-existing style.
The same blank-line diff is present in `HEAD:autodev-core/src/agent/builtin_tools/acknowledge_worker_message.rs`
before this rename, verified by running `rustfmt --check` on that HEAD blob.
Formatting the whole Rust tree would be an unrelated broad rewrite.

## Still open

- A real desktop Gemini turn and process-list proof for S8-02 require the
  provider key in Arcana's vault; the `.env` key alone cannot supply it.
- P0's e2e harness task, P3 memory-gate clauses and P4–P7 workflows remain
  open in the active MDP/readiness map. P9 release, signing and cross-platform
  packaging are later work.
- Desktop launch is verified: `arcana.exe` had a responsive window titled
  `Arcana`, the fresh `~/.arcana` root appeared, and its dev URL returned 200.
  The process and port 1420 were both gone after shutdown. Default fleet
  workers announced auto-start at launch; P4 should verify idle behavior and
  provider configuration before treating that as a working task flow.
- The startup scrub reported removing `LD_LIBRARY_PATH`,
  `CARGO_MANIFEST_PATH`, and Codex path variables as credential-shaped. Those
  names did not contain provider keys; the filter needs review before
  cross-platform worker-command proof so it does not silently break a child
  process. No values were logged.
