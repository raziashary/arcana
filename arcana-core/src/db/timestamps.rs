// Shared database timestamp contract.
//
// New Rust and SQLite writes use fixed-millisecond UTC RFC 3339 with `Z`,
// e.g. `2026-09-29T09:32:17.418Z`. The parser also accepts two legacy shapes:
// offset-bearing RFC 3339 and SQLite's offset-less UTC values such as
// `2026-09-01 06:23:58`. Old values remain readable; new writers should use
// `format_utc_timestamp`, `now_rfc3339`, or `SQLITE_UTC_NOW`.
//
// The offset-less legacy form must be interpreted as UTC. That was measured
// rather than assumed, because the obvious guess about the separator is wrong:
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

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};

use crate::errors::{ArcanaError, Result};
use rusqlite::types::Type;

/// Legacy formats emitted by `datetime('now')` and `CURRENT_TIMESTAMP` before
/// P4-57, in the order they are tried.
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
        return Err(ArcanaError::Agent("empty timestamp column".to_string()));
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

    Err(ArcanaError::Agent(format!(
        "unrecognised timestamp {s:?}: not RFC 3339 and not a SQLite datetime"
    )))
}

/// Read and parse a timestamp column without converting malformed data into a
/// plausible epoch/current-time value.
pub fn parse_row_timestamp(
    row: &rusqlite::Row<'_>,
    column: usize,
) -> rusqlite::Result<DateTime<Utc>> {
    let raw: String = row.get(column)?;
    parse_db_timestamp(&raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })
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

/// The current time in Arcana's canonical fixed-millisecond UTC format.
pub fn now_rfc3339() -> String {
    #[cfg(test)]
    if let Some(instant) = crate::db::test_clock::current() {
        return format_utc_timestamp(instant);
    }
    format_utc_timestamp(Utc::now())
}

/// Format an instant in Arcana's canonical persisted form:
/// `YYYY-MM-DDTHH:mm:ss.sssZ`.
///
/// Keep this paired with [`SQLITE_UTC_NOW`] so Rust and SQLite writes have the
/// same UTC representation and precision.
pub fn format_utc_timestamp(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// SQLite expression for the canonical persisted UTC form. SQLite schemas and
/// direct SQL writers use this instead of `CURRENT_TIMESTAMP` or
/// `datetime('now')`.
pub const SQLITE_UTC_NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";

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
        assert!(s.ends_with('Z'), "canonical UTC suffix expected: {s}");
        assert_eq!(s.len(), 24, "fixed millisecond precision expected: {s}");
    }

    #[test]
    fn canonical_timestamps_round_trip_a_leap_year_across_offset_extremes() {
        use chrono::{FixedOffset, TimeZone};

        let offsets = [-43_200, 0, 50_400];
        for ordinal in 1..=366 {
            let date = chrono::NaiveDate::from_yo_opt(2024, ordinal).expect("day in leap year");
            for seconds in [0, 1, 43_199, 86_399] {
                let local = date.and_hms_opt(0, 0, 0).expect("midnight")
                    + chrono::Duration::seconds(seconds);
                for offset_seconds in offsets {
                    let zone = FixedOffset::east_opt(offset_seconds).expect("valid UTC offset");
                    let instant = zone
                        .from_local_datetime(&local)
                        .single()
                        .expect("fixed offset is unambiguous")
                        .with_timezone(&Utc);
                    let encoded = format_utc_timestamp(instant);
                    let decoded = parse_db_timestamp(&encoded).expect("canonical timestamp parses");
                    assert_eq!(decoded, instant, "offset={offset_seconds}, local={local}");
                }
            }
        }
    }

    #[test]
    fn canonical_timestamps_round_trip_northern_and_southern_dst_edges() {
        use chrono::{FixedOffset, TimeZone};

        // US 2024 spring-forward and fall-back wall-clock boundaries.
        let cases = [
            ("2024-03-10 01:59:59", -18_000),
            ("2024-03-10 03:00:00", -14_400),
            ("2024-11-03 01:59:59", -14_400),
            ("2024-11-03 01:00:00", -18_000),
            // Sydney 2024 end and start boundaries (southern hemisphere).
            ("2024-04-07 02:59:59", 39_600),
            ("2024-04-07 02:00:00", 36_000),
            ("2024-10-06 01:59:59", 36_000),
            ("2024-10-06 03:00:00", 39_600),
        ];
        for (local, offset_seconds) in cases {
            let naive = NaiveDateTime::parse_from_str(local, "%Y-%m-%d %H:%M:%S")
                .expect("valid transition wall-clock");
            let offset = FixedOffset::east_opt(offset_seconds).expect("valid offset");
            let instant = offset
                .from_local_datetime(&naive)
                .single()
                .expect("fixed offset is unambiguous")
                .with_timezone(&Utc);
            let encoded = format_utc_timestamp(instant);
            assert_eq!(parse_db_timestamp(&encoded).unwrap(), instant, "{local}");
        }
    }

    #[test]
    fn sqlite_and_rust_writers_emit_the_same_canonical_shape() {
        let connection = rusqlite::Connection::open_in_memory().expect("in-memory SQLite");
        let sqlite_value: String = connection
            .query_row(&format!("SELECT {SQLITE_UTC_NOW}"), [], |row| row.get(0))
            .expect("SQLite UTC formatter");
        let rust_value = now_rfc3339();
        for value in [&sqlite_value, &rust_value] {
            assert!(
                value.ends_with('Z'),
                "canonical UTC suffix expected: {value}"
            );
            assert_eq!(value.len(), 24, "millisecond precision expected: {value}");
            parse_db_timestamp(value).expect("canonical timestamp must parse");
        }
    }

    #[test]
    fn row_parser_reads_canonical_and_legacy_storage_values() {
        let connection = rusqlite::Connection::open_in_memory().expect("in-memory SQLite");
        for (value, expected) in [
            ("2026-09-29T01:23:45.123Z", "2026-09-29T01:23:45.123Z"),
            (
                "2026-09-29T01:23:45.123456+00:00",
                "2026-09-29T01:23:45.123456Z",
            ),
            ("2026-09-29 01:23:45", "2026-09-29T01:23:45Z"),
        ] {
            let parsed = connection
                .query_row("SELECT ?1", [value], |row| parse_row_timestamp(row, 0))
                .expect("timestamp row parses");
            assert_eq!(
                parsed.to_rfc3339_opts(SecondsFormat::AutoSi, true),
                expected
            );
        }
        assert!(connection
            .query_row("SELECT 'not-a-time'", [], |row| parse_row_timestamp(row, 0))
            .is_err());
    }
}
