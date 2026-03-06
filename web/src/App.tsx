import { useEffect, useState } from 'react'
import './App.css'
import Board from './components/Board'
import GameInfo from './components/GameInfo'
import LevelSelect from './components/LevelSelect'
import ResultModal from './components/ResultModal'
import useGame from './hooks/useGame'
import { PLAYER_BLACK, PLAYER_WHITE, type Player, type Winner } from './types/player'
import workerUrl from './workers/wasm.worker.ts?worker&url'

declare global {
  interface Window {
    __reversiWorkerUrl?: string
  }
}

if (typeof window !== 'undefined') {
  window.__reversiWorkerUrl = workerUrl
}

type Screen = 'level_select' | 'game'

const DEFAULT_LEVEL = 3
const EMPTY_BOARD = Array.from({ length: 64 }, () => 0)
const MODEL_LOAD_ERROR =
  'モデルデータの読み込みに失敗しました。再試行してください。'
const MODEL_LOAD_GUIDANCE =
  '再試行しても改善しない場合は、ページを再読み込みして再度開始してください。'

const toPlayer = (value: number): Player =>
  value === PLAYER_WHITE ? PLAYER_WHITE : PLAYER_BLACK

const toWinner = (value: number): Winner =>
  value === PLAYER_BLACK || value === PLAYER_WHITE ? value : 0

function App() {
  const [screen, setScreen] = useState<Screen>('level_select')
  const [selectedLevel, setSelectedLevel] = useState(DEFAULT_LEVEL)
  const [initError, setInitError] = useState<string | null>(null)
  const [isResultModalOpen, setIsResultModalOpen] = useState(false)
  const {
    gameState,
    legalMoves,
    isThinking,
    error,
    result,
    startGame,
    placeStone,
    restart,
  } = useGame()

  useEffect(() => {
    setIsResultModalOpen(result !== null)
  }, [result])

  const handleStartAttempt = async (): Promise<void> => {
    setInitError(null)
    setIsResultModalOpen(false)

    try {
      await startGame(selectedLevel)
      setScreen('game')
    } catch {
      setInitError(MODEL_LOAD_ERROR)
      setScreen('level_select')
    }
  }

  const handleRestart = async (): Promise<void> => {
    setInitError(null)
    setIsResultModalOpen(false)

    try {
      await restart()
      setScreen('game')
    } catch {
      setInitError(MODEL_LOAD_ERROR)
      setScreen('level_select')
    }
  }

  const handleCellClick = (row: number, col: number): void => {
    if (gameState === null || gameState.current_player !== PLAYER_BLACK || isThinking) {
      return
    }

    // handleCellClick intentionally consumes placeStone rejections because useGame
    // exposes the error state to the UI; this catch only prevents unhandled
    // promise rejection warnings from bubbling out of the event handler.
    void placeStone(row, col).catch(() => undefined)
  }

  const handleReturnToTitle = (): void => {
    if (isThinking) {
      return
    }

    setScreen('level_select')
    setInitError(null)
    setIsResultModalOpen(false)
  }

  const board = gameState?.board ?? EMPTY_BOARD
  const flipped = gameState?.flipped ?? []
  const blackCount = gameState?.black_count ?? 0
  const whiteCount = gameState?.white_count ?? 0
  const currentPlayer = toPlayer(gameState?.current_player ?? PLAYER_BLACK)
  const isGameOver = gameState?.is_game_over ?? false
  const gameError = screen === 'game' ? error : null
  const isResultOpen = screen === 'game' && result !== null && isResultModalOpen

  return (
    <div className="app">
      <header className="app__header">
        <p className="app__eyebrow">Reversi</p>
        <h1>Reversi</h1>
        <p className="app__lead">
          Play a worker-backed Reversi match against the embedded AI.
        </p>
      </header>

      {screen === 'level_select' ? (
        <>
          <LevelSelect
            selectedLevel={selectedLevel}
            startDisabled={initError !== null}
            isLoading={isThinking}
            error={initError}
            onLevelChange={setSelectedLevel}
            onStart={() => {
              void handleStartAttempt()
            }}
          />
          {initError ? (
            <section className="app__retry-panel" aria-label="Initialization retry guidance">
              <p className="app__retry-copy">{MODEL_LOAD_GUIDANCE}</p>
              <button
                type="button"
                className="game-controls__button"
                onClick={() => {
                  void handleStartAttempt()
                }}
              >
                Retry initialization
              </button>
            </section>
          ) : null}
        </>
      ) : (
        <main className="game-layout">
          <Board
            board={board}
            legalMoves={legalMoves}
            flipped={flipped}
            isPlayerTurn={currentPlayer === PLAYER_BLACK && !isThinking && !isGameOver}
            onCellClick={handleCellClick}
          />
          <aside className="game-layout__panel">
            <GameInfo
              blackCount={blackCount}
              whiteCount={whiteCount}
              currentPlayer={currentPlayer}
              isThinking={isThinking}
              isPass={gameState?.is_pass ?? false}
              isGameOver={isGameOver}
            />
            {gameError ? (
              <p className="app__error" role="alert">
                {gameError}
              </p>
            ) : null}
            {result ? (
              <section className="app__result-summary" aria-label="Result summary">
                <p className="app__result-label">Latest result</p>
                <strong className="app__result-title">
                  {toWinner(result.winner) === PLAYER_BLACK
                    ? 'Black wins'
                    : toWinner(result.winner) === PLAYER_WHITE
                      ? 'White wins'
                      : 'Draw game'}
                </strong>
                <p className="app__result-copy">
                  Black {result.black_count} : White {result.white_count}
                </p>
                <div className="app__result-actions">
                  <button
                    type="button"
                    className="game-controls__button game-controls__button--subtle"
                    onClick={() => {
                      setIsResultModalOpen(true)
                    }}
                  >
                    Show result popup
                  </button>
                </div>
              </section>
            ) : null}
            <div className="game-controls">
              <button
                type="button"
                className="game-controls__button"
                onClick={() => {
                  void handleRestart()
                }}
                disabled={isThinking}
              >
                Restart level {selectedLevel}
              </button>
              <button
                type="button"
                className="game-controls__button game-controls__button--subtle"
                onClick={handleReturnToTitle}
                disabled={isThinking}
              >
                Back to level select
              </button>
            </div>
          </aside>
        </main>
      )}

      <ResultModal
        isOpen={isResultOpen}
        winner={toWinner(result?.winner ?? 0)}
        blackCount={result?.black_count ?? 0}
        whiteCount={result?.white_count ?? 0}
        onClose={() => {
          setIsResultModalOpen(false)
        }}
        onRestart={() => {
          void handleRestart()
        }}
      />
    </div>
  )
}

export default App
