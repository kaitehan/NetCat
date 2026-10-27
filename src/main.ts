import { createApp } from 'vue'
import './assets/main.css'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { rootComponentForWindow } from './bootstrap'

const label = window.__TAURI_INTERNALS__ ? getCurrentWindow().label : 'main'

createApp(rootComponentForWindow(label)).mount('#app')
