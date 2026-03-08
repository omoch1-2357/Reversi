export const RESULT_DIALOG_NAME = 'Game result'
export const LEGAL_MOVE_ARIA_SUFFIX = 'legal move'
export const PLAYER_TURN_TEXT = 'Your turn (Black)'

export const getCellAriaLabel = (row: number, col: number, isLegal: boolean): string =>
  `Cell ${row + 1}-${col + 1}${isLegal ? ` ${LEGAL_MOVE_ARIA_SUFFIX}` : ''}`
