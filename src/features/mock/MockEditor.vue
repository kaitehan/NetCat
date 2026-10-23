<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, shallowRef, useTemplateRef, watch } from 'vue'
import { Save, X } from '@lucide/vue'
import BodyEditor from '../../components/BodyEditor.vue'
import { bodyEditorPolicy } from '../../components/body-editor-policy'
import type { MatchKind, MockRule } from '../../shared/types'

const { rule, initialFocus = null } = defineProps<{
  rule: MockRule | null
  initialFocus?: 'headers' | 'body' | null
}>()

const emit = defineEmits<{
  save: [rule: MockRule]
  cancel: []
}>()

const error = shallowRef<string | null>(null)
const headersText = shallowRef('Content-Type: application/json')
const draft = reactive<MockRule>(emptyRule())
const headersInput = useTemplateRef<HTMLTextAreaElement>('headersInput')
const bodyInput = useTemplateRef<{ focus: () => void }>('bodyInput')
const bodyPolicy = computed(() => bodyEditorPolicy(
  draft.responseBody,
  responseContentType.value,
  false
))
const responseContentType = computed(() => {
  const header = headersText.value
    .split('\n')
    .find((line) => line.slice(0, line.indexOf(':')).trim().toLowerCase() === 'content-type')
  return header?.slice(header.indexOf(':') + 1).trim()
})

function focusInitialField() {
  if (initialFocus === 'headers') {
    headersInput.value?.focus()
  } else if (initialFocus === 'body') {
    bodyInput.value?.focus()
  }
}

watch(
  () => rule,
  (value) => {
    Object.assign(draft, value ? cloneRule(value) : emptyRule())
    headersText.value = Object.entries(draft.responseHeaders)
      .map(([name, headerValue]) => `${name}: ${headerValue}`)
      .join('\n')
    error.value = null
    void nextTick(focusInitialField)
  },
  { immediate: true }
)

onMounted(focusInitialField)

function emptyRule(): MockRule {
  return {
    id: '',
    name: '',
    enabled: true,
    priority: 0,
    methods: ['GET'],
    matchKind: 'contains',
    urlPattern: '',
    status: 200,
    responseHeaders: {
      'Content-Type': 'application/json'
    },
    responseBody: '',
    delayMs: 0,
    createdAt: ''
  }
}

function cloneRule(value: MockRule): MockRule {
  return {
    id: value.id,
    name: value.name,
    enabled: value.enabled,
    priority: value.priority,
    methods: [...value.methods],
    matchKind: value.matchKind,
    urlPattern: value.urlPattern,
    status: value.status,
    responseHeaders: { ...value.responseHeaders },
    responseBody: value.responseBody,
    delayMs: value.delayMs,
    createdAt: value.createdAt
  }
}

function parseHeaders(value: string): Record<string, string> | null {
  const headers: Record<string, string> = {}
  for (const line of value.split('\n').map((item) => item.trim()).filter(Boolean)) {
    const separator = line.indexOf(':')
    if (separator < 1) {
      return null
    }
    headers[line.slice(0, separator).trim()] = line.slice(separator + 1).trim()
  }
  return headers
}

function save() {
  error.value = null
  if (!draft.name.trim()) {
    error.value = '请输入规则名称'
    return
  }
  if (!draft.urlPattern) {
    error.value = '请输入 URL 匹配内容'
    return
  }
  if (draft.matchKind === 'regex') {
    try {
      new RegExp(draft.urlPattern)
    } catch {
      error.value = '正则表达式无效'
      return
    }
  }
  if (draft.status < 100 || draft.status > 599) {
    error.value = '状态码必须在 100 到 599 之间'
    return
  }
  if (draft.delayMs < 0 || draft.delayMs > 120000) {
    error.value = '延迟必须在 0 到 120000 毫秒之间'
    return
  }
  const headers = parseHeaders(headersText.value)
  if (!headers) {
    error.value = '响应头必须使用“名称: 值”格式'
    return
  }
  if ((headers['Content-Type'] ?? '').includes('json') && draft.responseBody.trim()) {
    try {
      JSON.parse(draft.responseBody)
    } catch {
      error.value = '响应正文不是有效 JSON'
      return
    }
  }
  emit('save', {
    ...cloneRule(draft),
    name: draft.name.trim(),
    urlPattern: draft.urlPattern.trim(),
    responseHeaders: headers
  })
}

function updateMatchKind(event: Event) {
  draft.matchKind = (event.target as HTMLSelectElement).value as MatchKind
}
</script>

<template>
  <form class="mock-editor" @submit.prevent="save">
    <header class="editor-header">
      <div>
        <h2>{{ rule ? '编辑 Mock' : '新建 Mock' }}</h2>
        <p>匹配请求并直接返回本地响应</p>
      </div>
      <button type="button" class="icon-button" title="关闭" @click="emit('cancel')">
        <X :size="17" />
      </button>
    </header>

    <div class="editor-body">
      <label class="field">
        <span>规则名称</span>
        <input v-model="draft.name" type="text" placeholder="例如：用户列表成功响应" />
      </label>

      <div class="field-row">
        <label class="field">
          <span>请求方法</span>
          <select v-model="draft.methods[0]">
            <option v-for="method in ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']" :key="method" :value="method">
              {{ method }}
            </option>
          </select>
        </label>
        <label class="field">
          <span>匹配方式</span>
          <select :value="draft.matchKind" @change="updateMatchKind">
            <option value="contains">包含</option>
            <option value="exact">精确</option>
            <option value="regex">正则</option>
          </select>
        </label>
      </div>

      <label class="field">
        <span>URL 匹配</span>
        <input v-model="draft.urlPattern" type="text" placeholder="/api/users" />
      </label>

      <div class="field-row field-row-three">
        <label class="field">
          <span>状态码</span>
          <input v-model.number="draft.status" type="number" min="100" max="599" />
        </label>
        <label class="field">
          <span>优先级</span>
          <input v-model.number="draft.priority" type="number" />
        </label>
        <label class="field">
          <span>延迟 (ms)</span>
          <input v-model.number="draft.delayMs" type="number" min="0" max="120000" />
        </label>
      </div>

      <label class="field">
        <span>响应头</span>
        <textarea ref="headersInput" v-model="headersText" rows="3" spellcheck="false" />
      </label>

      <div class="field field-grow">
        <span>响应正文</span>
        <BodyEditor
          ref="bodyInput"
          v-model="draft.responseBody"
          :language="bodyPolicy.language"
          :diagnostics="bodyPolicy.diagnostics"
          :ariaLabel="'响应正文'"
          class="mock-body-editor"
        />
      </div>

      <p v-if="error" class="form-error" role="alert">{{ error }}</p>
    </div>

    <footer class="editor-footer">
      <button type="button" class="secondary-button" @click="emit('cancel')">取消</button>
      <button type="submit" class="primary-button">
        <Save :size="15" />
        保存规则
      </button>
    </footer>
  </form>
</template>

<style scoped>
.mock-editor {
  display: grid;
  width: min(520px, 100vw);
  height: 100%;
  grid-template-rows: auto minmax(0, 1fr) auto;
  background: var(--surface);
}

.editor-header,
.editor-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 18px;
  border-bottom: 1px solid var(--border);
}

.editor-footer {
  justify-content: flex-end;
  gap: 8px;
  border-top: 1px solid var(--border);
  border-bottom: 0;
}

.editor-header h2 {
  margin: 0;
  font-size: 15px;
  line-height: 1.4;
}

.editor-header p {
  margin: 3px 0 0;
  color: var(--text-muted);
  font-size: 11px;
}

.editor-body {
  display: flex;
  min-height: 0;
  padding: 18px;
  overflow: auto;
  flex-direction: column;
  gap: 14px;
}

.field-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.field-row-three {
  grid-template-columns: repeat(3, 1fr);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 600;
}

.field input,
.field select,
.field textarea {
  width: 100%;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-control);
  outline: none;
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
  font-size: 12px;
  font-weight: 400;
}

.mock-body-editor {
  min-height: 280px;
  flex: 1;
}

.field input,
.field select {
  height: 32px;
  padding: 0 9px;
}

.field textarea {
  padding: 9px;
  resize: vertical;
  font-family: var(--font-mono);
  line-height: 1.5;
}

.field input:focus,
.field select:focus,
.field textarea:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.form-error {
  margin: 0;
  padding: 8px 10px;
  border-left: 2px solid var(--danger);
  background: var(--danger-soft);
  color: var(--danger);
  font-size: 11px;
}

.icon-button,
.primary-button,
.secondary-button {
  display: inline-flex;
  height: 32px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border-radius: var(--radius-control);
  font-family: inherit;
  font-size: 12px;
  font-weight: 600;
}

.icon-button {
  width: 32px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
}

.primary-button {
  padding: 0 12px;
  border: 1px solid var(--accent-strong);
  background: var(--accent);
  color: var(--on-accent);
}

.secondary-button {
  padding: 0 12px;
  border: 1px solid var(--border-strong);
  background: var(--surface-raised);
  color: var(--text);
}
</style>
