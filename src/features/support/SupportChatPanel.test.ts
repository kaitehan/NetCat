import { fireEvent, render, screen, waitFor, within } from '@testing-library/vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AssistantConversation, AssistantSnapshot } from '../../shared/types'
import SupportChatPanel from './SupportChatPanel.vue'

const conversations: AssistantConversation[] = [
  {
    id: 'first',
    sessionId: 'session-first',
    title: 'HTTPS 抓包问题',
    createdAt: '2026-11-24T10:00:00Z',
    updatedAt: '2026-11-24T11:00:00Z',
    hidden: false
  },
  {
    id: 'second',
    sessionId: 'session-second',
    title: '代理配置',
    createdAt: '2026-11-24T09:00:00Z',
    updatedAt: '2026-11-24T10:00:00Z',
    hidden: false
  }
]

function snapshot(conversation: AssistantConversation, content = ''): AssistantSnapshot {
  return {
    conversation,
    messages: content ? [{
      id: `message-${conversation.id}`,
      role: 'user',
      content,
      createdAt: conversation.updatedAt
    }] : [],
    activities: [],
    drafts: [],
    status: 'ready'
  }
}

function assistantApi() {
  const snapshots = new Map([
    ['first', snapshot(conversations[0]!, '为什么抓不到 HTTPS？')],
    ['second', snapshot(conversations[1]!)]
  ])
  return {
    listAssistantConversations: vi.fn().mockResolvedValue(conversations),
    createAssistantConversation: vi.fn().mockResolvedValue(conversations[0]),
    getAssistantSnapshot: vi.fn().mockImplementation(async (id: string) => snapshots.get(id)),
    renameAssistantConversation: vi.fn().mockImplementation(async (id: string, title: string) => ({
      ...conversations.find((conversation) => conversation.id === id)!,
      title
    })),
    hideAssistantConversation: vi.fn().mockResolvedValue(undefined),
    authorizeAssistantNetworkRequest: vi.fn().mockResolvedValue('single-use-handle'),
    executeAssistantNetworkRequest: vi.fn().mockImplementation(async (id: string) => ({
      ...snapshots.get(id)!,
      drafts: [],
      status: 'completed'
    })),
    saveMockRule: vi.fn().mockImplementation(async (rule) => rule),
    saveScript: vi.fn().mockImplementation(async (script) => script),
    resolveAssistantDraft: vi.fn().mockImplementation(async (id: string, draftId: string) => ({
      ...snapshots.get(id)!,
      drafts: snapshots.get(id)!.drafts.map((draft) => draft.id === draftId
        ? { ...draft, status: 'accepted' }
        : draft)
    })),
    submitAssistantInput: vi.fn().mockImplementation(async (id: string, input: { message: string }) => ({
      ...snapshots.get(id)!,
      messages: [{
        id: 'sent-message',
        role: 'user',
        content: input.message,
        createdAt: '2026-11-24T12:00:00Z'
      }]
    }))
  }
}

describe('SupportChatPanel', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('loads histories, restores the newest session, and switches conversations', async () => {
    const api = assistantApi()
    render(SupportChatPanel, { props: { api } })

    expect(await screen.findByRole('log', { name: 'HTTPS 抓包问题的消息' })).toHaveTextContent('为什么抓不到 HTTPS？')
    await fireEvent.click(screen.getByRole('button', { name: '打开会话 代理配置' }))

    expect(await screen.findByRole('heading', { name: '代理配置' })).toBeVisible()
    expect(api.getAssistantSnapshot).toHaveBeenLastCalledWith('second')
  })

  it('sends with Enter while preserving Shift+Enter and composition input', async () => {
    const api = assistantApi()
    render(SupportChatPanel, { props: { api } })
    const input = await screen.findByRole('textbox', { name: '输入消息' })

    await fireEvent.update(input, '第一行')
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: true })
    expect(input).toHaveValue('第一行')

    await fireEvent.keyDown(input, { key: 'Enter', isComposing: true })
    expect(input).toHaveValue('第一行')

    await fireEvent.keyDown(input, { key: 'Enter' })
    await waitFor(() => expect(input).toHaveValue(''))
    expect(api.submitAssistantInput).toHaveBeenCalledWith('first', { kind: 'message', message: '第一行' })
    expect(screen.getByRole('log', { name: 'HTTPS 抓包问题的消息' })).toHaveTextContent('第一行')
  })

  it('renames and hides a conversation through visible controls', async () => {
    const api = assistantApi()
    vi.spyOn(window, 'confirm').mockReturnValue(true)
    render(SupportChatPanel, { props: { api } })
    const history = await screen.findByRole('navigation', { name: '历史对话' })
    await screen.findByRole('button', { name: '重命名 HTTPS 抓包问题' })

    await fireEvent.click(within(history).getByRole('button', { name: '重命名 HTTPS 抓包问题' }))
    const titleInput = within(history).getByRole('textbox', { name: '会话名称' })
    await fireEvent.update(titleInput, '证书排查')
    await fireEvent.keyDown(titleInput, { key: 'Enter' })
    expect(await screen.findByRole('heading', { name: '证书排查' })).toBeVisible()

    await fireEvent.click(within(history).getByRole('button', { name: '删除 证书排查' }))
    await waitFor(() => expect(api.hideAssistantConversation).toHaveBeenCalledWith('first'))
    expect(screen.queryByText('证书排查')).not.toBeInTheDocument()
    expect(await screen.findByRole('heading', { name: '代理配置' })).toBeVisible()
  })

  it('shows API errors without discarding the panel and emits close', async () => {
    const api = assistantApi()
    api.listAssistantConversations.mockRejectedValue({ code: 'ASSISTANT_RUNTIME_ERROR', message: 'Runtime 启动失败' })
    const view = render(SupportChatPanel, { props: { api } })

    expect(await screen.findByRole('alert')).toHaveTextContent('Runtime 启动失败')
    await fireEvent.click(screen.getByRole('button', { name: '关闭智奈助手' }))
    expect(view.emitted().close).toHaveLength(1)
  })

  it('creates a new session and submits an exchange opened from traffic details', async () => {
    const api = assistantApi()
    render(SupportChatPanel, { props: { api, initialExchangeId: 'exchange-42' } })

    await waitFor(() => expect(api.createAssistantConversation).toHaveBeenCalledOnce())
    await waitFor(() => expect(api.submitAssistantInput).toHaveBeenCalledWith('first', {
        kind: 'exchange',
        exchangeId: 'exchange-42'
    }))
  })

  it('shows a bounded network preview and executes it only after confirmation', async () => {
    const api = assistantApi()
    api.getAssistantSnapshot.mockResolvedValue({
      ...snapshot(conversations[0]!),
      status: 'waitingApproval',
      drafts: [{
        id: 'network-1',
        kind: 'networkRequest',
        status: 'pending',
        payload: {
          domain: 'api.example.com',
          resolvedIps: ['203.0.113.8'],
          method: 'POST',
          bodyBytes: 128,
          sensitiveFields: ['header.authorization'],
          tlsVerification: true,
          insecureOptionIgnored: false
        }
      }]
    })
    render(SupportChatPanel, { props: { api } })

    expect(await screen.findByText('api.example.com')).toBeVisible()
    expect(screen.getByText('203.0.113.8')).toBeVisible()
    expect(screen.getByText('POST · 128 B')).toBeVisible()
    expect(screen.getByText('检测到 1 个敏感字段')).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '执行联网诊断' }))

    await waitFor(() => expect(api.authorizeAssistantNetworkRequest).toHaveBeenCalledOnce())
    expect(api.executeAssistantNetworkRequest).toHaveBeenCalledWith('first', 'single-use-handle')
  })

  it('saves validated Mock and QuickJS drafts disabled after explicit confirmation', async () => {
    const api = assistantApi()
    api.getAssistantSnapshot.mockResolvedValue({
      ...snapshot(conversations[0]!),
      drafts: [
        {
          id: 'mock-draft',
          kind: 'mock',
          status: 'pending',
          payload: {
            id: 'mock-1',
            name: '登录失败',
            enabled: false,
            priority: 0,
            methods: ['POST'],
            matchKind: 'exact',
            urlPattern: 'https://api.example.com/login',
            status: 401,
            responseHeaders: {},
            responseBody: '{"error":"unauthorized"}',
            delayMs: 0,
            createdAt: '2026-11-24T12:00:00Z'
          }
        },
        {
          id: 'script-draft',
          kind: 'script',
          status: 'pending',
          payload: {
            id: 'script-1',
            name: '添加诊断头',
            enabled: false,
            priority: 0,
            methods: ['POST'],
            matchKind: 'exact',
            urlPattern: 'https://api.example.com/login',
            source: 'function onRequest(request) { return request; }',
            createdAt: '2026-11-24T12:00:00Z'
          }
        }
      ]
    })
    render(SupportChatPanel, { props: { api } })

    await fireEvent.click(await screen.findByRole('button', { name: '保存 Mock 草稿' }))
    expect(api.saveMockRule).toHaveBeenCalledWith(expect.objectContaining({ id: 'mock-1', enabled: false }))
    expect(api.resolveAssistantDraft).toHaveBeenCalledWith('first', 'mock-draft', 'accepted')

    await fireEvent.click(screen.getByRole('button', { name: '保存 QuickJS 草稿' }))
    expect(api.saveScript).toHaveBeenCalledWith(expect.objectContaining({ id: 'script-1', enabled: false }))
    expect(api.resolveAssistantDraft).toHaveBeenCalledWith('first', 'script-draft', 'accepted')
  })
})
