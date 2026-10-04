// S2 · the spend ceilings actually stop work (S2-04 … S2-12).
//
// These run in CI with no credentials: every property here is about our own
// arithmetic and state, not about a provider's behaviour. The provider-facing
// half of S2 is `live_provider_smoke.rs`, behind the `live-smoke` feature.
//
// A ceiling that is configured but does not stop anything is worse than no
// ceiling, because the user believes they are protected. Each test below is
// written to fail in that direction.

use arcana_core::budget::daily::{CapState, DailyCap};
use arcana_core::cost::pricing;
use arcana_core::provider::usage::Usage;

/// Read a source file with line endings normalised.
///
/// ⚠ Found 2026-09-02: git's CRLF conversion broke `S2-11` below. It splits on
/// `"\n}\n"` to isolate a function body, that pattern does not occur in a CRLF
/// file, and the `unwrap_or` fallback silently widened the check to the whole
/// file — which then failed on a symbol 40 KB away. The ordering path was, and
/// is, correct.
fn read_source(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .replace("\r\n", "\n")
}

// ── S2-05 · exceeding the cap stops work ─────────────────────────────────────

#[test]
fn s2_05_the_ceiling_fires_at_the_limit_not_past_it() {
    let cap = DailyCap {
        enabled: true,
        limit_usd: 10.0,
    };

    assert_eq!(cap.evaluate(9.99), CapState::Ok);
    // ⚠ `>=`, not `>`. Firing only when spend EXCEEDS the ceiling lets a call
    // land exactly on it and pass, which means the cap is off by one call —
    // and the call that lands on it is unbounded in size.
    assert_eq!(
        cap.evaluate(10.0),
        CapState::Exceeded,
        "the cap must fire AT the limit"
    );
    assert_eq!(cap.evaluate(10.01), CapState::Exceeded);
}

#[test]
fn s2_05_a_disabled_ceiling_never_reports_exceeded() {
    // Off by default, and off must mean off: a disabled cap reporting
    // `Exceeded` would stop work nobody asked to stop.
    let cap = DailyCap {
        enabled: false,
        limit_usd: 0.01,
    };
    assert_eq!(cap.evaluate(1_000_000.0), CapState::Disabled);
    assert_eq!(
        cap.fraction_used(500.0),
        None,
        "a disabled cap has no meter to fill"
    );
}

#[test]
fn s2_05_the_meter_saturates_rather_than_reporting_over_one() {
    let cap = DailyCap {
        enabled: true,
        limit_usd: 10.0,
    };
    assert_eq!(cap.fraction_used(0.0), Some(0.0));
    assert_eq!(cap.fraction_used(5.0), Some(0.5));
    assert_eq!(cap.fraction_used(10.0), Some(1.0));
    assert_eq!(
        cap.fraction_used(999.0),
        Some(1.0),
        "a progress bar past 100% renders as a bug, not as a warning"
    );
}

// ── S2-12 · a bounded task stops at its bound ────────────────────────────────

#[test]
fn s2_12_a_fifty_cent_budget_stops_at_fifty_cents() {
    // Expressed against real prices rather than invented numbers, because the
    // question "did it stop at $0.50" is only meaningful if $0.50 of spend is
    // computed the same way the app computes it.
    let cap = DailyCap {
        enabled: true,
        limit_usd: 0.50,
    };

    let model = "claude-sonnet-5";
    let price = pricing::price_for(model).expect("the default model must be priced");

    // Tokens costing just under, and just over, the ceiling.
    let just_under = Usage {
        input: ((0.49 / price.input) * 1_000_000.0) as u32,
        cached_read: 0,
        cache_write: 0,
        output: 0,
        reasoning: 0,
        tool: 0,
    };
    let spent_under = pricing::cost_usd(model, &just_under).expect("priced");
    assert!(spent_under < 0.50, "fixture must sit under the ceiling");
    assert_eq!(cap.evaluate(spent_under), CapState::Ok);

    let just_over = Usage {
        input: ((0.51 / price.input) * 1_000_000.0) as u32,
        ..just_under
    };
    let spent_over = pricing::cost_usd(model, &just_over).expect("priced");
    assert!(spent_over > 0.50);
    assert_eq!(cap.evaluate(spent_over), CapState::Exceeded);
}

#[test]
fn s2_12_reasoning_tokens_count_toward_the_ceiling() {
    // Reasoning tokens are invisible in the response but bill at the OUTPUT
    // rate. A ceiling that ignored them would be systematically wrong on
    // exactly the models that spend the most.
    let model = "claude-sonnet-5";

    let visible_only = Usage {
        input: 1_000,
        cached_read: 0,
        cache_write: 0,
        output: 1_000,
        reasoning: 0,
        tool: 0,
    };
    let with_reasoning = Usage {
        reasoning: 10_000,
        ..visible_only
    };

    let a = pricing::cost_usd(model, &visible_only).expect("priced");
    let b = pricing::cost_usd(model, &with_reasoning).expect("priced");
    assert!(
        b > a,
        "reasoning tokens are billed and must raise the cost ({a} vs {b})"
    );
}

// ── S2-04 / S2-07 · a key never arrives from the environment ─────────────────

#[test]
fn s2_04_and_s2_07_no_provider_key_can_arrive_from_the_environment() {
    // The smoke asks: with the keychain empty and every provider env var set,
    // does the app refuse to run a turn?
    //
    // It refuses **by construction** — there is no code path that reads a
    // provider key from the environment, in the app or in `arcanactl`. That is
    // stronger than observing a refusal once, and it is what `TG-P2-07`
    // enforces in CI.
    //
    // Asserted here from the other side: setting the variables changes
    // nothing, because nothing consults them.
    std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-should-never-be-used");
    std::env::set_var("OPENAI_API_KEY", "sk-should-never-be-used");
    std::env::set_var("GOOGLE_API_KEY", "AIza-should-never-be-used");

    // Building a provider requires a key passed IN. There is no constructor
    // that finds one for itself.
    let p = arcana_core::provider::factory::build("anthropic", String::new())
        .expect("builds with an explicitly empty key");
    assert_eq!(p.id(), "anthropic");

    // Clean up: these are process-global and would otherwise leak into any
    // test that runs after this one in the same binary.
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("GOOGLE_API_KEY");
}

// ── S2-11 · tool-call order is deterministic ACROSS processes ────────────────

#[test]
fn s2_11_tool_call_ordering_does_not_depend_on_hash_iteration_order() {
    // The defect (P2-59) was non-deterministic across PROCESSES: `HashMap`
    // iteration order is randomised per process, so a transcript could be
    // correct on every run during development and wrong in the field.
    //
    // The fix was structural — `BTreeMap` keyed by the wire's content-block
    // index — so the property is checked structurally too. Re-running the same
    // assembly in one process cannot detect a per-process hash seed; asserting
    // the ordering path holds no `HashMap` can.
    let engine =
        read_source(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agent/engine.rs"));

    let assembler = engine
        .split("fn assemble_in_wire_order(")
        .nth(1)
        .expect("assemble_in_wire_order must exist");
    let body = assembler
        .split_once("\n}\n")
        .map(|(b, _)| b)
        .unwrap_or(assembler);

    assert!(
        !body.contains("HashMap"),
        "the ordering path uses a HashMap, whose iteration order is randomised per \
         process — the transcript would be correct in development and wrong in the field"
    );
    assert!(
        body.contains("BTreeMap"),
        "the ordering path must be keyed by the wire block index in a sorted map"
    );

    // And the prefix builder sorts its tools for the same reason.
    let prefix =
        read_source(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cache/prefix.rs"));
    assert!(
        prefix.contains("tools.sort_by"),
        "an unsorted tool list produces a different cache prefix on every restart"
    );
}

// ── S2-06 · the daily counter survives a restart ─────────────────────────────

#[test]
fn s2_06_the_daily_spend_counter_is_persisted_and_date_stamped() {
    // The defect: `cost_today_usd` lived only in `AppState`, so the ceiling read
    // ZERO on every launch. A user who reached the cap could close the app,
    // reopen it, and carry on spending — the cap was per-session while calling
    // itself daily.
    //
    // Checked structurally rather than by launching a second process: the
    // property is that spend is written on every add and read back at startup,
    // and both halves have to exist for either to matter.
    let state_src = read_source(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/state/app_state.rs"),
    );

    let add_cost = state_src
        .split("pub fn add_cost(")
        .nth(1)
        .and_then(|r| r.split_once("\n    }"))
        .map(|(b, _)| b)
        .expect("add_cost must exist");
    assert!(
        add_cost.contains("persist_today"),
        "add_cost does not persist, so the ceiling resets to zero on every launch"
    );

    assert!(
        state_src.contains("pub fn hydrate_today"),
        "nothing restores the counters, so persisting them changes nothing"
    );

    // The DATE is what makes restoring safe. Without it, yesterday's spend is
    // carried into today and caps a user who has spent nothing.
    let hydrate = state_src
        .split("pub fn hydrate_today(")
        .nth(1)
        .expect("hydrate_today");
    assert!(
        hydrate.contains("roll_day_if_needed"),
        "hydration must discard figures belonging to a previous day, or a restart \
         carries yesterday's spend into today"
    );

    // And startup must actually call it — a hydrate nobody calls is the same
    // silent nothing as no hydrate at all.
    let core_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = core_root.parent().expect("workspace root");
    let main_src = read_source(&workspace_root.join("src-tauri/src/main.rs"));
    assert!(
        main_src.contains("arcana_core::runtime::initialize("),
        "desktop startup must enter the shared runtime initializer"
    );
    let cli_src = read_source(&workspace_root.join("src-cli/src/commands/fleet.rs"));
    assert!(
        cli_src.contains("runtime::initialize(CoreRuntimeOptions"),
        "CLI Fleet must enter the same shared runtime initializer"
    );
    let runtime_src = read_source(&core_root.join("src/runtime.rs"));
    let initialize = runtime_src
        .split("pub async fn initialize(")
        .nth(1)
        .and_then(|body| body.split_once("\n}\n"))
        .map(|(body, _)| body)
        .expect("shared runtime initializer body");
    assert!(
        initialize.contains("hydrate_state(&state)"),
        "shared startup must call its state hydration helper"
    );
    let hydrate_state = runtime_src
        .split("fn hydrate_state(")
        .nth(1)
        .and_then(|body| body.split_once("\n}\n"))
        .map(|(body, _)| body)
        .expect("shared startup hydration body");
    assert!(
        hydrate_state.contains("state.hydrate_today()"),
        "shared startup hydration must restore persisted spend"
    );
}

#[test]
fn s2_06_a_stale_stored_day_is_discarded_rather_than_carried_forward() {
    // The other direction, tested against the real rollover logic: figures
    // stamped with a previous date must zero, not accumulate.
    use arcana_core::state::app_state::AppState;

    let mut s = AppState::new();
    s.cost_today_usd = 42.0;
    s.tokens_in_today = 1_000;
    s.cost_day = Some(chrono::NaiveDate::from_ymd_opt(2020, 1, 1).expect("a date in the past"));

    s.roll_day_if_needed(chrono::Local::now().date_naive());

    assert_eq!(
        s.cost_today_usd, 0.0,
        "yesterday's spend must not cap today"
    );
    assert_eq!(s.tokens_in_today, 0);
}
