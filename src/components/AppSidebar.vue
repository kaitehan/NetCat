<script setup lang="ts">
import { Braces, FlaskConical, Network, Settings } from '@lucide/vue'
import appIcon from '../../src-tauri/icons/icon.svg'

const { activeView, trafficCount, mockCount } = defineProps<{
  activeView: 'traffic' | 'mocks'
  trafficCount: number
  mockCount: number
}>()

const emit = defineEmits<{
  changeView: [view: 'traffic' | 'mocks']
}>()
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <img :src="appIcon" alt="" />
      <div>
        <strong>NetCapture</strong>
        <span>LOCAL PROXY</span>
      </div>
    </div>

    <nav class="navigation" aria-label="主导航">
      <button type="button" :class="{ active: activeView === 'traffic' }" @click="emit('changeView', 'traffic')">
        <Network :size="16" />
        <span>流量</span>
        <b>{{ trafficCount }}</b>
      </button>
      <button type="button" :class="{ active: activeView === 'mocks' }" @click="emit('changeView', 'mocks')">
        <FlaskConical :size="16" />
        <span>Mock 规则</span>
        <b>{{ mockCount }}</b>
      </button>
    </nav>

    <div class="sidebar-note">
      <Braces :size="14" />
      <p>流量只保存在本机</p>
    </div>

    <button type="button" class="settings-row" disabled>
      <Settings :size="15" />
      设置
      <span>即将推出</span>
    </button>
  </aside>
</template>

<style scoped>
.sidebar {
  display: grid;
  min-height: 0;
  grid-template-rows: auto auto 1fr auto;
  padding: 14px 10px 10px;
  background: var(--sidebar);
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
  border-radius: 7px;
}

.brand div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.brand strong {
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
  border-radius: 5px;
  background: transparent;
  color: var(--sidebar-muted);
  font-family: inherit;
  font-size: 11px;
  text-align: left;
}

.navigation button.active {
  background: var(--sidebar-raised);
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
  opacity: 0.52;
}

.settings-row span {
  font-size: 8px;
}
</style>
