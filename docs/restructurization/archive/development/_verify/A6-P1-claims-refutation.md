# A6 — P1 Claims Refutation Audit

**Auditor role:** adversarial claims refutation. Brief: attempt to REFUTE each claim by
measurement. Default to `REFUTED` when evidence is ambiguous.

**Status:** COMPLETE — all eleven claims measured. Written incrementally as each
measurement landed, then ordered 1–11 and closed out.

**Repo:** `C:\AIRS Foundry\autodev\autodev` (workspace: `autodev-core`, `src-tauri`
(pkg `autodev`), `src-cli` (`airsctl`), `src-shared`)

**Date:** 2026-09-01

---

## Headline

**Eleven claims attacked by measurement. Verdicts: 5 `CONFIRMED` (1, 2, 5, 6, 7), 2
`CONFIRMED with an overstated sibling clause` (9, 10), 2 `OVERSTATED` (3, 4), 2 `REFUTED`
(8, 11).** Two of the five that held up — Claim 6, the graph-column retraction this brief
flagged as a likely false retraction, and Claim 7, whose file demonstrably exists on disk today
— survived the hardest attacks in this audit, and the `ADR-LOG-RESTRUCT-0017` retraction is
correct.

**The single most consequential refutation is Claim 11, which takes Claim 8 down with it.**
`ToolExecutor::execute` — described in the MDP and in its own code comment as *"the single
funnel every tool result passes through"*, and the one place P1 wired tool-path redaction —
**has zero production callers.** `grep -rn "ToolExecutor"` over the whole repository returns two
construction sites: `agent/engine.rs:125`, which calls only `execute_batch`, and
`tests/s1_04_secret_never_reaches_the_prompt.rs:43`, which calls only `execute`. The live path,
`ToolExecutor::execute_batch` (`tool_executor.rs:246-491`), contains **no redaction call of any
kind** — every `redact_*` call in that file is at line 117, 218 or 219, all inside `execute`
(94-239) — and it carries raw tool input and raw tool output to all three destinations the
comment names: the conversation (`engine.rs:329-341`), the `messages` table
(`:343-350`, `:469-475`) and the UI events (`:309-328`, `:380-483`). Subagents inherit this,
because `native_runner.rs` runs them on the same `AgentEngine`.

Five behavioural tests pass green against this — four of them never touch a tool at all, and the
fifth drives the dead method. `TG-P1-06` passes too, because it only checks that the file
contains the string `security::redaction`, which `execute` alone satisfies. This is the
programme's own **inert-but-green** class one level up: not a statement that silently fails, but
a *test suite* that is green against a method production does not call.

Claim 8 then follows: of `M-40`'s five named boundaries — prompts, logs, memory writes,
analytics, egress — only **memory writes** is genuinely wired. Prompts are wired on the dead
path; `logging.rs`, `analytics/` and `provider/` contain zero redaction calls between them. And
the logs boundary is not merely unwired — a **164-character `sk-` credential, immediately
following the field name `api_key`, is sitting in `~/.autodev/logs/autodev-.2026-09-01..log`
right now**, written by a `tracing` DEBUG record through `logging.rs`'s file appender, with zero
`[REDACTED]` markers anywhere in that file. It is matched by two of the redactor's own rules, so
it would have been caught had `redact_text` run on that text. That key should be rotated
independently of anything else in this report.

---

## Per-claim verdicts

### Claim 1 — `rusqlite` sets `busy_timeout` to 5000 ms by default on every connection it opens

**As stated** (`autodev-core/src/db/mod.rs:27-38`, doc comment on `BUSY_TIMEOUT_MS`):

> **Measured, not assumed:** SQLite's own default is zero — a busy database
> returns `SQLITE_BUSY` immediately — but `rusqlite` overrides that and sets
> 5000 ms on every connection it opens. Verified by reading
> `PRAGMA busy_timeout` from a plain `Connection::open`, which reports 5000
> with no configuration at all.
>
> So this constant does **not** fix a bug.

**Verdict: `CONFIRMED`.**

**Measurement — runtime.** Throwaway crate in the scratchpad (outside the repo), pinned to
the workspace's exact dependency versions (`rusqlite 0.31.0`, `libsqlite3-sys 0.28.0` —
identical checksums to `Cargo.lock:4884` / `Cargo.lock:3297`):

```
$ cargo run --quiet
=== VERSIONS ===
bundled sqlite_version() = 3.45.0

=== CLAIM 1: rusqlite busy_timeout default ===
open_in_memory                 -> PRAGMA busy_timeout = 5000
Connection::open(path)         -> PRAGMA busy_timeout = 5000
open_with_flags                -> PRAGMA busy_timeout = 5000
open_with_flags(uri mode=ro)   -> PRAGMA busy_timeout = 5000
after explicit `PRAGMA busy_timeout=0` -> 0  (control: pragma reads back what is set)
```

The last line is the guard-the-guard control: `PRAGMA busy_timeout` demonstrably reads
back what is set, so the four `5000` readings are real values and not a constant the
pragma always reports.

**Measurement — source.** `rusqlite-0.31.0/src/inner_connection.rs:118-125`:

```rust
            // attempt to turn on extended results code; don't fail if we can't.
            ffi::sqlite3_extended_result_codes(db, 1);

            let r = ffi::sqlite3_busy_timeout(db, 5000);
            if r != ffi::SQLITE_OK {
                let e = error_from_handle(db, r);
                ffi::sqlite3_close(db);
                return Err(e);
            }
```

**"every connection it opens" holds.** Every public open constructor funnels into
`InnerConnection::open_with_flags`, which is the function above —
`rusqlite-0.31.0/src/lib.rs`:

```
445:    pub fn open<P: AsRef<Path>>(path: P) -> Result<Connection> {
447:        Connection::open_with_flags(path, flags)
456:    pub fn open_in_memory() -> Result<Connection> {
471:    pub fn open_with_flags<P: AsRef<Path>>(path: P, flags: OpenFlags) -> Result<Connection> {
473:        InnerConnection::open_with_flags(&c_path, flags, None).map(|db| Connection {
490:    pub fn open_with_flags_and_vfs<P: AsRef<Path>>(
497:        InnerConnection::open_with_flags(&c_path, flags, Some(&c_vfs)).map(|db| Connection {
512:    pub fn open_in_memory_with_flags(flags: OpenFlags) -> Result<Connection> {
513:        Connection::open_with_flags(":memory:", flags)
527:    pub fn open_in_memory_with_flags_and_vfs(flags: OpenFlags, vfs: &str) -> Result<Connection> {
528:        Connection::open_with_flags_and_vfs(":memory:", flags, vfs)
```

**One narrowing worth recording, not a refutation.** `Connection::from_handle` /
`from_handle_owned` (`src/lib.rs:947`, `:979`) wrap a pre-existing `*mut sqlite3` and do
**not** pass through `open_with_flags`, so such a connection inherits whatever the raw
handle carries (SQLite's own default of 0 if nothing set it). These are `unsafe`
adopt-a-handle constructors, not opens, so the claim's wording ("every connection it
opens") survives. `from_handle` appears nowhere in the workspace, so the code never takes
that path anyway.

**Consequence for the derived claim.** "Setting it explicitly fixes nothing" is therefore
also correct as stated: `open_tuned`'s `conn.busy_timeout(5000)` is a value-pinning no-op,
and the module's own admission that the busy-timeout assertion in
`open_tuned_sets_both_pragmas_that_matter` "would pass even if `open_tuned` set nothing"
(`autodev-core/src/db/mod.rs:96-99`) is an accurate self-assessment, not a hedge.

---

### Claim 2 — chrono `parse_from_rfc3339` accepts a space separator with an offset, rejects any value without one

**As stated** (`autodev-core/src/db/timestamps.rs:12-24`):

```text
  2026-09-01 06:23:58            -> FAIL   (no offset)
  2026-09-01T06:23:58            -> FAIL   (no offset, T separator)
  2026-09-01 06:23:58+00:00      -> OK     (space separator IS accepted)
  2026-09-01T06:23:58+00:00      -> OK
```
> So the separator is **not** the problem — the missing offset is.

**Verdict: `CONFIRMED`** — all four combinations reproduce exactly, at the workspace's
exact chrono version.

**Measurement.** Probe crate pinned to `chrono = { version = "=0.4.44", features = ["serde"] }`
(the workspace resolves `chrono 0.4.44`, `Cargo.lock:884`). Confirmed in the probe's own
lock file: `name = "chrono"` / `version = "0.4.44"`.

```
=== CLAIM 2: chrono DateTime::parse_from_rfc3339 ===
  [T sep + offset] parse_from_rfc3339("2026-09-01T12:34:56+00:00") -> Ok(2026-09-01 12:34:56 +00:00)
  [T sep + Z] parse_from_rfc3339("2026-09-01T12:34:56Z") -> Ok(2026-09-01 12:34:56 +00:00)
  [space sep + offset] parse_from_rfc3339("2026-09-01 12:34:56+00:00") -> Ok(2026-09-01 12:34:56 +00:00)
  [space sep + Z] parse_from_rfc3339("2026-09-01 12:34:56Z") -> Ok(2026-09-01 12:34:56 +00:00)
  [T sep, NO offset] parse_from_rfc3339("2026-09-01T12:34:56") -> Err(premature end of input)
  [space sep, NO offset] parse_from_rfc3339("2026-09-01 12:34:56") -> Err(premature end of input)
  [space + offset no colon] parse_from_rfc3339("2026-09-01 12:34:56+0000") -> Err(input contains invalid characters)
  [T + frac + offset] parse_from_rfc3339("2026-09-01T12:34:56.123456+05:30") -> Ok(2026-09-01 12:34:56.123456 +05:30)
  [space + frac + offset] parse_from_rfc3339("2026-09-01 12:34:56.123456+05:30") -> Ok(2026-09-01 12:34:56.123456 +05:30)
  [date only] parse_from_rfc3339("2026-09-01") -> Err(premature end of input)
  [t lowercase sep + Z] parse_from_rfc3339("2026-09-01t12:34:56Z") -> Ok(2026-09-01 12:34:56 +00:00)
  [underscore sep + Z] parse_from_rfc3339("2026-09-01_12:34:56Z") -> Err(input contains invalid characters)
```

I ran the same probe at `chrono 0.4.45` before pinning; the twelve results were
byte-identical, so the behaviour is not a version accident of 0.4.44.

**Attempts to break it.** I extended the four asserted combinations to twelve, hunting for a
case where the generalisation fails:
- *"rejects any value without an offset"* — `date only` also fails, and both offset-less
  datetime forms fail. No offset-less input parsed. Not refuted.
- *"accepts a space separator"* — I looked for a space case that fails **because of the
  space**: `"2026-09-01 12:34:56+0000"` fails, but the T-separator equivalent fails
  identically (the offset must carry a colon), so the space is still not the cause. Not
  refuted.
- The separator slot is genuinely lax, not merely space-tolerant: lowercase `t` is
  accepted, `_` is not — consistent with chrono treating it as `[Tt ]`.

**Also verified downstream:** `SQLITE_FORMATS` (`autodev-core/src/db/timestamps.rs:41-46`)
covers exactly the four shapes the strict parser rejects, and `parse_db_timestamp` returns
`Err` rather than `None` (`:57-77`), which is the actual fix for the `orphan/mod.rs`
`.ok()?` defect the comment describes.

---

### Claim 3 — `SENSITIVE_KEYS` has 22 entries matching the Python source; a second Python redactor exists and all its patterns were ported

**As stated** (`autodev-core/src/security/redaction.rs:10-17`, `:61-63`):

> Two Python redactors are ported here, not one. […]
>   * `security/redaction.py` — `SENSITIVE_KEYS` (**22** entries, not the 20 the
>     task states) plus `redact_dict` / `redact_string` / `redact_args`.
>   * `logging/centralized_collector.py` — `PIIRedactor`: email, phone, SSN,
>     credit card, `sk-`/`pk-` API keys, bearer tokens, `password=` assignments.

> Ported from `security/redaction.py`. **22 entries.** The plan says 20; they
> were counted, not trusted.

**Verdict:**
- **3a — the 22-entry count and its match to the Python source: `CONFIRMED`, exactly.**
- **3b — a second Python redactor exists at `logging/centralized_collector.py`: `CONFIRMED`.**
- **3c — "all its patterns were ported": `OVERSTATED`.** All seven pattern *categories* are
  present, but three were narrowed so that inputs the Python source redacts are **not**
  redacted by the Rust port, and two of those narrowings are undocumented.

#### 3a — the count

Both sides parsed, not eyeballed. The Python side was read with `ast.literal_eval` so a
duplicate inside the set literal could not inflate the count; the Rust side was extracted
from the `SENSITIVE_KEYS` block by regex and compared as both list and set.

```
$ python -c "<ast.literal_eval of SENSITIVE_KEYS>  +  <regex extract of the Rust block>"
rust len (list, order-preserving): 22
rust set len                     : 22
python set len                   : 22
in python not rust: []
in rust not python: []
EXACT SET MATCH   : True
```

Source, for the record — `submodules/core-orchestrator/src/airs/security/redaction.py:9-19`:

```python
SENSITIVE_KEYS = {
    "api_key", "apikey", "api-key",
    "secret", "secret_key", "secretkey",
    "password", "pass", "pwd",
    "token", "access_token", "refresh_token", "auth_token",
    "authorization", "auth",
    "private_key", "privatekey",
    "client_secret", "clientsecret",
    "bearer",
    "credential", "credentials"
}
```

I attempted two refutations and both failed: the Python literal is a `set`, so a duplicate
would have silently reduced its length below the written entry count — `ast` reports
`type: set / len: 22`, and the Rust list's own set has 22 members, so neither side is
padded. The plan's "20" is indeed wrong; the code's correction is right.

#### 3b — the second redactor exists, and there is no third

`submodules/core-orchestrator/src/airs/logging/centralized_collector.py:167-179` — file
present, 22307 bytes:

```python
class PIIRedactor:
    """Redact PII from log messages."""

    # Patterns to redact
    PATTERNS = {
        'email': (r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b', '[EMAIL]'),
        'phone': (r'\b\d{3}[-.]?\d{3}[-.]?\d{4}\b', '[PHONE]'),
        'ssn': (r'\b\d{3}-\d{2}-\d{4}\b', '[SSN]'),
        'credit_card': (r'\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b', '[CARD]'),
        'api_key': (r'\b(sk-|pk-|api[_-]?key[_-]?)[a-zA-Z0-9]{20,}\b', '[API_KEY]'),
        'bearer_token': (r'Bearer\s+[A-Za-z0-9\-._~+/]+=*', 'Bearer [TOKEN]'),
        'password': (r'(password|pwd|passwd)["\']?\s*[:=]\s*["\']?([^"\'\s]{6,})', r'\1=[REDACTED]'),
    }
```

I also checked whether the "two, not one" count is itself complete. Searching **all**
project Python source (not the vendored `.venv`) for
`SENSITIVE_KEYS|class \w*[Rr]edact|def redact|\[REDACTED\]|PIIRedactor` returns exactly
three files:

```
submodules\core-orchestrator\src\airs\security\redaction.py
submodules\core-orchestrator\src\airs\logging\__init__.py
submodules\core-orchestrator\src\airs\logging\centralized_collector.py
```

`logging/__init__.py` is a re-export only (`13:    PIIRedactor,` / `22:    "PIIRedactor",`).
The same search across `submodules/memory-mcp-server` returns **No files found**. So two is
the right number. (An earlier run of this search over all of `submodules/` was truncated by
a `head -40` and returned only `.venv` third-party hits — a sampling artifact, not a reading;
the scoped search above is the measurement.)

#### 3c — where "all its patterns were ported" breaks

The Rust `TEXT_PATTERNS` were transcribed verbatim from
`autodev-core/src/security/redaction.rs:107-157` into a standalone crate at
`regex = "=1.12.3"` (the workspace's resolved version, `Cargo.lock:4686`), driven by a
`redact_text` reimplementation that is a line-for-line copy of the shipped one (a sequential
`replace_all` over the pattern list, `redaction.rs:166-172`). The Python side was run through
`re` with the `PATTERNS` dict copied verbatim and `re.IGNORECASE`, as `PIIRedactor.__init__`
compiles them.

```
=== CLAIM 3b: PIIRedactor pattern coverage in the Rust port ===
  [REDACTED] PY phone, separators PRESENT : "call [PHONE] now"
  [UNCHANGED] PY phone, separators ABSENT : "call 5551234567 now"
  [REDACTED] PY phone, dots : "call [PHONE] now"
  [REDACTED] PY card, separators PRESENT : "card [CARD] end"
  [UNCHANGED] PY card, separators ABSENT : "card 4111111111111111 end"
  [REDACTED] PY card, spaces : "card [CARD] end"
  [REDACTED]  PY ssn : "ssn [SSN] end"
  [REDACTED] PY email : "mail [EMAIL] end"
  [REDACTED] PY bearer : "Authorization=[REDACTED] [REDACTED] end"
  [REDACTED] PY sk- key : "key [REDACTED_KEY] end"
  [UNCHANGED] PY apikey-prefix form (no sk-/pk-) : "key apikeyabcdefghijklmnopqrstuvwxyz end"
  [UNCHANGED] PY api_key- prefix form : "key api_key_abcdefghijklmnopqrstuvwxyz end"
  [REDACTED] PY password= assignment : "password=[REDACTED] end"
  [REDACTED] PY pwd: assignment : "pwd=[REDACTED] end"
  [REDACTED] PY passwd: assignment (py has 'passwd') : "passwd=[REDACTED] end"
  [REDACTED] github token : "tok [REDACTED_KEY] end"
  [REDACTED] aws key id : "id AKIAIOSFODNN7EXAMPLE end"  -> "[REDACTED_KEY]"
  [UNCHANGED] commit sha (must survive) : "sha 0e5c8f7a9b1d3e2f4a6c8b0d2e4f6a8c0b2d4e6f end"
```

The same six inputs through the **Python** `PIIRedactor`:

```
  [REDACTED ] PY phone, separators ABSENT : 'call [PHONE] now'
  [REDACTED ] PY card, separators ABSENT : 'card [CARD] end'
  [REDACTED ] PY apikey-prefix form : 'key [API_KEY] end'
  [REDACTED ] PY api_key- prefix form : 'key [API_KEY] end'
  [REDACTED ] PY passwd: assignment : 'passwd=[REDACTED] end'
  [UNCHANGED] commit sha (must survive) : 'sha 0e5c8f7a9b1d3e2f4a6c8b0d2e4f6a8c0b2d4e6f end'
```

**Three concrete divergences, Python redacts and Rust does not:**

| Input | Python | Rust |
|---|---|---|
| `call 5551234567 now` | `[PHONE]` | unchanged |
| `card 4111111111111111 end` | `[CARD]` | unchanged |
| `key apikeyabcdefghijklmnopqrstuvwxyz end` | `[API_KEY]` | unchanged |

The cause is visible by putting the two regexes side by side. The `?` that makes the
separator optional in Python is absent in Rust:

| | Python (`centralized_collector.py:173,175,176`) | Rust (`redaction.rs:154,148,116`) |
|---|---|---|
| phone | `\b\d{3}[-.]?\d{3}[-.]?\d{4}\b` | `\b\d{3}[-.]\d{3}[-.]\d{4}\b` |
| card | `\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b` | `\b\d{4}[-\s]\d{4}[-\s]\d{4}[-\s]\d{4}\b` |
| api key | `\b(sk-\|pk-\|api[_-]?key[_-]?)[a-zA-Z0-9]{20,}\b` | `(?i)\b(?:sk\|pk)-[A-Za-z0-9_\-]{20,}` |

**Disclosure status.** The module header documents exactly **one** deviation from the source
— the 32-character blanket rule (`redaction.rs:19-34`) — and describes the second redactor's
api-key rule accurately as "`sk-`/`pk-` API keys", so that narrowing is disclosed in passing.
The **phone and card narrowings are documented nowhere.** Searching the module for
`separator|optional|bare|phone|credit card` returns only the ordering comment at `:151-153`
("Phone last: its shape overlaps the card pattern"), which explains the pattern order, not
the required separator. No test covers a bare-digit phone or card.

**Accurate statement:** "`logging/centralized_collector.py`'s `PIIRedactor` contributes seven
pattern categories, all seven of which are represented here. Three are deliberately narrower
than the source: the api-key rule keeps only the `sk-`/`pk-` prefixes, and the phone and card
rules require an explicit separator, so a bare `5551234567` or `4111111111111111` is not
redacted. The separator requirement avoids redacting ordinary 10- and 16-digit numbers in code
and logs; the cost is that an unformatted phone number or card number passes through."

I attempted to break the count claim as well and could not: `password`, `pwd` **and**
`passwd` all redact through the Rust assignment pattern (`passwd` matches because
`SENSITIVE_KEYS` contains `pass` and the pattern brackets it with `[A-Za-z0-9_\-\.]*`), and
the 40-hex commit SHA survives both implementations, which is the deviation the header
promises.

---

### Claim 4 — `autodev-core` and `airsctl` have zero transitive Tauri/GTK/webkit dependencies; the desktop crate has 28

**Verdict:**
- **The zero: `CONFIRMED`, and it is robust** — it survives every feature and target
  resolution I could construct.
- **The 28: `OVERSTATED`.** It is not a property of the dependency graph. It is a line count
  from one specific `cargo tree` invocation on one specific host platform, and it moves to
  18, 46 or 124 under equally defensible invocations.

**Measurement — the headline numbers reproduce exactly.**

```
$ cargo tree -p autodev-core 2>/dev/null | grep -icE "tauri|gtk|webkit"
0
$ cargo tree -p airsctl 2>/dev/null | grep -icE "tauri|gtk|webkit"
0
$ cargo tree -p autodev 2>/dev/null | grep -icE "tauri|gtk|webkit"
28
```

**The zero holds under every variation I tried.** I went looking for a resolution that would
pull a GUI edge into `autodev-core` or `airsctl` and found none:

```
cargo tree -p autodev-core                          -> 0
cargo tree -p autodev-core --all-features           -> 0
cargo tree -p autodev-core --no-default-features    -> 0
cargo tree -p autodev-core --target all             -> 0
cargo tree -p autodev-core -e all                   -> 0
cargo tree -p autodev-core -e all --target all      -> 0
cargo tree -p airsctl                               -> 0
cargo tree -p airsctl --all-features                -> 0
cargo tree -p airsctl --target all                  -> 0
cargo tree -p airsctl -e all                        -> 0
cargo tree -p airsctl -e all --target all           -> 0
cargo tree -p autodev-shared -e all --target all    -> 0
```

`-e all --target all` is the widest net cargo offers — every edge kind (normal, build, dev,
proc-macro) for every target triple. Zero.

I also widened the *pattern* beyond `tauri|gtk|webkit`, since the phrase "Tauri/GTK/webkit"
could hide a GUI stack under other names. Matching
`^(tauri|gtk|gdk|glib|gio|gobject|javascriptcore|webkit|soup|atk|pango|cairo|wry|tao|muda|objc|cocoa|winit|x11|wayland)`
against the full trees of `autodev-core` and `airsctl` returns **0 unique packages**. The only
platform packages present are the `windows-*` family (`windows-sys`, `windows-targets`,
`windows-registry`, …), which are Win32 API bindings pulled in by `keyring`/`sysinfo`/`git2`,
not a UI toolkit.

**Where "28" does not hold.** The figure counts *matching lines*, and a package appearing at
several points in the tree contributes several lines:

```
cargo tree -p autodev                 | grep -icE "tauri|gtk|webkit"  ->  28   <- the claimed figure
cargo tree -p autodev -e normal       | grep -icE "tauri|gtk|webkit"  ->  18
cargo tree -p autodev --target all    | grep -icE "tauri|gtk|webkit"  ->  46
cargo tree -p autodev -e all          | grep -icE "tauri|gtk|webkit"  ->  124
```

and the number of **distinct packages** is smaller still:

```
$ cargo tree -p autodev 2>/dev/null | grep -oE "[a-z0-9_-]+ v[0-9][^ ]*" | grep -iE "tauri|gtk|webkit" | sort -u | wc -l
12
```

The spread has two independent causes, both of which matter:

1. **Edge kinds.** Default `cargo tree` includes build and dev edges; `-e normal` does not.
   28 vs 18.
2. **Host platform.** This measurement ran on `x86_64-pc-windows-msvc`. GTK and webkit are
   Linux-only dependencies of Tauri and are **filtered out entirely on Windows** — they appear
   only under `--target all`:

```
$ cargo tree -p autodev --target all 2>/dev/null | grep -oE "[a-z0-9_-]+ v[0-9][^ ]*" | grep -iE "tauri|gtk|webkit" | sort -u
gtk v0.18.2
gtk-sys v0.18.2
gtk3-macros v0.18.2
tauri v2.11.1
tauri-build v2.6.1
tauri-codegen v2.6.1
tauri-macros v2.6.1
tauri-plugin v2.6.1
tauri-plugin-dialog v2.7.3
tauri-plugin-fs v2.5.2
tauri-plugin-shell v2.3.5
tauri-runtime v2.11.1
tauri-runtime-wry v2.11.1
tauri-utils v2.9.1
tauri-winres v0.3.6
webkit2gtk v2.0.2
webkit2gtk-sys v2.0.2
```

So on this host the "GTK/webkit" half of the claimed 28 contributes **nothing at all** — the
28 is entirely Tauri lines. The same command on a Linux CI runner returns a different number.

**A caveat on the `--all-features` half of the question.** It is a weaker control than it
looks: **none of the four workspace crates declares a `[features]` section**
(`sed -n '/^\[features\]/,/^\[/p'` over all four `Cargo.toml` files returns nothing), so
`--all-features` on a workspace member is a no-op and cannot change the answer. The load-bearing
controls are `--target all` and `-e all`, both of which I ran and both of which return 0 for
core and cli.

**Accurate statement:** "`autodev-core`, `airsctl` and `autodev-shared` have zero Tauri, GTK
or webkit edges under every edge-kind and target resolution cargo can produce
(`-e all --target all`). The desktop crate has 12 distinct such packages; the figure `28` is
the line count from a default `cargo tree` on a Windows host and is not stable across
platforms or `-e` settings — quote it with its invocation or quote the package count."

**One incidental finding, relevant to the CI gate.** `cargo tree -p autodev-core` emits a
warning on **stderr** whose text contains the path `…\src-tauri\Cargo.toml`:

```
warning: profiles for the non root package will be ignored, specify profiles at the workspace root:
package:   C:\AIRS Foundry\autodev\autodev\src-tauri\Cargo.toml
```

Any gate that captures stderr into the match stream (`cargo tree … 2>&1 | grep -i tauri`) will
score `autodev-core` as **1**, not 0, and fail. The gate at `.github/workflows/ci.yml:81`
pipes stdout only, so it is safe today — but the guard is one `2>&1` away from a false
positive, and my own first capture of these trees hit exactly that.

---

### Claim 5 — all 17 production connection sites in core go through `db::open_tuned`

**As stated** (`MASTER_DEVELOPMENT_PLAN.md:573`, repeated at `TESTING_GATES.md:129`):

> One opener, `db::open_tuned`, used by **all 17 production connection sites** in core;
> `TG-P1-05` fails the build on any direct `Connection::open` […] Read-only
> `open_with_flags` and `open_in_memory` are deliberately exempt.

**Verdict: `CONFIRMED`** — 17 is exact, and no production site in core is missed. **Two
scope caveats** that a reader of the sentence would not expect are recorded below; neither
contradicts the claim as written.

**Measurement — the 17.**

```
$ grep -rn "open_tuned(" autodev-core/src | grep -v "pub fn open_tuned"
autodev-core/src/analytics/db.rs:30
autodev-core/src/db/annotations.rs:64
autodev-core/src/db/config_db.rs:91
autodev-core/src/db/mod.rs:88          <- inside #[cfg(test)]
autodev-core/src/db/plans.rs:218
autodev-core/src/db/runs.rs:111
autodev-core/src/db/tasks.rs:333
autodev-core/src/db/workers.rs:155
autodev-core/src/git/checkpoint.rs:30
autodev-core/src/graph/store.rs:72
autodev-core/src/graph/store.rs:85
autodev-core/src/graph/store.rs:112
autodev-core/src/graph/store.rs:184
autodev-core/src/graph/store.rs:227
autodev-core/src/graph/store.rs:497
autodev-core/src/hooks/db.rs:19
autodev-core/src/memory/db.rs:32
autodev-core/src/session/storage.rs:38
count: 18
```

18 call sites; **exactly one** is in a test module. I did not assume this from the one file
A3 checked — I resolved every file's `#[cfg(test)]` boundary and compared line numbers:

```
analytics/db.rs      : cfg(test) at NONE   (call at :30   -> production)
db/annotations.rs    : cfg(test) at NONE   (:64           -> production)
db/config_db.rs      : cfg(test) at 99     (:91  <  99    -> production)
db/mod.rs            : cfg(test) at 77     (:88  >  77    -> TEST)
db/plans.rs          : cfg(test) at NONE   (:218          -> production)
db/runs.rs           : cfg(test) at 279    (:111 < 279    -> production)
db/tasks.rs          : cfg(test) at 1765   (:333 < 1765   -> production)
db/workers.rs        : cfg(test) at 615    (:155 < 615    -> production)
git/checkpoint.rs    : cfg(test) at NONE   (:30           -> production)
graph/store.rs       : cfg(test) at NONE   (6 calls       -> production)
hooks/db.rs          : cfg(test) at NONE   (:19           -> production)
memory/db.rs         : cfg(test) at NONE   (:32           -> production)
session/storage.rs   : cfg(test) at NONE   (:38           -> production)
```

18 − 1 = **17**.

**Measurement — is any site missed?** I enumerated every way a `rusqlite::Connection` can be
constructed anywhere in core, not only `Connection::open`:

```
$ grep -rnE "Connection::(open|open_with_flags|open_in_memory|open_with_flags_and_vfs|from_handle)" autodev-core/src
```

Every production hit that is **not** `open_tuned` is a read-only open, which the rule exempts.
I checked the flags at each, rather than trusting the file name:

| Site | Flags |
|---|---|
| `autodev-core/src/analytics/queries.rs:106` | `SQLITE_OPEN_READ_ONLY \| SQLITE_OPEN_NO_MUTEX` |
| `autodev-core/src/db/backup.rs:131` | `SQLITE_OPEN_READ_ONLY \| SQLITE_OPEN_URI` |
| `autodev-core/src/db/backup.rs:162` | `SQLITE_OPEN_READ_ONLY` |
| `autodev-core/src/db/legacy_root.rs:78` | `SQLITE_OPEN_READ_ONLY \| SQLITE_OPEN_URI` |
| `autodev-core/src/db/legacy_root.rs:99` | `SQLITE_OPEN_READ_ONLY` |

Every remaining hit is past its file's `#[cfg(test)]` line, or is `open_in_memory`.

**Independent confirmation — I ran the gate rather than only reading it:**

```
$ cargo test -p autodev --lib ci_gate::tests::tg_p1_05 -- --nocapture
running 2 tests
test ci_gate::tests::tg_p1_05_detector_flags_untuned_opens_only ... ok
test ci_gate::tests::tg_p1_05_every_core_connection_is_opened_tuned ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.04s
```

**Caveat 1 — "in core" is load-bearing, and there is a live untuned site outside it.**
`src-shared/src/db/analytics.rs:55` opens the user's real database read-**write** with no
tuning at all:

```rust
    pub fn open() -> Result<Self> {
        let path = autodev_dir()?.join("analytics.db");
        // Analytics DB may not exist yet — open creates it but don't run migrations
        // (the Tauri app is responsible for the schema)
        let conn = Connection::open(&path)?;
```

This is not dead code: `autodev-shared` is a dependency of `src-cli`
(`src-cli/Cargo.toml:13`), and `src-cli/src/commands/analytics.rs:198` calls
`AnalyticsReader::open()`. So `airsctl analytics` opens `~/.autodev/analytics.db` outside the
funnel. The practical impact is small — `journal_mode` persists in the file, so it inherits
the WAL that core set, and `busy_timeout` is 5000 by rusqlite's default (Claim 1) — but it is
a production connection site that the sentence's plain reading would cover and the funnel does
not. **The claim is not refuted** ("in core" excludes it); it is narrower than it sounds.

**Caveat 2 — the gate cannot see either of those things.**
`src-tauri/src/ci_gate.rs:704` sets `let src = repo_root().join("autodev-core/src");`, so
TG-P1-05 walks core only; `src-shared` and `src-cli` are never scanned. And the detector
matches on one string:

```rust
            if code.contains("Connection::open(") && !code.contains("open_in_memory") {
```

A future read-**write** `Connection::open_with_flags(path, SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_CREATE)`
in core would bypass `open_tuned` and the gate would stay green — the exemption is written as
"`open_with_flags` … is the read-only backup path" (`ci_gate.rs:674-676`) but read-only-ness is
nowhere enforced. Every such site is read-only **today**, which I verified above, so this is a
guard hole rather than a live defect.

---

### Claim 6 — the four graph columns were in `CREATE_NODES` since v1, so v5's statements were redundant, not destructive

**As stated** (`autodev-core/src/graph/migrations.rs:125-131`, and `ADR-LOG-RESTRUCT-0017`):

> They were also **redundant**: `CREATE_NODES` already declares
> `agent_author`, `task_id`, `worker_locked` and `worktree`, and
> `CREATE_EDGES` declares `agent_author` and `task_id`, so every database
> created by `migration_v1` has had them all along.

**Verdict: `CONFIRMED`. The retraction in `ADR-LOG-RESTRUCT-0017` is correct.** This was the
claim I most expected to break, and it did not. One clause in the ADR's *justification* for
shipping v6 is **`OVERSTATED`** — see the last part of this section.

#### The git history: the columns were never added later

I did not rely on `--follow`, which is unreliable across renames. I searched for the strings
themselves across every ref (`git log --all -S`), which reports every commit that changed an
occurrence count anywhere in the tree.

```
$ git log --all --oneline -S "CREATE TABLE IF NOT EXISTS nodes"
2789d9f feat(phase 6): Graph Builder
```

**One commit. Ever.** The `nodes` DDL has been written exactly once in this repository's
history and never edited. That single commit already contains all four columns —
`git show 2789d9f -- src-tauri/src/graph/schema.rs`:

```
+    community INTEGER,
+    community_name TEXT,
+    agent_author TEXT,
+    task_id INTEGER,
+    worker_locked BOOLEAN DEFAULT 0,
+    worktree TEXT,
+    extra TEXT,
```

Cross-checks, all consistent:

```
$ git log --follow --oneline -- autodev-core/src/graph/schema.rs
eab345c refactor(core): extract autodev-core and break the four crate cycles
a478543 style: apply cargo fmt across both Rust crates
2789d9f feat(phase 6): Graph Builder

$ git log --all --oneline -S "worktree TEXT"
f71b8b7 feat(db): SQLite backup helper — P1-19, moved first because everything needs it
2789d9f feat(phase 6): Graph Builder
8b91acf docs(phase6): AutoDev Graph breakdown

$ git log --all --oneline -S "fn migration_v1"
2789d9f feat(phase 6): Graph Builder
8b91acf docs(phase6): AutoDev Graph breakdown
```

The two extra commits are not schema edits and I checked each rather than assuming:
- `8b91acf` is **docs-only** (`docs/development/DEVELOPMENT_PLAN.md`,
  `docs/reference/autodev-graph-master-spec.md`); `git grep -l "CREATE TABLE nodes" 8b91acf`
  returns only the spec markdown. It is an ancestor of `2789d9f` (2026-04-27 vs 2026-04-29).
- `f71b8b7`'s `--stat` touches no schema file at all — eight markdown files plus
  `src-tauri/src/db/backup.rs`, `db/mod.rs`, `reachability_gate.rs`. Its hits are the
  migration source **quoted inside** `_verify/V5-P1CD-schema-redaction.md`.

There are five refs total (`main`, `sidecar-backup`, and their remotes), all covered by
`--all`.

#### The decisive point: `CREATE_NODES` and `migration_v5` shipped in the same commit

```
$ git show 2789d9f:src-tauri/src/graph/migrations.rs | grep -n "fn migration_v5"
110:fn migration_v5(conn: &mut Connection) -> Result<()> {

$ git show 2789d9f:src-tauri/src/graph/schema.rs | grep -n "agent_author\|worker_locked\|worktree\|task_id"
57:    agent_author TEXT,
58:    task_id INTEGER,
59:    worker_locked BOOLEAN DEFAULT 0,
60:    worktree TEXT,
78:    agent_author TEXT,
79:    task_id INTEGER,
159:    "CREATE INDEX IF NOT EXISTS idx_nodes_task ON nodes(task_id)",
160:    "CREATE INDEX IF NOT EXISTS idx_nodes_author ON nodes(agent_author)",
```

The whole ladder v1…v5 and the full `CREATE_NODES` entered the tree together. **There has
never been a commit of this repository in which `migration_v5` existed and `CREATE_NODES`
lacked those columns.**

#### "Redundant, not destructive" — reconstructed and run

I rebuilt the original v5 (recovered with `git show 98b5fc1^:autodev-core/src/graph/migrations.rs`)
and ran it against a database built from v1's DDL with a row in it:

```
=== CLAIM 6: original migration_v5 behaviour ===
before: user_version=4, nodes columns = 22
before: node rows = 1
  swallowed: near "EXISTS": syntax error in ALTER TABLE nodes ADD COLUMN IF NOT EXISTS agent_author TEXT DEFAULT NULL at offset 36
  swallowed: near "EXISTS": syntax error in ALTER TABLE nodes ADD COLUMN IF NOT EXISTS task_id INTEGER DEFAULT NULL at offset 36
  swallowed: near "EXISTS": syntax error in ALTER TABLE nodes ADD COLUMN IF NOT EXISTS worker_locked BOOLEAN DEFAULT 0 at offset 36
  swallowed: near "EXISTS": syntax error in ALTER TABLE nodes ADD COLUMN IF NOT EXISTS worktree TEXT DEFAULT NULL at offset 36
  swallowed: near "EXISTS": syntax error in ALTER TABLE edges ADD COLUMN IF NOT EXISTS agent_author TEXT DEFAULT NULL at offset 36
  swallowed: near "EXISTS": syntax error in ALTER TABLE edges ADD COLUMN IF NOT EXISTS task_id INTEGER DEFAULT NULL at offset 36
  statements that failed and were discarded: 6/6
  CREATE INDEX ... ON nodes(agent_author) -> Ok(1)
  CREATE INDEX ... ON nodes(task_id)      -> Ok(1)
  PRAGMA user_version = 5 -> Ok(1)
  tx.commit() -> Ok(())
after : user_version = 5
after : nodes columns = 22 (unchanged: true)
after : node rows = 1 (unchanged: true)
```

Six failures, all discarded; the transaction survives (a syntax error is a *prepare*-time
failure, so nothing is executed and nothing is rolled back); the version is stamped and
committed; **not one column and not one row changed.** Redundant and inert — the "inert-but-green"
defect `ADR-LOG-RESTRUCT-0013` names — but not destructive.

#### The ADR's own corroborating argument, tested

`ADR-LOG-RESTRUCT-0017` argues that v5's two `CREATE INDEX` statements — which use `?`, not
`let _ =` — would have failed had the columns been absent, so their success was evidence
against the original finding. I tested that counterfactual rather than accepting it:

```
=== CLAIM 6 counterfactual: columns absent ===
  CREATE INDEX ... ON nodes(agent_author) on a table WITHOUT the column
    -> Err(SqlInputError { ..., msg: "no such column: agent_author",
         sql: "CREATE INDEX IF NOT EXISTS idx_nodes_author ON nodes(agent_author)", offset: 53 })
```

The argument holds. `IF NOT EXISTS` on the index does not excuse a missing column.

#### Where the ADR is overstated

`ADR-LOG-RESTRUCT-0017` justifies shipping `migration_v6` this way:

> It exists for a database created by a build whose `CREATE_NODES` predated those columns and
> which was then "migrated" by the broken v5 […] Since such a database cannot be ruled out on
> someone else's machine, v6 ships.

**Such a database can in fact be ruled out for any build of this repository**, because
`CREATE_NODES` and `migration_v5` were introduced in the same commit and `CREATE_NODES` has
never been edited. The state v6 defends against is unreachable from any commit on any ref.

**Accurate statement:** "v6 is defensive. No build of this repository can have produced a
graph database missing these columns — `CREATE_NODES` has carried all six since the single
commit that created it, and v5 shipped in that same commit. v6 costs nothing on every database
that can exist, and is kept as insurance against a hand-edited or externally-created graph
database only."

The same doc-comment claim is repeated at `autodev-core/src/graph/migrations.rs:160-163` and
in the test `a_v5_database_that_never_got_the_columns_is_repaired_by_v6` (`:254-259`), whose
docstring calls its fixture "the database v6 exists for". The test is a good test of v6's
behaviour; its *narrative* describes a database this repository has never been able to create.

---

### Claim 7 — `plans.db` never existed on disk, so the two incompatible plan designs carried no data risk

**As stated** (`MASTER_DEVELOPMENT_PLAN.md:559`, `:564`; `READINESS_MATRIX.md:90`):

> `plans.db` did not exist on disk at all, so the *"two incompatible designs"* carried
> **no data risk** […] `plans.db` was created fresh — it had never existed.

**Verdict: `CONFIRMED`.** The file did not exist before P1, it exists now because P1's own
verification created it, and it holds **zero rows** — so the no-data-risk conclusion is right
on both the historical and the present reading. One P1 document's **present-tense** phrasing
is now stale.

**Measurement 1 — it exists today.** `ls ~/.autodev/`:

```
-rw-r--r-- 1 Razi Ashary 197121  73728 Sep  1 13:50 /c/Users/Razi Ashary/.autodev/plans.db
```

**Measurement 2 — it is empty.** I copied the file into the scratchpad and opened the **copy**
read-only; the original was never opened, so nothing could migrate it.

```
=== CLAIM 7: ~/.autodev/plans.db (working on a byte copy) ===
copy size = 73728 bytes
user_version    = 1
journal_mode    = wal
integrity_check = ok
sqlite_master objects (17):
  table    plan_executions
  table    plan_item_dependencies
  table    plan_items
  table    plans
row counts:
  plan_executions            0
  plan_item_dependencies     0
  plan_items                 0
  plans                      0
TOTAL ROWS ACROSS ALL TABLES = 0
```

The schema on disk is core's UUID-keyed design (`id TEXT PRIMARY KEY` on `plans`,
`plan_items`, `plan_executions`), i.e. the design the MDP says "simply wins" — not the CLI's
flat two-table model.

**Measurement 3 — it genuinely did not exist before P1.** The pre-P1 full snapshot at
`~/autodev-backups/p1-20260901-113442/dot-autodev/`, taken 2026-09-01 11:34 before any P1
database work, lists every file in `~/.autodev` at that moment:

```
analytics.db  annotations.db  build.log  config.db  memory.db  package-lock.json
mcp.key  runtime_config.json  tasks.db  workers.db
agents/ checkpoints/ docs/ graph_cache/ graphs/ logs/ recipes/ skills/ tests/
```

**No `plans.db`. No `runs.db`.** Both appear only afterwards. `plans.db`'s mtime of `13:50`
is the same minute as `~/autodev-backups/p1-10-preflight/`, which is when
`examples/migrate_check` ran — exactly the origin the MDP records. No `plans.db` exists in any
backup directory (`find ~/autodev-backups ~/.autodev/backups -iname "*plan*"` returns nothing),
so there is no evidence of an earlier one having been displaced.

**The staleness.** `_verify/V5-P1CD-schema-redaction.md:48` and `:305` state
`~/.autodev/plans.db` **DOES NOT EXIST** / *file does not exist* in the present tense. That was
true when written and is false now. Both are point-in-time verification reports, so this is a
note rather than a defect; the MDP and READINESS_MATRIX both use the past tense ("did not
exist", "had never existed") and remain correct.

---

### Claim 8 — redaction is applied at every boundary the plan names: prompts, logs, memory writes, analytics, egress

**As stated.** The plan's `M-40` (`00-AUTODEV-RESTRUCTURE-MASTER-PLAN.md:207`) names five
boundaries and four destinations:

> apply at every boundary where content leaves the process or enters durable storage:
> prompts, logs, memory writes, analytics, and any egress
> […] `Where`: new `security/`, `memory/`, `logging.rs`, `provider/`

and `src-tauri/src/ci_gate.rs:778` asserts the gate covers them:

> // Each entry is a boundary named by `P1-21`: prompts, memory writes, logs, analytics, egress.

**Verdict: `REFUTED`.** Of the five named boundaries, **one** is wired (memory writes). Prompts
are wired on a dead code path (Claim 11). Logs, analytics and egress have no redaction at all.
The gate comment quoted above names five boundaries over a list containing four entries, none
of which is a log, an analytics or an egress site.

#### Boundary-by-boundary

The complete set of redaction call sites in the workspace, excluding the module itself:

```
$ grep -rn "redact_text\|redact_json\|redact_args\|redact_value_text\|security::redaction\|is_sensitive_key" --include=*.rs autodev-core/src src-tauri/src src-cli/src src-shared/src
autodev-core/src/agent/tool_executor.rs:117   redact_args
autodev-core/src/agent/tool_executor.rs:218   redact_text
autodev-core/src/agent/tool_executor.rs:219   redact_args
autodev-core/src/memory/episodic.rs:32,33,77,82,84,85,127
autodev-core/src/memory/procedural.rs:129
autodev-core/src/memory/semantic.rs:41
src-tauri/src/ci_gate.rs:767,802               (the gate's own string check)
```

Nine production call sites, in **two** modules.

| Boundary (plan's word) | Named destination | Wired? | Evidence |
|---|---|---|---|
| **memory writes** | `memory/` | **YES** | `episodic.rs` ×7, `semantic.rs:41`, `procedural.rs:129`; behavioural test `tests/redaction_boundaries.rs` |
| **prompts** | — | **NO (dead path)** | only `tool_executor.rs::execute`, which has zero production callers — Claim 11 |
| **logs** | `logging.rs` | **NO** | `grep -c "redact" src-tauri/src/logging.rs` → **0** |
| **analytics** | — | **NO** | `grep -rn "redact" autodev-core/src/analytics/` → no matches |
| **egress** | `provider/` | **NO** | `grep -rn "redact" autodev-core/src/provider/` → no matches |

I checked the two remaining plausible homes as well, in case the wiring had moved:
`grep -rn "redact" autodev-core/src/observability/` → no matches;
`grep -rn "redact" autodev-core/src/session/` → no matches.

#### Each negative reading is a real reading, not a missing signal

**Logs.** `src-tauri/src/logging.rs` initialises `tracing_subscriber` with a console layer and
a daily rolling file appender writing to `~/.autodev/logs/autodev.YYYY-MM-DD` (`:1-13`,
`:33-45`). There is no redacting layer and no `redact` anywhere in the file (count: 0). Every
`tracing::` call in the tree writes through it verbatim. `tool_executor.rs` itself logs tool
names and errors here.

**Analytics.** `autodev-core/src/analytics/db.rs:52-133` defines the whole schema. Three
columns take free text that a caller supplies:

```sql
            CREATE TABLE IF NOT EXISTS api_failures (
                …
                error_message TEXT NOT NULL,
            CREATE TABLE IF NOT EXISTS system_events (
                …
                message TEXT NOT NULL,
                metadata TEXT
```

`collector.rs:62` `record_api_failure` and `:84` `record_system_event` write them; live callers
include `recipes/runner.rs:581`, `recipes/scheduler.rs:249` and
`agent/builtin_tools/spawn_qc_review.rs:338`. A provider error string is exactly the kind of
value that carries a URL with an embedded token, and nothing on this path redacts.

**Egress.** `autodev-core/src/provider/` (`anthropic.rs`, `openai.rs`, `google.rs`,
`ollama.rs`, `streaming.rs`) contains no redaction. This is the one boundary where I think the
plan's wording is partly wrong rather than the code: the model *must* receive the prompt to be
useful, so "redact at egress to the model" is not straightforwardly implementable. But the
plan named `provider/` as a destination and nothing was done there or recorded as a deviation.

**The session store is a sixth, unnamed durable-storage boundary and is also unwired.**
`autodev-core/src/session/` has no redaction, so the `messages` table in `~/.airs/autodev.db`
receives user text and assistant text verbatim. The MDP's line that the tool executor "covers
… the `messages` table" is true only for tool messages, and — per Claim 11 — not even for those.

#### Live evidence: the logs boundary is not merely unwired, it has already leaked

I ran a **count-and-shape-only** scan (no matched value is printed here or was recorded
anywhere) over the user's `~/.autodev/logs/` and over read-only copies of the session and
analytics stores, looking for the shapes the redactor itself matches.

Session store `~/.airs/autodev.db` (tables `sessions`, `messages`, 13,931 chars of text) and
analytics store `~/.autodev/analytics.db` (tables `token_usage`, `daily_costs`, `worker_stats`,
`api_failures`, `system_events`, 54,729 chars) — **0 matches on every pattern**, and **0
`[REDACTED]` markers**. That is consistent with the wiring analysis but proves nothing on its
own: no secret appears to have passed through them yet, so a missing marker is not a reading.

The logs are different:

```
--- autodev-.2026-08-31..log (2539 chars) ---     all patterns: 0
--- autodev-.2026-09-01..log (58867 chars) ---
    [REDACTED] marker present    matches: 0
    sk-/pk- style key            matches: 8
    ...all other patterns:       0
--- core-orchestrator.log (704 chars) ---         all patterns: 0
--- memory-mcp.log (10290 chars) ---
    [REDACTED] marker present    matches: 0
    sk-/pk- style key            matches: 8
```

Shape of the matches, with the value never printed:

```
--- autodev-.2026-09-01..log: 8 matches ---
    prefix='sk-' length=164 sha256[:8]=70c64d8c occurrences=8
    nearest preceding identifier(s): ['api_key']
--- memory-mcp.log: 8 matches ---
    prefix='sk-' length=164 sha256[:8]=70c64d8c occurrences=8
    nearest preceding identifier(s): ['api_key']
```

A 164-character `sk-` token, the same value in both files (identical digest), sitting
immediately after the field name `api_key`. It is matched by **two** of the redactor's own
rules — the `sk-`/`pk-` provider-key pattern and the sensitive-key assignment pattern — so if
`redact_text` had run on this text it would have been replaced.

Which writer produced it (level and tracing target only, message withheld):

```
--- autodev-.2026-09-01..log: 4 offending lines ---
    ('DEBUG', 'autodev_lib::sidecar:')
--- memory-mcp.log: 4 offending lines ---
    ('(no tracing header)', 'INFO:src.memory_pr')
```

The AutoDev-side line is a `tracing` DEBUG record from `autodev_lib::sidecar`, written through
the rolling file appender that `src-tauri/src/logging.rs` installs — i.e. through the exact
boundary `M-40` named as `logging.rs` and that has zero redaction. **This is not a
hypothetical: a live credential is on disk in the user's log directory today, unredacted.**

Two qualifications, so this is not overstated:
- The producing module (`sidecar`) is scheduled for deletion in P8, so this particular
  producer will go away. The boundary will not: any `tracing` call that formats a payload
  writes verbatim.
- `memory-mcp.log` is written by the Python sidecar's own logger, not by `logging.rs`, so it
  is outside the Rust boundary — but it is the same secret in the same directory.

**This should be triaged separately from the audit finding:** the key is live on disk and
should be rotated, and `~/.autodev/logs/` reviewed, independently of whether redaction is
wired.

#### What P1 actually claimed vs. what the gate comment claims

`MASTER_DEVELOPMENT_PLAN.md:619` (`P1-21`) is narrower and more honest than the gate comment:
it says redaction was wired *"at `agent/tool_executor.rs::execute` … and at all four memory
write paths"* — two places, which matches the code. The task title, *"Apply at every
boundary"*, and the gate comment at `ci_gate.rs:778` are what overstate it.

**Accurate statement:** "Redaction is wired at the memory write paths (`episodic`, `semantic`,
`procedural`) and at `ToolExecutor::execute`. Of `M-40`'s five named boundaries only memory
writes are actually covered: the tool/prompt path is on a method the engine does not call
(Claim 11), and logs (`logging.rs`), analytics (`analytics/`) and egress (`provider/`) have no
redaction at all. `M-40`'s remaining destinations — `logging.rs` and `provider/` — are
unstarted."

---

### Claim 9 — `ADD COLUMN IF NOT EXISTS` is not valid SQLite, so `PRAGMA table_info` is the mechanism

**As stated** (`autodev-core/src/db/migrate.rs:165-167`):

> `ADD COLUMN IF NOT EXISTS` is not valid SQLite, so this check is the
> mechanism, not a convenience.

**Verdict: `CONFIRMED`** on the syntax fact. The sentence immediately above it, which the
claim leans on, is **`OVERSTATED`**.

**Measurement** — against the exact bundled SQLite this workspace links
(`libsqlite3-sys 0.28.0`, `Cargo.lock:3295-3297`; probe reports `sqlite_version() = 3.45.0`):

```
=== CLAIM 9: ADD COLUMN IF NOT EXISTS ===
ALTER TABLE t ADD COLUMN IF NOT EXISTS b TEXT;  -> Err: near "EXISTS": syntax error in ALTER TABLE t ADD COLUMN IF NOT EXISTS b TEXT; at offset 32
ALTER TABLE t ADD COLUMN b TEXT;               -> Ok(()) [control: plain form works]
duplicate plain ADD COLUMN                     -> Err: duplicate column name: b
```

Line 2 is the control: the plain form is accepted on the same connection and the same table,
so line 1's failure is the `IF NOT EXISTS` clause and not a broken statement or table.

**Where it is overstated.** The preceding sentence (`migrate.rs:159-162`) justifies the
pragma check on the grounds that the alternative idiom

> cannot tell "already there" from "the statement was wrong".

Line 3 of the measurement shows SQLite *does* distinguish these: a duplicate returns the
specific, matchable `duplicate column name: b`, whereas a malformed statement returns a
syntax error. What genuinely cannot distinguish them is the *discarding* form the codebase
actually had — `let _ = conn.execute_batch(...)` — which throws the error away.

**Accurate statement:** "`ADD COLUMN IF NOT EXISTS` is not valid SQLite, and the
`let _ = ALTER …` idiom this replaces discarded the error entirely, so it could not tell
'already there' from 'the statement was wrong'. Matching on the `duplicate column name`
error would also work; the pragma check is preferred because it does not depend on an error
string." The design conclusion is unchanged — only the reason given is stronger than the
facts support.

---

### Claim 10 — a failed backup stops the destructive operation, in `run_ladder` and `TasksDb::clear_all`

**As stated** (`autodev-core/src/db/migrate.rs:100-103`; `autodev-core/src/db/backup.rs:3`):

> A backup that cannot be taken **stops the migration**. Proceeding with a
> destructive change on the belief that a backup happened is the exact failure
> this is here to prevent.

> Every destructive database operation goes through this first (GAP-11, §0.8).

**Verdict:**
- **10a — "a failed backup stops the operation": `CONFIRMED`.** I found no route through
  either call site where `backup_before` returns `Err` and the destructive work still runs.
- **10b — "backup succeeds but the file is empty or corrupt": `CONFIRMED`.** All three
  failure modes are caught, and a corrupt *source* is caught earlier than I expected.
- **10c — "Every destructive database operation goes through this first": `REFUTED`
  as a general statement.** `is_destructive` misses five destructive SQL forms, including
  `ALTER TABLE … DROP COLUMN`, which this build's SQLite accepts. For those, **no backup is
  attempted at all** — so the guard is not reached rather than failed. Latent, not live: no
  migration in the tree uses those forms today.
- **A guard-the-guard gap:** the refusal branch is **untested** in both call sites.

#### 10a — tracing every route

`run_ladder` (`autodev-core/src/db/migrate.rs:105-124`):

```rust
        if is_destructive(m.sql) {
            if let Some(path) = conn
                .path()
                .filter(|p| !p.is_empty())
                .map(std::path::PathBuf::from)
            {
                let reason = format!("{db_name}-v{}-{}", m.version, m.name);
                match crate::db::backup::backup_before(&path, &reason) {
                    Ok(b) => tracing::info!("[migrate] {}", b.describe()),
                    Err(e) => {
                        return Err(AutoDevError::Agent(format!(
                            "{db_name}: refusing to apply destructive migration v{} ({}) …
```

`TasksDb::clear_all` (`autodev-core/src/db/tasks.rs:726-748`):

```rust
    pub fn clear_all(&self) -> Result<()> {
        if let Some(path) = self.conn.path().filter(|p| !p.is_empty()).map(PathBuf::from) {
            let backup = crate::db::backup::backup_before(&path, "clear-board").map_err(|e| {
                AutoDevError::Agent(format!(
                    "refusing to clear the task board because no backup could be taken: {e}"
                ))
            })?;
```

Both are unconditional early returns on `Err`; the backup precedes `BEGIN` / the `DELETE`
batch in both cases. There is no fallthrough, no `let _ =`, no `.ok()`, no logging-and-continue.

The one branch that *does* proceed without a backup is the pathless one. I measured what
`conn.path()` actually returns, rather than trusting that a file database always yields a
path:

```
=== CLAIM 10c: what conn.path() returns (the gate both call sites use) ===
  open_in_memory()            -> path() = Some("")
  open(file)                  -> path() = Some("C:\\...\\probe-path\\f.db")
  same conn after WAL         -> path() = Some("C:\\...\\probe-path\\f.db")
  open("")  [temp on-disk db] -> path() = Some("")
  uri mode=memory&cache=shared-> path() = Some("")
```

Every case that yields `Some("")` is a database with no user data on disk, and every
file-backed case yields the real path — including after the WAL switch `open_tuned` performs.
The `.filter(|p| !p.is_empty())` is the right discriminator. **Not refuted.**

#### 10b — an empty or corrupt backup

`backup_database_in` (`autodev-core/src/db/backup.rs`) checks, in order:

| Check | Line | Behaviour |
|---|---|---|
| source does not exist | `:97-102` | `Err` — "Refusing to back up a database that does not exist" |
| destination already exists | `:123-128` | `Err` — never overwrites |
| `VACUUM INTO` fails | `:142-150` | `Err`, named |
| backup file is **0 bytes** | `:152-158` | `Err` — "produced an empty file — treating as failure" |
| backup is not openable | `:162-163` | `Err` — "Backup is not readable" |
| `PRAGMA integrity_check != "ok"` | `:164-172` | `Err`, with the check's own message |

I tried to construct the failure the question asks about — a *successful-looking* backup of a
damaged source — by corrupting a page in a seeded 500-row database and running
`backup_before`'s exact sequence:

```
=== CLAIM 10b: does a corrupt source produce a 'successful' backup? ===
  source rows = 500, size = 16384
  corrupted 600 bytes at offset 8292
  backup_before-equivalent -> Err: VACUUM INTO failed: database disk image is malformed  (the corruption was caught)
```

`VACUUM INTO` reads every page, so source corruption fails the backup before the size and
integrity checks are even reached. I could not produce a backup that reported success while
holding less than the source.

**One real limitation, stated plainly:** nothing compares *content* between source and backup
— no row count, no checksum. A backup of a source that is valid but has already lost rows
would pass every check. That is not a defect of the helper (its contract is "a consistent
snapshot of what is there"), but "the backup is good" means "structurally sound", not "holds
what you expect".

#### 10c — the destructive-set classifier misses five forms

`is_destructive` (`autodev-core/src/db/migrate.rs:56-70`) is a lowercase substring match over
eight needles. I copied it verbatim and ran it against SQL forms that destroy data:

```
=== CLAIM 10a: is_destructive() coverage ===
  is_destructive(DROP TABLE              ) = true  (actually destructive: true)
  is_destructive(DELETE FROM             ) = true  (actually destructive: true)
  is_destructive(RENAME TO               ) = true  (actually destructive: true)
  is_destructive(RENAME COLUMN           ) = true  (actually destructive: true)
  is_destructive(DROP COLUMN             ) = false (actually destructive: true)  <== HOLE: destructive but NOT flagged, no backup taken
  is_destructive(UPDATE SET              ) = false (actually destructive: true)  <== HOLE
  is_destructive(INSERT OR REPLACE       ) = false (actually destructive: true)  <== HOLE
  is_destructive(DROP  TABLE (2 spaces)  ) = false (actually destructive: true)  <== HOLE
  is_destructive(DROP\nTABLE (newline)   ) = false (actually destructive: true)  <== HOLE
  is_destructive(ADD COLUMN (benign)     ) = false (actually destructive: false)
  is_destructive(CREATE TABLE (benign)   ) = false (actually destructive: false)
```

`DROP COLUMN` is not a hypothetical form. It is accepted by the SQLite this build links, and
it destroys the column's data:

```
=== CLAIM 10a-proof: DROP COLUMN really does destroy data on this SQLite ===
  before DROP COLUMN: rows with a secret = 2
  ALTER TABLE t DROP COLUMN secret; -> Ok(()) [ACCEPTED by SQLite 3.45.0]
  after : columns = ["id"]  (the data in `secret` is gone)
```

The whitespace holes matter because the needles are literal: a rung written as
`DROP\n TABLE nodes_fts` — which is ordinary formatting inside a multi-line `sql:` literal —
is not flagged.

**Live impact today: none.** Every `LADDER` in core is a single baseline rung
(`analytics/db.rs:52`, `db/annotations.rs:73`, `db/config_db.rs:26`, `db/plans.rs:226`,
`db/runs.rs:81`, `db/tasks.rs:352`, `db/workers.rs:175`, `memory/db.rs:40`,
`session/storage.rs:57`), and a search for `drop column|insert or replace|drop  table|truncate`
across all four crates' Rust returns no SQL — only prose and identifiers about output
truncation. So `run_ladder`'s backup branch **has never executed in production**; it is
exercised only by `a_destructive_migration_backs_up_first`.

**Accurate statement for `backup.rs:3`:** "Every destructive database operation *that
`is_destructive` recognises* goes through this first. The classifier matches eight literal
substrings and does not cover `DROP COLUMN`, `UPDATE … SET`, `INSERT OR REPLACE`, or
whitespace variants of the forms it does list — a future rung using any of those would migrate
with no backup and no error."

#### The guard-the-guard gap: the refusal branch is untested

Both refusal messages appear exactly once in the tree — at their definition sites, with no
test asserting them:

```
$ grep -rn "refusing to apply destructive migration\|refusing to clear the task board" --include=*.rs .
autodev-core/src/db/migrate.rs:119:  "{db_name}: refusing to apply destructive migration v{} ({}) … because no backup could be taken: {e}",
autodev-core/src/db/tasks.rs:735:    "refusing to clear the task board because no backup could be taken: {e}"
```

The two tests that touch this area exercise the *other* branches:
`a_destructive_migration_backs_up_first` (`migrate.rs:353`) proves the happy path and that the
backup holds the deleted row; `a_destructive_rung_on_a_pathless_database_still_applies`
(`migrate.rs:334`) proves the in-memory exemption. **Nothing forces `backup_before` to fail
and asserts the operation is refused.** A future edit that changed `Err(e) => return Err(...)`
to a log-and-continue would leave the suite green.

**One documentation defect found in passing.** `migrate.rs:329-333`, the doc comment on
`a_destructive_rung_on_a_pathless_database_still_applies`, opens:

> /// A destructive rung must not run when its backup cannot be taken.

The test asserts the opposite of that sentence — `assert_eq!(run_ladder(&c, "mem.db", D).expect("ladder"), 2)`
requires the rung **to** run. The rest of the comment explains the in-memory exemption
correctly, and the test's *name* is accurate; the opening line is a mis-stated summary that a
reader skimming for "is the refusal tested?" would take as a yes.

---

### Claim 11 — the tool executor is the single funnel every tool result passes through

*(Taken before Claim 8, because Claim 8 depends on it.)*

**As stated** (`MASTER_DEVELOPMENT_PLAN.md:619`, and `autodev-core/src/agent/tool_executor.rs:203-210`):

> Wired at `agent/tool_executor.rs::execute` — the single funnel **every** tool result
> passes through, so it covers the conversation, the `messages` table and the UI event at
> once rather than being repeated in twenty tools

> Every tool result in the application passes through here, which is
> why the redaction happens at this one point rather than in each of
> the twenty-odd tools. […] from here the content goes three places at once:
> into the conversation the model sees, into the messages table, and
> into the UI event.

**Verdict: `REFUTED`.** `ToolExecutor::execute` is not the funnel. It has **zero production
callers**. The agent loop uses `ToolExecutor::execute_batch`, a second, parallel implementation
in the same file that performs **no redaction at all** and reaches the same three destinations.

#### Measurement 1 — the redaction lives only in `execute`

```
$ grep -n "redact" autodev-core/src/agent/tool_executor.rs
117:            input: crate::security::redaction::redact_args(&input),
205:        // why the redaction happens at this one point rather than in each of
212:        // `G4:486` recorded unredacted tool arguments landing in procedural
216:        // real content to do its work, and what is redacted is what escapes.
218:        result.output = crate::security::redaction::redact_text(&result.output);
219:        let redacted_input = crate::security::redaction::redact_args(&input);
225:            redacted_input,
```

Three call sites — 117, 218, 219 — all inside `pub async fn execute`, which spans **94–239**.
`pub async fn execute_batch` spans **246–491**. The file is 529 lines. There is no redaction
call above line 117 or below line 225.

#### Measurement 2 — the engine calls `execute_batch`, never `execute`

```
$ grep -n "tool_executor" autodev-core/src/agent/engine.rs
16:use super::tool_executor::ToolExecutor;
125:        let tool_executor = ToolExecutor::new(
132:        let mut tool_executor = match &self.session_mgr {
133:            Some(sm) => tool_executor.with_session_manager(sm.clone()),
134:            None => tool_executor,
314:                tool_executor.execute_batch(tool_batch).await?
```

Line 314 is the only invocation, and `engine.rs:296-313` shows **every** tool call the model
makes is put into `tool_batch` and dispatched through it:

```rust
            for tc in &tool_calls {
                if let MessageContent::ToolUse { id, name, input } = tc {
                    match self.registry.get(name) {
                        Some(tool) => {
                            tool_batch.push((tool, input.clone(), id.clone()));
                        }
```

#### Measurement 3 — `execute` has no production caller anywhere in the repository

```
$ grep -rn "ToolExecutor" --include=*.rs .        (node_modules and target excluded)
./autodev-core/src/agent/engine.rs:16:use super::tool_executor::ToolExecutor;
./autodev-core/src/agent/engine.rs:125:        let tool_executor = ToolExecutor::new(
./autodev-core/src/agent/tool_executor.rs:26:pub struct ToolExecutor {
./autodev-core/src/agent/tool_executor.rs:41:impl ToolExecutor {
./autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:6://! through the real `ToolExecutor`, and the assertion is made against the
./autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:18:use autodev_core::agent::tool_executor::ToolExecutor;
./autodev-core/tests/s1_04_secret_never_reaches_the_prompt.rs:43:    let mut executor = ToolExecutor::new(
```

`ToolExecutor` is constructed in exactly **two** places: `engine.rs:125` (production, which
calls only `execute_batch`) and `s1_04_secret_never_reaches_the_prompt.rs:43` (a test, which
calls only `execute`). `ToolExecutor::execute` is dead in production.

#### Measurement 4 — `execute_batch` reaches all three destinations, unredacted

Both halves of `execute_batch` — the concurrent read-only path and the serial write path —
carry raw input and raw output to every sink:

| Destination | `execute` (dead) | `execute_batch` (live, read half) | `execute_batch` (live, write half) |
|---|---|---|---|
| UI start event | `:117` `redact_args(&input)` | `:309-314` `input: input.clone()` | `:380-385` `input: input.clone()` |
| UI end event | `:231` uses redacted `result.output` | `:323-328` `output: result.output.clone()` | `:478-483` `output: result.output.clone()` |
| `messages` table | `:220-226` `redacted_input`, redacted output | `:343-350` `Message::new_tool(… input, &result.output …)` raw | `:469-475` `save_tool_message(… input, &result.output …)` raw |
| conversation | `result.output` returned redacted | returned raw to `engine.rs:329-341`, wrapped in `MessageContent::ToolResult` | same |

`engine.rs:329-341` is where the raw output becomes the model's next input:

```rust
            tool_results.extend(batch_results.into_iter().map(|(tool_use_id, result)| {
                let content = match result.hint {
                    Some(hint) => format!("{}\n\n[HINT] {}", result.output, hint),
                    None => result.output,
                };
                MessageContent::ToolResult { tool_use_id, content, is_error: result.is_error }
```

#### Measurement 5 — subagents take the same path

`autodev-core/src/subagent/native_runner.rs:28,196,209` stands up a child `AgentEngine` and
calls `engine.run(...)`. Its own header says so: *"on the same Rust `AgentEngine` as every
other agent, inside this process"* (`:5`). So subagent tool results are unredacted for the same
reason.

#### Two further bypasses inside `execute` itself

Even on the dead path, redaction is not the first thing content meets:

1. **Hooks see raw output.** `post_ctx` is built from `&result.output` at `:181-199` and
   `HookRegistry::fire(&post_hook_type, &post_ctx).await` runs at `:200` — **eighteen lines
   before** `result.output = redact_text(...)` at `:218`. `PreToolUse` gets `input.clone()`
   raw at `:150`. Hooks are user-configurable, so this is an egress path.
2. **The permission prompt sees raw input.** `request_permission` (`:493-511`) emits
   `PermissionPayload { input: input.clone(), … }` with no redaction, in both `execute` and
   `execute_batch`.

#### Why no gate caught this

`TG-P1-06` (`src-tauri/src/ci_gate.rs:776-811`) checks only that the boundary **file**
contains the string `security::redaction`:

```rust
            let production = text.split("#[cfg(test)]").next().unwrap_or("");
            if !production.contains("security::redaction") {
```

`tool_executor.rs` satisfies that from `execute` alone. The gate's own doc comment predicts
this — *"it asserts the boundary file references `security::redaction`, which a caller could
satisfy while discarding the result. The real proof is behavioural"* (`:765-769`) — and the
behavioural proof it points to does not cover the tool path: `redaction_boundaries.rs`
exercises only `EpisodicStore::store_user_message`, `store_agent_response`,
`SemanticStore::store_fact` and a control. The only tool-path behavioural test is
`s1_04_secret_never_reaches_the_prompt.rs`, and it calls `executor.execute(read_file, …)`
(`:61-66`) — the dead method. **S1-04's claim of "end to end … through the real `ToolExecutor`"
is true of an object the application never uses this way.**

#### Green while broken — measured

Both behavioural suites pass today, which is what makes this hard to see:

```
$ cargo test -p autodev-core --test s1_04_secret_never_reaches_the_prompt --test redaction_boundaries
     Running tests/redaction_boundaries.rs
running 4 tests
test a_secret_in_a_stored_fact_never_reaches_the_memory_store ... ok
test a_secret_in_a_user_message_never_reaches_the_memory_store ... ok
test a_secret_in_an_agent_response_never_reaches_the_memory_store ... ok
test the_memory_store_does_keep_the_content_it_is_given ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

     Running tests/s1_04_secret_never_reaches_the_prompt.rs
running 1 test
test a_secret_read_from_disk_never_reaches_the_prompt ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

Five green tests, and none of them touches `execute_batch`. This is the programme's own
`inert-but-green` class, one layer up: not a statement that silently fails, but a *test* that
passes against a method production does not call.

**Accurate statement:** "`ToolExecutor::execute` redacts every tool result, but the agent loop
does not call it — `AgentEngine` dispatches all tool calls through `ToolExecutor::execute_batch`,
which performs no redaction. Tool output and tool arguments therefore reach the conversation,
the `messages` table and the UI verbatim. `execute` is currently reachable only from
`tests/s1_04_secret_never_reaches_the_prompt.rs`."

**Recommended smallest fix, for the record** (not applied — I am read-only): move the two
redaction calls into `save_tool_message` and to the point where `execute_batch` builds
`results[…]`, or have `execute_batch` delegate per-tool to `execute`. A regression test must
drive the secret through `AgentEngine`/`execute_batch`, not through `execute`.

---

## What I could not determine

Each item names the measurement that would settle it. None of these changes a verdict above;
they are the edges of what I actually measured.

1. **Whether Claim 3's phone/card narrowing was deliberate.** The module documents exactly one
   deviation from the Python source (the 32-character rule) and says nothing about requiring a
   separator in the phone and card patterns. I can measure the divergence; I cannot measure
   intent.
   *Settles it:* an ADR entry or a code comment stating the trade — or, if it was accidental,
   restoring the `?` and adding a test for `5551234567` / `4111111111111111`.

2. **Claim 4's desktop count on a Linux runner.** Every measurement here ran on
   `x86_64-pc-windows-msvc`. I used `--target all` to make the Linux-only GTK/webkit edges
   visible, which is a superset, not the Linux resolution.
   *Settles it:* `cargo tree -p autodev | grep -icE "tauri|gtk|webkit"` executed on the Linux
   CI runner, recorded next to the Windows figure.

3. **Claim 5: whether `airsctl analytics` can leave `analytics.db` untuned in practice.**
   `AnalyticsReader::open` (`src-shared/src/db/analytics.rs:55`) opens read-write with no
   pragmas. Today the file is already WAL because the desktop app set it, so the bypass is
   invisible; on a machine where `airsctl` runs before the app has ever opened the file, the
   database would be created in `journal=delete`.
   *Settles it:* on a clean profile, run only `airsctl analytics`, then read
   `PRAGMA journal_mode` from the resulting `analytics.db`.

4. **Claim 11: an end-to-end demonstration through `AgentEngine`.** My refutation is structural
   — zero callers, zero redaction calls in `execute_batch`, the batch path's raw sinks — and I
   believe it is airtight, but I did not run the agent loop against a live model to watch a
   secret arrive in `MessageContent::ToolResult`. Doing so needs a provider credential and a
   live call, which is outside my mandate here.
   *Settles it:* an integration test with a stub `ModelProvider` that emits one
   `ToolUse{read_file}` for a file containing a secret, driven through `AgentEngine::run`, and
   asserting on the resulting `ToolResult` content and on the `messages` row. That test is also
   the regression guard the fix needs.

5. **Claim 10: whether `is_destructive`'s holes are reachable in practice.** No ladder in the
   tree uses `DROP COLUMN`, `UPDATE … SET` or `INSERT OR REPLACE` today, so the classifier's
   gaps are latent. Whether a future rung will need one of those forms is a design question, not
   a measurement.
   *Settles it:* nothing measurable — but adding the missing needles (and a whitespace-tolerant
   matcher) costs one line each and removes the question.

6. **Whether the `api_key` value found in `~/.autodev/logs/` is currently valid.** I established
   its shape (164 chars, `sk-` prefix, adjacent to the field name `api_key`, identical in two
   files) and deliberately did **not** read, copy or transmit the value, so I cannot say whether
   it is live, revoked, or a placeholder.
   *Settles it:* the user checking the key against their provider account. Regardless of the
   answer, it should be rotated — it is on disk in plaintext.

---

## What held up

A refutation brief that only refutes is noise. Five claims survived a genuine attempt to break
them, and two of those are the ones I most expected to fall.

**Claim 1 — rusqlite's 5000 ms default.** I attacked it three ways: I read the vendored
`inner_connection.rs` to find the `sqlite3_busy_timeout(db, 5000)` call; I measured
`PRAGMA busy_timeout` on four different open paths *and* added a control (`PRAGMA busy_timeout = 0`
reads back `0`) so a constant-5000 pragma could not fool me; and I checked whether "every
connection it opens" has an exception, finding only the `unsafe` `from_handle` constructors,
which do not open anything and appear nowhere in the workspace. It is correct, and the
uncomfortable corollary the author drew from it — *"this constant does not fix a bug"* — is
correct too.

**Claim 2 — chrono's RFC 3339 parser.** I expanded the four asserted combinations to twelve,
specifically hunting for a space-separator case that fails *because of the space* (to break
"the separator is not the problem") and for an offset-less case that parses (to break "rejects
any value without an offset"). Neither exists. The separator slot really is `[Tt ]`; the failure
really is the missing offset. I also re-ran the whole set at chrono `0.4.45` before pinning to
the workspace's `0.4.44` to rule out a version accident.

**Claim 6 — the graph columns, and the retraction in `ADR-LOG-RESTRUCT-0017`.** This was the
brief's flagged candidate for a false retraction, and I went at it hardest. I refused
`--follow` and searched the string itself across every ref: `CREATE TABLE IF NOT EXISTS nodes`
was written in **one commit, ever**, and that commit already contains all four columns. I then
checked the two other commits the `-S` searches surfaced and found one docs-only and one whose
hits are the migration quoted inside a verify report. Then the decisive check: `CREATE_NODES`
and `migration_v5` entered the tree in the **same commit**, so the state v6 defends against is
unreachable from any commit on any ref. Finally I reconstructed the original v5 and ran it —
six swallowed syntax errors, version stamped, committed, and not one column or row changed —
and tested the ADR's own corroborating argument by confirming that `CREATE INDEX … ON
nodes(agent_author)` really does fail with `no such column` when the column is absent. The
retraction is right. Only its justification for shipping v6 is overstated.

**Claim 7 — `plans.db` and the data risk.** I expected this to break, because `plans.db`
**does** exist on disk right now. It does not break: the pre-P1 full snapshot at
`~/autodev-backups/p1-20260901-113442/dot-autodev/` lists every file in `~/.autodev` before any
P1 work and contains no `plans.db`; the file's 13:50 mtime matches the `p1-10-preflight`
backup, i.e. P1's own verification run created it; and reading a byte copy of it shows **0 rows
across all four tables**. The claim is about the past and the past is as described. Only a
point-in-time verify report's present tense has gone stale.

**Claim 5 — the 17 connection sites.** I did not accept the prior audit's count. I resolved
every file's `#[cfg(test)]` boundary and compared it against each call site's line number, then
enumerated *every* way a `rusqlite::Connection` can be constructed anywhere in core and checked
the flags at each non-`open_tuned` production site — all five are read-only, which the rule
exempts. Then I ran the gate rather than reading it: `tg_p1_05_every_core_connection_is_opened_tuned`
passes. 17 is exact. The caveats I recorded are about scope and about the gate's reach, not
about the count.

**Claim 10a/10b — the backup gating.** I tried to find a fallthrough in both call sites and
there is none; I tried to find a file-backed database whose `conn.path()` returns empty and
there is none; and I tried hardest to produce a *successful-looking* backup of a damaged source
by corrupting a page in a 500-row database — `VACUUM INTO` refused it outright with `database
disk image is malformed`, before the size and integrity checks were even reached. The helper is
better than the claim needed it to be. What I did break is the sentence one layer up, in
`backup.rs`'s header, and the absence of any test for the refusal branch.
