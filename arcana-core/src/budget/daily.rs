// The daily spend ceiling and the kill switch (P2-21, P2-22 · M-42 · G-04).
//
// ── Why this is not a per-task budget ────────────────────────────────────────
//
// `TaskBudget` caps one run. This caps the day, across every run, every worker
// and every subagent. They are different failures: a per-task budget stops one
// agent from looping; a daily ceiling stops fifty well-behaved agents from
// costing more than the user meant to spend in total.
//
// ── Why it lives at the provider boundary ────────────────────────────────────
//
// Spec `02 §5` / `G-04`: *"the one place every call passes."* A check in the
// agent loop would miss the recipe runner, subagents, compaction and the diff
// classifier — each of which builds its own provider and calls it directly. The
// enforcement is therefore a decorator over `Provider` (`provider::capped`), and
// this module holds the policy it reads.
//
// ── What the previous "cap" actually was ─────────────────────────────────────
//
// A toast. `dashboardStore.ts:366` pushed a `BudgetAlert` and called nothing
// else — no cancel, no pause, no path back into Rust. Its threshold was a
// hardcoded `50` in two separate files with no setter, it listened on an event
// the Mode 1 agent path never emits, and closing the window removed even that.

use crate::db::app_settings;
use crate::errors::{ArcanaError, Result};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::path::Path;

/// The settings key holding the ceiling, in USD.
const KEY_DAILY_CAP: &str = "budget.daily_cap_usd";
/// The settings key holding whether the ceiling is enforced at all.
const KEY_CAP_ENABLED: &str = "budget.daily_cap_enabled";

/// The default ceiling, used when the user has not set one.
///
/// Matches the `50` the UI already displayed, so enabling enforcement does not
/// silently change the number a user was shown. It was never enforceable
/// before — the value existed, the mechanism did not.
pub const DEFAULT_DAILY_CAP_USD: f64 = 50.0;

/// What the ceiling says about a moment's spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapState {
    /// No ceiling in force.
    Disabled,
    /// Under the ceiling.
    Ok,
    /// At or over it. Work must stop.
    Exceeded,
}

/// The daily ceiling as configured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DailyCap {
    pub enabled: bool,
    pub limit_usd: f64,
}

impl Default for DailyCap {
    fn default() -> Self {
        Self {
            // Off by default. A ceiling the user never chose, silently stopping
            // their work partway through a task, is worse than no ceiling —
            // `§0.15`: a capability existing is not consent to use it.
            enabled: false,
            limit_usd: DEFAULT_DAILY_CAP_USD,
        }
    }
}

impl DailyCap {
    /// Read the configured ceiling.
    ///
    /// A malformed stored value falls back to the default rather than failing:
    /// a corrupt settings row must not make the app unusable, and the default
    /// is the safe direction.
    pub fn load() -> Self {
        let enabled = app_settings::get(KEY_CAP_ENABLED)
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        let limit_usd = app_settings::get(KEY_DAILY_CAP)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(DEFAULT_DAILY_CAP_USD);
        Self { enabled, limit_usd }
    }

    /// Persist the ceiling.
    ///
    /// Rejects a non-finite or non-positive limit rather than storing it: a cap
    /// of `NaN` compares false against everything, so it would read as "never
    /// exceeded" and silently disable the ceiling it was meant to set.
    pub fn save(&self) -> Result<()> {
        let mut operation = crate::observability::timing::OperationTimer::start("daily_cap_save")
            .with_database("config");
        if !self.limit_usd.is_finite() || self.limit_usd <= 0.0 {
            return Err(crate::errors::ArcanaError::Agent(format!(
                "A daily cap must be a positive, finite number of dollars; got {}. \
                 A NaN or zero cap would compare false against every spend and \
                 silently enforce nothing.",
                self.limit_usd
            )));
        }
        let mut conn = app_settings::open()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        app_settings::set_in(
            &tx,
            KEY_CAP_ENABLED,
            if self.enabled { "true" } else { "false" },
        )?;
        app_settings::set_in(&tx, KEY_DAILY_CAP, &self.limit_usd.to_string())?;
        tx.commit()?;
        operation.succeed();
        Ok(())
    }

    /// Where `spent_today` stands against this ceiling.
    ///
    /// `>=`, not `>`: a cap of $50 means work stops **at** $50. Treating it as a
    /// strict inequality lets the run that reaches exactly the ceiling proceed,
    /// which is the one case a user setting a round number is thinking of.
    pub fn evaluate(&self, spent_today: f64) -> CapState {
        if !self.enabled {
            return CapState::Disabled;
        }
        if spent_today >= self.limit_usd {
            CapState::Exceeded
        } else {
            CapState::Ok
        }
    }

    /// Fraction of the ceiling consumed, for a meter. `None` when disabled.
    pub fn fraction_used(&self, spent_today: f64) -> Option<f64> {
        if !self.enabled || self.limit_usd <= 0.0 {
            return None;
        }
        Some((spent_today / self.limit_usd).clamp(0.0, 1.0))
    }
}

/// One durable hold against the daily ceiling. An unfinished provider call
/// remains charged at its reserved maximum, including across process restart.
#[derive(Debug, Clone)]
pub struct DailySpendReservation {
    pub id: String,
    local_date: String,
}

#[derive(Debug)]
pub enum ReservationDecision {
    Disabled,
    Granted(DailySpendReservation),
    Denied {
        spent_usd: f64,
        pending_usd: f64,
        requested_usd: f64,
        limit_usd: f64,
    },
}

const MICRODOLLARS_PER_USD: f64 = 1_000_000.0;

fn usd_to_micros_ceil(usd: f64) -> Result<i64> {
    if !usd.is_finite() || usd < 0.0 {
        return Err(ArcanaError::Agent(
            "daily budget reservation amount is invalid".to_string(),
        ));
    }
    let micros = (usd * MICRODOLLARS_PER_USD).ceil();
    if micros > i64::MAX as f64 {
        return Err(ArcanaError::Agent(
            "daily budget reservation amount exceeds storage range".to_string(),
        ));
    }
    Ok(micros as i64)
}

fn cap_from_settings(conn: &rusqlite::Connection) -> Result<DailyCap> {
    let enabled = match app_settings::get_in(conn, KEY_CAP_ENABLED)?.as_deref() {
        None | Some("false") => false,
        Some("true") => true,
        Some(_) => {
            return Err(ArcanaError::Agent(
                "daily spend cap setting is invalid; provider call refused".to_string(),
            ));
        }
    };
    let limit_usd = match app_settings::get_in(conn, KEY_DAILY_CAP)? {
        None => DEFAULT_DAILY_CAP_USD,
        Some(value) => value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| {
                ArcanaError::Agent(
                    "daily spend cap limit is invalid; provider call refused".to_string(),
                )
            })?,
    };
    Ok(DailyCap { enabled, limit_usd })
}

fn legacy_spend_micros(conn: &rusqlite::Connection, today: &str) -> Result<i64> {
    let stored_day = app_settings::get_in(conn, "cost_day")?;
    let stored_cost = app_settings::get_in(conn, "cost_today_usd")?;
    let Some(stored_cost) = stored_cost else {
        if stored_day.is_some() {
            return Ok(0);
        }
        return Ok(0);
    };
    let cost = stored_cost
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(|| {
            ArcanaError::Agent(
                "persisted daily spend is invalid; provider call refused".to_string(),
            )
        })?;
    let Some(stored_day) = stored_day else {
        if cost > 0.0 {
            return Err(ArcanaError::Agent(
                "persisted daily spend has no date; provider call refused".to_string(),
            ));
        }
        return Ok(0);
    };
    let parsed_day = chrono::NaiveDate::parse_from_str(&stored_day, "%Y-%m-%d").map_err(|_| {
        ArcanaError::Agent(
            "persisted daily spend date is invalid; provider call refused".to_string(),
        )
    })?;
    if parsed_day.to_string() != today {
        return Ok(0);
    }
    usd_to_micros_ceil(cost)
}

/// Reserve the maximum possible cost for a provider request in an immediate
/// SQLite transaction. Concurrent callers therefore see each other's holds.
pub fn reserve_provider_call(id: String, max_cost_usd: f64) -> Result<ReservationDecision> {
    let path = crate::db::arcana_dir()?.join("config.db");
    reserve_provider_call_at_estimated(
        &path,
        id,
        || Ok(max_cost_usd),
        chrono::Local::now().date_naive(),
    )
}

fn reserve_provider_call_at(
    path: &Path,
    id: String,
    max_cost_usd: f64,
    today: chrono::NaiveDate,
) -> Result<ReservationDecision> {
    reserve_provider_call_at_estimated(path, id, || Ok(max_cost_usd), today)
}

/// Variant used by the provider wrapper so disabled caps do not add model
/// lookup or cost estimation to calls that the user did not cap.
pub fn reserve_provider_call_estimated(
    id: String,
    estimate: impl FnOnce() -> Result<f64>,
) -> Result<ReservationDecision> {
    let path = crate::db::arcana_dir()?.join("config.db");
    reserve_provider_call_at_estimated(&path, id, estimate, chrono::Local::now().date_naive())
}

fn reserve_provider_call_at_estimated(
    path: &Path,
    id: String,
    estimate: impl FnOnce() -> Result<f64>,
    today: chrono::NaiveDate,
) -> Result<ReservationDecision> {
    let operation =
        crate::observability::timing::OperationTimer::start("daily_budget_provider_call_reserve")
            .with_database("config");
    let mut conn = app_settings::open_at(path)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let cap = cap_from_settings(&tx)?;
    if !cap.enabled {
        tx.commit()?;
        operation.with_result_count(0).set_outcome("disabled");
        return Ok(ReservationDecision::Disabled);
    }
    let max_micros = usd_to_micros_ceil(estimate()?)?;

    let day = today.to_string();
    // The legacy app-state counter remains the source for calls made while the
    // cap is disabled. Reconcile it on every reservation, not only when the
    // ledger row is first created: a user can disable the cap, spend, and
    // re-enable it later the same day. `MAX` avoids double-counting calls that
    // both the capped ledger and the legacy UI counter observed.
    let legacy_spend = legacy_spend_micros(&tx, &day)?;
    tx.execute(
        "INSERT INTO daily_budget_totals(local_date, spent_micros) VALUES (?1, ?2)
         ON CONFLICT(local_date) DO UPDATE SET spent_micros = MAX(spent_micros, excluded.spent_micros)",
        params![day, legacy_spend],
    )?;

    let spent: i64 = tx.query_row(
        "SELECT spent_micros FROM daily_budget_totals WHERE local_date = ?1",
        [&day],
        |row| row.get(0),
    )?;
    let pending: i64 = tx.query_row(
        "SELECT COALESCE(SUM(amount_micros), 0) FROM daily_budget_reservations WHERE local_date = ?1",
        [&day],
        |row| row.get(0),
    )?;
    let limit_micros = (cap.limit_usd * MICRODOLLARS_PER_USD).floor() as i64;
    let projected = spent
        .checked_add(pending)
        .and_then(|value| value.checked_add(max_micros))
        .ok_or_else(|| {
            ArcanaError::Agent("daily spend accounting overflow; provider call refused".to_string())
        })?;
    if projected >= limit_micros {
        tx.commit()?;
        operation.with_result_count(0).set_outcome("denied");
        return Ok(ReservationDecision::Denied {
            spent_usd: spent as f64 / MICRODOLLARS_PER_USD,
            pending_usd: pending as f64 / MICRODOLLARS_PER_USD,
            requested_usd: max_micros as f64 / MICRODOLLARS_PER_USD,
            limit_usd: cap.limit_usd,
        });
    }

    tx.execute(
        "INSERT INTO daily_budget_reservations(id, local_date, amount_micros) VALUES (?1, ?2, ?3)",
        params![id, day, max_micros],
    )?;
    tx.commit()?;
    operation.with_result_count(1).set_outcome("granted");
    Ok(ReservationDecision::Granted(DailySpendReservation {
        id,
        local_date: day,
    }))
}

/// Reconcile a known usage cost. `None` means the request may have reached the
/// provider but no reliable usage arrived, so the maximum reservation becomes
/// settled spend instead of being released.
pub fn settle_provider_call(
    reservation: &DailySpendReservation,
    actual_cost_usd: Option<f64>,
) -> Result<()> {
    settle_provider_call_at(
        &crate::db::arcana_dir()?.join("config.db"),
        reservation,
        actual_cost_usd,
    )
}

fn settle_provider_call_at(
    path: &Path,
    reservation: &DailySpendReservation,
    actual_cost_usd: Option<f64>,
) -> Result<()> {
    let operation =
        crate::observability::timing::OperationTimer::start("daily_budget_provider_call_settle")
            .with_database("config");
    let mut conn = app_settings::open_at(path)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let reserved: Option<(String, i64)> = tx
        .query_row(
            "SELECT local_date, amount_micros FROM daily_budget_reservations WHERE id = ?1",
            [&reservation.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((stored_day, max_micros)) = reserved else {
        tx.commit()?;
        operation.with_result_count(0).set_outcome("not_found");
        return Ok(());
    };
    if stored_day != reservation.local_date {
        return Err(ArcanaError::Agent(
            "daily spend reservation identity mismatch".to_string(),
        ));
    }
    let settled_micros = match actual_cost_usd {
        Some(actual) => usd_to_micros_ceil(actual)?,
        None => max_micros,
    };
    tx.execute(
        "INSERT INTO daily_budget_totals(local_date, spent_micros) VALUES (?1, ?2)
         ON CONFLICT(local_date) DO UPDATE SET spent_micros = spent_micros + excluded.spent_micros",
        params![stored_day, settled_micros],
    )?;
    tx.execute(
        "DELETE FROM daily_budget_reservations WHERE id = ?1",
        [&reservation.id],
    )?;
    tx.commit()?;
    operation
        .with_result_count(1)
        .set_outcome(match actual_cost_usd {
            Some(_) => "settled",
            None => "settled_unknown_usage",
        });
    Ok(())
}

/// Stop every worker because the ceiling was reached.
///
/// The kill switch is a real state change, not a notification. Workers move to
/// `paused`, which `recipes::scheduler` already honours by skipping them — so
/// the fleet stops picking up work rather than being asked nicely to.
///
/// Returns how many were paused, so the caller can say so.
pub fn pause_fleet(reason: &str) -> Result<usize> {
    use crate::db::workers::{WorkerStatus, WorkersDb};

    let db = WorkersDb::open()?;
    let workers = db.list()?;
    let mut paused = 0;
    for w in workers {
        // 🔴 **The guard used to read `== "running" || == "idle"`, and nothing
        // in this application writes `"running"` to a worker row.** The runner
        // writes `"working"` (`recipes/runner.rs`); `"running"` belongs to
        // `SubagentStatus`, a different enum entirely. So the first arm was
        // dead and only *idle* workers were paused — a worker mid-tick when the
        // ceiling tripped kept its tick, finished, was set back to `"idle"` by
        // the runner rather than to `"paused"`, and was then fired again by the
        // scheduler on its next cron beat. **The spend ceiling did not hold for
        // any worker that was actually spending when it tripped**, which is the
        // only case it exists for.
        //
        // Inverted deliberately: pause everything not already paused. `"stopped"`
        // is not checked because nothing in this application writes it — adding
        // it back would be another guard on a status that cannot occur, which is
        // the defect being fixed here.
        // An allowlist of "statuses that mean busy" is a list that must agree
        // with the runner's, and this is the second time that disagreement has
        // cost something here (`L-19`).
        if w.status != WorkerStatus::Paused {
            db.update_status(w.id, WorkerStatus::Paused)?;
            paused += 1;

            // 🔴 And it wrote the row without telling anyone. `stop_worker`
            // emits this same event so the roster dims the row immediately;
            // pausing the whole fleet for money did not, so the UI showed the
            // workers as they were until something else refreshed it.
            crate::events::emit_worker_status(crate::events::WorkerStatusPayload {
                slug: w.slug.clone(),
                status: "paused".to_string(),
                previous_status: Some(w.status.as_str().to_string()),
                reason: Some(format!("Daily spend ceiling reached ({reason})")),
                cooldown_seconds: None,
                tick_duration_ms: None,
                last_outcome: None,
                timestamp: crate::db::timestamps::now_rfc3339(),
            });
        }
    }
    tracing::error!("Daily spend ceiling reached ({reason}) — paused {paused} worker(s)");
    Ok(paused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn cap(enabled: bool, limit: f64) -> DailyCap {
        DailyCap {
            enabled,
            limit_usd: limit,
        }
    }

    #[test]
    fn a_disabled_cap_never_stops_anything() {
        assert_eq!(cap(false, 1.0).evaluate(1_000.0), CapState::Disabled);
    }

    #[test]
    fn the_cap_is_a_ceiling_not_a_suggestion() {
        // `>=`. A $50 cap means work stops AT $50 — the exact case a user
        // setting a round number has in mind.
        let c = cap(true, 50.0);
        assert_eq!(c.evaluate(49.99), CapState::Ok);
        assert_eq!(c.evaluate(50.0), CapState::Exceeded);
        assert_eq!(c.evaluate(50.01), CapState::Exceeded);
    }

    #[test]
    fn a_cap_is_off_by_default() {
        // A ceiling the user never chose, stopping their work partway through a
        // task, is worse than no ceiling.
        assert!(!DailyCap::default().enabled);
        assert_eq!(DailyCap::default().limit_usd, DEFAULT_DAILY_CAP_USD);
    }

    #[test]
    fn a_nonsense_cap_is_refused_rather_than_stored() {
        // NaN compares false against everything, so a stored NaN would read as
        // "never exceeded" and silently enforce nothing — a cap that looks set
        // and does nothing is worse than an absent one.
        for bad in [f64::NAN, f64::INFINITY, 0.0, -5.0] {
            assert!(
                cap(true, bad).save().is_err(),
                "a cap of {bad} must be refused"
            );
        }
    }

    #[test]
    fn daily_cap_save_span_reports_committed_value_and_rejects_invalid_input() {
        let home = tempfile::tempdir().expect("isolated Arcana profile");
        let _home = crate::db::test_support::with_home(home.path());
        let valid = cap(true, 123.45);
        let invalid = cap(true, f64::NAN);
        let ((saved, rejected), events) = crate::observability::timing::capture_json_events(|| {
            let saved = valid.save().is_ok();
            let rejected = invalid.save().is_err();
            (saved, rejected)
        });
        assert!(saved && rejected);
        assert_eq!(
            DailyCap::load(),
            valid,
            "the production reader sees the committed cap"
        );
        let spans = events
            .iter()
            .filter(|event| event["fields"]["operation"] == "daily_cap_save")
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 2);
        assert!(spans
            .iter()
            .any(|event| event["fields"]["outcome"] == "success"));
        assert!(spans
            .iter()
            .any(|event| event["fields"]["outcome"] == "error"));
        let serialized = events.iter().map(ToString::to_string).collect::<String>();
        assert!(!serialized.contains("123.45"));
        assert!(!serialized.contains("NaN"));
    }

    #[test]
    fn the_meter_fraction_is_clamped_and_absent_when_disabled() {
        assert_eq!(cap(false, 50.0).fraction_used(10.0), None);
        assert_eq!(cap(true, 50.0).fraction_used(25.0), Some(0.5));
        // Past the ceiling the meter reads full, not 140%.
        assert_eq!(cap(true, 50.0).fraction_used(70.0), Some(1.0));
        assert_eq!(cap(true, 50.0).fraction_used(-1.0), Some(0.0));
    }

    fn budget_db(tag: &str, limit: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "arcana-daily-budget-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&dir).expect("create unique isolated budget fixture");
        let path = dir.join("config.db");
        let conn = app_settings::open_at(&path).expect("open isolated config db");
        app_settings::set_in(&conn, KEY_CAP_ENABLED, "true").expect("enable isolated cap");
        app_settings::set_in(&conn, KEY_DAILY_CAP, limit).expect("set isolated cap");
        drop(conn);
        path
    }

    #[test]
    fn reservations_are_reconciled_to_reported_usage_and_unknown_usage_stays_charged() {
        let path = budget_db("settlement", "1.00");
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let ReservationDecision::Granted(first) =
            reserve_provider_call_at(&path, "first".into(), 0.70, today).unwrap()
        else {
            panic!("first reservation should be granted");
        };
        assert!(matches!(
            reserve_provider_call_at(&path, "blocked".into(), 0.31, today).unwrap(),
            ReservationDecision::Denied { .. }
        ));

        settle_provider_call_at(&path, &first, Some(0.10)).expect("settle known usage");
        assert!(matches!(
            reserve_provider_call_at(&path, "after-reconcile".into(), 0.80, today).unwrap(),
            ReservationDecision::Granted(_)
        ));

        let ReservationDecision::Granted(unknown) =
            reserve_provider_call_at(&path, "unknown-usage".into(), 0.09, today).unwrap()
        else {
            panic!("small reservation should fit");
        };
        settle_provider_call_at(&path, &unknown, None)
            .expect("unknown usage settles at conservative maximum");
        assert!(matches!(
            reserve_provider_call_at(&path, "after-unknown".into(), 0.02, today).unwrap(),
            ReservationDecision::Denied { .. }
        ));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn provider_budget_reservation_emits_decisions_without_private_values() {
        let path = budget_db("telemetry", "1.00");
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let granted = reserve_provider_call_at(
                &path,
                "private-grant-reservation-marker".into(),
                0.40,
                today,
            )
            .expect("reserve below cap");
            let ReservationDecision::Granted(reservation) = granted else {
                panic!("reservation should be granted");
            };
            let denied = reserve_provider_call_at(
                &path,
                "private-denied-reservation-marker".into(),
                0.70,
                today,
            )
            .expect("deny reservation at cap");
            settle_provider_call_at(&path, &reservation, Some(0.20))
                .expect("settle known provider usage");
            settle_provider_call_at(&path, &reservation, Some(0.20))
                .expect("duplicate settlement is harmless");
            let unknown_usage = reserve_provider_call_at(
                &path,
                "private-unknown-usage-reservation-marker".into(),
                0.10,
                today,
            )
            .expect("reserve for a provider response without reliable usage");
            let ReservationDecision::Granted(unknown_usage) = unknown_usage else {
                panic!("unknown-usage request should reserve under the cap");
            };
            settle_provider_call_at(&path, &unknown_usage, None)
                .expect("retain the conservative hold when usage is unknown");

            let conn = app_settings::open_at(&path).expect("open isolated config db");
            app_settings::set_in(&conn, KEY_CAP_ENABLED, "false").expect("disable cap");
            conn.execute_batch(
                "CREATE TRIGGER reject_reservation BEFORE INSERT ON daily_budget_reservations
                 BEGIN SELECT RAISE(ABORT, 'private-reservation-failure-marker'); END;",
            )
            .expect("install isolated reservation failure trigger");
            drop(conn);
            let disabled = reserve_provider_call_at(
                &path,
                "private-disabled-reservation-marker".into(),
                0.10,
                today,
            )
            .expect("disabled cap bypasses reservation");

            let conn = app_settings::open_at(&path).expect("reopen isolated config db");
            app_settings::set_in(&conn, KEY_CAP_ENABLED, "true").expect("re-enable cap");
            drop(conn);
            let failed = reserve_provider_call_at(
                &path,
                "private-failed-reservation-marker".into(),
                0.10,
                today,
            )
            .is_err();
            (denied, disabled, failed)
        });

        assert!(matches!(observed.0, ReservationDecision::Denied { .. }));
        assert!(matches!(observed.1, ReservationDecision::Disabled));
        assert!(observed.2);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for (outcome, operation) in [
            ("granted", "daily_budget_provider_call_reserve"),
            ("denied", "daily_budget_provider_call_reserve"),
            ("disabled", "daily_budget_provider_call_reserve"),
            ("error", "daily_budget_provider_call_reserve"),
            ("settled", "daily_budget_provider_call_settle"),
            ("settled_unknown_usage", "daily_budget_provider_call_settle"),
            ("not_found", "daily_budget_provider_call_settle"),
        ] {
            assert!(
                operations.iter().any(|event| {
                    event["fields"]["operation"] == operation
                        && event["fields"]["outcome"] == outcome
                        && event["fields"]["database"] == "config"
                        && event["fields"]["duration_ns"].as_u64().is_some()
                }),
                "missing {operation} outcome {outcome}"
            );
        }
        let serialized = serde_json::to_string(&operations).expect("serialize captured events");
        for private_value in [
            "private-grant-reservation-marker",
            "private-denied-reservation-marker",
            "private-disabled-reservation-marker",
            "private-failed-reservation-marker",
            "private-unknown-usage-reservation-marker",
            "private-reservation-failure-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "budget telemetry leaked {private_value}"
            );
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn re_enabling_cap_reconciles_spend_accumulated_while_disabled() {
        let path = budget_db("reenable", "1.00");
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let ReservationDecision::Granted(under_cap) =
            reserve_provider_call_at(&path, "under-cap".into(), 0.20, today).unwrap()
        else {
            panic!("initial reservation should be granted");
        };
        settle_provider_call_at(&path, &under_cap, Some(0.20)).expect("settle capped spend");

        let conn = app_settings::open_at(&path).expect("open isolated config db");
        app_settings::set_in(&conn, KEY_CAP_ENABLED, "false").expect("disable cap");
        // AppState keeps tracking the whole local day's spend while the cap is
        // disabled. This represents additional uncapped requests.
        app_settings::set_in(&conn, "cost_day", &today.to_string()).expect("set legacy date");
        app_settings::set_in(&conn, "cost_today_usd", "0.95").expect("set legacy spend");
        app_settings::set_in(&conn, KEY_CAP_ENABLED, "true").expect("re-enable cap");
        drop(conn);

        assert!(
            matches!(
                reserve_provider_call_at(&path, "must-fit-only-if-under-counted".into(), 0.06, today)
                    .unwrap(),
                ReservationDecision::Denied { spent_usd, .. } if spent_usd == 0.95
            ),
            "the ledger must reconcile the higher legacy total before granting a new call"
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn concurrent_requests_cannot_reserve_past_the_daily_ceiling() {
        let path = budget_db("concurrent", "1.00");
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let workers = 8;
        let barrier = Arc::new(Barrier::new(workers));
        let mut joins = Vec::new();
        for index in 0..workers {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            joins.push(thread::spawn(move || {
                barrier.wait();
                reserve_provider_call_at(&path, format!("parallel-{index}"), 0.25, today)
                    .expect("atomic reservation")
            }));
        }
        let granted = joins
            .into_iter()
            .map(|join| join.join().expect("reservation thread"))
            .filter(|decision| matches!(decision, ReservationDecision::Granted(_)))
            .count();
        assert_eq!(
            granted, 3,
            "fourth reservation reaches the >= cap and is denied"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn malformed_enabled_cap_fails_closed_before_creating_a_reservation() {
        let path = budget_db("malformed", "not-a-number");
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        assert!(reserve_provider_call_at(&path, "refused".into(), 0.01, today).is_err());
        let conn = app_settings::open_at(&path).expect("reopen isolated config db");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM daily_budget_reservations",
                [],
                |row| row.get(0),
            )
            .expect("reservation count");
        assert_eq!(count, 0);
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
