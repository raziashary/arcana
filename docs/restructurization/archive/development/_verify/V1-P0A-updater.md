# V1 · P0-A adversarial verification — the updater trust root

**Role.** Adversarial verifier. Brief: *refute* the claims; default to REFUTED on ambiguity.
**Mode.** Read-only. No build, no cargo, no server. All quotes are from the working tree as read on 2026-08-31.
**Scope.** `src-tauri/src/updater/*`, `src-tauri/src/commands/updater_commands.rs`, plus repo-wide dependency questions.
**Maps to.** MDP §6 `P0-A · The updater trust root`, tasks P0-01 … P0-08.

---

## 1 · Verdict table

| # | Claim (abbrev.) | Verdict | One-line reason |
|---|---|---|---|
| **C1** | Placeholder pubkey → 53 bytes → verification can never succeed | **PARTIALLY CONFIRMED — mechanism misstated** | 53 bytes and the debug-skip / release-bail are exact, but the key is a **runtime** `std::env::var` lookup, not a compile-time embed — so "never" is false, and **P0-01 as written would not fix it**. |
| **C2** | `if !sig_url.is_empty()` skips the whole Ed25519 branch | **CONFIRMED** | `verify.rs:90` verbatim; the signature branch is inside the guard, nothing else looks at signatures. |
| **C3** | Empty checksum ⇒ `Ok(())` | **CONFIRMED (verbatim, exact lines)** | `verify.rs:18-21` is exactly as claimed. |
| **C4** | `.unwrap_or_default()` turns a network failure into "no checksum" | **CONFIRMED — and understated** | `updater_commands.rs:111` verbatim; a *successful* HTTP response with an empty/whitespace body also yields `""` without any error to swallow. |
| **C5** | `schedule_install` has no verify call; only gate is in-memory `stage == Ready` | **CONFIRMED (verbatim, exact lines)** | `install.rs:34-50` and `updater_commands.rs:179` are exactly as claimed. |
| **C6** | `AUTODEV_UPDATE_URL` + asset URLs unvalidated; `http://` accepted | **CONFIRMED** | `check.rs:70-71`; no `https_only`, no scheme check anywhere in `check.rs` or `download.rs`. |
| **C7** | `filename_from_url` unsanitised ⇒ path escape on Windows | **CONFIRMED — and understated** | `download.rs:184-186` verbatim; an *absolute* segment discards the base directory entirely, which is worse than `..` traversal. |
| **C8** | `patch.rs` non-functional and safe to delete; no caller outside `updater/` | **CONFIRMED** | Every sub-claim holds; repo-wide the **only** reference is `pub mod patch;` at `updater/mod.rs:14`. |

**Nothing in C1–C8 has already been fixed.** Every quoted line is live in the working tree, the module is wired into the running app (`main.rs:508-511` spawns the 4-hour background checker; `lib.rs:401-408` registers all eight updater commands), and there is **no test anywhere** that pins the current behaviour.

---

## 2 · Per-claim detail

### C1 — the embedded public key · **PARTIALLY CONFIRMED, mechanism misstated**

The literal and the byte count are right. `src-tauri/src/updater/verify.rs:13-14`:

```rust
const EMBEDDED_PUBLIC_KEY: &str =
    "dW5kZWZpbmVkLXB1YmxpYy1rZXktZm9yLWRldmVsb3BtZW50LW9ubHktcGxhY2Vob2xkZXI=";
```

Decoded: `undefined-public-key-for-development-only-placeholder` — **53 bytes**. (Note for the record: the older grounding note `G1-rust-architecture-map.md:215` says *51 bytes*; **53 is correct** and the claim table is right.)

The length branch, `verify.rs:52-58`:

```rust
    if pubkey_bytes.len() != 32 {
        // Dev placeholder key — skip verification
        #[cfg(debug_assertions)]
        return Ok(());
        #[cfg(not(debug_assertions))]
        anyhow::bail!("Invalid public key length ({})", pubkey_bytes.len());
    }
```

So "debug builds skip it, release builds reject" is exact for the fallback path.

**Where the claim is wrong, and it matters.** The key is *not* embedded. `verify.rs:46-47`:

```rust
    let pubkey_b64 = std::env::var("AUTODEV_UPDATE_PUBKEY")
        .unwrap_or_else(|_| EMBEDDED_PUBLIC_KEY.to_string());
```

`std::env::var` is a **runtime** read of the process environment. Three consequences the phase must not miss:

1. **"Can never succeed" is false.** If the running process has `AUTODEV_UPDATE_PUBKEY` set to a valid 32-byte base64 key, verification proceeds and can succeed. The claim is true only of a process with no such variable — which is the shipped case.
2. **The CI secret does nothing.** `.github/workflows/build-release.yml:34` sets `AUTODEV_UPDATE_PUBKEY: ${{ secrets.AUTODEV_UPDATE_PUBKEY }}` as a **build**-time job env var, and the file's own header comment (`verify.rs:3`) claims *"embedded at compile time from AUTODEV_UPDATE_PUBKEY env var"*. `src-tauri/build.rs` is two lines (`tauri_build::build()`) and emits no `cargo:rustc-env`, and the code uses `std::env::var`, not `env!`. **The signing key configured in CI has zero effect on the shipped binary.** `installers/wix/build-msi.ps1:133-135` carries the same misconception.
3. **It is a trust-root injection surface.** Anything that can set an environment variable for the AutoDev process substitutes its own verification key — the same class of hole as C6's `AUTODEV_UPDATE_URL`.

**Impact on P0-01.** As written — *"Replace `EMBEDDED_PUBLIC_KEY` … with a real Ed25519 public key; private half in CI secrets only"* — the task would swap the constant and leave the runtime override in place, producing a build that still trusts `$AUTODEV_UPDATE_PUBKEY` and a CI pipeline that still believes it is injecting the key. P0-01 needs a second half: **make the key compile-time (`env!`/`option_env!` + `build.rs` rerun-if-env-changed) and delete the runtime `std::env::var` override.**

### C2 — the empty-signature-URL skip · **CONFIRMED**

`verify.rs:80-98`:

```rust
pub async fn verify_update(
    file_path: &Path,
    expected_checksum: &str,
    sig_url: &str,
) -> Result<()> {
    // 1. SHA-256 checksum (fast, local)
    verify_checksum(file_path, expected_checksum)
        .context("Checksum verification failed")?;

    // 2. Ed25519 signature (requires downloading .sig)
    if !sig_url.is_empty() {
        let sig_bytes = super::download::download_signature(sig_url).await
```

Line 90 is exactly `    if !sig_url.is_empty() {`. One precision note: the function returns `Ok(())` on an empty `sig_url` **provided the checksum step passed** — which, per C3, it does trivially whenever the checksum is also empty. The two holes compose into "verified" with zero cryptographic work.

This is the *live* path, not a theoretical one: `check.rs:136-137` initialises `signature: String::new()` and only fills it in the second pass (`check.rs:147-153`) if the release publishes a `.sig` asset whose base name matches a platform pattern. A release with no `.sig` asset — or with one whose name does not match `platform_key_from_filename` — reaches `verify_update` with `sig_url == ""`.

Also dead as a result: `verify_signature`'s own empty-`sig_bytes` branch (`verify.rs:38-44`) is unreachable from `verify_update`, because `verify_update` only calls it when the URL is non-empty.

### C3 — empty checksum is a passing checksum · **CONFIRMED, verbatim, exact lines**

`verify.rs:17-21`:

```rust
pub fn verify_checksum(file_path: &Path, expected_hex: &str) -> Result<()> {
    if expected_hex.is_empty() {
        // No checksum provided — skip (warn in production)
        return Ok(());
    }
```

The comment says *"warn in production"*; there is no warning, in production or otherwise — a docstring promising behaviour the code does not have. Line numbers in the claim are exact.

### C4 — `unwrap_or_default()` on the checksum fetch · **CONFIRMED, and the claim understates the hole**

`src-tauri/src/commands/updater_commands.rs:109-116`:

```rust
                // Verify checksum + signature
                let expected_checksum = if !checksum_url.is_empty() {
                    updater::download::download_checksum(&checksum_url).await.unwrap_or_default()
                } else {
                    String::new()
                };

                let verify_result = updater::verify_update(&path, &expected_checksum, &sig_url).await;
```

Line 111 is exact. Two additions the phase should fold into P0-04:

- **The outer guard is redundant** — `download.rs:160-163` already returns `Ok(String::new())` for an empty URL, so both arms produce `""`.
- **A network *error* is not the only way to get `""`.** `download.rs:168-170`:

  ```rust
      let text = client.get(checksum_url).send().await?.text().await?;
      // Format: "sha256hex  filename"
      Ok(text.split_whitespace().next().unwrap_or("").to_string())
  ```

  There is **no HTTP status check**. A 404/500 with an empty or whitespace-only body is a *successful* `Result` carrying `""`. Deleting `.unwrap_or_default()` (P0-04 as written) does **not** close that path — `download_checksum` must also reject non-2xx responses and refuse to return an empty string. The same missing status check exists in `download_signature` (`download.rs:155`), where a 404 body is handed to `verify_signature` as if it were a signature (that one fails closed on the 64-byte length check, so it is sloppy rather than dangerous).

### C5 — no verification at install time · **CONFIRMED, verbatim, exact lines**

`src-tauri/src/updater/install.rs:34-50` in full — no `verify_*` call, and no `use` of `super::verify` anywhere in the file:

```rust
pub fn schedule_install(
    installer_path: &Path,
    version: &str,
    current_version: &str,
) -> Result<InstallResult> {
    #[cfg(target_os = "windows")]
    let result = schedule_windows(installer_path, version)?;
    #[cfg(target_os = "macos")]
    let result = schedule_macos(installer_path, version)?;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = schedule_linux(installer_path, version)?;

    // Record the pending update
    record_pending_update(version, current_version).ok();

    Ok(result)
}
```

The only gate, `updater_commands.rs:176-183`:

```rust
    let progress = state.progress.lock().await.clone()
        .ok_or_else(|| AutoDevError::Agent("No download in progress — call download_update first".into()))?;

    if progress.stage != DownloadStage::Ready {
        return Err(AutoDevError::Agent(
            format!("Download not ready (stage: {:?})", progress.stage)
        ));
    }
```

Line 179 is exact. The state is in-memory and mutable — `mod.rs:34`: `pub progress: Mutex<Option<DownloadProgress>>`.

Two qualifications, so P0-05 is scoped honestly:

- **Not directly frontend-settable.** `stage` is written only by the spawned download task (`updater_commands.rs:118-131`) and by `download_update`'s progress stream. The frontend cannot post a forged `Ready`. The real exposure is (a) any *future* writer of `state.progress`, and (b) the gap between verification and install — the file at `progress.path` sits in `~/.autodev/updates/` and is never re-read, re-hashed or re-signature-checked before `schedule_install` hands it to `msiexec` / `pkexec`. That TOCTOU is what P0-05 must close.
- **A defect the claim misses in the same function:** `install_update`'s `version` argument comes from the frontend and is **never** compared to `progress.version`, though the sibling `download_update` does exactly that check at `updater_commands.rs:63-67`. The frontend-supplied string is what reaches `schedule_install` (`updater_commands.rs:197`) and gets written to the update history, so the history can be made to lie about which version was installed.

`schedule_install` is also `pub` and re-exported (`mod.rs:20`), so any future caller inherits "no verification" by default.

### C6 — no scheme validation · **CONFIRMED**

`src-tauri/src/updater/check.rs:70-71`:

```rust
    let url = std::env::var("AUTODEV_UPDATE_URL")
        .unwrap_or_else(|_| DEFAULT_UPDATE_URL.to_string());
```

No scheme check here or downstream. Asset URLs are taken verbatim from the server's JSON — `check.rs:126` (`browser_download_url` → `full_url`), `check.rs:145`+`:151` (`.sig` → `signature`), `check.rs:145`+`:158` (`.sha256` → `checksum`) — and every `reqwest::Client::builder()` in the module is built without `.https_only(true)`: `check.rs:79-83`, `download.rs:66-69`, `download.rs:151-154`, `download.rs:164-167`. Plain `http://` is accepted at every hop, and redirects follow reqwest's default policy (up to 10), so an `https://` metadata URL can still redirect an asset fetch to `http://`. **P0-06 must cover the redirect policy, not just the initial scheme.**

`AUTODEV_UPDATE_URL` combined with C1's runtime `AUTODEV_UPDATE_PUBKEY` means a single environment-variable write gives an attacker both the update source *and* the key used to verify it.

### C7 — `filename_from_url` · **CONFIRMED, and understated**

`src-tauri/src/updater/download.rs:184-186`, the whole function:

```rust
fn filename_from_url(url: &str) -> String {
    url.split('/').last().unwrap_or("autodev-update").to_string()
}
```

Used at `download.rs:58`: `let dest = download_dir()?.join(filename_from_url(url));`. Note it operates on the **raw URL string** before reqwest parses or normalises it, so the `url` crate's WHATWG backslash handling does not protect this path.

Four problems, only the first of which the claim names:

1. **`..\` traversal on Windows** — `~/.autodev/updates/` + `..\..\evil.exe` resolves out of the directory. Confirmed as claimed.
2. **Worse: absolute-path replacement.** A final segment such as `C:\Users\Public\…\x.exe` carries a Windows prefix and root, so `PathBuf::join` **discards the base directory entirely** and writes wherever the server names. That is a write-anywhere primitive, not merely traversal.
3. **Query strings and fragments are not stripped.** `…/autodev_x64.msi?token=abc` yields a filename containing `?` — illegal on Windows, so `File::create` fails with a confusing I/O error.
4. **`unwrap_or("autodev-update")` is dead code.** `str::split` always yields at least one item, so `last()` is never `None`; a URL ending in `/` produces `""`, making `dest` the directory itself. P0-07 should replace that fallback, not preserve it.

### C8 — `patch.rs` · **CONFIRMED on every sub-claim**

- **Passthrough stub** — `patch.rs:134-138`:

```rust
fn bzip2_decompress(data: &[u8]) -> Result<Vec<u8>> {
    // TODO: Add `bzip2 = "0.4"` to Cargo.toml and implement proper decompression.
    // For now return raw data (works if patch was built without compression).
    Ok(data.to_vec())
}
```

  The doc comment above it (`patch.rs:130-133`) and `apply_bsdiff`'s header (`patch.rs:54-57`) both claim three bzip2 streams are decompressed.

- **`bzip2` genuinely absent from `src-tauri/Cargo.toml`** — confirmed; the only compression dep is `flate2 = "1"` (line 117). Updater deps that *are* present: `ed25519-dalek = { version = "2", features = ["std"] }` (104), `semver = "1"` (105), `sha2 = "0.10"` (111), `dirs = "5"` (116), `base64 = "0.22"` (118).

- **Two's complement vs sign-magnitude** — `patch.rs:90-95`:

```rust
    let read_i64 = |r: &mut &[u8]| -> Result<i64> {
        let mut buf = [0u8; 8];
        r.read_exact(&mut buf).context("ctrl read error")?;
        let v = u64::from_le_bytes(buf) as i64;
        Ok(v)
    };
```

  bsdiff4's `offtin` treats the high bit of the eighth byte as a sign flag over a magnitude; this reinterprets the whole word as two's complement, so any negative control value (routine — `z` is frequently negative) decodes wrongly.

- **`create_patch` emits `ctrl_len = 0`** — `patch.rs:154-165`:

```rust
    let new_size = new_data.len() as u64;
    let ctrl_data: Vec<u8> = {
        // One control tuple: (0, new_size, 0) — copy all from extra
        let mut c = b"BSDIFF40".to_vec();
        c.extend_from_slice(&new_size.to_le_bytes()); // new_size
        c.extend_from_slice(&0u64.to_le_bytes());      // ctrl_len (raw, no ctrl)
        c.extend_from_slice(&0u64.to_le_bytes());      // diff_len
        c
    };
```

  The comment claims "one control tuple"; **none is emitted**. On apply: `ctrl_len == 0` ⇒ `ctrl_raw` empty ⇒ the first `read_i64` inside `while new_pos < new_size` fails with `ctrl read error`. The round trip is broken in both directions. The sole input that "succeeds" is a zero-byte new file (`new_size == 0`, loop never entered).

- **More dead code in the same function:** `let old_data = std::fs::read(old_path)?;` (`patch.rs:150`) is never used.

- **Deletion safety — confirmed; see §4.1.**

---

## 3 · What I could not determine

Stated plainly, because an unsettled claim is not a confirmed claim.

1. **Nothing was compiled or executed** (per the constraint). Every verdict above is a static reading of the source. Specifically *not* proven by execution: release-mode compilation of the `#[cfg(debug_assertions)]` / `#[cfg(not(...))]` split branches at `verify.rs:38-44` and `:52-58`; reqwest's effective default redirect limit (no `.redirect(...)` call exists — that is evidence, not proof of the default); Windows `PathBuf::join` replacing the base on an absolute segment (documented `std` behaviour, not demonstrated here); and that `VerifyingKey::from_bytes` would accept a real key on the happy path, which no code in this repo exercises.
2. **macOS artifact naming.** I did not read `installers/dmg/create-dmg.sh`, so I cannot say whether `platform_key_from_filename` matches the DMG names CI actually publishes. The Windows and Linux answers in §5-M3 are grounded; the macOS one is open.
3. **Whether any real published release exists** with the asset layout the code expects. No network was used; the CI analysis is from `.github/workflows/build-release.yml` source alone.
4. **The contents of the repo-root `.env`.** It exists and is loaded at startup (§5-M2), but I did not open it, so I cannot say whether `AUTODEV_UPDATE_URL` / `AUTODEV_UPDATE_PUBKEY` are set in this working copy.
5. **Runtime severity of C7.** The traversal is real in the code path; I did not attempt to establish that a GitHub-hosted `browser_download_url` can actually carry a backslash segment. Against a *non-GitHub* update server (which C6 permits) the constraint disappears entirely, so the fix is warranted either way — but "GitHub would let you do this" is not something I verified.

---

## 4 · The four dependency questions

### 4.1 · `patch.rs` deletion safety — **safe, with exactly one required companion edit**

Repo-wide, across all of `src-tauri/src/**`, the **only** reference to anything in `patch.rs` is the module declaration itself:

- `src-tauri/src/updater/mod.rs:14` — `pub mod patch;`

Nothing else. Confirmed by searching for `patch::`, `updater::patch`, `apply_patch`, `create_patch`, `should_use_delta`, `cached_installer_path`, `apply_bsdiff`, `bzip2_decompress` across the repo (excluding `node_modules`, `.venv`, and `submodules/`): all hits are inside `patch.rs` itself, plus the vision/grounding docs. `mod.rs`'s re-export block (`mod.rs:18-22`) exports nothing from `patch`. No `#[tauri::command]` exists in the file, so no frontend `invoke` target disappears.

**To delete cleanly, P0-08 must also:**

1. Remove `pub mod patch;` (`updater/mod.rs:14`) — otherwise the build fails immediately.
2. Fix the module doc comment `updater/mod.rs:7` — `//   patch.rs      — optional delta patch support (bsdiff4 format)` — which becomes stale the moment the file goes (Rule 2 no-stale).
3. Decide the fate of the now-orphaned delta plumbing, none of which `patch.rs` owns but all of which exists only for it:
   - `download.rs:42-56` — the `use_delta: bool` parameter and the `delta_url`/`delta_size` branch. The one caller passes `false` unconditionally (`updater_commands.rs:107`).
   - `check.rs:37-39` — `PlatformAsset.delta_url` / `delta_size`, which `check.rs:133-135` always populates as `None`.
   - `src/stores/updateStore.ts:13,15` — the TS mirror declares `delta_url: string` and `delta_size: number` as **non-optional**, while Rust serialises `Option<…>` → `null`. That contract is already wrong today; stripping the Rust fields makes it a compile-relevant change on the frontend too.

### 4.2 · Other callers of `verify_update` / `verify_checksum` / `verify_signature` — **none; making them strict breaks nothing**

| Function | Callers |
|---|---|
| `verify_update` | `commands/updater_commands.rs:116` — the only one. Re-exported at `updater/mod.rs:22`. |
| `verify_checksum` | `updater/verify.rs:86` only (inside `verify_update`). `pub`, not re-exported from `mod.rs`. |
| `verify_signature` | `updater/verify.rs:93` only (inside `verify_update`). `pub`, not re-exported from `mod.rs`. |
| `download_checksum` | `commands/updater_commands.rs:111` only. |
| `download_signature` | `updater/verify.rs:91` only. |

So P0-02/03/04 have a single downstream consumer, which the same phase is already editing. **One forward-looking trap:** `PlatformAsset.checksum` is documented as *"SHA-256 hex of the full file"* (`check.rs:42`) but assigned a **URL** (`check.rs:158`); `signature` likewise (`check.rs:40-41` vs `:151`). Any new caller that passes `asset.checksum` directly into `verify_checksum` — the natural reading of the type — will compare a URL against a digest. Renaming those fields to `checksum_url` / `signature_url` belongs in this phase, since the phase touches every one of their uses anyway.

### 4.3 · Existing updater tests — **there are none, anywhere**

- `src-tauri/tests/` **does not exist**.
- `#[cfg(test)]` appears in exactly 11 files in `src-tauri/src/`: `agent/builtin_tools/{build_evidence,e2e_probe,path_jail,path_lock}.rs`, `db/checkpoints.rs`, `observability/{error_kinds,log_retention,stray_files}.rs`, `subagent/{bridge,pool,slug}.rs`. **None** under `updater/` or `commands/`.
- The frontend has no test runner at all — `package.json` scripts are `dev`, `build`, `tauri`, `typecheck`, `lint`; there is no `test` script, no vitest/jest config, and no `*.test.*` / `*.spec.*` file under `src/`.

**Consequence for the phase, both ways:** no existing test asserts the current permissive behaviour, so nothing will break when it is made strict — *and* there is no harness to add to. Every `UNIT-PROVEN` in P0-A (P0-01 … P0-07) requires standing up the first updater test module from zero. Per house Rule 12, those tests should be written from the rule text, not from the patched implementation.

### 4.4 · `AUTODEV_UPDATE_URL` elsewhere — **read in exactly one place**

- `src-tauri/src/updater/check.rs:70` — the only read in the codebase. Every other hit is documentation (`09-DELIVERY.md`, `G3c`, `R3`, the MDP).
- No `.cargo/config.toml` or `tauri.conf.json` sets it (`tauri.conf.json` has no updater section at all — the Tauri built-in updater plugin is **not** in use; this is a hand-rolled updater).
- Sibling: `AUTODEV_UPDATE_PUBKEY` is read at `verify.rs:46` (runtime) and set at `.github/workflows/build-release.yml:34` (build-time job env) and referenced at `installers/wix/build-msi.ps1:133` — see C1.

---

## 5 · What the claims MISS

Ordered by consequence for P0-A. Each of these changes what the phase should build.

**M1 · The trust root is a runtime environment read, and the CI secret does nothing.** Covered in C1 above; repeated here because it rewrites P0-01. Fixing the constant alone leaves `std::env::var("AUTODEV_UPDATE_PUBKEY")` (`verify.rs:46`) in place and leaves the release pipeline believing it injects a key that never reaches the binary (`build.rs` is `tauri_build::build()` and nothing else).

**M2 · `.env` makes both env-var overrides locally injectable.** `src-tauri/src/main.rs:28` runs `let dotenv_loaded = dotenvy::dotenv().ok();` at startup, and the comment at `:26` states plainly *"dotenvy walks up from CWD looking for `.env`"*. So `AUTODEV_UPDATE_PUBKEY` (the verification key) and `AUTODEV_UPDATE_URL` (the update source) are settable by **any `.env` file in the working directory or any ancestor of it**. That downgrades C6's "anything that can set an env var" from a theoretical precondition to "drop a file next to the app". P0-01 and P0-06 should both refuse env overrides in release builds, not merely validate them.

**M3 · The release pipeline publishes no `.sig` asset at all — so P0-02 will block every update.** There is **zero** occurrence of `.sig` in `.github/workflows/build-release.yml`; the Windows/macOS/Linux upload steps push only `*.msi` / `*.dmg` / `*.AppImage` / `*.deb` and `*.sha256`. Consequently `PlatformAsset.signature` (`check.rs:136`) is never filled, `sig_url` is always `""`, and the Ed25519 branch (C2) has *never once executed in production*. Making an empty signature URL a hard failure — P0-02, exactly as specified — will therefore reject **100 % of releases** until the pipeline is taught to sign artifacts and upload `<artifact>.sig`. **P0-02 must land together with a signing step in `build-release.yml`, or the updater goes from insecure to inoperative.** This is a sequencing hazard of the same class the MDP already flags for P0-C.

**M4 · The platform matcher does not match the artifacts CI actually builds — the Windows updater cannot download anything today.** `platform_key_from_filename` (`check.rs:201-218`) matches `_x64.msi` or `_x64-setup.exe`, but `installers/wix/build-msi.ps1:221` names the artifact:

```powershell
$DestName      = "autodev_${Version}${ChannelSuffix}_x64_en-US.exe"
```

`autodev_1.0.0_x64_en-US.exe` contains neither pattern → `platform_key_from_filename` returns `None` → no `windows-x64` entry → `asset_for_current_platform()` returns `None` → `download_update` fails with *"No update available for this platform"* (`updater_commands.rs:69-71`). The same script's own header comment (`build-msi.ps1:10`) claims it outputs `autodev_<version>_x64_en-US.msi`, and the CI upload step filters `dist -Filter "*.msi"` — three different names for one artifact. Linux is half-broken the same way: the AppImage (`autodev_<v>_x86_64.AppImage`, `scripts/build-linux.sh:74`) *does* match `_x86_64.appimage`, but the `.deb` (`..._x86_64.deb`, `:90`) is matched only against `_amd64.deb` and is silently dropped. **A P0-A end-to-end smoke against a real release is impossible on Windows until this is fixed** — plan the gate around a local fixture server, or add the naming fix to the phase.

**M5 · `download_checksum` / `download_signature` never check HTTP status.** `download.rs:155` and `:168` call `.send().await?` and go straight to `.bytes()` / `.text()`. A 404 or 500 with an empty body is a *successful* `Result` carrying `""`, which C3 then treats as a passing checksum — no `unwrap_or_default()` involved. P0-04 as written does not close this; the fix belongs in `download_checksum` itself.

**M6 · Beta and Nightly channels can never return an update.** `check.rs:73-77`:

```rust
    let url = match channel {
        UpdateChannel::Stable  => url,
        UpdateChannel::Beta    => url.replace("releases/latest", "releases?per_page=5"),
        UpdateChannel::Nightly => url.replace("releases/latest", "releases?per_page=10"),
    };
```

For a custom `AUTODEV_UPDATE_URL` the `replace` is a silent no-op; for the GitHub default it produces a **list** endpoint whose response is a JSON *array*, so `release["tag_name"]` (`check.rs:99`) is `null` → `latest_tag` empty → `Ok(None)` at `:104-106`. Both non-stable channels are dead. (`main.rs:511` hardcodes `UpdateChannel::Stable` for the background check, so only the manual command reaches this.) Not a security hole, but it means any "channel" behaviour the phase assumes is fiction.

**M7 · Rollback / replay is unaddressed by the C1-C8 set.** `verify_signature` signs only the installer bytes — nothing binds a signature to a version, platform, or channel. Since the metadata itself is unauthenticated (C6/M2), an attacker who controls the metadata can advertise `tag_name: 99.0.0` while serving a genuinely signed *older* installer, and every check in this module passes. If the phase is rebuilding the trust root anyway, sign a manifest (version + platform + digest), not the raw file.

**M8 · A second TOCTOU that P0-05 will not cover.** `install.rs:60-88` writes `%TEMP%\autodev_update.bat` and registers it under `HKCU\…\RunOnce`; the batch executes **after** AutoDev exits. Re-verifying inside `schedule_install` (P0-05) still leaves the window between "batch written" and "batch runs as the user at next logon", during which any local process can rewrite either the batch or the installer at `~/.autodev/updates/`. The macOS path is the same shape (`install.rs:121-179`, a LaunchAgent plist pointing at a world-readable script), and Linux worse — `install.rs:243` writes to a fixed `/tmp/autodev-update.sh`, a classic shared-tmp target.

**M9 · Resume logic trusts a pre-existing file it never identifies.** `download.rs:61-91`: if any file with the same derived name already exists in `~/.autodev/updates/`, its length is sent as a `Range` header and the response is **appended** to it. Nothing checks that those bytes belong to this asset. A stale or planted partial file is silently prepended to the downloaded installer — which only the checksum would catch, and the checksum is exactly what C3/C4/M5 make skippable. Whatever P0-03 does, the resume path needs either a same-asset marker or deletion-before-download.

**M10 · `install_update` never checks its `version` argument against `progress.version`** (detail under C5), unlike `download_update` (`updater_commands.rs:63-67`). The frontend-supplied string is what lands in the update history.

**M11 · `apply_bsdiff` allocates `vec![0u8; new_size]` from an attacker-controlled `u64`** (`patch.rs:69,83`) and its length guard `patch_body.len() < ctrl_len + diff_len` (`:74`) can be bypassed by `usize` overflow. Moot under P0-08's deletion — and it is the strongest argument *for* deleting rather than repairing.

---

## 6 · Bottom line for the phase

- **P0-01 is under-specified** — add "make the key compile-time and remove the runtime env override" (C1 / M1 / M2).
- **P0-02 is a release-breaking change without a signing pipeline** — it must ship with `.sig` generation and upload (M3).
- **P0-03 / P0-04 are correct but incomplete** — also fix the missing HTTP status checks in `download_checksum` (M5).
- **P0-05's stated gate is real** (C5 confirmed verbatim), but two further TOCTOUs sit past `schedule_install` (M8).
- **P0-06 must include redirect policy**, not just the initial scheme (C6).
- **P0-07 must also reject absolute paths and strip query/fragment**, and replace the dead `unwrap_or` fallback (C7).
- **P0-08 is safe**: delete `patch.rs`, remove `mod.rs:14`, fix `mod.rs:7`, and decide on the orphaned `use_delta` / `delta_*` plumbing (§4.1).
- **No test exists to break, and none exists to build on** (§4.3).
- **Nothing here has already been fixed.** All eight claims describe live code.
