// Vitest global setup.
//
// Two jobs: bring in jest-dom's matchers, and make the Tauri IPC bridge
// unavailable-by-default so a test that accidentally reaches for the backend
// fails loudly instead of hanging or silently resolving undefined.

import '@testing-library/jest-dom/vitest'
import { afterEach, vi } from 'vitest'
import { cleanup } from '@testing-library/react'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

// jsdom implements neither of these, and several components call them on mount.
if (!window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia
}

if (!window.ResizeObserver) {
  window.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof window.ResizeObserver
}

// Tauri's `invoke` and `listen` reach a Rust process that does not exist under
// jsdom. Left unmocked they reject with an opaque error; mocked to throw, a
// test that depends on the backend says so in its failure message.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    throw new Error(
      `Test called the Tauri command "${cmd}" without mocking it. ` +
        `Mock '@tauri-apps/api/core' in this test file, or test a pure function instead.`,
    )
  }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}))
