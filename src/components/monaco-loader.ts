import EditorWorker from 'monaco-editor/editor/editor.worker?worker'
import JsonWorker from 'monaco-editor/language/json/json.worker?worker'
import CssWorker from 'monaco-editor/language/css/css.worker?worker'
import HtmlWorker from 'monaco-editor/language/html/html.worker?worker'
import TypeScriptWorker from 'monaco-editor/language/typescript/ts.worker?worker'

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
      if (label === 'css' || label === 'scss' || label === 'less') {
        return new CssWorker()
      }
      if (label === 'html' || label === 'handlebars' || label === 'razor') {
        return new HtmlWorker()
      }
      if (label === 'typescript' || label === 'javascript') {
        return new TypeScriptWorker()
      }
      return new EditorWorker()
    }
  }
}

export function loadMonaco(): Promise<MonacoApi> {
  if (!monacoPromise) {
    configureWorkers()
    monacoPromise = import('monaco-editor')
  }
  return monacoPromise
}
