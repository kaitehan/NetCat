<script setup lang="ts">
import { Braces, Play, Plus, Save, Trash2 } from '@lucide/vue'
import { reactive, shallowRef } from 'vue'
import BodyEditor from '../../components/BodyEditor.vue'
import type { ProxyScript, ScriptTestResult } from '../../shared/types'

defineProps<{
  scripts: readonly ProxyScript[]
  busy: boolean
  testResult: ScriptTestResult | null
}>()

const emit = defineEmits<{
  save: [script: ProxyScript]
  delete: [id: string]
  toggle: [id: string, enabled: boolean]
  test: [script: ProxyScript]
}>()

const selectedId = shallowRef<string | null>(null)
const editing = shallowRef(false)
const draft = reactive<ProxyScript>(emptyScript())

function emptyScript(): ProxyScript {
  return {
    id: '',
    name: '',
    enabled: true,
    priority: 0,
    methods: ['GET'],
    matchKind: 'contains',
    urlPattern: '',
    source: 'function onRequest(ctx) {\n  return {};\n}\n\nfunction onResponse(ctx) {\n  return {};\n}',
    createdAt: ''
  }
}

function edit(script: ProxyScript) {
  Object.assign(draft, { ...script, methods: [...script.methods] })
  selectedId.value = script.id
  editing.value = true
}

function createScript() {
  Object.assign(draft, emptyScript())
  selectedId.value = null
  editing.value = true
}

function save() {
  if (!draft.name.trim() || !draft.source.trim()) {
    return
  }
  emit('save', { ...draft, methods: [...draft.methods] })
}
</script>

<template>
  <section class="script-workspace" aria-labelledby="scripts-title">
    <header class="script-header">
      <div>
        <h1 id="scripts-title">JavaScript 脚本</h1>
        <p>代理脚本运行在隔离 QuickJS 环境。</p>
      </div>
      <button type="button" class="primary-action" aria-label="新建脚本" @click="createScript">
        <Plus :size="15" />
        新建脚本
      </button>
    </header>

    <div class="script-body">
      <aside class="script-list" aria-label="脚本列表">
        <div v-if="!scripts.length" class="empty-state">
          <Braces :size="26" />
          <span>暂无代理脚本</span>
        </div>
        <article v-for="script in scripts" :key="script.id" :class="{ selected: selectedId === script.id }">
          <label class="script-toggle" :title="script.enabled ? '停用脚本' : '启用脚本'">
            <input
              type="checkbox"
              :checked="script.enabled"
              @change="emit('toggle', script.id, ($event.target as HTMLInputElement).checked)"
            />
          </label>
          <button type="button" class="script-name" :aria-label="`编辑 ${script.name}`" @click="edit(script)">
            <strong>{{ script.name }}</strong>
            <small>{{ script.urlPattern || '全部流量' }}</small>
          </button>
          <button
            type="button"
            class="icon-button"
            :aria-label="`删除 ${script.name}`"
            @click="emit('delete', script.id)"
          >
            <Trash2 :size="14" />
          </button>
        </article>
      </aside>

      <form v-if="editing" class="script-editor" @submit.prevent="save">
        <div class="editor-heading">
          <div>
            <h2>{{ draft.id ? '编辑脚本' : '新建脚本' }}</h2>
          </div>
          <div class="editor-actions">
            <button
              type="button"
              :disabled="busy"
              aria-label="测试脚本"
              @click="emit('test', { ...draft })"
            >
              <Play :size="14" />
              测试
            </button>
            <button type="submit" :disabled="busy || !draft.name.trim() || !draft.source.trim()" aria-label="保存脚本">
              <Save :size="14" />
              保存
            </button>
          </div>
        </div>

        <div class="script-fields">
          <label>
            <span>名称</span>
            <input v-model.trim="draft.name" aria-label="脚本名称" required />
          </label>
          <label>
            <span>方法</span>
            <select v-model="draft.methods[0]" aria-label="请求方法">
              <option v-for="method in ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'ALL']" :key="method" :value="method === 'ALL' ? '' : method">
                {{ method }}
              </option>
            </select>
          </label>
          <label class="wide-field">
            <span>URL 匹配</span>
            <input v-model="draft.urlPattern" aria-label="URL 匹配内容" placeholder="/api" />
          </label>
        </div>

        <BodyEditor v-model="draft.source" language="javascript" ariaLabel="脚本源码" />

        <div v-if="testResult" class="test-result" role="status">
          <strong>{{ testResult.errors.length ? '执行失败' : '执行成功' }}</strong>
          <code>{{ testResult.request.method }} {{ testResult.request.url }}</code>
          <span v-for="error in testResult.errors" :key="error.scriptId">{{ error.message }}</span>
        </div>
      </form>

      <div v-else class="editor-empty">
        <Braces :size="30" />
        <span>选择脚本查看详情，或新建脚本</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.script-workspace {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: auto minmax(0, 1fr);
  background: var(--surface);
}

.script-header,
.editor-heading,
.editor-actions {
  display: flex;
  align-items: center;
}

.script-header {
  padding: 18px 20px;
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
}

.script-header h1,
.editor-heading h2 {
  margin: 0;
  font-size: var(--font-size-xl);
}

.script-header p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.primary-action,
.editor-actions button {
  display: inline-flex;
  height: 31px;
  padding: 0 10px;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-sm);
  font-weight: 700;
}

.primary-action {
  border-color: var(--accent-strong);
  background: var(--accent);
  color: var(--on-accent);
}

.script-body {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-columns: 260px minmax(0, 1fr);
}

.script-list {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  border-right: 1px solid var(--border);
}

.script-list article {
  display: grid;
  min-height: 58px;
  padding: 8px 10px;
  grid-template-columns: 22px minmax(0, 1fr) 28px;
  align-items: center;
  border-bottom: 1px solid var(--border-soft);
}

.script-list article.selected {
  background: var(--accent-soft);
}

.script-name {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
  border: 0;
  background: transparent;
  color: var(--text);
  text-align: left;
}

.script-name strong,
.script-name small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.script-name small {
  color: var(--text-muted);
}

.icon-button {
  width: 28px;
  height: 28px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
}

.script-editor {
  display: grid;
  min-width: 0;
  min-height: 0;
  padding: 16px;
  grid-template-rows: auto auto minmax(220px, 1fr) auto;
  gap: 12px;
}

.editor-heading {
  justify-content: space-between;
}

.editor-actions {
  gap: 7px;
}

.script-fields {
  display: grid;
  min-width: 0;
  grid-template-columns: minmax(160px, 1fr) 110px minmax(200px, 1.5fr);
  gap: 10px;
}

.script-fields label {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 5px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.script-fields input,
.script-fields select {
  height: 31px;
  min-width: 0;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
}

.empty-state,
.editor-empty {
  display: flex;
  height: 100%;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 8px;
  color: var(--text-muted);
  font-size: var(--font-size-sm);
}

.test-result {
  display: flex;
  min-width: 0;
  gap: 8px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.test-result code {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 1100px) {
  .script-body {
    grid-template-columns: 180px minmax(0, 1fr);
  }

  .script-fields {
    grid-template-columns: minmax(0, 1fr) 110px;
  }

  .wide-field {
    grid-column: 1 / -1;
  }
}
</style>
