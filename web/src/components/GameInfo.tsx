import styles from '../styles/GameInfo.module.css'

interface GameInfoProps {
  blackCount: number
  whiteCount: number
  currentPlayer: number
  isThinking: boolean
  isGameOver: boolean
}

const getTurnLabel = (currentPlayer: number, isGameOver: boolean): string => {
  if (isGameOver) {
    return 'Game over'
  }
  return currentPlayer === 1 ? 'Your turn (Black)' : 'AI turn (White)'
}

function GameInfo({
  blackCount,
  whiteCount,
  currentPlayer,
  isThinking,
  isGameOver,
}: GameInfoProps) {
  return (
    <section className={styles['game-info']} aria-label="Game status">
      <h2 className={styles['game-info__title']}>Game Info</h2>
      <div className={styles['game-info__counts']}>
        <p>
          <span className={styles['game-info__label']}>Black</span>
          <strong>{blackCount}</strong>
        </p>
        <p>
          <span className={styles['game-info__label']}>White</span>
          <strong>{whiteCount}</strong>
        </p>
      </div>
      <p className={styles['game-info__turn']}>{getTurnLabel(currentPlayer, isGameOver)}</p>
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
