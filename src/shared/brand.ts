import packageJson from '../../package.json'

export const APP_BRAND = Object.freeze({
  name: 'NetCat',
  chineseName: '奈特猫',
  tagline: '本地守候每一次请求',
  description: 'NetCat（奈特猫）是一款面向开发者的本地 HTTP/HTTPS 调试代理，支持流量分析、请求重放、Mock、QuickJS 代理脚本和代理层弱网模拟。流量、规则、脚本与设置默认只保存在本机。',
  author: 'kaite han',
  version: packageJson.version
})
