import assert from 'node:assert/strict'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { createServer as createHttpServer } from 'node:http'
import { createServer as createSocketServer } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'

import { DeepSeekHarness } from '@deepseek-ai/dsh-sdk-client'

const repositoryRoot = resolve(import.meta.dirname, '../..')

test('real DSH runtime resolves the OpenAI credential through NetCat private IPC', async () => {
  const home = await mkdtemp(join(tmpdir(), 'netcat-credential-runtime-'))
  const socketPath = join(home, 'tool.sock')
  const token = 'integration-token'
  const credentialCalls: unknown[] = []
  const socketServer = createSocketServer(socket => {
    let request = ''
    socket.setEncoding('utf8')
    socket.on('data', chunk => { request += chunk })
    socket.on('end', () => {
      const payload = JSON.parse(request.trim()) as Record<string, unknown>
      credentialCalls.push(payload)
      socket.end(`${JSON.stringify({ result: { configured: true, value: 'test-only-key' } })}\n`)
    })
  })
  const httpServer = createHttpServer((_request, response) => {
    response.writeHead(401, { 'content-type': 'application/json' })
    response.end(JSON.stringify({ error: { message: 'expected integration stop' } }))
  })

  await new Promise<void>((resolvePromise, reject) => {
    socketServer.once('error', reject)
    socketServer.listen(socketPath, resolvePromise)
  })
  await new Promise<void>((resolvePromise, reject) => {
    httpServer.once('error', reject)
    httpServer.listen(0, '127.0.0.1', resolvePromise)
  })

  const address = httpServer.address()
  assert(address && typeof address === 'object')
  const openAiPatch = join(home, 'openai.patch.yml')
  await writeFile(openAiPatch, `- insert:\n    - id: llm-pi-ai\n      name: '@deepseek-ai/dsh-llm-pi-ai'\n      config:\n        providers:\n          openai:\n            apiKeyEnv: OPENAI_API_KEY\n            api: openai-completions\n            baseURL: http://127.0.0.1:${address.port}/v1\n            models:\n              - id: test-model\n`)

  const harness = new DeepSeekHarness({
    provider: 'openai',
    model: 'test-model',
    dshBin: resolve(repositoryRoot, 'resources/harness/runtime/node_modules/@deepseek-ai/dsh/lib/bin.js'),
    dshHome: home,
    cwd: repositoryRoot,
    profile: 'sdk-minimal',
    patches: [
      resolve(repositoryRoot, 'harness/profile/cordis.patch.yml'),
      openAiPatch
    ],
    env: {
      ...process.env,
      NETCAT_TOOL_ENDPOINT: socketPath,
      NETCAT_TOOL_TOKEN: token
    },
    initializeTimeoutMs: 10_000,
    requestTimeoutMs: 15_000
  })

  try {
    const result = await harness.run('hello', { sessionId: 'credential-integration' })
    assert.deepEqual(credentialCalls, [{
      token,
      method: 'credentials/resolve',
      params: { reference: 'OPENAI_API_KEY' }
    }], JSON.stringify({
      finishReason: result.finishReason,
      events: result.events,
      stderr: (harness.client as unknown as { stderrTail?: string[] }).stderrTail
    }))
  } finally {
    await harness.close()
    await new Promise<void>(resolvePromise => socketServer.close(() => resolvePromise()))
    await new Promise<void>(resolvePromise => httpServer.close(() => resolvePromise()))
    await rm(home, { recursive: true, force: true })
  }
})
