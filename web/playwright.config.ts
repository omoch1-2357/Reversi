import { defineConfig } from '@playwright/test'

const desktopViewport = { width: 1280, height: 720 }
const mobileViewport = { width: 375, height: 667 }
const desktopChromium = {
  browserName: 'chromium' as const,
  viewport: desktopViewport,
}

export default defineConfig({
  testDir: './e2e',
  expect: {
    timeout: 15_000,
  },
  timeout: 120_000,
  use: {
    baseURL: 'http://127.0.0.1:4173',
  },
  webServer: {
    command:
      'npm run build && npm run preview -- --host 127.0.0.1 --port 4173 --strictPort',
    port: 4173,
    reuseExistingServer: !process.env.CI,
    timeout: 240_000,
  },
  projects: [
    // Keep separate project names so CI scripts can target the functional
    // desktop suite and the broader compatibility suite independently.
    {
      name: 'desktop-chromium',
      use: desktopChromium,
    },
    {
      name: 'mobile-chromium',
      use: {
        browserName: 'chromium',
        viewport: mobileViewport,
        isMobile: true,
        hasTouch: true,
      },
    },
    {
      name: 'compat-chromium',
      use: desktopChromium,
    },
    {
      name: 'compat-firefox',
      use: {
        browserName: 'firefox',
        viewport: desktopViewport,
      },
    },
    {
      name: 'compat-webkit',
      use: {
        browserName: 'webkit',
        viewport: desktopViewport,
      },
    },
  ],
})
