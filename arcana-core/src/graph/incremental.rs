// What a graph refresh reports.
//
// The git-diff incremental path that used to live here (`get_git_changed_files`,
// `incremental_update`, `find_dependents`) was removed in R2 (D-013). It diffed
// only against `HEAD~1`, deleted each changed file's nodes before re-extracting
// them, and the `ON DELETE CASCADE` foreign keys took with them every edge from
// an unchanged file into those nodes, which nothing recreated. `update_graph`
// now does a full re-index with non-destructive upserts and a key-set prune
// (`build::reindex`). True incremental update is P5-08..13, deferred; it must
// start from that design, not from the deleted code.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStats {
    pub files_changed: usize,
    pub nodes_added: usize,
    pub nodes_removed: usize,
    pub edges_added: usize,
    pub edges_removed: usize,
    pub elapsed_ms: u64,
}
