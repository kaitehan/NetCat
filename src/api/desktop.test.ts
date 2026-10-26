import { afterEach, describe, expect, it, vi } from 'vitest'
import { desktopApi } from './desktop'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

afterEach(() => {
  delete window.__TAURI_INTERNALS__
  vi.resetModules()
  vi.clearAllMocks()
})

describe('desktopApi browser fallback', () => {
  it('reports unsupported native trust without claiming ownership in browser preview', async () => {
    expect(await desktopApi.getPlatformInfo()).toMatchObject({ certificateTrustSupported: false })
    expect(await desktopApi.getSystemProxyStatus()).toEqual({ managed: false, recoveryPending: false, conflict: false })
    expect(await desktopApi.getCertificateTrustStatus()).toBe('unsupported')
    expect(await desktopApi.installCaTrust()).toBe('unsupported')
    expect(await desktopApi.removeCaTrust()).toBe('unsupported')
  })

  it('returns an actionable error when the app cannot open native DevTools', async () => {
    await expect(desktopApi.openDevtools()).rejects.toMatchObject({
      code: 'DEVTOOLS_UNAVAILABLE',
      message: '浏览器预览模式无法由应用打开调试面板',
      action: '请使用浏览器自带的开发者工具'
    })
  })
})

describe('desktopApi native platform commands', () => {
  it.each([
    ['getPlatformInfo', 'get_platform_info', { os: 'windows', version: '10.0.19045', arch: 'x86_64', certificateTrustSupported: true }],
    ['getSystemProxyStatus', 'get_system_proxy_status', { managed: false, recoveryPending: true, conflict: true }],
    ['getCertificateTrustStatus', 'get_certificate_trust_status', 'notTrusted'],
    ['getCertificateTrustDetails', 'get_certificate_trust_details', { publicTrust: 'trusted', privateKeyState: 'missing' }],
    ['installCaTrust', 'install_ca_trust', 'trusted'],
    ['removeCaTrust', 'remove_ca_trust', 'notTrusted'],
    ['getLanIp', 'get_lan_ip', '192.168.1.24']
  ] as const)('maps %s to its native command and preserves the result', async (method, command, response) => {
    window.__TAURI_INTERNALS__ = {} as Window['__TAURI_INTERNALS__']
    vi.mocked(invoke).mockResolvedValue(response)
    const { desktopApi: nativeApi } = await import('./desktop')
    const commandMethod = nativeApi[method]
    expect(commandMethod).toBeDefined()
    expect(await commandMethod!()).toEqual(response)
    expect(invoke).toHaveBeenCalledWith(command)
  })

  it('maps pairing lifecycle commands with their typed payloads', async () => {
    window.__TAURI_INTERNALS__ = {} as Window['__TAURI_INTERNALS__']
    vi.mocked(invoke)
      .mockResolvedValueOnce(15)
      .mockResolvedValueOnce({ url: 'http://192.168.1.8:28086/__netcat/pair/code', expiresAt: '2026-10-26T10:05:00Z' })
      .mockResolvedValueOnce([])
    const { desktopApi: nativeApi } = await import('./desktop')

    expect(await nativeApi.setPairingIdleTimeout(15)).toBe(15)
    expect(invoke).toHaveBeenNthCalledWith(1, 'set_pairing_idle_timeout', { minutes: 15 })
    await nativeApi.createPairingChallenge('device-1')
    expect(invoke).toHaveBeenNthCalledWith(2, 'create_pairing_challenge', { deviceToken: 'device-1' })
    await nativeApi.listPairedDevices()
    expect(invoke).toHaveBeenNthCalledWith(3, 'list_paired_devices')
  })
})
