import { describe, expect, it } from 'vitest'
import { filterTraffic } from './traffic-filter'
import type { TrafficSummary } from '../../shared/types'

const rows: TrafficSummary[] = [
  {
    id: '1',
    startedAt: '2026-10-23T10:00:00Z',
    method: 'GET',
    url: 'https://api.example.com/users',
    host: 'api.example.com',
    status: 200,
    durationMs: 42,
    responseBytes: 512,
    contentType: 'application/json',
    mocked: false,
    truncated: false,
    error: null
  },
  {
    id: '2',
    startedAt: '2026-10-23T10:00:01Z',
    method: 'POST',
    url: 'https://auth.example.com/login',
    host: 'auth.example.com',
    status: 401,
    durationMs: 81,
    responseBytes: 128,
    contentType: 'application/json',
    mocked: true,
    truncated: false,
    error: null
  }
]

describe('filterTraffic', () => {
  it('combines keyword, method, status and host filters', () => {
    expect(filterTraffic(rows, {
      query: 'login',
      method: 'POST',
      status: '4xx',
      host: 'auth.example.com'
    })).toEqual([rows[1]])
  })

  it('matches keywords without case sensitivity', () => {
    expect(filterTraffic(rows, {
      query: 'API.EXAMPLE',
      method: 'ALL',
      status: 'ALL',
      host: ''
    })).toEqual([rows[0]])
  })
})
