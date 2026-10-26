import { render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import AboutPage from './AboutPage.vue'

describe('AboutPage', () => {
  it('shows the NetCat identity and release details', () => {
    render(AboutPage)

    expect(screen.getByRole('heading', { name: '关于 NetCat' })).toBeVisible()
    expect(screen.getByText('奈特猫 · 本地守候每一次请求')).toBeVisible()
    expect(screen.getByText(/NetCat（奈特猫）是一款面向开发者/)).toBeVisible()
    expect(screen.getByText('0.5.0')).toBeVisible()
    expect(screen.getByText('kaite han')).toBeVisible()
  })
})
