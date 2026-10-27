import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import { desktopApi, desktopApiKey, type DesktopApi } from './api/desktop'
import type { MockRule, ProxyStatus, TrafficDetail, TrafficSummary } from './shared/types'

const status: ProxyStatus = {
  state: 'stopped',
  mode: 'computer',
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

function platformApi() {
  return {
    getPlatformInfo: vi.fn().mockResolvedValue({ os: 'macos', version: '15', arch: 'arm64', certificateTrustSupported: false }),
    getSystemProxyStatus: vi.fn().mockResolvedValue({ managed: false, recoveryPending: false, conflict: false }),
    getCertificateTrustStatus: vi.fn().mockResolvedValue('unsupported'),
    getCertificateTrustDetails: vi.fn().mockResolvedValue({ publicTrust: 'unsupported', privateKeyState: 'unavailable' }),
    installCaTrust: vi.fn(),
    removeCaTrust: vi.fn(),
    getPairingIdleTimeout: vi.fn().mockResolvedValue(15),
    setPairingIdleTimeout: vi.fn().mockImplementation(async (minutes: number) => minutes),
    listPairedDevices: vi.fn().mockResolvedValue([]),
    createPairingChallenge: vi.fn(),
    renamePairedDevice: vi.fn(),
    disconnectPairedDevice: vi.fn(),
    deletePairedDeviceData: vi.fn()
  }
}

function windowsApi(): DesktopApi {
  return {
    ...desktopApi,
    getPlatformInfo: vi.fn().mockResolvedValue({ os: 'windows', version: '10.0.19045', arch: 'x86_64', certificateTrustSupported: true }),
    getProxyStatus: vi.fn().mockResolvedValue(status),
    getCaStatus: vi.fn().mockResolvedValue({ generated: true, certificatePath: 'C:\\NetCat\\ca.crt', fingerprint: 'AA' }),
    getSystemProxyStatus: vi.fn().mockResolvedValue({ managed: false, recoveryPending: false, conflict: false }),
    getCertificateTrustStatus: vi.fn().mockResolvedValue('notTrusted'),
    getCertificateTrustDetails: vi.fn().mockResolvedValue({ publicTrust: 'notTrusted', privateKeyState: 'ready' }),
    installCaTrust: vi.fn().mockResolvedValue('trusted'),
    removeCaTrust: vi.fn().mockResolvedValue('notTrusted'),
    listTraffic: vi.fn().mockResolvedValue([]),
    listPairedDevices: vi.fn().mockResolvedValue([]),
    listMockRules: vi.fn().mockResolvedValue([]),
    hasAcceptedDisclaimer: vi.fn().mockResolvedValue(true),
    getPetPreferences: vi.fn().mockResolvedValue({ version: 1, enabled: true, placement: null }),
    setPetEnabled: vi.fn().mockImplementation(async (enabled: boolean) => ({ version: 1, enabled, placement: null })),
    listen: vi.fn().mockResolvedValue(() => undefined)
  }
}

function renderWithApi(api: DesktopApi) {
  return render(App, { global: { provide: { [desktopApiKey as symbol]: api } } })
}

describe('App', () => {
  it('loads and updates the desktop pet preference from system settings', async () => {
    const api = windowsApi()
    renderWithApi(api)

    await fireEvent.click(await screen.findByRole('button', { name: '设置' }))
    await fireEvent.click(await screen.findByRole('tab', { name: '系统配置' }))
    const checkbox = screen.getByRole('checkbox', { name: '显示桌面小橘' })
    expect(checkbox).toBeChecked()

    await fireEvent.click(checkbox)
    await waitFor(() => expect(api.setPetEnabled).toHaveBeenCalledWith(false))
    expect(checkbox).not.toBeChecked()
  })

  it.each([
    ['getCaStatus', 'CA_KEY_MISMATCH'],
    ['getCaStatus', 'CA_KEY_UNAVAILABLE'],
    ['getCertificateTrustDetails', 'CA_KEY_MISMATCH'],
    ['getCertificateTrustDetails', 'CA_KEY_UNAVAILABLE']
  ] as const)('keeps core initialization and events when %s fails with %s', async (method, code) => {
    const api = windowsApi()
    const handlers = new Map<string, (payload: unknown) => void>()
    vi.mocked(api[method]).mockRejectedValue({ code, message: '本地 CA 不可用' })
    vi.mocked(api.getProxyStatus).mockResolvedValue({ ...status, state: 'running', port: 8888 })
    vi.mocked(api.listTraffic).mockResolvedValue([row])
    api.listen = vi.fn(async (event: string, handler: (payload: unknown) => void) => {
      handlers.set(event, handler)
      return () => handlers.delete(event)
    })
    renderWithApi(api)
    expect(await screen.findByText('本地 CA 不可用')).toBeVisible()
    expect(await screen.findByRole('button', { name: '停止代理' })).toBeVisible()
    expect(await screen.findByRole('button', { name: '展开 api.example.com 分组' })).toBeVisible()
    expect(screen.queryByRole('dialog', { name: /使用声明/ })).not.toBeInTheDocument()
    await waitFor(() => expect(handlers.has('diagnostic-log')).toBe(true))
    handlers.get('proxy-status-changed')?.(status)
    expect(await screen.findByRole('button', { name: '启动代理' })).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    expect(await screen.findByRole('spinbutton', { name: '代理端口' })).toHaveValue(8888)
    await fireEvent.click(screen.getByRole('tab', { name: '证书配置' }))
    expect(screen.getByRole('heading', { name: '本机 Windows' })).toBeVisible()
  })

  it.each(['missing', 'mismatch', 'unavailable'] as const)('allows explicit removal of a confirmed trusted certificate when the private key is %s', async (privateKey) => {
    const api = windowsApi()
    vi.mocked(api.getCertificateTrustDetails)
      .mockResolvedValueOnce({ publicTrust: 'trusted', privateKeyState: privateKey })
      .mockResolvedValue({ publicTrust: 'notTrusted', privateKeyState: privateKey })
    renderWithApi(api)
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    await fireEvent.click(await screen.findByRole('tab', { name: '证书配置' }))
    await fireEvent.click(await screen.findByRole('button', { name: '移除信任' }))
    await waitFor(() => expect(api.removeCaTrust).toHaveBeenCalledOnce())
    expect(await screen.findByText('当前 CA 尚未信任')).toBeVisible()
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '移除信任' })).not.toBeInTheDocument()
    expect(api.installCaTrust).not.toHaveBeenCalled()
  })

  it.each([false, true])('keeps the system proxy checkbox unchecked after an enable failure (conflict=%s)', async (conflict) => {
    const api = windowsApi()
    vi.mocked(api.getProxyStatus).mockResolvedValue({ ...status, state: 'running' })
    api.enableSystemProxy = vi.fn().mockRejectedValue({ code: 'SYSTEM_PROXY_ERROR', message: '系统代理启用失败' })
    vi.mocked(api.getSystemProxyStatus)
      .mockResolvedValueOnce({ managed: false, recoveryPending: false, conflict: false })
      .mockResolvedValue({ managed: false, recoveryPending: conflict, conflict })
    renderWithApi(api)
    await screen.findByRole('button', { name: '停止代理' })
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    const checkbox = await screen.findByRole('checkbox', { name: '系统代理' })
    expect(checkbox).not.toBeChecked()
    await fireEvent.click(checkbox)
    expect(await screen.findByText('系统代理启用失败')).toBeVisible()
    await waitFor(() => expect(api.getSystemProxyStatus).toHaveBeenCalledTimes(2))
    expect(checkbox).not.toBeChecked()
    if (conflict) {
      expect(checkbox).toBeDisabled()
      expect(screen.getByRole('button', { name: '重试恢复系统代理' })).toBeVisible()
    } else {
      expect(checkbox).toBeEnabled()
    }
  })

  it('keeps core state and live subscriptions when the system proxy status query fails', async () => {
    const api = windowsApi()
    const handlers = new Map<string, (payload: unknown) => void>()
    vi.mocked(api.getProxyStatus).mockResolvedValue({ ...status, state: 'running', port: 8888 })
    vi.mocked(api.listTraffic).mockResolvedValue([row])
    vi.mocked(api.getSystemProxyStatus).mockRejectedValue({ code: 'SYSTEM_PROXY_RESTORE_ERROR', message: '系统代理恢复记录损坏' })
    api.listen = vi.fn(async (event: string, handler: (payload: unknown) => void) => {
      handlers.set(event, handler)
      return () => undefined
    })
    renderWithApi(api)
    expect(await screen.findByText('系统代理恢复记录损坏')).toBeVisible()
    expect(await screen.findByRole('button', { name: '停止代理' })).toBeVisible()
    expect(await screen.findByRole('button', { name: '展开 api.example.com 分组' })).toBeVisible()
    expect(screen.queryByRole('dialog', { name: /使用声明/ })).not.toBeInTheDocument()
    await waitFor(() => expect(handlers.has('diagnostic-log')).toBe(true))
    handlers.get('proxy-status-changed')?.(status)
    expect(await screen.findByRole('button', { name: '启动代理' })).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    expect(await screen.findByRole('spinbutton', { name: '代理端口' })).toHaveValue(8888)
    await fireEvent.click(screen.getByRole('tab', { name: '证书配置' }))
    expect(screen.getByRole('heading', { name: '本机 Windows' })).toBeVisible()
    expect(screen.getByRole('button', { name: '安装信任' })).toBeEnabled()
  })

  it('loads Windows trust state and changes it only through explicit trust buttons', async () => {
    const api = windowsApi()
    renderWithApi(api)
    await waitFor(() => expect(api.getCertificateTrustDetails).toHaveBeenCalledOnce())
    expect(api.installCaTrust).not.toHaveBeenCalled()
    expect(api.removeCaTrust).not.toHaveBeenCalled()
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    await fireEvent.click(await screen.findByRole('tab', { name: '证书配置' }))
    await fireEvent.click(screen.getByRole('button', { name: '安装信任' }))
    expect(await screen.findByText('当前 CA 已信任')).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '移除信任' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '安装信任' })).toBeEnabled())
    expect(api.installCaTrust).toHaveBeenCalledOnce()
    expect(api.removeCaTrust).toHaveBeenCalledOnce()
  })

  it('rechecks trust after generating a CA without automatically installing trust', async () => {
    const api = windowsApi()
    vi.mocked(api.getCaStatus).mockResolvedValue({ generated: false, certificatePath: null, fingerprint: null })
    vi.mocked(api.getCertificateTrustDetails).mockResolvedValueOnce({ publicTrust: 'notGenerated', privateKeyState: 'missing' }).mockResolvedValue({ publicTrust: 'notTrusted', privateKeyState: 'ready' })
    api.generateCa = vi.fn().mockResolvedValue({ generated: true, certificatePath: 'C:\\NetCat\\ca.crt', fingerprint: 'AA' })
    renderWithApi(api)
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    await fireEvent.click(await screen.findByRole('tab', { name: '证书配置' }))
    await fireEvent.click(await screen.findByRole('button', { name: '生成证书' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '安装信任' })).toBeEnabled())
    expect(api.installCaTrust).not.toHaveBeenCalled()
  })

  it('recovers a missing private key after explicitly removing trust and regenerating the CA', async () => {
    const api = windowsApi()
    vi.mocked(api.getCertificateTrustDetails)
      .mockResolvedValueOnce({ publicTrust: 'trusted', privateKeyState: 'missing' })
      .mockResolvedValueOnce({ publicTrust: 'notTrusted', privateKeyState: 'missing' })
      .mockResolvedValue({ publicTrust: 'notTrusted', privateKeyState: 'ready' })
    api.generateCa = vi.fn().mockResolvedValue({ generated: true, certificatePath: 'C:\\NetCat\\ca.crt', fingerprint: 'CC' })
    renderWithApi(api)

    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    await fireEvent.click(await screen.findByRole('tab', { name: '证书配置' }))
    expect(screen.queryByRole('button', { name: '重新生成证书' })).not.toBeInTheDocument()

    await fireEvent.click(await screen.findByRole('button', { name: '移除信任' }))
    const regenerate = await screen.findByRole('button', { name: '重新生成证书' })
    await fireEvent.click(regenerate)

    await waitFor(() => expect(api.generateCa).toHaveBeenCalledOnce())
    expect(await screen.findByText('CA 私钥已就绪')).toBeVisible()
    expect(api.installCaTrust).not.toHaveBeenCalled()
  })

  it('shows pending proxy recovery and refreshes ownership after recovery', async () => {
    const api = windowsApi()
    vi.mocked(api.getSystemProxyStatus)
      .mockResolvedValueOnce({ managed: false, recoveryPending: true, conflict: true })
      .mockResolvedValue({ managed: false, recoveryPending: false, conflict: false })
    api.restoreSystemProxy = vi.fn().mockResolvedValue(undefined)
    renderWithApi(api)
    await waitFor(() => expect(api.getSystemProxyStatus).toHaveBeenCalledOnce())
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    const recovery = await screen.findByRole('button', { name: '重试恢复系统代理' })
    expect(screen.getByRole('checkbox', { name: '系统代理' })).not.toBeChecked()
    await fireEvent.click(recovery)
    await waitFor(() => expect(screen.queryByRole('button', { name: '重试恢复系统代理' })).not.toBeInTheDocument())
    expect(api.restoreSystemProxy).toHaveBeenCalledOnce()
  })

  it('refreshes proxy conflicts after a failed enable instead of showing the preference as enabled', async () => {
    const api = windowsApi()
    api.startProxy = vi.fn().mockResolvedValue({ ...status, state: 'running' })
    api.enableSystemProxy = vi.fn().mockRejectedValue({ code: 'SYSTEM_PROXY_CONFLICT', message: '系统代理冲突' })
    vi.mocked(api.getSystemProxyStatus)
      .mockResolvedValueOnce({ managed: false, recoveryPending: false, conflict: false })
      .mockResolvedValue({ managed: false, recoveryPending: true, conflict: true })
    renderWithApi(api)
    await waitFor(() => expect(api.getSystemProxyStatus).toHaveBeenCalledOnce())
    await fireEvent.click(screen.getByRole('button', { name: '启动代理' }))
    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    expect(await screen.findByRole('button', { name: '重试恢复系统代理' })).toBeVisible()
    expect(screen.getByRole('checkbox', { name: '系统代理' })).not.toBeChecked()
  })

  it('starts the proxy with defaults and automatically enables the system proxy', async () => {
    const startProxy = vi.fn().mockResolvedValue({ ...status, state: 'running' })
    const enableSystemProxy = vi.fn()
    const api = {
      ...desktopApi,
      ...platformApi(),
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
      mode: 'computer',
      captureEnabled: true
    }))
    await waitFor(() => expect(enableSystemProxy).toHaveBeenCalledOnce())
  })

  it('moves proxy and certificate configuration into the settings page', async () => {
    const api = {
      ...desktopApi,
      ...platformApi(),
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
    expect(container.querySelector('.toolbar')).not.toBeInTheDocument()
    expect(container.querySelector('.app-shell')).toHaveClass('toolbarless-active')
    expect(screen.getByRole('tab', { name: '代理配置' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('spinbutton', { name: '代理端口' })).toHaveValue(28086)
    expect(screen.getByRole('checkbox', { name: '系统代理' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '记录请求' })).toBeChecked()

    await fireEvent.click(screen.getByRole('tab', { name: '证书配置' }))
    expect(screen.getByText('本地 CA 已就绪')).toBeVisible()
  })

  it('confirms the device request count before deleting historical device data', async () => {
    const api = windowsApi()
    const deviceRow = {
      ...row,
      origin: 'mobile' as const,
      deviceToken: 'history-token',
      deviceName: '历史手机'
    }
    vi.mocked(api.getProxyStatus).mockResolvedValue({ ...status, mode: 'mobile' })
    vi.mocked(api.listTraffic).mockResolvedValueOnce([deviceRow]).mockResolvedValue([])
    api.deletePairedDeviceData = vi.fn().mockResolvedValue(1)
    const confirm = vi.spyOn(window, 'confirm').mockReturnValueOnce(false).mockReturnValueOnce(true)
    renderWithApi(api)

    await fireEvent.click(screen.getByRole('button', { name: '设置' }))
    const deleteButton = await screen.findByRole('button', { name: '删除设备数据' })
    await fireEvent.click(deleteButton)
    expect(confirm).toHaveBeenCalledWith('确认断开该设备并删除其 1 条请求吗？此操作无法撤销。')
    expect(api.deletePairedDeviceData).not.toHaveBeenCalled()

    await fireEvent.click(deleteButton)
    await waitFor(() => expect(api.deletePairedDeviceData).toHaveBeenCalledWith('history-token'))
    confirm.mockRestore()
  })

  it('opens About as a standalone app view without the business toolbar', async () => {
    const api = windowsApi()
    renderWithApi(api)

    expect(await screen.findByRole('button', { name: '启动代理' })).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '关于' }))

    expect(await screen.findByRole('heading', { name: '关于 NetCat' })).toBeVisible()
    expect(screen.getByText('奈特猫 · 本地守候每一次请求')).toBeVisible()
    expect(screen.queryByRole('button', { name: '启动代理' })).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: '设置' })).not.toBeInTheDocument()
  })

  it('opens Mock rules without the runtime toolbar and keeps the inspector column', async () => {
    const api = windowsApi()
    const { container } = renderWithApi(api)

    await fireEvent.click(screen.getByRole('button', { name: 'Mock 规则' }))

    expect(await screen.findByRole('heading', { name: 'Mock 规则' })).toBeVisible()
    expect(screen.getByRole('heading', { name: '规则执行顺序' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '启动代理' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '限流配置：无限流' })).not.toBeInTheDocument()
    expect(container.querySelector('.app-shell')).toHaveClass('toolbarless-active')
    expect(container.querySelector('.app-body')).not.toHaveClass('full-workspace-layout')
  })

  it.each([
    ['脚本', 'JavaScript 脚本']
  ])('opens %s in the full workspace layout', async (navigationLabel, heading) => {
    const api = windowsApi()
    const { container } = renderWithApi(api)

    expect(screen.queryByRole('button', { name: 'SSL Pinning' })).not.toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: navigationLabel }))

    expect(await screen.findByRole('heading', { name: heading })).toBeVisible()
    expect(screen.queryByRole('button', { name: '启动代理' })).not.toBeInTheDocument()
    expect(container.querySelector('.app-shell')).toHaveClass('toolbarless-active')
    expect(container.querySelector('.app-body')).toHaveClass('full-workspace-layout')
    expect(container.querySelector('.workspace')).toHaveClass('full-workspace')
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
      ...desktopApi,
      ...platformApi(),
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
      ...desktopApi,
      ...platformApi(),
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
    await fireEvent.click(await screen.findByRole('tab', { name: '系统配置' }))
    await fireEvent.click(await screen.findByRole('button', { name: '打开调试面板' }))
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

  it('opens and closes the support overlay without unmounting the workspace', async () => {
    localStorage.removeItem('netcat-support-conversations:v1')
    const { container } = renderWithApi(windowsApi())

    const trigger = await screen.findByRole('button', { name: '打开智奈助手' })
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByTestId('support-trigger-icon')).toHaveAttribute('src', '/src-tauri/icons/icon.png')
    expect(screen.getByTestId('support-trigger-icon')).toHaveAttribute('alt', '')
    expect(container.querySelector('.app-shell')).toBeInTheDocument()

    await fireEvent.click(trigger)
    expect(screen.getByRole('complementary', { name: '智奈助手' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '打开智奈助手' })).not.toBeInTheDocument()
    expect(container.querySelector('.app-shell')).toBeInTheDocument()

    await fireEvent.click(screen.getByRole('button', { name: '关闭智奈助手' }))
    expect(screen.queryByRole('complementary', { name: '智奈助手' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '打开智奈助手' })).toHaveAttribute('aria-expanded', 'false')
  })
})
