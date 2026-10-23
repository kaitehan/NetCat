import { describe, expect, it } from 'vitest'
import { buildReplayUrl, bytesToHex, hexToBytes, parseHeaderLines, splitReplayUrl } from './replay-request'

describe('replay request helpers', () => {
  it('round trips duplicate and encoded query parameters', () => {
    const parts = splitReplayUrl('https://api.test/items?q=hello%20world&tag=a&tag=b')

    expect(parts).toEqual({
      baseUrl: 'https://api.test/items',
      query: [
        { name: 'q', value: 'hello world' },
        { name: 'tag', value: 'a' },
        { name: 'tag', value: 'b' }
      ]
    })
    expect(buildReplayUrl(parts.baseUrl, parts.query)).toBe('https://api.test/items?q=hello+world&tag=a&tag=b')
  })

  it('parses edited headers and rejects malformed lines', () => {
    expect(parseHeaderLines('Content-Type: application/json\nX-Trace: abc')).toEqual({
      'Content-Type': 'application/json',
      'X-Trace': 'abc'
    })
    expect(() => parseHeaderLines('invalid')).toThrow('请求头必须使用“名称: 值”格式')
  })

  it('round trips binary request bodies through strict hex', () => {
    expect(bytesToHex([0, 1, 2, 255])).toBe('00 01 02 ff')
    expect(hexToBytes('00 01\n02 ff')).toEqual([0, 1, 2, 255])
    expect(() => hexToBytes('0 ff')).toThrow('Hex 正文必须由完整字节组成')
    expect(() => hexToBytes('zz')).toThrow('Hex 正文包含非法字符')
  })
})
