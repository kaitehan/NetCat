import { render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import CertificatePanel from './CertificatePanel.vue'

describe('CertificatePanel', () => {
  it('renders a scannable CA download QR code when LAN capture is available', () => {
    render(CertificatePanel, {
      props: {
        open: true,
        caStatus: {
          generated: true,
          certificatePath: '/tmp/netcapture-ca.crt',
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
    expect(screen.getByText('http://192.168.1.24:8888/__netcapture/ca.crt')).toBeVisible()
  })
})
