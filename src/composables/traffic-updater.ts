import type { TrafficSummary } from '../shared/types'

export function applyTrafficUpdates(
  current: readonly TrafficSummary[],
  updates: readonly TrafficSummary[],
  limit: number
): TrafficSummary[] {
  const next = new Map(current.map((row) => [row.id, row]))
  for (const update of updates) {
    next.delete(update.id)
    next.set(update.id, update)
  }
  return [...next.values()].reverse().slice(0, limit)
}
