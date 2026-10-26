import { describe, expect, it } from 'vitest'
import type { TrafficSummary } from '../../shared/types'
import { filterTrafficByDevice, listTrafficDevices } from './device-filter'

const rows: TrafficSummary[] = [
  { id: 'local', startedAt: '2026-10-26T10:00:00Z', method: 'GET', url: 'https://local.test', host: 'local.test', status: 200, durationMs: 1, responseBytes: 1, contentType: null, mocked: false, truncated: false, origin: 'computer', deviceToken: null, deviceName: null, error: null },
  { id: 'phone-a', startedAt: '2026-10-26T10:02:00Z', method: 'GET', url: 'https://a.test', host: 'a.test', status: 200, durationMs: 1, responseBytes: 1, contentType: null, mocked: false, truncated: false, origin: 'mobile', deviceToken: 'a', deviceName: '测试手机', error: null },
  { id: 'phone-b', startedAt: '2026-10-26T10:01:00Z', method: 'GET', url: 'https://b.test', host: 'b.test', status: 200, durationMs: 1, responseBytes: 1, contentType: null, mocked: false, truncated: false, origin: 'mobile', deviceToken: 'b', deviceName: '备用手机', error: null },
  { id: 'replay-a', startedAt: '2026-10-26T10:03:00Z', method: 'GET', url: 'https://a.test/replay', host: 'a.test', status: 200, durationMs: 1, responseBytes: 1, contentType: null, mocked: false, truncated: false, origin: 'replay', deviceToken: 'a', deviceName: '测试手机', error: null }
]

describe('device traffic filtering', () => {
  it('keeps device replay with the originating device and local replay in the local tab', () => {
    expect(filterTrafficByDevice(rows, 'device:a').map((row) => row.id)).toEqual(['phone-a', 'replay-a'])
    expect(filterTrafficByDevice(rows, 'local').map((row) => row.id)).toEqual(['local'])
  })

  it('orders online devices before historical devices and then by latest traffic', () => {
    expect(listTrafficDevices(rows, [{ deviceToken: 'b', name: '备用手机', boundIp: '192.168.1.26', createdAt: '', lastSeenAt: '2026-10-26T10:04:00Z' }]))
      .toEqual([
        { token: 'b', name: '备用手机', online: true, count: 1 },
        { token: 'a', name: '测试手机', online: false, count: 2 }
      ])
  })
})
