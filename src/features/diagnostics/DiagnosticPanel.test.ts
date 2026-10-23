import { fireEvent, render, screen, within } from '@testing-library/vue'
import { describe, expect, it, vi } from 'vitest'
import type { DiagnosticLog, DiagnosticTrafficDetail, TrafficSummary } from '../../shared/types'
import DiagnosticPanel from './DiagnosticPanel.vue'

const logs: DiagnosticLog[] = [
  {
    timestamp: '2026-10-23T10:00:00Z',
    level: 'info',
    scope: 'proxy',
    event: 'proxy.started',
    message: '代理已启动',
    details: { port: 28086 }
  },
  {
    timestamp: '2026-10-23T10:01:00Z',
    level: 'error',
    scope: 'replay',
    event: 'replay.failed',
    message: '请求重发失败',
    exchangeId: 'exchange-1'
  }
]

const row: TrafficSummary = {
  id: 'exchange-1',
  startedAt: '2026-10-23T10:01:00Z',
  method: 'POST',
  url: 'https://api.example.com/v1/orders',
  host: 'api.example.com',
  status: 502,
  durationMs: 1200,
  responseBytes: 84,
  contentType: 'application/json',
  mocked: false,
  truncated: false,
  error: '上游连接失败'
}

const detail: DiagnosticTrafficDetail = {
  ...row,
  requestHeaders: {
    authorization: '[REDACTED]',
    accept: 'application/json'
  },
  responseHeaders: {
    'content-type': 'application/json',
    'set-cookie': '[REDACTED]'
  },
  requestOriginalBytes: 16,
  responseOriginalBytes: 84
}

describe('DiagnosticPanel', () => {
  it('shows a dark Console and filters and clears logs', async () => {
    const { emitted } = render(DiagnosticPanel, {
      props: {
        logs,
        traffic: [row],
        detail: null,
        loadingDetail: false
      }
    })

    const panel = screen.getByRole('complementary', { name: '调试面板' })
    expect(panel).toHaveClass('diagnostic-panel--dark')
    const consoleContent = screen.getByRole('region', { name: 'Console 内容' })
    expect(within(consoleContent).getByRole('log', { name: '运行日志' })).toBeVisible()
    expect(screen.getByText('代理已启动')).toBeVisible()
    expect(screen.getByText('请求重发失败')).toBeVisible()

    await fireEvent.update(screen.getByRole('combobox', { name: '日志级别' }), 'error')
    expect(screen.queryByText('代理已启动')).not.toBeInTheDocument()
    expect(screen.getByText('请求重发失败')).toBeVisible()

    await fireEvent.update(screen.getByRole('combobox', { name: '日志级别' }), 'ALL')
    await fireEvent.update(screen.getByRole('combobox', { name: '日志范围' }), 'replay')
    expect(screen.queryByText('代理已启动')).not.toBeInTheDocument()
    expect(screen.getByText('请求重发失败')).toBeVisible()

    await fireEvent.update(screen.getByRole('combobox', { name: '日志范围' }), 'ALL')
    await fireEvent.update(screen.getByRole('searchbox', { name: '搜索日志' }), 'proxy.started')
    expect(screen.getByText('代理已启动')).toBeVisible()
    expect(screen.queryByText('请求重发失败')).not.toBeInTheDocument()

    await fireEvent.click(screen.getByRole('button', { name: '清空日志' }))
    expect(emitted().clearLogs).toHaveLength(1)
  })

  it('switches to Network and requests a sanitized local detail without sending traffic', async () => {
    const { emitted, rerender } = render(DiagnosticPanel, {
      props: {
        logs,
        traffic: [row],
        detail: null,
        loadingDetail: false
      }
    })

    await fireEvent.click(screen.getByRole('tab', { name: 'Network' }))
    await fireEvent.click(screen.getByRole('button', { name: /POST.*api\.example\.com\/v1\/orders/ }))
    expect(emitted().selectTraffic).toEqual([['exchange-1']])

    await rerender({ detail })
    expect(screen.getByText('authorization: [REDACTED]')).toBeVisible()
    expect(screen.getByText('set-cookie: [REDACTED]')).toBeVisible()
    expect(screen.getByText('请求正文 16 B')).toBeVisible()
    expect(screen.getByText('响应正文 84 B')).toBeVisible()
  })

  it('emits close and supports resizing from its left edge', async () => {
    const setPointerCapture = vi.fn()
    const { emitted } = render(DiagnosticPanel, {
      props: {
        logs,
        traffic: [row],
        detail: null,
        loadingDetail: false
      }
    })

    await fireEvent.click(screen.getByRole('button', { name: '关闭调试面板' }))
    expect(emitted().close).toHaveLength(1)

    const resizeHandle = screen.getByRole('separator', { name: '调整调试面板宽度' })
    Object.defineProperty(resizeHandle, 'setPointerCapture', { value: setPointerCapture })
    const pointerDown = new Event('pointerdown', { bubbles: true })
    Object.defineProperties(pointerDown, {
      pointerId: { value: 1 },
      clientX: { value: 700 }
    })
    const pointerMove = new Event('pointermove', { bubbles: true })
    Object.defineProperties(pointerMove, {
      pointerId: { value: 1 },
      clientX: { value: 600 }
    })
    await fireEvent(resizeHandle, pointerDown)
    await fireEvent(resizeHandle, pointerMove)
    expect(setPointerCapture).toHaveBeenCalledWith(1)
  })
})
