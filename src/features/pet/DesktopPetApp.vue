<script setup lang="ts">
import { inject, onBeforeUnmount, onMounted, shallowRef, watch } from 'vue'
import { desktopApi, desktopApiKey } from '../../api/desktop'
import type { DiagnosticLog, ProxyStatus } from '../../shared/types'
import spriteUrl from '../../assets/pet/xiaoju-sprite.png'
import fallbackIconUrl from '../../../src-tauri/icons/icon.png'
import { createPetStateMachine, type PetSnapshot } from './pet-state'
import { petWindowRuntime, petWindowRuntimeKey } from './pet-window'

const api = inject(desktopApiKey, desktopApi)
const windowRuntime = inject(petWindowRuntimeKey, petWindowRuntime)
const snapshot = shallowRef<PetSnapshot>({ mode: 'idle', label: '待机中', frame: 0 })
const spriteImage = shallowRef<HTMLImageElement | null>(null)
const spriteFailed = shallowRef(false)
const canvas = shallowRef<HTMLCanvasElement | null>(null)
const reducedMotionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
const reducedMotion = shallowRef(reducedMotionQuery.matches)
const machine = createPetStateMachine({ reducedMotion: reducedMotion.value })
const unlisteners: Array<() => void> = []
let disposed = false

function drawFrame(): void {
  const image = spriteImage.value
  if (!image || !image.complete || image.naturalWidth === 0) return
  const context = canvas.value?.getContext('2d')
  if (!context) return
  const frameSize = 384
  const column = snapshot.value.frame % 4
  const row = Math.floor(snapshot.value.frame / 4)
  context.clearRect(0, 0, 180, 180)
  context.drawImage(
    image,
    column * frameSize,
    row * frameSize,
    frameSize,
    frameSize,
    0,
    0,
    180,
    180
  )
}

function handleSpriteLoad(event: Event): void {
  spriteImage.value = event.currentTarget as HTMLImageElement
  spriteFailed.value = false
  drawFrame()
}

function handleSpriteError(): void {
  spriteFailed.value = true
  spriteImage.value = null
}

function handleReducedMotionChange(event: MediaQueryListEvent): void {
  reducedMotion.value = event.matches
  machine.setReducedMotion(event.matches)
}

async function startDrag(event: PointerEvent): Promise<void> {
  if (event.button !== undefined && event.button !== 0) return
  event.preventDefault()
  try {
    const placement = await windowRuntime.dragAndSnap(reducedMotion.value)
    await api.savePetPlacement(placement)
  } catch {
    machine.triggerTransientError()
  }
}

watch(snapshot, drawFrame)

onMounted(async () => {
  const unsubscribeState = machine.subscribe((nextSnapshot) => {
    snapshot.value = nextSnapshot
  })
  unlisteners.push(unsubscribeState)
  reducedMotionQuery.addEventListener('change', handleReducedMotionChange)

  try {
    const [preferences, status] = await Promise.all([
      api.getPetPreferences(),
      api.getProxyStatus()
    ])
    if (disposed) return
    machine.setProxyState(status.state)

    const eventUnlisteners = await Promise.all([
      api.listen<ProxyStatus>('proxy-status-changed', (payload) => machine.setProxyState(payload.state)),
      api.listen('traffic-created', () => machine.triggerRequest()),
      api.listen('storage-warning', () => machine.triggerTransientError()),
      api.listen<DiagnosticLog>('diagnostic-log', (payload) => {
        if (payload.level === 'error') machine.triggerTransientError()
      })
    ])
    if (disposed) {
      eventUnlisteners.forEach((unlisten) => unlisten())
      return
    }
    unlisteners.push(...eventUnlisteners)
    if (preferences.enabled) {
      await windowRuntime.restoreAndShow(preferences.placement, reducedMotion.value)
    } else {
      await windowRuntime.restorePosition(preferences.placement)
    }
  } catch {
    machine.triggerTransientError()
  }
})

onBeforeUnmount(() => {
  disposed = true
  reducedMotionQuery.removeEventListener('change', handleReducedMotionChange)
  unlisteners.splice(0).forEach((unlisten) => unlisten())
  machine.dispose()
})
</script>

<template>
  <main class="desktop-pet" :data-mode="snapshot.mode">
    <button
      class="pet-drag-handle"
      type="button"
      data-testid="desktop-pet-drag-handle"
      aria-label="拖动小橘桌面助手"
      @pointerdown="startDrag"
    >
      <canvas
        v-show="!spriteFailed"
        ref="canvas"
        class="pet-canvas"
        width="180"
        height="180"
        aria-hidden="true"
      />
      <img
        v-if="spriteFailed"
        class="pet-fallback"
        :src="fallbackIconUrl"
        alt="小橘桌面助手"
      >
    </button>
    <span class="pet-status" role="status">{{ snapshot.label }}</span>
    <img
      class="sprite-source"
      :src="spriteUrl"
      alt=""
      data-testid="desktop-pet-sprite-source"
      @load="handleSpriteLoad"
      @error="handleSpriteError"
    >
  </main>
</template>

<style scoped>
:global(html),
:global(body),
:global(#app) {
  min-width: 0;
  background: transparent;
}

.desktop-pet {
  display: flex;
  width: 220px;
  height: 220px;
  padding: 6px 20px 8px;
  align-items: center;
  flex-direction: column;
  justify-content: flex-start;
  overflow: hidden;
  background: transparent;
  user-select: none;
}

.pet-drag-handle {
  display: block;
  flex: 0 0 180px;
  width: 180px;
  height: 180px;
  padding: 0;
  overflow: hidden;
  border: 0;
  background: transparent;
  cursor: grab;
  touch-action: none;
}

.pet-drag-handle:active {
  cursor: grabbing;
}

.pet-drag-handle:focus-visible {
  outline: 2px solid var(--accent-strong);
  outline-offset: 1px;
}

.pet-canvas,
.pet-fallback {
  display: block;
  width: 180px;
  height: 180px;
  object-fit: contain;
  pointer-events: none;
}

.pet-status {
  max-width: 172px;
  min-height: 22px;
  padding: 3px 9px;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-translucent);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  line-height: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: var(--shadow-surface);
}

.desktop-pet[data-mode='request'] .pet-status {
  border-color: var(--accent);
  color: var(--accent-strong);
}

.desktop-pet[data-mode='error'] .pet-status {
  border-color: var(--danger-border);
  background: var(--danger-soft);
  color: var(--danger);
}

.sprite-source {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  opacity: 0;
  pointer-events: none;
}
</style>
