// SD0-01 — the design sidecar, spawned as a real child process.
//
// ── Why this file exists ─────────────────────────────────────────────────────
//
// `SD0-01` was recorded `SMOKE-PROVEN` on the strength of a nine-case run that
// lived in a scratchpad directory and was never committed. The work audit
// (`W-2`) found that eight of the nine cases could not be reproduced from this
// repository at all, and that the one clause the ADR added specifically to make
// the smoke falsifiable — *"a killed child leaves no partial output claiming
// success"* — had never been run. The only thing the repository could prove
// about exit code 130 was that the string appeared in a markdown file.
//
// A smoke the register calls "throwaway" is still evidence, and evidence that
// cannot be re-run is an assertion. So it is committed, it is an npm script,
// and it fails loudly with a reason.
//
//   npm run smoke:design
//
// ── What it covers ───────────────────────────────────────────────────────────
//
//   4  operations with valid input        → exit 0, four events, terminal last
//   4  operations with unserviceable input → exit 0, outcome "held"
//   5  malformed requests                  → exit 2, NOTHING on stdout
//   1  killed child mid-flight             → no output claiming success
//
// Every case spawns `node dist-sidecar/design/contract/main.js`. Nothing here
// runs in-process: the in-process paths are covered by the unit tests, and the
// whole point of this file is the boundary they cannot cross.

import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import process from 'node:process'

const SIDECAR = 'dist-sidecar/design/contract/main.js'
const EXIT = { SETTLED: 0, UNSETTLED: 1, PROTOCOL: 2, INTERRUPTED: 130 }

if (!existsSync(SIDECAR)) {
  console.error(`FAIL  ${SIDECAR} is not built. Run: npm run build:sidecar`)
  process.exit(1)
}

const results = []
function record(name, passed, detail) {
  results.push({ name, passed, detail })
  console.log(`${passed ? 'PASS' : 'FAIL'}  ${name}\n        ${detail}`)
}

/** Spawn the sidecar, write `input`, and collect everything it said. */
function run(input, { kill = false } = {}) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [SIDECAR], { stdio: ['pipe', 'pipe', 'pipe'] })
    let stdout = ''
    let stderr = ''
    child.stdout.on('data', (d) => (stdout += d))
    child.stderr.on('data', (d) => (stderr += d))
    child.on('close', (code, signal) => resolve({ stdout, stderr, code, signal }))

    if (kill) {
      // A genuine mid-flight kill and not a simulated one: write a fragment of
      // the request and never close stdin, so the child is blocked in `readAll`
      // with the work not begun. Then kill it.
      //
      // ⚠ On Windows `child.kill()` is `TerminateProcess`, which runs no
      // handler — so exit 130 is unreachable there and the process dies
      // abruptly. That is stated rather than asserted, because a handler that
      // cannot run is not a guarantee. What must hold on EVERY platform is the
      // clause this case exists for: nothing on stdout claims success.
      child.stdin.write('{"contractVersion":"autodev-design-request/0.1.0",')
      setTimeout(() => child.kill('SIGINT'), 150)
      return
    }
    child.stdin.end(input)
  })
}

const design = {
  contractVersion: 'autodev-design-ref/0.1.0',
  schemaVersion: 'autodev-design-ir/0.1.0',
  ref: 'autodev-design:smoke',
  sha256: '0'.repeat(64),
  byteLength: 1024,
}

const VALID_INPUT = {
  ingest: { source: 'autodev-source:smoke.psd' },
  export: { format: 'json' },
  adapt: { target: { width: 1080, height: 1920 } },
  change: { instructions: [{ kind: 'smoke' }] },
}

const request = (operation, input) =>
  JSON.stringify({
    contractVersion: 'autodev-design-request/0.1.0',
    operationId: `smoke-${operation}`,
    operation,
    design: operation === 'ingest' ? null : design,
    input: input ?? VALID_INPUT[operation],
  })

// ── 1. the four operations settle ───────────────────────────────────────────

for (const operation of ['ingest', 'export', 'adapt', 'change']) {
  const { stdout, stderr, code } = await run(request(operation))
  const lines = stdout.trim().split('\n').filter(Boolean)
  let parsed = []
  let parseError = null
  try {
    parsed = lines.map((l) => JSON.parse(l))
  } catch (error) {
    parseError = String(error)
  }
  const last = parsed[parsed.length - 1]
  const ok =
    code === EXIT.SETTLED &&
    stderr === '' &&
    parseError === null &&
    lines.length === 4 &&
    last?.type === 'settled' &&
    last?.operation === operation &&
    parsed.filter((e) => e.type === 'settled').length === 1
  record(
    `${operation}: settles with exit 0, four NDJSON events, terminal last`,
    ok,
    `exit=${code} lines=${lines.length} last=${last?.type}/${last?.outcome} ` +
      `stderr=${JSON.stringify(stderr.slice(0, 60))}${parseError ? ` parse=${parseError}` : ''}`,
  )
}

// ── 2. the refusal path settles too, and says so ────────────────────────────
//
// W-7: this is the half that had no producer at all until the audit. A hold is
// a successful invocation carrying an unsuccessful outcome — exit 0, stdout
// carries the reason. If this ever exits 2, the contract's central distinction
// has collapsed into "error".

for (const operation of ['ingest', 'export', 'adapt', 'change']) {
  const { stdout, stderr, code } = await run(request(operation, {}))
  const lines = stdout.trim().split('\n').filter(Boolean)
  let last = null
  try {
    last = JSON.parse(lines[lines.length - 1] ?? 'null')
  } catch {
    /* reported by the assertion below */
  }
  const ok =
    code === EXIT.SETTLED &&
    stderr === '' &&
    last?.outcome === 'held' &&
    last?.design === null &&
    Array.isArray(last?.diagnostics) &&
    last.diagnostics.some((d) => d.severity === 'failure')
  record(
    `${operation}: an unserviceable request is HELD, not errored`,
    ok,
    `exit=${code} outcome=${last?.outcome} code=${last?.diagnostics?.[0]?.code}`,
  )
}

// ── 3. malformed requests: exit 2, nothing on stdout ────────────────────────

const malformed = [
  ['bad JSON', '{not json'],
  ['an unknown verb', request('sculpt')],
  ['the reserved verb', request('measure')],
  [
    'an unknown schema version',
    JSON.stringify({
      contractVersion: 'autodev-design-request/0.1.0',
      operationId: 'x',
      operation: 'adapt',
      design: { ...design, schemaVersion: 'autodev-design-ir/9.9.9' },
      input: VALID_INPUT.adapt,
    }),
  ],
  [
    'an undeclared field',
    JSON.stringify({
      contractVersion: 'autodev-design-request/0.1.0',
      operationId: 'x',
      operation: 'ingest',
      design: null,
      input: {},
      somethingNew: true,
    }),
  ],
]

for (const [label, payload] of malformed) {
  const { stdout, stderr, code } = await run(payload)
  const ok = code === EXIT.PROTOCOL && stdout === '' && stderr.startsWith('PROTOCOL:')
  record(
    `refuses ${label} with exit 2 and nothing on stdout`,
    ok,
    `exit=${code} stdout=${JSON.stringify(stdout.slice(0, 40))} ` +
      `stderr=${JSON.stringify(stderr.trim().slice(0, 90))}`,
  )
}

// ── 4. the kill clause, which had never been run ────────────────────────────

{
  const { stdout, stderr, code, signal } = await run(null, { kill: true })
  const lines = stdout.trim().split('\n').filter(Boolean)
  // The clause, stated exactly: a killed child leaves no partial output
  // claiming success. Three ways that could be violated — a settled event, a
  // truncated line that a lenient consumer might half-read, or exit 0.
  const claimsSuccess = stdout.includes('"type":"settled"')
  let truncated = false
  for (const line of lines) {
    try {
      JSON.parse(line)
    } catch {
      truncated = true
    }
  }
  const ok = !claimsSuccess && !truncated && code !== EXIT.SETTLED
  record(
    'a killed child leaves no partial output claiming success',
    ok,
    `exit=${code} signal=${signal} stdout=${JSON.stringify(stdout.slice(0, 60))} ` +
      `stderr=${JSON.stringify(stderr.trim().slice(0, 60))} platform=${process.platform}`,
  )
  // Reported, not asserted: Node never delivers SIGTERM on Windows and
  // `child.kill()` there is TerminateProcess, which runs no handler at all. So
  // exit 130 is reachable on POSIX and unreachable on Windows, and this line
  // is what makes the smoke honest about which one it just measured.
  console.log(
    `        note: exit 130 ${code === EXIT.INTERRUPTED ? 'was' : 'was NOT'} observed on ` +
      `${process.platform}; the contract documents it as POSIX-only.`,
  )
}

// ── verdict ─────────────────────────────────────────────────────────────────

const failed = results.filter((r) => !r.passed)
console.log(
  `\n${results.length} cases, ${failed.length} failed — ` +
    `${failed.length === 0 ? 'SMOKE PASSED' : 'SMOKE FAILED'}`,
)
process.exit(failed.length === 0 ? 0 : 1)
