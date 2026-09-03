// Reading timestamps back out of SQLite.
//
// Two formats live in these databases, and they are not interchangeable:
//
//   * **chrono-written**, e.g. `2026-05-14T09:32:17.418759+00:00`. Full
//     RFC 3339, with an offset. This is what `analytics` and `routing_history`
//     contain.
//   * **SQLite-written**, e.g. `2026-09-01 06:23:58`. Produced by
//     `datetime('now')` and by `DEFAULT CURRENT_TIMESTAMP`. Space separator,
//     **no offset**, always UTC.
//
// `DateTime::parse_from_rfc3339` handles the first and rejects the second. That
// was measured rather than assumed, because the obvious guess is wrong in an
// interesting way:
//
// ```text
//   2026-09-01 06:23:58            -> FAIL   (no offset)
//   2026-09-01T06:23:58            -> FAIL   (no offset, T separator)
//   2026-09-01 06:23:58+00:00      -> OK     (space separator IS accepted)
//   2026-09-01T06:23:58+00:00      -> OK
// ```
//
// So the separator is **not** the problem — the missing offset is. A fix aimed
// at the space would have changed nothing.
//
// What this cost: `orphan/mod.rs` read `tasks.updated_at` — written by
// `datetime('now')`, so always offset-less — with `parse_from_rfc3339(…).ok()?`.
// The parse failed every single time, and `.ok()?` turned that failure into
// *"this task is not an orphan"*. The TTL fallback that releases tasks claimed
// on another host, or claimed without a PID, could therefore never fire once.
// A parse error was indistinguishable from a healthy task.
//
// See MDP `§7 P1-13`.

use chrono::{DateTime, NaiveDateTime, Utc};

use crate::errors::{AutoDevError, Result};

/// Formats `datetime('now')` and `CURRENT_TIMESTAMP` produce, in the order
/// they are tried.
const SQLITE_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M:%S",    // datetime('now'), CURRENT_TIMESTAMP
    "%Y-%m-%d %H:%M:%S%.f", // with fractional seconds
    "%Y-%m-%dT%H:%M:%S",    // some paths use the T separator
    "%Y-%m-%dT%H:%M:%S%.f", //
];

/// Parse a timestamp column, whichever of the two formats wrote it.
///
/// An offset-less value is interpreted as UTC, which is what SQLite's
/// `datetime('now')` produces by definition.
///
/// Returns `Err` rather than `None`: a timestamp that cannot be read is a
/// defect in the data or the writer, and callers must not be able to mistake it
/// for a legitimate absence. That distinction is the entire reason this
/// function exists.
pub fn parse_db_timestamp(raw: &str) -> Result<DateTime<Utc>> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(AutoDevError::Agent("empty timestamp column".to_string()));
    }

    // The offset-bearing form first: it is unambiguous.
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    for fmt in SQLITE_FORMATS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(naive.and_utc());
        }
    }

    Err(AutoDevError::Agent(format!(
        "unrecognised timestamp {s:?}: not RFC 3339 and not a SQLite datetime"
    )))
}

/// How old a timestamp column is, in whole hours.
///
/// Negative when the value is in the future, which is left to the caller to
/// interpret — clock skew across hosts is real, and silently clamping it would
/// hide it.
pub fn age_hours(raw: &str) -> Result<i64> {
    let then = parse_db_timestamp(raw)?;
    Ok(Utc::now().signed_duration_since(then).num_hours())
}

/// The current time in the offset-bearing format, for writing from Rust.
///
/// Prefer this to `datetime('now')` in new code: a column that always carries
/// an offset can be read by `parse_from_rfc3339` alone.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {

    /// `P8-31` — the exact string SQLite writes, against both parsers.
    ///
    /// 🔴 `get_qc_queue` used `updated_at.parse::<DateTime<Utc>>()`, whose
    /// `FromStr` requires RFC 3339. `datetime('now')` emits a **space**
    /// separator and **no offset**, so the parse failed on every row and
    /// `.unwrap_or(0)` turned that into an age of **zero seconds** — every
    /// review looked brand new and the stalled-review alert could never fire.
    /// A silent wrong answer, not an error.
    ///
    /// ⚠ Asserts the CONTRAST, not just that the helper works: the second
    /// assertion is what proves the old call site was broken, and it is the
    /// reason this is a behavioural test rather than a grep. The previous
    /// sweep for this class (`P1-13`) searched for the string
    /// `parse_from_rfc3339`, which the broken line did not contain.
    #[test]
    fn p8_31_the_sqlite_form_parses_here_and_fails_the_rfc3339_parser() {
        let sqlite_wrote = "2026-09-01 06:23:58";

        let ours = parse_db_timestamp(sqlite_wrote).expect("the helper must accept it");
        // Asserted as the instant it denotes, not as an epoch integer I
        // computed by hand -- the first version of this line was off by
        // exactly two hours, which is the same class of error the test is
        // about.
        assert_eq!(ours.to_rfc3339(), "2026-09-01T06:23:58+00:00");

        // The call site's old expression, verbatim.
        let theirs = sqlite_wrote.parse::<DateTime<Utc>>();
        assert!(
            theirs.is_err(),
            "if this now succeeds the defect's mechanism has changed and the \
             comment above is stale"
        );
    }

    /// The age arithmetic the call site performs, end to end.
    #[test]
    fn p8_31_an_hour_old_row_reports_about_an_hour() {
        let an_hour_ago = Utc::now() - chrono::Duration::hours(1);
        let as_sqlite_writes_it = an_hour_ago.format("%Y-%m-%d %H:%M:%S").to_string();

        let parsed = parse_db_timestamp(&as_sqlite_writes_it).expect("parse");
        let age = Utc::now().timestamp() - parsed.timestamp();

        // Range, not equality — the clock moves between the two calls.
        assert!(
            (3_590..=3_610).contains(&age),
            "age was {age}s; the old code reported 0 here, forever"
        );
    }

    use super::*;

    #[test]
    fn a_sqlite_datetime_now_column_parses() {
        // The exact shape `datetime('now')` and `CURRENT_TIMESTAMP` produce,
        // and the one that `parse_from_rfc3339` rejects outright.
        let dt = parse_db_timestamp("2026-09-01 06:23:58").expect("must parse");
        assert_eq!(dt.to_rfc3339(), "2026-09-01T06:23:58+00:00");
    }

    #[test]
    fn a_chrono_written_column_parses() {
        let dt = parse_db_timestamp("2026-05-14T09:32:17.418759+00:00").expect("must parse");
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-05-14 09:32:17"
        );
    }

    /// The measurement this module is built on. If `parse_from_rfc3339` ever
    /// starts accepting offset-less input, this test tells us the workaround
    /// is no longer needed — and if it never did, the premise was wrong.
    #[test]
    fn parse_from_rfc3339_alone_cannot_read_a_sqlite_column() {
        assert!(
            DateTime::parse_from_rfc3339("2026-09-01 06:23:58").is_err(),
            "the defect this module exists for has disappeared; re-check P1-13"
        );
        // And the separator is not the reason — with an offset, a space is fine.
        assert!(DateTime::parse_from_rfc3339("2026-09-01 06:23:58+00:00").is_ok());
    }

    #[test]
    fn fractional_seconds_are_accepted_in_both_shapes() {
        assert!(parse_db_timestamp("2026-09-01 06:23:58.123").is_ok());
        assert!(parse_db_timestamp("2026-09-01T06:23:58.123456+00:00").is_ok());
    }

    #[test]
    fn an_unreadable_timestamp_is_an_error_not_a_silent_none() {
        // The whole point: `.ok()?` made a parse failure look like a healthy
        // task. An error that names the offending value cannot.
        let err = parse_db_timestamp("not a date").expect_err("must fail");
        assert!(err.to_string().contains("not a date"), "got: {err}");

        assert!(parse_db_timestamp("").is_err());
        assert!(parse_db_timestamp("   ").is_err());
    }

    #[test]
    fn age_is_measured_from_a_sqlite_column() {
        let past = (Utc::now() - chrono::Duration::hours(30))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let hours = age_hours(&past).expect("must parse");
        assert!(
            (29..=31).contains(&hours),
            "expected roughly 30 hours, got {hours}"
        );
    }

    #[test]
    fn a_future_timestamp_reports_negative_age_rather_than_zero() {
        let future = (Utc::now() + chrono::Duration::hours(5))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert!(age_hours(&future).expect("must parse") < 0);
    }

    #[test]
    fn now_rfc3339_round_trips_through_the_parser() {
        let s = now_rfc3339();
        assert!(parse_db_timestamp(&s).is_ok(), "produced {s}");
        assert!(
            DateTime::parse_from_rfc3339(&s).is_ok(),
            "what we write must be readable by the strict parser too"
        );
    }
}
