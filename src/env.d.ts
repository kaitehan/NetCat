/// <reference types="vite/client" />

declare module 'monaco-editor/editor/editor.api' {
  export * from 'monaco-editor'
}

declare module 'monaco-editor/language/json/monaco.contribution' {
  const contribution: unknown
  export default contribution
}

declare interface Window {
  __TAURI_INTERNALS__?: unknown
}
