<script setup lang="ts">
import {
  CircleStop,
  Download,
  Laptop,
  Pause,
  Play,
  ShieldCheck,
  Trash2,
  Wifi
} from '@lucide/vue'
import type { ProxyStatus } from '../shared/types'

const {
  status,
  allowLan,
  captureEnabled,
  systemProxyEnabled,
  caGenerated,
  port,
  busy
} = defineProps<{
  status: ProxyStatus
  allowLan: boolean
  captureEnabled: boolean
  systemProxyEnabled: boolean
  caGenerated: boolean
  port: number
  busy: boolean
}>()

const emit = defineEmits<{
  toggleProxy: []
  updateAllowLan: [value: boolean]
  updatePort: [value: number]
  toggleCapture: [value: boolean]
  toggleSystemProxy: []
  clear: []
  export: []
  certificate: []
}>()

function updatePort(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isInteger(value) && value >= 1 && value <= 65535) {
    emit('updatePort', value)
  }
}
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

    <div class="toolbar-settings" role="group" aria-label="代理与流量设置">
      <div class="settings-controls">
        <label class="port-control" title="代理端口">
          <span>端口</span>
          <input :value="port" type="number" min="1" max="65535" :disabled="status.state === 'running'" @change="updatePort" />
        </label>

        <div class="toolbar-divider" />

        <label class="toggle-control" title="允许同一局域网中的手机连接">
          <input
            type="checkbox"
            :checked="allowLan"
            :disabled="status.state === 'running'"
            @change="emit('updateAllowLan', ($event.target as HTMLInputElement).checked)"
          />
          <Wifi :size="14" />
          局域网
        </label>

        <label class="toggle-control" title="暂停后仍会转发请求，但不再保存新记录">
          <input
            type="checkbox"
            :checked="captureEnabled"
            @change="emit('toggleCapture', ($event.target as HTMLInputElement).checked)"
          />
          <Pause :size="14" />
          记录
        </label>

        <button
          type="button"
          class="tool-button"
          :class="{ active: systemProxyEnabled }"
          :disabled="status.state !== 'running'"
          title="切换本机系统代理"
          @click="emit('toggleSystemProxy')"
        >
          <Laptop :size="15" />
          系统代理
        </button>
      </div>

      <div class="utility-actions">
        <button type="button" class="icon-tool" title="清空流量" aria-label="清空流量" @click="emit('clear')">
          <Trash2 :size="16" />
        </button>
        <button type="button" class="icon-tool" title="导出 HAR" aria-label="导出 HAR" @click="emit('export')">
          <Download :size="16" />
        </button>
        <button
          type="button"
          class="certificate-button"
          :class="{ ready: caGenerated }"
          @click="emit('certificate')"
        >
          <ShieldCheck :size="15" />
          证书
        </button>
      </div>
    </div>
  </header>
</template>

<style scoped>
.toolbar {
  display: flex;
  min-width: 0;
  height: 52px;
  padding: 0 14px;
  align-items: center;
  gap: 16px;
  overflow: hidden;
  border-radius: var(--radius-panel);
  background: var(--surface-translucent);
  box-shadow: var(--shadow-toolbar);
}

.proxy-cluster,
.toolbar-settings,
.settings-controls,
.utility-actions {
  display: flex;
  min-width: 0;
  align-items: center;
}

.proxy-cluster {
  gap: 8px;
}

.toolbar-settings {
  justify-content: space-between;
  gap: 16px;
  flex: 1;
}

.settings-controls,
.utility-actions {
  gap: 8px;
}

.proxy-button,
.tool-button,
.certificate-button,
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
  font-size: 11px;
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
  font-size: 10px;
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

.port-control,
.toggle-control {
  display: inline-flex;
  height: 30px;
  align-items: center;
  gap: 6px;
  color: var(--text-muted);
  font-size: 10px;
  white-space: nowrap;
}

.port-control input {
  width: 62px;
  height: 28px;
  padding: 0 7px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 10px;
}

.toggle-control {
  padding: 0 5px;
  color: var(--text-secondary);
}

.toggle-control input {
  width: 14px;
  height: 14px;
  margin: 0;
  accent-color: var(--accent-strong);
}

.toolbar-divider {
  width: 1px;
  height: 20px;
  margin: 0 2px;
  background: var(--border);
}

.tool-button,
.certificate-button {
  padding: 0 9px;
}

.tool-button.active,
.certificate-button.ready {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.icon-tool {
  width: 30px;
  border-color: transparent;
  background: transparent;
}

.icon-tool:hover,
.tool-button:hover,
.certificate-button:hover {
  background: var(--surface-hover);
}

button:disabled,
input:disabled {
  opacity: 0.48;
}

@media (max-width: 1100px) {
  .toolbar {
    padding-right: 10px;
    padding-left: 10px;
    gap: 10px;
  }

  .toolbar-settings {
    gap: 10px;
  }

  .settings-controls,
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

  .toggle-control svg,
  .toggle-control {
    font-size: 0;
  }
}
</style>
