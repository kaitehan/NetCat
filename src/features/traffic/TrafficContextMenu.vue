<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue'
import { ChevronRight, Copy, EyeOff, RefreshCw, Replace, TextQuote } from '@lucide/vue'

export type TrafficMenuAction =
  | 'copy:url'
  | 'copy:curl'
  | 'copy:powershell'
  | 'copy:fetchBrowser'
  | 'copy:fetchNode'
  | 'copy:requestHeaders'
  | 'copy:responseHeaders'
  | 'copy:responseBody'
  | 'hide:url'
  | 'hide:host'
  | 'replay:upstream'
  | 'replay:mockAware'
  | 'mock:headers'
  | 'mock:body'

const { open, x, y, disabledReasons = {} } = defineProps<{
  open: boolean
  x: number
  y: number
  disabledReasons?: Partial<Record<TrafficMenuAction, string>>
}>()

const emit = defineEmits<{
  close: []
  action: [action: TrafficMenuAction]
}>()

const menu = useTemplateRef<HTMLElement>('menu')
const activeSubmenu = shallowRef<'copy' | 'hide' | 'replay' | null>(null)
const MENU_WIDTH = 194
const MENU_HEIGHT = 230
const SUBMENU_WIDTH = 218

const menuStyle = computed(() => ({
  left: `${Math.max(8, Math.min(x, window.innerWidth - MENU_WIDTH - 8))}px`,
  top: `${Math.max(8, Math.min(y, window.innerHeight - MENU_HEIGHT - 8))}px`
}))

const submenuClass = computed(() => ({
  flipped: x + MENU_WIDTH + SUBMENU_WIDTH > window.innerWidth - 8
}))

function activate(action: TrafficMenuAction) {
  if (disabledReasons[action]) {
    return
  }
  emit('action', action)
}

function closeOnOutside(event: PointerEvent) {
  if (open && !menu.value?.contains(event.target as Node)) {
    emit('close')
  }
}

function closeOnViewportChange() {
  if (open) {
    emit('close')
  }
}

function handleKeydown(event: KeyboardEvent) {
  if (!open) {
    return
  }
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  const buttons = [...(menu.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])]
    .filter((button) => {
      const submenu = button.closest<HTMLElement>('.submenu')
      return !submenu || submenu.dataset.name === activeSubmenu.value
    })
  const index = buttons.indexOf(document.activeElement as HTMLButtonElement)
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const direction = event.key === 'ArrowDown' ? 1 : -1
    buttons[(index + direction + buttons.length) % buttons.length]?.focus()
  } else if (event.key === 'ArrowRight') {
    const submenu = (document.activeElement as HTMLElement)?.dataset.submenu as 'copy' | 'hide' | 'replay' | undefined
    if (submenu) {
      event.preventDefault()
      activeSubmenu.value = submenu
      void nextTick(() => menu.value?.querySelector<HTMLButtonElement>(`.submenu[data-name="${submenu}"] button:not(:disabled)`)?.focus())
    }
  } else if (event.key === 'ArrowLeft' && activeSubmenu.value) {
    event.preventDefault()
    const name = activeSubmenu.value
    activeSubmenu.value = null
    void nextTick(() => menu.value?.querySelector<HTMLButtonElement>(`[data-submenu="${name}"]`)?.focus())
  }
}

watch(() => open, (value) => {
  activeSubmenu.value = null
  if (value) {
    void nextTick(() => menu.value?.querySelector<HTMLButtonElement>('button')?.focus())
  }
})

onMounted(() => {
  document.addEventListener('pointerdown', closeOnOutside, true)
  window.addEventListener('keydown', handleKeydown)
  window.addEventListener('resize', closeOnViewportChange)
  window.addEventListener('scroll', closeOnViewportChange, true)
  if (open) {
    void nextTick(() => menu.value?.querySelector<HTMLButtonElement>('button')?.focus())
  }
})

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', closeOnOutside, true)
  window.removeEventListener('keydown', handleKeydown)
  window.removeEventListener('resize', closeOnViewportChange)
  window.removeEventListener('scroll', closeOnViewportChange, true)
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="menu"
      class="context-menu"
      :style="menuStyle"
      role="menu"
      aria-label="流量操作"
    >
      <div class="menu-branch" @mouseenter="activeSubmenu = 'copy'">
        <button type="button" role="menuitem" data-submenu="copy" @click="activeSubmenu = 'copy'">
          <Copy :size="14" />
          <span>复制</span>
          <ChevronRight :size="13" class="branch-arrow" />
        </button>
        <div v-if="activeSubmenu === 'copy'" class="submenu" :class="submenuClass" data-name="copy" role="menu">
          <button v-for="item in [
            ['url', '网址'], ['curl', 'cURL'], ['powershell', 'PowerShell'],
            ['fetchBrowser', '浏览器 fetch'], ['fetchNode', 'Node.js fetch'],
            ['requestHeaders', '请求头'], ['responseHeaders', '响应头'], ['responseBody', '响应正文']
          ]" :key="item[0]" type="button" role="menuitem" :disabled="Boolean(disabledReasons[`copy:${item[0]}` as TrafficMenuAction])" :title="disabledReasons[`copy:${item[0]}` as TrafficMenuAction]" @click="activate(`copy:${item[0]}` as TrafficMenuAction)">
            {{ item[1] }}
          </button>
        </div>
      </div>
      <div class="menu-branch" @mouseenter="activeSubmenu = 'hide'">
        <button type="button" role="menuitem" data-submenu="hide" @click="activeSubmenu = 'hide'">
          <EyeOff :size="14" />
          <span>屏蔽</span>
          <ChevronRight :size="13" class="branch-arrow" />
        </button>
        <div v-if="activeSubmenu === 'hide'" class="submenu" :class="submenuClass" data-name="hide" role="menu">
          <button type="button" role="menuitem" @click="activate('hide:url')">屏蔽完整 URL</button>
          <button type="button" role="menuitem" @click="activate('hide:host')">屏蔽域名</button>
        </div>
      </div>
      <div class="menu-branch" @mouseenter="activeSubmenu = 'replay'">
        <button type="button" role="menuitem" data-submenu="replay" @click="activeSubmenu = 'replay'">
          <RefreshCw :size="14" />
          <span>重新发送</span>
          <ChevronRight :size="13" class="branch-arrow" />
        </button>
        <div v-if="activeSubmenu === 'replay'" class="submenu" :class="submenuClass" data-name="replay" role="menu">
          <button type="button" role="menuitem" :disabled="Boolean(disabledReasons['replay:upstream'])" :title="disabledReasons['replay:upstream']" @click="activate('replay:upstream')">直接发送到真实上游</button>
          <button type="button" role="menuitem" :disabled="Boolean(disabledReasons['replay:mockAware'])" :title="disabledReasons['replay:mockAware']" @click="activate('replay:mockAware')">应用当前 Mock 后发送</button>
        </div>
      </div>
      <div class="menu-separator" />
      <button type="button" role="menuitem" :disabled="Boolean(disabledReasons['mock:headers'])" :title="disabledReasons['mock:headers']" @click="activate('mock:headers')">
        <TextQuote :size="14" />
        <span>替换标头</span>
      </button>
      <button type="button" role="menuitem" :disabled="Boolean(disabledReasons['mock:body'])" :title="disabledReasons['mock:body']" @click="activate('mock:body')">
        <Replace :size="14" />
        <span>替换内容</span>
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.context-menu,
.submenu {
  position: fixed;
  z-index: 120;
  width: 194px;
  padding: 5px;
  border-radius: var(--radius-control);
  background: var(--surface);
  box-shadow: var(--shadow-float);
  color: var(--text-secondary);
}

.context-menu button,
.submenu button {
  display: flex;
  width: 100%;
  min-height: 30px;
  padding: 0 8px;
  align-items: center;
  gap: 8px;
  border: 0;
  border-radius: var(--radius-control);
  outline: 0;
  background: transparent;
  color: inherit;
  font-family: inherit;
  font-size: 11px;
  text-align: left;
}

.context-menu button:hover,
.context-menu button:focus,
.submenu button:hover,
.submenu button:focus {
  background: var(--surface-hover);
}

.context-menu button:disabled,
.submenu button:disabled {
  color: var(--text-muted);
  opacity: 0.48;
}

.menu-branch {
  position: relative;
}

.branch-arrow {
  margin-left: auto;
}

.submenu {
  position: absolute;
  top: -5px;
  left: calc(100% + 5px);
  width: 218px;
}

.submenu.flipped {
  right: calc(100% + 5px);
  left: auto;
}

.menu-separator {
  height: 1px;
  margin: 4px 5px;
  background: var(--border);
}
</style>
