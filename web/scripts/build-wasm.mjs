import { existsSync, statSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { dirname, isAbsolute, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const wasmOutputDir = '../web/src/wasm/pkg'
const rustCrateDir = '../rust'

const parseArgs = (argv) => {
  let modelPath = null
  const passthrough = []

  for (let index = 0; index < argv.length; index += 1) {
    const current = argv[index]
    if (current === '--model-path') {
      const next = argv[index + 1]
      if (!next) {
        throw new Error('--model-path requires a value')
      }
      modelPath = next
      index += 1
      continue
    }

    if (current.startsWith('--model-path=')) {
      modelPath = current.slice('--model-path='.length)
      continue
    }

    passthrough.push(current)
  }

  return { modelPath, passthrough }
}

const resolveModelPath = (rawPath) => {
  if (rawPath === null) {
    return null
  }

  const resolved = isAbsolute(rawPath) ? rawPath : resolve(packageRoot, rawPath)
  if (!existsSync(resolved)) {
    throw new Error(`Model file does not exist: ${resolved}`)
  }
  if (!statSync(resolved).isFile()) {
    throw new Error(`Model path must point to a file: ${resolved}`)
  }

  return resolved
}

const run = (command, args, env = process.env) => {
  const result = spawnSync(command, args, {
    cwd: packageRoot,
    env,
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
  const { modelPath, passthrough } = parseArgs(process.argv.slice(2))
  const resolvedModelPath = resolveModelPath(modelPath)
  const env = resolvedModelPath === null
    ? process.env
    : { ...process.env, REVERSI_MODEL_PATH: resolvedModelPath }

  run('wasm-pack', [
    'build',
    rustCrateDir,
    '--target',
    'web',
    '--out-dir',
    wasmOutputDir,
    ...passthrough,
  ], env)
}

main()
