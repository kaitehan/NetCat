const REQUEST_METHODS = new Set([
  'runtime/initialize',
  'runtime/shutdown',
  'session/create',
  'session/resume',
  'session/run',
  'session/snapshot',
  'session/stop'
])

const EVENT_TYPES = new Set([
  'runtime/status',
  'session/status',
  'session/activity',
  'tool/started',
  'tool/completed',
  'approval/requested',
  'session/completed',
  'session/failed'
])

type JsonObject = Record<string, unknown>

export interface BridgeRequest {
  jsonrpc: '2.0'
  id: string
  method: string
  params: JsonObject
}

export interface BridgeEvent {
  type: string
  requestId: string
  conversationId: string
  sessionId: string
  operationId: string
  payload: JsonObject
}

function record(value: unknown, label: string): JsonObject {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError(`${label} must be an object`)
  }
  return value as JsonObject
}

function stringField(value: JsonObject, field: string): string {
  const result = value[field]
  if (typeof result !== 'string' || result.length === 0) {
    throw new TypeError(`${field} must be a non-empty string`)
  }
  return result
}

function requireOperationIdentity(params: JsonObject): void {
  for (const field of ['requestId', 'conversationId', 'sessionId', 'operationId']) {
    stringField(params, field)
  }
}

export function parseBridgeRequest(value: unknown): BridgeRequest {
  const request = record(value, 'bridge request')
  if (request.jsonrpc !== '2.0') throw new TypeError('jsonrpc must equal 2.0')
  const id = stringField(request, 'id')
  const method = stringField(request, 'method')
  if (!REQUEST_METHODS.has(method)) throw new TypeError(`unsupported method: ${method}`)
  const params = record(request.params, 'params')
  if (method.startsWith('session/')) requireOperationIdentity(params)
  return { jsonrpc: '2.0', id, method, params }
}

export function parseBridgeEvent(value: unknown): BridgeEvent {
  const event = record(value, 'bridge event')
  const type = stringField(event, 'type')
  if (!EVENT_TYPES.has(type)) throw new TypeError(`unsupported event: ${type}`)
  return {
    type,
    requestId: stringField(event, 'requestId'),
    conversationId: stringField(event, 'conversationId'),
    sessionId: stringField(event, 'sessionId'),
    operationId: stringField(event, 'operationId'),
    payload: record(event.payload ?? {}, 'payload')
  }
}

export class OperationTracker {
  readonly #active = new Map<string, string>()

  start(conversationId: string, operationId: string): void {
    this.#active.set(conversationId, operationId)
  }

  stop(conversationId: string, operationId: string): void {
    if (this.#active.get(conversationId) === operationId) this.#active.delete(conversationId)
  }

  accepts(conversationId: string, operationId: string): boolean {
    return this.#active.get(conversationId) === operationId
  }
}
