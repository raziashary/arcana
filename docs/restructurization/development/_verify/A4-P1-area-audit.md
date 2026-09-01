# A4 — P1 Area Audit

**Auditor:** P1 Area Auditor (read-only)
**Scope:** the codebase area P1 touched — 261 files, `git diff --name-only eab345c~1..HEAD`
**Status:** COMPLETE. Written incrementally; every finding was on disk before the next was started.

---

## 1. Headline

I audited the code P1 created or changed: the new `autodev-core/src/db/` layer (`migrate`, `config_db`, `timestamps`, `legacy_root`, `runs`, `backup`, `workers`, `tasks`, `app_settings`, `open_tuned`), `security/redaction.rs`, the `EventSink` boundary, `agent/tool_executor.rs` and `agent/engine.rs`, `recipes/{orchestrator,runner,scheduler}.rs`, the three memory stores, `orphan/mod.rs`, `graph/migrations.rs`, the three repointed `airsctl` command modules, and the two gate files in `src-tauri`. Everything compiles, clippy is clean of errors, and all 138 tests pass — so none of what follows is visible from the build.

**43 findings: 1 BLOCKER, 19 MAJOR, 17 MINOR, 6 NIT** (counted from the file, not estimated). Nineteen of forty redaction bypass attempts left the secret intact (§3).

**The single most serious is A-01.** P1's redaction boundary lives in `ToolExecutor::execute`, whose comment states that *"Every tool result in the application passes through here"*. It does not: the agent loop calls `ToolExecutor::execute_batch` (`agent/engine.rs:314`), and neither of that method's two branches calls `redact_text` or `redact_args` — the file's only three redaction call sites are lines 117, 218 and 219, all inside `execute`. `execute` has **no production caller at all**. The phase's headline smoke test, `autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs`, drives `execute` and asserts the secret is gone — so the property is proven on a method nothing uses, while a `read_file` on a `.env` in the real product goes verbatim into the model's context, the `messages` table on disk, and the UI event. The CI gate that certifies the boundary (`ci_gate.rs:777`) is a substring check on the whole file, so it is green.

Three second-order themes run through the rest. **Guards satisfied by fiat or scoped past the violation:** `airsctl task claim` writes `status = 'assigned'` itself to satisfy the atomic claim guard it is supposed to be constrained by (A-22); the "every connection is tuned" gate walks only `autodev-core` while a live untuned open sits in `src-shared` (A-17); `reap_abandoned` treats "not my pid" as "dead" and marks running sibling processes abandoned, with `is_pid_alive` sitting unused in the same crate (A-09). **`let _ =` on a fallible write followed immediately by an event announcing it succeeded** — three fresh instances (A-12, A-33, A-34), in files whose own comments condemn the pattern. And **redaction that is simultaneously too weak and too strong**: `Authorization: Basic` leaks by exactly the mechanism the module header says was found and fixed for `Bearer` (A-02), raw JSON is untouched (A-03), and meanwhile `pass`/`auth`/`token` as bare substrings turn `author`, `passed` and `prompt_tokens` into `"[REDACTED]"` — changing JSON numbers and booleans into strings (A-06) — and corrupt source code in the transcript the model reads (A-07).

None of these are verdicts. They are evidence to be checked in the main session; §4 lists what I could not determine and §5 the code that held up under attack, including a state-level verification that the `~/.airs → ~/.autodev` session-store copy is byte-for-byte complete.

**Severity index.**
- **BLOCKER (1):** A-01
- **MAJOR (19):** A-02, A-03, A-04, A-05, A-06, A-07, A-08, A-09, A-12, A-13, A-14, A-16, A-17, A-22, A-23, A-26, A-33, A-34, A-42
- **MINOR (17):** A-10, A-11, A-15, A-18, A-19, A-21, A-24, A-25, A-29, A-30, A-31, A-32, A-35, A-37, A-39, A-41, A-43
- **NIT (6):** A-20, A-27, A-28, A-36, A-38, A-40

**By defect class from the measured profile.** #1 `let _ =` on a fallible call: A-12, A-13, A-15, A-32, A-33, A-34, A-38, A-39. #2 dead parameters: A-18, A-36, A-38. #3 guards that cannot fire: A-01, A-02, A-03, A-05, A-08, A-09, A-14, A-17, A-19, A-22, A-23, A-26, A-30. #4 repeated calls: A-13, A-20, A-25, A-29, A-42. #5 inert-but-green: A-01, A-12, A-16, A-24, A-28, A-33, A-34. #6 writer with no reader: A-09, A-11, A-27. #7 error paths that lose information: A-04, A-06, A-07, A-10, A-20, A-23, A-29, A-39, A-43. NOVEL: A-06 (JSON type change), A-21, A-22, A-31, A-37, A-40, A-41.

---

## 2. Findings

### A-01 — `execute_batch` bypasses the redaction boundary entirely (both branches)

**What is wrong.** P1's redaction boundary is applied in `ToolExecutor::execute`, whose comment claims it is the one point every tool result passes through. It is not. `ToolExecutor::execute_batch` — the method the agent loop actually calls — never calls `redact_text` or `redact_args` on any path.

**Evidence.**

`autodev-core/src/agent/tool_executor.rs:202-219` (the single-call path, redacted):

```rust
// ── Redaction boundary (M-40, P1-21) ────────────────────────────
//
// Every tool result in the application passes through here, which is
// why the redaction happens at this one point rather than in each of
// the twenty-odd tools. A `read_file` on a `.env`, a `bash` that echoes
// an environment, a `grep` that matches a credential — all of them
// arrive here, ...
let mut result = result;
result.output = crate::security::redaction::redact_text(&result.output);
let redacted_input = crate::security::redaction::redact_args(&input);
```

The batch path, same file. Concurrent read-only branch, `autodev-core/src/agent/tool_executor.rs:308-313`:

```rust
                    emit_tool_start(ToolStartPayload {
                        session_id: session_id.clone(),
                        call_id: call_id.clone(),
                        tool_name: tool.name().to_string(),
                        input: input.clone(),          // raw, not redact_args
                    });
```

`autodev-core/src/agent/tool_executor.rs:341-352` persists the raw output and raw input to the messages table:

```rust
                            let tool_msg = Message::new_tool(
                                &self.session_id,
                                &call_id,
                                &tool_name,
                                input,                  // raw
                                &result.output,         // raw
                                result.is_error,
                            );
```

`autodev-core/src/agent/tool_executor.rs:355` returns the raw result to the caller: `results[idx] = (tool_use_id, result);`

Serial write branch, `autodev-core/src/agent/tool_executor.rs:468-484`: same — `self.save_tool_message(&call_id, tool.name(), input, &result.output, result.is_error);` then `emit_tool_end(... output: result.output.clone() ...)`. No redaction call anywhere in the method.

A grep confirms it: the only redaction call sites in the whole file are lines 117, 218 and 219 — all three inside `execute`.

**And `execute_batch` is the live path.** `autodev-core/src/agent/engine.rs:314`:

```rust
                tool_executor.execute_batch(tool_batch).await?
```

**Concrete failure scenario.** The agent issues `read_file(".env")`. `read_file` is read-only, so it lands in the concurrent branch of `execute_batch`. The file's contents go verbatim into (a) the conversation the model sees, (b) the `messages` table in the session store on disk, and (c) the `tool_end` UI event. Every stated goal of M-40 fails, silently, on the path the agent actually uses.

**Severity:** `BLOCKER`
**Class:** #3 guards that cannot fire / #5 inert-but-green — the boundary exists, compiles, is unit-tested, and is not on the hot path.
**Fix:** extract the three redaction lines into a private helper and call it in all three result paths (or route `execute_batch` through `execute`), then assert it with a test that drives `execute_batch`.

> **See also "A-01 — additional evidence"** at the end of §2: `ToolExecutor::execute` has no production caller at all, and the phase's headline smoke test drives it.

---

### A-02 — `Authorization: Basic` / `Token` / any non-Bearer scheme leaks the credential

**What is wrong.** `TEXT_PATTERNS` has a `Bearer`-specific rule placed before the assignment rule *precisely because* the assignment rule stops at whitespace and would otherwise consume `Authorization:` and leave the token. That reasoning is written into the module header. But the fix was made for the literal word `Bearer` only. Every other auth scheme still hits exactly the documented failure.

**Evidence.** `autodev-core/src/security/redaction.rs:103-108`:

```rust
/// **Order matters, and getting it wrong leaks.** The narrow, unambiguous token
/// rules run first. The assignment rule stops at whitespace, so on
/// `Authorization: Bearer eyJhbGci…` it would match `Authorization: Bearer`,
/// rewrite that to `Authorization=[REDACTED]`, and leave the actual token in the
/// text.
```

Measured against the shipped patterns (harness `include!`s `redaction.rs` verbatim):

```
SURVIVED | Authorization: Basic
   in:  "Authorization: Basic dXNlcjpzdXBlcnNlY3JldHBhc3N3b3Jk"
   out: "Authorization=[REDACTED] dXNlcjpzdXBlcnNlY3JldHBhc3N3b3Jk"
SURVIVED | Authorization: Token
   in:  "Authorization: Token abcdef0123456789abcdef0123456789"
   out: "Authorization=[REDACTED] abcdef0123456789abcdef0123456789"
```

**Concrete failure scenario.** A `bash` tool runs `curl -v` against a registry that uses Basic auth, or the agent reads a `.netrc` / CI config containing `Authorization: Basic <base64(user:pass)>`. The line is "redacted" — the marker is visibly present, so a reviewer reading the log believes redaction worked — and the base64 credential is written to the messages table verbatim.

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire (the guard fires on the wrong half of the input).
**Fix:** generalise the first pattern to `(?i)\b(?:Bearer|Basic|Token|Digest|ApiKey)\s+\S+`, or let the assignment rule consume the rest of the line when the key is `authorization`.

---

### A-03 — `redact_text` does not redact secrets inside JSON text

**What is wrong.** The assignment pattern requires the key characters to be immediately followed by optional whitespace then `:` or `=`. A JSON key is quoted, so the closing quote breaks the match. `redact_text` leaves JSON documents completely untouched.

**Evidence.** `autodev-core/src/security/redaction.rs:131-137`:

```rust
            re: Regex::new(&format!(
                r#"(?i)\b([A-Za-z0-9_\-\.]*(?:{sensitive})[A-Za-z0-9_\-\.]*)\s*[:=]\s*["']?([^\s"',;]+)"#
            ))
            .expect("assignment pattern"),
            with: "$1=[REDACTED]",
```

Measured:

```
SURVIVED | raw JSON text, api_key
   in:  {"api_key": "hunter2sekrit"}      out: {"api_key": "hunter2sekrit"}
SURVIVED | raw JSON text, password
   in:  {"password":"hunter2sekrit"}      out: {"password":"hunter2sekrit"}
SURVIVED | json, space before colon
   in:  "api_key" : "hunter2sekrit"       out: "api_key" : "hunter2sekrit"
```

**Concrete failure scenario.** `redact_text` is the function applied to tool *output* (`autodev-core/src/agent/tool_executor.rs:218`) and to memory content (`autodev-core/src/memory/episodic.rs:32,77,127`, `semantic.rs:41`, `procedural.rs:129`). Tool output is very often JSON: a `bash` running `gh api …` or `aws … --output json`, a `read_file` on `credentials.json`, `mcp.json`, or a `launch.json` holding an API key. In every one of those the secret is stored verbatim and shown to the model.

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire.
**Fix:** permit an optional closing quote between the key and the separator (`["']?\s*[:=]`).

---

### A-04 — the assignment rule truncates at the first separator, so multi-token and multi-line secrets survive

**What is wrong.** The value group is `([^\s"',;]+)`. Any secret containing a space, comma, semicolon or newline is only partially redacted; the tail is left in the text next to a `[REDACTED]` marker that claims it was handled.

**Evidence.** `autodev-core/src/security/redaction.rs:133` (value class quoted above). Measured:

```
SURVIVED | password with space in value
   in:  PASSWORD="my secret passphrase"
   out: PASSWORD=[REDACTED] secret passphrase"
SURVIVED | password value with comma
   in:  password=abc,def          out: password=[REDACTED],def
SURVIVED | password value with semicolon
   in:  Password=abc;def          out: Password=[REDACTED];def
SURVIVED | multi-line yaml block scalar
   in:  "api_key: |\n  linetokenone\n  linetokentwo"
   out: "api_key=[REDACTED]\n  linetokenone\n  linetokentwo"
```

**Concrete failure scenario.** A `.yaml` secrets file using a block scalar (`api_key: |`) is read by the agent: the literal `|` is redacted and the key itself is written to the store. An ODBC/SQL-Server connection string `Server=x;Database=y;Password=p;Trusted=no` redacts only up to the `;`, i.e. it redacts `p` but a value like `Password=a b` leaks `b`.

**Severity:** `MAJOR`
**Class:** #7 — a success path that reports success while doing part of the job.
**Fix:** when an opening quote is consumed, match to the closing quote; otherwise match to end of line rather than to the first `[\s,;]`.

---

### A-05 — connection-string credentials survive unless the host happens to contain a dot

**What is wrong.** `scheme://user:password@host/db` matches no pattern. It is redacted only *by accident*, when the email pattern happens to match `password@host.tld` — which requires the host to contain a dot. `localhost`, a docker service name, or a bare internal hostname all leak.

**Evidence.** Measured:

```
SURVIVED | postgres conn string
   in:  postgres://admin:s3cr3tPassw0rd@localhost:5432/appdb
   out: postgres://admin:s3cr3tPassw0rd@localhost:5432/appdb
SURVIVED | https url with creds
   in:  https://user:s3cr3tPassw0rd@internal/path
   out: https://user:s3cr3tPassw0rd@internal/path
redacted | postgres conn, dotted host  (only because the EMAIL rule fired)
   out: postgres://admin:[EMAIL]:5432/appdb
SURVIVED | JSON conn string value (redact_json)
   in:  {"url":"postgres://admin:s3cr3tPassw0rd@localhost/db"}
   out: {"url":"postgres://admin:s3cr3tPassw0rd@localhost/db"}
```

The last line matters: `redact_json` does not save it either. The value is not under a sensitive key, and the long-token rule `\b[A-Za-z0-9_\-]{32,}\b` (`autodev-core/src/security/redaction.rs:163-164`) does not fire because word boundaries split the URL into short pieces.

**Concrete failure scenario.** `autodev-core/src/agent/tool_executor.rs:110-112` states the reason arguments are redacted: *"a token passed to an HTTP call, a password in a connection string"*. The connection string is the one shape not caught.

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire.
**Fix:** add, ahead of the email rule, `Regex::new(r"(?i)\b([a-z][a-z0-9+.\-]*)://([^/\s:@]+):([^/\s@]+)@")` with replacement `"$1://$2:[REDACTED]@"`.

---

### A-06 — `is_sensitive_key` substring matching destroys ordinary fields, and changes their JSON type

**What is wrong.** `SENSITIVE_KEYS` contains the bare fragments `pass`, `auth` and `token`, matched as case-insensitive substrings. `author`, `passed`, `pass_rate`, `prompt_tokens`, `token_count`, `passing`, `bypass`, `compass`, `surpassed` and `authenticity` are therefore all classed as secrets. In `redact_json` the value is replaced wholesale *with a string*, so a bool or a number becomes `"[REDACTED]"`.

**Evidence.** `autodev-core/src/security/redaction.rs:72-74` (`"password"`, `"pass"`, `"pwd"`), `:75` (`"token"`), `:80` (`"auth"`), applied by `autodev-core/src/security/redaction.rs:91-94`:

```rust
pub fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SENSITIVE_KEYS.iter().any(|k| lower.contains(k))
}
```

Measured:

```
is_sensitive_key("author") = true          is_sensitive_key("passed") = true
is_sensitive_key("pass_rate") = true       is_sensitive_key("prompt_tokens") = true
is_sensitive_key("token_count") = true     is_sensitive_key("compass") = true
is_sensitive_key("bypass") = true          is_sensitive_key("surpassed") = true
is_sensitive_key("authored_at") = true     is_sensitive_key("tokenizer") = true

redacted | JSON passed bool
   in:  {"passed":true,"failed":false}       out: {"failed":false,"passed":"[REDACTED]"}
redacted | JSON prompt_tokens number
   in:  {"prompt_tokens":1523}               out: {"prompt_tokens":"[REDACTED]"}
redacted | JSON token_count number
   in:  {"token_count": 42}                  out: {"token_count":"[REDACTED]"}
redacted | JSON author field
   in:  {"author":"Razi Ashary","msg":"fix"} out: {"author":"[REDACTED]","msg":"fix"}
```

The existing test `keys_match_as_substrings_case_insensitively` (`autodev-core/src/security/redaction.rs:237-251`) checks only `username`, `task_id`, `file_path`, `title` on the negative side — none of which contain `pass`/`auth`/`token`. The defect is invisible to it.

**Concrete failure scenario.** `redact_json` is applied to tool-call metadata written to episodic memory (`autodev-core/src/memory/episodic.rs:84-85`) and to every tool's input (`tool_executor.rs:117,219`). A usage record `{"prompt_tokens": 1523, "completion_tokens": 88}` is stored with a *string* in a numeric field; anything that later deserialises those rows into a typed struct fails, and token accounting reading that metadata silently loses the numbers.

**Severity:** `MAJOR`
**Class:** #7 information destroyed on a success path; `NOVEL` in that it changes the JSON *type*, not just the value.
**Fix:** require a word/`_`/`-` boundary around the short fragments (or drop bare `pass`/`auth` in favour of `password`, `passwd`, `auth_`, `_auth`), and preserve the original JSON type when redacting.

---

### A-07 — `redact_text` corrupts source code and prose *in the transcript the model reads*

**What is wrong.** The assignment rule fires on any identifier containing `pass`, `auth`, `token` or `secret` followed by `:` or `=`. That is ubiquitous in source code. And `tool_executor.rs:218` mutates `result.output` **before returning it** (`Ok(result)` at `:239`), so the model sees the corrupted text — this is not merely a logging concern.

**Evidence.** `autodev-core/src/agent/tool_executor.rs:217-218` and `:239`:

```rust
        let mut result = result;
        result.output = crate::security::redaction::redact_text(&result.output);
        ...
        Ok(result)
```

Measured:

```
in:  let auth = build_auth(cfg);            out: let auth=[REDACTED];
in:  Author: Razi Ashary <razi@example.com> out: Author=[REDACTED] Ashary <[EMAIL]>
in:  authorization: role-based access control
                                            out: authorization=[REDACTED] access control
in:  Bearer of bad news arrived today       out: Bearer [REDACTED] bad news arrived today
in:  pass_rate: 0.97                        out: pass_rate=[REDACTED]
in:  prompt_tokens: 1523                    out: prompt_tokens=[REDACTED]
in:  assert passed = true                   out: assert passed=[REDACTED]
in:  <config apiKey="hunter2sekrit"/>       out: <config apiKey=[REDACTED]"/>   (unbalanced quote)
```

Rust/TypeScript struct fields (`pub token: String,`, `password: string;`), `git log` output (`Author:` lines) and test summaries (`passed = …`) are all hit.

**Concrete failure scenario.** The agent runs `read_file` on a file containing `pub token: String,` and is handed `pub token=[REDACTED],`. It then edits the file based on text that does not exist on disk, and `edit_file`'s old-string match fails — or worse, succeeds against a different line. The module header at `autodev-core/src/security/redaction.rs:19-49` argues at length that over-redaction of free text is unacceptable *for exactly this reason*, and rejects the long-token rule on those grounds; the assignment rule it kept produces the same class of damage on far more common input.

**Severity:** `MAJOR`
**Class:** #7 information destroyed on a success path; the module's own stated design intent is violated by the shipped rule.
**Fix:** as A-06 (boundary the short fragments), and require the value to look secret-ish (length ≥ 8, or quoted) before replacing. Note this interacts with A-01: fixing A-01 without fixing this makes the corruption universal.

---

### A-08 — the PreToolUse / PostToolUse hook payload carries the unredacted input and output

**What is wrong.** Hooks fire *before* the redaction line and are handed the raw `input` and raw `result.output`. Hooks run user-configured external commands and their contexts are logged.

**Evidence.** `autodev-core/src/agent/tool_executor.rs:184-198` — on the single-call path, i.e. the path that *does* redact, but at line 218, after this:

```rust
                tool_call: Some(ToolCallContext {
                    id: call_id.clone(),
                    name: tool.name().to_string(),
                    input: input.clone(),
                    result: Some(serde_json::json!({"output": &result.output})),
                    is_error: result.is_error,
                }),
                ...
                extra: serde_json::json!({
                    "output": &result.output,
```

`HookRegistry::fire` then persists a log row per hook execution — `autodev-core/src/hooks/registry.rs:54-57`:

```rust
            let log = HookExecutor::execute(hook, ctx).await;

            // Persist log
            if let Err(e) = db.insert_log(&log) {
```

**Concrete failure scenario.** With any PostToolUse hook configured, a `read_file` on a `.env` hands the raw contents to the hook process and to the hook-log table. The comment at `tool_executor.rs:204` ("Every tool result in the application passes through here") is true of the return value on this path but not of the hook payload.

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire (boundary placed downstream of a consumer).
**Fix:** redact before constructing `post_ctx`, or redact inside `HookContext` construction.

---
### A-09 — `reap_abandoned` marks *live sibling processes* as abandoned; it never checks whether the pid is alive

**What is wrong.** `RunsDb::reap_abandoned` closes every open run whose `(pid, host)` differs from the calling process's. "Not my pid" is treated as "dead". The repository already contains a liveness checker and it is not used.

**Evidence.** `autodev-core/src/db/runs.rs:233-246`:

```rust
    pub fn reap_abandoned(&self) -> Result<usize> {
        let rows = self
            .conn
            .execute(
                "UPDATE runs SET ended_at = ?1, outcome = ?2 \
                 WHERE outcome IS NULL AND NOT (pid = ?3 AND host = ?4)",
```

with the docstring at `autodev-core/src/db/runs.rs:229-230` asserting the inverse:

```rust
    /// previous launch's row tells the truth. Runs belonging to *this* pid on
    /// *this* host are left alone — those are ours and genuinely open.
```

Available and unused: `autodev-core/src/orphan/pid_checker.rs:7` — `pub fn is_pid_alive(pid: u32) -> bool`.

Both entry points call it unconditionally at startup:
`src-tauri/src/main.rs:531` — `let _ = runs.reap_abandoned();`
`src-cli/src/main.rs:163` — `let _ = db.reap_abandoned();`

**Concrete failure scenario.** The desktop app is running; its `app` run is open with pid 1234. The user runs `airsctl task list` (pid 5678, same host). The CLI's `reap_abandoned` marks the running app's row `outcome = 'abandoned'`. When the app later exits, `runs.finish(id, OK)` (`src-tauri/src/main.rs:552`) matches zero rows because `finish` requires `outcome IS NULL` (`runs.rs:166`), returns the error at `runs.rs:170-174`, and logs *"could not close run … it was already closed"*. The ledger permanently records a clean app session as abandoned. Any concurrent pair of `airsctl` invocations does the same to each other. This is guaranteed, not a race.

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire — combined with #6 (an in-tree capability, `is_pid_alive`, with no consumer).
**Fix:** select open runs first, and only reap those whose `host` matches *and* `!is_pid_alive(pid)`; leave rows from other hosts alone.

---

### A-10 — `hostname()` returns `"unknown"` on essentially every non-shell Linux/macOS launch

**What is wrong.** `hostname()` reads only the environment. `COMPUTERNAME` is Windows-only; `HOSTNAME` is a **bash shell variable that is not exported** by default, so a process started by systemd, a desktop launcher, `.app` bundle, cron, or CI sees neither.

**Evidence.** `autodev-core/src/db/runs.rs:272-277`:

```rust
fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
```

**Concrete failure scenario.** On macOS/Linux the `host` column is the constant `"unknown"`, so the stated purpose in the docstring at `runs.rs:269` ("for telling apart runs from different machines") is not served at all. It also collapses A-09's predicate to pid alone: two machines sharing `~/.autodev` over a synced or networked home directory will reap each other's live runs on every start, and pid reuse (Linux wraps at 32768 by default) makes the *opposite* error possible — a genuinely dead run whose pid was reused is never reaped and reads as "still running" forever.

The `sysinfo` crate is already a direct dependency (`autodev-core/Cargo.toml`), and `std::env::var("COMPUTERNAME")` has no fallback to it.

**Severity:** `MINOR` on its own; it is the amplifier for A-09.
**Class:** #7 an error path that loses information (`unwrap_or_else` on a value that is meaningful).
**Fix:** fall back to `sysinfo::System::host_name()` before `"unknown"`.

---

### A-11 — `runs.db` is a writer with no reader

**What is wrong.** Nothing in production reads the ledger. `get`, `recent` and `open_runs` are called only from the module's own tests.

**Evidence.** Every production reference to `RunsDb` in the tree:

```
./src-cli/src/main.rs:161:    let ledger = autodev_core::db::runs::RunsDb::open().ok();
./src-cli/src/main.rs:163:        let _ = db.reap_abandoned();
./src-cli/src/main.rs:164:        db.begin(... kind::CLI, Some("airsctl"))
./src-cli/src/main.rs:177:        if let Err(e) = db.finish(id, result)
./src-tauri/src/main.rs:529/531/532/551/552  — open, reap, begin, open, finish
./autodev-core/examples/migrate_check.rs:46 — open only
```

`recent(…)` and `open_runs()` (`autodev-core/src/db/runs.rs:192,209`) have no non-test caller: no `airsctl runs` subcommand exists (`src-cli/src/main.rs` `Commands` enum), and no Tauri command exposes them.

**In fairness**, the module header is explicit and honest about part of this — `autodev-core/src/db/runs.rs:14-18` states that no other table carries a `run_id` yet. What it does *not* say is that the ledger has no query surface at all.

**Concrete failure scenario.** Rows accumulate in `~/.autodev/runs.db` forever with no retention policy (there is no delete/prune method in the module) and no way for a user or the app to look at them. The write path can regress — e.g. A-09 corrupting every outcome — with nothing observing it.

**Severity:** `MINOR` (documented as deliberate sequencing, but incomplete)
**Class:** #6 a writer with no reader.
**Fix:** add an `airsctl runs [--open|--recent N]` subcommand, which also gives A-09 a way to be noticed.

---

### A-12 — two surviving `let _ = self.conn.execute(...)` in `tasks.rs`, each followed by an event announcing the write succeeded

**What is wrong.** The brief for P1 removed twenty of these from `tasks.rs`; `unblock_ready_tasks` still has two. In both, the discarded `execute` is immediately followed by pushing to the return value and emitting a `task_changed` event stating the new status — so a failed UPDATE is reported to the caller and the UI as a success.

**Evidence.** `autodev-core/src/db/tasks.rs:1552-1564`:

```rust
            if all_done {
                // Transition needs_info → ready
                let _ = self.conn.execute(
                    "UPDATE tasks SET status = 'ready', updated_at = datetime('now') WHERE id = ?1",
                    params![task.id],
                );
                unblocked_ids.push(task.id);
                crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                    task_id: Some(task.id),
                    kind: "unblocked".into(),
                    status: Some("ready".into()),
```

and `autodev-core/src/db/tasks.rs:1575-1587`:

```rust
                    if let Ok(epic_id_int) = epic_id.parse::<i64>() {
                        let _ = self.conn.execute(
                            "UPDATE tasks SET status = 'done', completed_at = datetime('now'),
                             updated_at = datetime('now')
                             WHERE id = ?1 AND task_type = 'epic'",
                            params![epic_id_int],
                        );
                        crate::events::emit_task_changed(... kind: "epic_complete" ... );
```

The function's own signature is `pub fn unblock_ready_tasks(&self, completed_task_id: i64) -> Result<Vec<i64>>` (`tasks.rs:1530`) — `?` is available; the discard is not forced by the return type. The file's own header condemns the idiom at `autodev-core/src/db/tasks.rs:347`.

Note the second one is doubly silent: the `WHERE … AND task_type = 'epic'` clause can legitimately match **zero rows** (if `epic_id` points at a non-epic row), and even a checked `execute` returning `Ok(0)` would need a rowcount assertion to catch it.

**Concrete failure scenario.** The database is locked past the 5 s busy timeout by the fleet scheduler mid-tick. The UPDATE returns `SQLITE_BUSY`; the task stays `needs_info`; the caller receives its id in `unblocked_ids` and the UI shows it as `ready`. The task is then never picked up, and nothing anywhere records why.

**Severity:** `MAJOR`
**Class:** #1 `let _ =` on a fallible call; #5 inert-but-green.
**Fix:** `self.conn.execute(...)?` and assert the row count before pushing/emitting.

---

### A-13 — `let _ = db.update_task_budget_usage(...)` discards the only write that makes budget enforcement possible, and re-opens a connection per LLM turn

**What is wrong.** Two profile defects in five lines: a discarded fallible write, and a database connection opened inside the agent's per-turn loop.

**Evidence.** `autodev-core/src/agent/engine.rs:235-243`:

```rust
                if let Some(tid) = self.task_id {
                    if let Ok(db) = crate::db::tasks::TasksDb::open() {
                        let _ = db.update_task_budget_usage(
                            tid,
                            budget_status.tokens_used,
                            budget_status.cost_usd,
                            budget_status.turns_used,
                        );
                    }
                }
```

`TasksDb::open()` goes to `crate::db::open_tuned` (`autodev-core/src/db/tasks.rs:333`), which opens the file, sets `busy_timeout` and issues `PRAGMA journal_mode = WAL` — every turn, per agent, for the lifetime of the run.

**Concrete failure scenario.** If the write fails (lock contention with the fleet scheduler, which is the documented contention shape — see `autodev-core/src/analytics/queries.rs:98-101`), the task's recorded budget stops advancing while the agent keeps spending. Any budget cap reading `tasks` sees a stale figure and never trips. Both the failure and the overspend are invisible.

**Severity:** `MAJOR`
**Class:** #1 `let _ =` on a fallible call; #4 functions called repeatedly where one call would do.
**Fix:** log the error at `warn!`; hold one `TasksDb` for the engine's lifetime rather than opening per turn.

---

### A-14 — `migrate.rs`'s `is_destructive` does not recognise `DROP COLUMN` or `UPDATE`, so the automatic backup is skipped for them

**What is wrong.** The backup-before-destruction guard is driven by a fixed list of substrings. `ALTER TABLE … DROP COLUMN` — supported by SQLite since 3.35 and the most likely destructive schema change a future migration will make — is not in it. Neither is a mass `UPDATE … SET`, nor `REPLACE INTO`.

**Evidence.** `autodev-core/src/db/migrate.rs:56-70`:

```rust
fn is_destructive(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    [
        "drop table",
        "drop index",
        "drop view",
        "drop trigger",
        "delete from",
        "rename to",
        "rename column",
        "truncate",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}
```

The docstring at `autodev-core/src/db/migrate.rs:54-55` states the intent this misses: *"so that the safety net is automatic rather than something each future migration's author has to remember."*

The unit test `destructive_sql_is_recognised_and_creation_is_not` (`migrate.rs:313-327`) asserts only the four forms that *are* in the list, so the gap is invisible to it.

Two smaller edges in the same function: matching is on raw substrings, so `DROP  TABLE` (two spaces) is missed, and a comment containing "drop table" triggers a spurious backup of the whole database.

**Concrete failure scenario.** A P2 migration `ALTER TABLE tasks DROP COLUMN legacy_notes;` runs against a user's real `tasks.db` with no backup taken and no warning, because the guard reports the statement as non-destructive.

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire.
**Fix:** add `"drop column"`, `"update "`, `"replace into"`, `"drop database"`; normalise whitespace before matching; strip SQL comments.

---

### A-15 — `run_ladder` claims "was rolled back" without checking whether the rollback succeeded

**What is wrong.** The error path discards the `ROLLBACK` result and then asserts in the returned message that the rollback happened. If the rollback fails, the connection is left inside a failed transaction and the message is false.

**Evidence.** `autodev-core/src/db/migrate.rs:141-150`:

```rust
            Err(e) => {
                // Roll back before reporting, or the connection stays inside a
                // failed transaction and every later statement fails with a
                // confusing secondary error.
                let _ = conn.execute_batch("ROLLBACK");
                return Err(AutoDevError::Agent(format!(
                    "{db_name}: migration v{} ({}) failed and was rolled back: {e}",
                    m.version, m.name
                )));
            }
```

A test at `autodev-core/src/db/migrate.rs:485` asserts on the *string*: `assert!(err.to_string().contains("rolled back"))` — it proves the message says it, not that it happened.

**Concrete failure scenario.** SQLite auto-rolls-back and closes the transaction on some errors (e.g. `SQLITE_FULL` with `ON CONFLICT ROLLBACK`), after which an explicit `ROLLBACK` returns *"cannot rollback - no transaction is active"*. Discarded, so harmless there. The dangerous case is the opposite: an I/O error where the `ROLLBACK` itself fails, the transaction is still open, and the caller — told the database was rolled back — carries on using the connection.

**Severity:** `MINOR`
**Class:** #1 `let _ =` on a fallible call; the claim in the message is unverified (#5-adjacent).
**Fix:** capture the result and word the message accordingly (`"…failed; rollback also failed: {e2}"`).

---

### A-16 — `migrate_sqlite_once` is not crash-safe: a process death between the copy and the integrity check permanently adopts a partial file

**What is wrong.** The function writes the destination in place and only then verifies it. On the next run, the `dest.exists()` short-circuit returns `AlreadyPresent` — the integrity check that exists to catch exactly this is never reached for the half-written file.

**Evidence.** `autodev-core/src/db/legacy_root.rs:69-71` (the short-circuit):

```rust
    if dest.exists() {
        return Ok(Migrated::AlreadyPresent);
    }
```

`autodev-core/src/db/legacy_root.rs:84-113` (copy into `dest`, *then* verify):

```rust
    let escaped = dest.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))
    ...
    // Prove the copy is a readable database before reporting success. ...
        let ok: String = check
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        if ok != "ok" {
            let _ = std::fs::remove_file(dest);
```

**Concrete failure scenario.** The user's `~/.airs/autodev.db` session store is large. The app is killed (OOM, power loss, forced restart) part-way through `VACUUM INTO`. A truncated `~/.autodev/autodev.db` is left on disk. On the next launch the guard sees the file exists, reports `AlreadyPresent`, and the truncated file becomes the live session store. The intact original at `~/.airs/autodev.db` is still there — so recovery is possible — but nothing tells the user, and the application will now fail or silently show an empty history. Answering the brief's question directly: the function is idempotent for the *success* path and for `NothingToDo`, and **not** idempotent across an interrupted copy.

Two related notes on the same path: `let _ = std::fs::remove_file(dest)` at `legacy_root.rs:107` means a failed cleanup also leaves a bad file that the next run adopts; and the integrity check uses the default `PRAGMA integrity_check`, which on a large database is slow but correct — that part holds up.

**Severity:** `MAJOR`
**Class:** #5 inert-but-green (a verification step that the failure mode routes around).
**Fix:** `VACUUM INTO` a sibling temp path, run the integrity check on it, then `fs::rename` into place — rename is atomic on both NTFS and POSIX.

---

### A-17 — TG-P1-05's "every connection is tuned" gate walks only `autodev-core/src`, and a live untuned production open sits just outside it

**What is wrong.** The gate that enforces `open_tuned` collects files from one directory. `src-shared` contains a production read-write `Connection::open` on a real user database, reached from the CLI, and the gate cannot see it.

**Evidence.** The gate's scope, `src-tauri/src/ci_gate.rs:700-703`:

```rust
fn tg_p1_05_every_core_connection_is_opened_tuned() {
    let src = repo_root().join("autodev-core/src");
    let mut files = Vec::new();
    collect_rust_files(&src, &mut files);
```

The offender, `src-shared/src/db/analytics.rs:52-56`:

```rust
        let path = autodev_dir()?.join("analytics.db");
        // Analytics DB may not exist yet — open creates it but don't run migrations
        // (the Tauri app is responsible for the schema)
        let conn = Connection::open(&path)?;
```

It is live: `src-cli/src/commands/analytics.rs:3` — `use autodev_shared::db::analytics::AnalyticsReader;` — dispatched from `src-cli/src/main.rs:203`.

A second, narrower hole in the same gate: the detector matches the literal `Connection::open(` only —

`src-tauri/src/ci_gate.rs:684`:

```rust
        if code.contains("Connection::open(") && !code.contains("open_in_memory") {
```

so `Connection::open_with_flags(` is unconditionally exempt. The docstring at `ci_gate.rs:674-675` justifies that as *"the read-only backup path"*, but the exemption is granted by call *shape*, not by the flags actually passed: a future `open_with_flags(p, SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_CREATE)` is invisible to the gate.

**Concrete failure scenario.** A user who only ever runs `airsctl analytics cost` creates `~/.autodev/analytics.db` through this path. `journal_mode` is a persistent file property; created by a plain `Connection::open` it is `delete`, which is precisely the state the gate's own docstring (`ci_gate.rs:694-698`) says P1 fixed. It stays that way until something calls `AnalyticsDb::open` (`autodev-core/src/analytics/db.rs:30`), which for a CLI-only user is never.

The count in the P1 claim checks out for the scope it was measured over: production `open_tuned` call sites in `autodev-core` are exactly 17 (analytics/db, db/annotations, db/config_db, db/plans, db/runs, db/tasks, db/workers, git/checkpoint, graph/store ×5, hooks/db, memory/db, session/storage). `src-shared` is the eighteenth site and was not in scope.

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire (the guard's scope excludes a real violation).
**Fix:** walk `src-shared/src` and `src-cli/src` too; and match `Connection::open` as a prefix, exempting `open_in_memory` and `open_with_flags` only when the same line contains `READ_ONLY`.

---
### A-18 — `is_claim_recent_and_live` still takes a `_live_ids` parameter it never reads

**What is wrong.** The known instance from the defect profile survives P1 unchanged in the moved file. The function name promises a liveness check against the live registry; the argument that would provide it is discarded.

**Evidence.** `autodev-core/src/orphan/mod.rs:131-145`:

```rust
fn is_claim_recent_and_live(
    claim_token: &str,
    _live_ids: &std::collections::HashSet<String>,
) -> bool {
    // Parse the ts suffix off the claim_token.
    let parts: Vec<&str> = claim_token.rsplitn(2, '-').collect();
    let ts_ms: i64 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => return false, // malformed → treat as stranded
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    let age_ms = now_ms.saturating_sub(ts_ms);
    // Anything younger than 90 s might still be mid-flight.
    age_ms < 90_000
}
```

Called with the set at `autodev-core/src/orphan/mod.rs:104-105`:

```rust
        let still_claimed = if slug.starts_with("qc-rev-task") {
            is_claim_recent_and_live(&slug, &live_ids)
```

**Concrete failure scenario.** A synthetic `qc-rev-task<id>-<ts>` claim younger than 90 s is reported "live" even when the subagent registry is empty — including immediately after a process restart, where `reconcile_stranded_qc_claims`'s own docstring (`orphan/mod.rs:68-69`) states *"On startup `SubagentManager::list` returns an empty registry, so every claim from the previous process gets cleared on the first reconciler tick."* For a claim placed less than 90 s before a crash, that documented behaviour does not happen: the claim survives one extra tick. Small in impact, but the docstring and the code disagree.

**Severity:** `MINOR`
**Class:** #2 dead parameters.
**Fix:** either use the set (`age_ms < 90_000 && !live_ids.is_empty()`, or drop the age heuristic when the registry is populated) or remove the parameter and rename the function to `is_claim_recent`.

---

### A-19 — the "multi-machine safety" host guard in `check_task` cannot fire on Linux/macOS

**What is wrong.** `current_hostname()` reads only `COMPUTERNAME`/`HOSTNAME` and falls back to `"unknown"` — the same defect as A-10, in a second copy of the same function. On a platform where neither variable is exported, *every* host compares equal, so the guard that exists to stop one machine killing another machine's in-flight work is a no-op.

**Evidence.** `autodev-core/src/orphan/mod.rs:216-220`:

```rust
fn current_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
```

Used at `autodev-core/src/orphan/mod.rs:156-165`:

```rust
        let current_host = current_hostname();
        let same_host = task
            .claim_host
            .as_deref()
            .map(|h| h.eq_ignore_ascii_case(&current_host))
            .unwrap_or(true); // missing host metadata → treat as local (back-compat)

        if !same_host {
            // Different machine claimed it; rely on TTL only.
        } else if !pid_checker::is_pid_alive(pid) {
            return Some(format!("host process PID {} exited", pid));
```

This is a byte-for-byte duplicate of `autodev-core/src/db/runs.rs:272-277` (A-10) — the same helper written twice, in two modules, neither exported.

**Concrete failure scenario.** Two Linux machines share `~/.autodev` (a networked or synced home). Machine A claims task 7 with pid 4211. Machine B's reconciler resolves `current_host = "unknown"`, `claim_host = "unknown"`, so `same_host` is true; pid 4211 on machine B is either dead (→ *"host process PID 4211 exited"*, task released while it is actively being worked) or alive but a different process (→ *"PID reuse suspected"*, same outcome). The task is released out from under a live worker, which is exactly the failure the comment at `orphan/mod.rs:151-155` says it prevents.

**Severity:** `MINOR` (requires a shared data root, which is not a supported configuration today) — but the guard's stated purpose is unmet on two of three platforms.
**Class:** #3 guards that cannot fire.
**Fix:** one shared `crate::host::name()` using `sysinfo::System::host_name()` with the env vars as a fallback, and delete the duplicate.

---

### A-20 — the CLI's `install_sink` return value is discarded, defeating the reason the API returns it

**What is wrong.** `install_sink` returns `bool` specifically so a host can detect double-initialisation. The Tauri host checks it; the CLI does not, and `bool` is not `#[must_use]`, so nothing warns.

**Evidence.** The contract, `autodev-core/src/events.rs:53-60`:

```rust
/// Install the host's sink. The first call wins; later calls are ignored.
///
/// Returns whether this call installed it, so a host can tell the difference
/// between "I configured the process" and "something already had". Silently
/// returning either way would hide a double-initialisation bug.
pub fn install_sink(sink: Arc<dyn EventSink>) -> bool {
    SINK.set(sink).is_ok()
}
```

Checked correctly at `src-tauri/src/main.rs:206`:

```rust
                if !autodev_lib::events::install_app_handle(app.handle().clone()) {
```

Discarded at `src-cli/src/main.rs:130`:

```rust
    autodev_core::events::install_sink(std::sync::Arc::new(autodev_core::events::NullEventSink));
```

**Concrete failure scenario.** Weak: `airsctl` is a single-shot process and the CLI's sink is the null sink, so a lost `false` costs nothing today. It becomes real the moment the CLI grows a TUI or log-stream sink (which the comment at `src-cli/src/main.rs:127-129` anticipates) — that sink would be silently dropped if anything installed first. Downgraded accordingly.

Related, in the same module: `sink()` allocates a fresh `Arc<NullEventSink>` on every call when no host has installed one — `autodev-core/src/events.rs:67-71` — so a headless process heap-allocates once per emitted event.

**Severity:** `NIT`
**Class:** #7 an error path that loses information; the allocation is #4.
**Fix:** `#[must_use]` on `install_sink`; a `static NULL: LazyLock<Arc<dyn EventSink>>` for the fallback.

**Answering the brief's question about `EventSink` directly:** if `install_sink` is never called, `sink()` returns a `NullEventSink` and every `emit` is a silent no-op — correct and covered by `src-cli/tests/core_runtime.rs:112`. In tests, `SINK` being process-global makes `core_runtime.rs:120`'s `assert!(install_sink(...))` order-dependent: it asserts *no sink was installed before this test ran*, which holds today only because that binary contains exactly one installer. A second test in `src-cli/tests/core_runtime.rs` that installs a sink would make this test fail depending on scheduling.

---

### A-21 — `~/.airs/config.toml` is named in user-facing CLI help and in three docstrings; the code writes `~/.autodev/config.toml`

**What is wrong.** P1-14 moved the data root. `config_path()` was updated; every string describing it was not — including the `--config` flag's help text, which users read.

**Evidence.** The code, `src-shared/src/config.rs:9-16`:

```rust
pub fn config_path() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| SharedError::Other("Cannot determine home directory".into()))?;
    let dir = PathBuf::from(home).join(".autodev");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.toml"))
}
```

The stale descriptions:

```
src-shared/src/config.rs:63:    /// Load from `~/.airs/config.toml`, returning defaults if the file doesn't exist.
src-shared/src/config.rs:74:    /// Persist to `~/.airs/config.toml`.
src-shared/src/config.rs:82:    /// Create a default config file at `~/.airs/config.toml`.
src-cli/src/commands/config.rs:8:    /// Create default config at ~/.airs/config.toml
src-cli/src/main.rs:22:    /// Path to config file (default: ~/.airs/config.toml)
src-cli/src/commands/plugin.rs:540:    // Read ~/.airs/plugins/ for installed plugins
```

`src-cli/src/main.rs:22` and `src-cli/src/commands/config.rs:8` are clap doc-comments, i.e. they are printed by `airsctl --help` and `airsctl config --help`.

**Concrete failure scenario.** A user follows `airsctl --help`, creates `~/.airs/config.toml` by hand, and the CLI ignores it — `load()` reads `~/.autodev/config.toml`, finds nothing, and silently returns `Self::default()` (`src-shared/src/config.rs:66-68`). No error is produced; the user's provider and orchestrator settings are simply not applied. This also re-creates the `~/.airs` directory that `legacy_root.rs:35-38` says must stop being re-created.

**Severity:** `MINOR` (`MAJOR` for the two clap strings if judged as user-facing behaviour)
**Class:** `NOVEL` — a documentation/behaviour mismatch introduced by the P1 data-root move; the house rule treats a docstring that contradicts the code as a bug.
**Fix:** replace the six strings; and note that `legacy_root.rs`'s module header (`autodev-core/src/db/legacy_root.rs:4-7`) states `~/.airs` held "only the session store" — `config.toml` belongs in that inventory even though this machine happens not to have one.

---
### A-22 — `airsctl task claim` defeats core's atomic claim guard by forcing `status = 'assigned'` first, and leaves the task stranded when the claim then fails

**What is wrong.** P1 repointed the CLI's `claim` onto core's guarded `claim_task_with_pid`, whose whole safety property is the atomic `WHERE id = ? AND status = 'assigned'`. To satisfy it, the CLI first *writes* `status = 'assigned'` with a separate, unguarded statement — which both re-opens the race the guard closes and, when the guarded step then fails, leaves the task mutated with no rollback.

**Evidence.** `src-cli/src/commands/task.rs` (post-P1), the claim body:

```rust
    let next = db
        .list(Some("ready"))?
        .into_iter()
        .min_by(|a, b| a.created_at.cmp(&b.created_at));

    match next {
        None => ctx.info("No ready tasks available."),
        Some(t) => {
            db.update_status(t.id, "assigned", Some(&args.worker))?;
            db.claim_task_with_pid(t.id, &args.worker, std::process::id())?;
```

The guard being bypassed, `autodev-core/src/db/tasks.rs:1045-1061`:

```rust
        let rows = self.conn.execute(
            "UPDATE tasks
             SET status = 'in_progress', assigned_worker = ?1,
                 claimed_by_pid = ?2, claim_host = ?3,
                 updated_at = datetime('now')
             WHERE id = ?4 AND status = 'assigned'",
```

And `update_status` — the statement used to force the precondition — is completely unguarded, `autodev-core/src/db/tasks.rs:664-670`:

```rust
        if let Some(worker) = assigned_worker {
            let sql = format!(
                "UPDATE tasks SET status = ?1, assigned_worker = ?2, rejection_reason = ?3, \
                 updated_at = datetime('now'), completed_at = {completed_at_sql} WHERE id = ?4"
            );
```

**Concrete failure scenario 1 — task theft.** Two `airsctl task claim` processes read the ready list at the same instant and both select task 5. A runs `update_status(5,"assigned",A)` then claims it (`in_progress`, `claimed_by_pid = pidA`). B then runs `update_status(5,"assigned",B)` — which unconditionally overwrites both `status` and `assigned_worker`, destroying A's claim — and B's `claim_task_with_pid` now finds `status = 'assigned'` and succeeds. Both CLIs print `Claimed task #5`. `claimed_by_pid` is B's, so orphan detection can never recover A's half.

**Concrete failure scenario 2 — the common one, no race needed.** Worker `w` already holds an `in_progress` task. `airsctl task claim --worker w` selects the oldest ready task, sets it to `assigned`/`w`, and then `claim_task_with_pid` returns the "already owns N in_progress task(s)" error at `tasks.rs:1033-1040`. The `?` propagates and the command fails — leaving the task in `assigned` with `claimed_by_pid = NULL`. `get_orphan_candidates` (`tasks.rs:1080`) does include `assigned`, but `check_task` has no PID to test and falls to the TTL, so the task is parked, invisible to the ready queue, for **four hours** (`autodev-core/src/orphan/mod.rs:13` — `STALE_TTL_HOURS: i64 = 4`).

**Severity:** `MAJOR`
**Class:** #3 guards that cannot fire (the guard is satisfied by fiat rather than met); the missing rollback is `NOVEL`.
**Fix:** add a core method that does the whole `ready → in_progress` transition in one guarded statement (`WHERE id = ? AND status = 'ready'`), or wrap the two CLI statements in a transaction and re-check status inside it.

---

### A-23 — the "one task per worker" guard opens itself when its own count query fails

**What is wrong.** The pre-flight count that enforces the worker-recipe rule quoted in its own error message uses `.unwrap_or(0)`. A failed query is read as "this worker has no active tasks", i.e. the guard passes.

**Evidence.** `autodev-core/src/db/tasks.rs:1024-1040`:

```rust
        let active: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tasks
             WHERE assigned_worker = ?1 AND status = 'in_progress' AND id != ?2",
                params![worker_slug, id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if active > 0 {
            return Err(AutoDevError::Agent(format!(
                "Worker '{worker_slug}' already owns {active} in_progress task(s); \
                 finish or release them before claiming task #{id}. This guard \
                 backs the worker recipe's 'NEVER claim a second task while one \
                 is in_progress' rule."
```

**Concrete failure scenario.** The database is locked past the busy timeout by a concurrent fleet tick — the same contention shape `autodev-core/src/analytics/queries.rs:98-101` documents as *"a race that silently returns (0, 0, 0)"*. `query_row` returns `Err(SqliteFailure(SQLITE_BUSY))`, `.unwrap_or(0)` turns it into `0`, and the worker claims a second task while still working the first. The rule the message quotes is broken precisely when the system is under load, which is the only time it matters.

The function is `-> Result<()>`; `?` is available.

**Severity:** `MAJOR`
**Class:** #7 an error path that loses information, inside a guard (#3).
**Fix:** propagate with `?`, or `.unwrap_or(i64::MAX)` so a query failure closes the guard rather than opening it.

---

### A-24 — `update_status` ignores its row count, so `airsctl task assign` reports success for a task that does not exist

**What is wrong.** `update_status`/`update_status_with_reason` discard the `usize` that `execute` returns. Zero rows updated is indistinguishable from one.

**Evidence.** `autodev-core/src/db/tasks.rs:664-679`:

```rust
        if let Some(worker) = assigned_worker {
            let sql = format!(...);
            self.conn
                .execute(&sql, params![status, worker, rejection_reason, id])?;
        } else {
            let sql = format!(...);
            self.conn
                .execute(&sql, params![status, rejection_reason, id])?;
        }
        Ok(())
```

The caller P1 introduced, `src-cli/src/commands/task.rs`:

```rust
fn assign(args: AssignArgs, ctx: OutputContext) -> Result<()> {
    let db = TasksDb::open()?;
    db.update_status(args.id, "assigned", Some(&args.worker))?;
    ctx.success(&format!(
        "Assigned task #{} to worker '{}'",
        args.id, args.worker
    ));
```

**Concrete failure scenario.** `airsctl task assign 9999 --worker backend` on a nonexistent id prints `Assigned task #9999 to worker 'backend'` and exits 0. A script driving the CLI cannot tell the difference. Contrast `claim_task_with_pid`, which does check (`tasks.rs:1053`), and `finish` in `runs.rs:170`, which also checks — so the pattern is established in the same codebase and simply not applied here.

Note also that `update_status` sets `completed_at = NULL` for every status other than `done` (`tasks.rs:659-663`), so re-assigning a `done` task silently erases its completion timestamp.

**Severity:** `MINOR`
**Class:** #5 inert-but-green.
**Fix:** return the row count (or `Err` on zero) and have the CLI report accordingly.

---

### A-25 — `hostname()` is written three times, in three modules, with the same `"unknown"` bug

**What is wrong.** The same six-line helper exists independently in three places, none exported, all with the defect described in A-10.

**Evidence.**

`autodev-core/src/db/runs.rs:272-277`:

```rust
fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
```

`autodev-core/src/orphan/mod.rs:216-220` — identical, named `current_hostname`.

`autodev-core/src/db/tasks.rs:1042-1044` — inline, inside `claim_task_with_pid`:

```rust
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
```

The third is the *writer* of `claim_host`; the second is its *reader*. They agree today only because they are copies.

**Severity:** `MINOR`
**Class:** #4 (repeated where one would do) with a correctness consequence (A-10, A-19).
**Fix:** one `pub fn host_name()` in core, backed by `sysinfo`, used by all three.

---

### A-26 — TG-P1-06 passes for `tool_executor.rs` while its live path redacts nothing, and no behavioural test covers that boundary

**What is wrong.** The gate that certifies the redaction boundaries is a substring check on the whole file, and the behavioural suite that was written to be the real proof covers only the three memory stores — not the tool executor, which is the first boundary the gate names.

**Evidence.** The gate, `src-tauri/src/ci_gate.rs:780-804`:

```rust
        const BOUNDARIES: &[(&str, &str)] = &[
            (
                "agent/tool_executor.rs",
                "every tool result on its way into the conversation, the messages table and the UI",
            ),
            ...
        ];
        ...
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            if !production.contains("security::redaction") {
```

`tool_executor.rs` contains that string three times — all inside `execute` (lines 117, 218, 219) — so the gate is green while `execute_batch`, the path `engine.rs:314` actually calls, redacts nothing (A-01).

The behavioural suite, `autodev-core/tests/redaction_boundaries.rs`, has exactly four tests:

```
75:  a_secret_in_a_user_message_never_reaches_the_memory_store
107: a_secret_in_an_agent_response_never_reaches_the_memory_store
139: a_secret_in_a_stored_fact_never_reaches_the_memory_store
178: the_memory_store_does_keep_the_content_it_is_given
```

None drives a tool. The gate's own docstring (`ci_gate.rs:766-771`) is candid that the substring check is coarse and defers the real proof to that file — which does not carry it for this boundary.

**Severity:** `MAJOR` (this is the mechanism by which A-01 shipped)
**Class:** #3 guards that cannot fire.
**Fix:** a behavioural test that runs a fake `Tool` returning a secret through `execute_batch` and asserts the secret is absent from the returned `ToolResult`, the messages row, and the emitted event.

---

### A-27 — `NoOrchestrator` has no production consumer, and the CLI it was written for cannot build a `RecipeRunner` at all

**What is wrong.** The host seam is declared and implemented, but the null implementation is constructed only in its own unit tests, and the only `RecipeRunner` construction in the tree is in the desktop app.

**Evidence.** Every `NoOrchestrator` reference:

```
autodev-core/src/recipes/orchestrator.rs:67   pub struct NoOrchestrator;
autodev-core/src/recipes/orchestrator.rs:70   impl OrchestratorProbe for NoOrchestrator
autodev-core/src/recipes/orchestrator.rs:100  (test)
autodev-core/src/recipes/orchestrator.rs:105  (test)
```

Every `RecipeRunner` construction:

```
src-tauri/src/main.rs:302:                let runner = Arc::new(RecipeRunner::new(
```

The docstring at `autodev-core/src/recipes/orchestrator.rs:59-61` describes it as *"The implementation for a host with no orchestrator — `airsctl`, and any desktop build after P8"*. `airsctl` never reaches it.

**Concrete failure scenario.** None today — the seam is inert by design and the module header (`orchestrator.rs:13-18`) says so. The claim that needs correcting is the docstring's, not the code's: `airsctl` does not use `NoOrchestrator`, because `airsctl` does not run the fleet. It is `pub`, so no dead-code warning will ever raise this.

**Severity:** `NIT`
**Class:** #6 a writer with no reader (here, an implementation with no constructor).
**Fix:** reword the docstring to say the CLI has no fleet runner yet, or wire one.

---

### A-28 — `src-cli`'s `DEFAULT_POLLING_INTERVAL` duplicates a schema default with no drift test, unlike `DEFAULT_MODEL` which has one

**What is wrong.** The CLI hard-codes `120` with a comment asserting it matches the schema. It does today. `DEFAULT_MODEL` — the analogous constant — is protected by a test that fails if the schema drifts; `polling_interval` is not.

**Evidence.** `src-cli/src/commands/worker.rs`:

```rust
/// Matches the `polling_interval` DEFAULT in core's workers schema.
const DEFAULT_POLLING_INTERVAL: i64 = 120;
```

The schema, `autodev-core/src/db/workers.rs:187`:

```
                polling_interval  INTEGER NOT NULL DEFAULT 120,
```

The protection that exists for the other constant, `autodev-core/src/db/workers.rs:806-807`:

```rust
            chosen, DEFAULT_MODEL,
            "the schema default and DEFAULT_MODEL have drifted"
```

**Concrete failure scenario.** A later phase changes the schema default to 60. CLI-registered workers keep polling every 120 s while UI-registered ones poll every 60 s, and the comment still claims they match. Nothing fails.

**Severity:** `NIT`
**Class:** #5 inert-but-green (a claim with no check behind it).
**Fix:** export the default from core (`pub const DEFAULT_POLLING_INTERVAL`) and assert it against the schema the way `DEFAULT_MODEL` is.

---

### A-29 — `config.db` is opened, migrated and closed on every single-key read

**What is wrong.** `app_settings::get`/`set` call `open()`, which runs the full `config_db::open_at`: a file open, `busy_timeout`, `PRAGMA journal_mode = WAL` **twice**, `validate_ladder`, a `PRAGMA user_version` read, and a second `user_version` read at the end of `run_ladder`. That is six statements plus a file open to read one row.

**Evidence.** `autodev-core/src/db/app_settings.rs:38-40, 54-56, 73-75`:

```rust
pub fn get(key: &str) -> Result<Option<String>> {
    get_in(&open()?, key)
}
...
pub fn set(key: &str, value: &str) -> Result<()> {
    set_in(&open()?, key, value)
}
...
pub fn load_workspace_dir() -> String {
    get(WORKSPACE_KEY).ok().flatten().unwrap_or_default()
}
```

`autodev-core/src/db/config_db.rs:90-97`:

```rust
pub fn open_at(path: &std::path::Path) -> Result<Connection> {
    let conn = crate::db::open_tuned(path)?;
    // Per-connection, so it is set here rather than in a migration rung.
    conn.execute_batch("PRAGMA journal_mode = WAL;")
        .map_err(|e| AutoDevError::Agent(format!("config.db journal_mode: {e}")))?;
    migrate::run_ladder(&conn, "config.db", LADDER)?;
    Ok(conn)
}
```

`open_tuned` has *already* set it — `autodev-core/src/db/mod.rs:61-62`:

```rust
    conn.pragma_update(None, "journal_mode", "WAL")
```

and the comment justifying the second call is wrong on its own module's terms: `db/mod.rs:50-51` states *"`journal_mode` happens to persist as a property of the file"*, i.e. it is not per-connection. `busy_timeout` is the per-connection one, and it is not what line 93 sets.

**Concrete failure scenario.** `load_workspace_dir()` is called on the UI request path — `src-tauri/src/commands/ide_commands.rs:19` and `src-tauri/src/commands/file_commands.rs:73` — and on every fleet tick (`autodev-core/src/recipes/runner.rs:203`). Each of those pays the full open+migrate cost. This is a count, not a timing claim: no benchmark was run.

Related, on the same lines: `load_workspace_dir`'s `.ok().flatten().unwrap_or_default()` folds "config.db refused to open" into "no workspace configured". `run_ladder` refuses a `config.db` written by a newer build (`migrate.rs:89-94`), so after a downgrade the user's configured workspace silently reverts to the process directory with no message anywhere.

**Severity:** `MINOR`
**Class:** #4 functions called repeatedly where one call would do; the swallowed error is #7.
**Fix:** hold one `config.db` connection (a `OnceLock<Mutex<Connection>>` or a passed handle); delete the duplicate pragma at `config_db.rs:93-94`; log at `warn!` when `load_workspace_dir` swallows an error.

---

### A-30 — the graph ladder still has no "newer database" refusal and no backup on its destructive rung

**What is wrong.** `graph/migrations.rs` was P1-touched (v5 repaired, v6 added) but was not converted to `migrate::run_ladder`, so it keeps neither of the two invariants P1 built that module to enforce.

**Evidence.** `autodev-core/src/graph/migrations.rs:12-49` is a chain of `if current < N` blocks ending in:

```rust
    tracing::info!("[Migrations] All migrations complete");
    Ok(())
```

with no equivalent of `run_ladder`'s refusal at `autodev-core/src/db/migrate.rs:89-94`:

```rust
    if current > target {
        return Err(AutoDevError::Agent(format!(
            "{db_name} is at schema version {current}, but this build only knows up to {target}. \
```

And `migration_v4` drops a table with no backup — `autodev-core/src/graph/migrations.rs:105-107`:

```rust
    tx.execute_batch("DROP TABLE IF EXISTS nodes_fts")?;
    tx.execute_batch(CREATE_NODES_FTS)?;
    tx.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')")?;
```

`is_destructive` would flag `"drop table"` and take a backup — but only if this ladder went through `run_ladder`, which it does not.

**Concrete failure scenario.** A user opens a repo whose `graphs/*.db` was written by a newer build (`user_version = 7`). Every `if current < N` is false, the function logs *"All migrations complete"*, and the store then runs queries against a schema this build does not understand. The v4 backup gap is lower-stakes — graph stores are per-repo caches and rebuildable — but the FTS index is dropped and rebuilt with no snapshot.

Two smaller notes on the same file: `get_schema_version` returns `u32` while `migrate::user_version` returns `i32`, so the two version representations differ; and the six `✅ complete` log lines are now honest, because each `migration_vN(conn)?` propagates.

**Severity:** `MINOR`
**Class:** #3 guards that cannot fire (the guard exists elsewhere and this ladder is outside it).
**Fix:** express the six migrations as a `&[Migration]` and call `run_ladder`.

---
### A-31 — `run_ladder` opens a **deferred** transaction where the rest of the codebase uses `BEGIN IMMEDIATE`

**What is wrong.** `BEGIN` in SQLite is deferred: no lock is taken until the first statement needs one. Two processes can therefore both enter the migration, both start applying DDL, and one is refused the write lock part-way through. `graph/store.rs` uses `BEGIN IMMEDIATE` for exactly this reason, five times.

**Evidence.** `autodev-core/src/db/migrate.rs:127-132`:

```rust
        conn.execute_batch("BEGIN")
            .map_err(|e| AutoDevError::Agent(format!("{db_name} v{}: begin: {e}", m.version)))?;

        let applied = conn
            .execute_batch(m.sql)
            .and_then(|_| conn.execute_batch(&format!("PRAGMA user_version = {}", m.version)));
```

versus `autodev-core/src/graph/store.rs:136,186,229,267,499`:

```rust
        conn.execute_batch("BEGIN IMMEDIATE")?;
```

**Concrete failure scenario.** The desktop app and `airsctl` are started within the same second — a normal thing to do — and both open `tasks.db`. Both `run_ladder` calls read `user_version = 0`, both `BEGIN`, both begin applying the baseline rung. The second to attempt a write gets `SQLITE_BUSY` after the 5 s `busy_timeout` (SQLite does not retry a deferred transaction's lock upgrade for the full timeout in all cases), the migration errors, and the process reports the database as unusable. With `BEGIN IMMEDIATE` the second process would simply wait.

**Answering the brief's question directly:** if the connection already has a transaction open, `execute_batch("BEGIN")` returns *"cannot start a transaction within a transaction"* and `run_ladder` fails loudly at `migrate.rs:128` — no corruption, but the message (`"{db} v{n}: begin: {e}"`) does not name the real cause, and a backup may already have been taken for a rung that never ran. No current caller holds an open transaction, so this is latent.

**Severity:** `MINOR`
**Class:** `NOVEL` — an inconsistency between two transaction idioms in the same crate, where the weaker one is in the newer, more central code.
**Fix:** `BEGIN IMMEDIATE`.

---

### A-32 — the analytics `let _ =` lesson was applied to one call and not to its three siblings in the same files

**What is wrong.** `runner.rs` carries a comment recording that a discarded analytics write "silently swallowed every analytics write for the entire session and was the reason the Dashboard showed zeros for days". The call it annotates was fixed. Three sibling analytics calls in the same two files still discard their results.

**Evidence.** The fixed one, `autodev-core/src/recipes/runner.rs:453-459`:

```rust
                                // Phase 1 of docs/endtoendrun/14may2026/02 — fail
                                // loudly. The previous `let _ = ...` silently
                                // swallowed every analytics write for the entire
                                // session and was the reason the Dashboard
                                // showed zeros for days.
                                if let Err(e) =
                                    crate::analytics::global().collector().record_token_usage(
```

The three that were not:

```
autodev-core/src/recipes/runner.rs:581:    let _ = crate::analytics::global().collector().record_system_event(
autodev-core/src/recipes/runner.rs:766:    let _ = crate::analytics::global().collector().record_worker_stats(
autodev-core/src/recipes/runner.rs:782:    let _ = crate::analytics::global().collector().record_worker_stats(
autodev-core/src/recipes/scheduler.rs:249: let _ = crate::analytics::global().collector().record_system_event(
```

`runner.rs:581` is inside the Mode 2 invoke-error handler — the path that records *why a tick failed*.

**Concrete failure scenario.** The same one the comment describes, for a different table: `analytics.db` is locked by a concurrent write, `record_worker_stats` fails, and the worker-throughput panel shows zeros while the fleet is running. Nothing logs the failure. `record_system_event` for `mode2_invoke_error` failing means the diagnostic record of a failed tick is the first thing lost when the system is under stress.

**Severity:** `MINOR`
**Class:** #1 `let _ =` on a fallible call.
**Fix:** `if let Err(e) = … { warn!(…) }` at all four, matching the pattern already established at `runner.rs:458`.

---

### A-33 — `set_worker_status_ex` discards the status write, then emits an event announcing the transition as the "single source of truth"

**What is wrong.** Same shape as A-12: a discarded write immediately followed by an event that asserts it happened.

**Evidence.** `autodev-core/src/recipes/runner.rs:903-934`:

```rust
        let previous = if let Ok(db) = WorkersDb::open() {
            if let Ok(Some(worker)) = db.get_by_slug(slug) {
                let prev = worker.status.clone();
                let _ = db.update_status(worker.id, status);
                Some(prev)
            } else {
                None
            }
        } else {
            None
        };
        // Phase 3.0 R2 — push the single source-of-truth transition to the UI.
        ...
        emit_worker_status(WorkerStatusPayload {
            slug: slug.to_string(),
            status: status.to_string(),
            previous_status: previous,
```

Three failures are folded into one silent path here: `WorkersDb::open()` failing, `get_by_slug` failing, and `update_status` failing. In the first two the event still fires, with `previous_status: None`.

**Concrete failure scenario.** `workers.db` is write-locked during a fleet tick. The worker's row stays `working`; the UI is told it went `idle`. On the next tick the DB gate at `autodev-core/src/recipes/scheduler.rs:419` (`if w.status == "working" { … return; }`) skips the worker, so a worker the UI shows as idle is permanently skipped by the scheduler until something else rewrites its status.

**Severity:** `MAJOR`
**Class:** #1 `let _ =`; #5 inert-but-green.
**Fix:** propagate or `warn!`, and do not emit a transition whose write failed.

---

### A-34 — the build-guard failure path discards both the note it tells the user to read and the status write that makes QC see the failure

**What is wrong.** Two discarded fallible calls in four lines, on the path whose only job is to make a build failure visible.

**Evidence.** `autodev-core/src/recipes/runner.rs:987-997`:

```rust
        let guard_log = working_dir.join("build_guard.log");
        let note = format!(
            "[AutoDev Build Guard] {:?} dependencies not installed.\nExpected sentinel: {:?}\nRun '{}' and '{}' in this directory, then retry.\n",
            ...
        );
        let _ = fs::write(&guard_log, note);

        let _ = db.update_status(task.id, "build_failed", Some(worker_slug));
```

The function's own docstring, `autodev-core/src/recipes/runner.rs:940-944`:

```rust
    /// If sentinel is missing, update the active in_progress task to
    /// `build_failed` so QC can inspect it.
```

**Concrete failure scenario.** The workspace is read-only or the path does not exist, so `fs::write` fails; and the task-board write fails under lock contention. The task stays `in_progress` with no `build_guard.log` anywhere. QC never sees it, the worker's next tick re-runs the same failing build, and no record of any of it exists.

**Severity:** `MAJOR`
**Class:** #1 `let _ =` on a fallible call; #5 inert-but-green.
**Fix:** `warn!` on both; the status write in particular should be checked, since the docstring makes a promise about it.

---

### A-35 — the `app_settings` single-owner refactor moved the schema but left the key constant and the SQL duplicated, plus two dead imports

**What is wrong.** P1-15 made `db::app_settings` the single owner of the table. `workspace_commands.rs` was repointed for the *connection* but still carries its own copy of the key name and its own read and write SQL, and still imports what it no longer uses.

**Evidence.** `src-tauri/src/commands/workspace_commands.rs:10-14`:

```rust
use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection, OptionalExtension};

const WORKSPACE_KEY: &str = "workspace_dir";
```

`autodev_dir` and `Connection` are unused — clippy reports both (`src-tauri\src\commands\workspace_commands.rs:10:5` and `:12:24`). The key already exists in core, `autodev-core/src/db/app_settings.rs:22-23`:

```rust
/// The workspace-directory setting key.
pub const WORKSPACE_KEY: &str = "workspace_dir";
```

And the accessors exist too (`app_settings.rs:43` `get_in`, `:59` `set_in`), yet `src-tauri/src/commands/workspace_commands.rs:30-37` and `:47-60` re-issue their own SQL, including a `DELETE` branch that core's `set_in` has no equivalent for:

```rust
    if path.is_empty() {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            params![WORKSPACE_KEY],
        )
```

**Concrete failure scenario.** `set_workspace_dir("")` deletes the row; `app_settings::set(WORKSPACE_KEY, "")` would store an empty string. Both read back as `""` through `get`, so today they agree — but two writers with different semantics on one row is the exact condition P1-15 existed to remove, and if the key strings ever drift the two halves of the feature read different rows with no error.

The same commit left three more unused imports flagged by clippy in `src-tauri/src/commands/github_mcp_commands.rs:6` (`crate::db::autodev_dir`) and `:8` (`Connection`).

**Severity:** `MINOR`
**Class:** #4/#11 — a half-completed consolidation.
**Fix:** `use autodev_core::db::app_settings::{self, WORKSPACE_KEY};` and call `get_in`/`set_in`; delete the local const and the dead imports.

---

### A-36 — five Tauri commands in `plan_commands.rs` now take an `app: AppHandle` they never use

**What is wrong.** P1 changed `emit_plan_changed(&app, payload)` to `emit_plan_changed(payload)` — correctly, since events now route through `EventSink`. The now-unused parameters were left in the command signatures.

**Evidence.** Clippy, on `src-tauri/src/commands/plan_commands.rs`:

```
plan_commands.rs:67:37: warning: unused variable: `app`
plan_commands.rs:81:66: warning: unused variable: `app`
plan_commands.rs:110:5:  warning: unused variable: `app`
plan_commands.rs:145:5:  warning: unused variable: `app`
plan_commands.rs:177:5:  warning: unused variable: `app`
```

matching the five call-site rewrites in the P1 diff (`delete_plan`, `break_down_epic`, `claim_plan_item`, `create_subplan`, `submit_plan_for_qc`). `src-tauri/src/commands/agent_commands.rs:100` has the same leftover, `app_clone`.

**Concrete failure scenario.** No runtime failure — Tauri injects `AppHandle`, so the frontend contract is unaffected. The cost is that a reader of `pub fn delete_plan(plan_id: String, app: AppHandle)` believes the command needs a window handle, which is precisely the coupling ADR-LOG-RESTRUCT-0011 was removing.

**Severity:** `NIT`
**Class:** #2 dead parameters.
**Fix:** remove the parameters (or `_app` if Tauri's macro requires the arity).

---

### A-37 — `a_destructive_migration_backs_up_first` mutates process-global `HOME`/`USERPROFILE` with no lock, inside the parallel lib-test binary

**What is wrong.** The integration suite that does this (`autodev-core/tests/redaction_boundaries.rs`) guards it with a mutex and says why. The unit test in `migrate.rs` does the same thing with no guard, in the binary where ~130 other tests run concurrently on multiple threads.

**Evidence.** `autodev-core/src/db/migrate.rs:373-376` and `:392-399`:

```rust
        let prev = std::env::var("USERPROFILE").ok();
        let prev_home = std::env::var("HOME").ok();
        std::env::set_var("USERPROFILE", &dir);
        std::env::set_var("HOME", &dir);
        ...
        match prev {
            Some(v) => std::env::set_var("USERPROFILE", v),
            None => std::env::remove_var("USERPROFILE"),
        }
```

The pattern done properly, `autodev-core/tests/redaction_boundaries.rs:20-22, 34-35`:

```rust
/// `HOME`/`USERPROFILE` are process-global; serialise every test that swaps
/// them rather than relying on `--test-threads=1`, which nobody remembers.
static HOME_LOCK: Mutex<()> = Mutex::new(());
...
fn with_home<T>(dir: &PathBuf, f: impl FnOnce() -> T) -> T {
    let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
```

**Concrete failure scenario.** Any concurrently-running unit test that resolves `autodev_dir()` during the window writes into the migrate test's temp sandbox — or, if it lands just after the restore, into the user's real `~/.autodev`. I searched the lib-test tree and found no such test today (`legacy_root.rs:279` is the only other home-resolving test and it is sandboxed), so this is latent, not live. I verified empirically: `cargo test --workspace --all-targets` passed 138 tests and a before/after listing of `~/.autodev` and `~/.airs` showed **no file added, removed, or modified**.

Two smaller edges in the same test: the sandbox path is fixed (`std::env::temp_dir().join("autodev-migrate-backup-test")`), so two concurrent `cargo test` runs collide; and the env is restored *after* the assertions rather than in a guard, so a failing assertion panics with `HOME` still pointing at the sandbox for the rest of the run.

**Severity:** `MINOR`
**Class:** `NOVEL` — a test-isolation hazard against live user data; the mitigation already exists elsewhere in the same crate and was not applied here.
**Fix:** move `with_home` into a shared test helper and use it in `migrate.rs`, restoring in a `Drop` guard.

---

### A-38 — two more discarded values in `scheduler.rs`: a dead binding and a swallowed `OnceLock::set`

**What is wrong.** `let _ = worker_opt;` binds a `Worker` from the database and throws it away — the `let _` silences the compiler rather than removing the value. And `install_global` discards `OnceLock::set`, so a second installation is invisible; contrast `events::install_sink`, which returns the bool for exactly that reason.

**Evidence.** `autodev-core/src/recipes/scheduler.rs:416-452`:

```rust
        let (worker_opt, recipe_id_opt) = match WorkersDb::open() {
            Ok(db) => match db.get_by_slug(worker_slug) {
                Ok(Some(w)) => {
                    ...
                    let rid = w.recipe_id.clone();
                    (Some(w), rid)
                }
                ...
        };
        let _ = worker_opt;
```

`autodev-core/src/recipes/scheduler.rs:543-545`:

```rust
pub fn install_global(scheduler: Arc<RecipeScheduler>) {
    let _ = GLOBAL_SCHEDULER.set(scheduler);
}
```

The contrasting design, `autodev-core/src/events.rs:58-60`, returns `bool` and `src-tauri/src/main.rs:206` logs when it is `false`.

**Concrete failure scenario.** For `worker_opt`: none — it is dead weight, one full `Worker` row cloned and dropped per kick. For `install_global`: a second `install_global` call silently keeps the first scheduler; `kick()` then dispatches to a scheduler nobody expects, and the warning path at `scheduler.rs:552-555` ("global scheduler not installed") never fires because one *is* installed.

**Severity:** `NIT`
**Class:** #2 dead binding; #1 discarded `Result`-like.
**Fix:** return `(recipe_id_opt)` only; make `install_global` return `bool` and log at the call site.

---

### A-39 — `SemanticStore::extract_and_store` discards every store, so auto-extracted facts can vanish entirely

**What is wrong.** The extraction loop discards the result of each write. The function returns `()`, so the caller cannot tell that nothing was stored.

**Evidence.** `autodev-core/src/memory/semantic.rs:218-229`:

```rust
    pub fn extract_and_store(content: &str, session_id: &str, working_dir: Option<&str>) {
        let extractions = extract_facts(content);
        for (text, category) in extractions {
            let _ = Self::store_fact(
                &text,
                category,
                0.75,
                "extracted",
                Some(session_id),
                working_dir,
                &[],
            );
        }
    }
```

`store_fact` opens the memory database internally, so a failure here means `memory.db` was unavailable — not a per-fact problem but a total one, silently absorbed for every fact in the batch.

**Concrete failure scenario.** `memory.db` is locked or its migration refused (e.g. written by a newer build). Every auto-extracted preference is dropped for the whole session, and the only symptom is that the agent does not remember anything the user said — with no log line anywhere.

The same file has a second instance of the pattern: `autodev-core/src/memory/semantic.rs:200-211` counts active memories with `.ok()` … `.unwrap_or(0)`, so an unreadable database reports "0 semantic memories" rather than an error.

**Severity:** `MINOR`
**Class:** #1 `let _ =`; #7 error path that loses information.
**Fix:** count failures and `warn!` once per batch.

---

### A-40 — backup filenames have one-second resolution, so a second backup of the same database for the same reason within one second refuses and blocks the operation

**What is wrong.** The filename is `{stem}-{timestamp}-{reason}.db` with `timestamp` at second granularity, and an existing destination is a hard refusal — which, at every current call site, means the destructive operation is refused too.

**Evidence.** `autodev-core/src/db/backup.rs:122-128` and `:186-188`:

```rust
    let dest = root.join(format!("{stem}-{timestamp}-{safe_reason}.db"));
    if dest.exists() {
        return Err(AutoDevError::Agent(format!(
            "Backup already exists, refusing to overwrite: {}",
            dest.display()
        )));
    }
...
pub fn timestamp_now() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}
```

The consequence at the call sites — `autodev-core/src/db/migrate.rs:117-122` and `autodev-core/src/db/tasks.rs:733-737` — is a hard stop:

```rust
                        return Err(AutoDevError::Agent(format!(
                            "{db_name}: refusing to apply destructive migration v{} ({}) because no backup could be taken: {e}",
```

**Concrete failure scenario.** The desktop app and `airsctl` start in the same second and both reach the same destructive migration rung on the same database. The reason string is identical (`"{db_name}-v{n}-{name}"`), so the second process's backup name collides, the backup is refused, and the migration is refused — the database is reported unusable to one of the two processes. No shipped ladder has a destructive rung today, so this is latent. `clear_all` has the same shape with the fixed reason `"clear-board"`.

Also note `chrono::Local` here versus `chrono::Utc` everywhere else in the P1 code (`timestamps::now_rfc3339`, `runs.rs`), so backup filenames are in local time while every timestamp column is UTC — comparing them requires knowing that.

**Severity:** `NIT`
**Class:** `NOVEL` — a uniqueness assumption that the failure path turns into a denial of the operation it protects.
**Fix:** add sub-second precision or a short random suffix; on collision, pick the next free name rather than refusing.

---

### A-41 — deleting a session destroys an entire conversation with no backup, on the reasoning that "a single row is not the loss this rule is about"

**What is wrong.** `clear_all` takes a backup and explains that row-level deletes deliberately do not. For `sessions`, one row *is* an entire conversation, and its messages go with it.

**Evidence.** The stated rule, `autodev-core/src/db/tasks.rs:723-726`:

```rust
    /// Row-level deletes deliberately do not do this. Copying the database
    /// before deleting one task would make the board unusable, and a single row
    /// is not the loss this rule is about.
```

The delete it does not cover, `autodev-core/src/session/storage.rs:337`:

```rust
            .execute("DELETE FROM sessions WHERE id=?1", params![session_id])?;
```

**Concrete failure scenario.** A user deletes a session from the UI. Every message in it is gone with no snapshot anywhere — and `~/.airs/autodev.db`, the pre-P1 copy that would have been the accidental fallback, is described in `legacy_root.rs:23-25` as staying only "until a later phase". After that phase there is no copy at all.

I am flagging this as a *boundary question for the main session*, not asserting the rule is wrong: a per-delete database copy is genuinely expensive, and this may be the intended trade.

**Severity:** `MINOR`
**Class:** `NOVEL` — a scope question about the P1 backup rule, raised because the justification given ("a single row") does not describe this case.
**Fix (if accepted):** export the session's rows to JSON before the delete, rather than copying the database.

---
### A-01 — additional evidence (the phase's headline smoke test exercises a method with no production caller)

Found after A-01 was written; it is the mechanism by which A-01 shipped, so it is recorded here rather than folded in silently.

`ToolExecutor::execute` — the only method that redacts — has **no caller anywhere in the tree except one test**. Measured:

```
$ grep -rn "ToolExecutor" --include=*.rs .
./autodev-core/src/agent/engine.rs:16:use super::tool_executor::ToolExecutor;
./autodev-core/src/agent/engine.rs:125:        let tool_executor = ToolExecutor::new(
./autodev-core/src/agent/tool_executor.rs:26:pub struct ToolExecutor {
./autodev-core/src/agent/tool_executor.rs:41:impl ToolExecutor {
./autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:18:use autodev_core::agent::tool_executor::ToolExecutor;
./autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:43:    let mut executor = ToolExecutor::new(
```

`engine.rs` constructs it at `:125` and calls `execute_batch` at `:314`. Nothing calls `execute`.

The S1-04 smoke — described in its own header as *"the phase's headline smoke, run end to end"* — calls the unused method. `autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:61-67`:

```rust
    let result = executor
        .execute(
            read_file,
            serde_json::json!({ "path": env_file.to_string_lossy() }),
        )
        .await
        .expect("read_file should succeed");
```

and asserts, at `:69-74`:

```rust
    // `result.output` is what goes into the conversation the model sees.
    assert!(
        !result.output.contains(SECRET),
```

Its header states the claim that A-01 refutes — `s1_04_secret_never_reaches_the_prompt.rs:10-13`:

```
//! the redactor is actually *on the path* — that a secret read from disk cannot
//! reach the conversation, whatever any individual tool does, because every tool
//! result funnels through one place.
```

`read_file` is read-only, so in production it goes to the concurrent branch of `execute_batch`, which does not redact. The test passes; the property does not hold.

**This raises A-01 from MAJOR to BLOCKER and makes the fix two-part:** redact in `execute_batch`, and re-point the S1-04 smoke at `execute_batch` so it can fail.

---

### A-42 — every `WorkersDb::open()` runs three unconditional `UPDATE`s, so opening the worker registry is always a write — and the in-code diagnosis of the resulting contention names the wrong function

**What is wrong.** `open_at` calls `migrate_model_names`, which issues three `UPDATE` statements with no version gate and no precondition. SQLite takes a write (RESERVED) lock for an `UPDATE` even when it matches zero rows, so *every* open of `workers.db` — 28 call sites — is a writer. A separate comment in the tree blames the contention on `seed_defaults`, which in fact early-returns and writes nothing after first run.

**Evidence.** The open path, `autodev-core/src/db/workers.rs:154-165`:

```rust
    pub fn open_at(db_path: &std::path::Path) -> Result<Self> {
        let conn = crate::db::open_tuned(db_path)
            .map_err(|e| AutoDevError::Agent(format!("workers.db open failed: {e}")))?;
        let db = Self { conn };
        db.add_missing_columns()?;
        migrate::run_ladder(&db.conn, "workers.db", Self::LADDER)?;
        db.migrate_model_names()?; // Phase 7.1 — fix any stale old-format model slugs
        db.seed_defaults()?;
        Ok(db)
    }
```

The unconditional writer, `autodev-core/src/db/workers.rs:276-287`:

```rust
    fn migrate_model_names(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
            UPDATE workers SET model = 'claude-haiku-4-5'  WHERE model = 'claude-4-5-haiku';
            UPDATE workers SET model = 'claude-sonnet-4-5' WHERE model = 'claude-4-5-sonnet';
            UPDATE workers SET model = 'claude-opus-4-5'   WHERE model = 'claude-4-5-opus';
        "#,
            )
```

The non-writer it is blamed on, `autodev-core/src/db/workers.rs:291-298`:

```rust
    fn seed_defaults(&self) -> Result<()> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM workers", [], |r| r.get(0))
            .unwrap_or(0);
        if count > 0 {
            return Ok(());
        }
```

The misdiagnosis, `autodev-core/src/analytics/queries.rs:98-101`:

```rust
        // Workers: open workers.db READ-ONLY to avoid write-lock contention.
        // WorkersDb::open() calls seed_defaults() which needs a write lock.
        // The recipe scheduler holds that lock on every tick, causing a race
        // that silently returns (0, 0, 0). A read-only connection never blocks.
```

Twenty-eight call sites, including the hot ones: `autodev-core/src/recipes/runner.rs:903` (per worker status transition), `autodev-core/src/recipes/scheduler.rs:216, 315, 416` (per tick and per kick), `autodev-core/src/routing/mod.rs:42`, and five in `src-cli/src/commands/worker.rs`.

**Concrete failure scenario.** The scheduler ticks; `kick_worker` opens `workers.db` and takes a RESERVED lock for three no-op UPDATEs. Concurrently `set_worker_status_ex` (`runner.rs:903`) opens it too and its `update_status` is discarded on `SQLITE_BUSY` (A-33), so the UI shows a status the database does not hold. The read-only workaround in `analytics/queries.rs` treats a symptom; the cause is still on every path, and the comment recording it points at the wrong function, so a future reader looking to remove the workaround will look in the wrong place.

**Severity:** `MAJOR`
**Class:** #4 functions called repeatedly where one call would do; the misdiagnosis is #5-adjacent (a recorded conclusion with no check behind it).
**Fix:** make `migrate_model_names` a `v2` ladder rung so it runs once, or guard it with a `SELECT COUNT(*) … WHERE model IN (…)` before writing; and correct the comment in `analytics/queries.rs`.

---

### A-43 — `seed_defaults` decides whether to write seven rows from a count that defaults to zero on failure

**What is wrong.** Same pattern as A-23: `.unwrap_or(0)` on the query that decides whether the table is empty. A failed count is read as "empty", which is the branch that writes.

**Evidence.** `autodev-core/src/db/workers.rs:292-298` (quoted above), then `autodev-core/src/db/workers.rs:380-386`:

```rust
        for (name, slug, wtype, model, interval, system, desc, recipe_id) in seeds {
            self.conn.execute(
                "INSERT INTO workers (name, slug, worker_type, model, polling_interval, is_system_defined, description, recipe_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![name, slug, wtype, model, interval, *system as i64, desc, recipe_id],
            ).map_err(|e| AutoDevError::Agent(format!("seed workers: {e}")))?;
        }
```

**Concrete failure scenario.** `SELECT COUNT(*)` fails under lock contention → `count = 0` → the seed loop runs against a populated table → the first `INSERT` violates the `slug` UNIQUE constraint (`workers.rs:182`) → `WorkersDb::open()` returns `"seed workers: UNIQUE constraint failed: workers.slug"`. So the failure mode is a confusing error rather than data loss — the UNIQUE constraint is doing the work the guard should have done — but the reported cause is wrong, and any future seed row without a unique constraint would silently duplicate.

**Severity:** `MINOR`
**Class:** #7 an error path that loses information, inside a guard.
**Fix:** `?` the count, or `INSERT OR IGNORE`.

---

## 3. Redaction bypass attempts

Every input below was run against the **shipped** `redact_text` / `redact_json`: a harness crate that `include!`s `autodev-core/src/security/redaction.rs` verbatim, so the regexes measured are the ones in the file. "SURVIVED" means the secret string was still present in the output.

### 3.1 `redact_text` — secrets

| # | Input | Result | Output |
|---|---|---|---|
| 1 | `{"api_key": "hunter2sekrit"}` | **SURVIVED** | unchanged |
| 2 | `{"password":"hunter2sekrit"}` | **SURVIVED** | unchanged |
| 3 | `"api_key" : "hunter2sekrit"` | **SURVIVED** | unchanged |
| 4 | `postgres://admin:s3cr3tPassw0rd@localhost:5432/appdb` | **SURVIVED** | unchanged |
| 5 | `postgres://admin:s3cr3tPassw0rd@db.example.com:5432/appdb` | redacted | `postgres://admin:[EMAIL]:5432/appdb` (by the **email** rule, incidentally) |
| 6 | `https://user:s3cr3tPassw0rd@internal/path` | **SURVIVED** | unchanged |
| 7 | `https://user:s3cr3tPassw0rd@example.com/path` | redacted | `https://user:[EMAIL]/path` |
| 8 | `Authorization: Basic dXNlcjpzdXBlcnNlY3JldHBhc3N3b3Jk` | **SURVIVED** | `Authorization=[REDACTED] dXNlcjpzdXBlcnNlY3JldHBhc3N3b3Jk` |
| 9 | `Authorization: Token abcdef0123456789abcdef0123456789` | **SURVIVED** | `Authorization=[REDACTED] abcdef…` |
| 10 | `authorization: token gXs_not_matching_shape_here` | **SURVIVED** | `authorization=[REDACTED] gXs_…` |
| 11 | `X-Api-Key: abcdef0123456789` | redacted | `X-Api-Key=[REDACTED]` |
| 12 | `PASSWORD="my secret passphrase"` | **SURVIVED** | `PASSWORD=[REDACTED] secret passphrase"` |
| 13 | `password=abc,def` | **SURVIVED** | `password=[REDACTED],def` |
| 14 | `Password=abc;def` | **SURVIVED** | `Password=[REDACTED];def` |
| 15 | `api_key: \|\n  linetokenone\n  linetokentwo` (YAML block scalar) | **SURVIVED** | `api_key=[REDACTED]\n  linetokenone\n  linetokentwo` |
| 16 | `# api_key=leakedvalue123` | redacted | `# api_key=[REDACTED]` |
| 17 | `the blob is aGVsbG8gd29ybGQgdGhpcyBpcyBiYXNlNjQ=` | **SURVIVED** | unchanged (documented limit) |
| 18 | `authorization: bearer eyJhbGciOiJIUzI1NiJ9` | redacted | `authorization=[REDACTED] [REDACTED]` |
| 19 | `Bearer abc.def.ghi==` | redacted | `Bearer [REDACTED]` |
| 20 | `Bearer eyJh-bGci_OiJI.UzI1NiJ9.sig` | redacted | `Bearer [REDACTED]` |
| 21 | `https://x/cb?access_token=abcdef0123456789&next=/` | redacted | `…?access_token=[REDACTED]` |
| 22 | `https://hooks.slack.com/services/T00/B00/XXXXsecretXXXX` | **SURVIVED** | unchanged (webhook URL = the credential) |
| 23 | `database:\n  password: hunter2sekrit\n` | redacted | `database:\n  password=[REDACTED]\n` |
| 24 | `password = 'hunter2sekrit'` | redacted | `password=[REDACTED]'` (stray quote) |
| 25 | `export AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMIK7…` | redacted | `export AWS_SECRET_ACCESS_KEY=[REDACTED]` |
| 26 | `sk-abc123` | **SURVIVED** | unchanged (below the 20-char floor; not a real key shape) |
| 27 | `sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789` | redacted | `[REDACTED_KEY]` |
| 28 | `github_pat_11ABCDEFG0abcdefghijkl_ABCDEF…` (fine-grained PAT) | **SURVIVED** | unchanged — the pattern only matches `gh[pousr]_` |
| 29 | `glpat-abcdefghij1234567890` (GitLab PAT) | **SURVIVED** | unchanged |
| 30 | `xoxb-123456789012-1234567890123-abcdefghijklmnopqrstuvwx` (Slack) | **SURVIVED** | unchanged |
| 31 | `-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA1234\n…` | **SURVIVED** | unchanged |
| 32 | `{\"api_key\": \"hunter2sekrit\"}` (escaped-quote JSON in a string) | **SURVIVED** | unchanged |
| 33 | `api_key : hunter2sekrit` | redacted | `api_key=[REDACTED]` |
| 34 | `api_key=\nhunter2sekrit` | redacted | `api_key=[REDACTED]` (value on the next line survives the newline as whitespace) |
| 35 | `curl -H 'Authorization: Bearer abc123def456' https://x` | redacted | `curl -H 'Authorization=[REDACTED] [REDACTED]' …` |
| 36 | `psql --password=hunter2sekrit` | redacted | `psql --password=[REDACTED]` |
| 37 | `mysql -p hunter2sekrit` | **SURVIVED** | unchanged |
| 38 | `SECRET_KEY=ab/cd+ef=gh` | redacted | `SECRET_KEY=[REDACTED]` |
| 39 | `password%3Dhunter2sekrit` (URL-encoded `=`) | **SURVIVED** | unchanged |
| 40 | `<config apiKey="hunter2sekrit"/>` | redacted | `<config apiKey=[REDACTED]"/>` (unbalanced quote left behind) |

**19 of 40 survived.** The four that matter most, because they are shapes an agent meets constantly: raw JSON (#1–#3), `Authorization: Basic`/`Token` (#8–#10), connection strings on a dotless host (#4, #6), and any value containing a space, comma or semicolon (#12–#15).

### 3.2 `redact_text` — over-redaction (false positives)

| Input | Result | Output |
|---|---|---|
| `Author: Razi Ashary <razi@example.com>` | mangled | `Author=[REDACTED] Ashary <[EMAIL]>` |
| `assert passed = true` | mangled | `assert passed=[REDACTED]` |
| `pass_rate: 0.97` | mangled | `pass_rate=[REDACTED]` |
| `prompt_tokens: 1523` | mangled | `prompt_tokens=[REDACTED]` |
| `authorization: role-based access control` | mangled | `authorization=[REDACTED] access control` |
| `Bearer of bad news arrived today` | mangled | `Bearer [REDACTED] bad news arrived today` |
| `let auth = build_auth(cfg);` | mangled | `let auth=[REDACTED];` |
| `src/auth/mod.rs:42` | untouched | correct |

### 3.3 `redact_json`

| Input | Result | Output |
|---|---|---|
| `{"api_key":"hunter2sekrit"}` | redacted | `{"api_key":"[REDACTED]"}` |
| `{"credentials":{"user":"u","password":"p"}}` | redacted | `{"credentials":"[REDACTED]"}` |
| `{"url":"postgres://admin:s3cr3tPassw0rd@localhost/db"}` | **SURVIVED** | unchanged |
| `{"header":"Authorization: Basic dXNlcjpwYXNzd29yZA=="}` | **SURVIVED** | `{"header":"Authorization=[REDACTED] dXNlcjpwYXNzd29yZA=="}` |
| `{"argv":["--password","hunter2sekrit"]}` | **SURVIVED** | unchanged — the secret is a sibling array element, not a value under a key |
| `{"content":"fn main(){let x=1;}"}` | untouched | correct |
| `{"author":"Razi Ashary","msg":"fix"}` | over-redacted | `{"author":"[REDACTED]","msg":"fix"}` |
| `{"passed":true,"failed":false}` | over-redacted, **type changed** | `{"failed":false,"passed":"[REDACTED]"}` |
| `{"pass_rate":0.97}` | over-redacted, **type changed** | `{"pass_rate":"[REDACTED]"}` |
| `{"prompt_tokens":1523}` | over-redacted, **type changed** | `{"prompt_tokens":"[REDACTED]"}` |
| `{"token_count": 42}` | over-redacted, **type changed** | `{"token_count":"[REDACTED]"}` |
| `{"sha":"a1b2c3d4…e7f80912"}` | destroyed | `{"sha":"[REDACTED_KEY]"}` (documented long-token rule) |
| `{"path":"src/verylongdirectoryname_abcdefghijklmnop/x.rs"}` | partially destroyed | `{"path":"src/[REDACTED_KEY]/x.rs"}` |

### 3.4 Idempotency (all stable — this held up)

| Input | After one pass | After two |
|---|---|---|
| `Authorization: Basic dXNlcjpwYXNz` | `Authorization=[REDACTED] dXNlcjpwYXNz` | unchanged |
| `author: Razi` | `author=[REDACTED]` | unchanged |
| `api_key: abc` | `api_key=[REDACTED]` | unchanged |
| `Bearer abc.def` | `Bearer [REDACTED]` | unchanged |
| `Authorization: Bearer abc.def.ghi` | `Authorization=[REDACTED] [REDACTED]` | unchanged |

### 3.5 `is_sensitive_key` substring behaviour

`true` for all of: `author`, `authored_at`, `author_email`, `auth_method`, `authenticity`, `passed`, `passing`, `pass_rate`, `bypass`, `compass`, `surpassed`, `prompt_tokens`, `tokenizer`.

---

## 4. What I could not determine

1. **Whether the redaction gaps have already leaked into the user's live stores.** Reading `~/.autodev/memory.db` or the session store would require opening a SQLite database, which writes (`-shm`, and a WAL open is a write). The constraint forbids it. The main session should check, with a read-only connection and `immutable=1`: `SELECT COUNT(*) FROM episodic_memories WHERE content LIKE '%Authorization: Basic %' OR content LIKE '%://%:%@%'`.

2. **Whether `runs.db` contains rows wrongly marked `abandoned` by A-09.** The file exists (24 KB, last written 2026-09-01 14:25) but I could not query it for the same reason. The check is `SELECT outcome, COUNT(*) FROM runs GROUP BY outcome` — a large `abandoned` count relative to `ok` would confirm A-09 in the field.

3. **Actual latency cost of A-29 and A-42.** I counted statements per open; I did not benchmark. The findings state counts, not times, deliberately.

4. **Whether `PRAGMA user_version` is rolled back with the rest of a failed rung.** `migrate.rs`'s test `a_failing_migration_neither_stamps_nor_half_applies` (`migrate.rs:465-503`) asserts it and passes, so the behaviour is proven for the in-memory case; I did not separately verify it for a file-backed WAL database.

5. **Whether the graph store's `run_migrations` can actually be reached with `user_version > 6` in practice.** That depends on whether any released build ever stamped 7+, which I have no way to check from this tree.

6. **Whether any of the `let _ =` sites I did not read in full (e.g. `src-cli/src/commands/plugin.rs:677,691,735`) are production or cleanup.** `plugin.rs:691` — `let _ = std::fs::write(install_dir.join("manifest.yaml"), &yaml);` — looks like a production write with a discarded result, but plugin install is outside the P1 area and I did not trace it.

7. **The `.autodev` directory's mtime changed during the test run** (16:44:37 → 16:46:19) while no file inside it was added, removed, or modified. Most likely a `create_dir_all` from `autodev_dir()`; I could not attribute it to a specific test.

---

## 5. What held up

Code I attacked that turned out correct. Each of these was a hypothesis I tried to break.

1. **The `~/.airs → ~/.autodev` session-store copy is sound — verified against the files, not the code.** Reading only the 100-byte SQLite headers (no database was opened):

   ```
   ~/.airs/autodev.db      page_size=4096  pages=100  freelist=89  → 45,056 bytes of live content
   ~/.autodev/autodev.db   page_size=4096  pages=11   freelist=0   → 45,056 bytes, user_version=1
   ```

   The destination is exactly the source's live content with the 89 free pages compacted away. `VACUUM INTO` did what the module claims. The 400 KB → 45 KB size drop, which looks alarming, is entirely freelist.

2. **The orphan TTL repair fires.** `crate::db::timestamps::age_hours` parses `datetime('now')`'s offset-less form (`timestamps.rs:68-72`), and the test at `orphan/mod.rs` constructs the fixture by `INSERT` rather than `UPDATE` — with a comment explaining that the `AFTER UPDATE` trigger would otherwise overwrite the backdated `updated_at`. It also asserts the fixture is genuinely unparseable by `parse_from_rfc3339` before testing, so it cannot pass for the wrong reason. This is the strongest test in the phase.

3. **`ensure_columns` genuinely distinguishes "already there" from "the statement was wrong",** and its negative test uses deliberately invalid DDL (`ALTER TABLE t ADD COLUMN x NOT_A_TYPE(;`) rather than a duplicate column, so it proves the distinction rather than assuming it (`migrate.rs:290-299`).

4. **`WorkersDb`'s `last_seen` repair is correctly ordered.** `add_missing_columns` runs *before* `run_ladder` (`workers.rs:158-161`) with a comment explaining that a `v2` rung would fail with *duplicate column name* on databases that already have it. `heartbeat` checks its row count (`workers.rs:234-236`) instead of reporting success for a nonexistent slug. The ADR-0016 defect is properly fixed.

5. **`backup_database_in` fails in the safe direction throughout.** A missing source, a zero-byte result, an unreadable snapshot and a failed `PRAGMA integrity_check` are each a hard error; `std::fs::metadata(...).unwrap_or(0)` is followed by `if bytes == 0 { return Err(...) }`, so even the one `unwrap_or` closes rather than opens. `VACUUM INTO` on a read-only connection is the right primitive and the WAL rationale in the header is correct.

6. **`clear_all` is properly guarded.** The backup is taken first and a failure refuses the clear with a message that names the reason (`tasks.rs:733-737`).

7. **`run_ladder`'s newer-database refusal is real and tested,** including the assertion that no DDL runs when validation fails (`migrate.rs:534-541`).

8. **The graph v5 repair is right.** The six `ADD COLUMN IF NOT EXISTS` statements were removed rather than "fixed", with the reasoning recorded, and `migration_v6` uses `ensure_columns` so it checks before it alters and propagates failure.

9. **The redaction module's stated deviation is honest and holds.** `free_text_does_not_destroy_long_identifiers` and `a_bare_token_in_prose_is_not_caught_by_free_text_redaction` both pass, and `redaction_is_idempotent` holds for every input I threw at it, including the ones that leak. The module header's list of what it does *not* catch is accurate as far as it goes — it simply omits the cases in §3.1.

10. **`DEFAULT_MODEL` is protected against schema drift** by an actual assertion (`workers.rs:806-807`), which is why A-28 (the unprotected `polling_interval` twin) is worth fixing rather than accepting.

11. **The workspace builds and the suite is green.** `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets` both exit 0; `cargo test --workspace --all-targets` passed **138 tests, 0 failed** (130 lib + 3 + 4 + 1 integration). A before/after listing of `~/.autodev` and `~/.airs` showed **no file added, removed, or modified** by the run — the sandboxing in the test suite works.

12. **`NullEventSink` is the correct default and emitting before any host installs one is safe** — `core_runtime.rs:112-125` proves it, and `sink()` never returns `None`.
