import { spawnSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const nodeExecutable = process.execPath
const buildWasmScript = resolve(packageRoot, 'scripts/build-wasm.mjs')
const tscCli = resolve(packageRoot, 'node_modules/typescript/bin/tsc')
const viteCli = resolve(packageRoot, 'node_modules/vite/bin/vite.js')

const run = (command, args) => {
  const result = spawnSync(command, args, {
    cwd: packageRoot,
    stdio: 'inherit',
  })

  if (result.error) {
    throw result.error
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

const main = () => {
  const forwardedArgs = process.argv.slice(2)

  run(nodeExecutable, [buildWasmScript, ...forwardedArgs])
  run(nodeExecutable, [tscCli, '-b'])
  run(nodeExecutable, [viteCli, 'build'])
}

main()
