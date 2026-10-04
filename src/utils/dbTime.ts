/**
 * Parse a timestamp written by SQLite into a `Date`.
 *
 * 🔴 **Why this exists (`P8-31`, extended by P4-57).** Legacy database rows
 * written by SQLite's `datetime('now')` or `CURRENT_TIMESTAMP` emit
 *
 *     2026-09-01 06:23:58
 *
 * — a **space** separator and **no offset**. Handing that to `new Date(...)`
 * makes V8 read it as **local time**, because the ECMAScript spec only
 * mandates UTC for the ISO form with a `T`. On a machine at UTC+5:30 that is
 * a **5.5-hour** error, and it lands in the direction that flatters: a task
 * assigned one minute ago renders as *"6 hours ago"*.
 *
 * The Rust half of this defect lied in the opposite direction —
 * `get_qc_queue` parsed the same string with an RFC-3339 parser, failed on
 * every row, and `.unwrap_or(0)`'d the failure into an age of zero, so the
 * stalled-review alert could never fire. One column, two readers, wrong both
 * ways.
 *
 * ⚠ **Never write `new Date(row.created_at)` directly.** The value is a
 * database string, not an ISO instant. `TG8-xx` asserts this behaviourally
 * rather than by grep, because the previous sweep for this defect class
 * (`P1-13`) searched for the string `parse_from_rfc3339` and missed the site
 * that did not contain it.
 */
export function parseDbTimestamp(raw: string | null | undefined): Date | null {
  if (!raw) return null
  const s = raw.trim()
  if (!s) return null

  // An offset-bearing value is unambiguous — trust it as-is. This covers
  // anything already written as RFC 3339 / ISO 8601.
  if (/[zZ]$/.test(s) || /[+-]\d{2}:?\d{2}$/.test(s)) {
    const d = new Date(s)
    return Number.isNaN(d.getTime()) ? null : d
  }

  // The SQLite form: "YYYY-MM-DD HH:MM:SS" with optional fractional seconds.
  // Normalise to the ISO shape and pin it to UTC, which is what SQLite wrote.
  const m = s.match(/^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}:\d{2})(\.\d+)?$/)
  if (m) {
    const d = new Date(`${m[1]}T${m[2]}${m[3] ?? ''}Z`)
    return Number.isNaN(d.getTime()) ? null : d
  }

  // Unrecognised. Return null rather than a plausible-looking wrong Date —
  // a caller can render "unknown", but it cannot detect a silently shifted one.
  return null
}

/**
 * Age in whole seconds of a database timestamp, or `null` if unparseable.
 *
 * Clamped at zero: a clock skew between the database host and the renderer
 * should read as "just now", never as a negative age.
 */
export function dbAgeSeconds(raw: string | null | undefined): number | null {
  const d = parseDbTimestamp(raw)
  if (!d) return null
  return Math.max(0, Math.floor((Date.now() - d.getTime()) / 1000))
}

/** Whether a database timestamp falls on the current local calendar day. */
export function isDbTimestampToday(
  raw: string | null | undefined,
  now: Date = new Date(),
): boolean {
  const d = parseDbTimestamp(raw)
  if (!d) return false
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  )
}

/** Format the elapsed time since a database timestamp as `MM:SS`. */
export function dbElapsedClock(
  raw: string | null | undefined,
  now: Date = new Date(),
): string | null {
  const d = parseDbTimestamp(raw)
  if (!d) return null
  const diffMs = now.getTime() - d.getTime()
  const diffMin = Math.floor(diffMs / 60_000)
  const diffSec = Math.floor((diffMs % 60_000) / 1_000)
  return `${String(diffMin).padStart(2, '0')}:${String(diffSec).padStart(2, '0')}`
}
