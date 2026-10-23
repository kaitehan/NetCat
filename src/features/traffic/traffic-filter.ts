import type { TrafficFilter, TrafficSummary } from '../../shared/types'

export function filterTraffic(
  rows: readonly TrafficSummary[],
  filter: TrafficFilter
): TrafficSummary[] {
  const query = filter.query.trim().toLocaleLowerCase()
  const host = filter.host.trim().toLocaleLowerCase()

  return rows.filter((row) => {
    const keywordMatches = !query
      || row.url.toLocaleLowerCase().includes(query)
      || row.host.toLocaleLowerCase().includes(query)
    const methodMatches = filter.method === 'ALL' || row.method === filter.method
    const hostMatches = !host || row.host.toLocaleLowerCase() === host
    const statusMatches = filter.status === 'ALL'
      || (filter.status === '2xx' && row.status !== null && row.status >= 200 && row.status < 300)
      || (filter.status === '3xx' && row.status !== null && row.status >= 300 && row.status < 400)
      || (filter.status === '4xx' && row.status !== null && row.status >= 400 && row.status < 500)
      || (filter.status === '5xx' && row.status !== null && row.status >= 500 && row.status < 600)
      || (filter.status === 'ERR' && row.error !== null)

    return keywordMatches && methodMatches && hostMatches && statusMatches
  })
}
