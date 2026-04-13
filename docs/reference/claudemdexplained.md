# Dissecting `CLAUDE.md` in Claude Code

> A complete breakdown of what it is, how it works, who it's for, and how to set it up — including the full hierarchy, loading behavior, and best practices.

---

## 1. What Is `CLAUDE.md`?

`CLAUDE.md` is a **plain Markdown file** that Claude Code reads automatically at the start of every session. It acts as **persistent memory injected into Claude's system prompt** — everything you write in it is loaded as context before you type your first message.

Think of it as a memo to a teammate who forgets everything overnight. Every morning, that teammate reads the memo and knows your project's rules, commands, structure, and conventions — without you repeating yourself.

**Without `CLAUDE.md`:** Claude makes educated guesses about your project.  
**With `CLAUDE.md`:** Claude starts every session already knowing your stack, commands, conventions, and constraints.

---

## 2. How It Works Technically

When you run `claude` in a directory, Claude Code:

1. Searches the filesystem (from current directory upward toward root)
2. Discovers all `CLAUDE.md` files across the hierarchy
3. Assembles them into a single context block prefixed with:

```
"Codebase and user instructions are shown below. Be sure to adhere to these instructions. IMPORTANT: These instructions OVERRIDE any default behavior..."
```

4. Injects this block **before your conversation begins** — it becomes part of the system prompt
5. This survives `/clear`, `/compact`, and context resets — it is **re-read fresh every session**

> **Key distinction:** Unlike conversation history (which degrades/compacts), `CLAUDE.md` is stable and always present.

**Lazy loading for subdirectories:** `CLAUDE.md` files in subdirectories are NOT loaded upfront. They are loaded on demand when Claude reads files in that directory. This saves context window space on large projects.

**Max size:** 40,000 characters per file. Files that exceed this are flagged and may not be fully read.

---

## 3. The Three-Tier Hierarchy

Claude Code loads `CLAUDE.md` from multiple locations, merged in priority order. **Later (more specific) files win conflicts.**

```
LOAD ORDER (lowest → highest priority)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[0] SYSTEM / MANAGED (lowest, enterprise only)
    /etc/claude-code/CLAUDE.md
    Set by IT/admins. Cannot be excluded via settings.
    Applies to ALL users on the machine.

[1] GLOBAL / USER
    ~/.claude/CLAUDE.md
    Your personal preferences. Private, never committed.
    Applies to ALL projects on your machine.

[2] PROJECT
    ./CLAUDE.md  (preferred)
    or ./.claude/CLAUDE.md  (alternative)
    Team-shared conventions. Committed to version control.
    Applies to this project only.

[3] LOCAL / PERSONAL PROJECT (highest priority)
    ./.claude/CLAUDE.local.md   ← gitignored
    Personal overrides for this project.
    Applies to you on this project only.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Precedence: Local > Project > Global > System
```

**Conflict resolution example:**  
Global says `indent: 2 spaces` → Project says `indent: 4 spaces` → **4 spaces wins**.

---

## 4. Who Is Each Tier For?

| Tier | Who Writes It | Who Reads It | Committed to Git? |
|------|--------------|--------------|-------------------|
| **System** | IT/Admins | Everyone on machine | N/A |
| **Global** `~/.claude/CLAUDE.md` | Individual developer | Just you, all projects | ❌ No |
| **Project** `./CLAUDE.md` | Team lead / whole team | Everyone on project | ✅ Yes |
| **Local** `.claude/CLAUDE.local.md` | Individual developer | Just you, this project | ❌ No (gitignored) |

---

## 5. What Goes In Each Tier

### Global (`~/.claude/CLAUDE.md`) — Personal Preferences
```markdown
# Personal Dev Preferences

## Communication
- Respond in English unless I explicitly write in another language
- Keep explanations concise; I'm a senior dev

## Commit Style
- Commit format: `type(scope): description`
- Types: feat, fix, refactor, style, docs, test, chore

## Code Style (all projects)
- Prefer named exports over default exports
- Always use strict TypeScript (no `any`)
```

### Project (`./CLAUDE.md`) — Team Conventions
```markdown
# My Project

## Stack
- TypeScript + Next.js 14 App Router
- PostgreSQL via Prisma
- Tailwind CSS, no custom CSS files

## Commands
- `npm run dev`: Start dev server (port 3000)
- `npm test`: Run Jest
- `npm run test:e2e`: Playwright E2E
- `npm run lint`: ESLint
- `npm run db:migrate`: Prisma migrations

## Architecture
- `/app`: Next.js pages and layouts
- `/components/ui`: Reusable UI components
- `/lib`: Utilities and shared logic
- `/prisma`: Database schema and migrations
- `/app/api`: API routes

## Conventions
- NEVER commit .env files
- Stripe webhook handler must validate signatures
- Product images stored in Cloudinary, not locally
- See @docs/authentication.md for auth flow

## Git
- Branch names: `feat/<ticket>-short-description`
- Run tests and typecheck before committing
```

### Local (`CLAUDE.local.md`) — Personal Notes
```markdown
# My Local Notes (not shared)

## Personal Dev Setup
- Using local sandbox at http://localhost:9000 for testing
- My debug breakpoints strategy: always isolate auth first
- DB reset command: `npm run db:reset:dev`
```

---

## 6. `CLAUDE.md` vs `settings.json` — Critical Distinction

These two are often confused but work very differently:

| | `CLAUDE.md` | `settings.json` |
|--|--|--|
| **Type** | Guidance / instructions | Enforcement / configuration |
| **Claude reads it?** | Yes, reasons over it | No, Claude Code enforces it before Claude sees your prompt |
| **Analogy** | Style guide for a human dev | Linter config / CI gate |
| **Can Claude ignore it?** | In theory, yes (it's guidance) | No, it's enforced at system level |
| **Examples** | Code style, arch notes, commands | Permissions, allowed tools, hooks |

**Rule of thumb:**
- Want Claude to *know* something → `CLAUDE.md`
- Want Claude Code to *enforce* something → `settings.json`

Hooks (shell commands that fire on lifecycle events like `PostToolUse`) also go in `settings.json`, not `CLAUDE.md`. Where CLAUDE.md says *"please format with Prettier,"* a hook **actually runs Prettier** after every edit.

---

## 7. Modular Rules: `.claude/rules/` Directory

For large projects, you don't have to cram everything into one `CLAUDE.md`. You can organize into **topic-specific files**:

```
.claude/
└── rules/
    ├── testing.md
    ├── api-design.md
    ├── git-conventions.md
    └── security.md
```

Rules support **path scoping** — they only inject when Claude is working on matching files:

```markdown
---
paths:
  - "src/api/**"
  - "src/services/**"
---

Always use dependency injection.
Never import concrete implementations directly.
```

This keeps context lean and avoids loading unrelated rules.

---

## 8. `@` Import Syntax — Reference Other Files

You can reference external docs directly inside `CLAUDE.md`:

```markdown
## Authentication
See @docs/authentication.md for the full auth flow.

## API Conventions
@./docs/api-conventions.md
```

- Both relative and absolute paths work
- Recursive imports supported (max depth: 5)
- First import from external path triggers an approval dialog
- Prevents duplication — reference your existing docs instead of copying

---

## 9. How to Set It Up

### Option A: Auto-generate with `/init` (Recommended Starting Point)
```bash
cd your-project
claude        # launch Claude Code
/init         # Claude scans your project and generates CLAUDE.md
```

Claude examines: `package.json`, existing docs, config files, folder structure — then generates a starter file with detected build commands, test instructions, key directories, and conventions.

**Then:** Delete what's obvious/generic. Keep only what adds value. Every line competes for context window space.

### Option B: Create Manually
```bash
touch CLAUDE.md        # project level
touch ~/.claude/CLAUDE.md   # global level
touch .claude/CLAUDE.local.md   # personal project level (add to .gitignore)
```

Add to `.gitignore`:
```
.claude/CLAUDE.local.md
.claude/settings.local.json
```

### Option C: Let Claude Write It For You (Mid-Session)
In any Claude Code session, use `#` to add to memory:
```
# Always use Ruff for Python linting in this project
```
Claude will append this to the appropriate `CLAUDE.md` file.

### Open the Memory Editor
```bash
/memory    # inside a Claude Code session
```
This opens the memory file editor — shows all loaded files, lets you edit and reload.

---

## 10. `CLAUDE.md` vs Auto Memory (MEMORY.md)

Starting in Claude Code v2.1.59, there's a second memory system called **Auto Memory**:

| | `CLAUDE.md` | Auto Memory (`MEMORY.md`) |
|--|--|--|
| **Written by** | You | Claude automatically |
| **Content** | Your explicit instructions | Learnings Claude accumulates (patterns, bug insights, arch notes) |
| **Control** | Manual | Automatic (Claude decides what to save) |
| **Loaded** | Full file, every session | First 200 lines or 25KB, session start |
| **Toggle** | Always on | `/memory` → auto memory toggle |

They are **complementary**: `CLAUDE.md` = your rules, `MEMORY.md` = Claude's notes.

---

## 11. Monorepo Pattern

For monorepos, use nested `CLAUDE.md` files:

```
monorepo/
├── CLAUDE.md              ← shared conventions for everything
├── packages/
│   ├── frontend/
│   │   └── CLAUDE.md      ← frontend-specific (loaded lazily)
│   ├── api/
│   │   └── CLAUDE.md      ← API-specific (loaded lazily)
│   └── shared/
│       └── CLAUDE.md      ← shared utils conventions
```

Claude loads root `CLAUDE.md` upfront and subdirectory files on demand when working in those directories.

---

## 12. What NOT to Put in `CLAUDE.md`

- ❌ API keys, credentials, database connection strings
- ❌ Anything you'd be embarrassed to make public (project `CLAUDE.md` is committed to git)
- ❌ Things that change weekly — use prompts for those
- ❌ Overly verbose documentation — context is precious
- ❌ Instructions so broad they won't change Claude's behavior in practice

---

## 13. Quick Reference — File Locations

```
~/.claude/CLAUDE.md                 ← Global: your personal prefs (all projects)
./CLAUDE.md                         ← Project: team conventions (committed)
./.claude/CLAUDE.md                 ← Project (alternative location)
./.claude/CLAUDE.local.md           ← Personal project notes (gitignored)
./.claude/rules/*.md                ← Modular rules (path-scopable)
/etc/claude-code/CLAUDE.md          ← System/managed (enterprise, admin-set)
```

---

## 14. Real Example: AIRS Foundry Forge Project

```markdown
# AIRS Forge — Agent System

## Stack
- Python 3.11 + FastAPI backend
- TypeScript React frontend
- Docker Compose multi-container
- GCP Cloud Run (asia-southeast2)
- MCP protocol for tool integration
- A2A protocol for agent-to-agent communication

## Commands
- `docker compose up`: Start full stack locally
- `npm run dev`: Frontend dev server (port 3000)
- `pytest tests/`: Run backend tests
- `ruff check .`: Python linting
- `npm run typecheck`: TypeScript checks

## Architecture
- `/forge-api`: FastAPI agent orchestration backend
- `/forge-ui`: React frontend (visual node editor — in development)
- `/mcp-tools`: MCP server definitions
- `/agents`: Agent definitions and schemas

## Agent Layer Rules
- Photoshop MCP tools: layer ordering is visual hierarchy via array INDEX
  (index 0 = bottom, highest index = top of layer stack)
- A2A handoffs must always pass explicit context — no implicit state
- Never hardcode API keys; use GCP Secret Manager refs

## Deployment
- Target region: asia-southeast2 (DO NOT change without team sign-off)
- AIRS Arcade GCE VM is separate — do not touch its docker-compose
- Cloud Run services are stateless; all state in Firestore or GCS

## Git
- Branch: `feat/<ticket>-short-desc` or `fix/<ticket>-short-desc`
- Always run lint + typecheck before committing
- PR must reference ticket ID
```

---

*Keep it concise. Keep it current. Every line earns its place.*