# End-to-end harness

WebdriverIO driving the **built** AutoDev binary through `tauri-driver`.

Why against the built binary rather than the dev server: every defect in the
restructure's register survived `tsc` and `cargo clippy`. A harness that tests
the frontend in a browser with the Rust side mocked cannot catch the class of
bug this project actually has — a command that is registered but never called, a
window that opens onto an empty root, a database that fails to migrate on a
clean machine.

## Prerequisites

```bash
cargo install tauri-driver --locked
```

Then the platform's native webview driver:

| Platform | Driver | Notes |
|---|---|---|
| Linux | `apt-get install webkit2gtk-driver xvfb` | run headless under `xvfb-run` |
| Windows | Microsoft Edge WebDriver | must match the installed Edge build |
| macOS | — | **not supported**: `tauri-driver` has no WKWebView backend |

macOS is therefore covered by the build smoke in CI rather than by this
harness. That is a real gap in the platform triple, not an oversight — it is
recorded as such rather than papered over with a skipped test that reports
green.

## Running

```bash
npm run tauri -- build --debug
npm run test:e2e
```

The config fails immediately with an explanatory message when `tauri-driver` is
absent, rather than timing out against a driver that was never started.

## Scope today

One scenario: the application launches, the webview is reachable, and the React
root actually mounts. It exists to prove the harness works and that an
assertion in it can fail.

The full acceptance test — *a user installs on a clean machine, enters their own
API key, points at a repo, types one sentence, and receives a build-verified
change* (`docs/restructurization/vision/10-PROOF.md §2`) — is built on this
foundation in P9. The harness lands in P0 because P4's exit gate needs it to
exist.
