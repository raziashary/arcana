// One confirmation for every destructive action that has no undo.
//
// The message always names what is about to go and says it cannot be brought
// back, so the user decides on facts instead of on a bare "Are you sure?".
// It wraps the webview's own confirm dialog, which the app already used for a
// few deletes; wrapping it keeps the wording in one place and lets tests
// answer yes or no. v1 has no undo (D-017), so the confirmation is the only
// protection these actions get.

/**
 * Ask before a destructive action. `action` is the verb phrase ("Delete the
 * hook \"lint\""), `lost` is what disappears with it. Returns true to proceed.
 */
export function confirmDestructive(action: string, lost: string): boolean {
  return window.confirm(`${action}?\n\n${lost}\n\nThis cannot be undone.`)
}
