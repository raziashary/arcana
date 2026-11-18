import { describe, expect, it } from 'vitest'
import { explainTurnError } from './errorHints'

describe('explainTurnError', () => {
  it.each([
    ['Anthropic stream 401 Unauthorized: invalid x-api-key', /Settings → Providers/],
    ['OpenAI 403 Forbidden', /Settings → Providers/],
    ['Budget exceeded: daily ceiling reached', /spend limit/],
    ['Google 429 Too Many Requests', /limiting requests/],
    ['Anthropic stream 529 Overloaded', /busy/],
    ['Unsupported provider: acme', /Pick a provider/],
  ])('%s gets a next step', (message, hint) => {
    const shown = explainTurnError(message)
    expect(shown.startsWith(`Error: ${message}`)).toBe(true)
    expect(shown).toMatch(hint)
  })

  it('shows an unrecognised message unchanged rather than guessing', () => {
    expect(explainTurnError('Something odd happened')).toBe('Error: Something odd happened')
  })

  it('never returns an empty message', () => {
    expect(explainTurnError('   ')).toBe('Error: The turn failed without a message.')
  })

  it('does not treat a spend limit as an auth failure', () => {
    expect(explainTurnError('Budget exceeded: 401 tokens over')).toMatch(/spend limit/)
  })
})
