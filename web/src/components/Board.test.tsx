import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import Board from './Board'

describe('Board', () => {
  it('highlights legal moves and invokes click callback with coordinates', async () => {
    const user = userEvent.setup()
    const onCellClick = vi.fn()
    const board = Array.from({ length: 64 }, () => 0)

    render(
      <Board
        board={board}
        legalMoves={[{ row: 2, col: 3 }]}
        flipped={[27]}
        isPlayerTurn
        onCellClick={onCellClick}
      />,
    )

    const legalCell = screen.getByRole('button', { name: /Cell 3-4 legal move/ })
    await user.click(legalCell)
    expect(onCellClick).toHaveBeenCalledWith(2, 3)

    const nonLegalCell = screen.getByRole('button', { name: 'Cell 1-1' })
    expect(nonLegalCell).toBeDisabled()
  })

  it('disables all moves when it is not the player turn', async () => {
    const user = userEvent.setup()
    const onCellClick = vi.fn()
    const board = Array.from({ length: 64 }, () => 0)

    render(
      <Board
        board={board}
        legalMoves={[{ row: 2, col: 3 }]}
        flipped={[]}
        isPlayerTurn={false}
        onCellClick={onCellClick}
      />,
    )

    const cell = screen.getByRole('button', { name: 'Cell 3-4' })
    expect(cell).toBeDisabled()
    await user.click(cell)
    expect(onCellClick).not.toHaveBeenCalled()
  })
})
