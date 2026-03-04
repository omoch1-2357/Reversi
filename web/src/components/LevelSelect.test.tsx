import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import LevelSelect from './LevelSelect'

describe('LevelSelect', () => {
  it('renders level options and sends level change events', async () => {
    const user = userEvent.setup()
    const onLevelChange = vi.fn()
    const onStart = vi.fn()

    render(
      <LevelSelect
        selectedLevel={3}
        onLevelChange={onLevelChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /^Level 5$/ }))
    expect(onLevelChange).toHaveBeenCalledWith(5)

    await user.click(screen.getByRole('button', { name: 'Start level 3' }))
    expect(onStart).toHaveBeenCalledTimes(1)
  })
})
