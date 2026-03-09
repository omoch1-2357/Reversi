import styles from '../styles/LevelSelect.module.css'
import { playerLabel, type Player } from '../types/player'

interface LevelSelectProps {
  selectedLevel: number
  selectedPlayer: Player
  disabled?: boolean
  startDisabled?: boolean
  isLoading?: boolean
  error?: string | null
  onLevelChange: (level: number) => void
  onPlayerChange: (player: Player) => void
  onStart: () => void
}

const LEVELS = [1, 2, 3, 4, 5, 6] as const
const PLAYER_OPTIONS: Player[] = [1, 2]

function LevelSelect({
  selectedLevel,
  selectedPlayer,
  disabled = false,
  startDisabled = false,
  isLoading = false,
  error = null,
  onLevelChange,
  onPlayerChange,
  onStart,
}: LevelSelectProps) {
  const areOptionsDisabled = disabled || isLoading
  const isStartActionDisabled = areOptionsDisabled || startDisabled

  return (
    <section className={styles['level-select']} aria-label="Level selection">
      <h2 className={styles['level-select__title']}>Start game</h2>
      <p className={styles['level-select__description']}>
        Choose your color and difficulty before launching the match.
      </p>
      <div className={styles['level-select__section']}>
        <p className={styles['level-select__label']}>Turn order</p>
        <div className={styles['level-select__grid']} role="group" aria-label="Turn order">
          {PLAYER_OPTIONS.map((player) => {
            const selected = player === selectedPlayer
            return (
              <button
                key={player}
                type="button"
                aria-pressed={selected}
                className={[
                  styles['level-select__option'],
                  selected ? styles['level-select__option--selected'] : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
                onClick={() => onPlayerChange(player)}
                disabled={areOptionsDisabled}
              >
                {player === 1 ? 'Play first' : 'Play second'}
                <span className={styles['level-select__option-meta']}>
                  You: {playerLabel(player)}
                </span>
              </button>
            )
          })}
        </div>
      </div>
      <div className={styles['level-select__section']}>
        <p className={styles['level-select__label']}>Difficulty</p>
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
      </div>
      <button
        type="button"
        className={styles['level-select__start']}
        onClick={onStart}
        disabled={isStartActionDisabled}
      >
        {isLoading
          ? 'Preparing...'
          : `Start level ${selectedLevel} as ${playerLabel(selectedPlayer)}`}
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
