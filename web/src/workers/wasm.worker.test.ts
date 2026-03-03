import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { GameResult, GameState, Position } from '../wasm'
import {
  createWorkerMessageHandler,
  installWorkerMessageHandler,
  type WorkerRequest,
  type WorkerResponse,
  type WorkerScopeLike,
} from './wasm.worker'

const wasmMock = vi.hoisted(() => ({
  ensureWasmModuleLoaded: vi.fn(),
  initGame: vi.fn(),
  getLegalMoves: vi.fn(),
  placeStone: vi.fn(),
  aiMove: vi.fn(),
  getResult: vi.fn(),
}))

vi.mock('../wasm', () => ({
  ensureWasmModuleLoaded: wasmMock.ensureWasmModuleLoaded,
  initGame: wasmMock.initGame,
  getLegalMoves: wasmMock.getLegalMoves,
  placeStone: wasmMock.placeStone,
  aiMove: wasmMock.aiMove,
  getResult: wasmMock.getResult,
}))

const makeGameState = (overrides: Partial<GameState> = {}): GameState => ({
  board: Array.from({ length: 64 }, () => 0),
  current_player: 1,
  black_count: 2,
  white_count: 2,
  is_game_over: false,
  is_pass: false,
  flipped: [],
  ...overrides,
})

const makeResult = (): GameResult => ({
  winner: 1,
  black_count: 40,
  white_count: 24,
})

const makeScope = (): { scope: WorkerScopeLike; posted: WorkerResponse[] } => {
  const posted: WorkerResponse[] = []
  return {
    scope: {
      onmessage: null,
      postMessage: (message: WorkerResponse): void => {
        posted.push(message)
      },
    },
    posted,
  }
}

describe('wasm worker handler', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    wasmMock.ensureWasmModuleLoaded.mockResolvedValue(undefined)
    wasmMock.initGame.mockReturnValue(makeGameState())
    wasmMock.getLegalMoves.mockReturnValue([{ row: 2, col: 3 } satisfies Position])
    wasmMock.placeStone.mockReturnValue(makeGameState({ current_player: 1 }))
    wasmMock.aiMove.mockReturnValue(makeGameState({ current_player: 1 }))
    wasmMock.getResult.mockReturnValue(makeResult())
  })

  it('handles init_game and posts game_state with legal moves', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)

    await handler({ data: { type: 'init_game', payload: { level: 3 } } })

    expect(wasmMock.ensureWasmModuleLoaded).toHaveBeenCalledTimes(1)
    expect(wasmMock.initGame).toHaveBeenCalledWith(3)
    expect(wasmMock.getLegalMoves).toHaveBeenCalledTimes(1)
    expect(posted).toEqual([
      {
        type: 'game_state',
        payload: {
          state: makeGameState(),
          moves: [{ row: 2, col: 3 }],
        },
      },
    ])
  })

  it('handles place_stone and posts game_state when turn returns to player', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const nextState = makeGameState({ current_player: 1 })
    wasmMock.placeStone.mockReturnValueOnce(nextState)
    wasmMock.getLegalMoves.mockReturnValueOnce([{ row: 4, col: 5 }])

    await handler({ data: { type: 'place_stone', payload: { row: 2, col: 3 } } })

    expect(wasmMock.placeStone).toHaveBeenCalledWith(2, 3)
    expect(wasmMock.aiMove).not.toHaveBeenCalled()
    expect(posted).toEqual([
      {
        type: 'game_state',
        payload: {
          state: nextState,
          moves: [{ row: 4, col: 5 }],
        },
      },
    ])
  })

  it('runs AI loop and emits ai_step for each AI move before returning game_state', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const afterPlayerMove = makeGameState({ current_player: 2 })
    const aiStep1 = makeGameState({ current_player: 2, flipped: [12] })
    const aiStep2 = makeGameState({ current_player: 1, flipped: [22, 23] })
    wasmMock.placeStone.mockReturnValueOnce(afterPlayerMove)
    wasmMock.aiMove.mockReturnValueOnce(aiStep1).mockReturnValueOnce(aiStep2)
    wasmMock.getLegalMoves.mockReturnValueOnce([{ row: 5, col: 4 }])

    await handler({ data: { type: 'place_stone', payload: { row: 2, col: 3 } } })

    expect(wasmMock.aiMove).toHaveBeenCalledTimes(2)
    expect(posted).toEqual([
      { type: 'ai_step', payload: { state: aiStep1 } },
      { type: 'ai_step', payload: { state: aiStep2 } },
      {
        type: 'game_state',
        payload: { state: aiStep2, moves: [{ row: 5, col: 4 }] },
      },
    ])
  })

  it('runs AI loop and posts game_over when AI ends the game', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const afterPlayerMove = makeGameState({ current_player: 2 })
    const finalState = makeGameState({ current_player: 2, is_game_over: true })
    const result = makeResult()
    wasmMock.placeStone.mockReturnValueOnce(afterPlayerMove)
    wasmMock.aiMove.mockReturnValueOnce(finalState)
    wasmMock.getResult.mockReturnValueOnce(result)

    await handler({ data: { type: 'place_stone', payload: { row: 0, col: 0 } } })

    expect(posted).toEqual([
      { type: 'ai_step', payload: { state: finalState } },
      { type: 'game_over', payload: { state: finalState, result } },
    ])
  })

  it('posts error and exits when AI loop exceeds safety cap', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const afterPlayerMove = makeGameState({ current_player: 2 })
    const loopingState = makeGameState({ current_player: 2, is_game_over: false })
    wasmMock.placeStone.mockReturnValueOnce(afterPlayerMove)
    wasmMock.aiMove.mockImplementation(() => loopingState)

    await handler({ data: { type: 'place_stone', payload: { row: 1, col: 1 } } })

    expect(wasmMock.aiMove).toHaveBeenCalledTimes(64)
    expect(wasmMock.getLegalMoves).not.toHaveBeenCalled()
    expect(wasmMock.getResult).not.toHaveBeenCalled()
    expect(posted[posted.length - 1]).toEqual({
      type: 'error',
      payload: 'AI move loop exceeded safety cap (64)',
    })
  })

  it('posts game_over immediately if place_stone ends the game', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const finalState = makeGameState({ is_game_over: true })
    const result = makeResult()
    wasmMock.placeStone.mockReturnValueOnce(finalState)
    wasmMock.getResult.mockReturnValueOnce(result)

    await handler({ data: { type: 'place_stone', payload: { row: 7, col: 7 } } })

    expect(wasmMock.aiMove).not.toHaveBeenCalled()
    expect(posted).toEqual([
      { type: 'game_over', payload: { state: finalState, result } },
    ])
  })

  it('handles get_result and posts result payload', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const result = makeResult()
    wasmMock.getResult.mockReturnValueOnce(result)

    await handler({ data: { type: 'get_result' } })

    expect(posted).toEqual([{ type: 'result', payload: result }])
  })

  it('posts error when message type is unknown', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const unknown = { type: 'unknown' } as unknown as WorkerRequest

    await handler({ data: unknown })

    expect(wasmMock.ensureWasmModuleLoaded).not.toHaveBeenCalled()
    expect(posted).toEqual([
      { type: 'error', payload: 'Unknown worker message type: unknown' },
    ])
  })

  it('posts error for null message data without initializing wasm', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)

    await handler({ data: null as unknown as WorkerRequest })

    expect(wasmMock.ensureWasmModuleLoaded).not.toHaveBeenCalled()
    expect(posted).toEqual([{ type: 'error', payload: 'Invalid worker message shape' }])
  })

  it('posts error for message data missing type without initializing wasm', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)

    await handler({ data: {} as unknown as WorkerRequest })

    expect(wasmMock.ensureWasmModuleLoaded).not.toHaveBeenCalled()
    expect(posted).toEqual([{ type: 'error', payload: 'Invalid worker message shape' }])
  })

  it('installWorkerMessageHandler wires onmessage to worker message logic', async () => {
    const { scope, posted } = makeScope()
    installWorkerMessageHandler(scope)
    const onmessage = scope.onmessage

    expect(onmessage).not.toBeNull()

    await onmessage?.({ data: { type: 'init_game', payload: { level: 3 } } })

    expect(wasmMock.ensureWasmModuleLoaded).toHaveBeenCalledTimes(1)
    expect(wasmMock.initGame).toHaveBeenCalledWith(3)
    expect(wasmMock.getLegalMoves).toHaveBeenCalledTimes(1)
    expect(posted).toEqual([
      {
        type: 'game_state',
        payload: {
          state: makeGameState(),
          moves: [{ row: 2, col: 3 }],
        },
      },
    ])
  })

  it('posts error when wasm initialization fails', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    wasmMock.ensureWasmModuleLoaded.mockRejectedValueOnce(
      new Error('wasm init failed'),
    )

    await handler({ data: { type: 'get_result' } })

    expect(wasmMock.getResult).not.toHaveBeenCalled()
    expect(posted).toEqual([{ type: 'error', payload: 'wasm init failed' }])
  })
})
