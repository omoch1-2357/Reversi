import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import App from './App'

describe('App', () => {
  it('starts from level select and transitions to game preview', async () => {
    const user = userEvent.setup()

    render(<App />)

    expect(
      screen.getByRole('heading', { name: 'Select difficulty' }),
    ).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /^Level 4$/ }))
    await user.click(screen.getByRole('button', { name: 'Start level 4' }))

    expect(screen.getByRole('grid', { name: 'Reversi board' })).toBeInTheDocument()
    expect(screen.getByText('Your turn (Black)')).toBeInTheDocument()
  })

  it('opens and closes the result modal via restart', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('button', { name: 'Start level 3' }))
    const [previewButton] = screen.getAllByRole('button', { name: 'Preview result' })
    await user.click(previewButton)

    expect(screen.getByRole('dialog', { name: 'Game result' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Restart' }))

    expect(screen.queryByRole('dialog', { name: 'Game result' })).not.toBeInTheDocument()
  })
})
