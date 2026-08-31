// ESLint flat config.
//
// Until P0 this project had NO linter: `npm run lint` was literally
// `tsc --noEmit`, and CI ran that same command a second time under the name
// "ESLint" — two green checks, one of them measuring nothing.
//
// This config is deliberately correctness-first rather than style-first. The
// codebase is ~39k lines that have never been linted, so a full recommended
// preset at error severity would report hundreds of findings and the gate would
// be switched off within a day. Rules that catch real defects are errors;
// rules that express taste are off. Tightening happens by promoting rules
// deliberately, not by turning the whole preset on and adding suppressions.

import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  {
    ignores: [
      'dist/**',
      'build/**',
      'node_modules/**',
      // Rust build output. `target/` holds tauri-codegen asset bundles — minified
      // and sometimes binary .js — which ESLint will happily try to parse.
      'target/**',
      'src-tauri/**',
      'src-cli/**',
      'submodules/**',
      'installers/**',
      'coverage/**',
      '*.config.js',
      '*.config.ts',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      globals: { ...globals.browser, ...globals.es2021 },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      // ── Correctness: these catch defects, so they fail the build ──────────
      'react-hooks/rules-of-hooks': 'error',
      'no-const-assign': 'error',
      'no-dupe-keys': 'error',
      'no-duplicate-case': 'error',
      'no-unreachable': 'error',
      'no-fallthrough': 'error',
      'no-self-compare': 'error',
      'no-unsafe-negation': 'error',
      'no-unsafe-optional-chaining': 'error',
      'use-isnan': 'error',
      'valid-typeof': 'error',
      '@typescript-eslint/no-misused-new': 'error',
      '@typescript-eslint/no-unsafe-declaration-merging': 'error',

      // A missing dependency is a real bug class (stale closures over state),
      // but the existing code has many. Warn now, promote in P7 with the UI
      // work, when the components are being touched anyway.
      'react-hooks/exhaustive-deps': 'warn',
      'react-refresh/only-export-components': 'off',

      // ── Deliberately not errors yet ──────────────────────────────────────
      // `any` is pervasive in the current IPC surface. P2 replaces those with
      // generated provider types; promoting this now would be ~hundreds of
      // findings against code already scheduled for replacement.
      '@typescript-eslint/no-explicit-any': 'off',
      // Unused vars are worth seeing but are not defects; tsc already fails on
      // unused locals where it matters.
      '@typescript-eslint/no-unused-vars': [
        'warn',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrors: 'none' },
      ],
      '@typescript-eslint/ban-ts-comment': 'warn',
      '@typescript-eslint/no-empty-object-type': 'off',
    },
  },
  {
    // Test files get the test globals and a looser hand.
    files: ['**/*.test.{ts,tsx}', 'src/test/**'],
    languageOptions: {
      globals: { ...globals.node },
    },
    rules: {
      '@typescript-eslint/no-unused-expressions': 'off',
    },
  },
)
