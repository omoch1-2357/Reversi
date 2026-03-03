export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module

export interface InitOutput {
  readonly memory: WebAssembly.Memory
}

export const ai_move = (): never => {
  throw new Error('test mock: ai_move is not implemented')
}

export const get_legal_moves = (): never => {
  throw new Error('test mock: get_legal_moves is not implemented')
}

export const get_result = (): never => {
  throw new Error('test mock: get_result is not implemented')
}

export const init_game = (level: number): never => {
  void level
  throw new Error('test mock: init_game is not implemented')
}

export const place_stone = (row: number, col: number): never => {
  void row
  void col
  throw new Error('test mock: place_stone is not implemented')
}

export const wasm_ready = (): boolean => false

const init = async (): Promise<InitOutput> =>
  ({ memory: new WebAssembly.Memory({ initial: 1 }) }) as InitOutput

export default init
