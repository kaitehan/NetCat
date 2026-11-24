type JsonObject = Record<string, unknown>

export interface RuntimeManifest {
  schemaVersion: 1
  dshVersion: '0.2.3-alpha.2'
  nodeVersion: '24.22.3'
  bridgeVersion: string
  platform: string
  arch: string
  files: Array<{ path: string; sha256: string }>
}

function record(value: unknown): JsonObject {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TypeError('runtime manifest must be an object')
  }
  return value as JsonObject
}

export function validateRuntimeManifest(
  value: unknown,
  target: { platform: string; arch: string }
): RuntimeManifest {
  const manifest = record(value)
  if (manifest.schemaVersion !== 1) throw new TypeError('unsupported schemaVersion')
  if (manifest.dshVersion !== '0.2.3-alpha.2') throw new TypeError('unsupported dshVersion')
  if (manifest.nodeVersion !== '24.22.3') throw new TypeError('unsupported nodeVersion')
  if (manifest.platform !== target.platform) throw new TypeError('runtime platform mismatch')
  if (manifest.arch !== target.arch) throw new TypeError('runtime arch mismatch')
  if (typeof manifest.bridgeVersion !== 'string' || manifest.bridgeVersion.length === 0) {
    throw new TypeError('bridgeVersion must be present')
  }
  if (!Array.isArray(manifest.files) || manifest.files.length === 0) {
    throw new TypeError('runtime files must not be empty')
  }
  for (const file of manifest.files) {
    const entry = record(file)
    if (typeof entry.path !== 'string' || entry.path.length === 0) throw new TypeError('file path is invalid')
    if (typeof entry.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(entry.sha256)) {
      throw new TypeError('file sha256 is invalid')
    }
  }
  return value as RuntimeManifest
}
