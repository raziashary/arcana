// Removing credentials from the process environment (`F-8`, P2 post-phase audit).
//
// ── The hole ─────────────────────────────────────────────────────────────────
//
// `main.rs` calls `dotenvy::dotenv()` unconditionally, in the SHIPPED binary,
// to pick up two sidecar path variables. The repo `.env` also declares
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
// The variables the app legitimately needs from `.env` are read first, then
// every credential-shaped variable is removed from the process environment. The
// app never needed them: BYOK reads keys from the OS credential store, and the
// live smoke reads `.env` directly from disk rather than from the environment.

use crate::security::redaction::is_sensitive_key;

/// Variables the application genuinely consumes from `.env`.
///
/// Anything not on this list that looks like a credential is removed. The list
/// is short on purpose — it is an allowlist, so a new secret added to `.env`
/// is scrubbed by default rather than by remembering to add it somewhere.
const KEEP: &[&str] = &["AUTODEV_SIDECAR_PATH", "AUTODEV_MEMORY_PATH"];

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
    fn the_paths_the_app_actually_needs_are_kept() {
        // These are why `.env` is loaded at all. Scrubbing them would break
        // sidecar path resolution, so they are an explicit allowlist rather
        // than an accident of the pattern not matching.
        for k in ["AUTODEV_SIDECAR_PATH", "AUTODEV_MEMORY_PATH"] {
            assert!(KEEP.contains(&k), "{k} must be kept");
        }
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
