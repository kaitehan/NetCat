<script setup lang="ts">
import { Laptop, Pause, Radio, Router, ShieldCheck } from '@lucide/vue'
import { shallowRef } from 'vue'
import type { CaStatus, ProxyStatus } from '../../shared/types'
import CertificatePanel from '../certificate/CertificatePanel.vue'

const {
  proxyStatus,
  port,
  allowLan,
  captureEnabled,
  systemProxyPreferred,
  systemProxyEnabled,
  caStatus,
  busy
} = defineProps<{
  proxyStatus: ProxyStatus
  port: number
  allowLan: boolean
  captureEnabled: boolean
  systemProxyPreferred: boolean
  systemProxyEnabled: boolean
  caStatus: CaStatus
  busy: boolean
}>()

const emit = defineEmits<{
  updatePort: [value: number]
  updateAllowLan: [value: boolean]
  updateCapture: [value: boolean]
  updateSystemProxy: [value: boolean]
  generateCa: []
  exportCa: []
}>()

const activeTab = shallowRef<'proxy' | 'certificate'>('proxy')

function updatePort(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isInteger(value) && value >= 1 && value <= 65535) {
    emit('updatePort', value)
  }
}
</script>

<template>
  <section class="settings-page" aria-labelledby="settings-title">
    <header class="settings-header">
      <div>
        <h1 id="settings-title">设置</h1>
        <p>管理代理接入、请求记录与 HTTPS 证书</p>
      </div>
      <div class="settings-tabs" role="tablist" aria-label="设置分类">
        <button
          type="button"
          role="tab"
          :aria-selected="activeTab === 'proxy'"
          :tabindex="activeTab === 'proxy' ? 0 : -1"
          @click="activeTab = 'proxy'"
        >
          <Router :size="14" />
          代理配置
        </button>
        <button
          type="button"
          role="tab"
          :aria-selected="activeTab === 'certificate'"
          :tabindex="activeTab === 'certificate' ? 0 : -1"
          @click="activeTab = 'certificate'"
        >
          <ShieldCheck :size="14" />
          证书配置
        </button>
      </div>
    </header>

    <div v-if="activeTab === 'proxy'" class="settings-content" role="tabpanel">
      <section class="settings-section">
        <div class="section-heading">
          <Radio :size="17" />
          <div>
            <h2>监听配置</h2>
            <p>端口和接入范围需要在代理停止时修改。</p>
          </div>
        </div>

        <div class="setting-row">
          <div>
            <label for="proxy-port">代理端口</label>
            <p>本机和移动设备连接 NetCapture 使用的端口。</p>
          </div>
          <input
            id="proxy-port"
            :value="port"
            type="number"
            min="1"
            max="65535"
            :disabled="proxyStatus.state === 'running'"
            @change="updatePort"
          />
        </div>

        <div class="setting-row">
          <div>
            <label for="allow-lan">局域网接入</label>
            <p>允许同一私有网络中的手机和平板连接代理。</p>
          </div>
          <label class="switch-control">
            <input
              id="allow-lan"
              type="checkbox"
              :checked="allowLan"
              :disabled="proxyStatus.state === 'running'"
              @change="emit('updateAllowLan', ($event.target as HTMLInputElement).checked)"
            />
            <span aria-hidden="true" />
          </label>
        </div>
      </section>

      <section class="settings-section">
        <div class="section-heading">
          <Laptop :size="17" />
          <div>
            <h2>代理行为</h2>
            <p>控制系统请求接入和会话记录。</p>
          </div>
        </div>

        <div class="setting-row">
          <div>
            <label for="system-proxy">系统代理</label>
            <p v-if="proxyStatus.state === 'running'">
              {{ systemProxyEnabled ? '本机 HTTP/HTTPS 流量已接入。' : '当前未修改本机系统代理。' }}
            </p>
            <p v-else>开启后将在代理启动时自动接入，停止时恢复原配置。</p>
          </div>
          <label class="switch-control">
            <input
              id="system-proxy"
              type="checkbox"
              :checked="systemProxyPreferred"
              :disabled="busy"
              @change="emit('updateSystemProxy', ($event.target as HTMLInputElement).checked)"
            />
            <span aria-hidden="true" />
          </label>
        </div>

        <div class="setting-row">
          <div>
            <label for="capture-enabled">记录请求</label>
            <p>关闭后请求仍正常转发，但不会创建新的请求记录。</p>
          </div>
          <label class="switch-control">
            <input
              id="capture-enabled"
              type="checkbox"
              :checked="captureEnabled"
              :disabled="busy"
              @change="emit('updateCapture', ($event.target as HTMLInputElement).checked)"
            />
            <span aria-hidden="true" />
          </label>
        </div>
      </section>

      <div class="settings-note">
        <Pause :size="15" />
        <span>停止记录不会中断当前连接，也不会删除已有请求。</span>
      </div>
    </div>

    <div v-else class="certificate-content" role="tabpanel">
      <CertificatePanel
        embedded
        :ca-status="caStatus"
        :proxy-status="proxyStatus"
        :allow-lan="allowLan"
        :busy="busy"
        @generate="emit('generateCa')"
        @export="emit('exportCa')"
      />
    </div>
  </section>
</template>

<style scoped>
.settings-page {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: auto minmax(0, 1fr);
  overflow: hidden;
}

.settings-header {
  display: flex;
  padding: 18px 20px 0;
  align-items: end;
  justify-content: space-between;
  gap: 24px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-raised);
}

.settings-header h1 {
  margin: 0;
  font-size: 18px;
}

.settings-header p {
  margin: 5px 0 16px;
  color: var(--text-muted);
  font-size: 10px;
}

.settings-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
}

.settings-tabs button {
  display: inline-flex;
  height: 36px;
  padding: 0 12px;
  align-items: center;
  gap: 6px;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-family: inherit;
  font-size: 11px;
  font-weight: 600;
}

.settings-tabs button[aria-selected='true'] {
  border-bottom-color: var(--accent-strong);
  color: var(--accent-strong);
}

.settings-content,
.certificate-content {
  min-height: 0;
  overflow: auto;
}

.settings-content {
  width: min(720px, 100%);
  padding: 20px;
}

.settings-section {
  border-bottom: 1px solid var(--border);
}

.settings-section + .settings-section {
  margin-top: 22px;
}

.section-heading {
  display: flex;
  padding-bottom: 10px;
  align-items: start;
  gap: 9px;
  color: var(--accent-strong);
}

.section-heading h2 {
  margin: 0;
  color: var(--text);
  font-size: 13px;
}

.section-heading p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 10px;
}

.setting-row {
  display: grid;
  min-height: 68px;
  padding: 12px 2px;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 24px;
  border-top: 1px solid var(--border-soft);
}

.setting-row label {
  color: var(--text);
  font-size: 11px;
  font-weight: 600;
}

.setting-row p {
  margin: 5px 0 0;
  color: var(--text-muted);
  font-size: 10px;
  line-height: 1.5;
}

.setting-row > input {
  width: 112px;
  height: 30px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
}

.switch-control {
  position: relative;
  display: inline-flex;
  width: 36px;
  height: 20px;
}

.switch-control input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
}

.switch-control span {
  position: relative;
  width: 36px;
  height: 20px;
  border-radius: 10px;
  background: var(--border-strong);
  transition: background 120ms ease;
}

.switch-control span::after {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--surface);
  box-shadow: var(--shadow-active);
  content: '';
  transition: transform 120ms ease;
}

.switch-control input:checked + span {
  background: var(--accent-strong);
}

.switch-control input:checked + span::after {
  transform: translateX(16px);
}

.switch-control input:focus-visible + span {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.switch-control input:disabled + span,
.setting-row > input:disabled {
  opacity: 0.5;
}

.settings-note {
  display: flex;
  margin-top: 18px;
  padding: 10px 12px;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text-secondary);
  font-size: 10px;
}

@media (max-width: 720px) {
  .settings-header {
    align-items: start;
    flex-direction: column;
    gap: 0;
  }

  .settings-header p {
    margin-bottom: 8px;
  }

  .settings-content {
    padding: 16px;
  }
}
</style>
