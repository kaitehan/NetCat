import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import { desktopApiKey, type DesktopApi } from './api/desktop'
import type { MockRule, ProxyStatus, TrafficDetail, TrafficSummary } from './shared/types'

const status: ProxyStatus = {
  state: 'stopped',
  bindAddress: null,
  port: 28086,
  lanIp: null,
  error: null
}

const row: TrafficSummary = {
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
}

describe('App', () => {
  it('starts the proxy with defaults and automatically enables the system proxy', async () => {
    const startProxy = vi.fn().mockResolvedValue({ ...status, state: 'running' })
    const enableSystemProxy = vi.fn()
    const api = {
      getProxyStatus: vi.fn().mockResolvedValue(status),
      startProxy,
      stopProxy: vi.fn(),
      setCaptureEnabled: vi.fn(),
      getCaStatus: vi.fn().mockResolvedValue({ generated: true, certificatePath: '/ca.crt', fingerprint: 'AA' }),
      generateCa: vi.fn(),
      exportCa: vi.fn(),
      enableSystemProxy,
      restoreSystemProxy: vi.fn(),
      isSystemProxyEnabled: vi.fn().mockResolvedValue(false),
      listTraffic: vi.fn().mockResolvedValue([row]),
      getTrafficDetail: vi.fn(),
      clearTraffic: vi.fn(),
      listMockRules: vi.fn().mockResolvedValue([]),
      saveMockRule: vi.fn(),
      deleteMockRule: vi.fn(),
      setMockEnabled: vi.fn(),
      exportHar: vi.fn(),
      copyCurl: vi.fn(),
      copyExchange: vi.fn(),
      replayExchange: vi.fn(),
      hasAcceptedDisclaimer: vi.fn().mockResolvedValue(true),
      acceptDisclaimer: vi.fn(),
      openDevtools: vi.fn(),
      listen: vi.fn().mockResolvedValue(() => undefined)
    } satisfies DesktopApi

    const { container } = render(App, {
      global: {
        provide: {
          [desktopApiKey as symbol]: api
        }
      }
    })

    const sidebar = container.querySelector('.sidebar')
    expect(sidebar).not.toBeNull()
    expect(sidebar?.closest('.app-body')).toBeNull()

    expect(await screen.findByRole('button', { name: '展开 api.example.com 分组' })).toBeVisible()
    expect(screen.queryByRole('button', { name: /GET.*api\.example\.com\/users/ })).not.toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: '启动代理' }))
    await waitFor(() => expect(startProxy).toHaveBeenCalledWith({
      port: 28086,
      allowLan: false,
      captureEnabled: true
    }))
    await waitFor(() => expect(enableSystemProxy).toHaveBeenCalledOnce())
  })

  it('moves proxy and certificate configuration into the settings page', async () => {
    const api = {
      getProxyStatus: vi.fn().mockResolvedValue(status),
      startProxy: vi.fn(),
      stopProxy: vi.fn(),
      setCaptureEnabled: vi.fn(),
      getCaStatus: vi.fn().mockResolvedValue({ generated: true, certificatePath: '/ca.crt', fingerprint: 'AA' }),
      generateCa: vi.fn(),
      exportCa: vi.fn(),
      enableSystemProxy: vi.fn(),
      restoreSystemProxy: vi.fn(),
      isSystemProxyEnabled: vi.fn().mockResolvedValue(false),
      listTraffic: vi.fn().mockResolvedValue([]),
      getTrafficDetail: vi.fn(),
      clearTraffic: vi.fn(),
      listMockRules: vi.fn().mockResolvedValue([]),
      saveMockRule: vi.fn(),
      deleteMockRule: vi.fn(),
      setMockEnabled: vi.fn(),
      exportHar: vi.fn(),
      copyCurl: vi.fn(),
      copyExchange: vi.fn(),
      replayExchange: vi.fn(),
      hasAcceptedDisclaimer: vi.fn().mockResolvedValue(true),
      acceptDisclaimer: vi.fn(),
      openDevtools: vi.fn(),
      listen: vi.fn().mockResolvedValue(() => undefined)
    } satisfies DesktopApi

    const { container } = render(App, {
      global: {
        provide: {
          [desktopApiKey as symbol]: api
        }
      }
    })

    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    expect(await screen.findByRole('heading', { name: '设置' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '启动代理' })).not.toBeInTheDocument()
    expect(container.querySelector('.app-shell')).toHaveClass('settings-active')
    expect(screen.getByRole('tab', { name: '代理配置' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('spinbutton', { name: '代理端口' })).toHaveValue(28086)
    expect(screen.getByRole('checkbox', { name: '系统代理' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '记录请求' })).toBeChecked()

    await fireEvent.click(screen.getByRole('tab', { name: '证书配置' }))
    expect(screen.getByText('本地 CA 已就绪')).toBeVisible()
  })

  it('hides a URL for the session and creates a focused Mock draft from its response', async () => {
    const detail: TrafficDetail = {
      ...row,
      requestHeaders: { accept: 'application/json' },
      responseHeaders: {
        'content-type': 'application/json',
        'content-length': '11',
        'content-encoding': 'gzip',
        'x-source': 'upstream'
      },
      requestBody: [],
      responseBody: [...new TextEncoder().encode('{"ok":true}')],
      requestBodyText: '',
      responseBodyText: '{"ok":true}',
      requestOriginalBytes: 0,
      responseOriginalBytes: 11
    }
    const existingRule: MockRule = {
      id: 'existing',
      name: 'existing',
      enabled: true,
      priority: 5,
      methods: ['GET'],
      matchKind: 'contains',
      urlPattern: '/old',
      status: 200,
      responseHeaders: {},
      responseBody: '',
      delayMs: 0,
      createdAt: '2026-10-23T09:00:00Z'
    }
    const api = {
      getProxyStatus: vi.fn().mockResolvedValue(status),
      startProxy: vi.fn(),
      stopProxy: vi.fn(),
      setCaptureEnabled: vi.fn(),
      getCaStatus: vi.fn().mockResolvedValue({ generated: true, certificatePath: '/ca.crt', fingerprint: 'AA' }),
      generateCa: vi.fn(),
      exportCa: vi.fn(),
      enableSystemProxy: vi.fn(),
      restoreSystemProxy: vi.fn(),
      isSystemProxyEnabled: vi.fn().mockResolvedValue(false),
      listTraffic: vi.fn().mockResolvedValue([row]),
      getTrafficDetail: vi.fn().mockResolvedValue(detail),
      clearTraffic: vi.fn(),
      listMockRules: vi.fn().mockResolvedValue([existingRule]),
      saveMockRule: vi.fn(),
      deleteMockRule: vi.fn(),
      setMockEnabled: vi.fn(),
      exportHar: vi.fn(),
      copyCurl: vi.fn(),
      copyExchange: vi.fn(),
      replayExchange: vi.fn(),
      hasAcceptedDisclaimer: vi.fn().mockResolvedValue(true),
      acceptDisclaimer: vi.fn(),
      openDevtools: vi.fn(),
      listen: vi.fn().mockResolvedValue(() => undefined)
    } satisfies DesktopApi

    render(App, {
      global: {
        provide: {
          [desktopApiKey as symbol]: api
        }
      }
    })

    const group = await screen.findByRole('button', { name: '展开 api.example.com 分组' })
    await fireEvent.click(group)
    let trafficRow = screen.getByRole('button', { name: /GET.*api\.example\.com\/users/ })
    await fireEvent.contextMenu(trafficRow, { clientX: 80, clientY: 90 })
    await fireEvent.click(await screen.findByRole('menuitem', { name: '屏蔽' }))
    await fireEvent.click(screen.getByRole('menuitem', { name: '屏蔽完整 URL' }))

    expect(screen.queryByRole('button', { name: /GET.*api\.example\.com\/users/ })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '已屏蔽 1' })).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '已屏蔽 1' }))
    trafficRow = screen.getByRole('button', { name: /GET.*api\.example\.com\/users/ })
    expect(trafficRow).toHaveTextContent('已屏蔽')

    await fireEvent.contextMenu(trafficRow, { clientX: 80, clientY: 90 })
    await fireEvent.click(await screen.findByRole('menuitem', { name: '替换内容' }))

    expect(screen.queryByRole('button', { name: '已屏蔽该 URL，点击撤销' })).not.toBeInTheDocument()
    expect(await screen.findByDisplayValue('替换响应内容 · GET /users')).toBeVisible()
    expect(screen.getByLabelText('URL 匹配')).toHaveValue(row.url)
    expect(screen.getByLabelText('优先级')).toHaveValue(6)
    expect(screen.getByRole('textbox', { name: '响应正文' })).toHaveValue('{"ok":true}')
    expect(screen.getByLabelText('响应头')).toHaveValue('content-type: application/json\nx-source: upstream')
    expect(screen.getByRole('textbox', { name: '响应正文' })).toHaveFocus()
  })

  it('opens the dark diagnostics panel beside the app and forwards diagnostic events to it', async () => {
    const handlers = new Map<string, (payload: unknown) => void>()
    const unlisten = vi.fn()
    const openDevtools = vi.fn().mockResolvedValue(undefined)
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const api = {
      getProxyStatus: vi.fn().mockResolvedValue(status),
      startProxy: vi.fn(),
      stopProxy: vi.fn(),
      setCaptureEnabled: vi.fn(),
      getCaStatus: vi.fn().mockResolvedValue({ generated: true, certificatePath: '/ca.crt', fingerprint: 'AA' }),
      generateCa: vi.fn(),
      exportCa: vi.fn(),
      enableSystemProxy: vi.fn(),
      restoreSystemProxy: vi.fn(),
      isSystemProxyEnabled: vi.fn().mockResolvedValue(false),
      listTraffic: vi.fn().mockResolvedValue([]),
      getTrafficDetail: vi.fn(),
      clearTraffic: vi.fn(),
      listMockRules: vi.fn().mockResolvedValue([]),
      saveMockRule: vi.fn(),
      deleteMockRule: vi.fn(),
      setMockEnabled: vi.fn(),
      exportHar: vi.fn(),
      copyCurl: vi.fn(),
      copyExchange: vi.fn(),
      replayExchange: vi.fn(),
      hasAcceptedDisclaimer: vi.fn().mockResolvedValue(true),
      acceptDisclaimer: vi.fn(),
      openDevtools,
      listen: vi.fn(async (event: string, handler: (payload: unknown) => void) => {
        handlers.set(event, handler)
        return unlisten
      })
    } satisfies DesktopApi

    const { unmount } = render(App, {
      global: {
        provide: {
          [desktopApiKey as symbol]: api
        }
      }
    })

    await waitFor(() => expect(handlers.has('diagnostic-log')).toBe(true))
    handlers.get('diagnostic-log')?.({
      timestamp: '2026-10-23T10:00:00Z',
      level: 'warn',
      scope: 'mock',
      event: 'mock.failed',
      message: 'Mock 响应生成失败',
      exchangeId: 'exchange-1',
      details: { ruleId: 'rule-1' }
    })
    expect(warn).toHaveBeenCalledWith(
      '[NetCat][mock] Mock 响应生成失败',
      {
        timestamp: '2026-10-23T10:00:00Z',
        event: 'mock.failed',
        exchangeId: 'exchange-1',
        ruleId: 'rule-1'
      }
    )

    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))
    await fireEvent.click(screen.getByRole('button', { name: '打开调试面板' }))
    expect(await screen.findByRole('complementary', { name: '调试面板' })).toBeVisible()
    expect(screen.getByRole('tab', { name: 'Console' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByText('Mock 响应生成失败')).toBeVisible()
    expect(openDevtools).not.toHaveBeenCalled()

    await fireEvent.click(screen.getByRole('button', { name: '关闭调试面板' }))
    expect(screen.queryByRole('complementary', { name: '调试面板' })).not.toBeInTheDocument()

    unmount()
    expect(unlisten).toHaveBeenCalled()
    warn.mockRestore()
  })
})
