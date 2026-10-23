import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import SettingsPage from './SettingsPage.vue'

const props = {
  platform: { os: 'windows' as const, version: '10.0.19045', arch: 'x86_64', certificateTrustSupported: true },
  trustState: 'notTrusted' as const,
  privateKeyState: 'ready' as const,
  systemProxyStatus: { managed: false, recoveryPending: false, conflict: false },
  proxyStatus: {
    state: 'stopped' as const,
    bindAddress: null,
    port: 28086,
    lanIp: null,
    error: null
  },
  port: 28086,
  allowLan: false,
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

  it('shows the NetCat brand, version and author in system settings', async () => {
    render(SettingsPage, { props })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('heading', { name: '关于 NetCat' })).toBeVisible()
    expect(screen.getByText('奈特猫 · 本地守候每一次请求')).toBeVisible()
    expect(screen.getByText('0.3.0')).toBeVisible()
    expect(screen.getByText('kaite han')).toBeVisible()
  })

  it('disables the debug panel action while another settings action is busy', async () => {
    render(SettingsPage, { props: { ...props, busy: true } })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('button', { name: '打开调试面板' })).toBeDisabled()
  })
})
