import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import test from 'node:test'

const repositoryRoot = resolve(import.meta.dirname, '../..')

test('NetCat profile mounts its credential provider in the sdk-minimal composition', async () => {
  const dshHome = await mkdtemp(join(tmpdir(), 'netcat-dsh-profile-'))
  const result = spawnSync(
    process.execPath,
    [
      resolve(repositoryRoot, 'resources/harness/runtime/node_modules/@deepseek-ai/dsh/lib/bin.js'),
      '--profile',
      'sdk-minimal',
      '--dump-config',
      '--patch',
      resolve(repositoryRoot, 'harness/profile/cordis.patch.yml')
    ],
    {
      cwd: repositoryRoot,
      encoding: 'utf8',
      env: {
        ...process.env,
        DSH_HOME: dshHome
      }
    }
  )

  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stderr, '')
  assert.match(result.stdout, /id: credentials\n\s+name: '@netcat\/dsh-plugin\/credentials'/)
  assert.match(result.stdout, /id: netcat-tools\n\s+name: '@netcat\/dsh-plugin'/)
})
