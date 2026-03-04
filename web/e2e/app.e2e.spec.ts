import { expect, test } from '@playwright/test'

test('app flow: level select -> board -> result modal -> restart', async ({ page }) => {
  await page.goto('/Reversi/')

  await expect(page.getByRole('heading', { name: 'Select difficulty' })).toBeVisible()

  await page.getByRole('button', { name: 'Level 4' }).click()
  await page.getByRole('button', { name: 'Start level 4' }).click()

  await expect(page.getByRole('grid', { name: 'Reversi board' })).toBeVisible()
  await expect(page.getByText('Your turn (Black)')).toBeVisible()

  await page.getByRole('button', { name: 'Preview result' }).click()
  await expect(page.getByRole('dialog', { name: 'Game result' })).toBeVisible()

  await page.getByRole('button', { name: 'Restart' }).click()
  await expect(page.getByRole('dialog', { name: 'Game result' })).toHaveCount(0)
})

test('worker e2e flow is deterministic with real wasm', async ({ page }) => {
  await page.goto('/Reversi/')

  const summary = await page.evaluate(async () => {
    type Position = { row: number; col: number }
    type GameState = {
      current_player: number
      black_count: number
      white_count: number
      is_game_over: boolean
      flipped: number[]
    }
    type GameResult = {
      winner: number
      black_count: number
      white_count: number
    }
    type WorkerRequest =
      | { type: 'init_game'; payload: { level: number } }
      | { type: 'place_stone'; payload: { row: number; col: number } }
      | { type: 'get_result' }
    type WorkerResponse =
      | { type: 'game_state'; payload: { state: GameState; moves: Position[] } }
      | { type: 'ai_step'; payload: { state: GameState } }
      | { type: 'game_over'; payload: { state: GameState; result: GameResult } }
      | { type: 'result'; payload: GameResult }
      | { type: 'error'; payload: string }

    const workerUrl = window.__reversiWorkerUrl
    if (!workerUrl) {
      throw new Error('worker URL is not available on window.__reversiWorkerUrl')
    }

    const worker = new Worker(workerUrl, { type: 'module' })

    const sendAndCollect = (request: WorkerRequest): Promise<WorkerResponse[]> =>
      new Promise((resolve, reject) => {
        const collected: WorkerResponse[] = []
        const onError = (event: ErrorEvent): void => {
          cleanup()
          reject(
            new Error(
              `worker runtime error: ${event.message} (${event.filename}:${event.lineno}:${event.colno})`,
            ),
          )
        }
        const onMessageError = (): void => {
          cleanup()
          reject(new Error('worker messageerror event'))
        }

        const cleanup = (): void => {
          window.clearTimeout(timeoutId)
          worker.removeEventListener('message', onMessage)
          worker.removeEventListener('error', onError)
          worker.removeEventListener('messageerror', onMessageError)
        }

        const timeoutId = window.setTimeout(() => {
          cleanup()
          reject(new Error(`worker response timeout: ${request.type}`))
        }, 15_000)

        const onMessage = (event: MessageEvent<WorkerResponse>): void => {
          const message = event.data
          collected.push(message)
          if (
            message.type === 'game_state'
            || message.type === 'game_over'
            || message.type === 'result'
            || message.type === 'error'
          ) {
            cleanup()
            resolve(collected)
          }
        }

        worker.addEventListener('message', onMessage)
        worker.addEventListener('error', onError)
        worker.addEventListener('messageerror', onMessageError)
        worker.postMessage(request)
      })

    try {
      const initCycle = await sendAndCollect({ type: 'init_game', payload: { level: 1 } })
      let terminal = initCycle[initCycle.length - 1]
      if (!terminal || terminal.type !== 'game_state') {
        throw new Error('init_game did not return game_state')
      }

      let aiStepCount = 0
      let playerTurns = 0
      while (terminal.type === 'game_state') {
        const move = terminal.payload.moves[0]
        if (!move) {
          throw new Error('no player move available before game over')
        }

        playerTurns += 1
        const cycle = await sendAndCollect({
          type: 'place_stone',
          payload: { row: move.row, col: move.col },
        })
        aiStepCount += cycle.filter((entry) => entry.type === 'ai_step').length
        const last = cycle[cycle.length - 1]
        if (!last) {
          throw new Error('place_stone produced no response')
        }
        if (last.type === 'error') {
          throw new Error(last.payload)
        }
        if (last.type === 'game_over') {
          const resultCycle = await sendAndCollect({ type: 'get_result' })
          const resultMessage = resultCycle[resultCycle.length - 1]
          if (!resultMessage || resultMessage.type !== 'result') {
            throw new Error('get_result did not return result')
          }

          return {
            aiStepCount,
            playerTurns,
            finalResult: resultMessage.payload,
          }
        }
        if (last.type !== 'game_state') {
          throw new Error(`unexpected terminal response: ${last.type}`)
        }

        terminal = last
        if (playerTurns > 80) {
          throw new Error('turn safety cap exceeded')
        }
      }

      throw new Error('game loop ended without terminal state')
    } finally {
      worker.terminate()
    }
  })

  expect(summary.aiStepCount).toBeGreaterThan(0)
  expect(summary.finalResult).toEqual({
    winner: 2,
    black_count: 19,
    white_count: 45,
  })
})
