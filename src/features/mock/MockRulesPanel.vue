<script setup lang="ts">
import { ArrowDown, ArrowUp, Copy, Edit3, FlaskConical, Play, Plus, Trash2 } from '@lucide/vue'
import { reactive } from 'vue'
import type { MockRule, MockRuleTestResult } from '../../shared/types'

const { rules, testResult } = defineProps<{
  rules: readonly MockRule[]
  testResult: MockRuleTestResult | null
}>()

const emit = defineEmits<{
  add: []
  edit: [rule: MockRule]
  toggle: [id: string, enabled: boolean]
  delete: [id: string]
  duplicate: [id: string]
  move: [id: string, direction: -1 | 1]
  test: [payload: { method: string; url: string }]
}>()

const testInput = reactive({
  method: 'GET',
  url: 'https://example.com/api'
})

function formatLastHit(value?: string | null) {
  return value ? new Date(value).toLocaleString() : '尚未命中'
}
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

    <form class="rule-tester" @submit.prevent="emit('test', testInput)">
      <select v-model="testInput.method" aria-label="测试请求方法">
        <option v-for="method in ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']" :key="method" :value="method">
          {{ method }}
        </option>
      </select>
      <input v-model.trim="testInput.url" type="url" required aria-label="测试 URL" placeholder="https://example.com/api" />
      <button type="submit" class="test-button">
        <Play :size="14" />
        测试规则
      </button>
    </form>

    <div v-if="testResult" class="test-result" :class="{ conflict: testResult.conflict }" role="status">
      <strong v-if="testResult.selectedRuleId">
        生效规则：{{ testResult.matches.find((match) => match.ruleId === testResult.selectedRuleId)?.name }}
      </strong>
      <strong v-else>没有匹配的规则</strong>
      <span v-if="testResult.conflict">共匹配 {{ testResult.matches.length }} 条，优先级最高的规则生效</span>
      <span v-else-if="testResult.matches.length">匹配结果与运行时一致</span>
    </div>

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
          <small>命中 {{ rule.hitCount ?? 0 }} 次 · {{ formatLastHit(rule.lastHitAt) }}</small>
        </div>
        <div class="rule-response">
          <strong>{{ rule.status }}</strong>
          <span>{{ rule.delayMs }} ms</span>
        </div>
        <div class="priority-actions">
          <button type="button" class="icon-button" title="提高优先级" @click="emit('move', rule.id, -1)">
            <ArrowUp :size="14" />
          </button>
          <button type="button" class="icon-button" title="降低优先级" @click="emit('move', rule.id, 1)">
            <ArrowDown :size="14" />
          </button>
        </div>
        <button type="button" class="icon-button" title="复制规则" @click="emit('duplicate', rule.id)">
          <Copy :size="15" />
        </button>
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
  font-size: var(--font-size-xl);
}

.rules-header p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.add-button {
  display: inline-flex;
  height: 31px;
  padding: 0 11px;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--accent-strong);
  border-radius: var(--radius-control);
  background: var(--accent);
  color: var(--on-accent);
  font-family: inherit;
  font-size: var(--font-size-sm);
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
  font-size: var(--font-size-sm);
}

.rule-tester {
  display: grid;
  padding: 10px 14px;
  grid-template-columns: 96px minmax(0, 1fr) auto;
  gap: 8px;
  border-bottom: 1px solid var(--border-soft);
}

.rule-tester select,
.rule-tester input {
  min-width: 0;
  height: 32px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-md);
}

.test-button {
  display: inline-flex;
  height: 32px;
  padding: 0 11px;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-control);
  background: var(--surface-hover);
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-md);
  font-weight: 700;
}

.test-result {
  display: flex;
  padding: 9px 14px;
  align-items: center;
  gap: 8px;
  border-bottom: 1px solid var(--border-soft);
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.test-result strong {
  color: var(--accent-strong);
}

.test-result.conflict strong {
  color: var(--danger);
}

.rules-empty strong {
  color: var(--text);
  font-size: var(--font-size-lg);
}

.rules-list {
  display: flex;
  flex-direction: column;
}

.rule-row {
  display: grid;
  min-height: 66px;
  padding: 10px 14px;
  grid-template-columns: 34px minmax(0, 1fr) 70px 58px 30px 30px 30px;
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
  font-size: var(--font-size-md);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-title span {
  color: var(--accent-strong);
  font-size: var(--font-size-2xs);
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
  font-size: var(--font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-main small {
  display: block;
  margin-top: 4px;
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

.priority-actions {
  display: flex;
}

.rule-response {
  display: flex;
  align-items: end;
  flex-direction: column;
  gap: 3px;
}

.rule-response strong {
  color: var(--accent-strong);
  font-size: var(--font-size-sm);
}

.rule-response span {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

.icon-button {
  display: grid;
  width: 28px;
  height: 28px;
  place-items: center;
  border: 0;
  border-radius: var(--radius-control);
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
  background: var(--qr-surface);
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
