import { afterEach, describe, expect, it, vi } from 'vitest'
import { createPetStateMachine } from './pet-state'

describe('pet state machine', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('maps stopped and running proxy states to idle and listening', () => {
    const machine = createPetStateMachine()

    expect(machine.snapshot()).toMatchObject({ mode: 'idle', label: '待机中', frame: 0 })

    machine.setProxyState('running')
    expect(machine.snapshot()).toMatchObject({ mode: 'listening', label: '正在监听', frame: 4 })
  })

  it('plays all request frames once without restarting for traffic bursts', () => {
    vi.useFakeTimers()
    const machine = createPetStateMachine()
    machine.setProxyState('running')

    machine.triggerRequest()
    expect(machine.snapshot()).toMatchObject({ mode: 'request', label: '捕获到请求', frame: 0 })

    vi.advanceTimersByTime(400)
    expect(machine.snapshot().frame).toBe(5)
    machine.triggerRequest()

    vi.advanceTimersByTime(800)
    expect(machine.snapshot().frame).toBe(15)
    vi.advanceTimersByTime(80)
    expect(machine.snapshot()).toMatchObject({ mode: 'listening', label: '正在监听', frame: 4 })
  })

  it('gives errors priority and restores the underlying proxy state', () => {
    vi.useFakeTimers()
    const machine = createPetStateMachine()
    machine.setProxyState('running')
    machine.triggerRequest()

    machine.triggerTransientError()
    expect(machine.snapshot()).toMatchObject({ mode: 'error', label: '需要关注', frame: 15 })

    vi.advanceTimersByTime(4_000)
    expect(machine.snapshot()).toMatchObject({ mode: 'listening', label: '正在监听', frame: 4 })

    machine.setProxyState('error')
    vi.advanceTimersByTime(10_000)
    expect(machine.snapshot().mode).toBe('error')
  })

  it('uses a representative frame instead of rapid animation for reduced motion', () => {
    vi.useFakeTimers()
    const machine = createPetStateMachine({ reducedMotion: true })
    machine.setProxyState('running')

    machine.triggerRequest()
    expect(machine.snapshot()).toMatchObject({ mode: 'request', frame: 9 })

    vi.advanceTimersByTime(1_280)
    expect(machine.snapshot()).toMatchObject({ mode: 'listening', frame: 4 })
  })

  it('clears scheduled work when disposed', () => {
    vi.useFakeTimers()
    const machine = createPetStateMachine()
    machine.triggerRequest()
    machine.triggerTransientError()

    machine.dispose()

    expect(vi.getTimerCount()).toBe(0)
  })
})
