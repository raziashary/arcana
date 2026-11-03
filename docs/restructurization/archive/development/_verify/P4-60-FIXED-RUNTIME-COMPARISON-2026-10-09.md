# P4-60 fixed runtime comparison — 2026-10-09

Owner approved ADR0334. Microsoft’s official WebView2 download page provided FixedVersionRuntime152.0.4191.62 x64; the matching EdgeDriver152.0.4191.62 archive was downloaded from Microsoft. Both extracted executables have valid Microsoft Corporation Authenticode signatures. Files remain under ignored target/e2e-tools/webview2-152. The shipped Tauri configuration is unchanged.

The diagnostic’s explicit --fixed-runtime-152 flag selects only the approved ignored runtime/driver paths. It uses a fresh profile/workspaces, removes credential variables, bounds readiness and checks owned process/port cleanup. node --check passes. The first attempt stopped at the old test path guard before launching; that guard now validates the exact chosen test driver path.

The actual isolated-build comparison launches the fixed152 browser and renderer with the intended debug flag, but no owned TCP listener appears and endpoint readiness fails after30 seconds. Cleanup finishes; no Arcana/EdgeDriver/tauri-driver process remains in the final process inventory. This falsifies the narrow hypothesis that switching from153 to152 alone fixes attachment; the underlying cause remains unknown. Do not repeat timeout increases or close P4-60/TG4-16 on this result.

Evidence: target/p4-ci-checks/fixed-webview-152-2026-10-09.log. No provider or Calendar run.

Official references: [fixed runtime downloads](https://developer.microsoft.com/en-us/microsoft-edge/webview2/), [Microsoft WebView2 WebDriver](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webdriver).