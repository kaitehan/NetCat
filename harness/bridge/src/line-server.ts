import { createInterface } from 'node:readline'
import type { Readable } from 'node:stream'

type LineHandler = (line: string) => boolean | Promise<boolean>

export async function serveLines(input: Readable, handleLine: LineHandler): Promise<void> {
  const lines = createInterface({ input, crlfDelay: Infinity, terminal: false })
  try {
    for await (const line of lines) {
      if (await handleLine(line)) break
    }
  } finally {
    lines.close()
  }
}
