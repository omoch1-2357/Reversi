import { readFileSync } from 'node:fs'
import { gzipSync } from 'node:zlib'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const wasmPath = path.resolve(scriptDir, '../src/wasm/pkg/reversi_bg.wasm')
const gzipLimitBytes = 10 * 1024 * 1024

const wasmBytes = readFileSync(wasmPath)
const gzipBytes = gzipSync(wasmBytes).byteLength

console.log(`WASM gzip size: ${gzipBytes} bytes`)

if (gzipBytes > gzipLimitBytes) {
  console.error(
    `WASM gzip size ${gzipBytes} exceeds limit ${gzipLimitBytes} (${wasmPath})`,
  )
  process.exit(1)
}
