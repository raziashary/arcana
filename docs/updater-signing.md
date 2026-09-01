# Update signing — key generation and CI setup

AutoDev refuses to install an update unless **both** a SHA-256 checksum and a
detached Ed25519 signature verify against the key compiled into the running
binary. Both are hard failures in every build profile
(`src-tauri/src/updater/verify.rs`).

That makes signing part of releasing. A release published without `.sig` and
`.sha256` assets is uninstallable by every client, and the release workflow
fails rather than publishing one.

> **This runbook is for the repository owner.** It involves a private key.
> Claude does not generate, read, handle, or enter the private half — the
> commands below are run by a human, and the resulting secrets are pasted into
> GitHub's own secrets UI.

---

## 0. Getting `openssl` on Windows

**You already have it.** Git for Windows ships OpenSSL; it is simply not on
PowerShell's `PATH`, which is why `openssl` alone reports
*"The term 'openssl' is not recognized"*.

It lives at:

```
C:\Program Files\Git\usr\bin\openssl.exe
```

Two ways to use it — pick either, they produce identical output:

- **Git Bash** — open "Git Bash" from the Start menu and `openssl` just works,
  along with the `|` pipelines below.
- **PowerShell** — call it by full path with `&`. PowerShell cannot run the
  `| tail -c 32 | base64` pipeline, so §1 gives a native equivalent.

To put it on PATH for the current PowerShell session only:

```powershell
$env:PATH = "C:\Program Files\Git\usr\bin;$env:PATH"
```

---

## 1. Generate the keypair (once)

Run this on a trusted machine. It produces a private key file that must **never**
be committed, shared, or pasted into a chat window.

### PowerShell (Windows)

```powershell
$ossl = "C:\Program Files\Git\usr\bin\openssl.exe"
& $ossl genpkey -algorithm ed25519 -out autodev-update-private.pem
& $ossl pkey -in autodev-update-private.pem -pubout -outform DER -out autodev-update-public.der
$bytes = Get-Content autodev-update-public.der -Encoding Byte
[Convert]::ToBase64String($bytes[-32..-1])
```

> `-Encoding Byte` is Windows PowerShell 5.1. On PowerShell 7+ use
> `Get-Content ... -AsByteStream`.

### Git Bash, macOS or Linux

```bash
openssl genpkey -algorithm ed25519 -out autodev-update-private.pem
openssl pkey -in autodev-update-private.pem -pubout -outform DER | tail -c 32 | base64
```

Either way the last command prints **one 44-character base64 line ending in
`=`**. That is the value of `AUTODEV_UPDATE_PUBKEY`.

An Ed25519 public key is 44 bytes in DER with a fixed 12-byte header; both forms
above take the trailing 32 raw bytes, which is what `verify.rs` expects. If the
line is not 44 characters, something took the wrong slice — do not proceed.

*(Both command sets were executed against a throwaway keypair before being
written here, and that keypair was destroyed. They produce a 44-character key
that decodes to exactly 32 bytes.)*

---

## 2. Set the repository secrets

In **Settings → Secrets and variables → Actions**, add two repository secrets:

| Secret | Value | Used by |
|---|---|---|
| `AUTODEV_UPDATE_PUBKEY` | the base64 line from step 1 | compiled into the binary; also used to verify signatures before publishing |
| `AUTODEV_UPDATE_PRIVKEY` | the **entire contents** of `autodev-update-private.pem`, including the `-----BEGIN`/`-----END` lines | the `sign-release` job only |

Then store `autodev-update-private.pem` somewhere durable and offline (a password
manager entry or an encrypted backup), and delete the copy in your working
directory. (`*.pem` is gitignored and `TG0-04` fails if a private key is ever
staged, but neither helps if the file simply sits on a shared machine.)

**If the private key is lost, no future release can be signed with the same
key**, and every already-installed client will refuse every subsequent update
until it is reinstalled by hand. Losing this key is not recoverable in software.

---

## 3. What happens automatically after that

- `build.rs` declares `cargo:rerun-if-env-changed=AUTODEV_UPDATE_PUBKEY`, so
  changing the key forces a rebuild rather than leaving a stale trust root
  baked into an incremental build.
- `verify.rs` reads the key with `option_env!` at **compile** time. There is
  deliberately no runtime override: `main.rs` loads `.env` files by walking up
  from the working directory, so a runtime lookup would let anyone who can drop
  a file next to the binary replace the update trust root.
- A **release** build with no key configured **fails to compile**. This is
  intentional — such a binary could only ever refuse updates, which a user
  reads as "updates are broken" rather than "this build was mis-produced".
- The `sign-release` job signs every installer, regenerates its `.sha256`, and
  then **verifies each signature against the public key** before uploading. A
  signature that does not verify fails the release instead of shipping one that
  every client rejects.

### Is CI's OpenSSL signature actually accepted by the app?

Yes, and it is tested rather than assumed. CI signs with
`openssl pkeyutl -sign -rawin`; the app verifies with `ed25519-dalek`. Both
implement RFC 8032, but if they had disagreed, every release would have been
signed by CI and refused by every client with nothing failing until a user tried
to update.

`updater::verify::tests::a_signature_made_by_openssl_verifies_here` pins a fixed
OpenSSL-produced signature and asserts the shipped verifier accepts it — and
rejects the same signature over altered bytes.

---

## 4. Rotating the key

Rotation is a breaking change for clients that have not yet updated: a client
running an old binary embeds the old public key and will refuse anything signed
with the new private key.

The safe sequence is therefore:

1. Publish a release signed with the **old** key whose binary embeds the **new**
   public key. Every current client accepts this update, and afterwards embeds
   the new key.
2. Allow time for that release to roll out.
3. Only then start signing with the new private key.

Skipping step 1 strands every existing installation.

---

## 5. Verifying a downloaded artifact by hand

Useful when diagnosing a refused update.

### PowerShell (Windows)

```powershell
$ossl = "C:\Program Files\Git\usr\bin\openssl.exe"
& $ossl pkey -in autodev-update-private.pem -pubout -out autodev-update-public.pem
& $ossl pkeyutl -verify -pubin -inkey autodev-update-public.pem -rawin `
    -in autodev_x.y.z_x64_en-US.exe -sigfile autodev_x.y.z_x64_en-US.exe.sig

# Checksum: compare against the .sha256 sidecar
(Get-FileHash autodev_x.y.z_x64_en-US.exe -Algorithm SHA256).Hash.ToLower()
Get-Content autodev_x.y.z_x64_en-US.exe.sha256
```

### Git Bash, macOS or Linux

```bash
openssl pkey -in autodev-update-private.pem -pubout -out autodev-update-public.pem
openssl pkeyutl -verify -pubin -inkey autodev-update-public.pem -rawin \
    -in autodev_x.y.z_x64_en-US.exe -sigfile autodev_x.y.z_x64_en-US.exe.sig
sha256sum -c autodev_x.y.z_x64_en-US.exe.sha256
```

`Signature Verified Successfully` is the only passing output. Anything else,
including `Signature Verification Failure`, means the artifact does not match
the signature — do not install it.

If you only have the public key (not the private one), reconstruct the PEM from
the base64 secret the same way CI does:

```bash
printf '302a300506032b6570032100' > pub.hex
printf '%s' "$AUTODEV_UPDATE_PUBKEY" | base64 -d | xxd -p -c 32 >> pub.hex
tr -d '\n' < pub.hex | xxd -r -p > pub.der
openssl pkey -pubin -inform DER -in pub.der -out autodev-update-public.pem
```

---

## Related

- `src-tauri/src/updater/verify.rs` — the verification itself, and its tests
- `.github/workflows/build-release.yml` — the `sign-release` job
- `docs/restructurization/development/ADR_LOG.md` — `ADR-LOG-RESTRUCT-0002`
  (compile-time trust root) and `ADR-LOG-RESTRUCT-0003` (why signing had to ship
  in the same phase as the hard failure)
