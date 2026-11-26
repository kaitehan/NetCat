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
    appendAssistantError: vi.fn().mockImplementation(async (id: string, content: string) => ({
      ...snapshots.get(id)!,
      messages: [...snapshots.get(id)!.messages, {
        id: 'error-message',
        role: 'system',
        content,
        createdAt: '2026-11-24T12:00:00Z',
        messageType: 'error'
      }]
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

  it('clears the composer as soon as submission starts', async () => {
    const api = assistantApi()
    api.submitAssistantInput.mockImplementation(() => new Promise(() => {}))
    render(SupportChatPanel, { props: { api } })
    const input = await screen.findByRole('textbox', { name: '输入消息' })

    await fireEvent.update(input, '立即清空')
    await fireEvent.keyDown(input, { key: 'Enter' })

    expect(input).toHaveValue('')
  })

  it('renders assistant markdown, avatar, timestamps, and persistent error messages', async () => {
    const api = assistantApi()
    api.getAssistantSnapshot.mockResolvedValue({
      ...snapshot(conversations[0]!),
      messages: [
        {
          id: 'assistant-message',
          role: 'assistant',
          content: '## 分析结果\n\n- 接口返回 401',
          createdAt: '2026-11-24T11:00:00Z'
        },
        {
          id: 'system-error',
          role: 'system',
          content: '无法解析 cURL',
          createdAt: '2026-11-24T11:01:00Z',
          messageType: 'error'
        }
      ]
    })
    render(SupportChatPanel, { props: { api } })

    expect(await screen.findByRole('heading', { name: '分析结果' })).toBeVisible()
    expect(screen.getByRole('img', { name: '智奈' })).toBeVisible()
    expect(screen.getByText('无法解析 cURL').closest('article')).toHaveAttribute('data-message-type', 'error')
    expect(screen.getAllByText(/\d{2}-\d{2} \d{2}:\d{2}/)).toHaveLength(2)
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

  it('opens a generated Mock draft for review without saving it immediately', async () => {
    const api = assistantApi()
    api.getAssistantSnapshot.mockResolvedValue({
      ...snapshot(conversations[0]!),
      drafts: [{
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
      }]
    })
    const view = render(SupportChatPanel, { props: { api } })

    await fireEvent.click(await screen.findByRole('button', { name: '按以上信息创建 Mock 规则' }))
    expect(api.saveMockRule).not.toHaveBeenCalled()
    expect(api.resolveAssistantDraft).toHaveBeenCalledWith('first', 'mock-draft', 'accepted')
    await waitFor(() => {
      const events = (view.emitted().prepareMockRule
        ?? view.emitted()['prepare-mock-rule']) as unknown[][] | undefined
      expect(events?.[0]?.[0]).toMatchObject({
        id: 'mock-1',
        name: '登录失败',
        enabled: false,
        methods: ['POST'],
        status: 401,
        responseBody: '{"error":"unauthorized"}'
      })
    })
  })

  it('saves a validated QuickJS draft disabled after explicit confirmation', async () => {
    const api = assistantApi()
    api.getAssistantSnapshot.mockResolvedValue({
      ...snapshot(conversations[0]!),
      drafts: [{
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
      }]
    })
    render(SupportChatPanel, { props: { api } })

    await fireEvent.click(await screen.findByRole('button', { name: '保存 QuickJS 草稿' }))
    expect(api.saveScript).toHaveBeenCalledWith(expect.objectContaining({ id: 'script-1', enabled: false }))
    expect(api.resolveAssistantDraft).toHaveBeenCalledWith('first', 'script-draft', 'accepted')
  })

  it('renders action drafts directly after the assistant message that produced them', async () => {
    const api = assistantApi()
    api.getAssistantSnapshot.mockResolvedValue({
      ...snapshot(conversations[0]!),
      messages: [
        {
          id: 'assistant-first',
          role: 'assistant',
          content: '第一轮筛选完成',
          createdAt: '2026-11-25T10:00:00Z'
        },
        {
          id: 'user-second',
          role: 'user',
          content: '继续排查',
          createdAt: '2026-11-25T10:01:00Z'
        }
      ],
      drafts: [{
        id: 'search-first',
        kind: 'trafficSearch',
        status: 'pending',
        messageId: 'assistant-first',
        createdAt: '2026-11-25T10:00:00Z',
        payload: {
          filter: { query: 'users', domain: '', method: null, statusGroup: null },
          total: 0,
          summaries: []
        }
      }]
    })
    render(SupportChatPanel, { props: { api } })

    const assistantMessage = await screen.findByText('第一轮筛选完成')
    const actionCard = screen.getByRole('region', { name: '抓包搜索结果' })
    const nextMessage = screen.getByText('继续排查')
    expect(assistantMessage.compareDocumentPosition(actionCard) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(actionCard.compareDocumentPosition(nextMessage) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('compacts superseded drafts and leaves only the latest same-kind action available', async () => {
    const api = assistantApi()
    api.getAssistantSnapshot.mockResolvedValue({
      ...snapshot(conversations[0]!),
      messages: [
        {
          id: 'assistant-first',
          role: 'assistant',
          content: '第一轮筛选',
          createdAt: '2026-11-25T10:00:00Z'
        },
        {
          id: 'assistant-second',
          role: 'assistant',
          content: '第二轮筛选',
          createdAt: '2026-11-25T10:01:00Z'
        }
      ],
      drafts: [
        {
          id: 'search-first',
          kind: 'trafficSearch',
          status: 'superseded',
          messageId: 'assistant-first',
          createdAt: '2026-11-25T10:00:00Z',
          payload: {
            filter: { query: 'users', domain: '', method: null, statusGroup: null },
            total: 4,
            summaries: [{
              method: 'GET',
              url: 'https://api.example.com/users',
              status: 200,
              durationMs: 12,
              startedAt: '2026-11-25T09:59:00Z'
            }]
          }
        },
        {
          id: 'search-second',
          kind: 'trafficSearch',
          status: 'pending',
          messageId: 'assistant-second',
          createdAt: '2026-11-25T10:01:00Z',
          payload: {
            filter: { query: 'orders', domain: '', method: null, statusGroup: null },
            total: 2,
            summaries: []
          }
        }
      ]
    })
    render(SupportChatPanel, { props: { api } })

    expect(await screen.findByText('已被后续操作替代')).toBeVisible()
    expect(screen.queryByText('https://api.example.com/users')).not.toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: '在网络请求中查看' })).toHaveLength(1)
    expect(screen.getByText('关键词：users')).toBeVisible()
    expect(screen.getByText('关键词：orders')).toBeVisible()
  })
})
