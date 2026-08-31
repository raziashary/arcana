// Vitest configuration.
//
// Before P0 this project had no frontend test framework at all — no runner, no
// `test` script, and not a single `.test.ts` file. Every frontend defect in the
// restructure's register was found by reading code, not by running it.
//
// Kept separate from vite.config.ts so the app build has no test-only config in
// its graph; the two share the same alias and plugin setup via `mergeConfig`.

import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      globals: true,
      setupFiles: ['./src/test/setup.ts'],
      include: ['src/**/*.{test,spec}.{ts,tsx}'],
      exclude: ['node_modules/**', 'dist/**', 'target/**', 'submodules/**', 'e2e/**'],
      // A test that neither passes nor fails is not a gate. Fail the run when a
      // file matches the include glob but contains no tests.
      passWithNoTests: false,
      coverage: {
        provider: 'v8',
        include: ['src/**/*.{ts,tsx}'],
        exclude: ['src/**/*.{test,spec}.{ts,tsx}', 'src/test/**'],
      },
    },
  }),
)
