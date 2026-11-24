import { createHash } from 'node:crypto'
import {
  chmod,
  cp,
  lstat,
  mkdir,
  readFile,
  readdir,
  readlink,
  realpath,
  rm,
  writeFile
} from 'node:fs/promises'
import { dirname, join, relative, resolve, sep } from 'node:path'

export interface RuntimeManifest {
  schemaVersion: 1
  dshVersion: '0.2.3-alpha.2'
  nodeVersion: '24.22.3'
  bridgeVersion: string
  platform: string
  arch: string
  files: Array<{ path: string; sha256: string }>
}

export interface RuntimeManifestMetadata {
  bridgeVersion: string
  platform: string
  arch: string
}

export interface RuntimeLayoutOptions {
  output: string
  nodeExecutable: string
  bridgeDirectory: string
  pluginDirectory: string
  profileDirectory: string
  platform: string
}

export async function prepareRuntimeLayout(options: RuntimeLayoutOptions): Promise<void> {
  await rm(options.output, { recursive: true, force: true })
  await mkdir(join(options.output, 'bin'), { recursive: true })
  await mkdir(join(options.output, 'bridge'), { recursive: true })
  await mkdir(join(options.output, 'runtime'), { recursive: true })

  const nodeName = options.platform === 'win32' ? 'node.exe' : 'node'
  const bundledNode = join(options.output, 'bin', nodeName)
  await cp(options.nodeExecutable, bundledNode)
  if (options.platform !== 'win32') await chmod(bundledNode, 0o755)

  await cp(options.bridgeDirectory, join(options.output, 'runtime', 'bridge'), { recursive: true })
  await cp(options.pluginDirectory, join(options.output, 'runtime', 'packages', 'dsh-plugin'), {
    recursive: true
  })
  await cp(options.profileDirectory, join(options.output, 'profile-template'), { recursive: true })
  await writeFile(
    join(options.output, 'bridge', 'index.mjs'),
    "import '../runtime/bridge/index.js'\n"
  )
  await writeFile(join(options.output, 'runtime', 'package.json'), `${JSON.stringify({
    name: '@netcat/embedded-harness-runtime',
    private: true,
    version: '0.6.0',
    type: 'module',
    dependencies: {
      '@deepseek-ai/dsh-sdk-client': '0.2.3-alpha.2',
      '@deepseek-ai/dsh-sdk-minimal': '0.2.3-alpha.2',
      '@deepseek-ai/dsh-llm-pi-ai': '0.2.3-alpha.2',
      '@netcat/dsh-plugin': 'file:./packages/dsh-plugin'
    }
  }, null, 2)}\n`)
}

function portablePath(path: string): string {
  return path.split(sep).join('/')
}

async function listFiles(root: string, directory = root): Promise<string[]> {
  const paths: string[] = []
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) paths.push(...await listFiles(root, path))
    else if (entry.isFile() || entry.isSymbolicLink()) paths.push(path)
  }
  return paths
    .filter(path => portablePath(relative(root, path)) !== 'manifest.json')
    .sort((left, right) => portablePath(relative(root, left)).localeCompare(portablePath(relative(root, right))))
}

async function fileDigest(root: string, path: string): Promise<string> {
  const info = await lstat(path)
  const hash = createHash('sha256')
  if (info.isSymbolicLink()) {
    const target = await readlink(path)
    const resolvedTarget = await realpath(resolve(dirname(path), target))
    const resolvedRoot = await realpath(root)
    if (resolvedTarget !== resolvedRoot && !resolvedTarget.startsWith(`${resolvedRoot}${sep}`)) {
      throw new Error(`runtime symlink escapes bundle: ${portablePath(relative(root, path))}`)
    }
    hash.update(`symlink:${portablePath(target)}`)
  } else {
    hash.update(await readFile(path))
  }
  return hash.digest('hex')
}

export async function buildRuntimeManifest(
  root: string,
  metadata: RuntimeManifestMetadata
): Promise<RuntimeManifest> {
  const files = []
  for (const path of await listFiles(root)) {
    files.push({
      path: portablePath(relative(root, path)),
      sha256: await fileDigest(root, path)
    })
  }
  const manifest: RuntimeManifest = {
    schemaVersion: 1,
    dshVersion: '0.2.3-alpha.2',
    nodeVersion: '24.22.3',
    bridgeVersion: metadata.bridgeVersion,
    platform: metadata.platform,
    arch: metadata.arch,
    files
  }
  await writeFile(join(root, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`)
  return manifest
}

export async function verifyRuntimeFiles(root: string, manifest: RuntimeManifest): Promise<void> {
  const actualPaths = (await listFiles(root)).map(path => portablePath(relative(root, path)))
  const expectedPaths = manifest.files.map(file => file.path)
  if (JSON.stringify(actualPaths) !== JSON.stringify(expectedPaths)) {
    throw new Error('runtime file list mismatch')
  }
  for (const file of manifest.files) {
    const actual = await fileDigest(root, join(root, ...file.path.split('/')))
    if (actual !== file.sha256) throw new Error(`runtime hash mismatch for ${file.path}`)
  }
}
