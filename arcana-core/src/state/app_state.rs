// Shared mutable application state held behind Arc<Mutex<>> in Tauri.
// Avoid storing large data here — this is for lightweight runtime flags.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAgent {
    pub session_id: String,
    pub is_running: bool,
    pub cancel_flag: bool,
}


// ── Persisted spend counters (S2-06) ─────────────────────────────────────────
//
// The daily ceiling reads `cost_today()`. Held in memory alone, that figure was
// zero on every launch — so a user who reached the cap could restart the app and
// keep going. These four keys are what make "daily" mean daily.
const KEY_COST_DAY: &str = "cost_day";
const KEY_COST_TODAY: &str = "cost_today_usd";
const KEY_TOKENS_IN_TODAY: &str = "tokens_in_today";
const KEY_TOKENS_OUT_TODAY: &str = "tokens_out_today";
#[derive(Debug, Default)]
pub struct AppState {
    // Active agent sessions (session_id → state)
    pub agents: HashMap<String, ActiveAgent>,

    // Currently active provider name (e.g. "anthropic")
    pub active_provider: Option<String>,

    // Active model identifier (e.g. "claude-sonnet-4-5")
    pub active_model: Option<String>,

    // Today's running cost in USD. Reset at the local-date rollover — see
    // `add_cost`.
    pub cost_today_usd: f64,

    // Today's cumulative token counts
    pub tokens_in_today: u64,
    pub tokens_out_today: u64,

    /// The local date these "today" figures belong to.
    ///
    /// ⚠ **Without this, "today" meant "since process start" (`IN-50`).** The
    /// counters were only ever incremented — `grep` found the declaration, one
    /// `+=`, and three reads, and no reset anywhere. A desktop app left running
    /// for a week reported a week's spend as today's, and the dashboard's
    /// headline "today" figure was whatever had accumulated since launch.
    ///
    /// It also made a **daily** cap impossible to build honestly: a ceiling over
    /// a counter that resets on restart is a per-process ceiling, and a user who
    /// restarts twice spends three times the cap.
    pub cost_day: Option<chrono::NaiveDate>,

    // Custom system prompt override (if set, replaces BASE_SYSTEM_PROMPT)
    pub system_prompt_override: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            system_prompt_override: None,
            ..Self::default()
        }
    }

    pub fn cancel_agent(&mut self, session_id: &str) {
        if let Some(agent) = self.agents.get_mut(session_id) {
            agent.cancel_flag = true;
        }
    }

    pub fn is_cancelled(&self, session_id: &str) -> bool {
        self.agents
            .get(session_id)
            .map(|a| a.cancel_flag)
            .unwrap_or(false)
    }

    /// Add one turn's spend, rolling the counters over at local midnight.
    ///
    /// **Local** date, not UTC: a user in UTC+7 whose "day" rolled at 07:00
    /// local would see the figure they are watching reset mid-morning, and a
    /// daily cap would refill at a time that means nothing to them.
    pub fn add_cost(&mut self, cost_usd: f64, tokens_in: u32, tokens_out: u32) {
        self.roll_day_if_needed(chrono::Local::now().date_naive());
        self.cost_today_usd += cost_usd;
        self.tokens_in_today += tokens_in as u64;
        self.tokens_out_today += tokens_out as u64;
        self.persist_today();
    }

    /// Write today's counters to app settings (`S2-06`).
    ///
    /// ── Why the ceiling needs this to mean anything ──────────────────────────
    ///
    /// These counters lived **only in memory**. The daily cap reads
    /// `cost_today()`, so on every launch it saw zero — a user who reached the
    /// ceiling could close the app, reopen it, and carry on spending. A cap
    /// that resets on restart is not a cap; it is a per-session cap that
    /// describes itself as daily.
    ///
    /// A failed write is logged and not propagated: losing the persistence of
    /// one turn is bad, and failing the turn over it is worse. The next turn
    /// rewrites the running total, so a transient failure self-heals.
    fn persist_today(&self) {
        use crate::db::app_settings;

        let write = |key: &str, value: String| {
            if let Err(e) = app_settings::set(key, &value) {
                tracing::warn!("could not persist {key}: {e}");
            }
        };

        if let Some(day) = self.cost_day {
            write(KEY_COST_DAY, day.to_string());
        }
        write(KEY_COST_TODAY, self.cost_today_usd.to_string());
        write(KEY_TOKENS_IN_TODAY, self.tokens_in_today.to_string());
        write(KEY_TOKENS_OUT_TODAY, self.tokens_out_today.to_string());
    }

    /// Restore today's counters at startup (`S2-06`).
    ///
    /// ⚠ **The stored DATE is what makes this safe.** Restoring the figures
    /// without checking which day they belong to would carry yesterday's spend
    /// into today and cap a user who has spent nothing. `roll_day_if_needed`
    /// zeroes them when the date has moved, so a stale row is discarded rather
    /// than trusted.
    pub fn hydrate_today(&mut self) {
        use crate::db::app_settings;

        let get = |key: &str| app_settings::get(key).ok().flatten();

        self.cost_day = get(KEY_COST_DAY).and_then(|v| v.parse().ok());
        self.cost_today_usd = get(KEY_COST_TODAY)
            .and_then(|v| v.parse().ok())
            // A negative or non-finite stored figure would make the cap
            // unreachable. Discarded rather than trusted.
            .filter(|v: &f64| v.is_finite() && *v >= 0.0)
            .unwrap_or(0.0);
        self.tokens_in_today = get(KEY_TOKENS_IN_TODAY).and_then(|v| v.parse().ok()).unwrap_or(0);
        self.tokens_out_today = get(KEY_TOKENS_OUT_TODAY).and_then(|v| v.parse().ok()).unwrap_or(0);

        // Discards the restored figures if they belong to a previous day.
        self.roll_day_if_needed(chrono::Local::now().date_naive());

        if self.cost_today_usd > 0.0 {
            tracing::info!(
                "Restored today's spend: ${:.4} ({} in / {} out tokens)",
                self.cost_today_usd,
                self.tokens_in_today,
                self.tokens_out_today
            );
        }
    }

    /// Zero the "today" counters when the local date has changed.
    ///
    /// Takes the date rather than reading the clock so it can be tested without
    /// waiting for midnight — a reset that can only be observed once a day is a
    /// reset nobody verifies.
    pub fn roll_day_if_needed(&mut self, today: chrono::NaiveDate) {
        match self.cost_day {
            Some(d) if d == today => {}
            _ => {
                self.cost_day = Some(today);
                self.cost_today_usd = 0.0;
                self.tokens_in_today = 0;
                self.tokens_out_today = 0;
            }
        }
    }

    /// Today's spend, rolling over first so a read never returns yesterday's.
    ///
    /// Every read goes through this. Reading the field directly is how a stale
    /// figure reaches the dashboard on the first look after midnight, before any
    /// new spend has triggered a rollover.
    pub fn cost_today(&mut self) -> f64 {
        self.roll_day_if_needed(chrono::Local::now().date_naive());
        self.cost_today_usd
    }
}

#[cfg(test)]
mod daily_rollover_tests {
    use super::*;
    use chrono::NaiveDate;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn spend_accumulates_within_one_day() {
        let mut s = AppState::new();
        s.roll_day_if_needed(day(2026, 9, 2));
        s.cost_today_usd += 0.10;
        s.roll_day_if_needed(day(2026, 9, 2));
        s.cost_today_usd += 0.15;
        assert!((s.cost_today_usd - 0.25).abs() < 1e-9);
    }

    #[test]
    fn crossing_midnight_resets_every_counter() {
        // The defect: without this, "today" meant "since process start", so a
        // long-running desktop session reported a week as one day.
        let mut s = AppState::new();
        s.roll_day_if_needed(day(2026, 9, 2));
        s.cost_today_usd = 4.20;
        s.tokens_in_today = 900_000;
        s.tokens_out_today = 80_000;

        s.roll_day_if_needed(day(2026, 9, 3));
        assert_eq!(s.cost_today_usd, 0.0);
        assert_eq!(s.tokens_in_today, 0, "tokens must roll over with cost");
        assert_eq!(s.tokens_out_today, 0);
    }

    #[test]
    fn the_first_spend_of_a_process_establishes_the_day() {
        // A fresh AppState has no day. If that were treated as "same day", the
        // first turn after launch would inherit whatever the field defaulted to.
        let mut s = AppState::new();
        assert_eq!(s.cost_day, None);
        s.roll_day_if_needed(day(2026, 9, 2));
        assert_eq!(s.cost_day, Some(day(2026, 9, 2)));
    }

    #[test]
    fn a_backwards_clock_still_rolls_rather_than_accumulating() {
        // Clocks move backwards — NTP correction, a timezone change, a user
        // setting the date. Treating "not equal" as a rollover keeps the figure
        // honest for whatever day it now is, instead of merging two days.
        let mut s = AppState::new();
        s.roll_day_if_needed(day(2026, 9, 3));
        s.cost_today_usd = 1.00;
        s.roll_day_if_needed(day(2026, 9, 2));
        assert_eq!(s.cost_today_usd, 0.0);
        assert_eq!(s.cost_day, Some(day(2026, 9, 2)));
    }
}

// ── The process-wide handle ──────────────────────────────────────────────────
//
// The daily ceiling reads today's spend from `AppState`, and most provider call
// sites already hold one. Two do not: `analyze_diff_tool` builds an analyzer
// straight from the vault, and any future core-side tool would be in the same
// position — tools receive a `ToolContext`, not application state.
//
// Without this, those paths would be the one place a call could pass **without**
// meeting the ceiling, which is exactly the hole `P2-21` exists to close. A hole
// in a spend cap is not a partial cap; it is the path spend will take.
//
// Set once at startup by whichever binary owns the state. Absent in a test
// process, where `global()` returns `None` and the caller falls back to running
// uncapped — a unit test must not depend on a ceiling it never configured.

use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

static GLOBAL_STATE: OnceLock<Arc<Mutex<AppState>>> = OnceLock::new();

/// Publish the application state for core-side callers.
///
/// Idempotent: a second call is ignored rather than panicking, because both the
/// desktop host and the CLI may reasonably try, and losing a race here should
/// not take down the process.
pub fn set_global(state: Arc<Mutex<AppState>>) {
    let _ = GLOBAL_STATE.set(state);
}

/// The published state, if a binary has set one.
pub fn global() -> Option<Arc<Mutex<AppState>>> {
    GLOBAL_STATE.get().cloned()
}
