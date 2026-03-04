import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import App from './App'
import * as DemoAiModule from './demoAi'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  vi.useRealTimers()
})

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

  it('runs deterministic AI timer flow based on selected level', async () => {
    vi.useFakeTimers()
    const delaySpy = vi.spyOn(DemoAiModule.demoAiLogic, 'getAIDelay')
    const chooseSpy = vi.spyOn(DemoAiModule.demoAiLogic, 'chooseAIMoveIndex')
    const snapshots: string[] = []

    for (let run = 0; run < 2; run += 1) {
      const { unmount } = render(<App />)

      fireEvent.click(screen.getByRole('button', { name: /^Level 4$/ }))
      fireEvent.click(screen.getByRole('button', { name: 'Start level 4' }))
      fireEvent.click(screen.getByRole('button', { name: 'Cell 3-4 legal move' }))

      expect(screen.getByRole('status')).toHaveTextContent('AI is thinking...')
      expect(delaySpy).toHaveBeenLastCalledWith(4)
      expect(delaySpy.mock.results.at(-1)?.value).toBe(460)

      act(() => {
        vi.advanceTimersByTime(460)
      })

      expect(chooseSpy).toHaveBeenCalledWith(expect.any(Array), 4)
      expect(screen.queryByRole('status')).not.toBeInTheDocument()
      expect(screen.getByText('Your turn (Black)')).toBeInTheDocument()

      const legalMoves = screen
        .getAllByRole('button', { name: /legal move/ })
        .map((button) => button.getAttribute('aria-label'))
        .sort()
      expect(legalMoves).toEqual(['Cell 3-3 legal move', 'Cell 5-3 legal move'])

      const boardSignature = screen
        .getAllByRole('button', { name: /^Cell / })
        .map((button) => {
          const className = String(button.className)
          if (className.includes('cell--black')) {
            return 'B'
          }
          if (className.includes('cell--white')) {
            return 'W'
          }
          return '_'
        })
        .join('')
      const flippedCount = document.querySelectorAll('button[class*="cell--flipped"]').length
      snapshots.push(`${boardSignature}|${legalMoves.join(',')}|flipped:${flippedCount}`)

      unmount()
      cleanup()
    }

    expect(snapshots[0]).toBe(snapshots[1])
  })
})
