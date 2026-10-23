import EditorWorker from 'monaco-editor/editor/editor.worker?worker'
import JsonWorker from 'monaco-editor/language/json/json.worker?worker'

import type * as Monaco from 'monaco-editor'

type MonacoApi = typeof Monaco

declare global {
  interface Window {
    MonacoEnvironment?: {
      getWorker: (_moduleId: string, label: string) => Worker
    }
  }
}

let monacoPromise: Promise<MonacoApi> | null = null

function configureWorkers() {
  window.MonacoEnvironment = {
    getWorker(_moduleId: string, label: string) {
      if (label === 'json') {
        return new JsonWorker()
      }
      return new EditorWorker()
    }
  }
}

export function loadMonaco(): Promise<MonacoApi> {
  if (!monacoPromise) {
    configureWorkers()
    monacoPromise = Promise.all([
      import('monaco-editor/editor/editor.api'),
      import('monaco-editor/language/json/monaco.contribution')
    ]).then(([monaco]) => monaco as MonacoApi)
  }
  return monacoPromise
}
