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
use crate::errors::Result;

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
        if !self.limit_usd.is_finite() || self.limit_usd <= 0.0 {
            return Err(crate::errors::ArcanaError::Agent(format!(
                "A daily cap must be a positive, finite number of dollars; got {}. \
                 A NaN or zero cap would compare false against every spend and \
                 silently enforce nothing.",
                self.limit_usd
            )));
        }
        app_settings::set(KEY_CAP_ENABLED, if self.enabled { "true" } else { "false" })?;
        app_settings::set(KEY_DAILY_CAP, &self.limit_usd.to_string())?;
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

/// Stop every worker because the ceiling was reached.
///
/// The kill switch is a real state change, not a notification. Workers move to
/// `paused`, which `recipes::scheduler` already honours by skipping them — so
/// the fleet stops picking up work rather than being asked nicely to.
///
/// Returns how many were paused, so the caller can say so.
pub fn pause_fleet(reason: &str) -> Result<usize> {
    use crate::db::workers::WorkersDb;

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
        if w.status.as_str() != "paused" {
            db.update_status(w.id, "paused")?;
            paused += 1;

            // 🔴 And it wrote the row without telling anyone. `stop_worker`
            // emits this same event so the roster dims the row immediately;
            // pausing the whole fleet for money did not, so the UI showed the
            // workers as they were until something else refreshed it.
            crate::events::emit_worker_status(crate::events::WorkerStatusPayload {
                slug: w.slug.clone(),
                status: "paused".to_string(),
                previous_status: Some(w.status.clone()),
                reason: Some(format!("Daily spend ceiling reached ({reason})")),
                cooldown_seconds: None,
                tick_duration_ms: None,
                last_outcome: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    }
    tracing::error!("Daily spend ceiling reached ({reason}) — paused {paused} worker(s)");
    Ok(paused)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_meter_fraction_is_clamped_and_absent_when_disabled() {
        assert_eq!(cap(false, 50.0).fraction_used(10.0), None);
        assert_eq!(cap(true, 50.0).fraction_used(25.0), Some(0.5));
        // Past the ceiling the meter reads full, not 140%.
        assert_eq!(cap(true, 50.0).fraction_used(70.0), Some(1.0));
        assert_eq!(cap(true, 50.0).fraction_used(-1.0), Some(0.0));
    }
}
