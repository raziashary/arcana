# Adversarial dependency verification — P3-F · Memory

**Brief:** refute, don't confirm. Default to REFUTED on ambiguity.
**Date:** 2026-09-02
**Machine:** Windows 11, `x86_64-pc-windows-msvc`, `rustc 1.94.0 (4a4ef493e 2026-03-02)`, `cargo 1.94.0`.
**Repo pins (read-only, unmodified):**
- `autodev-core/Cargo.toml:27`, `src-shared/Cargo.toml:10`, `src-tauri/Cargo.toml:43` — all three:
  `rusqlite = { version = "0.31", features = ["bundled"] }`
- `Cargo.lock`: `rusqlite 0.31.0`, `libsqlite3-sys 0.28.0`

**Probe:** `…/scratchpad/depprobe/` — a standalone crate (own `[workspace]`, outside the repo).
The repository was not modified in any way.

---

## C-1 · sqlite-vec static link composes with bundled rusqlite · **CONFIRMED**

**Claim:** *"SQLite + `sqlite-vec` + FTS5 behind a retrieval-backend trait"*; *"`sqlite-vec` — Rust crate compiles the C source and statically links it; no extension loading, no runtime dependency"*.

**Evidence:**

Crate exists. `crates.io/api/v1/crates/sqlite-vec`:
- `max_stable_version: 0.1.9` (published 2026-03-31), `newest_version: 0.1.10-alpha.4` (2026-05-18)
- 84 versions, 2,646,759 all-time downloads, owner `asg017` (Alex Garcia — the sqlite-vec author)
- description: `"FFI bindings to the sqlite-vec SQLite extension"`

Dependency set of `sqlite-vec 0.1.9` — note what is *absent*:
```
build cc         ^1.0     (build-dependency)
dev   rusqlite   ^0.31.0  (dev-dependency ONLY — not a normal dependency)
```
`[dependencies]` is **empty**. `features` is `{}` — the crate has no Cargo features at all.

It really does compile C, statically. The whole of `build.rs`:
```rust
fn main() {
    cc::Build::new().file("sqlite-vec.c").define("SQLITE_CORE", None).compile("sqlite_vec0");
}
```
The `.crate` ships `sqlite-vec.c` (320,026 bytes) plus `sqlite3.h` / `sqlite3ext.h`. `-DSQLITE_CORE`
selects the static path in `sqlite-vec.h`:
```c
#ifndef SQLITE_CORE
#include "sqlite3ext.h"     // dynamic loadable-extension path — NOT taken
#else
#include "sqlite3.h"        // static path — taken
#endif
```
So it calls SQLite symbols directly rather than through the `sqlite3_api_routines` trampoline. There is
no `.dll`/`.so` and no `sqlite3_load_extension` call.

`rusqlite`'s `loadable_extension` feature is **not** required and is **not** used. The two are in fact
mutually exclusive by design (that feature is for *building* a Rust loadable extension), but the
question is moot: nothing in this path loads an extension. Registration is via the in-process
auto-extension hook, `rusqlite::ffi::sqlite3_auto_extension`, which `bundled` already exposes.

**Build proof** (`cargo build`, exit 0, 15.72s, MSVC host — full log below):
```
   Compiling cc v1.4.4
   Compiling libsqlite3-sys v0.28.0
   Compiling sqlite-vec v0.1.9
   Compiling rusqlite v0.31.0
   Compiling depprobe v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.72s
```
No warnings about symbol collision; single static SQLite in the binary.

**Runtime proof** (`depprobe.exe`, exit 0):
```
sqlite_version = 3.45.0
vec_version    = v0.1.9
[ OK ] FTS5 create
[ OK ] FTS5 insert
[ OK ] FTS5 match -> "hello world memory"
```
`vec_version()` answering at all proves the statically-linked extension registered against
libsqlite3-sys's bundled SQLite 3.45.0. **FTS5 is present in that same bundled build** — the
third leg of the triple needed no extra feature flag. All three of SQLite + sqlite-vec + FTS5
coexist in one connection.

**Version compatibility:** there is **no semver coupling to `rusqlite` at all.** `rusqlite` appears
only as a dev-dependency of `sqlite-vec`, so it does not participate in the dependency resolution of
a consumer. `sqlite-vec` links against whatever SQLite symbols the final binary exports. It composes
with the pinned `rusqlite 0.31` / `libsqlite3-sys 0.28` — verified by execution above — and is not
blocked from a future `rusqlite` bump.

**Verdict rationale:** every sub-claim held under execution, on the target platform, at the repo's
exact pins. Static link: confirmed by `build.rs` + `-DSQLITE_CORE`. No extension loading: confirmed by
the absence of any load path. No runtime dependency: confirmed — empty `[dependencies]`, no dylib.
No `loadable_extension` conflict: confirmed, that feature is not in play.

**Two warts the plan should nonetheless write down** (neither refutes the claim):

1. **The crate is 6 lines and its FFI signature is deliberately wrong.** All of `src/lib.rs`:
   ```rust
   #[link(name = "sqlite_vec0")]
   extern "C" { pub fn sqlite3_vec_init(); }
   ```
   declared with **zero** parameters, while `sqlite-vec.h` declares
   `int sqlite3_vec_init(sqlite3*, char**, const sqlite3_api_routines*)`. The only supported use is
   therefore a `transmute` through a raw pointer, exactly as the crate's own test does:
   ```rust
   unsafe { sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ()))); }
   ```
   This is the upstream-documented pattern and it works, but it is an `unsafe` fn-pointer
   `transmute` that must be wrapped once, centrally, in the retrieval backend — not sprinkled.

2. **`sqlite3_auto_extension` is process-global, not per-connection.** It registers the extension for
   *every* SQLite connection opened afterwards in the process. Against a design of "one SQLite file
   per subsystem", this means the memory subsystem's initialisation silently changes every other
   subsystem's connections. Harmless in effect (it only adds functions), but it is global mutable
   state established at an arbitrary time, so it must be done once at startup, before any subsystem
   opens its DB — not lazily on first memory use. Ordering bug risk if left implicit.

**If REFUTED, what the plan must do instead:** n/a — confirmed.

---

## C-2 · `vec0` partition keys are real, and are a genuine pre-filter · **CONFIRMED**

**Claim:** *"Scope as a declared `vec0` partition key — not a post-filter over the top-k."*

**Evidence:**

**They exist, and P3-31 is safe.** Partition keys are present in the version that builds here (0.1.9).
Bisected across published versions by grepping the shipped `sqlite-vec.c`:

| version | `PARTITION KEY definition` | `VEC0_MAX_PARTITION_COLUMNS` |
|---|---|---|
| 0.1.4 | 0 | 0 |
| 0.1.5 | 0 | 0 |
| **0.1.6-alpha.1** | **1** | **7** |
| 0.1.6 | 1 | 7 |
| 0.1.7 | 1 | 7 |
| 0.1.9 (used) | 1 | 7 |

**Introduced in `0.1.6-alpha.1`; first stable release with them is `0.1.6`.** The plan's default of the
current stable `0.1.9` clears that floor with room to spare.

**Exact DDL syntax.** From the parser `vec0_parse_partition_key_definition()` (sqlite-vec.c:1998), the
grammar is exactly four tokens:
```
<column_name> {TEXT | INT | INTEGER} PARTITION KEY
```
The parser matches the type token against `"text"` → `SQLITE_TEXT`, or `"int"`/`"integer"` →
`SQLITE_INTEGER`, and returns `SQLITE_EMPTY` (not a partition key) for anything else. Matching is
`sqlite3_strnicmp` — case-insensitive.

Working DDL, executed:
```sql
CREATE VIRTUAL TABLE m USING vec0(
    scope TEXT PARTITION KEY,
    embedding float[4]
);
```

**Documented limits — all three verified by execution, not by reading:**

1. **Maximum 4 partition key columns** (`#define VEC0_MAX_PARTITION_COLUMNS 4`, sqlite-vec.c:3484).
   Probe output:
   ```
   [ OK ] DDL with 2 partition keys
   [ OK ] DDL with 4 partition keys
   [FAIL] DDL with 5 partition keys -> vec0 constructor error: More than 4 partition key columns were provided
   ```
2. **Types restricted to TEXT and INTEGER.** Probe output:
   ```
   [ OK ] partition key type TEXT
   [ OK ] partition key type INTEGER
   [FAIL] partition key type FLOAT -> vec0 constructor error: could not parse vector column 'p FLOAT PARTITION KEY'
   [FAIL] partition key type BLOB  -> vec0 constructor error: could not parse table option 'p BLOB PARTITION KEY'
   ```
   Note the *misleading* error text: an unsupported partition type does not report "bad partition key",
   it falls through and reports `could not parse vector column` / `could not parse table option`. If
   scope is ever modelled as anything but TEXT/INTEGER, the failure will not say so.
3. **Composes with metadata and auxiliary columns.** Verified in one table:
   ```sql
   CREATE VIRTUAL TABLE mix USING vec0(
       scope TEXT PARTITION KEY,   -- partition
       embedding float[4],         -- vector
       kind TEXT,                  -- metadata (filterable)
       +body TEXT                  -- auxiliary (+ prefix, not filterable)
   );
   ```
   ```
   [ OK ] partition + metadata + aux
   [ OK ] mix insert
   [ OK ] mix query -> ("s1", "note", "hello", 0.0)
   ```

**The load-bearing part — it is a PRE-filter, not a post-filter over the top-k.** This is the actual
substance of the claim, and it needed a test that could distinguish the two. Fixture, 3 rows:

| rowid | scope | vector |
|---|---|---|
| 1 | a | `[1,0,0,0]` |
| 2 | b | `[1,0,0,0]` |
| 3 | a | `[0,1,0,0]` |

Query: `WHERE embedding MATCH '[1,0,0,0]' AND k = 2 AND scope = 'a'`.

- **If post-filter:** global top-2 = rowid 1 (d=0) and rowid 2 (d=0, scope b) → filter to `scope='a'`
  → **1 row**.
- **If pre-filter:** top-2 *within partition a* = rowid 1 (d=0) and rowid 3 (d=1.4142) → **2 rows**.

Observed:
```
[ OK ] KNN with partition filter -> 2 rows
         rowid=1 scope=a dist=0.0000
         rowid=3 scope=a dist=1.4142
[ OK ] partition isolation: no rows from other scopes
[ OK ] KNN unfiltered -> scopes ["a", "b", "a"]
```
Two rows, including the far one at d=1.4142 that a post-filter could never have surfaced. **`k` is
honoured within the partition.** The mechanism is visible in the source: partition values are stored
on the *chunk* (`partition00`, `partition01`, … columns in the `_chunks` shadow table,
sqlite-vec.c:4036/4213/4486), so chunks belonging to other partitions are skipped before any distance
is computed. This is real partition pruning.

**Verdict rationale:** partition keys exist in the buildable version, the DDL syntax is as the plan
assumes, and the discriminating test rules out post-filtering. The distinguishing evidence — a result
row at distance 1.4142 that post-filtering cannot produce — is what makes this CONFIRMED rather than
merely "it returned the right scopes".

**Constraints P3-31 must be written against (none fatal, all now measured):**
- at most **4** partition key columns per table
- partition key type is **TEXT or INTEGER only**; a wrong type fails with an error naming the *vector*
  column, not the partition column
- a partition key is declared at `CREATE VIRTUAL TABLE` time and is therefore **schema, not policy** —
  changing the scope model later means a table rebuild and re-embed, not an `ALTER`

**If REFUTED, what the plan must do instead:** n/a — confirmed.

---

## C-3 · fastembed / BGE-small-en-v1.5 / 384 dims / ~130 MB · **CONFIRMED**

**Claim:** *"`fastembed-rs` / BGE-small-en-v1.5, 384 dimensions"*, download *"~130 MB"*.

**Evidence:**

Crate exists: `fastembed`, **current version 6.0.2** (published 2026-08-27), 3,103,828 downloads,
repo `https://github.com/Anush008/fastembed-rs`. The plan's name "fastembed-rs" is the *repository*
name; the **crate name is `fastembed`** — that is what goes in `Cargo.toml`.

The model ships, and is the crate's default. `src/models/text_embedding.rs:28-30`:
```rust
/// BAAI/bge-small-en-v1.5 - Default
#[default]
BGESmallENV15,
```
Its `ModelInfo` (`text_embedding.rs:189-197`) — note `dim`:
```rust
ModelInfo {
    model: EmbeddingModel::BGESmallENV15,
    dim: 384,
    description: String::from("v1.5 release of the fast and default English model"),
    model_code: String::from("Xenova/bge-small-en-v1.5"),
    model_file: String::from("onnx/model.onnx"),
    additional_files: Vec::new(),
    output_key: None,
}
```
**384 dimensions — confirmed at the source, not inferred.** This matches the `float[384]` the vec0
schema will need.

**Download size — confirmed, and the plan's estimate is good.** The weights come from the HF repo
`Xenova/bge-small-en-v1.5`, file `onnx/model.onnx`. HuggingFace API (`?blobs=true`) reports:

| file | size |
|---|---|
| `onnx/model.onnx` | **133.09 MB** |
| `tokenizer.json` | 0.71 MB |
| `vocab.txt` | 0.23 MB |
| `config.json`, `tokenizer_config.json`, `special_tokens_map.json` | <0.01 MB each |
| **total first-run fetch** | **≈ 134 MB** |

The plan's "~130 MB" is accurate (within 3%). Worth knowing: it is the **unquantized fp32** model.
The same repo carries `model_quantized.onnx` at **34.01 MB** and `model_fp16.onnx` at 66.75 MB, and
fastembed exposes the quantized one as a separate enum variant `BGESmallENV15Q` (`dim: 384` also,
`model_code: "Qdrant/bge-small-en-v1.5-onnx-Q"`). If first-run download weight matters for a desktop
app, there is a 4× smaller option at the same dimensionality — a decision the plan has not yet made.

**Verdict rationale:** every element of the claim checked out against the crate source and the model
host: crate exists, model ships and is the default, dimension is exactly 384, size estimate is
within 3% of the true 133 MB.

**If REFUTED, what the plan must do instead:** n/a — confirmed. One naming correction: the dependency
line is `fastembed = "6"`, not `fastembed-rs`.

---

## C-4 · "ort's default features download from a third-party CDN" · **PARTIALLY CONFIRMED — right behaviour, wrong crate**

**Claim:** *"`ort`'s default features download the ONNX runtime from a third-party CDN at build time."*

**Evidence:**

**The behaviour is real, and the CDN is genuinely third-party.** `ort-sys 2.0.0-rc.13` ships a
`build/download/dist.tsv` of 16 prebuilt archives, every one served from **`cdn.pyke.io`**:
```
x86_64-pc-windows-msvc  directml  https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0/x86_64-pc-windows-msvc+directml.tar.lzma2  f7c654b3…
x86_64-unknown-linux-gnu  none    https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0/x86_64-unknown-linux-gnu.tar.lzma2         e454f710…
```
This is pyke.io — the `ort` maintainer's own CDN — redistributing ONNX Runtime **1.28.0**. Not
Microsoft, not crates.io. Every row carries a SHA256, and `build/download/verify.rs` checks it, so the
downloads are integrity-verified; the trust question is *who serves them*, not tampering in transit.
The build script's own error strings concede the fragility: it warns about the CDN returning 404
("a temporary brownout"), 410, and being "currently down".

**But the attribution is wrong where it matters, and the obvious fix would not have worked.**

It is true in isolation that `ort 2.0.0-rc.13`'s defaults include the download:
```
DEFAULT = ['std', 'ndarray', 'tracing', 'download-binaries', 'tls-native', 'copy-dylibs', 'api-27']
```
**However, `fastembed` does not use `ort`'s defaults.** Its manifest declares:
```
ort  =2.0.0-rc.13   default_features=False   features=['ndarray','std','api-24']
```
`ort`'s defaults are already off. The download is switched back on by **`fastembed`'s own default
feature**:
```
default = ["ort-download-binaries-native-tls", "hf-hub-native-tls", "image-models"]
ort-download-binaries-native-tls = ["ort/download-binaries", "ort/tls-native"]
```
Proven by feature resolution, not by reading — `cargo tree -e features -i ort` on a scratch crate
depending on `fastembed = "6"`. The inverse tree names exactly one enabler:
```
├── ort feature "download-binaries"
│   └── fastembed feature "ort-download-binaries-native-tls" (*)
```
`ort`'s `default` feature appears nowhere in that chain. So **the feature to disable is on
`fastembed`, not on `ort`** — and since `ort` is not a direct dependency of the app at all, writing
`ort = { …, default-features = false }` would have had no effect whatsoever. This is the practical
consequence of the plan's imprecision, and it is why the claim is downgraded rather than confirmed.

**The feature name that disables it, and what it costs — measured:**

| | default | `default-features = false` |
|---|---|---|
| packages locked | **332** | **109** |
| `ort/download-binaries` | enabled | **absent** |
| `hf-hub` | present | **absent** |
| resolved `ort` features | `…, download-binaries, tls-native, ndarray, std` | `api-17..24, ndarray, std` only |

```toml
fastembed = { version = "6", default-features = false }
```
verified to remove `ort/download-binaries` from the resolved graph.

**What disabling requires instead — three consequences, all load-bearing:**

1. **You must supply ONNX Runtime yourself.** With `download-binaries` off, `ort-sys` links against a
   system library. The env contract (`ort-sys build/vars.rs`) is:
   - `ORT_LIB_LOCATION` / `ORT_LIB_PATH` — point at your own ONNX Runtime build
   - `ORT_SKIP_DOWNLOAD`, `ORT_OFFLINE`, or `CARGO_NET_OFFLINE` — suppress the fetch
   - `ORT_PREFER_DYNAMIC_LINK`, `ORT_LIB_PROFILE`, `ORT_VCPKG_TARGET` — link tuning

   Or take the runtime-loading route instead of the build-time one, via fastembed's
   **`ort-load-dynamic`** feature (`= ["ort/load-dynamic"]`, which is
   `["std","preload-dylibs","ort-sys/disable-linking"]`). Then ship `onnxruntime.dll`/`.so` beside the
   app and point `ORT_DYLIB_PATH` at it at runtime. **For a Tauri desktop app that must ship a
   self-contained installer, this is the more honest option** — the runtime becomes a bundled asset
   you version deliberately, not a build-machine download.

2. **`default-features = false` also removes `hf-hub` — the model-downloading API disappears.**
   Verified: no `hf-hub` in the resolved tree. `TextEmbedding::try_new` is `#[cfg(feature = "hf-hub")]`
   gated (`src/text_embedding/impl.rs:32`). Without it you must use
   `TextEmbedding::try_new_from_user_defined(UserDefinedEmbeddingModel, InitOptionsUserDefined)`
   (`impl.rs:87`) and hand it the `.onnx` bytes and tokenizer files yourself. You cannot merely flip
   the flag and keep the convenient constructor. To keep model fetching while dropping only the
   runtime download:
   ```toml
   fastembed = { version = "6", default-features = false,
                 features = ["hf-hub-rustls-tls", "ort-load-dynamic"] }
   ```

3. **Windows has no CPU-only prebuilt.** `dist.tsv` offers a `none` feature set for
   `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, but for `x86_64-pc-windows-msvc` the
   only rows are `directml`, `webgpu`, `nvrtx,directml`, and `cuda13,tensorrt,nvrtx,directml`. There
   is no plain-CPU Windows archive. So the default Windows build pulls a DirectML-carrying runtime
   whether or not the app wants GPU execution.

**Verdict rationale:** the plan correctly identified a real build-time third-party download and was
right to flag it. It is marked PARTIALLY CONFIRMED rather than CONFIRMED because the crate it names as
the source of the default is not the crate that enables it in this graph, and acting on the claim as
written (disabling `ort`'s defaults) would have changed nothing while appearing to fix it.

**What the plan must say instead:** "**`fastembed`'s** default features enable `ort/download-binaries`,
which downloads ONNX Runtime 1.28.0 from `cdn.pyke.io` at build time. Disable with
`default-features = false` on **`fastembed`**; this also drops `hf-hub`, so either re-add
`hf-hub-rustls-tls` or supply the model via `try_new_from_user_defined`. Supply the runtime via
`ort-load-dynamic` + a bundled `onnxruntime.dll`, or via `ORT_LIB_LOCATION`."

---

## C-5 · fastembed cache path defaults to CWD-relative · **CONFIRMED** (and worse than stated)

**Claim:** *"The fastembed cache path defaults to CWD-relative."*

**Evidence:**

`src/common.rs:11-15`, verbatim and complete:
```rust
const DEFAULT_CACHE_DIR: &str = ".fastembed_cache";

pub fn get_cache_dir() -> String {
    std::env::var("FASTEMBED_CACHE_DIR").unwrap_or(DEFAULT_CACHE_DIR.into())
}
```
`".fastembed_cache"` is a **relative** path with no base — it resolves against the process CWD. The
crate's own docs say so plainly (`src/lib.rs:19`): models download to `./.fastembed_cache` on first
use. This is fed into the default `InitOptions` (`src/init.rs:41` and `:54`):
```rust
cache_dir: get_cache_dir().into(),
```

For a Tauri desktop app this is a live bug waiting to happen: CWD for a packaged app is wherever the
OS launched it from (on Windows, often `C:\Program Files\...` or the shortcut's "Start in" directory),
which is frequently **not writable**, and differs between `cargo run` in development and a shipped
`.exe`. A 133 MB download landing in an arbitrary, possibly-unwritable, possibly-per-launch-different
directory is exactly the failure the plan is right to have flagged.

**The API to set it explicitly** — a builder method on the init options:
```rust
pub fn with_cache_dir(mut self, cache_dir: PathBuf) -> Self   // src/init.rs:77 and :116
```
i.e. `TextInitOptions::new(EmbeddingModel::BGESmallENV15).with_cache_dir(app_data_path)`.
The env var `FASTEMBED_CACHE_DIR` sets the same thing.

**The claim is CONFIRMED, but it understates the hazard — `with_cache_dir` is not authoritative.**
`pull_from_hf` (`src/common.rs:245-247`) *discards* the configured directory whenever `HF_HOME` is set:
```rust
let cache_dir = env::var("HF_HOME")
    .map(PathBuf::from)
    .unwrap_or(default_cache_dir);      // <- the with_cache_dir value is only the FALLBACK
```
Confirmed by the crate's own doc line (`src/lib.rs:21`): `HF_HOME` takes precedence over both.

**Full precedence, highest first:**
1. `HF_HOME` (env) — **overrides everything, including an explicit `with_cache_dir`**
2. `with_cache_dir(PathBuf)` (builder) — overrides `FASTEMBED_CACHE_DIR`
3. `FASTEMBED_CACHE_DIR` (env)
4. `./.fastembed_cache` — CWD-relative default

So a developer machine or a user machine that already has `HF_HOME` set (common for anyone who uses
Python `transformers`, `huggingface_hub`, or Ollama tooling) will silently place AutoDev's model
outside the app-data directory the plan intends. `HF_ENDPOINT` similarly redirects the *download
origin* to an arbitrary mirror.

**Verdict rationale:** CONFIRMED — `DEFAULT_CACHE_DIR` is the bare relative string
`".fastembed_cache"`, which can only resolve against the process CWD, and the crate's own docs say
so. It is not marked OVERSTATED because the claim is true as written; it is marked "and worse than
stated" because the plan implies `with_cache_dir` is the fix, and the source shows `HF_HOME`
silently overrides it — so a plan that acts only on the claim as written would still ship a
model cache in an unintended location on any machine with `HF_HOME` set.

**What the plan must do:** setting `with_cache_dir` is necessary but **not sufficient**. To guarantee
the model lands under the user's app-data directory, the memory subsystem must either explicitly set
`HF_HOME` to its own path before constructing the embedder, or read `HF_HOME` and refuse/warn when it
points elsewhere. Silently inheriting the user's `HF_HOME` should be a conscious decision, not an
accident. Assert the resolved path after init.

---

## C-6 · Build feasibility across Windows / Linux / macOS · **PARTIALLY CONFIRMED — one hard platform gap**

**Claim:** *(implicit in the plan)* the stack builds on Windows (this machine) and on Linux and macOS in CI.

**Evidence:**

**The SQLite half: no new platform constraint at all.** `rusqlite` + `sqlite-vec` built and ran on
`x86_64-pc-windows-msvc` (rustc 1.94.0), exit 0, 15.72s cold. A C compiler *is* required — `sqlite-vec`
compiles `sqlite-vec.c` through the `cc` crate — but **the workspace already pays that cost**:
`rusqlite`'s `bundled` feature already compiles SQLite itself via `libsqlite3-sys`. The probe build log
shows `cc v1.4.4` and `find-msvc-tools v0.1.11` resolving the MSVC toolchain without configuration.
`sqlite-vec.c` contains only **2** occurrences of `_WIN32`/`_MSC_VER` in 320 KB of source — it is
plain portable C. **Adding `sqlite-vec` introduces no toolchain requirement the repo does not already
satisfy.**

Persistence and failure modes, verified on disk (probe 2):
```
[ OK ] phase1: created + inserted on disk
        shadow tables: ["m", "m_chunks", "m_info", "m_rowids", "m_vector_chunks00", "sqlite_sequence"]
[ OK ] phase2 reopen+query -> "a"  (data survives restart)
[ OK ] phase3: a DIFFERENT, later connection also has vec_version() = v0.1.9 -> auto_extension is PROCESS-GLOBAL
[ OK ] NULL partition key ACCEPTED on insert
        rows retrievable via `scope IS NULL`: 1
```
**A vec0 database is not bricked if the extension is missing** — a separate binary that never
registers sqlite-vec:
```
[ OK ] opened the DB file with NO vec extension registered
[ OK ] sqlite_master readable (8 objects) -> DB is NOT bricked
[ OK ] shadow table m_rowids readable (2 rows) -> raw data recoverable
[EXPECTED] vec0 table query fails -> no such module: vec0
[ OK ] VACUUM succeeded
```
Only queries against the virtual table fail (`no such module: vec0`); the file opens, the schema
reads, the shadow tables are plain readable tables, and `VACUUM` works. This is a benign
failure mode and it gives the retrieval-backend trait a real fallback: detect `no such module: vec0`
and degrade to FTS5-only rather than failing the whole subsystem.

**The embedding half: one target has no prebuilt runtime.** `ort-sys 2.0.0-rc.13` ships prebuilts for
exactly these 8 targets:
```
aarch64-apple-darwin      aarch64-apple-ios       aarch64-apple-ios-sim   aarch64-linux-android
aarch64-pc-windows-msvc   aarch64-unknown-linux-gnu   x86_64-pc-windows-msvc   x86_64-unknown-linux-gnu
```
**`x86_64-apple-darwin` — Intel macOS — is absent.** Confirmed twice: absent from `dist.tsv`, and the
CDN itself returns **HTTP 404** for the corresponding URL. On Intel Macs the build fails at
`ort-sys`'s resolver with `no prebuilt binaries available for target x86_64-apple-darwin`, and the
only routes forward are compiling ONNX Runtime from source, `ORT_LIB_LOCATION`, or an alternate
backend. **If CI builds or ships an Intel macOS artifact, P3-F breaks that job.** Apple Silicon is
fine.

Also absent: **any `*-windows-gnu` target** (only `-msvc`) and any `musl` target. The workspace host
is `x86_64-pc-windows-msvc`, so this machine is fine, but a MinGW/GNU Windows toolchain or an
Alpine/musl CI container has no prebuilt either.

Download sizes, by HTTP HEAD against the CDN (metadata only — **I did not download these**):

| target | archive | size (compressed) |
|---|---|---|
| `x86_64-pc-windows-msvc+directml` | `.tar.lzma2` | **29.6 MiB** |
| `x86_64-unknown-linux-gnu` (none) | `.tar.lzma2` | 9.6 MiB |
| `aarch64-apple-darwin+coreml` | `.tar.lzma2` | 8.8 MiB |
| `x86_64-apple-darwin` | — | **HTTP 404** |

Windows is ~3× Linux because, per C-4, there is no CPU-only Windows build — DirectML rides along.

**Dependency weight** (measured via `cargo tree`): `fastembed = "6"` with default features locks
**332 packages**; with `default-features = false`, **109**. For comparison a project depending only on `rusqlite`(bundled) + `sqlite-vec` resolves to **24 packages total**. The plan should know that the embedding dependency is by far the larger surface, and that
roughly two-thirds of it is optional.

**Verdict rationale:** PARTIALLY CONFIRMED. Windows — the machine that matters most here — is proven
by execution for the SQLite half and has prebuilt coverage for the embedding half. Linux and Apple
Silicon are covered. But "builds on macOS in CI" is **false for Intel macOS**, verified by a 404, and
that is a concrete gap rather than a theoretical one.

**What the plan must do:** state the supported target list explicitly and drop or specially-handle
`x86_64-apple-darwin`. If Intel Mac support is required, the `ort-load-dynamic` route from C-4 (ship
the runtime as a bundled asset) sidesteps the missing prebuilt entirely — which is another argument
for taking that route on all platforms rather than only where the CDN happens to have a build.

---

## Summary table

| # | Claim | Verdict |
|---|---|---|
| C-1 | sqlite-vec statically links, composes with bundled rusqlite, no extension loading | **CONFIRMED** (built + ran) |
| C-2 | Scope as a declared `vec0` partition key, not a post-filter | **CONFIRMED** (pre-filter proven by discriminating test) |
| C-3 | fastembed / BGE-small-en-v1.5 / 384 dims / ~130 MB | **CONFIRMED** (dim 384, 133.09 MB) |
| C-4 | "`ort`'s default features" download from a third-party CDN | **PARTIALLY CONFIRMED** — real behaviour, wrong crate; the flag is on `fastembed` |
| C-5 | fastembed cache path defaults to CWD-relative | **CONFIRMED**, and understated — `HF_HOME` overrides even `with_cache_dir` |
| C-6 | Builds on Windows / Linux / macOS | **PARTIALLY CONFIRMED** — no prebuilt for Intel macOS (HTTP 404) |

**Nothing in the plan's core architecture was refuted.** The SQLite + sqlite-vec + FTS5 + partition-key
design is sound and now executed end to end. The corrections are all in the embedding dependency's
configuration and platform surface.

---

## What I could not determine

Each item names the exact measurement that would settle it.

1. **Whether `fastembed` actually compiles end-to-end on this machine.** I proved the *dependency and
   feature graph* with `cargo tree` (which does not run build scripts), but I did not run a full
   `cargo build`. Doing so with default features triggers the ~29.6 MiB download of a prebuilt
   ONNX Runtime from `cdn.pyke.io` — a third-party binary fetch, and precisely the behaviour under
   review — so I did not trigger it unilaterally. Everything in C-3/C-4/C-5 is from crate source and
   resolved metadata, which is authoritative for those claims but is not a compile.
   **Measurement:** `cargo build` in `…/scratchpad/fe_default/` (default features; expect a
   29.6 MiB CDN download), and again in `…/scratchpad/fe_nodefault/` (expect a link failure naming
   `ORT_LIB_LOCATION`, which would confirm the opt-out's cost). Both projects are already written and
   their dependency graphs already resolved — only the build step remains.

2. **Whether the model yields 384-length vectors at runtime.** `dim: 384` is a source constant in
   `ModelInfo`; I did not run inference. A wrong constant is unlikely but is exactly the class of
   thing that should not be taken on trust for a schema-defining number (`float[384]` is baked into
   the vec0 DDL and a rebuild is the only way to change it).
   **Measurement:** after (1), `TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15))`,
   then `assert_eq!(model.embed(vec!["hello"], None)?[0].len(), 384)`.

3. **Which Windows prebuilt variant `ort-sys` selects when no execution-provider features are
   requested.** I *inferred* `directml` because `resolve.rs` does
   `candidates.sort_by_key(|c| Reverse(c.1))` — a stable sort — and with an empty requested feature
   set all four Windows rows have intersection 0, so `dist.tsv` order is preserved and `directml`
   is first. This is reasoning from source, not observation.
   **Measurement:** build with ort-sys build logging enabled and read the emitted
   `looking for prebuilt binaries matching feature set: …` / chosen-URL line from the build script
   output.

4. **The SQLite header/library version skew.** `sqlite-vec` compiles against its own bundled
   `sqlite3.h`, which declares `#define SQLITE_VERSION "3.45.3"` / `SQLITE_VERSION_NUMBER 3045003`,
   while it links `libsqlite3-sys 0.28.0`'s SQLite **3.45.0** (the probe reported
   `sqlite_version = 3.45.0`). It built, linked and ran, so nothing it references postdates 3.45.0
   today — but I did not audit the full referenced-symbol set, and this is a *forward* risk: a future
   `sqlite-vec` that bumps its vendored header and calls a newer API would fail to link against the
   pinned `libsqlite3-sys 0.28`.
   **Measurement:** re-run the depprobe build on every `sqlite-vec` bump — a link error is the
   symptom and it is loud. For a positive check, `dumpbin /symbols` on `sqlite_vec0.lib` (MSVC) or
   `nm -u` (ELF) and diff the undefined `sqlite3_*` set against 3.45.0's export list.

5. **Whether `sqlite-vec 0.1.10-alpha.4` changes partition-key semantics.** All C-1/C-2 execution was
   against **0.1.9**, the current stable and the sensible pin. The alpha is untested here.
   **Measurement:** change one line in `…/scratchpad/depprobe/Cargo.toml` to
   `sqlite-vec = "=0.1.10-alpha.4"` and re-run both probe binaries.

6. **Partition-pruning behaviour and recall at realistic scale.** The pre-filter proof used **3 rows**.
   That is sufficient to establish the *mechanism* (a post-filter provably cannot return the observed
   rows) but says nothing about performance, chunk-packing behaviour with many small partitions, or
   recall when a partition spans many chunks.
   **Measurement:** load N = 10^5 rows across, say, 50 scopes; compare wall-clock and returned-set
   for `k=10` with and without the partition predicate against a brute-force ground truth.

7. **Hybrid vec0 + FTS5 in a single query.** I proved both exist and work **in the same connection**,
   which is what the "SQLite + sqlite-vec + FTS5" composition claim needed. I did not write a joined
   hybrid-retrieval query (e.g. reciprocal-rank fusion across a `vec0` KNN and an FTS5 `MATCH`).
   **Measurement:** a query joining the two on rowid, checked against separately-computed rankings.

8. **Linux and macOS builds of the SQLite half.** Executed on Windows MSVC only. The C is portable
   (2 platform guards in 320 KB) and the crates are cross-platform, so the risk is low — but low is
   not zero and it was not measured.
   **Measurement:** run the same `depprobe` on a Linux and an Apple Silicon runner.

**Scope note:** the repository at `C:\AIRS Foundry\autodev\autodev` was **not modified** — no edits, no
new files, no `cargo add`, no `Cargo.toml`/`Cargo.lock` change, no git operations. It was read only
(three `Cargo.toml` dependency lines and `Cargo.lock`). The repo's `.env` was never read, printed, or
used. No repository test suite was run. All builds happened in
`…/scratchpad/depprobe/`, `…/scratchpad/fe_default/`, and `…/scratchpad/fe_nodefault/`, each carrying
its own `[workspace]` key so none of them can attach to the repo workspace.

Verified after the fact — the three declaring manifests and the lockfile all predate this session
(which began ~20:00 on 2026-09-02):
```
2026-09-02 13:50  Cargo.lock
2026-09-02 14:19  Cargo.toml
2026-09-02 13:50  autodev-core/Cargo.toml
2026-04-30 09:04  src-shared/Cargo.toml
2026-09-02 14:19  src-tauri/Cargo.toml
```
Note for completeness: `.git/index`, `.git/COMMIT_EDITMSG` and `.git/FETCH_HEAD` carry mtimes of
19:47 and 19:54 — **before this session's first command** — so they are someone else's commit/fetch,
not mine. I ran no git command of any kind.
