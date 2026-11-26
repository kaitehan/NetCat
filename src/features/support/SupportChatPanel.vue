<script setup lang="ts">
import { computed, nextTick, onMounted, shallowRef, useTemplateRef, watch } from 'vue'
import { CircleStop, History, LoaderCircle, Pencil, Plus, Send, Trash2, X } from '@lucide/vue'
import AssistantNetworkDraft from './AssistantNetworkDraft.vue'
import AssistantMarkdown from './AssistantMarkdown.vue'
import AssistantMockDraft from './AssistantMockDraft.vue'
import AssistantScriptDraft from './AssistantScriptDraft.vue'
import AssistantTrafficSearchDraft from './AssistantTrafficSearchDraft.vue'
import { useSupportChat, type AssistantSessionApi } from './useSupportChat'
import type { AssistantDraft, AssistantInput, MockRule } from '../../shared/types'

interface AssistantLaunchRequest {
  sequence: number
  input: AssistantInput
}

const { api, initialExchangeId, launchRequest } = defineProps<{
  api?: AssistantSessionApi
  initialExchangeId?: string | null
  launchRequest?: AssistantLaunchRequest | null
}>()

const emit = defineEmits<{
  close: []
  applyTrafficFilter: [filter: { query: string; domain: string; method: string | null; statusGroup: number | null }]
  prepareMockRule: [rule: MockRule]
}>()

const chat = useSupportChat({ api })
const draft = shallowRef('')
const editingId = shallowRef<string | null>(null)
const editingTitle = shallowRef('')
const composing = shallowRef(false)
const inputRef = useTemplateRef<HTMLTextAreaElement>('messageInput')
const messageListRef = useTemplateRef<HTMLElement>('messageList')
let handledLaunchSequence = 0

const conversationTitle = computed(() => chat.activeConversation.value?.title ?? '新对话')
const messageLogLabel = computed(() => `${conversationTitle.value}的消息`)
const canSend = computed(() => draft.value.trim().length > 0 && !chat.submitting.value)
const draftsByMessage = computed(() => {
  const grouped = new Map<string, AssistantDraft[]>()
  for (const item of chat.drafts.value) {
    if (!item.messageId) continue
    const drafts = grouped.get(item.messageId) ?? []
    drafts.push(item)
    grouped.set(item.messageId, drafts)
  }
  return grouped
})
const unboundDrafts = computed(() => chat.drafts.value.filter((item) => !item.messageId))
const statusLabel = computed(() => ({
  starting: 'Runtime 启动中',
  ready: '就绪',
  waitingModel: '等待模型',
  callingTool: '正在调用工具',
  waitingApproval: '等待确认',
  runningDiagnosis: '正在执行联网诊断',
  completed: '已完成',
  stopped: '已停止',
  recovering: '正在恢复',
  credentialRequired: '需要配置凭据',
  error: '运行出错'
}[chat.status.value]))

function focusInput() {
  void nextTick(() => inputRef.value?.focus())
}

function scrollToLatest() {
  void nextTick(() => {
    if (messageListRef.value) {
      messageListRef.value.scrollTop = messageListRef.value.scrollHeight
    }
  })
}

function startNewConversation() {
  chat.newConversation()
  draft.value = ''
  editingId.value = null
  focusInput()
}

async function selectConversation(id: string) {
  await chat.selectConversation(id)
  editingId.value = null
  scrollToLatest()
  focusInput()
}

async function sendMessage() {
  const content = draft.value
  draft.value = ''
  await chat.sendMessage(content)
  scrollToLatest()
  focusInput()
}

function formatMessageTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  const now = new Date()
  const sameDay = date.getFullYear() === now.getFullYear()
    && date.getMonth() === now.getMonth()
    && date.getDate() === now.getDate()
  const time = date.toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false
  })
  if (sameDay) return time
  const day = date.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' }).replace('/', '-')
  return `${day} ${time}`
}

async function handleLaunchRequest(request: AssistantLaunchRequest | null | undefined) {
  if (!request || request.sequence === handledLaunchSequence) return
  handledLaunchSequence = request.sequence
  chat.newConversation()
  await chat.sendInput(request.input)
  scrollToLatest()
  focusInput()
}

async function applyTrafficSearch(
  draftId: string,
  filter: { query: string; domain: string; method: string | null; statusGroup: number | null }
) {
  if (await chat.applyTrafficSearchDraft(draftId)) {
    emit('applyTrafficFilter', filter)
  }
}

async function prepareMockRule(draftId: string, rule: MockRule) {
  if (await chat.prepareMockDraft(draftId)) {
    emit('prepareMockRule', rule)
  }
}

function handleMessageKeydown(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing || composing.value) return
  event.preventDefault()
  sendMessage()
}

function startRenaming(id: string, title: string) {
  editingId.value = id
  editingTitle.value = title
}

async function finishRenaming(id: string) {
  if (await chat.renameConversation(id, editingTitle.value)) {
    editingId.value = null
  }
}

function cancelRenaming() {
  editingId.value = null
}

async function deleteConversation(id: string, title: string) {
  if (!window.confirm(`确认删除会话“${title}”吗？`)) return
  await chat.deleteConversation(id)
  const nextConversation = chat.conversations.value[0]
  if (!chat.activeConversation.value && nextConversation) {
    await chat.selectConversation(nextConversation.id)
  }
  editingId.value = null
  focusInput()
}

function formatUpdatedAt(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false
  })
}

onMounted(async () => {
  await chat.load()
  if (initialExchangeId) {
    chat.newConversation()
    await chat.sendInput({ kind: 'exchange', exchangeId: initialExchangeId })
  } else {
    const firstConversation = chat.conversations.value[0]
    if (firstConversation) await chat.selectConversation(firstConversation.id)
  }
  await handleLaunchRequest(launchRequest)
  focusInput()
  scrollToLatest()
})

watch(() => launchRequest?.sequence, () => {
  void handleLaunchRequest(launchRequest)
})
</script>

<template>
  <aside class="support-panel" aria-label="智奈助手">
    <header class="support-header">
      <div class="support-title">
        <img class="support-brand-icon" src="/src-tauri/icons/icon.png" alt="" />
        <h2>智奈助手</h2>
      </div>
      <button type="button" class="icon-button" aria-label="关闭智奈助手" title="关闭智奈助手" @click="emit('close')">
        <X :size="16" />
      </button>
    </header>

    <div class="support-content">
      <nav class="history-panel" aria-label="历史对话">
        <div class="history-toolbar">
          <button
            type="button"
            class="new-conversation-button"
            aria-label="新建对话"
            :disabled="chat.submitting.value"
            @click="startNewConversation"
          >
            <Plus :size="14" />
            新建对话
          </button>
        </div>

        <div class="history-label">
          <History :size="12" />
          历史对话
        </div>

        <div class="history-list">
          <div v-if="chat.conversations.value.length === 0" class="history-empty">暂无历史对话</div>
          <div
            v-for="conversation in chat.conversations.value"
            :key="conversation.id"
            class="history-item"
            :class="{ selected: chat.activeConversation.value?.id === conversation.id }"
          >
            <template v-if="editingId === conversation.id">
              <input
                v-model="editingTitle"
                class="title-input"
                aria-label="会话名称"
                maxlength="60"
                :disabled="chat.submitting.value"
                @keydown.enter.prevent="finishRenaming(conversation.id)"
                @keydown.esc.prevent="cancelRenaming"
                @blur="finishRenaming(conversation.id)"
              />
            </template>
            <template v-else>
              <button
                type="button"
                class="conversation-select"
                :class="{ selected: chat.activeConversation.value?.id === conversation.id }"
                :aria-label="`打开会话 ${conversation.title}`"
                :aria-current="chat.activeConversation.value?.id === conversation.id ? 'page' : undefined"
                :disabled="chat.submitting.value"
                @click="selectConversation(conversation.id)"
              >
                <strong>{{ conversation.title }}</strong>
                <span>{{ formatUpdatedAt(conversation.updatedAt) }}</span>
              </button>
              <div class="history-actions">
                <button
                  type="button"
                  class="history-action"
                  :aria-label="`重命名 ${conversation.title}`"
                  title="重命名"
                  :disabled="chat.submitting.value"
                  @click="startRenaming(conversation.id, conversation.title)"
                >
                  <Pencil :size="11" />
                </button>
                <button
                  type="button"
                  class="history-action danger"
                  :aria-label="`删除 ${conversation.title}`"
                  title="删除"
                  :disabled="chat.submitting.value"
                  @click="deleteConversation(conversation.id, conversation.title)"
                >
                  <Trash2 :size="11" />
                </button>
              </div>
            </template>
          </div>
        </div>
      </nav>

      <section class="conversation-panel" aria-label="当前对话">
        <header class="conversation-header">
          <h3>{{ conversationTitle }}</h3>
          <div class="conversation-header-actions">
            <div class="conversation-status" role="status">
              <LoaderCircle v-if="chat.loading.value || chat.submitting.value" :size="12" class="status-spinner" />
              <CircleStop v-else-if="chat.status.value === 'stopped'" :size="12" />
              <span>{{ statusLabel }}</span>
            </div>
            <button
              v-if="chat.submitting.value"
              type="button"
              class="stop-button"
              aria-label="停止当前分析"
              title="停止当前分析"
              @click="chat.stop"
            >
              <CircleStop :size="13" />
            </button>
          </div>
        </header>

        <div
          ref="messageList"
          class="message-list"
          role="log"
          :aria-label="messageLogLabel"
          aria-live="polite"
        >
          <div v-if="!chat.activeConversation.value" class="conversation-empty">
            <img class="empty-state-icon" src="/src-tauri/icons/icon.png" alt="" />
            <strong>你好，我是智奈</strong>
            <span>请描述你遇到的问题</span>
          </div>
          <div v-else-if="chat.activeConversation.value.messages.length === 0" class="conversation-empty">
            <img class="empty-state-icon" src="/src-tauri/icons/icon.png" alt="" />
            <strong>你好，我是智奈</strong>
            <span>请描述你遇到的问题</span>
          </div>
          <template v-for="message in chat.activeConversation.value?.messages ?? []" :key="message.id">
            <article
              class="message-bubble"
              :class="`message-${message.role}`"
              :data-message-type="message.messageType ?? 'normal'"
            >
              <img
                v-if="message.role === 'assistant'"
                class="message-avatar"
                src="/src-tauri/icons/icon.png"
                alt="智奈"
              />
              <div class="message-content">
                <AssistantMarkdown v-if="message.role === 'assistant'" :content="message.content" />
                <span v-else>{{ message.content }}</span>
                <time :datetime="message.createdAt">{{ formatMessageTime(message.createdAt) }}</time>
              </div>
            </article>
            <AssistantNetworkDraft
              v-for="networkDraft in draftsByMessage.get(message.id)?.filter((item) => item.kind === 'networkRequest') ?? []"
              :key="networkDraft.id"
              :draft="networkDraft"
              :pending="chat.submitting.value"
              @execute="chat.executeNetworkDraft"
            />
            <AssistantTrafficSearchDraft
              v-for="searchDraft in draftsByMessage.get(message.id)?.filter((item) => item.kind === 'trafficSearch') ?? []"
              :key="searchDraft.id"
              :draft="searchDraft"
              :pending="chat.submitting.value"
              @apply="applyTrafficSearch"
            />
            <AssistantMockDraft
              v-for="mockDraft in draftsByMessage.get(message.id)?.filter((item) => item.kind === 'mock') ?? []"
              :key="mockDraft.id"
              :draft="mockDraft"
              :pending="chat.submitting.value"
              @prepare="prepareMockRule(mockDraft.id, $event)"
            />
            <AssistantScriptDraft
              v-for="scriptDraft in draftsByMessage.get(message.id)?.filter((item) => item.kind === 'script') ?? []"
              :key="scriptDraft.id"
              :draft="scriptDraft"
              :pending="chat.submitting.value"
              @save="chat.acceptScriptDraft(scriptDraft.id, $event)"
            />
          </template>
          <article
            v-if="chat.notice.value"
            class="message-bubble message-system"
            data-message-type="error"
            role="alert"
          >
            <div class="message-content">
              <span>{{ chat.notice.value }}</span>
            </div>
          </article>
          <div v-if="chat.activities.value.length > 0" class="activity-list" aria-label="分析活动">
            <div v-for="activity in chat.activities.value" :key="activity.id" class="activity-row">
              <span class="activity-dot" :data-status="activity.status"></span>
              <span>{{ activity.label }}</span>
            </div>
          </div>
          <AssistantNetworkDraft
            v-for="networkDraft in unboundDrafts.filter((item) => item.kind === 'networkRequest')"
            :key="networkDraft.id"
            :draft="networkDraft"
            :pending="chat.submitting.value"
            @execute="chat.executeNetworkDraft"
          />
          <AssistantTrafficSearchDraft
            v-for="searchDraft in unboundDrafts.filter((item) => item.kind === 'trafficSearch')"
            :key="searchDraft.id"
            :draft="searchDraft"
            :pending="chat.submitting.value"
            @apply="applyTrafficSearch"
          />
          <AssistantMockDraft
            v-for="mockDraft in unboundDrafts.filter((item) => item.kind === 'mock')"
            :key="mockDraft.id"
            :draft="mockDraft"
            :pending="chat.submitting.value"
            @prepare="prepareMockRule(mockDraft.id, $event)"
          />
          <AssistantScriptDraft
            v-for="scriptDraft in unboundDrafts.filter((item) => item.kind === 'script')"
            :key="scriptDraft.id"
            :draft="scriptDraft"
            :pending="chat.submitting.value"
            @save="chat.acceptScriptDraft(scriptDraft.id, $event)"
          />
        </div>

        <form class="message-composer" @submit.prevent="sendMessage">
          <textarea
            ref="messageInput"
            v-model="draft"
            aria-label="输入消息"
            maxlength="4000"
            placeholder="输入消息"
            rows="3"
            :disabled="chat.submitting.value"
            @compositionstart="composing = true"
            @compositionend="composing = false"
            @keydown="handleMessageKeydown"
          />
          <button type="submit" class="send-button" :disabled="!canSend" aria-label="发送消息" title="发送消息">
            <LoaderCircle v-if="chat.submitting.value" :size="14" class="status-spinner" />
            <Send v-else :size="14" />
          </button>
        </form>
      </section>
    </div>
  </aside>
</template>

<style scoped>
.support-panel {
  position: fixed;
  z-index: 85;
  top: 10px;
  right: 10px;
  bottom: 10px;
  display: grid;
  width: min(520px, calc(100vw - 20px));
  min-width: 0;
  grid-template-rows: 48px minmax(0, 1fr);
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-panel);
  background: var(--surface);
  box-shadow: var(--shadow-panel);
}

.support-header {
  display: flex;
  min-width: 0;
  padding: 0 12px;
  align-items: center;
  justify-content: space-between;
  background: var(--surface);
}

.support-title {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.support-title h2 {
  margin: 0;
  color: var(--text);
  font-size: var(--font-size-xl);
}

.support-brand-icon {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-control);
  object-fit: cover;
}

.icon-button {
  display: grid;
  width: 28px;
  height: 28px;
  padding: 0;
  place-items: center;
  border: 1px solid transparent;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-secondary);
}

.icon-button:hover {
  border-color: var(--border);
  background: var(--surface-hover);
  color: var(--accent-strong);
}

.support-content {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-columns: 176px minmax(0, 1fr);
  background: var(--surface-raised);
}

.history-panel {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: 56px 28px minmax(0, 1fr);
  background: var(--surface-raised);
}

.history-toolbar {
  min-width: 0;
  padding: 8px 10px;
}

.history-label {
  display: flex;
  min-width: 0;
  padding: 0 12px;
  align-items: center;
  gap: 5px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
  font-weight: 600;
}

.new-conversation-button {
  display: flex;
  width: 100%;
  height: 38px;
  padding: 0 10px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--accent-strong);
  font-family: inherit;
  font-size: var(--font-size-sm);
  font-weight: 600;
}

.new-conversation-button:hover {
  background: var(--accent-soft);
}

.history-list {
  min-height: 0;
  padding: 4px 10px 10px;
  overflow-y: auto;
}

.history-empty {
  padding: 18px 8px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
  text-align: center;
}

.history-item {
  position: relative;
  display: flex;
  min-width: 0;
  min-height: 48px;
  margin-bottom: 7px;
  align-items: stretch;
  border: 0;
  border-radius: var(--radius-control);
  background: var(--surface);
}

.history-item:hover {
  background: var(--surface-hover);
}

.history-item.selected {
  background: var(--selection);
}

.conversation-select {
  display: flex;
  min-width: 0;
  padding: 7px 45px 7px 8px;
  flex: 1;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  gap: 4px;
  overflow: hidden;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text);
  font-family: inherit;
  text-align: left;
}

.conversation-select strong,
.conversation-select span {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conversation-select strong {
  font-size: var(--font-size-xs);
  font-weight: 600;
}

.conversation-select span {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
  font-variant-numeric: tabular-nums;
}

.history-actions {
  position: absolute;
  top: 50%;
  right: 4px;
  display: flex;
  gap: 1px;
  opacity: 0;
  transform: translateY(-50%);
  transition: opacity 120ms ease;
}

.history-item:hover .history-actions,
.history-item:focus-within .history-actions,
.history-item.selected .history-actions {
  opacity: 1;
}

.history-action {
  display: grid;
  width: 20px;
  height: 22px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-muted);
}

.history-action:hover {
  background: var(--surface-hover);
  color: var(--accent-strong);
}

.history-action.danger:hover {
  background: var(--danger-soft);
  color: var(--danger);
}

.title-input {
  width: calc(100% - 10px);
  height: 28px;
  margin: 9px 5px;
  padding: 0 6px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
  font-family: inherit;
  font-size: var(--font-size-xs);
}

.conversation-panel {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: 42px minmax(0, 1fr) auto auto;
  background: var(--surface);
}

.conversation-header {
  display: flex;
  min-width: 0;
  padding: 0 14px;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.conversation-header h3 {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  color: var(--text);
  font-size: var(--font-size-md);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conversation-header span {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.conversation-status {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 5px;
  color: var(--text-muted);
}

.conversation-header-actions {
  display: flex;
  align-items: center;
  gap: 7px;
}

.stop-button {
  display: grid;
  width: 26px;
  height: 26px;
  padding: 0;
  place-items: center;
  border: 1px solid var(--danger-border);
  border-radius: var(--radius-control);
  background: var(--danger-soft);
  color: var(--danger);
}

.stop-button:hover {
  border-color: var(--danger);
}

.status-spinner {
  animation: support-spin 900ms linear infinite;
}

.message-list {
  display: flex;
  min-width: 0;
  min-height: 0;
  padding: 14px;
  flex-direction: column;
  overflow-y: auto;
  background: var(--surface);
}

.conversation-empty {
  display: flex;
  min-height: 100%;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 8px;
  color: var(--text-muted);
  text-align: center;
}

.empty-state-icon {
  width: 48px;
  height: 48px;
  margin-bottom: 4px;
  border-radius: var(--radius-panel);
  object-fit: cover;
  box-shadow: var(--shadow-brand);
}

.conversation-empty strong {
  color: var(--text-secondary);
  font-size: var(--font-size-md);
}

.conversation-empty span {
  font-size: var(--font-size-xs);
}

.message-bubble {
  display: flex;
  max-width: 88%;
  margin: 0 0 8px auto;
  padding: 8px 10px;
  overflow-wrap: anywhere;
  border: 0;
  border-radius: var(--radius-control);
  background: var(--accent-soft);
  color: var(--text);
  font-size: var(--font-size-sm);
  line-height: 1.55;
  white-space: pre-wrap;
}

.message-content {
  min-width: 0;
}

.message-content time {
  display: block;
  margin-top: 4px;
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
  font-variant-numeric: tabular-nums;
}

.message-avatar {
  width: 24px;
  height: 24px;
  margin-right: 7px;
  flex: 0 0 auto;
  border-radius: 50%;
  object-fit: cover;
}

.message-assistant {
  padding: 0;
  align-items: flex-start;
  border: 0;
  background: transparent;
  margin-right: auto;
  margin-left: 0;
}

.message-assistant .message-content {
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
}

.message-system[data-message-type='error'] {
  margin-right: auto;
  margin-left: 0;
  border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--border));
  background: var(--danger-soft);
  color: var(--danger);
}

.activity-list {
  display: grid;
  margin: 2px 0 10px;
  gap: 5px;
}

.activity-row {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 7px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.activity-dot {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: var(--border-strong);
}

.activity-dot[data-status='running'] {
  background: var(--accent);
}

.activity-dot[data-status='completed'] {
  background: var(--success);
}

.activity-dot[data-status='failed'] {
  background: var(--danger);
}

.message-composer {
  display: grid;
  min-width: 0;
  margin: 10px;
  padding: 7px;
  grid-template-columns: minmax(0, 1fr) 30px;
  align-items: end;
  gap: 6px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-panel);
  background: var(--surface);
}

.message-composer:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.message-composer textarea {
  width: 100%;
  min-width: 0;
  min-height: 64px;
  max-height: 120px;
  padding: 6px;
  resize: vertical;
  border: 0;
  border-radius: 0;
  background: var(--surface);
  color: var(--text);
  font-family: inherit;
  font-size: var(--font-size-sm);
  line-height: 1.45;
}

.message-composer textarea:focus-visible {
  outline: 0;
}

.message-composer textarea::placeholder {
  color: var(--text-muted);
}

.send-button {
  display: grid;
  width: 30px;
  height: 30px;
  padding: 0;
  place-items: center;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--accent);
  color: var(--on-accent);
}

.send-button:hover:not(:disabled) {
  border-color: var(--accent-strong);
  background: var(--accent-strong);
}

.send-button:disabled {
  border-color: var(--border);
  background: var(--surface-raised);
  color: var(--text-muted);
  cursor: not-allowed;
}

@keyframes support-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 820px) {
  .support-content {
    grid-template-columns: 156px minmax(0, 1fr);
  }
}

@media (prefers-reduced-motion: reduce) {
  .history-actions {
    transition: none;
  }

  .status-spinner {
    animation: none;
  }
}
</style>
