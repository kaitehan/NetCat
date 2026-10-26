<script setup lang="ts">
import { Braces, FlaskConical, Info, Network, Settings, ShieldCheck } from '@lucide/vue'
import appIcon from '../../src-tauri/icons/icon.png'
import { APP_BRAND } from '../shared/brand'

const { activeView, trafficCount, mockCount } = defineProps<{
  activeView: 'traffic' | 'mocks' | 'scripts' | 'pinning' | 'settings' | 'about'
  trafficCount: number
  mockCount: number
}>()

const emit = defineEmits<{
  changeView: [view: 'traffic' | 'mocks' | 'scripts' | 'pinning' | 'settings' | 'about']
}>()
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <img :src="appIcon" :alt="`${APP_BRAND.name} 应用图标`" />
      <div>
        <strong>{{ APP_BRAND.name }}</strong>
        <span>{{ APP_BRAND.chineseName }} · LOCAL PROXY</span>
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
      <button
        type="button"
        :class="{ active: activeView === 'scripts' }"
        aria-label="脚本"
        @click="emit('changeView', 'scripts')"
      >
        <Braces :size="16" />
        <span>脚本</span>
      </button>
      <button
        type="button"
        :class="{ active: activeView === 'pinning' }"
        aria-label="SSL Pinning"
        @click="emit('changeView', 'pinning')"
      >
        <ShieldCheck :size="16" />
        <span>SSL Pinning</span>
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
      <button
        type="button"
        class="about-row"
        :class="{ active: activeView === 'about' }"
        aria-label="关于"
        :title="`关于 ${APP_BRAND.name}`"
        @click="emit('changeView', 'about')"
      >
        <Info :size="13" />
        <span>关于</span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  display: grid;
  min-height: 0;
  grid-template-rows: auto auto 1fr auto;
  padding: 6px 10px 10px;
  border-radius: var(--radius-panel);
  background: var(--sidebar);
  box-shadow: var(--shadow-surface);
  color: var(--sidebar-text);
}

.brand {
  display: flex;
  padding: 2px 6px 13px;
  align-items: center;
  gap: 10px;
}

.brand img {
  width: 36px;
  height: 36px;
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
  font-size: var(--font-size-xl);
  font-weight: 750;
  line-height: 1.15;
}

.brand span {
  color: var(--sidebar-muted);
  font-size: var(--font-size-2xs);
  font-weight: 700;
  line-height: 1.2;
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
  font-size: var(--font-size-sm);
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
  font-size: var(--font-size-2xs);
  font-weight: 600;
  text-align: right;
}

.sidebar-note {
  display: flex;
  padding: 18px 8px;
  align-items: start;
  gap: 7px;
  color: var(--sidebar-muted);
  font-size: var(--font-size-2xs);
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
  gap: 2px;
}

.about-row {
  display: grid;
  width: 100%;
  height: 26px;
  padding: 0 9px;
  grid-template-columns: 20px 1fr;
  align-items: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: var(--sidebar-muted);
  font-family: inherit;
  font-size: var(--font-size-2xs);
  text-align: left;
  opacity: 0.78;
}

.about-row:hover,
.about-row.active {
  color: var(--accent-strong);
  opacity: 1;
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
  .about-row span {
    display: none;
  }

  .navigation button,
  .settings-row,
  .about-row {
    padding: 0;
    grid-template-columns: 1fr;
    place-items: center;
  }

  .sidebar-note {
    padding-right: 0;
    padding-left: 0;
    justify-content: center;
  }
}
</style>
