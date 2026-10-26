import { fireEvent, render, screen, within } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import NetworkProfileMenu from './NetworkProfileMenu.vue'
import type { NetworkProfile } from '../../shared/types'

const profiles: NetworkProfile[] = [
  { id: 'fast-4g', name: '高速 4G', kind: 'preset', latencyMs: 20, jitterMs: 0, downloadKbps: 9000, uploadKbps: 1500, timeoutPercent: 0, disconnectPercent: 0, methods: [], matchKind: 'contains', urlPattern: '', builtin: true, createdAt: '' },
  { id: 'slow-4g', name: '低速 4G', kind: 'preset', latencyMs: 150, jitterMs: 0, downloadKbps: 1600, uploadKbps: 750, timeoutPercent: 0, disconnectPercent: 0, methods: [], matchKind: 'contains', urlPattern: '', builtin: true, createdAt: '' },
  { id: 'office', name: '办公室 Wi-Fi', kind: 'custom', latencyMs: 80, jitterMs: 10, downloadKbps: 4000, uploadKbps: 1000, timeoutPercent: 0, disconnectPercent: 0, methods: [], matchKind: 'contains', urlPattern: '', builtin: false, createdAt: '' }
]

describe('NetworkProfileMenu', () => {
  it('groups presets and custom profiles and activates a selection', async () => {
    const view = render(NetworkProfileMenu, { props: { profiles, activeId: 'fast-4g' } })
    await fireEvent.click(screen.getByRole('button', { name: '限流配置：高速 4G' }))

    const menu = screen.getByRole('menu', { name: '限流配置' })
    expect(within(menu).getByText('预设')).toBeVisible()
    expect(within(menu).getByText('自定义')).toBeVisible()
    expect(within(menu).getByRole('menuitemradio', { name: /高速 4G/ })).toHaveAttribute('aria-checked', 'true')
    expect(within(menu).getByRole('menuitemradio', { name: /办公室 Wi-Fi/ })).toBeVisible()

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: /低速 4G/ }))
    expect(view.emitted().activate).toEqual([['slow-4g']])
  })

  it('can disable throttling and request a custom profile', async () => {
    const view = render(NetworkProfileMenu, { props: { profiles, activeId: null } })
    await fireEvent.click(screen.getByRole('button', { name: '限流配置：无限流' }))
    await fireEvent.click(screen.getByRole('menuitemradio', { name: '无限流' }))
    expect(view.emitted().activate).toEqual([[null]])

    await fireEvent.click(screen.getByRole('button', { name: '限流配置：无限流' }))
    await fireEvent.click(screen.getByRole('menuitem', { name: '添加自定义配置' }))
    expect(view.emitted().add).toHaveLength(1)
  })

  it('closes the menu when the user clicks outside it', async () => {
    render(NetworkProfileMenu, { props: { profiles, activeId: null } })
    const trigger = screen.getByRole('button', { name: '限流配置：无限流' })

    await fireEvent.click(trigger)
    expect(screen.getByRole('menu', { name: '限流配置' })).toBeVisible()

    await fireEvent.pointerDown(document.body)

    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('menu', { name: '限流配置' })).not.toBeInTheDocument()
  })
})
