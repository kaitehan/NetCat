import assert from 'node:assert/strict'
import test from 'node:test'

import {
  OperationTracker,
  parseBridgeRequest,
  parseBridgeEvent
} from '../bridge/src/protocol.ts'

test('bridge requests require stable correlation identifiers', () => {
  assert.throws(() => parseBridgeRequest({
    jsonrpc: '2.0',
    id: 'request-1',
    method: 'session/run',
    params: {
      requestId: 'request-1',
      conversationId: 'conversation-1',
      operationId: 'operation-1'
    }
  }), /sessionId/)

  assert.deepEqual(parseBridgeRequest({
    jsonrpc: '2.0',
    id: 'request-1',
    method: 'session/run',
    params: {
      requestId: 'request-1',
      conversationId: 'conversation-1',
      sessionId: 'session-1',
      operationId: 'operation-1',
      input: 'hello'
    }
  }).params.operationId, 'operation-1')
})

test('bridge events reject unknown kinds and malformed identities', () => {
  assert.throws(() => parseBridgeEvent({ type: 'assistant/delta' }), /unsupported event/)
  assert.equal(parseBridgeEvent({
    type: 'session/completed',
    requestId: 'request-1',
    conversationId: 'conversation-1',
    sessionId: 'session-1',
    operationId: 'operation-1',
    payload: { finalResponse: 'done' }
  }).type, 'session/completed')
})

test('operation tracker drops events from stopped or replaced operations', () => {
  const tracker = new OperationTracker()
  tracker.start('conversation-1', 'operation-1')
  assert.equal(tracker.accepts('conversation-1', 'operation-1'), true)
  tracker.stop('conversation-1', 'operation-1')
  assert.equal(tracker.accepts('conversation-1', 'operation-1'), false)
  tracker.start('conversation-1', 'operation-2')
  assert.equal(tracker.accepts('conversation-1', 'operation-1'), false)
  assert.equal(tracker.accepts('conversation-1', 'operation-2'), true)
})
