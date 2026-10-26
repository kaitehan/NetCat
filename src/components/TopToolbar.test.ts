import { render, screen, within } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import TopToolbar from './TopToolbar.vue'
import type { ProxyStatus } from '../shared/types'

const stoppedStatus: ProxyStatus = {
  state: 'stopped',
  mode: 'computer',
  bindAddress: null,
  port: 28086,
  lanIp: null,
  error: null
}

describe('TopToolbar', () => {
  it('keeps only runtime controls and traffic actions in the toolbar', () => {
    render(TopToolbar, {
      props: {
        status: stoppedStatus,
        busy: false
      }
    })

    const proxyGroup = screen.getByRole('group', { name: '代理控制' })
    expect(within(proxyGroup).getByRole('button', { name: '启动代理' })).toBeVisible()
    expect(within(proxyGroup).getByText('代理已停止')).toBeVisible()

    const actionsGroup = screen.getByRole('group', { name: '请求操作' })
    expect(within(actionsGroup).getByRole('button', { name: '清空请求记录' })).toBeVisible()
    expect(within(actionsGroup).getByRole('button', { name: '导出 HAR' })).toBeVisible()
    expect(screen.queryByRole('spinbutton', { name: '端口' })).not.toBeInTheDocument()
    expect(screen.queryByRole('checkbox', { name: '记录' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '系统代理' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '证书' })).not.toBeInTheDocument()
  })
})
