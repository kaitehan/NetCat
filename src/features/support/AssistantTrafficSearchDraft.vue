<script setup lang="ts">
import { computed } from 'vue'
import { ArrowRight } from '@lucide/vue'
import type { AssistantDraft } from '../../shared/types'

interface TrafficSearchFilter {
  query: string
  domain: string
  method: string | null
  statusGroup: number | null
}

interface TrafficSearchSummary {
  method: string
  url: string
  status: number | null
  durationMs: number | null
  startedAt: string
}

const { draft, pending = false } = defineProps<{
  draft: AssistantDraft
  pending?: boolean
}>()

const emit = defineEmits<{
  apply: [draftId: string, filter: TrafficSearchFilter]
}>()

const payload = draft.payload as {
  filter: TrafficSearchFilter
  total: number
  summaries: TrafficSearchSummary[]
}

const filterLabels = computed(() => {
  const labels: string[] = []
  if (payload.filter.query) labels.push(`关键词：${payload.filter.query}`)
  if (payload.filter.domain) labels.push(`域名：${payload.filter.domain}（含子域名）`)
  if (payload.filter.method) labels.push(`方法：${payload.filter.method}`)
  if (payload.filter.statusGroup) labels.push(`状态：${payload.filter.statusGroup}xx`)
  return labels
})
const superseded = computed(() => draft.status === 'superseded')
</script>

<template>
  <section class="traffic-search-draft" aria-label="抓包搜索结果">
    <header class="draft-header">
      <strong>{{ superseded ? '网络请求筛选' : `找到 ${payload.total} 条请求` }}</strong>
      <span>{{ superseded ? '已被后续操作替代' : `展示前 ${payload.summaries.length} 条摘要` }}</span>
    </header>
    <div class="filter-summary" aria-label="筛选条件">
      <strong>筛选条件</strong>
      <ul>
        <li v-for="label in filterLabels" :key="label">{{ label }}</li>
      </ul>
    </div>
    <ul v-if="!superseded && payload.summaries.length > 0" class="summary-list">
      <li v-for="item in payload.summaries" :key="`${item.startedAt}-${item.method}-${item.url}`">
        <span class="method">{{ item.method }}</span>
        <span class="url">{{ item.url }}</span>
        <span class="metrics">{{ item.status ?? 'ERR' }} · {{ item.durationMs ?? '-' }} ms</span>
      </li>
    </ul>
    <div v-else-if="!superseded" class="empty-summary">当前抓包中没有匹配请求</div>
    <button
      v-if="!superseded"
      type="button"
      class="apply-button"
      :disabled="pending || draft.status !== 'pending'"
      @click="emit('apply', draft.id, payload.filter)"
    >
      在网络请求中查看
      <ArrowRight :size="14" />
    </button>
  </section>
</template>

<style scoped>
.traffic-search-draft {
  display: grid;
  margin: 2px 0 10px;
  padding: 10px;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-panel);
  background: var(--surface-raised);
}

.draft-header {
  display: flex;
  min-width: 0;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.draft-header strong {
  color: var(--text);
  font-size: var(--font-size-sm);
}

.draft-header span,
.empty-summary {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

.filter-summary {
  display: grid;
  gap: 5px;
}

.filter-summary strong {
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.filter-summary ul {
  display: flex;
  min-width: 0;
  margin: 0;
  padding: 0;
  flex-wrap: wrap;
  gap: 5px;
  list-style: none;
}

.filter-summary li {
  max-width: 100%;
  padding: 3px 6px;
  overflow: hidden;
  border: 1px solid var(--border-soft);
  border-radius: 4px;
  background: var(--surface);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-2xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary-list {
  display: grid;
  margin: 0;
  padding: 0;
  gap: 4px;
  list-style: none;
}

.summary-list li {
  display: grid;
  min-width: 0;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
  font-size: var(--font-size-xs);
}

.method,
.metrics {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

.url {
  min-width: 0;
  overflow: hidden;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.metrics {
  color: var(--text-muted);
}

.apply-button {
  display: inline-flex;
  min-height: 30px;
  padding: 0 10px;
  justify-self: end;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--accent);
  color: white;
  font-family: inherit;
  font-size: var(--font-size-xs);
}

.apply-button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.apply-button:disabled {
  opacity: 0.5;
}
</style>
