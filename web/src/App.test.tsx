import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import App from './App'

vi.mock('/vite.svg', () => ({ default: '/vite.svg' }))
vi.mock('./assets/react.svg', () => ({ default: '/react.svg' }))

describe('App', () => {
  it('renders initial UI and increments counter on click', async () => {
    const user = userEvent.setup()

    render(<App />)

    expect(screen.getByRole('heading', { name: 'Vite + React' })).toBeInTheDocument()
    const counterButton = screen.getByRole('button', { name: 'count is 0' })
    expect(counterButton).toBeInTheDocument()

    await user.click(counterButton)

    expect(screen.getByRole('button', { name: 'count is 1' })).toBeInTheDocument()
  })
})
