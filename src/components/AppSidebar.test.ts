import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import AppSidebar from './AppSidebar.vue'

describe('AppSidebar', () => {
  it('shows the PNG application icon in the brand area', () => {
    render(AppSidebar, {
      props: {
        activeView: 'traffic',
        trafficCount: 4,
        mockCount: 1
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
        mockCount: 1
      }
    })

    await fireEvent.click(screen.getByRole('button', { name: '设置' }))

    expect(emitted().changeView).toEqual([['settings']])
  })

  it('opens scripts without exposing SSL Pinning navigation', async () => {
    const { emitted } = render(AppSidebar, {
      props: {
        activeView: 'traffic',
        trafficCount: 4,
        mockCount: 1
      }
    })

    await fireEvent.click(screen.getByRole('button', { name: '脚本' }))
    expect(screen.queryByRole('button', { name: 'SSL Pinning' })).not.toBeInTheDocument()

    expect(emitted().changeView).toEqual([['scripts']])
  })

  it('opens the low-priority About page below settings without a proxy status row', async () => {
    const { emitted } = render(AppSidebar, {
      props: {
        activeView: 'about',
        trafficCount: 4,
        mockCount: 1
      }
    })

    const settings = screen.getByRole('button', { name: '设置' })
    const about = screen.getByRole('button', { name: '关于' })

    expect(settings.compareDocumentPosition(about) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(about).toHaveClass('active')
    expect(screen.queryByLabelText('代理状态')).not.toBeInTheDocument()
    expect(screen.queryByText('就绪')).not.toBeInTheDocument()

    await fireEvent.click(about)
    expect(emitted().changeView).toEqual([['about']])
  })

  it('keeps settings above the About action', () => {
    render(AppSidebar, {
      props: {
        activeView: 'settings',
        trafficCount: 4,
        mockCount: 1
      }
    })

    const settings = screen.getByRole('button', { name: '设置' })
    const about = screen.getByRole('button', { name: '关于' })

    expect(settings.compareDocumentPosition(about) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })
})
