import { beforeEach, describe, expect, it } from 'vitest'
import { SUPPORT_CHAT_STORAGE_KEY, type SupportChatState } from './support-chat-storage'
import { useSupportChat } from './useSupportChat'

function ids(...values: string[]) {
  let index = 0
  return () => values[index++] ?? `generated-${index}`
}

describe('useSupportChat', () => {
  beforeEach(() => localStorage.clear())

  it('creates and persists a conversation from the first non-empty message', () => {
    const chat = useSupportChat({
      storage: localStorage,
      now: () => '2026-10-27T10:00:00.000Z',
      createId: ids('conversation-1', 'message-1')
    })

    expect(chat.sendMessage('   ')).toBe(false)
    expect(chat.sendMessage('  为什么抓不到 HTTPS？\n请帮我看看  ')).toBe(true)

    expect(chat.activeConversation.value).toMatchObject({
      id: 'conversation-1',
      title: '为什么抓不到 HTTPS？',
      messages: [{ id: 'message-1', content: '为什么抓不到 HTTPS？\n请帮我看看' }]
    })
    const saved = JSON.parse(localStorage.getItem(SUPPORT_CHAT_STORAGE_KEY) ?? '{}') as SupportChatState
    expect(saved.activeConversationId).toBe('conversation-1')
    expect(saved.conversations).toHaveLength(1)
  })

  it('sorts histories by update time and restores the active conversation', () => {
    const state: SupportChatState = {
      version: 1,
      activeConversationId: 'older',
      conversations: [
        { id: 'older', title: '较早会话', createdAt: '2026-10-25T10:00:00.000Z', updatedAt: '2026-10-25T10:00:00.000Z', messages: [] },
        { id: 'newer', title: '最近会话', createdAt: '2026-10-26T10:00:00.000Z', updatedAt: '2026-10-26T10:00:00.000Z', messages: [] }
      ]
    }
    localStorage.setItem(SUPPORT_CHAT_STORAGE_KEY, JSON.stringify(state))

    const chat = useSupportChat({ storage: localStorage })

    expect(chat.conversations.value.map((conversation) => conversation.id)).toEqual(['newer', 'older'])
    expect(chat.activeConversation.value?.id).toBe('older')
  })

  it('switches, renames, creates a draft, and deletes conversations', () => {
    const state: SupportChatState = {
      version: 1,
      activeConversationId: 'first',
      conversations: [
        { id: 'first', title: '第一个会话', createdAt: '2026-10-25T10:00:00.000Z', updatedAt: '2026-10-25T10:00:00.000Z', messages: [] },
        { id: 'second', title: '第二个会话', createdAt: '2026-10-26T10:00:00.000Z', updatedAt: '2026-10-26T10:00:00.000Z', messages: [] }
      ]
    }
    localStorage.setItem(SUPPORT_CHAT_STORAGE_KEY, JSON.stringify(state))
    const chat = useSupportChat({ storage: localStorage })

    chat.selectConversation('second')
    chat.renameConversation('second', '  HTTPS 证书问题  ')
    expect(chat.activeConversation.value?.title).toBe('HTTPS 证书问题')

    chat.newConversation()
    expect(chat.activeConversation.value).toBeNull()

    chat.selectConversation('first')
    chat.deleteConversation('first')
    expect(chat.activeConversation.value?.id).toBe('second')
    expect(chat.conversations.value.map((conversation) => conversation.id)).toEqual(['second'])
  })

  it('keeps a sent message in memory when persistence fails', () => {
    const storage = {
      getItem: () => null,
      setItem: () => { throw new DOMException('quota exceeded', 'QuotaExceededError') }
    }
    const chat = useSupportChat({
      storage,
      now: () => '2026-10-27T10:00:00.000Z',
      createId: ids('conversation-1', 'message-1')
    })

    chat.sendMessage('保存不了也要显示')

    expect(chat.activeConversation.value?.messages[0]?.content).toBe('保存不了也要显示')
    expect(chat.notice.value).toBe('历史对话保存失败，本次内容仅在当前运行期间保留')
  })
})
