import App from './App.vue'
import DesktopPetApp from './features/pet/DesktopPetApp.vue'

export function rootComponentForWindow(label: string) {
  return label === 'pet' ? DesktopPetApp : App
}
