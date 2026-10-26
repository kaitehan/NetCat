import type { PairedDevice, TrafficSummary } from '../../shared/types'

export type DeviceScope = 'all' | 'local' | `device:${string}`

export interface TrafficDevice {
  token: string
  name: string
  online: boolean
  count: number
}

export function filterTrafficByDevice(
  rows: readonly TrafficSummary[],
  scope: DeviceScope
): TrafficSummary[] {
  if (scope === 'all') return [...rows]
  if (scope === 'local') return rows.filter((row) => !row.deviceToken)
  const token = scope.slice('device:'.length)
  return rows.filter((row) => row.deviceToken === token)
}

export function listTrafficDevices(
  rows: readonly TrafficSummary[],
  pairedDevices: readonly PairedDevice[]
): TrafficDevice[] {
  const online = new Map(pairedDevices.map((device) => [device.deviceToken, device]))
  const devices = new Map<string, TrafficDevice & { latest: string }>()
  for (const device of pairedDevices) {
    devices.set(device.deviceToken, {
      token: device.deviceToken,
      name: device.name,
      online: true,
      count: 0,
      latest: device.lastSeenAt
    })
  }
  for (const row of rows) {
    if (!row.deviceToken) continue
    const existing = devices.get(row.deviceToken)
    if (existing) {
      existing.count += 1
      if (row.startedAt > existing.latest) existing.latest = row.startedAt
    } else {
      devices.set(row.deviceToken, {
        token: row.deviceToken,
        name: row.deviceName || '历史设备',
        online: online.has(row.deviceToken),
        count: 1,
        latest: row.startedAt
      })
    }
  }
  return [...devices.values()]
    .sort((left, right) => Number(right.online) - Number(left.online) || right.latest.localeCompare(left.latest))
    .map(({ latest: _, ...device }) => device)
}
