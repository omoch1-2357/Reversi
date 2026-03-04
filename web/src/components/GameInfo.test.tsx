import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import GameInfo from './GameInfo'

describe('GameInfo', () => {
  it('shows stone counts, turn label, and thinking indicator', () => {
    render(
      <GameInfo
        blackCount={18}
        whiteCount={12}
        currentPlayer={2}
        isThinking
        isGameOver={false}
      />,
    )

    expect(screen.getByText('Black')).toBeInTheDocument()
    expect(screen.getByText('White')).toBeInTheDocument()
    expect(screen.getByText('18')).toBeInTheDocument()
    expect(screen.getByText('12')).toBeInTheDocument()
    expect(screen.getByText('AI turn (White)')).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('AI is thinking...')
  })
})
