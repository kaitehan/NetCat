<script setup lang="ts">
import { computed } from 'vue'
import { Code2, Save } from '@lucide/vue'
import type { AssistantDraft, ProxyScript } from '../../shared/types'

const { draft, pending } = defineProps<{
  draft: AssistantDraft
  pending: boolean
}>()

const emit = defineEmits<{
  save: [script: ProxyScript]
}>()

const script = computed(() => ({
  ...(draft.payload as unknown as ProxyScript),
  enabled: false
}))
</script>

<template>
  <section class="assistant-draft" aria-label="QuickJS 草稿">
    <header>
      <span><Code2 :size="14" />QuickJS 草稿</span>
      <strong>{{ script.name }}</strong>
    </header>
    <p>{{ script.methods.join(', ') }} · {{ script.urlPattern }}</p>
    <pre>{{ script.source }}</pre>
    <footer>
      <span>保存后保持停用，可在脚本页面检查并启用。</span>
      <button
        type="button"
        :disabled="pending || draft.status !== 'pending'"
        aria-label="保存 QuickJS 草稿"
        @click="emit('save', script)"
      >
        <Save :size="13" />
        保存
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
  color: var(--success);
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

p {
  margin: 7px 0;
  overflow-wrap: anywhere;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-2xs);
}

pre {
  max-height: 130px;
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
