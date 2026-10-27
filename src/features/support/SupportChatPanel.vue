<script setup lang="ts">
import { computed, nextTick, onMounted, shallowRef, useTemplateRef } from 'vue'
import { History, Pencil, Plus, Send, Trash2, X } from '@lucide/vue'
import { useSupportChat } from './useSupportChat'

const emit = defineEmits<{
  close: []
}>()

const chat = useSupportChat()
const draft = shallowRef('')
const editingId = shallowRef<string | null>(null)
const editingTitle = shallowRef('')
const composing = shallowRef(false)
const inputRef = useTemplateRef<HTMLTextAreaElement>('messageInput')
const messageListRef = useTemplateRef<HTMLElement>('messageList')

const conversationTitle = computed(() => chat.activeConversation.value?.title ?? '新对话')
const messageLogLabel = computed(() => `${conversationTitle.value}的消息`)
const canSend = computed(() => draft.value.trim().length > 0)

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

function selectConversation(id: string) {
  chat.selectConversation(id)
  editingId.value = null
  scrollToLatest()
  focusInput()
}

function sendMessage() {
  if (!chat.sendMessage(draft.value)) return
  draft.value = ''
  scrollToLatest()
  focusInput()
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

function finishRenaming(id: string) {
  if (chat.renameConversation(id, editingTitle.value)) {
    editingId.value = null
  }
}

function cancelRenaming() {
  editingId.value = null
}

function deleteConversation(id: string, title: string) {
  if (!window.confirm(`确认删除会话“${title}”吗？`)) return
  chat.deleteConversation(id)
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

onMounted(() => {
  focusInput()
  scrollToLatest()
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
          <button type="button" class="new-conversation-button" aria-label="新建对话" @click="startNewConversation">
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
                  @click="startRenaming(conversation.id, conversation.title)"
                >
                  <Pencil :size="11" />
                </button>
                <button
                  type="button"
                  class="history-action danger"
                  :aria-label="`删除 ${conversation.title}`"
                  title="删除"
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
          <span v-if="chat.activeConversation.value">{{ chat.activeConversation.value.messages.length }} 条消息</span>
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
            <strong>你好，我是小橘</strong>
            <span>请描述你遇到的问题</span>
          </div>
          <div v-else-if="chat.activeConversation.value.messages.length === 0" class="conversation-empty">
            <img class="empty-state-icon" src="/src-tauri/icons/icon.png" alt="" />
            <strong>你好，我是小橘</strong>
            <span>请描述你遇到的问题</span>
          </div>
          <article
            v-for="message in chat.activeConversation.value?.messages ?? []"
            :key="message.id"
            class="message-bubble"
          >
            {{ message.content }}
          </article>
        </div>

        <div v-if="chat.notice.value" class="storage-notice" role="alert">
          <span>{{ chat.notice.value }}</span>
          <button type="button" aria-label="关闭历史提示" title="关闭提示" @click="chat.dismissNotice">
            <X :size="12" />
          </button>
        </div>

        <form class="message-composer" @submit.prevent="sendMessage">
          <textarea
            ref="messageInput"
            v-model="draft"
            aria-label="输入消息"
            maxlength="4000"
            placeholder="输入消息"
            rows="3"
            @compositionstart="composing = true"
            @compositionend="composing = false"
            @keydown="handleMessageKeydown"
          />
          <button type="submit" class="send-button" :disabled="!canSend" aria-label="发送消息" title="发送消息">
            <Send :size="14" />
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

.storage-notice {
  display: flex;
  min-width: 0;
  padding: 7px 8px 7px 10px;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  background: var(--warning-soft);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.storage-notice span {
  min-width: 0;
  overflow-wrap: anywhere;
}

.storage-notice button {
  display: grid;
  width: 22px;
  height: 22px;
  padding: 0;
  flex: 0 0 auto;
  place-items: center;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-muted);
}

.storage-notice button:hover {
  background: var(--surface);
  color: var(--text);
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

@media (max-width: 820px) {
  .support-content {
    grid-template-columns: 156px minmax(0, 1fr);
  }
}

@media (prefers-reduced-motion: reduce) {
  .history-actions {
    transition: none;
  }
}
</style>
