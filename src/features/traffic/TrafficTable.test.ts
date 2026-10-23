import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import TrafficTable from './TrafficTable.vue'
import type { TrafficSummary } from '../../shared/types'

const row: TrafficSummary = {
  id: 'traffic-1',
  startedAt: '2026-10-23T10:00:00Z',
  method: 'GET',
  url: 'https://api.example.com/users',
  host: 'api.example.com',
  status: 200,
  durationMs: 42,
  responseBytes: 512,
  contentType: 'application/json',
  mocked: true,
  truncated: false,
  error: null
}

const secondApiRow: TrafficSummary = {
  ...row,
  id: 'traffic-2',
  method: 'POST',
  url: 'https://api.example.com/session',
  mocked: false
}

const assetRow: TrafficSummary = {
  ...row,
  id: 'traffic-3',
  url: 'https://cdn.example.com/avatar.png',
  host: 'cdn.example.com',
  mocked: false
}

describe('TrafficTable', () => {
  it('marks mocked traffic and emits the selected row', async () => {
    const { emitted } = render(TrafficTable, {
      props: {
        rows: [row],
        selectedId: null
      }
    })

    await fireEvent.click(screen.getByRole('button', { name: '展开 api.example.com 分组' }))
    expect(screen.getByText('Mock')).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: /GET.*api\.example\.com\/users/ }))
    expect(emitted().select).toEqual([[row]])
  })

  it('groups traffic by domain collapsed by default and expands one domain independently', async () => {
    render(TrafficTable, {
      props: {
        rows: [row, secondApiRow, assetRow],
        selectedId: null
      }
    })

    const apiGroup = screen.getByRole('button', { name: '展开 api.example.com 分组' })
    expect(apiGroup).toHaveTextContent('2')
    expect(screen.queryByRole('button', { name: /GET.*api\.example\.com\/users/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /POST.*api\.example\.com\/session/ })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '展开 cdn.example.com 分组' })).toHaveTextContent('1')

    await fireEvent.click(apiGroup)

    expect(screen.getByRole('button', { name: /GET.*api\.example\.com\/users/ })).toBeVisible()
    expect(screen.getByRole('button', { name: /POST.*api\.example\.com\/session/ })).toBeVisible()
    expect(screen.queryByRole('button', { name: /GET.*cdn\.example\.com\/avatar\.png/ })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '收起 api.example.com 分组' })).toBeVisible()
  })

  it('emits context menu coordinates only for traffic rows', async () => {
    const { emitted } = render(TrafficTable, {
      props: {
        rows: [row],
        selectedId: null,
        hiddenIds: new Set<string>()
      }
    })

    const group = screen.getByRole('button', { name: '展开 api.example.com 分组' })
    await fireEvent.contextMenu(group, { clientX: 10, clientY: 20 })
    expect(emitted().contextmenu).toBeUndefined()

    await fireEvent.click(group)
    await fireEvent.contextMenu(screen.getByRole('button', { name: /GET.*users/ }), {
      clientX: 120,
      clientY: 80
    })
    expect(emitted().contextmenu).toEqual([[{ row, x: 120, y: 80 }]])
  })
})
