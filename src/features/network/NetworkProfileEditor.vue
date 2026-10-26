<script setup lang="ts">
import { Save, X } from '@lucide/vue'
import { reactive } from 'vue'
import type { MatchKind, NetworkProfile } from '../../shared/types'

const emit = defineEmits<{
  save: [profile: NetworkProfile]
  cancel: []
}>()

const draft = reactive({
  name: '',
  latencyMs: 100,
  jitterMs: 20,
  downloadKbps: 1600 as number | null,
  uploadKbps: 750 as number | null,
  timeoutPercent: 0,
  disconnectPercent: 0,
  methods: '',
  matchKind: 'contains' as MatchKind,
  urlPattern: ''
})

function save() {
  if (!draft.name.trim()) {
    return
  }
  emit('save', {
    id: '',
    name: draft.name.trim(),
    kind: 'custom',
    latencyMs: Math.max(0, draft.latencyMs),
    jitterMs: Math.max(0, draft.jitterMs),
    downloadKbps: normalizeRate(draft.downloadKbps),
    uploadKbps: normalizeRate(draft.uploadKbps),
    timeoutPercent: clampPercent(draft.timeoutPercent),
    disconnectPercent: clampPercent(draft.disconnectPercent),
    methods: draft.methods.split(',').map((value) => value.trim().toUpperCase()).filter(Boolean),
    matchKind: draft.matchKind,
    urlPattern: draft.urlPattern.trim(),
    builtin: false,
    createdAt: ''
  })
}

function normalizeRate(value: number | null) {
  return value && value > 0 ? value : null
}

function clampPercent(value: number) {
  return Math.min(100, Math.max(0, value))
}
</script>

<template>
  <form class="network-editor" role="dialog" aria-modal="true" aria-labelledby="network-editor-title" @submit.prevent="save">
    <header>
      <div>
        <h2 id="network-editor-title">自定义弱网</h2>
        <p>配置仅作用于 NetCat 代理转发，不模拟真实 IP 丢包或 TCP 重排。</p>
      </div>
      <button type="button" aria-label="关闭自定义弱网" @click="emit('cancel')">
        <X :size="16" />
      </button>
    </header>

    <div class="fields">
      <label class="wide">
        <span>名称</span>
        <input v-model.trim="draft.name" aria-label="配置名称" required />
      </label>
      <label><span>延迟 ms</span><input v-model.number="draft.latencyMs" type="number" min="0" aria-label="延迟" /></label>
      <label><span>抖动 ms</span><input v-model.number="draft.jitterMs" type="number" min="0" aria-label="抖动" /></label>
      <label><span>下行 Kbps</span><input v-model.number="draft.downloadKbps" type="number" min="0" aria-label="下行速率" /></label>
      <label><span>上行 Kbps</span><input v-model.number="draft.uploadKbps" type="number" min="0" aria-label="上行速率" /></label>
      <label><span>超时概率 %</span><input v-model.number="draft.timeoutPercent" type="number" min="0" max="100" aria-label="超时概率" /></label>
      <label><span>中断概率 %</span><input v-model.number="draft.disconnectPercent" type="number" min="0" max="100" aria-label="中断概率" /></label>
      <label>
        <span>匹配方式</span>
        <select v-model="draft.matchKind" aria-label="匹配方式">
          <option value="contains">包含</option>
          <option value="exact">精确</option>
          <option value="regex">正则</option>
        </select>
      </label>
      <label><span>方法</span><input v-model="draft.methods" aria-label="请求方法列表" placeholder="GET, POST；留空表示全部" /></label>
      <label class="wide"><span>URL 匹配</span><input v-model="draft.urlPattern" aria-label="URL 匹配" placeholder="留空表示全部 URL" /></label>
    </div>

    <footer>
      <button type="button" @click="emit('cancel')">取消</button>
      <button type="submit" class="primary" :disabled="!draft.name.trim()">
        <Save :size="14" />
        保存并启用
      </button>
    </footer>
  </form>
</template>

<style scoped>
.network-editor {
  width: min(620px, calc(100vw - 32px));
  border: 1px solid var(--border);
  border-radius: var(--radius-panel);
  background: var(--surface);
  box-shadow: var(--shadow-overlay);
}

header,
footer {
  display: flex;
  padding: 16px 18px;
  align-items: center;
  justify-content: space-between;
}

header {
  border-bottom: 1px solid var(--border);
}

header h2,
header p {
  margin: 0;
}

header h2 {
  font-size: var(--font-size-xl);
}

header p {
  margin-top: 4px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

header button,
footer button {
  display: inline-flex;
  min-height: 31px;
  padding: 0 10px;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
}

header button {
  width: 31px;
  padding: 0;
  justify-content: center;
}

.fields {
  display: grid;
  padding: 18px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

label {
  display: grid;
  gap: 5px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

label.wide {
  grid-column: 1 / -1;
}

input,
select {
  width: 100%;
  height: 32px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-sm);
}

footer {
  justify-content: flex-end;
  gap: 8px;
  border-top: 1px solid var(--border);
}

footer .primary {
  border-color: var(--accent);
  background: var(--accent);
  color: white;
}

@media (max-width: 640px) {
  .fields {
    grid-template-columns: 1fr;
  }

  label.wide {
    grid-column: auto;
  }
}
</style>
