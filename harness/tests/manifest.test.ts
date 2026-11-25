import assert from 'node:assert/strict'
import test from 'node:test'

import { validateRuntimeManifest } from '../bridge/src/runtime-manifest.ts'

test('runtime manifest requires exact product and platform compatibility', () => {
  const manifest = {
    schemaVersion: 1,
    dshVersion: '0.2.3-alpha.2',
    nodeVersion: '24.22.3',
    bridgeVersion: '0.6.5',
    platform: 'darwin',
    arch: 'arm64',
    files: [{ path: 'bridge/index.mjs', sha256: 'a'.repeat(64) }]
  }
  assert.deepEqual(validateRuntimeManifest(manifest, {
    platform: 'darwin',
    arch: 'arm64'
  }), manifest)
  assert.throws(() => validateRuntimeManifest(manifest, {
    platform: 'win32',
    arch: 'x64'
  }), /platform/)
  assert.throws(() => validateRuntimeManifest({ ...manifest, files: [] }, {
    platform: 'darwin',
    arch: 'arm64'
  }), /files/)
})
