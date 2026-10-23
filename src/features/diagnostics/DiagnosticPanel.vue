<script setup lang="ts">
import { computed, shallowRef } from 'vue'
import { CircleAlert, CircleCheck, CircleX, Network, Terminal, Trash2, X } from '@lucide/vue'
import type {
  DiagnosticLevel,
  DiagnosticLog,
  DiagnosticTrafficDetail,
  TrafficSummary
} from '../../shared/types'

const {
  logs,
  traffic,
  detail,
  loadingDetail
} = defineProps<{
  logs: readonly DiagnosticLog[]
  traffic: readonly TrafficSummary[]
  detail: DiagnosticTrafficDetail | null
  loadingDetail: boolean
}>()

const emit = defineEmits<{
  close: []
  clearLogs: []
  selectTraffic: [id: string]
}>()

type PanelTab = 'console' | 'network'
type LevelFilter = 'ALL' | DiagnosticLevel

const activeTab = shallowRef<PanelTab>('console')
const levelFilter = shallowRef<LevelFilter>('ALL')
const selectedTrafficId = shallowRef<string | null>(null)
const resizing = shallowRef(false)
const panelWidth = shallowRef(clampWidth(Math.round(window.innerWidth * 0.45)))

const filteredLogs = computed(() => levelFilter.value === 'ALL'
  ? logs
  : logs.filter((log) => log.level === levelFilter.value))
const panelStyle = computed(() => ({ width: `${panelWidth.value}px` }))
const requestHeaderLines = computed(() => detail ? formatHeaders(detail.requestHeaders) : [])
const responseHeaderLines = computed(() => detail ? formatHeaders(detail.responseHeaders) : [])

function clampWidth(width: number): number {
  const maximum = Math.max(280, Math.min(760, window.innerWidth - 320))
  const minimum = Math.min(360, maximum)
  return Math.max(minimum, Math.min(maximum, width))
}

function formatHeaders(headers: Readonly<Record<string, string>>): string[] {
  return Object.entries(headers)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, value]) => `${name}: ${value}`)
}

function formatDetails(log: DiagnosticLog): string {
  return log.details && Object.keys(log.details).length > 0
    ? JSON.stringify(log.details, null, 2)
    : ''
}

function formatTime(timestamp: string): string {
  const date = new Date(timestamp)
  return Number.isNaN(date.getTime())
    ? timestamp
    : date.toLocaleTimeString('zh-CN', { hour12: false })
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`
  }
  return `${(bytes / 1024).toFixed(1)} KB`
}

function selectTraffic(id: string) {
  selectedTrafficId.value = id
  emit('selectTraffic', id)
}

function startResize(event: PointerEvent) {
  resizing.value = true
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}

function resizePanel(event: PointerEvent) {
  if (!resizing.value) {
    return
  }
  panelWidth.value = clampWidth(window.innerWidth - event.clientX)
}

function stopResize() {
  resizing.value = false
}
</script>

<template>
  <aside
    class="diagnostic-panel diagnostic-panel--dark"
    :style="panelStyle"
    aria-label="调试面板"
  >
    <div
      class="resize-handle"
      role="separator"
      aria-label="调整调试面板宽度"
      aria-orientation="vertical"
      @pointerdown="startResize"
      @pointermove="resizePanel"
      @pointerup="stopResize"
      @pointercancel="stopResize"
    />

    <header class="panel-header">
      <div class="panel-tabs" role="tablist" aria-label="调试模块">
        <button
          type="button"
          role="tab"
          :aria-selected="activeTab === 'console'"
          @click="activeTab = 'console'"
        >
          <Terminal :size="14" />
          Console
        </button>
        <button
          type="button"
          role="tab"
          aria-label="Network"
          :aria-selected="activeTab === 'network'"
          @click="activeTab = 'network'"
        >
          <Network :size="14" />
          Network
          <span>{{ traffic.length }}</span>
        </button>
      </div>
      <button type="button" class="icon-button" aria-label="关闭调试面板" @click="emit('close')">
        <X :size="15" />
      </button>
    </header>

    <section v-if="activeTab === 'console'" class="console-panel" aria-label="Console 内容">
      <div class="panel-toolbar">
        <label>
          <span>级别</span>
          <select v-model="levelFilter" aria-label="日志级别">
            <option value="ALL">全部</option>
            <option value="info">信息</option>
            <option value="warn">警告</option>
            <option value="error">错误</option>
          </select>
        </label>
        <span class="toolbar-count">{{ filteredLogs.length }} 条</span>
        <button type="button" class="toolbar-button" aria-label="清空日志" @click="emit('clearLogs')">
          <Trash2 :size="13" />
          清空
        </button>
      </div>

      <div class="console-view" role="log" aria-label="运行日志">
        <div v-if="filteredLogs.length === 0" class="empty-state">
          暂无诊断日志
        </div>
        <article
          v-for="(log, index) in filteredLogs"
          :key="`${log.timestamp}-${log.event}-${index}`"
          class="log-entry"
          :class="`log-entry--${log.level}`"
        >
          <CircleCheck v-if="log.level === 'info'" :size="13" />
          <CircleAlert v-else-if="log.level === 'warn'" :size="13" />
          <CircleX v-else :size="13" />
          <time :datetime="log.timestamp">{{ formatTime(log.timestamp) }}</time>
          <span class="log-scope">{{ log.scope }}</span>
          <div class="log-content">
            <div>
              <strong>{{ log.message }}</strong>
              <code>{{ log.event }}</code>
            </div>
            <pre v-if="formatDetails(log)">{{ formatDetails(log) }}</pre>
          </div>
        </article>
      </div>
    </section>

    <template v-else>
      <div class="network-view">
        <div class="network-list" aria-label="本地网络请求">
          <div class="network-heading">
            <span>请求</span>
            <span>状态</span>
          </div>
          <div v-if="traffic.length === 0" class="empty-state">
            暂无网络请求
          </div>
          <button
            v-for="row in traffic"
            :key="row.id"
            type="button"
            class="network-row"
            :class="{ selected: selectedTrafficId === row.id }"
            :aria-label="`${row.method} ${row.url}`"
            @click="selectTraffic(row.id)"
          >
            <span class="method">{{ row.method }}</span>
            <span class="request-url">{{ row.url }}</span>
            <span class="request-status" :class="{ failed: row.error || (row.status ?? 0) >= 400 }">
              {{ row.status ?? 'ERR' }}
            </span>
          </button>
        </div>

        <section class="network-detail" aria-label="请求诊断详情">
          <div v-if="loadingDetail" class="empty-state">正在读取请求详情...</div>
          <div v-else-if="!detail" class="empty-state">选择一条请求查看脱敏信息</div>
          <template v-else>
            <header class="detail-title">
              <span class="method">{{ detail.method }}</span>
              <strong>{{ detail.url }}</strong>
            </header>
            <div class="detail-metrics">
              <span>状态 {{ detail.status ?? 'ERR' }}</span>
              <span>耗时 {{ detail.durationMs ?? 0 }} ms</span>
              <span>请求正文 {{ formatBytes(detail.requestOriginalBytes) }}</span>
              <span>响应正文 {{ formatBytes(detail.responseOriginalBytes) }}</span>
            </div>
            <section class="header-block">
              <h3>Request Headers</h3>
              <code v-for="line in requestHeaderLines" :key="line">{{ line }}</code>
            </section>
            <section class="header-block">
              <h3>Response Headers</h3>
              <code v-for="line in responseHeaderLines" :key="line">{{ line }}</code>
            </section>
          </template>
        </section>
      </div>
    </template>
  </aside>
</template>

<style scoped>
.diagnostic-panel {
  position: relative;
  display: grid;
  min-width: 280px;
  height: 100%;
  flex: 0 0 auto;
  grid-template-rows: 42px minmax(0, 1fr);
  overflow: hidden;
  border-left: 1px solid #303640;
  color: #d8dee9;
  font-family: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

.diagnostic-panel--dark {
  color-scheme: dark;
  background: #15181d;
}

.resize-handle {
  position: absolute;
  z-index: 3;
  top: 0;
  bottom: 0;
  left: -3px;
  width: 7px;
  cursor: col-resize;
  touch-action: none;
}

.resize-handle:hover,
.resize-handle:focus-visible {
  background: #3b82f6;
}

.panel-header {
  display: flex;
  min-width: 0;
  padding: 0 8px;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid #303640;
  background: #1b1f25;
}

.panel-tabs {
  display: flex;
  min-width: 0;
  height: 100%;
  align-items: stretch;
}

.panel-tabs button {
  position: relative;
  display: flex;
  min-width: 96px;
  padding: 0 12px;
  align-items: center;
  justify-content: center;
  gap: 7px;
  border: 0;
  background: transparent;
  color: #939cab;
  font: inherit;
  font-size: 11px;
}

.panel-tabs button[aria-selected='true'] {
  color: #f5f7fa;
}

.panel-tabs button[aria-selected='true']::after {
  position: absolute;
  right: 8px;
  bottom: 0;
  left: 8px;
  height: 2px;
  background: #58a6ff;
  content: '';
}

.panel-tabs span {
  color: #727c8c;
  font-size: 9px;
}

.icon-button,
.toolbar-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  background: transparent;
  color: #9ba4b3;
  font: inherit;
}

.icon-button {
  width: 28px;
  height: 28px;
  padding: 0;
}

.icon-button:hover,
.toolbar-button:hover {
  color: #ffffff;
}

.panel-toolbar {
  display: flex;
  min-width: 0;
  height: 34px;
  padding: 0 9px;
  align-items: center;
  gap: 10px;
  border-bottom: 1px solid #292e36;
  background: #171b20;
}

.console-panel {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: 34px minmax(0, 1fr);
  overflow: hidden;
}

.panel-toolbar label {
  display: flex;
  align-items: center;
  gap: 6px;
  color: #7f8998;
  font-size: 9px;
}

.panel-toolbar select {
  height: 24px;
  padding: 0 23px 0 7px;
  border: 1px solid #343b46;
  border-radius: 4px;
  background: #20252c;
  color: #d8dee9;
  font: inherit;
  font-size: 10px;
}

.toolbar-count {
  margin-left: auto;
  color: #727c8c;
  font-size: 9px;
}

.toolbar-button {
  height: 26px;
  padding: 0 5px;
  gap: 5px;
  font-size: 10px;
}

.console-view {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  background: #111419;
  font-family: var(--font-mono);
}

.log-entry {
  display: grid;
  min-width: 0;
  padding: 8px 10px;
  grid-template-columns: 14px 60px 48px minmax(0, 1fr);
  align-items: start;
  gap: 6px;
  border-bottom: 1px solid #242932;
  color: #b9c1ce;
  font-size: 10px;
  line-height: 1.5;
}

.log-entry > svg {
  margin-top: 1px;
  color: #58a6ff;
}

.log-entry--warn > svg {
  color: #e5b44b;
}

.log-entry--error > svg {
  color: #ff6b6b;
}

.log-entry time,
.log-scope {
  color: #687384;
  font-variant-numeric: tabular-nums;
}

.log-scope {
  text-transform: uppercase;
}

.log-content {
  min-width: 0;
}

.log-content > div {
  display: flex;
  min-width: 0;
  align-items: baseline;
  gap: 8px;
}

.log-content strong {
  color: #dce3ed;
  font-weight: 500;
}

.log-content code {
  overflow: hidden;
  color: #7c8797;
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.log-content pre {
  margin: 5px 0 0;
  color: #8bb9ef;
  font: inherit;
  white-space: pre-wrap;
  word-break: break-word;
}

.empty-state {
  display: grid;
  min-height: 92px;
  padding: 20px;
  place-items: center;
  color: #687384;
  font-size: 10px;
  text-align: center;
}

.network-view {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: minmax(180px, 42%) minmax(0, 1fr);
  background: #111419;
}

.network-list,
.network-detail {
  min-width: 0;
  min-height: 0;
  overflow: auto;
}

.network-list {
  border-bottom: 1px solid #303640;
}

.network-heading {
  position: sticky;
  z-index: 1;
  top: 0;
  display: grid;
  height: 28px;
  padding: 0 10px;
  grid-template-columns: 1fr 44px;
  align-items: center;
  border-bottom: 1px solid #303640;
  background: #1b1f25;
  color: #7f8998;
  font-size: 9px;
}

.network-heading span:last-child {
  text-align: right;
}

.network-row {
  display: grid;
  width: 100%;
  min-width: 0;
  height: 34px;
  padding: 0 10px;
  grid-template-columns: 42px minmax(0, 1fr) 44px;
  align-items: center;
  gap: 7px;
  border: 0;
  border-bottom: 1px solid #242932;
  background: transparent;
  color: #c9d1dc;
  font: inherit;
  text-align: left;
}

.network-row:hover {
  background: #1d2229;
}

.network-row.selected {
  background: #243449;
}

.method {
  color: #70b7ff;
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
}

.request-url {
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.request-status {
  color: #75d19a;
  font-family: var(--font-mono);
  font-size: 9px;
  text-align: right;
}

.request-status.failed {
  color: #ff7b7b;
}

.network-detail {
  padding: 12px;
}

.detail-title {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: start;
  gap: 8px;
}

.detail-title strong {
  overflow-wrap: anywhere;
  color: #e5eaf1;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 500;
  line-height: 1.5;
}

.detail-metrics {
  display: flex;
  margin: 12px 0;
  flex-wrap: wrap;
  gap: 6px;
}

.detail-metrics span {
  padding: 3px 6px;
  border: 1px solid #343b46;
  border-radius: 4px;
  background: #1b2026;
  color: #9ca6b5;
  font-size: 9px;
}

.header-block {
  margin-top: 14px;
}

.header-block h3 {
  margin: 0 0 7px;
  color: #8f9baa;
  font-size: 9px;
  font-weight: 600;
  text-transform: uppercase;
}

.header-block code {
  display: block;
  padding: 3px 0;
  overflow-wrap: anywhere;
  color: #c7cfda;
  font-family: var(--font-mono);
  font-size: 9px;
  line-height: 1.45;
}
</style>
