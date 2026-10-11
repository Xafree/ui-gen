import { defineConfig } from '@playwright/test';

process.env['UIGEN_SHOW_INACTIVE'] = '1';

export default defineConfig({
  testDir: 'tests/e2e',
  testMatch: '**/*.spec.ts',
  fullyParallel: false,
  workers: 1,
  forbidOnly: true,
  retries: 0,
  reporter: 'list',
  use: {
    trace: 'retain-on-failure',
  },
});
