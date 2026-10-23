import { fireEvent, render, screen } from '@testing-library/vue'
import { afterEach, describe, expect, it } from 'vitest'
import FilterSelect from './FilterSelect.vue'

const options = [
  { value: 'ALL', label: '全部方法' },
  { value: 'GET', label: 'GET' },
  { value: 'POST', label: 'POST' }
]

afterEach(() => {
  document.body.innerHTML = ''
})

describe('FilterSelect', () => {
  it('opens the themed listbox and emits the selected value', async () => {
    const { emitted } = render(FilterSelect, {
      props: {
        modelValue: 'ALL',
        label: '请求方法',
        options
      }
    })

    const trigger = screen.getByRole('button', { name: '请求方法：全部方法' })
    await fireEvent.click(trigger)

    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('listbox', { name: '请求方法选项' })).toBeVisible()
    expect(screen.getByRole('option', { name: '全部方法' })).toHaveAttribute('aria-selected', 'true')

    await fireEvent.click(screen.getByRole('option', { name: 'POST' }))

    expect(emitted()['update:modelValue']).toEqual([['POST']])
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('supports arrow navigation and Enter selection', async () => {
    const { emitted } = render(FilterSelect, {
      props: {
        modelValue: 'ALL',
        label: '请求方法',
        options
      }
    })

    const trigger = screen.getByRole('button', { name: '请求方法：全部方法' })
    trigger.focus()
    await fireEvent.keyDown(trigger, { key: 'ArrowDown' })

    expect(screen.getByRole('option', { name: 'GET' })).toHaveAttribute('data-active', 'true')

    await fireEvent.keyDown(trigger, { key: 'Enter' })

    expect(emitted()['update:modelValue']).toEqual([['GET']])
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('closes without changing the value on Escape and outside click', async () => {
    const { emitted } = render(FilterSelect, {
      props: {
        modelValue: 'ALL',
        label: '请求方法',
        options
      }
    })

    const trigger = screen.getByRole('button', { name: '请求方法：全部方法' })
    await fireEvent.click(trigger)
    await fireEvent.keyDown(trigger, { key: 'Escape' })
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()

    await fireEvent.click(trigger)
    await fireEvent.pointerDown(document.body)

    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(emitted()['update:modelValue']).toBeUndefined()
  })
})
