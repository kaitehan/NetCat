import { createConnection } from 'node:net'

export const name = 'netcat-tools'
export const inject = ['tools']

export const NETCAT_TOOL_NAMES = [
  'netcat_search_exchanges',
  'netcat_parse_curl',
  'netcat_get_exchange',
  'netcat_find_similar_exchanges',
  'netcat_get_diagnostics',
  'netcat_compare_exchanges',
  'netcat_create_mock_draft',
  'netcat_create_script_draft',
  'netcat_execute_curl'
] as const

export type NetcatToolName = typeof NETCAT_TOOL_NAMES[number]
type JsonObject = Record<string, unknown>
type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue }
type Transport = (payload: JsonObject) => Promise<unknown>

const FORBIDDEN_KEYS = new Set([
  'path',
  'command',
  'apikey',
  'api_key',
  'authorization',
  'cookie',
  'password',
  'secret',
  'token'
])

function jsonObject(value: unknown): JsonObject {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError('tool arguments must be an object')
  }
  return value as JsonObject
}

export function validateToolArguments(value: unknown): JsonObject {
  const args = jsonObject(value)
  for (const key of Object.keys(args)) {
    if (FORBIDDEN_KEYS.has(key.toLowerCase())) {
      throw new TypeError(`${key} is not accepted by NetCat tools`)
    }
  }
  return args
}

function assertJsonValue(value: unknown, seen: WeakSet<object>): asserts value is JsonValue {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return
  if (typeof value === 'number') {
    if (Number.isFinite(value)) return
    throw new TypeError('tool result must be a JSON value')
  }
  if (typeof value !== 'object') throw new TypeError('tool result must be a JSON value')
  if (seen.has(value)) throw new TypeError('tool result must be a JSON value')
  seen.add(value)
  if (Array.isArray(value)) {
    for (const item of value) assertJsonValue(item, seen)
  } else {
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) {
      throw new TypeError('tool result must be a JSON value')
    }
    for (const item of Object.values(value)) assertJsonValue(item, seen)
  }
  seen.delete(value)
}

export function validateToolResult(value: unknown): JsonValue {
  assertJsonValue(value, new WeakSet())
  return value
}

export class NetcatToolClient {
  private readonly token: string
  private readonly transport: Transport

  constructor(
    token: string,
    transport: Transport
  ) {
    if (!token) throw new TypeError('NetCat tool token is required')
    this.token = token
    this.transport = transport
  }

  call(method: NetcatToolName, params: unknown): Promise<unknown> {
    return this.transport({
      token: this.token,
      method,
      params: validateToolArguments(params)
    })
  }
}

function socketTransport(endpoint: string): Transport {
  return payload => new Promise((resolve, reject) => {
    const socket = createConnection(endpoint)
    let response = ''
    socket.setEncoding('utf8')
    socket.once('error', reject)
    socket.on('data', chunk => { response += chunk })
    socket.once('end', () => {
      try {
        const parsed = JSON.parse(response) as { result?: unknown; error?: { message?: string } }
        if (parsed.error) reject(new Error(parsed.error.message ?? 'NetCat tool request failed'))
        else resolve(parsed.result)
      } catch (error) {
        reject(error)
      }
    })
    socket.once('connect', () => socket.end(`${JSON.stringify(payload)}\n`))
  })
}

const descriptions: Record<NetcatToolName, string> = {
  netcat_search_exchanges: 'Search captured exchanges by visible URL, domain, method, and status conditions without requiring a resourceId.',
  netcat_parse_curl: 'Read the redacted structured cURL request identified by resourceId.',
  netcat_get_exchange: 'Read one redacted captured HTTP exchange by resourceId.',
  netcat_find_similar_exchanges: 'Find a bounded list of similar captured exchanges.',
  netcat_get_diagnostics: 'Read redacted proxy, TLS, certificate, and network diagnostics.',
  netcat_compare_exchanges: 'Compare two captured exchanges and return structured differences.',
  netcat_create_mock_draft: 'Create a validated Mock rule draft without saving or enabling it.',
  netcat_create_script_draft: 'Create a validated QuickJS script draft without saving or enabling it.',
  netcat_execute_curl: 'Use a single-use requestHandle previously approved by the user.'
}

type ToolParameter = {
  type: 'string' | 'integer'
  required?: true
}

export const TOOL_PARAMETER_SCHEMAS: Record<NetcatToolName, Record<string, ToolParameter>> = {
  netcat_search_exchanges: {
    query: { type: 'string' },
    domain: { type: 'string' },
    method: { type: 'string' },
    statusGroup: { type: 'integer' },
    limit: { type: 'integer' }
  },
  netcat_parse_curl: {
    resourceId: { type: 'string', required: true }
  },
  netcat_get_exchange: {
    resourceId: { type: 'string', required: true }
  },
  netcat_find_similar_exchanges: {
    resourceId: { type: 'string', required: true },
    limit: { type: 'integer' }
  },
  netcat_get_diagnostics: {},
  netcat_compare_exchanges: {
    resourceId: { type: 'string', required: true },
    otherResourceId: { type: 'string', required: true }
  },
  netcat_create_mock_draft: {
    resourceId: { type: 'string', required: true },
    name: { type: 'string', required: true },
    status: { type: 'integer', required: true },
    responseBody: { type: 'string', required: true }
  },
  netcat_create_script_draft: {
    resourceId: { type: 'string', required: true },
    name: { type: 'string', required: true },
    source: { type: 'string', required: true }
  },
  netcat_execute_curl: {
    requestHandle: { type: 'string', required: true }
  }
}

export interface Config {
  endpoint?: string
  token?: string
}

export async function apply(ctx: any, config: Config = {}): Promise<void> {
  const endpoint = config.endpoint ?? process.env.NETCAT_TOOL_ENDPOINT
  const token = config.token ?? process.env.NETCAT_TOOL_TOKEN
  if (!endpoint || !token) throw new Error('NetCat tool IPC is not configured')
  const client = new NetcatToolClient(token, socketTransport(endpoint))
  const { defineTool } = await import('@deepseek-ai/dsh-tools')

  for (const toolName of NETCAT_TOOL_NAMES) {
    const output = {
      schema: { type: 'json' as const },
      render: (_args: unknown, value: JsonValue) => [{ type: 'text' as const, text: JSON.stringify(value) }]
    }
    const execute = async (args: JsonObject) => validateToolResult(await client.call(toolName, args))

    ctx.tools.register(defineTool({
      name: toolName,
      description: descriptions[toolName],
      parameters: TOOL_PARAMETER_SCHEMAS[toolName],
      output,
      execute
    }))
  }
}
