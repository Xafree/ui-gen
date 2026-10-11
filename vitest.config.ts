import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';

const litDecorators = {
  tsconfigRaw: {
    compilerOptions: {
      experimentalDecorators: true,
      useDefineForClassFields: false,
    },
  },
};

export default defineConfig({
  test: {
    passWithNoTests: true,
    coverage: {
      provider: 'v8',
      include: ['src/modules/**/*.ts'],
      reporter: ['text', 'lcov'],
      reportsDirectory: 'coverage',
      thresholds: {
        lines: 80,
        branches: 80,
        functions: 80,
        statements: 80,
      },
    },
    projects: [
      {
        extends: true,
        test: {
          name: 'node',
          environment: 'node',
          include: ['tests/modules/**/*.test.ts', 'tests/main/**/*.test.ts'],
        },
      },
      {
        extends: true,
        esbuild: litDecorators,
        optimizeDeps: {
          include: ['lit', 'lit/decorators.js'],
        },
        test: {
          name: 'browser',
          include: ['tests/renderer/**/*.test.ts'],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright(),
            instances: [{ browser: 'chromium' }],
          },
        },
      },
    ],
  },
});
