# Update signing — key generation and CI setup

AutoDev refuses to install an update unless **both** a SHA-256 checksum and a
detached Ed25519 signature verify against the key compiled into the running
binary. Both are hard failures in every build profile
(`src-tauri/src/updater/verify.rs`).

That makes signing part of releasing. A release published without `.sig` and
`.sha256` assets is uninstallable by every client, and the release workflow
fails rather than publishing one.

> **This runbook is for the repository owner.** It involves a private key.
> Claude does not generate, read, handle, or enter the private half — the two
> commands below are run by a human, and the resulting secrets are pasted into
> GitHub's own secrets UI.

---

## 1. Generate the keypair (once)

Run this on a trusted machine. It produces a private key file that must **never**
be committed, shared, or pasted into a chat window.

```bash
openssl genpkey -algorithm ed25519 -out autodev-update-private.pem
```

Derive the base64 public half that the application will embed. Ed25519 public
keys are 44 bytes in DER with a fixed 12-byte header; the application wants the
raw 32 bytes:

```bash
openssl pkey -in autodev-update-private.pem -pubout -outform DER | tail -c 32 | base64
```

That prints a single 44-character base64 line ending in `=`. That is the value of
`AUTODEV_UPDATE_PUBKEY`.

---

## 2. Set the repository secrets

In **Settings → Secrets and variables → Actions**, add two repository secrets:

| Secret | Value | Used by |
|---|---|---|
| `AUTODEV_UPDATE_PUBKEY` | the base64 line from step 1 | compiled into the binary; also used to verify signatures before publishing |
| `AUTODEV_UPDATE_PRIVKEY` | the **entire contents** of `autodev-update-private.pem`, including the `-----BEGIN`/`-----END` lines | the `sign-release` job only |

Then store `autodev-update-private.pem` somewhere durable and offline (a password
manager entry or an encrypted backup). **If it is lost, no future release can be
signed with the same key**, and every already-installed client will refuse every
subsequent update until it is reinstalled by hand. Losing this key is not
recoverable in software.

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

Useful when diagnosing a refused update:

```bash
openssl pkey -in autodev-update-private.pem -pubout -out autodev-update-public.pem
openssl pkeyutl -verify -pubin -inkey autodev-update-public.pem -rawin -in autodev_x.y.z_x64_en-US.exe -sigfile autodev_x.y.z_x64_en-US.exe.sig
sha256sum -c autodev_x.y.z_x64_en-US.exe.sha256
```

---

## Related

- `src-tauri/src/updater/verify.rs` — the verification itself, and its tests
- `.github/workflows/build-release.yml` — the `sign-release` job
- `docs/restructurization/development/ADR_LOG.md` — `ADR-LOG-RESTRUCT-0002`
  (compile-time trust root) and `ADR-LOG-RESTRUCT-0003` (why signing had to ship
  in the same phase as the hard failure)
