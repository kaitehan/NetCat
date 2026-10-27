import { beforeEach, describe, expect, it } from 'vitest'
import {
  SUPPORT_CHAT_STORAGE_KEY,
  loadSupportState,
  saveSupportState,
  type SupportChatState
} from './support-chat-storage'

const storedState: SupportChatState = {
  version: 1,
  activeConversationId: 'conversation-1',
  conversations: [
    {
      id: 'conversation-1',
      title: 'HTTPS 抓包问题',
      createdAt: '2026-10-27T10:00:00.000Z',
      updatedAt: '2026-10-27T10:01:00.000Z',
      messages: [
        {
          id: 'message-1',
          role: 'user',
          content: '为什么抓不到 HTTPS？',
          createdAt: '2026-10-27T10:01:00.000Z'
        }
      ]
    }
  ]
}

describe('support chat storage', () => {
  beforeEach(() => localStorage.clear())

  it('restores a valid versioned conversation state', () => {
    localStorage.setItem(SUPPORT_CHAT_STORAGE_KEY, JSON.stringify(storedState))

    expect(loadSupportState(localStorage)).toEqual({ state: storedState, error: null })
  })

  it('falls back to an empty state when stored data is malformed', () => {
    localStorage.setItem(SUPPORT_CHAT_STORAGE_KEY, JSON.stringify({ version: 1, conversations: [{ id: 1 }] }))

    expect(loadSupportState(localStorage)).toEqual({
      state: { version: 1, activeConversationId: null, conversations: [] },
      error: '历史对话读取失败，已打开空白会话'
    })
  })

  it('reports a save failure without throwing away the in-memory state', () => {
    const failingStorage = {
      setItem() {
        throw new DOMException('quota exceeded', 'QuotaExceededError')
      }
    } as Pick<Storage, 'setItem'>

    expect(saveSupportState(failingStorage, storedState)).toBe('历史对话保存失败，本次内容仅在当前运行期间保留')
  })
})
