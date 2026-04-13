# Dissecting Claude Code Auto Memory & `MEMORY.md`

> Available since **Claude Code v2.1.59**. A full breakdown of how Claude Code writes its own notes, where they live, what gets saved, how it cleans itself up, and what's coming next.

---

## 1. The Core Concept: Two Different Memory Types

Before dissecting Auto Memory, the fundamental distinction must be clear:

```
┌─────────────────────────────────────────────────────────────────┐
│  CLAUDE.md                    │  MEMORY.md (Auto Memory)        │
│  ─────────────────────────    │  ──────────────────────────     │
│  Written BY YOU               │  Written BY CLAUDE              │
│  Your instructions to Claude  │  Claude's notebook about you    │
│  Static — only changes if     │  Dynamic — grows as you work    │
│  you edit it                  │                                  │
│  "Use pnpm, not npm"          │  "Build cmd is pnpm build,      │
│  "Never commit .env"          │   tests in __tests__/"          │
│  RULES                        │  LEARNINGS                      │
└─────────────────────────────────────────────────────────────────┘
```

The mental model: **CLAUDE.md is the instruction manual you write. MEMORY.md is the notebook Claude keeps for itself.**

---

## 2. What Auto Memory Actually Is

Auto Memory is a persistent directory where Claude records learnings, patterns, and insights as it works — without you doing anything. It is **not** Session Memory (which saves conversation summaries) — it operates at a deeper, durable level that persists across all sessions regardless of which conversation produced the knowledge.

**Before Auto Memory:** Useful things Claude discovered (undocumented commands, quirky workarounds, your corrections) vanished at session end. If you remembered and bothered, you'd manually add them to `CLAUDE.md`. Most people didn't.

**With Auto Memory:** Claude captures these on its own. The next session starts with that knowledge already loaded.

---

## 3. File Structure — Where It Lives

```
~/.claude/projects/<project>/memory/
├── MEMORY.md              ← Main index — loaded every session (≤200 lines)
├── debugging.md           ← Detailed debugging patterns
├── api-conventions.md     ← API design decisions
├── architecture.md        ← Structural decisions
└── ...                    ← Any topic files Claude creates
```

**How `<project>` is resolved:**
- Derived from the **git repository root** — not your current subdirectory
- All worktrees and subdirectories in the same repo share **one** auto memory directory
- `cd src/api/` and launch Claude → same memory as launching from repo root
- Git worktrees get **separate** memory directories (intentional — different branches, different state)
- Outside a git repo → working directory is used

**Custom location** (user/local settings only, not project settings):
```json
{
  "autoMemoryDirectory": "~/my-custom-memory-dir"
}
```

> Why not project settings? To prevent a shared `.claude/settings.json` from redirecting memory writes to sensitive locations.

---

## 4. The `MEMORY.md` Index File — How It Loads

`MEMORY.md` is the **entrypoint**. It is not where all memories are stored — it's an **index** that points to topic files.

**Loading rules:**
- Only the **first 200 lines OR 25KB** (whichever comes first) load at session startup
- This is a hard cap — anything beyond line 200 is invisible to Claude at startup
- Topic files (e.g., `debugging.md`) are referenced from the index and can be read on demand
- The index must stay lean: each entry should be ~one line, ~150 characters max

**What a healthy `MEMORY.md` looks like:**
```markdown
# Project Memory Index

## Build & Commands
- Build: `pnpm build`, dev server: `pnpm dev` (port 3000)
- Tests: `pnpm test` (Vitest + happy-dom)
- Lint: `pnpm lint` (ESLint + Prettier pre-commit hook)

## Architecture
- Monorepo: packages/frontend, packages/api, packages/shared
- API uses Fastify (migrated from Express on 2026-02-14)
- Auth: JWT in httpOnly cookie, not localStorage

## Key Decisions
- [Redis caching strategy](./architecture.md) — decided 2026-03-15
- [Debugging patterns](./debugging.md) — common Docker port conflicts

## User Preferences
- Prefers early returns over nested if/else
- Commit format: `type(scope): desc`
- Yarn/pnpm only — never npm

## Recent Learnings
- E2E tests require local Redis instance on port 6379
- Stripe webhook endpoint must validate signatures before processing
```

---

## 5. What Claude Saves — The Four Categories

Auto Memory captures across four organized categories:

### 5.1 Project Patterns
Build commands, test conventions, code style preferences, dependency choices. Things like:
- `build command is pnpm build`
- `tests live in __tests__/`
- `API uses Express with middleware in src/middleware/`

### 5.2 User Feedback / Corrections
When you correct Claude mid-session — "no, that's wrong" — it captures the correction so it doesn't make the same mistake again. This is the highest-signal input for Auto Memory.

Examples of correction triggers:
- "Don't use `any` in TypeScript"
- "We migrated from Express to Fastify — stop suggesting Express"
- "Use early returns, not nested conditions"

### 5.3 Architecture & Decisions
Structural choices and key decisions made during work:
- "Decided to use Redis for caching on 2026-03-15"
- "Product images in Cloudinary, not local storage"
- "Auth flow documented at docs/authentication.md"

### 5.4 Reference / Where Things Live
Pointers to where things are in the codebase:
- "Stripe webhook: `/app/api/webhooks/stripe`"
- "DB models: `src/models/`"
- "Environment config: `config/` — never hardcode"

---

## 6. What Claude Does NOT Save

Claude is selective — it doesn't save everything. The internal design targets **durable, non-obvious knowledge** — things that would waste time to rediscover, and that aren't visible in the codebase itself. It skips:
- Information obvious from reading the code
- One-off task-specific context unlikely to recur
- Things that change too frequently to be useful
- Conversation filler with no long-term value

> **From the leaked system prompt:** Claude is told to treat its own memory as a **hint, not a fact** — it should verify against real code before acting on anything remembered. This design reduces hallucinations from stale memories.

---

## 7. How to Trigger Saves — Explicit vs. Implicit

### Implicit (Automatic)
Claude decides on its own when something is worth saving. No action from you.

### Explicit (You Tell Claude)
```
"Remember that the API tests require a local Redis instance."
"Save this to memory: we always use named exports, never default."
"Remember this debugging approach for Docker port conflicts."
```

Claude writes to auto memory, **not** to `CLAUDE.md` unless you specify otherwise:
```
"Add this to CLAUDE.md — always use pnpm, not npm."
```

### Via `/memory` Command
Running `/memory` inside a session opens the memory editor — shows all loaded files (both `CLAUDE.md` and auto memory), lets you edit and reload.

---

## 8. Controlling Auto Memory

Auto Memory ships **enabled by default**. Here are all the control mechanisms:

### Toggle (Per Session)
```
/memory    → use the auto memory toggle in the UI
```

### `settings.json`
```json
{
  "autoMemoryEnabled": false
}
```

### Environment Variable (Highest Priority — Overrides Everything)
```bash
export CLAUDE_CODE_DISABLE_AUTO_MEMORY=1   # force off (for CI/CD)
export CLAUDE_CODE_DISABLE_AUTO_MEMORY=0   # force on
```

> **CI/CD Rule:** Always set `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` in automated pipelines. You don't want Claude accumulating notes about your build environment.

### Inspect What's Been Saved
```bash
# List all memory files for a project
ls ~/.claude/projects/<project>/memory/

# Read the index
cat ~/.claude/projects/<project>/memory/MEMORY.md

# Read a topic file
cat ~/.claude/projects/<project>/memory/debugging.md
```
All files are plain Markdown — edit, delete, or reorganize whenever you want.

---

## 9. Auto Dream — The REM Sleep Cycle

Auto Memory without cleanup is a note-taker who never organizes their notebook. Over time:
- Three entries say essentially the same thing
- A note says you use PostgreSQL; a newer one says you migrated to MySQL
- `MEMORY.md` exceeds 200 lines — entries stop loading
- "Yesterday's deploy bug" is meaningless a month later

**Auto Dream** is the solution. It's a background sub-agent that runs between sessions and consolidates everything Auto Memory has written — the way REM sleep consolidates the day's experiences into organized long-term memory.

> The dream agent's system prompt literally starts: *"You are performing a dream — a reflective pass over your memory files."*

### 9.1 The Dual-Gate Trigger

Both conditions must be true before a dream cycle fires:

```
Time gate:    ≥ 24 hours since last dream cycle
Session gate: ≥ 5 new sessions accumulated
```

- One long 2-day session → won't trigger (too few sessions)
- 10 quick sessions in 2 hours → won't trigger (not enough time)
- Prevents wasted consolidation on low-activity projects
- Observed: 913 sessions consolidated in ~8-9 minutes, runs in background

### 9.2 The Four Phases

**Phase 1 — Orient**
Claude scans the memory directory, reads `MEMORY.md`, skims existing topic files. Builds a map of the current memory state before making any changes. Answers: *"What do I already know, and how is it organized?"*

**Phase 2 — Gather Signal**
Searches recent session transcripts (the JSONL files stored at `~/.claude/projects/`) using **targeted grep** — not full reads. Searches for:
- User corrections ("no, that's wrong" / redirected approach)
- Explicit saves ("remember this")
- Recurring themes (patterns across multiple sessions)
- Important decisions (architecture, tool selections, workflow changes)

**Phase 3 — Consolidate**
The core maintenance phase:
- Merges new information into existing topic files (no near-duplicates)
- **Converts relative dates → absolute dates:** `"Yesterday we decided to use Redis"` → `"On 2026-03-15 we decided to use Redis"` — prevents temporal confusion as memories age
- **Deletes contradicted facts:** switched from Express to Fastify → old "API uses Express" entry removed
- **Removes stale memories:** debugging notes for a deleted file → pruned
- **Merges overlapping entries:** 3 sessions noting the same build command quirk → 1 clean entry

**Phase 4 — Prune & Index**
Rebuilds `MEMORY.md` as a lean pointer system:
- Keeps index under 200 lines and ~25KB
- Entries kept under 150 characters
- Verbose lines demoted: gist stays in index, detail moves to topic file
- Removes pointers to non-existent or stale files
- Resolves contradictions between the index and topic files
- Files that need no changes are left untouched — Auto Dream is surgical, not a full rewrite

### 9.3 Manual Trigger

```bash
# Inside a Claude Code session:
/dream

# Or just say:
"consolidate my memory files"
"run a dream cycle"
```

> `/dream` is still rolling out and may return "Unknown skill" on some versions. The natural language trigger works on all versions.

**When to trigger manually:**
- After a major refactor or framework migration
- After renaming/deleting many directories
- Before closing after a long, correction-heavy session
- When `MEMORY.md` looks bloated or contradictory

---

## 10. The Complete Claude Code Memory Stack

As of April 2026, there are four distinct memory layers:

```
┌───────────────────────────────────────────────────────────┐
│ Layer 1: CLAUDE.md                                         │
│   What: Your explicit rules and instructions               │
│   Written by: You                                          │
│   Scope: Global / Project / Local                          │
│   Persists: Always — re-read fresh every session           │
├───────────────────────────────────────────────────────────┤
│ Layer 2: Auto Memory (MEMORY.md + topic files)             │
│   What: Claude's learnings from working with you           │
│   Written by: Claude automatically                         │
│   Scope: Per git repository                                │
│   Persists: Indefinitely until edited/pruned               │
│   Loads: First 200 lines of MEMORY.md at session start     │
├───────────────────────────────────────────────────────────┤
│ Layer 3: Session Memory                                    │
│   What: Conversation-level summaries for cross-session     │
│         recall within a project                            │
│   Written by: Claude                                       │
│   Scope: Per conversation/session                          │
│   Persists: Until cleared                                  │
├───────────────────────────────────────────────────────────┤
│ Layer 4: Auto Dream (Consolidation)                        │
│   What: Background cleanup of Layer 2                      │
│   Written by: Claude sub-agent                             │
│   Trigger: 24h elapsed + 5+ sessions                       │
│   Persists: Rewrites/prunes Layer 2 files                  │
└───────────────────────────────────────────────────────────┘
```

**Analogy:** CLAUDE.md = constitution. Auto Memory = daily notes. Session Memory = short-term recall. Auto Dream = REM sleep.

---

## 11. Subagents and Auto Memory

Subagents spawned during Claude Code sessions can maintain their own memory context. Configure this via the `memory` frontmatter field in subagent definitions:

```yaml
memory: user       # Load user-level memory only
memory: project    # Load project-level memory only
memory: all        # Load all memory scopes
```

This means a specialized subagent (e.g., a code review agent) can have its own focused memory that doesn't pollute the main project memory.

---

## 12. Known Limitations

| Limitation | Detail |
|------------|--------|
| **200-line hard cap** | Anything beyond line 200 of `MEMORY.md` is invisible at session start |
| **No semantic search** | Memory retrieval is keyword/grep based — "port conflicts" won't find a note that says "docker-compose mapping" |
| **Not portable** | Stays in Claude Code only — switch to a different agent and you start from zero |
| **Can go stale** | Without Auto Dream running, contradictions and outdated entries accumulate |
| **Claude decides what to save** | May miss things you'd want remembered; use explicit saves for critical knowledge |
| **Not machine-enforceable** | Like `CLAUDE.md`, Claude *tries* to honor memory — it's context, not enforced configuration |

---

## 13. KAIROS — What's Coming (Unreleased)

In March 2026, Claude Code v2.1.88 accidentally shipped a 59.8 MB source map containing the full 512,000-line TypeScript codebase. The memory system caught attention. Among the findings: a feature called **KAIROS**.

Named after the Greek word for *"the right moment"*, KAIROS appears 150+ times in the source. It would transform Claude Code from a reactive tool into a **background always-on assistant**:
- Continuously watches your project
- Keeps a running log of observations, decisions, and actions throughout the day
- Acts at the right moment without being invoked
- Has a built-in 15-second blocking budget for decisions

KAIROS is not in any public build yet — but it represents where Auto Memory is likely heading.

---

## 14. Best Practices

### Do This
- ✅ **Use explicit saves** for critical findings: `"Remember that X requires Y"`
- ✅ **Run manual dream** after major refactors: `"consolidate my memory files"`
- ✅ **Review periodically**: run `cat ~/.claude/projects/<project>/memory/MEMORY.md`
- ✅ **Disable in CI/CD**: `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`
- ✅ **Let Auto Dream run** — don't fight the cleanup cycles

### Don't Do This
- ❌ Don't duplicate between `CLAUDE.md` and Auto Memory — if it's a rule, it goes in `CLAUDE.md`; if it's a learned pattern, let Auto Memory handle it
- ❌ Don't let `MEMORY.md` grow past 200 lines without running a dream cycle
- ❌ Don't store sensitive info — memory files are local plaintext

### The Golden Rule of Split Responsibilities
```
If it's a RULE that must always apply  → CLAUDE.md
If it's something Claude LEARNED       → Auto Memory (MEMORY.md)
If it might CHANGE over time           → Auto Memory (will get updated)
If it must NEVER be forgotten          → Both (CLAUDE.md + explicit memory save)
```

---

## 15. Quick Command Reference

```bash
# Check your version (need v2.1.59+)
claude --version

# Open memory editor (see all loaded files, toggle auto memory)
/memory

# Manual dream consolidation
/dream
# or: "consolidate my memory files"

# Inspect memory files
ls ~/.claude/projects/<project>/memory/
cat ~/.claude/projects/<project>/memory/MEMORY.md

# Disable for a session (env var)
CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 claude

# Custom memory directory (in ~/.claude/settings.json)
{ "autoMemoryDirectory": "~/custom-memory-path" }
```

---

## 16. Session Flow — How It All Works Together

```
New Session Starts
      │
      ▼
Load CLAUDE.md files (global → project → local)
      │
      ▼
Load MEMORY.md (first 200 lines / 25KB)
      │
      ▼
Claude has: your rules + its own accumulated learnings
      │
      ▼
Work happens → Claude may write new notes to memory files
      │
      ▼
Session ends → check dream trigger conditions
      │
  ┌───┴──────────────────────────────────┐
  │ 24h+ elapsed AND 5+ new sessions?   │
  └──────┬───────────────────────────────┘
         │ Yes
         ▼
   Auto Dream runs in background
   Phase 1: Orient → Phase 2: Gather → Phase 3: Consolidate → Phase 4: Prune
         │
         ▼
   MEMORY.md is clean, under 200 lines, contradictions resolved
         │
         ▼
   Next session starts with organized, accurate knowledge
```

---

*Auto Memory is Claude Code's institutional memory. The more you work with it on a project, the faster and more accurate it gets — not because the model changed, but because the context got better.*