import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it, vi } from 'vitest'
import SettingsPage from './SettingsPage.vue'

const props = {
  platform: { os: 'windows' as const, version: '10.0.19045', arch: 'x86_64', certificateTrustSupported: true },
  trustState: 'notTrusted' as const,
  privateKeyState: 'ready' as const,
  systemProxyStatus: { managed: false, recoveryPending: false, conflict: false },
  proxyStatus: {
    state: 'stopped' as const,
    mode: 'computer' as const,
    bindAddress: null,
    port: 28086,
    lanIp: null,
    error: null
  },
  port: 28086,
  proxyMode: 'computer' as const,
  lanIp: '192.168.1.24',
  pairedDevices: [],
  historicalDevices: [],
  pairingIdleTimeout: 15,
  pairingChallenge: null,
  captureEnabled: true,
  systemProxyPreferred: true,
  systemProxyEnabled: false,
  caStatus: {
    generated: true,
    certificatePath: '/tmp/netcat-ca.crt',
    fingerprint: 'AA:BB'
  },
  busy: false
}

describe('SettingsPage', () => {
  it('switches capture source while stopped and exposes the bounded idle timeout', async () => {
    const { emitted } = render(SettingsPage, { props })

    await fireEvent.click(screen.getByRole('radio', { name: '手机抓包' }))
    expect(emitted().updateProxyMode).toEqual([['mobile']])
    const timeout = screen.getByRole('spinbutton', { name: '空闲断开时间' })
    expect(timeout).toHaveAttribute('min', '5')
    expect(timeout).toHaveAttribute('max', '120')
    await fireEvent.update(timeout, '30')
    await fireEvent.change(timeout)
    expect(emitted().updatePairingIdleTimeout).toEqual([[30]])
  })

  it('keeps proxy startup and one-time pairing in one mobile connection flow', async () => {
    const { emitted, rerender } = render(SettingsPage, { props })

    expect(screen.getByRole('heading', { name: '手机连接' })).toBeVisible()
    await rerender({
      ...props,
      proxyMode: 'mobile',
      proxyStatus: { ...props.proxyStatus, state: 'running', mode: 'mobile', lanIp: '192.168.1.24' },
      pairingChallenge: { url: 'http://192.168.1.24:28086/__netcat/pair/once', expiresAt: '2026-10-26T12:00:00Z' }
    })
    expect(screen.getByText('192.168.1.24:28086')).toBeVisible()
    expect(screen.getByLabelText('手机配对二维码')).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '停止代理' }))
    expect(emitted().toggleProxy).toHaveLength(1)

    await fireEvent.click(screen.getByRole('tab', { name: '证书配置' }))
    expect(screen.queryByRole('heading', { name: 'iOS / Android' })).not.toBeInTheDocument()
  })

  it('shows online and historical devices and forwards their management actions', async () => {
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('开发手机')
    const online = {
      deviceToken: 'online-token',
      name: '测试手机',
      boundIp: '192.168.1.42',
      createdAt: '2026-10-26T10:00:00Z',
      lastSeenAt: '2026-10-26T10:10:00Z'
    }
    const { emitted } = render(SettingsPage, { props: {
      ...props,
      proxyMode: 'mobile',
      proxyStatus: { ...props.proxyStatus, mode: 'mobile', state: 'running' },
      pairedDevices: [online],
      historicalDevices: [
        { deviceToken: 'online-token', name: '测试手机', requestCount: 2 },
        { deviceToken: 'history-token', name: '历史手机', requestCount: 3 }
      ]
    } })

    expect(screen.getByText('192.168.1.42')).toBeVisible()
    expect(screen.getByText('3 条历史请求')).toBeVisible()
    expect(screen.getByRole('checkbox', { name: '系统代理' })).toBeDisabled()
    await fireEvent.click(screen.getAllByRole('button', { name: '改名' })[0])
    await fireEvent.click(screen.getAllByRole('button', { name: '重新配对' })[1])
    await fireEvent.click(screen.getByRole('button', { name: '断开设备' }))
    await fireEvent.click(screen.getAllByRole('button', { name: '删除设备数据' })[1])

    expect(emitted().renameDevice).toEqual([['online-token', '开发手机']])
    expect(emitted().rePairDevice).toEqual([['history-token']])
    expect(emitted().disconnectDevice).toEqual([['online-token']])
    expect(emitted().deleteDevice).toEqual([['history-token']])
    prompt.mockRestore()
  })

  it('shows proxy conflicts and requests recovery without reporting the proxy as enabled', async () => {
    const { emitted } = render(SettingsPage, { props: {
      ...props,
      proxyStatus: { ...props.proxyStatus, state: 'running' },
      systemProxyEnabled: true,
      systemProxyStatus: { managed: true, recoveryPending: true, conflict: true }
    } })
    expect(screen.getByRole('checkbox', { name: '系统代理' })).not.toBeChecked()
    expect(screen.getByText(/系统代理已被其他程序或用户修改/)).toBeVisible()
    expect(screen.queryByText('本机 HTTP/HTTPS 流量已接入。')).not.toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: '重试恢复系统代理' }))
    expect(emitted().restoreSystemProxy).toHaveLength(1)
  })

  it('offers pending recovery while stopped and disables recovery when busy', () => {
    render(SettingsPage, { props: { ...props, busy: true, systemProxyStatus: { managed: false, recoveryPending: true, conflict: false } } })
    expect(screen.getByRole('checkbox', { name: '系统代理' })).not.toBeChecked()
    expect(screen.getByText(/上次系统代理配置尚未恢复/)).toBeVisible()
    expect(screen.getByRole('button', { name: '恢复系统代理' })).toBeDisabled()
  })

  it('reflects actual system proxy ownership while running', () => {
    render(SettingsPage, { props: { ...props, proxyStatus: { ...props.proxyStatus, state: 'running' } } })
    expect(screen.getByRole('checkbox', { name: '系统代理' })).not.toBeChecked()
  })

  it('forwards explicit certificate trust actions', async () => {
    const { emitted, rerender } = render(SettingsPage, { props })
    await fireEvent.click(screen.getByRole('tab', { name: '证书配置' }))
    await fireEvent.click(screen.getByRole('button', { name: '安装信任' }))
    expect(emitted().trustCa).toHaveLength(1)
    await rerender({ ...props, trustState: 'trusted' })
    await fireEvent.click(screen.getByRole('button', { name: '移除信任' }))
    expect(emitted().removeCaTrust).toHaveLength(1)
  })

  it('opens the system configuration tab and requests the diagnostics panel', async () => {
    const { emitted } = render(SettingsPage, { props })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('heading', { name: '调试工具' })).toBeVisible()
    const button = screen.getByRole('button', { name: '打开调试面板' })
    await fireEvent.click(button)
    expect(emitted().openDiagnostics).toHaveLength(1)
    await fireEvent.click(screen.getByRole('button', { name: '导出诊断包' }))
    expect(emitted().exportDiagnostics).toHaveLength(1)
  })

  it('keeps About content out of system settings', async () => {
    render(SettingsPage, { props })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('heading', { name: '调试工具' })).toBeVisible()
    expect(screen.queryByRole('heading', { name: '关于 NetCat' })).not.toBeInTheDocument()
    expect(screen.queryByText('奈特猫 · 本地守候每一次请求')).not.toBeInTheDocument()
  })

  it('disables the debug panel action while another settings action is busy', async () => {
    render(SettingsPage, { props: { ...props, busy: true } })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('button', { name: '打开调试面板' })).toBeDisabled()
  })
})
