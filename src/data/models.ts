// The model registry — one list, consumed everywhere.
//
// ── Why this file exists ─────────────────────────────────────────────────────
//
// There were two lists and they disagreed. `WorkerTable.tsx` offered
// `claude-sonnet-4-5` and `gemini-3-pro`; `WorkerCreateModal.tsx` offered
// `claude-opus-4-5` and `gemini-2.0-flash`. Same product, two pickers, different
// models — so which one a worker could actually run depended on which dialog
// created it.
//
// Both were also stale in a way that mattered: they listed **retired** models.
// `gpt-4o`, `gpt-4.1`, `gpt-4.1-mini` and `o4-mini` were retired in February
// 2026, and the picker was still offering them. Selecting one is not a
// degraded experience, it is a call that cannot succeed.
//
// ── Scope (razi-ruled, 2026-09-01) ───────────────────────────────────────────
//
// Three providers: **Anthropic, OpenAI, Google.** xAI/Grok was removed, and the
// DeepSeek / Moonshot / Groq / SeaLion / Dashscope / NVIDIA groups are gone with
// it — a worker could not run any of them, because `is_supported_provider` only
// accepts these three and there is nowhere to store a key for the others.
// Offering a model no key can authenticate is a dead end dressed as a choice.
//
// Gemini is the **primary** provider for the first rollout.

export interface ModelGroup {
  /** Provider id, matching `is_supported_provider` in `vault_commands.rs`. */
  provider: 'anthropic' | 'openai' | 'google'
  label: string
  models: string[]
}

export const MODEL_GROUPS: readonly ModelGroup[] = [
  {
    provider: 'google',
    label: 'Google Gemini',
    // ⚠ 3.1 Pro, not 3.5 Pro. 3.5 Pro was announced at I/O 2026 for a June
    // release that never shipped and was delayed indefinitely; it was in this
    // list until 2026-09-02 as a primary option, so a user could select a
    // model that cannot be called (razi-ruled: use 3.1 Pro).
    models: ['gemini-3.1-pro', 'gemini-3.5-flash'],
  },
  {
    provider: 'anthropic',
    label: 'Anthropic Claude',
    models: ['claude-opus-5', 'claude-sonnet-5', 'claude-fable-5', 'claude-haiku-4-5'],
  },
  {
    provider: 'openai',
    label: 'OpenAI GPT',
    models: [
      'gpt-5.5',
      'gpt-5.5-pro',
      'gpt-5.4',
      'gpt-5.4-mini',
      'gpt-5.4-nano',
      'gpt-5.3-codex',
    ],
  },
] as const

/** Every model id the app will offer. */
export const ALL_KNOWN_MODELS: ReadonlySet<string> = new Set(
  MODEL_GROUPS.flatMap((g) => g.models),
)

/**
 * The default a new worker gets.
 *
 * Gemini Flash: the primary provider for rollout 1, and its cheapest tier —
 * a default that costs the most is a bill nobody chose.
 */
export const DEFAULT_MODEL = 'gemini-3.5-flash'

/**
 * Which provider serves a model id.
 *
 * Mirrors the prefix mapping in `worker_commands.rs`. Returns `undefined` for
 * an unknown id rather than guessing a provider — a wrong guess sends the
 * request, and the user's key, to the wrong vendor.
 */
export function providerForModel(model: string): ModelGroup['provider'] | undefined {
  return MODEL_GROUPS.find((g) => g.models.includes(model))?.provider
}
