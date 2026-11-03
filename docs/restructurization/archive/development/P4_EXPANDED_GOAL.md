# Arcana P4 Completion Goal

Continue and **finish P4 end-to-end** for Arcana according to:

- `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md`
- `docs/restructurization/development/TESTING_GATES.md`
- `docs/restructurization/development/READINESS_MATRIX.md`
- all other applicable documents under `docs/restructurization/development/`

This is no longer only a feature-completion pass.

P4 must now establish the first production-grade **reliability, recovery, observability, and failure-containment layer** for Arcana and the Fleet.

The system-design principle for this work is:

> **Production reliability does not mean failures never occur. It means failures are detected, bounded, explained, contained, recoverable, and do not unnecessarily destroy completed work.**

The desired Fleet behavior is therefore not merely:

> “Everything worked and the task completed.”

It is:

> “At every point I can see what Arcana is doing, what each agent is doing, what the system is waiting on, how long operations take, what resources are being consumed, what failed and why, what recovery action was taken, and whether useful work can continue despite that failure.”

The final state should leave **only the user's manual application-level Calendar scenario test through `npm run tauri dev`** outstanding.

---

# 1. Preserve Existing P4 Scope

Complete the original P4 goal in full:

- Source-grounded implementation.
- All applicable unit/integration/system testing gates.
- All applicable CLI and smoke tests.
- P1/P2/P3/P4/P8 cross-phase audit dispositions.
- Updated readiness documentation.
- Updated durable progress/state records.
- Independent GPT-6 Luna High review.
- Commit and push verified work to `main` under the user's prior authorization.
- Close the reopened **P8 host-enforced commit-safety task** and its associated gates.

Arcana's product goal remains:

> Arcana is a coding harness that owns deterministic development operations while using model intelligence for code and judgment; provides a strong built-in agent workflow by default with optional user tuning; and proves production consumers end-to-end.

Preserve all user decisions already recorded.

Do not reopen already-decided architecture without evidence that an implementation constraint makes the existing decision invalid.

The only previously pending P4 contract decisions remain:

1. failure-count semantics;
2. persisted user-authored artifact validation scope;
3. force-release CAS semantics;
4. DB-wide claim metadata enforcement.

If those decisions are still genuinely blocking implementation and cannot be safely inferred from existing architecture/docs/tests, isolate them clearly rather than silently inventing semantics.

---

# 2. Expand P4 Before Implementation

Before implementing the new work, update the development suite so the plan reflects the actual architecture being built.

At minimum update:

- `docs/restructurization/development/MASTER_DEVELOPMENT_PLAN.md`
- `docs/restructurization/development/TESTING_GATES.md`
- `docs/restructurization/development/READINESS_MATRIX.md`

Also update or create any appropriate supporting documents under:

- `docs/restructurization/development/`

The documentation must define:

- architecture;
- ownership boundaries;
- runtime contracts;
- failure semantics;
- recovery semantics;
- observability schema;
- instrumentation requirements;
- UI expectations;
- security requirements;
- testing expectations;
- readiness criteria;
- known limitations.

Do not let implementation outrun the documented contract.

---

# 3. Reliability and Failure-Containment Layer

P4 must explicitly handle failures across tools, workers, supervisors, persistence, connectors, and orchestration.

## 3.1 Tool Failure Handling

Every built-in tool under:

`arcana-core/src/agent/builtin_tools/`

and every shared tool execution/dispatch layer must return failures through a **consistent machine-readable error contract**.

Do not rely on arbitrary exception strings.

A failed tool operation must provide enough structured information for both:

- an agent/model to reason about what happened;
- the observability system and UI to explain the incident to a human.

Define a canonical structured error envelope containing at least:

```json
{
  "error_code": "...",
  "category": "...",
  "message": "...",
  "operation": "...",
  "tool": "...",
  "retryable": true,
  "retry_after_ms": null,
  "cause": "...",
  "suggested_action": "...",
  "context": {},
  "run_id": "...",
  "agent_id": "...",
  "task_id": "...",
  "timestamp": "..."
}
```

Expand this schema where appropriate.

Failures should distinguish categories such as:

- invalid input;
- unavailable dependency;
- timeout;
- cancellation;
- permission/security failure;
- filesystem error;
- command failure;
- malformed tool result;
- MCP/connector failure;
- model/provider failure;
- database failure;
- transient infrastructure failure;
- internal invariant violation;
- unknown/unclassified failure.

The agent must receive a useful explanation of:

1. what failed;
2. where it failed;
3. why it likely failed when known;
4. whether retry is appropriate;
5. what corrective action can be attempted.

Do not expose secrets, credentials, environment secrets, authorization headers, or sensitive payloads inside error objects or logs.

---

# 4. Agent Startup Failure and Retry Policy

If an agent/worker fails during creation or before meaningful work begins, Arcana must have a bounded retry mechanism.

Do **not** implement unbounded retries.

Use a proper retry policy based on:

- retry eligibility/classification;
- bounded exponential backoff;
- full jitter;
- maximum attempts;
- maximum delay;
- cancellation propagation;
- terminal failure state.

Conceptually:

```text
delay = random(0, min(max_delay, base_delay * 2^attempt))
```

or the closest appropriate implementation for the existing stack.

Do not retry errors that are known to be permanent, such as:

- invalid configuration;
- invalid credentials;
- unsupported operation;
- malformed request that cannot change between attempts;
- explicit policy/security rejection.

Retry events must be observable and expose:

- attempt number;
- maximum attempts;
- retry reason;
- calculated delay;
- failure category;
- eventual outcome.

The supervisor must know when a worker is:

- starting;
- healthy;
- retrying startup;
- running;
- blocked;
- degraded;
- recovering;
- failed;
- cancelled;
- completed.

---

# 5. Mid-Run Agent Failure and Continuity

A worker crashing after performing useful work must **not cause all completed work and orchestration knowledge to disappear**.

Build a durable **execution continuity and recovery system**.

The target is bounded work loss, not an impossible guarantee of zero interruption.

## 5.1 Durable Runtime State

Track sufficient information to reconstruct the worker's progress, including where applicable:

- run ID;
- task ID;
- worker/agent ID;
- parent/supervisor ID;
- assignment;
- current lifecycle state;
- started timestamp;
- last heartbeat;
- last successful checkpoint;
- active operation;
- completed operations;
- outstanding operations;
- dependencies;
- tool calls;
- relevant results;
- artifacts/files touched;
- durable artifact references;
- repository/worktree state;
- continuation/recovery instructions;
- failure information;
- retry/replacement information.

The persistence mechanism should fit the existing Arcana architecture rather than introducing an unrelated parallel subsystem.

## 5.2 Checkpoints

Introduce meaningful recovery checkpoints at safe boundaries rather than checkpointing every arbitrary token or event.

Examples:

- after a completed tool operation;
- after a completed deterministic development operation;
- after a file mutation batch;
- after a successful build/test operation;
- after a graph node transition;
- before/after externally mutating operations where appropriate.

Checkpoints must be sufficiently durable that another worker can understand what was already completed.

## 5.3 Repository State

Git may be used as **one source of recovery truth**, but do not treat `git diff` as the entire continuity system.

Capture useful repository state such as:

- current HEAD;
- branch/worktree;
- staged changes;
- unstaged changes;
- untracked files where relevant;
- files touched by the worker;
- diff or patch references where useful.

Recovery must not accidentally revert unrelated work performed by another worker.

Respect Arcana's commit-safety and concurrency rules.

## 5.4 Worker Replacement / Resume

If a worker fails mid-run, the supervisor must determine whether to:

- restart the same worker;
- resume from its durable checkpoint;
- assign the remaining work to an existing healthy worker;
- create a replacement worker;
- mark the work blocked/terminal if recovery is unsafe.

A replacement worker must receive enough structured context to distinguish:

- what is already done;
- what remains;
- which artifacts were changed;
- which assumptions were made;
- which tests were run;
- what failed;
- where continuation should begin.

Avoid blindly replaying already-successful side effects.

Where operations are not naturally idempotent, design explicit safeguards.

---

# 6. Worker Health and Fleet Supervision

The supervisor must maintain an observable state model for every worker.

At minimum expose:

- lifecycle state;
- current assignment;
- current operation;
- elapsed run time;
- last activity;
- heartbeat/health;
- retry state;
- checkpoint state;
- failure state;
- token/model usage where applicable;
- tool-call activity;
- child/subtask relationships.

The supervisor must be able to recognize dead/stale workers rather than waiting forever.

If the architecture already contains leases, claims, heartbeats, or similar mechanisms, integrate with those mechanisms instead of inventing competing semantics.

Failure handling must preserve concurrency safety.

---

# 7. Production Observability System

The current print-oriented logging system is insufficient.

Create a proper Arcana observability layer consisting of:

1. **Logging**
2. **Structured Events / Structured Logs**
3. **Metrics**
4. **Tracing**

These systems must share consistent correlation identifiers and runtime context.

At minimum support correlation through:

- `run_id`
- `task_id`
- `agent_id`
- `parent_agent_id`
- `operation_id`
- `trace_id`
- `span_id`

where appropriate.

Do not duplicate separate incompatible context models for logging, metrics, and tracing.

---

# 8. Logging

Preserve useful existing logging, but route it through a centralized logging abstraction where appropriate.

Logs should support sensible levels such as:

- trace/debug;
- info;
- warn;
- error.

Production-relevant failures must not depend solely on console output.

Logging must work for:

- application runtime;
- deterministic development operations;
- supervisor;
- workers/agents;
- graph execution;
- tool execution;
- MCP/connectors;
- memory;
- database/persistence;
- model/provider interactions;
- recovery system.

---

# 9. Structured Logs / Events

Important operations should produce structured event records rather than only human-formatted lines.

Every event should contain useful contextual fields such as:

```json
{
  "timestamp": "...",
  "level": "info",
  "event": "...",
  "component": "...",
  "operation": "...",
  "status": "...",
  "duration_ms": 0,
  "run_id": "...",
  "task_id": "...",
  "agent_id": "...",
  "trace_id": "...",
  "span_id": "...",
  "metadata": {}
}
```

Expand this schema as appropriate.

Instrument at least:

### Development operations
- command execution;
- build;
- test;
- lint;
- formatting;
- filesystem operations;
- repository/git operations;
- commit-safety operations;
- failures and retries.

### Agent/Fleet operations
- agent creation;
- assignment;
- startup;
- execution;
- handoff;
- retry;
- replacement;
- checkpoint;
- recovery;
- completion;
- cancellation;
- failure.

### Personal Agent / orchestration
Instrument applicable PA orchestration operations and delegation.

### Graph
- graph invocation;
- node start;
- node finish;
- node failure;
- edge/transition decisions where useful;
- graph total duration.

### MCP/connectors
- connection lifecycle;
- invocation;
- response;
- retry;
- latency;
- failure.

Never log credentials or sensitive request payloads by default.

### Memory
Observe:

- query/search;
- retrieval;
- write;
- update;
- deletion where applicable;
- serialization/deserialization;
- backend/database operation;
- result counts;
- latency;
- failures.

### Database
Observe database operations sufficiently to diagnose latency and failure.

Include information such as:

- operation type;
- subsystem;
- table/entity where safe;
- elapsed time;
- success/failure;
- retry;
- contention/claim behavior where applicable.

Avoid logging private values or raw sensitive records merely to gain observability.

---

# 10. Metrics

Provide runtime metrics useful for Fleet testing and future optimization.

At minimum consider:

## Model / agent metrics

- total model calls;
- model calls by agent;
- model calls by provider/model;
- input tokens;
- output tokens;
- total tokens;
- cached tokens where available;
- failed calls;
- retries;
- latency;
- time to first useful result where measurable.

## Tool metrics

- tool invocation count;
- success count;
- failure count;
- retries;
- latency;
- timeout count.

## Agent/Fleet metrics

- active workers;
- completed workers;
- failed workers;
- recovered workers;
- replacement workers;
- startup retries;
- task durations;
- idle/wait time;
- checkpoint count.

## Development-operation metrics

- builds;
- tests;
- command calls;
- command duration;
- failures;
- retry counts.

## Memory/database metrics

- reads;
- writes;
- queries;
- latency;
- failures;
- retries;
- slow operations.

## Graph metrics

- graph duration;
- node duration;
- node executions;
- node failures;
- transitions where useful.

Avoid uncontrolled high-cardinality metric labels.

Do not use raw prompts, user content, arbitrary filesystem paths, UUIDs, or full error messages as metric labels.

---

# 11. Distributed/Hierarchical Tracing

Implement tracing so a complete Arcana operation can be inspected as a hierarchy.

Example:

```text
Calendar Scenario
└── Supervisor Run
    ├── Planning
    ├── Worker A
    │   ├── Model Call
    │   ├── Tool: Read File
    │   ├── Tool: Edit File
    │   └── Test Command
    ├── Worker B
    │   ├── Memory Retrieval
    │   └── MCP Call
    ├── Validation
    └── Completion
```

Each span should capture where applicable:

- operation name;
- parent span;
- start;
- end;
- duration;
- status;
- component;
- agent;
- relevant safe metadata;
- error information.

Tracing must allow us to determine:

- where time was spent;
- which worker caused latency;
- which tool caused latency;
- where database time accumulated;
- where memory operations took time;
- how many nested operations occurred;
- where a failure originated;
- what happened immediately before and after it.

Use a recognized observability standard/library where appropriate rather than creating unnecessary proprietary tracing infrastructure.

---

# 12. Research Appropriate Libraries Before Selection

Before introducing new observability dependencies, research the current ecosystem and the existing Arcana stack.

Prefer:

- maintained libraries;
- established standards;
- minimal vendor lock-in;
- compatibility with Rust/Tauri/frontend/backend architecture;
- structured output;
- tracing context propagation;
- low runtime overhead;
- ability to export later if needed.

Strongly investigate **OpenTelemetry-compatible instrumentation** and established Rust tracing/logging libraries, but do not add a dependency merely because it is popular.

Document:

- alternatives considered;
- selected library/library set;
- why it fits Arcana;
- integration architecture;
- tradeoffs;
- export strategy;
- performance implications.

Do not add a large external observability service requirement merely to make local Fleet testing work.

Local development must remain useful without requiring paid SaaS infrastructure.

---

# 13. Observability UI

Create a usable observability interface now.

Do not defer it to P6.

The UI exists specifically so the upcoming Fleet/Calendar test can be inspected properly.

Create four major views/sections:

1. **Logs**
2. **Structured Events**
3. **Metrics**
4. **Traces**

These should be expandable/drillable.

## Logs

Support useful inspection/filtering such as:

- time;
- severity;
- component;
- worker/agent;
- run/task;
- search.

## Structured Events

Each event should be expandable to show the structured JSON payload.

The compact row should make the event understandable without expansion.

## Metrics

Provide useful operational summaries, not decorative charts.

Prioritize visibility into:

- model calls;
- tokens;
- Fleet activity;
- failures;
- retries;
- recovery;
- tool calls;
- command execution;
- memory/database latency;
- graph latency;
- overall run latency.

## Traces

Provide a hierarchical trace view.

The user should be able to inspect:

- parent span;
- child spans;
- duration;
- status;
- worker;
- operation;
- failure information.

A waterfall/timeline presentation is appropriate if it fits the current frontend architecture.

## Real-time behavior

The observability interface should update while a run is executing wherever the architecture reasonably allows.

Do not require a manual refresh to understand a currently running Fleet operation unless a documented platform constraint makes streaming impractical.

---

# 14. Log Rotation and Retention

Observability must not allow local logs to grow without bounds.

Implement a retention/rotation policy appropriate for Arcana's local runtime.

Default requirement:

- keep logs for no longer than **7 days**;
- keep at most **5 rotated log files per applicable log stream/process**, unless the selected logging architecture has a more appropriate equivalent;
- apply size-based protection as well if supported/necessary so a single pathological day cannot create an enormous file.

The effective rule should remove a log when it exceeds either retention policy rather than allowing one limit to defeat the other.

Rotation/cleanup must itself fail safely and must never crash Arcana merely because an old log cannot be deleted.

Document exact behavior.

Structured observability storage must also have bounded retention.

---

# 15. Security Hardening

Add a security layer appropriate to the new runtime and observability architecture.

At minimum address:

## Secrets and sensitive data

Never intentionally record:

- API keys;
- bearer tokens;
- authorization headers;
- passwords;
- session credentials;
- private environment variables;
- raw secrets.

Provide centralized redaction/sanitization where appropriate rather than relying on every call site to remember manually.

## Structured payloads

Tool/model/MCP/database payloads may contain sensitive information.

Prefer metadata and explicitly permitted fields over indiscriminately serializing entire requests/responses.

## UI safety

Treat logged/tool-generated content as untrusted data.

Do not create an injection/XSS path through the observability UI.

## Filesystem safety

Ensure log paths and observability storage cannot be abused for arbitrary path traversal or unsafe writes.

## Command observability

Do not expose secrets embedded in command arguments/environment variables.

## Data retention

Ensure expired observability data is actually eligible for cleanup.

Document security-sensitive decisions.

---

# 16. Degraded Observability Must Not Kill Core Work

Observability is important, but observability itself must not become a catastrophic dependency.

A telemetry/logging failure should not normally crash an otherwise healthy coding operation.

Design the system so:

```text
business/development operation
        ↓
observability emission
        ↓
if telemetry fails → degrade/report locally where possible
                     without corrupting the primary operation
```

Exceptions are acceptable only where observability is itself required for correctness/security and should be explicitly justified.

---

# 17. Cancellation and Failure Propagation

Define cancellation behavior across:

- supervisor;
- worker;
- tool;
- command;
- database;
- graph;
- MCP;
- model requests.

A cancelled operation must not accidentally become an ordinary retry.

Children should not continue indefinitely after their owning operation has been cancelled unless explicitly detached by design.

Record cancellation distinctly from failure.

---

# 18. Idempotency and Side-Effect Safety

Retries and recovery introduce duplicate-execution risk.

Audit operations that mutate state.

Where appropriate, protect against replay through:

- idempotency;
- CAS/version checks;
- execution IDs;
- existing claim semantics;
- duplicate detection;
- transaction boundaries;
- explicit non-retryable classification.

This is particularly important for:

- git operations;
- commits;
- filesystem mutation;
- persistent memory;
- database state;
- MCP mutations;
- external connector calls.

---

# 19. Testing Requirements

Do not treat compilation as sufficient evidence.

Add/update testing gates for the new behavior.

At minimum cover:

## Tool failures

Prove:

- structured failure output;
- correct retry classification;
- useful agent-visible error context;
- secret redaction.

## Agent startup failure

Simulate transient startup failures and prove:

- retries occur;
- backoff/jitter is bounded;
- terminal failure eventually occurs;
- supervisor state stays correct.

## Mid-run worker failure

Simulate a worker failing after useful work and prove:

- checkpoint survives;
- completed work is discoverable;
- replacement/resumption receives correct state;
- completed side effects are not blindly replayed;
- supervisor remains aware of the failure/recovery.

## Observability

Prove:

- logs emit;
- structured events emit;
- metrics change correctly;
- spans preserve hierarchy;
- IDs correlate across systems;
- durations are recorded;
- memory/database operations appear;
- Fleet events appear.

## Rotation/retention

Prove rotation and cleanup behavior without waiting seven real days.

Use injectable/testable clocks or equivalent deterministic mechanisms where appropriate.

## Security

Prove representative secret values are redacted from:

- logs;
- structured events;
- errors;
- traces.

## Degraded telemetry

Simulate observability-storage/export failure and prove core operations remain safe.

---

# 20. CLI Fleet Execution

Before declaring P4 ready for the user's UI Calendar test, run the Fleet through the CLI.

Exercise a representative end-to-end workflow.

Observe and verify:

- supervisor lifecycle;
- worker lifecycle;
- agent/tool activity;
- structured events;
- metrics;
- traces;
- memory activity;
- database activity;
- command execution;
- completion/failure behavior.

Include deliberate failure injection where practical so the new recovery path is tested rather than only the happy path.

The CLI smoke should prove the Fleet can execute without requiring the Tauri UI.

---

# 21. Cross-Phase Audit

Perform the required audit of:

- P1;
- P2;
- P3;
- P4;
- P8.

For every discovered issue, classify it clearly as:

- fixed now;
- intentionally deferred;
- superseded;
- not applicable;
- blocked by one of the outstanding contract decisions.

Do not silently ignore regressions because they originated in an earlier phase.

---

# 22. Readiness Definition

P4 is not complete until the readiness artifacts reflect actual implementation.

The readiness matrix must clearly distinguish:

- implemented;
- tested;
- integration-tested;
- CLI-smoked;
- manually pending;
- blocked;
- deferred.

The intended final status is:

> All automated/unit/integration/system/CLI gates relevant to P4 pass, the new observability and reliability systems are operational, failure recovery has been exercised, and only the user's final application-level Calendar test through `npm run tauri dev` remains.

Do not mark a gate passed based purely on implementation existence.

---

# 23. Independent Review

Once implementation and testing are complete, perform the previously required independent **GPT-6 Luna High review**.

The reviewer should specifically inspect:

- architectural coherence;
- concurrency safety;
- failure semantics;
- retry correctness;
- recovery correctness;
- idempotency;
- checkpoint durability;
- observability coverage;
- security/redaction;
- testing gaps;
- P8 commit-safety behavior;
- cross-phase regressions.

Treat reviewer findings as issues to disposition, not ceremonial approval.

Resolve actionable findings before declaring readiness unless explicitly documented and justified as deferred.

---

# 24. Commit and Push Strategy

There are already more than 100 changed files.

Do **not** create one giant commit.

First inspect and classify the working tree so unrelated or pre-existing user changes are not accidentally absorbed into incorrect commits.

Then commit and push in coherent stages corresponding to actual architectural work.

A reasonable shape may be:

```text
docs(p4): expand reliability and observability contracts

feat(errors): standardize structured tool failure handling

feat(runtime): add worker retry and lifecycle supervision

feat(recovery): add durable agent execution checkpoints

feat(observability): add structured logging and telemetry core

feat(observability): instrument fleet graph tools memory and database

feat(ui): add observability logs metrics events and trace explorer

feat(security): redact sensitive telemetry data

test(p4): add failure recovery and observability gates

fix(p8): enforce host commit safety

docs(p4): finalize readiness and audit dispositions
```

These are examples, not mandatory exact messages.

The important rules are:

- each commit should represent a coherent unit;
- do not mix unrelated features arbitrarily;
- do not overwrite unrelated user work;
- run the relevant gate before committing a phase;
- push verified stages;
- preserve traceability.

Under the user's existing authorization, commit and push verified work to `main`.

---

# 25. Engineering Constraints

While doing this work:

- Inspect existing architecture before adding abstractions.
- Reuse existing runtime concepts when they already solve part of the problem.
- Avoid duplicate state machines.
- Avoid duplicate logging frameworks.
- Avoid massive rewrites when targeted integration is sufficient.
- Do not hide errors to make tests pass.
- Do not weaken existing tests merely to fit implementation.
- Do not fake telemetry.
- Do not mark incomplete behavior as ready.
- Keep observability overhead reasonable.
- Keep metric cardinality bounded.
- Preserve deterministic development operations.
- Preserve P8 host-enforced commit safety.
- Preserve concurrent-agent correctness.
- Prefer structured contracts over parsing human-readable strings.

---

# 26. Required Final Verification

Before declaring the task finished, run all applicable:

- formatting;
- linting;
- type checking;
- Rust checks;
- frontend checks;
- unit tests;
- integration tests;
- P4 gates;
- P8 gates;
- newly created reliability gates;
- newly created observability gates;
- failure-injection tests;
- CLI Fleet smoke;
- relevant regression tests.

Investigate failures rather than simply rerunning until green.

Record what was executed and the final results in the durable development records.

---

# 27. Final Report

When all work is complete, report clearly:

1. what was implemented;
2. architecture chosen for observability;
3. libraries added and why;
4. tool failure contract;
5. retry policy;
6. worker checkpoint/recovery architecture;
7. supervisor failure-handling behavior;
8. logging retention/rotation policy;
9. security/redaction measures;
10. memory/database observability coverage;
11. observability UI capabilities;
12. testing/failure-injection performed;
13. CLI Fleet execution results;
14. P1/P2/P3/P4/P8 audit dispositions;
15. independent review findings and resolutions;
16. commits created and pushed;
17. remaining limitations;
18. exact readiness state.

The desired final handoff is:

> **P4 is automated-test and CLI verified, failure-containment/recovery and observability are operational, P8 commit safety is closed, all verified work has been committed and pushed in traceable stages, and the only remaining validation is the user's manual Calendar scenario through `npm run tauri dev`.**

Do not stop merely because the happy path works.

The objective is to turn Arcana from a one-shot coding application into a system whose execution can be **observed, diagnosed, recovered, and trusted under failure**.