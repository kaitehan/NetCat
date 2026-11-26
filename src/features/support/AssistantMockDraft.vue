<script setup lang="ts">
import { computed } from 'vue'
import { ArrowRight, Braces } from '@lucide/vue'
import type { AssistantDraft, MockRule } from '../../shared/types'

const { draft, pending } = defineProps<{
  draft: AssistantDraft
  pending: boolean
}>()

const emit = defineEmits<{
  prepare: [rule: MockRule]
}>()

const rule = computed(() => ({
  ...(draft.payload as unknown as MockRule),
  enabled: false
}))
const superseded = computed(() => draft.status === 'superseded')
</script>

<template>
  <section class="assistant-draft" aria-label="Mock 草稿">
    <header>
      <span><Braces :size="14" />{{ superseded ? 'Mock 草稿 · 已被后续操作替代' : 'Mock 草稿' }}</span>
      <strong>{{ rule.name }}</strong>
    </header>
    <p class="draft-target">{{ rule.methods.join(', ') }} · {{ rule.urlPattern }}</p>
    <pre v-if="!superseded">{{ rule.status }}
{{ rule.responseBody }}</pre>
    <p v-else class="superseded-status">状态码：{{ rule.status }}</p>
    <footer v-if="!superseded">
      <span>是否按以上信息建立 Mock 规则？确认后会打开 Mock 页面供你检查。</span>
      <button
        type="button"
        :disabled="pending || draft.status !== 'pending'"
        aria-label="按以上信息创建 Mock 规则"
        @click="emit('prepare', rule)"
      >
        创建规则
        <ArrowRight :size="13" />
      </button>
    </footer>
  </section>
</template>

<style scoped>
.assistant-draft {
  width: 100%;
  margin-top: 8px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
}

header,
header span,
footer,
button {
  display: flex;
  align-items: center;
}

header {
  min-width: 0;
  justify-content: space-between;
  gap: 8px;
}

header span {
  gap: 5px;
  color: var(--accent-strong);
  font-size: var(--font-size-xs);
  font-weight: 600;
}

header strong {
  min-width: 0;
  overflow: hidden;
  font-size: var(--font-size-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.draft-target {
  margin: 7px 0;
  overflow-wrap: anywhere;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-2xs);
}

.superseded-status {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

pre {
  max-height: 110px;
  margin: 0;
  padding: 7px;
  overflow: auto;
  border: 1px solid var(--border-soft);
  border-radius: 4px;
  background: var(--surface);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-2xs);
  white-space: pre-wrap;
}

footer {
  min-width: 0;
  margin-top: 9px;
  justify-content: space-between;
  gap: 8px;
}

footer > span {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

button {
  height: 29px;
  padding: 0 9px;
  justify-content: center;
  gap: 5px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-family: inherit;
  font-size: var(--font-size-xs);
  font-weight: 600;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
</style>
