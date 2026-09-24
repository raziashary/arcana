// UTF-8-safe truncation — one helper, every site (P3-04).
//
// ── Why this file exists ─────────────────────────────────────────────────────
//
// Slicing a `&str` at a byte index that is not a character boundary **panics**.
// Four sites in this codebase did exactly that, on strings that routinely
// contain non-ASCII:
//
//   * `remember_this.rs:101`      `&fact[..120]`  (file removed in P3-39)
//   * `annotate_file_tool.rs:109` `&note[..80]`
//   * `bash.rs:162`               a *computed tail*, `output[tail_start..]`
//   * `bash.rs:415`               `&command[i..]`
//
// `bash.rs` is the one most likely to fire: rustc and cargo emit `┌ │ └` box
// characters in ordinary diagnostics, so any command that compiles something can
// produce a panic here.
//
// ── The correction this file also records ────────────────────────────────────
//
// The plan, spec `06 §5`, and this project's ADR log all stated that *"the
// correct implementation already exists at `validate.rs:846-853`"*. It does not
// — that range is pyright/mypy dispatch plumbing. The citation travelled through
// four documents without anyone opening the file (`ADR-LOG-RESTRUCT-0028`).
//
// What does exist is `validate.rs:945-957`, which truncates one character early
// and carries an `unwrap_or(MAX_CHECKER_BYTES)` fallback that would be the exact
// unsafe slice if it were reachable — and **two genuinely correct helpers no
// document mentions**, duplicated byte-for-byte at `memory/recall.rs:183-192`
// and `memory/summarizer.rs:148-157`. This module is those two, promoted to one
// place, so that the next site has somewhere to call instead of a fifth copy.

/// Truncate `s` to at most `max_bytes`, never splitting a character.
///
/// Returns the string unchanged when it already fits. Appends nothing — callers
/// that want an ellipsis or a "truncated" note use [`truncate_with_note`], so
/// that the marker is never counted against the caller's own budget by accident.
///
/// ⚠ `max_bytes` is a **byte** budget, not a character count, because every
/// caller here is bounding a payload sent to a provider or written to a
/// database — both of which measure bytes.
pub fn truncate_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Walk back to the nearest boundary. At most three steps: UTF-8 encodes a
    // character in four bytes or fewer.
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Truncate from the **end**, keeping the last `max_bytes` — for tails such as
/// the last N bytes of command output.
///
/// ⚠ This is the shape `bash.rs` needed and the reason a fix templated on
/// `&x[..N]` would have left it panicking: it slices a computed *start* index,
/// not a length, so walking backwards from it is wrong. Here the index walks
/// **forward** to the next boundary, because moving backwards would include a
/// partial character rather than exclude one.
pub fn truncate_bytes_from_end(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut start = s.len() - max_bytes;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

/// Truncate and append a marker, so the reader — human or model — can tell that
/// content is missing rather than inferring it.
///
/// The marker is appended *after* truncation and is not counted against
/// `max_bytes`: a caller bounding a 120-byte database column wants 120 bytes of
/// content plus a visible marker, not 117 plus a marker.
pub fn truncate_with_note(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    format!("{}…", truncate_bytes(s, max_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact inputs that panicked at the four live sites.
    const BOX_DRAWING: &str = "error[E0308]: mismatched types\n  ┌─ src/main.rs:4:9\n  │\n4 │     let x: u32 = \"hello\";\n  │            ───   ^^^^^^^ expected `u32`\n  └─ help: change the type";

    #[test]
    fn a_multibyte_character_is_never_split() {
        // Every byte offset into a string of 3-byte characters. Without the
        // boundary walk, two of every three panic.
        let s = "日本語のテキストです";
        for n in 0..=s.len() {
            let out = truncate_bytes(s, n);
            assert!(out.len() <= n);
            // The result must itself be valid UTF-8 that round-trips.
            assert_eq!(out, std::str::from_utf8(out.as_bytes()).unwrap());
        }
    }

    #[test]
    fn rustc_diagnostics_do_not_panic() {
        // The realistic case: `bash.rs` truncating compiler output. Box-drawing
        // characters are 3 bytes each and appear at unpredictable offsets.
        for n in 0..=BOX_DRAWING.len() {
            let _ = truncate_bytes(BOX_DRAWING, n);
            let _ = truncate_bytes_from_end(BOX_DRAWING, n);
        }
    }

    #[test]
    fn a_string_that_fits_is_returned_unchanged() {
        assert_eq!(truncate_bytes("hello", 5), "hello");
        assert_eq!(truncate_bytes("hello", 99), "hello");
        assert_eq!(
            truncate_with_note("hello", 5),
            "hello",
            "no marker when nothing was cut"
        );
    }

    #[test]
    fn truncating_from_the_end_keeps_the_end() {
        assert_eq!(truncate_bytes_from_end("abcdef", 3), "def");
        // And it moves FORWARD off a boundary, so the result is shorter than
        // the budget rather than containing a partial character.
        let s = "aé日"; // 1 + 2 + 3 bytes
        let out = truncate_bytes_from_end(s, 4);
        assert!(s.ends_with(out), "must be a genuine tail");
        assert!(out.len() <= 4);
        assert_eq!(
            out, "日",
            "walked forward past the split é rather than including half of it"
        );
    }

    #[test]
    fn a_budget_of_zero_yields_an_empty_string_rather_than_panicking() {
        // `end > 0` in the loop guard is what makes this terminate; without it
        // the walk underflows on a leading multi-byte character.
        assert_eq!(truncate_bytes("日本語", 0), "");
        assert_eq!(
            truncate_bytes("日本語", 1),
            "",
            "cannot fit even one character"
        );
        assert_eq!(truncate_bytes("日本語", 2), "");
        assert_eq!(truncate_bytes("日本語", 3), "日");
    }

    #[test]
    fn the_marker_is_not_charged_against_the_budget() {
        // A caller bounding a 120-byte column wants 120 bytes plus a marker.
        let s = "x".repeat(200);
        let out = truncate_with_note(&s, 120);
        assert!(out.starts_with(&"x".repeat(120)));
        assert!(out.ends_with('…'));
    }

    #[test]
    fn a_combining_sequence_is_not_split_mid_codepoint() {
        // Combining marks are separate codepoints, so a boundary between base
        // and mark is *valid* UTF-8 — this asserts no panic and valid output,
        // not grapheme integrity, which would need a segmentation crate and is
        // not what these call sites need.
        let s = "e\u{0301}e\u{0301}e\u{0301}";
        for n in 0..=s.len() {
            let out = truncate_bytes(s, n);
            assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        }
    }
}
