// Runner side of the parity-status/1 protocol (see helpers/status-helper.c).
// One channel per confined invocation: a fresh 127.0.0.1 listener, a fresh
// 128-bit nonce and an opaque run identity. The helper connects before it
// forks, the runner checks HELLO and ACKs, then the helper's single RESULT
// frame is the only authenticated source of the target's exact exit status.
// Anything else (no HELLO, wrong nonce, second connection, extra bytes,
// missing, duplicate, malformed or late RESULT) is a TargetStatusError:
// a harness failure, never a target result and never a synthesized code.
import { encodeHex } from "@std/encoding/hex"

export const STATUS_PROTOCOL = "parity-status/1"
export const STATUS_FRAME_MAX = 256
const TOKEN_BYTES = 16

/** Pinned Linux x86_64 signal numbers; the helper carries the same table. */
export const LINUX_SIGNALS: ReadonlyMap<number, string> = new Map([
  [1, "SIGHUP"],
  [2, "SIGINT"],
  [3, "SIGQUIT"],
  [4, "SIGILL"],
  [5, "SIGTRAP"],
  [6, "SIGABRT"],
  [7, "SIGBUS"],
  [8, "SIGFPE"],
  [9, "SIGKILL"],
  [10, "SIGUSR1"],
  [11, "SIGSEGV"],
  [12, "SIGUSR2"],
  [13, "SIGPIPE"],
  [14, "SIGALRM"],
  [15, "SIGTERM"],
  [16, "SIGSTKFLT"],
  [17, "SIGCHLD"],
  [18, "SIGCONT"],
  [19, "SIGSTOP"],
  [20, "SIGTSTP"],
  [21, "SIGTTIN"],
  [22, "SIGTTOU"],
  [23, "SIGURG"],
  [24, "SIGXCPU"],
  [25, "SIGXFSZ"],
  [26, "SIGVTALRM"],
  [27, "SIGPROF"],
  [28, "SIGWINCH"],
  [29, "SIGIO"],
  [30, "SIGPWR"],
  [31, "SIGSYS"],
])

/** Documented pre-fork / harness exit codes of the helper (never target results). */
export const HELPER_EXIT_CODES: ReadonlyMap<number, string> = new Map([
  [120, "usage error"],
  [121, "socket or connect failure"],
  [122, "HELLO/ACK protocol failure"],
  [123, "pipe or fork failure"],
  [124, "target execve failed"],
  [125, "unrecognized wait status"],
])

const ERRNO_NAMES: ReadonlyMap<number, string> = new Map([
  [2, "ENOENT"],
  [7, "E2BIG"],
  [8, "ENOEXEC"],
  [12, "ENOMEM"],
  [13, "EACCES"],
  [20, "ENOTDIR"],
  [21, "EISDIR"],
  [26, "ETXTBSY"],
  [36, "ENAMETOOLONG"],
  [40, "ELOOP"],
])

export class TargetStatusError extends Error {}

/** Authenticated exit of the exact target process. */
export type TargetExit =
  | { code: number }
  | { signal: string; number: number }

export interface TargetStatusRecord {
  exit: TargetExit
  /** PID of the helper inside the sandbox PID namespace. */
  helperPid: number
  /** PID of the target inside the sandbox PID namespace. */
  targetPid: number
}

export interface StatusHello {
  helperPid: number
}

export interface TargetStatusChannel {
  port: number
  nonce: string
  identity: string
  /** Helper argv prefix: port, nonce, identity; the target path and argv follow. */
  helperArgs: string[]
  /** Resolves once a valid HELLO was ACKed; rejects on a protocol violation. */
  hello: Promise<StatusHello>
  /** Resolves with the authenticated RESULT after EOF; rejects on any violation. */
  result: Promise<TargetStatusRecord>
  /**
   * Resolve the result with a bounded wait after the outer process exited.
   * A missing result after the grace period is a TargetStatusError.
   */
  finish(graceMs: number): Promise<TargetStatusRecord>
  close(): void
}

const decoder = new TextDecoder()
const encoder = new TextEncoder()

function randomToken(): string {
  return encodeHex(crypto.getRandomValues(new Uint8Array(TOKEN_BYTES)))
}

function fail(message: string): never {
  throw new TargetStatusError(`target status channel: ${message}`)
}

async function readExactly(
  conn: Deno.Conn,
  length: number,
): Promise<Uint8Array | null> {
  const buffer = new Uint8Array(length)
  let filled = 0
  while (filled < length) {
    const read = await conn.read(buffer.subarray(filled))
    if (read == null) return filled === 0 ? null : fail("truncated frame")
    filled += read
  }
  return buffer
}

/** Read one length-prefixed frame; null means clean EOF before a frame. */
async function readFrame(conn: Deno.Conn): Promise<string | null> {
  const header = await readExactly(conn, 2)
  if (header == null) return null
  const length = (header[0] << 8) | header[1]
  if (length === 0) fail("empty frame")
  if (length > STATUS_FRAME_MAX) fail(`frame of ${length} bytes exceeds cap`)
  const payload = await readExactly(conn, length)
  if (payload == null) fail("truncated frame")
  for (const byte of payload) {
    if (byte < 0x20 || byte > 0x7e) fail("non-ASCII byte in frame")
  }
  return decoder.decode(payload)
}

async function writeFrame(conn: Deno.Conn, payload: string): Promise<void> {
  const bytes = encoder.encode(payload)
  if (bytes.length === 0 || bytes.length > STATUS_FRAME_MAX) {
    fail("outgoing frame size")
  }
  const frame = new Uint8Array(2 + bytes.length)
  frame[0] = bytes.length >> 8
  frame[1] = bytes.length & 0xff
  frame.set(bytes, 2)
  let written = 0
  while (written < frame.length) {
    written += await conn.write(frame.subarray(written))
  }
}

const HELLO_RE =
  /^parity-status\/1 HELLO ([0-9a-f]{32}) ([0-9a-f]{32}) ([1-9][0-9]{0,6})$/
const RESULT_RE =
  /^parity-status\/1 RESULT ([0-9a-f]{32}) ([0-9a-f]{32}) ([1-9][0-9]{0,6}) (exited (0|[1-9][0-9]{0,2})|signaled ([1-9][0-9]?) ([A-Z0-9]{3,10})|exec-failed ([1-9][0-9]{0,3}))$/

export function parseResultFrame(
  frame: string,
  nonce: string,
  identity: string,
  helperPid: number,
): TargetStatusRecord {
  const match = RESULT_RE.exec(frame)
  if (match == null) fail("malformed RESULT frame")
  if (match[1] !== nonce) fail("RESULT nonce mismatch")
  if (match[2] !== identity) fail("RESULT identity mismatch")
  const targetPid = Number(match[3])
  if (targetPid === helperPid) fail("RESULT names the helper as the target")
  if (match[5] != null) {
    const code = Number(match[5])
    if (code > 255) fail(`exit code ${code} out of range`)
    return { exit: { code }, helperPid, targetPid }
  }
  if (match[6] != null) {
    const number = Number(match[6])
    const name = LINUX_SIGNALS.get(number)
    if (name == null) fail(`signal number ${number} is not in the pinned table`)
    if (match[7] !== name) {
      fail(`signal name ${match[7]} does not match number ${number} (${name})`)
    }
    return { exit: { signal: name, number }, helperPid, targetPid }
  }
  const errno = Number(match[8])
  const errnoName = ERRNO_NAMES.get(errno)
  fail(
    `target execve failed with errno ${errno}${
      errnoName == null ? "" : ` (${errnoName})`
    }; this is a harness or program-path failure, not an exit code`,
  )
}

export interface OpenChannelOptions {
  /** Whole-channel deadline: a RESULT after this is late. */
  deadlineMs: number
}

/** Bind a fresh loopback listener and generate this run's nonce and identity. */
export function openTargetStatusChannel(
  options: OpenChannelOptions,
): TargetStatusChannel {
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 })
  const port = listener.addr.port
  const nonce = randomToken()
  const identity = randomToken()
  let conn: Deno.Conn | null = null
  let closed = false
  let helloResolve: (hello: StatusHello) => void = () => {}
  let helloReject: (error: Error) => void = () => {}
  const hello = new Promise<StatusHello>((resolve, reject) => {
    helloResolve = resolve
    helloReject = reject
  })
  let settled = false
  let timedOutLate = false
  const deadline = setTimeout(() => {
    timedOutLate = true
    close()
  }, options.deadlineMs)

  const close = () => {
    if (closed) return
    closed = true
    clearTimeout(deadline)
    try {
      listener.close()
    } catch {
      // already closed
    }
    try {
      conn?.close()
    } catch {
      // already closed
    }
  }

  const session = (async (): Promise<TargetStatusRecord> => {
    try {
      conn = await listener.accept()
    } catch (error) {
      fail(
        `listener closed before the helper connected${
          error instanceof Error ? `: ${error.message}` : ""
        }`,
      )
    }
    // Exactly one connection: the listener is closed the moment it is taken.
    listener.close()
    const first = await readFrame(conn)
    if (first == null) fail("EOF before HELLO")
    const match = HELLO_RE.exec(first)
    if (match == null) fail("malformed HELLO frame")
    if (match[1] !== nonce) fail("HELLO nonce mismatch")
    if (match[2] !== identity) fail("HELLO identity mismatch")
    const helperPid = Number(match[3])
    await writeFrame(conn, `${STATUS_PROTOCOL} ACK ${nonce}`)
    helloResolve({ helperPid })
    const second = await readFrame(conn)
    if (second == null) fail("EOF before RESULT")
    const record = parseResultFrame(second, nonce, identity, helperPid)
    const extra = await readExactly(conn, 1)
    if (extra != null) fail("bytes after RESULT")
    return record
  })()
  // Every channel failure surfaces as a TargetStatusError, including the
  // I/O interruption caused by closing the socket at the deadline.
  const result = session.then((record) => {
    settled = true
    return record
  }, (error: unknown) => {
    settled = true
    if (timedOutLate) {
      throw new TargetStatusError(
        `target status channel: late or missing result (${options.deadlineMs} ms deadline)`,
      )
    }
    if (error instanceof TargetStatusError) throw error
    const detail = error instanceof Error
      ? `${error.name}: ${error.message}`
      : String(error)
    throw new TargetStatusError(
      closed
        ? `target status channel: closed before RESULT (${detail})`
        : `target status channel: I/O failure (${detail})`,
    )
  })
  result.catch(() => {})
  // A rejected session settles hello for callers racing on it and closes the
  // socket at once, so a helper waiting for an ACK sees EOF and exits.
  session.catch((error: unknown) => {
    helloReject(
      error instanceof Error ? error : new TargetStatusError(String(error)),
    )
    close()
  })
  hello.catch(() => {})

  return {
    port,
    nonce,
    identity,
    helperArgs: [String(port), nonce, identity],
    hello,
    result,
    finish: async (graceMs: number) => {
      if (!settled) {
        let graceTimer: number | undefined
        const grace = new Promise<never>((_resolve, reject) => {
          graceTimer = setTimeout(() => {
            reject(
              new TargetStatusError(
                `target status channel: no RESULT within ${graceMs} ms of the outer exit`,
              ),
            )
          }, graceMs)
        })
        try {
          return await Promise.race([result, grace])
        } finally {
          clearTimeout(graceTimer)
          close()
        }
      }
      close()
      return await result
    },
    close,
  }
}

export function describeTargetExit(exit: TargetExit | null): string {
  if (exit == null) return "no authenticated target status"
  return "code" in exit ? `code ${exit.code}` : `signal ${exit.signal}`
}

/**
 * The outer bwrap status must agree with the authenticated target status:
 * the helper exits with the target's code or 128+signal. Any other outer
 * status (including a SIGKILL from the runner's own group cleanup) means the
 * observation is not a clean target exit.
 */
export function outerAgrees(
  outer: { code: number } | { signal: string },
  target: TargetExit,
): boolean {
  if (!("code" in outer)) return false
  return "code" in target
    ? outer.code === target.code
    : outer.code === 128 + target.number
}
