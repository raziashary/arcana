//! Unit-test-only clock scopes for real generated-now writers.

use chrono::{DateTime, Utc};
use rusqlite::{functions::FunctionFlags, types::Value, Connection};
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Mutex;

thread_local! {
    static INSTANT: Cell<Option<DateTime<Utc>>> = const { Cell::new(None) };
}

/// A scope cannot move across threads: dropping it restores its origin thread.
pub(crate) struct ClockScope {
    previous: Option<DateTime<Utc>>,
    _thread_bound: PhantomData<Rc<()>>,
}

pub(crate) fn at(instant: DateTime<Utc>) -> ClockScope {
    ClockScope {
        previous: INSTANT.with(|clock| clock.replace(Some(instant))),
        _thread_bound: PhantomData,
    }
}

pub(crate) fn current() -> Option<DateTime<Utc>> {
    INSTANT.with(Cell::get)
}

impl Drop for ClockScope {
    fn drop(&mut self) {
        INSTANT.with(|clock| clock.set(self.previous));
    }
}

/// Preserve SQLite's implementation, including modifiers, NULL and numeric
/// results. The delegate has no overrides or production profiling attached.
pub(crate) fn install(conn: &Connection) -> rusqlite::Result<()> {
    if current().is_none() {
        return Ok(());
    }
    // Numeric conversions retain their built-in determinism for expression
    // indexes. Canonical generated-now writers use strftime, not julianday.
    for name in ["strftime", "datetime"] {
        let delegate = Mutex::new(Connection::open_in_memory()?);
        conn.create_scalar_function(
            name,
            -1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_INNOCUOUS,
            move |ctx| {
                let instant = current();
                let mut arguments = (0..ctx.len())
                    .map(|index| ctx.get::<Value>(index))
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                // strftime's first argument is its format, not a time-value.
                let time_index = usize::from(name == "strftime");
                if let Some(instant) = instant {
                    if arguments.len() == time_index {
                        arguments.push(Value::Text(crate::db::timestamps::format_utc_timestamp(
                            instant,
                        )));
                    }
                    if let Some(Value::Text(value)) = arguments.get_mut(time_index) {
                        if value.eq_ignore_ascii_case("now") {
                            *value = crate::db::timestamps::format_utc_timestamp(instant);
                        }
                    }
                }
                let placeholders = (1..=arguments.len())
                    .map(|index| format!("?{index}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!("SELECT {name}({placeholders})");
                let delegate = delegate.lock().map_err(|_| {
                    rusqlite::Error::UserFunctionError(Box::new(std::io::Error::other(
                        "test clock delegate lock poisoned",
                    )))
                })?;
                delegate.query_row(&sql, rusqlite::params_from_iter(arguments.iter()), |row| {
                    row.get::<_, Value>(0)
                })
            },
        )?;
    }
    Ok(())
}

pub(crate) fn yearly_and_dst_instants() -> Vec<DateTime<Utc>> {
    use chrono::{Duration, FixedOffset, NaiveDate, NaiveDateTime, TimeZone};
    let mut instants = Vec::with_capacity(4 * 366 * 3 + 8);
    for ordinal in 1..=366 {
        let date = NaiveDate::from_yo_opt(2024, ordinal).expect("leap-year date");
        for seconds in [0, 1, 43_199, 86_399] {
            let wall_time =
                date.and_hms_opt(0, 0, 0).expect("midnight") + Duration::seconds(seconds);
            for offset_seconds in [-43_200, 0, 50_400] {
                let offset = FixedOffset::east_opt(offset_seconds).expect("valid UTC offset");
                instants.push(
                    offset
                        .from_local_datetime(&wall_time)
                        .single()
                        .expect("fixed-offset wall time")
                        .with_timezone(&Utc),
                );
            }
        }
    }

    for (wall_time, offset_seconds) in [
        ("2024-03-10 01:59:59", -18_000),
        ("2024-03-10 03:00:00", -14_400),
        ("2024-11-03 01:59:59", -14_400),
        ("2024-11-03 01:00:00", -18_000),
        ("2024-04-07 02:59:59", 39_600),
        ("2024-04-07 02:00:00", 36_000),
        ("2024-10-06 01:59:59", 36_000),
        ("2024-10-06 03:00:00", 39_600),
    ] {
        let wall_time = NaiveDateTime::parse_from_str(wall_time, "%Y-%m-%d %H:%M:%S")
            .expect("DST edge wall time");
        let offset = FixedOffset::east_opt(offset_seconds).expect("valid DST offset");
        instants.push(
            offset
                .from_local_datetime(&wall_time)
                .single()
                .expect("fixed-offset DST edge")
                .with_timezone(&Utc),
        );
    }
    instants
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instant(raw: &str) -> DateTime<Utc> {
        crate::db::timestamps::parse_db_timestamp(raw).unwrap()
    }

    #[test]
    fn scopes_restore_nested_instants_and_do_not_cross_threads() {
        assert!(current().is_none());
        let outer = at(instant("2024-03-10T06:59:59.123Z"));
        assert_eq!(
            crate::db::timestamps::now_rfc3339(),
            "2024-03-10T06:59:59.123Z"
        );
        {
            let _inner = at(instant("2024-11-03T06:00:00.456Z"));
            assert_eq!(
                crate::db::timestamps::now_rfc3339(),
                "2024-11-03T06:00:00.456Z"
            );
            std::thread::spawn(|| assert!(current().is_none()))
                .join()
                .unwrap();
        }
        assert_eq!(current(), Some(instant("2024-03-10T06:59:59.123Z")));
        drop(outer);
        assert!(current().is_none());
    }

    #[test]
    fn sqlite_delegates_non_now_values_and_observes_scope_changes() {
        let _outer = at(instant("2024-02-29T12:34:56.789Z"));
        let conn = Connection::open_in_memory().unwrap();
        install(&conn).unwrap();
        let value: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(value, "2024-02-29T12:34:56.789Z");
        let delegate = Connection::open_in_memory().unwrap();
        for sql in [
            "SELECT datetime('2024-03-10T01:00:00-05:00','+1 hour')",
            "SELECT julianday('2024-02-29 12:00:00')",
            "SELECT strftime('now','2024-02-29')",
            "SELECT datetime(NULL)",
            "SELECT unixepoch('2024-11-03T01:30:00-04:00')",
        ] {
            let expected: Value = delegate.query_row(sql, [], |row| row.get(0)).unwrap();
            let observed: Value = conn.query_row(sql, [], |row| row.get(0)).unwrap();
            assert_eq!(observed, expected, "{sql}");
        }
        let _inner = at(instant("2024-11-03T06:00:00.456Z"));
        let value: String = conn
            .query_row("SELECT datetime('now','+1 hour')", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "2024-11-03 07:00:00");
    }
    #[test]
    fn generated_task_and_run_writers_round_trip_a_leap_year_and_dst_after_reopen() {
        let home = tempfile::tempdir().expect("isolated annual writer home");
        let _home = crate::db::test_support::with_home(home.path());
        let instants = yearly_and_dst_instants();
        assert_eq!(instants.len(), 4400);
        let _setup = at(instants[0]);
        let tasks = crate::db::tasks::TasksDb::open().expect("actual task store");
        let runs = crate::db::runs::RunsDb::open().expect("actual run store");
        let mut references = Vec::with_capacity(instants.len());
        for (index, instant) in instants.iter().copied().enumerate() {
            let _created = at(instant);
            let task = tasks
                .create(crate::db::tasks::CreateTaskInput {
                    title: format!("annual generated-now task {index}"),
                    description: None,
                    priority: "p2".into(),
                    task_type: "feature".into(),
                    acceptance_criteria: "timestamp round trip".into(),
                    created_by: Some("clock-test".into()),
                })
                .expect("actual generated-now task writer");
            let run_id = runs
                .begin(crate::db::runs::kind::AGENT_TURN, Some("clock-test"))
                .expect("actual generated-now run writer");
            let updated = instant + chrono::Duration::milliseconds(123);
            let _updated = at(updated);
            tasks
                .update_status(task.id, "assigned", Some("worker-backend"))
                .expect("actual task update timestamp writer");
            runs.finish(&run_id, crate::db::runs::outcome::OK)
                .expect("actual run end timestamp writer");
            references.push((task.id, run_id, instant, updated));
        }
        drop(tasks);
        drop(runs);
        let tasks = crate::db::tasks::TasksDb::open().expect("reopen task store");
        let runs = crate::db::runs::RunsDb::open().expect("reopen run store");
        for (task_id, run_id, created, updated) in references {
            let task = tasks.get_by_id(task_id).unwrap().expect("persisted task");
            let run = runs.get(&run_id).unwrap().expect("persisted run");
            for (raw, expected) in [
                (task.created_at.as_str(), created),
                (task.updated_at.as_str(), updated),
                (run.started_at.as_str(), created),
                (run.ended_at.as_deref().expect("finished run"), updated),
            ] {
                assert_eq!(raw, crate::db::timestamps::format_utc_timestamp(expected));
                assert_eq!(
                    crate::db::timestamps::parse_db_timestamp(raw).unwrap(),
                    expected
                );
            }
        }
    }
    #[test]
    fn generated_worker_and_plan_writers_round_trip_a_leap_year_and_dst_after_reopen() {
        use crate::db::plans::{CreatePlanInput, CreatePlanItemInput, PlanScope, PlansDb};
        use crate::db::workers::WorkersDb;
        let home = tempfile::tempdir().expect("isolated worker/plan annual home");
        let _home = crate::db::test_support::with_home(home.path());
        let instants = yearly_and_dst_instants();
        assert_eq!(instants.len(), 4400);
        let _setup = at(instants[0]);
        let workers = WorkersDb::open().expect("actual worker store and indexes");
        let plans = PlansDb::open().expect("actual plan store");
        let mut references = Vec::with_capacity(instants.len());
        for (index, instant) in instants.iter().copied().enumerate() {
            let _created = at(instant);
            let slug = format!("clock-worker-{index}");
            workers
                .create(
                    "Annual writer fixture",
                    &slug,
                    "backend",
                    "gemini-3.8-flash",
                    &[],
                    120,
                    None,
                )
                .expect("actual worker create writer");
            let plan = plans
                .create_plan(CreatePlanInput {
                    title: format!("annual plan {index}"),
                    description: None,
                    session_id: None,
                    assigned_agent: None,
                    created_by: Some("clock-test".into()),
                    agent_scope: PlanScope::Personal,
                    parent_plan_id: None,
                    task_id: None,
                })
                .expect("actual plan create writer");
            let item = plans
                .add_plan_item(CreatePlanItemInput {
                    plan_id: plan.id.clone(),
                    parent_id: None,
                    title: "annual item".into(),
                    description: None,
                    order_index: 0,
                    agent_type: None,
                    estimated_tools: None,
                    assigned_worker: None,
                    worktree_branch: None,
                })
                .expect("actual plan item create writer");
            let updated = instant + chrono::Duration::milliseconds(123);
            let _updated = at(updated);
            workers
                .heartbeat(&slug)
                .expect("actual worker heartbeat writer");
            plans
                .update_plan_status(&plan.id, "active")
                .expect("actual plan update writer");
            plans
                .update_plan_item_status(&item.id, "done", None)
                .expect("actual item completion writer");
            references.push((slug, plan.id, item.id, instant, updated));
        }
        drop(workers);
        drop(plans);
        let workers = WorkersDb::open().expect("reopen worker store");
        let plans = PlansDb::open().expect("reopen plan store");
        for (slug, plan_id, item_id, created, updated) in references {
            let worker = workers
                .get_by_slug(&slug)
                .unwrap()
                .expect("persisted worker");
            let plan = plans.get_plan(&plan_id).unwrap().expect("persisted plan");
            let item = plans
                .get_plan_item(&item_id)
                .unwrap()
                .expect("persisted item");
            for (raw, expected) in [
                (worker.created_at.as_str(), created),
                (worker.updated_at.as_str(), updated),
                (worker.last_seen.as_deref().expect("heartbeat"), updated),
                (plan.created_at.as_str(), created),
                (plan.updated_at.as_str(), updated),
                (item.created_at.as_str(), created),
                (
                    item.completed_at.as_deref().expect("completed item"),
                    updated,
                ),
            ] {
                assert_eq!(raw, crate::db::timestamps::format_utc_timestamp(expected));
                assert_eq!(
                    crate::db::timestamps::parse_db_timestamp(raw).unwrap(),
                    expected
                );
            }
        }
    }
}
