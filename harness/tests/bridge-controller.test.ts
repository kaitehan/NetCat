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
  assert.equal((events[0] as any).type, 'session/status')

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
      return { sessionId: options.sessionId, finalResponse: 'done', finishReason: 'stop', events: [], notifications: [] }
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

test('controller translates tool notifications into stable bridge events', async () => {
  const events: any[] = []
  const draft = { id: 'draft-1', kind: 'mock', status: 'pending', payload: { name: '登录失败' } }
  const controller = new BridgeController(() => ({
    async start() {},
    async run(_input: string, options: any) {
      options.onNotification({
        method: 'tool.started',
        params: { toolName: 'netcat_create_mock_draft', toolCallId: 'call-1' }
      })
      options.onNotification({
        method: 'tool.completed',
        params: { toolName: 'netcat_create_mock_draft', toolCallId: 'call-1', result: draft }
      })
      return { sessionId: options.sessionId, finalResponse: 'done', finishReason: 'stop' }
    },
    async close() {}
  }), event => events.push(event))
  await controller.handle('runtime/initialize', {
    requestId: 'init', provider: 'deepseek-official', model: 'deepseek-v4-flash', dshHome: '/tmp/h', cwd: '/tmp/w'
  })

  await controller.handle('session/run', {
    requestId: 'r1', conversationId: 'c1', sessionId: 's1', operationId: 'o1', input: 'one'
  })

  assert.equal(events[0].type, 'tool/started')
  assert.deepEqual(events[0].payload, {
    toolName: 'netcat_create_mock_draft',
    toolCallId: 'call-1'
  })
  assert.equal(events[1].type, 'tool/completed')
  assert.deepEqual(events[1].payload.result, draft)
})

test('controller rejects failed model turns instead of completing with an empty reply', async () => {
  const events: any[] = []
  const controller = new BridgeController(() => ({
    async start() {},
    async run(_input: string, options: any) {
      return {
        sessionId: options.sessionId,
        finalResponse: '',
        finishReason: 'error',
        events: [{
          type: 'turn/end',
          data: {
            reason: {
              kind: 'error',
              error: {
                code: 'MISSING_CREDENTIAL',
                message: 'OpenAI API Key is not configured'
              }
            }
          }
        }]
      }
    },
    async close() {}
  }), event => events.push(event))
  await controller.handle('runtime/initialize', {
    requestId: 'init', provider: 'openai', model: 'gpt-4.1', dshHome: '/tmp/h', cwd: '/tmp/w'
  })

  await assert.rejects(controller.handle('session/run', {
    requestId: 'r1', conversationId: 'c1', sessionId: 's1', operationId: 'o1', input: 'hello'
  }), /当前服务商尚未配置 API Key/)

  assert.equal(events.at(-1).type, 'session/failed')
  assert.deepEqual(events.at(-1).payload, {
    code: 'MISSING_CREDENTIAL',
    message: '当前服务商尚未配置 API Key'
  })
  assert.equal(events.some(event => event.type === 'session/completed'), false)
})

test('controller redacts credential-like values from failed model turns', async () => {
  const events: any[] = []
  const controller = new BridgeController(() => ({
    async start() {},
    async run(_input: string, options: any) {
      return {
        sessionId: options.sessionId,
        finalResponse: '',
        finishReason: 'error',
        events: [{
          type: 'turn/end',
          data: {
            reason: {
              kind: 'error',
              error: {
                code: 'UPSTREAM_REJECTED',
                message: 'gateway rejected Bearer sk-secret-value'
              }
            }
          }
        }]
      }
    },
    async close() {}
  }), event => events.push(event))
  await controller.handle('runtime/initialize', {
    requestId: 'init', provider: 'openai', model: 'gpt-4.1', dshHome: '/tmp/h', cwd: '/tmp/w'
  })

  await assert.rejects(controller.handle('session/run', {
    requestId: 'r1', conversationId: 'c1', sessionId: 's1', operationId: 'o1', input: 'hello'
  }), /gateway rejected Bearer \[REDACTED\]/)

  assert.equal(events.at(-1).payload.message, 'gateway rejected Bearer [REDACTED]')
  assert.equal(JSON.stringify(events).includes('sk-secret-value'), false)
})

test('controller translates public model lifecycle events into stable progress summaries', async () => {
  const events: any[] = []
  const controller = new BridgeController(() => ({
    async start() {},
    async run(_input: string, options: any) {
      options.onNotification({
        method: 'session.event',
        params: { event: { type: 'turn/start', data: {} } }
      })
      options.onNotification({
        method: 'session.event',
        params: { event: { type: 'assistant/attempt', data: {} } }
      })
      return {
        sessionId: options.sessionId,
        finalResponse: '你好',
        finishReason: 'stop',
        events: []
      }
    },
    async close() {}
  }), event => events.push(event))
  await controller.handle('runtime/initialize', {
    requestId: 'init', provider: 'openai', model: 'gpt-4.1', dshHome: '/tmp/h', cwd: '/tmp/w'
  })

  await controller.handle('session/run', {
    requestId: 'r1', conversationId: 'c1', sessionId: 's1', operationId: 'o1', input: 'hello'
  })

  assert.deepEqual(
    events.filter(event => event.type === 'session/activity').map(event => event.payload),
    [
      { activityId: 'model-progress', kind: 'model', label: '正在准备模型请求', status: 'running' },
      { activityId: 'model-progress', kind: 'model', label: '模型正在生成回复', status: 'running' }
    ]
  )
})
