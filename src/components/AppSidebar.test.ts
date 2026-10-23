import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import AppSidebar from './AppSidebar.vue'

const stoppedStatus = {
  state: 'stopped' as const,
  bindAddress: null,
  port: 28086,
  lanIp: null,
  error: null
}

describe('AppSidebar', () => {
  it('shows the PNG application icon in the brand area', () => {
    render(AppSidebar, {
      props: {
        activeView: 'traffic',
        trafficCount: 4,
        mockCount: 1,
        proxyStatus: stoppedStatus
      }
    })

    const icon = screen.getByRole('img', { name: 'NetCat 应用图标' })
    expect(icon.getAttribute('src')).toMatch(/icon\.png$/)
    expect(screen.getByText('NetCat')).toBeVisible()
    expect(screen.getByText('奈特猫 · LOCAL PROXY')).toBeVisible()
    expect(screen.getByRole('button', { name: '网络请求' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '流量' })).not.toBeInTheDocument()
  })

  it('opens settings from the primary navigation', async () => {
    const { emitted } = render(AppSidebar, {
      props: {
        activeView: 'traffic',
        trafficCount: 4,
        mockCount: 1,
        proxyStatus: stoppedStatus
      }
    })

    await fireEvent.click(screen.getByRole('button', { name: '设置' }))

    expect(emitted().changeView).toEqual([['settings']])
  })

  it('shows proxy readiness below the settings action', () => {
    render(AppSidebar, {
      props: {
        activeView: 'settings',
        trafficCount: 4,
        mockCount: 1,
        proxyStatus: stoppedStatus
      }
    })

    const settings = screen.getByRole('button', { name: '设置' })
    const status = screen.getByText('就绪')

    expect(settings.compareDocumentPosition(status) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(status.closest('.sidebar-status')).not.toBeNull()
  })
})
