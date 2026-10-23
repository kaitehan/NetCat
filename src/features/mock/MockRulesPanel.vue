<script setup lang="ts">
import { Edit3, FlaskConical, Plus, Trash2 } from '@lucide/vue'
import type { MockRule } from '../../shared/types'

const { rules } = defineProps<{
  rules: readonly MockRule[]
}>()

const emit = defineEmits<{
  add: []
  edit: [rule: MockRule]
  toggle: [id: string, enabled: boolean]
  delete: [id: string]
}>()
</script>

<template>
  <section class="rules-panel">
    <header class="rules-header">
      <div>
        <h1>Mock 规则</h1>
        <p>命中规则的请求不会访问真实上游</p>
      </div>
      <button type="button" class="add-button" @click="emit('add')">
        <Plus :size="15" />
        新建规则
      </button>
    </header>

    <div v-if="rules.length === 0" class="rules-empty">
      <FlaskConical :size="30" :stroke-width="1.5" />
      <strong>还没有 Mock 规则</strong>
      <span>创建一条规则来模拟接口响应</span>
    </div>
    <div v-else class="rules-list">
      <article v-for="rule in rules" :key="rule.id" class="rule-row">
        <label class="switch" :title="rule.enabled ? '停用规则' : '启用规则'">
          <input
            type="checkbox"
            :checked="rule.enabled"
            @change="emit('toggle', rule.id, ($event.target as HTMLInputElement).checked)"
          />
          <span />
        </label>
        <div class="rule-main">
          <div class="rule-title">
            <strong>{{ rule.name }}</strong>
            <span>{{ rule.methods.join(', ') }}</span>
            <span class="match-kind">{{ rule.matchKind }}</span>
          </div>
          <code>{{ rule.urlPattern }}</code>
        </div>
        <div class="rule-response">
          <strong>{{ rule.status }}</strong>
          <span>{{ rule.delayMs }} ms</span>
        </div>
        <button type="button" class="icon-button" title="编辑规则" @click="emit('edit', rule)">
          <Edit3 :size="15" />
        </button>
        <button type="button" class="icon-button danger" title="删除规则" @click="emit('delete', rule.id)">
          <Trash2 :size="15" />
        </button>
      </article>
    </div>
  </section>
</template>

<style scoped>
.rules-panel {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  background: var(--surface);
}

.rules-header {
  display: flex;
  padding: 18px 20px;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
}

.rules-header h1 {
  margin: 0;
  font-size: 15px;
}

.rules-header p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 10px;
}

.add-button {
  display: inline-flex;
  height: 31px;
  padding: 0 11px;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--accent-strong);
  border-radius: 5px;
  background: var(--accent);
  color: #0d1a14;
  font-family: inherit;
  font-size: 11px;
  font-weight: 700;
}

.rules-empty {
  display: flex;
  height: 360px;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 8px;
  color: var(--text-muted);
  font-size: 11px;
}

.rules-empty strong {
  color: var(--text);
  font-size: 13px;
}

.rules-list {
  display: flex;
  flex-direction: column;
}

.rule-row {
  display: grid;
  min-height: 66px;
  padding: 10px 14px;
  grid-template-columns: 34px minmax(0, 1fr) 70px 30px 30px;
  align-items: center;
  gap: 8px;
  border-bottom: 1px solid var(--border-soft);
}

.rule-row:hover {
  background: var(--surface-hover);
}

.rule-main {
  min-width: 0;
}

.rule-title {
  display: flex;
  align-items: center;
  gap: 7px;
}

.rule-title strong {
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-title span {
  color: var(--accent-strong);
  font-size: 9px;
  font-weight: 700;
}

.rule-title .match-kind {
  color: var(--text-muted);
  font-weight: 500;
}

.rule-main code {
  display: block;
  margin-top: 6px;
  overflow: hidden;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-response {
  display: flex;
  align-items: end;
  flex-direction: column;
  gap: 3px;
}

.rule-response strong {
  color: var(--accent-strong);
  font-size: 11px;
}

.rule-response span {
  color: var(--text-muted);
  font-size: 9px;
}

.icon-button {
  display: grid;
  width: 28px;
  height: 28px;
  place-items: center;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-muted);
}

.icon-button:hover {
  background: var(--surface);
  color: var(--text);
}

.icon-button.danger:hover {
  color: var(--danger);
}

.switch input {
  position: absolute;
  opacity: 0;
}

.switch span {
  position: relative;
  display: block;
  width: 28px;
  height: 16px;
  border-radius: 8px;
  background: var(--border-strong);
}

.switch span::after {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #ffffff;
  content: '';
  transition: transform 140ms ease;
}

.switch input:checked + span {
  background: var(--accent-strong);
}

.switch input:checked + span::after {
  transform: translateX(12px);
}
</style>
