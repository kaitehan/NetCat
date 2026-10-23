import packageJson from '../../package.json'

export const APP_BRAND = Object.freeze({
  name: 'NetCat',
  chineseName: '奈特猫',
  tagline: '本地守候每一次请求',
  description: 'NetCat（奈特猫）是一款面向开发者的本地 HTTP/HTTPS 抓包与 Mock 工具，可查看请求与响应、筛选流量、重放请求、构造 Mock，并管理本地 CA 与系统代理。流量、规则与设置默认只保存在本机。',
  author: 'kaite han',
  version: packageJson.version
})
