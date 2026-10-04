import { describe, expect, it } from 'vitest'

import { formatDayBucketWeekday } from '../components/dashboard/TokenUsage'

describe('TokenUsage day bucket labels', () => {
  it('formats the weekday from the YYYY-MM-DD calendar date', () => {
    expect(formatDayBucketWeekday('2026-09-01')).toBe('Tue')
  })
})
