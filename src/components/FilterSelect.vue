<script setup lang="ts">
import { ChevronDown, Check } from '@lucide/vue'
import { computed, onBeforeUnmount, onMounted, shallowRef, useId, useTemplateRef } from 'vue'

interface FilterSelectOption {
  value: string
  label: string
}

const { label, options, wide = false } = defineProps<{
  label: string
  options: readonly FilterSelectOption[]
  wide?: boolean
}>()
const model = defineModel<string>({ required: true })
const root = useTemplateRef<HTMLElement>('root')
const open = shallowRef(false)
const activeIndex = shallowRef(0)
const componentId = useId()
const listboxId = `${componentId}-listbox`

const selectedIndex = computed(() => {
  const index = options.findIndex((option) => option.value === model.value)
  return index >= 0 ? index : 0
})
const selectedOption = computed(() => options[selectedIndex.value])
const activeOptionId = computed(() => open.value ? optionId(activeIndex.value) : undefined)

function optionId(index: number) {
  return `${componentId}-option-${index}`
}

function openMenu(index = selectedIndex.value) {
  if (!options.length) {
    return
  }
  activeIndex.value = index
  open.value = true
}

function closeMenu() {
  open.value = false
}

function toggleMenu() {
  if (open.value) {
    closeMenu()
  } else {
    openMenu()
  }
}

function selectOption(index: number) {
  const option = options[index]
  if (!option) {
    return
  }
  model.value = option.value
  closeMenu()
}

function moveActive(step: number) {
  if (!options.length) {
    return
  }
  const start = open.value ? activeIndex.value : selectedIndex.value
  activeIndex.value = (start + step + options.length) % options.length
  open.value = true
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    moveActive(1)
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    moveActive(-1)
  } else if (event.key === 'Home' && open.value) {
    event.preventDefault()
    activeIndex.value = 0
  } else if (event.key === 'End' && open.value) {
    event.preventDefault()
    activeIndex.value = options.length - 1
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    if (open.value) {
      selectOption(activeIndex.value)
    } else {
      openMenu()
    }
  } else if (event.key === 'Escape' && open.value) {
    event.preventDefault()
    closeMenu()
  }
}

function handlePointerDown(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) {
    closeMenu()
  }
}

onMounted(() => document.addEventListener('pointerdown', handlePointerDown))
onBeforeUnmount(() => document.removeEventListener('pointerdown', handlePointerDown))
</script>

<template>
  <div ref="root" class="filter-select" :class="{ 'filter-select--wide': wide, 'filter-select--open': open }">
    <button
      type="button"
      class="filter-select__trigger"
      :aria-label="`${label}：${selectedOption?.label ?? ''}`"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="listboxId"
      :aria-activedescendant="activeOptionId"
      @click="toggleMenu"
      @keydown="handleKeydown"
    >
      <span class="filter-select__value">{{ selectedOption?.label }}</span>
      <ChevronDown class="filter-select__chevron" :size="14" aria-hidden="true" />
    </button>

    <div v-if="open" :id="listboxId" class="filter-select__menu" role="listbox" :aria-label="`${label}选项`">
      <button
        v-for="(option, index) in options"
        :id="optionId(index)"
        :key="option.value"
        type="button"
        class="filter-select__option"
        role="option"
        :aria-selected="option.value === model"
        :data-active="index === activeIndex || undefined"
        @click="selectOption(index)"
        @pointerenter="activeIndex = index"
      >
        <span>{{ option.label }}</span>
        <Check v-if="option.value === model" :size="13" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.filter-select {
  position: relative;
  width: 102px;
  min-width: 76px;
  flex: 0 1 102px;
}

.filter-select--wide {
  width: 180px;
  min-width: 96px;
  flex-basis: 180px;
}

.filter-select__trigger {
  display: flex;
  width: 100%;
  height: 29px;
  padding: 0 8px 0 10px;
  align-items: center;
  justify-content: space-between;
  gap: 7px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: var(--font-size-xs);
  line-height: 1;
}

.filter-select__trigger:hover,
.filter-select--open .filter-select__trigger {
  border-color: var(--border-strong);
  background: var(--surface-hover);
  color: var(--text);
}

.filter-select--open .filter-select__trigger {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-soft);
}

.filter-select__value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.filter-select__chevron {
  flex: none;
  color: var(--text-muted);
  transition: transform 140ms ease;
}

.filter-select--open .filter-select__chevron {
  transform: rotate(180deg);
  color: var(--accent);
}

.filter-select__menu {
  position: absolute;
  z-index: 60;
  top: calc(100% + 5px);
  left: 0;
  width: max(100%, 118px);
  max-height: 248px;
  padding: 4px;
  overflow-y: auto;
  border-radius: var(--radius-panel);
  background: var(--surface);
  box-shadow: var(--shadow-float);
}

.filter-select--wide .filter-select__menu {
  width: max(100%, 190px);
}

.filter-select__option {
  display: flex;
  width: 100%;
  min-height: 30px;
  padding: 0 8px;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-secondary);
  font-family: inherit;
  font-size: var(--font-size-sm);
  text-align: left;
  white-space: nowrap;
}

.filter-select__option:hover,
.filter-select__option[data-active='true'] {
  background: var(--surface-hover);
  color: var(--text);
}

.filter-select__option[aria-selected='true'] {
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-weight: 600;
}
</style>
