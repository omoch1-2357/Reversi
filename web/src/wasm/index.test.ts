import { beforeEach, describe, expect, it, vi } from 'vitest'

const wasmMock = vi.hoisted(() => ({
  init: vi.fn(),
  ai_move: vi.fn(),
  get_legal_moves: vi.fn(),
  get_result: vi.fn(),
  init_game: vi.fn(),
  place_stone: vi.fn(),
  wasm_ready: vi.fn(),
}))

vi.mock('./pkg/reversi', () => ({
  default: wasmMock.init,
  ai_move: wasmMock.ai_move,
  get_legal_moves: wasmMock.get_legal_moves,
  get_result: wasmMock.get_result,
  init_game: wasmMock.init_game,
  place_stone: wasmMock.place_stone,
  wasm_ready: wasmMock.wasm_ready,
}))

const validGameState = () => ({
  board: Array.from({ length: 64 }, () => 0),
  current_player: 1,
  black_count: 2,
  white_count: 2,
  is_game_over: false,
  is_pass: false,
  flipped: [27, 28],
})

const validGameResult = () => ({
  winner: 1,
  black_count: 40,
  white_count: 24,
})

const loadWrapper = async () => import('./index')

describe('wasm wrapper', () => {
  beforeEach(() => {
    vi.resetModules()
    vi.clearAllMocks()
    wasmMock.init.mockResolvedValue({ ok: true })
    wasmMock.wasm_ready.mockReturnValue(true)
    wasmMock.init_game.mockReturnValue(validGameState())
    wasmMock.get_legal_moves.mockReturnValue([{ row: 2, col: 3 }])
    wasmMock.place_stone.mockReturnValue(validGameState())
    wasmMock.ai_move.mockReturnValue(validGameState())
    wasmMock.get_result.mockReturnValue(validGameResult())
  })

  it('ensureWasmModuleLoaded caches successful initialization', async () => {
    const wrapper = await loadWrapper()

    const first = wrapper.ensureWasmModuleLoaded()
    const second = wrapper.ensureWasmModuleLoaded()

    await expect(first).resolves.toEqual({ ok: true })
    await expect(second).resolves.toEqual({ ok: true })
    expect(first).toBe(second)
    expect(wasmMock.init).toHaveBeenCalledTimes(1)
  })

  it('ensureWasmModuleLoaded clears cache on rejection and allows retry', async () => {
    const wrapper = await loadWrapper()
    wasmMock.init.mockRejectedValueOnce(new Error('init failed'))
    wasmMock.init.mockResolvedValueOnce({ ok: 'retry' })

    await expect(wrapper.ensureWasmModuleLoaded()).rejects.toThrow('init failed')
    await expect(wrapper.ensureWasmModuleLoaded()).resolves.toEqual({ ok: 'retry' })
    expect(wasmMock.init).toHaveBeenCalledTimes(2)
  })

  it('wasmReady returns underlying wasm_ready status', async () => {
    const wrapper = await loadWrapper()
    wasmMock.wasm_ready.mockReturnValueOnce(false)

    expect(wrapper.wasmReady()).toBe(false)
    expect(wasmMock.wasm_ready).toHaveBeenCalledTimes(1)
  })

  it('initGame validates level before readiness check', async () => {
    const wrapper = await loadWrapper()

    expect(() => wrapper.initGame(0)).toThrow(
      'level must be an integer between 1 and 6',
    )
    expect(() => wrapper.initGame(7)).toThrow(
      'level must be an integer between 1 and 6',
    )
    expect(() => wrapper.initGame(1.5)).toThrow(
      'level must be an integer between 1 and 6',
    )
    expect(wasmMock.wasm_ready).not.toHaveBeenCalled()
    expect(wasmMock.init_game).not.toHaveBeenCalled()
  })

  it('initGame accepts boundary levels and returns validated GameState', async () => {
    const wrapper = await loadWrapper()

    const minLevelState = wrapper.initGame(1)
    const maxLevelState = wrapper.initGame(6)

    expect(minLevelState.board).toHaveLength(64)
    expect(maxLevelState.board).toHaveLength(64)
    expect(wasmMock.init_game).toHaveBeenNthCalledWith(1, 1)
    expect(wasmMock.init_game).toHaveBeenNthCalledWith(2, 6)
  })

  it('initGame throws when wasm module is not initialized', async () => {
    const wrapper = await loadWrapper()
    wasmMock.wasm_ready.mockReturnValueOnce(false)

    expect(() => wrapper.initGame(1)).toThrow(
      'WASM module is not initialized. Call ensureWasmModuleLoaded() first.',
    )
    expect(wasmMock.init_game).not.toHaveBeenCalled()
  })

  it('getLegalMoves returns positions and rejects non-array value', async () => {
    const wrapper = await loadWrapper()

    expect(wrapper.getLegalMoves()).toEqual([{ row: 2, col: 3 }])

    wasmMock.get_legal_moves.mockReturnValueOnce({ row: 2, col: 3 })
    expect(() => wrapper.getLegalMoves()).toThrow(
      'get_legal_moves return value must be an array',
    )
  })

  it('getLegalMoves rejects invalid position entry type', async () => {
    const wrapper = await loadWrapper()
    wasmMock.get_legal_moves.mockReturnValueOnce([{ row: 2, col: 'x' }])

    expect(() => wrapper.getLegalMoves()).toThrow('Position[0].col must be a number')
  })

  it('placeStone returns GameState and validates board length', async () => {
    const wrapper = await loadWrapper()

    expect(wrapper.placeStone(2, 3).board).toHaveLength(64)

    wasmMock.place_stone.mockReturnValueOnce({
      ...validGameState(),
      board: Array.from({ length: 63 }, () => 0),
    })
    expect(() => wrapper.placeStone(2, 3)).toThrow(
      'GameState.board must contain 64 items, got 63',
    )
  })

  it('placeStone rejects non-array fields validated by asNumberArray', async () => {
    const wrapper = await loadWrapper()
    wasmMock.place_stone.mockReturnValueOnce({
      ...validGameState(),
      flipped: 'not-an-array',
    })

    expect(() => wrapper.placeStone(2, 3)).toThrow('GameState.flipped must be an array')
  })

  it('aiMove rejects invalid boolean field in GameState', async () => {
    const wrapper = await loadWrapper()
    wasmMock.ai_move.mockReturnValueOnce({
      ...validGameState(),
      is_game_over: 'false',
    })

    expect(() => wrapper.aiMove()).toThrow('GameState.is_game_over must be a boolean')
  })

  it('placeStone rejects NaN entry in GameState numeric array', async () => {
    const wrapper = await loadWrapper()
    wasmMock.place_stone.mockReturnValueOnce({
      ...validGameState(),
      flipped: [1, Number.NaN],
    })

    expect(() => wrapper.placeStone(0, 0)).toThrow(
      'GameState.flipped[1] must be a number',
    )
  })

  it('getResult returns GameResult and validates numeric fields', async () => {
    const wrapper = await loadWrapper()

    expect(wrapper.getResult()).toEqual(validGameResult())

    wasmMock.get_result.mockReturnValueOnce({
      ...validGameResult(),
      winner: Number.NaN,
    })
    expect(() => wrapper.getResult()).toThrow('GameResult.winner must be a number')
  })

  it('getResult rejects invalid object shape', async () => {
    const wrapper = await loadWrapper()
    wasmMock.get_result.mockReturnValueOnce([])

    expect(() => wrapper.getResult()).toThrow('get_result return value must be an object')
  })
})
