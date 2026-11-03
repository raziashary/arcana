# P4-61 guarded copyback apply — 2026-10-13

Status: assigned Linux copy-back apply implementation verified in isolation; this report does not close P4-61 or TG4-31.

## Scope and constraints

- Add recoverable descriptor-relative apply for a caller-held `WorkspaceWriteLease` in the Linux staging candidate, confined to `linux_smoke/staging.rs` and optionally a new sibling `copyback.rs`.
- Preserve staged bytes and displaced host versions on conflict or failure. Journal each path durably and state explicitly that a multi-file apply is not filesystem-atomic.
- Fix the demonstrated root-identity versus mutable-directory-metadata distinction with a focused regression.
- Use only isolated temporary resources and the exact WSL Linux tests authorized by root. No live data, credentials, machine configuration, dependency/schema changes, Git actions, or unrelated integration files.

## Progress

- [x] Create this report before source changes.
- [x] Ground the approved journal/recovery boundary and descriptor primitives.
- [x] Implement apply, explicit recovery records, and root-identity regression in owned files.
- [x] Run the coordinated exact WSL test(s), then record actual apply, conflict, partial-failure recovery, and negative-path evidence.
- [x] Document required parent integration points without changing parent-owned files.

## Source/decision evidence

- ADR-LOG-RESTRUCT-0359 approves a managed workspace write lease plus recoverable atomic copy-back that preserves host/staged conflicts. ADR-LOG-RESTRUCT-0363 defines the canonical-workspace lease and states it coordinates Arcana writers but not external editors. The existing `WorkspaceWriteLease::covers` method allows the apply API to reject a lease for a different workspace.
- ADR-LOG-RESTRUCT-0235 requires stopped-child copyback after success/failure/timeout/cancellation while keeping build classification independent. This unit assumes the caller already proved child-tree stop; it does not add or claim that barrier.
- `_verify/P4-61-COPYBACK-WIRING-NEXT-2026-10-12.md` requires descriptor-relative staging, conflict checks, same-filesystem displacement, fsynced per-path progress, and explicit non-atomic multi-file semantics. Existing `staging.rs` supplied pinned root/directory descriptors, `openat`/`fstatat(AT_SYMLINK_NOFOLLOW)`, bounded hashing, and the planner, but no mutation/apply/recovery API.
- The stable root and directory identity comparison already ignores mutable size/link-count/timestamps while preserving device/inode/mode checks. This unit adds an apply-level regression showing unrelated top-level and same-parent host entries do not block a staged edit and survive it; root replacement and permission-change planner regressions remain in the same file.

## Implemented boundary

`apply_copy_back` requires a matching caller-held lease, a fresh plan equal to the supplied plan, pinned descriptor-relative workspace/stage access, and a caller-created private empty journal directory. The durable journal stores workspace/stage identities, byte-encoded relative paths, per-path expected/staged digests, safe mode, unique same-parent temp/backup names, and progress. Each file replacement/deletion uses `renameat2(RENAME_NOREPLACE)`; file contents are copied with size/digest checks, safe mode and `fsync`. Journal replacement uses a synced temporary file and synced journal directory. `recover_copy_back` resumes by validating recorded temps, displaced versions, and installed outputs; committed recovery is idempotent. Conflicts and incomplete applies retain staged files and any displaced backups. A multi-path sequence is explicitly not atomic as a whole.

The implementation is a candidate API in the test-only Linux smoke module. Parent-owned launcher/consumer, lease-admission, stop-barrier, durable caller journal-directory ownership, and stage-lifetime wiring remain required before production use.

## Execution evidence and limitations

The first authorized WSL run exposed a post-displacement identity mismatch: `renameat2` changes a regular file's ctime, so rechecking the moved backup against every original timestamp rejected the implementation's own rename. The fix keeps the full identity and digest comparison before displacement, then uses a backup-only verifier after displacement. It permits ctime to differ while still requiring the original inode, type, mode, link count, size, modification time, and content digest. Recovery and committed-backup cleanup use the same verifier; ordinary host-path conflict checks remain strict.

Exact isolated WSL command, run from `/mnt/c/arcana` with the target directory `/home/razi/arcana-p4-linux-target` and live credential variables unset:

```sh
cargo test --offline -p arcana-core --lib platform::process_sandbox::linux_smoke::staging::tests --locked -- --test-threads=1
```

Result: 12 passed, 0 failed, 0 ignored (1113 filtered out). The run proves successful apply with unrelated host edits preserved, refusal after an external host edit while preserving host and staged bytes, partial-apply recovery with displaced bytes retained, wrong-lease refusal, bounded snapshot/planning behavior, root/parent substitution rejection, and symlink/hardlink rejection. The original 10-passed/2-failed condition is resolved for this exact filter. Cargo emitted existing warnings outside this task's scope.

No production consumer wiring or gate closure is claimed. Parent-owned launcher/consumer integration, child-tree stop barrier, lease admission, durable caller journal-directory ownership, and stage-lifetime wiring remain required; P4-61 and TG4-31 remain open.
