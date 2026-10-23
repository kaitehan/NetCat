<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue'
import type * as Monaco from 'monaco-editor'
import type { BodyLanguage } from './body-editor-policy'
import { loadMonaco } from './monaco-loader'

const props = withDefaults(defineProps<{
  modelValue: string
  language?: BodyLanguage
  readOnly?: boolean
  ariaLabel: string
  diagnostics?: boolean
}>(), {
  language: 'plaintext',
  readOnly: false,
  diagnostics: true
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const host = useTemplateRef<HTMLDivElement>('host')
const fallbackInput = useTemplateRef<HTMLTextAreaElement>('fallbackInput')
const loading = shallowRef(true)
const failed = shallowRef(false)
const fallbackValue = computed({
  get: () => props.modelValue,
  set: (value: string) => emit('update:modelValue', value)
})

let editor: Monaco.editor.IStandaloneCodeEditor | null = null
let model: Monaco.editor.ITextModel | null = null
let contentSubscription: Monaco.IDisposable | null = null
let resizeObserver: ResizeObserver | null = null
let disposed = false
let syncingFromProps = false
let monacoApi: typeof Monaco | null = null
let focusRequested = false

async function mountEditor() {
  try {
    const monaco = await loadMonaco()
    if (disposed || !host.value) {
      return
    }
    monacoApi = monaco
    monaco.editor.defineTheme('netcapture-light', {
      base: 'vs',
      inherit: true,
      rules: [],
      colors: {
        'editor.background': '#FFFFFF',
        'editor.foreground': '#17202A',
        'editorLineNumber.foreground': '#98A2B3',
        'editorLineNumber.activeForeground': '#475467',
        'editorGutter.background': '#FFFFFF',
        'editor.selectionBackground': '#B9D7FF88',
        'editor.lineHighlightBackground': '#F7F9FC'
      }
    })
    model = monaco.editor.createModel(props.modelValue, props.language)
    editor = monaco.editor.create(host.value, {
      model,
      theme: 'netcapture-light',
      readOnly: props.readOnly,
      domReadOnly: props.readOnly,
      ariaLabel: props.ariaLabel,
      automaticLayout: true,
      wordWrap: 'on',
      wrappingIndent: 'same',
      lineNumbers: 'on',
      folding: true,
      foldingStrategy: 'indentation',
      minimap: { enabled: false },
      suggest: { showWords: false },
      quickSuggestions: false,
      parameterHints: { enabled: false },
      hover: { enabled: 'off' },
      contextmenu: true,
      scrollBeyondLastLine: false,
      overviewRulerLanes: 0,
      overviewRulerBorder: false,
      lineDecorationsWidth: 6,
      lineNumbersMinChars: 3,
      glyphMargin: false,
      padding: { top: 10, bottom: 10 },
      fontFamily: 'var(--font-mono)',
      fontSize: 12,
      tabSize: 2,
      renderValidationDecorations: props.diagnostics ? 'on' : 'off',
      renderWhitespace: 'none',
      stickyScroll: { enabled: false },
      bracketPairColorization: { enabled: true },
      matchBrackets: 'always'
    })
    contentSubscription = model.onDidChangeContent(() => {
      if (!props.readOnly && !syncingFromProps && model) {
        emit('update:modelValue', model.getValue())
      }
    })
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(() => editor?.layout())
      resizeObserver.observe(host.value)
    }
    loading.value = false
    if (focusRequested) {
      editor.focus()
    }
  } catch {
    loading.value = false
    failed.value = true
  }
}

watch(() => props.modelValue, (value) => {
  if (model && model.getValue() !== value) {
    syncingFromProps = true
    model.setValue(value)
    syncingFromProps = false
  }
})

watch(() => props.language, (language) => {
  if (monacoApi && model) {
    monacoApi.editor.setModelLanguage(model, language)
  }
})

watch(() => props.readOnly, (readOnly) => {
  editor?.updateOptions({ readOnly, domReadOnly: readOnly })
})

watch(() => props.diagnostics, (diagnostics) => {
  editor?.updateOptions({ renderValidationDecorations: diagnostics ? 'on' : 'off' })
})

function focus() {
  focusRequested = true
  if (editor) {
    editor.focus()
  } else {
    fallbackInput.value?.focus()
  }
}

defineExpose({ focus })

onMounted(() => void mountEditor())

onBeforeUnmount(() => {
  disposed = true
  resizeObserver?.disconnect()
  contentSubscription?.dispose()
  editor?.dispose()
  model?.dispose()
  resizeObserver = null
  contentSubscription = null
  editor = null
  model = null
})
</script>

<template>
  <div class="body-editor" role="region" :aria-label="ariaLabel">
    <div v-show="!failed" ref="host" class="body-editor-host" />
    <div v-if="loading" class="body-editor-loading">正在加载编辑器...</div>
    <pre v-if="failed && readOnly" class="body-editor-fallback">{{ modelValue }}</pre>
    <textarea
      v-else-if="failed || loading"
      ref="fallbackInput"
      v-model="fallbackValue"
      class="body-editor-fallback body-editor-textarea"
      :aria-label="ariaLabel"
      :readonly="readOnly"
      spellcheck="false"
    />
    <pre class="body-editor-accessible" aria-hidden="true">{{ modelValue }}</pre>
  </div>
</template>

<style scoped>
.body-editor {
  position: relative;
  width: 100%;
  height: 240px;
  min-height: 160px;
  overflow: hidden;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-control);
  background: var(--surface-raised);
}

.body-editor-host {
  width: 100%;
  height: 100%;
}

.body-editor-loading {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--surface-raised);
  color: var(--text-muted);
  font-size: 11px;
  pointer-events: none;
  z-index: 2;
}

.body-editor-fallback {
  width: 100%;
  height: 100%;
  margin: 0;
  padding: 10px;
  overflow: auto;
  border: 0;
  outline: none;
  background: var(--surface-raised);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.body-editor-textarea {
  position: absolute;
  inset: 0;
  resize: none;
}

.body-editor-accessible {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: pre-wrap;
  border: 0;
}
</style>
