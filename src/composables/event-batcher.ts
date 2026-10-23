export function createEventBatcher(callback: () => void | Promise<void>, delayMs: number) {
  let timer: ReturnType<typeof setTimeout> | null = null

  function schedule() {
    if (timer !== null) {
      return
    }
    timer = setTimeout(() => {
      timer = null
      void callback()
    }, delayMs)
  }

  function cancel() {
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
  }

  return { schedule, cancel }
}
