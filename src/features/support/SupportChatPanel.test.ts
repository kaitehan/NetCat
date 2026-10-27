import { fireEvent, render, screen, within } from '@testing-library/vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SUPPORT_CHAT_STORAGE_KEY, type SupportChatState } from './support-chat-storage'
import SupportChatPanel from './SupportChatPanel.vue'

const historyState: SupportChatState = {
  version: 1,
  activeConversationId: 'first',
  conversations: [
    {
      id: 'first',
      title: 'HTTPS 抓包问题',
      createdAt: '2026-10-27T09:00:00.000Z',
      updatedAt: '2026-10-27T09:01:00.000Z',
      messages: [
        { id: 'message-1', role: 'user', content: '为什么抓不到 HTTPS？', createdAt: '2026-10-27T09:01:00.000Z' }
      ]
    },
    {
      id: 'second',
      title: '代理配置',
      createdAt: '2026-10-27T10:00:00.000Z',
      updatedAt: '2026-10-27T10:00:00.000Z',
      messages: []
    }
  ]
}

describe('SupportChatPanel', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  it('restores histories, switches conversations, and creates a new draft', async () => {
    localStorage.setItem(SUPPORT_CHAT_STORAGE_KEY, JSON.stringify(historyState))
    render(SupportChatPanel)

    expect(screen.getByRole('complementary', { name: '智奈助手' })).toBeVisible()
    expect(screen.getByRole('log', { name: 'HTTPS 抓包问题的消息' })).toHaveTextContent('为什么抓不到 HTTPS？')

    await fireEvent.click(screen.getByRole('button', { name: '打开会话 代理配置' }))
    expect(screen.getByRole('heading', { name: '代理配置' })).toBeVisible()

    await fireEvent.click(screen.getByRole('button', { name: '新建对话' }))
    expect(screen.getByText('你好，我是小橘')).toBeVisible()
    expect(screen.getByRole('button', { name: '发送消息' })).toBeDisabled()
  })

  it('sends with Enter while preserving Shift+Enter and composition input', async () => {
    render(SupportChatPanel)
    const input = screen.getByRole('textbox', { name: '输入消息' })

    await fireEvent.update(input, '第一行')
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: true })
    expect(input).toHaveValue('第一行')

    await fireEvent.keyDown(input, { key: 'Enter', isComposing: true })
    expect(input).toHaveValue('第一行')

    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(input).toHaveValue('')
    expect(screen.getByRole('log', { name: '第一行的消息' })).toHaveTextContent('第一行')
  })

  it('renames and deletes a saved conversation through visible controls', async () => {
    localStorage.setItem(SUPPORT_CHAT_STORAGE_KEY, JSON.stringify(historyState))
    vi.spyOn(window, 'confirm').mockReturnValue(true)
    render(SupportChatPanel)

    const history = screen.getByRole('navigation', { name: '历史对话' })
    await fireEvent.click(within(history).getByRole('button', { name: '重命名 HTTPS 抓包问题' }))
    const titleInput = within(history).getByRole('textbox', { name: '会话名称' })
    await fireEvent.update(titleInput, '证书排查')
    await fireEvent.keyDown(titleInput, { key: 'Enter' })
    expect(screen.getByRole('heading', { name: '证书排查' })).toBeVisible()

    await fireEvent.click(within(history).getByRole('button', { name: '删除 证书排查' }))
    expect(window.confirm).toHaveBeenCalledWith('确认删除会话“证书排查”吗？')
    expect(screen.queryByText('证书排查')).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '代理配置' })).toBeVisible()
  })

  it('shows a non-blocking warning for damaged history and emits close', async () => {
    localStorage.setItem(SUPPORT_CHAT_STORAGE_KEY, '{bad json')
    const view = render(SupportChatPanel)

    expect(screen.getByRole('alert')).toHaveTextContent('历史对话读取失败，已打开空白会话')
    await fireEvent.click(screen.getByRole('button', { name: '关闭智奈助手' }))
    expect(view.emitted().close).toHaveLength(1)
  })
})
