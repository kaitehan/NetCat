import { render, screen, within } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import TopToolbar from './TopToolbar.vue'
import type { ProxyStatus } from '../shared/types'

const stoppedStatus: ProxyStatus = {
  state: 'stopped',
  bindAddress: null,
  port: 8888,
  lanIp: null,
  error: null
}

describe('TopToolbar', () => {
  it('separates proxy status from capture settings and utility actions', () => {
    render(TopToolbar, {
      props: {
        status: stoppedStatus,
        allowLan: false,
        captureEnabled: true,
        systemProxyEnabled: false,
        caGenerated: true,
        port: 8888,
        busy: false
      }
    })

    const proxyGroup = screen.getByRole('group', { name: '代理控制' })
    expect(within(proxyGroup).getByRole('button', { name: '启动代理' })).toBeVisible()
    expect(within(proxyGroup).getByText('代理已停止')).toBeVisible()

    const settingsGroup = screen.getByRole('group', { name: '代理与流量设置' })
    expect(within(settingsGroup).getByRole('spinbutton', { name: '端口' })).toHaveValue(8888)
    expect(within(settingsGroup).getByRole('checkbox', { name: '局域网' })).not.toBeChecked()
    expect(within(settingsGroup).getByRole('checkbox', { name: '记录' })).toBeChecked()
    expect(within(settingsGroup).getByRole('button', { name: '系统代理' })).toBeDisabled()
    expect(within(settingsGroup).getByRole('button', { name: '清空流量' })).toBeVisible()
    expect(within(settingsGroup).getByRole('button', { name: '导出 HAR' })).toBeVisible()
    expect(within(settingsGroup).getByRole('button', { name: '证书' })).toBeVisible()
  })
})
