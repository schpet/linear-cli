// Subprocess capture engine: every child runs in its own session/process
// group, with an explicit environment, a hard deadline, bounded output and
// group-wide SIGKILL on deadline, output cap, abort or exception.
import { concatBytes } from "./bytes.ts"

export interface Invocation {
  executable: string
  args: string[]
  cwd: string
  env: Record<string, string>
  stdin: Uint8Array
  timeoutMs: number
  outputCapBytes: number
  signal?: AbortSignal
}

export type ExitStatus = { code: number } | { signal: string }

export interface Observation {
  pid: number
  exit: ExitStatus
  stdout: Uint8Array
  stderr: Uint8Array
  /** True when a stream exceeded outputCapBytes; the child group was killed. */
  truncated: boolean
  /** True when the deadline elapsed; the child group was killed. */
  timedOut: boolean
  durationMs: number
}

export class AbortedError extends Error {}

const DRAIN_GRACE_MS = 2000

// Resolved absolutely: the child's PATH is the case's explicit PATH, which a
// sandbox may leave empty, and Rust's spawn resolves programs against it.
const SETSID_CANDIDATES = ["/usr/bin/setsid", "/bin/setsid"]
let setsidPath: string | undefined

async function resolveSetsid(): Promise<string> {
  if (setsidPath != null) return setsidPath
  for (const candidate of SETSID_CANDIDATES) {
    if (await Deno.stat(candidate).then(() => true, () => false)) {
      setsidPath = candidate
      return candidate
    }
  }
  throw new Error(`setsid not found at ${SETSID_CANDIDATES.join(" or ")}`)
}

function killGroup(pid: number): void {
  try {
    Deno.kill(-pid, "SIGKILL")
  } catch {
    // process group already gone
  }
}

export async function runIsolated(
  invocation: Invocation,
): Promise<Observation> {
  if (invocation.signal?.aborted) {
    throw new AbortedError("run aborted before spawn")
  }
  const started = performance.now()
  // setsid execs the program directly (no fork) because a freshly spawned
  // child is never a group leader, so child.pid is the session and group id.
  const child = new Deno.Command(await resolveSetsid(), {
    args: [invocation.executable, ...invocation.args],
    cwd: invocation.cwd,
    env: invocation.env,
    clearEnv: true,
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
  }).spawn()

  let exited = false
  let timedOut = false
  let truncated = false
  let aborted = false
  const cancelers: Array<() => void> = []
  let graceTimer: number | undefined

  const stop = () => {
    killGroup(child.pid)
    graceTimer ??= setTimeout(() => {
      for (const cancel of cancelers) cancel()
    }, DRAIN_GRACE_MS)
  }
  const deadline = setTimeout(() => {
    timedOut = true
    stop()
  }, invocation.timeoutMs)
  const onAbort = () => {
    aborted = true
    stop()
  }
  invocation.signal?.addEventListener("abort", onAbort, { once: true })

  const drain = async (stream: ReadableStream<Uint8Array>) => {
    const reader = stream.getReader()
    cancelers.push(() => {
      reader.cancel().catch(() => {})
    })
    const chunks: Uint8Array[] = []
    let total = 0
    let overflow = false
    try {
      for (;;) {
        const { value, done } = await reader.read()
        if (done) break
        if (overflow) continue
        if (total + value.length > invocation.outputCapBytes) {
          chunks.push(value.subarray(0, invocation.outputCapBytes - total))
          total = invocation.outputCapBytes
          overflow = true
          truncated = true
          stop()
          continue
        }
        chunks.push(value)
        total += value.length
      }
    } finally {
      reader.releaseLock()
    }
    return concatBytes(chunks)
  }

  const feed = async () => {
    const writer = child.stdin.getWriter()
    let releaseBlockedFeed: () => void = () => {}
    const canceled = new Promise<void>((resolve) => {
      releaseBlockedFeed = resolve
    })
    cancelers.push(() => {
      writer.abort().catch(() => {})
      releaseBlockedFeed()
    })
    const writing = (async () => {
      try {
        if (invocation.stdin.length > 0) await writer.write(invocation.stdin)
        await writer.close()
      } catch {
        // the child may exit without reading stdin
      } finally {
        writer.releaseLock()
      }
    })()
    await Promise.race([writing, canceled])
  }

  try {
    const statusPromise = child.status.then((status) => {
      exited = true
      return status
    })
    const [stdout, stderr, status] = await Promise.all([
      drain(child.stdout),
      drain(child.stderr),
      statusPromise,
      feed(),
    ])
    if (aborted) throw new AbortedError("run aborted")
    return {
      pid: child.pid,
      exit: status.signal != null
        ? { signal: status.signal }
        : { code: status.code },
      stdout,
      stderr,
      truncated,
      timedOut,
      durationMs: Math.round(performance.now() - started),
    }
  } finally {
    clearTimeout(deadline)
    if (graceTimer != null) clearTimeout(graceTimer)
    invocation.signal?.removeEventListener("abort", onAbort)
    // A direct child can exit while descendants still hold pipes, or after
    // redirecting them. Its exit never proves the process group is gone.
    killGroup(child.pid)
    if (!exited) {
      for (const cancel of cancelers) cancel()
      await child.status.catch(() => {})
    }
  }
}

/** Alive for our purposes means present and not a zombie awaiting reaping. */
export async function processAlive(pid: number): Promise<boolean> {
  try {
    const stat = await Deno.readTextFile(`/proc/${pid}/stat`)
    const state = stat.slice(
      stat.lastIndexOf(")") + 2,
      stat.lastIndexOf(")") + 3,
    )
    return state !== "Z" && state !== "X"
  } catch {
    return false
  }
}
