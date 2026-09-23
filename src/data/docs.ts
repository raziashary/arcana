// Documentation data — single source of truth for the Docs panel.
// Content is structured as typed blocks so the renderer can apply
// Arcana-native styling without parsing markdown.

export type Block =
  | { t: 'h2';      text: string }
  | { t: 'h3';      text: string }
  | { t: 'prose';   text: string }
  | { t: 'callout'; text: string; variant?: 'warn' | 'info' }
  | { t: 'list';    items: string[] }
  | { t: 'table';   headers: string[]; rows: string[][] }
  | { t: 'code';    lang: string; code: string }

export interface DocSection {
  id:       string
  num:      string
  title:    string
  category: string
  icon:     string
  lead:     string
  anchors:  string[]
  blocks:   Block[]
}

export const NAV_CATEGORIES: { label: string; ids: string[] }[] = [
  { label: 'Overview',    ids: ['what', 'arch'] },
  { label: 'Agent Modes', ids: ['pa-mode', 'fleet'] },
  { label: 'Automation',  ids: ['recipes', 'skills', 'hooks'] },
  { label: 'Interface',   ids: ['settings', 'dashboard', 'ide', 'graph'] },
  { label: 'Internals',   ids: ['internals', 'glossary'] },
]

// 🔴 `P8-61`. The `SECTIONS` array that stood here (~290 lines) is DELETED,
// and so is `docsMore.ts` (254 lines) alongside it.
//
// **Neither was rendered.** `Docs.tsx` builds its content from
// `A_SECTIONS`, `B_SECTIONS`, `C_SECTIONS` and `D_SECTIONS`, and imports from
// this file only `NAV_CATEGORIES` and the two types. `MORE_SECTIONS` had no
// importer at all. So half the documentation "source of truth" was a second,
// diverged copy that no user could reach — which is why it was also the most
// stale: nothing ever showed it to anyone who would have noticed.
//
// What survives here is what `Docs.tsx` actually uses.
