import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import ResultModal from './ResultModal'

describe('ResultModal', () => {
  it('does not render when closed', () => {
    const onRestart = vi.fn()
    const onClose = vi.fn()
    render(
      <ResultModal
        isOpen={false}
        winner={1}
        blackCount={32}
        whiteCount={32}
        onClose={onClose}
        onRestart={onRestart}
      />,
    )

    expect(screen.queryByRole('dialog', { name: 'Game result' })).not.toBeInTheDocument()
  })

  it('renders result details and restart action when open', async () => {
    const user = userEvent.setup()
    const onRestart = vi.fn()
    const onClose = vi.fn()
    render(
      <ResultModal
        isOpen
        winner={2}
        blackCount={20}
        whiteCount={44}
        onClose={onClose}
        onRestart={onRestart}
      />,
    )

    expect(screen.getByRole('dialog', { name: 'Game result' })).toBeInTheDocument()
    expect(screen.getByText('White wins')).toBeInTheDocument()
    expect(screen.getByText('Black 20 : White 44')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Restart' }))
    expect(onRestart).toHaveBeenCalledTimes(1)

    await user.click(screen.getByRole('button', { name: 'Close' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})
