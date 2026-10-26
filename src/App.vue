<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, reactive, shallowRef } from 'vue'
import { Search, X } from '@lucide/vue'
import AppSidebar from './components/AppSidebar.vue'
import FilterSelect from './components/FilterSelect.vue'
import TopToolbar from './components/TopToolbar.vue'
import DisclaimerDialog from './features/onboarding/DisclaimerDialog.vue'
import { sanitizeTrafficDetail } from './features/diagnostics/debug-bridge'
import TrafficContextMenu, { type TrafficMenuAction } from './features/traffic/TrafficContextMenu.vue'
import DeviceTabs from './features/traffic/DeviceTabs.vue'
import TrafficTable from './features/traffic/TrafficTable.vue'
import { filterTrafficByDevice, listTrafficDevices, type DeviceScope } from './features/traffic/device-filter'
import { filterTraffic } from './features/traffic/traffic-filter'
import { useNetCat } from './composables/useNetCat'
import type {
  DiagnosticTrafficDetail,
  MockRule,
  MockRuleTestResult,
  ReplayMode,
  ReplayDraft,
  ReplayRequest,
  TrafficCopyKind,
  TrafficDetail,
  TrafficFilter,
  TrafficSummary
} from './shared/types'

const MockEditor = defineAsyncComponent(() => import('./features/mock/MockEditor.vue'))
const MockRulesPanel = defineAsyncComponent(() => import('./features/mock/MockRulesPanel.vue'))
const SettingsPage = defineAsyncComponent(() => import('./features/settings/SettingsPage.vue'))
const AboutPage = defineAsyncComponent(() => import('./features/about/AboutPage.vue'))
const DiagnosticPanel = defineAsyncComponent(() => import('./features/diagnostics/DiagnosticPanel.vue'))
const TrafficInspector = defineAsyncComponent(() => import('./features/traffic/TrafficInspector.vue'))
const ReplayEditor = defineAsyncComponent(() => import('./features/replay/ReplayEditor.vue'))
const ScriptWorkspace = defineAsyncComponent(() => import('./features/scripts/ScriptWorkspace.vue'))
const PinningWorkspace = defineAsyncComponent(() => import('./features/pinning/PinningWorkspace.vue'))
const NetworkProfileEditor = defineAsyncComponent(() => import('./features/network/NetworkProfileEditor.vue'))

const capture = useNetCat()
const activeView = shallowRef<'traffic' | 'mocks' | 'scripts' | 'pinning' | 'settings' | 'about'>('traffic')
const mockEditorOpen = shallowRef(false)
const replayEditorOpen = shallowRef(false)
const replayDraft = shallowRef<ReplayDraft | null>(null)
const replayMode = shallowRef<ReplayMode>('upstream')
const editingRule = shallowRef<MockRule | null>(null)
const mockInitialFocus = shallowRef<'headers' | 'body' | null>(null)
const mockTestResult = shallowRef<MockRuleTestResult | null>(null)
const toast = shallowRef<string | null>(null)
const hiddenUrls = shallowRef<ReadonlySet<string>>(new Set())
const hiddenHosts = shallowRef<ReadonlySet<string>>(new Set())
const showHidden = shallowRef(false)
const lastHidden = shallowRef<{ kind: 'url' | 'host'; value: string } | null>(null)
const contextRow = shallowRef<TrafficSummary | null>(null)
const contextDetail = shallowRef<TrafficDetail | null>(null)
const contextPosition = reactive({ x: 0, y: 0 })
const diagnosticsOpen = shallowRef(false)
const networkEditorOpen = shallowRef(false)
const diagnosticDetail = shallowRef<DiagnosticTrafficDetail | null>(null)
const diagnosticDetailLoading = shallowRef(false)
const deviceScope = shallowRef<DeviceScope>('all')
const filter = reactive<TrafficFilter>({
  query: '',
  method: 'ALL',
  status: 'ALL',
  host: ''
})
const methodOptions = [
  { value: 'ALL', label: '全部方法' },
  { value: 'GET', label: 'GET' },
  { value: 'POST', label: 'POST' },
  { value: 'PUT', label: 'PUT' },
  { value: 'PATCH', label: 'PATCH' },
  { value: 'DELETE', label: 'DELETE' }
] as const
const statusOptions = [
  { value: 'ALL', label: '全部状态' },
  { value: '2xx', label: '2xx' },
  { value: '3xx', label: '3xx' },
  { value: '4xx', label: '4xx' },
  { value: '5xx', label: '5xx' },
  { value: 'ERR', label: '错误' }
] as const

const hiddenTrafficIds = computed<ReadonlySet<string>>(() => new Set(
  capture.traffic.value
    .filter((row) => hiddenUrls.value.has(row.url) || hiddenHosts.value.has(row.host))
    .map((row) => row.id)
))
const trafficDevices = computed(() => listTrafficDevices(capture.traffic.value, capture.pairedDevices.value))
const historicalDevices = computed(() => trafficDevices.value.map((device) => ({
  deviceToken: device.token,
  name: device.name,
  requestCount: device.count
})))
const deviceTraffic = computed(() => filterTrafficByDevice(capture.traffic.value, deviceScope.value))
const filteredTraffic = computed(() => filterTraffic(deviceTraffic.value, filter))
const visibleTraffic = computed(() => showHidden.value
  ? filteredTraffic.value
  : filteredTraffic.value.filter((row) => !hiddenTrafficIds.value.has(row.id)))
const hosts = computed(() => [...new Set(deviceTraffic.value.map((row) => row.host))].sort())
const hostOptions = computed(() => [
  { value: '', label: '全部域名' },
  ...hosts.value.map((host) => ({ value: host, label: host }))
])
const hiddenRuleCount = computed(() => hiddenUrls.value.size + hiddenHosts.value.size)
const contextOpen = computed(() => contextRow.value !== null)
const toolbarlessView = computed(() => activeView.value !== 'traffic')
const fullWorkspaceView = computed(() => ['scripts', 'pinning', 'settings', 'about'].includes(activeView.value))
const contextDisabledReasons = computed<Partial<Record<TrafficMenuAction, string>>>(() => {
  const row = contextRow.value
  const detail = contextDetail.value
  if (!row) {
    return {}
  }
  const reasons: Partial<Record<TrafficMenuAction, string>> = {}
  const replayReason = row.method === 'CONNECT'
    ? 'CONNECT 请求不能重新发送'
    : !/^https?:\/\//i.test(row.url)
      ? '仅支持重新发送 HTTP/HTTPS 请求'
      : detail && detail.requestBody.length !== detail.requestOriginalBytes
        ? '请求正文已截断，不能重新发送'
        : null
  if (replayReason) {
    reasons['replay:upstream'] = replayReason
    reasons['replay:mockAware'] = replayReason
  }
  const mockReason = row.method === 'CONNECT'
    ? 'CONNECT 请求不能创建响应 Mock'
    : row.status === null
      ? '响应尚未完成，不能创建 Mock'
      : row.truncated
        ? '响应正文已截断，不能创建 Mock'
        : detail && detail.responseBody.length > 0 && detail.responseBodyText === null
          ? '二进制响应正文不能创建文本 Mock'
          : null
  if (mockReason) {
    reasons['mock:headers'] = mockReason
    reasons['mock:body'] = mockReason
  }
  if (detail?.requestBody.length && detail.requestBodyText === null) {
    for (const kind of ['curl', 'powershell', 'fetchBrowser', 'fetchNode'] as const) {
      reasons[`copy:${kind}`] = '二进制请求正文不能生成请求命令'
    }
  }
  if (detail?.responseBody.length && detail.responseBodyText === null) {
    reasons['copy:responseBody'] = '二进制响应正文不能复制为文本'
  }
  return reasons
})

function setPort(value: number) {
  capture.port.value = value
}

function setDeviceScope(scope: DeviceScope) {
  deviceScope.value = scope
  const selected = capture.selectedDetail.value
  if (selected && !filterTrafficByDevice([selected], scope).length) capture.clearSelection()
}

async function deleteDeviceData(deviceToken: string) {
  const count = capture.traffic.value.filter((row) => row.deviceToken === deviceToken).length
  if (!window.confirm(`确认断开该设备并删除其 ${count} 条请求吗？此操作无法撤销。`)) return
  await capture.deletePairedDeviceData(deviceToken)
  if (deviceScope.value === `device:${deviceToken}`) deviceScope.value = 'all'
}

function openMockEditor(rule: MockRule | null, initialFocus: 'headers' | 'body' | null = null) {
  editingRule.value = rule
  mockInitialFocus.value = initialFocus
  mockEditorOpen.value = true
  toast.value = null
  lastHidden.value = null
}

async function openTrafficMenu(payload: { row: TrafficSummary; x: number; y: number }) {
  contextRow.value = payload.row
  contextDetail.value = null
  contextPosition.x = payload.x
  contextPosition.y = payload.y
  const detail = await capture.getTrafficDetail(payload.row.id)
  if (contextRow.value?.id === payload.row.id) {
    contextDetail.value = detail
  }
}

function closeTrafficMenu() {
  contextRow.value = null
  contextDetail.value = null
}

function hideTraffic(kind: 'url' | 'host', value: string) {
  const target = kind === 'url' ? hiddenUrls : hiddenHosts
  target.value = new Set(target.value).add(value)
  lastHidden.value = { kind, value }
  toast.value = `已屏蔽${kind === 'url' ? '该 URL' : '域名'}，点击撤销`
  const selected = capture.selectedDetail.value
  if (selected && (hiddenUrls.value.has(selected.url) || hiddenHosts.value.has(selected.host))) {
    capture.clearSelection()
  }
}

function undoLastHide() {
  const hidden = lastHidden.value
  if (!hidden) {
    toast.value = null
    return
  }
  const target = hidden.kind === 'url' ? hiddenUrls : hiddenHosts
  const next = new Set(target.value)
  next.delete(hidden.value)
  target.value = next
  lastHidden.value = null
  toast.value = '已撤销屏蔽'
}

function clearHiddenRules() {
  hiddenUrls.value = new Set()
  hiddenHosts.value = new Set()
  showHidden.value = false
  lastHidden.value = null
  toast.value = '已清除全部屏蔽规则'
}

function createMockDraft(detail: TrafficDetail, focus: 'headers' | 'body') {
  const responseHeaders = Object.fromEntries(Object.entries(detail.responseHeaders).filter(([name]) =>
    !['content-length', 'transfer-encoding', 'content-encoding'].includes(name.toLowerCase())
  ))
  let path = '/'
  try {
    const parsed = new URL(detail.url)
    path = `${parsed.pathname}${parsed.search}`
  } catch {
    path = detail.url
  }
  const priority = capture.mockRules.value.length
    ? Math.max(...capture.mockRules.value.map((rule) => rule.priority)) + 1
    : 0
  const label = focus === 'headers' ? '替换响应头' : '替换响应内容'
  activeView.value = 'mocks'
  openMockEditor({
    id: '',
    name: `${label} · ${detail.method} ${path}`,
    enabled: true,
    priority,
    methods: [detail.method],
    matchKind: 'exact',
    urlPattern: detail.url,
    status: detail.status ?? 200,
    responseHeaders,
    responseBody: detail.responseBodyText ?? '',
    delayMs: 0,
    createdAt: '',
    sourceExchangeId: detail.id
  }, focus)
}

async function handleTrafficAction(action: TrafficMenuAction) {
  const row = contextRow.value
  if (!row) {
    return
  }
  if (action.startsWith('copy:')) {
    const copied = await capture.copyExchange(row.id, action.slice(5) as TrafficCopyKind)
    if (copied) {
      toast.value = '内容已复制到剪贴板'
    }
  } else if (action === 'hide:url') {
    hideTraffic('url', row.url)
  } else if (action === 'hide:host') {
    hideTraffic('host', row.host)
  } else if (action.startsWith('replay:')) {
    replayMode.value = action === 'replay:upstream' ? 'upstream' : 'mockAware'
    const draft = await capture.getReplayDraft(row.id)
    if (draft) {
      replayDraft.value = draft
      replayEditorOpen.value = true
    }
  } else {
    const detail = contextDetail.value ?? await capture.getTrafficDetail(row.id)
    if (detail) {
      createMockDraft(detail, action === 'mock:headers' ? 'headers' : 'body')
    }
  }
  closeTrafficMenu()
}

async function sendReplay(request: ReplayRequest) {
  const result = await capture.sendReplay(request)
  if (result) {
    replayEditorOpen.value = false
    replayDraft.value = null
    toast.value = result.mocked ? '已重新发送并命中 Mock' : '已重新发送到真实上游'
  }
}

async function saveMock(rule: MockRule) {
  await capture.saveMock(rule)
  if (!capture.error.value) {
    mockEditorOpen.value = false
    toast.value = 'Mock 规则已保存'
  }
}

async function moveMock(id: string, direction: -1 | 1) {
  const orderedIds = capture.mockRules.value.map((rule) => rule.id)
  const index = orderedIds.indexOf(id)
  const target = index + direction
  if (index < 0 || target < 0 || target >= orderedIds.length) {
    return
  }
  ;[orderedIds[index], orderedIds[target]] = [orderedIds[target], orderedIds[index]]
  await capture.reorderMocks(orderedIds)
}

async function testMocks(payload: { method: string; url: string }) {
  mockTestResult.value = await capture.testMocks(payload.method, payload.url)
}

async function exportHar() {
  await capture.api.exportHar()
  toast.value = 'HAR 已导出'
}

async function copyCurl(id: string) {
  await capture.api.copyCurl(id)
  toast.value = 'cURL 已复制到剪贴板'
}

async function exportCa() {
  await capture.api.exportCa()
  toast.value = '证书已导出'
}

async function exportDiagnostics() {
  if (capture.api.exportDiagnosticBundle) {
    await capture.api.exportDiagnosticBundle(capture.diagnosticLogs.value)
    toast.value = '诊断包已导出'
  }
}

async function saveNetworkProfile(profile: import('./shared/types').NetworkProfile) {
  await capture.saveNetworkProfile(profile)
  if (!capture.error.value) {
    networkEditorOpen.value = false
  }
}

async function selectDiagnosticTraffic(id: string) {
  diagnosticDetailLoading.value = true
  try {
    const detail = await capture.getTrafficDetail(id)
    diagnosticDetail.value = detail ? sanitizeTrafficDetail(detail) : null
  } finally {
    diagnosticDetailLoading.value = false
  }
}

onMounted(() => void capture.initialize())
</script>

<template>
  <div class="app-frame" :class="{ 'diagnostics-open': diagnosticsOpen }">
    <div class="app-shell" :class="{ 'toolbarless-active': toolbarlessView }">
    <AppSidebar
      :active-view="activeView"
      :traffic-count="capture.traffic.value.length"
      :mock-count="capture.mockRules.value.length"
      @change-view="activeView = $event"
    />

    <TopToolbar
      v-if="activeView === 'traffic'"
      :status="capture.proxyStatus.value"
      :busy="capture.busy.value"
      :network-profiles="capture.networkProfiles.value"
      :active-network-profile-id="capture.activeNetworkProfileId.value"
      @toggle-proxy="capture.toggleProxy"
      @clear="capture.clearTraffic"
      @export="exportHar"
      @activate-network-profile="capture.setActiveNetworkProfile"
      @add-network-profile="networkEditorOpen = true"
    />

    <div class="app-body" :class="{ 'full-workspace-layout': fullWorkspaceView }">
      <main class="workspace" :class="{ 'full-workspace': fullWorkspaceView }">
        <template v-if="activeView === 'traffic'">
          <DeviceTabs
            :model-value="deviceScope"
            :devices="trafficDevices"
            :total-count="capture.traffic.value.length"
            :local-count="capture.traffic.value.filter((row) => !row.deviceToken).length"
            @update:model-value="setDeviceScope"
          />
          <div class="filter-bar">
            <label class="search-control">
              <Search :size="14" />
              <input v-model="filter.query" type="search" placeholder="搜索 URL 或域名" aria-label="搜索网络请求" />
            </label>
            <FilterSelect v-model="filter.method" label="请求方法" :options="methodOptions" />
            <FilterSelect v-model="filter.status" label="状态码" :options="statusOptions" />
            <FilterSelect v-model="filter.host" label="域名" :options="hostOptions" wide />
            <button
              v-if="hiddenRuleCount"
              type="button"
              class="hidden-control"
              :aria-pressed="showHidden"
              @click="showHidden = !showHidden"
            >
              已屏蔽 {{ hiddenRuleCount }}
            </button>
            <button v-if="hiddenRuleCount" type="button" class="clear-hidden" title="清除全部屏蔽规则" @click="clearHiddenRules">
              <X :size="12" />
            </button>
            <span class="result-count">{{ visibleTraffic.length }} / {{ capture.traffic.value.length }}</span>
          </div>
          <TrafficTable
            :rows="visibleTraffic"
            :selected-id="capture.selectedDetail.value?.id ?? null"
            :hidden-ids="hiddenTrafficIds"
            @select="capture.selectTraffic"
            @contextmenu="openTrafficMenu"
          />
        </template>

        <MockRulesPanel
          v-else-if="activeView === 'mocks'"
          :rules="capture.mockRules.value"
          :test-result="mockTestResult"
          @add="openMockEditor(null)"
          @edit="openMockEditor"
          @toggle="capture.setMockEnabled"
          @delete="capture.deleteMock"
          @duplicate="capture.duplicateMock"
          @move="moveMock"
          @test="testMocks"
        />
        <SettingsPage
          v-else-if="activeView === 'settings'"
          :platform="capture.platform.value"
          :trust-state="capture.trustState.value"
          :private-key-state="capture.privateKeyState.value"
          :system-proxy-status="capture.systemProxyStatus.value"
          :proxy-status="capture.proxyStatus.value"
          :proxy-mode="capture.proxyMode.value"
          :port="capture.port.value"
          :lan-ip="capture.lanIp.value"
          :paired-devices="capture.pairedDevices.value"
          :historical-devices="historicalDevices"
          :pairing-idle-timeout="capture.pairingIdleTimeout.value"
          :pairing-challenge="capture.pairingChallenge.value"
          :capture-enabled="capture.captureEnabled.value"
          :system-proxy-preferred="capture.systemProxyPreferred.value"
          :system-proxy-enabled="capture.systemProxyEnabled.value"
          :ca-status="capture.caStatus.value"
          :busy="capture.busy.value"
          @update-port="setPort"
          @update-proxy-mode="capture.setProxyMode"
          @update-pairing-idle-timeout="capture.setPairingIdleTimeout"
          @update-capture="capture.setCapture"
          @update-system-proxy="capture.setSystemProxy"
          @toggle-proxy="capture.toggleProxy"
          @restore-system-proxy="capture.restoreSystemProxy"
          @generate-ca="capture.generateCa"
          @create-pairing-challenge="capture.createPairingChallenge()"
          @rename-device="capture.renamePairedDevice"
          @re-pair-device="capture.createPairingChallenge"
          @disconnect-device="capture.disconnectPairedDevice"
          @delete-device="deleteDeviceData"
          @export-ca="exportCa"
          @trust-ca="capture.trustCa"
          @remove-ca-trust="capture.removeCaTrust"
          @open-diagnostics="diagnosticsOpen = true"
          @export-diagnostics="exportDiagnostics"
        />
        <ScriptWorkspace
          v-else-if="activeView === 'scripts'"
          :scripts="capture.scripts.value"
          :busy="capture.busy.value"
          :test-result="capture.scriptTestResult.value"
          @save="capture.saveScript"
          @delete="capture.deleteScript"
          @toggle="capture.setScriptEnabled"
          @test="capture.testScript"
        />
        <PinningWorkspace
          v-else-if="activeView === 'pinning'"
          :environment="capture.fridaEnvironment.value"
          :devices="capture.fridaDevices.value"
          :applications="capture.fridaApplications.value"
          :scripts="capture.scripts.value"
          :status="capture.pinningSession.value"
          :logs="capture.pinningLogs.value"
          :busy="capture.busy.value"
          @refresh="capture.refreshFrida"
          @load-applications="capture.loadFridaApplications"
          @start="capture.startPinningSession"
          @stop="capture.stopPinningSession"
        />
        <AboutPage v-else />
      </main>

      <TrafficInspector
        v-if="activeView === 'traffic'"
        :detail="capture.selectedDetail.value"
        @copy-curl="copyCurl"
      />
      <aside v-else-if="activeView === 'mocks'" class="mock-inspector">
        <h2>规则执行顺序</h2>
        <p>启用的规则按优先级从高到低匹配；优先级相同时，较早创建的规则先执行。</p>
        <dl>
          <dt>匹配位置</dt>
          <dd>请求发往真实上游之前</dd>
          <dt>响应处理</dt>
          <dd>重算 Content-Length 并移除冲突的传输编码</dd>
          <dt>最大延迟</dt>
          <dd>120 秒</dd>
        </dl>
      </aside>
    </div>

    <footer class="status-bar">
      <span class="status-limits">上限 5000 条 · 正文 10 MB</span>
    </footer>

    <div v-if="mockEditorOpen" class="drawer-backdrop" @click.self="mockEditorOpen = false">
      <MockEditor :rule="editingRule" :initial-focus="mockInitialFocus" @save="saveMock" @cancel="mockEditorOpen = false" />
    </div>

    <div v-if="replayEditorOpen && replayDraft" class="drawer-backdrop" @click.self="replayEditorOpen = false">
      <ReplayEditor
        :draft="replayDraft"
        :initial-mode="replayMode"
        @send="sendReplay"
        @cancel="replayEditorOpen = false"
      />
    </div>

    <div v-if="networkEditorOpen" class="drawer-backdrop" @click.self="networkEditorOpen = false">
      <NetworkProfileEditor @save="saveNetworkProfile" @cancel="networkEditorOpen = false" />
    </div>

    <DisclaimerDialog
      :open="capture.disclaimerAccepted.value === false"
      @accept="capture.acceptDisclaimer"
    />

    <TrafficContextMenu
      :open="contextOpen"
      :x="contextPosition.x"
      :y="contextPosition.y"
      :disabled-reasons="contextDisabledReasons"
      @close="closeTrafficMenu"
      @action="handleTrafficAction"
    />

    <div v-if="capture.error.value" class="error-banner" role="alert">
      <div>
        <strong>{{ capture.error.value.message }}</strong>
        <span v-if="capture.error.value.action">{{ capture.error.value.action }}</span>
      </div>
      <button type="button" title="关闭错误" @click="capture.dismissError">
        <X :size="15" />
      </button>
    </div>

    <button v-if="toast" type="button" class="toast" @click="lastHidden ? undoLastHide() : toast = null">{{ toast }}</button>
    </div>

    <DiagnosticPanel
      v-if="diagnosticsOpen"
      :logs="capture.diagnosticLogs.value"
      :traffic="capture.traffic.value"
      :detail="diagnosticDetail"
      :loading-detail="diagnosticDetailLoading"
      @close="diagnosticsOpen = false"
      @clear-logs="capture.clearDiagnosticLogs"
      @select-traffic="selectDiagnosticTraffic"
    />
  </div>
</template>

<style scoped>
.app-frame {
  display: flex;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.app-shell {
  --sidebar-width: 184px;
  display: grid;
  min-width: 0;
  height: 100%;
  flex: 1 1 auto;
  padding: 10px 10px 0;
  grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
  grid-template-rows: 52px minmax(0, 1fr) 20px;
  grid-template-areas:
    "sidebar toolbar"
    "sidebar body"
    "sidebar status";
  column-gap: 10px;
  row-gap: 10px;
  background: var(--app-bg);
}

.app-shell > :deep(.sidebar) {
  grid-area: sidebar;
}

.app-shell > :deep(.toolbar) {
  grid-area: toolbar;
}

.app-shell.toolbarless-active {
  grid-template-rows: minmax(0, 1fr) 20px;
  grid-template-areas:
    "sidebar body"
    "sidebar status";
}

.app-body {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-area: body;
  grid-template-columns: minmax(440px, 1fr) minmax(330px, 390px);
  gap: 10px;
}

.app-body.full-workspace-layout {
  grid-template-columns: minmax(0, 1fr);
}

.workspace {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: 38px 42px minmax(0, 1fr);
  overflow: hidden;
  border-radius: var(--radius-panel);
  background: var(--surface);
  box-shadow: var(--shadow-surface);
}

.workspace > :only-child {
  grid-row: 1 / -1;
}

.workspace.full-workspace {
  grid-template-rows: minmax(0, 1fr);
}

.filter-bar {
  display: flex;
  min-width: 0;
  padding: 7px 10px;
  align-items: center;
  gap: 7px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-raised);
}

.search-control {
  display: flex;
  min-width: 150px;
  height: 29px;
  padding: 0 8px;
  align-items: center;
  gap: 6px;
  flex: 1;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text-muted);
}

.search-control input {
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text);
  font-family: inherit;
  font-size: var(--font-size-sm);
  flex: 1;
}

.result-count {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.hidden-control,
.clear-hidden {
  height: 29px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: var(--font-size-xs);
  white-space: nowrap;
}

.hidden-control {
  padding: 0 8px;
}

.hidden-control[aria-pressed='true'] {
  border-color: var(--accent);
  color: var(--accent);
}

.clear-hidden {
  display: grid;
  width: 29px;
  padding: 0;
  place-items: center;
}

.mock-inspector {
  padding: 20px;
  overflow: hidden;
  border-radius: var(--radius-panel);
  background: var(--surface-raised);
  box-shadow: var(--shadow-surface);
}

.mock-inspector h2 {
  margin: 0 0 8px;
  font-size: var(--font-size-xl);
}

.mock-inspector p,
.mock-inspector dt,
.mock-inspector dd {
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  line-height: 1.65;
}

.mock-inspector dl {
  display: grid;
  margin-top: 22px;
  grid-template-columns: 78px 1fr;
  gap: 8px;
}

.mock-inspector dt {
  color: var(--text-muted);
}

.mock-inspector dd {
  margin: 0;
}

.status-bar {
  display: flex;
  min-width: 0;
  grid-area: status;
  align-items: center;
  justify-content: flex-end;
  gap: 16px;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

.status-limits {
  justify-self: end;
  white-space: nowrap;
}

.drawer-backdrop {
  position: fixed;
  z-index: 70;
  inset: 0;
  display: flex;
  justify-content: flex-end;
  background: var(--overlay);
}

.drawer-backdrop > * {
  box-shadow: var(--shadow-panel);
}

.error-banner {
  position: fixed;
  z-index: 90;
  right: 18px;
  bottom: 38px;
  display: flex;
  width: min(430px, calc(100vw - 36px));
  padding: 12px 13px;
  align-items: start;
  justify-content: space-between;
  gap: 12px;
  border: 1px solid var(--danger-border);
  border-radius: var(--radius-control);
  background: var(--danger-soft);
  box-shadow: var(--shadow-float);
}

.error-banner div {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.error-banner strong {
  color: var(--danger);
  font-size: var(--font-size-sm);
}

.error-banner span {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.error-banner button {
  display: grid;
  width: 24px;
  height: 24px;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--text-muted);
}

.toast {
  position: fixed;
  z-index: 95;
  right: 18px;
  bottom: 38px;
  min-height: 32px;
  padding: 0 12px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-family: inherit;
  font-size: var(--font-size-xs);
  font-weight: 600;
}

@media (max-width: 1100px) {
  .app-shell {
    --sidebar-width: 160px;
  }

  .app-body {
    grid-template-columns: minmax(420px, 1fr) 320px;
  }
}

@media (max-width: 820px) {
  .app-shell {
    --sidebar-width: 64px;
  }

  .app-body {
    grid-template-columns: minmax(0, 1fr);
  }

  .app-body > :deep(.inspector),
  .app-body > .mock-inspector {
    display: none;
  }

}
</style>
