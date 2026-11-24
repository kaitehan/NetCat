import { mkdtemp, mkdir, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'

import { verifyRuntimeFiles } from '../dist/runtime/src/bundle.js'

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(scriptDirectory, '../..')
const runtimeRoot = resolve(repositoryRoot, process.argv[2] ?? 'resources/harness')
const manifest = JSON.parse(await readFile(join(runtimeRoot, 'manifest.json'), 'utf8'))
if (manifest.platform !== process.platform || manifest.arch !== process.arch) {
  throw new Error(`runtime target ${manifest.platform}-${manifest.arch} cannot run on ${process.platform}-${process.arch}`)
}
await verifyRuntimeFiles(runtimeRoot, manifest)

const home = await mkdtemp(join(tmpdir(), 'netcat-embedded-runtime-'))
const workspace = join(home, 'workspace')
await mkdir(workspace)
const nodeName = process.platform === 'win32' ? 'node.exe' : 'node'
const startedAt = Date.now()
const child = spawn(
  join(runtimeRoot, 'bin', nodeName),
  [join(runtimeRoot, 'bridge', 'index.mjs')],
  {
    cwd: workspace,
    env: {
      PATH: process.env.PATH ?? '',
      SystemRoot: process.env.SystemRoot ?? '',
      TMPDIR: process.env.TMPDIR ?? '',
      TEMP: process.env.TEMP ?? '',
      TMP: process.env.TMP ?? '',
      HOME: process.env.HOME ?? '',
      USERPROFILE: process.env.USERPROFILE ?? '',
      DEEPSEEK_API_KEY: 'smoke-only',
      NETCAT_TOOL_ENDPOINT: join(home, process.platform === 'win32' ? 'tool.pipe' : 'tool.sock'),
      NETCAT_TOOL_TOKEN: 'smoke-only'
    },
    stdio: ['pipe', 'pipe', 'pipe']
  }
)

let stderr = ''
child.stderr.setEncoding('utf8')
child.stderr.on('data', chunk => { stderr += chunk })
const lines = createInterface({ input: child.stdout, crlfDelay: Infinity })

function send(id, method, params) {
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
}

const result = new Promise((resolvePromise, reject) => {
  const timeout = setTimeout(() => reject(new Error(`embedded runtime smoke timed out\n${stderr}`)), 30000)
  lines.on('line', line => {
    try {
      const message = JSON.parse(line)
      if (message.id === 'initialize') {
        if (message.error) throw new Error(message.error.message)
        send('shutdown', 'runtime/shutdown', {})
      } else if (message.id === 'shutdown') {
        if (message.error) throw new Error(message.error.message)
      }
    } catch (error) {
      clearTimeout(timeout)
      reject(error)
    }
  })
  child.once('error', error => {
    clearTimeout(timeout)
    reject(error)
  })
  child.once('exit', (code, signal) => {
    clearTimeout(timeout)
    if (code === 0) resolvePromise({ coldStartMs: Date.now() - startedAt })
    else reject(new Error(`embedded runtime exited with ${code ?? signal}\n${stderr}`))
  })
})

send('initialize', 'runtime/initialize', {
  provider: 'deepseek-official',
  model: 'deepseek-v4-flash',
  dshHome: home,
  cwd: workspace,
  profile: 'sdk-minimal',
  patches: [join(runtimeRoot, 'profile-template', 'cordis.patch.yml')]
})

try {
  console.log(JSON.stringify(await result))
} finally {
  lines.close()
  child.stdin.destroy()
  if (child.exitCode === null) child.kill('SIGKILL')
  await rm(home, { recursive: true, force: true })
}
