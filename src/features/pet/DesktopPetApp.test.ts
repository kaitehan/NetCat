import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { desktopApi, desktopApiKey, type DesktopApi } from '../../api/desktop'
import type { PetPlacement, ProxyStatus } from '../../shared/types'
import DesktopPetApp from './DesktopPetApp.vue'
import { petWindowRuntimeKey, type PetWindowRuntime } from './pet-window'

const stoppedStatus: ProxyStatus = {
  state: 'stopped',
  mode: 'computer',
  bindAddress: null,
  port: 28086,
  lanIp: null,
  error: null
}

function setup(enabled = true) {
  const handlers = new Map<string, (payload: unknown) => void>()
  const unlisteners: Array<ReturnType<typeof vi.fn>> = []
  const placement: PetPlacement = { monitorName: 'Built-in', edge: 'right', offset: 1 }
  const api: DesktopApi = {
    ...desktopApi,
    getProxyStatus: vi.fn().mockResolvedValue(stoppedStatus),
    getPetPreferences: vi.fn().mockResolvedValue({ version: 1, enabled, placement: null }),
    savePetPlacement: vi.fn().mockResolvedValue({ version: 1, enabled: true, placement }),
    listen: vi.fn(async (event: string, handler: (payload: unknown) => void) => {
      const unlisten = vi.fn(() => handlers.delete(event))
      unlisteners.push(unlisten)
      handlers.set(event, handler)
      return unlisten
    })
  }
  const runtime: PetWindowRuntime = {
    restorePosition: vi.fn().mockResolvedValue(undefined),
    restoreAndShow: vi.fn().mockResolvedValue(undefined),
    dragAndSnap: vi.fn().mockResolvedValue(placement)
  }
  const view = render(DesktopPetApp, {
    global: {
      provide: {
        [desktopApiKey as symbol]: api,
        [petWindowRuntimeKey as symbol]: runtime
      }
    }
  })
  return { ...view, api, runtime, handlers, unlisteners, placement }
}

describe('DesktopPetApp', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('shows live proxy, request, and error states from desktop events', async () => {
    vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }))
    const { handlers, runtime } = setup()

    expect(await screen.findByText('待机中')).toBeVisible()
    await waitFor(() => expect(runtime.restoreAndShow).toHaveBeenCalledWith(null, false))

    handlers.get('proxy-status-changed')?.({ ...stoppedStatus, state: 'running' })
    expect(await screen.findByText('正在监听')).toBeVisible()

    handlers.get('traffic-created')?.({ id: 'request-1' })
    expect(await screen.findByText('捕获到请求')).toBeVisible()

    handlers.get('diagnostic-log')?.({ level: 'error' })
    expect(await screen.findByText('需要关注')).toBeVisible()
  })

  it('starts native dragging from the cat and persists the snapped placement', async () => {
    vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }))
    const { api, runtime, placement } = setup()
    const cat = await screen.findByTestId('desktop-pet-drag-handle')

    await fireEvent.pointerDown(cat, { button: 0 })

    await waitFor(() => expect(runtime.dragAndSnap).toHaveBeenCalledWith(false))
    expect(api.savePetPlacement).toHaveBeenCalledWith(placement)
  })

  it('restores position without showing when the preference is disabled', async () => {
    vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }))
    const { runtime } = setup(false)

    await waitFor(() => expect(runtime.restorePosition).toHaveBeenCalledWith(null))
    expect(runtime.restoreAndShow).not.toHaveBeenCalled()
  })

  it('uses a static fallback if the sprite sheet cannot load', async () => {
    vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }))
    setup()

    await fireEvent.error(await screen.findByTestId('desktop-pet-sprite-source'))

    expect(screen.getByRole('img', { name: '小橘桌面助手' })).toBeVisible()
  })

  it('draws request frames from the sprite sheet from left to right', async () => {
    const drawImage = vi.fn()
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
      clearRect: vi.fn(),
      drawImage
    } as unknown as CanvasRenderingContext2D)
    vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }))
    const { handlers } = setup()
    const source = await screen.findByTestId('desktop-pet-sprite-source')
    Object.defineProperties(source, {
      complete: { configurable: true, value: true },
      naturalWidth: { configurable: true, value: 1536 }
    })
    await fireEvent.load(source)

    handlers.get('traffic-created')?.({ id: 'request-1' })
    await waitFor(() => expect(drawImage).toHaveBeenLastCalledWith(
      source,
      0,
      0,
      384,
      384,
      0,
      0,
      180,
      180
    ))
    await new Promise((resolve) => window.setTimeout(resolve, 90))
    await waitFor(() => expect(drawImage).toHaveBeenLastCalledWith(
      source,
      384,
      0,
      384,
      384,
      0,
      0,
      180,
      180
    ))
  })

  it('unsubscribes from desktop events when unmounted', async () => {
    vi.stubGlobal('matchMedia', vi.fn().mockReturnValue({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }))
    const { unmount, unlisteners } = setup()
    await screen.findByText('待机中')
    await waitFor(() => expect(unlisteners.length).toBe(4))

    unmount()

    expect(unlisteners.every((unlisten) => unlisten.mock.calls.length === 1)).toBe(true)
  })
})
