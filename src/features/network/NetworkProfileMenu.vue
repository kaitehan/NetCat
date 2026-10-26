<script setup lang="ts">
import { Check, Gauge, Plus } from '@lucide/vue'
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef } from 'vue'
import type { NetworkProfile } from '../../shared/types'

const props = defineProps<{
  profiles: readonly NetworkProfile[]
  activeId: string | null
}>()

const emit = defineEmits<{
  activate: [id: string | null]
  add: []
}>()

const root = useTemplateRef<HTMLElement>('root')
const open = shallowRef(false)
const presets = computed(() => props.profiles.filter((profile) => profile.kind === 'preset'))
const customProfiles = computed(() => props.profiles.filter((profile) => profile.kind === 'custom'))
const activeName = computed(() => props.profiles.find((profile) => profile.id === props.activeId)?.name ?? '无限流')

function activate(id: string | null) {
  emit('activate', id)
  open.value = false
}

function addProfile() {
  emit('add')
  open.value = false
}

function handlePointerDown(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) {
    open.value = false
  }
}

onMounted(() => document.addEventListener('pointerdown', handlePointerDown))
onBeforeUnmount(() => document.removeEventListener('pointerdown', handlePointerDown))
</script>

<template>
  <div ref="root" class="network-profile-menu">
    <button
      type="button"
      class="network-trigger"
      :class="{ active: activeId !== null }"
      :aria-label="`限流配置：${activeName}`"
      :aria-expanded="open"
      aria-haspopup="menu"
      @click="open = !open"
    >
      <Gauge :size="15" />
      <span>{{ activeName }}</span>
    </button>

    <Transition name="network-menu">
      <div v-if="open" class="network-menu" role="menu" aria-label="限流配置">
        <button
          type="button"
          role="menuitemradio"
          aria-label="无限流"
          :aria-checked="activeId === null"
          @click="activate(null)"
        >
          <Check :size="15" :class="{ hidden: activeId !== null }" />
          <span>无限流</span>
        </button>

        <span class="menu-heading">预设</span>
        <button
          v-for="profile in presets"
          :key="profile.id"
          type="button"
          role="menuitemradio"
          :aria-checked="activeId === profile.id"
          @click="activate(profile.id)"
        >
          <Check :size="15" :class="{ hidden: activeId !== profile.id }" />
          <span>{{ profile.name }}</span>
          <small v-if="profile.downloadKbps">{{ profile.downloadKbps }} Kbps</small>
        </button>

        <template v-if="customProfiles.length">
          <span class="menu-heading">自定义</span>
          <button
            v-for="profile in customProfiles"
            :key="profile.id"
            type="button"
            role="menuitemradio"
            :aria-checked="activeId === profile.id"
            @click="activate(profile.id)"
          >
            <Check :size="15" :class="{ hidden: activeId !== profile.id }" />
            <span>{{ profile.name }}</span>
          </button>
        </template>

        <span v-if="!customProfiles.length" class="menu-heading">自定义</span>
        <button type="button" role="menuitem" aria-label="添加自定义配置" @click="addProfile">
          <Plus :size="15" />
          <span>添加...</span>
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.network-profile-menu {
  position: relative;
}

.network-trigger {
  display: inline-flex;
  height: 30px;
  min-width: 92px;
  padding: 0 9px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid transparent;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-secondary);
  font: inherit;
  font-size: var(--font-size-sm);
  font-weight: 600;
}

.network-trigger:hover,
.network-trigger.active {
  border-color: var(--border);
  background: var(--surface-hover);
  color: var(--accent-strong);
}

.network-menu {
  position: absolute;
  z-index: 80;
  top: 36px;
  right: 0;
  width: 232px;
  padding: 6px;
  border: 0;
  border-radius: 8px;
  background: var(--surface-raised);
  box-shadow: var(--shadow-float);
  transform-origin: top right;
}

.network-menu-enter-active {
  transition:
    opacity 140ms ease,
    transform 140ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.network-menu-leave-active {
  transition:
    opacity 110ms ease-in,
    transform 110ms ease-in;
}

.network-menu-enter-from,
.network-menu-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}

.network-menu button {
  display: grid;
  width: 100%;
  min-height: 34px;
  padding: 0 8px;
  grid-template-columns: 18px minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-md);
  text-align: left;
}

.network-menu button:hover,
.network-menu button[aria-checked='true'] {
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.network-menu small {
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
}

.menu-heading {
  display: block;
  padding: 9px 26px 4px;
  color: var(--text-muted);
  font-size: var(--font-size-2xs);
  font-weight: 700;
}

.hidden {
  visibility: hidden;
}

@media (prefers-reduced-motion: reduce) {
  .network-menu-enter-active,
  .network-menu-leave-active {
    transition-duration: 0ms;
  }
}
</style>
