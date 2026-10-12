export default {
  testRunner: 'command',
  commandRunner: {
    command: 'npx --no-install vitest run --project node tests/modules',
  },
  coverageAnalysis: 'off',
  disableTypeChecks: false,
  concurrency: 4,
  timeoutMS: 60000,
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
