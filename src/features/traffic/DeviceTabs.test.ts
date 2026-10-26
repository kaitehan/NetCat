import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import DeviceTabs from './DeviceTabs.vue'

describe('DeviceTabs', () => {
  it('renders all, local and ordered device scopes and emits the selected scope', async () => {
    const { emitted } = render(DeviceTabs, {
      props: {
        modelValue: 'all',
        devices: [
          { token: 'online', name: '测试手机', online: true, count: 2 },
          { token: 'history', name: '历史手机', online: false, count: 1 }
        ],
        totalCount: 4,
        localCount: 1
      }
    })

    expect(screen.getByRole('tab', { name: '全部 4' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('tab', { name: '本机 1' })).toBeVisible()
    expect(screen.getByRole('tab', { name: '测试手机 2 在线' })).toBeVisible()
    await fireEvent.click(screen.getByRole('tab', { name: '历史手机 1 已断开' }))
    expect(emitted()['update:modelValue']).toEqual([['device:history']])
  })
})
