import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import PinningWorkspace from './PinningWorkspace.vue'
import type { PinningSessionConfig } from '../../shared/types'
import type { ProxyScript } from '../../shared/types'

const injectionScripts: ProxyScript[] = [
  { id: 'builtin-android-pinning', name: 'Android 常见 TLS 栈', enabled: true, priority: 100, methods: [], matchKind: 'contains', urlPattern: '', source: '', kind: 'injection', platform: 'android', builtin: true, createdAt: '' }
]

describe('PinningWorkspace', () => {
  it('shows actionable diagnostics when Frida is unavailable', () => {
    render(PinningWorkspace, {
      props: {
        environment: { available: false, version: null, error: 'command not found' },
        devices: [], applications: [], scripts: injectionScripts,
        status: { state: 'stopped', target: null, processId: null, error: null }, busy: false
      }
    })
    expect(screen.getByRole('status')).toHaveTextContent('未检测到 Frida')
    expect(screen.getByText(/pip install frida-tools/)).toBeVisible()
    expect(screen.getByRole('button', { name: '启动 Pinning 会话' })).toBeDisabled()
  })

  it('requires authorization and emits a spawn session configuration', async () => {
    const view = render(PinningWorkspace, {
      props: {
        environment: { available: true, version: '16.5.0', error: null },
        devices: [{ id: 'usb-1', kind: 'usb', name: 'Pixel 8' }],
        applications: [{ pid: 42, name: 'Demo', identifier: 'com.example.demo' }],
        scripts: injectionScripts,
        status: { state: 'stopped', target: null, processId: null, error: null }, busy: false
      }
    })
    await fireEvent.update(screen.getByLabelText('设备'), 'usb-1')
    await fireEvent.update(screen.getByLabelText('应用或进程'), 'com.example.demo')
    await fireEvent.click(screen.getByRole('checkbox', { name: /已获授权/ }))
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Android 常见 TLS 栈' }))
    await fireEvent.click(screen.getByRole('button', { name: '启动 Pinning 会话' }))

    const firstStart = view.emitted('start')[0] as [PinningSessionConfig]
    expect(firstStart[0]).toEqual({
      deviceId: 'usb-1', target: 'com.example.demo', mode: 'spawn',
      scriptIds: ['builtin-android-pinning'], gadgetAddress: null
    })
  })
})
