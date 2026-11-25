import assert from 'node:assert/strict'
import { access, lstat, mkdtemp, mkdir, readFile, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  buildRuntimeManifest,
  declareRuntimeDependency,
  materializeRuntimePackage,
  prepareRuntimeLayout,
  verifyRuntimeFiles
} from '../runtime/src/bundle.ts'

test('runtime materializes local plugin links for desktop packaging', async () => {
  const root = await mkdtemp(join(tmpdir(), 'netcat-runtime-materialize-'))
  const source = join(root, 'packages', 'dsh-plugin')
  const installed = join(root, 'node_modules', '@netcat', 'dsh-plugin')
  await mkdir(source, { recursive: true })
  await mkdir(join(root, 'node_modules', '@netcat'), { recursive: true })
  await writeFile(join(source, 'package.json'), JSON.stringify({ name: '@netcat/dsh-plugin' }))
  await writeFile(join(source, 'credentials.js'), 'export const configured = true\n')
  await symlink('../../packages/dsh-plugin', installed)

  await materializeRuntimePackage(installed, source)

  assert.equal((await lstat(installed)).isSymbolicLink(), false)
  assert.match(await readFile(join(installed, 'credentials.js'), 'utf8'), /configured = true/)
})

test('runtime dependency declaration exposes the bundled plugin to DSH resolution', async () => {
  const root = await mkdtemp(join(tmpdir(), 'netcat-runtime-dependency-'))
  const manifestPath = join(root, 'package.json')
  await writeFile(manifestPath, JSON.stringify({
    name: '@deepseek-ai/dsh',
    dependencies: { existing: '1.0.0' }
  }))

  await declareRuntimeDependency(manifestPath, '@netcat/dsh-plugin', '0.6.5')

  assert.deepEqual(JSON.parse(await readFile(manifestPath, 'utf8')).dependencies, {
    existing: '1.0.0',
    '@netcat/dsh-plugin': '0.6.5'
  })
})

test('runtime manifest hashes every bundled file in stable path order', async () => {
  const root = await mkdtemp(join(tmpdir(), 'netcat-runtime-manifest-'))
  await mkdir(join(root, 'bridge'), { recursive: true })
  await writeFile(join(root, 'bridge', 'index.mjs'), 'export {}\n')
  await writeFile(join(root, 'node'), 'node-binary')

  const manifest = await buildRuntimeManifest(root, {
    platform: 'darwin',
    arch: 'arm64',
    bridgeVersion: '0.6.5'
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
    bridgeVersion: '0.6.5'
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
