import { expect, type Page } from '@playwright/test'
import {
  LEGAL_MOVE_ARIA_SUFFIX,
  PLAYER_TURN_TEXT,
  RESULT_DIALOG_NAME,
} from '../src/constants/accessibility'

const APP_PATH = '/Reversi/'
const LEGAL_MOVE_NAME = new RegExp(`Cell \\d-\\d ${LEGAL_MOVE_ARIA_SUFFIX}`)

export const startLevel = async (page: Page, level: number): Promise<void> => {
  await page.goto(APP_PATH)
  await expect(page.getByRole('heading', { name: 'Select difficulty' })).toBeVisible()
  await page.getByRole('button', { name: `Level ${level}` }).click()
  await page.getByRole('button', { name: `Start level ${level}` }).click()
  await expect(page.getByRole('grid', { name: 'Reversi board' })).toBeVisible()
  await expect(page.getByText(PLAYER_TURN_TEXT)).toBeVisible()
}

export const advanceOnePlayerTurn = async (
  page: Page,
): Promise<'game_over' | 'player_turn'> => {
  const legalMove = page.getByRole('button', { name: LEGAL_MOVE_NAME }).first()
  await expect(legalMove).toBeVisible({ timeout: 30_000 })
  await legalMove.click()

  if (await isGameOverVisible(page, 30_000)) {
    return 'game_over'
  }

  await expect(page.getByText(PLAYER_TURN_TEXT)).toBeVisible({ timeout: 30_000 })
  return 'player_turn'
}

export const playUntilGameOver = async (page: Page): Promise<void> => {
  for (let turn = 0; turn < 80; turn += 1) {
    if (await isGameOverVisible(page, 500)) {
      return
    }

    const status = await advanceOnePlayerTurn(page)
    if (status === 'game_over') {
      return
    }
  }

  throw new Error('game did not finish within 80 player turns')
}

const isGameOverVisible = async (page: Page, timeout: number): Promise<boolean> => {
  try {
    await page
      .getByRole('dialog', { name: RESULT_DIALOG_NAME })
      .waitFor({ state: 'visible', timeout })
    return true
  } catch {
    return false
  }
}
