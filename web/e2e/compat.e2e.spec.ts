import { expect, test } from '@playwright/test'
import { startLevel } from './helpers'

test('supported-browser flow reaches the final result dialog', async ({ page }) => {
  await startLevel(page, 1)

  // The compatibility matrix runs slower than the Chromium-only suite, so this
  // spec intentionally bypasses `advanceOnePlayerTurn` / `playUntilGameOver`
  // and drives the DOM inside `page.evaluate` to avoid Playwright round-trips.
  await page.evaluate(async () => {
    const resultSelector = '[aria-label="Game result"]'
    const legalMoveSelector = 'button[aria-label$="legal move"]'
    const playerTurnText = 'Your turn (Black)'

    const waitForPlayableTurn = (): Promise<'game_over' | Element> =>
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
            resolve(legalMove)
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
      const nextAction = await waitForPlayableTurn()
      if (nextAction === 'game_over') {
        return
      }

      nextAction.click()
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
