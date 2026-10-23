import { describe, expect, it } from 'vitest'
import { desktopApi } from './desktop'

describe('desktopApi browser fallback', () => {
  it('returns an actionable error when the app cannot open native DevTools', async () => {
    await expect(desktopApi.openDevtools()).rejects.toMatchObject({
      code: 'DEVTOOLS_UNAVAILABLE',
      message: '浏览器预览模式无法由应用打开调试面板',
      action: '请使用浏览器自带的开发者工具'
    })
  })
})
