import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { GameResult, GameState, Position } from '../wasm'
import type { WorkerRequest, WorkerResponse } from '../workers/wasm.worker'
import { useGame } from './useGame'

class MockWorker {
  public onmessage: ((event: MessageEvent<WorkerResponse>) => void) | null = null
  public onerror: (() => void) | null = null
  public postedMessages: WorkerRequest[] = []
  public terminated = false

  public postMessage(message: WorkerRequest): void {
    this.postedMessages.push(message)
  }

  public terminate(): void {
    this.terminated = true
  }

  public emitMessage(message: WorkerResponse): void {
    this.onmessage?.({ data: message } as MessageEvent<WorkerResponse>)
  }
}

const makeState = (overrides: Partial<GameState> = {}): GameState => ({
  board: Array.from({ length: 64 }, () => 0),
  current_player: 1,
  black_count: 2,
  white_count: 2,
  is_game_over: false,
  is_pass: false,
  flipped: [],
  ...overrides,
})

const makeResult = (overrides: Partial<GameResult> = {}): GameResult => ({
  winner: 1,
  black_count: 33,
  white_count: 31,
  ...overrides,
})

const makeMoves = (moves: Position[]): Position[] => moves

describe('useGame', () => {
  it('creates and terminates worker with hook lifecycle', () => {
    const worker = new MockWorker()

    const { unmount } = renderHook(() =>
      useGame({ createWorker: () => worker as unknown as Worker }),
    )

    expect(worker.terminated).toBe(false)
    unmount()
    expect(worker.terminated).toBe(true)
  })

  it('starts game and syncs game_state into React state', async () => {
    const worker = new MockWorker()
    const { result } = renderHook(() =>
      useGame({ createWorker: () => worker as unknown as Worker }),
    )
    const openingState = makeState()
    const openingMoves = makeMoves([
      { row: 2, col: 3 },
      { row: 3, col: 2 },
      { row: 4, col: 5 },
      { row: 5, col: 4 },
    ])

    let startPromise!: Promise<void>
    act(() => {
      startPromise = result.current.startGame(3)
    })

    expect(worker.postedMessages).toHaveLength(1)
    expect(worker.postedMessages[0]).toEqual(
      expect.objectContaining({ type: 'init_game', payload: { level: 3 } }),
    )
    const requestId = worker.postedMessages[0].requestId
    expect(typeof requestId).toBe('string')
    expect(result.current.isThinking).toBe(true)

    act(() => {
      worker.emitMessage({
        requestId,
        type: 'game_state',
        payload: { state: openingState, moves: openingMoves },
      })
    })

    await expect(startPromise).resolves.toBeUndefined()
    await waitFor(() => {
      expect(result.current.gameState).toEqual(openingState)
      expect(result.current.legalMoves).toEqual(openingMoves)
      expect(result.current.isThinking).toBe(false)
      expect(result.current.error).toBeNull()
    })
  })

  it('handles ai_step followed by game_state for placeStone flow', async () => {
    const worker = new MockWorker()
    const { result } = renderHook(() =>
      useGame({ createWorker: () => worker as unknown as Worker }),
    )

    const initialState = makeState()
    const afterAiStep = makeState({ current_player: 2, black_count: 3, white_count: 3 })
    const afterTurn = makeState({ current_player: 1, black_count: 4, white_count: 3 })
    const nextMoves = makeMoves([{ row: 2, col: 2 }])

    let initPromise!: Promise<void>
    act(() => {
      initPromise = result.current.startGame(2)
    })
    const initRequestId = worker.postedMessages[0].requestId
    expect(typeof initRequestId).toBe('string')
    act(() => {
      worker.emitMessage({
        requestId: initRequestId,
        type: 'game_state',
        payload: { state: initialState, moves: [] },
      })
    })
    await expect(initPromise).resolves.toBeUndefined()

    let placePromise!: Promise<void>
    act(() => {
      placePromise = result.current.placeStone(2, 3)
    })
    expect(worker.postedMessages.at(-1)).toEqual(
      expect.objectContaining({ type: 'place_stone', payload: { row: 2, col: 3 } }),
    )
    const placeRequestId = worker.postedMessages.at(-1)?.requestId
    expect(typeof placeRequestId).toBe('string')
    expect(result.current.isThinking).toBe(true)

    act(() => {
      worker.emitMessage({ requestId: placeRequestId, type: 'ai_step', payload: { state: afterAiStep } })
    })
    expect(result.current.gameState).toEqual(afterAiStep)
    expect(result.current.legalMoves).toEqual([])
    expect(result.current.isThinking).toBe(true)

    act(() => {
      worker.emitMessage({
        requestId: placeRequestId,
        type: 'game_state',
        payload: { state: afterTurn, moves: nextMoves },
      })
    })

    await expect(placePromise).resolves.toBeUndefined()
    await waitFor(() => {
      expect(result.current.gameState).toEqual(afterTurn)
      expect(result.current.legalMoves).toEqual(nextMoves)
      expect(result.current.isThinking).toBe(false)
    })
  })

  it('stores game_over result and resolves action promise', async () => {
    const worker = new MockWorker()
    const { result } = renderHook(() =>
      useGame({ createWorker: () => worker as unknown as Worker }),
    )
    const finishedState = makeState({ is_game_over: true })
    const finishedResult = makeResult({ winner: 0 })

    let startPromise!: Promise<void>
    act(() => {
      startPromise = result.current.startGame(1)
    })
    const requestId = worker.postedMessages[0].requestId
    expect(typeof requestId).toBe('string')
    act(() => {
      worker.emitMessage({
        requestId,
        type: 'game_over',
        payload: { state: finishedState, result: finishedResult },
      })
    })

    await expect(startPromise).resolves.toBeUndefined()
    expect(result.current.gameState).toEqual(finishedState)
    expect(result.current.result).toEqual(finishedResult)
    expect(result.current.isThinking).toBe(false)
  })

  it('rejects pending request when worker returns error', async () => {
    const worker = new MockWorker()
    const { result } = renderHook(() =>
      useGame({ createWorker: () => worker as unknown as Worker }),
    )

    let startPromise!: Promise<void>
    act(() => {
      startPromise = result.current.startGame(4)
    })
    const requestId = worker.postedMessages[0].requestId
    expect(typeof requestId).toBe('string')
    act(() => {
      worker.emitMessage({ requestId, type: 'error', payload: 'WASM init failed' })
    })

    await expect(startPromise).rejects.toThrow('WASM init failed')
    expect(result.current.error).toBe('WASM init failed')
    expect(result.current.isThinking).toBe(false)
  })

  it('rejects concurrent requests and keeps first request ownership', async () => {
    const worker = new MockWorker()
    const { result } = renderHook(() =>
      useGame({ createWorker: () => worker as unknown as Worker }),
    )
    const firstState = makeState({ black_count: 5, white_count: 1 })
    const firstMoves = makeMoves([{ row: 2, col: 4 }])

    let firstPromise!: Promise<void>
    act(() => {
      firstPromise = result.current.startGame(5)
    })
    expect(worker.postedMessages).toHaveLength(1)
    const firstRequestId = worker.postedMessages[0].requestId
    expect(typeof firstRequestId).toBe('string')

    let secondPromise!: Promise<void>
    act(() => {
      secondPromise = result.current.placeStone(2, 3)
    })
    await expect(secondPromise).rejects.toThrow('Another worker request is already in progress')
    expect(worker.postedMessages).toHaveLength(1)
    expect(result.current.isThinking).toBe(true)
    expect(result.current.error).toBe('Another worker request is already in progress')

    act(() => {
      worker.emitMessage({
        requestId: firstRequestId,
        type: 'game_state',
        payload: { state: firstState, moves: firstMoves },
      })
    })

    await expect(firstPromise).resolves.toBeUndefined()
    await waitFor(() => {
      expect(result.current.gameState).toEqual(firstState)
      expect(result.current.legalMoves).toEqual(firstMoves)
      expect(result.current.isThinking).toBe(false)
      expect(result.current.error).toBeNull()
    })
  })
})
