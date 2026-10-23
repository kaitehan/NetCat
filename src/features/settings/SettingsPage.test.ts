import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import SettingsPage from './SettingsPage.vue'

const props = {
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
  it('opens the system configuration tab and requests the diagnostics panel', async () => {
    const { emitted } = render(SettingsPage, { props })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('heading', { name: '调试工具' })).toBeVisible()
    const button = screen.getByRole('button', { name: '打开调试面板' })
    await fireEvent.click(button)
    expect(emitted().openDiagnostics).toHaveLength(1)
  })

  it('shows the NetCat brand, version and author in system settings', async () => {
    render(SettingsPage, { props })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('heading', { name: '关于 NetCat' })).toBeVisible()
    expect(screen.getByText('奈特猫 · 本地守候每一次请求')).toBeVisible()
    expect(screen.getByText('0.2.0')).toBeVisible()
    expect(screen.getByText('kaite han')).toBeVisible()
  })

  it('disables the debug panel action while another settings action is busy', async () => {
    render(SettingsPage, { props: { ...props, busy: true } })

    await fireEvent.click(screen.getByRole('tab', { name: '系统配置' }))

    expect(screen.getByRole('button', { name: '打开调试面板' })).toBeDisabled()
  })
})
