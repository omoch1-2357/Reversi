import { PLAYER_BLACK, PLAYER_WHITE, type Player } from './types/player'

const AI_MOVE_PREFERENCES_BY_LEVEL: Record<number, number[]> = {
  1: [20],
  2: [20, 19, 26, 37, 44],
  3: [20, 19, 26, 37, 44, 18, 21, 34],
  4: [20, 26, 37, 19, 44, 34, 21],
  5: [0, 7, 56, 63, 20, 19, 26, 37, 44],
  6: [0, 7, 56, 63, 20, 26, 37, 19, 44, 34, 21],
}

export const getAIDelay = (level: number): number => 180 + level * 70

const BOARD_SIZE = 8
const BOARD_CELLS = BOARD_SIZE * BOARD_SIZE
const DIRECTIONS: Array<[number, number]> = [
  [-1, -1], [-1, 0], [-1, 1],
  [0, -1], [0, 1],
  [1, -1], [1, 0], [1, 1],
]

const isInBounds = (row: number, col: number): boolean =>
  row >= 0 && row < BOARD_SIZE && col >= 0 && col < BOARD_SIZE

export const getLegalMoveIndices = (board: number[], player: Player): number[] => {
  if (board.length !== BOARD_CELLS) {
    return []
  }

  const opponent = player === PLAYER_BLACK ? PLAYER_WHITE : PLAYER_BLACK
  const legalMoves: number[] = []

  for (let row = 0; row < BOARD_SIZE; row += 1) {
    for (let col = 0; col < BOARD_SIZE; col += 1) {
      const index = row * BOARD_SIZE + col
      if (board[index] !== 0) {
        continue
      }

      let isLegal = false
      for (const [rowDelta, colDelta] of DIRECTIONS) {
        let r = row + rowDelta
        let c = col + colDelta
        let seenOpponent = false

        while (isInBounds(r, c)) {
          const cell = board[r * BOARD_SIZE + c]
          if (cell === opponent) {
            seenOpponent = true
            r += rowDelta
            c += colDelta
            continue
          }
          if (cell === player && seenOpponent) {
            isLegal = true
          }
          break
        }

        if (isLegal) {
          break
        }
      }

      if (isLegal) {
        legalMoves.push(index)
      }
    }
  }

  return legalMoves
}

export const chooseAIMoveIndex = (
  board: number[],
  level: number,
  legalMoves: number[],
): number => {
  const validLegalMoves = legalMoves.filter(
    (index) => Number.isInteger(index) && index >= 0 && index < board.length && board[index] === 0,
  )
  if (validLegalMoves.length === 0) {
    return -1
  }

  const preferences = AI_MOVE_PREFERENCES_BY_LEVEL[level] ?? AI_MOVE_PREFERENCES_BY_LEVEL[3]
  const legalMoveSet = new Set(validLegalMoves)
  for (const index of preferences) {
    if (legalMoveSet.has(index)) {
      return index
    }
  }
  return validLegalMoves[0]
}

export const demoAiLogic = {
  getLegalMoveIndices,
  getAIDelay,
  chooseAIMoveIndex,
}
