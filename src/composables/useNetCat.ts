import { computed, inject, onBeforeUnmount, readonly, shallowRef } from 'vue'
import { desktopApi, desktopApiKey, normalizeDesktopError } from '../api/desktop'
import type {
  AppError,
  CaStatus,
  CaPrivateKeyState,
  CertificateTrustState,
  DiagnosticLog,
  MockRule,
  MockRuleTestResult,
  PlatformInfo,
  ProxyConfig,
  ProxyStatus,
  ReplayDraft,
  ReplayMode,
  ReplayRequest,
  ReplayResult,
  SystemProxyStatus,
  TrafficCopyKind,
  TrafficDetail,
  TrafficSummary
} from '../shared/types'
import { createEventBatcher } from './event-batcher'
import { applyTrafficUpdates } from './traffic-updater'
import {
  appendDiagnosticLog,
  installDebugBridge,
  writeDiagnosticLog
} from '../features/diagnostics/debug-bridge'

export function useNetCat() {
  const api = inject(desktopApiKey, desktopApi)
  const proxyStatus = shallowRef<ProxyStatus>({
    state: 'stopped',
    bindAddress: null,
    port: 28086,
    lanIp: null,
    error: null
  })
  const traffic = shallowRef<readonly TrafficSummary[]>([])
  const selectedDetail = shallowRef<TrafficDetail | null>(null)
  const mockRules = shallowRef<readonly MockRule[]>([])
  const caStatus = shallowRef<CaStatus>({ generated: false, certificatePath: null, fingerprint: null })
  const platform = shallowRef<PlatformInfo | null>(null)
  const trustState = shallowRef<CertificateTrustState>('unavailable')
  const privateKeyState = shallowRef<CaPrivateKeyState>('unavailable')
  const systemProxyStatus = shallowRef<SystemProxyStatus>({ managed: false, recoveryPending: false, conflict: false })
  const systemProxyEnabled = computed(() => systemProxyStatus.value.managed && !systemProxyStatus.value.conflict)
  const systemProxyPreferred = shallowRef(true)
  const disclaimerAccepted = shallowRef<boolean | null>(null)
  const error = shallowRef<AppError | null>(null)
  const busy = shallowRef(false)
  const allowLan = shallowRef(false)
  const lanIp = shallowRef<string | null>(null)
  const captureEnabled = shallowRef(true)
  const port = shallowRef(28086)
  const diagnosticLogs = shallowRef<readonly DiagnosticLog[]>([])
  const unlisteners: Array<() => void> = []
  const removeDebugBridge = installDebugBridge(api)
  const trafficEvents = createEventBatcher<TrafficSummary>((updates) => {
    traffic.value = applyTrafficUpdates(traffic.value, updates, 500)
  }, 75, (row) => row.id)

  const isRunning = computed(() => proxyStatus.value.state === 'running')

  async function initialize() {
    busy.value = true
    try {
      const [status, localLanIp, rows, rules, certificate, systemProxy, disclaimer, platformInfo, certificateTrust] = await Promise.all([
        api.getProxyStatus(),
        api.getLanIp?.() ?? Promise.resolve(null),
        api.listTraffic(),
        api.listMockRules(),
        api.getCaStatus().catch((cause) => {
          error.value = normalizeDesktopError(cause)
          return caStatus.value
        }),
        api.getSystemProxyStatus().catch((cause) => {
          error.value = normalizeDesktopError(cause)
          return { managed: false, recoveryPending: false, conflict: false }
        }),
        api.hasAcceptedDisclaimer(),
        api.getPlatformInfo(),
        api.getCertificateTrustDetails().catch((cause) => {
          error.value = normalizeDesktopError(cause)
          return { publicTrust: 'unavailable', privateKeyState: 'unavailable' } as const
        })
      ])
      proxyStatus.value = status
      lanIp.value = status.lanIp ?? localLanIp
      port.value = status.port
      traffic.value = rows
      mockRules.value = rules
      caStatus.value = certificate
      systemProxyStatus.value = systemProxy
      platform.value = platformInfo
      trustState.value = certificateTrust.publicTrust
      privateKeyState.value = certificateTrust.privateKeyState
      disclaimerAccepted.value = disclaimer
      unlisteners.push(await api.listen<ProxyStatus>('proxy-status-changed', (payload) => {
        proxyStatus.value = payload
        lanIp.value = payload.lanIp ?? lanIp.value
      }))
      unlisteners.push(await api.listen<TrafficSummary>('traffic-created', trafficEvents.schedule))
      unlisteners.push(await api.listen<TrafficSummary>('traffic-updated', trafficEvents.schedule))
      unlisteners.push(await api.listen<AppError>('storage-warning', (payload) => {
        error.value = payload
      }))
      unlisteners.push(await api.listen<DiagnosticLog>('diagnostic-log', (payload) => {
        diagnosticLogs.value = appendDiagnosticLog(diagnosticLogs.value, payload)
        writeDiagnosticLog(payload)
      }))
    } catch (cause) {
      error.value = normalizeDesktopError(cause)
      disclaimerAccepted.value ??= false
    } finally {
      busy.value = false
    }
  }

  async function reloadTraffic() {
    traffic.value = await api.listTraffic()
  }

  async function toggleProxy() {
    await runSystemProxyAction(async () => {
      if (isRunning.value) {
        proxyStatus.value = await api.stopProxy()
        return
      }

      proxyStatus.value = await api.startProxy({
        port: port.value,
        allowLan: allowLan.value,
        captureEnabled: captureEnabled.value
      })
      if (systemProxyPreferred.value) {
        await api.enableSystemProxy()
      }
    })
  }

  async function setSystemProxy(enabled: boolean) {
    systemProxyPreferred.value = enabled
    if (!isRunning.value || enabled === systemProxyEnabled.value) {
      return
    }
    await runSystemProxyAction(async () => {
      if (enabled) {
        await api.enableSystemProxy()
      } else {
        await api.restoreSystemProxy()
      }
    })
  }

  async function runSystemProxyAction(action: () => Promise<void>) {
    await runAction(async () => {
      try {
        await action()
      } finally {
        systemProxyStatus.value = await api.getSystemProxyStatus()
      }
    })
  }

  async function restoreSystemProxy() {
    await runSystemProxyAction(() => api.restoreSystemProxy())
  }

  async function setCapture(enabled: boolean) {
    captureEnabled.value = enabled
    await runAction(() => api.setCaptureEnabled(enabled))
  }

  async function setAllowLan(enabled: boolean) {
    allowLan.value = enabled
    if (enabled) {
      lanIp.value = await api.getLanIp?.() ?? null
    }
  }

  async function selectTraffic(row: TrafficSummary) {
    await runAction(async () => {
      selectedDetail.value = await api.getTrafficDetail(row.id)
    })
  }

  async function getTrafficDetail(id: string): Promise<TrafficDetail | null> {
    try {
      return await api.getTrafficDetail(id)
    } catch (cause) {
      error.value = normalizeDesktopError(cause)
      return null
    }
  }

  function clearSelection() {
    selectedDetail.value = null
  }

  async function copyExchange(id: string, kind: TrafficCopyKind): Promise<boolean> {
    let copied = false
    await runAction(async () => {
      await api.copyExchange(id, kind)
      copied = true
    })
    return copied
  }

  async function replayExchange(id: string, mode: ReplayMode): Promise<ReplayResult | null> {
    busy.value = true
    error.value = null
    try {
      const result = await api.replayExchange(id, mode)
      await reloadTraffic()
      selectedDetail.value = await api.getTrafficDetail(result.exchangeId)
      return result
    } catch (cause) {
      error.value = normalizeDesktopError(cause)
      await reloadTraffic()
      return null
    } finally {
      busy.value = false
    }
  }

  async function getReplayDraft(id: string): Promise<ReplayDraft | null> {
    if (!api.getReplayDraft) {
      error.value = {
        code: 'REPLAY_UNAVAILABLE',
        message: '当前环境不支持编辑并重发'
      }
      return null
    }
    try {
      return await api.getReplayDraft(id)
    } catch (cause) {
      error.value = normalizeDesktopError(cause)
      return null
    }
  }

  async function sendReplay(request: ReplayRequest): Promise<ReplayResult | null> {
    if (!api.sendReplay) {
      return replayExchange(request.sourceExchangeId, request.mode)
    }
    busy.value = true
    error.value = null
    try {
      const result = await api.sendReplay(request)
      traffic.value = applyTrafficUpdates(traffic.value, [result.exchange], 500)
      selectedDetail.value = await api.getTrafficDetail(result.exchangeId)
      return result
    } catch (cause) {
      error.value = normalizeDesktopError(cause)
      return null
    } finally {
      busy.value = false
    }
  }

  async function clearTraffic() {
    await runAction(async () => {
      await api.clearTraffic()
      traffic.value = []
      selectedDetail.value = null
    })
  }

  async function saveMock(rule: MockRule) {
    await runAction(async () => {
      await api.saveMockRule(rule)
      mockRules.value = await api.listMockRules()
    })
  }

  async function deleteMock(id: string) {
    await runAction(async () => {
      await api.deleteMockRule(id)
      mockRules.value = await api.listMockRules()
    })
  }

  async function setMockEnabled(id: string, enabled: boolean) {
    await runAction(async () => {
      await api.setMockEnabled(id, enabled)
      mockRules.value = await api.listMockRules()
    })
  }

  async function duplicateMock(id: string) {
    if (!api.duplicateMockRule) {
      return
    }
    await runAction(async () => {
      await api.duplicateMockRule!(id)
      mockRules.value = await api.listMockRules()
    })
  }

  async function reorderMocks(orderedIds: readonly string[]) {
    if (!api.reorderMockRules) {
      return
    }
    await runAction(async () => {
      await api.reorderMockRules!(orderedIds)
      mockRules.value = await api.listMockRules()
    })
  }

  async function testMocks(method: string, url: string): Promise<MockRuleTestResult | null> {
    if (!api.testMockRules) {
      return null
    }
    try {
      error.value = null
      return await api.testMockRules(method, url)
    } catch (cause) {
      error.value = normalizeDesktopError(cause)
      return null
    }
  }

  async function generateCa() {
    await runAction(async () => {
      caStatus.value = await api.generateCa()
      await refreshCertificateTrust()
    })
  }

  async function refreshCertificateTrust() {
    const details = await api.getCertificateTrustDetails().catch((cause) => {
      error.value = normalizeDesktopError(cause)
      return { publicTrust: 'unavailable', privateKeyState: 'unavailable' } as const
    })
    trustState.value = details.publicTrust
    privateKeyState.value = details.privateKeyState
  }

  function canManageCaTrust() {
    return !busy.value && platform.value?.os === 'windows' &&
      platform.value.certificateTrustSupported && caStatus.value.generated
  }

  async function trustCa() {
    if (!canManageCaTrust() || trustState.value !== 'notTrusted' || privateKeyState.value !== 'ready') {
      return
    }
    await runAction(async () => {
      trustState.value = await api.installCaTrust()
    })
  }

  async function removeCaTrust() {
    if (!canManageCaTrust() || trustState.value !== 'trusted') {
      return
    }
    await runAction(async () => {
      await api.removeCaTrust()
      await refreshCertificateTrust()
    })
  }

  async function acceptDisclaimer() {
    await runAction(async () => {
      await api.acceptDisclaimer()
      disclaimerAccepted.value = true
    })
  }

  async function openDevtools() {
    await runAction(async () => {
      await api.openDevtools()
      console.info('[NetCat][system] 调试面板已打开', {
        proxyStatus: proxyStatus.value,
        trafficQueries: [
          '__NETCAT_DEBUG__.status()',
          '__NETCAT_DEBUG__.recent()',
          '__NETCAT_DEBUG__.get(exchangeId)'
        ]
      })
    })
  }

  async function runAction(action: () => Promise<void>) {
    busy.value = true
    error.value = null
    try {
      await action()
    } catch (cause) {
      error.value = normalizeDesktopError(cause)
    } finally {
      busy.value = false
    }
  }

  function dismissError() {
    error.value = null
  }

  function clearDiagnosticLogs() {
    diagnosticLogs.value = []
  }

  onBeforeUnmount(() => {
    trafficEvents.cancel()
    unlisteners.splice(0).forEach((unlisten) => unlisten())
    removeDebugBridge()
  })

  return {
    api,
    proxyStatus: readonly(proxyStatus),
    traffic: readonly(traffic),
    selectedDetail: readonly(selectedDetail),
    mockRules: readonly(mockRules),
    caStatus: readonly(caStatus),
    platform: readonly(platform),
    trustState: readonly(trustState),
    privateKeyState: readonly(privateKeyState),
    systemProxyStatus: readonly(systemProxyStatus),
    systemProxyEnabled: readonly(systemProxyEnabled),
    systemProxyPreferred: readonly(systemProxyPreferred),
    disclaimerAccepted: readonly(disclaimerAccepted),
    error: readonly(error),
    busy: readonly(busy),
    allowLan,
    lanIp: readonly(lanIp),
    captureEnabled: readonly(captureEnabled),
    port,
    diagnosticLogs: readonly(diagnosticLogs),
    isRunning,
    initialize,
    toggleProxy,
    setSystemProxy,
    restoreSystemProxy,
    setAllowLan,
    setCapture,
    selectTraffic,
    getTrafficDetail,
    clearSelection,
    copyExchange,
    replayExchange,
    getReplayDraft,
    sendReplay,
    clearTraffic,
    saveMock,
    deleteMock,
    setMockEnabled,
    duplicateMock,
    reorderMocks,
    testMocks,
    generateCa,
    trustCa,
    removeCaTrust,
    acceptDisclaimer,
    openDevtools,
    clearDiagnosticLogs,
    dismissError
  }
}
