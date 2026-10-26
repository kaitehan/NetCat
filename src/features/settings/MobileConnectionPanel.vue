<script setup lang="ts">
import { computed } from 'vue'
import { KeyRound, Laptop, Pencil, Play, RefreshCw, Smartphone, Square, Trash2, Unplug } from '@lucide/vue'
import QrcodeVue from 'qrcode.vue'
import type { CaStatus, HistoricalDevice, PairedDevice, PairingChallenge, ProxyMode, ProxyStatus } from '../../shared/types'

const props = defineProps<{
  proxyStatus: ProxyStatus
  proxyMode: ProxyMode
  port: number
  lanIp: string | null
  caStatus: CaStatus
  pairedDevices: readonly PairedDevice[]
  historicalDevices: readonly HistoricalDevice[]
  pairingIdleTimeout: number
  pairingChallenge: PairingChallenge | null
  busy: boolean
}>()

const emit = defineEmits<{
  updatePort: [value: number]
  updateProxyMode: [value: ProxyMode]
  updatePairingIdleTimeout: [value: number]
  toggleProxy: []
  generateCa: []
  createPairingChallenge: []
  renameDevice: [deviceToken: string, name: string]
  rePairDevice: [deviceToken: string]
  disconnectDevice: [deviceToken: string]
  deleteDevice: [deviceToken: string]
}>()

const address = computed(() => props.proxyStatus.lanIp ?? props.lanIp)
const connectionAddress = computed(() => address.value ? `${address.value}:${props.port}` : null)
const isRunning = computed(() => props.proxyStatus.state === 'running')
const isTransitioning = computed(() => props.proxyStatus.state === 'starting' || props.proxyStatus.state === 'stopping')
const mobileMode = computed(() => props.proxyMode === 'mobile')
const deviceRows = computed(() => [
  ...props.pairedDevices.map((device) => ({
    deviceToken: device.deviceToken,
    name: device.name,
    detail: device.boundIp,
    online: true
  })),
  ...props.historicalDevices
    .filter((device) => !props.pairedDevices.some((online) => online.deviceToken === device.deviceToken))
    .map((device) => ({
      deviceToken: device.deviceToken,
      name: device.name,
      detail: `${device.requestCount} 条历史请求`,
      online: false
    }))
])
const actionLabel = computed(() => {
  if (props.proxyStatus.state === 'starting') return '正在启动'
  if (props.proxyStatus.state === 'stopping') return '正在停止'
  return isRunning.value ? '停止代理' : '启动代理'
})

function updatePort(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isInteger(value) && value >= 1 && value <= 65535) emit('updatePort', value)
}

function updateTimeout(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isInteger(value) && value >= 5 && value <= 120) emit('updatePairingIdleTimeout', value)
}

function rename(device: { deviceToken: string; name: string }) {
  const name = window.prompt('设备名称', device.name)?.trim()
  if (name && name !== device.name) emit('renameDevice', device.deviceToken, name)
}
</script>

<template>
  <section class="mobile-panel" aria-labelledby="mobile-connection-title">
    <div class="panel-heading">
      <Smartphone :size="18" />
      <div>
        <h2 id="mobile-connection-title">手机连接</h2>
        <p>电脑与手机抓包互斥；手机模式只接收已配对设备的请求。</p>
      </div>
    </div>

    <fieldset class="mode-control" :disabled="isRunning || isTransitioning">
      <legend>抓包来源</legend>
      <label :class="{ active: proxyMode === 'computer' }">
        <input aria-label="电脑抓包" type="radio" name="proxy-mode" value="computer" :checked="proxyMode === 'computer'" @change="emit('updateProxyMode', 'computer')" />
        <Laptop :size="15" />
        <span><strong>电脑抓包</strong><small>仅监听本机请求</small></span>
      </label>
      <label :class="{ active: proxyMode === 'mobile' }">
        <input aria-label="手机抓包" type="radio" name="proxy-mode" value="mobile" :checked="proxyMode === 'mobile'" @change="emit('updateProxyMode', 'mobile')" />
        <Smartphone :size="15" />
        <span><strong>手机抓包</strong><small>仅监听已配对手机</small></span>
      </label>
    </fieldset>

    <div class="control-row">
      <label class="number-control" for="mobile-proxy-port">
        <span>代理端口</span>
        <input id="mobile-proxy-port" :value="port" type="number" min="1" max="65535" :disabled="isRunning" @change="updatePort" />
      </label>
      <label class="number-control" for="pairing-idle-timeout">
        <span>空闲断开时间</span>
        <input id="pairing-idle-timeout" aria-label="空闲断开时间" :value="pairingIdleTimeout" type="number" min="5" max="120" :disabled="busy" @change="updateTimeout" />
        <small>分钟</small>
      </label>
      <button type="button" class="proxy-action" :class="{ running: isRunning }" :disabled="busy || isTransitioning || !caStatus.generated || (mobileMode && !connectionAddress)" @click="emit('toggleProxy')">
        <Square v-if="isRunning" :size="13" />
        <Play v-else :size="14" />
        {{ actionLabel }}
      </button>
    </div>

    <div v-if="!caStatus.generated" class="preparation-row">
      <KeyRound :size="16" />
      <div>
        <strong>先生成 HTTPS 证书</strong>
        <p>代理需要本地 CA 才能启动。</p>
      </div>
      <button type="button" class="secondary-action" :disabled="busy" @click="emit('generateCa')">生成证书</button>
    </div>

    <div v-if="mobileMode" class="mobile-workspace">
      <div class="pairing-access">
        <div v-if="pairingChallenge" class="qr-code" aria-label="手机配对二维码">
          <QrcodeVue :value="pairingChallenge.url" :size="120" level="M" render-as="svg" />
        </div>
        <div v-else class="qr-placeholder">
          <Smartphone :size="24" />
          <span>{{ isRunning ? '生成二维码后扫码配对' : '启动代理后可配对' }}</span>
        </div>
        <div class="pairing-copy">
          <span>一次性设备配对</span>
          <strong>{{ connectionAddress ?? '等待局域网地址' }}</strong>
          <code v-if="pairingChallenge">{{ pairingChallenge.url }}</code>
          <p>二维码 5 分钟内有效，使用一次后立即失效。</p>
          <button type="button" class="secondary-action" :disabled="busy || !isRunning" @click="emit('createPairingChallenge')">
            <RefreshCw :size="13" />
            {{ pairingChallenge ? '刷新二维码' : '生成配对二维码' }}
          </button>
        </div>
      </div>

      <div class="device-list">
        <div class="device-list-heading">
          <strong>设备</strong>
          <span>{{ pairedDevices.length }} 在线</span>
        </div>
        <p v-if="!deviceRows.length" class="empty-devices">暂无设备</p>
        <article v-for="device in deviceRows" :key="device.deviceToken" class="device-row">
          <span :class="['online-dot', { offline: !device.online }]" :aria-label="device.online ? '在线' : '已断开'" />
          <div>
            <strong>{{ device.name }}</strong>
            <small>{{ device.detail }}</small>
          </div>
          <div class="device-actions">
            <button type="button" title="改名" :disabled="busy" @click="rename(device)"><Pencil :size="13" /></button>
            <button type="button" title="重新配对" :disabled="busy || !isRunning" @click="emit('rePairDevice', device.deviceToken)"><RefreshCw :size="13" /></button>
            <button v-if="device.online" type="button" title="断开设备" :disabled="busy" @click="emit('disconnectDevice', device.deviceToken)"><Unplug :size="13" /></button>
            <button type="button" title="删除设备数据" :disabled="busy" @click="emit('deleteDevice', device.deviceToken)"><Trash2 :size="13" /></button>
          </div>
        </article>
      </div>
    </div>

    <p v-else class="computer-note">电脑模式仅监听 127.0.0.1，不接受局域网设备连接。</p>
  </section>
</template>

<style scoped>
.mobile-panel {
  padding-bottom: 14px;
  border-bottom: 1px solid var(--border);
}

.panel-heading {
  display: flex;
  padding-bottom: 12px;
  align-items: start;
  gap: 9px;
  color: var(--accent-strong);
}

.panel-heading h2,
.device-list-heading strong {
  margin: 0;
  color: var(--text);
  font-size: 13px;
}

.panel-heading p,
.preparation-row p,
.pairing-copy p,
.computer-note {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 10px;
  line-height: 1.5;
}

.mode-control {
  display: grid;
  margin: 0;
  padding: 0 0 12px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  border: 0;
}

.mode-control legend {
  padding: 0 0 7px;
  color: var(--text-muted);
  font-size: 9px;
}

.mode-control label {
  display: flex;
  min-height: 52px;
  padding: 8px 10px;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  color: var(--text-muted);
  cursor: pointer;
}

.mode-control label.active {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.mode-control input {
  position: absolute;
  opacity: 0;
}

.mode-control span {
  display: grid;
  gap: 2px;
}

.mode-control strong {
  color: var(--text);
  font-size: 11px;
}

.mode-control small,
.number-control small,
.device-row small {
  color: var(--text-muted);
  font-size: 9px;
}

.control-row {
  display: grid;
  padding: 12px 0;
  grid-template-columns: auto auto minmax(130px, 1fr);
  align-items: end;
  gap: 14px;
  border-top: 1px solid var(--border-soft);
}

.number-control {
  display: grid;
  grid-template-columns: auto auto;
  align-items: center;
  gap: 5px;
  color: var(--text-muted);
  font-size: 9px;
}

.number-control span {
  grid-column: 1 / -1;
}

.number-control input {
  width: 92px;
  height: 30px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
}

.proxy-action,
.secondary-action {
  display: inline-flex;
  height: 30px;
  padding: 0 12px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid var(--accent);
  border-radius: var(--radius-control);
  background: var(--accent);
  color: white;
  font-family: inherit;
  font-size: 10px;
  font-weight: 650;
}

.proxy-action.running {
  border-color: var(--danger-border);
  background: var(--danger-soft);
  color: var(--danger);
}

.secondary-action {
  background: transparent;
  color: var(--accent-strong);
}

button:disabled,
input:disabled {
  cursor: default;
  opacity: 0.5;
}

.preparation-row {
  display: grid;
  padding: 10px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
}

.preparation-row strong,
.pairing-copy strong,
.device-row strong {
  color: var(--text);
  font-size: 10px;
}

.mobile-workspace {
  display: grid;
  margin-top: 4px;
  grid-template-columns: minmax(0, 1fr) minmax(260px, 1fr);
  gap: 12px;
}

.pairing-access,
.device-list {
  min-width: 0;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
}

.pairing-access {
  display: flex;
  align-items: center;
  gap: 14px;
}

.qr-code,
.qr-placeholder {
  display: grid;
  width: 128px;
  height: 128px;
  flex: 0 0 128px;
  place-items: center;
  border: 1px solid var(--border);
  background: white;
}

.qr-placeholder {
  padding: 14px;
  background: var(--surface);
  color: var(--text-muted);
  text-align: center;
  font-size: 9px;
}

.pairing-copy {
  display: grid;
  min-width: 0;
  gap: 6px;
}

.pairing-copy > span {
  color: var(--text-muted);
  font-size: 9px;
}

.pairing-copy code {
  overflow: hidden;
  color: var(--text-secondary);
  font-size: 8px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device-list-heading {
  display: flex;
  min-height: 26px;
  align-items: center;
  justify-content: space-between;
}

.device-list-heading span {
  color: var(--text-muted);
  font-size: 9px;
}

.empty-devices {
  margin: 22px 0;
  color: var(--text-muted);
  text-align: center;
  font-size: 10px;
}

.device-row {
  display: grid;
  min-height: 42px;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  border-top: 1px solid var(--border-soft);
}

.device-row > div:nth-child(2) {
  display: grid;
  gap: 2px;
}

.online-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--success);
}

.online-dot.offline {
  background: var(--text-muted);
}

.device-actions {
  display: flex;
  gap: 2px;
}

.device-actions button {
  display: grid;
  width: 27px;
  height: 27px;
  padding: 0;
  place-items: center;
  border: 0;
  border-radius: var(--radius-control);
  background: transparent;
  color: var(--text-muted);
}

.device-actions button:hover {
  background: var(--surface);
  color: var(--text);
}

.computer-note {
  padding: 10px 0 0;
}

@media (max-width: 900px) {
  .mobile-workspace {
    grid-template-columns: 1fr;
  }
}
</style>
