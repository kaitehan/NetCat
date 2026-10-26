import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import CertificatePanel from './CertificatePanel.vue'

const platform = { os: 'windows' as const, version: '10.0.19045', arch: 'x86_64', certificateTrustSupported: true }
const generatedCa = { generated: true, certificatePath: 'C:\\NetCat\\ca.crt', fingerprint: 'AA:BB' }
const stoppedStatus = { state: 'stopped' as const, bindAddress: null, port: 28086, lanIp: null, error: null }
const windowsProps = { platform, trustState: 'notTrusted' as const, privateKeyState: 'ready' as const, caStatus: generatedCa, proxyStatus: stoppedStatus, allowLan: false, busy: false }

describe('CertificatePanel', () => {
  it.each(['missing', 'mismatch', 'unavailable'] as const)('shows trusted public identity and permits only removal when the private key is %s', (privateKeyState) => {
    render(CertificatePanel, { props: { ...windowsProps, trustState: 'trusted', privateKeyState } })
    expect(screen.getByText('当前 CA 已信任')).toBeVisible()
    expect(screen.getByText(/CA 私钥/)).toBeVisible()
    expect(screen.getByRole('button', { name: '移除信任' })).toBeEnabled()
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '重新生成证书' })).not.toBeInTheDocument()
  })

  it.each(['missing', 'mismatch', 'unavailable'] as const)('never offers trust mutation for an untrusted public certificate with a %s private key', (privateKeyState) => {
    render(CertificatePanel, { props: { ...windowsProps, privateKeyState } })
    expect(screen.getByText('当前 CA 尚未信任')).toBeVisible()
    expect(screen.queryByRole('button', { name: '移除信任' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
  })

  it.each(['missing', 'mismatch'] as const)('offers CA regeneration when the public certificate is untrusted and the private key is %s', async (privateKeyState) => {
    const { emitted } = render(CertificatePanel, { props: { ...windowsProps, privateKeyState } })

    const regenerate = screen.getByRole('button', { name: '重新生成证书' })
    expect(regenerate).toBeEnabled()
    await fireEvent.click(regenerate)
    expect(emitted().generate).toHaveLength(1)
  })

  it('shows neutral unavailable platform copy until platform information is ready', () => {
    render(CertificatePanel, { props: { ...windowsProps, platform: null } })
    expect(screen.getByText('平台信息加载中或暂不可用')).toBeVisible()
    expect(screen.queryByRole('heading', { name: '本机 Windows' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
  })

  it('offers current-user trust installation on Windows only after an explicit click', async () => {
    const { emitted } = render(CertificatePanel, { props: windowsProps })

    expect(emitted().trust).toBeUndefined()
    expect(screen.getByText(/当前用户/)).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '安装信任' }))
    expect(emitted().trust).toHaveLength(1)
    expect(screen.queryByText('本机 macOS')).not.toBeInTheDocument()
  })

  it('offers explicit removal only for the trusted current certificate', async () => {
    const { emitted } = render(CertificatePanel, { props: { ...windowsProps, trustState: 'trusted' } })

    expect(screen.getByText('当前 CA 已信任')).toBeVisible()
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: '移除信任' }))
    expect(emitted().removeTrust).toHaveLength(1)
  })

  it.each([
    ['notGenerated', false, '请先生成本地 CA'],
    ['mismatch', true, '证书指纹不匹配'],
    ['unavailable', true, '无法读取证书信任状态'],
    ['unsupported', true, '当前环境不支持自动管理证书信任']
  ] as const)('explains %s without allowing trust installation', (trustState, generated, message) => {
    render(CertificatePanel, { props: { ...windowsProps, trustState, caStatus: { ...generatedCa, generated } } })
    expect(screen.getByText(message, { exact: false })).toBeVisible()
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '移除信任' })).not.toBeInTheDocument()
  })

  it('hides unsupported Windows trust actions even if the reported state is notTrusted', () => {
    render(CertificatePanel, { props: { ...windowsProps, platform: { ...platform, certificateTrustSupported: false } } })
    expect(screen.getByText(/当前环境不支持自动管理证书信任/)).toBeVisible()
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
  })

  it('disables trust and export actions while busy', () => {
    render(CertificatePanel, { props: { ...windowsProps, busy: true } })
    expect(screen.getByRole('button', { name: '安装信任' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '导出' })).toBeDisabled()
  })

  it('preserves macOS export and Keychain instructions without native trust buttons', async () => {
    const { emitted } = render(CertificatePanel, { props: { ...windowsProps, platform: { ...platform, os: 'macos', certificateTrustSupported: false }, trustState: 'unsupported' } })
    expect(screen.getByText('本机 macOS')).toBeVisible()
    expect(screen.getByText(/导出证书并双击加入“钥匙串访问”/)).toBeVisible()
    await fireEvent.click(screen.getByRole('button', { name: '导出' }))
    expect(emitted().export).toHaveLength(1)
    expect(screen.queryByRole('button', { name: '安装信任' })).not.toBeInTheDocument()
  })

  it('renders a scannable CA download QR code when LAN capture is available', () => {
    render(CertificatePanel, {
      props: {
        platform: { ...platform, os: 'macos', certificateTrustSupported: false },
        trustState: 'unsupported',
        privateKeyState: 'ready',
        open: true,
        caStatus: {
          generated: true,
          certificatePath: '/tmp/netcat-ca.crt',
          fingerprint: 'AA:BB'
        },
        proxyStatus: {
          state: 'running',
          bindAddress: '0.0.0.0',
          port: 8888,
          lanIp: '192.168.1.24',
          error: null
        },
        allowLan: true,
        busy: false
      }
    })

    expect(screen.getByLabelText('CA 证书下载二维码')).toBeVisible()
    expect(screen.getByText('http://192.168.1.24:8888/__netcat/ca.crt')).toBeVisible()
    expect(screen.getByText(/NetCat Local Root CA/)).toBeVisible()
  })
})
