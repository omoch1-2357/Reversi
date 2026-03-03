import {
  aiMove,
  ensureWasmModuleLoaded,
  getLegalMoves,
  getResult,
  initGame,
  placeStone,
  type GameResult,
  type GameState,
  type Position,
} from '../wasm'

type InitGameRequest = { type: 'init_game'; payload: { level: number } }
type PlaceStoneRequest = {
  type: 'place_stone'
  payload: { row: number; col: number }
}
type GetResultRequest = { type: 'get_result' }

export type WorkerRequest = InitGameRequest | PlaceStoneRequest | GetResultRequest
type IncomingWorkerRequest = WorkerRequest | { type: string; payload?: unknown }

export type WorkerResponse =
  | { type: 'game_state'; payload: { state: GameState; moves: Position[] } }
  | { type: 'ai_step'; payload: { state: GameState } }
  | { type: 'game_over'; payload: { state: GameState; result: GameResult } }
  | { type: 'result'; payload: GameResult }
  | { type: 'error'; payload: string }

export interface WorkerMessageEvent {
  data: IncomingWorkerRequest
}

export interface WorkerScopeLike {
  onmessage: ((event: WorkerMessageEvent) => void | Promise<void>) | null
  postMessage: (message: WorkerResponse) => void
}

export interface WorkerDependencies {
  ensureWasmModuleLoaded: typeof ensureWasmModuleLoaded
  initGame: typeof initGame
  getLegalMoves: typeof getLegalMoves
  placeStone: typeof placeStone
  aiMove: typeof aiMove
  getResult: typeof getResult
}

const defaultDependencies: WorkerDependencies = {
  ensureWasmModuleLoaded,
  initGame,
  getLegalMoves,
  placeStone,
  aiMove,
  getResult,
}

export const createWorkerMessageHandler = (
  scope: WorkerScopeLike,
  dependencies: WorkerDependencies = defaultDependencies,
): ((event: WorkerMessageEvent) => Promise<void>) => {
  const emitError = (error: unknown): void => {
    const message = error instanceof Error ? error.message : String(error)
    scope.postMessage({ type: 'error', payload: message })
  }

  return async (event: WorkerMessageEvent): Promise<void> => {
    const request = event.data

    try {
      switch (request.type) {
        case 'init_game': {
          await dependencies.ensureWasmModuleLoaded()
          const state = dependencies.initGame((request as InitGameRequest).payload.level)
          const moves = dependencies.getLegalMoves()
          scope.postMessage({ type: 'game_state', payload: { state, moves } })
          return
        }
        case 'place_stone': {
          await dependencies.ensureWasmModuleLoaded()
          const payload = (request as PlaceStoneRequest).payload
          let state = dependencies.placeStone(
            payload.row,
            payload.col,
          )

          if (state.is_game_over) {
            const result = dependencies.getResult()
            scope.postMessage({ type: 'game_over', payload: { state, result } })
            return
          }

          while (state.current_player === 2 && !state.is_game_over) {
            state = dependencies.aiMove()
            scope.postMessage({ type: 'ai_step', payload: { state } })
          }

          if (state.is_game_over) {
            const result = dependencies.getResult()
            scope.postMessage({ type: 'game_over', payload: { state, result } })
            return
          }

          const moves = dependencies.getLegalMoves()
          scope.postMessage({ type: 'game_state', payload: { state, moves } })
          return
        }
        case 'get_result': {
          await dependencies.ensureWasmModuleLoaded()
          const result = dependencies.getResult()
          scope.postMessage({ type: 'result', payload: result })
          return
        }
        default: {
          scope.postMessage({
            type: 'error',
            payload: `Unknown worker message type: ${request.type}`,
          })
          return
        }
      }
    } catch (error: unknown) {
      emitError(error)
    }
  }
}

export const installWorkerMessageHandler = (
  scope: WorkerScopeLike,
  dependencies: WorkerDependencies = defaultDependencies,
): void => {
  scope.onmessage = createWorkerMessageHandler(scope, dependencies)
}

if (typeof document === 'undefined') {
  installWorkerMessageHandler(globalThis as unknown as WorkerScopeLike)
}
