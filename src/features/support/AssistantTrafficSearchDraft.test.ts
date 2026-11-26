import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import AssistantTrafficSearchDraft from './AssistantTrafficSearchDraft.vue'

describe('AssistantTrafficSearchDraft', () => {
  it('shows redacted summaries and emits the filter only after confirmation', async () => {
    const { emitted } = render(AssistantTrafficSearchDraft, {
      props: {
        draft: {
          id: 'search-1',
          kind: 'trafficSearch',
          status: 'pending',
          payload: {
            filter: { query: 'users', domain: 'example.com', method: 'GET', statusGroup: 2 },
            total: 12,
            summaries: [{
              method: 'GET',
              url: 'https://api.example.com/users?token=%5BREDACTED%5D',
              status: 200,
              durationMs: 42,
              startedAt: '2026-11-25T10:00:00Z'
            }]
          }
        }
      }
    })

    expect(screen.getByText('找到 12 条请求')).toBeVisible()
    expect(screen.getByText('筛选条件')).toBeVisible()
    expect(screen.getByText('关键词：users')).toBeVisible()
    expect(screen.getByText('域名：example.com（含子域名）')).toBeVisible()
    expect(screen.getByText('方法：GET')).toBeVisible()
    expect(screen.getByText('状态：2xx')).toBeVisible()
    expect(screen.getByText(/api\.example\.com\/users/)).toBeVisible()
    expect(emitted().apply).toBeUndefined()
    await fireEvent.click(screen.getByRole('button', { name: '在网络请求中查看' }))
    expect(emitted().apply).toEqual([[
      'search-1',
      { query: 'users', domain: 'example.com', method: 'GET', statusGroup: 2 }
    ]])
  })
})
