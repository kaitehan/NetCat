import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const patchUrl = new URL('../profile/cordis.patch.yml', import.meta.url)

test('netcat profile exposes only the approved domain tools', async () => {
  const source = await readFile(patchUrl, 'utf8')
  const allowed = [
    'netcat_parse_curl',
    'netcat_get_exchange',
    'netcat_find_similar_exchanges',
    'netcat_get_diagnostics',
    'netcat_compare_exchanges',
    'netcat_create_mock_draft',
    'netcat_create_script_draft',
    'netcat_execute_curl'
  ]
  for (const name of allowed) assert.match(source, new RegExp(name))
  for (const forbidden of ['tool-bash', 'tool-fs', 'tool-web', 'tool-browser', 'tool-plugin-manager']) {
    assert.doesNotMatch(source, new RegExp(`name:\\s+['\"]?@deepseek-ai/dsh-${forbidden}`))
  }
})
