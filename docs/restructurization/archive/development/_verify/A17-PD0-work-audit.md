<task-notification>
<task-id>adbcf37d430bd4b06</task-id>
<tool-use-id>toolu_01PA2Xn3jqWf7zsoucms2g3V</tool-use-id>
<output-file>C:\Users\RAZIAS~1\AppData\Local\Temp\claude\C--AIRS-Foundry-autodev\b0d308fa-5fa4-4bdf-b8ec-d366061e88e8\tasks\adbcf37d430bd4b06.output</output-file>
<status>completed</status>
<summary>Agent "PD0 work audit" finished</summary>
<note>A task-notification fires each time this agent stops with no live background children of its own. The user can send it another message and resume it, so the same task-id may notify more than once.</note>
<result>I could not create the report file: I have no file-writing tool in this session, and my operating rules for this audit forbid creating files (including via shell redirection). The full report follows inline.

---

# PD0 · WORK AUDIT — the capability contract

**Scope.** `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md §10.5.0`, `src/design/contract/*`, `docs/restructurization/public-facing/AUTODESIGN-CAPABILITY-CONTRACT.md`, `TESTING_GATES.md §6.5` + `gates.test.ts`, and the tracker family.
**Method.** Every "does not exist" below is a search of the tracked tree (ripgrep/grep over `src/`, `docs/`, `.github/`), never a sample. Commits inspected: `1cb8b9a` (the phase), `1311e65` (brief), `a03d7ed` (CI).
**One scope limit, stated up front:** every AutoDesign-side citation (`invocation.ts:35-39`, `artifacts.ts:4-21`, `lifecycle.ts:80-85`, `sourcePdf.ts:170`, `capabilities.ts:37`, `createLoopbackServer.ts:726`) points into a repository that is **not on this machine** — `C:/AIRS Foundry/apps` holds `airs-rnd-hub`, `llmcalculator`, `mempalace-main`, `package-scanner`, `pdftopsd`, `psdconversions` and no auto-design. Those line references are unverifiable here; where a claim about them is *internally* contradicted by shipped code, I say so (W-5).

---

## W-1 · `PD0-07` — "the contract states who executes" — is not in the contract · **FALSE**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1542` — `- [x] **PD0-07** 🔴 **The contract states who executes.** … Write it into the contract so the consumer does not have to guess. → \`DOC-GATE\` *(Written into the contract document's §1 and §3 rather than left to be inferred: the PA executes design work in Rollout 1, the fleet becomes an additional consumer in Rollout 2, and approval stays AutoDesign's to enforce before they spawn us.)*`

**What I found:** searched `AUTODESIGN-CAPABILITY-CONTRACT.md` and the whole of `src/design/contract/` for, separately: `\bPA\b`, `personal assistant`, `assistant`, `fleet`, `worker`, `supervisor`, `who (executes|drives|runs)`, `executes design`. **Zero hits in either.** The single occurrence of "Rollout" in the document is about PSD bytes, not about an executor:

- `docs/restructurization/public-facing/AUTODESIGN-CAPABILITY-CONTRACT.md:243` — `for Rollout 1 — the PSD arrives at ours — but it will matter at export.`

Document §1 (`:17-34`) is *"Why a sidecar, and not a service"* and names no executor. §3 (`:79-114`) is the operations table plus the mapping; the only clause of the three that landed is the approval one:

- `AUTODESIGN-CAPABILITY-CONTRACT.md:113` — `**Approval stays yours to enforce, before you spawn us.**`

**Verdict and consequence:** the task's whole purpose — *"the phases define capabilities and never say who drives them, which left AutoDesign to infer it"* — is unmet. Two of the note's three assertions describe text that does not exist in the artifact they name. The evidence class is `DOC-GATE` (a static check over document text), and the only gate that reads this document, `TGD0-07`, checks operations, outcomes, the reserved verb and exit codes — so nothing in the register would ever notice. **This is a checkbox flipped with no proof of any kind.**

---

## W-2 · `SD0-01`'s nine-case spawn smoke is not reproducible, and its sharpest clause was never run · **UNVERIFIABLE + OVERSTATED**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1549` — `*(**\`SMOKE-PROVEN\` 2026-09-03** — the sidecar spawned as a real child process, nine cases. All four operations settle with exit \`0\`, four events each, terminal event last and decodable. All five malformed inputs — bad JSON, an unknown verb, the reserved verb, an unknown schema version, an undeclared field — exit \`2\` with **nothing on stdout**…)*`

**What I found:**

1. **Exactly one child process is spawned anywhere in the tracked tree.** Searching `src/` for `execFileSync|spawnSync|child_process` returns two lines, both in the gate: `src/design/contract/gates.test.ts:11` and `:134` — `const stdout = execFileSync('node', [SIDECAR], { input: request, encoding: 'utf8' })`, and it runs **one** case: `const request = JSON.stringify(requestFor('ingest'))` (`gates.test.ts:133`). No script under `scripts/`, no npm script (`package.json:5-17` has no smoke/conformance entry), no harness in `_verify/`.
2. **The five refusals are tested in-process, not spawned, and only two of the five shapes exist.** `contract.test.ts:232-251` drives `runInvocation(...)` with an injected `SidecarIo` sink for bad JSON (`'{not json'`) and an unknown verb (`operation: 'sculpt'`). The reserved verb, unknown schema version and undeclared field have no invocation-level test at all — only decoder-level ones.
3. **The killed-child clause was never exercised, and the note's own arithmetic proves it.** `SD0-01`'s text (`:1549`) requires three no-fire halves, the third being *"a **killed child leaves no partial output claiming success**"* — added by `ADR_LOG.md:1681-1686` precisely to make the smoke falsifiable. The note enumerates 4 operations + 5 malformed inputs = the stated nine. **The kill case is not among them.** Searching `src/design/` for `INTERRUPTED|SIGTERM|SIGINT|130` returns only the production sites (`main.ts:40-43`, `sidecar.ts:53`) and one string comparison in a *document* check (`gates.test.ts:187`).

**Verdict and consequence:** the register's own §1 calls smokes *"throwaway"*, so the absence of a committed script is not by itself a rule break — but 8 of the 9 claimed cases cannot be reproduced from this repository, and the one clause the ADR added to stop the smoke being satisfiable by a constant was never run. The only thing proven about exit code `130` in this repository is that the string `**130**` appears in a markdown file.

---

## W-3 · `PD0-06` is marked `SMOKE-PROVEN`; its proof is a unit test driving a unit test · **OVERSTATED**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1541` — `⚠ **\`SMOKE-PROVEN\` means the reference implementation is driven by something that is not its own unit test** — the conformance suite is that something.`
Register definition it is being scored against, `TESTING_GATES.md:22` — `| \`SMOKE-PROVEN\` | Proven by the end-to-end smoke driving the real app |`.

**What I found:** `runConformance` has exactly four call sites, all inside vitest files: `contract.test.ts:29`, `:41`, `:51` and `gates.test.ts:199`, `:205`. There is no CLI, no npm script and no non-test caller anywhere in `src/`. So the "something that is not its own unit test" is itself started by `contract.test.ts:28-33`.

**Verdict and consequence:** the class as the register defines it is not met — nothing drives "the real app". Under the plan's own inline redefinition it is met only one level deep: the suite is not *reference.ts*'s unit test, but it is *a* unit test's payload. This is the exact question §0.4 exists to answer, and the honest class here is `UNIT-PROVEN` (which is what `PD0-02`…`PD0-05` claim for weaker work).

---

## W-4 · "The IR gained the version it never had" — the IR did not · **FALSE (as stated)**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1532` — `*(\`TGD0-02\`/\`TGD0-03\`, both falsified. **The IR gained the version it never had**, and the wire carries \`{schemaVersion, ref, sha256, byteLength}\` — never the document…)*`

**What I found:** `src/design/ir.ts` is **not in the phase's diffstat** (`git show --stat 1cb8b9a` lists 18 files; `ir.ts` is not one). It still reads:

- `src/design/ir.ts:155-164` — `export interface DesignDocument { width: number; height: number; sourceName: string; layers: DesignLayer[]; fonts: FontRequirement[]; assetIds: string[] }` — and `grep -n "version" src/design/ir.ts` returns one hit, a comment about PSD opacity at `:96`.

What gained a version is the *envelope*: `version.ts:48` `IR_SCHEMA_VERSION`, checked at `decode.ts:155` against the `DesignRef`'s field. Nothing binds the constant to the type it names — `version.ts:46` is a comment (*"Bump this when `DesignDocument` changes shape"*), and the pinning test is deferred to PD1 (`:1533`).

**Verdict and consequence:** the amended task ("the wire carries a reference") *was* built and is genuinely gated. The completion note's headline sentence is not true of the IR: a `DesignDocument` whose shape changes still decodes under `autodev-design-ir/0.1.0`, because the version is asserted about the pointer and never about the pointee. A partner team reading `:1532` would believe the document behind `ref` is version-checked. It is not.

---

## W-5 · "mirrors `artifactReferenceSchema` field-for-field, so a decoder they already own reads ours" · **FALSE, and self-contradicted in the same commit**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1539` — `*(Artifact references mirror AutoDesign's \`artifactReferenceSchema\` **field-for-field**, so a decoder they already own reads ours.)*` Republished to the partner team at `AUTODESIGN-CAPABILITY-CONTRACT.md:211-212` — `the field set **mirrors your \`artifactReferenceSchema\`** so a decoder you already have can read ours`.

**What I found:** the plan's own quotation of their schema lists **seven** fields (`:1540`): *"`artifactReferenceSchema` (`artifacts.ts:4-21`) carries `artifactId`, `mediaType`, `byteLength`, `sha256`, `storageRef`, `createdAt` and `expiresAt`"*. Ours carries **nine** (`types.ts:109-122`), adding `contractVersion` and `artifactKind` — and `artifactKind` is deliberately a different *type* from theirs:

- `types.ts:112` — `/** Free-form, unlike AutoDesign's closed enum — see the note above. */`

And the phase's own decoder file states the property that makes the interop claim impossible:

- `decode.ts:16-17` — `AutoDesign's package marks every object \`.strict()\` for the same reason, and keeps a \`fixtures/v0.2.0/invalid/\` corpus to prove it.`

**Verdict and consequence:** a `.strict()` decoder rejects undeclared keys by definition, so "a decoder they already own reads ours" cannot be true of a payload carrying two fields their schema does not declare — and even if extra keys were tolerated, a free-form `artifactKind` cannot pass a closed five-member enum. This is not a cross-repo unknown: both halves of the contradiction are in files this phase wrote. It is repeated to the partner team in the document they cannot ask questions about.

---

## W-6 · The plan claims eight gate falsifications; the gate register and the matrix record four · **OVERSTATED**

**The claim:** the completion notes assert falsification for every gate — `:1530` `(\`TGD0-01\`, falsified…)`, `:1532` `(\`TGD0-02\`/\`TGD0-03\`, **both falsified**…)`, `:1537` `(\`TGD0-04\`/\`TGD0-05\`, **both falsified**…)`, `:1541` `(\`TGD0-08\`, falsified…)`, `:1544` `(\`TGD0-06\`, falsified…)`, `:1545` `(\`TGD0-07\`, falsified…)`.

**What I found:** `TESTING_GATES.md §6.5` records a plant and a failure message for exactly four — `TGD0-02` (`:388`), `TGD0-04` (`:392`), `TGD0-06` (`:396`), `TGD0-07` (`:399`). The other four carry the bare class: `TGD0-01` `:386`, `TGD0-03` `:390`, `TGD0-05` `:394`, `TGD0-08` `:401` all end `**Evidence: \`GATE-PROVEN\` 2026-09-03**` with no falsification. `READINESS_MATRIX.md:206` says `8 new gates (\`TGD0-01\`…\`08\`) all in \`npm test\`, **4 of them falsified**`.

**Verdict and consequence:** two of the three trackers say four; the plan says eight. Falsification is this programme's strongest evidence currency (`L-25`, `L-26`, `L-27` all exist because unfalsified gates were green over live defects), and the plan inflates it 2×. One of the three documents is wrong on the record and the phase closed without reconciling them.

---

## W-7 · The published exit-code table promises a state no shipped code can reach; `complete` and `held` have no producer · **TRUE finding (doc is wrong about the product)**

**The claim:** `AUTODESIGN-CAPABILITY-CONTRACT.md:54` — `| **0** | A terminal \`settled\` event was emitted. **Read stdout for the outcome** — a refusal is still a successful invocation. |` and `:59-62` — `⚠ **\`0\` does not mean "the design converted".** … A \`held\` outcome — we refused your input — exits \`0\`…`

**What I found:** grepping `reference.ts` for `'held'`, `"held"` and `'complete'` returns **no matches**. Both settle branches hard-code one outcome — `reference.ts:143` `outcome: 'partial',` and `reference.ts:166` `outcome: 'partial',`. Every real refusal in the shipped sidecar happens before any event is emitted and exits `2`: `sidecar.ts:78-80` and `:88-90` both `return EXIT.PROTOCOL` after writing to stderr, with nothing on stdout.

**Verdict and consequence:** there is no reachable path in the shipped sidecar that produces exit `0` with a `held` outcome, or any `complete` outcome at all. Of the three outcomes the contract freezes, exactly **one** has a producer. This is the failure mode `PD0-04`'s own amendment names: `:1538` — *"`ErrorClass` in `provider/usage.rs` sat for months 'defined, unit-tested against itself, and never produced by anything'. **Every variant here needs a producer or it should not exist.**"* The same holds for seven of nine `DIAGNOSTIC_CATEGORIES`, for `severity: 'failure'`, for `retryable: true`, for `method: 'rendered-comparison'` and for `EXIT.UNSETTLED`. A consumer writing the `exit 0 + held` branch today is writing dead code, while the branch they actually need — `exit 2` — is documented as *"the request never decoded"*, which is exactly what a refusal is.

---

## W-8 · `TESTING_GATES.md` gained eight gates and updated none of its own registers · **STALE (§0.3)**

**The claim (implicit):** the phase's exit line, `MASTER_DEVELOPMENT_PLAN.md:1553` — `**Exit.** \`SD0\` → \`TG-PD0\` → matrix → progress → no-stale → commit.`

**What I found** (the phase's diffstat shows `TESTING_GATES.md | 33 +++` — §6.5 only, nothing else):

- `TESTING_GATES.md:9` — `**115 gates across P0–P9.** This document is the definition of done for the programme.`
- `TESTING_GATES.md:39-51` — the §2 phase index has **no PD0 row** and still totals `| | **Total** | **115** | |`.
- `TESTING_GATES.md:681` — `**Spec gate totals:** 115 named in the specs − 8 merged = **107 surviving** + **8 NEW** = **115**.` — no `TGD0` line in §14.
- `TESTING_GATES.md:742` — `the no-sidecar family's real weight is that **P8 re-runs all 115 gates**`.
- §17's test-family table (`:727-738`) contains no `TGD0` id; §15's risk table none; §20's amendment log ends at `2026-08-31` (`grep -c "2026-09-03" TESTING_GATES.md` = 5, all inside §6.5).

**Verdict and consequence:** the gate register's headline number, its phase index, its traceability arithmetic, its family coverage and its amendment log all still describe a 115-gate, PD0-less document. The one register whose job is to be the definition of done is stale about its own contents.

---

## W-9 · The plan's `§5.6` throughput totals were never recomputed for PD0 · **STALE**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:789` `### Recalibrated remainder — 319 open tasks`; `:802` `| **PD0** Contract | ~~7~~ **10** | ~1 h | **done** | — |`; `:810` `| **Rollout 1** | **236** | **≈23.5 h** | **≈39.5 h** | |`; `:816` `| **Programme** | **319** | **≈37.5 h** | **≈53 h** | |`.

**What I found:** the table's own rows sum to `1 + 65 + 0 + 21 + 27 + 15 + 33 + 13 + 10 + 44 = 229`, not 236 — the difference is exactly PD0's pre-amendment **7**. Rollout 2 (`21+28+18+16 = 83`) is right, so the programme total inherits the same 7. The public brief carries the corrected arithmetic: `AUTODEV-PROGRAMME-BRIEF.md:148-149` — `**Totals.** 229 tasks and roughly **38 hours** to complete Rollout 1; 83 tasks and roughly **14 hours** for Rollout 2. **312 open tasks** across the programme.`

**Verdict and consequence:** the plan (319) and the public brief (312) disagree by seven tasks and 1.5 hours, and the *authoritative* document is the wrong one. Worse for the reader: the plan tells them PD0 is "done" in one column while still counting it as open in the totals two rows below.

---

## W-10 · The public brief carries a second, stale Rollout-1 table the plan says "is corrected" · **STALE, and the plan's correction claim is FALSE**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:840-842` — `The public brief's phase table nonetheless listed it **last, after \`P4\`**, which reads as the contract arriving at the very end — the opposite of its intent. That was a presentation defect and **is corrected**.`

**What I found:** the brief has **two** Rollout-1 tables. The updated one at `:85-103` is correct (PD0 = 10 tasks, third row, `M1/M2/M3`). The one at `:121-137` was left behind untouched:

- `AUTODEV-PROGRAMME-BRIEF.md:137` — `| 2 | **PD0** Contract | The integration surface, published early | 6 | **~1 h** |` — **still listed last, after `P4` at `:136`**, and still 6 tasks.
- `:128` `| 1 | **P8** Sidecar removal | … | 24 | **~4 h** |`, `:129` `| 🤝 | **Handover** | **The AutoDesign team can begin wiring** | — | **~5 h away** |`, `:130-135` PD1=14, PD2=14, PD4=13, PD6=9 — every pre-2026-09-03 number, plus the single "Handover" milestone `§5.6` says was replaced by three.

`AUTODEV-PROGRAMME-BRIEF.html` has three `&lt;table&gt;` elements total and no such duplicate (`:402` carries the 10-task PD0 row).

**Verdict and consequence:** the defect the plan reports as fixed is still on the page, in a document published to a partner team; and the `.md` and `.html` now disagree on PD0's task count (6 vs 10) and on whether handover is one milestone or three. The brief's *own* totals line (`:148`) sits directly beneath the stale table and contradicts it.

---

## W-11 · "live stub" survives in six places after `PD0-06` struck it — including the milestone the ADR explicitly corrected · **STALE (§0.3)**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1541` — `- [x] **PD0-06** 🔴 **RE-POINTED.** ~~A **live stub** answers every operation over a transport~~ → **an executable contract**…` and `ADR_LOG.md:1697-1700` — `What it does correct is the milestone wording I wrote on 2026-09-03, which said AutoDesign *"can wire and test against the stub"*. **They cannot.** M1 delivers **an agreed, exercisable seam**.`

**What I found, after the phase closed:**

- `MASTER_DEVELOPMENT_PLAN.md:709` — `| **PD0** | R1 | The capability contract is published and answered by a **live stub** | P2 …|`
- `:685-686` — `3. A running AutoDev that answers the contract with **stubs** — every operation returns a valid, correctly-shaped response before it does the real work.`
- `:688-689` — `a contract with a live stub behind it is an integration they can build and test against on day one`
- `:825-826` — `What \`PD0\` delivers is a **contract plus a live stub** (\`PD0-06\`)`
- `:833` — `| **M1 · Contract** | Build and test their integration against a **live stub** answering all four verbs | **\`PD0\` only** | **≈1 h** |`
- `AUTODEV-PROGRAMME-BRIEF.md:172` and `AUTODEV-PROGRAMME-BRIEF.html:447` — `stands up a **live stub that answers all four**`; `.md:330` / `.html:636` — `with a published contract and live stubs so they can build in parallel`.

**Verdict and consequence:** the correction was logged in the ADR and applied to exactly one line (`:1541`). The phase table, the handover section, the M1 milestone row and both public renderings still promise a *listening stub* that Razi ruled against building and that does not exist. This is the single most repeated stale claim in the family, and it is the one a partner team reads first.

---

## W-12 · `§5.3`'s handover list still says the contract is "written to AutoDesign's own versioned shape" · **STALE**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:683` — `2. The **capability contract** (\`PD0\`), written to AutoDesign's own versioned shape.`

**What I found:** the phase established the opposite and shipped on it — `version.ts:12-17` — `We inherit the convention, not their types. Their \`capabilityIdSchema\` is a closed \`z.enum\` of eighteen AutoDesign capabilities and their host manifest declares \`productOwner: z.literal('airs_auto_design')\` — so AutoDev cannot publish a conforming manifest, and "write our contract to their shape" **was never executable**.` `NOVELTY_LEDGER.md:188` records the same under `airs_rejection`.

**Verdict and consequence:** a line describing the handover deliverable states the exact proposition the phase refuted, in the section a reader consults to know what AutoDesign receives.

---

## W-13 · "The register currently holds six rows" — it holds twenty · **FALSE, propagated four times**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1546` — `**PD0-10** … the register currently holds **six rows** and **none for the design programme**`, repeated in the completion note on the same line (`which had **six rows and none for design**`), in `ADR_LOG.md:1688-1691` (`The register holds six rows and **none for the design programme**`), and originating at `_verify/PD0-CORRECTIONS.md:89` — `\`NOVELTY_LEDGER.md\` holds 6 rows; none covers design.`

**What I found:** `grep -n "^### N-" NOVELTY_LEDGER.md` returns **twenty** rows before this phase — `N-01` (`:28`) through `N-20` (`:171`) — each a full row with source, adoption, rejection, proof and status. `N-21` is at `:181` and its id does not collide. The "none for design" half is true.

**Verdict and consequence:** a factual claim about a tracker in this repository, wrong by 3.3×, copied verbatim into three governance documents without re-checking — in a phase whose method rules are `§0.2L-3` (*"search for the string; do not sample"*) and `§0.2m` (re-verify in the main session, do not adopt from an agent). It does not change the decision to open the row, but it is exactly the class of unchecked propagation the register exists to prevent.

---

## W-14 · `TGD0-04`'s register entry promises a clause the gate does not assert · **OVERSTATED**

**The claim:** `TESTING_GATES.md:391` — `**A fidelity report cannot claim a score nothing measured.** \`method: 'unmeasured'\` must carry a \`null\` score; **any other method must carry a real one.** The honest report is accepted…`

**What I found:** `gates.test.ts:82-102` asserts two things — `{score: 0.99, method: 'unmeasured'}` throws `/null score/`, and `{score: null, method: 'unmeasured'}` does not throw. `'rendered-comparison'` never appears in `gates.test.ts`. The second clause is proven only by a unit test that may be freely rewritten: `contract.test.ts:165-172` (`'refuses a measured fidelity with no score'`).

**Note in the same family:** `TGD0-05` (`gates.test.ts:105-120`) has no positive control — a decoder that refused *every* `partial` would pass it. Its register entry claims no more than it asserts, so this is not a mismatch, but it is the only gate of the eight without the discrimination half that `TGD0-03` and `TGD0-04` both carry deliberately.

**Verdict and consequence:** the register entry describes a two-clause gate; one clause lives outside the gate boundary the section's own preamble draws (`gates.test.ts:4-6`: *"Those are unit tests: they may be rewritten freely… These encode promises made to a partner team"*).

---

## W-15 · `TGD0-06` runs against a prebuilt artifact nothing rebuilds, and its class was recorded before CI could be green · **UNVERIFIABLE + flipped ahead of proof**

**The claim:** `TESTING_GATES.md:396` — `**Evidence: \`GATE-PROVEN\`, falsified** by suppressing the terminal event — fails naming *"settled"*`; class definition at `TESTING_GATES.md:23` — `| \`GATE-PROVEN\` | A blocking CI job that can fail is green |`.

**What I found:**
1. The gate reads a build output, not source: `gates.test.ts:22` `const SIDECAR = 'dist-sidecar/design/contract/main.js'`, and `dist-sidecar/` is git-ignored (`.gitignore:5`). `npm test` is `vitest run` (`package.json:13`) and rebuilds nothing. So a plant in `reference.ts` reaches this gate **only if** the planter also ran `npm run build:sidecar` — nothing in the harness enforces or records that. The falsification may be genuine; the repository cannot tell, and the same harness makes a *false* falsification (plant, run, see red for the wrong reason — or plant, run, see green) equally easy.
2. The CI step that makes the artifact exist landed in a **later commit than the closeout**: `1cb8b9a` ("PD0 closed. 10 tasks, 8 gates…") does not touch `.github/`; `a03d7ed` ("ci: build the design sidecar before the unit tests") adds it, and its own comment admits the sequence — `.github/workflows/ci.yml:143-146` — `🔴 Caught by reading this file, not by a run: the gate was written and committed while CI had no build step, so it would have failed every run from the moment it landed`.

**Verdict and consequence:** at the moment all eight gates were recorded `GATE-PROVEN` — "a blocking CI job that can fail is green" — the blocking CI job would have been **red**, by the phase's own account. It was corrected in the next commit, which is the right outcome, but the class was written ahead of its proof. On the current tree the ordering is correct (`ci.yml:152-156`).

---

## W-16 · `SUPPORTED_VERSIONS` — exported, documented as a drift-preventer, read by nothing · **L-C12 inside the phase that counts L-C12 instances**

**The claim:** `version.ts:53-55` — `⚠ Exported as data **so a decoder cannot drift from the list**, and so a consumer can negotiate rather than guess. A hand-maintained \`if\` chain in each decoder is how one family silently accepts a version the others reject.`

**What I found:** searching `src/` for `SUPPORTED_VERSIONS` returns two lines — its definition (`version.ts:57`) and its re-export (`index.ts:45`). **No reader, in production or in any test.** Every decoder does exactly what the comment says it prevents: `decode.ts:152`, `:155`, `:179`, `:262`, `:316` each call `literal(...)` against an individual constant.

**Verdict and consequence:** a comment asserting a mechanism the code does not have, in the file whose sibling gate preamble says (`gates.test.ts:7-9`) *"None asserts that a type exists — `L-C12` has eight instances in this programme, and a contract is the easiest possible place to add a ninth."* This is the ninth.

---

## W-17 · `PD0-09` owes "a worked example per operation"; the document has one · **OVERSTATED**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1545` — `**PD0-09** … the four operations, the payload, the error taxonomy, the asset rules, the invocation model, and **a worked example per operation**.` The completion note quietly reduces this to `a worked example` (singular).

**What I found:** `AUTODESIGN-CAPABILITY-CONTRACT.md:247-264` — `## 8 · A worked example` — one request/response pair, for `adapt`. `ingest`, `export` and `change` have none. The omission matters most for `export`: it is the only operation whose settled event carries `artifacts` and a `null` fidelity (`reference.ts:139-160`), i.e. the one shape the single example does not show.

**Verdict and consequence:** the task shipped short of its written scope and the note elides the difference. `TGD0-07` parses only the operations table, so nothing detects it.

---

## W-18 · The published `§10` misdescribes what the conformance suite checks · **OVERSTATED (to the partner team)**

**The claim:** `AUTODESIGN-CAPABILITY-CONTRACT.md:302-307` — `It checks that every operation settles exactly once and last … and, in the half that matters, that **an implementation cannot** claim a fidelity score it never measured, emit an unevidenced \`partial\`, return output on a \`held\`, parse an unknown schema version, or silently ignore a field it does not know.` Same sentence in `MASTER_DEVELOPMENT_PLAN.md:1541`.

**What I found:** the four driving checks do drive `driver.run(...)` (`conformance.ts:85-125`) — those are real. The five "no-fire" checks do not touch the driver at all. Each builds a fixture and calls **our** decoder: `conformance.ts:137` `decodeSettled({ ...validSettledSkeleton(), fidelity: {score: 0.97, method: 'unmeasured', …} })`, and likewise `:151`, `:162`, `:178`, `:192`. `validSettledSkeleton()` (`:216-230`) is a local constant.

**Verdict and consequence:** run against AutoDesign's own stand-in, those five checks pass **regardless of what their implementation does** — they are a self-test of `decode.ts` shipped inside a suite advertised as testing the implementation. The property is partially covered by the per-operation `the terminal event decodes` check (`conformance.ts:97-106`), which does put the driver's output through `decodeSettled` — so the claim is not empty, but "the half that matters" is not the half that is doing the work, and a consumer who reads `§10` will over-trust a green report.

---

## W-19 · "AutoDev has no HTTP server" — the shipped tree dials two loopback servers it starts · **OVERSTATED (to the partner team)**

**The claim:** `AUTODESIGN-CAPABILITY-CONTRACT.md:20-22` — `**AutoDev has no HTTP server**, and adding one would put a listening socket in a local desktop application — the last one it had was an unauthenticated endpoint that was removed for being local remote-code-execution.`

**What I found:** the phase's own source says otherwise — `sidecar.ts:22-23` — `and the Python loopback **that remains** is deleted in P8.` And the shipped Rust client: `src-tauri/src/sidecar/client.rs:106-107` — `const BASE_URL: &amp;str = "http://127.0.0.1:4000";` / `const MEMORY_URL: &amp;str = "http://127.0.0.1:8000"; // memory-mcp-server base URL`, with a Redis liveness check on `127.0.0.1:6379` (`:582`).

**Verdict and consequence:** the *decision* ("we will not add one for you") is sound and unaffected. The *statement of fact* is not true of the current tree, and the reader most affected is the one the document is written for — a team sizing the security posture of a process they are about to spawn. The precise claim would be "no inbound surface an external process can reach", which is what `ADR_LOG.md:1620-1625` actually established.

---

## W-20 · `PD0-03` and `PD0-05` carry an evidence class with no named proof · **§0.4 shortfall (work exists)**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:68` — `Every task carries an evidence class (§0.4) inline, **and names the smoke check or gate that will prove it**. A task with no named proof is not done.`

**What I found:** `PD0-03` (`:1535`) and `PD0-05` (`:1539`) are both `UNIT-PROVEN` and name no gate, no smoke and no test. The work *is* covered — job ordering by `conformance.ts:85-125`, artifact lifetime by `contract.test.ts:177-207` — but the reader has to go find it. Every other PD0 task names its `TGD0` id.

**Verdict and consequence:** minor by itself; it is the pattern that makes W-1 possible, since a note with no named artifact is a note nothing can check.

---

## W-21 · MDP `§16 Change log` has no PD0 row · **STALE (the phase's own exit ritual)**

**The claim:** `MASTER_DEVELOPMENT_PLAN.md:1553` — `**Exit.** \`SD0\` → \`TG-PD0\` → matrix → **progress** → no-stale → commit.`

**What I found:** `§16` (`:2301` onward) is `*Append-only, newest first.*` and its newest rows are `2026-09-01` and `2026-09-02 / 1.9`. Searching every line after 2300 for `2026-09-03` returns nothing. `READINESS_MATRIX.md:206` did get its `2.0` row.

**Verdict and consequence:** four of the six exit steps are evidenced (gates, matrix, commit, and a partial no-stale sweep); the progress row was skipped.

---

## What held up (recorded so the negative findings are not read as a verdict on the whole phase)

- **`PD0-01`** is genuinely built and genuinely gated: `types.ts:36` freezes the four verbs, `decode.ts:245-250` refuses a reserved name *as reserved* (`"measure" is reserved for a future version and is not implemented`), and `gates.test.ts:26-43` asserts both, message included.
- **The outcome invariants are in the decoder, not the prose** — `decode.ts:298-313` — exactly as `:1537` claims, and the published table at `AUTODESIGN-CAPABILITY-CONTRACT.md:151-155` matches the code clause for clause.
- **The worked example's request decodes.** I ran `AUTODESIGN-CAPABILITY-CONTRACT.md:252` through `decodeRequest` by hand: keys pass `noExtraKeys` (`decode.ts:241`), `adapt` is not reserved, the nested ref passes all five field checks (`sha256` is 64 zeros → matches `/^[0-9a-f]{64}$/`), and the non-`ingest`+design rule at `:258` is satisfied. It decodes. The response block is explicitly `abridged` and contains `…`, so it is illustrative rather than literal — but its shapes match `reference.ts:117-123` and `:164-179` event for event, including `stage: "transform"`.
- **`TGD0-07` is genuinely non-blind.** It parses the first table under `## 3 · The four operations` and compares the **set** (`gates.test.ts:166-177`), and the register records that its first version was blind and why (`TESTING_GATES.md:398`). The heading string matches the document exactly.
- **"81 JS tests"** (`READINESS_MATRIX.md:206`) is exact: 21 + 8 + 17 + 10 + 19 + 6 = 81 across the six test files, counted statically.
- **The readiness matrix does not claim a score PD0 did not move** — `:206` says the opposite, loudly, and explains the structural gap (`PD0`–`PD6` have no scored area). That is the most honest line in the family.
- **`N-21`'s id does not collide** (`N-01`…`N-20` precede it) and its `proof_or_eval`/`airs_rejection` entries are accurate apart from the two claims already filed as W-5 and W-13.
- **`ADR-LOG-RESTRUCT-0034`'s in-repo citation is exact:** `src/design/ir.ts:155` is precisely `export interface DesignDocument {`, and the quoted field list matches `:156-163`. Its AutoDesign-side citations are unverifiable from this machine (see scope note).

---

## Summary

| # | Subject | Verdict |
|---|---|---|
| W-1 | `PD0-07` "the contract states who executes" — absent from the document and the code | **FALSE** |
| W-2 | `SD0-01` nine-case spawn smoke; kill-clause never run | **UNVERIFIABLE + OVERSTATED** |
| W-3 | `PD0-06` `SMOKE-PROVEN` rests on a unit test driving a unit test | **OVERSTATED** |
| W-4 | "The IR gained the version it never had" — `ir.ts` untouched | **FALSE (as stated)** |
| W-5 | Artifact ref "field-for-field … their decoder reads ours" | **FALSE** |
| W-6 | Eight falsifications claimed; four recorded | **OVERSTATED** |
| W-7 | Exit-0-`held` documented but unreachable; `complete`/`held` have no producer | **TRUE finding** |
| W-8 | `TESTING_GATES` still "115 gates", no PD0 in index/traceability/families/amendments | **STALE** |
| W-9 | `§5.6` totals 236/319/≈39.5 h vs brief's 229/312/38 h | **STALE** |
| W-10 | Brief's second, stale Rollout-1 table; plan says it "is corrected" | **STALE + FALSE** |
| W-11 | "live stub" in six places after `PD0-06` struck it | **STALE** |
| W-12 | `§5.3` "written to AutoDesign's own versioned shape" | **STALE** |
| W-13 | "the register currently holds six rows" — it holds twenty | **FALSE** |
| W-14 | `TGD0-04` register promises a clause the gate does not assert | **OVERSTATED** |
| W-15 | `TGD0-06` runs on an unrebuilt artifact; `GATE-PROVEN` recorded before CI could be green | **UNVERIFIABLE** |
| W-16 | `SUPPORTED_VERSIONS` — no reader, comment claims a mechanism | **TRUE finding** |
| W-17 | "a worked example per operation" — one example ships | **OVERSTATED** |
| W-18 | Published `§10` — five "no-fire" checks never touch the implementation | **OVERSTATED** |
| W-19 | "AutoDev has no HTTP server" — two loopback servers it starts | **OVERSTATED** |
| W-20 | `PD0-03`/`PD0-05` name no proof artifact | **§0.4 shortfall** |
| W-21 | No PD0 row in `§16` change log | **STALE** |

**Should anything here have blocked this phase from closing?** Yes — three things: **W-1**, a task ticked whose deliverable does not exist anywhere in the repository; **W-2**, the smoke half of `§0.5`'s "definition of done" claiming nine spawned cases of which one is reproducible and the falsifiable clause is not among them; and **W-6/W-15**, evidence classes (`SMOKE-PROVEN`, eight `GATE-PROVEN`s) recorded in a currency the other two trackers do not corroborate and that CI could not yet have paid.</result>
<usage><subagent_tokens>214690</subagent_tokens><tool_uses>69</tool_uses><duration_ms>976250</duration_ms></usage>
</task-notification>