import { describe, expect, it } from 'vitest'
import { parseLogLine } from './logParser'

describe('parseLogLine', () => {
  it('parses Rust module targets containing namespace separators', () => {
    const line = parseLogLine('2026-09-28T10:12:13.123Z  INFO arcana_core::agent::engine: task started: id=42')

    expect(line).toEqual({
      raw: '2026-09-28T10:12:13.123Z  INFO arcana_core::agent::engine: task started: id=42',
      timestamp: '2026-09-28 10:12:13.123',
      level: 'INFO',
      module: 'arcana_core::agent::engine',
      message: 'task started: id=42',
    })
  })

  it('preserves unstructured and panic records instead of dropping them', () => {
    expect(parseLogLine('thread panicked [PANIC] details').level).toBe('PANIC')
    expect(parseLogLine('provider diagnostic payload').level).toBe('UNKNOWN')
  })
})
