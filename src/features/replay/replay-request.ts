export interface QueryEntry {
  name: string
  value: string
}

export function splitReplayUrl(value: string): { baseUrl: string; query: QueryEntry[] } {
  const url = new URL(value)
  const query = [...url.searchParams.entries()].map(([name, entryValue]) => ({
    name,
    value: entryValue
  }))
  url.search = ''
  url.hash = ''
  return { baseUrl: url.toString(), query }
}

export function buildReplayUrl(baseUrl: string, query: readonly QueryEntry[]): string {
  const url = new URL(baseUrl)
  url.search = ''
  for (const entry of query) {
    if (entry.name) {
      url.searchParams.append(entry.name, entry.value)
    }
  }
  return url.toString()
}

export function parseHeaderLines(value: string): Record<string, string> {
  const headers: Record<string, string> = {}
  for (const line of value.split('\n').map((item) => item.trim()).filter(Boolean)) {
    const separator = line.indexOf(':')
    if (separator < 1) {
      throw new Error('请求头必须使用“名称: 值”格式')
    }
    const name = line.slice(0, separator).trim()
    const headerValue = line.slice(separator + 1).trim()
    if (/[\r\n]/.test(name) || /[\r\n]/.test(headerValue)) {
      throw new Error('请求头包含非法字符')
    }
    headers[name] = headerValue
  }
  return headers
}

export function bytesToHex(bytes: readonly number[]): string {
  return bytes.map((value) => value.toString(16).padStart(2, '0')).join(' ')
}

export function hexToBytes(value: string): number[] {
  const compact = value.replace(/\s/g, '')
  if (/[^0-9a-f]/i.test(compact)) {
    throw new Error('Hex 正文包含非法字符')
  }
  if (compact.length % 2 !== 0) {
    throw new Error('Hex 正文必须由完整字节组成')
  }
  return compact.match(/.{2}/g)?.map((byte) => Number.parseInt(byte, 16)) ?? []
}
