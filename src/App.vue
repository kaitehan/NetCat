<script setup lang="ts">
import { computed, onMounted, reactive, shallowRef } from 'vue'
import { Search, X } from '@lucide/vue'
import AppSidebar from './components/AppSidebar.vue'
import TopToolbar from './components/TopToolbar.vue'
import CertificatePanel from './features/certificate/CertificatePanel.vue'
import MockEditor from './features/mock/MockEditor.vue'
import MockRulesPanel from './features/mock/MockRulesPanel.vue'
import DisclaimerDialog from './features/onboarding/DisclaimerDialog.vue'
import TrafficInspector from './features/traffic/TrafficInspector.vue'
import TrafficContextMenu, { type TrafficMenuAction } from './features/traffic/TrafficContextMenu.vue'
import TrafficTable from './features/traffic/TrafficTable.vue'
import { filterTraffic } from './features/traffic/traffic-filter'
import { useNetCapture } from './composables/useNetCapture'
import type { MockRule, ReplayMode, TrafficCopyKind, TrafficDetail, TrafficFilter, TrafficSummary } from './shared/types'

const capture = useNetCapture()
const activeView = shallowRef<'traffic' | 'mocks'>('traffic')
const certificateOpen = shallowRef(false)
const mockEditorOpen = shallowRef(false)
const editingRule = shallowRef<MockRule | null>(null)
const mockInitialFocus = shallowRef<'headers' | 'body' | null>(null)
const toast = shallowRef<string | null>(null)
const hiddenUrls = shallowRef<ReadonlySet<string>>(new Set())
const hiddenHosts = shallowRef<ReadonlySet<string>>(new Set())
const showHidden = shallowRef(false)
const lastHidden = shallowRef<{ kind: 'url' | 'host'; value: string } | null>(null)
const contextRow = shallowRef<TrafficSummary | null>(null)
const contextDetail = shallowRef<TrafficDetail | null>(null)
const contextPosition = reactive({ x: 0, y: 0 })
const filter = reactive<TrafficFilter>({
  query: '',
  method: 'ALL',
  status: 'ALL',
  host: ''
})

const hiddenTrafficIds = computed<ReadonlySet<string>>(() => new Set(
  capture.traffic.value
    .filter((row) => hiddenUrls.value.has(row.url) || hiddenHosts.value.has(row.host))
    .map((row) => row.id)
))
const filteredTraffic = computed(() => filterTraffic(capture.traffic.value, filter))
const visibleTraffic = computed(() => showHidden.value
  ? filteredTraffic.value
  : filteredTraffic.value.filter((row) => !hiddenTrafficIds.value.has(row.id)))
const hosts = computed(() => [...new Set(capture.traffic.value.map((row) => row.host))].sort())
const hiddenRuleCount = computed(() => hiddenUrls.value.size + hiddenHosts.value.size)
const contextOpen = computed(() => contextRow.value !== null)
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

function setAllowLan(value: boolean) {
  capture.allowLan.value = value
}

function setPort(value: number) {
  capture.port.value = value
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
    createdAt: ''
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
    const mode: ReplayMode = action === 'replay:upstream' ? 'upstream' : 'mockAware'
    const result = await capture.replayExchange(row.id, mode)
    if (result) {
      toast.value = result.mocked ? '已重新发送并命中 Mock' : '已重新发送到真实上游'
    }
  } else {
    const detail = contextDetail.value ?? await capture.getTrafficDetail(row.id)
    if (detail) {
      createMockDraft(detail, action === 'mock:headers' ? 'headers' : 'body')
    }
  }
  closeTrafficMenu()
}

async function saveMock(rule: MockRule) {
  await capture.saveMock(rule)
  if (!capture.error.value) {
    mockEditorOpen.value = false
    toast.value = 'Mock 规则已保存'
  }
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

onMounted(() => void capture.initialize())
</script>

<template>
  <div class="app-shell">
    <TopToolbar
      :status="capture.proxyStatus.value"
      :allow-lan="capture.allowLan.value"
      :capture-enabled="capture.captureEnabled.value"
      :system-proxy-enabled="capture.systemProxyEnabled.value"
      :ca-generated="capture.caStatus.value.generated"
      :port="capture.port.value"
      :busy="capture.busy.value"
      @toggle-proxy="capture.toggleProxy"
      @update-allow-lan="setAllowLan"
      @update-port="setPort"
      @toggle-capture="capture.setCapture"
      @toggle-system-proxy="capture.toggleSystemProxy"
      @clear="capture.clearTraffic"
      @export="exportHar"
      @certificate="certificateOpen = true"
    />

    <div class="app-body">
      <AppSidebar
        :active-view="activeView"
        :traffic-count="capture.traffic.value.length"
        :mock-count="capture.mockRules.value.length"
        @change-view="activeView = $event"
      />

      <main class="workspace">
        <template v-if="activeView === 'traffic'">
          <div class="filter-bar">
            <label class="search-control">
              <Search :size="14" />
              <input v-model="filter.query" type="search" placeholder="搜索 URL 或域名" aria-label="搜索流量" />
            </label>
            <select v-model="filter.method" aria-label="请求方法">
              <option value="ALL">全部方法</option>
              <option v-for="method in ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']" :key="method" :value="method">{{ method }}</option>
            </select>
            <select v-model="filter.status" aria-label="状态码">
              <option value="ALL">全部状态</option>
              <option value="2xx">2xx</option>
              <option value="3xx">3xx</option>
              <option value="4xx">4xx</option>
              <option value="5xx">5xx</option>
              <option value="ERR">错误</option>
            </select>
            <select v-model="filter.host" aria-label="域名">
              <option value="">全部域名</option>
              <option v-for="host in hosts" :key="host" :value="host">{{ host }}</option>
            </select>
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
          v-else
          :rules="capture.mockRules.value"
          @add="openMockEditor(null)"
          @edit="openMockEditor"
          @toggle="capture.setMockEnabled"
          @delete="capture.deleteMock"
        />
      </main>

      <TrafficInspector
        v-if="activeView === 'traffic'"
        :detail="capture.selectedDetail.value"
        @copy-curl="copyCurl"
      />
      <aside v-else class="mock-inspector">
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
      <span>{{ capture.proxyStatus.value.state === 'running' ? 'HTTP/1.1 + HTTPS 捕获中' : '就绪' }}</span>
      <span v-if="capture.proxyStatus.value.lanIp">手机代理 {{ capture.proxyStatus.value.lanIp }}:{{ capture.proxyStatus.value.port }}</span>
      <span class="status-spacer" />
      <span>上限 5000 条 · 正文 10 MB</span>
    </footer>

    <div v-if="mockEditorOpen" class="drawer-backdrop" @click.self="mockEditorOpen = false">
      <MockEditor :rule="editingRule" :initial-focus="mockInitialFocus" @save="saveMock" @cancel="mockEditorOpen = false" />
    </div>

    <CertificatePanel
      :open="certificateOpen"
      :ca-status="capture.caStatus.value"
      :proxy-status="capture.proxyStatus.value"
      :allow-lan="capture.allowLan.value"
      :busy="capture.busy.value"
      @close="certificateOpen = false"
      @generate="capture.generateCa"
      @export="exportCa"
    />

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
</template>

<style scoped>
.app-shell {
  display: grid;
  width: 100%;
  height: 100%;
  grid-template-rows: 48px minmax(0, 1fr) 24px;
  background: var(--app-bg);
}

.app-body {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-columns: 172px minmax(440px, 1fr) minmax(320px, 390px);
}

.workspace {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: 42px minmax(0, 1fr);
  border-left: 1px solid #0e1713;
}

.workspace > :only-child {
  grid-row: 1 / -1;
}

.filter-bar {
  display: flex;
  min-width: 0;
  padding: 6px 10px;
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
  border-radius: 4px;
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
  font-size: 11px;
  flex: 1;
}

.filter-bar select {
  height: 29px;
  max-width: 132px;
  padding: 0 7px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: 10px;
}

.result-count {
  color: var(--text-muted);
  font-size: 9px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.hidden-control,
.clear-hidden {
  height: 29px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: 10px;
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
  border-left: 1px solid var(--border);
  background: var(--surface-raised);
}

.mock-inspector h2 {
  margin: 0 0 8px;
  font-size: 14px;
}

.mock-inspector p,
.mock-inspector dt,
.mock-inspector dd {
  color: var(--text-secondary);
  font-size: 11px;
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
  padding: 0 10px 0 184px;
  align-items: center;
  gap: 14px;
  border-top: 1px solid var(--border);
  background: var(--surface-raised);
  color: var(--text-muted);
  font-size: 9px;
}

.status-spacer {
  flex: 1;
}

.drawer-backdrop {
  position: fixed;
  z-index: 70;
  inset: 0;
  display: flex;
  justify-content: flex-end;
  background: rgb(15 20 18 / 38%);
}

.drawer-backdrop > * {
  box-shadow: -18px 0 54px rgb(0 0 0 / 15%);
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
  border: 1px solid #e8b0aa;
  border-radius: 6px;
  background: var(--danger-soft);
  box-shadow: 0 12px 34px rgb(0 0 0 / 14%);
}

.error-banner div {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.error-banner strong {
  color: var(--danger);
  font-size: 11px;
}

.error-banner span {
  color: var(--text-secondary);
  font-size: 10px;
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
  border: 1px solid #9fdabd;
  border-radius: 5px;
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-family: inherit;
  font-size: 10px;
  font-weight: 600;
}

@media (max-width: 1100px) {
  .app-body {
    grid-template-columns: 154px minmax(420px, 1fr) 320px;
  }
}

@media (max-width: 820px) {
  .app-body {
    grid-template-columns: 64px minmax(0, 1fr);
  }

  .app-body > :last-child {
    display: none;
  }

  .status-bar {
    padding-left: 74px;
  }
}
</style>
