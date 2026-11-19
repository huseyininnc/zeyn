import { defineConfig, devices } from '@playwright/test';

/**
 * Browser tests of the component catalog. The catalog app starts on `ZEYN_E2E_PORT` (3110 by
 * default) unless one already runs there. Every spec runs in four projects: the light theme with
 * the full checks, the dark theme, right to left, and a touch phone. Budgets are measured in the
 * light project only.
 */
const port = process.env.ZEYN_E2E_PORT ?? '3110';
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 4 : undefined,
  timeout: 60_000,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : [['list']],
  snapshotPathTemplate: '{testDir}/__snapshots__/{arg}{ext}',
  use: { baseURL, trace: 'retain-on-failure' },
  projects: [
    { name: 'light', use: { ...devices['Desktop Chrome'], colorScheme: 'light' } },
    {
      name: 'dark',
      testIgnore: '**/budgets.spec.ts',
      use: { ...devices['Desktop Chrome'], colorScheme: 'dark' },
    },
    { name: 'rtl', testIgnore: '**/budgets.spec.ts', use: { ...devices['Desktop Chrome'] } },
    { name: 'touch', testIgnore: '**/budgets.spec.ts', use: { ...devices['Pixel 7'] } },
  ],
  webServer: {
    command: 'cargo run -p zeyn-example-components',
    cwd: '..',
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    timeout: 600_000,
    env: { ZEYN_COMPONENTS_SERVER__ADDR: `127.0.0.1:${port}` },
  },
});
