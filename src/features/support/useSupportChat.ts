import { computed, readonly, shallowRef } from 'vue'
import {
  loadSupportState,
  saveSupportState,
  type SupportChatState,
  type SupportConversation,
  type SupportMessage
} from './support-chat-storage'

interface SupportStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

interface UseSupportChatOptions {
  storage?: SupportStorage
  now?: () => string
  createId?: () => string
}

function defaultId(): string {
  return globalThis.crypto.randomUUID()
}

function createTitle(content: string): string {
  const firstLine = content.split(/\r?\n/, 1)[0] ?? content
  const characters = Array.from(firstLine.trim())
  return characters.length > 28
    ? `${characters.slice(0, 28).join('')}...`
    : characters.join('')
}

function sortByUpdatedAt(conversations: readonly SupportConversation[]): SupportConversation[] {
  return [...conversations].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
}

export function useSupportChat(options: UseSupportChatOptions = {}) {
  const storage = options.storage ?? window.localStorage
  const now = options.now ?? (() => new Date().toISOString())
  const createId = options.createId ?? defaultId
  const loaded = loadSupportState(storage)
  const state = shallowRef<SupportChatState>(loaded.state)
  const notice = shallowRef<string | null>(loaded.error)

  const conversations = computed(() => sortByUpdatedAt(state.value.conversations))
  const activeConversation = computed(() => state.value.conversations.find(
    (conversation) => conversation.id === state.value.activeConversationId
  ) ?? null)

  function commit(nextState: SupportChatState) {
    state.value = nextState
    notice.value = saveSupportState(storage, nextState)
  }

  function newConversation() {
    commit({ ...state.value, activeConversationId: null })
  }

  function selectConversation(id: string) {
    if (!state.value.conversations.some((conversation) => conversation.id === id)) return
    commit({ ...state.value, activeConversationId: id })
  }

  function sendMessage(input: string): boolean {
    const content = input.trim()
    if (!content) return false

    const timestamp = now()
    const active = activeConversation.value
    const conversationId = active?.id ?? createId()
    const message: SupportMessage = {
      id: createId(),
      role: 'user',
      content,
      createdAt: timestamp
    }
    if (!active) {
      const conversation: SupportConversation = {
        id: conversationId,
        title: createTitle(content),
        createdAt: timestamp,
        updatedAt: timestamp,
        messages: [message]
      }
      commit({
        version: 1,
        activeConversationId: conversation.id,
        conversations: [...state.value.conversations, conversation]
      })
      return true
    }

    commit({
      ...state.value,
      conversations: state.value.conversations.map((conversation) => conversation.id === active.id
        ? { ...conversation, updatedAt: timestamp, messages: [...conversation.messages, message] }
        : conversation)
    })
    return true
  }

  function renameConversation(id: string, input: string): boolean {
    const title = input.trim()
    if (!title || !state.value.conversations.some((conversation) => conversation.id === id)) return false
    commit({
      ...state.value,
      conversations: state.value.conversations.map((conversation) => conversation.id === id
        ? { ...conversation, title }
        : conversation)
    })
    return true
  }

  function deleteConversation(id: string) {
    const remaining = state.value.conversations.filter((conversation) => conversation.id !== id)
    if (remaining.length === state.value.conversations.length) return
    const activeConversationId = state.value.activeConversationId === id
      ? sortByUpdatedAt(remaining)[0]?.id ?? null
      : state.value.activeConversationId
    commit({ ...state.value, activeConversationId, conversations: remaining })
  }

  function dismissNotice() {
    notice.value = null
  }

  return {
    conversations,
    activeConversation,
    notice: readonly(notice),
    newConversation,
    selectConversation,
    sendMessage,
    renameConversation,
    deleteConversation,
    dismissNotice
  }
}
