import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const patchUrl = new URL('../profile/cordis.patch.yml', import.meta.url)

test('netcat profile exposes only the approved domain tools', async () => {
  const source = await readFile(patchUrl, 'utf8')
  const allowed = [
    'netcat_search_exchanges',
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
  assert.match(source, /所有面向用户的回复必须使用中文/)
  assert.match(source, /不得要求用户提供 resourceId/)
  assert.match(source, /明确要求在网络请求页面筛选/)
  assert.match(source, /必须调用 netcat_search_exchanges/)
  assert.match(source, /先列出标准化筛选条件/)
  assert.match(source, /不得回答无法操作页面/)
  assert.match(source, /明确要求生成或创建 Mock/)
  assert.match(source, /必须调用 netcat_create_mock_draft/)
  assert.match(source, /按以上信息创建 Mock 规则/)
  for (const forbidden of ['tool-bash', 'tool-fs', 'tool-web', 'tool-browser', 'tool-plugin-manager']) {
    assert.doesNotMatch(source, new RegExp(`name:\\s+['\"]?@deepseek-ai/dsh-${forbidden}`))
  }
})
