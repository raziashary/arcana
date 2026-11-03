<task-notification>
<task-id>a629150bf6a2bb7da</task-id>
<tool-use-id>toolu_01ALSEsyT1wp5dAJxD83CF8V</tool-use-id>
<output-file>C:\Users\RAZIAS~1\AppData\Local\Temp\claude\C--AIRS-Foundry-autodev\b0d308fa-5fa4-4bdf-b8ec-d366061e88e8\tasks\a629150bf6a2bb7da.output</output-file>
<status>completed</status>
<summary>Agent "PD0 area audit" finished</summary>
<note>A task-notification fires each time this agent stops with no live background children of its own. The user can send it another message and resume it, so the same task-id may notify more than once.</note>
<result>**I could not create the report file.** I have no Write tool in this read-only mode, and my operating rules prohibit heredoc/redirect writes. The full report is inline below.

---

# PD0 audit — `src/design/`

## A-1 · The sidecar silently corrupts any non-ASCII request that spans a chunk boundary · **critical**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\sidecar.ts:111-117`
**Defect:** `readAll` decodes each stdin chunk independently with a fresh `new TextDecoder()` and no `{ stream: true }`. Node yields `Buffer`s split at arbitrary **byte** offsets, so a multi-byte UTF-8 sequence straddling two chunks is decoded as two invalid fragments and replaced with `U+FFFD`. The corruption is silent — the result is still valid JSON.
**Failure scenario:** Reproduced. Feeding `{"operationId":"日本語のデザイン"}` as two buffers split at byte 20 yields `{"operationId":"日���語のデザイン"}` and **`JSON.parse` succeeds**. `decodeRequest` accepts it (`str()` only checks non-empty), every event echoes the mangled `operationId`, and the caller cannot correlate the response with the request it sent. The same mechanism silently rewrites `design.ref`, `storageRef` and any non-ASCII path in `input`, so PD1 will resolve a reference that does not exist. Triggered by any stdin larger than the pipe buffer, or any parent that writes the request in more than one `stdin.write()`.
**Fix shape:** one decoder, `decode(chunk, { stream: true })`, plus a final `decode()` flush — or just collect `Buffer`s and `Buffer.concat(...).toString('utf8')`.
**Confidence:** certain (reproduced with `node`).

## A-2 · `timestamp()` accepts non-ISO, V8-only date strings while the contract promises ISO-8601 · **high**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\decode.ts:107-113`
**Defect:** The gate is `/(?:Z|[+-]\d{2}:\d{2})$/` plus `!Number.isNaN(Date.parse(s))`. `Date.parse` falls back to V8's legacy parser, which accepts a large space of non-ISO strings; the regex only looks at the last few characters.
**Failure scenario:** Reproduced — all of these are **ACCEPTED** by `timestamp()`:
- `"Sep 3, 2026 12:00:00 GMT+00:00"`
- `"Mon Sep 03 2026 12:00:00 GMT+05:30"`
- `"12/25/2026 GMT+01:00"` (month/day order is V8-specific)
- `"2026-09-03Z"` (a date with **no time at all**, silently becomes midnight)

A producer emits `observedAt: "12/25/2026 GMT+01:00"`; our decoder blesses it and the partner-facing doc (`§7`) tells AutoDesign we enforce ISO-8601 with an offset. Their `z.string().datetime()`, Rust `chrono::DateTime::parse_from_rfc3339`, or Python `datetime.fromisoformat` all reject it. The seam breaks on their side on a payload we certified. Worse, `"2026-09-03Z"` passing means `expiresAt`/`createdAt` comparisons run against a fabricated midnight. This is the third variant of the timestamp defect the file's own docstring says the programme has shipped twice.
**Fix shape:** anchor a full RFC3339 regex (`^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$`) *and* keep the `Date.parse` sanity check.
**Confidence:** certain (reproduced).

## A-3 · A stub that answers every request with the same fixed response is reported CONFORMANT · **high**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\conformance.ts:84-198`
**Defect:** Two compounding holes. (a) The four per-operation checks assert event count, one-settled-last, `operationId` echo and monotonic `sequence` — **none asserts that the settled event's `operation` matches the request's**, and `decodeSettled` does not cross-check it either. (b) The five "no-fire half" checks at lines 134-198 never call `driver` at all; they construct fixtures and run **our own decoder** on them.
**Failure scenario:** AutoDesign writes a stand-in whose `run()` ignores `request.operation` entirely and always returns the reference `ingest` event sequence with the caller's `operationId` substituted. It passes all 21 checks and `formatReport` prints `CONFORMANT`. They ship against a seam they have not actually implemented for `export`, `adapt` or `change`. Separately, a driver that happily accepts `operation: "measure"`, or emits `fidelity: {score: 0.97, method: 'unmeasured'}`, still passes the checks *named* for those behaviours, because those checks never touch the driver.
**Note on comments:** the file header (lines 20-23) states "every check below drives a real implementation and reads what it produced" and lines 128-133 state the negative half exists precisely because "every check above is satisfiable by an implementation that returns a fixed valid response to anything". The code does not implement either claim.
**Confidence:** certain (read the code).

## A-4 · `ExtraBold`/`UltraBold`/`ExtraLight`/`Thin` PostScript suffixes mangle the family name, re-creating the P3-48 defect · **high**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\fonts.ts:76-77` (`STYLE_SUFFIX`), consumed at `fonts.ts:84-90`, `fonts.ts:139-147`
**Defect:** `STYLE_SUFFIX` lists `Bold|SemiBold|Light|Black|…` but **not** `ExtraBold`, `UltraBold`, `ExtraLight`, `UltraLight`, `Thin`, `Heavy`, `Book`, `Demi`. The suffix therefore survives `baseFamilyName`, and the camel-case splitter then inserts a space inside it.
**Failure scenario:** Reproduced. `"Montserrat-ExtraBold"` → `baseFamilyName` = **`"Montserrat-Extra Bold"`**. Consequences, all silent:
- `cssFontFamily` emits `'Montserrat-Extra Bold',sans-serif` — a family that exists nowhere, so even a user with Montserrat installed gets generic sans.
- `resolveFont` compares `"montserrat-extra bold"` against the table key `"Montserrat"` → **`'unresolved'`**, although Montserrat is the first entry in `GOOGLE_FONTS`.
- `googleFontToken` returns `undefined`, so no `&lt;link&gt;` is ever emitted for it.
- `unresolvedFonts(doc)` reports Montserrat as unavailable — a false loss claim in the exact field `P3-48` created to stop false success claims.

`Montserrat-ExtraBold` is one of the most common headline faces in ad PSDs; `Kanit-ExtraLight` → `"Kanit-Extra Light"` fails identically.
**Confidence:** certain (reproduced).

## A-5 · `assertNoPixels` cannot fire on the mistake its own comment says it exists to catch · **high**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\ir.ts:196-211`
**Defect:** The guard walks layers and inspects only **top-level own properties** whose value is a **string** starting with `data:`. The docstring says: "the natural mistake — attaching the layer canvas 'just for preview' — silently restores the 17 MB problem. Cheaper to fail loudly."
**Failure scenario:** A developer does exactly what the comment names — `return { ...common, kind: 'image', assetId, canvas }` in `psdToIr.ts:262-272`. `canvas` is an `HTMLCanvasElement`, i.e. an **object**, so `typeof value === 'string'` is false and the guard passes. `designIrToJson` then serializes it (`JSON.stringify` of a canvas yields `{}`, so the size check in `psdToIr.test.ts:180` also stays green) — or, once a `toJSON`/data-URI wrapper is added, blows the payload up with no guard firing. Equally invisible: a data URI nested one level down (`style.backgroundImage`, `styleRuns[0].preview`, `assets`), because `Object.entries` is shallow. Only a data URI placed as a direct string property of a layer trips it, which is the one form nobody writes by accident.
**Confidence:** certain (read the code).

## A-6 · Exit code 130 is unreachable on Windows, and the signal handler races the settled path · **high**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\main.ts:40-49`
**Defect, part 1 (unreachable guard):** Node on Windows never emits `SIGTERM`; the listener can be registered but is never called. AutoDev is a Windows-first Tauri app, and a parent's `child.kill()` on Windows is `TerminateProcess`, which runs no handler at all. So the documented `130 · A signal arrived mid-flight` path cannot execute on the platform the product ships on.
**Failure scenario 1:** AutoDesign cancels an in-flight adapt on Windows. The child dies with exit **1**, which the partner doc defines as "Accepted, then failed before settling; stdout may hold partial events" — they log a spurious failure, and are invited to inspect partial output as if the run had genuinely broken.
**Defect, part 2 (race):** `process.stdout` is asynchronous for pipes. `runInvocation` queues four `write()`s, `main` sets `process.exitCode = 0`, and the process stays alive draining. The signal handler holds no "already settled" flag and calls `process.exit(130)`, which **discards queued writes**.
**Failure scenario 2:** SIGINT arrives in the drain window after the settled event was produced. The caller sees exit 130 ("no terminal event was emitted") *and* a stdout stream truncated mid-line — a partially-written JSON line, which the doc explicitly promises cannot happen. Both halves of the contract are wrong for the same invocation.
**Confidence:** part 1 certain (documented Node behaviour); part 2 certain in mechanism, needs a run to time.

## A-7 · `request.input` is validated as "an object" and then read by nothing — the type doc claims the opposite · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\decode.ts:266`; claim at `types.ts:293-295`; consumer at `reference.ts:113-180`
**Defect:** `types.ts` states "`input` is deliberately unconstrained here and **constrained per operation by the reference implementation**." `reference.ts` never references `request.input` — `runOperation` and `settle` read only `operation` and `operationId`. `input` is a dead field: shape-checked once, never consulted. It is also the one object in the contract that skips `noExtraKeys`, so `__proto__` / `constructor` keys pass straight through `decodeRequest` into whatever PD1 does with it.
**Failure scenario:** AutoDesign sends `{"operation":"adapt","input":{"targetWidth":1080,"targetHeight":1920}}`. The sidecar exits 0 with a `partial` outcome and a `REFERENCE_IMPLEMENTATION_ONLY` diagnostic — identical to the response for `input: {}`. Nothing tells them the dimensions were discarded, and the type they were handed says they were validated. Note the error message paths (`decode.ts:47`) promise `input.targetWidth: expected a positive integer` as the motivating example; no such check exists.
**Confidence:** certain (read the code).

## A-8 · Omitting `design` on an `ingest` request produces the error "a design object is required" for the one operation that must not have one · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\decode.ts:252`
**Defect:** `o.design === null ? null : decodeDesignRef(o.design, …)` treats **absent** as **present-but-wrong**. `noExtraKeys` constrains the key set but never requires presence, so omission is a reachable input.
**Failure scenario:** AutoDesign sends the idiomatic ingest envelope with the null field omitted: `{"contractVersion":…,"operationId":"op-1","operation":"ingest","input":{}}`. `decodeDesignRef(undefined)` throws `request.design: expected an object`, exit 2. Their integrator reads that as "ingest needs a design", adds one, and is then told `ingest produces a design and must not be given one`. Two contradictory errors for one request. The same trap exists for `expiresAt` (`decode.ts:171`) and `fidelity` (`decode.ts:326`), both documented as nullable.
**Confidence:** certain (read the code).

## A-9 · stdin is unbounded; the OOM/overflow path reports itself as "accepted, then failed" · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\sidecar.ts:111-117`, `main.ts:47-54`
**Defect:** No byte cap anywhere. `readAll` accumulates every chunk as a JS string, then `chunks.join('')` allocates one more copy. V8's `MAX_STRING_LENGTH` is 536,870,888 (measured on this machine).
**Failure scenario:** a 2 GB stdin. Best case `join('')` throws `RangeError: Invalid string length`; that lands in `main.ts:51`'s `.catch`, which sets `process.exitCode = EXIT.UNSETTLED` — exit **1**, documented as "**Accepted**, then failed before settling. stdout may hold partial events." Nothing was accepted and stdout is empty; the partner is told to treat a rejected 2 GB blob as a half-finished job. Before that limit is reached, ~4 GB of UTF-16 string data aborts the process with a V8 heap OOM and no contract line on stderr at all. Given AutoDesign's own envelope caps input at 16 KiB (`types.ts:65-70`), a cap of the same order plus `EXIT.PROTOCOL` is the obvious answer.
**Confidence:** certain for the exit-code mislabel; the OOM exit code needs a run.

## A-10 · `tsconfig.sidecar.json` emits the broken file its comment says it will refuse to emit · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\tsconfig.sidecar.json:9-13` (claim), `:14-28` (settings)
**Defect:** The comment: "this build **fails rather than emitting** a file that dies at runtime in the one place nobody is watching." `noEmitOnError` is not set, and its TypeScript default is `false`. `tsc` reports the error, exits 2 — **and writes the JavaScript anyway**.
**Failure scenario:** someone imports `psdToIr.ts` into `reference.ts` for PD1. `tsc -p tsconfig.sidecar.json` errors on `HTMLCanvasElement` (TS2304, correct — `lib` does omit DOM) but still emits `dist-sidecar/design/contract/reference.js`. `npm run build:sidecar` returns non-zero, so CI (`.github/workflows/ci.yml:153`) does catch it — but locally, and in any packaging step that runs `tauri build` after an ignored failure, `dist-sidecar/` now holds a fresh, complete, runtime-fatal binary. The half of the claim that holds is the type error; the half that does not is "rather than emitting". One line, `"noEmitOnError": true`, makes the comment true.
**Correct, for the record:** `lib: ["ES2022"]` genuinely does make `HTMLCanvasElement` an error; `rootDir: "src"` + `outDir` genuinely produce the `dist-sidecar/design/contract/main.js` path the doc advertises; `module: "CommonJS"` is right for a package with no `"type"` field; the test exclude is correct.
**Confidence:** certain (documented TS default + config read).

## A-11 · `SUPPORTED_VERSIONS` has no reader, and its comment claims it prevents the drift it does not prevent · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\version.ts:50-63`
**Defect:** The docstring: "Exported as data **so a decoder cannot drift from the list** … A hand-maintained `if` chain in each decoder is how one family silently accepts a version the others reject." Every decoder in `decode.ts` (lines 152, 155, 179, 262, 316) hand-codes `literal(o.contractVersion, &lt;individual constant&gt;)` and never reads `SUPPORTED_VERSIONS`. A repo-wide grep finds exactly two references: the definition and the `index.ts:45` re-export.
**Failure scenario:** `EVENT_CONTRACT_VERSION` is bumped to `0.2.0` and someone adds `'autodev-design-event/0.1.0'` to `SUPPORTED_VERSIONS.event` intending backwards compatibility. `decodeSettled` still calls `literal(..., EVENT_CONTRACT_VERSION)` and rejects every 0.1.0 event. The negotiation surface published to the partner says one thing, the decoder does another — the precise drift the comment claims is structurally impossible.
**Confidence:** certain (grep + read).

## A-12 · `cssFontWeight` maps `ExtraBold` to 600 and `UltraBold` to 900 — same weight, opposite ends · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\fonts.ts:115-124`
**Defect:** `ultrabold` sits in the `900` branch; `extrabold` sits in the `600` branch alongside `demibold`/`semibold`. ExtraBold and UltraBold are the same CSS weight, **800**. SemiBold/DemiBold is 600. Separately, `extralight`/`ultralight` (CSS 200) are bucketed with `thin` (CSS 100).
**Failure scenario:** Reproduced. `Montserrat-ExtraBold` → `"600"`, two steps lighter than designed; `Montserrat-UltraBold` → `"900"`, one step heavier. A design mixing both names for the same physical face renders them at visibly different weights. That weight is what `psdToIr.ts:326-333` writes into `FontRequirement.weights` and what `googleFontsHref` requests, so the wrong face is also the one fetched. The docstring says "Order matters: `extrabold` before `bold`" — the ordering is right; the value it returns is not.
**Confidence:** certain (reproduced).

## A-13 · `renderedFontSizePx`'s `|| 1` fallback re-creates the exact bug the function exists to fix · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\psdToIr.ts:152-157`
**Defect:** `base * (scaleY || 1)`. The transform is `[xx, xy, yx, yy, tx, ty]` (`ir.ts:116`) and index 3 is `yy`. For a layer rotated 90°, `yy = cos(90°) = 0`, so the guard converts a legitimate scale of 0 into "no scale". A negative `yy` (vertically flipped layer) passes through unmodified.
**Failure scenario:** a text layer rotated 90° and scaled 6.8× has `transform ≈ [0, 6.8, -6.8, 0, tx, ty]` and `fontSize: 20`. `scaleY = 0` → `0 || 1` → `1` → `fontSizePx = 20`, when the layer renders at 136 px. That is verbatim the failure the docstring says the function was written to prevent: "Reading `fontSize` alone — the obvious thing to do — produces text at a fraction of its real size on every transformed layer." Rotated headline text is common in ad PSDs. A flipped layer (`yy = -1`) yields `fontSizePx: -20`; `font-size: -20px` is an invalid CSS declaration and is dropped, so the glyphs fall back to the inherited size. Correct extraction is `Math.hypot(transform[2], transform[3])` (or `Math.abs` at minimum).
**Confidence:** certain (read the code; arithmetic verified).

## A-14 · The settled-event invariants run before `contractVersion` is checked, so version skew is reported as an invariant violation · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\decode.ts:288-313` vs `:316`
**Defect:** `hasOutput`/`failures` and the three outcome invariants execute at lines 288-313; `literal(o.contractVersion, EVENT_CONTRACT_VERSION, …)` executes at line 316, inside the return object. Same ordering issue in `decodeArtifactRef` (`:175` expiry check before `:179` version check).
**Failure scenario:** AutoDev ships `0.2.0`, in which `complete` no longer requires `design`. A `0.2.0` event reaches a `0.1.0` consumer. Instead of `event.contractVersion: expected "autodev-design-event/0.1.0", received "autodev-design-event/0.2.0"` — the whole reason the field exists (`version.ts:1-17`) — the partner gets `event.outcome: a complete outcome must carry a design or artifacts` and goes looking for a producer bug that does not exist. Check the version first; that is what pinning it is for.
**Confidence:** certain (read the code).

## A-15 · Three of the four event types have no decoder, so the partner can only validate one line in four · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\decode.ts` (no `decodeProgress`); `index.ts:48-56`; `types.ts:245-253`
**Defect:** `ProgressEvent` is declared, emitted by `reference.ts:117-121`, documented as stdout traffic — and has no decoder. `index.ts` exports six decoders, none for it. Nothing validates `sequence`, `observedAt`, `stage`, or the `completed`/`total` pairing on a progress event, in either direction.
**Failure scenario:** AutoDesign parses stdout line-by-line as the doc instructs and calls `decodeSettled` on the first line. It gets `event: unknown field(s): progress` — an error naming a field that is *required* on the event they received. Their only options are to hand-roll a progress decoder (defeating the "zero-dependency decoders you can copy" premise of `decode.ts:1-18`) or to skip validation on 75% of the stream. The `ProgressDetail` invariant that would matter — `completed` must not exceed `total`, and `null` "never a guess" — is enforced nowhere.
**Confidence:** certain (read the code).

## A-16 · `PTSans` and `Exo2` resolve as `unresolved` although both are in the Google Fonts table · **medium**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\fonts.ts:88` (camel-split), `:143-146` (exact-equality lookup)
**Defect:** `baseFamilyName` splits only on a lowercase→uppercase boundary (`/([a-z])([A-Z])/g`). It cannot split uppercase runs or letter→digit boundaries. `resolveFont` then requires exact string equality against the table key.
**Failure scenario:** Reproduced. `"PTSans-Bold"` → base `"PTSans"`; table key is `"PT Sans"` → no match → `resolution: 'unresolved'`, `googleFontToken` returns `undefined`, no stylesheet link, text renders in generic sans. `"Exo2-Bold"` → `"Exo2"` vs `"Exo 2"` → same. Both fonts are fetchable and listed at `fonts.ts:34` and `fonts.ts:44`. The direction of the error is at least honest (the loss is reported), but the design loses a typeface that was one string comparison away from working.
**Confidence:** certain (reproduced).

## A-17 · `googleFontsHref` puts every family in one URL, so one unavailable weight drops all of them · **medium** (latent — no caller yet)
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\fonts.ts:163-177`, weight source at `psdToIr.ts:188`
**Defect:** All families are joined into a single `css2?family=A&amp;family=B…` request. Google Fonts returns HTTP 400 for the **whole** request if any one `wght@` axis value does not exist for its family. `psdToIr.ts:188` forces `'700'` for any faux-bolded layer (`style.fauxBold ? '700' : …`) regardless of what the family actually ships.
**Failure scenario:** a designer faux-bolds a Bebas Neue headline — Bebas Neue ships **400 only**. `weights` for that family becomes `['700']`, the href includes `Bebas+Neue:wght@700`, Google returns 400 for the entire stylesheet, and **every** font in the document falls back to generic sans, including the ones that were perfectly resolvable. `unresolvedFonts()` reports nothing, because all of them resolved. Currently latent: `googleFontsHref` has no non-test caller.
**Confidence:** certain in mechanism; needs a run against the live endpoint to confirm the 400 cascades.

## A-18 · A duplicate PSD layer id silently overwrites another layer's bitmap · **low**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\psdToIr.ts:220-223`, `:262-272`
**Defect:** `assetId = 'asset-' + id` where `id` is `String(layer.id)` straight from the PSD. `AssetStore.put` is a bare `Map.set` (`psdToIr.ts:49`) with no collision check. Synthetic ids (`syn-N`) are only assigned when `layer.id` is absent, so a duplicate real id is not deduplicated.
**Failure scenario:** a PSD whose `lyid` values repeat after a merge/paste round-trip has two image layers with `id: 42`. The second `put('asset-42', …)` replaces the first canvas. `document.assetIds` lists `asset-42` once while two `ImageLayer`s reference it, so one layer renders the other's bitmap. Nothing reports it — not `assertNoPixels`, not `unresolvedFonts`, and the `assetIds` count check in `psdToIr.test.ts` passes.
**Confidence:** certain in mechanism; reachability depends on PSDs with duplicate `lyid`, which is uncommon but real.

## A-19 · `BLEND_TO_CSS` is a prototype-chained lookup, so a blend mode named `toString` yields a function · **low**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\psdToIr.ts:229`
**Defect:** `BLEND_TO_CSS[layer.blendMode ?? 'normal'] ?? 'normal'` — the `??` only catches `null`/`undefined`, and an object literal inherits `Object.prototype`. A key of `toString`, `constructor`, `valueOf` or `hasOwnProperty` returns an inherited function, which is not nullish.
**Failure scenario:** a malformed or hand-crafted PSD whose blend-mode string is `constructor` produces `blendMode: [Function: Object]` in a `DesignLayer`. `JSON.stringify` drops it, so `DesignDocument.blendMode` becomes `undefined` at runtime despite its `string` type; a non-JSON serializer emits the function source into a CSS attribute. `Object.create(null)`, a `Map`, or an `Object.hasOwn` guard removes the class entirely.
**Confidence:** certain in mechanism; reachability requires ag-psd to surface an unmapped blend string, which is only possible on a corrupt file.

## A-20 · No recursion bound on the layer walk · **low**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\ir.ts:169-178` (`walkLayers`), `psdToIr.ts:219-285` (`convertLayer`)
**Defect:** Both recurse on `children` with no depth cap and no cycle detection. `walkLayers` even takes a `depth` parameter and never uses it as a limit — it is passed to the visitor for reporting only.
**Failure scenario:** a PSD with a deeply nested group tree (or a cyclic tree from a corrupt file) overflows the stack. In-app that is an uncaught `RangeError`; once PD1 puts this behind the sidecar it becomes `UNSETTLED: RangeError: Maximum call stack size exceeded` on stderr and exit 1, with no diagnostic and no `held` outcome — the failure taxonomy the contract built is bypassed entirely by the one input most likely to hit it.
**Confidence:** certain (read the code).

## A-21 · Repeated work: per-chunk decoder, per-call table scan, double validation · **low**
**Where:** `sidecar.ts:114` · `fonts.ts:143`, `:152` · `psdToIr.ts:187` + `:329` · `decode.ts:130`, `:135` · `conformance.ts:86-125`
**Defect / instances:**
- `new TextDecoder()` allocated per stdin chunk (`sidecar.ts:114`) — one instance would do, and is also the fix for A-1.
- `Object.keys(GOOGLE_FONTS)` rebuilt on every `resolveFont` and every `googleFontToken` call; `baseFamilyName` recomputed independently in both.
- `cssFontFamily(postScriptName)` computed once per text layer in `toTextStyle` (`psdToIr.ts:187`) and then a second time for the same names in the fonts map (`psdToIr.ts:329`).
- `literal()` and `member()` each re-run `str()` on a value the caller already validated (`decode.ts:128-138`, called from `decode.ts:251` after `:242`).
- `runConformance` calls `driver.run(requestFor(operation))` **four separate times per operation** — 16 full runs where 4 would do. Harmless against the reference generator; against a partner driver that actually converts a PSD, the suite does 4× the work and the four checks can observe four different results.
**Failure scenario:** none of these corrupts data; the conformance one is the only behavioural risk (a non-deterministic driver can pass a check on run 2 that it failed on run 1). Label the rest efficiency, not correctness.
**Confidence:** certain (read the code).

## A-22 · `dist-sidecar` is gitignored and rebuilt only manually, so the one seam gate can pass against stale bytes locally · **low**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\gates.test.ts:127-145`; `.gitignore:5`; `package.json:8`
**Defect:** TGD0-06 spawns `dist-sidecar/design/contract/main.js` and checks only `existsSync`. `npm test` does not depend on `build:sidecar`. CI does sequence them correctly (`ci.yml:153` before `npm test`, with a good comment about why), so this is a local-only hazard.
**Failure scenario:** a developer edits `decode.ts` to add a validation rule, runs `npm test`, and TGD0-06 passes — against JavaScript compiled before the edit. The gate whose stated purpose is "the one gate that proves the SEAM rather than the shapes" is green on code that is not the code under test. (This working tree happened to be fresh at audit time: sources 12:09:22, build 12:09:37.) A mtime comparison, or making `pretest` run `build:sidecar`, closes it.
**Confidence:** certain (read the code + CI).

## A-23 · Discarded results and a stream with no error handler · **low**
**Where:** `main.ts:32-33` · `contract.test.ts:255`
**Defect:** (a) `process.stdout.write` / `process.stderr.write` return values are ignored and neither stream has an `'error'` listener. (b) `contract.test.ts:255` calls `await runInvocation(...)` and discards the exit code, so the test named "keeps contract traffic and diagnostics on separate streams" never asserts the invocation succeeded.
**Failure scenario:** (a) the parent reads the settled event and closes the pipe early; the next `write` raises `EPIPE` on a stream with no listener, which Node throws as an uncaught exception — a stack trace on stderr (contract traffic pollution) and a non-contract exit code, for what was a clean cancellation. (b) if `runInvocation` started returning `EXIT.PROTOCOL` for `export`, the test would still pass: `err` is empty and `out` is empty, so `out.every(...)` is vacuously true.
**Confidence:** (a) likely — Node's EPIPE-on-stdout behaviour varies by version and needs a run; (b) certain.

## A-24 · The `index.ts` barrel is imported by nothing in the repository · **low**
**Where:** `C:\AIRS Foundry\autodev\autodev\src\design\contract\index.ts`
**Defect:** `main.ts` imports `./sidecar` directly; both test files import individual modules. A repo-wide grep finds no import of `src/design/contract` from outside the directory. The barrel's only consumer is the partner-facing doc's example (`AUTODESIGN-CAPABILITY-CONTRACT.md:296`).
**Failure scenario:** the barrel can drop or misname an export and no test, typecheck or build notices — it is compiled but never resolved by anything that runs. It is defensible as the published surface for an external consumer; it is not defensible as *unexercised*. A one-line assertion in `contract.test.ts` importing from `./index` and touching each symbol would make it real. Label: style opinion with a concrete drift risk.
**Confidence:** certain (grep).

---

## Summary

| # | Severity | Finding |
|---|---|---|
| A-1 | **critical** | `readAll` corrupts multi-byte UTF-8 across chunk boundaries; `JSON.parse` still succeeds → silent request corruption |
| A-2 | high | `timestamp()` accepts V8 legacy date strings and date-only values while promising ISO-8601 |
| A-3 | high | Conformance suite: negative half never touches the driver; no operation-echo check → fixed-response stub is CONFORMANT |
| A-4 | high | `ExtraBold`/`UltraBold`/`ExtraLight` mangle the family name → Montserrat reported unresolved with a bogus CSS family |
| A-5 | high | `assertNoPixels` cannot fire on a canvas or a nested data URI — the two cases its comment names |
| A-6 | high | Exit 130 unreachable on Windows; signal handler races the settled path and truncates stdout mid-line |
| A-7 | medium | `request.input` is a dead field; `types.ts` claims the reference implementation constrains it |
| A-8 | medium | Omitted `design` on ingest → "expected an object", contradicting the next validation |
| A-9 | medium | Unbounded stdin; overflow/OOM reported as exit 1 "accepted, then failed" |
| A-10 | medium | `tsconfig.sidecar.json` lacks `noEmitOnError` — it emits the file its comment says it refuses to emit |
| A-11 | medium | `SUPPORTED_VERSIONS` has no reader; comment claims it prevents decoder drift |
| A-12 | medium | `ExtraBold`→600, `UltraBold`→900; `ExtraLight`→100 |
| A-13 | medium | `scaleY \|\| 1` re-creates the un-transformed font size for rotated layers; negative scale → negative `fontSizePx` |
| A-14 | medium | Outcome invariants run before `contractVersion`, so version skew is misreported as a producer bug |
| A-15 | medium | No progress-event decoder — 3 of 4 event types are unvalidatable by the partner |
| A-16 | medium | `PTSans` / `Exo2` resolve as unresolved despite being in the Google table |
| A-17 | medium | Single-URL Google Fonts href: one unavailable weight (faux-bold on Bebas Neue) drops every font — latent |
| A-18 | low | Duplicate PSD layer id silently overwrites another layer's bitmap |
| A-19 | low | `BLEND_TO_CSS` prototype-chained lookup returns a function for `toString`/`constructor` |
| A-20 | low | No recursion bound in `walkLayers`/`convertLayer`; stack overflow bypasses the failure taxonomy |
| A-21 | low | Repeated work: per-chunk decoder, per-call table scans, double `str()` validation, 16 driver runs where 4 suffice |
| A-22 | low | `dist-sidecar` staleness lets the seam gate pass against pre-edit bytes locally (CI is correct) |
| A-23 | low | `stdout.write` EPIPE unhandled; one test discards `runInvocation`'s exit code |
| A-24 | low | `index.ts` barrel imported by nothing in-repo |

**On the brief's item 5 (a capability with no caller):** `src/design/ir.ts`, `psdToIr.ts` and `fonts.ts` have **zero** non-test importers — `unresolvedFonts` (`psdToIr.ts:355`), whose docstring says "Callers must surface this … it is only closed if something downstream reads this", is read only by its own test, as are `designIrToJson`, `assertNoPixels`, `walkLayers`, `boundsWidth/Height`, `googleFontsHref` and `googleFontToken`. The contract makes this **neither better nor worse for those files** — `reference.ts:6-14` deliberately does not call them, and honestly says so. It does add two new instances of the same shape on its own side: `SUPPORTED_VERSIONS` (A-11) and `index.ts` (A-24). The contract's own core (`decode` → `sidecar` → `main`) *is* genuinely wired to a real spawned process and exercised by TGD0-06, which is a real improvement over the older tier.

---

## What I checked and found correct

- **`decode.ts` primitives.** `isObject` correctly excludes arrays and `null`; `array()` uses `Array.isArray`, so a string is rejected where an array is required; `nonNegativeInt` uses `Number.isSafeInteger`, which rejects `NaN`, `Infinity` and non-integers; the fidelity range check `!(score &gt;= 0 &amp;&amp; score &lt;= 1)` correctly rejects `NaN` and `Infinity` (a naive `&lt; 0 || &gt; 1` would have let `NaN` through). `-0` is accepted by `nonNegativeInt` but serializes as `0`, which is harmless.
- **Prototype pollution at the declared boundaries.** I verified in Node that `JSON.parse('{"__proto__":…}')` creates an *own enumerable* property and that `Object.keys` reports it — so `noExtraKeys` genuinely rejects `__proto__` and `constructor` on every declared object. The only gap is `request.input`, which skips `noExtraKeys` entirely (A-7).
- **No validated-value/returned-value mismatch.** I traced every field of all six decoders: every returned value is either the validator's return or a local the validator produced. `decodeDiagnostic` correctly returns the `str()`-validated `code` after the regex check rather than the raw input.
- **The `SHA256` regex** `/^[0-9a-f]{64}$/` is anchored on both ends and correctly rejects uppercase, matching the documented "64 lowercase hex". The diagnostic-code regex `/^[A-Z][A-Z0-9_]{0,127}$/` is anchored and its 128-character bound matches its message.
- **The reserved-operation guard is reachable.** `RESERVED_OPERATIONS` and `OPERATIONS` are disjoint, so the `measure` branch fires on real input and produces the "reserved" message rather than "unknown" — verified against both `contract.test.ts:64` and `gates.test.ts:26`.
- **`referenceDesign`'s digest stand-in** (`reference.ts:65`) always produces exactly 64 lowercase hex characters for any input, including surrogate pairs, so it cannot fail its own decoder.
- **`toTransform`** (`psdToIr.ts:296-302`) is the one place in the older tier that does the careful thing: it validates length *and* element type and builds the tuple element by element rather than casting, exactly as its comment claims.
- **`normalizePsdText`** correctly orders `\r\n` → `\u0003` → `\r` so that CRLF is not double-converted, and the `U+0003` literal is written as an escape with a narrowly-scoped eslint disable, as documented.
- **`resolveFont('')` is unreachable** from `psdToIr`, because `psdToIr.ts:247` guards on a non-empty `postScriptName` before adding a `FontRequirement` — so the false "unresolved" it would return for a font-less layer never materialises.
- **`tsconfig.sidecar.json`'s DOM exclusion works** as a *type check*: `lib: ["ES2022"]` with `types: ["node"]` does make `HTMLCanvasElement` an unresolved name. The output path, module format and test exclusion are all right. Only the "fails rather than emitting" half is false (A-10).
- **CI ordering is right.** `.github/workflows/ci.yml` builds the sidecar before `npm test`, and the comment explaining why is accurate — the gate would otherwise have failed from the moment it landed.
- **`googleFontsHref` sorts weights numerically** (`fonts.ts:172`), and the comment explaining why lexicographic `.sort()` is wrong is correct.</result>
<usage><subagent_tokens>146313</subagent_tokens><tool_uses>29</tool_uses><duration_ms>811134</duration_ms></usage>
</task-notification>