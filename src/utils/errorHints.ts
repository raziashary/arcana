// Turn a failed turn's error text into something the user can act on.
//
// The backend sends the adapter's own message (for example
// "Anthropic stream 401 Unauthorized: ..."), which names the failure but not
// what to do about it. This appends one sentence of next step. It is a
// heuristic over our own adapters' message formats, so an unrecognised message
// is shown unchanged rather than given a guess.

const HINTS: ReadonlyArray<{ test: RegExp; hint: string }> = [
  // Most specific first: a budget message can contain a number that looks like a status.
  {
    test: /budget exceeded|spend (cap|limit|ceiling)|daily (cap|ceiling|budget)/i,
    hint: 'A spend limit stopped this turn. Raise or clear the limit in Settings, or try again tomorrow.',
  },
  {
    test: /\b(401|403)\b|invalid api key|unauthori[sz]ed|authentication/i,
    hint: 'The provider rejected the API key. Open Settings → Providers and enter a valid key.',
  },
  {
    test: /\b429\b|rate.?limit|quota|insufficient[_ ]funds|billing/i,
    hint: "The provider is limiting requests or the account's quota is used up. Wait a moment, or check the account's billing with the provider.",
  },
  {
    test: /\b(500|502|503|504|529)\b|overloaded|timed? ?out|unreachable|transport|connection (refused|reset)/i,
    hint: 'The provider could not be reached or is busy. Try again in a moment.',
  },
  {
    test: /unsupported provider|no provider|provider not configured/i,
    hint: 'Pick a provider and model in Settings → Providers.',
  },
]

/**
 * Whether the backend ended the turn because the user stopped it. The engine
 * reports a cancel through the same `agent:error` event as a failure, with the
 * fixed text "The run was cancelled." (or "... before it started."), so the
 * chat would show the user's own Stop button as a red error. A typed
 * cancel-versus-error payload is the better fix and is recorded in the backlog.
 */
export function isCancellation(message: string): boolean {
  return /^the run was cancelled\b/i.test(message.trim())
}

/** The message to show for a failed turn: the error, plus a next step when one is known. */
export function explainTurnError(message: string): string {
  const text = message.trim() || 'The turn failed without a message.'
  const match = HINTS.find(({ test }) => test.test(text))
  return match ? `Error: ${text}\n\n${match.hint}` : `Error: ${text}`
}
