// Where a workspace's graph lives, and the one spelling of its root (P5-01..03).
//
// Two things used to depend on how the caller happened to spell the workspace
// path, and both made one workspace into several graphs:
//
//   * The database file. The desktop host hashed the raw string, so `C:\a`,
//     `c:\a`, `C:\a\` and `C:/a` were four databases.
//   * The node keys. A file node's `qualified_name` is the path the extractor
//     was handed, so even one database held disjoint node sets per spelling.
//
// Fixing only the first leaves the second. So every entry point resolves the
// root here, once, and builds every key from the resolved root; the database
// name is derived from the shared `path_key` (the same helper `path_lock` uses)
// so the two cannot drift apart again.

use crate::errors::{ArcanaError, Result};
use crate::security::path_key::{path_key, strip_verbatim};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A workspace's resolved root and its graph database.
#[derive(Debug, Clone)]
pub struct GraphLocation {
    /// The workspace root as the filesystem spells it: symlinks resolved, no
    /// Windows verbatim prefix, on-disk letter case. Build every node key from
    /// this, never from the string the caller passed.
    pub root: PathBuf,
    /// `<arcana dir>/graphs/<hash of the canonical key>.db`.
    pub db_path: PathBuf,
}

/// The workspace root as the filesystem spells it.
pub fn canonical_workspace(workspace: &Path) -> Result<PathBuf> {
    let resolved = workspace.canonicalize().map_err(|error| {
        ArcanaError::Agent(format!(
            "Workspace directory is not readable: {} ({error})",
            workspace.display()
        ))
    })?;
    Ok(strip_verbatim(&resolved))
}

fn stem_for(key: &str) -> String {
    let digest = hex::encode(Sha256::digest(key.as_bytes()));
    digest[..12].to_string()
}

fn graphs_dir() -> Result<PathBuf> {
    Ok(crate::db::arcana_dir()?.join("graphs"))
}

/// The database path for a workspace: one path for every spelling of it.
pub fn graph_db_path(workspace: &Path) -> Result<PathBuf> {
    Ok(graphs_dir()?.join(format!("{}.db", stem_for(&path_key(workspace)))))
}

/// Resolve a workspace as the caller spelled it, and make sure its graph can be
/// opened: create the graphs directory, and adopt a graph that an older build
/// stored under a raw-spelling name when the canonical one does not exist yet.
pub fn locate(workspace_as_given: &str) -> Result<GraphLocation> {
    let root = canonical_workspace(Path::new(workspace_as_given))?;
    let db_path = graph_db_path(&root)?;
    let parent = db_path
        .parent()
        .ok_or_else(|| ArcanaError::Agent("Graph database path has no directory".into()))?;
    std::fs::create_dir_all(parent)?;
    if !db_path.exists() {
        adopt_legacy_graph(workspace_as_given, &root, &db_path)?;
    }
    Ok(GraphLocation { root, db_path })
}

/// The location of a workspace's graph **only if it has been built**: no graph
/// database and no `graphs` directory is created and nothing is copied, so a
/// read (prompt assembly, a tool call) can never leave an empty database behind.
/// (Resolving the path ensures the app's own `~/.arcana` directory exists, as
/// every other store does.) A graph stored under a legacy spelling
/// is adopted by `locate`, which the Graph tab calls; until then this reports
/// none.
pub fn find_existing(workspace_as_given: &str) -> Result<Option<GraphLocation>> {
    let root = canonical_workspace(Path::new(workspace_as_given))?;
    let db_path = graph_db_path(&root)?;
    Ok(db_path.is_file().then_some(GraphLocation { root, db_path }))
}

/// The strings a file may be stored under in this workspace's graph, for a path
/// as a caller wrote it (relative to the root, or absolute; either separator).
/// Pure string work: it never touches the disk, so a path outside the
/// workspace simply matches nothing.
pub fn file_key_candidates(root: &Path, given: &str) -> Vec<String> {
    let given = given.trim();
    let path = Path::new(given);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    // Lexical clean-up only (`.` dropped, `..` popped): the path is a key, so
    // nothing here touches the disk and nothing can escape by being resolved.
    let mut cleaned = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                cleaned.pop();
            }
            other => cleaned.push(other.as_os_str()),
        }
    }
    let plain = strip_verbatim(&cleaned).display().to_string();
    let mut out = Vec::new();
    for form in [
        plain.clone(),
        plain.replace('\\', "/"),
        plain.replace('/', "\\"),
        given.to_string(),
    ] {
        if !form.is_empty() && !out.contains(&form) {
            out.push(form);
        }
    }
    out
}

/// Every spelling an older build could have hashed for this workspace: the
/// string as given, its slash and trailing-separator variants, and the
/// resolved forms.
fn legacy_spellings(workspace_as_given: &str, root: &Path) -> BTreeSet<String> {
    let mut bases = vec![workspace_as_given.to_string(), root.display().to_string()];
    bases.push(format!(r"\\?\{}", root.display()));
    let mut spellings = BTreeSet::new();
    for base in bases {
        for form in [
            base.clone(),
            base.replace('\\', "/"),
            base.replace('/', "\\"),
        ] {
            let trimmed = form.trim_end_matches(['/', '\\']).to_string();
            if trimmed.is_empty() {
                continue;
            }
            spellings.insert(format!("{trimmed}\\"));
            spellings.insert(format!("{trimmed}/"));
            spellings.insert(trimmed);
        }
    }
    spellings
}

fn node_count(db: &Path) -> i64 {
    rusqlite::Connection::open_with_flags(
        db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .and_then(|conn| conn.query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0)))
    .unwrap_or(0)
}

/// Copy the newest non-empty legacy graph to the canonical name. The legacy
/// file is never moved or modified: the graph is derived data, but it carries
/// worker locks and authorship that a rebuild does not recreate, so the original
/// stays until a person removes it.
fn adopt_legacy_graph(workspace_as_given: &str, root: &Path, canonical_db: &Path) -> Result<()> {
    let graphs = match canonical_db.parent() {
        Some(parent) => parent,
        None => return Ok(()),
    };
    let canonical_name = canonical_db.file_name().map(|n| n.to_os_string());
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for spelling in legacy_spellings(workspace_as_given, root) {
        let path = graphs.join(format!("{}.db", stem_for(&spelling)));
        if Some(path.file_name().unwrap_or_default().to_os_string()) == canonical_name {
            continue;
        }
        if !path.is_file() || node_count(&path) == 0 {
            continue;
        }
        let modified = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        candidates.push((modified, path));
    }
    candidates.sort();
    let Some((_, source)) = candidates.pop() else {
        return Ok(());
    };

    // `backup_database_in` takes a consistent snapshot (`VACUUM INTO`, which
    // includes uncheckpointed WAL) and verifies it; reusing it avoids a second
    // copy mechanism. The snapshot lands in a scratch directory beside the
    // target so the final step is a same-volume link that cannot overwrite.
    let scratch = graphs.join(format!(".adopt-{}", uuid::Uuid::new_v4()));
    let snapshot = crate::db::backup::backup_database_in(
        &scratch,
        &source,
        "graph-adopt-legacy",
        &crate::db::backup::timestamp_now(),
    )?;
    let linked = std::fs::hard_link(&snapshot.path, canonical_db);
    let _ = std::fs::remove_dir_all(&scratch);
    match linked {
        Ok(()) => {
            tracing::info!(
                target: "arcana::telemetry",
                event_type = "graph_legacy_adopted",
                bytes = snapshot.bytes,
                "Copied a graph stored under an older path spelling to its canonical name"
            );
            Ok(())
        }
        // Another caller adopted or built it first; theirs stands.
        Err(error) if canonical_db.exists() => {
            tracing::debug!("graph adoption lost a race: {error}");
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::schema::NodeInfo;
    use crate::graph::store::GraphStore;

    fn node(name: &str) -> NodeInfo {
        NodeInfo {
            kind: "function".into(),
            name: name.into(),
            qualified_name: name.into(),
            file_path: "src/a.rs".into(),
            line_start: 1,
            line_end: 2,
            language: "rust".into(),
            parent_name: None,
            params: None,
            return_type: None,
            modifiers: None,
            is_test: false,
            file_hash: "h".into(),
        }
    }

    /// A workspace directory and an isolated home, with the home lock held.
    struct Fixture {
        _home_guard: crate::db::test_support::HomeGuard,
        _home: tempfile::TempDir,
        workspace_dir: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let home = tempfile::tempdir().expect("isolated home");
        let guard = crate::db::test_support::with_home(home.path());
        let workspace_dir = tempfile::tempdir().expect("workspace");
        std::fs::create_dir(workspace_dir.path().join("proj")).expect("workspace root");
        Fixture {
            _home_guard: guard,
            _home: home,
            workspace_dir,
        }
    }

    fn graph_files() -> Vec<String> {
        let dir = graphs_dir().expect("graphs dir");
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.file_name().to_string_lossy().to_string())
                    .filter(|name| name.ends_with(".db"))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// S5-01: the same tree reached by different spellings is one database and
    /// one root. The spellings are the ones the live host could receive.
    #[test]
    fn every_spelling_of_a_workspace_is_one_root_and_one_database() {
        let f = fixture();
        let root = f.workspace_dir.path().join("proj");
        let plain = root.display().to_string();
        let mut spellings = vec![
            plain.clone(),
            format!("{plain}{}", std::path::MAIN_SEPARATOR),
            format!("{plain}{0}.{0}", std::path::MAIN_SEPARATOR),
            f.workspace_dir
                .path()
                .join("proj")
                .join("..")
                .join("proj")
                .display()
                .to_string(),
        ];
        if cfg!(windows) {
            spellings.push(plain.replace('\\', "/"));
            spellings.push(plain.to_lowercase());
            spellings.push(plain.to_uppercase());
            spellings.push(format!(r"\\?\{plain}"));
        }

        let mut roots = BTreeSet::new();
        let mut dbs = BTreeSet::new();
        for spelling in &spellings {
            let location = locate(spelling).unwrap_or_else(|e| panic!("{spelling}: {e}"));
            // Opening it is what used to create the extra file.
            GraphStore::open(&location.db_path).expect("open graph");
            roots.insert(location.root.display().to_string());
            dbs.insert(location.db_path.display().to_string());
        }
        assert_eq!(roots.len(), 1, "one root for {spellings:?}, got {roots:?}");
        assert_eq!(dbs.len(), 1, "one database for {spellings:?}, got {dbs:?}");
        assert_eq!(graph_files().len(), 1, "one .db file on disk");
    }

    /// Without the shared key the spellings would hash apart; this is the
    /// control that shows the old derivation really produced several names.
    #[test]
    fn hashing_the_raw_string_gives_a_different_name_per_spelling() {
        let a = stem_for(r"C:\work\proj");
        let b = stem_for(r"c:\work\proj");
        let c = stem_for(r"C:\work\proj\");
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    /// P5-03: a graph stored under the raw-spelling name is carried over, and
    /// the original is left exactly where it was.
    #[test]
    fn a_legacy_graph_is_copied_to_the_canonical_name_and_left_in_place() {
        let f = fixture();
        let given = f.workspace_dir.path().join("proj").display().to_string();
        let legacy = graphs_dir()
            .expect("graphs dir")
            .join(format!("{}.db", stem_for(&given)));
        std::fs::create_dir_all(legacy.parent().expect("parent")).expect("graphs dir");
        {
            let store = GraphStore::open(&legacy).expect("legacy store");
            store
                .upsert_node(&node("legacy_symbol"), None, None)
                .expect("seed legacy node");
            store.flush().expect("checkpoint legacy graph");
        }
        let legacy_bytes = std::fs::read(&legacy).expect("read legacy bytes");

        let location = locate(&given).expect("locate");
        assert_ne!(location.db_path, legacy, "the canonical name differs");
        assert_eq!(
            node_count(&location.db_path),
            1,
            "the node was carried over"
        );
        assert_eq!(
            std::fs::read(&legacy).expect("legacy still readable"),
            legacy_bytes,
            "the legacy file is untouched"
        );
        assert!(
            std::fs::read_dir(graphs_dir().expect("graphs dir"))
                .expect("list")
                .filter_map(|e| e.ok())
                .all(|e| !e.file_name().to_string_lossy().starts_with(".adopt-")),
            "the scratch directory is removed"
        );
    }

    #[test]
    fn an_empty_legacy_graph_is_not_adopted() {
        let f = fixture();
        let given = f.workspace_dir.path().join("proj").display().to_string();
        let legacy = graphs_dir()
            .expect("graphs dir")
            .join(format!("{}.db", stem_for(&given)));
        std::fs::create_dir_all(legacy.parent().expect("parent")).expect("graphs dir");
        GraphStore::open(&legacy).expect("empty legacy store");

        let location = locate(&given).expect("locate");
        assert!(!location.db_path.exists(), "nothing to carry over");
    }

    #[test]
    fn an_existing_canonical_graph_is_never_overwritten_by_a_legacy_one() {
        let f = fixture();
        let given = f.workspace_dir.path().join("proj").display().to_string();
        let location = locate(&given).expect("locate");
        {
            let store = GraphStore::open(&location.db_path).expect("canonical store");
            store
                .upsert_node(&node("canonical_symbol"), None, None)
                .expect("seed canonical");
            store.flush().expect("checkpoint");
        }
        let legacy = graphs_dir()
            .expect("graphs dir")
            .join(format!("{}.db", stem_for(&given)));
        if legacy != location.db_path {
            let store = GraphStore::open(&legacy).expect("legacy store");
            store
                .upsert_node(&node("legacy_symbol"), None, None)
                .expect("seed legacy");
            store
                .upsert_node(&node("second"), None, None)
                .expect("seed");
            store.flush().expect("checkpoint");
        }

        let again = locate(&given).expect("locate again");
        assert_eq!(again.db_path, location.db_path);
        assert_eq!(node_count(&again.db_path), 1, "canonical content kept");
    }

    #[test]
    fn a_missing_workspace_is_an_error_not_a_new_database() {
        let f = fixture();
        let missing = f.workspace_dir.path().join("does-not-exist");
        assert!(locate(&missing.display().to_string()).is_err());
        assert!(graph_files().is_empty(), "no database was created");
    }
}
