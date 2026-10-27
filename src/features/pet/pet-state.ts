import type { ProxyState } from '../../shared/types'

export type PetMode = 'idle' | 'listening' | 'request' | 'error'

export interface PetSnapshot {
  mode: PetMode
  label: '待机中' | '正在监听' | '捕获到请求' | '需要关注'
  frame: number
}

interface PetStateMachineOptions {
  reducedMotion?: boolean
}

const REQUEST_FRAME_COUNT = 16
const REQUEST_FRAME_DURATION_MS = 80
const TRANSIENT_ERROR_DURATION_MS = 4_000

export function createPetStateMachine(options: PetStateMachineOptions = {}) {
  let proxyState: ProxyState = 'stopped'
  let reducedMotion = options.reducedMotion ?? false
  let requestActive = false
  let transientErrorActive = false
  let requestFrame = 0
  let requestTimer: ReturnType<typeof setInterval> | null = null
  let requestEndTimer: ReturnType<typeof setTimeout> | null = null
  let errorTimer: ReturnType<typeof setTimeout> | null = null
  const listeners = new Set<(snapshot: PetSnapshot) => void>()

  function snapshot(): PetSnapshot {
    if (proxyState === 'error' || transientErrorActive) {
      return { mode: 'error', label: '需要关注', frame: 15 }
    }
    if (requestActive) {
      return { mode: 'request', label: '捕获到请求', frame: requestFrame }
    }
    if (proxyState === 'running') {
      return { mode: 'listening', label: '正在监听', frame: 4 }
    }
    return { mode: 'idle', label: '待机中', frame: 0 }
  }

  function notify() {
    const current = snapshot()
    listeners.forEach((listener) => listener(current))
  }

  function clearRequest() {
    if (requestTimer !== null) clearInterval(requestTimer)
    if (requestEndTimer !== null) clearTimeout(requestEndTimer)
    requestTimer = null
    requestEndTimer = null
    requestActive = false
    requestFrame = 0
  }

  function finishRequest() {
    clearRequest()
    notify()
  }

  function setProxyState(nextState: ProxyState) {
    proxyState = nextState
    if (nextState === 'error') clearRequest()
    notify()
  }

  function triggerRequest() {
    if (requestActive || proxyState === 'error') return
    requestActive = true
    requestFrame = reducedMotion ? 9 : 0
    if (!reducedMotion) {
      requestTimer = setInterval(() => {
        if (requestFrame < REQUEST_FRAME_COUNT - 1) {
          requestFrame += 1
          notify()
        }
      }, REQUEST_FRAME_DURATION_MS)
    }
    requestEndTimer = setTimeout(finishRequest, REQUEST_FRAME_COUNT * REQUEST_FRAME_DURATION_MS)
    notify()
  }

  function triggerTransientError() {
    clearRequest()
    transientErrorActive = true
    if (errorTimer !== null) clearTimeout(errorTimer)
    errorTimer = setTimeout(() => {
      transientErrorActive = false
      errorTimer = null
      notify()
    }, TRANSIENT_ERROR_DURATION_MS)
    notify()
  }

  function setReducedMotion(nextValue: boolean) {
    reducedMotion = nextValue
    if (requestActive && nextValue) {
      if (requestTimer !== null) clearInterval(requestTimer)
      requestTimer = null
      requestFrame = 9
      notify()
    }
  }

  function subscribe(listener: (snapshot: PetSnapshot) => void) {
    listeners.add(listener)
    listener(snapshot())
    return () => listeners.delete(listener)
  }

  function dispose() {
    clearRequest()
    if (errorTimer !== null) clearTimeout(errorTimer)
    errorTimer = null
    transientErrorActive = false
    listeners.clear()
  }

  return {
    snapshot,
    setProxyState,
    triggerRequest,
    triggerTransientError,
    setReducedMotion,
    subscribe,
    dispose
  }
}
