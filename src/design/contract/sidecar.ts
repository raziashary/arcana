// PD0-08 — the sidecar invocation model.
//
// ── The seam, in one paragraph ───────────────────────────────────────────────
//
// AutoDesign spawns AutoDev as a child process, the way it already spawns its
// own sidecar. One request arrives as a single JSON line on **stdin**. Zero or
// more progress events and exactly one terminal `settled` event are written as
// JSON lines on **stdout**. Anything that is not contract traffic — a protocol
// error, a crash — goes to **stderr** and never to stdout, so a consumer can
// parse stdout line-by-line without a mode switch.
//
// ── Why stdio and not a socket ───────────────────────────────────────────────
//
// Razi's ruling, 2026-09-03: *"AutoDesign does not have any opening point and
// AutoDev itself is a local application, so if we create an HTTP process it
// will be unnecessary; and if we make it an MCP server, AutoDesign does not
// have an MCP client and building one will take time."*
//
// It is also the only option that needs nothing from anybody. AutoDev has no
// server and no server dependency; its last listening socket was an
// unauthenticated MCP endpoint removed in P0 for being local RCE; and the
// Python loopback that remains is deleted in P8. A child process introduces no
// port, no listener and no credential to leak.
//
// ── Cancellation ─────────────────────────────────────────────────────────────
//
// The parent kills the child. There is no cancel message, because a cancel
// message that arrives on the same pipe as the request cannot be read while the
// operation is running without an interleaving protocol nobody needs yet. So
// every operation must be safely interruptible at any point, and on a signal we
// exit **without** emitting a terminal event — a `settled` the work did not
// finish is worse than silence, because the caller would believe it.

import { decodeRequest, ContractError } from './decode'
import { runOperation, systemClock, type Clock } from './reference'

/**
 * What the exit code means, independently of anything on stdout.
 *
 * ⚠ Stated as an enum because a consumer has to branch on it before it has
 * parsed a byte, and "non-zero means bad" is not enough: a `held` outcome is a
 * *successful invocation reporting a refusal*, and it exits 0. Conflating the
 * two would make every rejected input look like a crashed sidecar.
 */
export const EXIT = {
  /** A terminal `settled` event was emitted. Read stdout for the outcome. */
  SETTLED: 0,
  /** Accepted, then failed before settling. stdout may hold partial events. */
  UNSETTLED: 1,
  /** The request never decoded. Nothing was attempted; see stderr. */
  PROTOCOL: 2,
  /** A signal arrived mid-flight. No terminal event was emitted. */
  INTERRUPTED: 130,
} as const

/** Somewhere to write. Injected so the whole loop is testable in-process. */
export interface SidecarIo {
  writeOut(line: string): void
  writeErr(line: string): void
}

/**
 * Run one invocation end to end.
 *
 * Returns the exit code rather than calling `process.exit`, so a test can drive
 * the real function instead of a copy of it — `L-C9`: a check that exercises a
 * near-identical reimplementation is not a check on what ships.
 */
export async function runInvocation(
  rawRequest: string,
  io: SidecarIo,
  clock: Clock = systemClock,
): Promise<number> {
  let parsed: unknown
  try {
    parsed = JSON.parse(rawRequest)
  } catch {
    io.writeErr('PROTOCOL: stdin did not contain a single JSON object')
    return EXIT.PROTOCOL
  }

  let request
  try {
    request = decodeRequest(parsed)
  } catch (error) {
    // A refusal, not a crash — and it names the field, because a consumer
    // debugging across a process boundary has nothing else to go on.
    const detail = error instanceof ContractError ? error.message : String(error)
    io.writeErr(`PROTOCOL: ${detail}`)
    return EXIT.PROTOCOL
  }

  let settled = false
  try {
    for await (const event of runOperation(request, clock)) {
      io.writeOut(JSON.stringify(event))
      if (event.type === 'settled') settled = true
    }
  } catch (error) {
    io.writeErr(`UNSETTLED: ${String(error)}`)
    return EXIT.UNSETTLED
  }

  // ⚠ Belt and braces, and it earns its place: the exit code's whole meaning is
  // "a terminal event was emitted". Deriving it from anything other than having
  // actually emitted one would be a claim rather than an observation.
  return settled ? EXIT.SETTLED : EXIT.UNSETTLED
}

/**
 * The largest request we will read.
 *
 * ⚠ Same order as AutoDesign's own 16 KiB input envelope, with room to spare.
 * The wire carries references, not documents, so anything approaching this is
 * a caller sending the payload we asked them not to send — and telling them so
 * is more useful than an out-of-memory crash.
 */
export const MAX_REQUEST_BYTES = 1024 * 1024

/**
 * Read the whole of a stream as UTF-8.
 *
 * 🔴 **The first version decoded each chunk with its own `TextDecoder` and no
 * `{ stream: true }`.** Node splits stdin at arbitrary *byte* offsets, so a
 * multi-byte character straddling two chunks was decoded as two invalid
 * fragments and replaced with `U+FFFD` — and the result was still valid JSON,
 * so nothing downstream noticed. Reproduced: an `operationId` of
 * `"日本語のデザイン"` split at byte 20 became `"日\uFFFD\uFFFD\uFFFD語のデザイン"`,
 * `JSON.parse` succeeded, `decodeRequest` accepted it, and every event echoed
 * the mangled id — so the caller could not correlate the response with the
 * request it sent. The same mechanism silently rewrites `design.ref`, so a
 * later phase would resolve a reference that does not exist.
 *
 * One decoder, streaming, with a final flush. And a byte cap, because an
 * unbounded read turns a hostile or mistaken 2 GB stdin into an OOM whose exit
 * code claims the work was "accepted, then failed".
 */
export async function readAll(stream: AsyncIterable<Uint8Array | string>): Promise<string> {
  const decoder = new TextDecoder('utf-8', { fatal: false })
  const parts: string[] = []
  let bytes = 0
  for await (const chunk of stream) {
    if (typeof chunk === 'string') {
      bytes += Buffer.byteLength(chunk, 'utf8')
      parts.push(chunk)
    } else {
      bytes += chunk.byteLength
      parts.push(decoder.decode(chunk, { stream: true }))
    }
    if (bytes > MAX_REQUEST_BYTES) {
      throw new RequestTooLarge(bytes)
    }
  }
  parts.push(decoder.decode())
  return parts.join('')
}

/** Thrown by `readAll`, so the entry point can report it as a protocol error. */
export class RequestTooLarge extends Error {
  constructor(readonly bytes: number) {
    super(
      `the request exceeded ${MAX_REQUEST_BYTES} bytes (read ${bytes}). ` +
        'The wire carries a reference to a design, never the design itself.',
    )
    this.name = 'RequestTooLarge'
  }
}
