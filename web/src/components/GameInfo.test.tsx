import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import GameInfo from './GameInfo'

afterEach(() => {
  cleanup()
})

describe('GameInfo', () => {
  it('shows stone counts, turn label, and thinking indicator', () => {
    render(
      <GameInfo
        blackCount={18}
        whiteCount={12}
        currentPlayer={2}
        isThinking
        isPass={false}
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

  it('shows player turn label when current player is black', () => {
    render(
      <GameInfo
        blackCount={10}
        whiteCount={8}
        currentPlayer={1}
        isThinking={false}
        isPass={false}
        isGameOver={false}
      />,
    )

    expect(screen.getByText('Your turn (Black)')).toBeInTheDocument()
  })

  it('does not render thinking indicator when isThinking is false', () => {
    render(
      <GameInfo
        blackCount={22}
        whiteCount={20}
        currentPlayer={2}
        isThinking={false}
        isPass={false}
        isGameOver={false}
      />,
    )

    expect(screen.queryByRole('status')).not.toBeInTheDocument()
  })

  it('shows game over state when the game is finished', () => {
    render(
      <GameInfo
        blackCount={33}
        whiteCount={31}
        currentPlayer={1}
        isThinking={false}
        isPass={false}
        isGameOver
      />,
    )

    expect(screen.getByText('Game over')).toBeInTheDocument()
  })

  it('shows pass guidance when the AI has no legal moves', () => {
    render(
      <GameInfo
        blackCount={24}
        whiteCount={19}
        currentPlayer={1}
        isThinking={false}
        isPass
        isGameOver={false}
      />,
    )

    expect(screen.getByText('AI passed. Your turn continues.')).toBeInTheDocument()
  })
})
