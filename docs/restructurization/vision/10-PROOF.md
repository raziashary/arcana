# 10 — Proof: evidence, tests and gates

| | |
|---|---|
| **Parent** | `00-…-MASTER-PLAN.md` §Part 1 (INV-4, INV-8), §Part 1B (M-28), §Part 7 |
| **Locks** | Hands **ADR-04-03** to `00 §Part 8` |
| **Status** | PLANNED |
| **Grounding** | `_grounding/G0 §2` · every `G*` findings table (~415 defects — the population these gates must catch) |

---

## What this is

How the restructure proves it worked. This spec exists because of a single measured fact: **the entire ~415-defect register was accumulated by code that passes every check the project currently runs.**

That is the design constraint. A test suite that would not have caught BUG-COMPILE, BUG-DELIVER or BUG-CHAT is not a test suite — it is decoration. All three are invisible to `cargo clippy` and `tsc`, which are the only two gates that can currently fail.

**The subtle failures this subsystem is prone to:**

1. **Green that means nothing.** `continue-on-error: true` on a test job; a lint step aliased to a typecheck; a smoke that skips PRs.
2. **Testing the writer, not the loop.** Asserting a row was inserted proves nothing about whether the consumer reads it. Half this register is writer-with-no-reader.
3. **Testing on the dev machine only.** Windows is the development platform, and Windows is the platform on which the fleet cannot complete a task.
4. **A guard tested by its own assumptions.** A permission test that constructs `ToolContext` by hand passes while the three real construction sites hardcode the fields that make the guard work.

---

## 1. Evidence classes

A claim is done only with evidence of the right class. `DOC-CLAIM` never counts.

| Class | Means | Example |
|---|---|---|
| `CODE-PRESENT` | The code exists and was read | "the graph module has a query API" |
| `TEST-GREEN` | **A test that can fail** passes | "compaction fires and its output is used" |
| `MEASURED` | A number produced by running something | "input tokens do not grow linearly over 30 turns" |
| `PROVEN-READ` | A test asserts the intended consumer reads it back | "a recalled memory appears in the prompt bytes" |
| `PLATFORM-TRIPLE` | Verified on Windows, Linux **and** macOS | the end-to-end smoke |

---

## 2. The headline acceptance test

> **A user installs AutoDev on a clean machine of each of the three OSes, enters their own API key in the UI, points it at a repo, types one sentence, and receives a build-verified change in that repo which they can then run — without reading documentation, and without any Python on the machine.**

Each clause is a mechanic, and each is currently broken:

| Clause | Mechanic | Broken today because |
|---|---|---|
| clean machine | M-27, M-29 | the installer needs `uv` + Python on PATH |
| enters their own key | M-30, M-35 | no onboarding; the key path is several clicks deep |
| points it at a repo | M-14 | `load_workspace_dir()` returns `""` silently |
| one sentence | M-01 | works |
| build-verified | M-13 | a successful build is recorded as failed |
| in that repo | M-14, INV-5 | output lands in `~/.autodev/agents/` |
| which they can run | M-15 | there is no run/handoff step at all |
| without documentation | M-35…M-39 | first message errors into a blank bubble |
| no Python | M-29 | 225 MB of it is bundled |

**This single test is the restructure's definition of done.**

---

## 3. The product proof

Three things must be demonstrably true beyond the acceptance test:

1. **Cost is honest** (`MEASURED`) — a session's reported cost matches the provider's own billing for that session, within rounding. Today input tokens are 0 for all three cloud providers in streaming and the price fallback catches 7 of 11 models.
2. **Context is bounded** (`MEASURED`) — a 30-turn session's input tokens do not grow linearly with turn count.
3. **The fleet recovers** (`TEST-GREEN`) — kill a worker mid-task; the task is released and re-run with no human intervention. Today the orphan TTL can never fire and there is no manual release either.

---

## 4. Test families

Each family names the defect class it exists to catch. Full per-gate detail lives in the individual specs; this is the map.

| Family | Asserts | Population it catches |
|---|---|---|
| **Reachability** | Every `.rs` under `src/` is reachable from the crate root; every `invoke`/`listen` name resolves both directions | `graph_tools` + `graph_prompts` (918 LOC never compiled), `db/checkpoints.rs` (undo missing from the binary), 2 missing commands, 47 dead commands, 10 dead listeners, 3 dead events |
| **Provider conformance** | Per provider × model: streaming, tools, parallel tools, structured output, caching, usage, truncation, error classification | The `G2b` capability matrix in full |
| **Cache efficacy** | Non-zero `cache_read` from turn 2; byte-stable prefix; a sub-minimum prefix warns | Silent cache misses — the failure mode with no symptom but the invoice |
| **Reader-exists** (INV-8) | For every persisted artifact, a test reads it back **through the intended consumer** | `record_api_failure` (no writer), the graph (no reader), `ModelConfigsDb` (no reader), budget config (no reader), `plan:changed` (16 emitters, no listener) |
| **Guard-the-guard** | Every safety gate is proven to fire, **constructed the way production constructs it** | `requires_permission()` never called; the recursion guard inert; the build-evidence gate skipped on the native path; the Fix-8c sentinel unreachable by construction |
| **Platform triple** | The end-to-end smoke on all three OSes | BUG-COMPILE and the `P-01…P-35` portability class |
| **Destructive safety** | Every destructive path confirms; reversible where claimed | One-click `git_discard`; `delete_file` running `remove_dir_all`; `git_discard` with an empty file list reverting the whole worktree |
| **Anchor conformance** | Radii, fonts, tokens, class vocabulary, light-only, banned motion | 7px appearing zero times against 12+ radii; IBM Plex instead of Urbanist; 1 of 34 anchor classes; two conflicting dark palettes |
| **Migration** | Every ladder applies from empty and from head; malformed statements fail loudly; a backup precedes destruction | The `ADD COLUMN IF NOT EXISTS` v5 syntax error |
| **No-sidecar** | The full suite with the sidecar absent | **The P8 entry gate** |

---

## 5. The frontend gap

There is **no frontend test framework at all** — no vitest, jest, playwright, no `test` script (`G0 §2`). Every frontend defect in the register was found by reading, not by testing.

Minimum viable:
- **Unit** (vitest) — stores, the tool-output parser, the context meter, formatting
- **Component** — the surfaces with traced defects: Chat, Settings, Graph
- **End-to-end** (a WebDriver/Playwright harness against the built Tauri app) — the acceptance test of §2

**And real linting.** ESLint is not installed; `npm run lint` runs `tsc --noEmit`, which CI then runs a second time under the name "ESLint".

---

## 6. What blocks

**INV-4: a gate that cannot fail is not a gate.** No `continue-on-error` on any correctness check.

| Job | Today | Target |
|---|---|---|
| `cargo clippy` (both crates) | blocking | blocking |
| `cargo test` (both crates) | **`continue-on-error`** | blocking |
| `cargo fmt --check` | **`continue-on-error`** | blocking |
| `tsc --noEmit` | blocking | blocking |
| ESLint | **does not exist** | blocking |
| Frontend unit tests | **do not exist** | blocking |
| Build smoke — Windows | push-to-main only | **on PR** |
| Build smoke — Linux | push-to-main only | **on PR** |
| Build smoke — macOS | **does not exist** | **on PR** |
| End-to-end smoke ×3 | **does not exist** | on PR, or nightly if runtime forces it |

**Sequencing** (`00 §Part 6`): making the *existing* gates blocking is P0 — it is nearly free and it stops the register growing while the restructure runs. The frameworks and the end-to-end smoke follow in P9, except macOS build coverage, which moves to **P0** because R-4 identifies "macOS verified last" as a release-blocking risk.

---

## 7. The standing discipline

Three rules, each paid for by a defect in this register:

1. **A load-bearing claim needs a check that can fail.** "Should work", "free", "slightly slower" are measurements, not sentences.
2. **Prove the reader reads it.** A component can write rows, emit events and report healthy while nothing downstream consumes what it wrote.
3. **Presence of a capability is not intent to use it.** Never gate side-effectful behaviour on a credential or config merely existing — the reasoning behind INV-12 and the dev-`.env` decision in ADR-03-05.

---

> No ADR decision is recorded in this spec. It hands **ADR-04-03** to `00 §Part 8`.
