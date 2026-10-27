import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import ScriptWorkspace from './ScriptWorkspace.vue'
import type { ProxyScript } from '../../shared/types'

const scripts: ProxyScript[] = [
  { id: 'proxy-1', name: '添加调试头', enabled: true, priority: 10, methods: ['GET'], matchKind: 'contains', urlPattern: '/api', source: 'function onRequest(ctx) { return ctx; }', createdAt: '' }
]

describe('ScriptWorkspace', () => {
  it('edits proxy scripts directly without script type tabs or injection settings', async () => {
    render(ScriptWorkspace, { props: { scripts, busy: false, testResult: null } })
    expect(screen.queryByRole('tablist', { name: '脚本类型' })).not.toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: '编辑 添加调试头' }))
    expect(screen.getByLabelText('请求方法')).toHaveValue('GET')
    expect(screen.queryByLabelText('注入平台')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保存脚本' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '测试脚本' })).toBeEnabled()
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
      urlPattern: '/v1/users'
    }))
    expect(Object.keys(firstSave[0]).sort()).toEqual(['id', 'name', 'enabled', 'priority', 'methods', 'matchKind', 'urlPattern', 'source', 'createdAt'].sort())
  })
})
