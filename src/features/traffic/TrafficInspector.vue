<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue'
import { Clipboard, Clock3, FileWarning, MousePointer2 } from '@lucide/vue'
import type { TrafficDetail } from '../../shared/types'

const { detail } = defineProps<{
  detail: TrafficDetail | null
}>()

const emit = defineEmits<{
  copyCurl: [id: string]
}>()

type InspectorTab = 'request' | 'response' | 'timing' | 'hex'
const activeTab = shallowRef<InspectorTab>('request')

watch(() => detail?.id, () => {
  activeTab.value = 'request'
})

const formattedResponse = computed(() => formatBody(detail?.responseBodyText, detail?.contentType))
const formattedRequest = computed(() => formatBody(
  detail?.requestBodyText,
  detail?.requestHeaders['content-type']
))
const hexBody = computed(() => formatHex(
  activeTab.value === 'hex' ? detail?.responseBody ?? [] : []
))

function formatBody(value: string | null | undefined, contentType?: string | null): string {
  if (value === '') {
    return '无正文'
  }
  if (value == null) {
    return '二进制或无法解码的正文，请在 Hex 中查看'
  }
  if (contentType?.includes('json')) {
    try {
      return JSON.stringify(JSON.parse(value), null, 2)
    } catch {
      return value
    }
  }
  return value
}

function formatHex(body: readonly number[]): string {
  if (body.length === 0) {
    return '无正文'
  }
  return body.map((value, index) => {
    const prefix = index % 16 === 0 ? `${index.toString(16).padStart(8, '0')}  ` : ''
    const suffix = index % 16 === 15 ? '\n' : ' '
    return `${prefix}${value.toString(16).padStart(2, '0')}${suffix}`
  }).join('').trimEnd()
}
</script>

<template>
  <aside class="inspector" aria-label="请求详情">
    <div v-if="!detail" class="inspector-empty">
      <MousePointer2 :size="28" :stroke-width="1.5" />
      <strong>选择一条请求</strong>
      <span>查看请求头、响应正文和耗时</span>
    </div>
    <template v-else>
      <header class="summary-header">
        <div class="summary-title">
          <span class="method">{{ detail.method }}</span>
          <strong>{{ detail.host }}</strong>
        </div>
        <button type="button" class="copy-button" @click="emit('copyCurl', detail.id)">
          <Clipboard :size="14" />
          复制 cURL
        </button>
        <p>{{ detail.url }}</p>
        <div class="summary-meta">
          <span>{{ detail.status ?? 'ERR' }}</span>
          <span>{{ detail.durationMs ?? 0 }} ms</span>
          <span>{{ detail.responseBytes }} B</span>
          <span v-if="detail.mocked" class="mocked">Mock</span>
        </div>
      </header>

      <div class="tab-list" role="tablist" aria-label="详情视图">
        <button type="button" role="tab" :aria-selected="activeTab === 'request'" @click="activeTab = 'request'">请求</button>
        <button type="button" role="tab" :aria-selected="activeTab === 'response'" @click="activeTab = 'response'">响应</button>
        <button type="button" role="tab" :aria-selected="activeTab === 'timing'" @click="activeTab = 'timing'">时序</button>
        <button type="button" role="tab" :aria-selected="activeTab === 'hex'" @click="activeTab = 'hex'">Hex</button>
      </div>

      <div class="inspector-content">
        <template v-if="activeTab === 'request'">
          <section class="detail-section">
            <h3>Headers</h3>
            <dl class="header-list">
              <template v-for="(value, name) in detail.requestHeaders" :key="name">
                <dt>{{ name }}</dt>
                <dd>{{ value }}</dd>
              </template>
            </dl>
          </section>
          <section class="detail-section body-section">
            <h3>Body</h3>
            <pre>{{ formattedRequest }}</pre>
          </section>
        </template>

        <template v-else-if="activeTab === 'response'">
          <div v-if="detail.truncated" class="truncated-warning">
            <FileWarning :size="14" />
            正文超过 10 MB，当前显示已截断内容
          </div>
          <section class="detail-section">
            <h3>Headers</h3>
            <dl class="header-list">
              <template v-for="(value, name) in detail.responseHeaders" :key="name">
                <dt>{{ name }}</dt>
                <dd>{{ value }}</dd>
              </template>
            </dl>
          </section>
          <section class="detail-section body-section">
            <h3>Body</h3>
            <pre>{{ formattedResponse }}</pre>
          </section>
        </template>

        <section v-else-if="activeTab === 'timing'" class="timing-view">
          <Clock3 :size="20" />
          <div class="timing-bar"><span :style="{ width: '100%' }" /></div>
          <strong>{{ detail.durationMs ?? 0 }} ms</strong>
          <p>当前版本记录代理接收请求到响应完成的总耗时。</p>
        </section>

        <section v-else class="detail-section body-section">
          <h3>Response bytes</h3>
          <pre>{{ hexBody }}</pre>
        </section>
      </div>
    </template>
  </aside>
</template>

<style scoped>
.inspector {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: auto auto minmax(0, 1fr);
  overflow: hidden;
  border-radius: var(--radius-panel);
  background: var(--surface-raised);
  box-shadow: var(--shadow-surface);
}

.inspector-empty {
  display: flex;
  min-height: 0;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 8px;
  color: var(--text-muted);
  font-size: 12px;
}

.inspector-empty strong {
  color: var(--text);
}

.summary-header {
  min-width: 0;
  padding: 15px 16px 13px;
  border-bottom: 1px solid var(--border);
}

.summary-title {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.summary-title strong {
  overflow: hidden;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.method {
  color: var(--accent-strong);
  font-size: 10px;
  font-weight: 800;
}

.summary-header p {
  margin: 9px 0;
  overflow: hidden;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 1.45;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary-meta {
  display: flex;
  gap: 12px;
  color: var(--text-muted);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}

.mocked {
  color: var(--mock);
  font-weight: 700;
}

.copy-button {
  position: absolute;
  top: 13px;
  right: 12px;
  display: inline-flex;
  height: 28px;
  align-items: center;
  gap: 5px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: 10px;
}

.summary-header {
  position: relative;
  padding-right: 104px;
}

.tab-list {
  display: flex;
  height: 34px;
  padding: 0 12px;
  align-items: end;
  gap: 16px;
  border-bottom: 1px solid var(--border);
}

.tab-list button {
  height: 34px;
  padding: 0 1px;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-family: inherit;
  font-size: 11px;
  font-weight: 600;
}

.tab-list button[aria-selected='true'] {
  border-bottom-color: var(--accent-strong);
  color: var(--text);
}

.inspector-content {
  min-height: 0;
  overflow: auto;
}

.detail-section {
  padding: 15px 16px;
  border-bottom: 1px solid var(--border-soft);
}

.detail-section h3 {
  margin: 0 0 10px;
  color: var(--text-muted);
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
}

.header-list {
  display: grid;
  margin: 0;
  grid-template-columns: minmax(90px, 36%) 1fr;
  gap: 7px 10px;
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 1.5;
}

.header-list dt {
  overflow-wrap: anywhere;
  color: var(--accent-strong);
}

.header-list dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
  color: var(--text-secondary);
}

.body-section pre {
  margin: 0;
  overflow-wrap: anywhere;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 1.65;
  white-space: pre-wrap;
}

.truncated-warning {
  display: flex;
  margin: 12px;
  padding: 8px 10px;
  align-items: center;
  gap: 7px;
  border-left: 2px solid var(--warning);
  background: var(--warning-soft);
  color: var(--text-secondary);
  font-size: 10px;
}

.timing-view {
  padding: 24px 18px;
  color: var(--text-secondary);
}

.timing-view {
  display: grid;
  grid-template-columns: 22px 1fr auto;
  align-items: center;
  gap: 10px;
}

.timing-view p {
  margin: 8px 0 0;
  grid-column: 2 / -1;
  color: var(--text-muted);
  font-size: 10px;
  line-height: 1.5;
}

.timing-bar {
  height: 6px;
  overflow: hidden;
  border-radius: 3px;
  background: var(--border);
}

.timing-bar span {
  display: block;
  height: 100%;
  background: var(--accent-strong);
}
</style>
