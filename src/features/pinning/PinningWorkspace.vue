<script setup lang="ts">
import { Cable, Play, RefreshCw, ShieldAlert, Square } from '@lucide/vue'
import { computed, shallowRef } from 'vue'
import type {
  FridaApplication,
  FridaDevice,
  FridaEnvironmentStatus,
  PinningLaunchMode,
  PinningSessionConfig,
  PinningSessionStatus,
  ProxyScript
} from '../../shared/types'

const props = defineProps<{
  environment: FridaEnvironmentStatus
  devices: readonly FridaDevice[]
  applications: readonly FridaApplication[]
  scripts: readonly ProxyScript[]
  status: PinningSessionStatus
  busy: boolean
  logs?: readonly string[]
}>()

const emit = defineEmits<{
  refresh: []
  loadApplications: [deviceId: string]
  start: [config: PinningSessionConfig]
  stop: []
}>()

const deviceId = shallowRef('')
const target = shallowRef('')
const mode = shallowRef<PinningLaunchMode>('spawn')
const gadgetAddress = shallowRef('')
const authorized = shallowRef(false)
const selectedScriptIds = shallowRef<ReadonlySet<string>>(new Set())
const injectionScripts = computed(() => props.scripts.filter((script) => script.kind === 'injection'))
const canStart = computed(() => props.environment.available && !props.busy && authorized.value &&
  Boolean(deviceId.value || gadgetAddress.value.trim()) && Boolean(target.value.trim()) && selectedScriptIds.value.size > 0)

function selectDevice(value: string) {
  deviceId.value = value
  target.value = ''
  if (value) {
    emit('loadApplications', value)
  }
}

function toggleScript(id: string, enabled: boolean) {
  const next = new Set(selectedScriptIds.value)
  if (enabled) {
    next.add(id)
  } else {
    next.delete(id)
  }
  selectedScriptIds.value = next
}

function start() {
  if (!canStart.value) {
    return
  }
  emit('start', {
    deviceId: deviceId.value,
    target: target.value,
    mode: mode.value,
    scriptIds: [...selectedScriptIds.value],
    gadgetAddress: gadgetAddress.value.trim() || null
  })
}
</script>

<template>
  <section class="pinning-workspace" aria-labelledby="pinning-title">
    <header class="pinning-header">
      <div>
        <h1 id="pinning-title">SSL Pinning</h1>
        <p>连接已获授权的 Android 或 iOS 测试目标，并通过外部 Frida 注入脚本。</p>
      </div>
      <button type="button" class="refresh-button" :disabled="busy" aria-label="刷新 Frida 环境" @click="emit('refresh')">
        <RefreshCw :size="15" />
        刷新
      </button>
    </header>

    <div class="pinning-content">
      <section class="environment-panel">
        <div class="panel-heading">
          <Cable :size="17" />
          <div>
            <h2>Frida 环境</h2>
            <p v-if="environment.available">已检测到 Frida {{ environment.version }}</p>
            <p v-else>未检测到 Frida</p>
          </div>
          <span :class="environment.available ? 'ready' : 'missing'">
            {{ environment.available ? '可用' : '不可用' }}
          </span>
        </div>
        <div v-if="!environment.available" class="environment-help" role="status">
          <ShieldAlert :size="17" />
          <div>
            <strong>未检测到 Frida</strong>
            <code>pip install frida-tools</code>
            <p>设备端 frida-server 或 Gadget 需由你提前准备，并与客户端版本兼容。</p>
          </div>
        </div>
      </section>

      <form class="session-panel" @submit.prevent="start">
        <div class="panel-heading">
          <ShieldAlert :size="17" />
          <div>
            <h2>注入会话</h2>
            <p>支持高权限设备和已集成 Gadget 的测试包。</p>
          </div>
        </div>

        <div v-if="status.state === 'running'" class="running-session" role="status">
          <div>
            <strong>{{ status.target }}</strong>
            <span>Frida 进程 {{ status.processId }}</span>
          </div>
          <button type="button" :disabled="busy" aria-label="停止 Pinning 会话" @click="emit('stop')">
            <Square :size="14" />
            停止
          </button>
        </div>

        <template v-else>
          <div class="connection-grid">
            <label>
              <span>设备</span>
              <select :value="deviceId" aria-label="设备" :disabled="!environment.available || busy" @change="selectDevice(($event.target as HTMLSelectElement).value)">
                <option value="">选择 USB 设备</option>
                <option v-for="device in devices" :key="device.id" :value="device.id">
                  {{ device.name }} · {{ device.kind }}
                </option>
              </select>
            </label>
            <label>
              <span>Gadget 地址</span>
              <input v-model.trim="gadgetAddress" aria-label="Gadget 地址" placeholder="127.0.0.1:27042" :disabled="busy" />
            </label>
            <label>
              <span>应用或进程</span>
              <select v-model="target" aria-label="应用或进程" :disabled="busy || (!deviceId && !gadgetAddress)">
                <option value="">选择目标</option>
                <option v-for="application in applications" :key="application.identifier" :value="application.identifier">
                  {{ application.name }} · {{ application.identifier }}
                </option>
              </select>
            </label>
          </div>

          <fieldset class="mode-control">
            <legend>启动模式</legend>
            <button type="button" :aria-pressed="mode === 'spawn'" @click="mode = 'spawn'">Spawn</button>
            <button type="button" :aria-pressed="mode === 'attach'" @click="mode = 'attach'">Attach</button>
          </fieldset>

          <fieldset class="script-options">
            <legend>注入脚本</legend>
            <label v-for="script in injectionScripts" :key="script.id">
              <input
                type="checkbox"
                :aria-label="script.name"
                :checked="selectedScriptIds.has(script.id)"
                @change="toggleScript(script.id, ($event.target as HTMLInputElement).checked)"
              />
              <span>{{ script.name }}</span>
              <small>{{ script.platform }}{{ script.builtin ? ' · 内置' : '' }}</small>
            </label>
          </fieldset>

          <label class="authorization">
            <input v-model="authorized" type="checkbox" aria-label="我确认已获授权测试此设备和应用" />
            <span>我确认已获授权测试此设备和应用。</span>
          </label>

          <button type="submit" class="start-button" :disabled="!canStart" aria-label="启动 Pinning 会话">
            <Play :size="14" />
            启动会话
          </button>
        </template>
      </form>

      <section class="session-log" aria-label="Pinning 会话日志">
        <h2>会话日志</h2>
        <p v-if="status.error">{{ status.error }}</p>
        <pre v-else-if="logs?.length"><code>{{ logs.join('\n') }}</code></pre>
        <p v-else>会话日志仅在当前运行期间显示，不会持久化或上传。</p>
      </section>
    </div>
  </section>
</template>

<style scoped>
.pinning-workspace {
  display: grid;
  min-width: 0;
  min-height: 0;
  grid-template-rows: auto minmax(0, 1fr);
  overflow: hidden;
  background: var(--surface);
}

.pinning-header,
.panel-heading,
.running-session {
  display: flex;
  align-items: center;
}

.pinning-header {
  padding: 18px 20px;
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
}

.pinning-header h1,
.panel-heading h2,
.session-log h2 {
  margin: 0;
  font-size: var(--font-size-xl);
}

.pinning-header p,
.panel-heading p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.session-log pre {
  max-height: 220px;
  margin: 10px 0 0;
  padding: 10px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font-size: var(--font-size-xs);
  white-space: pre-wrap;
}

.refresh-button,
.start-button,
.running-session button {
  display: inline-flex;
  height: 31px;
  padding: 0 10px;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-sm);
  font-weight: 700;
}

.pinning-content {
  display: grid;
  box-sizing: border-box;
  width: 100%;
  padding: 18px 20px 28px;
  align-content: start;
  gap: 16px;
  overflow: auto;
}

.environment-panel,
.session-panel,
.session-log {
  border-top: 1px solid var(--border);
  padding-top: 14px;
}

.panel-heading {
  gap: 9px;
}

.panel-heading > div {
  flex: 1;
}

.ready,
.missing {
  font-size: var(--font-size-xs);
  font-weight: 700;
}

.ready {
  color: var(--success);
}

.missing {
  color: var(--danger);
}

.environment-help {
  display: flex;
  margin-top: 14px;
  padding: 12px;
  gap: 9px;
  border: 1px solid var(--danger-border);
  border-radius: var(--radius-control);
  background: var(--danger-soft);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.environment-help div {
  display: grid;
  gap: 5px;
}

.environment-help p {
  margin: 0;
}

.connection-grid {
  display: grid;
  margin-top: 14px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.connection-grid label {
  display: grid;
  gap: 5px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.connection-grid input,
.connection-grid select {
  min-width: 0;
  height: 32px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
  color: var(--text);
  font: inherit;
}

.mode-control,
.script-options {
  margin: 14px 0 0;
  padding: 0;
  border: 0;
}

.mode-control legend,
.script-options legend {
  margin-bottom: 7px;
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

.mode-control button {
  height: 29px;
  padding: 0 12px;
  border: 1px solid var(--border);
  background: var(--surface-raised);
  color: var(--text-muted);
}

.mode-control button[aria-pressed='true'] {
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.script-options {
  display: grid;
  gap: 6px;
}

.script-options label {
  display: grid;
  min-height: 34px;
  padding: 0 9px;
  grid-template-columns: 20px minmax(0, 1fr) auto;
  align-items: center;
  border: 1px solid var(--border-soft);
  border-radius: var(--radius-control);
  font-size: var(--font-size-sm);
}

.script-options small {
  color: var(--text-muted);
}

.authorization {
  display: flex;
  margin: 14px 0;
  align-items: center;
  gap: 7px;
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
}

.start-button {
  border-color: var(--accent-strong);
  background: var(--accent);
  color: var(--on-accent);
}

.running-session {
  margin-top: 14px;
  padding: 12px;
  justify-content: space-between;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-hover);
}

.running-session div {
  display: grid;
  gap: 4px;
}

.running-session span,
.session-log p {
  color: var(--text-muted);
  font-size: var(--font-size-xs);
}

button:disabled,
input:disabled,
select:disabled {
  opacity: 0.48;
}
</style>
