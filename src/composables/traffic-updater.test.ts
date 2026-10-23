import { describe, expect, it } from 'vitest'
import type { TrafficSummary } from '../shared/types'
import { applyTrafficUpdates } from './traffic-updater'

function row(id: string, status: number | null): TrafficSummary {
  return {
    id,
    startedAt: `2026-10-23T10:00:${id.padStart(2, '0')}Z`,
    method: 'GET',
    url: `https://example.test/${id}`,
    host: 'example.test',
    status,
    durationMs: null,
    responseBytes: 0,
    contentType: null,
    mocked: false,
    truncated: false,
    error: null,
    sourceExchangeId: null,
    matchedMockRuleId: null
  }
}

describe('applyTrafficUpdates', () => {
  it('inserts new traffic first and replaces existing traffic without a reload', () => {
    const initial = [row('1', null), row('0', 200)]
    const updated = { ...row('1', 201), durationMs: 12 }
    const created = row('2', 200)

    const result = applyTrafficUpdates(initial, [updated, created], 500)

    expect(result.map((item) => item.id)).toEqual(['2', '1', '0'])
    expect(result[1]).toEqual(updated)
  })

  it('keeps only the newest configured number of rows during an event storm', () => {
    const updates = Array.from({ length: 1_000 }, (_, index) => row(String(index), 200))

    const result = applyTrafficUpdates([], updates, 500)

    expect(result).toHaveLength(500)
    expect(result[0].id).toBe('999')
    expect(result[499].id).toBe('500')
  })
})
