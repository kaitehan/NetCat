import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AssistantConversation, AssistantSnapshot } from '../../shared/types'
import { useSupportChat } from './useSupportChat'

const conversation: AssistantConversation = {
  id: 'conversation-1',
  sessionId: 'session-1',
  title: 'HTTPS 诊断',
  createdAt: '2026-11-24T10:00:00Z',
  updatedAt: '2026-11-24T10:00:00Z',
  hidden: false
}

const snapshot: AssistantSnapshot = {
  conversation,
  messages: [{
    id: 'message-1',
    role: 'assistant',
    content: '请提供需要分析的请求。',
    createdAt: '2026-11-24T10:00:01Z'
  }],
  activities: [],
  drafts: [],
  status: 'ready'
}

function assistantApi() {
  return {
    listAssistantConversations: vi.fn().mockResolvedValue([conversation]),
    createAssistantConversation: vi.fn().mockResolvedValue(conversation),
    getAssistantSnapshot: vi.fn().mockResolvedValue(snapshot),
    renameAssistantConversation: vi.fn().mockImplementation(async (_id: string, title: string) => ({
      ...conversation,
      title
    })),
    hideAssistantConversation: vi.fn().mockResolvedValue(undefined),
    authorizeAssistantNetworkRequest: vi.fn().mockResolvedValue('single-use-handle'),
    executeAssistantNetworkRequest: vi.fn().mockResolvedValue({
      ...snapshot,
      status: 'completed'
    }),
    saveMockRule: vi.fn().mockImplementation(async (rule) => rule),
    saveScript: vi.fn().mockImplementation(async (script) => script),
    resolveAssistantDraft: vi.fn().mockResolvedValue(snapshot),
    submitAssistantInput: vi.fn().mockImplementation(async () => ({
      ...snapshot,
      messages: [
        ...snapshot.messages,
        { id: 'message-2', role: 'user', content: '分析这个请求', createdAt: '2026-11-24T10:01:00Z' }
      ]
    }))
  }
}

describe('useSupportChat', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  it('loads conversations and restores a snapshot without reading localStorage history', async () => {
    localStorage.setItem('netcat-support-conversations:v1', '{invalid legacy history')
    const storageRead = vi.spyOn(Storage.prototype, 'getItem')
    const api = assistantApi()
    const chat = useSupportChat({ api })

    await chat.load()
    await chat.selectConversation('conversation-1')

    expect(storageRead).not.toHaveBeenCalled()
    expect(api.listAssistantConversations).toHaveBeenCalledOnce()
    expect(api.getAssistantSnapshot).toHaveBeenCalledWith('conversation-1')
    expect(chat.activeConversation.value?.messages[0]?.content).toBe('请提供需要分析的请求。')
  })

  it('creates a DSH session before submitting the first non-empty message', async () => {
    const api = assistantApi()
    api.listAssistantConversations.mockResolvedValue([])
    const chat = useSupportChat({ api })
    await chat.load()

    expect(await chat.sendMessage('   ')).toBe(false)
    expect(await chat.sendMessage('  分析这个请求  ')).toBe(true)

    expect(api.createAssistantConversation).toHaveBeenCalledOnce()
    expect(api.submitAssistantInput).toHaveBeenCalledWith('conversation-1', {
      kind: 'message',
      message: '分析这个请求'
    })
    expect(chat.activeConversation.value?.messages.at(-1)?.content).toBe('分析这个请求')
  })

  it('submits pasted cURL as an offline curl input', async () => {
    const api = assistantApi()
    const chat = useSupportChat({ api })
    await chat.load()
    await chat.selectConversation('conversation-1')

    expect(await chat.sendMessage('  curl https://api.example.com/users  ')).toBe(true)

    expect(api.submitAssistantInput).toHaveBeenCalledWith('conversation-1', {
      kind: 'curl',
      curl: 'curl https://api.example.com/users'
    })
  })

  it('authorizes and executes a pending network draft exactly once', async () => {
    const api = assistantApi()
    const chat = useSupportChat({ api })
    await chat.load()
    await chat.selectConversation('conversation-1')

    expect(await chat.executeNetworkDraft()).toBe(true)

    expect(api.authorizeAssistantNetworkRequest).toHaveBeenCalledWith('conversation-1')
    expect(api.executeAssistantNetworkRequest).toHaveBeenCalledWith(
      'conversation-1',
      'single-use-handle'
    )
    expect(chat.status.value).toBe('completed')
  })

  it('renames and hides conversations through the desktop API', async () => {
    const api = assistantApi()
    const chat = useSupportChat({ api })
    await chat.load()
    await chat.selectConversation('conversation-1')

    expect(await chat.renameConversation('conversation-1', '  登录失败  ')).toBe(true)
    expect(chat.activeConversation.value?.title).toBe('登录失败')

    await chat.deleteConversation('conversation-1')
    expect(api.hideAssistantConversation).toHaveBeenCalledWith('conversation-1')
    expect(chat.activeConversation.value).toBeNull()
    expect(chat.conversations.value).toEqual([])
  })

  it('keeps existing content and exposes a retryable error when submission fails', async () => {
    const api = assistantApi()
    api.submitAssistantInput.mockRejectedValue({ code: 'ASSISTANT_RUNTIME_ERROR', message: 'Runtime 启动失败' })
    const chat = useSupportChat({ api })
    await chat.load()
    await chat.selectConversation('conversation-1')

    expect(await chat.sendMessage('继续分析')).toBe(false)
    expect(chat.activeConversation.value?.messages).toEqual(snapshot.messages)
    expect(chat.notice.value).toBe('Runtime 启动失败')
  })

  it('keeps the active session stable while a turn is submitting', async () => {
    const api = assistantApi()
    let resolveSubmit!: (value: AssistantSnapshot) => void
    api.submitAssistantInput.mockImplementation(() => new Promise((resolve) => {
      resolveSubmit = resolve
    }))
    const chat = useSupportChat({ api })
    await chat.load()
    await chat.selectConversation('conversation-1')

    const submission = chat.sendMessage('继续分析')
    await Promise.resolve()
    chat.newConversation()
    await chat.selectConversation('conversation-1')
    await chat.deleteConversation('conversation-1')
    expect(await chat.renameConversation('conversation-1', '不应生效')).toBe(false)
    resolveSubmit(snapshot)

    expect(await submission).toBe(true)
    expect(chat.activeConversation.value?.id).toBe('conversation-1')
    expect(api.hideAssistantConversation).not.toHaveBeenCalled()
    expect(api.renameAssistantConversation).not.toHaveBeenCalled()
  })

  it('restores the persisted stopped state without reporting an interrupt error', async () => {
    const api = { ...assistantApi(), stopAssistant: vi.fn() }
    let rejectSubmit!: (reason: unknown) => void
    api.submitAssistantInput.mockImplementation(() => new Promise((_resolve, reject) => {
      rejectSubmit = reject
    }))
    api.stopAssistant.mockImplementation(async () => {
      rejectSubmit({ code: 'ASSISTANT_RUNTIME_EXITED', message: 'Runtime 已停止' })
    })
    api.getAssistantSnapshot
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValue({ ...snapshot, status: 'stopped' })
    const chat = useSupportChat({ api })
    await chat.load()
    await chat.selectConversation('conversation-1')

    const submission = chat.sendMessage('继续分析')
    await Promise.resolve()
    await chat.stop()

    expect(await submission).toBe(false)
    expect(chat.status.value).toBe('stopped')
    expect(chat.notice.value).toBeNull()
  })
})
