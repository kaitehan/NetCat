export const SUPPORT_CHAT_STORAGE_KEY = 'netcat-support-conversations:v1'

export interface SupportMessage {
  id: string
  role: 'user'
  content: string
  createdAt: string
}

export interface SupportConversation {
  id: string
  title: string
  createdAt: string
  updatedAt: string
  messages: SupportMessage[]
}

export interface SupportChatState {
  version: 1
  activeConversationId: string | null
  conversations: SupportConversation[]
}

interface ReadableStorage {
  getItem(key: string): string | null
}

interface WritableStorage {
  setItem(key: string, value: string): void
}

export interface SupportStateLoadResult {
  state: SupportChatState
  error: string | null
}

function emptyState(): SupportChatState {
  return {
    version: 1,
    activeConversationId: null,
    conversations: []
  }
}

function isString(value: unknown): value is string {
  return typeof value === 'string'
}

function isSupportMessage(value: unknown): value is SupportMessage {
  if (!value || typeof value !== 'object') return false
  const message = value as Record<string, unknown>
  return isString(message.id)
    && message.role === 'user'
    && isString(message.content)
    && isString(message.createdAt)
}

function isSupportConversation(value: unknown): value is SupportConversation {
  if (!value || typeof value !== 'object') return false
  const conversation = value as Record<string, unknown>
  return isString(conversation.id)
    && isString(conversation.title)
    && isString(conversation.createdAt)
    && isString(conversation.updatedAt)
    && Array.isArray(conversation.messages)
    && conversation.messages.every(isSupportMessage)
}

function isSupportChatState(value: unknown): value is SupportChatState {
  if (!value || typeof value !== 'object') return false
  const state = value as Record<string, unknown>
  if (state.version !== 1 || !Array.isArray(state.conversations) || !state.conversations.every(isSupportConversation)) {
    return false
  }
  if (state.activeConversationId !== null && !isString(state.activeConversationId)) {
    return false
  }
  return state.activeConversationId === null
    || state.conversations.some((conversation) => conversation.id === state.activeConversationId)
}

export function loadSupportState(storage: ReadableStorage): SupportStateLoadResult {
  try {
    const serialized = storage.getItem(SUPPORT_CHAT_STORAGE_KEY)
    if (serialized === null) {
      return { state: emptyState(), error: null }
    }
    const parsed: unknown = JSON.parse(serialized)
    if (isSupportChatState(parsed)) {
      return { state: parsed, error: null }
    }
  } catch {
    // Invalid or unavailable local storage uses a fresh in-memory state.
  }
  return {
    state: emptyState(),
    error: '历史对话读取失败，已打开空白会话'
  }
}

export function saveSupportState(storage: WritableStorage, state: SupportChatState): string | null {
  try {
    storage.setItem(SUPPORT_CHAT_STORAGE_KEY, JSON.stringify(state))
    return null
  } catch {
    return '历史对话保存失败，本次内容仅在当前运行期间保留'
  }
}
