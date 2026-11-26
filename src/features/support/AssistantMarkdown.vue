<script setup lang="ts">
import { computed } from 'vue'
import DOMPurify from 'dompurify'
import { marked } from 'marked'

const { content } = defineProps<{
  content: string
}>()

const safeHtml = computed(() => {
  const rendered = marked.parse(content, { async: false }) as string
  const sanitized = DOMPurify.sanitize(rendered)
  const template = document.createElement('template')
  template.innerHTML = sanitized
  for (const link of template.content.querySelectorAll<HTMLAnchorElement>('a[href]')) {
    const target = new URL(link.href, window.location.href)
    if (target.origin !== window.location.origin) {
      link.target = '_blank'
      link.rel = 'noopener noreferrer'
    }
  }
  return template.innerHTML
})
</script>

<template>
  <div class="assistant-markdown" v-html="safeHtml" />
</template>

<style scoped>
.assistant-markdown {
  min-width: 0;
  overflow-wrap: anywhere;
}

.assistant-markdown :deep(:first-child) {
  margin-top: 0;
}

.assistant-markdown :deep(:last-child) {
  margin-bottom: 0;
}

.assistant-markdown :deep(h1),
.assistant-markdown :deep(h2),
.assistant-markdown :deep(h3) {
  margin: 10px 0 6px;
  color: var(--text);
  font-size: var(--font-size-md);
  line-height: 1.35;
}

.assistant-markdown :deep(p),
.assistant-markdown :deep(ul),
.assistant-markdown :deep(ol),
.assistant-markdown :deep(blockquote),
.assistant-markdown :deep(pre),
.assistant-markdown :deep(table) {
  margin: 0 0 8px;
}

.assistant-markdown :deep(ul),
.assistant-markdown :deep(ol) {
  padding-left: 20px;
}

.assistant-markdown :deep(blockquote) {
  padding-left: 9px;
  border-left: 3px solid var(--border-strong);
  color: var(--text-secondary);
}

.assistant-markdown :deep(code) {
  padding: 1px 4px;
  border-radius: 3px;
  background: var(--surface-hover);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
}

.assistant-markdown :deep(pre) {
  padding: 8px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-control);
  background: var(--surface-hover);
}

.assistant-markdown :deep(pre code) {
  padding: 0;
  background: transparent;
}

.assistant-markdown :deep(table) {
  width: 100%;
  border-collapse: collapse;
  font-variant-numeric: tabular-nums;
}

.assistant-markdown :deep(th),
.assistant-markdown :deep(td) {
  padding: 5px 6px;
  border: 1px solid var(--border);
  text-align: left;
}

.assistant-markdown :deep(a) {
  color: var(--accent);
}
</style>
