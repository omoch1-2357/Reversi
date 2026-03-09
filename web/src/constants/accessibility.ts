import { playerLabel, type Player } from '../types/player'

export const RESULT_DIALOG_NAME = 'Game result'
export const LEGAL_MOVE_ARIA_SUFFIX = 'legal move'
export const getPlayerTurnText = (player: Player): string => `Your turn (${playerLabel(player)})`

export const getCellAriaLabel = (row: number, col: number, isLegal: boolean): string =>
  `Cell ${row + 1}-${col + 1}${isLegal ? ` ${LEGAL_MOVE_ARIA_SUFFIX}` : ''}`
