import { defineConfig } from 'electron-vite';

export default defineConfig({
  main: {},
  preload: {
    build: {
      rollupOptions: {
        output: {
          format: 'cjs',
        },
      },
    },
  },
  renderer: {
    esbuild: {
      tsconfigRaw: {
        compilerOptions: {
          experimentalDecorators: true,
          useDefineForClassFields: false,
        },
      },
    },
  },
});
