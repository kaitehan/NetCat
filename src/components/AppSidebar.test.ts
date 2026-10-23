import { render, screen } from '@testing-library/vue'
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
  })
})
