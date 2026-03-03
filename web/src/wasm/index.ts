import init, * as reversiWasm from './pkg/reversi'

export const ensureWasmModuleLoaded = async (): Promise<unknown> => init()

export { reversiWasm }
