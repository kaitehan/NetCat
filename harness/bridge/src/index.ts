import { DeepSeekHarness } from '@deepseek-ai/dsh-sdk-client'

import { BridgeController } from './controller.js'
import { serveLines } from './line-server.js'
import { parseBridgeRequest } from './protocol.js'

function write(value: unknown): void {
  process.stdout.write(`${JSON.stringify(value)}\n`)
}

const controller = new BridgeController(
  options => new DeepSeekHarness(options),
  event => write({ jsonrpc: '2.0', method: 'event', params: event })
)

await serveLines(process.stdin, async line => {
  if (!line.trim()) return false
  let id: string | null = null
  let shutdown = false
  try {
    const request = parseBridgeRequest(JSON.parse(line) as unknown)
    id = request.id
    const result = await controller.handle(request.method, request.params)
    write({ jsonrpc: '2.0', id, result })
    shutdown = request.method === 'runtime/shutdown'
  } catch (error) {
    write({
      jsonrpc: '2.0',
      id,
      error: {
        code: -32602,
        message: error instanceof Error ? error.message : String(error)
      }
    })
  }
  return shutdown
})

await controller.handle('runtime/shutdown', {})
