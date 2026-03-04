const AI_MOVE_PREFERENCES_BY_LEVEL: Record<number, number[]> = {
  1: [20],
  2: [20, 19, 26, 37, 44],
  3: [20, 19, 26, 37, 44, 18, 21, 34],
  4: [20, 26, 37, 19, 44, 34, 21],
  5: [0, 7, 56, 63, 20, 19, 26, 37, 44],
  6: [0, 7, 56, 63, 20, 26, 37, 19, 44, 34, 21],
}

export const getAIDelay = (level: number): number => 180 + level * 70

export const chooseAIMoveIndex = (board: number[], level: number): number => {
  const preferences = AI_MOVE_PREFERENCES_BY_LEVEL[level] ?? AI_MOVE_PREFERENCES_BY_LEVEL[3]
  for (const index of preferences) {
    if (board[index] === 0) {
      return index
    }
  }
  return board.findIndex((cell) => cell === 0)
}

export const demoAiLogic = {
  getAIDelay,
  chooseAIMoveIndex,
}
