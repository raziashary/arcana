// File checkpointing — snapshots taken before every agent write so edits
// can be undone deterministically.
//
// Storage layout:
//   ~/.arcana/checkpoints/<session_id>/<timestamp>_<basename>
// Index:
//   config.db — `checkpoints` table (one row per snapshot)
//
// Retention: the most recent `RETENTION_PER_FILE` snapshots per file are
// kept; older ones are pruned after each new snapshot.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::arcana_dir;
use crate::errors::{ArcanaError, Result};

use super::diff;
use super::types::{CheckpointInfo, DiffResult, RestoreResult};

/// Keep this many snapshots per file before oldest gets evicted.
pub const RETENTION_PER_FILE: usize = 50;

// ─── Storage bootstrap ────────────────────────────────────────────────────────

/// Open `config.db` through its schema owner.
///
/// This used to issue its own `CREATE TABLE IF NOT EXISTS checkpoints`, which
/// made it one of three independent writers to a file whose `user_version` only
/// described half its contents. The DDL now lives in `db::config_db`'s ladder
/// (v2) — see that module's header for why the split was wrong.
fn open_db() -> Result<Connection> {
    crate::db::config_db::open().map_err(|e| ArcanaError::Git(format!("config.db open: {e}")))
}

fn checkpoints_root() -> Result<PathBuf> {
    let root = arcana_dir()?.join("checkpoints");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

fn now_ms() -> i64 {
    #[cfg(test)]
    if let Some(instant) = crate::db::test_clock::current() {
        return instant.timestamp_millis();
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Save the prior state of `file_path`, including its non-existence. Recording
/// absent files lets restore remove a file created by the task.
pub fn create(
    session_id: &str,
    task_id: Option<i64>,
    file_path: &str,
    reason: &str,
) -> Result<Option<CheckpointInfo>> {
    let mut operation = crate::observability::timing::OperationTimer::start("checkpoint_create")
        .with_session_id(session_id)
        .with_task_id(task_id);
    if session_id.is_empty()
        || session_id.len() > 128
        || session_id == "."
        || session_id == ".."
        || session_id
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    {
        return Err(ArcanaError::Git(
            "invalid checkpoint session identifier".into(),
        ));
    }
    let src = Path::new(file_path);
    let file_existed = src.exists();
    let bytes = if file_existed {
        Some(std::fs::read(src)?)
    } else {
        None
    };
    let size = bytes.as_ref().map_or(0, |bytes| bytes.len() as u64);

    let session_dir = checkpoints_root()?.join(session_id);
    std::fs::create_dir_all(&session_dir)?;

    let id = Uuid::new_v4().to_string();
    let stamp = now_ms();
    let base = src.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let snap_path = session_dir.join(format!("{stamp}_{base}.{id}.snap"));
    if let Some(bytes) = bytes.as_ref() {
        std::fs::write(&snap_path, bytes)?;
    }

    let snap_path_str = if file_existed {
        snap_path.to_string_lossy().to_string()
    } else {
        String::new()
    };
    let file_path_str = file_path.to_string();

    {
        let conn = open_db()?;
        conn.execute(
            "INSERT INTO checkpoints (id, session_id, task_id, file_path, snapshot_path, created_ms, reason, size_bytes, file_existed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![&id, session_id, task_id, &file_path_str, &snap_path_str, stamp, reason, size as i64, file_existed as i64],
        )
        .map_err(|e| ArcanaError::Git(format!("checkpoint insert: {e}")))?;
    }

    prune(&file_path_str, task_id)?;

    operation = operation
        .with_result_count(1)
        .with_result_bytes(size as usize);
    operation.succeed();
    Ok(Some(CheckpointInfo {
        id,
        session_id: session_id.to_string(),
        task_id,
        file_path: file_path_str,
        created_ms: stamp,
        reason: reason.to_string(),
        size_bytes: size,
        snapshot_path: snap_path_str,
    }))
}

/// Create a checkpoint only for a file that resolves inside the selected
/// canonical workspace. Used by renderer-facing commands; agent tools already
/// pass through their workspace path jail.
pub fn create_within_workspace(
    session_id: &str,
    task_id: Option<i64>,
    file_path: &str,
    reason: &str,
    workspace: &str,
) -> Result<Option<CheckpointInfo>> {
    if !Path::new(file_path).is_absolute() {
        return Err(ArcanaError::Git(
            "checkpoint file path must be absolute".into(),
        ));
    }
    let workspace = crate::platform::workspace::resolve_workspace_dir(Some(workspace))?;
    let workspace_text = workspace.to_string_lossy();
    let path = crate::agent::builtin_tools::path_jail::validate_workspace_path(
        Path::new(file_path),
        &workspace_text,
    )
    .map_err(ArcanaError::Git)?;
    create(session_id, task_id, &path.to_string_lossy(), reason)
}

/// Restore the file referenced by `checkpoint_id` from its snapshot. The
/// current contents are lost (callers may want to take a fresh checkpoint
/// first; the auto-checkpoint hook does this automatically).
pub fn restore(checkpoint_id: &str) -> Result<RestoreResult> {
    restore_inner(checkpoint_id, None)
}

/// Restore a checkpoint only when both its destination and snapshot resolve
/// inside their respective Arcana/workspace roots. The destination is checked
/// again immediately before the destructive filesystem operation.
pub fn restore_within_workspace(checkpoint_id: &str, workspace: &str) -> Result<RestoreResult> {
    let workspace = crate::platform::workspace::resolve_workspace_dir(Some(workspace))?;
    restore_inner(checkpoint_id, Some(&workspace))
}

fn restore_inner(checkpoint_id: &str, workspace: Option<&Path>) -> Result<RestoreResult> {
    let mut operation = crate::observability::timing::OperationTimer::start("checkpoint_restore");
    let (file_path, snapshot_path, file_existed, task_id, rowid) = {
        let conn = open_db()?;
        conn.query_row(
            "SELECT file_path, snapshot_path, file_existed, task_id, rowid FROM checkpoints WHERE id = ?1",
            params![checkpoint_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )
        .map_err(|e| ArcanaError::Git(format!("checkpoint lookup: {e}")))?
    };
    operation = operation.with_task_id(task_id);

    if let Some(workspace) = workspace {
        validate_checkpoint_target(Path::new(&file_path), workspace)?;
        if file_existed {
            let snapshot_root = checkpoints_root()?.canonicalize()?;
            let snapshot = Path::new(&snapshot_path).canonicalize()?;
            if !snapshot.starts_with(&snapshot_root) {
                return Err(ArcanaError::Git(
                    "checkpoint snapshot is outside Arcana's checkpoint directory".into(),
                ));
            }
        }
    }

    // A task snapshot must not roll back a later write checkpointed for a
    // different task (or for an interactive session). Refuse the stale restore
    // instead of silently replacing another actor's file state.
    if let Some(task_id) = task_id {
        let conn = open_db()?;
        let intervening: i64 = conn.query_row(
            "SELECT COUNT(*) FROM checkpoints
             WHERE file_path = ?1 AND rowid > ?2 AND task_id IS NOT ?3",
            params![file_path, rowid, task_id],
            |row| row.get(0),
        )?;
        if intervening > 0 {
            return Err(ArcanaError::Git(format!(
                "Task checkpoint {checkpoint_id} is stale: another task or session checkpointed this file afterward"
            )));
        }
    }

    if !file_existed {
        let destination = match workspace {
            Some(workspace) => validate_checkpoint_target(Path::new(&file_path), workspace)?,
            None => PathBuf::from(&file_path),
        };
        match std::fs::remove_file(destination) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let result = RestoreResult {
            file_path,
            restored_from: snapshot_path,
            bytes_written: 0,
        };
        operation
            .with_result_count(1)
            .with_result_bytes(0)
            .succeed();
        return Ok(result);
    }

    let data = std::fs::read(&snapshot_path)?;
    let destination = match workspace {
        Some(workspace) => validate_checkpoint_target(Path::new(&file_path), workspace)?,
        None => PathBuf::from(&file_path),
    };
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&destination, &data)?;
    let result = RestoreResult {
        file_path,
        restored_from: snapshot_path,
        bytes_written: data.len() as u64,
    };
    operation
        .with_result_count(1)
        .with_result_bytes(data.len())
        .succeed();
    Ok(result)
}

fn validate_checkpoint_target(path: &Path, workspace: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(ArcanaError::Git(
            "checkpoint destination is not an absolute path".into(),
        ));
    }
    let workspace_text = workspace.to_string_lossy();
    crate::agent::builtin_tools::path_jail::validate_workspace_path(path, &workspace_text)
        .map_err(ArcanaError::Git)
}

/// List snapshots optionally filtered by file path and/or session.
pub fn list(
    file_path: Option<&str>,
    session_id: Option<&str>,
    task_id: Option<i64>,
) -> Result<Vec<CheckpointInfo>> {
    let mut operation = crate::observability::timing::OperationTimer::start("checkpoint_list")
        .with_task_id(task_id);
    if let Some(session_id) = session_id {
        operation = operation.with_session_id(session_id);
    }
    let conn = open_db()?;
    let mut sql = String::from(
        "SELECT id, session_id, task_id, file_path, snapshot_path, created_ms, reason, size_bytes
         FROM checkpoints WHERE 1=1",
    );
    let mut args: Vec<String> = Vec::new();
    if let Some(p) = file_path {
        sql.push_str(" AND file_path = ?");
        args.push(p.to_string());
    }
    if let Some(s) = session_id {
        sql.push_str(" AND session_id = ?");
        args.push(s.to_string());
    }
    if let Some(task) = task_id {
        sql.push_str(" AND task_id = ?");
        args.push(task.to_string());
    }
    sql.push_str(" ORDER BY created_ms DESC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| ArcanaError::Git(format!("prepare: {e}")))?;
    let params_refs: Vec<&dyn rusqlite::ToSql> =
        args.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(params_refs.as_slice(), |r| {
            Ok(CheckpointInfo {
                id: r.get(0)?,
                session_id: r.get(1)?,
                task_id: r.get(2)?,
                file_path: r.get(3)?,
                snapshot_path: r.get(4)?,
                created_ms: r.get(5)?,
                reason: r.get(6)?,
                size_bytes: r.get::<_, i64>(7)? as u64,
            })
        })
        .map_err(|e| ArcanaError::Git(format!("query: {e}")))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| ArcanaError::Git(e.to_string()))?);
    }
    if out.is_empty() {
        operation.with_result_count(0).set_outcome("empty");
    } else {
        operation.with_result_count(out.len()).succeed();
    }
    Ok(out)
}

pub fn delete(checkpoint_id: &str) -> Result<()> {
    let operation = crate::observability::timing::OperationTimer::start("checkpoint_delete");
    let snapshot_path: Option<String> = {
        let conn = open_db()?;
        conn.query_row(
            "SELECT snapshot_path FROM checkpoints WHERE id = ?1",
            params![checkpoint_id],
            |r| r.get(0),
        )
        .ok()
    };
    if let Some(p) = snapshot_path.as_deref() {
        let _ = std::fs::remove_file(p);
    }
    let conn = open_db()?;
    let deleted = conn
        .execute(
            "DELETE FROM checkpoints WHERE id = ?1",
            params![checkpoint_id],
        )
        .map_err(|e| ArcanaError::Git(format!("delete: {e}")))?;
    if deleted == 0 {
        operation.with_result_count(0).set_outcome("not_found");
    } else {
        operation.with_result_count(deleted).succeed();
    }
    Ok(())
}

/// Produce a diff between two snapshots (old → new) using git's patch format.
pub fn compare(id_old: &str, id_new: &str) -> Result<DiffResult> {
    let operation = crate::observability::timing::OperationTimer::start("checkpoint_compare");
    let conn = open_db()?;
    let get_snap = |id: &str| -> Result<(String, bool)> {
        conn.query_row(
            "SELECT snapshot_path, file_existed FROM checkpoints WHERE id = ?1",
            params![id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)),
        )
        .map_err(|e| ArcanaError::Git(format!("lookup {id}: {e}")))
    };
    let (old_path, old_existed) = get_snap(id_old)?;
    let (new_path, new_existed) = get_snap(id_new)?;
    let old_bytes = if old_existed {
        std::fs::read(old_path)?
    } else {
        Vec::new()
    };
    let new_bytes = if new_existed {
        std::fs::read(new_path)?
    } else {
        Vec::new()
    };
    let result = diff::diff_blobs(&old_bytes, &new_bytes);
    operation
        .with_result_count(1)
        .with_result_bytes(result.patch.len())
        .succeed();
    Ok(result)
}

/// Evict old checkpoints for one file and ownership scope beyond the cap. A
/// busy task must not evict another task's only recovery points.
fn prune(file_path: &str, task_id: Option<i64>) -> Result<()> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, snapshot_path FROM checkpoints
         WHERE file_path = ?1 AND task_id IS ?2 ORDER BY created_ms DESC",
        )
        .map_err(|e| ArcanaError::Git(e.to_string()))?;
    let rows = stmt
        .query_map(params![file_path, task_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| ArcanaError::Git(e.to_string()))?;

    let mut keepers_seen = 0;
    let mut to_delete: Vec<(String, String)> = Vec::new();
    for r in rows {
        let (id, path) = r.map_err(|e| ArcanaError::Git(e.to_string()))?;
        if keepers_seen < RETENTION_PER_FILE {
            keepers_seen += 1;
            continue;
        }
        to_delete.push((id, path));
    }
    drop(stmt);

    for (id, path) in to_delete {
        let _ = std::fs::remove_file(&path);
        let _ = conn.execute("DELETE FROM checkpoints WHERE id = ?1", params![id]);
    }
    Ok(())
}

/// Synchronous hook used by write_file / edit_file / any other file-mutating
/// tool. Errors are returned so callers can refuse a mutation without a
/// recoverable prior state.
pub fn auto_checkpoint_before_write(
    session_id: &str,
    task_id: Option<i64>,
    file_path: &str,
) -> Result<()> {
    let operation = crate::observability::timing::OperationTimer::start("checkpoint_before_write")
        .with_session_id(session_id)
        .with_task_id(task_id);
    create(session_id, task_id, file_path, "pre_write")?;
    operation.with_result_count(1).succeed();
    Ok(())
}

#[cfg(test)]
mod task_association_tests {
    use super::*;

    #[test]
    fn checkpoint_lifecycle_operations_emit_private_result_spans() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let file = sandbox.path().join("private-source-path-marker.rs");
        std::fs::write(&file, "private-source-content-before").expect("seed source file");
        let path = file.to_string_lossy().into_owned();

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let before = create(
                "private-session-correlation-marker",
                Some(77),
                &path,
                "private-checkpoint-reason-marker",
            )
            .expect("create initial snapshot")
            .expect("existing source is snapshotted");
            std::fs::write(&file, "private-source-content-after").expect("mutate source file");
            let after = create(
                "private-session-correlation-marker",
                Some(77),
                &path,
                "private-checkpoint-reason-marker",
            )
            .expect("create later snapshot")
            .expect("existing source is snapshotted");
            let comparison = compare(&before.id, &after.id).expect("compare snapshots");
            let listed = list(
                Some(&path),
                Some("private-session-correlation-marker"),
                Some(77),
            )
            .expect("read snapshots");
            let restored = restore(&before.id).expect("restore earlier snapshot");
            delete(&after.id).expect("delete later snapshot");
            let missing_delete = delete(&after.id).is_ok();
            auto_checkpoint_before_write("private-session-correlation-marker", Some(77), &path)
                .expect("automatic pre-write checkpoint");
            (
                comparison,
                listed.len(),
                restored.bytes_written,
                missing_delete,
            )
        });

        assert_eq!(observed.1, 2);
        assert!(observed.0.patch.contains("private-source-content"));
        assert!(observed.2 > 0);
        assert!(
            observed.3,
            "deleting an already-removed checkpoint is harmless"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("restored source file"),
            "private-source-content-before"
        );

        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "checkpoint_create",
            "checkpoint_restore",
            "checkpoint_list",
            "checkpoint_compare",
            "checkpoint_delete",
            "checkpoint_before_write",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing checkpoint operation span {operation}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "checkpoint_delete"
                && event["fields"]["outcome"] == "not_found"
                && event["fields"]["result_count"] == 0
        }));
        let serialized = serde_json::to_string(&operations).expect("serialize checkpoint spans");
        for private_value in [
            "private-source-path-marker",
            "private-source-content-before",
            "private-source-content-after",
            "private-checkpoint-reason-marker",
            sandbox.path().to_string_lossy().as_ref(),
        ] {
            assert!(
                !serialized.contains(private_value),
                "checkpoint telemetry leaked {private_value}"
            );
        }
        assert!(serialized.contains("private-session-correlation-marker"));
        assert!(serialized.contains("\"task_id\":\"77\""));
    }

    #[test]
    fn worker_checkpoint_keeps_task_id_and_stale_restore_cannot_undo_another_task() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let file = sandbox.path().join("shared.rs");
        std::fs::write(&file, "initial").expect("seed file");
        let path = file.to_string_lossy().into_owned();

        let task_a = create("worker-session-a", Some(41), &path, "pre_write")
            .expect("create task checkpoint")
            .expect("existing file is snapshotted");
        assert_eq!(task_a.task_id, Some(41));
        assert_eq!(list(Some(&path), None, Some(41)).unwrap().len(), 1);

        std::fs::write(&file, "task A output").expect("write A output");
        create("worker-session-b", Some(42), &path, "pre_write").expect("create task B checkpoint");
        std::fs::write(&file, "task B output").expect("write B output");

        let error = restore(&task_a.id).expect_err("stale task A restore must refuse");
        assert!(error.to_string().contains("another task or session"));
        assert_eq!(std::fs::read_to_string(file).unwrap(), "task B output");
    }

    #[test]
    fn interactive_checkpoint_remains_session_only() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let file = sandbox.path().join("session.txt");
        std::fs::write(&file, "old").expect("seed file");
        let checkpoint = create(
            "interactive-session",
            None,
            &file.to_string_lossy(),
            "pre_write",
        )
        .expect("create session checkpoint")
        .expect("existing file is snapshotted");
        assert_eq!(checkpoint.task_id, None);
        assert_eq!(
            list(None, Some("interactive-session"), None).unwrap().len(),
            1
        );
    }

    #[test]
    fn restoring_a_creation_checkpoint_removes_the_task_created_file() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let file = sandbox.path().join("created-by-task.rs");
        let path = file.to_string_lossy().into_owned();
        let checkpoint = create("worker-session", Some(43), &path, "pre_write")
            .expect("persist absent-file checkpoint")
            .expect("absence is a restorable prior state");
        std::fs::write(&file, "task output").expect("simulate successful task write");

        restore(&checkpoint.id).expect("restore created-file checkpoint");
        assert!(
            !file.exists(),
            "restoring prior absence removes task output"
        );
    }

    #[test]
    fn scoped_checkpoint_restore_refuses_a_destination_outside_workspace() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let workspace = sandbox.path().join("workspace");
        let outside = sandbox.path().join("outside.txt");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(&outside, "before").unwrap();
        let checkpoint = create(
            "scoped-session",
            Some(44),
            &outside.to_string_lossy(),
            "pre_write",
        )
        .unwrap()
        .unwrap();
        std::fs::write(&outside, "after").unwrap();

        let error = restore_within_workspace(&checkpoint.id, &workspace.to_string_lossy())
            .expect_err("restore outside the workspace must refuse");

        assert!(error.to_string().contains("outside the workspace"));
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "after");
    }

    #[cfg(unix)]
    #[test]
    fn scoped_checkpoint_restore_refuses_a_retargeted_symlink() {
        use std::os::unix::fs::symlink;
        let sandbox = tempfile::tempdir().expect("sandbox");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let workspace = sandbox.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let file = workspace.join("target.txt");
        let external = sandbox.path().join("external.txt");
        std::fs::write(&file, "before").unwrap();
        std::fs::write(&external, "private").unwrap();
        let checkpoint = create(
            "symlink-session",
            Some(45),
            &file.to_string_lossy(),
            "pre_write",
        )
        .unwrap()
        .unwrap();
        std::fs::remove_file(&file).unwrap();
        symlink(&external, &file).unwrap();

        let error = restore_within_workspace(&checkpoint.id, &workspace.to_string_lossy())
            .expect_err("a retargeted destination must refuse");

        assert!(error.to_string().contains("outside the workspace"));
        assert_eq!(std::fs::read_to_string(external).unwrap(), "private");
    }
}
