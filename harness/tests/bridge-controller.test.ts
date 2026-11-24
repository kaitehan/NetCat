import assert from 'node:assert/strict'
import test from 'node:test'

import { BridgeController } from '../bridge/src/controller.ts'

test('controller initializes one runtime and runs named sessions', async () => {
  const created: unknown[] = []
  const calls: unknown[] = []
  const events: unknown[] = []
  let closed = false
  const controller = new BridgeController(options => {
    created.push(options)
    return {
      async start() {},
      async run(input: string, runOptions: any) {
        const { onNotification, ...publicOptions } = runOptions
        calls.push({ input, runOptions: publicOptions })
        onNotification({ method: 'session.status', params: { status: 'running' } })
        return {
          sessionId: runOptions.sessionId,
          finalResponse: 'analysis complete',
          finishReason: 'stop',
          events: [],
          notifications: []
        }
      },
      async close() { closed = true }
    }
  }, event => events.push(event))

  await controller.handle('runtime/initialize', {
    requestId: 'request-init',
    provider: 'openai',
    model: 'gpt-4.1',
    dshHome: '/tmp/netcat-harness',
    cwd: '/tmp/netcat-harness/workspace'
  })
  const result = await controller.handle('session/run', {
    requestId: 'request-1',
    conversationId: 'conversation-1',
    sessionId: 'session-1',
    operationId: 'operation-1',
    input: 'diagnose this request'
  })

  assert.equal(created.length, 1)
  assert.deepEqual(created[0], {
    provider: 'openai',
    model: 'gpt-4.1',
    dshHome: '/tmp/netcat-harness',
    cwd: '/tmp/netcat-harness/workspace',
    profile: 'sdk-minimal',
    patches: []
  })
  assert.deepEqual(calls[0], {
    input: 'diagnose this request',
    runOptions: { sessionId: 'session-1' }
  })
  assert.deepEqual(result, {
    sessionId: 'session-1',
    finalResponse: 'analysis complete',
    finishReason: 'stop'
  })
  assert.equal((events[0] as any).type, 'session/activity')

  await controller.handle('runtime/shutdown', {})
  assert.equal(closed, true)
})

test('controller refuses concurrent turns', async () => {
  let release!: () => void
  const pending = new Promise<void>(resolve => { release = resolve })
  const controller = new BridgeController(() => ({
    async start() {},
    async run(_input: string, options: any) {
      await pending
      return { sessionId: options.sessionId, finalResponse: '', finishReason: 'stop', events: [], notifications: [] }
    },
    async close() {}
  }), () => undefined)
  await controller.handle('runtime/initialize', {
    requestId: 'init', provider: 'deepseek-official', model: 'deepseek-v4-flash', dshHome: '/tmp/h', cwd: '/tmp/w'
  })
  const first = controller.handle('session/run', {
    requestId: 'r1', conversationId: 'c1', sessionId: 's1', operationId: 'o1', input: 'one'
  })
  await assert.rejects(controller.handle('session/run', {
    requestId: 'r2', conversationId: 'c2', sessionId: 's2', operationId: 'o2', input: 'two'
  }), /already running/)
  release()
  await first
})
