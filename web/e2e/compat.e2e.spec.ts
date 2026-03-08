import { expect, test } from '@playwright/test'
import { startLevel } from './helpers'

test('supported-browser flow reaches the final result dialog', async ({ page }) => {
  await startLevel(page, 1)

  await page.evaluate(async () => {
    const resultSelector = '[aria-label="Game result"]'
    const legalMoveSelector = 'button[aria-label$="legal move"]'
    const playerTurnText = 'Your turn (Black)'

    const waitForNextTurn = (): Promise<'game_over' | 'player_turn'> =>
      new Promise((resolve, reject) => {
        const deadline = performance.now() + 30_000

        const tick = (): void => {
          if (document.querySelector(resultSelector) !== null) {
            resolve('game_over')
            return
          }

          const legalMove = document.querySelector(legalMoveSelector)
          const bodyText = document.body.textContent ?? ''
          if (legalMove !== null && bodyText.includes(playerTurnText)) {
            resolve('player_turn')
            return
          }

          if (performance.now() > deadline) {
            reject(new Error('Timed out while waiting for the next playable turn'))
            return
          }

          requestAnimationFrame(tick)
        }

        tick()
      })

    for (let turn = 0; turn < 80; turn += 1) {
      if (document.querySelector(resultSelector) !== null) {
        return
      }

      const legalMove = document.querySelector(legalMoveSelector)
      if (!(legalMove instanceof HTMLButtonElement)) {
        throw new Error('No legal move button was available before game over')
      }

      legalMove.click()
      const status = await waitForNextTurn()
      if (status === 'game_over') {
        return
      }
    }

    throw new Error('Game did not finish within 80 player turns')
  })

  const resultDialog = page.getByRole('dialog', { name: 'Game result' })
  await expect(resultDialog).toBeVisible()
  await expect(resultDialog.getByText(/Black \d+ : White \d+/)).toBeVisible()

  await resultDialog.getByRole('button', { name: 'Close' }).click()
  await expect(resultDialog).toBeHidden()
  await expect(page.getByRole('button', { name: 'Show result popup' })).toBeVisible()
})
