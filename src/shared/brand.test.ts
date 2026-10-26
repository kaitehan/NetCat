import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import packageJson from '../../package.json'
import { APP_BRAND } from './brand'

describe('NetCat brand metadata', () => {
  it('uses the approved product identity and copy', () => {
    expect(APP_BRAND).toEqual({
      name: 'NetCat',
      chineseName: '奈特猫',
      tagline: '本地守候每一次请求',
      description: 'NetCat（奈特猫）是一款面向开发者的本地 HTTP/HTTPS 抓包与 Mock 工具，可查看请求与响应、筛选流量、重放请求、构造 Mock，并管理本地 CA 与系统代理。流量、规则与设置默认只保存在本机。',
      author: 'kaite han',
      version: '0.4.0'
    })
  })

  it('keeps npm, Cargo and Tauri release metadata aligned', () => {
    const cargo = readFileSync(resolve(process.cwd(), 'src-tauri/Cargo.toml'), 'utf8')
    const tauri = JSON.parse(readFileSync(resolve(process.cwd(), 'src-tauri/tauri.conf.json'), 'utf8'))

    expect(packageJson).toMatchObject({ name: 'netcat', version: '0.4.0', author: 'kaite han' })
    expect(cargo).toContain('name = "netcat"')
    expect(cargo).toContain('version = "0.4.0"')
    expect(cargo).toContain('authors = ["kaite han"]')
    expect(tauri).toMatchObject({
      productName: 'NetCat',
      version: '0.4.0',
      identifier: 'com.kaitehan.netcat'
    })
  })
})
