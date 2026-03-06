import styles from '../styles/LevelSelect.module.css'

interface LevelSelectProps {
  selectedLevel: number
  disabled?: boolean
  startDisabled?: boolean
  isLoading?: boolean
  error?: string | null
  onLevelChange: (level: number) => void
  onStart: () => void
}

const LEVELS = [1, 2, 3, 4, 5, 6] as const

function LevelSelect({
  selectedLevel,
  disabled = false,
  startDisabled = false,
  isLoading = false,
  error = null,
  onLevelChange,
  onStart,
}: LevelSelectProps) {
  const areOptionsDisabled = disabled || isLoading
  const isStartActionDisabled = areOptionsDisabled || startDisabled

  return (
    <section className={styles['level-select']} aria-label="Level selection">
      <h2 className={styles['level-select__title']}>Select difficulty</h2>
      <p className={styles['level-select__description']}>
        Choose a level from 1 to 6, then launch the game.
      </p>
      <div className={styles['level-select__grid']} role="group" aria-label="Levels">
        {LEVELS.map((level) => {
          const selected = level === selectedLevel
          return (
            <button
              key={level}
              type="button"
              aria-pressed={selected}
              className={[
                styles['level-select__option'],
                selected ? styles['level-select__option--selected'] : '',
              ]
                .filter(Boolean)
                .join(' ')}
              onClick={() => onLevelChange(level)}
              disabled={areOptionsDisabled}
            >
              Level {level}
            </button>
          )
        })}
      </div>
      <button
        type="button"
        className={styles['level-select__start']}
        onClick={onStart}
        disabled={isStartActionDisabled}
      >
        {isLoading ? 'Preparing...' : `Start level ${selectedLevel}`}
      </button>
      {error ? (
        <p role="alert" className={styles['level-select__error']}>
          {error}
        </p>
      ) : null}
    </section>
  )
}

export default LevelSelect
