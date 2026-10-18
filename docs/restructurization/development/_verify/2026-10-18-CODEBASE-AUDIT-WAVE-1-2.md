# 2026-10-18 — codebase audit wave 1 + wave 2 dispatch record

**Purpose:** the MDP rule is that a claim without a receipt under `_verify/` is not proven. The
owner asked for a whole-codebase audit, so this records what was actually dispatched, under what
constraints, and which load-bearing results were re-verified in the main session before being
acted on. The findings digest itself lives in
`../AUDIT_2026-10-18_CODEBASE_REFACTOR_PLAN.md` §3 (it is a proposal and is marked as such).

## Dispatches

| # | Area | Agent | Mode |
|---|---|---|---|
| 1 | agent / tool executor / permissions / journal | read-only explore | complete |
| 2 | fleet orchestration / recipes / workers / subagents | read-only explore | complete |
| 3 | persistence / migrations / schemas | read-only explore | complete |
| 4 | provider layer / observability / secrets | read-only explore | complete |
| 5 | platform, sandbox, app shell, hooks, vault, CLI | read-only explore | complete |
| 6 | frontend (React/TS), e2e, tooling, CI | read-only explore | complete |

Every brief stated: read-only (no edits, no commits, no state-changing git), no live credentials,
no billable provider calls, no network to providers, no app launch, no dev server, no wdio run
(unless the brief was the frontend one, which was additionally allowed to run the existing
read-only checks `npx eslint` / `npx tsc --noEmit` / `npx vitest run`). Each brief carried the
five findings already confirmed elsewhere so they would not be re-derived, and each was scoped to
one area with the frozen owner-WIP paths named as out of scope.

Audits 5 and 6 were dispatched together after the earlier lesson that six concurrent subagents
exhausted a provider rate limit. Audits 1–4 ran in two prior batches.

## What the main session re-verified rather than trusting

Each of these was checked against the source in this session before any action:

1. **Hook HTTP egress and hook Log path (audit 5, both high-severity).** Read
   `hooks/executor.rs` directly: confirmed `HookAction::Http` sends to a template-expanded URL with
   no `egress_consent` call, and `HookAction::Log` appends to an unjailed path while the sibling
   Shell action validates and sandboxes. **Corrected the audit's mechanism**: only the URL is
   substituted (not headers/body), and the Log `file` field is not substituted at all — so
   "POSTs file contents" is not the mechanism. Also verified by search that hooks are created only
   through the UI command path and no agent tool creates one, which is what makes the severity a
   trust *policy* question rather than a model-reachable hole. Recorded as an escalation.
2. **`iso_now` divergence (audit 5).** Read the function: it produces second-precision
   `...:ssZ`; the product's canonical `now_rfc3339` produces fixed-millisecond `...:ss.sssZ`.
   Confirmed a real divergence, fixed (`a875007`).
3. **`npm run lint` red and its blast radius (audit 6).** Ran `npx eslint . -f json`: reproduced
   95 errors in exactly the three files claimed, with the rule ids and the undefined identifiers
   per file. Confirmed the CI chain (`frontend-checks` → `build-smoke` → `e2e-smoke`) in
   `ci.yml`, then fixed and re-ran to 0 errors (`3ce44a5`).
4. **Provider-destination duplication (audit 5).** Read both copies; the consent owner also
   validates the id, so routing to it is a strict improvement. Applied (`a875007`).

## Not verified by the main session (stated as limits, not as facts)

- The `P4-18`/`P4-51`/`P4-56` status-token inconsistency is reported from reading the MDP lines;
  whether the tokens have a documented local meaning I have not yet settled, so it is an owner
  question, not a claimed defect.
- Whether any of the 56 registered-but-uninvoked Tauri commands is called by an out-of-repo
  consumer (a shipped frontend bundle or an external IPC client). A text search cannot prove that.
- Whether the two audit-5 hook behaviours are *intended*: only the absence of any consent/jail
  call is proven.

## Negative results worth recording

- The provider/observability audit found **no** secret-safety defect (bounded error bodies, keys
  in headers only, no raw provider body in errors or telemetry) — its warning is that this rests on
  authoring discipline rather than a type.
- The frontend audit **refuted** a prior audit's claim that five payload types in
  `src/types/live.ts` are orphans; they are consumed by `agentLiveStore.ts` and emitted from
  `arcana-core/src/events.rs`. Any plan citing them as orphans is stale.
- CI has no `continue-on-error` and no `-D warnings` suppression in any workflow step.
