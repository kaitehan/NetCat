import { afterEach, describe, expect, it, vi } from 'vitest'
import { createEventBatcher } from './event-batcher'

describe('createEventBatcher', () => {
  afterEach(() => vi.useRealTimers())

  it('coalesces a burst of events into one callback', () => {
    vi.useFakeTimers()
    const callback = vi.fn()
    const batcher = createEventBatcher(callback, 75)

    batcher.schedule()
    batcher.schedule()
    batcher.schedule()
    vi.advanceTimersByTime(74)
    expect(callback).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(callback).toHaveBeenCalledTimes(1)
  })

  it('delivers the latest value for every id in one batch', () => {
    vi.useFakeTimers()
    const callback = vi.fn()
    const batcher = createEventBatcher<{ id: string; status: number | null }>(callback, 75, (item) => item.id)

    batcher.schedule({ id: 'a', status: null })
    batcher.schedule({ id: 'b', status: 200 })
    batcher.schedule({ id: 'a', status: 201 })
    vi.advanceTimersByTime(75)

    expect(callback).toHaveBeenCalledWith([
      { id: 'b', status: 200 },
      { id: 'a', status: 201 }
    ])
  })
})
