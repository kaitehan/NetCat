import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import MockEditor from './MockEditor.vue'

describe('MockEditor', () => {
  it('blocks invalid regex and explains the field error', async () => {
    const { emitted } = render(MockEditor, { props: { rule: null } })

    await fireEvent.update(screen.getByLabelText('规则名称'), '用户列表')
    await fireEvent.update(screen.getByLabelText('匹配方式'), 'regex')
    await fireEvent.update(screen.getByLabelText('URL 匹配'), '[')
    await fireEvent.click(screen.getByRole('button', { name: '保存规则' }))

    expect(screen.getByRole('alert')).toHaveTextContent('正则表达式无效')
    expect(emitted().save).toBeUndefined()
  })

  it('emits a normalized valid rule', async () => {
    const { emitted } = render(MockEditor, { props: { rule: null } })

    expect(screen.getByRole('region', { name: '响应正文' })).toBeVisible()
    await fireEvent.update(screen.getByLabelText('规则名称'), '用户列表')
    await fireEvent.update(screen.getByLabelText('URL 匹配'), '/users')
    await fireEvent.update(screen.getByRole('textbox', { name: '响应正文' }), '{"ok":true}')
    await fireEvent.click(screen.getByRole('button', { name: '保存规则' }))

    const saved = emitted().save as unknown[][] | undefined
    expect(saved?.[0]?.[0]).toMatchObject({
      name: '用户列表',
      urlPattern: '/users',
      responseBody: '{"ok":true}',
      methods: ['GET']
    })
  })

  it('focuses the requested response field for a generated draft', async () => {
    render(MockEditor, {
      props: {
        rule: null,
        initialFocus: 'body'
      }
    })

    await waitFor(() => expect(screen.getByRole('textbox', { name: '响应正文' })).toHaveFocus())
  })
})
