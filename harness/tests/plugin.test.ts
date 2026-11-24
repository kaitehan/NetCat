import assert from 'node:assert/strict'
import test from 'node:test'

import {
  NETCAT_TOOL_NAMES,
  NetcatToolClient,
  validateToolArguments,
  validateToolResult
} from '../plugin-netcat/src/index.ts'

test('plugin exposes only the eight approved NetCat tools', () => {
  assert.deepEqual(NETCAT_TOOL_NAMES, [
    'netcat_parse_curl',
    'netcat_get_exchange',
    'netcat_find_similar_exchanges',
    'netcat_get_diagnostics',
    'netcat_compare_exchanges',
    'netcat_create_mock_draft',
    'netcat_create_script_draft',
    'netcat_execute_curl'
  ])
})

test('tool arguments reject local paths, commands, and credentials', () => {
  for (const key of ['path', 'command', 'apiKey', 'authorization', 'cookie']) {
    assert.throws(() => validateToolArguments({ resourceId: 'exchange-1', [key]: 'secret' }), new RegExp(key, 'i'))
  }
  assert.deepEqual(validateToolArguments({ resourceId: 'exchange-1', limit: 5 }), {
    resourceId: 'exchange-1',
    limit: 5
  })
})

test('tool client authenticates every Rust IPC request', async () => {
  const calls: unknown[] = []
  const client = new NetcatToolClient('startup-token', async payload => {
    calls.push(payload)
    return { ok: true }
  })
  await client.call('netcat_get_exchange', { resourceId: 'exchange-1' })
  assert.deepEqual(calls, [{
    token: 'startup-token',
    method: 'netcat_get_exchange',
    params: { resourceId: 'exchange-1' }
  }])
})

test('tool results must be durable JSON values', () => {
  assert.deepEqual(validateToolResult({ ok: true, values: [1, null, 'done'] }), {
    ok: true,
    values: [1, null, 'done']
  })
  assert.throws(() => validateToolResult({ value: undefined }), /JSON value/)
  assert.throws(() => validateToolResult(() => undefined), /JSON value/)
})
