// Redaction — the port that was dropped (M-40).
//
// `G4b §A.14` ruled the Python redaction module PORT and called its
// `SENSITIVE_KEYS` list "the spec". The vault half of that verdict became
// ADR-03-01; the redaction half was lost, and `redact`/`scrub`/`sanitiz`
// appeared **zero times** across the whole plan and all ten specs. Meanwhile
// `G4:486` recorded unredacted tool arguments landing in procedural memory
// verbatim — which, now that memory is persistent, means permanently.
//
// Two Python redactors are ported here, not one. The task named only the first;
// porting half a verdict is the same error that lost redaction in the first
// place (`L-1`).
//
//   * `security/redaction.py` — `SENSITIVE_KEYS` (**22** entries, not the 20 the
//     task states) plus `redact_dict` / `redact_string` / `redact_args`.
//   * `logging/centralized_collector.py` — `PIIRedactor`: email, phone, SSN,
//     credit card, `sk-`/`pk-` API keys, bearer tokens, `password=` assignments.
//
// ## One deliberate deviation from the source
//
// Python's `redact_string` includes a blanket *"32 or more alphanumeric
// characters"* rule. That is defensible for a value pulled out of a dict, and it
// is **not** defensible for free text: a git commit SHA is 40 hex characters, a
// SHA-256 digest is 64, and base64 payloads and minified code are longer still.
// Applied to file contents on their way into a prompt it would replace the very
// code the agent needs to reason about with `[REDACTED_KEY]`, and the agent
// would then be wrong for reasons nobody could see.
//
// So the heuristic is kept for dictionary values (`redact_json`), where the
// source applies it, and excluded from free text (`redact_text`), which relies
// on explicit patterns instead — including `NAME=value` assignments, which is
// what actually catches a secret sitting in a `.env` file and which the Python
// version never needed, because it only ever saw values already split out of a
// dict.
//
// ## What free-text redaction does not catch, stated plainly
//
// `redact_text` catches a secret that has **assignment context**
// (`AWS_SECRET_ACCESS_KEY=…`, `api_key: …`) or a **recognisable shape**
// (`sk-…`, `ghp_…`, `AKIA…`, `Bearer …`). A bare high-entropy token sitting in
// prose with neither — `the value is wJalrXUtnFEMIK7MDENG…` — is **not**
// caught, and cannot be without reinstating the 32-character rule that would
// also destroy every commit SHA and digest.
//
// That is a deliberate trade, not an oversight. Where the cost of
// over-redaction is low, the stricter `redact_value_text` is used instead:
// structured fields, model `thinking`, and every string inside a JSON payload.
// Where it is high — file contents and agent output, which carry the code the
// agent is reasoning about — it is not.
//
// See MDP `§7 P1-20` and `M-40`.

use std::sync::LazyLock;

use regex::Regex;

/// The marker substituted for a redacted value.
pub const REDACTED: &str = "[REDACTED]";

/// Key fragments that make a value secret, matched case-insensitively as
/// substrings — `stripe_api_key` matches on `api_key`.
///
/// Ported from `security/redaction.py`. **22 entries.** The plan says 20; they
/// were counted, not trusted.
pub const SENSITIVE_KEYS: &[&str] = &[
    "api_key",
    "apikey",
    "api-key",
    "secret",
    "secret_key",
    "secretkey",
    "password",
    "pass",
    "pwd",
    "token",
    "access_token",
    "refresh_token",
    "auth_token",
    "authorization",
    "auth",
    "private_key",
    "privatekey",
    "client_secret",
    "clientsecret",
    "bearer",
    "credential",
    "credentials",
];

/// Whether a key name marks its value as sensitive.
pub fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SENSITIVE_KEYS.iter().any(|k| lower.contains(k))
}

struct Pattern {
    re: Regex,
    with: &'static str,
}

/// Patterns applied to free text.
///
/// **Order matters, and getting it wrong leaks.** The narrow, unambiguous token
/// rules run first. The assignment rule stops at whitespace, so on
/// `Authorization: Bearer eyJhbGci…` it would match `Authorization: Bearer`,
/// rewrite that to `Authorization=[REDACTED]`, and leave the actual token in the
/// text. A test asserts the token is gone rather than that something was
/// replaced — which is how that was caught.
static TEXT_PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    let sensitive = SENSITIVE_KEYS.join("|");
    vec![
        Pattern {
            re: Regex::new(r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]+=*").expect("bearer"),
            with: "Bearer [REDACTED]",
        },
        // Provider-style keys.
        Pattern {
            re: Regex::new(r"(?i)\b(?:sk|pk)-[A-Za-z0-9_\-]{20,}").expect("sk key"),
            with: "[REDACTED_KEY]",
        },
        Pattern {
            re: Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{20,}").expect("github token"),
            with: "[REDACTED_KEY]",
        },
        // AWS access key ids have a fixed, unmistakable shape.
        Pattern {
            re: Regex::new(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b").expect("aws key id"),
            with: "[REDACTED_KEY]",
        },
        // `SOMETHING_SECRET=value`, `api_key: value`, `password = "value"`.
        Pattern {
            re: Regex::new(&format!(
                r#"(?i)\b([A-Za-z0-9_\-\.]*(?:{sensitive})[A-Za-z0-9_\-\.]*)\s*[:=]\s*["']?([^\s"',;]+)"#
            ))
            .expect("assignment pattern"),
            with: "$1=[REDACTED]",
        },
        // ── PII, from PIIRedactor ────────────────────────────────────────────
        Pattern {
            re: Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").expect("email"),
            with: "[EMAIL]",
        },
        Pattern {
            re: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("ssn"),
            with: "[SSN]",
        },
        Pattern {
            re: Regex::new(r"\b\d{4}[-\s]\d{4}[-\s]\d{4}[-\s]\d{4}\b").expect("card"),
            with: "[CARD]",
        },
        // Phone last: its shape overlaps the card pattern, so the more specific
        // rule must already have run.
        Pattern {
            re: Regex::new(r"\b\d{3}[-.]\d{3}[-.]\d{4}\b").expect("phone"),
            with: "[PHONE]",
        },
    ]
});

/// The blanket long-token rule, applied only to dictionary values.
///
/// See the module header for why it is not used on free text.
static LONG_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z0-9_\-]{32,}\b").expect("long token"));

/// Redact secrets and PII from free text.
///
/// Safe to apply to file contents, tool output, prompts and log lines: it does
/// not touch long identifiers such as commit hashes.
pub fn redact_text(text: &str) -> String {
    let mut out = text.to_string();
    for p in TEXT_PATTERNS.iter() {
        out = p.re.replace_all(&out, p.with).into_owned();
    }
    out
}

/// Redact a value that came out of a structured field.
///
/// Everything `redact_text` does, plus the source's long-token heuristic.
pub fn redact_value_text(text: &str) -> String {
    let first = redact_text(text);
    LONG_TOKEN
        .replace_all(&first, "[REDACTED_KEY]")
        .into_owned()
}

/// Recursively redact a JSON value.
///
/// A value under a sensitive key is replaced wholesale; every other string is
/// pattern-redacted. Arrays and nested objects are walked.
pub fn redact_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                if is_sensitive_key(k) {
                    // Wholesale, whatever the shape: a secret nested under
                    // `credentials` is still a secret.
                    out.insert(k.clone(), serde_json::Value::String(REDACTED.to_string()));
                } else {
                    out.insert(k.clone(), redact_json(v));
                }
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(redact_json).collect())
        }
        serde_json::Value::String(s) => serde_json::Value::String(redact_value_text(s)),
        other => other.clone(),
    }
}

/// Redact tool arguments before they are logged or written to memory.
///
/// `G4:486` recorded these landing in procedural memory verbatim.
pub fn redact_args(args: &serde_json::Value) -> serde_json::Value {
    redact_json(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_ported_key_list_has_the_count_the_source_has() {
        // The plan says 20. The source has 22. Counted, not trusted — and
        // pinned here so a later edit that drops one is visible.
        assert_eq!(SENSITIVE_KEYS.len(), 22);
        let unique: std::collections::HashSet<_> = SENSITIVE_KEYS.iter().collect();
        assert_eq!(unique.len(), 22, "the list has duplicates");
    }

    #[test]
    fn keys_match_as_substrings_case_insensitively() {
        for k in [
            "api_key",
            "API_KEY",
            "stripe_api_key",
            "X-Authorization",
            "userPassword",
            "refresh_token",
        ] {
            assert!(is_sensitive_key(k), "{k} should be sensitive");
        }
        for k in ["username", "task_id", "file_path", "title"] {
            assert!(!is_sensitive_key(k), "{k} should not be sensitive");
        }
    }

    /// The S1-04 smoke case, as a unit test: a secret sitting in a file.
    #[test]
    fn an_env_style_secret_in_free_text_is_redacted() {
        let file = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\n\
                    AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEY\n\
                    REGION=us-east-1\n";
        let out = redact_text(file);

        assert!(
            !out.contains("wJalrXUtnFEMI"),
            "the secret survived redaction: {out}"
        );
        assert!(
            !out.contains("AKIAIOSFODNN7EXAMPLE"),
            "the access key id survived: {out}"
        );
        // Non-secret configuration must survive, or redaction breaks the agent.
        assert!(out.contains("us-east-1"), "the region was destroyed: {out}");
    }

    #[test]
    fn provider_key_shapes_are_redacted() {
        for secret in [
            "sk-abcdefghijklmnopqrstuvwxyz012345",
            "ghp_abcdefghijklmnopqrstuvwxyz01234567",
            "AKIAIOSFODNN7EXAMPLE",
        ] {
            let out = redact_text(&format!("the value is {secret} ok"));
            assert!(!out.contains(secret), "{secret} survived: {out}");
        }
    }

    #[test]
    fn pii_patterns_are_redacted() {
        let out = redact_text(
            "contact razi@airsfoundry.com or 555-123-4567, ssn 123-45-6789, card 4111 1111 1111 1111",
        );
        assert!(!out.contains("razi@airsfoundry.com"), "{out}");
        assert!(!out.contains("555-123-4567"), "{out}");
        assert!(!out.contains("123-45-6789"), "{out}");
        assert!(!out.contains("4111 1111 1111 1111"), "{out}");
    }

    /// A header line is where pattern order decides whether a secret leaks. The
    /// assignment rule stops at whitespace, so if it ran first it would consume
    /// `Authorization: Bearer` and leave the token behind.
    #[test]
    fn a_bearer_token_in_a_header_line_does_not_survive() {
        let out = redact_text("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        assert!(
            !out.contains("eyJhbGci"),
            "the token survived — check pattern order: {out}"
        );
    }

    /// The deliberate deviation from the Python source, asserted so it cannot be
    /// "fixed" back without someone reading why.
    #[test]
    fn free_text_does_not_destroy_long_identifiers() {
        let sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let commit = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f80912";
        let text = format!("fixed in {commit}, digest {sha}");
        let out = redact_text(&text);
        assert!(
            out.contains(commit) && out.contains(sha),
            "a commit SHA and a digest must survive free-text redaction, or the \
             agent loses the code it is reasoning about: {out}"
        );

        // But a value pulled from a structured field still gets the heuristic.
        assert!(!redact_value_text(sha).contains(sha));
    }

    #[test]
    fn a_sensitive_key_redacts_its_value_whatever_the_shape() {
        let out = redact_json(&json!({
            "api_key": "short",
            "credentials": { "user": "razi", "password": "hunter2" },
            "items": [ { "auth_token": "abc" } ],
            "title": "a normal task",
            "count": 7
        }));

        assert_eq!(out["api_key"], REDACTED);
        // Nested under a sensitive key: redacted wholesale, not walked into.
        assert_eq!(out["credentials"], REDACTED);
        // `items` is not sensitive, so it is walked; the key inside is.
        // (An earlier version of this test used `tokens` as the container —
        // which contains "token", so the array was redacted wholesale and the
        // index returned Null. The code was right; the test was not.)
        assert_eq!(out["items"][0]["auth_token"], REDACTED);
        // Everything else survives untouched.
        assert_eq!(out["title"], "a normal task");
        assert_eq!(out["count"], 7);
    }

    /// The stated limit, asserted so it is a known property rather than
    /// something discovered later in an incident.
    #[test]
    fn a_bare_token_in_prose_is_not_caught_by_free_text_redaction() {
        let bare = "wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEYZ1";
        let prose = format!("the value is {bare}");
        assert!(
            redact_text(&prose).contains(bare),
            "free-text redaction is not supposed to catch a context-free token —              if it now does, the long-token rule has crept back in and commit              SHAs are being destroyed"
        );
        // The stricter rule, used for structured fields, does catch it.
        assert!(!redact_value_text(&prose).contains(bare));
    }

    #[test]
    fn redaction_is_idempotent() {
        let once = redact_text("AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEY");
        let twice = redact_text(&once);
        assert_eq!(once, twice, "redacting twice must not change the result");
    }

    #[test]
    fn ordinary_text_is_left_alone() {
        // Over-redaction is a real failure mode: it silently removes the
        // context an agent needs.
        for s in [
            "fn main() { println!(\"hello\"); }",
            "SELECT id, title FROM tasks WHERE status = 'ready'",
            "the build failed at src/main.rs:42",
        ] {
            assert_eq!(redact_text(s), s, "text was altered: {s}");
        }
    }
}
