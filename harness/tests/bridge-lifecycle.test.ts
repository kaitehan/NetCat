import assert from 'node:assert/strict'
import { PassThrough } from 'node:stream'
import test from 'node:test'

import { serveLines } from '../bridge/src/line-server.ts'

test('line server closes after shutdown without waiting for stdin EOF', async () => {
  const input = new PassThrough()
  const received: string[] = []
  const serving = serveLines(input, async line => {
    received.push(line)
    return line === 'shutdown'
  })

  input.write('shutdown\n')
  await Promise.race([
    serving,
    new Promise((_, reject) => setTimeout(() => reject(new Error('line server did not close')), 500))
  ])

  assert.deepEqual(received, ['shutdown'])
  assert.equal(input.destroyed, false)
  input.destroy()
})
