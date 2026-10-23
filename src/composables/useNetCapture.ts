import { computed, inject, onBeforeUnmount, readonly, shallowRef } from 'vue'
import { desktopApi, desktopApiKey, normalizeDesktopError } from '../api/desktop'
import type {
  AppError,
  CaStatus,
  MockRule,
  ProxyConfig,
  ProxyStatus,
  ReplayMode,
  ReplayResult,
  TrafficCopyKind,
  TrafficDetail,
  TrafficSummary
} from '../shared/types'
import { createEventBatcher } from './event-batcher'

export function useNetCapture() {
  const api = inject(desktopApiKey, desktopApi)
  const proxyStatus = shallowRef<ProxyStatus>({
    state: 'stopped',
    bindAddress: null,
    port: 8888,
    lanIp: null,
    error: null
  })
  const traffic = shallowRef<readonly TrafficSummary[]>([])
  const selectedDetail = shallowRef<TrafficDetail | null>(null)
  const mockRules = shallowRef<readonly MockRule[]>([])
  const caStatus = shallowRef<CaStatus>({ generated: false, certificatePath: null, fingerprint: null })
  const systemProxyEnabled = shallowRef(false)
  const disclaimerAccepted = shallowRef<boolean | null>(null)
  const error = shallowRef<AppError | null>(null)
  const busy = shallowRef(false)
  const allowLan = shallowRef(false)
  const captureEnabled = shallowRef(true)
  const port = shallowRef(8888)
  const unlisteners: Array<() => void> = []
  const trafficEvents = createEventBatcher(reloadTraffic, 75)

  const isRunning = computed(() => proxyStatus.value.state === 'running')

  async function initialize() {
    busy.value = true
    try {
      const [status, rows, rules, certificate, systemProxy, disclaimer] = await Promise.all([
        api.getProxyStatus(),
        api.listTraffic(),
        api.listMockRules(),
        api.getCaStatus(),
        api.isSystemProxyEnabled(),
        api.hasAcceptedDisclaimer()
      ])
      proxyStatus.value = status
      port.value = status.port
      traffic.value = rows
      mockRules.value = rules
      caStatus.value = certificate
      systemProxyEnabled.value = systemProxy
      disclaimerAccepted.value = disclaimer
      unlisteners.push(await api.listen<ProxyStatus>('proxy-status-changed', (payload) => {
        proxyStatus.value = payload
      }))
      unlisteners.push(await api.listen<string>('traffic-created', trafficEvents.schedule))
      unlisteners.push(await api.listen<string>('traffic-updated', trafficEvents.schedule))
      unlisteners.push(await api.listen<AppError>('storage-warning', (payload) => {
        error.value = payload
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
    await runAction(async () => {
      proxyStatus.value = isRunning.value
        ? await api.stopProxy()
        : await api.startProxy({
          port: port.value,
          allowLan: allowLan.value,
          captureEnabled: captureEnabled.value
        })
      if (!isRunning.value) {
        systemProxyEnabled.value = false
      }
    })
  }

  async function toggleSystemProxy() {
    await runAction(async () => {
      if (systemProxyEnabled.value) {
        await api.restoreSystemProxy()
      } else {
        await api.enableSystemProxy()
      }
      systemProxyEnabled.value = !systemProxyEnabled.value
    })
  }

  async function setCapture(enabled: boolean) {
    captureEnabled.value = enabled
    await runAction(() => api.setCaptureEnabled(enabled))
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

  async function generateCa() {
    await runAction(async () => {
      caStatus.value = await api.generateCa()
    })
  }

  async function acceptDisclaimer() {
    await runAction(async () => {
      await api.acceptDisclaimer()
      disclaimerAccepted.value = true
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

  onBeforeUnmount(() => {
    trafficEvents.cancel()
    unlisteners.splice(0).forEach((unlisten) => unlisten())
  })

  return {
    api,
    proxyStatus: readonly(proxyStatus),
    traffic: readonly(traffic),
    selectedDetail: readonly(selectedDetail),
    mockRules: readonly(mockRules),
    caStatus: readonly(caStatus),
    systemProxyEnabled: readonly(systemProxyEnabled),
    disclaimerAccepted: readonly(disclaimerAccepted),
    error: readonly(error),
    busy: readonly(busy),
    allowLan,
    captureEnabled: readonly(captureEnabled),
    port,
    isRunning,
    initialize,
    toggleProxy,
    toggleSystemProxy,
    setCapture,
    selectTraffic,
    getTrafficDetail,
    clearSelection,
    copyExchange,
    replayExchange,
    clearTraffic,
    saveMock,
    deleteMock,
    setMockEnabled,
    generateCa,
    acceptDisclaimer,
    dismissError
  }
}
