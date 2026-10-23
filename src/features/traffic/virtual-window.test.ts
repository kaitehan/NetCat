import { describe, expect, it } from 'vitest'
import { calculateVirtualWindow } from './virtual-window'

describe('calculateVirtualWindow', () => {
  it('returns an overscanned stable slice and spacer offsets', () => {
    expect(calculateVirtualWindow({
      itemCount: 1000,
      rowHeight: 36,
      viewportHeight: 360,
      scrollTop: 3600,
      overscan: 3
    })).toEqual({
      start: 97,
      end: 113,
      before: 3492,
      after: 31932
    })
  })
})
