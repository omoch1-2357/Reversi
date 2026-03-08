import { expect, test } from '@playwright/test'
import {
  LEGAL_MOVE_ARIA_SUFFIX,
  PLAYER_TURN_TEXT,
  RESULT_DIALOG_NAME,
} from '../src/constants/accessibility'
import { startLevel } from './helpers'

test('supported-browser flow reaches the final result dialog', async ({ page }) => {
  await startLevel(page, 1)

  // The compatibility matrix runs slower than the Chromium-only suite, so this
  // spec intentionally bypasses `advanceOnePlayerTurn` / `playUntilGameOver`
  // and drives the DOM inside `page.evaluate` to avoid Playwright round-trips.
  await page.evaluate(
    async ({ legalMoveAriaSuffix, playerTurnText, resultDialogName }) => {
      const resultSelector = `[aria-label="${resultDialogName}"]`
      const legalMoveSelector = `button[aria-label$="${legalMoveAriaSuffix}"]`

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
    },
    {
      legalMoveAriaSuffix: LEGAL_MOVE_ARIA_SUFFIX,
      playerTurnText: PLAYER_TURN_TEXT,
      resultDialogName: RESULT_DIALOG_NAME,
    },
  )

  const resultDialog = page.getByRole('dialog', { name: RESULT_DIALOG_NAME })
  await expect(resultDialog).toBeVisible()
  await expect(resultDialog.getByText(/Black \d+ : White \d+/)).toBeVisible()

  await resultDialog.getByRole('button', { name: 'Close' }).click()
  await expect(resultDialog).toBeHidden()
  await expect(page.getByRole('button', { name: 'Show result popup' })).toBeVisible()
})
