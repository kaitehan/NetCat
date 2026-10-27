import { fireEvent, render, screen } from '@testing-library/vue'
import { nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import SupportLauncher from './SupportLauncher.vue'

const greetings = [
  '你好，我是小橘，有什么可以帮你？',
  '遇到抓包问题了吗？小橘来帮你看看。',
  '代理或证书配置不顺利？可以问问小橘。',
  'Mock 没有按预期生效？我们一起排查一下。',
  '网络调试卡住了？小橘随时在这里。'
]

function setReducedMotion(reduced: boolean) {
  vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
    matches: reduced,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn()
  }))
}

function setChangeableMotionPreference() {
  let changeHandler: ((event: MediaQueryListEvent) => void) | undefined
  vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
    matches: false,
    addEventListener: vi.fn((_event: string, handler: (event: MediaQueryListEvent) => void) => {
      changeHandler = handler
    }),
    removeEventListener: vi.fn()
  }))
  return (matches: boolean) => changeHandler?.({ matches } as MediaQueryListEvent)
}

describe('SupportLauncher', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('reveals a random greeting with a typewriter effect after the initial delay', async () => {
    vi.useFakeTimers()
    vi.spyOn(Math, 'random').mockReturnValue(0)
    const { emitted } = render(SupportLauncher)

    expect(screen.queryByRole('button', { name: /打开智奈助手：/ })).not.toBeInTheDocument()
    await vi.advanceTimersByTimeAsync(1_500)
    expect(screen.getByTestId('support-greeting-text')).toHaveTextContent('你')

    await vi.advanceTimersByTimeAsync(45 * greetings[0].length)
    expect(screen.getByTestId('support-greeting-text')).toHaveTextContent(greetings[0])
    expect(screen.getByRole('status', { name: '智奈助手问候' })).toHaveTextContent(greetings[0])
    expect(emitted('open')).toBeUndefined()
  })

  it('opens the assistant when the greeting or launcher is clicked', async () => {
    vi.useFakeTimers()
    vi.spyOn(Math, 'random').mockReturnValue(0)
    const { emitted } = render(SupportLauncher)
    await vi.advanceTimersByTimeAsync(1_500 + 45 * greetings[0].length)

    await fireEvent.click(screen.getByRole('button', { name: `打开智奈助手：${greetings[0]}` }))
    expect(emitted('open')).toHaveLength(1)

    await fireEvent.click(screen.getByRole('button', { name: '打开智奈助手' }))
    expect(emitted('open')).toHaveLength(2)
  })

  it('shows the full greeting immediately when reduced motion is enabled', async () => {
    vi.useFakeTimers()
    setReducedMotion(true)
    vi.spyOn(Math, 'random').mockReturnValue(0)
    render(SupportLauncher)

    await vi.advanceTimersByTimeAsync(1_500)
    expect(screen.getByTestId('support-greeting-text')).toHaveTextContent(greetings[0])
  })

  it('finishes the current greeting when reduced motion is enabled while typing', async () => {
    vi.useFakeTimers()
    const changeMotionPreference = setChangeableMotionPreference()
    vi.spyOn(Math, 'random').mockReturnValue(0)
    render(SupportLauncher)
    await vi.advanceTimersByTimeAsync(1_500)
    expect(screen.getByTestId('support-greeting-text')).toHaveTextContent('你')

    changeMotionPreference(true)
    await nextTick()
    expect(screen.getByTestId('support-greeting-text')).toHaveTextContent(greetings[0])
    expect(screen.getByRole('status', { name: '智奈助手问候' })).toHaveTextContent(greetings[0])
  })

  it('does not repeat the same greeting in consecutive cycles', async () => {
    vi.useFakeTimers()
    vi.spyOn(Math, 'random')
      .mockReturnValueOnce(0)
      .mockReturnValueOnce(0)
      .mockReturnValueOnce(0.25)
    setReducedMotion(true)
    render(SupportLauncher)

    await vi.advanceTimersByTimeAsync(1_500)
    expect(screen.getByTestId('support-greeting-text')).toHaveTextContent(greetings[0])
    await vi.advanceTimersByTimeAsync(4_000 + 20_000)
    expect(screen.getByTestId('support-greeting-text')).toHaveTextContent(greetings[1])
  })

  it('clears all scheduled work when unmounted', async () => {
    vi.useFakeTimers()
    vi.spyOn(Math, 'random').mockReturnValue(0)
    const { unmount } = render(SupportLauncher)
    await vi.advanceTimersByTimeAsync(1_500)

    unmount()
    expect(vi.getTimerCount()).toBe(0)
  })
})
