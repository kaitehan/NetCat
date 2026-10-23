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
})
