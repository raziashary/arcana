# builtin_tools audit — still-open defects

Scope: every file in `autodev-core/src/agent/builtin_tools/`.
Read-only audit. Findings are written incrementally as confirmed.

Known-closed items (permission-before-partition, `check_file` interpreter
resolution, `e2e_probe` URL/credential handling, the seven jailed path tools,
byte-bounded outputs, `remember_this` deletion) are **not** re-reported.

---

# bash.rs

## What `bash` actually does now (established by reading, not by comment)

1. `execute` reads `command` (required) and `timeout_secs` (optional, `as_u64`).
2. `validate_workspace_path(&PathBuf::from(&ctx.working_dir), &ctx.working_dir)` —
   both arguments are the same value, so this is an existence/canonicalisation
   check, not a jail (self-documented at :61-85).
3. `check_redirect_targets(&command, &ctx.working_dir)` — a byte scanner over
   `>`, `>>`, `tee` and heredoc tails.
4. On Windows only: `rewrite_for_windows(&command, &ctx.working_dir)` rewrites the
   command text, then the REWRITTEN text is passed to `cmd /C`.
5. Non-Windows: `sh -c <command>`, unrewritten.
6. Credential-shaped variables are removed from the child env.
7. `tokio::time::timeout(...)` wraps `run_command`, which sets `current_dir` and
   calls `cmd.output().await`.
8. Exit code -> `is_error`; `build_evidence::record` is fed the (possibly
   rewritten) command and that exit code; output is labelled and tail-truncated
   to 50 000 bytes.

The historically-broken step 4 is still present and still feeds `cmd /C`.

---

## T-1 · The POSIX->PowerShell rewriter still runs, and its output is still executed by cmd.exe · critical
**Where:** `bash.rs:105-118` (apply + spawn); `bash.rs:514-547` (`rewrite_for_windows`)
**Defect:** On Windows the command text is rewritten to PowerShell cmdlets —
`Select-Object -First`, `Select-Object -Last`, `Select-String` (:535-543) — and the
rewritten string is then handed to `Command::new("cmd").args(["/C", &command])`
(:116-117). `cmd.exe` has no `Select-Object` and no `Select-String`; they are
PowerShell cmdlets, not executables on PATH. Nothing in this file ever spawns
`powershell` or `pwsh`. The rewrite therefore converts a working pipeline into a
guaranteed command-not-found on the right-hand side of the pipe, and in `cmd.exe`
the ERRORLEVEL of `a | b` is b's, so the left-hand side's real status is discarded.
**Failure scenario:** model calls
`bash{"command":"npm run build 2>&1 | tail -50"}` on Windows.
`rewrite_for_windows` (:539) turns it into
`npm run build 2>&1 | Select-Object -Last 50`.
`cmd /C` runs the build (which succeeds, exit 0), pipes into `Select-Object`,
which does not exist -> stderr "'Select-Object' is not recognized...", pipeline
exit code non-zero. `execute` sets `is_error = true` (:207,:224) and returns
`ToolResult::error`, and `build_evidence::record` (:158) stores the Node build with
a non-zero `exit_code`, which `update_task_status` then reads to veto the
`qc_review` transition. A successful build is reported to the model as failed and
recorded as failed. This is the exact regression the phase was meant to close; it
is unrepaired.
**Confidence:** certain (read the code) — the rewrite site, the `cmd` spawn site
and the absence of any `powershell` spawn are all in this one file.

## T-2 · grep rewrite ordering makes the -i branch dead and emits an invalid flag · high
**Where:** `bash.rs:542-543`
**Defect:** `out.replace(" | grep ", " | Select-String ")` runs BEFORE
`out.replace(" | grep -i ", " | Select-String -Pattern ")`. `" | grep "` is a
prefix of `" | grep -i "`, so line 542 always consumes the match first and line 543
can never fire — a guard constructed and never consulted. Line 542 also leaves the
Unix flag attached to a cmdlet that does not accept it.
**Failure scenario:** `cargo test 2>&1 | grep -i "error"` becomes
`cargo test 2>&1 | Select-String -i "error"`. Under `cmd /C` that is
command-not-found (T-1); under PowerShell `-i` is not a `Select-String` parameter
(`-CaseSensitive` is). Either way a passing test run returns `is_error = true`.
**Confidence:** certain (read the code).

## T-3 · head-/tail- are replaced blind, so any other flag is mangled · high
**Where:** `bash.rs:535-539`
**Defect:** `out.replace(" | head -", " | Select-Object -First ")` substitutes on
the prefix `head -`, not on a parsed `head -N`. Every flag other than a bare count
is corrupted, and a space is inserted between the cmdlet parameter and whatever
followed the dash.
**Failure scenario:** `cat build.log | tail -f` -> `... | Select-Object -Last f`.
`cat app.json | head -c 200` -> `... | Select-Object -First c 200`. Both are
nonsense; the tool reports the user's correct command as an error, and the output
does not mention `head`/`tail` at all, so the model cannot see what happened.
**Confidence:** certain (read the code).

## T-4 · The exit-255 self-correction hint is unreachable · high
**Where:** `bash.rs:211-223`
**Defect:** the structured `[SUGGESTION]` fires only when
`exit_code == 255 && cfg!(windows) && output.contains("is not recognized")`.
`cmd.exe` reports a missing command with ERRORLEVEL 9009 (or 1 from inside a
pipeline), never 255; `sh` uses 127. The one remediation the tool offers for
exactly the failure T-1 manufactures can never be emitted. It is also evaluated
AFTER truncation (:177-206) has replaced `output`, so even with the right code the
"is not recognized" substring would be gone whenever that message fell outside the
last 50 000 bytes.
**Failure scenario:** the T-1 scenario. The model receives a bare
"'Select-Object' is not recognized" with no guidance, retries the same pipeline,
and loops.
**Confidence:** certain that 255 is not the command-not-found code for `cmd`/`sh`;
the code path itself is read directly.

## T-5 · Timeout abandons the child instead of killing it — no kill_on_drop, no process-group kill · critical
**Where:** `bash.rs:142-151`; `run_command` at `bash.rs:234-263`
**Defect:** `timeout(Duration, run_command(...))` only drops the future.
`tokio::process::Command` defaults to `kill_on_drop(false)`; dropping the `Child`
detaches the process rather than killing it, and there is no process-group
creation and no `.kill()` anywhere in this file. Compare `validate.rs:277-281` and
`e2e_probe.rs:418,:636`, which both set `kill_on_drop(true)` — `bash.rs`, the tool
that runs model-authored commands, is the only executor that does not. There is
also no cancellation channel, so a user-cancelled turn leaves the child running.
**Failure scenario:** `bash{"command":"cargo build","timeout_secs":30}` on a cold
target dir. At 30 s the tool returns "Command timed out after 30s"; `cargo` keeps
running and keeps `target/.cargo-lock`. The next call, `cargo test`, blocks on
"Blocking waiting for file lock on build directory" for its full 120 s and also
times out. Every subsequent build in the session fails until the app is killed.
Nothing in the returned text points at the orphan.
**Confidence:** certain (read the code; grep confirms `kill_on_drop` appears only
in `validate.rs` and `e2e_probe.rs`).

## T-6 · timeout_secs is unvalidated: 0 makes every command fail instantly, a float is silently ignored · medium
**Where:** `bash.rs:57-59`, `:142-151`
**Defect:** `input["timeout_secs"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS)`. No
floor, no ceiling, and `as_u64()` returns `None` for a JSON float or a numeric
string, silently substituting 120 with no note in the result.
**Failure scenario A:** `timeout_secs: 0` (a plausible "no timeout" encoding).
`Duration::from_secs(0)` is ready immediately, so every command returns
"Command timed out after 0s: <cmd>" while the child is spawned and then abandoned
(T-5). Every bash call in that turn fails.
**Failure scenario B:** `timeout_secs: 600.0` (JSON float; an integer schema does
not stop a provider emitting one). `as_u64()` -> `None` -> 120 s. A ten-minute
suite is cut at two minutes and reported as "timed out after 120s" against a
request the model believes asked for 600.
**Confidence:** certain (read the code).

## T-7 · Redirect to /dev/null — and any absolute-path target — is refused outright · high
**Where:** `bash.rs:385-394`, via `path_jail.rs:204-222`
**Defect:** any redirect target beginning with `/` is validated against the
workspace and refused when it does not resolve inside it. `/dev/null` never does.
On Windows it is worse: `Path::new("/dev/null").is_absolute()` is false (no drive
prefix), so the `starts_with('/')` branch at :385 hands it to `canonicalize_target`
(`path_jail.rs:87-132`), whose existing-ancestor walk resolves `/` to the current
drive root and yields `C:\dev\null` — compared to the workspace and rejected.
**Failure scenario:** `bash{"command":"npm test 2>/dev/null"}` or
`npm run build > /dev/null 2>&1` — an idiom the model emits constantly — returns
"Refused: bash redirect target '/dev/null' is outside the workspace ... Use a
workspace-relative path. The command was not executed." The test never runs, and
the model cannot comply because there is no in-workspace null device and the
message does not say so.
**Confidence:** certain for the refusal; the Windows `C:\dev\null` resolution is
certain from reading `canonicalize_target` (`Path::new("/").exists()` is true on
Windows).

## T-8 · The $-refusal fires on comparison operators, not just on redirects · high
**Where:** `bash.rs:340-368`
**Defect:** the scanner treats EVERY `>` byte as a redirect marker and takes the
next whitespace-delimited token as the target. If that token contains `$` the
whole command is refused (:360-367). Nothing distinguishes a redirect from a
greater-than inside quotes, inside an awk/jq/node program, or in a comparison.
**Failure scenario:** `bash{"command":"ps aux | awk '$3 > $2 {print $1}'"}` — the
token after `>` is `$2` -> "Refused: bash redirect target '$2' contains a shell
variable or tilde expansion ... The command was not executed." Same for
`node -e "if (a > $x) ..."` and any snippet comparing a variable. The model is
told to "pass an already-expanded literal path" for something that is not a path.
On Windows the refusal is doubly wrong: `cmd.exe` does not expand `$` at all, so
the value the check cannot "prove" is already a literal.
**Confidence:** certain (read the code).

## T-9 · cmd /C receives MSVCRT-escaped quotes that cmd.exe does not understand · high
**Where:** `bash.rs:115-118`
**Defect:** `Command::new("cmd").args(["/C", &command])` uses std's ordinary
Windows argument quoting, which wraps an argument containing spaces in quotes and
escapes interior quotes as backslash-quote. `cmd.exe` has no backslash escape: per
its documented `/C` rule it strips the first and last quote of the remainder and
passes the backslashes through verbatim. The remedy for this is
`std::os::windows::process::CommandExt::raw_arg`, which grep finds nowhere in the
tree.
**Failure scenario:** a command whose text contains double quotes, e.g.
`echo "hello world" > out.txt`, writes the literal characters
backslash-quote-hello world-backslash-quote into `out.txt` and exits 0 — a wrong
result reported as SUCCESS, the harder direction to detect. For
`python -c "print('x')"` the interior quotes reach the interpreter as part of the
program text, so it evaluates a bare string literal, does nothing, and exits 0.
**Confidence:** likely — the `cmd /C` quote rule and std's escaping are both
documented, but the exact rendering for a given command should be confirmed with a
run on Windows.

## T-10 · Build evidence is recorded against the rewritten command, not the model's · medium
**Where:** `bash.rs:105-113` (shadowing) then `bash.rs:150`, `:158`
**Defect:** `let command = if cfg!(windows) { rewrite_for_windows(...) } else { command }`
shadows the original binding, so every later use — the timeout message and the
evidence record — sees the rewritten text and the original is unrecoverable.
**Failure scenario:** the model runs `npm run build | tee build.log`; the stored
`BuildEvidence.command` holds rewritten text the model never wrote. When
`update_task_status` refuses the `qc_review` transition and quotes the recorded
command back, the model is shown a command it did not issue and cannot reconcile
it with its own transcript. The timeout message has the same problem.
**Confidence:** certain (read the code).

## T-11 · The tool description contradicts itself and describes a shell that is not the one used · high
**Where:** `bash.rs:27-32` (`description`), `bash.rs:34-49` (`input_schema`)
**Defect:** the Windows description says, in one paragraph, "The shell is Windows
CMD" and "Use PowerShell-compatible alternatives". Those are mutually exclusive
instructions, and the code implements neither faithfully: it runs `cmd /C` but
silently rewrites parts of the command into PowerShell. The description omits
every behaviour that can refuse or alter the call:
- the redirect check (`>`/`>>`/`tee` targets must resolve inside the workspace;
  `/dev/null` and any `$`-bearing token are refused — T-7, T-8);
- the silent rewrite of `head`/`tail`/`grep` (T-1..T-3);
- credential-shaped environment variables being stripped from the child (T-13);
- that `timeout_secs` bounds the WAIT, not the process (T-5).
**Failure scenario:** the model follows the description's own advice and writes
`Get-ChildItem src | Select-Object -First 5`; `cmd /C` returns
"'Get-ChildItem' is not recognized". The model then looks for the `[SUGGESTION]`,
which never fires (T-4). The description is the proximate cause of the failure.
**Confidence:** certain (read both the description and the spawn site).

## T-12 · The "workspace path-jail" at the top of execute compares working_dir with itself · low (documentation/style)
**Where:** `bash.rs:86-92`
**Defect:** `validate_workspace_path(&PathBuf::from(&ctx.working_dir), &ctx.working_dir)`
— target and workspace are the same string, so `paths_match_prefix` is trivially
true; the only reachable failure is a non-canonicalisable `working_dir`. This is
now documented as not being a jail (:61-85), but the call still ships as a guard
and three other doc-comments in the same file (:94-100, :265-285, :399-403) still
refer to "the cwd jail" as the load-bearing check.
**Failure scenario:** no wrong output. The cost is that a reader or a future change
treats `bash` as contained when only the permission prompt contains it. Flagged as
a style/doc finding per the brief's rule. The comment's premise does check out:
grep finds no configured workspace root in `AppState` to compare against.
**Confidence:** certain (read the code).

## T-13 · Credential scrubbing also removes tokens that builds legitimately need · medium
**Where:** `bash.rs:136-140`, using `security/env_scrub.rs::looks_like_a_credential`
**Defect:** every variable whose name ends in `_KEY`/`_TOKEN`/`_SECRET`/
`_PASSWORD`/`_CREDENTIALS` or contains `API_KEY`/`SECRET`/`PASSWORD` is removed
from the child environment unconditionally. The `KEEP` allowlist in `env_scrub.rs`
is consulted only by `scrub_process_env`, not by this call site, and the removal is
never mentioned in the tool output.
**Failure scenario:** a repo whose `.npmrc` contains
`//registry.npmjs.org/:_authToken=${NPM_TOKEN}`. `bash{"command":"npm ci"}` runs
with `NPM_TOKEN` removed, npm gets 401, exit 1. The tool reports a build failure,
`build_evidence` records a failed Node build, and `qc_review` is vetoed — for a
command that succeeds in the user's own terminal. Same for `GITHUB_TOKEN` with
`gh`/`git push` and `CARGO_REGISTRY_TOKEN` with `cargo publish`. Nothing in the
output says the environment was altered.
**Confidence:** certain that the variables are removed; the npm/gh consequence is
those tools' documented behaviour.

## T-14 · strip_heredoc_bodies rebuilds the command byte-by-byte as char, corrupting UTF-8 · low
**Where:** `bash.rs:493`
**Defect:** `out.push(bytes[i] as char)` casts each BYTE to a `char` — a Latin-1
reinterpretation, not a UTF-8 decode. Any multi-byte character in the command
becomes two or more mojibake characters in the string that
`check_redirect_targets` then re-scans (:404-410).
**Failure scenario:** a heredoc whose body contains a non-ASCII character followed
by a chained redirect outside the workspace. The re-scan runs over mojibake, so the
refusal message quotes a mangled target, and a target whose bytes re-tokenise
differently can slip past or falsely trip the scan. No panic (every pushed `char`
is valid), so the corruption is silent.
**Confidence:** certain (read the code).

## T-15 · stdout/stderr interleaving is destroyed and the STDOUT header is the first thing truncation drops · low
**Where:** `bash.rs:160-206`
**Defect:** output is assembled as a `=== STDOUT ===` block then a `=== STDERR ===`
block, then truncated from the HEAD (keeping the tail). Causal order between the
two streams is lost, and when the combined text exceeds 50 000 bytes the
`=== STDOUT ===` label and all of stdout can vanish while `=== STDERR ===`
survives further down.
**Failure scenario:** `cargo test` with 60 KB of stdout and 2 KB of stderr. The
model receives a block that opens mid-line with only a later `=== STDERR ===`
label, attributes the surviving stdout lines to stderr, and reports "the build
emitted these errors" for ordinary test output.
**Confidence:** certain (read the code).

## T-16 · The evidence ring grows without bound and is only swept on read · low
**Where:** `build_evidence.rs:168-184`, `:189-201`
**Defect:** `record` pushes into `HashMap<String, Vec<BuildEvidence>>` with no cap
and no eviction; expiry only happens inside `most_recent_build` /
`most_recent_smoke`, and only for the session being queried. Session keys are never
removed, only their vectors emptied.
**Failure scenario:** a long-running fleet session that runs builds but never
transitions to `qc_review` accumulates one 200-char entry per build for the life of
the process. Not a correctness bug for any single call — a slow leak. Style-level.
**Confidence:** certain (read the code).

---

# Cross-tool findings

## T-17 · edit_file with an empty old_string spins forever, holding the per-path lock · critical
**Where:** `edit_file.rs:133-163`, specifically the position-scan loop at :151-157
**Defect:** `old_string` is taken straight from model input with no emptiness
check. For a non-empty file, `content.matches("").count()` is `len+1`, so
`match_count > 1`. With `replace_all` absent (defaults false via
`#[serde(default)]`) control enters the "report the match positions" loop:
```rust
while let Some(pos) = content[start..].find(&params.old_string) {
    let line_num = content[..start + pos].lines().count() + 1;
    positions.push(line_num);
    start += pos + params.old_string.len();   //  += 0 + 0
}
```
`find("")` returns `Some(0)` for every `start`, and `pos + old_string.len()` is
`0`, so `start` never advances. The loop never terminates and `positions` grows
without bound.
**Failure scenario:** `edit_file{"path":"src/main.rs","old_string":"","new_string":"// hdr"}`
— a shape a model reaches for when it means "insert at the top". The tool future
never completes, so: the turn hangs; `positions` allocates until the process is
OOM-killed; and the `OwnedMutexGuard` taken at :106 is never dropped, so every
later `write_file`/`edit_file`/`append_file` on that same path blocks forever
too. There is no timeout around tool execution to break it.
**Confidence:** certain (read the code; `str::find("")` returning `Some(0)` and
`str::matches("")` counting `len+1` are both documented std behaviour).

## T-18 · edit_file with an empty old_string and replace_all=true shreds the file · critical
**Where:** `edit_file.rs:165-169`
**Defect:** the same missing emptiness check on the other branch.
`content.replace("", &new_string)` inserts `new_string` at **every character
boundary**, including both ends.
**Failure scenario:** `edit_file{"path":"src/lib.rs","old_string":"","new_string":"X","replace_all":true}`
turns `fn main` into `XfXnX XmXaXiXnX`. The file is written (`fs::write` at :180)
and the tool returns **success**: `"✓ Edited src/lib.rs (N occurrences replaced,
40 lines → 40 lines)"`. The only recovery is the git checkpoint at :175, and the
agent is told the edit succeeded so it does not look.
**Confidence:** certain (read the code).

## T-19 · edit_file runs only Tier 1 validation; write_file runs four tiers · medium
**Where:** `edit_file.rs:220-225` vs `write_file.rs:167-215`
**Defect:** `write_file` runs `tier1_syntax`, `tier2_imports`, `tier2_5_exports`
and `tier2_6_api_contract`. `edit_file` runs `tier1_syntax` only. Nothing in
either description mentions the difference, and `write_file` refuses to touch an
existing file without `overwrite=true` — so the ordinary path for changing an
existing file is exactly the path with the weakest validation.
**Failure scenario:** an agent adds `import axios from 'axios'` to an existing
`src/api.ts` with `edit_file`. `package.json` has no `axios` dependency.
`tier2_imports` — the check whose own comment (`write_file.rs:173-177`) says it
"catches that in-process on the very write that introduced the mismatch" —
never runs. The mismatch survives to the build, which is where the calendar-run
bug this tier exists to prevent was originally found.
**Confidence:** certain (read both call sites).

## T-20 · append_file takes no checkpoint and fires no PostFileWrite hook · medium
**Where:** `append_file.rs:55-157` (whole `execute`)
**Defect:** it is the only mutating file tool that calls neither
`crate::git::checkpoint::auto_checkpoint_before_write` (which `write_file.rs:133`
and `edit_file.rs:175` both call) nor `HookRegistry::fire(PostFileWrite, ...)`
(`write_file.rs:161`, `edit_file.rs:212`). Its own description sells it as
"Safer than read+write".
**Failure scenario A:** an agent appends 400 lines of duplicated config to
`requirements.txt`. The user asks to undo; the checkpoint system has no snapshot
of the pre-append content because no checkpoint was taken, so the change is not
revertible through the app.
**Failure scenario B:** the graph engine and any user hook subscribed to
`PostFileWrite` never observe the file changing, so downstream state (graph
edges, telemetry, custom formatters) silently diverges from disk for every file
that was only ever appended to.
**Confidence:** certain (read the code and both comparison sites).

## T-21 · read_file's "has a range" test silently fails for non-integer JSON numbers · medium
**Where:** `read_file.rs:125-126`, `:146-147`
**Defect:** `has_range` is computed from `as_u64()`, which returns `None` for a
JSON float or a numeric string. There is no error for a wrongly-typed
`start_line`; the value is discarded and the call proceeds as if no range was
given. The same `as_u64()` at :146-147 then drops the value a second time.
**Failure scenario:** a provider serialises the model's `start_line` as `1.0`
(legal JSON, and the `"type": "integer"` schema is advisory to the provider, not
enforced here). On a 3 000-line file the call returns
"File has 3000 lines, which exceeds the 800-line no-range read limit. Re-read
with start_line and end_line" — advice the model has already followed. It retries
identically and loops until the turn budget is exhausted.
**Confidence:** certain that the value is dropped; the provider emitting a float
is the part that needs a run to observe.

## T-22 · Two different constants both called "256 KB" bound read_file · low
**Where:** `read_file.rs:31` (`READ_FILE_MAX_BYTES = 256 * 1024` = 262 144) vs
`read_file.rs:129` (`MAX_BYTES: u64 = 256_000`)
**Defect:** the pre-read metadata guard and the post-format byte cap use
different numbers, and the metadata guard's message renders `256_000 / 1024` as
"limit 250 KB" while the doc comment for the other constant says 256 KB.
**Failure scenario:** cosmetic only — a file between 250 000 and 262 144 bytes is
refused with a message quoting a limit that does not match the constant the code
elsewhere calls the limit. Style-level finding.
**Confidence:** certain (read the code).
## T-23 · grep's total-byte bound cannot bind when the matches are in one file · high
**Where:** `grep.rs:335-352`
**Defect:** the trimming loop is `while trimmed.results.len() > 1 && <still too big>
{ trimmed.results.pop(); }`. It stops at **one** `GrepFileResult`, so if a single
file's matches exceed 256 KB the loop exits with the payload still over the cap
and returns it via `success_with_hint`. Nothing trims *within* a file result.
The module doc at :70-77 and the test `the_total_bound_is_a_different_bound_from_the_per_line_one`
(:408-419) both assert a total bound exists; the test only compares two constants
and never exercises this path, so it passes while the bound does not hold.
**Failure scenario:** `grep{"pattern":"function","path":"dist/bundle.js","max_results":500,"context_lines":20}`.
Each match contributes one bounded line (2 KB) plus 40 bounded context lines
(80 KB) — roughly 82 KB per match, all inside one `GrepFileResult` because they
are all in one file. 500 matches produce a ~40 MB JSON string that is returned to
the model whole, with a hint claiming results were "trimmed to stay under 256 KB".
**Confidence:** certain (read the loop condition).

## T-24 · grep's fallback truncation slices a String at a raw byte index · low
**Where:** `grep.rs:343-344`
**Defect:** `.unwrap_or_else(|_| json_output[..MAX_TOTAL_BYTES.min(json_output.len())].to_string())`
is exactly the `&x[..N]` shape `security/truncate` exists to eliminate, on a
string built from file contents (arbitrary UTF-8). It panics if byte 262 144
falls inside a multi-byte character.
**Failure scenario:** reachable only when `serde_json::to_string` on `GrepOutput`
fails, which for this type it effectively cannot — so this is a latent panic in
near-dead code rather than a live one. Kept as a style/latent finding.
**Confidence:** certain about the shape; near-unreachable in practice.

## T-25 · grep claims to skip binary files; it has no binary detection and no per-file size cap · medium
**Where:** `grep.rs:89-95` (description) vs `grep.rs:241-244`
**Defect:** the only "binary skip" is that `fs::read_to_string` errors on invalid
UTF-8 and the entry is skipped. Any file that happens to be valid UTF-8 — a
multi-gigabyte log, a base64 blob, a generated CSV, a minified bundle — is read
into memory in full, with no size check before the read. The per-line and total
output bounds run only *after* the whole file is in RAM.
**Failure scenario:** a workspace containing a 2 GB UTF-8 `.jsonl` dataset.
`grep{"pattern":"error"}` from the workspace root allocates 2 GB in
`fs::read_to_string`, then another allocation for `lines()`. On a laptop this is
an OOM or a multi-minute stall with no timeout around it — the tool has no
timeout of its own. The description told the model this file would be "skipped".
**Confidence:** certain (read the code).

## T-26 · glob reports a total_files that is the returned count, not the match count, and sorts after truncating · medium
**Where:** `glob.rs:200-219`
**Defect:** the walk `break`s once `file_entries.len() >= max_results` (:207-209),
then the vector is sorted (:213), then `total_files` is set to
`file_entries.len()` (:217). Unlike `grep`, the serialized output carries **no
`truncated` flag** — the cap is disclosed only in the out-of-band `hint` string.
So the JSON the model parses says `total_files: 200` for a directory with 5 000
matches, and the 200 it gets are the first 200 in *walk order*, alphabetised
afterwards so they look like a complete sorted listing.
**Failure scenario:** `glob{"pattern":"**/*.ts"}` in a mid-size frontend repo.
The model receives 200 alphabetically-ordered paths and `total_files: 200`,
concludes it has enumerated every TypeScript file, and reports "there are 200 .ts
files" or refactors "all" of them. Files later in walk order are silently absent.
**Confidence:** certain (read the code).
## T-27 · list_directory follows symlinks and so lists outside the workspace jail · medium
**Where:** `list_directory.rs:60-63` (jail on the base only), `:163` (`entry_path.is_dir()`), `:189` (recursion)
**Defect:** the jail validates the *base* directory and nothing else.
`Path::is_dir()` follows symlinks, and `std::fs::read_dir` on a symlinked
directory reads the target. `grep.rs:198` and `glob.rs:153` both set
`follow_links(false)` on their walkers; `list_directory` has no equivalent.
**Failure scenario:** the workspace contains a symlink or directory junction
`vendor -> C:\Users\Razi`. `list_directory{"recursive":true}` returns the names,
`is_directory` flags and `size_bytes` of the user's home directory (three levels
deep, up to 500 entries) — file names alone routinely disclose client names,
project names and credential filenames. The jail message never fires because the
base path is legitimate.
**Confidence:** certain (read the code; `Path::is_dir` following symlinks is
documented std behaviour).

## T-28 · check_file returns SUCCESS after running nothing when `checks` is empty or misspelled · medium
**Where:** `check_file.rs:127-144`, `:259-274`
**Defect:** `checks` is read with `as_array()` and each element is kept only if it
is a string; there is no validation against the schema's
`enum ["syntax","imports","exports","contract","types"]`. An empty array, or an
array of unrecognised names, sets every `run_*` flag to false, produces no
sections, and falls into the `sections.is_empty()` branch — which returns
`ToolResult::success` with the text "No checkers available for .ts files in the
requested tiers (…)". The stated reason (no checker for this extension) is not
the actual reason (the caller asked for no checks).
**Failure scenario:** `check_file{"path":"src/api.ts","checks":["typecheck"]}` —
a plausible misspelling of `types`. The tool returns success. The agent's own
protocol ("call check_file before marking a task done") is satisfied by a call
that ran zero checkers, and the task is marked done with an unvalidated file.
**Confidence:** certain (read the code).

## T-29 · check_file's display path degrades to an absolute path because it strips a raw prefix from a canonical one · low
**Where:** `check_file.rs:251-255`
**Defect:** `path` is the canonical, verbatim-stripped path returned by the jail;
`ctx.working_dir` is the raw configured string. On Windows these differ in case
and in trailing-separator form, so `strip_prefix` returns `Err` and
`unwrap_or(&path)` falls back to the full absolute path.
**Failure scenario:** cosmetic — the model sees
`check_file: C:/AIRS Foundry/autodev/autodev/src/api.ts` instead of
`check_file: src/api.ts`, and echoes absolute paths back into its narration.
Style-level.
**Confidence:** likely — depends on the exact configured `working_dir` string.
## T-30 · spawn_subagent tells the model an empty `tools` list means "MCP tools only"; empty means UNRESTRICTED · high
**Where:** `spawn_subagent.rs:52-56` (schema), `:162-170` (the read), then
`subagent/native_runner.rs:315` -> `agent/engine.rs:104-107` ->
`agent/tool_registry.rs:42-47`
**Defect:** `restrict_to` returns immediately when `names.is_empty()` — an empty
subset is a **no-op**, so the child keeps the parent's entire registry: `bash`,
`write_file`, `edit_file`, and `spawn_subagent` itself. The schema string the
model reads says `"Empty = MCP tools only"`. `native_runner.rs:600` records this
exact history ("`tools` defaulted to empty (= unrestricted), and the child got
`bash`"), so the code knows; the description does not.
**Failure scenario:** the model wants a read-only research subagent and, taking
the schema at its word that omitting `tools` yields a minimal MCP set, calls
`spawn_subagent{"task_description":"summarise the auth flow"}` with no `tools`.
The child runs with the full write+execute set at the parent's permission mode
and can modify the repository. This is the same shape as the removed
`remember_this` problem: the description advertises a restriction that the code
does not implement.
**Confidence:** certain (read all four hops).

## T-31 · Two spawn tools tell the model to wait for a sidecar that no longer exists · medium
**Where:** `spawn_subagent.rs:110-114`, `spawn_qc_review.rs:93`
**Defect:** when `subagent::manager::global()` is `None` both tools return
"Subagent system unavailable: SubagentManager not yet initialised. Wait for
sidecar startup to complete and try again." The subagent path is now the
in-process `native_runner`; `recipes/defaults.rs` states in five places that this
path "needs no sidecar and no Python", and `observability/error_kinds.rs:13`
describes the Python sidecar as the retired core-orchestrator.
**Failure scenario:** the manager is genuinely uninitialised (startup ordering, or
a test harness). The model is told to wait for a process that will never start,
so it sleeps/retries in a loop instead of reporting the real condition to the
user or falling back to doing the work itself. The message names the wrong
subsystem, so a human reading the transcript investigates the sidecar.
**Confidence:** certain (read both messages and the retirement statements).

## T-32 · spawn_parallel_subagents reports SUCCESS even when every subagent failed · medium
**Where:** `spawn_parallel_subagents.rs:233-288`
**Defect:** each per-subtask failure is folded into the results array as
`{"status":"failed","error":...}` and the tool always ends with
`ToolResult::success(...)`. There is no path that sets `is_error`. The single
version does the opposite: `spawn_subagent.rs:204-214` returns
`ToolResult::error` when the child ends `Failed` or `Terminated`.
**Failure scenario:** the pool is saturated (`MAX_GLOBAL_CONCURRENT`), so all ten
`mgr.spawn` calls return `Err` and every entry is `[i] FAILED: subagent
rejected…`. The tool result carries `is_error = false`, so any caller that
branches on `is_error` — the executor's `OnToolError` hook dispatch at
`tool_executor.rs:505`, and the model's own reading of an error-marked result —
treats a total failure as a completed fan-out. The parent proceeds as though ten
subtasks were done.
**Confidence:** certain (read the code and the contrasting single-spawn path).

## T-33 · search_memory reports "no memory entries found" when the caller asked for zero · low
**Where:** `search_memory.rs:63-66`, `:90-97`
**Defect:** `limit` is clamped with `.min(30)` but has no floor. `limit: 0`
produces an empty result set, which is then rendered as
`"No memory entries found for this query."`
**Failure scenario:** the model probes with `limit: 0` (or a provider emits 0 for
an unset field) and concludes the memory store has nothing on the topic, then
asserts to the user that it has no record of a preference it does in fact hold.
Style/low, but it is a false statement rather than an empty one.
**Confidence:** certain (read the code).

## T-34 · propose_memory does not clamp the model-supplied confidence into [0,1] · low
**Where:** `propose_memory.rs:106`
**Defect:** `input["confidence"].as_f64().unwrap_or(0.8) as f32` is passed
straight to the gateway as `claimed_confidence` with no range check. The tool's
own comment stresses that a role's *ceiling* is enforced; nothing here rejects a
negative value, and the schema's "0.0–1.0" is advisory.
**Failure scenario:** `propose_memory{"content":"…","confidence":-1}`. If the
gateway caps with a `min` against the role ceiling, `-1` passes through
unchanged and lands in the `confidence` column, where the recall path orders by
confidence — the memory sorts below everything and is effectively invisible while
the tool reports "Stored". Needs the gateway's clamp read to confirm which way it
goes.
**Confidence:** needs a run / a read of `memory::gateway::propose` to confirm the
downstream clamp.

> **T-34 WITHDRAWN after checking the downstream.** `memory/gateway.rs:222` does
> `p.claimed_confidence.clamp(0.0, ceiling)`, so a negative or oversized claim is
> clamped correctly. The missing check in `propose_memory.rs` is harmless. Left
> in the report so the reasoning is auditable; excluded from the summary table.
## T-35 · "Pass null to unassign" does nothing, and re-creates the exact half-assigned state the guard above it exists to prevent · high
**Where:** `update_task_status_tool.rs:46-49` (schema), `:70`, `:87-93` (guard),
`:207-214`; `db/tasks.rs:664-678`
**Defect:** the schema tells the model `assigned_worker`: *"Pass null to
unassign."* `update_status_with_reason` branches on `Option`: when
`assigned_worker` is `None` it runs an UPDATE that **omits the
`assigned_worker` column entirely**, so the previous worker stays on the row.
There is no code path anywhere that clears `assigned_worker`.
**Failure scenario:** task #12 is `assigned` to `worker-backend` and needs to go
back to the ready queue. The supervisor does exactly what the schema says:
`update_task_status{"id":12,"status":"ready","assigned_worker":null}`. The
`status=='ready' && assigned_worker.is_some()` guard at :87 does not fire
(it is `None`), the update writes `status='ready'` and leaves
`assigned_worker='worker-backend'`. The board now holds precisely the state the
comment at :73-86 says the guard exists to prevent — "stuck in the READY column
tagged with a worker while workers ignore it because status != 'assigned'" — and
the tool returns success. The guard blocks the explicit spelling and the DB layer
reintroduces it through the implicit one.
**Confidence:** certain (read the tool and the DB method).

## T-36 · `status` is not validated against its own enum, and the DB layer writes whatever it is given · high
**Where:** `update_task_status_tool.rs:65-68`, `db/tasks.rs:649-679`
**Defect:** `status` is accepted as any non-empty string. The schema declares a
nine-value enum; nothing checks it. `update_status_with_reason` interpolates only
the `completed_at` expression and binds `status` as a parameter — no whitelist, no
CHECK constraint consulted in this path.
**Failure scenario:** the model writes `update_task_status{"id":12,"status":"complete"}`
(a natural synonym for `done`). Every guard is skipped: not `ready`, not
`assigned`/`in_progress`, not `rejected`, not `qc_review`. The row is written with
`status='complete'`, `completed_at=NULL`, no scheduler kick and no lifecycle hook.
The task now matches no board query — it disappears from the ready queue, from
"in progress", and from "Done Today" — while the tool reported success. Recovery
requires editing the database by hand.
**Confidence:** certain (read both layers).

## T-37 · The build-evidence gate is satisfied by any command whose TEXT contains a build phrase · high
**Where:** `build_evidence.rs:99-163` (`classify`), `:168-184` (`record`),
consumed at `update_task_status_tool.rs:141-193`
**Defect:** `classify` lowercases the command string and tests `contains` against
phrase lists. It matches the *text of the command*, never what ran. Any exit-0
command containing one of those substrings is stored as passing build evidence,
and `exit_code_is_attributable` only looks for `|`, `;`, `&&`, `||`, newline — so
a simple command with a build phrase inside a quoted argument counts as fully
attributable.
**Failure scenario (accidental, no bad intent required):** a worker is blocked by
the gate and searches the repo for how the build is invoked:
`bash{"command":"grep -rn \"npm run build\" package.json"}`. `grep` finds the
line and exits 0. `classify` sees "npm run build" -> `Node`;
`exit_code_is_attributable` sees no pipeline -> `true`. The next
`update_task_status(id,'qc_review')` passes the gate with zero builds run. The
gate's own refusal message (:160-170) quotes the exact phrases that count, so a
model reading it is being told which strings unlock the door.
**Failure scenario (direct):** `bash{"command":"echo cargo test"}` — exit 0,
classified `Cargo`, attributable. Gate open.
**Confidence:** certain (read `classify`, `record` and the gate).

## T-38 · annotate_file takes a file path and never jails it; `severity` is unvalidated and a "required" field silently defaults · medium
**Where:** `annotate_file_tool.rs:62-100`
**Defect:** `file_path` is stored verbatim in `annotations.db` with no
`validate_workspace_path` call — the only path-taking tool in this directory with
no jail hop at all. Separately, `severity` is declared `required` with a
three-value enum, yet :81 reads it with `.unwrap_or("info")`, so an absent or
misspelled severity is accepted and persisted.
**Failure scenario A:** `annotate_file{"file_path":"C:/Users/Razi/.ssh/config", …}`
writes a row the IDE will later try to resolve and render; the annotation store
accumulates rows pointing outside the project, and any consumer that opens
`file_path` to render gutter decorations is handed an arbitrary absolute path.
**Failure scenario B:** `severity:"critical"` is stored as-is. The IDE renders
gutter decorations by severity; an unknown value falls through whatever `match`
it uses, so a blocking QC finding displays as nothing at all while the tool
reported success.
**Confidence:** certain for both reads; the IDE rendering consequence is inferred
from the stored value being free-form.
## T-39 · e2e_probe is a second, unguarded shell — every bash guard is absent from it · high
**Where:** `e2e_probe.rs:373-405` (`sh -c` / `cmd /C` on model-supplied strings)
**Defect:** `start_cmds` is an array of "bash one-liners" spawned through the same
shells `bash.rs` uses, in the same `working_dir`, under the same `Execute`
effect — but with **none** of `bash.rs`'s checks: no `check_redirect_targets`, no
heredoc scan, no `$`/tilde refusal, no per-command timeout beyond the shared
probe timeout. It does inherit the credential scrub (:391-396) and, unlike
`bash.rs`, it does set `kill_on_drop(true)` (:418).
**Failure scenario:** `bash{"command":"tar czf - . > /tmp/repo.tgz"}` is refused
by `check_redirect_targets` ("target '/tmp/repo.tgz' is outside the workspace").
The identical string as
`e2e_probe{"start_cmds":["tar czf - . > /tmp/repo.tgz"],"probe_url":"http://localhost:9/","expected_text":"x"}`
runs to completion during the 8-second boot wait. The probe then reports FAIL,
which reads as a harmless smoke-test failure, and the write has already happened.
Any refusal `bash` issues is one `e2e_probe` call away from being executed.
**Confidence:** certain (read both spawn paths).

## T-40 · e2e_probe validates the URL by one DNS lookup and connects with another · medium
**Where:** `e2e_probe.rs:181-224` (`validate_probe_url`), `:499-533` (`http_probe`)
**Defect:** `validate_probe_url` calls `to_socket_addrs` and requires every
resolved address to be loopback; `reqwest` then performs its **own** resolution
when the request is sent. The comment at :195-198 asserts "`to_socket_addrs`
performs the same lookup the HTTP client will" — it performs *an equivalent*
lookup, at a different time. A DNS name with a 0-second TTL that answers
`127.0.0.1` to the first query and a routable address to the second passes the
guard and is then connected to.
**Failure scenario:** a prompt-injected model calls
`e2e_probe{"probe_url":"http://rebind.attacker.example/latest/meta-data/…", …}`.
Validation resolves to 127.0.0.1 and passes; reqwest resolves to the metadata
address and fetches it; up to 400 characters of the response come back in
`response_body_excerpt` (:474). This is the same readback `P3-68` closed,
through the resolution gap rather than the string check.
**Confidence:** certain that two independent resolutions occur; exploiting it
needs attacker-controlled DNS, so the end-to-end scenario needs a run to confirm.

## T-41 · e2e_probe reports "the probe URL did not respond" for a boot wait it never got past · low
**Where:** `e2e_probe.rs:216-222` (`boot_wait_s`), `:230-232` (`timeout_s`), `:253-257`
**Defect:** neither value is validated and they are not checked against each
other. `run_probe` sleeps for `boot_wait` *inside* the `timeout(total_timeout, …)`.
When `boot_wait_s >= timeout_s` the HTTP probe never runs.
**Failure scenario:** `e2e_probe{"boot_wait_s":60, …}` with `timeout_s` left at its
default 30. After 30 s the tool returns "e2e_probe timed out after 30s — probe URL
'…' did not respond with '…' before the deadline." No request was ever issued.
The model concludes the server is broken and starts debugging a healthy app.
**Confidence:** certain (read the code).

## T-42 · 27 tool structs in graph_tools.rs implement nothing and are registered nowhere; graph_prompts.rs instructs the model to call them · medium
**Where:** `graph_tools.rs` (27 `*Tool` structs, none with `impl Tool for`),
`graph_prompts.rs:1-78`, declared at `mod.rs:13-14`, registered in neither
`register_for_mode` nor `register_for_mcp_worker`
**Defect:** the modules compile and are declared, so the reachability gate is
green — `src-tauri/src/reachability_gate.rs:54-58` says so explicitly ("Compiling
is not wiring… nothing registers them, so the agent has no graph tools"). Nothing
has changed. Meanwhile `graph_prompts.rs` contains five workflow prompts whose
text instructs an agent to call `detect_graph_changes`, `get_impact_radius`,
`get_review_context`, `list_tests`, `semantic_search_nodes`, `query_graph`,
`find_graph_path`, `get_affected_flows`, `get_god_nodes` and `explain_node` — not
one of which is a registered tool name.
**Failure scenario:** if any of these prompts is ever served (they are `pub fn`
returning MCP prompt JSON), the model receives a five-step procedure in which
every step names a tool that does not exist, and spends a turn per step getting
"unknown tool" before improvising. Today they are dead: 27 KB of tool-shaped code
in the tools directory that a reader will take for available capability.
`graph_tools.rs:45,257,394` also carry `partial_cmp(..).unwrap()` float sorts that
would panic on a NaN score — latent, because nothing can call them.
**Confidence:** certain (grepped the whole repo for registration and for
`impl Tool for` in that file).

## T-43 · validate.rs still hand-rolls the truncation the shared helper exists for, and truncates one character early · low
**Where:** `validate.rs:1036-1050`
**Defect:** `security/truncate.rs` was created (its header says so) to be the one
place this happens, and `bash.rs`, `read_file.rs`, `grep.rs`, `list_memories.rs`,
`annotate_file_tool.rs` and `e2e_probe.rs` all call it. This site still computes
`safe_end` by hand as the index of the last character *starting* before
`MAX_CHECKER_BYTES`, then slices `&combined[..safe_end]` — which excludes that
character. The `unwrap_or(MAX_CHECKER_BYTES)` fallback would be the exact unsafe
slice, but is unreachable (the branch is only entered when the string is longer
than the cap, so index 0 always qualifies).
**Failure scenario:** one character of checker output is dropped at the boundary.
Style/duplication finding, not a live fault.
**Confidence:** certain (read the code).

## T-44 · `type_checker` is accepted and then silently ignored for several extensions · low
**Where:** `validate.rs:901-921`, surfaced through `check_file.rs:80-87,:146-149`
**Defect:** the match arms are `("py", Some("mypy"))`, `("py", _)`,
`("ts"|"tsx", _)`, `("js"|"jsx", Some("eslint")) | ("js"|"jsx", None)`, else
`did_not_run`. So `type_checker:"eslint"` on a `.ts` file silently runs `tsc`;
`type_checker:"tsc"` on a `.js` file falls to the catch-all and reports
"No type checker is configured for .js files — NOT checked", which is false —
eslint is configured for `.js`, the caller just named a different one.
**Failure scenario:** `check_file{"path":"src/app.js","checks":["types"],"type_checker":"tsc"}`
returns NOT CHECKED with a message blaming the file extension. The agent concludes
`.js` files cannot be type-checked at all and stops asking.
**Confidence:** certain (read the match).

## T-45 · check_file's own header comment still claims it resolves an interpreter from inside the inspected workspace · low (doc rot)
**Where:** `check_file.rs:22-27` vs `validate.rs:1095-1105`
**Defect:** the effect comment says the tool "runs external checkers, including
one it resolves from **inside the workspace under inspection** (`P3-19`), so a
cloned repository can supply the binary." `resolve_python` now returns a bare
`"python"`/`"python3"`. The comment describes the pre-fix behaviour as current.
**Failure scenario:** none at runtime. It is the failure mode this codebase keeps
recording — a comment asserting the opposite of the code — pointed the other way.
**Confidence:** certain (read both).
## T-46 · write_interface_spec silently downgrades merge to overwrite when the read fails · high
**Where:** `write_interface_spec.rs:162-166`, `:168-180`
**Defect:**
```rust
let existing = if mode == "merge" && path.exists() {
    std::fs::read_to_string(&path).unwrap_or_default()
} else { String::new() };
let final_body = if mode == "merge" && !existing.is_empty() { <append> } else { <fresh file> };
```
A failed read — a lock held by an editor, a permissions error, a transient I/O
fault — yields `String::new()`, which routes control into the **overwrite**
branch. The success message still says `(mode=merge)`.
**Failure scenario:** INTERFACE.md is open in an editor holding a share lock (or
the file is momentarily unreadable). `write_interface_spec{"routes":[…],"mode":"merge"}`
reads nothing, rebuilds the file from the header plus only the new routes, and
reports "Wrote 2 route(s) to '…' (mode=merge)". Every previously-declared route
is gone. `tier2_6_api_contract` then starts failing every frontend file that uses
a now-undeclared URL, and the frontend worker is told its correct URLs are wrong.
**Confidence:** certain (read the code).

## T-47 · write_interface_spec does a read-modify-write with no path lock, no checkpoint and no hook · high
**Where:** `write_interface_spec.rs:155-215`
**Defect:** it is the fourth mutating file writer in this directory and the only
one that skips all three of the shared mechanisms: it never calls
`path_lock::acquire_for` (which `write_file.rs:98`, `edit_file.rs:106` and
`append_file.rs:88` all do, and which `mod.rs:48-52` documents as the process-wide
per-file registry), never calls `auto_checkpoint_before_write`, and never fires
`PostFileWrite`. In `merge` mode the read and the write straddle no lock at all.
**Failure scenario:** `worker-backend` and `worker-api` both finish routes in the
same tick and both call `write_interface_spec(mode:"merge")`. Both read the same
`existing` string, both append their own routes, both write. Whichever writes
second wins and the other worker's routes vanish — the classic lost update the
path lock exists to prevent. The tool reports success to both. Separately, the
default `mode` is `overwrite`, so a single mistaken call destroys the file with no
checkpoint to restore from.
**Confidence:** certain (read the code and all three comparison sites).

## T-48 · create_task hardcodes `created_by: "pa"` although it is registered for every fleet worker · medium
**Where:** `create_task.rs:59` (`_ctx` deliberately unused), `:89`; registration at
`mod.rs:160` (MCP workers) and `mod.rs:209` / `:270` (PA modes)
**Defect:** `ToolContext.agent_role` is threaded specifically so tools can
attribute their writes (`tool_trait.rs:70-75`). This tool binds the context to
`_ctx` and writes the literal `"pa"` as the creator for every caller.
**Failure scenario:** `worker-backend` calls `create_task` (it is registered for
MCP workers). The board shows the task as created by the personal assistant.
Any later triage that asks "which worker is generating this backlog", and the
supervisor's own decompose reconciliation, attribute it to the user.
**Confidence:** certain (read the code and the registration sites).

## T-49 · create_task accepts out-of-enum `priority` and `task_type`, and counts title length in bytes · medium
**Where:** `create_task.rs:64-69`
**Defect:** `priority` and `task_type` are declared `required` with enums;
the code reads them with `.unwrap_or("p2")` / `.unwrap_or("feature")` and never
checks the value against the enum, then writes it straight to the DB. Separately,
`title.len() > 120` measures **bytes** while the schema says "max 120 chars".
**Failure scenario A:** `create_task{"title":"…","priority":"urgent","task_type":"spike","acceptance_criteria":"…"}`
is stored with `priority='urgent'`. Any board query or UI column that groups by
the four known priorities drops the task, and the supervisor's
"highest-priority-first" ordering has no bucket for it.
**Failure scenario B:** a 45-character Japanese title is 135 bytes and is rejected
with "title must be 120 characters or less" — a limit the model has not exceeded
and cannot reason about.
**Confidence:** certain (read the code).

## T-50 · assign_task discards the routing-history write that suggest_worker's "historical" method depends on · medium
**Where:** `assign_task.rs:118` — `let _ = crate::routing::TaskRouter::record_assignment(&task, &worker_slug);`
**Defect:** the one call that feeds the affinity scorer is discarded with `let _`
and no logging on the error branch. `suggest_worker`'s description advertises a
`method` of `'static' | 'historical' | 'default'`.
**Failure scenario:** the `routing_history` write fails persistently (schema
drift, a locked DB, a missing table after a migration). Every assignment is still
reported as "✅ Task #N assigned", no warning is logged, and `suggest_worker` never
returns `method: "historical"` because the history table stays empty. The
data-driven routing the supervisor recipe is built around is silently inert, and
the only visible symptom is that `method` is always `"static"`.
**Confidence:** certain that the result is discarded silently; whether the write
currently fails needs a run.
## T-51 · The QC reviewer is spawned with `tools: Vec::new() // MCP-only` and therefore gets bash, write_file and edit_file · high
**Where:** `spawn_qc_review.rs:187`, resolved through `native_runner.rs:315` ->
`engine.rs:104-107` -> `tool_registry.rs:42-47`
**Defect:** the same empty-subset no-op as T-30, at the site where it matters
most. `restrict_to(&[])` returns without filtering, so the reviewer inherits the
parent registry in full. The comment says "MCP-only". Compounding it, the config
sets `role: "qc-agent"` (:184), and `update_task_status_tool.rs:330-332`
(`promotes_on_behalf_of_others`) exempts any role starting with `qc-` from the
build-evidence gate.
**Failure scenario:** a reviewer is dispatched for task #12 to check acceptance
criteria. It has `edit_file` and `bash`, so when it finds a failing criterion the
cheapest way to satisfy its instructions is to fix the code itself — then move
the task to `done` with `update_task_status`, which for a `qc-` role skips the
build-evidence check entirely. The author and the reviewer of that change are the
same agent, and no build had to pass. The dispatcher's own comment says the
reviewer is restricted; nothing restricts it.
**Confidence:** certain (read all four hops plus the role exemption).

## T-52 · analyze_diff uploads an unbounded patch to the provider on the user's key · high
**Where:** `analyze_diff_tool.rs:108-113` (`branch_diff`), `:114` (`analyzer.analyze(&patch)`)
**Defect:** the patch produced by `git::diff::branch_diff` is passed to the LLM
with no size check, no truncation and no file filter. Every other tool in this
directory bounds what it emits (`read_file` 256 KB, `grep` 2 KB/line + 256 KB
total, `bash` 50 KB, `e2e_probe` 4 KB/child); this one bounds nothing on the way
OUT of the machine, which is the direction the tool's own effect comment
(:16-20) calls the point of the `Network` reclassification.
**Failure scenario:** `analyze_diff` on a branch that regenerated
`package-lock.json` and `Cargo.lock`. The patch is several megabytes; the whole
thing is sent as one prompt on the user's BYOK key. Either the provider rejects
it for exceeding the context window AFTER billing the input, or it is accepted
and a single QC call costs several dollars. The tool then returns "Diff analysis
failed: <provider error>" with no hint that the input was too large.
**Confidence:** certain that no bound exists; the exact provider behaviour on
oversize input needs a run.
## T-53 · planner lets the caller name which worker is claiming a step · medium
**Where:** `planner.rs:47` (`_ctx` unused), `:95` / `:124` (`claimItem` requires a
model-supplied `worker_id`), `:126` (`submitForQC` likewise)
**Defect:** `ToolContext.agent_role` is threaded so a tool never has to ask the
model who it is — `claim_task_tool.rs:71-102` and `propose_memory.rs:92-104` both
say so explicitly, the latter recording that reading identity from the input was
the exact defect it was rewritten to remove. `planner` binds the context to
`_ctx` and takes `worker_id` from the YAML the model wrote.
**Failure scenario:** `worker-frontend` sends
`action: claimItem / plan_id: 3 / worker_id: worker-backend`. Step 4 of plan 3 is
recorded as claimed by `worker-backend`, which never asked for it.
`worker-backend`'s own tick then finds no unblocked step and idles, while the
plan's audit trail names the wrong owner for work `worker-frontend` is doing. The
same input shape lets any worker `submitForQC` in another worker's name.
**Confidence:** certain (read the dispatch and the action contract).

## T-54 · assign_tasks_batch fires no lifecycle hook, so batched assignments are invisible to every hook consumer · medium
**Where:** `assign_tasks_batch.rs:183-215` vs `assign_task.rs:139-166`
**Defect:** the single-assignment tool builds a `HookContext` and fires
`HookType::OnTaskClaim`. The batch tool performs the identical DB update and UI
event and fires nothing — a grep for `HookType::` across the directory returns no
hit in `assign_tasks_batch.rs`.
**Failure scenario:** the supervisor dispatches its whole ready queue with one
`assign_tasks_batch` call (the reason the tool exists). Telemetry, the fleet
event bus, and any user-configured `OnTaskClaim` hook record ZERO assignments for
that tick. A dashboard counting claims shows an idle fleet while eight workers
start work.
**Confidence:** certain (read both and grepped the directory).

## T-55 · send_worker_message aborts a broadcast midway and reports total failure after partial delivery · medium
**Where:** `send_worker_message.rs:190-200`
**Defect:** the broadcast loop returns `ToolResult::error` on the first insert
failure, after earlier targets have already been inserted, had their Tauri event
emitted and been `scheduler::kick`ed. There is no transaction and no rollback.
**Failure scenario:** the supervisor broadcasts a stop-work notice to eight
recipients. The insert for recipient #4 fails (locked DB). Recipients 1-3 have
the message and have been woken; the tool returns "Failed to persist message to
'worker-data': …". The model reasonably retries the broadcast, and recipients 1-3
receive and act on the notice twice while 5-8 never received it at all.
**Confidence:** certain (read the loop).
## T-56 · spawn_qc_review discards every `release_qc_claim`, so a failed release strands the task under review permanently · medium
**Where:** `spawn_qc_review.rs:203`, `:266`, `:295` — all `let _ = ...release_qc_claim(...)`
**Defect:** the claim taken by `try_claim_for_qc_review` (:145) is released only
by these three discarded calls. `execute` itself refuses any task where
`under_review_by.is_some()` (:126-131). A release that fails is neither retried
nor logged at any level.
**Failure scenario:** the reviewer for task #12 finishes while the DB is briefly
locked. `release_qc_claim` returns `Err`, discarded silently. `under_review_by`
stays set forever. Every subsequent dispatcher tick calls `spawn_qc_review` for
#12 and gets "task #12 is already under review by 'qc-rev-task12-…'". The task
never leaves `qc_review`. Two of the three sites (:266, :295) sit inside a
detached `tokio::spawn`, so nothing surfaces to the model or the user. A
"stranded-reviewer reconciler" is referenced in the comments; whether it covers
this case needs its code read.
**Confidence:** certain that the results are discarded; the reconciler's coverage
is unverified.

## T-57 · Six files here document a Python sidecar / MCP tool_adapter / "Mode 2" architecture that no longer exists · medium (architecture clash)
**Where:** `mod.rs:1-4`, `mod.rs:48-52`, `path_lock.rs:40-41`, `path_jail.rs:3-18`,
`claim_task_tool.rs:71-88`, `spawn_subagent.rs:112`, `spawn_qc_review.rs:93`
**Defect:** `mcp_server::tool_adapter` is deleted — no Rust file in the workspace
defines it, and `tool_trait.rs:83` plus `native_runner.rs:11` describe it in the
past tense — yet `path_lock.rs`'s design note still names it as one of the four
callers that make the lock process-wide. `mod.rs:3` still says "Worker mode: tools
are forwarded to core-orchestrator with `__autodev` suffix";
`path_jail.rs:15-18` still says the jail "must remain in place even after the
Python MCPBridge starts forwarding `__working_dir` correctly";
`claim_task_tool.rs:76-79` still explains a fallback in terms of "the Python
bridge" not forwarding `__agent_role`. `recipes/defaults.rs` states five times
that the current path "needs no sidecar and no Python".
**Failure scenario:** not a runtime fault — a maintenance one, and the one this
codebase keeps paying for. A reader deciding whether `write_interface_spec` needs
the path lock (T-47) reads `path_lock.rs`'s list of callers, sees a fourth entry
that cannot be checked because the module does not exist, and treats the list as
authoritative. With the adapter gone the registry covers exactly three tools, and
the fourth writer is not one of them.
**Confidence:** certain (grepped the whole repo for the module and the suffix).

## T-58 · resolve_worker_request records no resolver identity · low
**Where:** `resolve_worker_request_tool.rs:57` (`_ctx` unused), `:76-79`
**Defect:** the tool writes `status` and an optional free-text `note` and never
records who resolved the request, although `ctx.agent_role` is available. It is
registered for every MCP worker (`mod.rs:169`), not only the supervisor.
**Failure scenario:** a worker resolves another worker's escalation as
`acknowledged`; the queue drains and the supervisor never sees it. Nothing in the
row says which agent did that. Auditability gap rather than a wrong output.
**Confidence:** certain (read the code).

---

## Summary

58 findings, one withdrawn after verification (T-34). Severity-ordered.

| # | Severity | File | One line |
|---|---|---|---|
| T-1 | critical | bash.rs:105-118,514-547 | POSIX->PowerShell rewriter still feeds `cmd /C`; a passing build is returned as an error AND recorded as a failed build |
| T-5 | critical | bash.rs:142-151,234-263 | No `kill_on_drop`, no process-group kill: a timed-out `cargo build` is orphaned and poisons every later build in the session |
| T-17 | critical | edit_file.rs:151-157 | `old_string: ""` -> infinite loop, unbounded allocation, per-path lock never released |
| T-18 | critical | edit_file.rs:165-169 | `old_string: ""` + `replace_all` inserts the replacement at every character boundary and reports success |
| T-2 | high | bash.rs:542-543 | `" grep "` replace runs before `" grep -i "`, making the second branch dead and emitting an invalid cmdlet flag |
| T-3 | high | bash.rs:535-539 | `head -`/`tail -` replaced on the prefix, so `tail -f`/`head -c` become nonsense |
| T-4 | high | bash.rs:211-223 | The exit-255 self-correction hint can never fire; `cmd` uses 9009/1 and `sh` uses 127 |
| T-7 | high | bash.rs:385-394 | `> /dev/null` (and every absolute target) is refused; on Windows it resolves to `C:\dev\null` first |
| T-8 | high | bash.rs:340-368 | The `$`-refusal treats any `>` as a redirect, so `awk '$3 > $2'` is refused as an unresolvable path |
| T-9 | high | bash.rs:115-118 | `cmd /C` gets MSVCRT backslash-escaped quotes it does not understand; embedded quotes are corrupted and can exit 0 |
| T-11 | high | bash.rs:27-32 | Description says "the shell is Windows CMD" and "use PowerShell-compatible alternatives" in one paragraph, and omits every refusal path |
| T-23 | high | grep.rs:335-352 | The 256 KB total bound stops trimming at one file result, so matches concentrated in one file return unbounded |
| T-30 | high | spawn_subagent.rs:52-56 | Schema says empty `tools` = "MCP tools only"; `restrict_to(&[])` is a no-op, so the child gets bash and write_file |
| T-35 | high | update_task_status_tool.rs:46-49 | "Pass null to unassign" never clears the column, recreating the half-assigned state the guard above it blocks |
| T-36 | high | update_task_status_tool.rs:65-68 | `status` is not checked against its enum; an invented status is written and the task leaves every board query |
| T-37 | high | build_evidence.rs:99-163 | The gate matches command TEXT, so `grep -rn "npm run build" package.json` counts as a passing build |
| T-39 | high | e2e_probe.rs:373-405 | `start_cmds` is a second shell with none of bash's checks — every bash refusal is one e2e_probe call away from running |
| T-46 | high | write_interface_spec.rs:162-180 | A failed read turns `mode:"merge"` into an overwrite while still reporting `(mode=merge)` |
| T-47 | high | write_interface_spec.rs:155-215 | Read-modify-write with no path lock, no checkpoint and no hook — the lost update the lock exists to prevent |
| T-51 | high | spawn_qc_review.rs:187 | `tools: Vec::new() // MCP-only` gives the QC reviewer bash and edit_file, and its `qc-` role skips the build gate |
| T-52 | high | analyze_diff_tool.rs:108-114 | The whole branch diff is uploaded to the provider unbounded, on the user's BYOK key |
| T-6 | medium | bash.rs:57-59 | `timeout_secs: 0` fails every command instantly; a JSON float silently reverts to 120 |
| T-10 | medium | bash.rs:105-113,150,158 | Build evidence and the timeout message record the rewritten command, not the model's |
| T-13 | medium | bash.rs:136-140 | `NPM_TOKEN`/`GITHUB_TOKEN` are stripped unconditionally, so a private-registry `npm ci` fails and is recorded as a failed build |
| T-19 | medium | edit_file.rs:220-225 | edit_file runs Tier 1 only; write_file runs four tiers, and edit_file is the normal path for existing files |
| T-20 | medium | append_file.rs (whole execute) | The only mutating file tool with no checkpoint and no PostFileWrite hook |
| T-21 | medium | read_file.rs:125-126,146-147 | A non-integer `start_line` is silently dropped, producing "pass start_line" advice the model has already followed |
| T-25 | medium | grep.rs:89-95,241-244 | "Skips binary files" is only "skips invalid UTF-8"; a 2 GB UTF-8 file is read into memory whole |
| T-26 | medium | glob.rs:200-219 | `total_files` is the returned count, there is no `truncated` flag in the JSON, and sorting happens after the cap |
| T-27 | medium | list_directory.rs:60-63,163,189 | Follows symlinks, so a junction inside the workspace lists the user's home directory past the jail |
| T-28 | medium | check_file.rs:127-144,259-274 | An empty or misspelled `checks` array returns SUCCESS after running zero checkers |
| T-31 | medium | spawn_subagent.rs:112, spawn_qc_review.rs:93 | Both tell the model to wait for a Python sidecar that was retired |
| T-32 | medium | spawn_parallel_subagents.rs:233-288 | Always returns `is_error = false`, even when all ten subagents failed |
| T-38 | medium | annotate_file_tool.rs:62-100 | The only path-taking tool with no jail; `severity` is `required` in the schema but defaults silently |
| T-40 | medium | e2e_probe.rs:181-224,499-533 | The loopback check and the connection use two separate DNS resolutions |
| T-42 | medium | graph_tools.rs, graph_prompts.rs | 27 tool structs implement nothing and are registered nowhere; the prompts instruct the model to call them by name |
| T-48 | medium | create_task.rs:59,89 | Hardcodes `created_by: "pa"` although every fleet worker can call it |
| T-49 | medium | create_task.rs:64-69 | `priority`/`task_type` bypass their enums; the title limit counts bytes, not characters |
| T-50 | medium | assign_task.rs:118 | The routing-history write that `suggest_worker`'s "historical" method depends on is discarded with `let _` and no log |
| T-53 | medium | planner.rs:47,124,126 | `claimItem`/`submitForQC` take `worker_id` from the model instead of `ctx.agent_role` |
| T-54 | medium | assign_tasks_batch.rs:183-215 | Fires no `OnTaskClaim`, so a whole batched dispatch is invisible to telemetry and hooks |
| T-55 | medium | send_worker_message.rs:190-200 | A broadcast that fails partway reports total failure after partial delivery, so a retry double-delivers |
| T-56 | medium | spawn_qc_review.rs:203,266,295 | Every `release_qc_claim` is discarded; one failure strands the task under review forever |
| T-57 | medium | mod.rs, path_lock.rs, path_jail.rs, claim_task_tool.rs | Six files document a deleted MCP tool_adapter / Python sidecar / "Mode 2" architecture as current |
| T-14 | low | bash.rs:493 | `bytes[i] as char` rebuilds the command as Latin-1, corrupting UTF-8 in the re-scanned copy |
| T-15 | low | bash.rs:160-206 | stdout/stderr causal order is lost and the `=== STDOUT ===` header is the first casualty of truncation |
| T-24 | low | grep.rs:343-344 | `json_output[..N]` raw byte slice in a near-unreachable fallback — the exact P3-04 shape |
| T-33 | low | search_memory.rs:63-66 | `limit: 0` reports "No memory entries found" rather than "you asked for none" |
| T-41 | low | e2e_probe.rs:216-232 | `boot_wait_s >= timeout_s` reports "the probe URL did not respond" for a request never issued |
| T-44 | low | validate.rs:901-921 | `type_checker` is silently ignored for `.ts` (eslint->tsc) and mis-blamed for `.js` (tsc->"not configured") |
| T-58 | low | resolve_worker_request_tool.rs:57 | No resolver identity is recorded although `ctx.agent_role` is available |
| T-12 | low (style) | bash.rs:86-92 | The "workspace path-jail" compares `working_dir` with itself; three sibling comments still call it load-bearing |
| T-16 | low (style) | build_evidence.rs:168-201 | The evidence ring grows without bound and is only swept on read |
| T-22 | low (style) | read_file.rs:31,129 | Two different constants (262144 and 256000) are both called "256 KB" |
| T-29 | low (style) | check_file.rs:251-255 | `strip_prefix` of a raw working_dir from a canonical path fails, so absolute paths are shown |
| T-43 | low (style) | validate.rs:1036-1050 | Hand-rolled truncation that `security/truncate` exists to replace, cutting one character early |
| T-45 | low (doc rot) | check_file.rs:22-27 | Claims it resolves an interpreter from inside the inspected workspace; `resolve_python` returns a bare name |
| T-34 | withdrawn | propose_memory.rs:106 | Confidence is clamped correctly downstream at `memory/gateway.rs:222` |

### Method notes

* Every finding above was read out of the source; no capability was inferred from
  a name, and no comment was accepted as evidence. Where a comment made a claim,
  the claim was checked against the code it describes (`pool.acquire`'s depth
  branch, `resolve_python`, `restrict_to`, `clamp`, `update_status_with_reason`).
* Three comment claims **checked out** and produced no finding: the subagent
  depth boundary really is decided once in `pool.acquire`
  (`subagent/pool.rs:96`, called first at `manager/spawn:119`); `max_turns == 0`
  and `timeout_secs == 0` really are defaulted in `manager.rs:122-127`; and
  `resolve_python` really does use bare names now.
* One finding was withdrawn on verification (T-34), recorded rather than deleted.

---

# TRIAGE — `P8-33`, completed 2026-09-03

`P8-33`: *"Work the rest to a decision each — fixed, filed with an owning
phase, or dismissed with a reason — and record the disposition in the audit
file."* All **58** findings are dispositioned below. Nothing is left unread.

**Outcome:** **20 fixed** · **2 already fixed before this phase (this document
was stale)** · **2 dismissed with a reason** · **34 filed with a named owning
task**.

⚠ **Two of this document's own four `critical` findings were already closed
when the triage reached them.** `T-17` and `T-18` were fixed in `7ae53ee`, with
two tests, before `P8` began — the audit was written against an earlier tree and
nobody re-read it. *An audit is a photograph too* (`L-15`), and a triage that
had trusted its severity column would have "fixed" a defect twice and reported
progress for it.

⚠ **And one finding was refuted on the facts.** `T-34` says `propose_memory`
does not clamp a model-supplied confidence. The tool does pass the raw value —
and `memory/gateway.rs:222` clamps it: `p.claimed_confidence.clamp(0.0,
ceiling)`. The gateway is the single write path (`TG-P3-39`), so the effect is
bounded; clamping in the tool as well would be `L-19`'s two-lists problem.

## Fixed in `P8-0`

| # | Finding | Closed by |
|---|---|---|
| `T-1` | the POSIX→PowerShell rewriter ran and `cmd` executed its output | `P8-26` — rewriter deleted |
| `T-2` | grep rewrite ordering made the `-i` branch dead | `P8-26` — dies with the rewriter |
| `T-3` | `head -`/`tail -` replaced blind, mangling other flags | `P8-26` — dies with the rewriter |
| `T-4` | the exit-255 self-correction hint was unreachable | `P8-26` — description rewritten to name `findstr`/`more` |
| `T-5` | a timeout abandoned the child instead of killing it | `P8-27` — `kill_on_drop` + process-group kill |
| `T-6` | `timeout_secs` unvalidated: `0` failed every command instantly | **`P8-33`** — clamped to `[1, 1800]` |
| `T-9` | `cmd /C` received MSVCRT-escaped quotes it cannot parse | `P8-26` — no rewriting, and the `sh -c` path is unaffected |
| `T-10` | build evidence recorded against the rewritten command | `P8-26` — there is no rewritten command |
| `T-11` | the tool description contradicted itself | `P8-26` — rewritten, and it now states the shell honestly |
| `T-23` | grep's total-byte bound **could not bind** with matches in one file | **`P8-33`** — matches are dropped inside the last result too |
| `T-24` | the fallback sliced a `String` at a raw byte index | **`P8-33`** — re-serialise, or error; never slice |
| `T-27` | `list_directory` followed symlinks out of the workspace jail | **`P8-33`** — canonicalise and compare, so junctions and `..` are caught too |
| `T-28` | `check_file` returned **SUCCESS having run nothing** | **`P8-33`** — an unrecognised `checks` list is refused |
| `T-30` | `spawn_subagent` said empty `tools` meant "MCP tools only" | `P8-30` — it means INHERIT, and now says so (`ADR-LOG-RESTRUCT-0039`) |
| `T-32` | `spawn_parallel_subagents` reported SUCCESS when **every** subagent failed | **`P8-33`** — all-failed is an error; partial stays success |
| `T-35` | *"Pass null to unassign"* did nothing | `P8-29` — clears the worker and the claim fields |
| `T-36` | `status` was not validated against its own nine-value enum | **`P8-33`** — round-trip check. ⚠ Worse than the finding said: `from_str`'s fallback is `_ => Ready`, so a typo **re-queued** the task rather than hiding it |
| `T-37` | the build-evidence gate was satisfied by command **text** | `P8-28` — a real exit code from a real build |
| `T-46` | `write_interface_spec` silently downgraded merge to **overwrite** | **`P8-33`** — a failed read refuses instead of destroying the file |
| `T-52` | `analyze_diff` uploaded an unbounded patch on the user's key | **`P8-33`** — capped at 256 KB with an actionable message |

## Already fixed — this document was stale

| # | Finding | Evidence |
|---|---|---|
| `T-17` | empty `old_string` spun forever holding the path lock | `7ae53ee`; guard at `edit_file.rs:155`, test `an_empty_old_string_is_refused_rather_than_shredding_the_file` |
| `T-18` | empty `old_string` + `replace_all` shredded the file | same commit and guard; test `the_schema_declares_old_string_non_empty` |

## Dismissed, with the reason

| # | Finding | Why |
|---|---|---|
| `T-12` | the "workspace path-jail" compares `working_dir` with itself | **Correct as written and self-documented at `bash.rs:61-85`.** It is an existence/canonicalisation check, not a jail, and it does not claim otherwise. The jail for file paths is `path_jail.rs`, applied by the seven file tools. Renaming the comment is `P8-60`'s. |
| `T-34` | `propose_memory` does not clamp the claimed confidence | **Refuted at the effect level.** `memory/gateway.rs:222` clamps to `[0, ceiling]` and is the single write path (`TG-P3-39`). A second clamp in the tool is `L-19`. |

## Filed with an owning task

Each row names the task that owns it and why it belongs there rather than in
`P8`. ⚠ None of these is *"later"* — every one has an owner.

| # | Finding | Owner | Why not `P8` |
|---|---|---|---|
| `T-7` | `/dev/null` and any absolute redirect target refused | `P4-16` | Needs the redirect scanner to understand shell grammar rather than bytes; `P4-16` already owns purging POSIX-only shell from recipes and skills |
| `T-8` | the `$`-refusal fires on comparison operators | `P4-16` | Same scanner, same redesign |
| `T-13` | credential scrubbing removes tokens builds legitimately need | `P4-11` | Needs a policy call on which variables a build may see; `P4-11` owns build verification |
| `T-14` | `strip_heredoc_bodies` rebuilds byte-by-byte as `char` | `P4-16` | Same scanner |
| `T-15` | stdout/stderr interleaving destroyed | `P4-11` | Build-output fidelity is what `P4-11` measures |
| `T-16` | the evidence ring grows unbounded, swept only on read | `P4-12` | `P4-12` replaces the in-memory map with a table, which removes the ring |
| `T-19` | `edit_file` runs only Tier 1 validation | `P4-19` | Sits with the checkpoint/validation work |
| `T-20` | `append_file` takes no checkpoint and fires no hook | `P4-19` | Same |
| `T-21` | `read_file`'s range test fails for non-integer JSON numbers | `P4-19` | Same file family |
| `T-22` | two different constants both called "256 KB" | `P4-19` | Same |
| `T-25` | grep claims to skip binary files and has no detection | `P4-19` | Same |
| `T-26` | glob's `total_files` is the returned count, not the match count | `P4-19` | Same |
| `T-29` | `check_file`'s display path degrades to absolute | `P8-60` | A display/comment defect, and `P8-60` is the comment sweep |
| `T-31` | two spawn tools tell the model to wait for a sidecar | **`P8-58`** | Already a task in this phase |
| `T-33` | `search_memory` reports "no entries" for a scope mismatch | `P3` residue → `P5-21` | Tool-description standard work |
| `T-38` | `annotate_file` never jails its path; `severity` unvalidated | `P4-18` | `P4-18` owns "every agent write resolves against the workspace resolver" |
| `T-39` | `e2e_probe` is a second, unguarded shell | `P4-11` | ⚠ Needs a **decision**, not a patch: whether `e2e_probe` should execute shell strings at all. Every `bash` guard is absent, and copying them creates a second copy to keep in step. `P4-11` owns the one build/verify path |
| `T-40` | `e2e_probe` validates by one DNS lookup and connects with another | `P4-11` | Same file, same owner |
| `T-41` | `e2e_probe` misreports a boot wait it never performed | `P4-11` | Same |
| `T-42` | 27 `graph_tools.rs` structs implement nothing and register nothing | **`P5-18`/`P5-19`** | `TG1-01`'s registration clause; explicitly `P5`'s entry condition |
| `T-43` | `validate.rs` hand-rolls the shared truncation helper | `P4-11` | Build-verification file |
| `T-44` | `type_checker` accepted then silently ignored | `P4-11` | Same |
| `T-45` | `check_file`'s header still claims interpreter resolution | `P8-60` | Comment sweep |
| `T-47` | `write_interface_spec` has no path lock, checkpoint or hook | `P4-19` | Needs the shared write plumbing the other three writers use; a fourth hand-rolled copy is the wrong fix |
| `T-48` | `create_task` hardcodes `created_by: "pa"` | `P4-41` | Epic/task creation surface |
| `T-49` | `create_task` accepts out-of-enum `priority` and `task_type` | `P4-02` | Same class as `T-36`; `P4-02` makes an unknown value a loud error at the DB layer |
| `T-50` | `assign_task` discards the routing-history write | **`P4-32`/`P4-25`** | Affinity is exactly what those tasks build |
| `T-51` | the QC reviewer is spawned with `tools: Vec::new() // MCP-only` | **`P8-58`** | The comment is wrong and `P8-58` owns those strings. ⚠ The *semantics* are now correct by `P8-30`: empty means inherit |
| `T-53` | the planner lets the caller name the claiming worker | **`P4-34`** | `P4-34` is "one claim path, and the recipes use it" |
| `T-54` | `assign_tasks_batch` fires no lifecycle hook | `P4-38` | Batch-assignment correctness |
| `T-55` | `send_worker_message` aborts a broadcast midway | **`P4-42`** | `P4-42` owns worker messaging |
| `T-56` | `spawn_qc_review` discards every `release_qc_claim` | `P4-34` | Claim lifecycle |
| `T-57` | six files document a Python sidecar / MCP adapter / "Mode 2" | **`P8-59`/`P8-60`** | Already tasks in this phase |
| `T-58` | `resolve_worker_request` records no resolver identity | `P4-42` | Worker request surface |

## What the triage itself found

1. **An audit is a photograph** (`L-15`). Two of four criticals were closed
   before the triage read them, and this document still presented them as open.
   A severity column is a claim about a past tree.
2. **Three fixes were larger than their finding.** `T-36`'s real behaviour was
   not "invisible task" but "task silently returned to the READY queue", because
   `TaskStatus::from_str`'s fallback arm is `_ => Self::Ready`. `T-23` and
   `T-24` are one site. `T-17`/`T-18` are one guard.
3. **Three of the four findings `P8-33` singles out are FALSE GREENS** —
   `check_file` passing having run nothing, `spawn_parallel_subagents` reporting
   success when everything failed, and grep's bound not binding. That is the
   defect class this programme exists to remove, and it is concentrated in the
   tools the *model* calls rather than in the code a human reads.
