// Removing credentials from the process environment (`F-8`, P2 post-phase audit).
//
// ── The hole ─────────────────────────────────────────────────────────────────
//
// `main.rs` USED TO CALL `dotenvy::dotenv()` unconditionally, in the SHIPPED
// binary, to pick up two path variables (removed in `P8-64`, along with the
// load itself — nothing read them). The repo `.env` also declares
// `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY` and
// `AWS_SECRET_ACCESS_KEY` — so every one of those landed in the process
// environment of a running application.
//
// From there they are inherited by every child process the agent spawns,
// including `bash.rs`, which executes commands the MODEL chooses. A prompt
// injection reading `echo $ANTHROPIC_API_KEY` needed no vulnerability beyond
// this.
//
// It also contradicted two things this project had written down: the standing
// instruction that the dev `.env` is excluded at runtime, and `P2-32`'s own
// claim that "the dev `.env` is outside this, not an exception to it".
//
// ── What this does ───────────────────────────────────────────────────────────
//
// Every credential-shaped variable is removed from the process environment at
// start-up. The app never needed them there: BYOK reads keys from the OS
// credential store, and the live smoke reads `.env` directly from disk rather
// than from the environment.
//
// ⚠ With the `.env` load gone this is no longer cleaning up after ourselves.
// It now guards what the USER's shell exported before AutoDev started — the
// case it was always really for, and the reason it survives `P8-64`.

use crate::security::redaction::is_sensitive_key;

/// Variables the application genuinely needs to survive the scrub.
///
/// ⚠ **Empty, and deliberately kept.** It held two path variables read from
/// `.env`; `P8-64` deleted that load, and nothing read either name anywhere in
/// the tree. An empty allowlist is the correct state — every credential-shaped
/// variable is now removed without exception — and the constant stays so the
/// next variable the app genuinely needs has an obvious, reviewed place to go
/// rather than being added as a special case inside the loop.
const KEEP: &[&str] = &[];

/// Remove every credential-shaped variable from the process environment.
///
/// Returns the names removed, for the startup log. **Names only — never
/// values.**
pub fn scrub_process_env() -> Vec<String> {
    let mut removed = Vec::new();

    for (key, _value) in std::env::vars() {
        if KEEP.contains(&key.as_str()) {
            continue;
        }
        if looks_like_a_credential(&key) {
            std::env::remove_var(&key);
            removed.push(key);
        }
    }

    removed.sort();
    removed
}

/// Whether a variable name looks like it carries a secret.
///
/// Reuses the redaction rules so the two cannot drift: a name the log redactor
/// considers sensitive is one this refuses to leave in the environment.
/// Extended with the credential shapes that appear as env vars specifically —
/// `is_sensitive_key` was written for JSON field names.
pub fn looks_like_a_credential(key: &str) -> bool {
    if is_sensitive_key(key) {
        return true;
    }

    let k = key.to_ascii_uppercase();
    // Deliberately broad. A false positive removes a variable the app does not
    // use; a false negative leaves a live credential where a model-chosen shell
    // command can read it.
    k.ends_with("_KEY")
        || k.ends_with("_TOKEN")
        || k.ends_with("_SECRET")
        || k.ends_with("_PASSWORD")
        || k.ends_with("_CREDENTIALS")
        || k.contains("API_KEY")
        || k.contains("APIKEY")
        || k.contains("SECRET")
        || k.contains("PASSWORD")
        || k.contains("PASSWD")
        || k.starts_with("AWS_")
        || k.starts_with("AZURE_")
        || k.starts_with("GCP_")
        || k.contains("_PAT")
        || k == "GITHUB_TOKEN"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_key_in_the_repo_env_is_recognised() {
        // The exact names the repo `.env` declares. If one of these stopped
        // matching, a live key would sit in the environment of a shipped app.
        for k in [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "GOOGLE_API_KEY",
            "GEMINI_API_KEY",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
        ] {
            assert!(looks_like_a_credential(k), "{k} would survive the scrub");
        }
    }

    #[test]
    fn the_allowlist_is_empty_and_that_is_the_point() {
        // \U0001F534 This test used to assert that two path variables were kept.
        // `P8-64` removed the `.env` load they came from, and nothing in the
        // tree read either name — so the allowlist protected two variables
        // that no longer existed, which is an exemption with no subject.
        //
        // Asserted rather than deleted: an allowlist that silently regrows is
        // how a credential ends up exempt, and this makes adding one a
        // deliberate act that fails a test first.
        assert!(
            KEEP.is_empty(),
            "a variable was added to the scrub allowlist: {KEEP:?}. Anything \
             here survives in the environment of every child process the agent \
             spawns, including the shell it runs model-authored commands in."
        );
    }

    #[test]
    fn ordinary_variables_are_left_alone() {
        for k in ["PATH", "HOME", "USERPROFILE", "LANG", "TERM", "CARGO_HOME"] {
            assert!(!looks_like_a_credential(k), "{k} should not be scrubbed");
        }
    }

    #[test]
    fn the_scrub_actually_removes_a_planted_credential() {
        // Proves the function does the removal, not merely the classification —
        // a classifier nobody calls is the shape this whole audit kept finding.
        std::env::set_var("AUTODEV_TEST_FAKE_API_KEY", "sk-not-a-real-key");
        assert!(std::env::var("AUTODEV_TEST_FAKE_API_KEY").is_ok());

        let removed = scrub_process_env();

        assert!(
            std::env::var("AUTODEV_TEST_FAKE_API_KEY").is_err(),
            "the credential survived the scrub"
        );
        assert!(removed.iter().any(|k| k == "AUTODEV_TEST_FAKE_API_KEY"));
        // Names are returned so the startup log can say what went. Values must
        // never appear.
        assert!(!removed.iter().any(|k| k.contains("sk-not-a-real-key")));
    }
}
