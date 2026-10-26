<script setup lang="ts">
import { Laptop, Smartphone } from '@lucide/vue'
import type { DeviceScope, TrafficDevice } from './device-filter'

const props = defineProps<{
  modelValue: DeviceScope
  devices: readonly TrafficDevice[]
  totalCount: number
  localCount: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: DeviceScope]
}>()

function select(scope: DeviceScope) {
  if (scope !== props.modelValue) emit('update:modelValue', scope)
}
</script>

<template>
  <nav class="device-tabs" role="tablist" aria-label="请求设备">
    <button
      type="button"
      role="tab"
      :aria-selected="modelValue === 'all'"
      @click="select('all')"
    >
      全部 <span>{{ totalCount }}</span>
    </button>
    <button
      type="button"
      role="tab"
      :aria-selected="modelValue === 'local'"
      @click="select('local')"
    >
      <Laptop :size="13" />本机 <span>{{ localCount }}</span>
    </button>
    <button
      v-for="device in devices"
      :key="device.token"
      type="button"
      role="tab"
      :aria-selected="modelValue === `device:${device.token}`"
      @click="select(`device:${device.token}`)"
    >
      <Smartphone :size="13" />{{ device.name }} <span>{{ device.count }}</span>
      <small>{{ device.online ? '在线' : '已断开' }}</small>
    </button>
  </nav>
</template>

<style scoped>
.device-tabs {
  display: flex;
  min-width: 0;
  height: 38px;
  padding: 0 10px;
  align-items: end;
  gap: 2px;
  overflow-x: auto;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  scrollbar-width: thin;
}

.device-tabs button {
  display: inline-flex;
  height: 32px;
  padding: 0 10px;
  flex: 0 0 auto;
  align-items: center;
  gap: 5px;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-family: inherit;
  font-size: 10px;
  cursor: pointer;
}

.device-tabs button[aria-selected='true'] {
  border-bottom-color: var(--accent-strong);
  color: var(--text);
  font-weight: 650;
}

.device-tabs button:hover {
  color: var(--text);
}

.device-tabs span {
  min-width: 18px;
  padding: 1px 4px;
  border-radius: 3px;
  background: var(--border-soft);
  text-align: center;
}

.device-tabs small {
  color: var(--text-muted);
  font-size: 8px;
}
</style>
