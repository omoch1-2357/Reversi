import { useEffect, useMemo, useRef, useState } from 'react'
import './App.css'
import Board from './components/Board'
import GameInfo from './components/GameInfo'
import LevelSelect from './components/LevelSelect'
import ResultModal from './components/ResultModal'
import { demoAiLogic } from './demoAi'
import { PLAYER_BLACK, PLAYER_WHITE, type Player, type Winner } from './types/player'
import workerUrl from './workers/wasm.worker.ts?worker&url'
import type { Position } from './wasm'

declare global {
  interface Window {
    __reversiWorkerUrl?: string
  }
}

if (typeof window !== 'undefined') {
  window.__reversiWorkerUrl = workerUrl
}

type Screen = 'level_select' | 'game'

interface DemoResult {
  winner: Winner
  black_count: number
  white_count: number
}

const BOARD_CELLS = 64
const BOARD_WIDTH = 8
const OPENING_BLACK = [28, 35]
const OPENING_WHITE = [27, 36]
const DIRECTIONS: Array<[number, number]> = [
  [-1, -1], [-1, 0], [-1, 1],
  [0, -1], [0, 1],
  [1, -1], [1, 0], [1, 1],
]

const createInitialBoard = (): number[] => {
  const board = Array.from({ length: BOARD_CELLS }, () => 0)
  for (const index of OPENING_BLACK) {
    board[index] = PLAYER_BLACK
  }
  for (const index of OPENING_WHITE) {
    board[index] = PLAYER_WHITE
  }
  return board
}

const countStones = (board: number[], stone: number): number =>
  board.reduce((count, cell) => count + (cell === stone ? 1 : 0), 0)

const toPositions = (indices: number[]): Position[] =>
  indices.map((index) => ({
    row: Math.floor(index / BOARD_WIDTH),
    col: index % BOARD_WIDTH,
  }))

const isInBounds = (row: number, col: number): boolean =>
  row >= 0 && row < BOARD_WIDTH && col >= 0 && col < BOARD_WIDTH

const collectFlippedIndices = (board: number[], moveIndex: number, player: Player): number[] => {
  if (moveIndex < 0 || moveIndex >= BOARD_CELLS || board[moveIndex] !== 0) {
    return []
  }

  const row = Math.floor(moveIndex / BOARD_WIDTH)
  const col = moveIndex % BOARD_WIDTH
  const opponent = player === PLAYER_BLACK ? PLAYER_WHITE : PLAYER_BLACK
  const flipped: number[] = []

  for (const [rowDelta, colDelta] of DIRECTIONS) {
    let r = row + rowDelta
    let c = col + colDelta
    const line: number[] = []

    while (isInBounds(r, c)) {
      const index = r * BOARD_WIDTH + c
      const cell = board[index]
      if (cell === opponent) {
        line.push(index)
        r += rowDelta
        c += colDelta
        continue
      }
      if (cell === player && line.length > 0) {
        flipped.push(...line)
      }
      break
    }
  }

  return flipped
}

const applyMove = (
  board: number[],
  moveIndex: number,
  player: Player,
): { board: number[], flipped: number[] } => {
  const flipped = collectFlippedIndices(board, moveIndex, player)
  if (flipped.length === 0) {
    return { board, flipped: [] }
  }

  const nextBoard = board.slice()
  nextBoard[moveIndex] = player
  for (const index of flipped) {
    nextBoard[index] = player
  }
  return { board: nextBoard, flipped }
}

const createInitialLegalMoves = (): Position[] =>
  toPositions(demoAiLogic.getLegalMoveIndices(createInitialBoard(), PLAYER_BLACK))

function App() {
  const [screen, setScreen] = useState<Screen>('level_select')
  const [selectedLevel, setSelectedLevel] = useState(3)
  const [board, setBoard] = useState<number[]>(() => createInitialBoard())
  const [legalMoves, setLegalMoves] = useState<Position[]>(() => createInitialLegalMoves())
  const [flipped, setFlipped] = useState<number[]>([])
  const [currentPlayer, setCurrentPlayer] = useState<Player>(PLAYER_BLACK)
  const [isThinking, setIsThinking] = useState(false)
  const [isResultOpen, setIsResultOpen] = useState(false)
  const [result, setResult] = useState<DemoResult>({
    winner: 0,
    black_count: 2,
    white_count: 2,
  })
  const aiTimerRef = useRef<number | null>(null)

  const blackCount = useMemo(
    () => countStones(board, PLAYER_BLACK),
    [board],
  )
  const whiteCount = useMemo(
    () => countStones(board, PLAYER_WHITE),
    [board],
  )

  useEffect(
    () => () => {
      if (aiTimerRef.current !== null) {
        window.clearTimeout(aiTimerRef.current)
      }
    },
    [],
  )

  const resetDemoGame = (): void => {
    if (aiTimerRef.current !== null) {
      window.clearTimeout(aiTimerRef.current)
      aiTimerRef.current = null
    }
    setBoard(createInitialBoard())
    setLegalMoves(createInitialLegalMoves())
    setFlipped([])
    setCurrentPlayer(PLAYER_BLACK)
    setIsThinking(false)
    setIsResultOpen(false)
  }

  const handleStartGame = (): void => {
    resetDemoGame()
    setScreen('game')
  }

  const handleCellClick = (row: number, col: number): void => {
    if (currentPlayer !== PLAYER_BLACK || isThinking) {
      return
    }

    const selectedIndex = row * BOARD_WIDTH + col
    const legalSet = new Set(legalMoves.map((move) => move.row * BOARD_WIDTH + move.col))
    if (!legalSet.has(selectedIndex)) {
      return
    }

    const playerMove = applyMove(board, selectedIndex, PLAYER_BLACK)
    if (playerMove.flipped.length === 0) {
      return
    }

    const boardAfterPlayerMove = playerMove.board
    const aiLegalMoves = demoAiLogic.getLegalMoveIndices(boardAfterPlayerMove, PLAYER_WHITE)

    setBoard(boardAfterPlayerMove)
    setFlipped(playerMove.flipped)
    setLegalMoves([])
    setCurrentPlayer(PLAYER_WHITE)

    if (aiLegalMoves.length === 0) {
      setCurrentPlayer(PLAYER_BLACK)
      setLegalMoves(
        toPositions(demoAiLogic.getLegalMoveIndices(boardAfterPlayerMove, PLAYER_BLACK)),
      )
      setIsThinking(false)
      return
    }

    setIsThinking(true)

    if (aiTimerRef.current !== null) {
      window.clearTimeout(aiTimerRef.current)
    }

    aiTimerRef.current = window.setTimeout(() => {
      const boardBeforeAiMove = boardAfterPlayerMove.slice()
      const aiLegalMoves = demoAiLogic.getLegalMoveIndices(boardBeforeAiMove, PLAYER_WHITE)
      const aiMoveIndex = demoAiLogic.chooseAIMoveIndex(
        boardBeforeAiMove,
        selectedLevel,
        aiLegalMoves,
      )
      const aiMove =
        aiMoveIndex >= 0
          ? applyMove(boardBeforeAiMove, aiMoveIndex, PLAYER_WHITE)
          : { board: boardBeforeAiMove, flipped: [] }
      const boardAfterAiMove = aiMove.board
      const blackLegalMoves = demoAiLogic.getLegalMoveIndices(boardAfterAiMove, PLAYER_BLACK)

      setBoard(boardAfterAiMove)
      setFlipped(aiMove.flipped)
      setCurrentPlayer(PLAYER_BLACK)
      setIsThinking(false)
      setLegalMoves(toPositions(blackLegalMoves))
      aiTimerRef.current = null
    }, demoAiLogic.getAIDelay(selectedLevel))
  }

  const handlePreviewResult = (): void => {
    if (isThinking) {
      return
    }

    const finalBlack = countStones(board, PLAYER_BLACK)
    const finalWhite = countStones(board, PLAYER_WHITE)
    setResult({
      winner: finalBlack === finalWhite
        ? 0
        : finalBlack > finalWhite
          ? PLAYER_BLACK
          : PLAYER_WHITE,
      black_count: finalBlack,
      white_count: finalWhite,
    })
    setIsResultOpen(true)
  }

  const handleRestart = (): void => {
    resetDemoGame()
  }

  return (
    <div className="app">
      <header className="app__header">
        <p className="app__eyebrow">Reversi</p>
        <h1>Reversi</h1>
        <p className="app__lead">
          Phase 4-4 component preview.
        </p>
      </header>

      {screen === 'level_select' ? (
        <LevelSelect
          selectedLevel={selectedLevel}
          onLevelChange={setSelectedLevel}
          onStart={handleStartGame}
        />
      ) : (
        <main className="game-layout">
          <Board
            board={board}
            legalMoves={legalMoves}
            flipped={flipped}
            isPlayerTurn={currentPlayer === PLAYER_BLACK && !isThinking}
            onCellClick={handleCellClick}
          />
          <aside className="game-layout__panel">
            <GameInfo
              blackCount={blackCount}
              whiteCount={whiteCount}
              currentPlayer={currentPlayer}
              isThinking={isThinking}
              isGameOver={false}
            />
            <div className="game-controls">
              <button
                type="button"
                className="game-controls__button"
                onClick={handlePreviewResult}
                disabled={isThinking}
              >
                Preview result
              </button>
              <button
                type="button"
                className="game-controls__button game-controls__button--subtle"
                onClick={() => {
                  resetDemoGame()
                  setScreen('level_select')
                }}
              >
                Back to level select
              </button>
            </div>
          </aside>
        </main>
      )}

      <ResultModal
        isOpen={isResultOpen}
        winner={result.winner}
        blackCount={result.black_count}
        whiteCount={result.white_count}
        onRestart={handleRestart}
      />
    </div>
  )
}

export default App
