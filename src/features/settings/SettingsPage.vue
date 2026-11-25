<script setup lang="ts">
import { Bot, Bug, CheckCircle2, Download, KeyRound, Laptop, Pause, RotateCcw, Router, Settings2, ShieldCheck } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import { desktopApi, normalizeDesktopError, type DesktopApi } from '../../api/desktop'
import type { AssistantCredentialDescription, AssistantProvider, AssistantSettings, CaStatus, CaPrivateKeyState, CertificateTrustState, HistoricalDevice, PairedDevice, PairingChallenge, PlatformInfo, ProxyMode, ProxyStatus, SystemProxyStatus } from '../../shared/types'
import CertificatePanel from '../certificate/CertificatePanel.vue'
import MobileConnectionPanel from './MobileConnectionPanel.vue'

const {
  platform,
  trustState,
  privateKeyState,
  systemProxyStatus,
  proxyStatus,
  port,
  proxyMode,
  pairedDevices,
  historicalDevices,
  pairingIdleTimeout,
  pairingChallenge,
  lanIp = null,
  captureEnabled,
  systemProxyPreferred,
  systemProxyEnabled,
  caStatus,
  busy,
  assistantApi
} = defineProps<{
  platform: PlatformInfo | null
  trustState: CertificateTrustState
  privateKeyState: CaPrivateKeyState
  systemProxyStatus: SystemProxyStatus
  proxyStatus: ProxyStatus
  port: number
  proxyMode: ProxyMode
  pairedDevices: readonly PairedDevice[]
  historicalDevices: readonly HistoricalDevice[]
  pairingIdleTimeout: number
  pairingChallenge: PairingChallenge | null
  lanIp?: string | null
  captureEnabled: boolean
  systemProxyPreferred: boolean
  systemProxyEnabled: boolean
  caStatus: CaStatus
  busy: boolean
  assistantApi?: Pick<DesktopApi,
    | 'getAssistantSettings'
    | 'saveAssistantSettings'
    | 'describeAssistantCredential'
    | 'saveAssistantCredential'
    | 'deleteAssistantCredential'
    | 'validateAssistantCredential'
    | 'testAssistantConnection'
    | 'restartAssistant'
  >
}>()

const emit = defineEmits<{
  updatePort: [value: number]
  updateProxyMode: [value: ProxyMode]
  updatePairingIdleTimeout: [value: number]
  updateCapture: [value: boolean]
  updateSystemProxy: [value: boolean]
  toggleProxy: []
  restoreSystemProxy: []
  generateCa: []
  createPairingChallenge: []
  renameDevice: [deviceToken: string, name: string]
  rePairDevice: [deviceToken: string]
  disconnectDevice: [deviceToken: string]
  deleteDevice: [deviceToken: string]
  exportCa: []
  trustCa: []
  removeCaTrust: []
  openDiagnostics: []
  exportDiagnostics: []
}>()

const activeTab = shallowRef<'proxy' | 'certificate' | 'system' | 'assistant'>('proxy')
const assistantClient = assistantApi ?? desktopApi
const assistantSettings = shallowRef<AssistantSettings | null>(null)
const assistantCredential = shallowRef<AssistantCredentialDescription | null>(null)
const assistantKey = shallowRef('')
const assistantBusy = shallowRef(false)
const assistantError = shallowRef<string | null>(null)
const assistantValidation = shallowRef<string | null>(null)
const openAiProvider = shallowRef('openai')
const openAiModel = shallowRef('gpt-4.1')
const openAiBaseUrl = shallowRef('https://api.openai.com/v1')
const openAiApi = shallowRef<AssistantSettings['openAiApi']>('openai-responses')
const openAiConfigurationDirty = computed(() => {
  const settings = assistantSettings.value
  if (!settings || settings.provider !== 'openAi') return false
  return settings.openAiProvider !== openAiProvider.value.trim() ||
    settings.model !== `${openAiProvider.value.trim()}/${openAiModel.value.trim()}` ||
    settings.openAiBaseUrl !== openAiBaseUrl.value.trim() ||
    settings.openAiApi !== openAiApi.value
})
const proxyRecoveryNeeded = computed(() => systemProxyStatus.conflict ||
  (systemProxyStatus.recoveryPending && (!systemProxyStatus.managed || proxyStatus.state !== 'running')))
const systemProxyChecked = computed(() => {
  if (proxyMode === 'mobile') {
    return false
  }
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

function renameDevice(deviceToken: string, name: string) {
  emit('renameDevice', deviceToken, name)
}

async function openAssistantSettings() {
  activeTab.value = 'assistant'
  assistantBusy.value = true
  assistantError.value = null
  try {
    assistantSettings.value = await assistantClient.getAssistantSettings()
    syncOpenAiForm(assistantSettings.value)
    assistantCredential.value = await assistantClient.describeAssistantCredential(assistantSettings.value.provider)
  } catch (cause) {
    assistantError.value = normalizeDesktopError(cause).message
  } finally {
    assistantBusy.value = false
  }
}

async function selectAssistantProvider(provider: AssistantProvider) {
  assistantError.value = null
  const model = provider === 'deepSeek'
    ? 'deepseek-official/deepseek-v4-flash'
    : `${openAiProvider.value}/${openAiModel.value}`
  if (provider === 'deepSeek') {
    assistantSettings.value = await assistantClient.saveAssistantSettings(provider, model)
  } else if (assistantSettings.value) {
    assistantSettings.value = { ...assistantSettings.value, provider, model }
  }
  assistantCredential.value = await assistantClient.describeAssistantCredential(provider)
  assistantKey.value = ''
}

function syncOpenAiForm(settings: AssistantSettings) {
  openAiProvider.value = settings.openAiProvider
  openAiBaseUrl.value = settings.openAiBaseUrl
  openAiApi.value = settings.openAiApi
  if (settings.provider !== 'openAi') {
    openAiModel.value = 'gpt-4.1'
    return
  }
  const prefix = `${settings.openAiProvider}/`
  openAiModel.value = settings.model.startsWith(prefix)
    ? settings.model.slice(prefix.length)
    : (settings.model.split('/').slice(1).join('/') || 'gpt-4.1')
}

async function saveAssistantModel() {
  if (assistantSettings.value?.provider !== 'openAi') return
  assistantBusy.value = true
  assistantError.value = null
  assistantValidation.value = null
  try {
    const route = openAiProvider.value.trim()
    const model = openAiModel.value.trim()
    assistantSettings.value = await assistantClient.saveAssistantSettings(
      'openAi',
      `${route}/${model}`,
      route,
      openAiBaseUrl.value.trim(),
      openAiApi.value
    )
    syncOpenAiForm(assistantSettings.value)
    assistantCredential.value = await assistantClient.describeAssistantCredential('openAi')
    assistantKey.value = ''
    assistantValidation.value = '模型配置已保存并应用到 Runtime。'
  } catch (cause) {
    assistantError.value = normalizeDesktopError(cause).message
  } finally {
    assistantBusy.value = false
  }
}

async function saveAssistantKey() {
  const provider = assistantSettings.value?.provider
  if (!provider) return
  assistantBusy.value = true
  assistantError.value = null
  assistantValidation.value = null
  try {
    assistantCredential.value = await assistantClient.saveAssistantCredential(provider, assistantKey.value)
    assistantKey.value = ''
    assistantValidation.value = 'API Key 已保存到系统安全存储。'
  } catch (cause) {
    assistantError.value = normalizeDesktopError(cause).message
  } finally {
    assistantBusy.value = false
  }
}

async function validateAssistantKey() {
  const provider = assistantSettings.value?.provider
  if (!provider) return
  assistantBusy.value = true
  assistantError.value = null
  assistantValidation.value = null
  try {
    assistantCredential.value = await assistantClient.validateAssistantCredential(provider)
    assistantValidation.value = '安全存储中的凭据可供 Runtime 使用。'
  } catch (cause) {
    assistantError.value = normalizeDesktopError(cause).message
  } finally {
    assistantBusy.value = false
  }
}

async function testAssistantConnection() {
  assistantBusy.value = true
  assistantError.value = null
  assistantValidation.value = null
  try {
    assistantValidation.value = await assistantClient.testAssistantConnection()
  } catch (cause) {
    const error = normalizeDesktopError(cause)
    assistantError.value = error.action ? `${error.message}；${error.action}` : error.message
  } finally {
    assistantBusy.value = false
  }
}

async function deleteAssistantKey() {
  const provider = assistantSettings.value?.provider
  if (!provider) return
  assistantBusy.value = true
  assistantError.value = null
  assistantValidation.value = null
  try {
    assistantCredential.value = await assistantClient.deleteAssistantCredential(provider)
    assistantValidation.value = 'API Key 已从系统安全存储删除。'
  } catch (cause) {
    assistantError.value = normalizeDesktopError(cause).message
  } finally {
    assistantBusy.value = false
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
        <button
          type="button"
          role="tab"
          :aria-selected="activeTab === 'assistant'"
          :tabindex="activeTab === 'assistant' ? 0 : -1"
          @click="openAssistantSettings"
        >
          <Bot :size="14" />
          智奈配置
        </button>
      </div>
    </header>

    <div v-if="activeTab === 'proxy'" class="settings-content" role="tabpanel">
      <MobileConnectionPanel
        :proxy-status="proxyStatus"
        :proxy-mode="proxyMode"
        :port="port"
        :lan-ip="lanIp"
        :ca-status="caStatus"
        :paired-devices="pairedDevices"
        :historical-devices="historicalDevices"
        :pairing-idle-timeout="pairingIdleTimeout"
        :pairing-challenge="pairingChallenge"
        :busy="busy"
        @update-port="emit('updatePort', $event)"
        @update-proxy-mode="emit('updateProxyMode', $event)"
        @update-pairing-idle-timeout="emit('updatePairingIdleTimeout', $event)"
        @toggle-proxy="emit('toggleProxy')"
        @generate-ca="emit('generateCa')"
        @create-pairing-challenge="emit('createPairingChallenge')"
        @rename-device="renameDevice"
        @re-pair-device="emit('rePairDevice', $event)"
        @disconnect-device="emit('disconnectDevice', $event)"
        @delete-device="emit('deleteDevice', $event)"
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
            <p v-if="proxyMode === 'mobile'">手机模式不会修改系统代理，避免抓取电脑请求。</p>
            <p v-else-if="proxyRecoveryNeeded">系统代理需要恢复，请查看下方提示。</p>
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
              :disabled="busy || proxyRecoveryNeeded || proxyMode === 'mobile'"
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

    <div v-else-if="activeTab === 'system'" class="settings-content" role="tabpanel">
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

    <div v-else class="settings-content assistant-settings" role="tabpanel">
      <div v-if="assistantError" class="assistant-error" role="alert">{{ assistantError }}</div>
      <section class="settings-section">
        <div class="section-heading">
          <Bot :size="17" />
          <div>
            <h2>模型服务</h2>
            <p>明确选择本次智奈会话使用的模型服务。</p>
          </div>
        </div>
        <fieldset class="provider-control" :disabled="assistantBusy">
          <legend>模型服务</legend>
          <label>
            <input
              type="radio"
              name="assistant-provider"
              value="deepSeek"
              aria-label="DeepSeek"
              :checked="assistantSettings?.provider === 'deepSeek'"
              @change="selectAssistantProvider('deepSeek')"
            />
            DeepSeek
          </label>
          <label>
            <input
              type="radio"
              name="assistant-provider"
              value="openAi"
              aria-label="OpenAI"
              :checked="assistantSettings?.provider === 'openAi'"
              @change="selectAssistantProvider('openAi')"
            />
            OpenAI
          </label>
        </fieldset>
        <div v-if="assistantSettings?.provider === 'openAi'" class="openai-compatible-form">
          <label>
            <span>供应商标识</span>
            <input v-model="openAiProvider" aria-label="供应商标识" autocomplete="off" placeholder="openai" />
          </label>
          <label>
            <span>模型标识</span>
            <input v-model="openAiModel" aria-label="模型标识" autocomplete="off" placeholder="gpt-4.1" />
          </label>
          <label class="openai-base-url">
            <span>API Base URL</span>
            <input
              v-model="openAiBaseUrl"
              aria-label="API Base URL"
              autocomplete="off"
              placeholder="https://api.openai.com/v1"
            />
          </label>
          <label>
            <span>API 协议</span>
            <select v-model="openAiApi" aria-label="API 协议">
              <option value="openai-responses">Responses API</option>
              <option value="openai-completions">Chat Completions</option>
            </select>
          </label>
          <button
            type="button"
            class="debug-button"
            :disabled="assistantBusy || !openAiProvider.trim() || !openAiModel.trim() || !openAiBaseUrl.trim()"
            aria-label="保存模型配置"
            @click="saveAssistantModel"
          >
            保存模型配置
          </button>
        </div>
        <div v-else class="assistant-model">{{ assistantSettings?.model ?? '正在读取模型配置' }}</div>
      </section>

      <section class="settings-section">
        <div class="section-heading">
          <KeyRound :size="17" />
          <div>
            <h2>API 凭据</h2>
            <p>{{ assistantCredential?.configured ? '已安全保存，输入框不会回填原值。' : '尚未配置当前服务商的 API Key。' }}</p>
          </div>
        </div>
        <div class="credential-row">
          <input
            v-model="assistantKey"
            type="password"
            :aria-label="`${assistantSettings?.provider === 'openAi' ? 'OpenAI' : 'DeepSeek'} API Key`"
            autocomplete="off"
            placeholder="输入新的 API Key"
          />
          <button
            type="button"
            class="debug-button"
            :disabled="assistantBusy || openAiConfigurationDirty || !assistantKey.trim()"
            @click="saveAssistantKey"
          >
            保存 API Key
          </button>
          <button
            v-if="assistantCredential?.configured"
            type="button"
            class="debug-button danger"
            :disabled="assistantBusy || openAiConfigurationDirty"
            @click="deleteAssistantKey"
          >
            删除
          </button>
          <button
            v-if="assistantCredential?.configured"
            type="button"
            class="debug-button"
            :disabled="assistantBusy || openAiConfigurationDirty"
            @click="validateAssistantKey"
          >
            <CheckCircle2 :size="14" />
            验证凭据
          </button>
          <button
            v-if="assistantSettings?.provider === 'openAi' && assistantCredential?.configured"
            type="button"
            class="debug-button"
            :disabled="assistantBusy || openAiConfigurationDirty"
            @click="testAssistantConnection"
          >
            <CheckCircle2 :size="14" />
            测试连接
          </button>
        </div>
        <p v-if="assistantValidation" class="assistant-validation" role="status">{{ assistantValidation }}</p>
      </section>

      <section class="settings-section">
        <div class="setting-row">
          <div>
            <span class="setting-label">Harness Runtime</span>
            <p>DSH 0.2.3-alpha.2 · Bridge 0.2.3-alpha.2 · Node.js 24.22.3</p>
          </div>
          <button type="button" class="debug-button" @click="assistantClient.restartAssistant()">
            <RotateCcw :size="14" />
            重新启动智奈
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

.assistant-settings {
  gap: 0;
}

.provider-control {
  display: inline-flex;
  margin: 12px 0 10px;
  padding: 3px;
  gap: 3px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
}

.provider-control legend {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
}

.provider-control label {
  position: relative;
  min-width: 92px;
  padding: 6px 10px;
  border-radius: calc(var(--radius-control) - 2px);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  text-align: center;
  cursor: pointer;
}

.provider-control label:has(input:checked) {
  background: var(--surface);
  color: var(--accent-strong);
  box-shadow: var(--shadow-active);
}

.provider-control input {
  position: absolute;
  opacity: 0;
}

.provider-control label:has(input:focus-visible) {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.assistant-model {
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
}

.openai-compatible-form {
  display: grid;
  margin: 2px 0 12px;
  grid-template-columns: minmax(140px, 0.7fr) minmax(180px, 1fr) auto;
  gap: 8px;
}

.openai-compatible-form label {
  display: grid;
  min-width: 0;
  gap: 5px;
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.openai-compatible-form input,
.openai-compatible-form select {
  width: 100%;
  min-width: 0;
  height: 32px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
}

.openai-base-url {
  grid-column: 1 / 3;
}

.openai-compatible-form > .debug-button {
  align-self: end;
}

.credential-row {
  display: grid;
  margin-top: 12px;
  grid-template-columns: minmax(180px, 1fr) repeat(4, auto);
  gap: 8px;
}

.assistant-validation {
  margin: 8px 0 0;
  color: var(--success);
  font-size: var(--font-size-xs);
}

.credential-row input {
  min-width: 0;
  height: 32px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: var(--font-size-sm);
}

.assistant-error {
  padding: 9px 12px;
  border: 1px solid var(--danger-border);
  border-radius: var(--radius-control);
  background: var(--danger-soft);
  color: var(--danger);
  font-size: var(--font-size-sm);
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

  .credential-row {
    grid-template-columns: minmax(0, 1fr);
  }

  .openai-compatible-form {
    grid-template-columns: minmax(0, 1fr);
  }

  .openai-base-url {
    grid-column: auto;
  }
}
</style>
