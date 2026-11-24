import { cp, mkdir, mkdtemp, readFile, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawn } from 'node:child_process'

import {
  buildRuntimeManifest,
  prepareRuntimeLayout,
  verifyRuntimeFiles
} from '../dist/runtime/src/bundle.js'

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(scriptDirectory, '../..')
const output = resolve(repositoryRoot, process.argv[2] ?? 'resources/harness')

if (process.version !== 'v24.22.3') {
  throw new Error(`Node 24.22.3 is required to build the Harness Runtime; received ${process.version}`)
}
if (!['darwin', 'win32'].includes(process.platform) || !['arm64', 'x64'].includes(process.arch)) {
  throw new Error(`unsupported Harness Runtime target: ${process.platform}-${process.arch}`)
}

async function run(command, args, cwd) {
  await new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, { cwd, stdio: 'inherit' })
    child.once('error', reject)
    child.once('exit', (code, signal) => {
      if (code === 0) resolvePromise()
      else reject(new Error(`${command} exited with ${code ?? signal}`))
    })
  })
}

const staging = await mkdtemp(join(tmpdir(), 'netcat-harness-build-'))
try {
  const plugin = join(staging, 'plugin')
  await mkdir(join(plugin, 'dist'), { recursive: true })
  await cp(join(repositoryRoot, 'harness/plugin-netcat/package.json'), join(plugin, 'package.json'))
  await cp(join(repositoryRoot, 'harness/plugin-netcat/cordis.patch.yml'), join(plugin, 'cordis.patch.yml'))
  await cp(join(repositoryRoot, 'harness/dist/plugin-netcat/src/index.js'), join(plugin, 'dist/index.js'))
  await cp(join(repositoryRoot, 'harness/dist/plugin-netcat/src/index.d.ts'), join(plugin, 'dist/index.d.ts'))

  await prepareRuntimeLayout({
    output,
    nodeExecutable: process.execPath,
    bridgeDirectory: join(repositoryRoot, 'harness/dist/bridge/src'),
    pluginDirectory: plugin,
    profileDirectory: join(repositoryRoot, 'harness/profile'),
    platform: process.platform
  })

  const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm'
  await run(npmCommand, [
    'install',
    '--omit=dev',
    '--ignore-scripts',
    '--no-audit',
    '--no-fund',
    '--package-lock=false'
  ], join(output, 'runtime'))

  const manifest = await buildRuntimeManifest(output, {
    platform: process.platform,
    arch: process.arch,
    bridgeVersion: '0.6.0'
  })
  await verifyRuntimeFiles(output, manifest)
  const bytes = (await Promise.all(manifest.files.map(async file => {
    const path = join(output, ...file.path.split('/'))
    return (await stat(path)).isFile() ? (await stat(path)).size : 0
  }))).reduce((sum, size) => sum + size, 0)
  console.log(JSON.stringify({
    output,
    platform: manifest.platform,
    arch: manifest.arch,
    files: manifest.files.length,
    bytes
  }))
} finally {
  await rm(staging, { recursive: true, force: true })
}
