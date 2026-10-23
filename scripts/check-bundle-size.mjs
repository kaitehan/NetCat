import { readFile, readdir, stat } from 'node:fs/promises'
import { join } from 'node:path'
import { gzip } from 'node:zlib'
import { promisify } from 'node:util'

const gzipAsync = promisify(gzip)
const assetsDirectory = new URL('../dist/assets/', import.meta.url)
const files = await readdir(assetsDirectory)
const javascript = files.filter((file) => file.endsWith('.js'))
const failures = []
let monacoBytes = 0

const manifest = JSON.parse(await readFile(new URL('../dist/.vite/manifest.json', import.meta.url), 'utf8'))
const entry = manifest['index.html'] ?? manifest['src/main.ts']
for (const imported of entry?.imports ?? []) {
  if (/monaco|mock|settings|diagnostic/i.test(imported)) {
    failures.push(`initial entry unexpectedly imports lazy feature chunk ${imported}`)
  }
}

for (const file of javascript) {
  const path = join(assetsDirectory.pathname, file)
  const { size } = await stat(path)
  const isMonaco = /monaco|worker|editor|jsonmode|body-editor|vs-/i.test(file)
  if (isMonaco) {
    monacoBytes += size
  } else if (size > 500_000) {
    failures.push(`${file} is ${(size / 1024).toFixed(1)} KB; regular chunks must stay below 500 KB`)
  }
  if (file.startsWith('index-')) {
    const compressed = await gzipAsync(await import('node:fs/promises').then(({ readFile }) => readFile(path)))
    if (size > 200_000 || compressed.length > 75_000) {
      failures.push(`${file} exceeds the 200 KB raw / 75 KB gzip entry budget`)
    }
  }
}

if (monacoBytes > 5_000_000) {
  failures.push(`Monaco assets total ${(monacoBytes / 1024 / 1024).toFixed(2)} MB; budget is 5 MB`)
}

if (failures.length > 0) {
  console.error(failures.join('\n'))
  process.exitCode = 1
} else {
  console.log(`Bundle budgets passed; Monaco assets ${(monacoBytes / 1024 / 1024).toFixed(2)} MB`)
}
