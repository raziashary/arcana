# R3 — Adversarial pre-development review: gaps, silences and unstated policy

| | |
|---|---|
| **Reviewing** | `vision/00-…-MASTER-PLAN.md` (rev 0.8) · specs `01`–`10` · `_grounding/G0`–`G12` · `ui-anchor.md` · `memory-anchor.md` |
| **Lens** | What the plan **does not say** but must. Silence forced into an explicit decision or a written acceptance. |
| **Reviewer** | Fresh-context adversarial pass. No implementation authority; no file in the suite was modified. |
| **Date** | 2026-08-31 |
| **Verdict** | The plan is unusually strong on *defect* coverage and unusually thin on *policy*. It answers 35 ADRs about how the machine is built and **none** about what the machine is allowed to do with the user's code, money, or memory. |

**Calibration note, stated once and applied throughout.** AutoDev is a single-user, local-first desktop developer tool. It is **not** a regulated multi-tenant service, and this review does not ask for consent ledgers, DSAR flows, data-processing agreements, audit trails, RBAC, or org-level policy. Several things a reviewer might reflexively demand are explicitly marked **NOT NEEDED** below, with reasons, so the owner can trust the items that *are* flagged. What a local tool does still owe its user: **say what leaves the machine, do not silently store their secrets forever, do not spend their money without a ceiling, and be reversible when it breaks their repository.** Every finding below reduces to one of those four.

---

## 1 · Executive summary — the silences that most need an owner decision

**1. The only redaction in the product is deleted by P8, and nothing notices.** `G4b §A.14` inspects the sidecar's `security/` module and rules `redact_dict()` with its 20-entry `SENSITIVE_KEYS` set **"PORT"** — *"126 lines, directly reusable — the `SENSITIVE_KEYS` list is the spec"* — and `G4b`'s own summary table repeats it: `security/` → *"Keep redaction"*. That verdict never reached the plan. The port set is the **nine** capabilities of `G1 §C.4` (P1–P9); redaction is not among them. No mechanic `M-01…M-39` owns it, no ADR decides it, no spec mentions it, and P8's entry gate is *"all nine ported capabilities pass a test with the sidecar not running."* After P8 the product has **zero** redaction anywhere — at the moment memory becomes persistent, provenance-tagged and undeletable. `G4:486` already records the failure this prevents: *"`context` carries the **unredacted** arguments — a secret passed as a tool argument lands in procedural memory verbatim."* Under the restructure that row survives forever.

**2. There is no data-egress policy, and the plan's own gate collides with autonomous operation.** Spec `06` deserves real credit: it identifies `analyze_diff` as unprompted repo exfiltration, introduces `ToolEffect::Network` as a first-class effect, and writes `G-PERM-NETWORK` — *"Any tool that sends workspace content off the machine prompts, in every mode."* But that gates one **tool**, not the **content**. Every `read_file`, `grep`, `glob` and `list_directory` result becomes prompt bytes on a third-party API, and all four are unjailed and auto-approved (`G10:1977`); `.gitignore` is never honoured despite a tool description claiming it is (`G2a T33`); `read_file("~/.ssh/id_rsa")` succeeds today (`G10 T12`). And `G-PERM-NETWORK`'s *"in every mode"* directly contradicts spec `04`'s `G-QC-ANALYZE`, which requires `analyze_diff` to actually run inside **unattended** QC. Two decided specs, opposite requirements, neither citing the other.

**3. Nothing stops spending.** ADR-02-04 deliberately drops the budget dimension of routing. The only ceiling is per-task, and spec `04 §5` records it **"FULLY INERT"**: `with_task_budget` has zero callers, no recipe calls `set_task_budget`, and the Settings default has no Rust reader. `G-BUDGET-FIRES` asserts a breaker trips — while the plan never states the default budget, who sets it, what a trip *does*, or whether any session/daily/global ceiling exists at all (`"budget cap"`, `"daily cap"`, `"kill switch"`, `"off switch"` = **0 occurrences** across the suite). `G9 F-BUD-04` names three unmetered LLM spend paths, and `G9:401-405` warns that a broadcast fires a full LLM tick per worker with *"no per-tick cap … pins the whole fleet."* This is a BYOK product: the runaway bill is the user's.

**4. Memory is permanent and the user cannot see it, export it, or delete it.** OQ-5 decides *"Memory tables: **No automatic deletion**."* Nothing decides **user-initiated** deletion, export, or inspection. The only surfaces that could show memory — `MemorySidebar` (408 LOC) and `MemoryStats` — sit inside the ~3,700 unreachable LOC whose fate spec `07 §6` leaves open (*"route it or delete it"*). `G-MEM-PROVENANCE` proves every row is auditable; nothing lets the **user** audit it. Combined with finding 1, a secret read once is stored forever, in a store with no delete button.

**5. Reversibility is assumed and does not exist.** Undo is absent from the shipped binary because `db/checkpoints.rs` is never compiled (`F-44`); R3 leaves two rival checkpoint systems at *"pick one"*; `write_file`'s `auto_checkpoint_before_write` returns `()`, so a failed checkpoint is invisible and the overwrite proceeds (`G10-11`). M-38's rule is *"undo where feasible"* with a parenthetical admitting undo does not currently exist. P1 declares the module and stops there. **No mechanic, no ADR and no gate makes an agent write reversible**, and nothing states how a user learns that a worker damaged their repository. `"auto-commit"` = 0 occurrences: the plan never even decides whether agent work is committed, staged, or left dirty.

**6. Every number in the two learning loops is inherited and unvalidated — and one is provably wrong.** Affinity uses `success_rate·0.6 + duration_score·0.4`, `confidence = n/(n+5)`, a `>= 3` sample gate, a **global** 200-row window, and pure greedy `scores.remove(0)` selection (`G9:512-537`). No exploration term exists anywhere (`"exploration"`, `"cold start"`, `"bandit"` = 0 hits). The proof that nobody validated these: the static keyword layer requires `conf > 0.85` while the maximum confidence any pattern declares is exactly `0.85` — a strict-inequality off-by-one that makes the branch permanently dead (`04 §5.1`). The plan carries the rest of the set forward untouched. The one genuinely *new* number is sourced to a word: ADR-02-02's *"only do B after **say** three consecutive failures"* (`00:760`).

**7. Two operational contracts have no owner at all.** `ErrorClass::Retryable` is defined with **no retry policy anywhere** — `"backoff"` = **0 occurrences** in the entire suite, while `G2a:680` records that a single 429 or 5xx aborts the run today. And the CI end-to-end smoke has no credential path: `G-KEY-NOENV` requires the app to refuse a turn with the keychain empty, ADR-03-05 confines the dev `.env` to a harness *not linked into the app*, and headless Linux has no working keychain at all (`G3c P-35`). The acceptance test cannot run in CI as specified.

**What this review is not saying.** The plan's engineering is excellent and its self-adversarial habit is real — spec `04 §1` catches a collision between two *already-decided* ADRs, and the ledger keeps a retracted answer visible in place. Section 6 lists eleven things I expected to be missing and found properly handled. The gap is one-dimensional: **this is a plan about correctness that has not yet been asked to be a plan about conduct.**

---

## 2 · Findings

Severity: **C** critical (ship-blocking or user-harming) · **H** high · **M** medium · **L** low / write-an-acceptance.

| ID | Sev | The silence | Why it must be decided | Consequence of leaving it | Recommendation |
|---|---|---|---|---|---|
| **GAP-01** | **C** | The plan never ports secret redaction. `G4b §A.14` rules it **PORT** (*"the `SENSITIVE_KEYS` list is the spec"*); the port set is nine capabilities and this is not one of them. | It is the only redaction the product has ever had, and P8 deletes the module containing it. | After P8 there is zero redaction, permanently, at the exact moment memory becomes persistent. `G4:486`: unredacted tool arguments already land in procedural memory verbatim. | New **M-40** (redaction port) + **ADR-06-02**. Extend P8's entry gate from nine to **ten** capabilities. |
| **GAP-02** | **C** | No data-egress policy. Nothing states what leaves the machine, when, to whom, or that the user is told. | The product's core loop ships the user's source code to a third party. Under BYOK the plan promises the *key* is theirs and is silent on the *code*. | The user cannot form a correct model of the tool. `G11:249` already drafted the missing sentence and it was not adopted. | **ADR-06-01**. One egress statement in first-run + one always-visible surface listing what was sent this session. |
| **GAP-03** | **H** | `G-PERM-NETWORK` (*"prompts, in every mode"*) vs `G-QC-ANALYZE` (`analyze_diff` must run in unattended QC). Neither spec cites the other. | Unattended workers cannot answer a prompt. | Either autonomous QC deadlocks on an unanswerable prompt, or the gate is quietly auto-approved and `G-PERM-NETWORK` becomes decoration — the exact `is_read_only` failure spec `06` exists to fix. | Decide in **ADR-06-01**: a per-workspace standing egress consent, granted once at setup, that autonomous mode may consume; never an implicit bypass. |
| **GAP-04** | **H** | No sensitive-path policy for reads, indexing, or context selection. `.gitignore` is never honoured (`G2a T33`); `~/.ssh/id_rsa` reads succeed (`G10 T12`); the 256 KB read cap is bypassable by supplying a line range (`G10-07`). | `G-JAIL-COVERAGE` stops reads *outside* the workspace. A `.env` **inside** the workspace is in scope and ungoverned — and M-18 now *selects* files into the prompt from an FTS index built over the whole repo (spec `05`), with no exclusion rule stated. | Secrets enter prompts, provider logs, and (persistently) memory, through the plan's own newest feature. | **ADR-06-03**: one denylist helper shared by read tools, graph indexing, and context selection; `.gitignore` honoured or the claim removed. |
| **GAP-05** | **H** | Memory has no user-facing inspect, export, or delete path. OQ-5 decides only *"no automatic deletion"*. | Memory is now persistent, indefinite, provenance-tagged, and holds user behaviour, project facts and agent observations. | If spec `07 §6` deletes `MemorySidebar`, the store becomes invisible and unremovable. A secret stored once (GAP-01/04) is unremovable too. | **ADR-06-04**: route the memory surface (not delete it); add `forget(id)`, `forget scope`, and a plain-file export. |
| **GAP-06** | **H** | No spend ceiling that stops work. Per-task budget is inert; no default stated; no session/daily/global cap exists. ADR-02-04 dropped the budget dimension by choice. | 7 workers × unattended ticks × a debug loop with no outer bound (FB-05) × unmetered ACP broadcasts (`G9:401`) on **the user's own key**. | A loop bills the user with nothing to stop it, and `G-BUDGET-FIRES` passes on a breaker no caller arms. | **ADR-06-05**: a daily hard ceiling and a per-task default, both with stated values, a stated trip behaviour, and a visible off switch. |
| **GAP-07** | **M** | Telemetry is not stated either way (`telemetry`, `opt-out`, `phone-home`, `crash-report` = 0 occurrences). Meanwhile the updater polls every 4 h to an env-overridable URL (`G3c IN-89`, `IN-19`). | "We send nothing" is a product claim worth one sentence, and it is currently unwritable because the update check is unspecified. | The plan cannot honestly answer *"does this app phone home?"* | **ADR-06-11**: state *no telemetry*, specify exactly what the update check transmits, and make the check disableable. |
| **GAP-08** | **H** | Autonomy limits are never enumerated. No statement of what an agent may **never** do. | `git_push` exists and returns `Ok` when the server rejected the ref (`G3b GI-14`); `e2e_probe` runs arbitrary shell with no jail and an unvalidated URL (SSRF, `G10-73/74`); `delete_file` runs `remove_dir_all`. M-38 governs **UI** destructive actions only. | The blast radius of an unattended worker is undefined, so it is unbounded. | **ADR-06-06** (with GAP-10): a written never-list — no push to a remote, no write outside the workspace, no network host outside the provider set — enforced in `ToolEffect`, not in prompts. |
| **GAP-09** | **L** | Multi-user / shared machines. | `~/.autodev/` and the OS keychain are per-OS-user on all three platforms. Two OS users are naturally isolated. | **NOT NEEDED as a mechanism.** One real edge remains: the keychain uses a single flat service name `"autodev"` (`G3c P-35`), so two AutoDev profiles for one OS user collide. | Write the acceptance ("single OS user per install") into §1.5 Non-goals, and namespace the keychain service key. |
| **GAP-10** | **H** | Nothing makes an agent write reversible, and nothing says how the user learns of damage. Undo is not in the binary (`F-44`); a failed checkpoint is invisible (`G10-11`); `"auto-commit"` = 0 occurrences. | The headline test hands the user a modified repository. Recovery is the difference between a bad turn and a lost afternoon. | A worker corrupts a repo, the user finds out by reading a diff, and there is nothing to roll back to. | **ADR-06-06**: one checkpoint system (resolve R3), a checkpoint before the first agent write of a task, a hard failure if the checkpoint fails, and a stated git-state contract. |
| **OPS-01** | **M** | Embedding backfill has no contract. The 130 MB model is skippable; nothing says what happens to memories written while it was absent. | Retrieval quality silently splits into two populations that look identical. | Permanently lexical-only rows with no symptom. | Name the backfill job in the cadence register (**ADR-06-08**). |
| **OPS-02** | **M** | The reconciliation sweep has no frequency, owner, overlap rule or failure behaviour. ADR-02-01 says only *"low-frequency"*. | It is the sole recovery path for stranded work after cron is demoted. | Today's 1–2 min cron is replaced by an unnamed number. `G3c IN-42` shows the adjacent loop already burst-fires on missed ticks. | **ADR-06-08**. |
| **OPS-03** | **M** | Graph build/rebuild has no trigger or cadence (`"watcher"`, `"debounce"`, `"interval"` = 0 occurrences). Rebuild is 30–90 s and manual today. | M-18 makes **every prompt's quality** depend on graph freshness. | A stale graph silently degrades context selection with no signal, and `G-GRAPH-SCOPE` still passes. | **ADR-06-08** + surface graph age wherever graph-scoped context is used. |
| **OPS-04** | **M** | Retention *policy* is decided (OQ-5); retention *cadence* is not. Memory is exempted from retention entirely. | Episodic writes one row per message forever, plus a 384-dim vector, with consolidation deferred to Phase 4. | `G-RETENTION` asserts a cap nobody enforces on a schedule; `memory.db` grows without bound or measurement. | **ADR-06-08** + a stated memory size budget with a user-visible number. |
| **OPS-05** | **M** | **No retry or backoff policy exists.** `ErrorClass::Retryable` is defined and never acted on; `"backoff"` = **0 occurrences**. | `G2a:680`: a single 429 or 5xx aborts the run today. Spec `02` classifies the error and stops. | The classification is a writer with no reader — precisely INV-8's failure mode, inside the invariant's own home spec. | Add a retry/backoff section to spec `02` with attempt caps, jitter, and `QuotaExhausted` excluded. |
| **OPS-06** | **H** | The CI end-to-end smoke has no credential path. | `G-KEY-NOENV` forbids env fallback; ADR-03-05 keeps `.env` out of the binary; headless Linux has no keychain (`G3c P-35`). | `G-PLATFORM-TRIPLE` and `G-FIRSTRUN` — the restructure's definition of done — cannot execute in CI. | **ADR-06-09**: decide between a CI keychain provisioning step and a signed mock-provider harness. |
| **FB-01** | **H** | Affinity has no cold start, no exploration, no decay, and a global 200-row window. | Turning it on (M-33) starts a **new** live loop with zero history and deterministic greedy selection. | Cold start ties every worker at 0.5, the stable sort returns `supervisor` (`F-ROU-02`), and the first tiebreak becomes the permanent winner. `F-ROU-05`: a busy worker's history is pushed out of the global window, silently resetting its affinity. | **ADR-06-07**: an exploration floor, per-worker (not global) windows, and outcome decay. |
| **FB-02** | **M** | Every threshold is inherited with no provenance: `>= 3`, `0.85`, `0.15`, `n/(n+5)`, 14-day half-life, `0.6/0.4`, "three consecutive failures". | Spec `03 §4.2` itself concedes the memory scorer's *"value has never actually been demonstrated in this product."* | Numbers ship as facts. One is already provably dead (`conf > 0.85` vs max `0.85`). | Mark every constant as a **named, config-surfaced provisional** with a measurement owed at its phase gate. |
| **FB-03** | **H** | Nothing states what an affinity outcome is derived from. | `G-AFFINITY-CLOSES` records an outcome on autonomous completion. Completion today can be satisfied by `git commit -m "cargo build fixes"` matching a substring gate (`G10-96`). | The router trains on labels produced by the gate M-13 exists to replace. | Add to **ADR-06-07**: affinity may learn **only** from Rust-observed `build_evidence`, never from a status transition. |
| **FB-04** | **H** | Memory has no correction, retraction or contradiction path in scope, and `authority` is not bound to `source`. | Provenance (`source`) is adopted — but nothing says the **scorer** treats `agent_observation` differently from `user`. Contradiction resolution is deferred to Phase 4 (ADR-05-05). Worse, `G4:467-477` describes a pre-flight memory check that **short-circuits a tool without executing it**, with *"no recency check, no confidence threshold, and no retry-anyway escape hatch"* — and the plan never says whether that behaviour is ported. | A wrong agent observation is recalled, restated, re-proposed and re-stored at equal standing; at worst it permanently blocks a working tool. | Bind `authority` to `source` now (one join, no migration); state explicitly whether the pre-flight short-circuit is ported and, if so, bound it. |
| **FB-05** | **H** | The LLM debug step has no outer bound. | ADR-02-02 engages it after 3 consecutive failures; nothing caps how many times it may engage, whether the counter resets on partial success, or whether its own edit counts as an attempt. `"max_retries"` = 0 occurrences. | An unbounded, unmetered `3 fails → ORGAN → retry` cycle on the user's key — compounding GAP-06. | State a hard attempt ceiling and attribute the ORGAN call to the task budget. |
| **FM-01** | **H** | No behaviour is defined for a locked, denied or absent OS keychain. Runtime is keychain-only by decision. | `G3c P-35`: on headless Linux *"every store/get fails and there is no fallback secret store at all."* Spec `09` fixes only the deb `depends` line. | The app cannot start a turn and cannot say why — colliding with `G-BYOK-VISIBLE`, which promises a specific actionable error. | Specify the diagnosis and the fallback stance (refuse loudly, name the missing service, link the fix). |
| **FM-02** | **M** | Disk full, DB corruption, and user-data backup are unaddressed (`"disk full"` = 0 occurrences). | 11 SQLite DBs + WAL + a 130 MB model + an unbounded `memory.db`. The only backup in the crate is an update-time restore point for two DBs. | A corrupt `tasks.db` or `memory.db` is unrecoverable and undiagnosed. | Add an integrity check on open and a stated recovery stance (quarantine-and-recreate is acceptable for a local tool; silence is not). |
| **FM-03** | **M** | The 130 MB embedding model download has no integrity verification. | INV-11 establishes that *absence of a signature is a failure* for the updater. The plan then downloads a large binary artifact over the network with no stated verification. | An internally inconsistent trust doctrine, and a corrupt model that fails at inference rather than at download. | Checksum-verify the model; on mismatch, degrade to FTS5 and say so (the `G-MEM-DEGRADE` path already exists). |
| **FM-04** | **M** | No behaviour when the repository moves or is deleted mid-task. | M-14 fails loudly at *resolution*; nothing covers the window between resolution and write, in-flight tasks, or the graph DB keyed on that path. | Workers write into a vanished path or a recreated one; the graph key silently forks (`G-PATHKEY` covers spelling, not existence). | State the mid-task revalidation point and the task disposition. |
| **FM-05** | **M** | No behaviour when a provider retires the persisted active model. | M-07 hydrates the saved model on startup. `G-PRICE`'s *"unknown model errors"* is a **pricing** rule, not a lifecycle one. | First turn after a vendor deprecation fails with a pricing-shaped error and no path forward — a first-run failure with no first run. | On hydration, validate the model against the live catalogue; fall back to a named default and tell the user. |
| **FM-06** | **L** | Clock skew is unaddressed (`"clock skew"` = 0 occurrences). | The 4 h orphan TTL, the 14-day memory half-life, cache TTLs and the daily rollup all read wall-clock. `G-TIME-ROUNDTRIP` property-tests DST, not a clock moving backwards. | A backwards clock jump makes TTLs unreachable — the same shape as the bug `GI-01` already caused. | Use monotonic time for durations, wall-clock only for display. One sentence in spec `08 §3.3`. |
| **FM-07** | **M** | No fleet-level behaviour on quota exhaustion. | `ErrorClass::QuotaExhausted` is correctly non-retryable at the **provider** layer; nothing says what **7 workers** do when they all hit it. | Every worker fails its task, the board fills with failures, and the user's first signal is an empty board. | Quota exhaustion pauses dispatch and raises one toast, not seven task failures. |
| **W-01** | **H** | The 130 MB model download is absent from the first-run flow that owns first run. | Spec `03 §4.4` says it *"is a first-run surface"* and `G-MEM-DOWNLOAD` gates it; spec `07 §7.2` — the M-35 flow — has three steps and no download. | Whoever implements M-35 from spec `07` ships without it, and `G-MEM-DOWNLOAD` fails against a flow that never had the step. | Reconcile: spec `07 §7.2` becomes the single authority and carries the step. |
| **W-02** | **H** | The user is never told what happens to their code. | `G11:249` drafts the exact copy and adds *"Anything less is not a BYOK product; it is a key field."* Spec `07 §7.2` omits it, and there is no equivalent line about the code, which **does** leave the machine. | The first-run flow tells a true story about the key and no story about the code. | Adopt `G11:249` verbatim and add its counterpart for source content (**ADR-06-01**). |
| **W-03** | **H** | *"Build-verified"* on a genuinely clean machine is unspecified. | `G-CLEAN-INSTALL` asserts a working app with **no Rust, Node, Python or uv**. The headline test requires a **build-verified** change. With no toolchain, M-13 produces no `build_evidence` and the `qc_review` gate hard-refuses — the exact `SK-50` loop. | The acceptance test can only pass on a machine that is not clean. ADR-03-03's *"report what is missing, never install"* is the right answer and is connected to nothing. | **ADR-06-10**: define the no-toolchain outcome (a named `ToolchainMissing` state, surfaced, not a build failure) and restate the acceptance test honestly. |
| **W-04** | **H** | The first turn runs an unattended writer against the user's own repository with no stated safety. | Spec `07 §7.2` step 3 is one line: *"a suggested prompt that demonstrably works."* Nothing says the first turn is scoped, previewed, or reversible — and undo does not exist (GAP-10). | The riskiest moment in the product is specified in nine words. | The first turn is diff-previewed and checkpointed before it is applied. |
| **W-05** | **M** | The git-state contract is undecided. | The user may point at a dirty tree, a detached HEAD, a mid-rebase, a submodule or a worktree; and nothing says whether agent output is committed, staged or left dirty. | This is what makes W-04 unrecoverable: no commit boundary means no revert point. | State a precondition check and one delivery state (recommend: agent work lands on a named branch, committed). |
| **W-06** | **L** | The acceptance-test first turn has no graph. | The graph must be built before M-18 can scope context, and build is 30–90 s and manually triggered (OPS-03). | The proof-of-value turn runs with the weakest possible context. | Fold into OPS-03's trigger decision. |

---

## 3 · Category 1 — Unstated policies

### 3.1 Data egress · GAP-02, GAP-03

**What the plan does say, and it is genuinely good.** Spec `06 §1.1` names `analyze_diff` as *"the sharpest finding in this spec"* — a tool declaring `is_read_only = true` that *"**uploads the full diff of any repository to a third-party LLM**"* with no prompt in any mode. `§1.2` promotes `Network { host }` to a first-class `ToolEffect` with the correct reasoning: *"A tool that sends the user's code off the machine is not 'read-only' under any reading."* `G-PERM-NETWORK` is a gate that can fail. Nothing in this review asks for that to be rediscovered.

**What it does not say.** Three things, each load-bearing.

**(a) The gate is on tools; the egress is in the loop.** `analyze_diff` is not the egress path — it is the *most visible* one. The ordinary path is: `read_file` / `grep` / `glob` / `list_directory` return content → the content becomes prompt bytes → the bytes go to Anthropic, OpenAI, Google or xAI. All four of those tools are **unjailed and auto-approved** (`G10:1977`), and M-18 now *adds* a mechanism that selects files into the prompt automatically. A permission model that prompts for the one tool with an explicit HTTP call, while the loop ships the same bytes on every turn, is a model that measures the wrong thing.

**(b) `analyze_diff`'s destination ignores the user's own configuration.** `G10:2302` / `G9:986`: provider and model are read from `AUTODEV_ACTIVE_PROVIDER` / `AUTODEV_ACTIVE_MODEL` env vars with a hardcoded `anthropic` fallback — *"User picks Ollama; every `analyze_diff` still bills Anthropic."* Spec `02 §7` lists this as one of 18 hardcoded model strings, framed as a **model-selection** defect. It is also an **egress-destination** defect: the user's code goes to a vendor they did not choose. The plan fixes the symptom without naming the class.

**(c) The collision.** `G-PERM-NETWORK` says *prompts, in every mode*. `G-QC-ANALYZE` says `analyze_diff` must actually run in autonomous QC. Both are decided; neither cites the other. The resolution cannot be "auto-approve in Autonomous" without recreating `is_read_only`. The workable shape is a **standing, per-workspace egress consent** granted once during setup — explicit, revocable, visible — which autonomous mode may consume and which the UI can show as *"code from this workspace is sent to <provider>."*

**Redaction before egress — GAP-01, restated as its own point.** The evidence chain is short and unambiguous:

- `G4b §A.14`: *"`redact_dict(...)` (`redaction.py:22`) with a 20-entry `SENSITIVE_KEYS` set (`:9-20`). **126 lines, directly reusable — the `SENSITIVE_KEYS` list is the spec. PORT.**"*
- `G4b` summary table: `security/` → *"Keep redaction; replace vault with OS keychain (already done in Rust)."* The second clause landed in the plan (ADR-03-01, spec `02 §6`). **The first did not.**
- The port set the plan actually carries is `G1 §C.4` P1–P9. Redaction is not P1–P9.
- Search across the whole suite: `redact` = **0**, `scrub` = **0**, `sanitiz` = **0**, `"secret scan"` = **0**.
- `G4:486`: *"`context` carries the **unredacted** arguments — a secret passed as a tool argument lands in procedural memory verbatim."*

Half of a two-clause verdict was adopted and half was dropped, silently. Because the dropped half has no mechanic, it will not appear in the MDP, will not appear in `TESTING_GATES.md`, and will not appear in P8's entry gate.

### 3.2 Secrets in the workspace · GAP-04

The plan closes the **escape** problem and leaves the **sensitivity** problem untouched.

- `G-JAIL-COVERAGE` makes every path-taking tool call the jail — correct, and it kills the *outside-the-workspace* read. Spec `06 §3.1` states the principle plainly: *"reading a file outside the workspace is an exfiltration primitive, not a harmless read."*
- But `.env`, `id_rsa`, `.pem`, `credentials.json`, `.npmrc` and `terraform.tfstate` are routinely **inside** a workspace, and nothing governs them.
- `.gitignore` is never honoured, while a tool description claims it is: `G2a T33` — *"description claims 'Automatically respects .gitignore'; the body uses `WalkDir` + a hardcoded `skip_dirs` array and never reads `.gitignore`."* Spec `06 §4` correctly classes false descriptions as defects (33 of 38 are defective) — but the fix is *"make the description true or make the code match"*, and the plan never decides which way this one goes.
- `G10-07`: supplying any line range disables the 256 KB read guard, *"returns the entire multi-megabyte file into the context window."*
- The only `.env` awareness in the Rust tree is a **risk score**, not an exclusion: `G3b:118` — `CONFIG_FILENAMES = ["Dockerfile", ".env", "docker-compose"]` feeding a 0.15 risk factor, by case-sensitive substring, so `config.environment.ts` matches spuriously.
- And the one control a user could install to protect themselves is fake: `G5c:279` — *"A user installs 'Block force push' and 'Block .env file commits', sees them listed as ENABLED, and is protected by nothing."*

**Two new surfaces make this worse, not better.** The graph (spec `05`) builds an FTS index over the repository with no stated exclusion, and M-18 then selects from that index into the prompt. So under the restructure a `.env` can be indexed, ranked, selected, sent, and — via the memory gateway — stored.

**This is not a demand for a secret scanner.** A denylist of filename patterns plus a `.gitignore` read, applied at one shared helper used by read tools, graph indexing and context selection, is proportionate for a local developer tool. What is not proportionate is having none of it while shipping persistent memory.

### 3.3 Memory and privacy · GAP-05

The memory workstream is the strongest-designed part of the plan and the one with the largest policy hole.

**Handled well** (see §6): mandatory scopes, `source` provenance pulled forward a whole phase, and ADR-05-04 removing `remember_this`'s unrestricted write in favour of propose-and-gate. Those are the right structural decisions.

**Not decided:**

| Question | Plan's position |
|---|---|
| Automatic deletion | Decided: **none**. OQ-5, citing anchor §11 (*"deprioritized before they are destroyed"*). |
| User-initiated deletion | **Silent.** `"delete memory"`, `"forget"` (as a capability) = 0 occurrences. |
| Export | **Silent.** `"export memory"` = 0 occurrences. |
| Inspection | **Undecided.** The only surfaces are `MemorySidebar` (408 LOC) + `MemoryStats`, inside the ~3,700 unreachable LOC at spec `07 §6`'s *"route it or delete it."* |
| Consent | **Silent.** `"consent"`, `"privacy"` = 0 occurrences. The first-run flow (spec `07 §7.2`) never mentions memory. |
| Growth bound | **None.** Memory is exempted from OQ-5 retention; episodic writes one row per message forever + a 384-dim vector; consolidation is deferred to Phase 4. |

The gateway's evaluation criteria are **importance, novelty, duplication, confidence** (anchor §18). **Sensitivity is not among them** — which is the same gap as GAP-01/GAP-04 arriving at the persistence layer.

Note the interaction that makes this urgent rather than tidy: the anchor's *"deprioritize before destroying"* is exactly right for a memory *quality* system and exactly wrong as a *privacy* answer. Decay affects ranking, not existence. A secret that decays is still on disk.

**Proportionate ask:** a routed memory surface, `forget(id)` / `forget(scope)`, and an export. Three small features. Not a consent framework.

### 3.4 Autonomy limits · GAP-06, GAP-08, GAP-10

`"blast radius"` appears five times in the suite — every one of them about **UI confirmations** (M-38, spec `07 §7.5`). The agent's own blast radius is never bounded.

**What an unattended worker can reach, per `G10:2193-2206`** (gate column in `PermissionMode::Autonomous`): `bash` **none** · `e2e_probe` **none** · `write_file` / `edit_file` / `append_file` **none** · `spawn_subagent` / `spawn_parallel_subagents` **none** · `update_task_status` **none**. And `G10:2208`: *"The only genuinely enforced controls in the whole surface are `path_jail` (four tools) and `bash`'s redirect scanner."*

The plan fixes the **plumbing** — one execution path (M-08), a truthful `ToolContext` (M-09), a real `ToolEffect` gate — and never writes the **policy** those mechanisms would enforce. A permission model with no never-list is a prompt-approval UI, not a boundary.

Three specific silences:

**(a) Network reach.** `e2e_probe`'s `probe_url` accepts any host with no validation (`G10-74`, SSRF), returning 400 chars of the body to the model. The plan lists `e2e_probe` as CRITICAL and prescribes the jail; it never states the **allowed host set**. For a product whose only legitimate outbound destinations are four provider APIs plus the update endpoint, that set is short and worth writing down.

**(b) Git reach.** `git_push` exists and *"returns `Ok(())` even when the server rejected the ref"* (`G3b GI-14`). Nothing in the plan says whether an agent may push. Given `"git push"` = 0 occurrences and `"auto-commit"` = 0 occurrences, the plan has not decided whether agents commit at all — while the acceptance test hands the user a changed repository.

**(c) The off switch.** Cancellation is per-session and well-specified (spec `01 §3.3`: a `CancellationToken` threaded into the provider call, the tool executor and every permission wait — genuinely good). There is **no fleet-level stop**. `"kill switch"`, `"off switch"`, `"panic button"` = 0 occurrences. A user watching workers do the wrong thing has no single control.

**Reversibility (GAP-10) is the one that turns a bad turn into a lost afternoon.** The chain: `db/checkpoints.rs` is not compiled, so `file_checkpoints` does not exist at runtime (`F-44`); R3 leaves two rival checkpoint systems at *"pick one"*; `git/checkpoint.rs` opens an unpooled connection per operation with no busy timeout (`G3b GI-27`); and `write_file:115`'s `auto_checkpoint_before_write` returns `()`, so *"a failed checkpoint is invisible and the overwrite proceeds"* (`G10-11`). P1 declares the module. **No mechanic makes the write reversible, no ADR picks the system, no gate proves a checkpoint exists before an agent's first write.** M-38's *"undo where feasible"* with its own parenthetical admission is the plan acknowledging the gap without owning it.

### 3.5 Telemetry · GAP-07

Zero occurrences of `telemetry`, `opt-out`, `phone-home`, `crash-report`, `PostHog`, `Sentry`. The grounding confirms the same on the code side: no third-party SDK, and the local `analytics.db` is a dashboard, not a sink.

So the *answer* is almost certainly "we send nothing" — and that is a **feature**, currently unwritable, because one thing is unspecified: the updater polls **every 4 hours** (`G3c IN-89`) to a URL read from `AUTODEV_UPDATE_URL` with no scheme validation (`IN-19`). The plan fixes the **verification** of what comes back and never states what goes out, or whether the check can be turned off.

For a local-first BYOK tool, "no telemetry, and the only outbound request we make on our own initiative is a version check that sends nothing but a version" is a strong sentence. It costs one paragraph and one settings toggle.

### 3.6 Multi-user · GAP-09 — **mostly NOT NEEDED**

`~/.autodev/` resolves per-OS-user on all three platforms, and the OS keychain is per-user by construction. Two OS users on one machine are already isolated without any mechanism. §1.5 says *"Single user, single machine, local-first"* — which is about hosting; the same sentence should be extended to say the same thing about OS users, as an **acceptance**, not a feature.

The one genuine residue: `G3c P-35` records a *"single flat service name `\"autodev\"`"* with keys namespaced only by provider. Two AutoDev profiles for one OS user collide silently. That is a one-line fix (namespace the service key), not a workstream.

### 3.7 Failure disclosure — see GAP-10, FM-01, FM-07

The plan's error contract (M-36) is about **UI failures**: 88 `console.*`-only sites, 8 `alert()`, the toast system that has never rendered. It is excellent work on the right problem — *"every failure surfaces what failed, why, and what to do next."*

It does not cover **agent-caused damage**, which is a different disclosure problem: nobody is at the keyboard when it happens, the failure is in the user's files rather than in the app, and it may be silent. There is no notion of a per-task change summary the user reviews, and — given W-05 — no commit boundary to review it against.

---

## 4 · Category 2 — Undefined operational contracts

The clean measurement: across all 13 plan documents and both anchors, `"interval"` = **0**, `"cadence"` = **0**, `"how often"` = **0**, `"watcher"` = **0**, `"debounce"` = **0**, `"backoff"` = **0**, `"max_retries"` = **0**. Every mechanism below is described by **what** it does; none is described by **when**.

| Mechanism | Spec says | Not stated | Failure mode if left |
|---|---|---|---|
| **Embedding (write path)** | `03 §4.4`: writes *"enqueue and embed behind"* on `spawn_blocking` | Queue depth, drain rate, retry on failure, what happens on shutdown with a non-empty queue | Rows that are never embedded and are indistinguishable from rows that are |
| **Embedding backfill** | — | Whether skipped-model-era rows are ever embedded once the model arrives | Two silently different retrieval populations (**OPS-01**) |
| **Consolidation** | Deferred to Phase 4 (ADR-05-05) | — (correctly deferred) | Acceptable — but see growth, below |
| **Memory growth** | OQ-5: *"No automatic deletion"* | Any size budget, any measurement, any user-visible number | `memory.db` grows one row + one vector per message, forever (**OPS-04**) |
| **Reconciliation sweep** | ADR-02-01: *"a low-frequency reconciliation sweep"* | The frequency; who starts it; overlap behaviour; failure behaviour | It is the only stranded-work recovery path after cron is demoted (**OPS-02**). `G3c IN-42` shows the adjacent 30 s loop already burst-fires on missed ticks with default `MissedTickBehavior` |
| **Worker kick** | `04 §2.2`: RAII + timeout for `kick_inflight` / `SK-42` | The timeout value; and that a kick is a **full LLM tick** (`G9:387`), so kick frequency is spend frequency | Unbounded cost per unit time (**GAP-06**) |
| **Graph build / incremental update** | `05`: mechanics only | Trigger, cadence, staleness bound, whether a watcher exists | M-18's context quality silently degrades with graph age (**OPS-03**) |
| **Retention pruning + VACUUM** | OQ-5 per-table policy; *"Plus a scheduled `VACUUM`"* | The schedule; the owner; behaviour on partial failure | `G-RETENTION` asserts a cap with no enforcer (**OPS-04**) |
| **Update check** | `09 §3`: verification rules | Cadence (4 h, from grounding only), payload, opt-out | Unstatable telemetry answer (**GAP-07**) |
| **Provider retry** | `02`: `ErrorClass::{Retryable, Overloaded, …}` | **Any retry policy at all.** Attempts, backoff, jitter, budget interaction | `ErrorClass` is a writer with no reader — INV-8's own failure mode (**OPS-05**) |
| **Cache-hit alarm** | `02 §4.2`: build the alarm before the cache | Threshold, window, and what the app does when it fires | An alarm with no trip point is a log line |
| **Cost rollup** | `02 §5`: real date handling | When the rollup runs now that the old hourly sleep is deleted | The `IN-56` defect is diagnosed and its replacement schedule is unnamed |

**Ownership is the common thread.** Not one of these names a component that owns it. In a single-process desktop app the right answer is probably one supervisor task with a table of jobs — which is exactly what **ADR-06-08** proposes.

---

## 5 · Category 3 — Feedback and statistical traps

Two loops in this system learn from their own outputs. Both are currently inert, which means turning them on is **new-feature risk wearing a port's clothes** — the same framing the plan itself used, correctly, in ADR-01-07.

### 5.1 Routing affinity — FB-01, FB-02, FB-03

**The full algorithm** (`G9:512-537`):

```
score      = success_rate · 0.6 + duration_score · 0.4
confidence = total / (total + 5.0)
qualifies  = (successes + failures) >= 3
window     = last 200 routing_history rows, GLOBAL
selection  = scores.remove(0)          // greedy argmax, stable sort, no randomization
blend      = static_conf > 0.85 AND historical >= 0.3
```

**Provenance: none.** The only justification anywhere in 18,000 lines of grounding is the auditor's own sentence — `G9:715` — *"a defensible cold-start policy worth keeping verbatim."* No benchmark, no derivation, no citation. The corpus twice says the opposite about sibling constants: `G10:672` *"the number is invented; no benchmark exists in the tree"*; `G7:422` *"No document reports measured AutoDev numbers against these targets."*

**Cold start is handled, and the handling is the trap.** Below 3 samples every worker gets `score 0.5, confidence 0.0`. All scores tie. The stable sort plus `ORDER BY is_system_defined DESC, id ASC` returns **`supervisor`** (`F-ROU-02`). So on a fresh install the router recommends the Supervisor for implementation work, that assignment produces the first outcome, and the first outcome is the first affinity. The seed is not random — it is deterministic and wrong.

**Rich-get-richer has no counterweight.** Selection is pure greedy argmax. Zero occurrences of `bandit`, `thompson`, `ucb`, `epsilon-greedy`, `softmax`, or `exploration` anywhere in `docs\restructurization\`. There is no tie-break randomization, no exploration floor, and no decay on old outcomes. Once a worker is ahead it is selected, which produces more outcomes, which keeps it ahead.

**The window is global, and that is worse than it looks.** `F-ROU-05`: the last 200 rows **across all workers**. A busy worker's history for a given task type can be pushed out of the window entirely, *"silently resetting its affinity"* — so the learned signal is not just biased, it is non-stationary in a way nobody chose.

**Labels are unverified — FB-03.** `G-AFFINITY-CLOSES` records an outcome on autonomous completion. What makes a completion? Today, a status transition that a substring gate can satisfy: `git commit -m "cargo build fixes"` records passing build evidence (`G10-96`). M-13 fixes the evidence, and the plan never states the consequence — **affinity must learn only from Rust-observed `build_evidence`, never from a status transition.** Without that line, M-13's correctness does not propagate to the learner.

**Credit where due.** The plan *does* catch the adjacent trap: R-8 names *"fixing the fleet re-enables a broken feedback loop"*, and Part 6 carries the hard sequencing constraint **M-12 before M-33** so affinity does not train on stolen-task attributions, plus *"consider discarding existing `routing_history`."* That is exactly right. FB-01/02/03 are the **next** layer, not a restatement.

### 5.2 Memory reinforcement — FB-04

**The write side is well-governed** (ADR-05-04, and it is the plan's best memory decision): agents propose, a deterministic gateway decides, `remember_this` is removed. **The read side is not.**

- **`authority` is not bound to `source`.** Provenance is adopted as a column with six values. The scorer's `authority` is a per-record-class constant (`user_correction` 0.85, episodic 0.4 — `G4c:466`). Nothing in spec `03` says an `agent_observation` scores lower than a `user` statement. So the *distinction the column exists to make* is not made at the point it matters.
- **Confidence is two-valued.** `G4c:190`: `0.95` if `source_type == "user_message"` else `0.85`. That constant pair is the entire confidence model being ported.
- **Contradiction handling is log-only and deferred.** `G4c:110`: conflicts are *"logged only (top 3) … they never block, merge, or supersede a write"*, over a corpus capped at 5 cache keys × 20 facts. ADR-05-05 defers contradiction resolution to Phase 4. Correct scoping — but it means the **only** thing standing between a wrong memory and permanence is the write-time novelty check, which by construction *rewards* a novel wrong memory.
- **No correction path is in scope.** `user_correction` is the highest-authority class and `G4c:296` records *"No dedup. No contradiction check. No idempotency key. Every tool call writes a new row."* There is no retraction, no supersede, no way for the user to say *"that is wrong."*
- **The sharpest one, and the plan is entirely silent on it.** `G4:467-477` documents a **pre-flight memory check** that returns `ToolResult(success=False, from_memory=True, suggested_fix=resolution)` — verbatim: *"**This short-circuits the tool without executing it** … a stale learned 'resolution' permanently blocks a tool that may now work. There is **no recency check, no confidence threshold, and no retry-anyway escape hatch**."* This is LivingAgent behaviour, and M-01 ports *"the LivingAgent loop"* into a fleet-agent layer. **The plan never says whether this mechanism comes with it.** If it does, a single wrong procedural memory can permanently disable a working tool for a worker, and no gate in spec `03` or `04` would catch it.

### 5.3 The debug loop — FB-05

ADR-02-02's decision text is `00:760`: *"only do B after **say** three consecutive failures."* The word *say* is the number's entire provenance, and the plan ratifies it four more times (`04 §4.1`, the diagram, `G-BUILD-ESCALATE`, the balance table) without adding one.

The diagram (`04:91-101`) is `attempt 1-2-3 → ORGAN diagnoses → DET re-runs` — **with no exit**. Unstated: how many times the cycle may repeat; whether the counter resets on a partial success; whether the debug step's own suggested edit consumes an attempt; and what the ORGAN call costs. `"max_retries"` = 0 occurrences; the only bound anywhere is the pre-existing `retry_count` column whose value is never specified.

Compounded by GAP-06: the ORGAN call is a full-context model call on the user's key, and the budget that would meter it has zero callers.

### 5.4 The inherited-constant register — FB-02

Every threshold the plan carries forward, with its provenance:

| Constant | Where | Provenance |
|---|---|---|
| `>= 3` outcomes | `scorer.rs:70`, ratified `04 §5` | none |
| `0.6 / 0.4` score weights | `scorer.rs:82` | none |
| `n / (n + 5)` confidence | `scorer.rs:83` | none |
| 200-row global window | `G9:512` | none; and `F-ROU-05` shows it is actively harmful |
| `conf > 0.85` vs max `0.85` | `scorer.rs:121` vs `patterns.rs:15` | **provably wrong — the branch is dead code** (`04 §5.1`) |
| `0.15` marginal-utility stop | `G4c:489`, spec `03 §4.2` | none |
| `0.30 / 0.25 / 0.20` marginal weights | `G4c:487` | none |
| 14-day recency half-life | `G4c:467` | none |
| 9-term weight vector summing to 1.00 | `G4c:450` | none |
| BM25 `k1=1.5, b=0.75`; `0.3/0.7` blend | `G4c:388-394` | library defaults |
| `MIN_RELEVANCE_THRESHOLD = 0.0` | `G4c:412` | *"i.e. the filter is disabled by default"* |
| "three consecutive failures" | `00:760` | the word *"say"* |

Spec `03 §4.2` is admirably honest about the set as a whole: the competitive scorer's *"value has never actually been demonstrated in this product"* — while §4.3 lists seven defects proving the recall path those weights rank was broken the entire time. **These numbers were tuned against a retrieval path that ignored its own query.**

The ask is not to derive them. It is to **mark them provisional, put them behind config, and owe a measurement at the phase gate that turns each loop on** — which is exactly what ADR-01-08's phased answer already does for the budgeting model, applied to the constants too.

---

## 6 · Category 4 — Missing failure modes

For each major mechanism: *what happens when it fails, and who notices?*

| Mechanism | Failure | Plan's answer | Who notices |
|---|---|---|---|
| **OS keychain** | Locked, denied, or absent (headless Linux: *"every store/get fails and there is no fallback secret store at all"*, `G3c P-35`) | Deb `depends` fixed (`09 §2.1`). **Runtime behaviour unstated.** Keychain-only by decision. | Nobody — and it collides with `G-BYOK-VISIBLE`'s promise of a specific actionable error (**FM-01**) |
| **Keychain read at boot** | Swallowed to `console.error` (`G5c F28`), Providers UI silently empties, no retry control | M-36's error contract covers the class generically | The user sees an empty Providers list and concludes their key was lost |
| **Disk full** | Any of 11 SQLite DBs, WAL, or the 130 MB model download | **Unaddressed.** `"disk full"` = 0 occurrences | Nobody (**FM-02**) |
| **DB corruption** | `memory.db` (unbounded), `tasks.db`, `graph.db` | Only migration-time backup (`G-MIGRATE-BACKUP`). No integrity check on open, no recovery path, no user-data backup | Nobody (**FM-02**) |
| **Embedding model corrupt / partial** | 130 MB over the network | Resumable + skippable (`G-MEM-DOWNLOAD`). **No integrity verification** | Fails at inference, not at download — and it contradicts INV-11's own doctrine (**FM-03**) |
| **Repo deleted / moved mid-task** | Between resolution and write | M-14 fails loudly **at resolution** | In-flight tasks, and the graph DB keyed on the path, are unspecified (**FM-04**) |
| **Model deprecated by vendor** | Persisted active model retired | `G-PRICE`: *"an unknown model errors"* — a **pricing** rule | First turn after hydration fails with a pricing-shaped error (**FM-05**) |
| **Clock skew / backwards jump** | 4 h orphan TTL, 14-day half-life, cache TTLs, daily rollup | `G-TIME-ROUNDTRIP` covers format and DST, not monotonicity | Same shape as `GI-01`, which already cost the orphan TTL (**FM-06**) |
| **Quota exhausted mid-fleet** | 7 workers, one non-retryable class | `ErrorClass::QuotaExhausted` correctly non-retryable at the **provider** layer | The board fills with 7 failures; no dispatch pause, no single signal (**FM-07**) |
| **Provider 429 / 5xx** | Any turn | **No retry policy exists.** `"backoff"` = 0 occurrences | The run aborts (`G2a:680`) (**OPS-05**) |
| **Background task panics** | Any spawned job | `G3c IN-77`: *"the panic is swallowed into a JoinError nobody inspects"* | Nobody — and the plan adds more background jobs (§4) without addressing it |
| **Update endpoint unreachable** | Indefinitely | `G3c IN-89`: *"errors are printed and the loop continues with no backoff and no counter"* → *"silently stops receiving security updates"* | Nobody. The plan fixes verification, not liveness |

Two of these — **FM-03** and the background-panic row — are notable because the plan already established the correct principle elsewhere and did not apply it here. INV-11 says *absence of a signature is a failure, never a pass*, and the plan then downloads a 130 MB binary with no verification. INV-4 says *a gate that cannot fail is not a gate*, and background jobs whose panics are swallowed cannot fail visibly.

---

## 7 · Category 5 — The plan versus a person using it

Walking the headline acceptance test (`10 §2`) literally, on a genuinely clean machine.

> *A user installs AutoDev on a clean machine of each of the three OSes, enters their own API key in the UI, points it at a repo, types one sentence, and receives a build-verified change in that repo which they can then run — without reading documentation, and without any Python on the machine.*

**Install.** Covered — `G-BUNDLE`, `G-SIGNED`, `G-CLEAN-INSTALL`. Note `G-CLEAN-INSTALL`'s wording: *"On a machine with no Rust, Node, Python or `uv`, the installer produces a working app."* Hold that thought.

**Launch.** Spec `07 §7.2` gives a three-step setup: workspace → provider → first turn. `G-BOOT-ESCAPE` covers the boot overlay that can lock the user out. Good.

**Step 1, workspace.** *"pick a folder; explain what agents will do in it."* The explanation slot exists and its content is unspecified — and it is the only place the egress statement (**W-02**) could go. `G11:249` already wrote the key half: *"Your key is stored in the operating system keychain and never leaves this machine,"* adding *"Anything less is not a BYOK product; it is a key field."* Spec `07 §7.2` does not carry it, and there is no counterpart sentence for the code, which **does** leave the machine.

Also unstated here: the git-state precondition (**W-05**). A dirty tree, a detached HEAD, a mid-rebase, a submodule, a worktree — the plan checks none of them, and never decides whether agent output ends up committed, staged, or dirty.

**Step 2, provider.** Genuinely well-specified: four providers, "get a key" links, **validation against the provider before storage**, the provider's actual failure reason on error. `G-BYOK-VISIBLE` is a gate that can fail. This is the strongest part of the first-run design — with one hole: it assumes the keychain works (**FM-01**).

**The missing step.** Spec `03 §4.4` states that the ~130 MB embedding model download *"is a first-run surface … it must show progress, survive interruption, and be **skippable**"*, and `G-MEM-DOWNLOAD` gates exactly that. **Spec `07 §7.2` — the flow that owns M-35 — has no such step** (**W-01**). Two specs describe two different first runs, and the one an implementer will build from is the one missing the step.

**Step 3, first turn.** *"a suggested prompt that demonstrably works."* Nine words for the riskiest moment in the product (**W-04**): a worker that writes files and runs shell, on the user's own repository, with no diff preview, no checkpoint (undo is not in the binary — GAP-10), and no stated scope. Everything else in W6 is specified to the gate level; this is specified to the sentence level.

**"…receives a build-verified change…"** Here `G-CLEAN-INSTALL` and the acceptance test collide (**W-03**). On a machine with no Rust and no Node, M-13's ecosystem detection finds a toolchain it cannot invoke, no `build_evidence` row is written, and `update_task_status` hard-refuses `qc_review` — which is the `SK-50` loop the restructure exists to kill, arriving by a different road. ADR-03-03 already chose the right answer for the adjacent problem — *"a declarative environment-check tool that reports what is missing and never installs anything"* — and **nothing connects it to the build-evidence gate, to the acceptance test, or to first run.** As written, the headline test can only pass on a machine that is not clean.

**"…which they can then run…"** M-15 covers reveal / open / run. Good.

**"…without reading documentation…"** `G-FIRSTRUN` is a real gate. Note it must now also cover W-01's missing step and W-03's no-toolchain path, or it will pass on a machine that has everything and fail on the machine the test names.

**And in CI (OPS-06).** `G-PLATFORM-TRIPLE` runs this smoke on three OSes. The app reads keys from the keychain only (`G-KEY-NOENV` *requires* it to refuse otherwise); the dev `.env` is confined to a harness *"not linked into the app"*; and headless Linux CI has no working keychain at all. **There is no specified way for the acceptance test to obtain a key in CI** — which means the restructure's own definition of done has no execution path.

---

## 8 · Proposed new ADRs — Group G

Eleven. Each in the ledger's house format. Numbering continues the ledger's convention as `ADR-06-NN` to avoid colliding with Groups A–F.

---

### ADR-06-01 — The data-egress contract

**The call:** What content leaves the machine, to whom, under what consent — and how is the user told?

**Why it matters:** The product's core loop sends the user's source code to a third party. BYOK makes the plan careful about the **key** and silent about the **code**. Spec `06` gates the one tool with an explicit HTTP call (`G-PERM-NETWORK`) while `read_file` / `grep` / `glob` / `list_directory` — unjailed and auto-approved (`G10:1977`) — ship the same bytes on every turn, and M-18 now selects files into the prompt automatically. Compounding it, `analyze_diff`'s destination is read from env vars with a hardcoded `anthropic` fallback (`G10:2302`), so the code can go to a vendor the user did not choose. And `G-PERM-NETWORK`'s *"prompts, in every mode"* directly contradicts `G-QC-ANALYZE`'s requirement that `analyze_diff` run in **unattended** QC.

**Options:**
- **(A)** Standing per-workspace egress consent, granted explicitly at setup, revocable, visible; autonomous mode consumes it; a session-scoped "what was sent" view lists files and byte counts. Every model call uses the user-selected provider — no env-var destinations.
- **(B)** Per-turn prompting for network effects. Correct in theory, incompatible with unattended workers in practice.
- **(C)** Status quo: gate the tool, leave the loop ungoverned, and let the autonomous-mode collision be resolved by whoever hits it first.

**My recommendation: (A).** It is the only option that resolves GAP-03 without recreating the `is_read_only` failure — an implicit bypass "because it's autonomous" is the same mistake in a new flag. The consent is granted **once, per workspace, by a human**, which is exactly the property unattended operation needs. The "what was sent" view is small and it is the only thing that makes the claim checkable. And routing every call through the user's selected provider is already required by M-07; this ADR just names egress as a second reason.

---

### ADR-06-02 — Secret redaction: port it, or accept its deletion

**The call:** Does `redact_dict()` and its `SENSITIVE_KEYS` set come across, or does the product ship with no redaction?

**Why it matters:** `G4b §A.14` ruled it **PORT** — *"126 lines, directly reusable — the `SENSITIVE_KEYS` list is the spec"* — and `G4b`'s summary table says `security/` → *"Keep redaction."* The vault half of that verdict landed (ADR-03-01); the redaction half did not. It is not in the nine ported capabilities, not in any mechanic, not in any spec, and not in P8's entry gate. `redact` / `scrub` / `sanitiz` = **0 occurrences** across the entire plan suite. `G4:486` records the concrete consequence already observed: unredacted tool arguments landing in procedural memory verbatim — which, post-restructure, means **permanently**.

**Options:**
- **(A)** Port it as **M-40**: a Rust `redact()` applied at three chokepoints — outbound provider payloads, memory writes through the gateway, and log/event emission. Extend P8's entry gate to **ten** capabilities.
- **(B)** Port it at the memory gateway only (cheapest; leaves prompts and logs unredacted).
- **(C)** Accept the deletion and write it down as a known limitation.

**My recommendation: (A).** 126 lines with a ready-made key list is the cheapest safety in the entire programme, and the three chokepoints already exist as named seams in the plan (S2 the provider seam, the ADR-05-04 gateway, and the event taxonomy). **(C)** is only defensible if it is *written down*, and writing it down makes it obviously unacceptable next to persistent memory. The P8 gate change is the load-bearing part: without it, this is a good intention with no gate, which is the failure mode the whole plan is built to prevent.

---

### ADR-06-03 — Sensitive-path policy for reads, indexing and context

**The call:** Is there a class of file an agent may not read, index, or place in a prompt — and is `.gitignore` honoured?

**Why it matters:** The jail fixes *escape*, not *sensitivity*. `.env`, `id_rsa`, `.pem`, `credentials.json`, `.npmrc` and `terraform.tfstate` live **inside** workspaces. `.gitignore` is never read while a tool description claims it is (`G2a T33`); `read_file("~/.ssh/id_rsa")` succeeds unjailed and auto-approved (`G10 T12`); the 256 KB cap is bypassed by supplying a line range (`G10-07`). Two **new** surfaces widen it: the graph builds an FTS index over the repo with no stated exclusion, and M-18 selects from that index into the prompt. The one user-facing control that would help — the "Block .env commits" hook — installs, shows ENABLED and protects nothing (`G5c:279`).

**Options:**
- **(A)** One shared `is_sensitive_path()` helper — a filename/extension denylist plus `.gitignore` — enforced at read tools, graph indexing, and context selection. Reads of denied paths are refused with a message naming the rule; the user can allow a specific path.
- **(B)** Content-based secret scanning (entropy/regex) on outbound payloads.
- **(C)** Nothing; rely on the jail plus ADR-06-02's redaction.

**My recommendation: (A), with (B) explicitly out of scope.** Content scanning on every outbound payload is a latency and false-positive cost a local tool should not pay, and ADR-06-02 already covers structured secrets. Path-based exclusion is cheap, predictable, explicable to the user, and it composes with the *"one shared helper, not three implementations"* discipline the plan already applies to path canonicalisation in specs `05 §2.2` and `06 §3.2`. **This ADR also settles `G2a T33` in the honest direction: make the description true rather than deleting the claim.**

---

### ADR-06-04 — Memory rights: inspect, export, forget

**The call:** Can the user see what is stored about them, take it with them, and delete it?

**Why it matters:** Memory becomes persistent, indefinite (OQ-5: *"No automatic deletion"*), provenance-tagged, and unbounded. `G-MEM-PROVENANCE` proves every row is *auditable*; nothing lets the **user** audit it. The only surfaces that could — `MemorySidebar` (408 LOC), `MemoryStats` — sit in the ~3,700 unreachable LOC that spec `07 §6` leaves at *"route it or delete it."* The anchor's *"deprioritize before destroying"* is right for memory **quality** and wrong as a **privacy** answer: a decayed secret is still on disk.

**Options:**
- **(A)** Route the memory surface; add `forget(id)` and `forget(scope)`; add a plain-format export (JSON or Markdown); show a size and row count.
- **(B)** Delete the memory UI; expose forget/export as commands only.
- **(C)** Defer to the post-restructure memory phases.

**My recommendation: (A).** It resolves an already-open decision (spec `07 §6`) rather than adding one, and it is three small features on a schema that already has every column they need — `source`, scope, tier discriminator. **(C)** is the dangerous option: memory ships in Phases 1–3 *inside* this restructure, so deferring the controls means shipping an indefinite store of the user's behaviour with no way to look at it. This also gives GAP-01/GAP-04 a remedy of last resort: if a secret does land in memory, the user can remove it.

---

### ADR-06-05 — The spend ceiling that actually stops work

**The call:** What is the default per-task budget, is there a daily/global ceiling, what does a trip do, and where is the off switch?

**Why it matters:** This is a BYOK product — the runaway bill is the user's. ADR-02-04 deliberately dropped the budget dimension. The per-task budget is **FULLY INERT** (`04 §5`): zero callers, no recipe arms it, and the Settings default has no Rust reader. `G-BUDGET-FIRES` asserts a breaker trips without stating what arms it or what it does. Meanwhile `G9 F-BUD-04` names three unmetered LLM spend paths, `G9:401` warns a broadcast fires a full LLM tick per worker with *"no per-tick cap … pins the whole fleet,"* the debug loop has no outer bound (FB-05), and `"budget cap"` / `"daily cap"` / `"kill switch"` = **0 occurrences**.

**Options:**
- **(A)** Three layers: a per-task default (stated value, arms automatically), a **daily hard ceiling** across the whole app, and a fleet-wide off switch. Trip behaviour: the task **pauses and escalates** — never fails silently, never continues.
- **(B)** Per-task only, arm the existing inert machinery, no global ceiling.
- **(C)** Display-only; warn and never stop.

**My recommendation: (A).** A per-task budget cannot bound a fleet: seven workers each inside budget is seven times the spend, and the ACP broadcast path multiplies it again. The daily ceiling is the only control that maps to what the user actually cares about, and it is cheap once M-06's accounting is honest — which this restructure delivers anyway. **Pause-and-escalate** rather than fail matters because the escalation queue already works (`04 §5`, the one capability rated ✅) and because a failed task at a budget trip destroys work the user already paid for. **(C)** is what exists today, and `G2a T40` records the outcome: *"the very sessions the owner reports ballooning are the ones with the breaker disabled."*

---

### ADR-06-06 — Autonomy limits and reversibility

**The call:** What may an agent never do, and how does a user undo what one did?

**Why it matters:** No never-list exists. In Autonomous mode the enforced-control column is **"none"** for `bash`, `e2e_probe`, `write_file`, `edit_file`, `spawn_subagent` and `update_task_status` (`G10:2193-2206`), and *"the only genuinely enforced controls in the whole surface are `path_jail` and `bash`'s redirect scanner"* (`G10:2208`). `git_push` exists and returns `Ok` on a rejected ref (`G3b GI-14`); `e2e_probe` reaches any host (`G10-74`). And reversibility does not exist: `db/checkpoints.rs` is not compiled (`F-44`), R3 leaves two checkpoint systems unresolved, and a failed checkpoint is invisible while the overwrite proceeds (`G10-11`). `"auto-commit"` = 0 occurrences — the plan has not decided whether agents commit at all.

**Options:**
- **(A)** A written never-list enforced in `ToolEffect`, not in prompts: no push to a remote; no write outside the resolved workspace; no network host outside the provider set plus the update endpoint; no execution of a binary resolved from the workspace (already `G-NO-WORKSPACE-EXEC`). **Plus:** resolve R3 to one checkpoint system, checkpoint before a task's first agent write, **fail the write if the checkpoint fails**, and state the git-delivery contract (recommend: a named branch, committed).
- **(B)** Reversibility only; leave the never-list to the permission prompt.
- **(C)** Never-list only; treat undo as post-restructure.

**My recommendation: (A), as one ADR because they are one property.** A boundary without a rollback is a boundary you discover by crossing it. The never-list is nearly free — every clause is already a mechanism the plan builds (the jail, `ToolEffect::Network`, `G-NO-WORKSPACE-EXEC`); it just needs to be *stated* so it can be tested. The checkpoint half is the expensive half and it is the one the acceptance test depends on: the test hands the user a modified repository, and W-04 makes the very first turn the riskiest one. **The `let _ =` on `auto_checkpoint_before_write` is the whole finding in one line** — it is the plan's own *guard-the-guard* pattern, unfixed.

---

### ADR-06-07 — Learning-loop safety: cold start, exploration, label provenance

**The call:** Under what conditions may routing affinity be switched on, and what constitutes a valid training label?

**Why it matters:** Affinity has *"never influenced a decision"* (`TRACKER.md:95`), so enabling it is new-feature risk. The algorithm is greedy argmax with no exploration term (`bandit`/`ucb`/`epsilon-greedy`/`exploration` = 0 hits corpus-wide), a `>= 3` sample gate, `n/(n+5)` confidence, and a **global** 200-row window that `F-ROU-05` shows can silently reset a worker's affinity. Cold start ties every worker at 0.5, and the stable sort returns `supervisor` (`F-ROU-02`) — so the deterministic first pick becomes the first label becomes the affinity. And labels are unverified: completion can be satisfied by a substring gate (`G10-96`), which is the very gate M-13 replaces.

**Options:**
- **(A)** Three conditions before enable: (i) an exploration floor — a small share of assignments ignore affinity; (ii) **per-worker per-task-type** windows with time decay, not a global 200-row cap; (iii) labels derived **only** from Rust-observed `build_evidence`, never from a status transition. Plus: discard existing `routing_history` at enable (R-8 already suggests it), and surface the current affinity table so a wrong one is visible.
- **(B)** Enable as-is behind the M-12 sequencing constraint the plan already has.
- **(C)** Ship `suggest_worker` static-only; defer learning entirely.

**My recommendation: (A).** R-8 and the M-12-before-M-33 constraint already fix *corrupted attribution* — this fixes *corrupted learning dynamics*, which is a different failure and survives the M-12 fix untouched. Condition (iii) is the important one and it is free: it makes M-13's correctness propagate into the learner instead of stopping at the gate. Condition (i) is a few lines and it is the only thing standing between the product and a fleet where one worker does everything because it went first. **(C)** is a defensible fallback if (A) is judged too much for this restructure — but then say so, because "affinity exists and is inert" is the status quo the plan is supposed to end.

**Also settled here:** every constant in the FB-02 register is marked **provisional**, surfaced in config, and owes a measurement at the phase gate that turns its loop on. Starting with `conf > 0.85` vs max `0.85`, which is dead code today.

---

### ADR-06-08 — The background cadence register

**The call:** Who runs each periodic job, how often, what happens on overlap, and what happens when it fails?

**Why it matters:** `"interval"`, `"cadence"`, `"how often"`, `"watcher"`, `"debounce"` = **0 occurrences** across all 13 documents. Eleven mechanisms are specified by *what* and never by *when*: embedding drain and backfill, reconciliation sweep, graph build/update, retention pruning, `VACUUM`, update check, cost rollup, cache-hit alarm, worker kick timeout, orphan release, memory growth. The plan removes today's numbers (the 1–2 minute cron becomes *"a low-frequency reconciliation sweep"*) without supplying replacements. The failure modes are already documented on adjacent loops: `G3c IN-42` — a 30 s `tokio::time::interval` with default missed-tick behaviour **burst-fires**; `G3c IN-89` — the update loop continues *"with no backoff and no counter"*; `G3c IN-77` — a background panic *"is swallowed into a JoinError nobody inspects."*

**Options:**
- **(A)** One table in spec `08` — job · owner · trigger · interval · overlap policy · failure policy · observability — and one supervisor task that owns them all, with `MissedTickBehavior::Delay`, single-flight per job, and panics surfaced.
- **(B)** Per-spec cadence sections in each owning spec.
- **(C)** Leave cadence to implementation.

**My recommendation: (A).** In a single-process desktop app these jobs compete for the same SQLite files and the same Tokio runtime, so their interactions are a property of the **set**, not of any member — which is precisely why per-spec sections (B) would miss them. One table is also the only artifact that makes "what is this app doing when I'm not looking?" answerable, which is a real question for a product that runs agents unattended. **(C)** is how the 1-minute cron reading the board 40 times a minute (`SK-33`) came to exist.

---

### ADR-06-09 — Keychain unavailability, and how CI runs a real turn

**The call:** What does the app do when the OS keychain is locked, denied or absent — and how does the CI acceptance test obtain a key?

**Why it matters:** Two consequences of one decision. ADR-03-05 chose **keychain-only at runtime**, correctly. But `G3c P-35` records that on headless Linux *"every store/get fails and there is no fallback secret store at all,"* and the plan specifies **no** runtime behaviour for that — colliding with `G-BYOK-VISIBLE`, which promises a specific actionable error for the key-missing case. Second: `G-KEY-NOENV` requires the app to refuse a turn with the keychain empty and every env var set; ADR-03-05 confines the dev `.env` to a harness *"not linked into the app"*; and `G-PLATFORM-TRIPLE` runs the end-to-end smoke — which must complete a real turn — on Linux CI, which has no keychain. **The restructure's definition of done currently has no way to execute.**

**Options:**
- **(A)** Runtime: a distinct `KeychainUnavailable` state, surfaced with the specific missing service and the fix, never conflated with "no key". CI: provision a keyring daemon on the Linux runner and write the key from a CI secret through the **app's own** `set_provider_key` path, so the smoke exercises the real code path.
- **(B)** Runtime as (A). CI: a signed mock provider behind a test-only feature flag; the smoke tests everything except a real API call.
- **(C)** Runtime as (A). CI: relax `G-KEY-NOENV` on CI builds only.

**My recommendation: (A), with (B) as the fallback if runner keyring provisioning proves brittle.** (A) is the only option where the gate tests what ships. **(C) must be rejected explicitly** — it puts an env-var credential path back into the binary for exactly the reason INV-12 exists (*"presence of a capability is not intent to use it"*), and it would be discovered later as a defect rather than remembered as a decision. If (B) is taken, say plainly that `G-PLATFORM-TRIPLE` then proves the *pipeline* and not the *provider*, and keep a periodic real-key run outside CI.

---

### ADR-06-10 — Toolchain absence and what "build-verified" means

**The call:** What happens when the user's repository needs a toolchain the machine does not have?

**Why it matters:** `G-CLEAN-INSTALL` asserts a working app on a machine with **no Rust, Node, Python or `uv`**. The headline acceptance test requires a **build-verified** change. Both cannot hold: with no toolchain, M-13 detects an ecosystem it cannot invoke, no `build_evidence` row is written, and `update_task_status` hard-refuses `qc_review` — the `SK-50` loop arriving by a different road. ADR-03-03 already chose the right adjacent answer — a declarative environment check that *"reports what is missing and never installs anything"* — and it is connected to nothing: not to the build-evidence gate, not to the acceptance test, not to first run.

**Options:**
- **(A)** A distinct `ToolchainMissing` outcome, separate from `BuildFailed`: the task pauses rather than loops, the environment check names the missing tool and the command to install it, and first run performs the check against the chosen workspace so the user learns before their first turn. Restate the acceptance test to say the clean machine has the repository's toolchain, or to use a repository whose build needs nothing extra.
- **(B)** Treat a missing toolchain as a build failure and let retries exhaust.
- **(C)** Bundle a toolchain.

**My recommendation: (A).** **(B)** is the current behaviour and it is `SK-50` — a task that loops toward exhaustion while telling the user nothing useful. **(C)** contradicts INV-1 and the 225 MB deletion that motivates the whole programme. (A) costs one enum variant, one first-run check, and one honest edit to the acceptance test — and the honest edit matters most: a test that cannot pass on the machine it names will be quietly reinterpreted rather than fixed. This ADR also gives ADR-03-03's option (C) an actual consumer, which INV-8 would otherwise flag as a capability with no reader.

---

### ADR-06-11 — Telemetry: state it

**The call:** Does AutoDev send anything anywhere other than the user's chosen LLM providers — and can the update check be turned off?

**Why it matters:** `telemetry`, `opt-out`, `phone-home`, `crash-report`, `PostHog`, `Sentry` = **0 occurrences** across the suite, and the grounding confirms no third-party SDK exists. So the answer is almost certainly *"nothing"* — which is a **feature** of a local-first BYOK tool and is currently unwritable, because one outbound request is unspecified: the updater polls every 4 hours (`G3c IN-89`) to a URL read from `AUTODEV_UPDATE_URL` with no scheme validation (`IN-19`). The plan repairs what comes back and never states what goes out.

**Options:**
- **(A)** State it: no telemetry, no analytics egress, no crash reporting. Specify the update check's exact payload (version, platform, architecture — nothing else), make it disableable in Settings, and put the statement in first run beside the BYOK line.
- **(B)** State it in docs only.
- **(C)** Leave it unstated.

**My recommendation: (A).** This is the cheapest ADR in the group and one of the few that is purely additive to the product's credibility. It also forces a small real fix — the update check's payload becomes a specified thing rather than whatever the code happens to send — and it pairs naturally with ADR-06-01's egress statement so the user gets one coherent account of everything that leaves their machine. **(C)** is the status quo, and the cost of the status quo is that nobody can answer the question truthfully.

---

## 9 · Negative results — checked, and the plan handles these

Listed so the positive findings can be trusted. Each of these is something I expected to be missing and was not.

| # | Expected gap | What the plan actually does |
|---|---|---|
| 1 | `analyze_diff`'s unprompted repo upload goes unnoticed | **Found and named as the sharpest finding in spec `06`.** `ToolEffect::Network` becomes a first-class effect with the right reasoning; `G-PERM-NETWORK` is a gate that can fail. Only the *policy* and the autonomous-mode collision are missing — the mechanism is there |
| 2 | Keys readable from the webview | **ADR-03-01 is exactly right.** Asymmetric by construction: inbound `set_provider_key`, **no** outbound command; `get_provider_key`/`get_github_pat` removed; `G-KEY-ASYM` is a static check. INV-2 states it as an invariant |
| 3 | A dev `.env` becomes a runtime fallback | **INV-12 + ADR-03-05 + `G-KEY-NOENV`** — a gate that fails if the app runs a turn with the keychain empty and every env var set. The reasoning (*"presence of a capability is not intent to use it"*) is stated and applied |
| 4 | Memory has no provenance | **`source` pulled forward a whole phase** from anchor Phase 4, on the correct argument that it is one column now and a migration later. Six values, with file/line where applicable |
| 5 | Memory recall leaks across agents and projects | **Scopes are mandatory retrieval filters** (ADR-05-03), with `G-MEM-SCOPE` and `G-MEM-SCOPED`. Closes the observed `agent_id`-never-forwarded contamination class structurally |
| 6 | Agents write whatever they like into memory | **ADR-05-04 removes `remember_this`'s unrestricted write**; agents propose, a deterministic gateway decides on importance/novelty/duplication/confidence. The best structural decision in the memory workstream |
| 7 | Unbounded recursion, fan-out or tool loops | **All three bounded:** subagent depth cap with `depth+1` from a real `ToolContext` (`G-DEPTH`), semaphore pool with a `Drop`-released permit (`G-POOL`), `MAX_TOOL_TURNS`, and INV-9's timeout-everywhere rule |
| 8 | Cancellation is a boolean nobody checks | **Spec `01 §3.3`:** a `CancellationToken` threaded into the provider call, the tool executor **and** every pending-permission wait, each with a timeout. `G-CANCEL` is parameterised over all three |
| 9 | The updater keeps installing unverified binaries | **INV-11 + ADR-03-04:** absence of a signature is a **hard failure**; verification re-runs at install; HTTPS enforced; filenames sanitised; the never-working delta path deleted rather than patched |
| 10 | Local RCE surfaces survive | **Both removed by decision.** The unauthenticated `/mcp` server is deleted (ADR-03-02); the plugin system is reduced to verify-don't-install (ADR-03-03 (C)), which removes the `rustup self uninstall` class outright |
| 11 | MCP client shipped without a permission story | **Deferred with the reason stated** — no permission model exists for third-party tools, and admitting them would mean changing the agent core path. The false UI claim is deleted immediately. A genuinely disciplined deferral |
| 12 | Retention is left as "someday" | **OQ-5 decides a policy per table.** Only the cadence is missing (OPS-04) |
| 13 | Affinity trains on corrupted attributions | **Caught: R-8**, plus the hard sequencing constraint *M-12 before M-33* in Part 6 and *"consider discarding existing `routing_history`."* FB-01/02/03 are the next layer, not a repeat |
| 14 | Caching is adopted and silently does nothing | **Caught: R-2**, with the correct instruction *"build the cache-hit alarm before the cache"*, plus `G-CACHE-HIT`, `G-CACHE-STABLE`, `G-CACHE-MIN`. Only the alarm's threshold is unstated |
| 15 | The escalation queue starves the fleet | **Caught:** `04 §5` names the no-expiry starvation, and `G-ESCALATION-LIVENESS` gates it |
| 16 | Decided ADRs quietly contradict each other | **Spec `04 §1` catches ADR-02-06 vs ADR-03-02** — preserving ACP messaging while deleting the server that supplies sender identity — and turns it into a sequencing constraint. The plan already performs this kind of adversarial self-read; GAP-03 is the same class, not yet found |
| 17 | Multi-user isolation | **Correctly not a requirement.** Per-OS-user data root and keychain give it for free. Only the flat keychain service name needs a line (GAP-09) |
| 18 | An overturned recommendation gets quietly dropped | **Kept visible in place** — ADR-01-07 and ADR-05-02 both record the overturn, and ADR-05-02's struck answer is preserved with the propagated 1536→384 correction called out. The amendment log is honest about its own errors |

---

## 10 · Suggested disposition

1. **Answer ADR-06-01, -02, -05 and -06 before P2.** They change what gets built in the provider, memory and tool layers, and three of them add gates (egress consent, redaction chokepoints, checkpoint-before-write) that are expensive to retrofit.
2. **Add M-40 (redaction) to Part 1B and extend P8's entry gate to ten capabilities.** This is the single change most likely to be lost otherwise, because a capability with no mechanic never reaches the MDP.
3. **Answer ADR-06-09 and -06-10 before P9 planning**, since both determine whether `G-PLATFORM-TRIPLE` and the headline acceptance test can execute at all.
4. **Fold ADR-06-08's cadence table into spec `08`** and harvest it into `TESTING_GATES.md` the same way the test families are harvested.
5. **Reconcile spec `07 §7.2` with spec `03 §4.4`** (W-01) as a documentation no-stale fix — one flow, one authority, with the model-download step in it.
6. **Restate the headline acceptance test** once ADR-06-10 lands, so the definition of done is a sentence that can be true.
