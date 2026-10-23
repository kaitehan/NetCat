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

    const icon = screen.getByRole('img', { name: 'NetCapture 应用图标' })
    expect(icon.getAttribute('src')).toMatch(/icon\.png$/)
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
})
