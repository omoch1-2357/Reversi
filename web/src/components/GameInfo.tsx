import styles from '../styles/GameInfo.module.css'
import { PLAYER_TURN_TEXT } from '../constants/accessibility'
import { PLAYER_BLACK, PLAYER_WHITE, type Player } from '../types/player'

interface GameInfoProps {
  blackCount: number
  whiteCount: number
  currentPlayer: Player
  isThinking: boolean
  isPass: boolean
  isGameOver: boolean
}

const getTurnLabel = (currentPlayer: Player, isGameOver: boolean): string => {
  if (isGameOver) {
    return 'Game over'
  }
  return currentPlayer === PLAYER_BLACK
    ? PLAYER_TURN_TEXT
    : currentPlayer === PLAYER_WHITE
      ? 'AI turn (White)'
      : 'Unknown turn'
}

function GameInfo({
  blackCount,
  whiteCount,
  currentPlayer,
  isThinking,
  isPass,
  isGameOver,
}: GameInfoProps) {
  const passMessage =
    !isPass || isGameOver
      ? null
      : currentPlayer === PLAYER_BLACK
        ? 'AI passed. Your turn continues.'
        : 'You have no legal moves. AI continues.'

  return (
    <section className={styles['game-info']} aria-label="Game status">
      <div className={styles['game-info__header']}>
        <p className={styles['game-info__eyebrow']}>Match status</p>
        <h2 className={styles['game-info__title']}>Game Info</h2>
      </div>
      <div className={styles['game-info__counts']}>
        <p className={styles['game-info__count-card']}>
          <span className={styles['game-info__label']}>Black</span>
          <strong>{blackCount}</strong>
        </p>
        <p className={styles['game-info__count-card']}>
          <span className={styles['game-info__label']}>White</span>
          <strong>{whiteCount}</strong>
        </p>
      </div>
      <p className={styles['game-info__turn']}>{getTurnLabel(currentPlayer, isGameOver)}</p>
      {passMessage ? <p className={styles['game-info__thinking']}>{passMessage}</p> : null}
      {isThinking ? (
        <p className={styles['game-info__thinking']} role="status">
          <span className={styles['game-info__spinner']} aria-hidden="true" />
          AI is thinking...
        </p>
      ) : null}
    </section>
  )
}

export default GameInfo
