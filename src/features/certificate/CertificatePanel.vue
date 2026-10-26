<script setup lang="ts">
import { computed } from 'vue'
import { CheckCircle2, Download, KeyRound, Smartphone, X } from '@lucide/vue'
import QrcodeVue from 'qrcode.vue'
import type { CaStatus, CaPrivateKeyState, CertificateTrustState, PlatformInfo, ProxyStatus } from '../../shared/types'

const { open = true, embedded = false, platform, trustState, privateKeyState, caStatus, proxyStatus, allowLan, busy } = defineProps<{
  open?: boolean
  embedded?: boolean
  platform: PlatformInfo | null
  trustState: CertificateTrustState
  privateKeyState: CaPrivateKeyState
  caStatus: CaStatus
  proxyStatus: ProxyStatus
  allowLan: boolean
  busy: boolean
}>()

const emit = defineEmits<{
  close: []
  generate: []
  export: []
  trust: []
  removeTrust: []
}>()

const trustSupported = computed(() => platform?.os === 'windows' && platform.certificateTrustSupported && trustState !== 'unsupported')
const canRegenerate = computed(() => platform?.os === 'windows' && caStatus.generated &&
  trustState !== 'trusted' && (privateKeyState === 'missing' || privateKeyState === 'mismatch'))
const privateKeyMessage = computed(() => {
  if (trustState === 'trusted' && (privateKeyState === 'missing' || privateKeyState === 'mismatch')) {
    return 'CA 私钥缺失或不匹配，请先移除当前 CA 信任，再重新生成本地 CA。'
  }
  return ({
    ready: 'CA 私钥已就绪',
    missing: 'CA 私钥缺失，请重新生成本地 CA。',
    mismatch: 'CA 私钥与公钥证书不匹配，请重新生成本地 CA。',
    unavailable: 'CA 私钥不可用，请检查私钥访问权限。'
  })[privateKeyState]
})
const trustMessage = computed(() => {
  if (!trustSupported.value) {
    return '当前环境不支持自动管理证书信任，请导出证书并手动安装。'
  }
  if (!caStatus.generated) {
    return '请先生成本地 CA'
  }
  const messages: Record<CertificateTrustState, string> = {
    unsupported: '当前环境不支持自动管理证书信任',
    notGenerated: '请先生成本地 CA',
    notTrusted: '当前 CA 尚未信任',
    trusted: '当前 CA 已信任',
    mismatch: '证书指纹不匹配，请检查证书存储中的旧证书后重试。',
    unavailable: '无法读取证书信任状态，请检查当前用户证书存储后重试。'
  }
  return messages[trustState]
})

const certificateUrl = computed(() => {
  if (!allowLan || proxyStatus.state !== 'running' || !proxyStatus.lanIp) {
    return null
  }
  return `http://${proxyStatus.lanIp}:${proxyStatus.port}/__netcat/ca.crt`
})
const healthUrl = computed(() => certificateUrl.value?.replace('/__netcat/ca.crt', '/__netcat/health') ?? null)
</script>

<template>
  <div v-if="open" :class="embedded ? 'embedded-panel' : 'panel-backdrop'" @click.self="!embedded && emit('close')">
    <section
      class="certificate-panel"
      :class="{ embedded }"
      :role="embedded ? 'region' : 'dialog'"
      :aria-modal="embedded ? undefined : 'true'"
      aria-labelledby="certificate-title"
    >
      <header class="panel-header">
        <div>
          <h2 id="certificate-title">HTTPS 证书</h2>
          <p>仅为您授权的测试设备安装并信任此 CA</p>
        </div>
        <button v-if="!embedded" type="button" class="icon-button" title="关闭" @click="emit('close')">
          <X :size="17" />
        </button>
      </header>

      <div class="panel-body">
        <section class="certificate-status">
          <div :class="['status-icon', { ready: caStatus.generated }]">
            <CheckCircle2 v-if="caStatus.generated" :size="24" />
            <KeyRound v-else :size="24" />
          </div>
          <div>
            <strong>{{ caStatus.generated ? '本地 CA 已就绪' : '尚未生成本地 CA' }}</strong>
            <p v-if="caStatus.fingerprint">指纹 {{ caStatus.fingerprint }}</p>
            <p v-else-if="platform?.os === 'macos'">生成后私钥将安全保存在 macOS 钥匙串</p>
            <p v-else>生成后私钥将安全保存在本机</p>
          </div>
          <button v-if="!caStatus.generated" type="button" class="primary-button" :disabled="busy" @click="emit('generate')">
            生成证书
          </button>
          <button v-else type="button" class="secondary-button" :disabled="busy" @click="emit('export')">
            <Download :size="14" />
            导出
          </button>
        </section>

        <section v-if="platform?.os === 'macos'" class="instruction-section">
          <h3>本机 macOS</h3>
          <ol>
            <li>导出证书并双击加入“钥匙串访问”。</li>
            <li>在“系统”钥匙串中找到 NetCat Local Root CA。</li>
            <li>展开“信任”，将“使用此证书时”设为“始终信任”。</li>
            <li>从 NetCat 升级时，请重新信任此证书，并可手动删除旧根证书。</li>
          </ol>
        </section>

        <section v-else-if="platform?.os === 'windows'" class="instruction-section">
          <h3>本机 Windows</h3>
          <p v-if="trustSupported">仅在当前用户的“受信任的根证书颁发机构”中安装或移除当前 CA。</p>
          <p role="status">{{ trustMessage }}</p>
          <p v-if="caStatus.generated" role="status">{{ privateKeyMessage }}</p>
          <button
            v-if="trustSupported && caStatus.generated && trustState === 'notTrusted' && privateKeyState === 'ready'"
            type="button"
            class="primary-button"
            :disabled="busy"
            @click="emit('trust')"
          >
            安装信任
          </button>
          <button
            v-if="trustSupported && caStatus.generated && trustState === 'trusted'"
            type="button"
            class="secondary-button"
            :disabled="busy"
            @click="emit('removeTrust')"
          >
            移除信任
          </button>
          <button
            v-if="canRegenerate"
            type="button"
            class="primary-button"
            :disabled="busy"
            @click="emit('generate')"
          >
            重新生成证书
          </button>
        </section>

        <section v-else class="instruction-section">
          <p role="status">平台信息加载中或暂不可用</p>
        </section>

        <section class="instruction-section">
          <div class="section-title">
            <Smartphone :size="16" />
            <h3>iOS / Android</h3>
          </div>
          <template v-if="certificateUrl">
            <p>手机与电脑保持同一 Wi-Fi，代理填写 {{ proxyStatus.lanIp }}:{{ proxyStatus.port }}，然后访问：</p>
            <div class="certificate-download">
              <div class="qr-code" aria-label="CA 证书下载二维码">
                <QrcodeVue :value="certificateUrl" :size="116" level="M" render-as="svg" />
              </div>
              <code>{{ certificateUrl }}</code>
            </div>
            <p>代理连通性自检：<code class="inline-code">{{ healthUrl }}</code></p>
          </template>
          <p v-else>启动代理并开启“局域网”，这里会显示手机证书下载地址。</p>
          <ol>
            <li>CA 地址无法下载：先访问连通性自检地址，确认代理和局域网配置。</li>
            <li>CA 已下载但 HTTPS 仍失败：iOS 需在“证书信任设置”中开启完全信任。</li>
            <li>TLS 握手失败：Android 7+ 应用可能不信任用户 CA，需由被测应用显式允许。</li>
            <li>浏览器正常但特定应用失败：该应用可能启用了证书固定，本工具不会绕过该保护。</li>
          </ol>
        </section>
      </div>
    </section>
  </div>
</template>

<style scoped>
.panel-backdrop {
  position: fixed;
  z-index: 80;
  inset: 0;
  display: flex;
  justify-content: flex-end;
  background: var(--overlay);
}

.embedded-panel {
  min-width: 0;
}

.certificate-panel {
  display: grid;
  width: min(520px, 100%);
  height: 100%;
  grid-template-rows: auto minmax(0, 1fr);
  background: var(--surface);
  box-shadow: var(--shadow-panel);
}

.certificate-panel.embedded {
  width: 100%;
  height: auto;
  min-height: 100%;
  box-shadow: none;
}

.panel-header {
  display: flex;
  padding: 17px 18px;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
}

.panel-header h2 {
  margin: 0;
  font-size: 15px;
}

.panel-header p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 10px;
}

.panel-body {
  padding: 18px;
  overflow: auto;
}

.certificate-status {
  display: grid;
  padding-bottom: 18px;
  grid-template-columns: 42px 1fr auto;
  align-items: center;
  gap: 10px;
  border-bottom: 1px solid var(--border);
}

.status-icon {
  display: grid;
  width: 40px;
  height: 40px;
  place-items: center;
  border-radius: var(--radius-panel);
  background: var(--warning-soft);
  color: var(--warning);
}

.status-icon.ready {
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.certificate-status strong {
  font-size: 12px;
}

.certificate-status p {
  margin: 4px 0 0;
  overflow-wrap: anywhere;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 9px;
}

.instruction-section {
  padding: 18px 0;
  border-bottom: 1px solid var(--border-soft);
}

.instruction-section h3 {
  margin: 0 0 10px;
  font-size: 12px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 7px;
}

.section-title h3 {
  margin: 0;
}

.instruction-section p,
.instruction-section li {
  color: var(--text-secondary);
  font-size: 11px;
  line-height: 1.65;
}

.instruction-section ol {
  margin: 10px 0 0;
  padding-left: 18px;
}

.instruction-section code {
  display: block;
  min-width: 0;
  padding: 9px 10px;
  overflow-wrap: anywhere;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--accent-strong);
  font-size: 10px;
}

.instruction-section .inline-code {
  display: inline;
  padding: 2px 4px;
}

.certificate-download {
  display: grid;
  grid-template-columns: 132px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
}

.qr-code {
  display: grid;
  width: 132px;
  height: 132px;
  place-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--qr-surface);
}

@media (max-width: 480px) {
  .certificate-download {
    grid-template-columns: 1fr;
  }
}

.icon-button,
.primary-button,
.secondary-button {
  display: inline-flex;
  height: 30px;
  align-items: center;
  justify-content: center;
  gap: 5px;
  border-radius: var(--radius-control);
  font-family: inherit;
  font-size: 10px;
  font-weight: 600;
}

.icon-button {
  width: 30px;
  border: 0;
  background: transparent;
  color: var(--text-muted);
}

.primary-button,
.secondary-button {
  padding: 0 10px;
}

.primary-button {
  border: 1px solid var(--accent-strong);
  background: var(--accent);
  color: var(--on-accent);
}

.secondary-button {
  border: 1px solid var(--border-strong);
  background: var(--surface-raised);
  color: var(--text-secondary);
}
</style>
