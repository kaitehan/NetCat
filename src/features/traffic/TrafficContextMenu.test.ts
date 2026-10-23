import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { afterEach, describe, expect, it } from 'vitest'
import TrafficContextMenu from './TrafficContextMenu.vue'

afterEach(() => {
  document.body.innerHTML = ''
})

describe('TrafficContextMenu', () => {
  it('opens copy submenu and emits the selected copy action', async () => {
    const { emitted } = render(TrafficContextMenu, {
      props: { open: true, x: 40, y: 50 }
    })

    await fireEvent.click(screen.getByRole('menuitem', { name: '复制' }))
    await fireEvent.click(screen.getByRole('menuitem', { name: 'cURL' }))

    expect(emitted().action).toEqual([['copy:curl']])
  })

  it('supports keyboard navigation and closes on Escape', async () => {
    const { emitted } = render(TrafficContextMenu, {
      props: { open: true, x: 40, y: 50 }
    })

    await waitFor(() => expect(screen.getByRole('menuitem', { name: '复制' })).toHaveFocus())
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    expect(screen.getByRole('menuitem', { name: '屏蔽' })).toHaveFocus()
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(emitted().close).toEqual([[]])
  })

  it('disables unsupported actions and exposes the reason', async () => {
    render(TrafficContextMenu, {
      props: {
        open: true,
        x: 40,
        y: 50,
        disabledReasons: { 'mock:body': '响应正文已截断，不能创建 Mock' }
      }
    })

    const replaceBody = screen.getByRole('menuitem', { name: '替换内容' })
    expect(replaceBody).toBeDisabled()
    expect(replaceBody).toHaveAttribute('title', '响应正文已截断，不能创建 Mock')
  })
})
