<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef } from 'vue'
import { Braces, ChevronDown, ChevronRight, CircleAlert, FileWarning } from '@lucide/vue'
import type { TrafficSummary } from '../../shared/types'
import { calculateVirtualWindow } from './virtual-window'

const { rows, selectedId, hiddenIds = new Set<string>() } = defineProps<{
  rows: readonly TrafficSummary[]
  selectedId: string | null
  hiddenIds?: ReadonlySet<string>
}>()

const emit = defineEmits<{
  select: [row: TrafficSummary]
  contextmenu: [payload: { row: TrafficSummary; x: number; y: number }]
}>()

const viewport = useTemplateRef<HTMLElement>('viewport')
const scrollTop = shallowRef(0)
const viewportHeight = shallowRef(480)
const expandedHosts = shallowRef<ReadonlySet<string>>(new Set())
let resizeObserver: ResizeObserver | undefined

interface DomainGroup {
  host: string
  rows: TrafficSummary[]
  mockedCount: number
  errorCount: number
}

type TableEntry =
  | { kind: 'group'; key: string; group: DomainGroup }
  | { kind: 'traffic'; key: string; row: TrafficSummary }

const groups = computed<DomainGroup[]>(() => {
  const grouped = new Map<string, DomainGroup>()
  for (const row of rows) {
    const host = row.host || '未知域名'
    const existing = grouped.get(host)
    if (existing) {
      existing.rows.push(row)
      existing.mockedCount += Number(row.mocked)
      existing.errorCount += Number(row.error !== null || (row.status !== null && row.status >= 400))
      continue
    }
    grouped.set(host, {
      host,
      rows: [row],
      mockedCount: Number(row.mocked),
      errorCount: Number(row.error !== null || (row.status !== null && row.status >= 400))
    })
  }
  return [...grouped.values()]
})

const entries = computed<TableEntry[]>(() => groups.value.flatMap((group) => {
  const groupEntry: TableEntry = {
    kind: 'group',
    key: `group:${group.host}`,
    group
  }
  if (expandedHosts.value.has(group.host)) {
    return [
      groupEntry,
      ...group.rows.map<TableEntry>((row) => ({
        kind: 'traffic',
        key: `traffic:${row.id}`,
        row
      }))
    ]
  }
  return [groupEntry]
}))

const virtualWindow = computed(() => calculateVirtualWindow({
  itemCount: entries.value.length,
  rowHeight: 38,
  viewportHeight: viewportHeight.value,
  scrollTop: scrollTop.value,
  overscan: 6
}))

const visibleEntries = computed(() => entries.value.slice(
  virtualWindow.value.start,
  virtualWindow.value.end
))

function toggleHost(host: string) {
  const next = new Set(expandedHosts.value)
  if (next.has(host)) {
    next.delete(host)
  } else {
    next.add(host)
  }
  expandedHosts.value = next
}

function handleScroll(event: Event) {
  scrollTop.value = (event.currentTarget as HTMLElement).scrollTop
}

function openContextMenu(event: MouseEvent, row: TrafficSummary) {
  event.preventDefault()
  emit('contextmenu', { row, x: event.clientX, y: event.clientY })
}

function formatBytes(value: number): string {
  if (value < 1024) {
    return `${value} B`
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`
  }
  return `${(value / 1024 / 1024).toFixed(1)} MB`
}

onMounted(() => {
  if (!viewport.value || typeof ResizeObserver === 'undefined') {
    return
  }
  resizeObserver = new ResizeObserver(([entry]) => {
    viewportHeight.value = entry.contentRect.height
  })
  resizeObserver.observe(viewport.value)
})

onBeforeUnmount(() => resizeObserver?.disconnect())
</script>

<template>
  <section class="traffic-table" aria-label="网络请求列表">
    <div class="table-head" aria-hidden="true">
      <span>方法</span>
      <span>请求</span>
      <span>状态</span>
      <span>耗时</span>
      <span>大小</span>
    </div>
    <div
      ref="viewport"
      class="table-viewport"
      @scroll="handleScroll"
    >
      <div v-if="rows.length === 0" class="empty-state">
        <Braces :size="28" :stroke-width="1.6" />
        <strong>等待网络请求</strong>
        <span>启动代理后，请求会实时出现在这里</span>
      </div>
      <template v-else>
        <div :style="{ height: `${virtualWindow.before}px` }" />
        <template v-for="entry in visibleEntries" :key="entry.key">
          <button
            v-if="entry.kind === 'group'"
            type="button"
            class="domain-row"
            :aria-label="`${expandedHosts.has(entry.group.host) ? '收起' : '展开'} ${entry.group.host} 分组`"
            :aria-expanded="expandedHosts.has(entry.group.host)"
            @click="toggleHost(entry.group.host)"
          >
            <ChevronDown v-if="expandedHosts.has(entry.group.host)" :size="14" />
            <ChevronRight v-else :size="14" />
            <strong>{{ entry.group.host }}</strong>
            <span class="domain-count">{{ entry.group.rows.length }}</span>
            <span v-if="entry.group.mockedCount" class="domain-summary mock-summary">
              {{ entry.group.mockedCount }} Mock
            </span>
            <span v-if="entry.group.errorCount" class="domain-summary error-summary">
              {{ entry.group.errorCount }} 错误
            </span>
          </button>
          <button
            v-else
            type="button"
            class="traffic-row"
            :class="{ selected: entry.row.id === selectedId, hidden: hiddenIds.has(entry.row.id) }"
            :aria-label="`${entry.row.method} ${entry.row.url}`"
            @click="emit('select', entry.row)"
            @contextmenu="openContextMenu($event, entry.row)"
          >
            <span class="method" :data-method="entry.row.method">{{ entry.row.method }}</span>
            <span class="request-cell">
              <span class="request-path">{{ entry.row.url.replace(/^https?:\/\/[^/]+/, '') || '/' }}</span>
              <span v-if="entry.row.mocked" class="mock-mark">Mock</span>
              <span v-if="hiddenIds.has(entry.row.id)" class="hidden-mark">已屏蔽</span>
              <FileWarning v-if="entry.row.truncated" :size="13" aria-label="正文已截断" />
              <CircleAlert v-if="entry.row.error" :size="13" class="error-icon" aria-label="请求失败" />
            </span>
            <span :class="['status', { failed: entry.row.status !== null && entry.row.status >= 400 }]">
              {{ entry.row.status ?? 'ERR' }}
            </span>
            <span class="muted">{{ entry.row.durationMs === null ? '—' : `${entry.row.durationMs} ms` }}</span>
            <span class="muted">{{ formatBytes(entry.row.responseBytes) }}</span>
          </button>
        </template>
        <div :style="{ height: `${virtualWindow.after}px` }" />
      </template>
    </div>
  </section>
</template>

<style scoped>
.traffic-table {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: 34px minmax(0, 1fr);
  background: var(--surface);
}

.table-head,
.traffic-row {
  display: grid;
  grid-template-columns: 88px minmax(196px, 1fr) 64px 72px 74px;
  align-items: center;
}

.domain-row {
  display: flex;
  width: 100%;
  height: 38px;
  padding: 0 12px;
  align-items: center;
  gap: 7px;
  border: 0;
  border-bottom: 1px solid var(--border);
  background: var(--surface-raised);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: 11px;
  text-align: left;
  cursor: default;
}

.domain-row:hover {
  background: var(--surface-hover);
}

.domain-row strong {
  overflow: hidden;
  color: var(--text);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.domain-count {
  min-width: 20px;
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--border-soft);
  color: var(--text-muted);
  font-size: 9px;
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.domain-summary {
  margin-left: 3px;
  font-size: 9px;
  font-weight: 600;
}

.mock-summary {
  color: var(--mock);
}

.error-summary {
  color: var(--danger);
}

.table-head {
  padding: 0 12px;
  border-bottom: 1px solid var(--border);
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 600;
}

.table-viewport {
  min-height: 0;
  overflow: auto;
}

.traffic-row {
  width: 100%;
  height: 38px;
  padding: 0 12px;
  border: 0;
  border-bottom: 1px solid var(--border-soft);
  background: transparent;
  color: var(--text);
  font-family: inherit;
  font-size: 12px;
  text-align: left;
  cursor: default;
}

.traffic-row .method {
  padding-left: 18px;
}

.traffic-row:hover {
  background: var(--surface-hover);
}

.traffic-row.selected {
  background: var(--selection);
  box-shadow: inset 2px 0 var(--accent);
}

.traffic-row.hidden {
  opacity: 0.52;
}

.method {
  min-width: 0;
  overflow: hidden;
  color: var(--accent-strong);
  font-size: 10px;
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.request-cell {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 7px;
}

.request-path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mock-mark {
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--mock-soft);
  color: var(--mock);
  font-size: 9px;
  font-weight: 700;
}

.hidden-mark {
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--border-soft);
  color: var(--text-muted);
  font-size: 9px;
  font-weight: 700;
}

.status {
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}

.status.failed,
.error-icon {
  color: var(--danger);
}

.muted {
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.empty-state {
  display: flex;
  height: 100%;
  min-height: 300px;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 8px;
  color: var(--text-muted);
  font-size: 12px;
}

.empty-state strong {
  color: var(--text);
  font-size: 13px;
}
</style>
