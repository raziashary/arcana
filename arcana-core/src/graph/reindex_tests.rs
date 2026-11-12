// R2 proofs (P5-04..07, S5-02): a re-index keeps every edge it does not touch,
// removes only what the workspace no longer has, and the v7 migration backs up
// before it deduplicates. They run the real detector, the real extractor and the
// real store: a hand-built graph would not exercise the keys the extractor makes.

use super::build::{reindex, BuildStats};
use super::detect::CodeDetector;
use super::extract::extract_all;
use super::migrations::SCHEMA_HEAD;
use super::schema::NodeInfo;
use super::store::GraphStore;
use rusqlite::{params, Connection};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

fn write(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write fixture file");
}

/// The shipped path of `update_graph`: detect, extract, re-index.
fn index(root: &Path, store: &GraphStore) -> BuildStats {
    let files = CodeDetector::new(root).detect_files().expect("detect");
    let workspace: HashSet<String> = files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    let extractions: Vec<_> = extract_all(&files, root)
        .into_iter()
        .filter_map(|(_, result)| result.ok())
        .collect();
    reindex(extractions, &workspace, store, None, None).expect("reindex")
}

type Edge = (String, String, String, String, i32);

fn edges(store: &GraphStore) -> BTreeSet<Edge> {
    store
        .get_all_edges_for_graph(i64::MAX, 0)
        .expect("edges")
        .into_iter()
        .map(|e| {
            (
                e.kind,
                e.source_qualified,
                e.target_qualified,
                e.file_path,
                e.line,
            )
        })
        .collect()
}

fn edge_rows(store: &GraphStore) -> usize {
    store
        .get_all_edges_for_graph(i64::MAX, 0)
        .expect("edges")
        .len()
}

fn node_names(store: &GraphStore) -> BTreeSet<String> {
    store
        .get_all_nodes_for_graph(i64::MAX, 0)
        .expect("nodes")
        .into_iter()
        .map(|n| n.name)
        .collect()
}

/// `a.py` defines `foo`; `b.py` imports it with a relative import (the form the
/// extractor resolves to the real file node), so `b` has an edge into `a`.
fn two_file_workspace() -> (tempfile::TempDir, tempfile::TempDir, GraphStore) {
    let dir = tempfile::tempdir().expect("workspace");
    write(dir.path(), "a.py", "def foo():\n    return 1\n");
    write(
        dir.path(),
        "b.py",
        "from .a import foo\n\n\ndef bar():\n    return foo()\n",
    );
    let db = tempfile::tempdir().expect("db dir");
    let store = GraphStore::open(&db.path().join("graph.db")).expect("open store");
    // The store holds only a path, so the database directory is returned to be
    // kept alive for the length of the test.
    (dir, db, store)
}

fn cross_file_edges(store: &GraphStore) -> Vec<Edge> {
    edges(store)
        .into_iter()
        .filter(|(_, _, target, file, _)| file.ends_with("b.py") && target.contains("a.py"))
        .collect()
}

#[test]
fn the_fixture_really_has_edges_from_b_into_a() {
    let (dir, _db, store) = two_file_workspace();
    index(dir.path(), &store);
    assert!(
        !cross_file_edges(&store).is_empty(),
        "without cross-file edges these tests prove nothing; got {:?}",
        edges(&store)
    );
}

/// P5-06: re-indexing the same tree changes nothing. Before v7 every re-index
/// appended a second copy of every edge.
#[test]
fn reindexing_an_unchanged_tree_keeps_exactly_the_same_nodes_and_edges() {
    let (dir, _db, store) = two_file_workspace();
    index(dir.path(), &store);
    let first_nodes = node_names(&store);
    let first_edges = edges(&store);
    let first_rows = edge_rows(&store);

    let second = index(dir.path(), &store);
    assert_eq!(node_names(&store), first_nodes);
    assert_eq!(edges(&store), first_edges);
    assert_eq!(edge_rows(&store), first_rows, "no duplicate edge rows");
    assert_eq!((second.nodes_pruned, second.edges_pruned), (0, 0));
}

/// S5-02: change `a.py`; every edge that was not about the change survives,
/// including the edges from the unchanged `b.py` into `a`'s nodes. The deleted
/// incremental path lost exactly these.
#[test]
fn changing_a_file_keeps_edges_from_unchanged_files_into_it() {
    let (dir, _db, store) = two_file_workspace();
    index(dir.path(), &store);
    let before = edges(&store);
    let cross_before = cross_file_edges(&store);

    write(
        dir.path(),
        "a.py",
        "def foo():\n    return 2\n\n\ndef baz():\n    return 3\n",
    );
    index(dir.path(), &store);

    assert!(
        node_names(&store).contains("baz"),
        "the new symbol is added"
    );
    let after = edges(&store);
    for edge in &before {
        assert!(
            after.contains(edge),
            "edge lost across the re-index: {edge:?}"
        );
    }
    assert_eq!(cross_file_edges(&store), cross_before);
}

/// What the workspace no longer has stops being cited.
#[test]
fn a_deleted_symbol_and_a_deleted_file_are_pruned_with_their_edges() {
    let (dir, _db, store) = two_file_workspace();
    write(dir.path(), "c.py", "def lonely():\n    return 0\n");
    index(dir.path(), &store);
    assert!(node_names(&store).contains("lonely"));

    write(dir.path(), "a.py", "def other():\n    return 1\n");
    std::fs::remove_file(dir.path().join("c.py")).expect("delete c.py");
    let stats = index(dir.path(), &store);

    let names = node_names(&store);
    assert!(
        !names.contains("foo"),
        "a removed symbol is pruned: {names:?}"
    );
    assert!(
        !names.contains("lonely"),
        "a removed file's nodes are pruned"
    );
    assert!(names.contains("other") && names.contains("bar"));
    assert!(stats.nodes_pruned >= 2, "{stats:?}");
    assert!(
        edges(&store).iter().all(|(_, source, target, _, _)| {
            !source.contains("lonely") && !target.contains("lonely")
        }),
        "no edge of a pruned node remains"
    );
}

/// One parse failure must not wipe part of the graph.
#[test]
fn a_file_that_fails_to_extract_keeps_its_rows() {
    let (dir, _db, store) = two_file_workspace();
    index(dir.path(), &store);
    let b_nodes_before: BTreeSet<String> = node_names(&store);
    let b_edges_before: Vec<Edge> = edges(&store)
        .into_iter()
        .filter(|(_, _, _, file, _)| file.ends_with("b.py"))
        .collect();
    assert!(!b_edges_before.is_empty());

    // b.py is in the workspace but its extraction is missing from this run.
    let files = CodeDetector::new(dir.path())
        .detect_files()
        .expect("detect");
    let workspace: HashSet<String> = files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    let only_a: Vec<_> = files
        .iter()
        .filter(|path| path.ends_with("a.py"))
        .cloned()
        .collect();
    let extractions: Vec<_> = extract_all(&only_a, dir.path())
        .into_iter()
        .filter_map(|(_, result)| result.ok())
        .collect();
    reindex(extractions, &workspace, &store, None, None).expect("reindex");

    assert_eq!(node_names(&store), b_nodes_before);
    let b_edges_after: Vec<Edge> = edges(&store)
        .into_iter()
        .filter(|(_, _, _, file, _)| file.ends_with("b.py"))
        .collect();
    assert_eq!(b_edges_after, b_edges_before);
}

fn node(name: &str) -> NodeInfo {
    NodeInfo {
        kind: "function".into(),
        name: name.into(),
        qualified_name: name.into(),
        file_path: "src/x.rs".into(),
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

fn seed_pair(conn: &Connection) {
    conn.execute_batch("PRAGMA foreign_keys=ON").expect("fk");
    for name in ["x::a", "x::b"] {
        conn.execute(
            "INSERT INTO nodes (kind, name, qualified_name, file_path, line_start, line_end, language, file_hash)
             VALUES ('function', ?, ?, 'src/x.rs', 1, 2, 'rust', 'h')",
            params![name, name],
        )
        .expect("seed node");
    }
    conn.execute(
        "INSERT INTO edges (kind, source_qualified, target_qualified, file_path, line)
         VALUES ('calls', 'x::a', 'x::b', 'src/x.rs', 1)",
        [],
    )
    .expect("seed edge");
}

/// The instrument: the old statement, run against this schema, deletes the
/// edge. If this stops failing the way it should, the next test proves nothing.
#[test]
fn control_insert_or_replace_on_a_node_cascade_deletes_its_edges() {
    let db = tempfile::tempdir().expect("db dir");
    let path = db.path().join("graph.db");
    drop(GraphStore::open(&path).expect("open"));
    let conn = Connection::open(&path).expect("raw");
    seed_pair(&conn);
    conn.execute(
        "INSERT OR REPLACE INTO nodes (kind, name, qualified_name, file_path, line_start, line_end, language, file_hash)
         VALUES ('function', 'x::b', 'x::b', 'src/x.rs', 1, 2, 'rust', 'h2')",
        [],
    )
    .expect("old-style replace");
    let remaining: i64 = conn
        .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))
        .expect("count");
    assert_eq!(remaining, 0, "the old statement lost the edge");
}

/// The fix: an upsert updates in place, keeps the edge, and keeps the columns
/// the writer does not supply (community, worker lock, worktree).
#[test]
fn upserting_a_node_keeps_its_edges_and_the_columns_the_writer_does_not_supply() {
    let db = tempfile::tempdir().expect("db dir");
    let path = db.path().join("graph.db");
    let store = GraphStore::open(&path).expect("open");
    let conn = Connection::open(&path).expect("raw");
    seed_pair(&conn);
    conn.execute(
        "UPDATE nodes SET community = 7, community_name = 'core', worker_locked = 1, worktree = 'wt'
         WHERE qualified_name = 'x::b'",
        [],
    )
    .expect("set kept columns");

    let mut changed = node("x::b");
    changed.file_hash = "h2".into();
    store
        .upsert_nodes_batch(&[changed.clone()], Some("agent-1"), Some(5))
        .expect("batch upsert");
    let rowid_one = store
        .upsert_node(&changed, None, None)
        .expect("single upsert");
    assert!(rowid_one > 0);

    let edges_left: i64 = conn
        .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))
        .expect("count");
    assert_eq!(edges_left, 1, "the edge survives");
    let (hash, community, locked, worktree, author): (String, i64, i64, String, String) = conn
        .query_row(
            "SELECT file_hash, community, worker_locked, worktree, agent_author
             FROM nodes WHERE qualified_name = 'x::b'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("read back");
    assert_eq!(hash, "h2", "the supplied column updated");
    assert_eq!((community, locked, worktree.as_str()), (7, 1, "wt"));
    assert_eq!(
        author, "agent-1",
        "an unset author does not erase a set one"
    );
}

#[test]
fn upserting_the_same_edge_twice_is_one_row_and_the_id_is_stable() {
    let db = tempfile::tempdir().expect("db dir");
    let store = GraphStore::open(&db.path().join("graph.db")).expect("open");
    store
        .upsert_nodes_batch(&[node("x::a"), node("x::b")], None, None)
        .expect("nodes");
    let edge = super::schema::EdgeInfo {
        kind: "calls".into(),
        source_qualified: "x::a".into(),
        target_qualified: "x::b".into(),
        file_path: "src/x.rs".into(),
        line: 3,
        confidence: 1.0,
        confidence_tier: "certain".into(),
        weight: 1.0,
        relation: None,
    };
    let first = store.upsert_edge(&edge, None, None).expect("first");
    let second = store.upsert_edge(&edge, None, None).expect("second");
    assert_eq!(first, second, "the same edge keeps its id");
    store
        .upsert_edges_batch(&[edge.clone(), edge], None, None)
        .expect("batch of duplicates");
    assert_eq!(edge_rows(&store), 1);
}

/// v7 on a database that already holds duplicate edges: backed up first, then
/// deduplicated, then unique.
#[test]
fn migrating_a_v6_graph_backs_it_up_and_removes_duplicate_edges() {
    let db = tempfile::tempdir().expect("db dir");
    let path = db.path().join("graph.db");
    drop(GraphStore::open(&path).expect("create"));
    {
        let conn = Connection::open(&path).expect("raw");
        conn.execute_batch("DROP INDEX idx_edges_identity; PRAGMA user_version = 6;")
            .expect("rewind to v6");
        seed_pair(&conn);
        for _ in 0..2 {
            conn.execute(
                "INSERT INTO edges (kind, source_qualified, target_qualified, file_path, line)
                 VALUES ('calls', 'x::a', 'x::b', 'src/x.rs', 1)",
                [],
            )
            .expect("duplicate edge");
        }
    }

    let store = GraphStore::open(&path).expect("migrating open");
    assert_eq!(store.schema_version().expect("version"), SCHEMA_HEAD);
    assert_eq!(edge_rows(&store), 1, "duplicates removed");
    let conn = Connection::open(&path).expect("raw");
    let indexed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_edges_identity'",
            [],
            |row| row.get(0),
        )
        .expect("index lookup");
    assert_eq!(indexed, 1, "the unique index exists");
    assert!(
        conn.execute(
            "INSERT INTO edges (kind, source_qualified, target_qualified, file_path, line)
             VALUES ('calls', 'x::a', 'x::b', 'src/x.rs', 1)",
            [],
        )
        .is_err(),
        "a duplicate edge is now refused"
    );

    let backups = db.path().join(".arcana").join("backups");
    let backup = std::fs::read_dir(&backups)
        .expect("backup directory exists beside the fixture")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains("graph-migration-v"))
        .expect("a graph migration backup was taken");
    let saved = Connection::open(&backup).expect("open backup");
    let saved_edges: i64 = saved
        .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))
        .expect("count");
    assert_eq!(saved_edges, 3, "the backup holds the pre-migration rows");
}

/// A database already at the head is not backed up on every open.
#[test]
fn opening_a_current_graph_takes_no_backup() {
    let db = tempfile::tempdir().expect("db dir");
    let path = db.path().join("graph.db");
    drop(GraphStore::open(&path).expect("create"));
    drop(GraphStore::open(&path).expect("reopen"));
    assert!(
        !db.path().join(".arcana").exists(),
        "no backup directory appears for an up-to-date database"
    );
}

/// S5-01, end to end: the same tree indexed through different spellings of its
/// path is one database holding one set of node and edge keys, not disjoint
/// sets per spelling. Fixing only the database name would pass the file count
/// and fail this.
#[test]
fn indexing_one_tree_through_different_spellings_gives_one_graph() {
    let home = tempfile::tempdir().expect("isolated home");
    let _guard = crate::db::test_support::with_home(home.path());
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path().join("proj");
    std::fs::create_dir(&root).expect("workspace root");
    write(&root, "a.py", "def foo():\n    return 1\n");
    write(
        &root,
        "b.py",
        "from .a import foo\n\n\ndef bar():\n    return foo()\n",
    );

    let plain = root.display().to_string();
    let mut spellings = vec![
        plain.clone(),
        format!("{plain}{}", std::path::MAIN_SEPARATOR),
        format!("{plain}{0}.{0}", std::path::MAIN_SEPARATOR),
    ];
    if cfg!(windows) {
        spellings.push(plain.replace('\\', "/"));
        spellings.push(plain.to_lowercase());
        spellings.push(plain.to_uppercase());
    }

    let mut key_sets = BTreeSet::new();
    let mut databases = BTreeSet::new();
    let mut last_rows = (0, 0);
    for spelling in &spellings {
        let location = super::location::locate(spelling).expect("locate");
        let store = GraphStore::open(&location.db_path).expect("open");
        index(&location.root, &store);
        let nodes: BTreeSet<String> = store
            .get_all_nodes_for_graph(i64::MAX, 0)
            .expect("nodes")
            .into_iter()
            .map(|n| n.qualified_name)
            .collect();
        last_rows = (nodes.len(), edge_rows(&store));
        key_sets.insert((nodes, edges(&store)));
        databases.insert(location.db_path);
    }
    assert_eq!(databases.len(), 1, "one database for {spellings:?}");
    assert_eq!(
        key_sets.len(),
        1,
        "the same node and edge keys whichever spelling indexed them"
    );
    // The last run's database holds exactly that one set, with no leftovers.
    let (nodes, edge_count) = last_rows;
    let only = key_sets.into_iter().next().expect("one set");
    assert_eq!((nodes, edge_count), (only.0.len(), only.1.len()));
}
