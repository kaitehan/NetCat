import { describe, expect, it, vi } from 'vitest'
import type { DesktopApi } from '../../api/desktop'
import type { DiagnosticLog, TrafficDetail, TrafficSummary } from '../../shared/types'
import { appendDiagnosticLog, createDebugBridge, installDebugBridge, writeDiagnosticLog } from './debug-bridge'

const row: TrafficSummary = {
  id: 'exchange-1',
  startedAt: '2026-10-23T10:00:00Z',
  method: 'POST',
  url: 'https://api.example.com/session',
  host: 'api.example.com',
  status: 201,
  durationMs: 48,
  responseBytes: 12,
  contentType: 'application/json',
  mocked: false,
  truncated: false,
  error: null
}

const detail: TrafficDetail = {
  ...row,
  requestHeaders: {
    authorization: 'Bearer secret',
    Cookie: 'session=secret',
    'x-api-key': 'api-secret',
    'x-client-secret': 'client-secret',
    'content-type': 'application/json'
  },
  responseHeaders: {
    'Set-Cookie': 'session=next',
    'x-auth-token': 'response-secret',
    server: 'example'
  },
  requestBody: [123, 125],
  responseBody: [123, 125],
  requestBodyText: '{}',
  responseBodyText: '{}',
  requestOriginalBytes: 2,
  responseOriginalBytes: 2
}

function apiWithRows(rows: TrafficSummary[]): Pick<DesktopApi, 'getProxyStatus' | 'listTraffic' | 'getTrafficDetail'> {
  return {
    getProxyStatus: vi.fn().mockResolvedValue({
      state: 'running',
      bindAddress: '127.0.0.1',
      port: 28086,
      lanIp: null,
      error: null
    }),
    listTraffic: vi.fn().mockResolvedValue(rows),
    getTrafficDetail: vi.fn().mockResolvedValue(detail)
  }
}

describe('createDebugBridge', () => {
  it('returns 20 recent records by default and caps explicit requests at 100', async () => {
    const rows = Array.from({ length: 150 }, (_, index) => ({
      ...row,
      id: `exchange-${index}`
    }))
    const bridge = createDebugBridge(apiWithRows(rows))

    expect(await bridge.recent()).toHaveLength(20)
    expect(await bridge.recent(500)).toHaveLength(100)
  })

  it('redacts sensitive headers and excludes request and response bodies', async () => {
    const bridge = createDebugBridge(apiWithRows([row]))

    const result = await bridge.get('exchange-1')

    expect(result.requestHeaders).toEqual({
      authorization: '[REDACTED]',
      Cookie: '[REDACTED]',
      'x-api-key': '[REDACTED]',
      'x-client-secret': '[REDACTED]',
      'content-type': 'application/json'
    })
    expect(result.responseHeaders).toEqual({
      'Set-Cookie': '[REDACTED]',
      'x-auth-token': '[REDACTED]',
      server: 'example'
    })
    expect(result).not.toHaveProperty('requestBody')
    expect(result).not.toHaveProperty('responseBody')
    expect(result).not.toHaveProperty('requestBodyText')
    expect(result).not.toHaveProperty('responseBodyText')
    expect(result.requestOriginalBytes).toBe(2)
    expect(result.responseOriginalBytes).toBe(2)
  })

  it('installs a read-only global helper and removes only its own instance', () => {
    const target = {} as Window
    const remove = installDebugBridge(apiWithRows([row]), target)

    expect(Object.getOwnPropertyDescriptor(target, '__NETCAPTURE_DEBUG__')).toMatchObject({
      configurable: true,
      enumerable: false,
      writable: false
    })
    expect(Object.isFrozen(target.__NETCAPTURE_DEBUG__)).toBe(true)

    remove()
    expect(target.__NETCAPTURE_DEBUG__).toBeUndefined()
  })
})

describe('writeDiagnosticLog', () => {
  it.each([
    ['info', 'info'],
    ['warn', 'warn'],
    ['error', 'error']
  ] as const)('writes %s events to console.%s with a structured payload', (level, method) => {
    const consoleTarget = {
      info: vi.fn(),
      warn: vi.fn(),
      error: vi.fn()
    }
    const log: DiagnosticLog = {
      timestamp: '2026-10-23T10:00:00Z',
      level,
      scope: 'proxy',
      event: 'proxy.started',
      message: '代理已启动',
      exchangeId: 'exchange-1',
      details: { port: 28086 }
    }

    writeDiagnosticLog(log, consoleTarget)

    expect(consoleTarget[method]).toHaveBeenCalledWith(
      '[NetCapture][proxy] 代理已启动',
      {
        timestamp: '2026-10-23T10:00:00Z',
        event: 'proxy.started',
        exchangeId: 'exchange-1',
        port: 28086
      }
    )
  })
})

describe('appendDiagnosticLog', () => {
  it('keeps the newest 500 diagnostic entries', () => {
    const existing = Array.from({ length: 500 }, (_, index): DiagnosticLog => ({
      timestamp: `2026-10-23T10:00:${index}Z`,
      level: 'info',
      scope: 'system',
      event: `event-${index}`,
      message: `日志 ${index}`
    }))
    const latest: DiagnosticLog = {
      timestamp: '2026-10-23T11:00:00Z',
      level: 'warn',
      scope: 'proxy',
      event: 'proxy.latest',
      message: '最新日志'
    }

    const result = appendDiagnosticLog(existing, latest)

    expect(result).toHaveLength(500)
    expect(result[0]?.event).toBe('event-1')
    expect(result.at(-1)).toBe(latest)
  })
})
