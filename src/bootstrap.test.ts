import { describe, expect, it } from 'vitest'
import App from './App.vue'
import DesktopPetApp from './features/pet/DesktopPetApp.vue'
import { rootComponentForWindow } from './bootstrap'

describe('rootComponentForWindow', () => {
  it('loads the lightweight desktop pet only in the pet window', () => {
    expect(rootComponentForWindow('pet')).toBe(DesktopPetApp)
    expect(rootComponentForWindow('main')).toBe(App)
    expect(rootComponentForWindow('unknown')).toBe(App)
  })
})
