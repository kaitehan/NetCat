import { createConnection } from 'node:net'

import CredentialProvider, {
  type CredentialInfo,
  type CredentialKey,
  type CredentialRecord,
  type CredentialRecordEntry,
  type CredentialRecordInfo,
  type CredentialRef,
  type ResolvedCredential
} from '@deepseek-ai/dsh-credentials'
import type { Context } from '@deepseek-ai/cordis'

type JsonObject = Record<string, unknown>
type Transport = (payload: JsonObject) => Promise<unknown>

const ALLOWED_REFERENCES = new Set(['DEEPSEEK_API_KEY', 'OPENAI_API_KEY'])

function object(value: unknown): JsonObject {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError('NetCat credential response must be an object')
  }
  return value as JsonObject
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
        const parsed = object(JSON.parse(response))
        const error = parsed.error
        if (error !== undefined) {
          const message = object(error).message
          reject(new Error(typeof message === 'string' ? message : 'NetCat credential request failed'))
        } else {
          resolve(parsed.result)
        }
      } catch (error) {
        reject(error)
      }
    })
    socket.once('connect', () => socket.end(`${JSON.stringify(payload)}\n`))
  })
}

export class NetcatCredentialClient {
  private readonly token: string
  private readonly transport: Transport

  constructor(
    token: string,
    transport: Transport
  ) {
    if (!token) throw new TypeError('NetCat credential token is required')
    this.token = token
    this.transport = transport
  }

  async resolve(reference: string): Promise<ResolvedCredential | undefined> {
    this.assertAllowed(reference)
    const response = object(await this.call('credentials/resolve', reference))
    if (response.configured === false) return undefined
    if (response.configured !== true || typeof response.value !== 'string' || response.value.length === 0) {
      throw new TypeError('NetCat credential response is invalid')
    }
    return { value: response.value, source: 'netcat-secure-storage' }
  }

  async describe(reference: string): Promise<CredentialInfo> {
    this.assertAllowed(reference)
    const response = object(await this.call('credentials/describe', reference))
    if (typeof response.configured !== 'boolean') {
      throw new TypeError('NetCat credential description is invalid')
    }
    return {
      configured: response.configured,
      source: response.configured ? 'netcat-secure-storage' : undefined,
      writable: false,
      masked: response.configured ? '********' : undefined
    }
  }

  private call(method: string, reference: string): Promise<unknown> {
    return this.transport({
      token: this.token,
      method,
      params: { reference }
    })
  }

  private assertAllowed(reference: string): void {
    if (!ALLOWED_REFERENCES.has(reference)) {
      throw new TypeError(`Credential reference ${reference} is not allowed`)
    }
  }
}

export interface Config {
  endpoint?: string
  token?: string
}

export class NetcatCredentialProvider extends CredentialProvider {
  private readonly client: NetcatCredentialClient

  constructor(ctx: Context, config: Config = {}) {
    super(ctx)
    const endpoint = config.endpoint ?? process.env.NETCAT_TOOL_ENDPOINT
    const token = config.token ?? process.env.NETCAT_TOOL_TOKEN
    if (!endpoint || !token) throw new Error('NetCat credential IPC is not configured')
    this.client = new NetcatCredentialClient(token, socketTransport(endpoint))
  }

  resolve(ref: CredentialRef): Promise<ResolvedCredential | undefined> {
    return this.client.resolve(ref)
  }

  describe(ref: CredentialRef): Promise<CredentialInfo> {
    return this.client.describe(ref)
  }

  set(_ref: CredentialRef, _value: string): Promise<void> {
    return Promise.reject(new Error('NetCat credentials are managed by the desktop application'))
  }

  unset(_ref: CredentialRef): Promise<void> {
    return Promise.reject(new Error('NetCat credentials are managed by the desktop application'))
  }

  readRecord(_key: CredentialKey): Promise<CredentialRecord | undefined> {
    return Promise.resolve(undefined)
  }

  describeRecord(_key: CredentialKey): Promise<CredentialRecordInfo> {
    return Promise.resolve({ configured: false, writable: false })
  }

  listRecords(): Promise<readonly CredentialRecordEntry[]> {
    return Promise.resolve([])
  }

  modifyRecord(
    _key: CredentialKey,
    _mutate: (current: CredentialRecord | undefined) => Promise<CredentialRecord | undefined>
  ): Promise<CredentialRecord | undefined> {
    return Promise.reject(new Error('NetCat credential records are read-only'))
  }

  deleteRecord(_key: CredentialKey): Promise<void> {
    return Promise.reject(new Error('NetCat credential records are read-only'))
  }
}

export default NetcatCredentialProvider
