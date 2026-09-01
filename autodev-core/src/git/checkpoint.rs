// File checkpointing — snapshots taken before every agent write so edits
// can be undone deterministically.
//
// Storage layout:
//   ~/.autodev/checkpoints/<session_id>/<timestamp>_<basename>
// Index:
//   config.db — `checkpoints` table (one row per snapshot)
//
// Retention: the most recent `RETENTION_PER_FILE` snapshots per file are
// kept; older ones are pruned after each new snapshot.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};

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
    crate::db::config_db::open().map_err(|e| AutoDevError::Git(format!("config.db open: {e}")))
}

fn checkpoints_root() -> Result<PathBuf> {
    let root = autodev_dir()?.join("checkpoints");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Take a snapshot of `file_path`. No-op when the file does not yet exist
/// (nothing to preserve) — returns None in that case.
pub fn create(session_id: &str, file_path: &str, reason: &str) -> Result<Option<CheckpointInfo>> {
    let src = Path::new(file_path);
    if !src.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(src)?;
    let size = bytes.len() as u64;

    let session_dir = checkpoints_root()?.join(session_id);
    std::fs::create_dir_all(&session_dir)?;

    let id = Uuid::new_v4().to_string();
    let stamp = now_ms();
    let base = src.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let snap_path = session_dir.join(format!("{stamp}_{base}.{id}.snap"));
    std::fs::write(&snap_path, &bytes)?;

    let snap_path_str = snap_path.to_string_lossy().to_string();
    let file_path_str = file_path.to_string();

    {
        let conn = open_db()?;
        conn.execute(
            "INSERT INTO checkpoints (id, session_id, file_path, snapshot_path, created_ms, reason, size_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![&id, session_id, &file_path_str, &snap_path_str, stamp, reason, size as i64],
        )
        .map_err(|e| AutoDevError::Git(format!("checkpoint insert: {e}")))?;
    }

    prune(&file_path_str)?;

    Ok(Some(CheckpointInfo {
        id,
        session_id: session_id.to_string(),
        file_path: file_path_str,
        created_ms: stamp,
        reason: reason.to_string(),
        size_bytes: size,
        snapshot_path: snap_path_str,
    }))
}

/// Restore the file referenced by `checkpoint_id` from its snapshot. The
/// current contents are lost (callers may want to take a fresh checkpoint
/// first; the auto-checkpoint hook does this automatically).
pub fn restore(checkpoint_id: &str) -> Result<RestoreResult> {
    let (file_path, snapshot_path) = {
        let conn = open_db()?;
        conn.query_row(
            "SELECT file_path, snapshot_path FROM checkpoints WHERE id = ?1",
            params![checkpoint_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|e| AutoDevError::Git(format!("checkpoint lookup: {e}")))?
    };

    let data = std::fs::read(&snapshot_path)?;
    if let Some(parent) = Path::new(&file_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&file_path, &data)?;
    Ok(RestoreResult {
        file_path,
        restored_from: snapshot_path,
        bytes_written: data.len() as u64,
    })
}

/// List snapshots optionally filtered by file path and/or session.
pub fn list(file_path: Option<&str>, session_id: Option<&str>) -> Result<Vec<CheckpointInfo>> {
    let conn = open_db()?;
    let mut sql = String::from(
        "SELECT id, session_id, file_path, snapshot_path, created_ms, reason, size_bytes
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
    sql.push_str(" ORDER BY created_ms DESC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| AutoDevError::Git(format!("prepare: {e}")))?;
    let params_refs: Vec<&dyn rusqlite::ToSql> =
        args.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(params_refs.as_slice(), |r| {
            Ok(CheckpointInfo {
                id: r.get(0)?,
                session_id: r.get(1)?,
                file_path: r.get(2)?,
                snapshot_path: r.get(3)?,
                created_ms: r.get(4)?,
                reason: r.get(5)?,
                size_bytes: r.get::<_, i64>(6)? as u64,
            })
        })
        .map_err(|e| AutoDevError::Git(format!("query: {e}")))?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| AutoDevError::Git(e.to_string()))?);
    }
    Ok(out)
}

pub fn delete(checkpoint_id: &str) -> Result<()> {
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
    conn.execute(
        "DELETE FROM checkpoints WHERE id = ?1",
        params![checkpoint_id],
    )
    .map_err(|e| AutoDevError::Git(format!("delete: {e}")))?;
    Ok(())
}

/// Produce a diff between two snapshots (old → new) using git's patch format.
pub fn compare(id_old: &str, id_new: &str) -> Result<DiffResult> {
    let conn = open_db()?;
    let get_snap = |id: &str| -> Result<String> {
        conn.query_row(
            "SELECT snapshot_path FROM checkpoints WHERE id = ?1",
            params![id],
            |r| r.get::<_, String>(0),
        )
        .map_err(|e| AutoDevError::Git(format!("lookup {id}: {e}")))
    };
    let old_path = get_snap(id_old)?;
    let new_path = get_snap(id_new)?;
    let old_bytes = std::fs::read(&old_path)?;
    let new_bytes = std::fs::read(&new_path)?;
    Ok(diff::diff_blobs(&old_bytes, &new_bytes))
}

/// Evict old checkpoints for `file_path` beyond RETENTION_PER_FILE.
fn prune(file_path: &str) -> Result<()> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, snapshot_path FROM checkpoints
         WHERE file_path = ?1 ORDER BY created_ms DESC",
        )
        .map_err(|e| AutoDevError::Git(e.to_string()))?;
    let rows = stmt
        .query_map(params![file_path], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| AutoDevError::Git(e.to_string()))?;

    let mut keepers_seen = 0;
    let mut to_delete: Vec<(String, String)> = Vec::new();
    for r in rows {
        let (id, path) = r.map_err(|e| AutoDevError::Git(e.to_string()))?;
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
/// tool. Errors are logged, never propagated — checkpointing must never
/// block the actual edit.
pub fn auto_checkpoint_before_write(session_id: &str, file_path: &str) {
    if let Err(e) = create(session_id, file_path, "pre_write") {
        tracing::debug!("[checkpoint] skipping for {file_path}: {e}");
    }
}
