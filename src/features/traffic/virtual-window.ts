export interface VirtualWindowInput {
  itemCount: number
  rowHeight: number
  viewportHeight: number
  scrollTop: number
  overscan: number
}

export interface VirtualWindow {
  start: number
  end: number
  before: number
  after: number
}

export function calculateVirtualWindow(input: VirtualWindowInput): VirtualWindow {
  const firstVisible = Math.floor(Math.max(0, input.scrollTop) / input.rowHeight)
  const visibleCount = Math.ceil(input.viewportHeight / input.rowHeight)
  const start = Math.max(0, firstVisible - input.overscan)
  const end = Math.min(input.itemCount, firstVisible + visibleCount + input.overscan)

  return {
    start,
    end,
    before: start * input.rowHeight,
    after: Math.max(0, input.itemCount - end) * input.rowHeight
  }
}
