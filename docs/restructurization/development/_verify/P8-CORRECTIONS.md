# P8 · Pre-build corrections — what the refutation changed before a line was written

**Required by `§0.2m`.** Four adversarial verifiers were briefed to *refute*
`P8`'s load-bearing claims, over disjoint task groups, each defaulting to
REFUTED on ambiguous evidence. All four were killed mid-flight by a session
rate limit — and **all four had their findings on disk**, because `§0.2i`'s
incremental-write clause was in every dispatch. That clause exists because the
identical failure destroyed both `P0` auditors with nothing recoverable
(`L-C8`, `L-10`); this is the first time it has been paid back.

Every load-bearing result below was **re-verified in the main session** before
it changed a row, per `§0.2m`.

| Report | Lines salvaged | Groups |
|---|---|---|
| `P8-R1-live-defects.md` | 473 | `P8-25`…`P8-28` |
| `P8-R2-excision.md` | 185 | footprint, `P8-09` |
| `P8-R3-conversion.md` | 478 | `P8-45`…`P8-55` |
| `P8-R4-prompt-and-seams.md` | 698 | `P8-34`…`P8-44` |

---

## 🔴 1. `P8-09` orders the deletion of update signature verification

**The single most dangerous finding, and it was inside a task's explicit file list.**

`P8-09` says: *"Remove sidecar branches from … `updater/{check,download}.rs` …"*.

Neither file has anything to do with Python. Re-verified in the main session:

```
src-tauri/src/updater/download.rs:158  /// Download the signature sidecar file (.sig URL → bytes).
src-tauri/src/updater/download.rs:165      let client = sidecar_client()?;
src-tauri/src/updater/download.rs:211  fn sidecar_client() -> Result<reqwest::Client> {
                                           .https_only(true)
```

`sidecar_client()` is an HTTPS-only, short-timeout client that fetches the
`.sig` and `.sha256` **sidecar assets** gating every update. `download_signature`
depends on it, and its own docstring says *"`verify_update` must never be handed
a silently-degraded result."* A grep of both files for `python|orchestrator|
memory-mcp|8000|4000` returns **nothing**.

**Executing `P8-09` as written removes update signature verification.**

⚠ This is `L-C6` — *a check written from the CLAIM rather than the INVARIANT* —
recurring **after** the programme caught it once, in `P8-21`'s `grep -i sidecar`
exit assertion (`ADR-LOG-RESTRUCT-0036`). It is worse here: a task's file list
reads as already-triaged, so the second reader does not re-derive it.

**Correction:** both files struck from `P8-09` and added to the protected list
in `P8-21`/`P8-65`.

### The word "sidecar" has FOUR meanings in this tree

The plan protects one alternate meaning. There are three.

| Meaning | Where | Status |
|---|---|---|
| Python subsystem | `src-tauri/src/sidecar/`, `resources/sidecars/` | **the deletion target** |
| AutoDev as a sidecar *of AutoDesign* | `src/design/`, `tsconfig.sidecar.json` | protected (`ADR-0036`) |
| **Update `.sig`/`.sha256` sidecar assets** | `updater/{check,download}.rs` | 🔴 **unprotected, and named for deletion** |
| **SQLite `-wal` sidecar** | `db/backup.rs:13,331,354` | 🔴 **unprotected**; `:354` is a test's failure message |

### `P8-09`'s remaining files, re-measured

| File | What is actually there |
|---|---|
| `startup_commands.rs` | real branch |
| `worker_commands.rs` | real branch |
| `runtime_config.rs` | real field |
| `budget_commands.rs` | **one comment** (`:57`) |
| `mcp/mod.rs` | **one comment** (`:7`) |
| `logging.rs` | **one comment** (`:24`) — the rationale for `RedactingWriter`, which stays |

**`P8-09` verdict: OVERSTATED and partly WRONG.** Two files must not be touched;
three carry a comment, not a branch.

---

## 🔴 2. `P8-26`'s mechanism, chain, and remedy were all wrong — razi-ruled

**The defect is real. Everything the task said about it was not.**

**Stated mechanism:** *"`cmd` returns the last element's status, `build_evidence::record`
stores a successful build as failed."*
**Measured, on this host, with a passing control:**

| probe | exit | side effect |
|---|---|---|
| `shim.cmd` alone (control) | **0** | runs |
| `shim.cmd 2>&1 \| more` (control) | **0** | runs |
| `shim.cmd 2>&1 \| Select-Object -Last 50` | **255** | **does not run** |
| `shim.cmd 2>&1 \| tail -50` | **0** | runs — `tail` is at `/usr/bin/tail` |

When the right half of a `cmd` pipeline cannot be resolved, **cmd does not
execute the left half either**. So the build does not run and have its status
lost — **it never runs at all**.

And the reverse of what the task assumed: `| tail -50` **works** on any machine
with Git's `usr/bin` on PATH, while `| Select-Object` **can never** work under
`cmd`. **`rewrite_for_windows` converts a working command into a broken one.**

**Stated chain to the `qc_review` veto: refuted.** `build_evidence.rs:54-60`'s
`exit_code_is_attributable` returns false for any command containing `|`, and
the rewrite preserves the `|`. So every rewritten command is recorded
`exit_code_attributable: false`, and `update_task_status_tool.rs:158-190`
matches the attributability arm **first** — the agent is told *"re-run as a
plain command"*, never *"the build failed"*. The rewriter is structurally
incapable of driving the veto the task blames it for.

**Stated remedy: a category error.** `platform::command::resolve(toolchain,
intent)` returns fixed argv — `(Npm, Build) → ["run","build"]` — and cannot
express an arbitrary model-authored string. Confirmed zero callers outside its
own module.

> **▶ RAZI RULED, 2026-09-03:** *delete `rewrite_for_windows`.*

This also opens a closed loop: `bash.rs:29` tells the model to avoid Unix
utilities on Windows, and `bash.rs:211-223` suggests `Select-Object -First N`
on exit 255 — **the exact syntax the rewriter had just substituted and that
`cmd` cannot run.** Deleting the rewriter makes both guides honest.

---

## 3. Corrections to the remaining tasks

| Task | Verdict | Correction |
|---|---|---|
| `P8-25` | PARTIALLY CONFIRMED — **worse than stated** | The `Mode2Only` guard is *unconditional*, not conditional. The final clause (*"editing a shipped recipe strands it too"*) is OVERSTATED. |
| `P8-27` | **CONFIRMED in every particular** | none |
| `P8-28` | **CONFIRMED** | the exploit is simpler than the one named |
| `P8-45` | mechanism exact, **trigger point wrong** | re-point before writing the replacement |
| `P8-46` | **CONFIRMED** — every cited line still holds | none |
| `P8-47` | CONFIRMED on location, **INCOMPLETE on inventory** | re-derive the full inventory before deleting |
| `P8-48` | structure exact, **the named endpoint is wrong** | correct the endpoint |
| `P8-49` | **CONFIRMED**, and supersedes `P8-04` for a stronger reason than stated | none |
| `P8-50` | **CONFIRMED** — zero callers, exactly two comments | none |
| `P8-51` | two of three as stated; **the third is misclassified and the destination file does not exist** | re-triage the third; create the destination |
| `P8-54` | `SystemHealth.tsx` exact; **the rest takes live UI down with it** | narrow the prescription |
| `P8-55` | 🔴 **REFUTED on the split line** | `startup:progress` is the **DEAD** half, not the live one — it sets sidecar boot state from orchestrator phases (`sidecarStore.ts:148-157`) and its emitter is deleted by `P8-52`. Following the task literally moves a dead listener *into* the store meant to hold the live feature. The live half is six `mcp:*` listeners the task never names. |
| `P8-36` | **cache warning misaimed** | prepending the core is **byte-safe**; the warning points at the wrong risk |
| `P8-38` | **CONFIRMED**, and the cleanup check is *worse than vacuous* | none |
| `P8-41` | 🔴 **REFUTED as worded** | the equivalence is trivially true under one reading and false by design under the other. Replaced by three falsifiable properties: framing survives the flattener at the **seam's** output; tool arguments are non-contaminated; retrieval happens once per turn. |

### Footprint

LOC figures have **drifted +3** since 2026-09-01. The *"69 whole-tree / 63
outside"* pair is **arithmetically impossible** and no scoping reproduces it —
struck rather than re-stated, and `P8-21`/`P8-65` are the gates that will
produce the real number by executing.

---

## What this cost, and what it bought

Four dispatches, all killed by a rate limit, all recovered from disk. Between
them they found **one security regression the plan actively ordered**, **one
remedy that could not be implemented**, **two gates that would have proven
nothing**, and **one refactor whose split line was backwards** — every one of
them before the first line of code.

⚠ And the one that should be read twice: `P8-09`'s updater clause is the same
defect class the programme had *already caught and written an ADR about* two
days earlier. Catching it once did not prevent it. That is `L-C8` — *a lesson
already recorded was not applied to the next opportunity* — and it is the
argument for `§0.2m` being a rule rather than a habit.
