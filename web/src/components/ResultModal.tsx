import { RESULT_DIALOG_NAME } from '../constants/accessibility'
import styles from '../styles/ResultModal.module.css'
import { PLAYER_BLACK, PLAYER_WHITE, type Winner } from '../types/player'

interface ResultModalProps {
  isOpen: boolean
  winner: Winner
  blackCount: number
  whiteCount: number
  onClose: () => void
  onRestart: () => void
}

const getResultMessage = (winner: Winner): string => {
  if (winner === PLAYER_BLACK) {
    return 'Black wins'
  }
  if (winner === PLAYER_WHITE) {
    return 'White wins'
  }
  return 'Draw game'
}

function ResultModal({
  isOpen,
  winner,
  blackCount,
  whiteCount,
  onClose,
  onRestart,
}: ResultModalProps) {
  if (!isOpen) {
    return null
  }

  return (
    <div className={styles.overlay}>
      <section
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-label={RESULT_DIALOG_NAME}
      >
        <p className={styles['modal__eyebrow']}>Final result</p>
        <h2 className={styles['modal__title']}>{getResultMessage(winner)}</h2>
        <p className={styles['modal__score']}>
          Black {blackCount} : White {whiteCount}
        </p>
        <div className={styles['modal__actions']}>
          <button type="button" className={styles['modal__restart']} onClick={onRestart}>
            Restart
          </button>
          <button type="button" className={styles['modal__close']} onClick={onClose}>
            Close
          </button>
        </div>
      </section>
    </div>
  )
}

export default ResultModal
