export interface HarnessLike {
  start(): Promise<void>
  run(input: string, options: { sessionId: string; onNotification?: (notification: unknown) => void }): Promise<{
    sessionId: string
    finalResponse: string
    finishReason: string | null
  }>
  resumeSession?(sessionId: string): Promise<unknown>
  close(): Promise<void>
}

export interface HarnessOptions {
  provider: string
  model: string
  dshHome: string
  cwd: string
  profile: string
  patches: string[]
}

type HarnessFactory = (options: HarnessOptions) => HarnessLike
type EventSink = (event: Record<string, unknown>) => void
type Params = Record<string, unknown>

function record(value: unknown): Params | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? value as Params
    : null
}

function notificationEvent(notification: unknown): { type: string; payload: Params } {
  const value = record(notification)
  const method = typeof value?.method === 'string' ? value.method.replaceAll('.', '/') : ''
  const params = record(value?.params) ?? {}
  if (method === 'tool/started' || method === 'tool/completed' || method === 'approval/requested') {
    return { type: method, payload: params }
  }
  if (method === 'session/status') {
    return { type: 'session/status', payload: params }
  }
  return { type: 'session/activity', payload: { notification } }
}

function requiredString(params: Params, name: string): string {
  const value = params[name]
  if (typeof value !== 'string' || value.length === 0) throw new TypeError(`${name} is required`)
  return value
}

export class BridgeController {
  private readonly createHarness: HarnessFactory
  private readonly emit: EventSink
  private harness: HarnessLike | null = null
  private options: HarnessOptions | null = null
  private running = false

  constructor(createHarness: HarnessFactory, emit: EventSink) {
    this.createHarness = createHarness
    this.emit = emit
  }

  async handle(method: string, params: Params): Promise<unknown> {
    switch (method) {
      case 'runtime/initialize':
        return this.initialize(params)
      case 'runtime/shutdown':
        await this.shutdown()
        return { status: 'stopped' }
      case 'session/create':
        return { sessionId: requiredString(params, 'sessionId') }
      case 'session/resume':
        return this.resume(params)
      case 'session/run':
        return this.run(params)
      case 'session/snapshot':
        return { sessionId: requiredString(params, 'sessionId'), messages: [], activities: [] }
      case 'session/stop':
        await this.stop(params)
        return { status: 'stopped' }
      default:
        throw new TypeError(`unsupported method: ${method}`)
    }
  }

  private async initialize(params: Params): Promise<{ status: 'ready' }> {
    if (this.harness) return { status: 'ready' }
    const patches = params.patches
    this.options = {
      provider: requiredString(params, 'provider'),
      model: requiredString(params, 'model'),
      dshHome: requiredString(params, 'dshHome'),
      cwd: requiredString(params, 'cwd'),
      profile: typeof params.profile === 'string' ? params.profile : 'sdk-minimal',
      patches: Array.isArray(patches) && patches.every(value => typeof value === 'string') ? patches : []
    }
    this.harness = this.createHarness(this.options)
    await this.harness.start()
    return { status: 'ready' }
  }

  private async resume(params: Params): Promise<{ sessionId: string }> {
    const harness = this.requireHarness()
    const sessionId = requiredString(params, 'sessionId')
    await harness.resumeSession?.(sessionId)
    return { sessionId }
  }

  private async run(params: Params): Promise<unknown> {
    if (this.running) throw new Error('an assistant turn is already running')
    const harness = this.requireHarness()
    const requestId = requiredString(params, 'requestId')
    const conversationId = requiredString(params, 'conversationId')
    const sessionId = requiredString(params, 'sessionId')
    const operationId = requiredString(params, 'operationId')
    const input = requiredString(params, 'input')
    this.running = true
    try {
      const result = await harness.run(input, {
        sessionId,
        onNotification: notification => {
          const event = notificationEvent(notification)
          this.emit({
            type: event.type,
            requestId,
            conversationId,
            sessionId,
            operationId,
            payload: event.payload
          })
        }
      })
      const response = {
        sessionId: result.sessionId,
        finalResponse: result.finalResponse,
        finishReason: result.finishReason
      }
      this.emit({
        type: 'session/completed',
        requestId,
        conversationId,
        sessionId,
        operationId,
        payload: response
      })
      return response
    } catch (error) {
      this.emit({
        type: 'session/failed',
        requestId,
        conversationId,
        sessionId,
        operationId,
        payload: { message: error instanceof Error ? error.message : String(error) }
      })
      throw error
    } finally {
      this.running = false
    }
  }

  private async stop(params: Params): Promise<void> {
    const conversationId = requiredString(params, 'conversationId')
    const sessionId = requiredString(params, 'sessionId')
    const operationId = requiredString(params, 'operationId')
    await this.shutdown()
    this.emit({
      type: 'session/status',
      requestId: requiredString(params, 'requestId'),
      conversationId,
      sessionId,
      operationId,
      payload: { status: 'stopped' }
    })
  }

  private requireHarness(): HarnessLike {
    if (!this.harness) throw new Error('runtime is not initialized')
    return this.harness
  }

  private async shutdown(): Promise<void> {
    const harness = this.harness
    this.harness = null
    this.running = false
    if (harness) await harness.close()
  }
}
