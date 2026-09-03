# P8 · Pre-build baseline — measured 2026-09-03, before the first line of the phase

**Why this file exists.** `TG8-01` asserts *"the **full suite** passes with the
sidecar absent"*. That is a comparison, and a comparison needs a **before**.
Without this record, any test that fails after the deletion is ambiguous — was
it the deletion, or was it already red? A deletion phase that cannot answer
that question attributes every pre-existing defect to itself, which is the
specific harm `P8-0`'s preamble was written to avoid.

Taken on the commit that closed `ADR-LOG-RESTRUCT-0037` (`37d4dd3`), with the
working tree clean and **no untracked files** (`§0.2L-5`).

## The numbers

| Check | Command | Result |
|---|---|---|
| Rust compile | `cargo check --workspace --all-targets` | **exit 0** — warnings only (17 in `autodev` lib) |
| Rust tests | `cargo test --workspace` | **exit 0** — **611 passed**, 0 failed, 0 ignored, across **17 targets** |
| TypeScript | `npx tsc --noEmit -p tsconfig.json` | **exit 0** |
| Frontend tests | `npm test` (vitest) | **exit 0** — **118 passed**, 7 files |

⚠ **The Rust suite was run with `GOOGLE_API_KEY`, `ANTHROPIC_API_KEY` and
`OPENAI_API_KEY` set EMPTY**, per `§0.8` (*"suites run without live credentials
by default"*). `live_provider_smoke.rs` gates every case behind `require_key!`,
so it skips rather than billing. The 611 figure is therefore the **offline**
suite; the live smoke is run deliberately at closeout (`L-15`).

## `§0.8` discharged before anything was run

The destructive set was identified **first**, not discovered afterwards:

| Candidate | Verdict |
|---|---|
| `autodev-core/tests/concurrent_writers.rs` — four `remove_dir_all` calls | **Safe.** `sandbox()` builds every path under `std::env::temp_dir()` (`:41-43`). |
| `autodev-core/tests/redaction_boundaries.rs` — two `remove_dir_all` calls | **Safe.** Same shape (`:27-29`); the `.autodev` it creates is *inside* the temp dir. |
| `live_provider_smoke.rs` | **Key-gated**, skips without credentials. |
| Migration repair of `graph/` v5 (`§0.8`'s standing warning) | **Not reached** — no test in the integration dirs performs a downgrade, drop or truncate. |

No test in `autodev-core/tests`, `src-cli/tests` or `src-tauri/tests` touches
the user's real `~/.autodev/`. Two source files say so explicitly and were
checked rather than believed (`db/backup.rs:88`, `db/workers.rs:154`).

## Entry gate `P8-A`, verified in the main session

| Task | Condition | Verdict |
|---|---|---|
| `P8-01` / `TG8-02` | every capability with a `PORT` verdict has a **named, tested** destination in the Rust tree | ✅ **satisfiable.** Population taken from `G4b`'s own *PORT / DELETE SUMMARY*: 4 `PORT` + 6 `PARTIAL` = the **ten** `TG8-02` names, with `security/` (redaction) as the tenth. All ten resolve. |
| `P8-02` | `skip_sidecars` exists and is honoured | ✅ `RuntimeConfig.skip_sidecars`, written by `set_skip_sidecars` (`startup_commands.rs:47`), branched on at `main.rs:352`. |
| `P8-03` | native memory serves every read `memory-mcp-server` served | ✅ recall is native (`autodev-core/src/memory/recall.rs`, `get_memory.rs`, `list_memories.rs`, `search_memory.rs`). Every `:8000` reference in the tree is **inside `src-tauri/src/sidecar/`** — the directory being deleted — plus one log line at `main.rs:426`. `MEMORY_URL` is declared at `sidecar/client.rs:107` and dies with it. |

## 🔴 The instrument was wrong twice before it was right

Recorded here rather than in the closeout, because the next person to write a
scanning check in this repository should read it first.

**False RED #1 — all ten capabilities reported MISSING.** The check shelled out
to `rg`, which is **not installed on this machine**. Every call raised, the
handler stringified the exception, and the next line *filtered those strings
out* — so *"I could not look"* became *"I found nothing"*, and the entry gate
reported BLOCKED against a codebase that was fine. This is `L-24`'s
`unwrap_or(whole_file)` in different clothes: **when a locator fails to locate,
the honest outcome is an error, never a default.** The rewrite walks the tree
itself and treats an unreadable file as a reported finding.

**False RED #2 — `streaming/` reported as having no test.** It has one. The
detector looked for `#[cfg(test)]` *in the same file as the match*, and this
repository puts provider tests in **sibling `*_tests.rs` files**:
`parse_sse_line` — the whole of the ported `streaming/` capability, 148 lines
with exactly one `pub fn` — is exercised **eight times** across
`anthropic_tests.rs`'s 13 tests. `L-C6`: the check was written from the symbols
and layout I guessed, not from the property it protects.

**And the summary line contradicted its own data**, printing `SATISFIABLE`
while listing `streaming/` as untested three lines above — because the verdict
read `no_dest` and ignored `no_test`, though `TG8-02`'s words are *"a named,
**tested** destination"*.

⚠ Three defects, one instrument, all three of them mine, none of them in the
code under test. **A check that has not been falsified is a hypothesis**, and
the two REDs above are what a hypothesis looks like when it is wrong in the
direction that feels like diligence.
