import type { Position } from '../wasm'
import Cell from './Cell'
import styles from '../styles/Board.module.css'

interface BoardProps {
  board: number[]
  legalMoves: Position[]
  flipped: number[]
  isPlayerTurn: boolean
  onCellClick: (row: number, col: number) => void
}

const BOARD_SIZE = 8
const BOARD_CELLS = 64

const toIndex = (row: number, col: number): number => row * BOARD_SIZE + col

function Board({
  board,
  legalMoves,
  flipped,
  isPlayerTurn,
  onCellClick,
}: BoardProps) {
  const safeBoard =
    board.length === BOARD_CELLS
      ? board
      : Array.from({ length: BOARD_CELLS }, (_, index) => board[index] ?? 0)
  const legalSet = new Set(legalMoves.map((move) => toIndex(move.row, move.col)))
  const flippedSet = new Set(flipped)

  return (
    <section className={styles['board-shell']}>
      <h2 className={styles['board-shell__title']}>Board</h2>
      <div className={styles.board} role="grid" aria-label="Reversi board">
        {Array.from({ length: BOARD_CELLS }, (_, index) => {
          const row = Math.floor(index / BOARD_SIZE)
          const col = index % BOARD_SIZE
          const isLegal = isPlayerTurn && legalSet.has(index)
          return (
            <Cell
              key={index}
              value={safeBoard[index]}
              row={row}
              col={col}
              isLegal={isLegal}
              isFlipped={flippedSet.has(index)}
              onClick={() => {
                if (isLegal) {
                  onCellClick(row, col)
                }
              }}
            />
          )
        })}
      </div>
    </section>
  )
}

export default Board
