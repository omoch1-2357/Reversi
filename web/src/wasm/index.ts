import init, {
  ai_move as wasmAiMove,
  get_legal_moves as wasmGetLegalMoves,
  get_result as wasmGetResult,
  init_game as wasmInitGame,
  place_stone as wasmPlaceStone,
  wasm_ready as wasmReadyRaw,
  type InitInput,
  type InitOutput,
} from './pkg/reversi'

export interface Position {
  row: number
  col: number
}

export interface GameState {
  board: number[]
  current_player: number
  black_count: number
  white_count: number
  is_game_over: boolean
  is_pass: boolean
  flipped: number[]
}

export interface GameResult {
  winner: number
  black_count: number
  white_count: number
}

type UnknownRecord = Record<string, unknown>

let wasmInitPromise: Promise<InitOutput> | null = null

export const ensureWasmModuleLoaded = (
  input?: InitInput | Promise<InitInput>,
): Promise<InitOutput> => {
  if (wasmInitPromise === null) {
    wasmInitPromise = init(input).catch((error: unknown) => {
      wasmInitPromise = null
      throw error
    })
  }
  return wasmInitPromise
}

export const wasmReady = (): boolean => wasmReadyRaw()

export const initGame = (level: number): GameState => {
  assertValidLevel(level)
  assertWasmReady()
  return asGameState(wasmInitGame(level), 'init_game')
}

export const getLegalMoves = (): Position[] => {
  assertWasmReady()
  return asPositions(wasmGetLegalMoves(), 'get_legal_moves')
}

export const placeStone = (row: number, col: number): GameState => {
  assertValidBoardCoordinate(row, 'row')
  assertValidBoardCoordinate(col, 'col')
  assertWasmReady()
  return asGameState(wasmPlaceStone(row, col), 'place_stone')
}

export const aiMove = (): GameState => {
  assertWasmReady()
  return asGameState(wasmAiMove(), 'ai_move')
}

export const getResult = (): GameResult => {
  assertWasmReady()
  return asGameResult(wasmGetResult(), 'get_result')
}

const assertWasmReady = (): void => {
  if (!wasmReadyRaw()) {
    throw new Error(
      'WASM module is not initialized. Call ensureWasmModuleLoaded() first.',
    )
  }
}

const asGameState = (value: unknown, source: string): GameState => {
  const obj = asRecord(value, `${source} return value`)

  const board = asNumberArray(obj.board, 'GameState.board')
  if (board.length !== 64) {
    throw new Error(`GameState.board must contain 64 items, got ${board.length}`)
  }

  return {
    board,
    current_player: asNumber(obj.current_player, 'GameState.current_player'),
    black_count: asNumber(obj.black_count, 'GameState.black_count'),
    white_count: asNumber(obj.white_count, 'GameState.white_count'),
    is_game_over: asBoolean(obj.is_game_over, 'GameState.is_game_over'),
    is_pass: asBoolean(obj.is_pass, 'GameState.is_pass'),
    flipped: asNumberArray(obj.flipped, 'GameState.flipped'),
  }
}

const asGameResult = (value: unknown, source: string): GameResult => {
  const obj = asRecord(value, `${source} return value`)
  return {
    winner: asNumber(obj.winner, 'GameResult.winner'),
    black_count: asNumber(obj.black_count, 'GameResult.black_count'),
    white_count: asNumber(obj.white_count, 'GameResult.white_count'),
  }
}

const asPositions = (value: unknown, source: string): Position[] => {
  if (!Array.isArray(value)) {
    throw new Error(`${source} return value must be an array`)
  }

  return value.map((entry, index) => {
    const obj = asRecord(entry, `Position[${index}]`)
    return {
      row: asNumber(obj.row, `Position[${index}].row`),
      col: asNumber(obj.col, `Position[${index}].col`),
    }
  })
}

const asRecord = (value: unknown, label: string): UnknownRecord => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${label} must be an object`)
  }
  return value as UnknownRecord
}

const asNumber = (value: unknown, label: string): number => {
  if (typeof value !== 'number' || Number.isNaN(value)) {
    throw new Error(`${label} must be a number`)
  }
  return value
}

const asBoolean = (value: unknown, label: string): boolean => {
  if (typeof value !== 'boolean') {
    throw new Error(`${label} must be a boolean`)
  }
  return value
}

const asNumberArray = (value: unknown, label: string): number[] => {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`)
  }

  return value.map((entry, index) => asNumber(entry, `${label}[${index}]`))
}

const assertValidLevel = (level: number): void => {
  if (!Number.isInteger(level) || level < 1 || level > 6) {
    throw new Error('level must be an integer between 1 and 6')
  }
}

const assertValidBoardCoordinate = (value: number, name: 'row' | 'col'): void => {
  if (!Number.isInteger(value) || value < 0 || value > 7) {
    throw new Error(
      `placeStone: ${name} out of bounds (expected integer in range 0..7)`,
    )
  }
}
