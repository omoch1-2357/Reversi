import styles from '../styles/ResultModal.module.css'

interface ResultModalProps {
  isOpen: boolean
  winner: number
  blackCount: number
  whiteCount: number
  onRestart: () => void
}

const getResultMessage = (winner: number): string => {
  if (winner === 1) {
    return 'Black wins'
  }
  if (winner === 2) {
    return 'White wins'
  }
  return 'Draw game'
}

function ResultModal({
  isOpen,
  winner,
  blackCount,
  whiteCount,
  onRestart,
}: ResultModalProps) {
  if (!isOpen) {
    return null
  }

  return (
    <div className={styles.overlay}>
      <section className={styles.modal} role="dialog" aria-modal="true" aria-label="Game result">
        <p className={styles['modal__eyebrow']}>Final result</p>
        <h2 className={styles['modal__title']}>{getResultMessage(winner)}</h2>
        <p className={styles['modal__score']}>
          Black {blackCount} : White {whiteCount}
        </p>
        <button type="button" className={styles['modal__restart']} onClick={onRestart}>
          Restart
        </button>
      </section>
    </div>
  )
}

export default ResultModal
