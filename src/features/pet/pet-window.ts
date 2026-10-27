import { PhysicalPosition } from '@tauri-apps/api/dpi'
import {
  availableMonitors,
  currentMonitor,
  getCurrentWindow,
  primaryMonitor,
  type Monitor
} from '@tauri-apps/api/window'
import type { InjectionKey } from 'vue'
import type { PetPlacement } from '../../shared/types'
import {
  calculateSnapTarget,
  restorePetPosition,
  type PetMonitor,
  type PhysicalPoint
} from './pet-placement'

const EDGE_MARGIN_LOGICAL = 12
const DRAG_SETTLE_MS = 120
const SNAP_DURATION_MS = 140

export interface PetWindowRuntime {
  restorePosition(placement: PetPlacement | null): Promise<void>
  restoreAndShow(placement: PetPlacement | null, reducedMotion: boolean): Promise<void>
  dragAndSnap(reducedMotion: boolean): Promise<PetPlacement>
}

export const petWindowRuntimeKey: InjectionKey<PetWindowRuntime> = Symbol('pet-window-runtime')

function toPetMonitor(monitor: Monitor, primaryName: string | null): PetMonitor {
  return {
    name: monitor.name,
    scaleFactor: monitor.scaleFactor,
    primary: monitor.name === primaryName,
    workArea: {
      position: { x: monitor.workArea.position.x, y: monitor.workArea.position.y },
      size: { width: monitor.workArea.size.width, height: monitor.workArea.size.height }
    }
  }
}

async function setPosition(point: PhysicalPoint): Promise<void> {
  await getCurrentWindow().setPosition(new PhysicalPosition(Math.round(point.x), Math.round(point.y)))
}

async function animatePosition(from: PhysicalPoint, to: PhysicalPoint): Promise<void> {
  const startedAt = performance.now()
  await new Promise<void>((resolve) => {
    const step = async (time: number) => {
      const progress = Math.min((time - startedAt) / SNAP_DURATION_MS, 1)
      const eased = 1 - (1 - progress) ** 3
      await setPosition({
        x: from.x + (to.x - from.x) * eased,
        y: from.y + (to.y - from.y) * eased
      })
      if (progress < 1) {
        requestAnimationFrame(step)
      } else {
        resolve()
      }
    }
    requestAnimationFrame(step)
  })
}

async function readMonitors(): Promise<PetMonitor[]> {
  const [monitors, primary] = await Promise.all([availableMonitors(), primaryMonitor()])
  return monitors.map((monitor) => toPetMonitor(monitor, primary?.name ?? null))
}

async function restorePosition(placement: PetPlacement | null): Promise<void> {
  const appWindow = getCurrentWindow()
  const [monitors, windowSize] = await Promise.all([readMonitors(), appWindow.outerSize()])
  const position = restorePetPosition({
    placement,
    monitors,
    windowSize,
    marginLogical: EDGE_MARGIN_LOGICAL
  })
  await setPosition(position)
}

async function waitForDragToSettle(): Promise<void> {
  const appWindow = getCurrentWindow()
  await new Promise<void>(async (resolve, reject) => {
    let timer: number | null = null
    let unlisten: (() => void) | null = null
    const finish = () => {
      if (timer !== null) window.clearTimeout(timer)
      unlisten?.()
      resolve()
    }
    const schedule = () => {
      if (timer !== null) window.clearTimeout(timer)
      timer = window.setTimeout(finish, DRAG_SETTLE_MS)
    }
    try {
      unlisten = await appWindow.onMoved(schedule)
      await appWindow.startDragging()
      schedule()
    } catch (error) {
      if (timer !== null) window.clearTimeout(timer)
      unlisten?.()
      reject(error)
    }
  })
}

export const petWindowRuntime: PetWindowRuntime = {
  restorePosition,

  async restoreAndShow(placement, _reducedMotion) {
    const appWindow = getCurrentWindow()
    await restorePosition(placement)
    await appWindow.show()
  },

  async dragAndSnap(reducedMotion) {
    const appWindow = getCurrentWindow()
    await waitForDragToSettle()

    const [tauriMonitor, fallbackMonitor, primary, position, windowSize] = await Promise.all([
      currentMonitor(),
      availableMonitors().then((monitors) => monitors[0] ?? null),
      primaryMonitor(),
      appWindow.outerPosition(),
      appWindow.outerSize()
    ])
    const selectedMonitor = tauriMonitor ?? fallbackMonitor ?? primary
    if (!selectedMonitor) throw new Error('未找到可用显示器')
    const monitor = toPetMonitor(selectedMonitor, primary?.name ?? null)
    const target = calculateSnapTarget({
      monitor,
      windowPosition: position,
      windowSize,
      marginLogical: EDGE_MARGIN_LOGICAL
    })

    if (reducedMotion) {
      await setPosition(target.position)
    } else {
      await animatePosition(position, target.position)
    }
    return {
      monitorName: target.monitorName,
      edge: target.edge,
      offset: target.offset
    }
  }
}
