fn main() {
    // The update-signing public key is baked into the binary at compile time by
    // `option_env!("ARCANA_UPDATE_PUBKEY")` in src/updater/verify.rs. Without
    // this line, changing the key would not trigger a rebuild and a stale trust
    // root would silently persist in the binary.
    println!("cargo:rerun-if-env-changed=ARCANA_UPDATE_PUBKEY");
    // The opt-out for a release build with no key is read the same way.
    println!("cargo:rerun-if-env-changed=ARCANA_ALLOW_NO_UPDATER");

    tauri_build::build()
}
