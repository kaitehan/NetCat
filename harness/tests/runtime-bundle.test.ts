import assert from 'node:assert/strict'
import { access, mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  buildRuntimeManifest,
  prepareRuntimeLayout,
  verifyRuntimeFiles
} from '../runtime/src/bundle.ts'

test('runtime manifest hashes every bundled file in stable path order', async () => {
  const root = await mkdtemp(join(tmpdir(), 'netcat-runtime-manifest-'))
  await mkdir(join(root, 'bridge'), { recursive: true })
  await writeFile(join(root, 'bridge', 'index.mjs'), 'export {}\n')
  await writeFile(join(root, 'node'), 'node-binary')

  const manifest = await buildRuntimeManifest(root, {
    platform: 'darwin',
    arch: 'arm64',
    bridgeVersion: '0.6.0'
  })

  assert.deepEqual(manifest.files.map(file => file.path), [
    'bridge/index.mjs',
    'node'
  ])
  assert.equal(manifest.dshVersion, '0.2.3-alpha.2')
  assert.equal(manifest.nodeVersion, '24.22.3')
  await verifyRuntimeFiles(root, manifest)
})

test('runtime verification rejects a file changed after manifest generation', async () => {
  const root = await mkdtemp(join(tmpdir(), 'netcat-runtime-tamper-'))
  await writeFile(join(root, 'node'), 'original')
  const manifest = await buildRuntimeManifest(root, {
    platform: 'win32',
    arch: 'x64',
    bridgeVersion: '0.6.0'
  })

  await writeFile(join(root, 'node'), 'tampered')
  await assert.rejects(verifyRuntimeFiles(root, manifest), /hash mismatch.*node/i)
  assert.equal(JSON.parse(await readFile(join(root, 'manifest.json'), 'utf8')).platform, 'win32')
})

test('runtime layout keeps the bridge beside its production dependency tree', async () => {
  const fixture = await mkdtemp(join(tmpdir(), 'netcat-runtime-source-'))
  const output = await mkdtemp(join(tmpdir(), 'netcat-runtime-output-'))
  await mkdir(join(fixture, 'bridge'), { recursive: true })
  await mkdir(join(fixture, 'plugin'), { recursive: true })
  await mkdir(join(fixture, 'profile'), { recursive: true })
  await writeFile(join(fixture, 'node'), 'node')
  await writeFile(join(fixture, 'bridge', 'index.js'), 'export {}\n')
  await writeFile(join(fixture, 'bridge', 'controller.js'), 'export {}\n')
  await writeFile(join(fixture, 'plugin', 'index.js'), 'export {}\n')
  await writeFile(join(fixture, 'plugin', 'package.json'), JSON.stringify({ name: '@netcat/dsh-plugin' }))
  await writeFile(join(fixture, 'profile', 'cordis.patch.yml'), '[]\n')
  await writeFile(join(fixture, 'profile', 'package.json'), JSON.stringify({ name: '@netcat/assistant-profile' }))

  await prepareRuntimeLayout({
    output,
    nodeExecutable: join(fixture, 'node'),
    bridgeDirectory: join(fixture, 'bridge'),
    pluginDirectory: join(fixture, 'plugin'),
    profileDirectory: join(fixture, 'profile'),
    platform: 'darwin'
  })

  assert.match(await readFile(join(output, 'bridge', 'index.mjs'), 'utf8'), /runtime\/bridge\/index\.js/)
  await access(join(output, 'bin', 'node'))
  await access(join(output, 'runtime', 'bridge', 'controller.js'))
  await access(join(output, 'runtime', 'packages', 'dsh-plugin', 'index.js'))
  await access(join(output, 'profile-template', 'cordis.patch.yml'))
  const runtimePackage = JSON.parse(await readFile(join(output, 'runtime', 'package.json'), 'utf8'))
  assert.equal(runtimePackage.dependencies['@deepseek-ai/dsh-sdk-client'], '0.2.3-alpha.2')
  assert.equal(runtimePackage.dependencies['@netcat/dsh-plugin'], 'file:./packages/dsh-plugin')
})
