export default {
  testRunner: 'vitest',
  vitest: {
    configFile: 'vitest.config.ts',
    dir: 'tests/modules',
  },
  mutate: ['src/modules/*/domain/**/*.ts', 'src/modules/*/application/**/*.ts'],
  incremental: true,
  incrementalFile: 'reports/stryker-incremental.json',
  reporters: ['clear-text', 'progress', 'html'],
  htmlReporter: {
    fileName: 'reports/mutation/index.html',
  },
  thresholds: {
    high: 90,
    low: 80,
    break: 80,
  },
};
