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
| `TG0-01` | HIGH | The updater's *accept* path is unproven and every update is currently refused. | Needs the signing keypair, which only you can generate. `docs/updater-signing.md` has the commands (now Windows-correct) and the secret names. |

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

## `F-14` — assessed and closed, no history rewrite

**Decision: leave git history alone.** Razi delegated the call; this is the
reasoning, so it can be revisited on evidence rather than re-argued.

What is actually in the file, checked rather than assumed: `dump.rdb` is a Redis
snapshot holding **agent procedural memory** — `procedural:supervisor` keys
containing reasoning traces from a development run. A credential scan over the
extracted blob (`sk-*`, `ghp_*`, `AIza*`, `xai-*`, `api_key`, `secret`,
`password`, `BEGIN … PRIVATE`) returns **zero hits**; the one `BEGIN` match is
the word "begin" in prose. It is 175 KB and contains nothing that is not
already in the repository.

The rest of history is clean too, which is the check that would have forced a
purge: `.env` has **never** been added in any commit on any branch, and no
`.pem`, `.key`, `.p12`, `.pfx`, `id_rsa` or `credentials` file has ever been
added. Razi's statement that `.env` was never committed is verified, not taken
on trust.

Against that, a purge costs: rewriting every commit SHA from the first
`dump.rdb` commit forward (5 of 202 commits touch it), a force-push over
already-published `main`, and `sidecar-backup` — the backup branch — losing
shared ancestry with the branch it backs up. That is real disruption to buy the
removal of 175 KB of non-secret text.

So the file stays in history, and the claim is narrowed to match: `TG0-04`
states that it checks the **index**, and `.gitignore` plus the gate make it
impossible to reintroduce. If the repository is ever made public and those
reasoning traces matter for a different reason, the purge is a
`git filter-repo --path src-tauri/dump.rdb --invert-paths` away — it just is not
justified by anything found here.

*(Hardened in the same pass: `*.pem`, `*.key`, `*.p12` and `*.pfx` are now
gitignored and covered by `TG0-04`, because the signing runbook has the owner
generating a private key in the working directory.)*

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
