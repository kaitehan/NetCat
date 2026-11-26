import assert from 'node:assert/strict'
import test from 'node:test'

import {
  NETCAT_TOOL_NAMES,
  NetcatToolClient,
  TOOL_PARAMETER_SCHEMAS,
  validateToolArguments,
  validateToolResult
} from '../plugin-netcat/src/index.ts'
import { NetcatCredentialClient } from '../plugin-netcat/src/credentials.ts'

test('plugin exposes only the nine approved NetCat tools', () => {
  assert.deepEqual(NETCAT_TOOL_NAMES, [
    'netcat_search_exchanges',
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

test('each NetCat tool exposes its own least-privilege parameter schema', () => {
  assert.deepEqual(Object.keys(TOOL_PARAMETER_SCHEMAS.netcat_search_exchanges), [
    'query',
    'domain',
    'method',
    'statusGroup',
    'limit'
  ])
  assert.deepEqual(Object.keys(TOOL_PARAMETER_SCHEMAS.netcat_compare_exchanges), [
    'resourceId',
    'otherResourceId'
  ])
  assert.deepEqual(Object.keys(TOOL_PARAMETER_SCHEMAS.netcat_create_mock_draft), [
    'resourceId',
    'name',
    'status',
    'responseBody'
  ])
  assert.deepEqual(Object.keys(TOOL_PARAMETER_SCHEMAS.netcat_create_script_draft), [
    'resourceId',
    'name',
    'source'
  ])
  assert.deepEqual(TOOL_PARAMETER_SCHEMAS.netcat_execute_curl, {
    requestHandle: { type: 'string', required: true }
  })
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

test('credential client resolves only approved references over authenticated IPC', async () => {
  const calls: unknown[] = []
  const client = new NetcatCredentialClient('startup-token', async payload => {
    calls.push(payload)
    return { configured: true, value: 'sk-private' }
  })

  assert.deepEqual(await client.resolve('DEEPSEEK_API_KEY'), {
    value: 'sk-private',
    source: 'netcat-secure-storage'
  })
  assert.deepEqual(calls, [{
    token: 'startup-token',
    method: 'credentials/resolve',
    params: { reference: 'DEEPSEEK_API_KEY' }
  }])
  await assert.rejects(() => client.resolve('UNAPPROVED_API_KEY'), /not allowed/)
})
