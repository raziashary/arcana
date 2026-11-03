# P2 post-phase audit — triage and disposition

**Date:** 2026-09-02 · **Auditors:** two independent agents, neither shown the
implementer's reasoning · **Reports:** [`A7-P2-work-audit.md`](A7-P2-work-audit.md)
(claim-to-code, 18 findings) and [`A8-P2-area-audit.md`](A8-P2-area-audit.md)
(adversarial defect hunt, 14 findings).

Every finding below was **independently verified against the code before being
accepted**. Two were downgraded on inspection; the rest held.

---

## The one-line verdict

The audit was worth more than the phase's own testing. P2 wrote 27 fixture
tests and 12 live tests, all green — and the auditors found **a live 2× cost
bug, a credential leak into the process environment, a critical spend hole, and
five features that were built, tested, documented and never reached**.

The single most useful thing either auditor did was refuse to accept that a
green test meant a working feature.

---

## Fixed (16)

| # | Severity | What it was | Where it is fixed |
|---|---|---|---|
| **D-9** | **CRITICAL (found as MEDIUM)** | **Every Anthropic *streaming* call reported 2× its real cost.** Anthropic's per-message usage is cumulative and repeats the input side on both `message_start` and `message_delta`; the engine summed them. Settled by capturing the raw wire, because two comments in this repo asserted opposite things. `complete()` was unaffected — which is exactly why the cost-reconciliation smoke passed throughout. | `streaming.rs`, + regression tests using the captured bytes verbatim |
| **D-1** | CRITICAL | A failed or budget-halted run recorded **$0.00** against the daily ceiling. `add_cost` sat after the turn loop, reachable only via `break`; three `return`s jumped it — including the budget path, which by construction ends the most expensive runs. | `record_spend()`, called on every exit · `TG-P2-10` |
| **F-8** | HIGH (security) | The **shipped binary loaded the dev `.env`**, putting three provider keys and AWS credentials in the process environment — inherited by every child, including the model-driven `bash` tool. `echo $ANTHROPIC_API_KEY` needed no vulnerability. | `security/env_scrub.rs`, allowlist-based · stripped again in `bash.rs` |
| **D-12 / F-12** | HIGH | The P2-14 stable/volatile prompt split was **built, documented, unit-tested and never used** — the engine called the joining wrapper. Memory sat inside the cached region, so every learned fact invalidated the prefix and each turn paid the cache-write surcharge for a prefix never read back. | `engine.rs` keeps the halves apart · `TG-P2-11` |
| **D-10 / F-3** | HIGH | Google key validation called a **retired model** in a **billed completion**, while the UI told the user validation "costs nothing". Since P2-23 refuses to store a rejected key, **a valid Google key could not be saved** — on the primary provider. | models-list endpoint · `TG-P2-12` · live `s2_13` |
| **F-7** | HIGH | **OpenAI parallel tool calls were silently dropped** — the parser returned one chunk per frame. The same defect P2-35 fixed on Google, left live here. | `parse_openai_chunk` returns every chunk |
| **D-2** | HIGH | A key rotation could leave the old (possibly leaked) key alive in the other store. | a keychain write clears the fallback copy |
| **D-3** | HIGH | A crash mid-save **destroyed every stored credential** — the vault is one encrypted blob and `save` truncated in place. | atomic temp-and-rename |
| **D-7 / F-11** | HIGH | An unpriced model recorded **$0.00**, which under a dollar-denominated ceiling means uncapped. | logs `UNMETERED SPEND` by name |
| **F-15** | HIGH | The daily ceiling shipped with **no way to switch it on** — `setDailyBudget` had no caller, so P2's own success criterion was not executable. | `DailyCapCard` in Settings |
| **F-14** | MEDIUM | P2-33's orphan list and P2-71's backend disclosure **both stopped at the IPC boundary** — no UI called either, so "tell the user which store is in use" was invisible. | `VaultBackendCard` in Settings |
| **D-6** | MEDIUM | The credential conduit found in P2's *pre-build* audit was closed on one adapter of three; the other two returned the raw `reqwest` error (whose Display includes the URL) across the IPC boundary. P2-23 made it load-bearing. | fixed text · `TG-P2-13` |
| **D-4** | MEDIUM | A missing key file silently **bricked the vault** by generating a fresh key over an existing one. | refused, with the remedy stated |
| **D-8** | MEDIUM | A network error raised a **false "caching is broken" alarm** — the monitor was consulted before the stream result was checked. | gated on `stream_result.is_ok()` |
| **F-13** | MEDIUM | P2-64's single-use-prefix alarm asked a **cross-run** question of a **per-run** monitor, so `distinct` was structurally 1 and the condition unsatisfiable. | process-wide monitor |
| **F-6** | MEDIUM | P2-52's zero-usage alarm read `turn > 0`, but `turn` counts tool-executing iterations — it is 0 for a single no-tool answer, the commonest turn there is. | asks about content |
| **F-1** | MEDIUM | `models.ts` `DEFAULT_MODEL` was a retired model — the initial model for every new worker. Invisible to `TG-P2-09`, which scans Rust only. | `TG-P2-05` |
| **F-2** | — | **`TG-P2-05` was cited twice in `catalogue.rs` as failing the build, and did not exist.** The two lists had already diverged (F-1). | now written |
| **F-4** | MEDIUM | Two calls to the same tool collided on the **unary** Gemini path — P2-36 fixed only the streaming one. Results are correlated by id, so one result answered both calls. | part index in the id |
| **F-16** | MEDIUM | `max_output` was added in P2-03 explicitly to clamp `max_tokens`, and nothing read it. | clamped |

Six new gates, each falsified: `TG-P2-05`, `TG-P2-10`, `TG-P2-11`, `TG-P2-12`,
`TG-P2-13`, plus the `env_scrub` suite.

---

## Open, with reasons (6)

Recorded rather than quietly closed. None is a correctness defect in shipped
behaviour; each is a real gap in coverage or completeness.

| # | What | Why it is open |
|---|---|---|
| **F-5** | `ErrorClass` is computed and flattened into a string; no adapter returns it as a type. | Half-closed: the live callability test now classifies rather than pattern-matches, giving it a first typed consumer, and the classifier tagged a real 529 correctly and unprompted. Making it structural means a new `AutoDevError` variant — a change to a type every module uses, which belongs with **P3**'s retry work, where a typed consumer actually exists. |
| **F-10** | `airsctl` does not share the provider factory, so `S2-07` passes for a different reason than recorded. | The *claim* holds — `TG-P2-07` proves no shipped code reads a key from argv or the environment, `airsctl` included. What is inaccurate is the stated mechanism. Recorded here; the factory unification is P3 cleanup. |
| **F-17** | `P2-30` fixed the sleep and left the "aggregates only yesterday" half of its own defect statement. | Genuinely incomplete. It is an analytics rollup, not a spend or correctness path. Carried to P3 with the defect statement intact. |
| **D-13** | `CachePolicy::Explicit::breakpoints` is constructed and never read. | Only one breakpoint is ever placed, so the field is unused rather than wrong. Reading it means multi-breakpoint support, which needs a prefix with several regions of differing lifetime — a P3 shape, not a P2 one. |
| **D-14** | `analyze_diff_tool` pairs a defaulted provider with an independently-read model with no consistency check. | Now much narrower after `P2-31`: both come from the same persisted selection, and the model falls back to one the provider actually serves. A mismatch requires hand-editing app settings. |
| **TG2-01/03/06/08** | Four gate rows in `TESTING_GATES.md` are AMBER, not green. | Already recorded there with the specific gap named — capability coverage is not exhaustive per provider × model; tool arguments are not property-tested over generated trees; the prefix is not asserted across processes; and nothing yet persists a turn to disk and replays it. |

---

## What this cost, and what it bought

The audit added roughly a third again to P2's elapsed time and found **four
defects that would each have reached users**: a doubled bill on the primary
agent path, credentials readable by any model-chosen shell command, a spend
ceiling that recorded nothing on the runs that spent most, and a primary
provider whose keys could not be saved.

None of them failed to compile. None of them failed a test. Three of them were
*inside code written to prevent exactly that class of failure*.

The recurring shape — **built, tested, documented, never reached** — accounts
for five of the sixteen. It is now `L-18` in the learnings register, and
`L-21` below records why writing the lesson down did not prevent committing it
again in the same phase.
