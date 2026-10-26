<script setup lang="ts">
import { Bug, Download, Laptop, Pause, Router, Settings2, ShieldCheck } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import type { CaStatus, CaPrivateKeyState, CertificateTrustState, PlatformInfo, ProxyStatus, SystemProxyStatus } from '../../shared/types'
import CertificatePanel from '../certificate/CertificatePanel.vue'
import MobileConnectionPanel from './MobileConnectionPanel.vue'

const {
  platform,
  trustState,
  privateKeyState,
  systemProxyStatus,
  proxyStatus,
  port,
  allowLan,
  lanIp = null,
  captureEnabled,
  systemProxyPreferred,
  systemProxyEnabled,
  caStatus,
  busy
} = defineProps<{
  platform: PlatformInfo | null
  trustState: CertificateTrustState
  privateKeyState: CaPrivateKeyState
  systemProxyStatus: SystemProxyStatus
  proxyStatus: ProxyStatus
  port: number
  allowLan: boolean
  lanIp?: string | null
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
  toggleProxy: []
  restoreSystemProxy: []
  generateCa: []
  exportCa: []
  trustCa: []
  removeCaTrust: []
  openDiagnostics: []
  exportDiagnostics: []
}>()

const activeTab = shallowRef<'proxy' | 'certificate' | 'system'>('proxy')
const proxyRecoveryNeeded = computed(() => systemProxyStatus.conflict ||
  (systemProxyStatus.recoveryPending && (!systemProxyStatus.managed || proxyStatus.state !== 'running')))
const systemProxyChecked = computed(() => {
  if (proxyRecoveryNeeded.value) {
    return false
  }
  return proxyStatus.state === 'running' ? systemProxyEnabled && systemProxyStatus.managed : systemProxyPreferred
})

function updateSystemProxy(event: Event) {
  const input = event.target as HTMLInputElement
  const requested = input.checked
  // Native clicks change the DOM before the parent confirms the OS operation.
  input.checked = systemProxyChecked.value
  emit('updateSystemProxy', requested)
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
        <button
          type="button"
          role="tab"
          :aria-selected="activeTab === 'system'"
          :tabindex="activeTab === 'system' ? 0 : -1"
          @click="activeTab = 'system'"
        >
          <Settings2 :size="14" />
          系统配置
        </button>
      </div>
    </header>

    <div v-if="activeTab === 'proxy'" class="settings-content" role="tabpanel">
      <MobileConnectionPanel
        :proxy-status="proxyStatus"
        :port="port"
        :allow-lan="allowLan"
        :lan-ip="lanIp"
        :ca-status="caStatus"
        :busy="busy"
        @update-port="emit('updatePort', $event)"
        @update-allow-lan="emit('updateAllowLan', $event)"
        @toggle-proxy="emit('toggleProxy')"
        @generate-ca="emit('generateCa')"
      />

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
            <p v-if="proxyRecoveryNeeded">系统代理需要恢复，请查看下方提示。</p>
            <p v-else-if="proxyStatus.state === 'running'">
              {{ systemProxyEnabled ? '本机 HTTP/HTTPS 流量已接入。' : '当前未修改本机系统代理。' }}
            </p>
            <p v-else>开启后将在代理启动时自动接入，停止时恢复原配置。</p>
          </div>
          <label class="switch-control">
            <input
              id="system-proxy"
              type="checkbox"
              :checked="systemProxyChecked"
              :disabled="busy || proxyRecoveryNeeded"
              @change="updateSystemProxy"
            />
            <span aria-hidden="true" />
          </label>
        </div>

        <div v-if="proxyRecoveryNeeded" class="setting-row" role="status">
          <div>
            <span class="setting-label">系统代理恢复</span>
            <p v-if="systemProxyStatus.conflict">系统代理已被其他程序或用户修改，请检查系统代理设置后重试恢复。</p>
            <p v-else>上次系统代理配置尚未恢复，请恢复原配置后继续。</p>
          </div>
          <button type="button" class="debug-button" :disabled="busy" @click="emit('restoreSystemProxy')">
            {{ systemProxyStatus.conflict ? '重试恢复系统代理' : '恢复系统代理' }}
          </button>
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

    <div v-else-if="activeTab === 'certificate'" class="certificate-content" role="tabpanel">
      <CertificatePanel
        embedded
        :platform="platform"
        :trust-state="trustState"
        :private-key-state="privateKeyState"
        :ca-status="caStatus"
        :busy="busy"
        @generate="emit('generateCa')"
        @export="emit('exportCa')"
        @trust="emit('trustCa')"
        @remove-trust="emit('removeCaTrust')"
      />
    </div>

    <div v-else class="settings-content" role="tabpanel">
      <section class="settings-section">
        <div class="section-heading">
          <Bug :size="17" />
          <div>
            <h2>调试工具</h2>
            <p>查看代理、Mock 与请求重发的实时运行日志。</p>
          </div>
        </div>

        <div class="setting-row">
          <div>
            <span class="setting-label">应用调试面板</span>
            <p>在应用右侧查看实时诊断日志和本地网络请求。</p>
          </div>
          <button
            type="button"
            class="debug-button"
            :disabled="busy"
            @click="emit('openDiagnostics')"
          >
            <Bug :size="14" />
            打开调试面板
          </button>
        </div>
        <div class="setting-row">
          <div>
            <span class="setting-label">诊断包</span>
            <p>导出版本、代理、CA 公钥状态、数据库检查和脱敏日志。</p>
          </div>
          <button
            type="button"
            class="debug-button"
            :disabled="busy"
            @click="emit('exportDiagnostics')"
          >
            <Download :size="14" />
            导出诊断包
          </button>
        </div>
      </section>

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
  font-size: var(--font-size-2xl);
}

.settings-header p {
  margin: 5px 0 16px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
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
  font-size: var(--font-size-sm);
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

.mobile-panel + .settings-section {
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
  font-size: var(--font-size-lg);
}

.section-heading p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
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
  font-size: var(--font-size-sm);
  font-weight: 600;
}

.setting-label {
  color: var(--text);
  font-size: var(--font-size-sm);
  font-weight: 600;
}

.debug-button {
  display: inline-flex;
  height: 32px;
  padding: 0 12px;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--accent-strong);
  border-radius: var(--radius-control);
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-family: inherit;
  font-size: var(--font-size-sm);
  font-weight: 600;
}

.debug-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.setting-row p {
  margin: 5px 0 0;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
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
  font-size: var(--font-size-sm);
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
  font-size: var(--font-size-xs);
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
