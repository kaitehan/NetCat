import type { PetEdge, PetPlacement } from '../../shared/types'

export type { PetEdge, PetPlacement } from '../../shared/types'

export interface PhysicalPoint {
  x: number
  y: number
}

export interface PhysicalSize {
  width: number
  height: number
}

export interface PhysicalRect {
  position: PhysicalPoint
  size: PhysicalSize
}

export interface PetMonitor {
  name: string | null
  scaleFactor: number
  primary: boolean
  workArea: PhysicalRect
}

export interface PetSnapResult extends PetPlacement {
  position: PhysicalPoint
}

interface CalculateSnapTargetOptions {
  monitor: PetMonitor
  windowPosition: PhysicalPoint
  windowSize: PhysicalSize
  marginLogical: number
}

interface RestorePetPositionOptions {
  placement: PetPlacement | null
  monitors: readonly PetMonitor[]
  windowSize: PhysicalSize
  marginLogical: number
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum)
}

function boundsFor(monitor: PetMonitor, windowSize: PhysicalSize, marginLogical: number) {
  const margin = marginLogical * monitor.scaleFactor
  const minimumX = monitor.workArea.position.x + margin
  const minimumY = monitor.workArea.position.y + margin
  return {
    minimumX,
    minimumY,
    maximumX: Math.max(minimumX, monitor.workArea.position.x + monitor.workArea.size.width - windowSize.width - margin),
    maximumY: Math.max(minimumY, monitor.workArea.position.y + monitor.workArea.size.height - windowSize.height - margin)
  }
}

function offsetFor(value: number, minimum: number, maximum: number): number {
  if (maximum <= minimum) return 0
  return clamp((value - minimum) / (maximum - minimum), 0, 1)
}

export function calculateSnapTarget(options: CalculateSnapTargetOptions): PetSnapResult {
  const { monitor, windowPosition, windowSize, marginLogical } = options
  const bounds = boundsFor(monitor, windowSize, marginLogical)
  const clampedX = clamp(windowPosition.x, bounds.minimumX, bounds.maximumX)
  const clampedY = clamp(windowPosition.y, bounds.minimumY, bounds.maximumY)
  const distances: ReadonlyArray<{ edge: PetEdge; distance: number }> = [
    { edge: 'top', distance: Math.abs(windowPosition.y - bounds.minimumY) },
    { edge: 'right', distance: Math.abs(windowPosition.x - bounds.maximumX) },
    { edge: 'bottom', distance: Math.abs(windowPosition.y - bounds.maximumY) },
    { edge: 'left', distance: Math.abs(windowPosition.x - bounds.minimumX) }
  ]
  const edge = distances.reduce((nearest, candidate) => candidate.distance < nearest.distance ? candidate : nearest).edge
  const horizontalEdge = edge === 'top' || edge === 'bottom'
  const position = {
    x: edge === 'left' ? bounds.minimumX : edge === 'right' ? bounds.maximumX : clampedX,
    y: edge === 'top' ? bounds.minimumY : edge === 'bottom' ? bounds.maximumY : clampedY
  }
  return {
    monitorName: monitor.name,
    edge,
    offset: horizontalEdge
      ? offsetFor(position.x, bounds.minimumX, bounds.maximumX)
      : offsetFor(position.y, bounds.minimumY, bounds.maximumY),
    position
  }
}

export function restorePetPosition(options: RestorePetPositionOptions): PhysicalPoint {
  const { placement, monitors, windowSize, marginLogical } = options
  const primary = monitors.find((monitor) => monitor.primary) ?? monitors[0]
  if (!primary) return { x: 0, y: 0 }
  const savedMonitor = placement?.monitorName === null
    ? null
    : monitors.find((monitor) => monitor.name === placement?.monitorName)
  const monitor = savedMonitor ?? primary
  const validPlacement = savedMonitor && placement
    ? placement
    : { monitorName: monitor.name, edge: 'right' as const, offset: 1 }
  const bounds = boundsFor(monitor, windowSize, marginLogical)
  const offset = clamp(Number.isFinite(validPlacement.offset) ? validPlacement.offset : 1, 0, 1)
  const horizontalPosition = bounds.minimumX + (bounds.maximumX - bounds.minimumX) * offset
  const verticalPosition = bounds.minimumY + (bounds.maximumY - bounds.minimumY) * offset

  return {
    x: validPlacement.edge === 'left'
      ? bounds.minimumX
      : validPlacement.edge === 'right'
        ? bounds.maximumX
        : Math.round(horizontalPosition),
    y: validPlacement.edge === 'top'
      ? bounds.minimumY
      : validPlacement.edge === 'bottom'
        ? bounds.maximumY
        : Math.round(verticalPosition)
  }
}
