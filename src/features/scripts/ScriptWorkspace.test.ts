import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import ScriptWorkspace from './ScriptWorkspace.vue'
import type { ProxyScript } from '../../shared/types'

const scripts: ProxyScript[] = [
  { id: 'proxy-1', name: '添加调试头', enabled: true, priority: 10, methods: ['GET'], matchKind: 'contains', urlPattern: '/api', source: 'function onRequest(ctx) { return ctx; }', kind: 'proxy', platform: null, builtin: false, createdAt: '' },
  { id: 'builtin-android-pinning', name: 'Android 常见 TLS 栈', enabled: true, priority: 100, methods: [], matchKind: 'contains', urlPattern: '', source: 'Java.perform(function () {});', kind: 'injection', platform: 'android', builtin: true, createdAt: '' }
]

describe('ScriptWorkspace', () => {
  it('separates proxy and injection scripts and protects built-ins', async () => {
    render(ScriptWorkspace, { props: { scripts, busy: false, testResult: null } })
    expect(screen.getByText('添加调试头')).toBeVisible()

    await fireEvent.click(screen.getByRole('tab', { name: '注入脚本' }))
    await fireEvent.click(screen.getByRole('button', { name: '编辑 Android 常见 TLS 栈' }))
    expect(screen.getByText('内置脚本只读')).toBeVisible()
    expect(screen.getByRole('button', { name: '保存脚本' })).toBeDisabled()
  })

  it('creates a proxy script with an explicit matcher and source', async () => {
    const view = render(ScriptWorkspace, { props: { scripts: [], busy: false, testResult: null } })
    await fireEvent.click(screen.getByRole('button', { name: '新建脚本' }))
    await fireEvent.update(screen.getByLabelText('脚本名称'), '改写 API')
    await fireEvent.update(screen.getByLabelText('URL 匹配内容'), '/v1/users')
    await fireEvent.update(screen.getByRole('textbox', { name: '脚本源码' }), 'function onRequest(ctx) { return { url: ctx.url }; }')
    await fireEvent.click(screen.getByRole('button', { name: '保存脚本' }))

    const firstSave = view.emitted('save')[0] as [ProxyScript]
    expect(firstSave[0]).toEqual(expect.objectContaining({
      name: '改写 API',
      urlPattern: '/v1/users',
      kind: 'proxy'
    }))
  })
})
