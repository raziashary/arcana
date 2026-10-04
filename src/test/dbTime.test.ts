import { describe, expect, it } from 'vitest'

import {
  dbAgeSeconds,
  dbElapsedClock,
  isDbTimestampToday,
  parseDbTimestamp,
} from '../utils/dbTime'

/**
 * `P8-31` — the frontend half of a defect that lied in both directions.
 *
 * SQLite writes `2026-09-01 06:23:58`: a space separator and no offset.
 * `new Date(...)` reads that as **local** time, so at UTC+5:30 a task assigned
 * one minute ago renders as "6 hours ago". The Rust half of the same column
 * failed its RFC-3339 parse and `.unwrap_or(0)`'d every age to zero.
 *
 * ⚠ These assert BEHAVIOUR, not the absence of a string. The previous sweep
 * for this defect class searched for `parse_from_rfc3339` and missed the site
 * that did not contain it.
 */
describe('parseDbTimestamp', () => {
  it('reads a SQLite timestamp as UTC, not local', () => {
    const d = parseDbTimestamp('2026-09-01 06:23:58')
    expect(d).not.toBeNull()
    // The assertion that catches the bug: the same wall-clock text must mean
    // the same instant regardless of the machine's zone.
    expect(d!.toISOString()).toBe('2026-09-01T06:23:58.000Z')
  })

  it('disagrees with a naive `new Date` wherever the host is not UTC', () => {
    // 🔴 This is the defect, stated as a comparison. On a UTC machine both
    // agree and the test still passes; anywhere else, the naive read is off by
    // exactly the host offset — which is why the bug survived review on a
    // UTC CI box and showed up on a developer's laptop.
    const raw = '2026-09-01 06:23:58'
    const correct = parseDbTimestamp(raw)!.getTime()
    const naive = new Date(raw).getTime()
    const offsetMs = new Date(raw.replace(' ', 'T') + 'Z').getTimezoneOffset() * 60_000
    expect(correct - naive).toBe(-offsetMs)
  })

  it('accepts fractional seconds', () => {
    const d = parseDbTimestamp('2026-09-01 06:23:58.123')
    expect(d!.toISOString()).toBe('2026-09-01T06:23:58.123Z')
  })

  it('passes an offset-bearing value straight through', () => {
    const d = parseDbTimestamp('2026-09-01T06:23:58+02:00')
    expect(d!.toISOString()).toBe('2026-09-01T04:23:58.000Z')
  })

  it('accepts the canonical RFC3339 UTC form', () => {
    const d = parseDbTimestamp('2026-09-01T06:23:58Z')
    expect(d!.toISOString()).toBe('2026-09-01T06:23:58.000Z')
  })

  it('returns null rather than a plausible wrong Date', () => {
    // A caller can render "unknown"; it cannot detect a silently shifted one.
    for (const bad of ['', '   ', 'not a date', '2026-13-45 99:99:99', null, undefined]) {
      expect(parseDbTimestamp(bad as string)).toBeNull()
    }
  })
})

describe('database timestamp calendar displays', () => {
  it('keeps elapsed time correct across local midnight for SQLite UTC values', () => {
    const beforeMidnight = new Date(2026, 8, 1, 23, 59, 59)
    const afterMidnight = new Date(2026, 8, 2, 0, 0, 1)
    const sqliteForm = beforeMidnight.toISOString().replace('T', ' ').slice(0, 19)

    expect(dbElapsedClock(sqliteForm, afterMidnight)).toBe('00:02')
    expect(isDbTimestampToday(sqliteForm, afterMidnight)).toBe(false)
  })

  it('includes a SQLite UTC timestamp from just after local midnight in today', () => {
    const afterMidnight = new Date(2026, 8, 2, 0, 0, 1)
    const justAfterMidnight = new Date(2026, 8, 2, 0, 0, 0)
    const sqliteForm = justAfterMidnight.toISOString().replace('T', ' ').slice(0, 19)

    expect(isDbTimestampToday(sqliteForm, afterMidnight)).toBe(true)
  })
})

describe('dbAgeSeconds', () => {
  it('reports a real age for a SQLite timestamp an hour old', () => {
    const anHourAgo = new Date(Date.now() - 3_600_000)
    const sqliteForm = anHourAgo.toISOString().replace('T', ' ').slice(0, 19)
    const age = dbAgeSeconds(sqliteForm)
    expect(age).not.toBeNull()
    // ⚠ Range, not equality: the clock moves between the two calls.
    expect(age!).toBeGreaterThan(3_590)
    expect(age!).toBeLessThan(3_610)
  })

  it('clamps a future timestamp to zero rather than reporting negative age', () => {
    const later = new Date(Date.now() + 60_000).toISOString().replace('T', ' ').slice(0, 19)
    expect(dbAgeSeconds(later)).toBe(0)
  })

  it('returns null for an unparseable value', () => {
    expect(dbAgeSeconds('nonsense')).toBeNull()
  })
})
