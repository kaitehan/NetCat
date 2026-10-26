<script setup lang="ts">
import { computed, reactive, shallowRef } from 'vue'
import { Plus, Send, Trash2, X } from '@lucide/vue'
import BodyEditor from '../../components/BodyEditor.vue'
import { bodyEditorPolicy } from '../../components/body-editor-policy'
import type { ReplayDraft, ReplayMode, ReplayRequest } from '../../shared/types'
import {
  buildReplayUrl,
  bytesToHex,
  hexToBytes,
  parseHeaderLines,
  splitReplayUrl,
  type QueryEntry
} from './replay-request'

const { draft, initialMode } = defineProps<{
  draft: ReplayDraft
  initialMode: ReplayMode
}>()

const emit = defineEmits<{
  send: [request: ReplayRequest]
  cancel: []
}>()

const split = splitReplayUrl(draft.url)
const method = shallowRef(draft.method)
const baseUrl = shallowRef(split.baseUrl)
const mode = shallowRef(initialMode)
const query = reactive<QueryEntry[]>(split.query)
const headersText = shallowRef(Object.entries(draft.headers).map(([name, value]) => `${name}: ${value}`).join('\n'))
const bodyText = shallowRef(draft.binary ? bytesToHex(draft.body) : (draft.bodyText ?? ''))
const error = shallowRef<string | null>(null)
const bodyPolicy = computed(() => draft.binary
  ? { value: bodyText.value, language: 'plaintext' as const, diagnostics: false }
  : bodyEditorPolicy(bodyText.value, draft.contentType, false))
const sideEffecting = computed(() => ['POST', 'PUT', 'PATCH', 'DELETE'].includes(method.value.toUpperCase()))

function addQuery() {
  query.push({ name: '', value: '' })
}

function removeQuery(index: number) {
  query.splice(index, 1)
}

function submit() {
  error.value = null
  try {
    const url = buildReplayUrl(baseUrl.value, query)
    const headers = parseHeaderLines(headersText.value)
    const body = draft.binary
      ? hexToBytes(bodyText.value)
      : [...new TextEncoder().encode(bodyText.value)]
    if ((draft.contentType ?? '').toLowerCase().includes('json') && bodyText.value.trim()) {
      JSON.parse(bodyText.value)
    }
    if (sideEffecting.value && !window.confirm(`${method.value.toUpperCase()} 请求可能产生业务副作用，确认继续发送？`)) {
      return
    }
    emit('send', {
      sourceExchangeId: draft.sourceExchangeId,
      method: method.value.toUpperCase(),
      url,
      headers,
      body,
      mode: mode.value
    })
  } catch (cause) {
    error.value = cause instanceof Error
      ? cause.message
      : '请求内容无效'
  }
}
</script>

<template>
  <form class="replay-editor" @submit.prevent="submit">
    <header class="replay-header">
      <div>
        <h2>编辑并重发</h2>
        <p>新请求会关联到原请求 {{ draft.sourceExchangeId }}</p>
      </div>
      <button type="button" class="icon-button" aria-label="关闭重发编辑器" @click="emit('cancel')">
        <X :size="17" />
      </button>
    </header>

    <div class="replay-content">
      <div class="request-line">
        <label>
          <span>Method</span>
          <select v-model="method" aria-label="请求方法">
            <option v-for="item in ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']" :key="item">{{ item }}</option>
          </select>
        </label>
        <label class="url-field">
          <span>URL</span>
          <input v-model="baseUrl" aria-label="请求 URL" />
        </label>
      </div>

      <fieldset class="mode-control">
        <legend>发送模式</legend>
        <label><input v-model="mode" type="radio" value="upstream" />直接访问上游</label>
        <label><input v-model="mode" type="radio" value="mockAware" />应用当前 Mock</label>
      </fieldset>

      <section class="query-section">
        <div class="section-title">
          <span>Query</span>
          <button type="button" class="icon-button" title="添加 Query" @click="addQuery"><Plus :size="15" /></button>
        </div>
        <div v-for="(entry, index) in query" :key="index" class="query-row">
          <input v-model="entry.name" :aria-label="`Query 名称 ${index + 1}`" placeholder="name" />
          <input v-model="entry.value" :aria-label="`Query 值 ${index + 1}`" placeholder="value" />
          <button type="button" class="icon-button" title="删除 Query" @click="removeQuery(index)"><Trash2 :size="14" /></button>
        </div>
      </section>

      <label class="field">
        <span>请求头</span>
        <textarea v-model="headersText" aria-label="请求头" rows="5" spellcheck="false" />
      </label>

      <div class="field body-field">
        <span>{{ draft.binary ? '请求正文（Hex）' : '请求正文' }}</span>
        <BodyEditor
          v-model="bodyText"
          :language="bodyPolicy.language"
          :diagnostics="bodyPolicy.diagnostics"
          :ariaLabel="'请求正文'"
        />
      </div>

      <p v-if="sideEffecting" class="side-effect-warning">该请求方法可能修改上游业务数据，发送前会再次确认。</p>
      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    </div>

    <footer class="replay-footer">
      <button type="button" class="secondary-button" @click="emit('cancel')">取消</button>
      <button type="submit" class="primary-button"><Send :size="15" />发送请求</button>
    </footer>
  </form>
</template>

<style scoped>
.replay-editor {
  display: grid;
  width: min(620px, 100vw);
  height: 100%;
  grid-template-rows: auto minmax(0, 1fr) auto;
  background: var(--surface);
}

.replay-header,
.replay-footer {
  display: flex;
  padding: 16px 18px;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
}

.replay-footer {
  justify-content: flex-end;
  gap: 8px;
  border-top: 1px solid var(--border);
  border-bottom: 0;
}

.replay-header h2,
.replay-header p {
  margin: 0;
}

.replay-header h2 {
  font-size: var(--font-size-xl);
}

.replay-header p {
  margin-top: 4px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.replay-content {
  display: flex;
  min-height: 0;
  padding: 18px;
  overflow: auto;
  flex-direction: column;
  gap: 14px;
}

.request-line {
  display: grid;
  grid-template-columns: 110px minmax(0, 1fr);
  gap: 10px;
}

.request-line label,
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--text-muted);
  font-size: var(--font-size-sm);
  font-weight: 600;
}

.request-line input,
.request-line select,
.query-row input,
.field textarea {
  min-width: 0;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-control);
  outline: none;
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-md);
}

.request-line input,
.request-line select,
.query-row input {
  height: 32px;
  padding: 0 9px;
}

.field textarea {
  padding: 9px;
  resize: vertical;
  font-family: var(--font-mono);
  line-height: 1.5;
}

.mode-control {
  display: flex;
  margin: 0;
  padding: 10px;
  gap: 18px;
  border: 1px solid var(--border);
}

.mode-control legend,
.mode-control label,
.section-title {
  color: var(--text-muted);
  font-size: var(--font-size-sm);
}

.query-section {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.section-title,
.query-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 30px;
  align-items: center;
  gap: 8px;
}

.section-title span {
  grid-column: 1 / 3;
  font-weight: 600;
}

.body-field :deep(.body-editor) {
  height: 300px;
}

.side-effect-warning,
.form-error {
  margin: 0;
  padding: 8px 10px;
  border-left: 2px solid var(--warning);
  background: var(--warning-soft);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
}

.form-error {
  border-left-color: var(--danger);
  background: var(--danger-soft);
  color: var(--danger);
}

.icon-button,
.primary-button,
.secondary-button {
  display: inline-flex;
  height: 31px;
  padding: 0 10px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text-secondary);
  font-family: inherit;
}

.icon-button {
  width: 30px;
  padding: 0;
}

.primary-button {
  border-color: var(--accent-strong);
  background: var(--accent);
  color: var(--on-accent);
}
</style>
