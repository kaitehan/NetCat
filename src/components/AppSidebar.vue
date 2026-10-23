<script setup lang="ts">
import { Braces, FlaskConical, Network, Settings } from '@lucide/vue'
import { computed } from 'vue'
import appIcon from '../../src-tauri/icons/icon.png'
import type { ProxyStatus } from '../shared/types'

const { activeView, trafficCount, mockCount, proxyStatus } = defineProps<{
  activeView: 'traffic' | 'mocks' | 'settings'
  trafficCount: number
  mockCount: number
  proxyStatus: ProxyStatus
}>()

const emit = defineEmits<{
  changeView: [view: 'traffic' | 'mocks' | 'settings']
}>()

const statusLabel = computed(() => {
  switch (proxyStatus.state) {
    case 'running':
      return '捕获中'
    case 'starting':
      return '启动中'
    case 'stopping':
      return '停止中'
    case 'error':
      return '代理异常'
    default:
      return '就绪'
  }
})
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <img :src="appIcon" alt="NetCapture 应用图标" />
      <div>
        <strong>NetCapture</strong>
        <span>LOCAL PROXY</span>
      </div>
    </div>

    <nav class="navigation" aria-label="主导航">
      <button
        type="button"
        :class="{ active: activeView === 'traffic' }"
        aria-label="网络请求"
        @click="emit('changeView', 'traffic')"
      >
        <Network :size="16" />
        <span>网络请求</span>
        <b>{{ trafficCount }}</b>
      </button>
      <button
        type="button"
        :class="{ active: activeView === 'mocks' }"
        aria-label="Mock 规则"
        @click="emit('changeView', 'mocks')"
      >
        <FlaskConical :size="16" />
        <span>Mock 规则</span>
        <b>{{ mockCount }}</b>
      </button>
    </nav>

    <div class="sidebar-note">
      <Braces :size="14" />
      <p>请求数据只保存在本机</p>
    </div>

    <div class="sidebar-footer">
      <button
        type="button"
        class="settings-row"
        :class="{ active: activeView === 'settings' }"
        aria-label="设置"
        @click="emit('changeView', 'settings')"
      >
        <Settings :size="15" />
        <span>设置</span>
      </button>
      <div class="sidebar-status" :class="`state-${proxyStatus.state}`" aria-label="代理状态">
        <i aria-hidden="true" />
        <span>{{ statusLabel }}</span>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  display: grid;
  min-height: 0;
  grid-template-rows: auto auto 1fr auto;
  padding: 16px 10px 10px;
  border-radius: var(--radius-panel);
  background: var(--sidebar);
  box-shadow: var(--shadow-surface);
  color: var(--sidebar-text);
}

.brand {
  display: flex;
  padding: 2px 6px 17px;
  align-items: center;
  gap: 9px;
}

.brand img {
  width: 31px;
  height: 31px;
  border-radius: var(--radius-control);
  box-shadow: var(--shadow-brand);
}

.brand div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.brand strong {
  color: var(--sidebar-text);
  font-size: 13px;
}

.brand span {
  color: var(--sidebar-muted);
  font-size: 8px;
  font-weight: 700;
}

.navigation {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.navigation button,
.settings-row {
  display: grid;
  width: 100%;
  height: 36px;
  padding: 0 9px;
  grid-template-columns: 20px 1fr auto;
  align-items: center;
  gap: 6px;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--sidebar-muted);
  font-family: inherit;
  font-size: 11px;
  text-align: left;
}

.navigation button.active,
.settings-row.active {
  border: 1px solid var(--border);
  background: var(--sidebar-raised);
  box-shadow: var(--shadow-active);
  color: var(--accent-strong);
}

.navigation button:hover:not(.active) {
  background: var(--sidebar-hover);
  color: var(--sidebar-text);
}

.navigation b {
  min-width: 22px;
  color: inherit;
  font-size: 9px;
  font-weight: 600;
  text-align: right;
}

.sidebar-note {
  display: flex;
  padding: 18px 8px;
  align-items: start;
  gap: 7px;
  color: var(--sidebar-muted);
  font-size: 9px;
}

.sidebar-note p {
  margin: 0;
  line-height: 1.5;
}

.settings-row {
  cursor: pointer;
}

.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sidebar-status {
  display: flex;
  height: 10px;
  padding: 0 9px;
  align-items: center;
  gap: 6px;
  color: var(--sidebar-muted);
  font-size: 9px;
  line-height: 10px;
}

.sidebar-status i {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: currentColor;
}

.sidebar-status.state-running {
  color: var(--mock);
}

.sidebar-status.state-error {
  color: var(--danger);
}

.settings-row:hover:not(.active) {
  background: var(--sidebar-hover);
  color: var(--sidebar-text);
}

@media (max-width: 820px) {
  .sidebar {
    padding-right: 8px;
    padding-left: 8px;
  }

  .brand {
    padding-right: 0;
    padding-left: 0;
    justify-content: center;
  }

  .brand div,
  .navigation button span,
  .navigation b,
  .sidebar-note p,
  .settings-row span,
  .sidebar-status span {
    display: none;
  }

  .navigation button,
  .settings-row {
    padding: 0;
    grid-template-columns: 1fr;
    place-items: center;
  }

  .sidebar-note {
    padding-right: 0;
    padding-left: 0;
    justify-content: center;
  }

  .sidebar-status {
    padding: 0;
    justify-content: center;
  }
}
</style>
