export type ProxyState = 'stopped' | 'starting' | 'running' | 'stopping' | 'error'

export interface ProxyConfig {
  port: number
  allowLan: boolean
  captureEnabled: boolean
}

export interface ProxyStatus {
  state: ProxyState
  bindAddress: string | null
  port: number
  lanIp: string | null
  error: string | null
}

export interface TrafficSummary {
  id: string
  startedAt: string
  method: string
  url: string
  host: string
  status: number | null
  durationMs: number | null
  responseBytes: number
  contentType: string | null
  mocked: boolean
  truncated: boolean
  error: string | null
}

export interface TrafficDetail extends TrafficSummary {
  requestHeaders: Readonly<Record<string, string>>
  responseHeaders: Readonly<Record<string, string>>
  requestBody: readonly number[]
  responseBody: readonly number[]
  requestBodyText: string | null
  responseBodyText: string | null
  requestOriginalBytes: number
  responseOriginalBytes: number
}

export type DiagnosticLevel = 'info' | 'warn' | 'error'

export type DiagnosticScope = 'proxy' | 'mock' | 'replay' | 'system'

export interface DiagnosticLog {
  timestamp: string
  level: DiagnosticLevel
  scope: DiagnosticScope
  event: string
  message: string
  exchangeId?: string
  details?: Readonly<Record<string, unknown>>
}

export type DiagnosticTrafficSummary = TrafficSummary

export interface DiagnosticTrafficDetail extends TrafficSummary {
  requestHeaders: Readonly<Record<string, string>>
  responseHeaders: Readonly<Record<string, string>>
  requestOriginalBytes: number
  responseOriginalBytes: number
}

export interface NetCaptureDebug {
  status(): Promise<ProxyStatus>
  recent(limit?: number): Promise<DiagnosticTrafficSummary[]>
  get(exchangeId: string): Promise<DiagnosticTrafficDetail>
}

export type TrafficCopyKind =
  | 'url'
  | 'curl'
  | 'powershell'
  | 'fetchBrowser'
  | 'fetchNode'
  | 'requestHeaders'
  | 'responseHeaders'
  | 'responseBody'

export type ReplayMode = 'upstream' | 'mockAware'

export interface ReplayResult {
  exchangeId: string
  mocked: boolean
  exchange: TrafficSummary
}

export type MatchKind = 'exact' | 'contains' | 'regex'

export interface MockRule {
  id: string
  name: string
  enabled: boolean
  priority: number
  methods: readonly string[]
  matchKind: MatchKind
  urlPattern: string
  status: number
  responseHeaders: Readonly<Record<string, string>>
  responseBody: string
  delayMs: number
  createdAt: string
}

export interface AppError {
  code: string
  message: string
  detail?: string | null
  action?: string | null
}

export interface CaStatus {
  generated: boolean
  certificatePath: string | null
  fingerprint: string | null
}

export interface TrafficFilter {
  query: string
  method: string
  status: string
  host: string
}
