import { assert, assertEquals, assertRejects } from "@std/assert"
import { join } from "@std/path"
import {
  AbortedError,
  type Invocation,
  processAlive,
  runIsolated,
} from "./engine.ts"

const decoder = new TextDecoder()
const baseEnv = { PATH: "/usr/bin:/bin" }

async function withTemp<T>(fn: (dir: string) => Promise<T>): Promise<T> {
  const dir = await Deno.makeTempDir({ prefix: "linear-parity-engine-" })
  try {
    return await fn(dir)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

function shell(
  script: string,
  dir: string,
  extra: Partial<Invocation> = {},
): Invocation {
  return {
    executable: "/bin/sh",
    args: ["-c", script],
    cwd: dir,
    env: baseEnv,
    stdin: new Uint8Array(),
    timeoutMs: 10_000,
    outputCapBytes: 16 * 1024 * 1024,
    ...extra,
  }
}

async function waitDead(pid: number): Promise<boolean> {
  for (let i = 0; i < 50; i++) {
    if (!(await processAlive(pid))) return true
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  return false
}

Deno.test("child runs in its own session and process group with only the explicit environment", async () => {
  await withTemp(async (dir) => {
    const result = await runIsolated(
      shell("cat /proc/$$/stat; echo ---; env", dir, {
        env: { ...baseEnv, PARITY_MARKER: "yes" },
      }),
    )
    assertEquals(result.exit, { code: 0 })
    const [stat, env] = decoder.decode(result.stdout).split("---\n")
    const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ")
    const pgid = Number(fields[2])
    const sid = Number(fields[3])
    assertEquals(pgid, result.pid)
    assertEquals(sid, result.pid)
    const lines = env.trim().split("\n").sort()
    assertEquals(
      lines,
      ["PARITY_MARKER=yes", "PATH=/usr/bin:/bin", "PWD=" + dir].sort(),
    )
  })
})

Deno.test("deadline kills the whole group including a sleeping grandchild", async () => {
  await withTemp(async (dir) => {
    const result = await runIsolated(
      shell("sleep 300 & echo $!; wait", dir, { timeoutMs: 500 }),
    )
    assertEquals(result.timedOut, true)
    assertEquals(result.truncated, false)
    assertEquals(result.exit, { signal: "SIGKILL" })
    const grandchild = Number(decoder.decode(result.stdout).trim())
    assert(grandchild > 1, "grandchild pid was reported")
    assert(
      await waitDead(grandchild),
      `grandchild ${grandchild} survived the group kill`,
    )
    assert(result.durationMs < 5000, "returned promptly after the deadline")
  })
})

Deno.test("deadline kills a grandchild holding pipes after its parent exits", async () => {
  await withTemp(async (dir) => {
    const result = await runIsolated(
      shell("sleep 300 & echo $!", dir, { timeoutMs: 400 }),
    )
    const grandchild = Number(decoder.decode(result.stdout).trim())
    assertEquals(result.exit, { code: 0 })
    assertEquals(result.timedOut, true)
    assert(result.durationMs < 4000)
    assert(await waitDead(grandchild), `grandchild ${grandchild} survived`)
  })
})

Deno.test("successful parent exit cleans up a grandchild with redirected pipes", async () => {
  await withTemp(async (dir) => {
    const pidFile = join(dir, "redirected.pid")
    const result = await runIsolated(
      shell(`sleep 300 >/dev/null 2>&1 & echo $! > ${pidFile}`, dir),
    )
    const grandchild = Number((await Deno.readTextFile(pidFile)).trim())
    assertEquals(result.exit, { code: 0 })
    assertEquals(result.timedOut, false)
    assert(await waitDead(grandchild), `grandchild ${grandchild} survived`)
  })
})

Deno.test("output cap truncates, kills the group including a grandchild, and is distinct from timeout", async () => {
  await withTemp(async (dir) => {
    const cap = 100_000
    const result = await runIsolated(
      shell("sleep 300 & echo $!; head -c 5000000 /dev/zero; wait", dir, {
        outputCapBytes: cap,
        timeoutMs: 10_000,
      }),
    )
    assertEquals(result.truncated, true)
    assertEquals(result.timedOut, false)
    assertEquals(result.stdout.length, cap)
    assertEquals(result.exit, { signal: "SIGKILL" })
    const grandchild = Number(
      decoder.decode(result.stdout.subarray(0, 16)).split("\n")[0],
    )
    assert(grandchild > 1, "grandchild pid was reported")
    assert(
      await waitDead(grandchild),
      `grandchild ${grandchild} survived the group kill`,
    )
  })
})

Deno.test("10 MB on stderr with an idle stdout is drained without deadlock or truncation", async () => {
  await withTemp(async (dir) => {
    const result = await runIsolated(
      shell("head -c 10485760 /dev/zero >&2", dir),
    )
    assertEquals(result.exit, { code: 0 })
    assertEquals(result.stdout.length, 0)
    assertEquals(result.stderr.length, 10_485_760)
    assertEquals(result.truncated, false)
    assertEquals(result.timedOut, false)
  })
})

Deno.test("a signal exit is reported as a signal, not a code", async () => {
  await withTemp(async (dir) => {
    const result = await runIsolated(shell("kill -TERM $$", dir))
    assertEquals(result.exit, { signal: "SIGTERM" })
    assertEquals(result.timedOut, false)
  })
})

Deno.test("stdin bytes are delivered exactly and stdout/stderr stay separate", async () => {
  await withTemp(async (dir) => {
    const stdin = new Uint8Array([0x00, 0xff, 0x61, 0x0a, 0x1b])
    const result = await runIsolated({
      ...shell("", dir),
      executable: "/bin/sh",
      args: ["-c", "cat; echo err >&2"],
      stdin,
    })
    assertEquals(result.exit, { code: 0 })
    assertEquals(Array.from(result.stdout), Array.from(stdin))
    assertEquals(decoder.decode(result.stderr), "err\n")
  })
})

Deno.test("deadline releases a blocked large stdin feed", async () => {
  await withTemp(async (dir) => {
    const result = await runIsolated(shell("sleep 300", dir, {
      stdin: new Uint8Array(16 * 1024 * 1024),
      timeoutMs: 300,
    }))
    assertEquals(result.timedOut, true)
    assertEquals(result.exit, { signal: "SIGKILL" })
    assert(result.durationMs < 4000, "blocked stdin held the runner")
  })
})

Deno.test("an abort kills the group and rejects; the grandchild does not survive", async () => {
  await withTemp(async (dir) => {
    const pidFile = join(dir, "grandchild.pid")
    const abort = new AbortController()
    setTimeout(() => abort.abort(), 400)
    await assertRejects(
      () =>
        runIsolated(
          shell(`sleep 300 & echo $! > ${pidFile}; wait`, dir, {
            signal: abort.signal,
          }),
        ),
      AbortedError,
    )
    const grandchild = Number((await Deno.readTextFile(pidFile)).trim())
    assert(grandchild > 1, "grandchild pid was recorded")
    assert(
      await waitDead(grandchild),
      `grandchild ${grandchild} survived the abort`,
    )
  })
})

Deno.test("a missing executable is observed as a failed exec, not a runner crash", async () => {
  await withTemp(async (dir) => {
    const result = await runIsolated({
      ...shell("", dir),
      executable: join(dir, "does-not-exist"),
      args: [],
    })
    assertEquals(result.exit, { code: 127 })
    assert(decoder.decode(result.stderr).includes("does-not-exist"))
  })
})
