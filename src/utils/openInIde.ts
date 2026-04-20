// `openInIde(path, gotoLine?)` — single entry point used by chat-side
// tool-result renderers (FileContentDisplay, GrepResultsDisplay, etc.)
// to surface a file in the IDE tab.
//
// Behaviour:
//   1. Pushes / focuses a tab in `useIdeStore` (with optional line jump).
//   2. Switches the top-level tab to 'ide' via `useUiStore`.
//
// Both store writes happen outside React render, so this helper is safe
// to call from event handlers without being wrapped in hooks.

import { useIdeStore } from '../stores/ideStore'
import { useUiStore }  from '../stores/uiStore'

export function openInIde(path: string, gotoLine?: number): void {
  if (!path) return

  // Normalise Windows-style paths to forward slashes; the backend
  // resolvers accept both but the IDE store keys tabs by the exact
  // relative path, so keeping a single canonical form avoids duplicate
  // tabs when the same file is referenced with different slashes.
  const normalised = path.replace(/\\/g, '/')

  void useIdeStore.getState().openFile(normalised, gotoLine)
  useUiStore.getState().setActiveTab('ide')
}
