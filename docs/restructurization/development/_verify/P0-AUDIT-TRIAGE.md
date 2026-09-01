# P0 post-phase audit — triage

The two mandatory §0.2a auditors returned **27 code findings** (`A1-P0-code-audit.md`)
and **41 stale-documentation rows** (`A2-P0-doc-staleness.md`).

§0.2b says a phase cannot close over an open finding. This is the disposition of
every one of them: fixed, assigned to a phase, or needing Razi. Nothing is
dropped, and nothing is closed by assertion.

**Fixed in P0 (14 of 27):** `F-02` `F-03` `F-04` `F-05` `F-09` `F-10` `F-11`
`F-15` `F-16` `F-18` `F-21` `F-22` `F-23`(the interaction) `F-24`, plus the
user-facing and operational half of the documentation rows.

Four of those were regressions **P0 itself introduced** — `F-10`, `F-21`,
`F-22`, and `F-04`/`F-05`. That is the audit earning its cost.

---

## Needs Razi — cannot be decided or done by me

| # | Sev | What | Why it needs you |
|---|---|---|---|
| `F-14` | HIGH | `dump.rdb` is gone from the index and gitignored, but it is **still in git history**. `TG0-04` therefore tests a weaker claim than the one it states. | Removing it means rewriting history and force-pushing — Rule 10 forbids me doing either without your explicit say-so. It is 175 KB of a local Redis snapshot; the exposure is repository size and the precedent, not a credential. Options: leave it and narrow the gate's wording (done), or purge with `git filter-repo` and force-push. |
| `TG0-01` | HIGH | The updater's *accept* path is unproven and every update is currently refused. | Needs the signing keypair, which only you can generate. `docs/updater-signing.md` has the two commands and the secret names. |

---

## Assigned to a later phase, with the reason

| # | Sev | Finding | Owner | Why not now |
|---|---|---|---|---|
| `F-01` | HIGH | `SubagentResult` reports `cost_usd`/`tokens_in`/`tokens_out` as 0 and `files_modified` empty; a live consumer in `spawn_qc_review` silently no-ops on them. | **P3** | Real per-subagent accounting needs the usage plumbing P3 builds (`Usage` six-field, provider-reported). Inventing numbers now would be worse than zeros, but the consumer should say "not measured" rather than silently doing nothing — that half is cheap and moves with P3. |
| `F-07` | MED | `terminate()`/`abort()` cannot stop a subagent that is already being awaited. | **P4** | The fix is a cancellation token threaded through the engine turn loop; P4 owns fleet lifecycle and already needs one for worker cancellation. |
| `F-08` | LOW | A registry-insert race can drop the first lifecycle events of a subagent. | **P4** | Same area, same change. |
| `F-12` | LOW-MED | `Recipe.agent_role` is now write-only — nothing reads it since identity moved to `AgentIdentity`. | **P4** | It should become the source of a worker's role rather than a parallel field. Removing or wiring it is fleet work. |
| `F-17` | MED | 46 registered Tauri commands have no frontend consumer, two of them shipped by P0. | **P6** | The UI phase is where a command either gets a consumer or gets deleted. Recorded so it is a work item and not a discovery. |
| `F-19` | MED | The vault INV-12 gate is enforced for one file plus two names, not repo-wide. | **P2** | A repo-wide "no command returns key material" rule needs the provider layer's types to be stable; the broad version was tried in P0 and produced false positives on two correct commands. |
| `F-20` | MED | The reachability gate matches module names globally, so two files with the same stem could hide an orphan. | **P1** | P1 extracts `autodev-core` and moves whole module trees; the gate should become a real mod-graph walk in the same change rather than being patched twice. |
| `F-23` | HIGH | `platform/command.rs` has no callers — BUG-COMPILE's root cause is fixed in isolation, not wired. | **P4** | Wiring it means changing how recipes express build steps, which is `P4-11`. **Its dangerous half is already closed**: the build-evidence gate no longer trusts a pipeline's exit status, so it can no longer certify a failed build as passing. |
| `F-26` | LOW-MED | `PoolPermit::drop`'s comment describes code that is not there, and the drop path can panic. | **P4** | Pool lifecycle; grouped with `F-07`/`F-08`. |
| `F-06` `F-25` `F-27` | LOW | Stale comment fragments and small claim/evidence gaps left by the P0 deletions. | **P1** | Swept with P1's own no-stale pass; each is a comment, none changes behaviour. |

---

## Accepted as stated, no work

| # | Finding | Disposition |
|---|---|---|
| `F-13` | Four commit messages call `cargo clippy` "clean" when it emits warnings (no `-D warnings`). | **Correct, and the wording was loose.** "Clean" meant zero *errors*, which is what the gate enforces. The warnings are pre-existing dead code in `graph/` and `analytics/`. Promoting clippy to `-D warnings` is a P1 decision, not a silent one. |

---

## The audit's own value, recorded

The single most useful finding was `F-10`, and it is worth stating why: I made
Mode 2 unreachable, verified the code *below* my change routed to Mode 1, and
never read the twenty lines *above* it — where a latch returned early from every
tick. On a machine without Python the fleet was silently dead and reporting
itself idle, while my own comment two lines below the latch claimed the
opposite. Three further findings (`F-11`, `F-21`, `F-22`) are the same shape.

That class is now `L-C4` at three instances and lesson `L-11` in `MDP §0.16`:
**verifying the consequence you intended is not verifying the change.**
