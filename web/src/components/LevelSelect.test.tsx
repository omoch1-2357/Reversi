import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import LevelSelect from './LevelSelect'

afterEach(() => {
  cleanup()
})

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

  it('disables level options and start button when disabled is true', () => {
    const onLevelChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        disabled
        onLevelChange={onLevelChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Start level 3' })).toBeDisabled()
  })

  it('disables only the start button when startDisabled is true', () => {
    const onLevelChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        startDisabled
        onLevelChange={onLevelChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeEnabled()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Start level 3' })).toBeDisabled()
  })

  it('shows preparing text while loading', () => {
    const onLevelChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        isLoading
        onLevelChange={onLevelChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Preparing...' })).toBeDisabled()
  })

  it('renders alert message when error is provided', () => {
    const onLevelChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        error="Model loading failed"
        onLevelChange={onLevelChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('alert')).toHaveTextContent('Model loading failed')
  })
})
