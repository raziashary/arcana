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
    // ⚠ Two corrections live here, both from selecting models that cannot be
    // called.
    //
    // 3.5 Pro was announced at I/O 2026 for a June release that never shipped
    // and was delayed indefinitely, yet sat here as a primary option
    // (razi-ruled: use 3.1 Pro). Then the live smoke got a 404 for `gemini-3.1-pro`
    // too — the id it actually ships under is `gemini-3.1-pro-preview`, and
    // no fixture test could have found that.
    //
    // Fast model is 3.7 Flash as of 2026-09-02 (razi-ruled): newer than 3.5
    // Flash and half the price at the current published rate.
    models: ['gemini-3.1-pro-preview', 'gemini-3.7-flash'],
  },
  {
    provider: 'anthropic',
    label: 'Anthropic Claude',
    models: ['claude-opus-5', 'claude-sonnet-5', 'claude-fable-5', 'claude-haiku-4-5'],
  },
  {
    provider: 'openai',
    label: 'OpenAI GPT',
    // gpt-5.5-pro and gpt-5.3-codex were removed on 2026-09-02 (razi-ruled).
    // No verified price could be found for either, and an unpriced model bills
    // at $0 — which makes its spend invisible to the daily cap and lets it run
    // uncapped. They return when a price does.
    models: ['gpt-5.5', 'gpt-5.4', 'gpt-5.4-mini', 'gpt-5.4-nano'],
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
