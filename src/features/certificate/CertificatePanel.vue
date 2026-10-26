<script setup lang="ts">
import { computed } from 'vue'
import { CheckCircle2, Download, KeyRound, ShieldAlert, ShieldCheck, X } from '@lucide/vue'
import type { CaStatus, CaPrivateKeyState, CertificateTrustState, PlatformInfo } from '../../shared/types'

const { open = true, embedded = false, platform, trustState, privateKeyState, caStatus, busy } = defineProps<{
  open?: boolean
  embedded?: boolean
  platform: PlatformInfo | null
  trustState: CertificateTrustState
  privateKeyState: CaPrivateKeyState
  caStatus: CaStatus
  busy: boolean
}>()

const emit = defineEmits<{
  close: []
  generate: []
  export: []
  trust: []
  removeTrust: []
}>()

const trustSupported = computed(() => platform?.certificateTrustSupported && trustState !== 'unsupported')
const canRegenerate = computed(() => trustSupported.value && caStatus.generated &&
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

        <section v-if="platform?.os === 'macos' || platform?.os === 'windows'" class="trust-section">
          <div class="trust-heading">
            <div class="trust-heading-icon" aria-hidden="true">
              <ShieldCheck :size="18" />
            </div>
            <div>
              <h3>{{ platform.os === 'macos' ? '本机 macOS' : '本机 Windows' }}</h3>
              <p v-if="trustSupported && platform.os === 'macos'">管理当前用户登录钥匙串中的 CA 信任</p>
              <p v-else-if="trustSupported">管理当前用户的受信任根证书</p>
              <p v-else>当前平台需要手动管理 CA 信任</p>
            </div>
          </div>

          <div class="trust-row">
            <div class="trust-summary" role="status">
              <CheckCircle2 v-if="trustState === 'trusted'" class="trust-state-icon trusted" :size="18" />
              <ShieldAlert v-else class="trust-state-icon" :size="18" />
              <div>
                <strong>{{ trustMessage }}</strong>
                <p v-if="caStatus.generated" class="private-key-state">
                  <KeyRound :size="13" aria-hidden="true" />
                  {{ privateKeyMessage }}
                </p>
              </div>
            </div>

            <div class="trust-actions">
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
            </div>
          </div>

          <details v-if="platform.os === 'macos'" class="manual-instructions">
            <summary>手动处理</summary>
            <ol>
              <li>导出证书并双击加入“钥匙串访问”。</li>
              <li>在“登录”钥匙串中找到 NetCat Local Root CA。</li>
              <li>展开“信任”，将“使用此证书时”设为“始终信任”。</li>
            </ol>
          </details>
        </section>

        <section v-else class="trust-section unavailable">
          <p role="status">平台信息加载中或暂不可用</p>
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
  font-size: var(--font-size-xl);
}

.panel-header p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
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
  font-size: var(--font-size-md);
}

.certificate-status p {
  margin: 4px 0 0;
  overflow-wrap: anywhere;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: var(--font-size-2xs);
}

.trust-section {
  padding: 18px 0 0;
  border-bottom: 1px solid var(--border-soft);
}

.trust-section.unavailable {
  padding-bottom: 18px;
}

.trust-heading {
  display: flex;
  padding: 0 2px 14px;
  align-items: center;
  gap: 10px;
}

.trust-heading-icon {
  display: grid;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: var(--radius-control);
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.trust-heading h3 {
  margin: 0;
  font-size: var(--font-size-lg);
}

.trust-heading p {
  margin: 3px 0 0;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.trust-row {
  display: grid;
  min-height: 68px;
  padding: 14px 2px;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 20px;
  border-top: 1px solid var(--border-soft);
}

.trust-summary {
  display: flex;
  min-width: 0;
  align-items: flex-start;
  gap: 9px;
}

.trust-state-icon {
  flex: 0 0 auto;
  margin-top: 1px;
  color: var(--warning);
}

.trust-state-icon.trusted {
  color: var(--success);
}

.trust-summary strong {
  display: block;
  color: var(--text);
  font-size: var(--font-size-md);
  line-height: 1.4;
}

.private-key-state {
  display: flex;
  margin: 5px 0 0;
  align-items: center;
  gap: 5px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
  line-height: 1.45;
}

.trust-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.manual-instructions {
  padding: 12px 2px 14px;
  border-top: 1px solid var(--border-soft);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.manual-instructions summary {
  color: var(--text-secondary);
  font-weight: 600;
}

.manual-instructions ol {
  margin: 10px 0 0;
  padding-left: 20px;
  line-height: 1.65;
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
  font-size: var(--font-size-xs);
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

@media (max-width: 560px) {
  .certificate-status,
  .trust-row {
    grid-template-columns: minmax(0, 1fr);
  }

  .certificate-status > button,
  .trust-actions {
    justify-self: start;
  }
}
</style>
