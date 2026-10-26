<script setup lang="ts">
import { computed } from 'vue'
import { CheckCircle2, KeyRound, Play, Smartphone, Square } from '@lucide/vue'
import QrcodeVue from 'qrcode.vue'
import type { CaStatus, ProxyStatus } from '../../shared/types'

const { proxyStatus, port, allowLan, lanIp, caStatus, busy } = defineProps<{
  proxyStatus: ProxyStatus
  port: number
  allowLan: boolean
  lanIp: string | null
  caStatus: CaStatus
  busy: boolean
}>()

const emit = defineEmits<{
  updatePort: [value: number]
  updateAllowLan: [value: boolean]
  toggleProxy: []
  generateCa: []
}>()

const address = computed(() => proxyStatus.lanIp ?? lanIp)
const connectionAddress = computed(() => address.value ? `${address.value}:${port}` : null)
const certificateUrl = computed(() => allowLan && address.value && caStatus.generated
  ? `http://${address.value}:${port}/__netcat/ca.crt`
  : null)
const isRunning = computed(() => proxyStatus.state === 'running')
const isTransitioning = computed(() => proxyStatus.state === 'starting' || proxyStatus.state === 'stopping')
const actionLabel = computed(() => {
  if (proxyStatus.state === 'starting') {
    return '正在启动'
  }
  if (proxyStatus.state === 'stopping') {
    return '正在停止'
  }
  return isRunning.value ? '停止代理' : '启动代理'
})

function updatePort(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isInteger(value) && value >= 1 && value <= 65535) {
    emit('updatePort', value)
  }
}
</script>

<template>
  <section class="mobile-panel" aria-labelledby="mobile-connection-title">
    <div class="panel-heading">
      <Smartphone :size="18" />
      <div>
        <h2 id="mobile-connection-title">手机连接</h2>
        <p>在同一 Wi-Fi 下连接手机，集中完成代理地址与 HTTPS 证书配置。</p>
      </div>
    </div>

    <div class="control-row">
      <div>
        <label for="allow-lan">允许手机连接此代理</label>
        <p>开启后，手机可通过电脑 IP 和端口接入并抓取 HTTP/HTTPS 请求。</p>
      </div>
      <label class="port-control" for="mobile-proxy-port">
        <span>代理端口</span>
        <input
          id="mobile-proxy-port"
          :value="port"
          type="number"
          min="1"
          max="65535"
          :disabled="isRunning"
          @change="updatePort"
        />
      </label>
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

    <div v-if="!allowLan" class="disabled-state">
      <Smartphone :size="18" />
      <p>开启后，这里会显示手机代理地址、启动操作和证书二维码。</p>
    </div>

    <div v-else class="connection-workspace">
      <div class="connection-summary">
        <div class="connection-status">
          <span :class="['status-dot', { active: isRunning }]" />
          <div>
            <strong>{{ isRunning ? '代理已就绪' : '等待启动代理' }}</strong>
            <p>{{ isRunning ? '手机现在可以连接此代理。' : '完成下方准备后启动代理。' }}</p>
          </div>
        </div>

        <div class="address-row">
          <div>
            <span>手机代理地址</span>
            <code>{{ connectionAddress ?? '正在获取局域网地址' }}</code>
          </div>
        </div>

        <div v-if="!caStatus.generated" class="preparation-row">
          <KeyRound :size="16" />
          <div>
            <strong>先生成 HTTPS 证书</strong>
            <p>代理需要本地 CA 才能启动并提供手机证书下载。</p>
          </div>
          <button type="button" class="secondary-action" :disabled="busy" @click="emit('generateCa')">
            生成证书
          </button>
        </div>

        <button
          type="button"
          class="proxy-action"
          :class="{ running: isRunning }"
          :disabled="busy || isTransitioning || !caStatus.generated || !connectionAddress"
          @click="emit('toggleProxy')"
        >
          <Square v-if="isRunning" :size="13" />
          <Play v-else :size="14" />
          {{ actionLabel }}
        </button>
      </div>

      <div class="certificate-access">
        <div v-if="certificateUrl" class="qr-code" aria-label="CA 证书下载二维码">
          <QrcodeVue :value="certificateUrl" :size="120" level="M" render-as="svg" />
        </div>
        <div v-else class="qr-placeholder">
          <KeyRound :size="24" />
          <span>证书准备后显示二维码</span>
        </div>
        <div class="certificate-copy">
          <span>手机 HTTPS 证书</span>
          <strong>{{ caStatus.generated ? '扫码安装 CA 证书' : '等待生成 CA 证书' }}</strong>
          <code v-if="certificateUrl">{{ certificateUrl }}</code>
          <p v-if="!isRunning">二维码可提前查看，启动代理后手机才能下载。</p>
          <p v-else><CheckCircle2 :size="13" />证书下载地址已可访问。</p>
        </div>
      </div>

      <ol class="connection-steps">
        <li><span>1</span>手机与电脑连接同一 Wi-Fi</li>
        <li><span>2</span>将 Wi-Fi 代理设为上方地址</li>
        <li><span>3</span>扫码安装并信任 HTTPS 证书</li>
      </ol>
    </div>
  </section>
</template>

<style scoped>
.mobile-panel {
  border-bottom: 1px solid var(--border);
}

.panel-heading {
  display: flex;
  padding-bottom: 12px;
  align-items: start;
  gap: 9px;
  color: var(--accent-strong);
}

.panel-heading h2 {
  margin: 0;
  color: var(--text);
  font-size: 13px;
}

.panel-heading p,
.control-row p,
.connection-status p,
.preparation-row p,
.certificate-copy p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 10px;
  line-height: 1.5;
}

.control-row {
  display: grid;
  min-height: 68px;
  padding: 12px 2px;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 24px;
  border-top: 1px solid var(--border-soft);
}

.control-row label {
  color: var(--text);
  font-size: 11px;
  font-weight: 600;
}

.port-control {
  display: grid;
  gap: 5px;
}

.port-control span {
  color: var(--text-muted);
  font-size: 9px;
}

.port-control input {
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

.port-control input:disabled {
  opacity: 0.5;
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

.switch-control input:disabled + span {
  opacity: 0.5;
}

.disabled-state {
  display: flex;
  min-height: 68px;
  padding: 14px;
  align-items: center;
  gap: 9px;
  border-top: 1px solid var(--border-soft);
  color: var(--text-muted);
}

.disabled-state p {
  margin: 0;
  font-size: 10px;
}

.connection-workspace {
  padding: 16px 0 18px;
  border-top: 1px solid var(--border-soft);
}

.connection-summary {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 14px 18px;
}

.connection-status {
  display: flex;
  align-items: center;
  gap: 9px;
}

.connection-status strong,
.preparation-row strong,
.certificate-copy strong {
  color: var(--text);
  font-size: 11px;
}

.status-dot {
  width: 8px;
  height: 8px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: var(--warning);
  box-shadow: 0 0 0 3px var(--warning-soft);
}

.status-dot.active {
  background: var(--mock);
  box-shadow: 0 0 0 3px var(--mock-soft);
}

.address-row {
  display: grid;
  grid-column: 1 / -1;
  padding: 12px 0;
  grid-template-columns: minmax(0, 1fr);
  align-items: center;
  gap: 12px;
  border-top: 1px solid var(--border-soft);
  border-bottom: 1px solid var(--border-soft);
}

.address-row span,
.certificate-copy > span {
  color: var(--text-muted);
  font-size: 9px;
  font-weight: 600;
}

.address-row code {
  display: block;
  margin-top: 4px;
  color: var(--accent-strong);
  font-family: var(--font-mono);
  font-size: 12px;
}

.preparation-row {
  display: grid;
  grid-column: 1 / -1;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  color: var(--warning);
}

.proxy-action,
.secondary-action {
  display: inline-flex;
  height: 32px;
  padding: 0 12px;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border-radius: var(--radius-control);
  font-family: inherit;
  font-size: 11px;
  font-weight: 600;
}

.proxy-action {
  grid-column: 2;
  grid-row: 1;
  border: 1px solid var(--accent-strong);
  background: var(--accent);
  color: var(--on-accent);
}

.proxy-action.running {
  border-color: var(--border-strong);
  background: var(--surface-raised);
  color: var(--text-secondary);
}

.secondary-action {
  border: 1px solid var(--accent-strong);
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.proxy-action:disabled,
.secondary-action:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.certificate-access {
  display: grid;
  margin-top: 16px;
  grid-template-columns: 136px minmax(0, 1fr);
  align-items: center;
  gap: 16px;
}

.qr-code,
.qr-placeholder {
  display: grid;
  width: 136px;
  height: 136px;
  place-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
}

.qr-code {
  background: var(--qr-surface);
}

.qr-placeholder {
  align-content: center;
  gap: 8px;
  background: var(--surface-raised);
  color: var(--text-muted);
}

.qr-placeholder span {
  max-width: 100px;
  text-align: center;
  font-size: 9px;
  line-height: 1.4;
}

.certificate-copy {
  min-width: 0;
}

.certificate-copy strong {
  display: block;
  margin-top: 5px;
}

.certificate-copy code {
  display: block;
  margin-top: 9px;
  overflow-wrap: anywhere;
  color: var(--accent-strong);
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 1.5;
}

.certificate-copy p {
  display: flex;
  align-items: center;
  gap: 5px;
}

.connection-steps {
  display: grid;
  margin: 16px 0 0;
  padding: 14px 0 0;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  border-top: 1px solid var(--border-soft);
  list-style: none;
}

.connection-steps li {
  display: flex;
  align-items: center;
  gap: 7px;
  color: var(--text-secondary);
  font-size: 10px;
  line-height: 1.4;
}

.connection-steps span {
  display: grid;
  width: 20px;
  height: 20px;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid var(--border-strong);
  border-radius: 50%;
  color: var(--accent-strong);
  font-family: var(--font-mono);
  font-size: 9px;
}

@media (max-width: 620px) {
  .control-row {
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 14px;
  }

  .control-row > div:first-child {
    grid-column: 1 / -1;
  }

  .port-control {
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
  }

  .connection-summary,
  .certificate-access,
  .connection-steps {
    grid-template-columns: 1fr;
  }

  .proxy-action {
    grid-column: 1;
    grid-row: auto;
    width: 100%;
  }

  .address-row {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
