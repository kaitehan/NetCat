import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import TrafficInspector from './TrafficInspector.vue'
import type { TrafficDetail } from '../../shared/types'

const detail: TrafficDetail = {
  id: '1',
  startedAt: '2026-10-23T10:00:00Z',
  method: 'GET',
  url: 'https://api.example.com/users',
  host: 'api.example.com',
  status: 200,
  durationMs: 42,
  responseBytes: 11,
  contentType: 'application/json',
  mocked: false,
  truncated: false,
  error: null,
  requestHeaders: { accept: 'application/json' },
  responseHeaders: { 'content-type': 'application/json' },
  requestBody: [],
  responseBody: [0x1f, 0x8b, 0x08, 0x00],
  requestBodyText: '',
  responseBodyText: '{"ok":true}',
  requestOriginalBytes: 0,
  responseOriginalBytes: 11
}

describe('TrafficInspector', () => {
  it('shows formatted response and emits copy cURL', async () => {
    const { emitted } = render(TrafficInspector, { props: { detail } })
    await fireEvent.click(screen.getByRole('tab', { name: '响应' }))
    expect(screen.getByText(/"ok": true/)).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '复制 cURL' }))
    expect(emitted().copyCurl).toEqual([[detail.id]])
  })
})
