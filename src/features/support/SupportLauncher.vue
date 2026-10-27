<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef } from 'vue'

const emit = defineEmits<{
  open: []
}>()

const greetings = [
  '你好，我是小橘，有什么可以帮你？',
  '遇到抓包问题了吗？小橘来帮你看看。',
  '代理或证书配置不顺利？可以问问小橘。',
  'Mock 没有按预期生效？我们一起排查一下。',
  '网络调试卡住了？小橘随时在这里。'
] as const

const visible = shallowRef(false)
const fullGreeting = shallowRef('')
const displayedGreeting = shallowRef('')
const announcedGreeting = shallowRef('')
const typing = shallowRef(false)
const reducedMotion = shallowRef(false)
let previousGreetingIndex = -1
let initialTimer: number | null = null
let nextGreetingTimer: number | null = null
let hideTimer: number | null = null
let typingTimer: number | null = null
let motionQuery: MediaQueryList | null = null

const greetingLabel = computed(() => `打开智奈助手：${fullGreeting.value}`)

function clearTimer(timer: number | null) {
  if (timer === null) return
  window.clearTimeout(timer)
  window.clearInterval(timer)
}

function clearAllTimers() {
  clearTimer(initialTimer)
  clearTimer(nextGreetingTimer)
  clearTimer(hideTimer)
  clearTimer(typingTimer)
  initialTimer = null
  nextGreetingTimer = null
  hideTimer = null
  typingTimer = null
}

function selectGreeting(): string {
  let index = Math.floor(Math.random() * greetings.length)
  if (greetings.length > 1 && index === previousGreetingIndex) {
    index = (index + 1) % greetings.length
  }
  previousGreetingIndex = index
  return greetings[index]
}

function scheduleNextGreeting() {
  nextGreetingTimer = window.setTimeout(() => {
    nextGreetingTimer = null
    showGreeting()
  }, 20_000 + Math.floor(Math.random() * 15_001))
}

function hideGreeting() {
  visible.value = false
  typing.value = false
  displayedGreeting.value = ''
  announcedGreeting.value = ''
  hideTimer = null
  scheduleNextGreeting()
}

function scheduleHide() {
  hideTimer = window.setTimeout(hideGreeting, 4_000)
}

function showGreeting() {
  clearTimer(typingTimer)
  typingTimer = null
  clearTimer(hideTimer)
  hideTimer = null
  fullGreeting.value = selectGreeting()
  visible.value = true

  if (reducedMotion.value) {
    displayedGreeting.value = fullGreeting.value
    typing.value = false
    announcedGreeting.value = fullGreeting.value
    scheduleHide()
    return
  }

  typing.value = true
  let characterIndex = 1
  displayedGreeting.value = fullGreeting.value.slice(0, characterIndex)
  typingTimer = window.setInterval(() => {
    characterIndex += 1
    displayedGreeting.value = fullGreeting.value.slice(0, characterIndex)
    if (characterIndex >= fullGreeting.value.length) {
      clearTimer(typingTimer)
      typingTimer = null
      typing.value = false
      announcedGreeting.value = fullGreeting.value
      scheduleHide()
    }
  }, 45)
}

function openSupport() {
  clearAllTimers()
  visible.value = false
  emit('open')
}

function updateMotionPreference(event: MediaQueryListEvent) {
  reducedMotion.value = event.matches
  if (!event.matches || !visible.value || !typing.value) return

  clearTimer(typingTimer)
  typingTimer = null
  displayedGreeting.value = fullGreeting.value
  announcedGreeting.value = fullGreeting.value
  typing.value = false
  scheduleHide()
}

onMounted(() => {
  if (typeof window.matchMedia !== 'function') {
    initialTimer = window.setTimeout(() => {
      initialTimer = null
      showGreeting()
    }, 1_500)
    return
  }

  motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  reducedMotion.value = motionQuery.matches
  motionQuery.addEventListener('change', updateMotionPreference)
  initialTimer = window.setTimeout(() => {
    initialTimer = null
    showGreeting()
  }, 1_500)
})

onBeforeUnmount(() => {
  clearAllTimers()
  motionQuery?.removeEventListener('change', updateMotionPreference)
})
</script>

<template>
  <div class="support-launcher">
    <Transition name="support-greeting">
      <button
        v-if="visible"
        type="button"
        class="support-greeting"
        :aria-label="greetingLabel"
        @click="openSupport"
      >
        <span data-testid="support-greeting-text">{{ displayedGreeting }}</span>
        <span v-if="typing" class="support-greeting-caret" aria-hidden="true">|</span>
      </button>
    </Transition>

    <span class="sr-only" role="status" aria-label="智奈助手问候" aria-live="polite">
      {{ announcedGreeting }}
    </span>

    <button
      type="button"
      class="support-trigger"
      aria-label="打开智奈助手"
      aria-controls="support-chat-panel"
      aria-expanded="false"
      title="智奈助手"
      @click="openSupport"
    >
      <img
        class="support-trigger-icon"
        src="/src-tauri/icons/icon.png"
        alt=""
        data-testid="support-trigger-icon"
      />
    </button>
  </div>
</template>

<style scoped>
.support-launcher {
  position: fixed;
  z-index: 84;
  right: 18px;
  bottom: 38px;
  width: 44px;
  height: 44px;
  isolation: isolate;
}

.support-trigger {
  position: relative;
  z-index: 1;
  display: grid;
  width: 44px;
  height: 44px;
  padding: 0;
  place-items: center;
  border: 1px solid var(--assistant-accent);
  border-radius: 50%;
  background: var(--assistant-accent);
  box-shadow:
    var(--shadow-float),
    0 0 0 3px color-mix(in srgb, var(--assistant-accent) 18%, transparent);
}

.support-trigger::before,
.support-trigger::after {
  position: absolute;
  z-index: -1;
  inset: -1px;
  border: 2px solid var(--assistant-accent);
  border-radius: 50%;
  content: '';
  pointer-events: none;
  animation: support-trigger-ripple 2.4s ease-out infinite;
}

.support-trigger::after {
  animation-delay: 1.2s;
}

.support-trigger-icon {
  display: block;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
}

.support-trigger:hover {
  box-shadow:
    var(--shadow-panel),
    0 0 0 4px color-mix(in srgb, var(--assistant-accent) 24%, transparent);
}

.support-trigger:active {
  box-shadow: var(--shadow-active);
}

.support-trigger:focus-visible,
.support-greeting:focus-visible {
  outline: 2px solid var(--assistant-accent);
  outline-offset: 3px;
}

.support-greeting {
  position: absolute;
  right: -4px;
  bottom: 56px;
  display: block;
  width: max-content;
  min-width: 180px;
  max-width: 240px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-panel);
  background: var(--surface);
  color: var(--text);
  font-family: inherit;
  font-size: var(--font-size-md);
  line-height: 1.5;
  text-align: left;
  box-shadow: var(--shadow-panel);
  overflow-wrap: anywhere;
  transition:
    opacity 140ms ease-out,
    transform 140ms ease-out;
}

.support-greeting::after {
  position: absolute;
  right: 14px;
  bottom: -6px;
  width: 10px;
  height: 10px;
  border-right: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  content: '';
  transform: rotate(45deg);
}

.support-greeting-caret {
  color: var(--assistant-accent);
  font-weight: 700;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@keyframes support-trigger-ripple {
  0% {
    opacity: 0.45;
    transform: scale(1);
  }

  75%,
  100% {
    opacity: 0;
    transform: scale(1.7);
  }
}

.support-greeting-enter-from,
.support-greeting-leave-to {
  opacity: 0;
  transform: translateY(4px);
}

@media (prefers-reduced-motion: reduce) {
  .support-trigger::before,
  .support-trigger::after {
    animation: none;
  }

  .support-greeting {
    transition: none;
  }
}
</style>
