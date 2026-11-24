<script setup lang="ts">
import { computed } from 'vue'
import { Globe, LoaderCircle, Play, ShieldCheck, ShieldAlert } from '@lucide/vue'
import type { AssistantDraft } from '../../shared/types'

const { draft, pending } = defineProps<{
  draft: AssistantDraft
  pending: boolean
}>()

const emit = defineEmits<{
  execute: []
}>()

const domain = computed(() => String(draft.payload.domain ?? '未知域名'))
const resolvedIps = computed(() => Array.isArray(draft.payload.resolvedIps)
  ? draft.payload.resolvedIps.map(String)
  : [])
const method = computed(() => String(draft.payload.method ?? 'GET'))
const bodyBytes = computed(() => Number(draft.payload.bodyBytes ?? 0))
const sensitiveCount = computed(() => Array.isArray(draft.payload.sensitiveFields)
  ? draft.payload.sensitiveFields.length
  : 0)
const tlsVerification = computed(() => draft.payload.tlsVerification !== false)
const insecureOptionIgnored = computed(() => draft.payload.insecureOptionIgnored === true)

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  return `${(bytes / 1024).toFixed(1)} KB`
}
</script>

<template>
  <section class="network-draft" aria-label="联网诊断确认">
    <header class="network-draft-header">
      <div class="network-target">
        <Globe :size="15" />
        <strong>{{ domain }}</strong>
      </div>
      <span class="request-summary">{{ method }} · {{ formatBytes(bodyBytes) }}</span>
    </header>

    <div v-if="resolvedIps.length > 0" class="resolved-ips" aria-label="解析地址">
      <code v-for="ip in resolvedIps" :key="ip">{{ ip }}</code>
    </div>

    <div class="network-safety">
      <span :class="{ warning: !tlsVerification }">
        <ShieldCheck v-if="tlsVerification" :size="13" />
        <ShieldAlert v-else :size="13" />
        {{ tlsVerification ? 'TLS 校验已启用' : 'TLS 校验不可用' }}
      </span>
      <span v-if="sensitiveCount > 0" class="warning">
        <ShieldAlert :size="13" />
        检测到 {{ sensitiveCount }} 个敏感字段
      </span>
    </div>

    <p v-if="insecureOptionIgnored" class="network-warning">
      已忽略 cURL 中关闭证书校验的选项。
    </p>

    <footer class="network-draft-footer">
      <span>此授权仅生效一次，有效期五分钟。</span>
      <button
        type="button"
        class="execute-button"
        :disabled="pending || draft.status !== 'pending'"
        aria-label="执行联网诊断"
        @click="emit('execute')"
      >
        <LoaderCircle v-if="pending" :size="13" class="draft-spinner" />
        <Play v-else :size="13" />
        执行
      </button>
    </footer>
  </section>
</template>

<style scoped>
.network-draft {
  width: 100%;
  margin-top: 8px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
}

.network-draft-header,
.network-target,
.network-safety,
.network-safety span,
.network-draft-footer,
.execute-button {
  display: flex;
  align-items: center;
}

.network-draft-header {
  min-width: 0;
  justify-content: space-between;
  gap: 8px;
}

.network-target {
  min-width: 0;
  gap: 6px;
  color: var(--accent-strong);
}

.network-target strong {
  min-width: 0;
  overflow: hidden;
  font-size: var(--font-size-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.request-summary {
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.resolved-ips {
  display: flex;
  min-width: 0;
  margin-top: 8px;
  flex-wrap: wrap;
  gap: 5px;
}

.resolved-ips code {
  padding: 2px 5px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface);
  color: var(--text-secondary);
  font-size: var(--font-size-2xs);
}

.network-safety {
  margin-top: 9px;
  flex-wrap: wrap;
  gap: 6px 12px;
  color: var(--success);
  font-size: var(--font-size-xs);
}

.network-safety span {
  gap: 4px;
}

.network-safety .warning {
  color: var(--warning);
}

.network-warning {
  margin: 8px 0 0;
  padding: 6px 7px;
  border: 1px solid var(--warning);
  border-radius: 4px;
  background: var(--warning-soft);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.network-draft-footer {
  min-width: 0;
  margin-top: 10px;
  justify-content: space-between;
  gap: 8px;
}

.network-draft-footer > span {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

.execute-button {
  height: 29px;
  padding: 0 9px;
  justify-content: center;
  gap: 5px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--accent);
  color: white;
  font-family: inherit;
  font-size: var(--font-size-xs);
  font-weight: 600;
}

.execute-button:hover:not(:disabled) {
  background: var(--accent-strong);
}

.execute-button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.draft-spinner {
  animation: network-spin 900ms linear infinite;
}

@keyframes network-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 560px) {
  .network-draft-header,
  .network-draft-footer {
    align-items: flex-start;
    flex-direction: column;
  }

  .execute-button {
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .draft-spinner {
    animation: none;
  }
}
</style>
