/// <reference types="node" />
import { existsSync, readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { GameResult, GameState, Position } from '../wasm'
import {
  createWorkerMessageHandler,
  installWorkerMessageHandler,
  type WorkerDependencies,
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

const wasmPath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../wasm/pkg/reversi_bg.wasm',
)
const reversiModulePath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../wasm/pkg/reversi.js',
)
const hasRealWasmBindings = existsSync(wasmPath) && existsSync(reversiModulePath)
const isCi = process.env.CI === 'true'
const requireRealWasm = process.env.REQUIRE_REAL_WASM === 'true' || isCi

type ReversiBindingsModule = {
  default: (input?: unknown) => Promise<unknown>
  init_game: (level: number) => unknown
  get_legal_moves: () => unknown
  place_stone: (row: number, col: number) => unknown
  ai_move: () => unknown
  get_result: () => unknown
}

let realBindingsModule: ReversiBindingsModule | null = null
let wasmBytes: Uint8Array | null = null
let realWasmInitPromise: ReturnType<WorkerDependencies['ensureWasmModuleLoaded']> | null = null
// Deterministic baseline from real WASM integration: level=1, player always picks
// the first legal move (moves[0]) until game_over, with no RNG involved. These
// values are the resulting final stone counts and winner for that exact flow.
const expectedDeterministicResult: GameResult = {
  winner: 2,
  black_count: 19,
  white_count: 45,
}

const loadRealBindings = async (): Promise<ReversiBindingsModule> => {
  if (!hasRealWasmBindings) {
    throw new Error('real wasm bindings are not available')
  }

  if (realBindingsModule === null) {
    realBindingsModule = (await import(
      pathToFileURL(reversiModulePath).href
    )) as ReversiBindingsModule
  }
  return realBindingsModule
}

const getLoadedBindings = (): ReversiBindingsModule => {
  if (realBindingsModule === null) {
    throw new Error('real wasm bindings are not loaded')
  }
  return realBindingsModule
}

const ensureRealWasmLoaded: WorkerDependencies['ensureWasmModuleLoaded'] = async () => {
  const bindings = await loadRealBindings()
  if (wasmBytes === null) {
    wasmBytes = readFileSync(wasmPath)
  }
  if (realWasmInitPromise === null) {
    realWasmInitPromise = bindings.default(
      wasmBytes,
    ) as ReturnType<WorkerDependencies['ensureWasmModuleLoaded']>
  }
  return realWasmInitPromise
}

const realWasmDependencies: WorkerDependencies = {
  ensureWasmModuleLoaded: ensureRealWasmLoaded,
  initGame: (level: number): GameState => getLoadedBindings().init_game(level) as GameState,
  getLegalMoves: (): Position[] => getLoadedBindings().get_legal_moves() as Position[],
  placeStone: (row: number, col: number): GameState =>
    getLoadedBindings().place_stone(row, col) as GameState,
  aiMove: (): GameState => getLoadedBindings().ai_move() as GameState,
  getResult: (): GameResult => getLoadedBindings().get_result() as GameResult,
}

const runDeterministicGameWithWorkerHandler = async (
  level: number,
): Promise<{
  aiStepFingerprint: string[]
  playerTurns: number
  result: GameResult
}> => {
  const { scope, posted } = makeScope()
  const handler = createWorkerMessageHandler(scope, realWasmDependencies)

  await handler({ data: { type: 'init_game', payload: { level } } })
  let terminalMessage = posted[posted.length - 1]
  if (terminalMessage?.type !== 'game_state') {
    throw new Error('init_game did not produce game_state')
  }

  const aiStepFingerprint: string[] = []
  let playerTurns = 0
  while (terminalMessage.type === 'game_state') {
    const move = terminalMessage.payload.moves[0]
    if (!move) {
      throw new Error('player had no legal moves before game over')
    }

    playerTurns += 1
    const cycleStart = posted.length
    await handler({
      data: {
        type: 'place_stone',
        payload: { row: move.row, col: move.col },
      },
    })

    const cycleMessages = posted.slice(cycleStart)
    for (const message of cycleMessages) {
      if (message.type === 'error') {
        throw new Error(message.payload)
      }
      if (message.type === 'ai_step') {
        aiStepFingerprint.push(
          `${message.payload.state.current_player}:${message.payload.state.black_count}:${message.payload.state.white_count}:${message.payload.state.flipped.join('.')}`,
        )
      }
    }

    const lastCycleMessage = cycleMessages[cycleMessages.length - 1]
    if (lastCycleMessage === undefined) {
      throw new Error('place_stone produced no response')
    }

    if (lastCycleMessage.type === 'game_over') {
      const resultStart = posted.length
      await handler({ data: { type: 'get_result' } })
      const resultMessage = posted[resultStart]
      if (resultMessage?.type !== 'result') {
        throw new Error('get_result did not produce result message')
      }

      return {
        aiStepFingerprint,
        playerTurns,
        result: resultMessage.payload,
      }
    }

    if (lastCycleMessage.type !== 'game_state') {
      throw new Error(`unexpected terminal message: ${lastCycleMessage.type}`)
    }

    terminalMessage = lastCycleMessage
    if (playerTurns > 80) {
      throw new Error('deterministic integration exceeded turn cap')
    }
  }

  throw new Error('game loop terminated unexpectedly')
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

  it('echoes requestId in worker responses', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const requestId = 'req-123'
    const nextState = makeGameState({ black_count: 4, white_count: 1 })
    wasmMock.initGame.mockReturnValueOnce(nextState)
    wasmMock.getLegalMoves.mockReturnValueOnce([{ row: 2, col: 3 }])

    await handler({ data: { requestId, type: 'init_game', payload: { level: 2 } } })

    expect(posted).toEqual([
      {
        requestId,
        type: 'game_state',
        payload: {
          state: nextState,
          moves: [{ row: 2, col: 3 }],
        },
      },
    ])
  })

  it('echoes requestId for ai_step responses in place_stone flow', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const requestId = 'req-ai-step'
    const afterPlayerMove = makeGameState({ current_player: 2 })
    const aiStep = makeGameState({ current_player: 1, flipped: [12] })
    wasmMock.placeStone.mockReturnValueOnce(afterPlayerMove)
    wasmMock.aiMove.mockReturnValueOnce(aiStep)
    wasmMock.getLegalMoves.mockReturnValueOnce([{ row: 5, col: 4 }])

    await handler({
      data: { requestId, type: 'place_stone', payload: { row: 2, col: 3 } },
    })

    expect(posted).toEqual([
      {
        requestId,
        type: 'ai_step',
        payload: { state: afterPlayerMove },
      },
      {
        requestId,
        type: 'ai_step',
        payload: { state: aiStep },
      },
      {
        requestId,
        type: 'game_state',
        payload: { state: aiStep, moves: [{ row: 5, col: 4 }] },
      },
    ])
  })

  it('echoes requestId for game_over responses in place_stone flow', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const requestId = 'req-game-over'
    const finalState = makeGameState({ is_game_over: true })
    const result = makeResult()
    wasmMock.placeStone.mockReturnValueOnce(finalState)
    wasmMock.getResult.mockReturnValueOnce(result)

    await handler({
      data: { requestId, type: 'place_stone', payload: { row: 7, col: 7 } },
    })

    expect(posted).toEqual([
      {
        requestId,
        type: 'game_over',
        payload: { state: finalState, result },
      },
    ])
  })

  it('echoes requestId for error responses when wasm throws', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)
    const requestId = 'req-error'
    wasmMock.placeStone.mockImplementationOnce(() => {
      throw new Error('place_stone failed')
    })

    await handler({
      data: { requestId, type: 'place_stone', payload: { row: 1, col: 1 } },
    })

    expect(posted).toEqual([
      {
        requestId,
        type: 'error',
        payload: 'place_stone failed',
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
      { type: 'ai_step', payload: { state: afterPlayerMove } },
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
      { type: 'ai_step', payload: { state: afterPlayerMove } },
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

  it('posts error for init_game without payload before initializing wasm', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)

    await handler({ data: { type: 'init_game' } as unknown as WorkerRequest })

    expect(wasmMock.ensureWasmModuleLoaded).not.toHaveBeenCalled()
    expect(posted).toEqual([{ type: 'error', payload: 'Invalid worker message shape' }])
  })

  it('posts error for place_stone with malformed payload before initializing wasm', async () => {
    const { scope, posted } = makeScope()
    const handler = createWorkerMessageHandler(scope)

    await handler({
      data: { type: 'place_stone', payload: { x: 1 } } as unknown as WorkerRequest,
    })

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

if (requireRealWasm && !hasRealWasmBindings) {
  describe('wasm worker deterministic integration', () => {
    it('requires generated wasm bindings when CI/REQUIRE_REAL_WASM is enabled', () => {
      throw new Error(
        `Missing real wasm bindings for deterministic integration: expected ${reversiModulePath} and ${wasmPath}.`,
      )
    })
  })
} else {
  const describeRealWasm = hasRealWasmBindings ? describe : describe.skip

  describeRealWasm('wasm worker deterministic integration', () => {
    it('produces identical ai steps and final result across repeated runs', async () => {
      const first = await runDeterministicGameWithWorkerHandler(1)
      const second = await runDeterministicGameWithWorkerHandler(1)

      expect(first).toEqual(second)
      expect(first.aiStepFingerprint.length).toBeGreaterThan(0)
      expect(first.result).toEqual(expectedDeterministicResult)
    })
  })
}
