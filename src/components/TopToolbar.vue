<script setup lang="ts">
import {
  CircleStop,
  Download,
  Play,
  Trash2
} from '@lucide/vue'
import NetworkProfileMenu from '../features/network/NetworkProfileMenu.vue'
import type { NetworkProfile, ProxyStatus } from '../shared/types'

const { status, busy } = defineProps<{
  status: ProxyStatus
  busy: boolean
  networkProfiles: readonly NetworkProfile[]
  activeNetworkProfileId: string | null
}>()

const emit = defineEmits<{
  toggleProxy: []
  clear: []
  export: []
  activateNetworkProfile: [id: string | null]
  addNetworkProfile: []
}>()
</script>

<template>
  <header class="toolbar">
    <div class="proxy-cluster" role="group" aria-label="代理控制">
      <button
        type="button"
        class="proxy-button"
        :class="{ running: status.state === 'running' }"
        :disabled="busy || status.state === 'starting' || status.state === 'stopping'"
        :aria-label="status.state === 'running' ? '停止代理' : '启动代理'"
        @click="emit('toggleProxy')"
      >
        <CircleStop v-if="status.state === 'running'" :size="15" />
        <Play v-else :size="15" fill="currentColor" />
        {{ status.state === 'running' ? '停止代理' : '启动代理' }}
      </button>

      <div class="status-group">
        <span class="status-dot" :data-state="status.state" />
        <span class="status-label">
          {{ status.state === 'running' ? `${status.bindAddress}:${status.port}` : '代理已停止' }}
        </span>
      </div>
    </div>

    <div class="utility-actions" role="group" aria-label="请求操作">
      <NetworkProfileMenu
        :profiles="networkProfiles"
        :active-id="activeNetworkProfileId"
        @activate="emit('activateNetworkProfile', $event)"
        @add="emit('addNetworkProfile')"
      />
      <button type="button" class="icon-tool" title="清空请求记录" aria-label="清空请求记录" @click="emit('clear')">
        <Trash2 :size="16" />
      </button>
      <button type="button" class="icon-tool" title="导出 HAR" aria-label="导出 HAR" @click="emit('export')">
        <Download :size="16" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.toolbar {
  position: relative;
  z-index: 20;
  display: flex;
  min-width: 0;
  height: 52px;
  padding: 0 14px;
  align-items: center;
  gap: 16px;
  overflow: visible;
  border-radius: var(--radius-panel);
  background: var(--surface-translucent);
  box-shadow: var(--shadow-toolbar);
}

.proxy-cluster,
.utility-actions {
  display: flex;
  min-width: 0;
  align-items: center;
}

.proxy-cluster {
  gap: 8px;
  flex: 1;
}

.utility-actions {
  gap: 8px;
}

.proxy-button,
.icon-tool {
  display: inline-flex;
  height: 30px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: var(--font-size-sm);
  font-weight: 600;
  white-space: nowrap;
}

.proxy-button {
  min-width: 76px;
  padding: 0 6px;
  gap: 4px;
  border-color: var(--accent-strong);
  background: var(--accent);
  color: var(--on-accent);
}

.proxy-button.running {
  border-color: var(--danger-border);
  background: var(--danger-soft);
  color: var(--danger);
}

.status-group {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
  flex: 1;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-muted);
}

.status-dot[data-state='running'] {
  background: var(--accent-strong);
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.status-dot[data-state='error'] {
  background: var(--danger);
}

.status-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.icon-tool {
  width: 30px;
  border-color: transparent;
  background: transparent;
}

.icon-tool:hover {
  background: var(--surface-hover);
}

button:disabled {
  opacity: 0.48;
}

@media (max-width: 1100px) {
  .toolbar {
    padding-right: 10px;
    padding-left: 10px;
    gap: 10px;
  }

  .utility-actions {
    gap: 6px;
  }
}

@media (max-width: 820px) {
  .status-group {
    display: none;
  }

  .proxy-button {
    width: 100%;
    min-width: 0;
  }

}
</style>
