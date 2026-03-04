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
const OPENING_LEGAL_MOVES: Position[] = [
  { row: 2, col: 3 },
  { row: 3, col: 2 },
  { row: 4, col: 5 },
  { row: 5, col: 4 },
]
const FOLLOWUP_LEGAL_MOVES: Position[] = [
  { row: 2, col: 2 },
  { row: 2, col: 4 },
  { row: 4, col: 2 },
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

function App() {
  const [screen, setScreen] = useState<Screen>('level_select')
  const [selectedLevel, setSelectedLevel] = useState(3)
  const [board, setBoard] = useState<number[]>(() => createInitialBoard())
  const [legalMoves, setLegalMoves] = useState<Position[]>(OPENING_LEGAL_MOVES)
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
    setLegalMoves(OPENING_LEGAL_MOVES)
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

    const boardAfterPlayerMove = board.slice()
    boardAfterPlayerMove[selectedIndex] = PLAYER_BLACK

    const playerFlipMap: Record<number, number[]> = {
      19: [27],
      26: [27],
      37: [36],
      44: [36],
    }
    const playerFlipped = playerFlipMap[selectedIndex] ?? []
    for (const index of playerFlipped) {
      if (boardAfterPlayerMove[index] === PLAYER_WHITE) {
        boardAfterPlayerMove[index] = PLAYER_BLACK
      }
    }

    setBoard(boardAfterPlayerMove)
    setFlipped(playerFlipped)
    setLegalMoves([])
    setCurrentPlayer(PLAYER_WHITE)
    setIsThinking(true)

    if (aiTimerRef.current !== null) {
      window.clearTimeout(aiTimerRef.current)
    }

    aiTimerRef.current = window.setTimeout(() => {
      const boardAfterAiMove = boardAfterPlayerMove.slice()
      const aiMoveIndex = demoAiLogic.chooseAIMoveIndex(boardAfterAiMove, selectedLevel)
      if (aiMoveIndex >= 0) {
        boardAfterAiMove[aiMoveIndex] = PLAYER_WHITE
      }

      const aiFlipped =
        aiMoveIndex === 20 && boardAfterAiMove[28] === PLAYER_BLACK
          ? [28]
          : []
      for (const index of aiFlipped) {
        boardAfterAiMove[index] = PLAYER_WHITE
      }

      setBoard(boardAfterAiMove)
      setFlipped(aiFlipped)
      setCurrentPlayer(PLAYER_BLACK)
      setIsThinking(false)
      setLegalMoves(
        FOLLOWUP_LEGAL_MOVES.filter(
          ({ row: moveRow, col: moveCol }) =>
            boardAfterAiMove[moveRow * BOARD_WIDTH + moveCol] === 0,
        ),
      )
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
