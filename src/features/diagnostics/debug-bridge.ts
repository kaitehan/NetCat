import type { DesktopApi } from '../../api/desktop'
import type {
  DiagnosticLog,
  DiagnosticTrafficDetail,
  NetCaptureDebug,
  TrafficDetail
} from '../../shared/types'

type DebugApi = Pick<DesktopApi, 'getProxyStatus' | 'listTraffic' | 'getTrafficDetail'>

type ConsoleTarget = Pick<Console, 'info' | 'warn' | 'error'>

const SENSITIVE_HEADER_PATTERN = /(authorization|cookie|token|api[-_]?key|secret)/i
const DIAGNOSTIC_LOG_LIMIT = 500

export function appendDiagnosticLog(
  logs: readonly DiagnosticLog[],
  entry: DiagnosticLog
): readonly DiagnosticLog[] {
  return [...logs, entry].slice(-DIAGNOSTIC_LOG_LIMIT)
}

function redactHeaders(headers: Readonly<Record<string, string>>): Readonly<Record<string, string>> {
  return Object.fromEntries(Object.entries(headers).map(([name, value]) => [
    name,
    SENSITIVE_HEADER_PATTERN.test(name) ? '[REDACTED]' : value
  ]))
}

export function sanitizeTrafficDetail(detail: TrafficDetail): DiagnosticTrafficDetail {
  const {
    requestHeaders,
    responseHeaders,
    requestBody: _requestBody,
    responseBody: _responseBody,
    requestBodyText: _requestBodyText,
    responseBodyText: _responseBodyText,
    ...metadata
  } = detail
  return {
    ...metadata,
    requestHeaders: redactHeaders(requestHeaders),
    responseHeaders: redactHeaders(responseHeaders)
  }
}

function normalizeLimit(limit: number | undefined): number {
  if (limit === undefined || !Number.isFinite(limit)) {
    return 20
  }
  return Math.min(100, Math.max(1, Math.trunc(limit)))
}

export function createDebugBridge(api: DebugApi): NetCaptureDebug {
  const bridge: NetCaptureDebug = {
    status: () => api.getProxyStatus(),
    async recent(limit?: number) {
      const rows = await api.listTraffic()
      return rows.slice(0, normalizeLimit(limit))
    },
    async get(exchangeId: string) {
      return sanitizeTrafficDetail(await api.getTrafficDetail(exchangeId))
    }
  }
  return Object.freeze(bridge)
}

export function writeDiagnosticLog(log: DiagnosticLog, target: ConsoleTarget = console): void {
  const payload = {
    timestamp: log.timestamp,
    event: log.event,
    ...(log.exchangeId ? { exchangeId: log.exchangeId } : {}),
    ...log.details
  }
  target[log.level](`[NetCapture][${log.scope}] ${log.message}`, payload)
}

export function installDebugBridge(api: DebugApi, target: Window = window): () => void {
  const bridge = createDebugBridge(api)
  Object.defineProperty(target, '__NETCAPTURE_DEBUG__', {
    value: bridge,
    configurable: true,
    enumerable: false,
    writable: false
  })
  return () => {
    if (target.__NETCAPTURE_DEBUG__ === bridge) {
      delete target.__NETCAPTURE_DEBUG__
    }
  }
}

declare global {
  interface Window {
    __NETCAPTURE_DEBUG__?: NetCaptureDebug
  }
}
