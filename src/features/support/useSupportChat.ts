import { computed, getCurrentScope, onScopeDispose, readonly, shallowRef } from 'vue'
import { desktopApi, normalizeDesktopError, type DesktopApi } from '../../api/desktop'
import type {
  AssistantConversation,
  AssistantInput,
  AssistantSnapshot,
  MockRule,
  ProxyScript
} from '../../shared/types'

export type AssistantSessionApi = Pick<DesktopApi,
  | 'listAssistantConversations'
  | 'createAssistantConversation'
  | 'getAssistantSnapshot'
  | 'renameAssistantConversation'
  | 'hideAssistantConversation'
  | 'submitAssistantInput'
  | 'authorizeAssistantNetworkRequest'
  | 'executeAssistantNetworkRequest'
  | 'saveMockRule'
  | 'saveScript'
  | 'resolveAssistantDraft'
> & Partial<Pick<DesktopApi, 'listen' | 'stopAssistant'>>

interface UseSupportChatOptions {
  api?: AssistantSessionApi
}

function sortByUpdatedAt(conversations: readonly AssistantConversation[]): AssistantConversation[] {
  return [...conversations].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
}

export function useSupportChat(options: UseSupportChatOptions = {}) {
  const api = options.api ?? desktopApi
  const conversationList = shallowRef<readonly AssistantConversation[]>([])
  const snapshot = shallowRef<AssistantSnapshot | null>(null)
  const notice = shallowRef<string | null>(null)
  const loading = shallowRef(false)
  const submitting = shallowRef(false)
  let stopRequested = false
  let stoppedConversationId: string | null = null
  let unlistenSession: (() => void) | null = null

  const conversations = computed(() => sortByUpdatedAt(conversationList.value))
  const activeConversation = computed(() => snapshot.value
    ? { ...snapshot.value.conversation, messages: snapshot.value.messages }
    : null)
  const status = computed(() => snapshot.value?.status ?? (loading.value ? 'starting' : 'ready'))
  const activities = computed(() => snapshot.value?.activities ?? [])
  const drafts = computed(() => snapshot.value?.drafts ?? [])

  async function load() {
    loading.value = true
    notice.value = null
    try {
      conversationList.value = await api.listAssistantConversations()
      if (!unlistenSession && api.listen) {
        unlistenSession = await api.listen<{
          identity: { conversationId: string }
        }>('assistant-session-event', async (event) => {
          const conversationId = snapshot.value?.conversation.id
          if (conversationId && event.identity.conversationId === conversationId) {
            const refreshed = await api.getAssistantSnapshot(conversationId)
            if (snapshot.value?.conversation.id === conversationId) {
              snapshot.value = refreshed
            }
          }
        })
      }
    } catch (cause) {
      notice.value = normalizeDesktopError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function stop() {
    if (!api.stopAssistant || !submitting.value) return
    stopRequested = true
    stoppedConversationId = snapshot.value?.conversation.id ?? null
    try {
      await api.stopAssistant()
      if (snapshot.value) {
        snapshot.value = await api.getAssistantSnapshot(snapshot.value.conversation.id)
      }
    } catch (cause) {
      notice.value = normalizeDesktopError(cause).message
    } finally {
      stopRequested = false
    }
  }

  function newConversation() {
    if (submitting.value) return
    snapshot.value = null
    notice.value = null
  }

  async function selectConversation(id: string) {
    if (submitting.value) return
    if (!conversationList.value.some((conversation) => conversation.id === id)) return
    loading.value = true
    notice.value = null
    try {
      snapshot.value = await api.getAssistantSnapshot(id)
    } catch (cause) {
      notice.value = normalizeDesktopError(cause).message
    } finally {
      loading.value = false
    }
  }

  async function sendInput(input: AssistantInput): Promise<boolean> {
    if (submitting.value) return false
    submitting.value = true
    notice.value = null
    stoppedConversationId = null
    let submittedConversationId: string | null = null
    try {
      let conversation = snapshot.value?.conversation
      if (!conversation) {
        conversation = await api.createAssistantConversation()
        conversationList.value = sortByUpdatedAt([...conversationList.value, conversation])
      }
      submittedConversationId = conversation.id
      snapshot.value = await api.submitAssistantInput(conversation.id, input)
      conversationList.value = sortByUpdatedAt(conversationList.value.map((item) =>
        item.id === conversation.id ? snapshot.value!.conversation : item
      ))
      return true
    } catch (cause) {
      if (!stopRequested && stoppedConversationId !== submittedConversationId) {
        notice.value = normalizeDesktopError(cause).message
      }
      return false
    } finally {
      submitting.value = false
    }
  }

  async function sendMessage(input: string): Promise<boolean> {
    const message = input.trim()
    if (!message) return false
    if (/^curl(?:\s|$)/i.test(message)) {
      return sendInput({ kind: 'curl', curl: message })
    }
    return sendInput({ kind: 'message', message })
  }

  async function executeNetworkDraft(): Promise<boolean> {
    const conversationId = snapshot.value?.conversation.id
    if (!conversationId || submitting.value) return false
    submitting.value = true
    notice.value = null
    try {
      const requestHandle = await api.authorizeAssistantNetworkRequest(conversationId)
      snapshot.value = await api.executeAssistantNetworkRequest(conversationId, requestHandle)
      conversationList.value = sortByUpdatedAt(conversationList.value.map((conversation) =>
        conversation.id === conversationId ? snapshot.value!.conversation : conversation
      ))
      return true
    } catch (cause) {
      notice.value = normalizeDesktopError(cause).message
      return false
    } finally {
      submitting.value = false
    }
  }

  async function acceptDraft(
    draftId: string,
    save: () => Promise<unknown>
  ): Promise<boolean> {
    const conversationId = snapshot.value?.conversation.id
    if (!conversationId || submitting.value) return false
    submitting.value = true
    notice.value = null
    try {
      await save()
      snapshot.value = await api.resolveAssistantDraft(conversationId, draftId, 'accepted')
      return true
    } catch (cause) {
      notice.value = normalizeDesktopError(cause).message
      return false
    } finally {
      submitting.value = false
    }
  }

  function acceptMockDraft(draftId: string, rule: MockRule) {
    return acceptDraft(draftId, () => api.saveMockRule({ ...rule, enabled: false }))
  }

  function acceptScriptDraft(draftId: string, script: ProxyScript) {
    return acceptDraft(draftId, () => api.saveScript({ ...script, enabled: false }))
  }

  async function renameConversation(id: string, input: string): Promise<boolean> {
    if (submitting.value) return false
    const title = input.trim()
    if (!title || !conversationList.value.some((conversation) => conversation.id === id)) return false
    try {
      const updated = await api.renameAssistantConversation(id, title)
      conversationList.value = conversationList.value.map((conversation) =>
        conversation.id === id ? updated : conversation
      )
      if (snapshot.value?.conversation.id === id) {
        snapshot.value = { ...snapshot.value, conversation: updated }
      }
      return true
    } catch (cause) {
      notice.value = normalizeDesktopError(cause).message
      return false
    }
  }

  async function deleteConversation(id: string) {
    if (submitting.value) return
    try {
      await api.hideAssistantConversation(id)
      conversationList.value = conversationList.value.filter((conversation) => conversation.id !== id)
      if (snapshot.value?.conversation.id === id) snapshot.value = null
    } catch (cause) {
      notice.value = normalizeDesktopError(cause).message
    }
  }

  function dismissNotice() {
    notice.value = null
  }

  if (getCurrentScope()) {
    onScopeDispose(() => {
      unlistenSession?.()
      unlistenSession = null
    })
  }

  return {
    conversations,
    activeConversation,
    activities,
    drafts,
    status,
    loading: readonly(loading),
    submitting: readonly(submitting),
    notice: readonly(notice),
    load,
    newConversation,
    selectConversation,
    sendInput,
    sendMessage,
    executeNetworkDraft,
    stop,
    acceptMockDraft,
    acceptScriptDraft,
    renameConversation,
    deleteConversation,
    dismissNotice
  }
}
