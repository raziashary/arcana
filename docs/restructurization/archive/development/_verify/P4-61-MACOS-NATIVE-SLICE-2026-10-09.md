# P4-61 macOS Native Sandbox Slice — 2026-10-09

## Scope
Only `native/macos-sandbox/**`, `scripts/build-macos-sandbox.sh`, and this report were changed. No Rust, Tauri, CI, release, tests, formatters, or Git state were touched.

## Findings and decision
ADR-LOG-RESTRUCT-0170 approves a separately signed App Sandbox XPC service with strict external-read/network denial and requires real descendant, cancellation, and output proof before execution is enabled. ADR-0171 authorizes guarded workspace staging/copy-back but leaves path, symlink, identity, bounded write-set, and atomic replacement checks to implementation. ADR-0337 selects SDK/clang scripts rather than an Xcode project.

The live `ProcessSpec` in `arcana-core/src/platform/process_sandbox/mod.rs` carries a program, arguments, cwd, environment edits, and a workspace path. It has no security-scoped bookmark, open directory handle, runtime capability descriptor, or staging manifest. The current macOS `spawn` path canonicalizes the workspace and then fails closed; `SandboxedChild` expects stdout/stderr, actual child exit status, and process-group teardown. With no grounded workspace authority or staged before/after contract available inside this ownership boundary, a safe execution/copy-back runner cannot be implemented here. This slice therefore provides only a signed-peer readiness handshake and does not alter the Rust refusal.

Apple’s XPC documentation describes the embedded `Contents/XPCServices` bundle and `xpc_main` startup. `SecCodeCreateWithXPCMessage` lets each endpoint validate the peer’s signed code identity from the actual XPC message. The service entitlement file contains only `com.apple.security.app-sandbox`; it declares no network or broad filesystem access. The SDK/clang script signs the nested XPC service before signing the containing host app.

## Implementation
- `native/macos-sandbox/service.c` accepts a dictionary with exactly two fields and one operation (`readiness`, protocol version 1), validates the sender’s signed identifier, and replies with a fixed readiness result. Other operations, versions, or extra fields fail; the service does not launch children.
- `native/macos-sandbox/host.c` exposes only `--readiness`, connects to the bundled service, validates its signed identifier and exact reply shape, and times out after five seconds. It reports `execution_enabled:false`.
- `native/macos-sandbox/Host-Info.plist` and `Service-Info.plist` define a minimal host app and embedded XPC service, with macOS 11 as the minimum for the public XPC-message code-identity API.
- `native/macos-sandbox/service-entitlements.plist` enables the App Sandbox and no additional capabilities.
- `scripts/build-macos-sandbox.sh` compiles both C binaries against the Apple macOS SDK, assembles the host/XPC bundle, signs the nested service first, signs the host second, and verifies both signatures. It defaults to ad-hoc identity `-`, labels that output as non-release, accepts `SIGNING_IDENTITY` for explicit signing, and refuses an already-existing output app rather than deleting it.

## Build and smoke commands (not run)
Run on macOS with Apple Command Line Tools installed:

```bash
SIGNING_IDENTITY=- bash scripts/build-macos-sandbox.sh "$PWD/target/macos-sandbox"
"$PWD/target/macos-sandbox/ArcanaSandboxHost.app/Contents/MacOS/ArcanaSandboxHost" --readiness
```

Expected positive output: `{"ready":true,"protocol_version":1,"execution_enabled":false}`. The host returns 64 for missing/unknown arguments; it returns 69 for service unavailability, timeout, identity mismatch, or malformed reply. Service negatives to exercise on macOS are unknown operation, wrong protocol version, and extra request keys; they return an unsupported response and never start a child. These build, signing, handshake, and negative-case checks were not run on this Windows host.

For a Developer ID build, set `SIGNING_IDENTITY` to the selected signing identity. Ad-hoc CI output is not Developer ID identity or release signing.

## Limits and integration requirements
This is service/package readiness only. It accepts no `ProcessSpec`, passes no file descriptors, launches no untrusted process, stages no workspace/runtime, performs no copy-back, and returns no process exit code. Readiness must never be treated as a successful process launch. P4/TG gates remain open.

Before execution work, the shared Rust boundary needs a grounded host workspace-access authority and bounded runtime/staging request. Follow-on proof must cover canonical path and symlink/race protection, bounded atomic copy-back, XPC stdout/stderr FD passing, actual exit status, descendant cancellation, external-read denial, and network denial. The current code requirements check peer signing identifiers; ad-hoc identities have no Developer ID team identity and do not authorize execution. Production execution must pin the production signing team/identity or an equivalent exact code requirement before enabling any child launch.

## Sources
- Repository: `ADR_LOG.md` entries ADR-LOG-RESTRUCT-0170, 0171, 0337; `arcana-core/src/platform/process_sandbox/mod.rs` (`ProcessSpec`, `spawn`, `SandboxedChild`); `_verify/P4-MACOS-PACKAGING-NEXT-2026-10-06.md`.
- Apple: [XPC service bundle layout and `xpc_main`](https://developer.apple.com/documentation/xpc/creating-xpc-services), [code identity from an XPC message](https://developer.apple.com/documentation/security/seccodecreatewithxpcmessage), [`xpc_dictionary_get_remote_connection`](https://developer.apple.com/documentation/xpc/1505637-xpc_dictionary_get_remote_connec).
