import { useCallback, useEffect, useRef, useState } from 'react'
import type { GameResult, GameState, Position } from '../wasm'
import type { WorkerRequest, WorkerResponse } from '../workers/wasm.worker'
import workerUrl from '../workers/wasm.worker.ts?worker&url'

export interface GameHook {
  gameState: GameState | null
  legalMoves: Position[]
  isThinking: boolean
  error: string | null
  result: GameResult | null
  startGame: (level: number) => Promise<void>
  placeStone: (row: number, col: number) => Promise<void>
  restart: () => Promise<void>
}

export interface UseGameOptions {
  createWorker?: () => Worker
}

interface PendingRequest {
  resolve: () => void
  reject: (reason?: unknown) => void
}

const DEFAULT_LEVEL = 1
const createDefaultWorker = (): Worker => new Worker(workerUrl, { type: 'module' })

const isValidLevel = (level: number): boolean =>
  Number.isInteger(level) && level >= 1 && level <= 6

const isValidCell = (value: number): boolean =>
  Number.isInteger(value) && value >= 0 && value <= 7

const isTerminalResponse = (type: WorkerResponse['type']): boolean =>
  type === 'game_state' || type === 'game_over' || type === 'result' || type === 'error'

export const useGame = (options: UseGameOptions = {}): GameHook => {
  const createWorkerRef = useRef(options.createWorker ?? createDefaultWorker)
  const workerRef = useRef<Worker | null>(null)
  const levelRef = useRef(DEFAULT_LEVEL)
  const pendingRequestRef = useRef<PendingRequest | null>(null)

  const [gameState, setGameState] = useState<GameState | null>(null)
  const [legalMoves, setLegalMoves] = useState<Position[]>([])
  const [isThinking, setIsThinking] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<GameResult | null>(null)

  const settlePendingRequest = useCallback((response: WorkerResponse): void => {
    if (!isTerminalResponse(response.type)) {
      return
    }

    const pendingRequest = pendingRequestRef.current
    if (pendingRequest === null) {
      return
    }

    pendingRequestRef.current = null
    if (response.type === 'error') {
      pendingRequest.reject(new Error(response.payload))
      return
    }

    pendingRequest.resolve()
  }, [])

  const handleWorkerResponse = useCallback(
    (response: WorkerResponse): void => {
      switch (response.type) {
        case 'game_state':
          setGameState(response.payload.state)
          setLegalMoves(response.payload.moves)
          setIsThinking(false)
          setError(null)
          break
        case 'ai_step':
          setGameState(response.payload.state)
          setLegalMoves([])
          setIsThinking(true)
          setError(null)
          break
        case 'game_over':
          setGameState(response.payload.state)
          setLegalMoves([])
          setResult(response.payload.result)
          setIsThinking(false)
          setError(null)
          break
        case 'result':
          setResult(response.payload)
          setIsThinking(false)
          setError(null)
          break
        case 'error':
          setIsThinking(false)
          setError(response.payload)
          break
      }

      settlePendingRequest(response)
    },
    [settlePendingRequest],
  )

  const sendRequest = useCallback((request: WorkerRequest): Promise<void> => {
    const worker = workerRef.current
    if (worker === null) {
      const notReadyError = new Error('Worker is not initialized')
      setError(notReadyError.message)
      return Promise.reject(notReadyError)
    }

    setError(null)
    setIsThinking(true)

    return new Promise((resolve, reject) => {
      if (pendingRequestRef.current !== null) {
        pendingRequestRef.current.reject(new Error('Previous worker request was cancelled'))
      }
      pendingRequestRef.current = { resolve, reject }
      worker.postMessage(request)
    })
  }, [])

  useEffect(() => {
    const worker = createWorkerRef.current()
    workerRef.current = worker

    worker.onmessage = (event: MessageEvent<WorkerResponse>): void => {
      handleWorkerResponse(event.data)
    }

    worker.onerror = (): void => {
      const message = 'Worker runtime error'
      setIsThinking(false)
      setError(message)

      if (pendingRequestRef.current !== null) {
        pendingRequestRef.current.reject(new Error(message))
        pendingRequestRef.current = null
      }
    }

    return () => {
      if (pendingRequestRef.current !== null) {
        pendingRequestRef.current.reject(new Error('Worker terminated'))
        pendingRequestRef.current = null
      }
      worker.terminate()
      workerRef.current = null
    }
  }, [handleWorkerResponse])

  const startGame = useCallback(
    async (level: number): Promise<void> => {
      if (!isValidLevel(level)) {
        const levelError = new Error('level must be an integer between 1 and 6')
        setError(levelError.message)
        return Promise.reject(levelError)
      }

      levelRef.current = level
      setResult(null)
      setLegalMoves([])
      await sendRequest({ type: 'init_game', payload: { level } })
    },
    [sendRequest],
  )

  const placeStoneAction = useCallback(
    async (row: number, col: number): Promise<void> => {
      if (!isValidCell(row) || !isValidCell(col)) {
        const coordinateError = new Error('row and col must be integers in range 0..7')
        setError(coordinateError.message)
        return Promise.reject(coordinateError)
      }

      await sendRequest({ type: 'place_stone', payload: { row, col } })
    },
    [sendRequest],
  )

  const restart = useCallback(async (): Promise<void> => {
    setResult(null)
    setLegalMoves([])
    await sendRequest({ type: 'init_game', payload: { level: levelRef.current } })
  }, [sendRequest])

  return {
    gameState,
    legalMoves,
    isThinking,
    error,
    result,
    startGame,
    placeStone: placeStoneAction,
    restart,
  }
}

export default useGame
