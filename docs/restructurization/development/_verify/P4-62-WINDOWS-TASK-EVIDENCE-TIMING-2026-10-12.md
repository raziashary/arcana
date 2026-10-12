# P4-62 Windows task-attributed evidence timing proof

Status: implementation and accepted Windows measurement run recorded; broader P4-62/TG4-32 remain open.

This report records the Windows-only ignored measurement proof in `arcana-core/src/agent/builtin_tools/project_command.rs`. It will contain the governing requirements, measured samples, results, and limitations. The measurement must run three isolated disposable Arcana homes/workspace/cache profiles, each with one cold and five warm real ProjectCommandTool builds, and verify the persisted passing batch through TasksDb.

## Progress

- Created the report before investigating requirements or implementation.
- Located the supplied next-proof receipt under `docs/restructurization/development/_verify/` (the initial `restructurization/` path was not the repository's canonical docs tree; this report is now at the requested canonical path).
- Existing checkout has unrelated dirty work. I will only edit the assigned test file and this report.
## Draft assertions from the requirement

For each of three independent profiles, the test will create a disposable Arcana home (and TasksDb), a repo-local disposable Windows workspace, and a fresh application/runtime staging cache. It will run the actual `ProjectCommandTool::execute` with Python `Build` intent six times over identical project source: sample zero is cold relative only to that profile's Arcana/workspace/application caches, followed by five warm samples with those caches retained. No OS cache flush is part of this measurement.

Every run must succeed and the latest `TasksDb` batch for the claimed task must read back exactly one selected, observed, passing child. The captured stage events must contain exactly one successful event correlated to the current call ID and task ID for each of `runtime_prepare`, `sandbox_prepare`, `process_wait`, `output_drain`, and `evidence_persistence`; their duration sum must not exceed the measured ProjectCommandTool wall time. The actual raw nanosecond samples will be emitted and retained in this report, along with medians/ranges summarized across all three profiles. This evidence describes observed command-stage timing only; it does not equate total time with compiler time or establish a general speedup.

Potential ambiguity to check against source before implementation: whether `with_home` nesting is safe per-profile in a loop, how to clear the Python runtime staging cache without changing production behavior, and which legal terminal transition releases a successfully claimed task.
## Implementation notes

- Added `assert_task_build_timing_sample` for the matching Linux/Windows contract. The existing Linux fixture name, profile count, call loop, task setup, and reported sample behavior remain intact; it now shares the assertion helper and additionally checks exactly one valid persisted evidence row for the selected workspace.
- Added ignored, Windows-only `p4_62_measure_windows_task_build_stages_with_evidence`. It creates a distinct disposable Arcana home, TasksDb, workspace under `arcana-core/target/p4-acl-tests`, task assignment and atomic PID claim for each of three profiles. It resets only the test Python process-stage cache before each profile's cold sample and retains the project/runtime caches for the five warm calls.
- The fixture source and `{"intent":"build","toolchain":"python"}` input are identical for every profile. Each call goes through `ProjectCommandTool::execute`, captures the five stage events, and reads the latest persisted batch through `TasksDb`. The task is completed through the worker-claim-aware status transition after its six calls.
- The new test prints each raw wall/stage sample in nanoseconds and cold/warm medians and ranges. Its report text and assertions explicitly limit “cold” to Arcana-owned profile/workspace/application caches and leave OS caches untouched.

## Validation

Only the coordinator-prescribed filtered Windows measurement test will be run, with the listed provider/database variables removed from that command's process environment.

## First prescribed run and assertion correction

The filtered test compiled and started, then stopped on its first sample after 69.25 seconds because the shared assertion expected the QC workspace-selection SQL function (`batch.8`) to return true. Source inspection shows that field tests a separately bound QC transition context, which this direct ProjectCommandTool measurement intentionally does not perform. That expectation was over-scoped. I am replacing it with checks on the actual persisted batch workspace identity and `latest_build_evidence` reader (successful attributable result, same workspace path and fingerprint), which prove the required persisted consumer contract without inventing a QC transition. The test process exited on the assertion; its owned temp profile/workspace guards ran during unwinding.

The corrected check reads the latest batch and evidence rows independently: the batch must have one selected/observed/passing child, one evidence row and no invalid rows; the persisted path must equal the measured canonical workspace; the evidence must report exit 0, attributable exit status, and the same workspace path/fingerprint. The QC-only selected-context field is no longer asserted.

The prescribed filtered command is being rerun unchanged after this test-only correction.

## Second prescribed run and Windows path normalization

The build and task/batch/evidence reads reached the first sample, then the test stopped after 39.22 seconds on a case-sensitive string comparison: Windows persisted the workspace as `C:\\...\\p4-...tB7Bhi`, while `workspace_identity` normalizes it to lowercase `...tb7bhi` for equality. This is expected Windows path normalization, not a build or persistence failure. I will compare the persisted path and the fixture path by their canonical `workspace_identity` values, including the evidence row, then rerun only the prescribed command.

## Accepted run: raw samples as produced

The accepted run is the final prescribed test invocation only. Samples from either prior assertion-failing run are excluded.

```text
P4-62 Windows observed sample {"cold":true,"profile":0,"sample":0,"stages_ns":{"evidence_persistence":6753300,"output_drain":198500,"process_wait":599822800,"runtime_prepare":36374880300,"sandbox_prepare":101701900},"wall_ns":37140962300}
P4-62 Windows observed sample {"cold":false,"profile":0,"sample":1,"stages_ns":{"evidence_persistence":7077000,"output_drain":119600,"process_wait":504205100,"runtime_prepare":3458688600,"sandbox_prepare":74463300},"wall_ns":4082649900}
P4-62 Windows observed sample {"cold":false,"profile":0,"sample":2,"stages_ns":{"evidence_persistence":6590100,"output_drain":137100,"process_wait":510623200,"runtime_prepare":3078323800,"sandbox_prepare":70058900},"wall_ns":3700112700}
P4-62 Windows observed sample {"cold":false,"profile":0,"sample":3,"stages_ns":{"evidence_persistence":7007500,"output_drain":166800,"process_wait":502076600,"runtime_prepare":3026279400,"sandbox_prepare":58494900},"wall_ns":3628243000}
P4-62 Windows observed sample {"cold":false,"profile":0,"sample":4,"stages_ns":{"evidence_persistence":10294700,"output_drain":120800,"process_wait":498831800,"runtime_prepare":3033436300,"sandbox_prepare":78672300},"wall_ns":3655381200}
P4-62 Windows observed sample {"cold":false,"profile":0,"sample":5,"stages_ns":{"evidence_persistence":6277500,"output_drain":142600,"process_wait":491421600,"runtime_prepare":3046617700,"sandbox_prepare":70106200},"wall_ns":3648731400}
```

```text
P4-62 Windows observed sample {"cold":true,"profile":1,"sample":0,"stages_ns":{"evidence_persistence":6030700,"output_drain":123100,"process_wait":534431000,"runtime_prepare":34297365900,"sandbox_prepare":59245500},"wall_ns":34936091300}
P4-62 Windows observed sample {"cold":false,"profile":1,"sample":1,"stages_ns":{"evidence_persistence":6061500,"output_drain":131800,"process_wait":503633800,"runtime_prepare":3205673500,"sandbox_prepare":64423900},"wall_ns":3814379500}
P4-62 Windows observed sample {"cold":false,"profile":1,"sample":2,"stages_ns":{"evidence_persistence":6999700,"output_drain":172700,"process_wait":502373500,"runtime_prepare":3010768100,"sandbox_prepare":58341900},"wall_ns":3613567600}
P4-62 Windows observed sample {"cold":false,"profile":1,"sample":3,"stages_ns":{"evidence_persistence":6118400,"output_drain":136700,"process_wait":514722800,"runtime_prepare":3025443300,"sandbox_prepare":60285000},"wall_ns":3642803600}
P4-62 Windows observed sample {"cold":false,"profile":1,"sample":4,"stages_ns":{"evidence_persistence":5906100,"output_drain":140500,"process_wait":514726500,"runtime_prepare":3175819500,"sandbox_prepare":52701900},"wall_ns":3781338200}
P4-62 Windows observed sample {"cold":false,"profile":1,"sample":5,"stages_ns":{"evidence_persistence":7587000,"output_drain":114700,"process_wait":519681500,"runtime_prepare":3103983100,"sandbox_prepare":55627000},"wall_ns":3721155200}
```

```text
P4-62 Windows observed sample {"cold":true,"profile":2,"sample":0,"stages_ns":{"evidence_persistence":6312500,"output_drain":173500,"process_wait":529271100,"runtime_prepare":34623523300,"sandbox_prepare":57602300},"wall_ns":35254929800}
P4-62 Windows observed sample {"cold":false,"profile":2,"sample":1,"stages_ns":{"evidence_persistence":6028500,"output_drain":125400,"process_wait":502385400,"runtime_prepare":3211517100,"sandbox_prepare":56801700},"wall_ns":3814970200}
P4-62 Windows observed sample {"cold":false,"profile":2,"sample":2,"stages_ns":{"evidence_persistence":6985000,"output_drain":117700,"process_wait":495221500,"runtime_prepare":3017459900,"sandbox_prepare":51596900},"wall_ns":3601544500}
```

```text
P4-62 Windows observed sample {"cold":false,"profile":2,"sample":3,"stages_ns":{"evidence_persistence":6179000,"output_drain":134800,"process_wait":499904200,"runtime_prepare":3016648200,"sandbox_prepare":60359200},"wall_ns":3617947700}
P4-62 Windows observed sample {"cold":false,"profile":2,"sample":4,"stages_ns":{"evidence_persistence":6410200,"output_drain":131300,"process_wait":520446600,"runtime_prepare":3038303400,"sandbox_prepare":57366000},"wall_ns":3653664700}
P4-62 Windows observed sample {"cold":false,"profile":2,"sample":5,"stages_ns":{"evidence_persistence":6030800,"output_drain":118700,"process_wait":491059400,"runtime_prepare":3129332100,"sandbox_prepare":54084700},"wall_ns":3711487100}
```

## Accepted run results

The prescribed command passed: `1 passed; 0 failed; 0 ignored`, with the selected measurement test completing in 166.19 seconds. It performed exactly 18 calls: three cold samples and fifteen warm samples across three distinct Arcana homes, TasksDb instances, repository-local Windows workspaces, and reset Python application-stage caches. Each call returned success, persisted a latest batch with `(outcome, expected, observed, passing) = (passed, 1, 1, 1)`, one valid evidence row, exit code 0, attributable exit status, and matching canonical workspace identity/fingerprint. Each call emitted exactly one successful correlated event for all five stages, and summed stage durations were no greater than ProjectCommandTool wall time.

Final-run summary, in nanoseconds:

| Measure | Cold median (range) | Warm median (range) |
|---|---:|---:|
| ProjectCommandTool wall time | 35,254,929,800 (34,936,091,300–37,140,962,300) | 3,655,381,200 (3,601,544,500–4,082,649,900) |
| runtime_prepare | 34,623,523,300 (34,297,365,900–36,374,880,300) | 3,046,617,700 (3,010,768,100–3,458,688,600) |
| sandbox_prepare | 59,245,500 (57,602,300–101,701,900) | 58,494,900 (51,596,900–78,672,300) |
| process_wait | 534,431,000 (529,271,100–599,822,800) | 502,385,400 (491,059,400–520,446,600) |
| output_drain | 173,500 (123,100–198,500) | 131,800 (114,700–172,700) |
| evidence_persistence | 6,312,500 (6,030,700–6,753,300) | 6,410,200 (5,906,100–10,294,700) |

These are measurements of this fixture on this Windows host. “Cold” describes fresh Arcana profile/workspace/application caches; OS caches were not flushed. Wall time includes setup, sandbox preparation, build process, output collection, persistence and other tool work; it is not compiler time. The QC-only selected-workspace field (`batch.8`) was deliberately not asserted because this direct tool invocation did not bind a QC transition context. This proof therefore does not establish the QC transition, general speedup, Mac behavior, or P4/TG gate closure.

The test runtime and temporary directory guards completed normally, and no `p4-62-windows-task-build-*` temp workspace remained under the repository-local `target/p4-acl-tests` parent. The parent directory was retained. No production code, schema, dependencies, policy, credentials, providers, live resources, or git state were changed.


Source-contract note: the TasksDb batch reader returns fields 0–7 as outcome/counts/workspace/fingerprint/evidence-count/invalid-evidence-count. Field 8 is `arcana_qc_selected_workspace_matches`, used by the separate QC admission check; it does not prove a persistent build result and remains outside this proof.

## Governing scope and boundary

The supplied MDP P4-62 / TG4-32 wording requires separate sandbox-preparation, process-wait, output-drain, and build-evidence timing correlated to the tool call, plus identical cold/warm fixture runs with variance reported without calling harness time compiler time. The next-proof receipt identifies the specific Windows gap: its existing cold/warm test exercised only `run_resolved_command`, with no task claim, evidence-persistence stage, or TasksDb reader. This test addresses only that matched Windows task/evidence measurement slice. It does not rerun the already-proven CLI deadline/JSONL consumers, close other P4-62/TG4-32 clauses, or supply macOS data.

## Coordinator validation and task closeout

Root reviewed the complete source diff and producer/reader APIs. Windows normal core/CLI cargo check passes in17.91s; Linux cargo check --tests passes in2m55s, type-checking the changed shared Linux assertion without repeating the earlier measurement. Existing unrelated warnings remain. The coordinator applied rustfmt to the owning file; cargo fmt --all -- --check and git diff --check pass. No listener was started. The exact selected measurement remains the accepted18call run above; formatting does not change its behavior.

The independent Luna High source/requirement review reports no confirmed scoped defect. ADR0374 maps the combined existing and new evidence to the named P4-62/TG4-32 contract and closes those two rows. The original report's broader-open statement describes the pre-review status; its limits still apply: this Windows measurement does not itself prove QC admission, macOS confinement or paid-vendor completion. Those independently owned requirements and P4 phase exit stay open.
