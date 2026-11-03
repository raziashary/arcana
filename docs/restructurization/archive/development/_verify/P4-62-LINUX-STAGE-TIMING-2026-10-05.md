# P4-62 Linux task-build timings — 2026-10-05

## Production change and observed defect

Successful project-command stage timers previously stayed alive until the whole launcher returned. An observed 454.6ms call reported 418.8ms sandbox preparation plus 407.1ms process wait, double-counting compiler time. The launcher now drops each completed sandbox/process/output timer at its own boundary; interrupted stages still report incomplete. The prior overlapping trace values are retracted as stage measurements (ADR-0299). No command policy or schema changes.

## Controlled measurement

Exact ignored filter: `p4_62_measure_linux_task_build_stages_with_evidence`. Ubuntu WSL: 1/1 pass, 3.78s for 18 actual ProjectCommandTool Python Build calls. Three fresh workspace/cache profiles share one disposable Arcana home and process; each contains identical Python inputs and runs one cold plus five warm calls with generated caches retained. This is cold workspace-cache behavior, not cold OS/toolchain state. Linux runtime preparation has no staging work yet; its tiny reported span is bookkeeping. No live provider or production database is used.

Every sample is associated with an atomically claimed task, reads back a complete passing build batch, and emits exactly one successful event per stage with matching task/call identity. Sequential stage totals must not exceed the separately measured full call. Each profile finishes its task before claiming the next, preserving the one-active-task invariant.

| Observed metric | Cold median (range), ms | Warm median (range), ms |
|---|---:|---:|
| Full tool call | 208.245 (191.504–427.257) | 69.420 (49.583–118.541) |
| Runtime preparation | 0.002 (0.001–0.002) | 0.002 (0.001–0.003) |
| Sandbox preparation | 2.112 (1.355–17.574) | 2.604 (1.182–5.492) |
| Process wait | 187.367 (170.243–365.408) | 37.346 (28.492–85.002) |
| Output drain | 0.109 (0.103–4.638) | 0.115 (0.089–0.230) |
| Evidence persistence | 6.288 (6.162–7.101) | 6.549 (5.698–15.946) |

Stage medians do not necessarily sum to the median wall time; profiles/samples vary and full calls also include discovery, fingerprinting and task/batch work. No universal speedup claim is made. Raw measurements: ignored `target/p4-linux-task-build-timing.txt`.

## Remaining proof

The actual descendant-timeout regression passes with final stage-order assertions on Linux (1/1, 1.20s) and Windows (1/1, 2.44s). It starts a real descendant, observes writes, drops the timed-out launcher future, then verifies the heartbeat stops and the late side effect is absent. Sandbox completion is reported before the incomplete process-wait outcome. Scheduled DB recovery is a separate check. Workspace check passes (26.10s), TypeScript check passes. P4-62/TG4-32 remain open for actual CLI deadline/shutdown, full cross-platform measurements and the outstanding sandbox boundary. The fixture initially failed at its second claim because the prior task remained active; correcting fixture completion did not change product behavior.
