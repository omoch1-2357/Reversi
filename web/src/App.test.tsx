import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import App from './App'
import type { GameResult, GameState, Position } from './wasm'
import type { WorkerRequest, WorkerResponse } from './workers/wasm.worker'

class MockWorker {
  static instances: MockWorker[] = []

  public onmessage: ((event: MessageEvent<WorkerResponse>) => void) | null = null
  public onerror: ((event: ErrorEvent) => void) | null = null
  public postedMessages: WorkerRequest[] = []

  constructor() {
    MockWorker.instances.push(this)
  }

  postMessage(message: WorkerRequest): void {
    this.postedMessages.push(message)
  }

  terminate(): void {}

  emitMessage(message: WorkerResponse): void {
    this.onmessage?.({ data: message } as MessageEvent<WorkerResponse>)
  }

  static latest(): MockWorker {
    const instance = MockWorker.instances.at(-1)
    if (!instance) {
      throw new Error('MockWorker instance was not created')
    }
    return instance
  }

  static reset(): void {
    MockWorker.instances = []
  }
}

const originalWorker = globalThis.Worker

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

const makeMoves = (moves: Position[]): Position[] => moves

const makeResult = (overrides: Partial<GameResult> = {}): GameResult => ({
  winner: 1,
  black_count: 33,
  white_count: 31,
  ...overrides,
})

beforeEach(() => {
  MockWorker.reset()
  Object.defineProperty(globalThis, 'Worker', {
    configurable: true,
    writable: true,
    value: MockWorker,
  })
})

afterEach(() => {
  cleanup()
  if (originalWorker === undefined) {
    delete (globalThis as { Worker?: typeof Worker }).Worker
  } else {
    Object.defineProperty(globalThis, 'Worker', {
      configurable: true,
      writable: true,
      value: originalWorker,
    })
  }
})

describe('App', () => {
  it('starts from level select and transitions to the worker-backed game screen', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: 'Level 4' }))
    await user.click(screen.getByRole('button', { name: 'Start level 4' }))

    expect(screen.getByRole('button', { name: 'Preparing...' })).toBeDisabled()

    const worker = MockWorker.latest()
    const requestId = worker.postedMessages[0].requestId
    expect(typeof requestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId,
        type: 'game_state',
        payload: {
          state: makeState(),
          moves: makeMoves([
            { row: 2, col: 3 },
            { row: 3, col: 2 },
          ]),
        },
      })
    })

    await waitFor(() => {
      expect(screen.getByRole('grid', { name: 'Reversi board' })).toBeInTheDocument()
      expect(screen.getByText('Your turn (Black)')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Restart level 4' })).toBeInTheDocument()
    })
  })

  it('shows init_game failure UI, disables the start button, and retries successfully', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: 'Start level 3' }))

    const worker = MockWorker.latest()
    const firstRequestId = worker.postedMessages[0].requestId
    expect(typeof firstRequestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId: firstRequestId,
        type: 'error',
        payload: 'CRC32 mismatch',
      })
    })

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(
        'モデルデータの読み込みに失敗しました。再試行してください。',
      )
    })
    expect(screen.getByRole('button', { name: 'Start level 3' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Retry initialization' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Retry initialization' }))

    const secondRequestId = worker.postedMessages[1].requestId
    expect(typeof secondRequestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId: secondRequestId,
        type: 'game_state',
        payload: {
          state: makeState(),
          moves: makeMoves([{ row: 2, col: 3 }]),
        },
      })
    })

    await waitFor(() => {
      expect(screen.getByRole('grid', { name: 'Reversi board' })).toBeInTheDocument()
    })
  })

  it('ignores rapid repeated start attempts while the first init request is in flight', () => {
    render(<App />)

    const startButton = screen.getByRole('button', { name: 'Start level 3' })
    fireEvent.click(startButton)
    fireEvent.click(startButton)

    const worker = MockWorker.latest()
    expect(worker.postedMessages).toHaveLength(1)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('can close the result modal without resetting and restart from outside the modal', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: 'Start level 3' }))

    const worker = MockWorker.latest()
    const initRequestId = worker.postedMessages[0].requestId
    expect(typeof initRequestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId: initRequestId,
        type: 'game_state',
        payload: {
          state: makeState(),
          moves: makeMoves([{ row: 2, col: 3 }]),
        },
      })
    })

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Cell 3-4 legal move' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'Cell 3-4 legal move' }))

    const placeRequestId = worker.postedMessages[1].requestId
    expect(typeof placeRequestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId: placeRequestId,
        type: 'ai_step',
        payload: {
          state: makeState({
            current_player: 2,
            black_count: 4,
            white_count: 3,
            flipped: [28],
          }),
        },
      })
    })

    expect(screen.getByRole('status')).toHaveTextContent('AI is thinking...')

    act(() => {
      worker.emitMessage({
        requestId: placeRequestId,
        type: 'game_over',
        payload: {
          state: makeState({
            current_player: 1,
            black_count: 20,
            white_count: 44,
            is_game_over: true,
          }),
          result: makeResult({
            winner: 2,
            black_count: 20,
            white_count: 44,
          }),
        },
      })
    })

    await waitFor(() => {
      expect(screen.getByRole('dialog', { name: 'Game result' })).toBeInTheDocument()
      expect(
        screen.getByRole('dialog', { name: 'Game result' }),
      ).toHaveTextContent('White wins')
    })

    await user.click(screen.getByRole('button', { name: 'Close' }))

    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Game result' })).not.toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Show result popup' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Restart level 3' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'Restart level 3' }))

    const restartRequestId = worker.postedMessages[2].requestId
    expect(typeof restartRequestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId: restartRequestId,
        type: 'game_state',
        payload: {
          state: makeState(),
          moves: makeMoves([{ row: 2, col: 3 }]),
        },
      })
    })

    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Game result' })).not.toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Restart level 3' })).toBeInTheDocument()
    })
  })

  it('shows pass guidance when the turn returns to the player without an AI move', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: 'Start level 3' }))

    const worker = MockWorker.latest()
    const initRequestId = worker.postedMessages[0].requestId
    expect(typeof initRequestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId: initRequestId,
        type: 'game_state',
        payload: {
          state: makeState(),
          moves: makeMoves([{ row: 2, col: 3 }]),
        },
      })
    })

    await user.click(await screen.findByRole('button', { name: 'Cell 3-4 legal move' }))

    const placeRequestId = worker.postedMessages[1].requestId
    expect(typeof placeRequestId).toBe('string')

    act(() => {
      worker.emitMessage({
        requestId: placeRequestId,
        type: 'game_state',
        payload: {
          state: makeState({
            current_player: 1,
            is_pass: true,
            black_count: 5,
            white_count: 2,
          }),
          moves: makeMoves([{ row: 4, col: 5 }]),
        },
      })
    })

    await waitFor(() => {
      expect(screen.getByText('AI passed. Your turn continues.')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Cell 5-6 legal move' })).toBeInTheDocument()
    })
  })
})
