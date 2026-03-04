import styles from '../styles/Cell.module.css'

export interface CellProps {
  value: number
  row: number
  col: number
  isLegal: boolean
  isFlipped: boolean
  onClick: () => void
}

function Cell({ value, row, col, isLegal, isFlipped, onClick }: CellProps) {
  const stateClass =
    value === 1 ? styles['cell--black'] : value === 2 ? styles['cell--white'] : ''
  const className = [
    styles.cell,
    stateClass,
    isLegal ? styles['cell--legal'] : '',
    isFlipped ? styles['cell--flipped'] : '',
  ]
    .filter(Boolean)
    .join(' ')

  const label = `Cell ${row + 1}-${col + 1}${isLegal ? ' legal move' : ''}`

  return (
    <button
      type="button"
      className={className}
      onClick={onClick}
      disabled={!isLegal}
      aria-label={label}
    >
      {value !== 0 ? <span className={styles.stone} aria-hidden="true" /> : null}
    </button>
  )
}

export default Cell
