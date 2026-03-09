import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import LevelSelect from './LevelSelect'
import { PLAYER_BLACK, PLAYER_WHITE } from '../types/player'

afterEach(() => {
  cleanup()
})

describe('LevelSelect', () => {
  it('renders level options and sends level change events', async () => {
    const user = userEvent.setup()
    const onLevelChange = vi.fn()
    const onPlayerChange = vi.fn()
    const onStart = vi.fn()

    render(
      <LevelSelect
        selectedLevel={3}
        selectedPlayer={PLAYER_BLACK}
        onLevelChange={onLevelChange}
        onPlayerChange={onPlayerChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /Play first/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Play second/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /Play second/i }))
    expect(onPlayerChange).toHaveBeenCalledWith(PLAYER_WHITE)

    await user.click(screen.getByRole('button', { name: /^Level 5$/ }))
    expect(onLevelChange).toHaveBeenCalledWith(5)

    await user.click(screen.getByRole('button', { name: 'Start level 3 as Black' }))
    expect(onStart).toHaveBeenCalledTimes(1)
  })

  it('disables level options and start button when disabled is true', () => {
    const onLevelChange = vi.fn()
    const onPlayerChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        selectedPlayer={PLAYER_BLACK}
        disabled
        onLevelChange={onLevelChange}
        onPlayerChange={onPlayerChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /Play first/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /Play second/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Start level 3 as Black' })).toBeDisabled()
  })

  it('disables only the start button when startDisabled is true', () => {
    const onLevelChange = vi.fn()
    const onPlayerChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        selectedPlayer={PLAYER_BLACK}
        startDisabled
        onLevelChange={onLevelChange}
        onPlayerChange={onPlayerChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /Play first/i })).toBeEnabled()
    expect(screen.getByRole('button', { name: /Play second/i })).toBeEnabled()
    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeEnabled()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Start level 3 as Black' })).toBeDisabled()
  })

  it('shows preparing text while loading', () => {
    const onLevelChange = vi.fn()
    const onPlayerChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        selectedPlayer={PLAYER_BLACK}
        isLoading
        onLevelChange={onLevelChange}
        onPlayerChange={onPlayerChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('button', { name: /Play first/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /Play second/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Preparing...' })).toBeDisabled()
  })

  it('renders alert message when error is provided', () => {
    const onLevelChange = vi.fn()
    const onPlayerChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        selectedPlayer={PLAYER_BLACK}
        error="Model loading failed"
        onLevelChange={onLevelChange}
        onPlayerChange={onPlayerChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('alert')).toHaveTextContent('Model loading failed')
  })

  it('keeps level buttons enabled when startDisabled and error are both set', () => {
    const onLevelChange = vi.fn()
    const onPlayerChange = vi.fn()
    const onStart = vi.fn()
    render(
      <LevelSelect
        selectedLevel={3}
        selectedPlayer={PLAYER_WHITE}
        startDisabled
        error="init_game failed"
        onLevelChange={onLevelChange}
        onPlayerChange={onPlayerChange}
        onStart={onStart}
      />,
    )

    expect(screen.getByRole('alert')).toHaveTextContent('init_game failed')
    expect(screen.getByRole('button', { name: /Play first/i })).toBeEnabled()
    expect(screen.getByRole('button', { name: /Play second/i })).toBeEnabled()
    expect(screen.getByRole('button', { name: /^Level 1$/ })).toBeEnabled()
    expect(screen.getByRole('button', { name: /^Level 6$/ })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Start level 3 as White' })).toBeDisabled()
  })
})
