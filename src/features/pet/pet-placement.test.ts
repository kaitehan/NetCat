import { describe, expect, it } from 'vitest'
import {
  calculateSnapTarget,
  restorePetPosition,
  type PetMonitor,
  type PetPlacement
} from './pet-placement'

const primaryMonitor: PetMonitor = {
  name: 'Built-in',
  scaleFactor: 2,
  primary: true,
  workArea: {
    position: { x: 0, y: 0 },
    size: { width: 2880, height: 1800 }
  }
}

describe('pet placement', () => {
  it('snaps to the nearest of all four work-area edges', () => {
    const result = calculateSnapTarget({
      monitor: primaryMonitor,
      windowPosition: { x: 1_100, y: 40 },
      windowSize: { width: 440, height: 440 },
      marginLogical: 12
    })

    expect(result.edge).toBe('top')
    expect(result.position).toEqual({ x: 1_100, y: 24 })
    expect(result.offset).toBeCloseTo(1_076 / 2_392)
  })

  it('supports monitors with negative coordinates', () => {
    const monitor: PetMonitor = {
      name: 'Left display',
      scaleFactor: 1,
      primary: false,
      workArea: {
        position: { x: -1920, y: 0 },
        size: { width: 1920, height: 1040 }
      }
    }

    const result = calculateSnapTarget({
      monitor,
      windowPosition: { x: -1900, y: 400 },
      windowSize: { width: 220, height: 220 },
      marginLogical: 12
    })

    expect(result).toMatchObject({
      monitorName: 'Left display',
      edge: 'left',
      position: { x: -1908, y: 400 }
    })
  })

  it('restores a relative offset on a resized monitor', () => {
    const placement: PetPlacement = {
      monitorName: 'Built-in',
      edge: 'right',
      offset: 0.5
    }

    expect(restorePetPosition({
      placement,
      monitors: [primaryMonitor],
      windowSize: { width: 440, height: 440 },
      marginLogical: 12
    })).toEqual({ x: 2_416, y: 680 })
  })

  it('falls back to the primary monitor bottom-right when the saved monitor is gone', () => {
    expect(restorePetPosition({
      placement: { monitorName: 'Missing', edge: 'left', offset: 0.1 },
      monitors: [primaryMonitor],
      windowSize: { width: 440, height: 440 },
      marginLogical: 12
    })).toEqual({ x: 2_416, y: 1_336 })
  })

  it('clamps malformed offsets into the available work area', () => {
    expect(restorePetPosition({
      placement: { monitorName: 'Built-in', edge: 'bottom', offset: 9 },
      monitors: [primaryMonitor],
      windowSize: { width: 440, height: 440 },
      marginLogical: 12
    })).toEqual({ x: 2_416, y: 1_336 })
  })
})
