# P8-R2 — Adversarial verification: measured footprint & excision mechanics

**Verifier:** adversarial pre-build reviewer (no exposure to the plan author's reasoning)
**Repository:** `C:\AIRS Foundry\autodev\autodev`
**Date of re-measurement:** 2026-09-03 (plan's numbers were taken 2026-09-01)
**Posture:** REFUTE by default. Ambiguity resolves against the claim.

> Status: IN PROGRESS — this file is written incrementally as evidence is gathered.

---

## 0. Method

All measurements re-run today with the exact commands recorded beside each result.
Read-only throughout: no `git` write commands, no deletion, no builds executed against
the resources under test.

---
## 1. The measured footprint table — re-measured 2026-09-03

**VERDICT: PARTIALLY CONFIRMED — the Rust LOC numbers have drifted and are now stale by +3.**

Commands and output (Git Bash, repo root, tree clean at `37d4dd3`):

```
$ wc -l src-tauri/src/sidecar/*.rs src-tauri/src/commands/sidecar_commands.rs
  1605 src-tauri/src/sidecar/client.rs
  1271 src-tauri/src/sidecar/mod.rs
   201 src-tauri/src/sidecar/sse.rs
   223 src-tauri/src/sidecar/startup_progress.rs
   570 src-tauri/src/commands/sidecar_commands.rs
```

Every file ends in `0a`, so `wc -l` is the true line count (checked with `tail -c 1 | xxd -p`).

| Subject | Plan claims (2026-09-01) | Measured 2026-09-03 | Verdict |
|---|---|---|---|
| `client.rs` | 1,604 | **1,605** | drifted +1 |
| `mod.rs` | 1,270 | **1,271** | drifted +1 |
| `sse.rs` | 201 | 201 | ok |
| `startup_progress.rs` | 222 | **223** | drifted +1 |
| `sidecar_commands.rs` | 570 | 570 | ok |
| `#[tauri::command]` in `sidecar_commands.rs` | 17 | **17** | ok |
| Sub-total `sidecar/` (`P8-05` says "3,297 LOC") | 3,297 | **3,300** | drifted +3 |
| **Total Rust** | **3,867** | **3,870** | drifted +3 |
| `src-tauri/resources/sidecars/` | 225 MB | **225 MiB** (`du -sb` = 226,500,953 B) | ok |
| `.gitmodules` stanzas | 2 | **2** | ok |

**The cause is identified, and it is not a rounding difference.** The numbers were correct
at `eab345c` (2026-09-01 13:00 +0700, *"refactor(core): extract autodev-core…"*):

```
$ for f in ...; do git show eab345c:$f | wc -l; done
client.rs = 1604   mod.rs = 1270   sse.rs = 201   startup_progress.rs = 222
```

Two commits on 2026-09-02 changed them after the measurement was taken:

```
$ git diff --stat eab345c HEAD -- src-tauri/src/sidecar/ src-tauri/src/commands/sidecar_commands.rs
 src-tauri/src/sidecar/client.rs           | 27 +++++++-------
 src-tauri/src/sidecar/mod.rs              | 61 ++++++++++++++++---------------
 src-tauri/src/sidecar/startup_progress.rs |  5 ++-
 3 files changed, 48 insertions(+), 45 deletions(-)
```
`4106d7a` *"fix(P3-A): P3-01/P3-04 — a profile Cargo was discarding, and twelve panicking slices"*
and `e2997fe` *"fix(P3-A): P3-02/P3-03 — one panic no longer poisons the application"*.

**Why this matters beyond three lines:** P3-A hardened code that P8 is about to delete.
The drift is evidence that *fix work is still landing inside the deletion set* — every
hour spent on `sidecar/*.rs` between now and P8-05 is thrown away. That is a sequencing
observation the plan does not make anywhere I can find.

**Could not determine:** nothing here. The measurement is exact and reproducible.

---
## 2. "Files referencing sidecar: 69 whole-tree / 63 outside / 41 code / 22 comment-only / 10 in `src/design/`"

**VERDICT: REFUTED — the pair 69/63 is arithmetically impossible, and no scoping I can
construct reproduces either number. The 10 `src/design/` files and the "15 filenames"
sub-claim are CONFIRMED.**

### 2a. 69 − 63 = 6, but the excluded directory holds 4 files

The plan states 69 whole-tree "of which **63** are outside `src-tauri/src/sidecar/` itself".
`src-tauri/src/sidecar/` contains exactly four files, and all four contain the word. So the
difference between the two figures must be 4. It is stated as 6. The two numbers cannot
both be measurements of the same tree under the stated exclusion.

### 2b. No scoping reproduces 69 or 63

```
$ git grep -il sidecar -- .                                          → 194
$ git grep -il sidecar -- . ':!src-tauri/src/sidecar/'               → 190
$ git grep -il sidecar -- . ':!src-tauri/src/sidecar/' ':!docs/'     →  83
$ git grep -il sidecar -- autodev-core/src src-tauri/src src-cli/src src-shared/src src
                                                                     →  70
$ ... same, ':!src-tauri/src/sidecar/'                                →  66
```
The closest reproduction (source trees only, no `docs/`, no root manifests) is **70 / 66**,
and it has the *correct* internal difference of 4. Splitting those 66 by whether any
non-comment line matches (block- and line-comment aware, `//`, `#`, `/* */`, `*`):

```
TOTAL 66   CODE 42   COMMENT-ONLY 24
```
against the claimed 63 / 41 / 22. Every figure in the row is 2–3 too low, in the same
direction, which is consistent with the row having been measured before 2026-09-03's
commits and not re-run — the same staleness as §1.

**What I could not determine:** the author's exact scope (which top-level directories,
tracked vs. untracked, and the comment-stripping rule). The row should state its command,
as `P8-21`'s revision note does. Until it does, the row is not checkable, and an
unverifiable measurement in a table headed *"Measured footprint"* is the failure mode the
plan elsewhere names `L-C6`.

### 2c. CONFIRMED sub-claims

- `git ls-files | grep -i sidecar` returns **15**, exactly as `P8-21`'s revision note says.
- **10** files under `src/design/` contain the word, exactly as claimed:
  `contract/{contract.test.ts,gates.test.ts,index.test.ts,index.ts,main.ts,reference.ts,sidecar.ts,types.ts}`, `ir.test.ts`, `ir.ts`.
  These are the AutoDesign seam and must survive.

---

## 3. 🔴 The word "sidecar" has FOUR meanings in this tree, not two — and `P8-09` orders the deletion of one of them

The plan protects one alternate meaning (`src/design/` — AutoDev as a sidecar *of*
AutoDesign). There are two more, both live, both unprotected, and one of them is inside a
task's explicit file list.

### 3a. 🔴 Updater `.sig` / `.sha256` sidecars — `P8-09` names both files

`P8-09`: *"Remove sidecar branches from … `updater/{check,download}.rs` …"*.
Neither file has anything to do with Python. Verbatim:

```
src-tauri/src/updater/check.rs:35:  /// `signature` and `checksum` are **URLs of sidecar assets**, not the values
src-tauri/src/updater/check.rs:44:      /// URL of the `.sha256` sidecar for `full_url`.
src-tauri/src/updater/check.rs:175:     // Second pass: attach .sig and .sha256 sidecars to the asset they
src-tauri/src/updater/check.rs:182:     // update as a mismatch. Require the sidecar's base name to be the asset
src-tauri/src/updater/download.rs:158: /// Download the signature sidecar file (.sig URL → bytes).
src-tauri/src/updater/download.rs:165:     let client = sidecar_client()?;
src-tauri/src/updater/download.rs:182: /// Download the SHA-256 checksum sidecar file.
src-tauri/src/updater/download.rs:210: /// Short-timeout HTTPS-only client for the small sidecar files.
src-tauri/src/updater/download.rs:211: fn sidecar_client() -> Result<reqwest::Client> {
```

`download.rs` contains a live function **named `sidecar_client()`** whose job is fetching
the signature and checksum that gate every update. Executing `P8-09` as written against
these two files removes update signature verification. This is precisely the class of
error the plan says it caught once (`L-C6`, the `grep -i sidecar` exit assertion) — it
recurred inside a task's file list, where it is more dangerous, because a task list reads
as already-triaged.

**These two files should be struck from `P8-09` and named in `P8-21`/`P8-65` as protected,
alongside `src/design/`.**

### 3b. A fourth meaning: the SQLite `-wal` sidecar

```
autodev-core/src/db/backup.rs:13:  //   A SQLite database in WAL mode keeps recent commits in a `-wal` sidecar
autodev-core/src/db/backup.rs:331: /// In WAL mode, commits live in a `-wal` sidecar until a checkpoint folds
autodev-core/src/db/backup.rs:354:     "expected a -wal sidecar to exist for this test to mean anything"
```
Line 354 is a **test assertion message**, so a content sweep here edits a test's failure
text. No task names `backup.rs`; nothing protects it either.

### 3c. `P8-09`'s remaining files: three of eight have no "branch" to remove

| File in `P8-09` | What is actually there |
|---|---|
| `startup_commands.rs` | real — `use crate::sidecar::startup_progress::…` (`:15`), `set_skip_sidecars` (`:47`) |
| `worker_commands.rs` | real — `OrchestratorClient` state, live HTTP path (`:50-202`) |
| `runtime_config.rs` | real — the `skip_sidecars` field (`:44`) |
| `budget_commands.rs` | **one comment**, `:57` *"Stored in `~/.autodev/runtime_config.json` alongside skip_sidecars"* |
| `mcp/mod.rs` | **one comment**, `:7` — history of a deleted MCP server |
| `logging.rs` | **one comment**, `:24` — the rationale for `RedactingWriter`, which stays. `P8-60` class, not a branch |
| `updater/check.rs` | **wrong subject** (§3a) |
| `updater/download.rs` | **wrong subject** (§3a) |

**VERDICT on `P8-09`: OVERSTATED and partly WRONG.** Two of its eight files must not be
touched; three contain a comment, not a branch.

---
