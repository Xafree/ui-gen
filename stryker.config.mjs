export default {
  testRunner: 'command',
  commandRunner: {
    command: 'npx --no-install vitest run --project node tests/modules',
  },
  coverageAnalysis: 'off',
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
