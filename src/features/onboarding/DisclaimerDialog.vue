<script setup lang="ts">
import { shallowRef, watch } from 'vue'
import { ShieldCheck } from '@lucide/vue'
import { APP_BRAND } from '../../shared/brand'

const { open } = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  accept: []
}>()

const acknowledged = shallowRef(false)

watch(() => open, (value) => {
  if (value) {
    acknowledged.value = false
  }
})
</script>

<template>
  <div v-if="open" class="dialog-backdrop" role="presentation">
    <section class="disclaimer-dialog" role="dialog" aria-modal="true" aria-labelledby="disclaimer-title">
      <div class="shield-mark">
        <ShieldCheck :size="30" :stroke-width="1.7" />
      </div>
      <h1 id="disclaimer-title">授权使用声明</h1>
      <p class="lead">{{ APP_BRAND.name }} 仅用于您拥有或已获得明确授权的应用、服务和接口调试。</p>
      <div class="statement">
        <p>未经授权抓取、拦截或篡改第三方网络流量，可能违反网络安全和个人信息保护相关法律法规。</p>
        <p>QuickJS 代理脚本仅用于自有或明确授权的网络流量调试；脚本只保存在本机，不会上传或联网获取。</p>
        <p>本工具不提供证书固定绕过或代码注入能力。</p>
      </div>
      <label class="acknowledgement">
        <input v-model="acknowledged" type="checkbox" />
        <span>我仅将本工具用于自有或已获授权的项目调试，并承担因不当使用产生的责任。</span>
      </label>
      <button
        type="button"
        class="continue-button"
        :disabled="!acknowledged"
        @click="emit('accept')"
      >
        进入 {{ APP_BRAND.name }}
      </button>
    </section>
  </div>
</template>

<style scoped>
.dialog-backdrop {
  position: fixed;
  z-index: 100;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: var(--overlay-strong);
}

.disclaimer-dialog {
  width: min(520px, 100%);
  padding: 30px;
  border-radius: var(--radius-panel);
  background: var(--surface);
  box-shadow: var(--shadow-panel);
}

.shield-mark {
  display: grid;
  width: 50px;
  height: 50px;
  margin-bottom: 20px;
  place-items: center;
  border-radius: var(--radius-panel);
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.disclaimer-dialog h1 {
  margin: 0;
  color: var(--text);
  font-size: var(--font-size-3xl);
  line-height: 1.35;
}

.lead {
  margin: 8px 0 20px;
  color: var(--text-muted);
  font-size: var(--font-size-lg);
  line-height: 1.7;
}

.statement {
  padding: 14px 16px;
  border-left: 3px solid var(--warning);
  background: var(--warning-soft);
  color: var(--text-secondary);
  font-size: var(--font-size-md);
  line-height: 1.65;
}

.statement p {
  margin: 0;
}

.statement p + p {
  margin-top: 8px;
}

.acknowledgement {
  display: grid;
  margin: 20px 0;
  grid-template-columns: 18px 1fr;
  gap: 10px;
  color: var(--text-secondary);
  font-size: var(--font-size-md);
  line-height: 1.55;
  cursor: pointer;
}

.acknowledgement input {
  width: 16px;
  height: 16px;
  margin: 1px 0 0;
  accent-color: var(--accent-strong);
}

.continue-button {
  width: 100%;
  height: 38px;
  border: 1px solid var(--accent-strong);
  border-radius: var(--radius-control);
  background: var(--accent);
  color: var(--on-accent);
  font-family: inherit;
  font-size: var(--font-size-lg);
  font-weight: 700;
}

.continue-button:disabled {
  border-color: var(--border);
  background: var(--surface-hover);
  color: var(--text-muted);
}
</style>
