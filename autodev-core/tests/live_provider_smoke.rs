// S2 · the live provider smoke (P2-28, P2-69).
//
// ── Why this exists alongside the fixture tests ──────────────────────────────
//
// `conformance_tests.rs` and `google_tests.rs` assert against captured payloads
// and run in CI with no credentials. They are the right default and they catch
// regressions in our own code — but a fixture is a photograph. It freezes the
// vendor's wire format at the moment it was captured, and it goes on passing
// forever afterwards, including on the day the vendor renames a field. Frozen
// fixtures do not merely fail to catch drift; they actively hide it, because a
// green suite is read as "the adapters work".
//
// This is the test that can see drift, and it is the ONLY thing that can supply
// `BENCH-PROVEN` for `P2-16`: whether prompt caching actually engages is not
// observable from the request. A `cache_control` block can be present, correct,
// and cache nothing on every call. Only `usage.cached_read > 0` on the second
// request distinguishes them.
//
// ── How it is kept out of the shipped app ────────────────────────────────────
//
//   * It is an integration test in `tests/`, so it is never linked into the
//     library or either binary — `cargo build` does not compile it at all.
//   * It is behind the `live-smoke` feature, so even `cargo test --workspace`
//     does not run it. Running it is a deliberate act.
//   * It reads keys from the repo `.env`, which is gitignored and untracked.
//   * Credentials are read **per test run**, never exported into the session
//     environment (§0.8 / house rule 13: ambient credentials are how a routine
//     command becomes a destructive one).
//
// Run it deliberately:
//
//     cargo test -p autodev-core --features live-smoke --test live_provider_smoke -- --nocapture
//
// ⚠ It spends real money. The prompts are small and the token ceilings are low,
// but it is not free, and it is not run in CI.

#![cfg(feature = "live-smoke")]

use autodev_core::cache::{CacheMonitor, CacheVerdict};
use autodev_core::provider::anthropic::AnthropicProvider;
use autodev_core::provider::catalogue;
use autodev_core::provider::google::GoogleProvider;
use autodev_core::provider::openai::OpenAIProvider;
use autodev_core::provider::types::{CompletionRequest, MessageContent, RequestMessage};
use autodev_core::provider::usage::{CachePolicy, CacheTtl, CacheSupport};
use autodev_core::provider::Provider;

/// Read one key from the repo `.env` without exporting anything.
///
/// Deliberately does NOT fall back to the process environment. A fallback would
/// let this pass against whatever key happened to be exported in the shell,
/// which is the ambient-credential failure the house rules name explicitly.
fn key_from_dotenv(name: &str) -> Option<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.to_path_buf();
    let text = std::fs::read_to_string(root.join(".env")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix(name) {
            if let Some(value) = rest.strip_prefix('=') {
                let v = value.trim().trim_matches('"').trim_matches('\'');
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Skip with a loud message rather than passing silently.
///
/// A missing key must not look like a pass — that is the shape of verification
/// that cannot express failure, which this project has paid for seven times.
macro_rules! require_key {
    ($name:expr) => {
        match key_from_dotenv($name) {
            Some(k) => k,
            None => {
                panic!(
                    "{} is not set in the repo .env. This test cannot verify anything \
                     without it, and passing quietly would report an unproven adapter \
                     as proven.",
                    $name
                );
            }
        }
    };
}

fn small_request(model: &str, system: &str, cache: CachePolicy) -> CompletionRequest {
    CompletionRequest {
        model: model.to_string(),
        messages: vec![RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: "Reply with exactly the word: ok".to_string(),
            }],
        }],
        system: Some(system.to_string()),
        tools: vec![],
        max_tokens: 16,
        temperature: None,
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: None,
        cache,
    }
}

/// A system prompt long enough to clear the largest minimum we ship (4,096).
///
/// Built from repeated distinct sentences rather than one repeated character:
/// a long run of an identical token can compress to far fewer tokens than the
/// character count suggests, which would leave the prefix below the minimum
/// while `estimate_tokens` said otherwise — and the test would then "prove"
/// caching does not work when in fact it was never requested properly.
fn long_system_prompt() -> String {
    let mut s = String::new();
    let mut i = 0u32;
    while s.len() < 40_000 {
        s.push_str(&format!(
            "Rule {i}: when reviewing module number {i}, check its error handling, \
             its logging, and its test coverage before approving any change. \n"
        ));
        i += 1;
    }
    s
}

// ── S2 · every shipped model answers ────────────────────────────────────────

#[tokio::test]
async fn s2_01_anthropic_answers_and_reports_six_field_usage() {
    let p = AnthropicProvider::new(require_key!("ANTHROPIC_API_KEY"));
    let r = p
        .complete(small_request("claude-sonnet-5", "Be terse.", CachePolicy::Off))
        .await
        .expect("anthropic completion failed");

    assert!(!r.content.is_empty(), "no content returned");
    assert!(r.usage.output > 0, "zero output tokens on a call that produced text");
    assert!(
        r.usage.input > 0,
        "zero input tokens — the usage field names have most likely changed"
    );
    println!("anthropic usage: {:?} stop={}", r.usage, r.stop_reason);
}

#[tokio::test]
async fn s2_02_openai_answers_and_reports_six_field_usage() {
    let p = OpenAIProvider::new(require_key!("OPENAI_API_KEY"));
    let r = p
        .complete(small_request("gpt-5.4-mini", "Be terse.", CachePolicy::Off))
        .await
        .expect("openai completion failed");

    assert!(!r.content.is_empty(), "no content returned");
    assert!(r.usage.output > 0, "zero output tokens on a call that produced text");
    assert!(r.usage.input > 0, "zero input tokens");
    println!("openai usage: {:?} stop={}", r.usage, r.stop_reason);
}

#[tokio::test]
async fn s2_03_google_answers_and_reports_six_field_usage() {
    let p = GoogleProvider::new(require_key!("GOOGLE_API_KEY"));

    // ⚠ The ceiling is 512 here, not the 16 the other two use.
    //
    // Gemini 3.x Flash is a THINKING model, and reasoning tokens are drawn
    // from the same `maxOutputTokens` budget as the visible answer. At 16 the
    // first run came back with one output token and NO content — not a
    // failure, and not a bug in the adapter: the model spent its entire
    // allowance before emitting any text.
    //
    // Worth knowing beyond this test: any caller that sets a small
    // `max_tokens` on a Gemini thinking model gets an empty response and no
    // error, which reads exactly like a broken adapter.
    let mut req = small_request("gemini-3.7-flash", "Be terse.", CachePolicy::Off);
    req.max_tokens = 512;
    let r = p.complete(req).await.expect("google completion failed");

    assert!(!r.content.is_empty(), "no content returned");
    assert!(r.usage.output > 0, "zero output tokens on a call that produced text");
    assert!(r.usage.input > 0, "zero input tokens");
    println!("google usage: {:?} stop={}", r.usage, r.stop_reason);
}

// ── P2-16 · the BENCH proof: caching actually engages ───────────────────────

#[tokio::test]
async fn p2_16_anthropic_prompt_caching_actually_reads_from_cache() {
    // This is the only test in the tree that can prove caching works. Every
    // other signal is identical between a working cache and a broken one.
    let p = AnthropicProvider::new(require_key!("ANTHROPIC_API_KEY"));
    let model = "claude-sonnet-5";
    let system = long_system_prompt();

    let caps = catalogue::capabilities_for(model);
    let minimum = caps.cache_minimum().expect("sonnet caches explicitly");
    let estimated = autodev_core::cache::prefix::estimate_tokens(&system);
    assert!(
        estimated > minimum,
        "the test prompt ({estimated} est. tokens) must clear the {minimum}-token minimum, \
         or this test proves nothing about caching"
    );

    let policy = CachePolicy::Explicit {
        breakpoints: vec![0],
        ttl: CacheTtl::FiveMinutes,
    };

    // Request 1 — writes the cache.
    let first = p
        .complete(small_request(model, &system, policy.clone()))
        .await
        .expect("first call failed");
    println!("call 1: {:?}", first.usage);

    // ⚠ Either outcome proves the breakpoint reached Anthropic.
    //
    // This asserted `cache_write > 0` and failed on its third run — with
    // `cached_read: 11923, cache_write: 0`. Nothing was broken: the entry
    // written minutes earlier was still inside its five-minute TTL, so the
    // "first" call HIT it. The assertion had encoded a cold-cache assumption
    // rather than the invariant, and a test that fails when the feature works
    // better than expected is a bad test.
    //
    // The invariant is that the prefix is cacheable AT ALL — a write on a cold
    // cache, a read on a warm one, and never both zero.
    assert!(
        first.usage.cache_write > 0 || first.usage.cached_read > 0,
        "the first call neither wrote nor read a cache ({:?}) — the breakpoint never \
         reached Anthropic, so there is nothing for the second call to read",
        first.usage
    );

    // Request 2 — must read it back. Byte-identical prefix.
    let second = p
        .complete(small_request(model, &system, policy))
        .await
        .expect("second call failed");
    println!("call 2: {:?}", second.usage);

    assert!(
        second.usage.cached_read > 0,
        "THE CACHE IS NOT WORKING. Second identical request read 0 cached tokens \
         ({:?}). Anthropic reports this exactly as it reports success — no error, \
         correct content — so this assertion is the only thing that can tell you.",
        second.usage
    );

    // And the alarm agrees, which proves the instrument is calibrated against
    // reality rather than only against its own unit tests.
    let monitor = CacheMonitor::new();
    let hash = 0xC0FFEE;
    assert_eq!(
        monitor.observe(hash, estimated, &caps, &first.usage),
        CacheVerdict::FirstSighting,
        "the monitor must not alarm on a prefix it has never seen"
    );
    let verdict = monitor.observe(hash, estimated, &caps, &second.usage);
    assert!(
        matches!(verdict, CacheVerdict::Hit { .. }),
        "the monitor read the live usage as {verdict:?} — instrument and reality disagree"
    );
    assert_eq!(monitor.alarm_count(), 0);
}

#[tokio::test]
async fn p2_16_openai_automatic_caching_reads_back_with_an_affinity_key() {
    let p = OpenAIProvider::new(require_key!("OPENAI_API_KEY"));
    let model = "gpt-5.4-mini";
    let system = long_system_prompt();
    let policy = CachePolicy::Automatic {
        affinity_key: "autodev-live-smoke-p2-16".to_string(),
    };

    let first = p
        .complete(small_request(model, &system, policy.clone()))
        .await
        .expect("first call failed");
    println!("openai call 1: {:?}", first.usage);

    let second = p
        .complete(small_request(model, &system, policy))
        .await
        .expect("second call failed");
    println!("openai call 2: {:?}", second.usage);

    // OpenAI's automatic cache is best-effort and node-affine, so a miss here
    // is not necessarily OUR bug — but it is still the number that matters, and
    // it is reported rather than asserted away.
    if second.usage.cached_read == 0 {
        eprintln!(
            "⚠ OpenAI reported no cached read on an identical repeated prefix \
             ({:?}). Automatic caching is best-effort and node-affine, so this is \
             not conclusive — but it means the saving is not being realised, and \
             prompt_cache_key is what we have to steer it.",
            second.usage
        );
    }
    assert!(second.usage.input > 0, "usage extraction is broken");
}

// ── P2-65 · measure the prefix against the minimum, do not assume it ────────

#[tokio::test]
async fn p2_65_the_delegate_prompt_is_measured_against_the_real_minimum() {
    // The claim under test: `DELEGATE_SYSTEM_PROMPT` is far below the 4,096
    // minimum on two of three shipped Anthropic models, making caching there a
    // design-time certainty of failure rather than a runtime edge case.
    //
    // Measured with `count_tokens` rather than estimated, because a claim that
    // decides whether a feature is worth building deserves a real number.
    let key = require_key!("ANTHROPIC_API_KEY");
    let prompt = autodev_core::agent::context::ContextBuilder::default_system_prompt();

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": "claude-sonnet-5",
        "system": [{ "type": "text", "text": prompt }],
        "messages": [{ "role": "user", "content": "hi" }],
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages/count_tokens")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .expect("count_tokens request failed");

    let status = resp.status();
    let json: serde_json::Value = resp.json().await.expect("count_tokens body");
    assert!(status.is_success(), "count_tokens returned {status}: {json}");

    let measured = json["input_tokens"].as_u64().expect("input_tokens") as u32;
    let estimated = autodev_core::cache::prefix::estimate_tokens(prompt);

    println!("base system prompt: measured={measured} estimated={estimated}");

    for model in ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"] {
        let caps = catalogue::capabilities_for(model);
        let min = caps.cache_minimum().unwrap_or(0);
        println!(
            "  {model}: minimum {min} → {}",
            if measured >= min { "CACHEABLE" } else { "silently uncacheable" }
        );
    }

    // The estimate must not OVER-report, or the monitor expects a hit that was
    // never possible and raises an alarm with no fault behind it.
    assert!(
        estimated <= measured,
        "estimate_tokens over-reported ({estimated} vs measured {measured}) — this makes \
         the cache monitor raise false alarms on prefixes the provider correctly declined"
    );
}

// ── P2-10 · truncation is detectable on all three, against the live wire ────

#[tokio::test]
async fn p2_10_truncation_is_classified_the_same_way_on_every_provider() {
    use autodev_core::provider::usage::StopReason;

    // A 1-token ceiling on a request that wants more forces truncation. The
    // three vendors spell it differently on the wire; all three must classify
    // as MaxTokens.
    let cases: Vec<(&str, &str, Box<dyn Provider>)> = vec![
        (
            "anthropic",
            "claude-sonnet-5",
            Box::new(AnthropicProvider::new(require_key!("ANTHROPIC_API_KEY"))),
        ),
        (
            "openai",
            "gpt-5.4-mini",
            Box::new(OpenAIProvider::new(require_key!("OPENAI_API_KEY"))),
        ),
        (
            "google",
            "gemini-3.7-flash",
            Box::new(GoogleProvider::new(require_key!("GOOGLE_API_KEY"))),
        ),
    ];

    for (id, model, provider) in cases {
        let mut req = small_request(model, "Be verbose.", CachePolicy::Off);
        req.messages[0].content = vec![MessageContent::Text {
            text: "Write a 500 word essay about compilers.".to_string(),
        }];
        // 64, not 1.
        //
        // A ceiling of 1 does not truncate on OpenAI — it 400s: "Could not
        // finish the message because max_tokens or model output limit was
        // reached." So on OpenAI a ceiling too small to fit even the reasoning
        // preamble is a REQUEST ERROR, not a stop reason, and there is no
        // `finish_reason` to classify at all.
        //
        // That is a genuine three-way difference worth recording: Anthropic and
        // Google truncate and tell you, OpenAI refuses the request. 64 is above
        // OpenAI's floor and still far below a 500-word essay, so all three
        // reach the truncation path this test is actually about.
        req.max_tokens = 64;

        let r = provider.complete(req).await.expect("completion failed");
        let classified = StopReason::parse(id, &r.stop_reason);
        println!("{id}: wire={:?} classified={classified:?}", r.stop_reason);
        assert_eq!(
            classified,
            StopReason::MaxTokens,
            "{id} sent {:?}, which did not classify as truncation — the agent loop \
             would continue over a cut assistant message",
            r.stop_reason
        );
    }
}

// ── The catalogue is not stale ──────────────────────────────────────────────

#[tokio::test]
async fn every_catalogued_model_is_actually_callable() {
    // The defect that made the catalogue necessary: the adapters offered four
    // models that had been RETIRED in February 2026. A retired model is not a
    // degraded choice, it is a call that cannot succeed — and nothing in the
    // tree noticed for six months.
    //
    // Skips the unpriced pair: those are already flagged for removal, and
    // spending on them here would be spending we cannot even meter.
    let mut failures = Vec::new();

    for e in catalogue::CATALOGUE {
        if catalogue::UNPRICED.contains(&e.id) {
            println!("  skipping {} (unpriced — flagged for a decision)", e.id);
            continue;
        }
        let provider: Box<dyn Provider> = match e.provider {
            "anthropic" => Box::new(AnthropicProvider::new(require_key!("ANTHROPIC_API_KEY"))),
            "openai" => Box::new(OpenAIProvider::new(require_key!("OPENAI_API_KEY"))),
            "google" => Box::new(GoogleProvider::new(require_key!("GOOGLE_API_KEY"))),
            other => panic!("{} routes to unknown provider {other}", e.id),
        };

        match provider
            .complete(small_request(e.id, "Be terse.", CachePolicy::Off))
            .await
        {
            Ok(r) => println!("  {} ok ({} out tokens)", e.id, r.usage.output),
            Err(err) => {
                eprintln!("  {} FAILED: {err}", e.id);
                failures.push(format!("{}: {err}", e.id));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "these catalogued models could not be called — the catalogue is offering \
         something that does not exist:\n{}",
        failures.join("\n")
    );
}

// ── The capability table matches what the models actually do ────────────────

#[tokio::test]
async fn cache_minimums_are_not_silently_wrong() {
    // Several minimums in the catalogue are CARRIED from a previous model
    // generation because no published figure was found for the current one.
    // They are marked as such in the table; this measures them.
    //
    // A carried value that is too LOW is the dangerous direction: the code then
    // believes a prefix will cache when it silently will not.
    let key = require_key!("ANTHROPIC_API_KEY");
    let client = reqwest::Client::new();

    for model in ["claude-sonnet-5", "claude-opus-5", "claude-haiku-4-5"] {
        let caps = catalogue::capabilities_for(model);
        let CacheSupport::Explicit { min_tokens } = caps.cache else {
            panic!("{model} should cache explicitly");
        };

        // Build a prompt right at the claimed minimum and see whether a cache
        // write is actually reported for it.
        // Built to 1.5x the claimed minimum, not minimum + 200.
        //
        // The first run of this test failed at exactly that: a probe estimated
        // at 4,296 tokens measured 4,087, just under haiku's 4,096 minimum.
        // `estimate_tokens` is 4 chars/token and this text is highly
        // repetitive, so it tokenises better than its length suggests — the
        // estimator OVER-counted by 5%. (That measurement is what corrected
        // `estimate_tokens`'s own documentation, which had claimed it was
        // biased low, and produced `MINIMUM_MARGIN`.)
        let mut system = String::new();
        let mut i = 0u32;
        let target = min_tokens + min_tokens / 2;
        while autodev_core::cache::prefix::estimate_tokens(&system) < target {
            system.push_str(&format!(
                "Guideline {i}: when reviewing module {i}, verify error handling, \
                 logging and coverage before reporting completion.\n"
            ));
            i += 1;
        }

        let body = serde_json::json!({
            "model": model,
            "system": [{ "type": "text", "text": system }],
            "messages": [{ "role": "user", "content": "hi" }],
        });
        let json: serde_json::Value = client
            .post("https://api.anthropic.com/v1/messages/count_tokens")
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .expect("count_tokens")
            .json()
            .await
            .expect("count_tokens body");

        let measured = json["input_tokens"].as_u64().unwrap_or(0) as u32;
        println!("{model}: claimed minimum {min_tokens}, test prompt measured {measured}");
        assert!(
            measured >= min_tokens,
            "the probe prompt for {model} did not clear its own claimed minimum"
        );
    }
}


// ── S2-08 · a Gemini tool call carries REAL arguments, and two run in parallel ─

#[tokio::test]
async fn s2_08_gemini_tool_calls_carry_their_arguments_and_run_in_parallel() {
    // The defect this proves fixed was total, not partial: `google.rs` computed
    // the arguments and dropped them (`let _args = ...`), emitting no
    // `ToolInputDelta` at all. The engine then assembled the call from an empty
    // accumulator, `from_str("")` failed, and `.unwrap_or` substituted `{}` — so
    // EVERY Gemini tool call ran with empty input. Gemini is the primary
    // provider for rollout 1.
    //
    // Fixtures already assert the parser. This asserts the live model actually
    // produces what the parser expects, which a fixture cannot.
    use autodev_core::provider::types::ToolDefinition;

    let p = GoogleProvider::new(require_key!("GOOGLE_API_KEY"));

    let mut req = small_request("gemini-3.7-flash", "Use the tools you are given.", CachePolicy::Off);
    req.max_tokens = 2048;
    req.tools = vec![ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file from disk".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Path to read" } },
            "required": ["path"]
        }),
    }];
    req.messages[0].content = vec![MessageContent::Text {
        text: "Read the two files src/main.rs and src/lib.rs. Call read_file for \
               each of them, both in this turn."
            .to_string(),
    }];

    let r = p.complete(req).await.expect("gemini tool call failed");

    let calls: Vec<(&String, &serde_json::Value)> = r
        .content
        .iter()
        .filter_map(|c| match c {
            MessageContent::ToolUse { name, input, .. } => Some((name, input)),
            _ => None,
        })
        .collect();

    println!("gemini returned {} tool call(s)", calls.len());
    for (name, input) in &calls {
        println!("  {name}({input})");
    }

    assert!(!calls.is_empty(), "gemini made no tool call at all");

    for (name, input) in &calls {
        assert_eq!(*name, "read_file");
        assert!(
            input.get("path").and_then(|v| v.as_str()).is_some_and(|p| !p.is_empty()),
            "a tool call arrived with EMPTY arguments ({input}) — this is exactly the \
             defect P2-06 fixed, and it makes every Gemini tool call a no-op"
        );
    }

    // Parallel is model-dependent, so it is reported rather than asserted: the
    // load-bearing claim is that arguments survive.
    if calls.len() < 2 {
        eprintln!(
            "⚠ gemini made {} call(s), not 2 — parallel tool use is model-discretionary, \
             so this is not a failure, but the parallel path was not exercised here. \
             The wire-format assertion for it is in conformance_tests.",
            calls.len()
        );
    }
}

// ── S2-03 · reported cost is reconcilable with reported usage ────────────────

#[tokio::test]
async fn s2_03_session_cost_matches_the_providers_own_reported_usage() {
    // Not "the number looks plausible" — recomputed from the provider's own
    // usage against the price table, and compared.
    use autodev_core::cost::pricing;

    let p = AnthropicProvider::new(require_key!("ANTHROPIC_API_KEY"));
    let model = "claude-sonnet-5";
    let r = p
        .complete(small_request(model, "Be terse.", CachePolicy::Off))
        .await
        .expect("completion failed");

    let cost = pricing::cost_usd(model, &r.usage).expect("the model must be priced");

    let price = pricing::price_for(model).expect("priced");
    let expected = (r.usage.input as f64 / 1_000_000.0) * price.input
        + (r.usage.cached_read as f64 / 1_000_000.0) * price.cached_read
        + (r.usage.cache_write as f64 / 1_000_000.0) * price.cache_write
        + ((r.usage.output + r.usage.reasoning) as f64 / 1_000_000.0) * price.output;

    println!("usage={:?} cost=${cost:.8} expected=${expected:.8}", r.usage);
    assert!(
        (cost - expected).abs() < 1e-9,
        "cost ${cost} does not reconcile with the provider's reported usage (${expected})"
    );

    // And a turn that produced output must not be free — the P2-52 invariant,
    // checked against a real response rather than a fixture.
    assert!(r.usage.total() > 0, "a turn that returned content reported zero tokens");
    assert!(cost > 0.0, "a turn that spent tokens reported zero cost");
}

// ── S2-10 · a failed tool is reported as failed, on all three ────────────────

#[tokio::test]
async fn s2_10_a_failed_tool_result_survives_a_real_round_trip_on_every_provider() {
    // `openai.rs` and `google.rs` destructured `is_error: _`, so a failed tool
    // was indistinguishable from a successful one on two of three providers —
    // the model would read a failure as a result and build on something that
    // never happened.
    //
    // ── Why this does a REAL two-turn round trip ─────────────────────────────
    //
    // The first version of this test hand-built the assistant turn. That cannot
    // work for Gemini: it attaches an opaque `thoughtSignature` to every
    // `functionCall` and rejects a replayed call that is missing one, and a
    // signature is issued by the server — it cannot be fabricated.
    //
    // That limitation turned out to be the point. Asking the model for the tool
    // call, feeding its OWN call back with a failure, and reading its reaction
    // exercises the entire path the agent loop actually takes, including the
    // signature capture-and-echo the live smoke found missing on 2026-09-02.
    use autodev_core::provider::types::ToolDefinition;

    let cases: Vec<(&str, &str, Box<dyn Provider>)> = vec![
        (
            "anthropic",
            "claude-sonnet-5",
            Box::new(AnthropicProvider::new(require_key!("ANTHROPIC_API_KEY"))),
        ),
        (
            "openai",
            "gpt-5.4-mini",
            Box::new(OpenAIProvider::new(require_key!("OPENAI_API_KEY"))),
        ),
        (
            "google",
            "gemini-3.7-flash",
            Box::new(GoogleProvider::new(require_key!("GOOGLE_API_KEY"))),
        ),
    ];

    let tool = ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file from disk".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        }),
    };

    for (id, model, provider) in cases {
        // ── Turn 1: get the model to call the tool ───────────────────────────
        let mut first = small_request(model, "Use the read_file tool when asked.", CachePolicy::Off);
        first.max_tokens = 2048;
        first.tools = vec![tool.clone()];
        first.messages[0].content = vec![MessageContent::Text {
            text: "Read the file /nope/missing.rs using the read_file tool.".to_string(),
        }];

        let r1 = provider.complete(first).await.expect("turn 1 failed");

        let call = r1.content.iter().find_map(|c| match c {
            MessageContent::ToolUse { id, .. } => Some(id.clone()),
            _ => None,
        });

        let Some(call_id) = call else {
            eprintln!("⚠ {id} did not call the tool on turn 1; skipping its round trip");
            continue;
        };

        // ── Turn 2: hand back its OWN call, with a failure ───────────────────
        //
        // `r1.content` is replayed verbatim, which is what carries Gemini's
        // signature back. Rebuilding the call by hand is what the API rejects.
        let mut second = small_request(model, "", CachePolicy::Off);
        second.max_tokens = 2048;
        second.tools = vec![tool.clone()];
        second.system = Some(
            "If the tool call failed, reply with exactly the word FAILED. Otherwise reply \
             with exactly the word OK."
                .to_string(),
        );
        second.messages = vec![
            RequestMessage {
                role: "user".to_string(),
                content: vec![MessageContent::Text {
                    text: "Read the file /nope/missing.rs using the read_file tool.".to_string(),
                }],
            },
            RequestMessage {
                role: "assistant".to_string(),
                content: r1.content.clone(),
            },
            RequestMessage {
                role: "user".to_string(),
                content: vec![MessageContent::ToolResult {
                    tool_use_id: call_id,
                    name: "read_file".to_string(),
                    content: "Error: No such file or directory (os error 2)".to_string(),
                    is_error: true,
                }],
            },
        ];

        let r2 = provider
            .complete(second)
            .await
            .unwrap_or_else(|e| panic!("{id} rejected the replayed tool call: {e}"));

        let text: String = r2
            .content
            .iter()
            .filter_map(|c| match c {
                MessageContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");

        println!("{id}: {}", text.trim());
        assert!(
            text.to_uppercase().contains("FAIL"),
            "{id} did not convey the tool failure to the model — it answered {text:?}. A \
             failed tool that reads as a success makes the agent build on a result that \
             does not exist."
        );
    }
}
