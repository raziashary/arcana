// PD0-08 — the spawnable entry point.
//
// This is the executable AutoDesign spawns. It is deliberately tiny: it wires
// the real streams to `runInvocation` and translates the returned code into a
// process exit. All of the behaviour — decoding, refusing, emitting, settling —
// lives in `sidecar.ts` and `reference.ts`, where the conformance suite drives
// it directly.
//
// ── Invocation ───────────────────────────────────────────────────────────────
//
//   node dist-sidecar/design/contract/main.js  < request.json
//
//   stdin   one JSON object, the request, terminated by EOF
//   stdout  newline-delimited JSON events; exactly one has type "settled"
//   stderr  protocol errors and crashes, never contract traffic
//   exit    0 settled · 1 accepted but unsettled · 2 protocol · 130 interrupted
//
// ⚠ **Why the whole request arrives before anything runs, rather than streaming.**
// One invocation is one request. A persistent request loop would need framing,
// cancellation-in-band and a session model — none of which the seam needs while
// a single operation is a single spawn, and every one of which would be a shape
// to version forever. If a caller wants concurrency it spawns more children,
// which is what process isolation is for.

import { EXIT, readAll, runInvocation, type SidecarIo } from './sidecar'

/**
 * ⚠ Kept separate from `runInvocation` so nothing that writes to a real stream
 * is in the path the tests drive. The suite injects its own `SidecarIo`.
 */
const processIo: SidecarIo = {
  writeOut: (line) => process.stdout.write(`${line}\n`),
  writeErr: (line) => process.stderr.write(`${line}\n`),
}

async function main(): Promise<void> {
  // Cancellation is "the parent kills the child" (`PD0-08`). On a signal we
  // exit **without** a terminal event: a `settled` for work that did not finish
  // is worse than silence, because the caller would believe it.
  for (const signal of ['SIGINT', 'SIGTERM'] as const) {
    process.on(signal, () => {
      process.stderr.write(`INTERRUPTED: ${signal} received before settling\n`)
      process.exit(EXIT.INTERRUPTED)
    })
  }

  const raw = await readAll(process.stdin)
  process.exitCode = await runInvocation(raw, processIo)
}

void main().catch((error: unknown) => {
  process.stderr.write(`UNSETTLED: ${String(error)}\n`)
  process.exitCode = EXIT.UNSETTLED
})
