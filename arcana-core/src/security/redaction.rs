// Redaction — the port that was dropped (M-40).
//
// `G4b §A.14` ruled the Python redaction module PORT and called its
// `SENSITIVE_KEYS` list "the spec". The vault half of that verdict became
// ADR-03-01; the redaction half was lost, and `redact`/`scrub`/`sanitiz`
// appeared **zero times** across the whole plan and all ten specs. Meanwhile
// `G4:486` recorded unredacted tool arguments landing in procedural memory
// verbatim — which, now that memory is persistent, means permanently.
//
// Two Python redactors are ported here, not one:
//
//   * `security/redaction.py` — `SENSITIVE_KEYS` (**22** entries, not the 20 the
//     task states) plus `redact_dict` / `redact_string` / `redact_args`.
//   * `logging/centralized_collector.py` — `PIIRedactor`: email, phone, SSN,
//     credit card, `sk-`/`pk-`/`api_key`-prefixed keys, bearer tokens, and
//     `password=` assignments.
//
// ## Key matching is segment-aware, not substring
//
// The Python source tests `any(sk in key_lower for sk in SENSITIVE_KEYS)` — a
// raw substring match. Ported literally, that redacts far more than it should,
// because three of the entries are short and common as fragments:
//
// ```text
//   author         contains "auth"    -> value replaced
//   passed         contains "pass"    -> value replaced
//   prompt_tokens  contains "token"   -> value replaced
// ```
//
// In a JSON payload that also changes the *type*: a numeric `prompt_tokens`
// became the string `"[REDACTED]"`. And in free text `let auth = build_auth(cfg);`
// became `let auth=[REDACTED];` in the transcript the model reads — silently
// removing the code it was asked to reason about.
//
// So a key is split into segments on `_`, `-`, `.` and camelCase boundaries, and
// matched by whole segment, with multi-word terms matched against the joined
// form. `auth` matches; `author` does not. `token` matches; `prompt_tokens` does
// not. The set of names treated as sensitive is unchanged — only the matching
// is corrected.
//
// ## One deliberate deviation from the source
//
// Python's `redact_string` includes a blanket *"32 or more alphanumeric
// characters"* rule. That is defensible for a value pulled out of a dict, and it
// is **not** defensible for free text: a git commit SHA is 40 hex characters, a
// SHA-256 digest is 64, and base64 payloads and minified code are longer still.
// Applied to file contents on their way into a prompt it would replace the very
// code the agent needs to reason about with `[REDACTED_KEY]`.
//
// So the heuristic is kept for structured values (`redact_value_text`,
// `redact_json`) and excluded from free text (`redact_text`).
//
// ## One narrowing, documented rather than silent
//
// `PIIRedactor`'s phone pattern is `\d{3}[-.]?\d{3}[-.]?\d{4}` — separators
// optional — so it also matches any bare ten-digit number. This tree is full of
// them: `1777963265` is a Unix timestamp, and checkpoint filenames are built
// from exactly that. Redacting timestamps out of diagnostics would be a
// regression, so **a separator is required here**. A bare ten-digit run is not
// redacted as a phone number. The card and `api_key`-prefix patterns, which the
// first port also narrowed, are restored in full — a sixteen-digit run really is
// card-shaped, and nothing else in this tree looks like it.
//
// ## What free-text redaction does not catch, stated plainly
//
// `redact_text` catches a secret with **assignment context**
// (`AWS_SECRET_ACCESS_KEY=…`, `"api_key": "…"`), a **recognisable shape**
// (`sk-…`, `ghp_…`, `AKIA…`), an **auth scheme** (`Bearer`, `Basic`, `Token`),
// or **URL credentials** (`postgres://user:pw@host`). A bare high-entropy token
// in prose with none of those — `the value is wJalrXUtnFEMIK7MDENG…` — is
// **not** caught, and cannot be without reinstating the 32-character rule that
// would also destroy every commit SHA.
//
// See MDP `§7 P1-20`/`P1-21`, `M-40`, and ADR-LOG-RESTRUCT-0019.

use std::sync::LazyLock;

use regex::Regex;

/// The marker substituted for a redacted value.
pub const REDACTED: &str = "[REDACTED]";

/// Key fragments that make a value secret.
///
/// Ported from `security/redaction.py`. **22 entries**, set-identical to the
/// source — the plan says 20; they were counted, not trusted. Matching is
/// segment-aware (see the module header), not raw substring.
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

/// Sensitive terms that are a single segment.
static STANDALONE: LazyLock<Vec<String>> = LazyLock::new(|| {
    SENSITIVE_KEYS
        .iter()
        .filter(|k| !k.contains('_') && !k.contains('-'))
        .map(|k| k.to_string())
        .collect()
});

/// Sensitive terms spanning more than one segment, with separators stripped —
/// `api_key` and `api-key` both become `apikey`.
///
/// Only terms that actually carry a separator qualify. A first version filtered
/// on length instead, which let the single word `token` through — and since the
/// test is against the *concatenated* segments, `prompt_tokens` joined to
/// `prompttokens`, matched, and a token count was redacted as a token.
static JOINED: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut v: Vec<String> = SENSITIVE_KEYS
        .iter()
        .filter(|k| k.contains('_') || k.contains('-'))
        .map(|k| k.replace(['_', '-'], ""))
        .collect();
    v.sort();
    v.dedup();
    v
});

/// Split an identifier into lowercase segments on `_`, `-`, `.` and camelCase.
fn segments(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in key.chars() {
        if ch == '_' || ch == '-' || ch == '.' || ch == ' ' || ch == '/' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = ch.is_lowercase() || ch.is_numeric();
        cur.push(ch.to_ascii_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Whether a key name marks its value as sensitive.
pub fn is_sensitive_key(key: &str) -> bool {
    let segs = segments(key);
    if segs.iter().any(|s| STANDALONE.contains(s)) {
        return true;
    }
    let joined: String = segs.concat();
    JOINED.iter().any(|t| joined.contains(t.as_str()))
}

struct Pattern {
    re: Regex,
    with: &'static str,
}

/// Patterns applied to free text.
///
/// **Order matters, and getting it wrong leaks.** The narrow, unambiguous rules
/// run first. The assignment rule is greedier, and if it ran first it would
/// consume `Authorization: Bearer` and leave the token behind — which is
/// exactly what happened once, and was caught only because the test asserted
/// the token was *gone* rather than that something had been replaced.
static TEXT_PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    vec![
        // `Bearer` is distinctive enough to match anywhere.
        Pattern {
            re: Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]+=*").expect("bearer"),
            with: "Bearer [REDACTED]",
        },
        // `Basic`, `Token` and `Digest` leaked by the identical mechanism, but
        // all three are ordinary English words — an unanchored rule redacted
        // "digest e3b0c44…" in a diagnostic. They are matched only in a header
        // context, where the word is unambiguous.
        Pattern {
            re: Regex::new(r"(?i)\b((?:proxy-)?authorization\s*:\s*\w+)\s+[A-Za-z0-9\-._~+/]+=*")
                .expect("auth header"),
            with: "$1 [REDACTED]",
        },
        // Credentials embedded in a URL: postgres://user:pw@host, https://u:p@h.
        Pattern {
            re: Regex::new(r"(?i)\b([a-z][a-z0-9+.\-]*://[^:@/\s]+):[^@/\s]+@")
                .expect("url credentials"),
            with: "$1:[REDACTED]@",
        },
        // Provider-style keys, including the `api_key`-prefixed form the first
        // port dropped.
        Pattern {
            re: Regex::new(r"(?i)\b(?:sk|pk)-[A-Za-z0-9_\-]{20,}").expect("sk key"),
            with: "[REDACTED_KEY]",
        },
        Pattern {
            re: Regex::new(r"(?i)\bapi[_-]?key[_-][A-Za-z0-9]{20,}").expect("api_key prefix"),
            with: "[REDACTED_KEY]",
        },
        Pattern {
            re: Regex::new(r"\bgh[pousr]_[A-Za-z0-9]{20,}").expect("github token"),
            with: "[REDACTED_KEY]",
        },
        Pattern {
            re: Regex::new(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b").expect("aws key id"),
            with: "[REDACTED_KEY]",
        },
        // The `NAME=value` rule is NOT here — it needs to consult
        // `is_sensitive_key`, which a plain regex substitution cannot do. See
        // `redact_assignments`.
        // ── PII, from PIIRedactor ────────────────────────────────────────────
        Pattern {
            re: Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").expect("email"),
            with: "[EMAIL]",
        },
        Pattern {
            re: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("ssn"),
            with: "[SSN]",
        },
        // Card: separators optional, restoring the source's breadth.
        Pattern {
            re: Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").expect("card"),
            with: "[CARD]",
        },
        // Phone: a separator is REQUIRED — see the module header. Runs last
        // because its shape overlaps the card pattern's.
        Pattern {
            re: Regex::new(r"\b\d{3}[-.]\d{3}[-.]\d{4}\b").expect("phone"),
            with: "[PHONE]",
        },
    ]
});

/// `key = value`, `"key": "value"`, `key: value` — any key, any value shape.
///
/// Deliberately broad: the decision about *which* keys matter is made in
/// `redact_assignments` by `is_sensitive_key`, so that free text and JSON keys
/// are judged by exactly the same rule. Encoding the key list into this regex
/// is what made `prompt_tokens=1523` look like a secret assignment.
static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    // `[ \t]*` and NOT `\s*` around the separator. With `\s*` the gap could
    // cross a newline, so in a `.env` file the harmless `config:` ending one
    // line matched, took the whole `AWS_SECRET_ACCESS_KEY=…` line below as its
    // value, found `config` not sensitive, returned the match unchanged — and
    // consumed the real assignment so it was never examined. The secret
    // survived. Caught by the memory-boundary test, not by reading.
    Regex::new(r#"(?i)\b([A-Za-z0-9_\-\.]+)"?[ \t]*[:=][ \t]*("[^"\n]*"|'[^'\n]*'|[^\s,;\n]+)"#)
        .expect("assignment")
});

/// Whether a captured value is source code rather than a secret.
///
/// `let auth = build_auth(cfg);` is an assignment to a sensitive-looking name,
/// and the literal rule rewrote it to `let auth=[REDACTED];` in the transcript
/// the model reads — deleting the code it had been asked to reason about.
/// Parentheses and braces are the discriminator: a credential does not contain
/// them, and a call or a block always does.
fn looks_like_code(value: &str) -> bool {
    value.contains('(')
        || value.contains(')')
        || value.contains('{')
        || value.contains('}')
        || value.contains("::")
}

/// Redact the value of any assignment whose key is sensitive.
fn redact_assignments(text: &str) -> String {
    ASSIGNMENT
        .replace_all(text, |caps: &regex::Captures<'_>| {
            let whole = caps.get(0).map(|m| m.as_str()).unwrap_or_default();
            let key = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let value = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            if is_sensitive_key(key) && !looks_like_code(value) {
                format!("{key}={REDACTED}")
            } else {
                whole.to_string()
            }
        })
        .into_owned()
}

/// The blanket long-token rule, applied only to structured values.
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
    // Last: the narrow, unambiguous rules above have already consumed the
    // shapes they own, so what reaches here is a genuine `key = value`.
    redact_assignments(&out)
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
pub fn redact_args(args: &serde_json::Value) -> serde_json::Value {
    redact_json(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SECRET: &str = "wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEYZ1";

    #[test]
    fn the_ported_key_list_has_the_count_the_source_has() {
        assert_eq!(SENSITIVE_KEYS.len(), 22);
        let unique: std::collections::HashSet<_> = SENSITIVE_KEYS.iter().collect();
        assert_eq!(unique.len(), 22, "the list has duplicates");
    }

    #[test]
    fn sensitive_keys_are_matched_by_segment() {
        for k in [
            "api_key",
            "API_KEY",
            "apiKey",
            "stripe_api_key",
            "X-Authorization",
            "userPassword",
            "refresh_token",
            "auth",
            "pwd",
            "client_secret",
        ] {
            assert!(is_sensitive_key(k), "{k} should be sensitive");
        }
    }

    /// The over-redaction the literal port caused. Each of these contains a
    /// sensitive term as a *fragment* and must not be redacted.
    #[test]
    fn common_words_containing_a_sensitive_fragment_are_not_redacted() {
        for k in [
            "author",        // contains "auth"
            "authored_by",   // contains "auth"
            "passed",        // contains "pass"
            "passing",       // contains "pass"
            "prompt_tokens", // contains "token"
            "tokens_used",   // contains "token"
            "completion_tokens",
            "username",
            "task_id",
            "file_path",
        ] {
            assert!(!is_sensitive_key(k), "{k} must NOT be treated as sensitive");
        }
    }

    /// The type-corruption half of the same defect.
    #[test]
    fn a_numeric_field_whose_name_merely_contains_token_stays_numeric() {
        let out = redact_json(&json!({
            "prompt_tokens": 1523,
            "completion_tokens": 88,
            "author": "razi",
            "cached": true
        }));
        assert_eq!(out["prompt_tokens"], 1523, "a token count is not a token");
        assert_eq!(out["completion_tokens"], 88);
        assert_eq!(out["author"], "razi");
        assert_eq!(out["cached"], true);
    }

    #[test]
    fn code_mentioning_auth_survives_free_text_redaction() {
        // This became `let auth=[REDACTED];` under the literal port, silently
        // removing the code the agent was asked to reason about.
        let code = "let auth = build_auth(cfg);";
        assert_eq!(redact_text(code), code);
    }

    #[test]
    fn an_env_style_secret_in_free_text_is_redacted() {
        let file = format!(
            "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\nAWS_SECRET_ACCESS_KEY={SECRET}\nREGION=us-east-1\n"
        );
        let out = redact_text(&file);
        assert!(!out.contains(SECRET), "{out}");
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"), "{out}");
        assert!(out.contains("us-east-1"), "region destroyed: {out}");
    }

    /// A-02: `Basic` and `Token` leaked by the identical mechanism `Bearer`
    /// was fixed for.
    #[test]
    fn every_auth_scheme_is_redacted_not_just_bearer() {
        for scheme in ["Bearer", "Basic", "Token", "Digest"] {
            let line = format!("Authorization: {scheme} eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
            let out = redact_text(&line);
            assert!(!out.contains("eyJhbGci"), "{scheme} token survived: {out}");
        }
    }

    /// A-03: raw JSON in a tool result was not matched at all.
    #[test]
    fn a_secret_inside_raw_json_text_is_redacted() {
        let blob = format!(r#"{{"model":"gpt-4","api_key":"{SECRET}","stream":true}}"#);
        let out = redact_text(&blob);
        assert!(!out.contains(SECRET), "{out}");
        assert!(out.contains("gpt-4"), "non-secret fields destroyed: {out}");
    }

    /// A-04: a quoted value containing a space was only half-redacted.
    #[test]
    fn a_quoted_value_containing_spaces_is_fully_redacted() {
        let out = redact_text(r#"password = "hunter 2 with spaces""#);
        assert!(!out.contains("hunter"), "{out}");
        assert!(!out.contains("spaces"), "the tail survived: {out}");
    }

    /// A-05: connection-string credentials.
    #[test]
    fn credentials_in_a_url_are_redacted() {
        for url in [
            "postgres://admin:hunter2@localhost/db",
            "https://user:s3cr3t@example.com/path",
            "redis://default:pw123456@127.0.0.1:6379",
        ] {
            let out = redact_text(url);
            assert!(!out.contains("hunter2"), "{out}");
            assert!(!out.contains("s3cr3t"), "{out}");
            assert!(!out.contains("pw123456"), "{out}");
        }
    }

    #[test]
    fn provider_key_shapes_are_redacted() {
        for secret in [
            "sk-abcdefghijklmnopqrstuvwxyz012345",
            "ghp_abcdefghijklmnopqrstuvwxyz01234567",
            "AKIAIOSFODNN7EXAMPLE",
            "api_key_abcdefghijklmnopqrstuvwxyz01",
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

    /// The card pattern is restored to the source's breadth: separators
    /// optional, so a bare sixteen-digit run is caught.
    #[test]
    fn a_bare_card_number_is_redacted() {
        let out = redact_text("card 4111111111111111 on file");
        assert!(!out.contains("4111111111111111"), "{out}");
    }

    /// The documented narrowing. A bare ten-digit run is a Unix timestamp far
    /// more often than a phone number in this tree, and checkpoint filenames
    /// are built from exactly that.
    #[test]
    fn a_bare_ten_digit_number_is_left_alone_because_it_is_a_timestamp() {
        let out = redact_text("checkpoint 1777963265 written");
        assert!(
            out.contains("1777963265"),
            "a Unix timestamp must survive: {out}"
        );
        // With a separator it is treated as a phone number.
        assert!(!redact_text("call 555.123.4567").contains("555.123.4567"));
    }

    #[test]
    fn free_text_does_not_destroy_long_identifiers() {
        let sha = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let commit = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f80912";
        let out = redact_text(&format!("fixed in {commit}, digest {sha}"));
        assert!(out.contains(commit) && out.contains(sha), "{out}");
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
        assert_eq!(out["credentials"], REDACTED);
        assert_eq!(out["items"][0]["auth_token"], REDACTED);
        assert_eq!(out["title"], "a normal task");
        assert_eq!(out["count"], 7);
    }

    /// The newline-crossing regression, pinned.
    ///
    /// A harmless `key:` ending one line must not swallow the sensitive
    /// assignment on the next — which is exactly what `\s*` around the
    /// separator allowed.
    #[test]
    fn a_harmless_key_does_not_swallow_the_secret_on_the_following_line() {
        let file =
            format!("here is my config:\nAWS_SECRET_ACCESS_KEY={SECRET}\nREGION=us-east-1\n");
        let out = redact_text(&file);
        assert!(
            !out.contains(SECRET),
            "the secret survived because a preceding key consumed it:\n{out}"
        );
        assert!(out.contains("us-east-1"), "{out}");
    }

    #[test]
    fn redaction_is_idempotent() {
        for input in [
            format!("AWS_SECRET_ACCESS_KEY={SECRET}"),
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9".to_string(),
            "postgres://admin:hunter2@localhost/db".to_string(),
            r#"{"api_key":"sk-abcdefghijklmnopqrstuvwxyz012345"}"#.to_string(),
        ] {
            let once = redact_text(&input);
            let twice = redact_text(&once);
            assert_eq!(once, twice, "not idempotent for {input}");
        }
    }

    #[test]
    fn ordinary_text_is_left_alone() {
        for s in [
            "fn main() { println!(\"hello\"); }",
            "SELECT id, title FROM tasks WHERE status = 'ready'",
            "the build failed at src/main.rs:42",
            "let author = commit.author_name();",
            "assert_eq!(usage.prompt_tokens, 1523);",
        ] {
            assert_eq!(redact_text(s), s, "text was altered: {s}");
        }
    }

    #[test]
    fn a_bare_token_in_prose_is_not_caught_by_free_text_redaction() {
        let prose = format!("the value is {SECRET}");
        assert!(
            redact_text(&prose).contains(SECRET),
            "free-text redaction is not supposed to catch a context-free token — \
             if it now does, the long-token rule has crept back in"
        );
        assert!(!redact_value_text(&prose).contains(SECRET));
    }
}
