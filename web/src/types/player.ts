export type Player = 1 | 2
export type Winner = 0 | Player

export const PLAYER_BLACK: Player = 1
export const PLAYER_WHITE: Player = 2

export const opponentOf = (player: Player): Player =>
  player === PLAYER_BLACK ? PLAYER_WHITE : PLAYER_BLACK

export const playerLabel = (player: Player): 'Black' | 'White' =>
  player === PLAYER_BLACK ? 'Black' : 'White'
