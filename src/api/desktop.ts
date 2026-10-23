import type { InjectionKey } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { save } from '@tauri-apps/plugin-dialog'
import type {
  AppError,
  CaStatus,
  CertificateTrustState,
  DiagnosticLog,
  MockRule,
  MockRuleTestResult,
  PlatformInfo,
  ProxyConfig,
  ProxyStatus,
  ReplayMode,
  ReplayDraft,
  ReplayRequest,
  ReplayResult,
  SystemProxyStatus,
  TrafficCopyKind,
  TrafficDetail,
  TrafficSummary
} from '../shared/types'

export type DesktopEvent =
  | 'proxy-status-changed'
  | 'traffic-created'
  | 'traffic-updated'
  | 'storage-warning'
  | 'diagnostic-log'

export interface DesktopApi {
  getPlatformInfo(): Promise<PlatformInfo>
  getSystemProxyStatus(): Promise<SystemProxyStatus>
  getCertificateTrustStatus(): Promise<CertificateTrustState>
  installCaTrust(): Promise<CertificateTrustState>
  removeCaTrust(): Promise<CertificateTrustState>
  getProxyStatus(): Promise<ProxyStatus>
  startProxy(config: ProxyConfig): Promise<ProxyStatus>
  stopProxy(): Promise<ProxyStatus>
  setCaptureEnabled(enabled: boolean): Promise<void>
  getCaStatus(): Promise<CaStatus>
  generateCa(): Promise<CaStatus>
  exportCa(): Promise<void>
  enableSystemProxy(): Promise<void>
  restoreSystemProxy(): Promise<void>
  isSystemProxyEnabled(): Promise<boolean>
  listTraffic(): Promise<TrafficSummary[]>
  getTrafficDetail(id: string): Promise<TrafficDetail>
  clearTraffic(): Promise<void>
  checkDatabaseIntegrity?(): Promise<string>
  exportDiagnosticBundle?(logs: readonly DiagnosticLog[]): Promise<void>
  listMockRules(): Promise<MockRule[]>
  saveMockRule(rule: MockRule): Promise<MockRule>
  deleteMockRule(id: string): Promise<void>
  setMockEnabled(id: string, enabled: boolean): Promise<void>
  duplicateMockRule?(id: string): Promise<MockRule>
  reorderMockRules?(orderedIds: readonly string[]): Promise<void>
  testMockRules?(method: string, url: string): Promise<MockRuleTestResult>
  exportHar(): Promise<void>
  copyCurl(id: string): Promise<void>
  copyExchange(id: string, kind: TrafficCopyKind): Promise<void>
  replayExchange(id: string, mode: ReplayMode): Promise<ReplayResult>
  getReplayDraft?(id: string): Promise<ReplayDraft>
  sendReplay?(request: ReplayRequest): Promise<ReplayResult>
  hasAcceptedDisclaimer(): Promise<boolean>
  acceptDisclaimer(): Promise<void>
  openDevtools(): Promise<void>
  listen<T>(event: DesktopEvent, handler: (payload: T) => void): Promise<UnlistenFn>
}

export const desktopApiKey: InjectionKey<DesktopApi> = Symbol('desktop-api')

const browserRows: TrafficSummary[] = [
  {
    id: 'demo-1',
    startedAt: '2026-10-23T10:18:14Z',
    method: 'GET',
    url: 'https://api.staging.example.com/v1/users?page=1',
    host: 'api.staging.example.com',
    status: 200,
    durationMs: 86,
    responseBytes: 4280,
    contentType: 'application/json',
    mocked: false,
    truncated: false,
    error: null
  },
  {
    id: 'demo-2',
    startedAt: '2026-10-23T10:18:12Z',
    method: 'POST',
    url: 'https://api.staging.example.com/v1/session/refresh',
    host: 'api.staging.example.com',
    status: 200,
    durationMs: 124,
    responseBytes: 964,
    contentType: 'application/json',
    mocked: true,
    truncated: false,
    error: null
  },
  {
    id: 'demo-3',
    startedAt: '2026-10-23T10:18:09Z',
    method: 'GET',
    url: 'https://cdn.example.com/assets/avatar-42.png',
    host: 'cdn.example.com',
    status: 304,
    durationMs: 31,
    responseBytes: 0,
    contentType: 'image/png',
    mocked: false,
    truncated: false,
    error: null
  },
  {
    id: 'demo-4',
    startedAt: '2026-10-23T10:18:05Z',
    method: 'GET',
    url: 'https://api.staging.example.com/v1/notifications',
    host: 'api.staging.example.com',
    status: 502,
    durationMs: 2003,
    responseBytes: 82,
    contentType: 'application/json',
    mocked: false,
    truncated: false,
    error: '上游连接失败'
  }
]

const browserMocks: MockRule[] = [
  {
    id: 'mock-demo',
    name: '刷新令牌成功响应',
    enabled: true,
    priority: 10,
    methods: ['POST'],
    matchKind: 'contains',
    urlPattern: '/session/refresh',
    status: 200,
    responseHeaders: { 'Content-Type': 'application/json' },
    responseBody: '{"token":"local-mock-token"}',
    delayMs: 120,
    createdAt: '2026-10-23T10:00:00Z'
  }
]

let browserStatus: ProxyStatus = {
  state: 'stopped',
  bindAddress: null,
  port: 28086,
  lanIp: null,
  error: null
}

const browserApi: DesktopApi = {
  async getPlatformInfo() {
    return { os: 'macos', version: '浏览器预览', arch: 'unknown', certificateTrustSupported: false }
  },
  async getSystemProxyStatus() {
    return { managed: false, recoveryPending: false, conflict: false }
  },
  async getCertificateTrustStatus() {
    return 'unsupported'
  },
  async installCaTrust() {
    return 'unsupported'
  },
  async removeCaTrust() {
    return 'unsupported'
  },
  async getProxyStatus() {
    return browserStatus
  },
  async startProxy(config) {
    browserStatus = {
      state: 'running',
      bindAddress: config.allowLan ? '0.0.0.0' : '127.0.0.1',
      port: config.port,
      lanIp: config.allowLan ? '192.168.1.24' : null,
      error: null
    }
    return browserStatus
  },
  async stopProxy() {
    browserStatus = { ...browserStatus, state: 'stopped', bindAddress: null, lanIp: null }
    return browserStatus
  },
  async setCaptureEnabled() {},
  async getCaStatus() {
    return { generated: true, certificatePath: '/Users/demo/NetCat/netcat-ca.crt', fingerprint: '8A:31:4D:CC:12:9B' }
  },
  async generateCa() {
    return { generated: true, certificatePath: '/Users/demo/NetCat/netcat-ca.crt', fingerprint: '8A:31:4D:CC:12:9B' }
  },
  async exportCa() {},
  async enableSystemProxy() {},
  async restoreSystemProxy() {},
  async isSystemProxyEnabled() {
    return false
  },
  async listTraffic() {
    return browserRows
  },
  async getTrafficDetail(id) {
    const row = browserRows.find((candidate) => candidate.id === id) ?? browserRows[0]
    const body = row.mocked ? '{"token":"local-mock-token"}' : '{"items":[{"id":42,"name":"Lin"}],"next":null}'
    return {
      ...row,
      requestHeaders: {
        accept: 'application/json',
        'user-agent': 'NetCat Preview/1.0'
      },
      responseHeaders: {
        'content-type': 'application/json',
        'cache-control': 'no-store'
      },
      requestBody: [],
      responseBody: [...new TextEncoder().encode(body)],
      requestBodyText: '',
      responseBodyText: body,
      requestOriginalBytes: 0,
      responseOriginalBytes: body.length
    }
  },
  async clearTraffic() {
    browserRows.splice(0)
  },
  async checkDatabaseIntegrity() {
    return 'ok'
  },
  async exportDiagnosticBundle() {},
  async listMockRules() {
    return browserMocks
  },
  async saveMockRule(rule) {
    const saved = {
      ...rule,
      id: rule.id || crypto.randomUUID(),
      createdAt: rule.createdAt || new Date().toISOString()
    }
    const index = browserMocks.findIndex((candidate) => candidate.id === saved.id)
    if (index >= 0) {
      browserMocks[index] = saved
    } else {
      browserMocks.push(saved)
    }
    return saved
  },
  async deleteMockRule(id) {
    const index = browserMocks.findIndex((candidate) => candidate.id === id)
    if (index >= 0) {
      browserMocks.splice(index, 1)
    }
  },
  async setMockEnabled(id, enabled) {
    const rule = browserMocks.find((candidate) => candidate.id === id)
    if (rule) {
      rule.enabled = enabled
    }
  },
  async duplicateMockRule(id) {
    const source = browserMocks.find((candidate) => candidate.id === id)
    if (!source) {
      throw new Error('未找到该 Mock 规则')
    }
    const copy = { ...source, id: crypto.randomUUID(), name: `${source.name} 副本`, hitCount: 0 }
    browserMocks.push(copy)
    return copy
  },
  async reorderMockRules(orderedIds) {
    const byId = new Map(browserMocks.map((rule) => [rule.id, rule]))
    browserMocks.splice(0, browserMocks.length, ...orderedIds.map((id) => byId.get(id)!).filter(Boolean))
  },
  async testMockRules(method, url) {
    const matches = browserMocks
      .filter((rule) => rule.enabled && rule.methods.includes(method) && url.includes(rule.urlPattern))
      .sort((left, right) => right.priority - left.priority)
      .map((rule) => ({ ruleId: rule.id, name: rule.name, priority: rule.priority }))
    return { matches, selectedRuleId: matches[0]?.ruleId ?? null, conflict: matches.length > 1 }
  },
  async exportHar() {},
  async copyCurl(id) {
    const row = browserRows.find((candidate) => candidate.id === id)
    if (row) {
      await navigator.clipboard?.writeText(`curl -X '${row.method}' '${row.url}'`)
    }
  },
  async copyExchange(id, kind) {
    const detail = await this.getTrafficDetail(id)
    const value = kind === 'url'
      ? detail.url
      : kind === 'responseBody'
        ? detail.responseBodyText ?? ''
        : kind === 'requestHeaders'
          ? Object.entries(detail.requestHeaders).map(([name, value]) => `${name}: ${value}`).join('\n')
          : kind === 'responseHeaders'
            ? Object.entries(detail.responseHeaders).map(([name, value]) => `${name}: ${value}`).join('\n')
            : `curl -X '${detail.method}' '${detail.url}'`
    await navigator.clipboard?.writeText(value)
  },
  async replayExchange(id) {
    const original = await this.getTrafficDetail(id)
    const exchange = {
      ...original,
      id: crypto.randomUUID(),
      startedAt: new Date().toISOString()
    }
    browserRows.unshift(exchange)
    return { exchangeId: exchange.id, mocked: false, exchange }
  },
  async getReplayDraft(id) {
    const detail = await this.getTrafficDetail(id)
    return {
      sourceExchangeId: id,
      method: detail.method,
      url: detail.url,
      headers: detail.requestHeaders,
      body: detail.requestBody,
      bodyText: detail.requestBodyText,
      contentType: detail.requestHeaders['content-type'] ?? null,
      binary: detail.requestBody.length > 0 && detail.requestBodyText === null
    }
  },
  async sendReplay(request) {
    const source = await this.getTrafficDetail(request.sourceExchangeId)
    const exchange = {
      ...source,
      id: crypto.randomUUID(),
      startedAt: new Date().toISOString(),
      method: request.method,
      url: request.url,
      requestHeaders: request.headers,
      requestBody: request.body,
      sourceExchangeId: request.sourceExchangeId
    }
    browserRows.unshift(exchange)
    return { exchangeId: exchange.id, mocked: false, exchange }
  },
  async hasAcceptedDisclaimer() {
    return localStorage.getItem('netcat-disclaimer') === 'accepted'
  },
  async acceptDisclaimer() {
    localStorage.setItem('netcat-disclaimer', 'accepted')
  },
  async openDevtools() {
    throw {
      code: 'DEVTOOLS_UNAVAILABLE',
      message: '浏览器预览模式无法由应用打开调试面板',
      action: '请使用浏览器自带的开发者工具'
    } satisfies AppError
  },
  async listen() {
    return () => undefined
  }
}

const tauriApi: DesktopApi = {
  getPlatformInfo: () => invoke('get_platform_info'),
  getSystemProxyStatus: () => invoke('get_system_proxy_status'),
  getCertificateTrustStatus: () => invoke('get_certificate_trust_status'),
  installCaTrust: () => invoke('install_ca_trust'),
  removeCaTrust: () => invoke('remove_ca_trust'),
  getProxyStatus: () => invoke('get_proxy_status'),
  startProxy: (config) => invoke('start_proxy', { config }),
  stopProxy: () => invoke('stop_proxy'),
  setCaptureEnabled: (enabled) => invoke('set_capture_enabled', { enabled }),
  getCaStatus: () => invoke('get_ca_status'),
  generateCa: () => invoke('generate_ca'),
  async exportCa() {
    const destination = await save({ defaultPath: 'netcat-ca.crt', filters: [{ name: 'Certificate', extensions: ['crt'] }] })
    if (destination) {
      await invoke('export_ca', { destination })
    }
  },
  enableSystemProxy: () => invoke('enable_system_proxy'),
  restoreSystemProxy: () => invoke('restore_system_proxy'),
  isSystemProxyEnabled: () => invoke('is_system_proxy_enabled'),
  listTraffic: () => invoke('list_traffic', { filter: {}, cursor: null, limit: 500 }),
  getTrafficDetail: (id) => invoke('get_traffic_detail', { id }),
  clearTraffic: () => invoke('clear_traffic'),
  checkDatabaseIntegrity: () => invoke('check_database_integrity'),
  async exportDiagnosticBundle(logs) {
    const destination = await save({ defaultPath: 'netcat-diagnostics.zip', filters: [{ name: 'ZIP Archive', extensions: ['zip'] }] })
    if (destination) {
      await invoke('export_diagnostic_bundle', { destination, logs })
    }
  },
  listMockRules: () => invoke('list_mock_rules'),
  saveMockRule: (rule) => invoke('save_mock_rule', { rule }),
  deleteMockRule: (id) => invoke('delete_mock_rule', { id }),
  setMockEnabled: (id, enabled) => invoke('set_mock_enabled', { id, enabled }),
  duplicateMockRule: (id) => invoke('duplicate_mock_rule', { id }),
  reorderMockRules: (orderedIds) => invoke('reorder_mock_rules', { orderedIds }),
  testMockRules: (method, url) => invoke('test_mock_rules', { method, url }),
  async exportHar() {
    const destination = await save({ defaultPath: 'netcat-session.har', filters: [{ name: 'HTTP Archive', extensions: ['har'] }] })
    if (destination) {
      await invoke('export_har', { destination })
    }
  },
  async copyCurl(id) {
    const command = await invoke<string>('build_exchange_copy', { exchangeId: id, kind: 'curl' })
    await writeText(command)
  },
  async copyExchange(id, kind) {
    const content = await invoke<string>('build_exchange_copy', { exchangeId: id, kind })
    await writeText(content)
  },
  replayExchange: (id, mode) => invoke('replay_exchange', { exchangeId: id, mode }),
  getReplayDraft: (id) => invoke('get_replay_draft', { exchangeId: id }),
  sendReplay: (request) => invoke('send_replay', { request }),
  hasAcceptedDisclaimer: () => invoke('has_accepted_disclaimer'),
  acceptDisclaimer: () => invoke('accept_disclaimer'),
  openDevtools: () => invoke('open_devtools'),
  async listen<T>(event: DesktopEvent, handler: (payload: T) => void) {
    return tauriListen<T>(event, ({ payload }) => handler(payload))
  }
}

export const desktopApi: DesktopApi = window.__TAURI_INTERNALS__ ? tauriApi : browserApi

export function normalizeDesktopError(cause: unknown): AppError {
  if (cause && typeof cause === 'object' && 'code' in cause && 'message' in cause) {
    return cause as AppError
  }
  return {
    code: 'UNEXPECTED_ERROR',
    message: cause instanceof Error ? cause.message : String(cause),
    action: '请重试；若问题持续，请检查代理和证书设置'
  }
}
