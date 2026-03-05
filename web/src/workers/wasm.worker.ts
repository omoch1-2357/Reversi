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

type RequestWithId = { requestId?: string }

type InitGameRequest = RequestWithId & { type: 'init_game'; payload: { level: number } }
type PlaceStoneRequest = {
  type: 'place_stone'
  payload: { row: number; col: number }
} & RequestWithId
type GetResultRequest = RequestWithId & { type: 'get_result' }

export type WorkerRequest = InitGameRequest | PlaceStoneRequest | GetResultRequest
type IncomingWorkerRequest = WorkerRequest | { type: string; payload?: unknown; requestId?: unknown }

export type WorkerResponse =
  | { requestId?: string; type: 'game_state'; payload: { state: GameState; moves: Position[] } }
  | { requestId?: string; type: 'ai_step'; payload: { state: GameState } }
  | { requestId?: string; type: 'game_over'; payload: { state: GameState; result: GameResult } }
  | { requestId?: string; type: 'result'; payload: GameResult }
  | { requestId?: string; type: 'error'; payload: string }

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

const MAX_AI_STEPS = 64
const INVALID_MESSAGE_SHAPE = 'Invalid worker message shape'

const isIntegerInRange = (value: unknown, min: number, max: number): boolean =>
  typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max

const isValidInitPayload = (payload: unknown): payload is InitGameRequest['payload'] => {
  if (typeof payload !== 'object' || payload === null || !('level' in payload)) {
    return false
  }

  return isIntegerInRange((payload as { level: unknown }).level, 1, 6)
}

const isValidPlaceStonePayload = (
  payload: unknown,
): payload is PlaceStoneRequest['payload'] => {
  if (
    typeof payload !== 'object'
    || payload === null
    || !('row' in payload)
    || !('col' in payload)
  ) {
    return false
  }

  const row = (payload as { row: unknown }).row
  const col = (payload as { col: unknown }).col
  return isIntegerInRange(row, 0, 7) && isIntegerInRange(col, 0, 7)
}

export const createWorkerMessageHandler = (
  scope: WorkerScopeLike,
  dependencies: WorkerDependencies = defaultDependencies,
): ((event: WorkerMessageEvent) => Promise<void>) => {
  const postResponse = (message: WorkerResponse, requestId?: string): void => {
    if (requestId === undefined) {
      scope.postMessage(message)
      return
    }
    scope.postMessage({ ...message, requestId })
  }

  const emitError = (error: unknown, requestId?: string): void => {
    const message = error instanceof Error ? error.message : String(error)
    postResponse({ type: 'error', payload: message }, requestId)
  }

  return async (event: WorkerMessageEvent): Promise<void> => {
    const maybeRequest = event.data as unknown
    if (
      typeof maybeRequest !== 'object'
      || maybeRequest === null
      || !('type' in maybeRequest)
      || typeof (maybeRequest as { type: unknown }).type !== 'string'
    ) {
      scope.postMessage({ type: 'error', payload: INVALID_MESSAGE_SHAPE })
      return
    }

    const request = maybeRequest as IncomingWorkerRequest
    const rawRequestId = (request as { requestId?: unknown }).requestId
    if (rawRequestId !== undefined && typeof rawRequestId !== 'string') {
      scope.postMessage({ type: 'error', payload: INVALID_MESSAGE_SHAPE })
      return
    }
    const requestId = rawRequestId

    try {
      switch (request.type) {
        case 'init_game': {
          const payload = (request as { payload?: unknown }).payload
          if (!isValidInitPayload(payload)) {
            postResponse({ type: 'error', payload: INVALID_MESSAGE_SHAPE }, requestId)
            return
          }

          await dependencies.ensureWasmModuleLoaded()
          const state = dependencies.initGame(payload.level)
          const moves = dependencies.getLegalMoves()
          postResponse({ type: 'game_state', payload: { state, moves } }, requestId)
          return
        }
        case 'place_stone': {
          const payload = (request as { payload?: unknown }).payload
          if (!isValidPlaceStonePayload(payload)) {
            postResponse({ type: 'error', payload: INVALID_MESSAGE_SHAPE }, requestId)
            return
          }

          await dependencies.ensureWasmModuleLoaded()
          let state = dependencies.placeStone(
            payload.row,
            payload.col,
          )

          if (state.is_game_over) {
            const result = dependencies.getResult()
            postResponse({ type: 'game_over', payload: { state, result } }, requestId)
            return
          }

          let aiStepCount = 0
          while (state.current_player === 2 && !state.is_game_over) {
            if (aiStepCount >= MAX_AI_STEPS) {
              postResponse({
                type: 'error',
                payload: `AI move loop exceeded safety cap (${MAX_AI_STEPS})`,
              }, requestId)
              return
            }
            aiStepCount += 1
            state = dependencies.aiMove()
            postResponse({ type: 'ai_step', payload: { state } }, requestId)
          }

          if (state.is_game_over) {
            const result = dependencies.getResult()
            postResponse({ type: 'game_over', payload: { state, result } }, requestId)
            return
          }

          const moves = dependencies.getLegalMoves()
          postResponse({ type: 'game_state', payload: { state, moves } }, requestId)
          return
        }
        case 'get_result': {
          await dependencies.ensureWasmModuleLoaded()
          const result = dependencies.getResult()
          postResponse({ type: 'result', payload: result }, requestId)
          return
        }
        default: {
          postResponse({
            type: 'error',
            payload: `Unknown worker message type: ${request.type}`,
          }, requestId)
          return
        }
      }
    } catch (error: unknown) {
      emitError(error, requestId)
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
