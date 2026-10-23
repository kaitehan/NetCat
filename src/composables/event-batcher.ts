export function createEventBatcher(callback: () => void | Promise<void>, delayMs: number): {
  schedule: () => void
  cancel: () => void
}
export function createEventBatcher<T>(
  callback: (items: readonly T[]) => void | Promise<void>,
  delayMs: number,
  key: (item: T) => string
): {
  schedule: (item: T) => void
  cancel: () => void
}
export function createEventBatcher<T>(
  callback: (() => void | Promise<void>) | ((items: readonly T[]) => void | Promise<void>),
  delayMs: number,
  key?: (item: T) => string
) {
  let timer: ReturnType<typeof setTimeout> | null = null
  const pending = new Map<string, T>()

  function schedule(item?: T) {
    if (key && item !== undefined) {
      const itemKey = key(item)
      pending.delete(itemKey)
      pending.set(itemKey, item)
    }
    if (timer !== null) {
      return
    }
    timer = setTimeout(() => {
      timer = null
      if (key) {
        const items = [...pending.values()]
        pending.clear()
        void (callback as (items: readonly T[]) => void | Promise<void>)(items)
      } else {
        void (callback as () => void | Promise<void>)()
      }
    }, delayMs)
  }

  function cancel() {
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
    pending.clear()
  }

  return { schedule, cancel }
}
