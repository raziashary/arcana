fn legacy_created(raw: &str) {
    assert_eq!(
        crate::db::timestamps::parse_db_timestamp(raw).expect("reader timestamp parses"),
        crate::db::timestamps::parse_db_timestamp("2024-02-29 12:34:56").unwrap(),
        "reader retains original creation instant"
    );
}

fn legacy_resolved(raw: &str) {
    assert_eq!(
        crate::db::timestamps::parse_db_timestamp(raw).expect("reader timestamp parses"),
        crate::db::timestamps::parse_db_timestamp("2024-11-03T01:30:00-04:00").unwrap(),
        "reader retains original offset instant"
    );
}

#[test]
fn task_auxiliary_timestamp_readers_preserve_legacy_rows_after_reopen() {
    use crate::db::tasks::{
        CreateTaskInput, CreateWorkerRequestInput, RoutingHistoryEntry, TasksDb,
    };
    let temp = tempfile::tempdir().expect("isolated task legacy home");
    let path = temp.path().join("tasks.db");
    let db = TasksDb::open_at(&path).expect("open task store");
    let task = db
        .create(CreateTaskInput {
            title: "legacy auxiliary timestamps".into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "legacy reader".into(),
            created_by: Some("clock-test".into()),
        })
        .expect("create real task parent");
    let request = db
        .create_worker_request(&CreateWorkerRequestInput {
            worker_slug: "worker-legacy".into(),
            task_id: Some(task.id),
            request_type: "question".into(),
            payload: "{}".into(),
        })
        .expect("create worker request");
    db.update_worker_request_status(request.id, "completed", Some("resolved"))
        .expect("resolve worker request");
    let message = db
        .insert_worker_message(
            "worker-legacy",
            "supervisor",
            "legacy message",
            "nudge",
            Some(task.id),
        )
        .expect("insert worker message");
    let route = RoutingHistoryEntry {
        id: "legacy-route".into(),
        task_id: task.id.to_string(),
        task_title: task.title.clone(),
        task_type: "feature".into(),
        worker_slug: "worker-legacy".into(),
        assigned_at: "2024-02-29 12:34:56".into(),
        outcome: Some("success".into()),
        outcome_at: Some("2024-11-03T01:30:00-04:00".into()),
        duration_ms: Some(10),
        tokens_used: Some(1),
    };
    db.assign_with_routing_history(task.id, "worker-legacy", &route)
        .expect("assign task and insert automatic routing row");
    db.insert_build_evidence(task.id, "legacy-session", "cargo test", 0, true, "cargo")
        .expect("insert real build evidence");

    let budget_task = db
        .create(CreateTaskInput {
            title: "legacy budget audit".into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "budget audit".into(),
            created_by: Some("clock-test".into()),
        })
        .expect("create budget task");
    db.set_task_budget(budget_task.id, Some(100), Some(1.0), Some(4))
        .expect("set task budget");
    db.update_task_budget_usage(budget_task.id, 100, 1.0, 4)
        .expect("record task budget usage");
    db.update_status(budget_task.id, "assigned", Some("worker-legacy"))
        .expect("assign task");
    db.claim_task_with_pid(budget_task.id, "worker-legacy", 782)
        .expect("claim task");
    db.mark_task_budget_exceeded(budget_task.id, "saved cap reached")
        .expect("hold exceeded budget");
    db.resolve_exceeded_task_budget(
        budget_task.id,
        200,
        2.0,
        8,
        "supervisor",
        "reviewed legacy fixture",
    )
    .expect("create budget resolution audit");

    db.conn
        .execute_batch(
            "DROP TRIGGER IF EXISTS worker_requests_timestamps_canonical_update;
         DROP TRIGGER IF EXISTS worker_messages_timestamps_canonical_update;
         DROP TRIGGER IF EXISTS routing_history_timestamps_canonical_insert;
         DROP TRIGGER IF EXISTS routing_history_timestamps_canonical_update;
         DROP TRIGGER IF EXISTS build_evidence_timestamps_canonical_update;
         DROP TRIGGER IF EXISTS task_budget_resolution_update_guard;",
        )
        .expect("prepare isolated legacy-format rows");
    db.conn.execute(
        "UPDATE worker_requests SET created_at='2024-02-29 12:34:56', resolved_at='2024-11-03T01:30:00-04:00' WHERE id=?1",
        [request.id],
    ).expect("seed legacy request timestamps");
    db.conn.execute(
        "UPDATE worker_messages SET created_at='2024-02-29 12:34:56', read_at='2024-11-03T01:30:00-04:00' WHERE id=?1",
        [message.id],
    ).expect("seed legacy message timestamps");
    db.conn.execute(
        "UPDATE routing_history SET assigned_at='2024-02-29 12:34:56', outcome_at='2024-11-03T01:30:00-04:00' WHERE id='legacy-route'",
        [],
    ).expect("seed legacy routing timestamps");
    db.conn
        .execute(
            "UPDATE build_evidence SET created_at='2024-02-29 12:34:56' WHERE task_id=?1",
            [task.id],
        )
        .expect("seed legacy build-evidence timestamp");
    db.conn.execute(
        "UPDATE task_budget_resolutions SET resolved_at='2024-11-03T01:30:00-04:00' WHERE task_id=?1",
        [budget_task.id],
    ).expect("seed legacy budget timestamp");
    drop(db);

    let reopened = TasksDb::open_at(&path).expect("reopen task store");
    let request = reopened
        .get_worker_request_by_id(request.id)
        .unwrap()
        .unwrap();
    legacy_created(&request.created_at);
    legacy_resolved(request.resolved_at.as_deref().unwrap());
    let message = reopened.get_worker_message(message.id).unwrap().unwrap();
    legacy_created(&message.created_at);
    legacy_resolved(message.read_at.as_deref().unwrap());
    let route = reopened.get_routing_history(10).unwrap().remove(0);
    legacy_created(&route.assigned_at);
    legacy_resolved(route.outcome_at.as_deref().unwrap());
    legacy_created(
        &reopened
            .latest_build_evidence(task.id)
            .unwrap()
            .unwrap()
            .created_at,
    );
    let history = reopened
        .get_task_budget_resolution_history(budget_task.id, 10)
        .unwrap();
    legacy_resolved(&history[0].resolved_at);
}

#[test]
fn run_health_and_journal_timestamp_readers_preserve_legacy_rows_after_reopen() {
    use crate::db::runs::RunsDb;
    let temp = tempfile::tempdir().expect("isolated runs legacy home");
    let path = temp.path().join("runs.db");
    let db = RunsDb::open_at(&path).expect("open runs store");
    let run_id = db
        .begin_worker_tick("legacy-worker", "legacy-session")
        .unwrap();
    db.conn
        .execute(
            "UPDATE runs SET started_at='2024-02-29 12:34:56' WHERE id=?1",
            [&run_id],
        )
        .unwrap();
    db.conn.execute(
        "UPDATE worker_run_state SET heartbeat_at='2024-11-03T01:30:00-04:00', last_progress_at='2024-02-29 12:34:56' WHERE run_id=?1",
        [&run_id],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO worker_run_events (run_id, event_at, operation, outcome) VALUES (?1, '2024-02-29 12:34:56', 'legacy-tool', 'ok')",
        [&run_id],
    ).unwrap();
    let intent_id = db.conn.last_insert_rowid();
    db.conn.execute(
        "INSERT INTO operation_journal (run_id, record_type, operation_id, operation, effect, target_paths, created_at) VALUES (?1, 'intent', 'legacy-operation', 'legacy-tool', 'read', '[]', '2024-02-29 12:34:56')",
        [&run_id],
    ).unwrap();
    let journal_id = db.conn.last_insert_rowid();
    db.conn.execute(
        "INSERT INTO operation_journal (run_id, record_type, parent_id, outcome, changed_paths, created_at) VALUES (?1, 'outcome', ?2, 'ok', '[]', '2024-11-03T01:30:00-04:00')",
        rusqlite::params![run_id, journal_id],
    ).unwrap();
    assert!(intent_id > 0);
    drop(db);

    let reopened = RunsDb::open_at(&path).expect("reopen runs store");
    let tick = reopened
        .latest_worker_tick("legacy-worker")
        .unwrap()
        .unwrap();
    legacy_resolved(&tick.heartbeat_at);
    legacy_created(tick.last_progress_at.as_deref().unwrap());
    let progress = reopened.recent_worker_progress(&run_id, 10).unwrap();
    assert_eq!(progress.len(), 1);
    legacy_created(&progress[0].0);
    let page = reopened.operation_journal_page(&run_id, 0, 10).unwrap();
    assert_eq!(page.entries.len(), 1);
    legacy_created(&page.entries[0].intent_at);
    legacy_resolved(page.entries[0].outcome_at.as_deref().unwrap());
}

#[test]
fn worker_plan_hook_config_timestamp_readers_preserve_legacy_rows_after_reopen() {
    use crate::db::plans::{CreatePlanInput, CreatePlanItemInput, PlanScope, PlansDb};
    use crate::db::workers::WorkersDb;
    use crate::hooks::db::HooksDb;
    use crate::hooks::types::{AddHookInput, HookAction, HookType};
    let temp = tempfile::tempdir().expect("isolated worker-plan-hook home");
    let workers_path = temp.path().join("workers.db");
    let plans_path = temp.path().join("plans.db");
    let config_path = temp.path().join("config.db");
    let workers = WorkersDb::open_at(&workers_path).unwrap();
    workers
        .create(
            "Legacy fixture",
            "legacy-worker",
            "backend",
            "gemini-3.8-flash",
            &[],
            120,
            None,
        )
        .unwrap();
    drop(workers);
    let raw = rusqlite::Connection::open(&workers_path).unwrap();
    raw.execute_batch("DROP TRIGGER IF EXISTS update_workers_timestamp; DROP TRIGGER IF EXISTS workers_timestamps_canonical_update;").unwrap();
    raw.execute(
        "UPDATE workers SET updated_at='2024-02-29 12:34:56' WHERE slug='legacy-worker'",
        [],
    )
    .unwrap();
    drop(raw);

    let plans = PlansDb::open_at(&plans_path).unwrap();
    let plan = plans
        .create_plan(CreatePlanInput {
            title: "legacy plan".into(),
            description: None,
            session_id: None,
            assigned_agent: None,
            created_by: None,
            agent_scope: PlanScope::Personal,
            parent_plan_id: None,
            task_id: None,
        })
        .unwrap();
    let item = plans
        .add_plan_item(CreatePlanItemInput {
            plan_id: plan.id.clone(),
            parent_id: None,
            title: "legacy item".into(),
            description: None,
            order_index: 0,
            agent_type: None,
            estimated_tools: None,
            assigned_worker: None,
            worktree_branch: None,
        })
        .unwrap();
    drop(plans);
    let raw = rusqlite::Connection::open(&plans_path).unwrap();
    raw.execute_batch("DROP TRIGGER IF EXISTS plans_updated_at; DROP TRIGGER IF EXISTS plans_timestamps_canonical_update; DROP TRIGGER IF EXISTS plan_items_timestamps_canonical_update;").unwrap();
    raw.execute(
        "UPDATE plans SET updated_at='2024-02-29 12:34:56' WHERE id=?1",
        [&plan.id],
    )
    .unwrap();
    raw.execute("UPDATE plan_items SET created_at='2024-02-29 12:34:56', completed_at='2024-11-03T01:30:00-04:00' WHERE id=?1", [&item.id]).unwrap();
    drop(raw);

    let hooks = HooksDb::open_at(&config_path).unwrap();
    let hook = hooks
        .insert(&AddHookInput {
            name: "legacy hook".into(),
            hook_type: HookType::SessionStart,
            enabled: Some(true),
            priority: Some(1),
            conditions: None,
            action: HookAction::Log {
                file: "test.log".into(),
                message: "legacy".into(),
            },
        })
        .unwrap();
    drop(hooks);
    let raw = rusqlite::Connection::open(&config_path).unwrap();
    raw.execute_batch("DROP TRIGGER IF EXISTS config_hooks_timestamp_default;")
        .unwrap();
    raw.execute("UPDATE hooks SET created_at='2024-02-29 12:34:56', updated_at='2024-11-03T01:30:00-04:00' WHERE id=?1", [&hook.id]).unwrap();
    drop(raw);

    let workers = WorkersDb::open_at(&workers_path).unwrap();
    legacy_created(
        &workers
            .get_by_slug("legacy-worker")
            .unwrap()
            .unwrap()
            .updated_at,
    );
    let plans = PlansDb::open_at(&plans_path).unwrap();
    legacy_created(&plans.get_plan(&plan.id).unwrap().unwrap().updated_at);
    let item = plans.get_plan_item(&item.id).unwrap().unwrap();
    legacy_created(&item.created_at);
    legacy_resolved(item.completed_at.as_deref().unwrap());
    let hooks = HooksDb::open_at(&config_path).unwrap();
    let hook = hooks.get(&hook.id).unwrap().unwrap();
    legacy_created(&hook.created_at);
    legacy_resolved(&hook.updated_at);
}
