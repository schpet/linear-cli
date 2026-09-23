// Wrapper tests: argv construction, bind validation, denial canaries through
// the real wrapper, and process cleanup inside the nested PID namespace.
// These run outside the lane; nothing here touches the network.
import { assert, assertEquals, assertRejects } from "@std/assert"
import { fromFileUrl, join } from "@std/path"
import {
  bwrapArgs,
  bwrapDiagnostic,
  CASE_ROOT_PARENT,
  type Confinement,
  ConfinementError,
  HARNESS_ENV,
  planConfinement,
  prepareConfinement,
  programInvocation,
  runConfined,
  SANDBOX_HOSTNAME,
  validateBindSource,
} from "./bwrap.ts"
import { AbortedError } from "./engine.ts"
import { probeConfinement } from "./preflight.ts"
import { createSandbox, type Sandbox } from "./sandbox.ts"

const decoder = new TextDecoder()

interface Lane {
  dir: string
  denoDir: string
  confinement: Confinement
}

async function withLane<T>(fn: (lane: Lane) => Promise<T>): Promise<T> {
  const dir = await Deno.makeTempDir({
    dir: CASE_ROOT_PARENT,
    prefix: "linear-parity-bwrap-",
  })
  const denoDir = join(dir, "deno-dir")
  await Deno.mkdir(denoDir)
  try {
    return await fn({
      dir,
      denoDir,
      confinement: await prepareConfinement({ denoDir }),
    })
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

async function withSandbox<T>(
  lane: Lane,
  fn: (sandbox: Sandbox) => Promise<T>,
): Promise<T> {
  const sandbox = await createSandbox(lane.dir, null)
  try {
    return await fn(sandbox)
  } finally {
    await sandbox.remove().catch(() => {})
  }
}

function shell(
  sandbox: Sandbox,
  script: string,
  extra: Partial<Parameters<typeof runConfined>[1]> = {},
): Parameters<typeof runConfined>[1] {
  return {
    executable: "/bin/sh",
    args: ["-c", script],
    readOnly: [],
    caseRoot: sandbox.root,
    cwd: sandbox.cwd,
    tmp: sandbox.tmp,
    env: { PATH: "/usr/bin:/bin", HOME: sandbox.home },
    stdin: new Uint8Array(),
    timeoutMs: 10_000,
    outputCapBytes: 16 * 1024 * 1024,
    ...extra,
  }
}

/** Host-wide search for live processes whose argv mentions the token. */
async function survivors(token: string): Promise<number[]> {
  const pids: number[] = []
  for await (const entry of Deno.readDir("/proc")) {
    if (!/^\d+$/.test(entry.name)) continue
    const cmdline = await Deno.readFile(`/proc/${entry.name}/cmdline`).catch(
      () => null,
    )
    if (cmdline != null && decoder.decode(cmdline).includes(token)) {
      pids.push(Number(entry.name))
    }
  }
  return pids
}

async function waitGone(token: string): Promise<boolean> {
  for (let i = 0; i < 50; i++) {
    if ((await survivors(token)).length === 0) return true
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  return false
}

async function waitPresent(token: string): Promise<boolean> {
  for (let i = 0; i < 100; i++) {
    if ((await survivors(token)).length > 0) return true
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  return false
}

/** An escaped descendant: its own session and process group, no pipes. */
function escaped(token: string): string {
  return `/usr/bin/setsid /bin/sleep ${token} </dev/null >/dev/null 2>&1 &`
}

Deno.test("bwrap argv is an allowlisted root in mount order: /usr read-only with /usr/local masked, private proc/dev, case root writable at its own path, case tmp on /tmp, program binds read-only, remount-ro last", () => {
  const confinement: Confinement = {
    bwrap: "/usr/bin/bwrap",
    version: "bubblewrap 0.9.0",
    systemLinks: [{ path: "/bin", target: "usr/bin" }, {
      path: "/lib64",
      target: "usr/lib64",
    }],
    systemBinds: ["/lib"],
    sharedReadOnly: [],
  }
  const args = bwrapArgs(confinement, {
    caseRoot: "/var/tmp/lane/case",
    cwd: "/var/tmp/lane/case/cwd",
    tmp: "/var/tmp/lane/case/tmp",
    readOnly: [
      {
        source: "/opt/real/deno",
        dest: "/home/u/bin/deno",
        kind: "executable",
      },
      { source: "/home/u/stage", dest: "/home/u/stage", kind: "directory" },
    ],
    env: {
      PATH: "/var/tmp/lane/case/bin",
      DENO_NO_UPDATE_CHECK: "1",
      HOME: "/h",
    },
    executable: "/home/u/bin/deno",
    args: ["run", "x"],
  })
  assertEquals(args, [
    "--unshare-user",
    "--unshare-pid",
    "--unshare-uts",
    "--hostname",
    SANDBOX_HOSTNAME,
    "--uid",
    "1000",
    "--gid",
    "1000",
    "--cap-drop",
    "ALL",
    "--disable-userns",
    "--assert-userns-disabled",
    "--die-with-parent",
    "--clearenv",
    "--setenv",
    "DENO_NO_UPDATE_CHECK",
    "1",
    "--setenv",
    "HOME",
    "/h",
    "--setenv",
    "PATH",
    "/var/tmp/lane/case/bin",
    "--ro-bind",
    "/usr",
    "/usr",
    "--tmpfs",
    "/usr/local",
    "--remount-ro",
    "/usr/local",
    "--symlink",
    "usr/bin",
    "/bin",
    "--symlink",
    "usr/lib64",
    "/lib64",
    "--ro-bind",
    "/lib",
    "/lib",
    "--proc",
    "/proc",
    "--dev",
    "/dev",
    "--bind",
    "/var/tmp/lane/case",
    "/var/tmp/lane/case",
    "--bind",
    "/var/tmp/lane/case/tmp",
    "/tmp",
    "--ro-bind",
    "/opt/real/deno",
    "/home/u/bin/deno",
    "--ro-bind",
    "/home/u/stage",
    "/home/u/stage",
    "--remount-ro",
    "/",
    "--chdir",
    "/var/tmp/lane/case/cwd",
    "--",
    "/home/u/bin/deno",
    "run",
    "x",
  ])
  for (
    const forbidden of [
      "--new-session",
      "--as-pid-1",
      "--unshare-net",
      "--share-net",
      "--bind /home",
      "--ro-bind /home /home",
      "--ro-bind /etc",
    ]
  ) {
    assert(
      !args.join(" ").includes(forbidden),
      `argv must not contain ${forbidden}`,
    )
  }
})

Deno.test("bind sources are validated: missing, non-executable, virtual, host temp, masked and whole-tree paths are refused", async () => {
  await withLane(async (lane) => {
    const plain = join(lane.dir, "plain.txt")
    await Deno.writeTextFile(plain, "x")
    const executable = join(lane.dir, "prog.sh")
    await Deno.writeTextFile(executable, "#!/bin/sh\n", { mode: 0o755 })
    assertEquals(await validateBindSource(executable, "executable"), {
      source: executable,
      dest: executable,
      kind: "executable",
    })
    assertEquals(await validateBindSource(lane.denoDir, "directory"), {
      source: lane.denoDir,
      dest: lane.denoDir,
      kind: "directory",
    })
    const refused: Array<
      [string, Parameters<typeof validateBindSource>[1], string]
    > = [
      [join(lane.dir, "missing"), "file", "does not exist"],
      [plain, "executable", "not executable"],
      [plain, "directory", "not a directory"],
      [lane.dir, "file", "not a regular file"],
      ["relative/path", "file", "must be absolute"],
      ["/proc/self/exe", "executable", "is under /proc"],
      ["/dev/null", "file", "is under /dev"],
      ["/tmp", "directory", "is under /tmp"],
      ["/run", "directory", "is under /run"],
      ["/etc/passwd", "file", "is under /etc"],
      ["/usr/local/bin/anything", "executable", "/usr/local"],
      ["/", "directory", "whole tree"],
      ["/home", "directory", "whole tree"],
      ["/usr", "directory", "whole tree"],
      ["/var/tmp", "directory", "whole tree"],
    ]
    for (const [path, kind, message] of refused) {
      await assertRejects(
        () => validateBindSource(path, kind),
        ConfinementError,
        message,
        path,
      )
    }
    // A symlink into a forbidden tree is caught through its realpath.
    const link = join(lane.dir, "sneaky")
    await Deno.symlink("/etc/passwd", link)
    await assertRejects(
      () => validateBindSource(link, "file"),
      ConfinementError,
      "is under /etc",
    )
  })
})

Deno.test("plans reject case roots under /tmp, paths outside the case root, programs inside it, harness-constant overrides and system shadowing", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const base = shell(sandbox, "true")
      const plan = await planConfinement(lane.confinement, base)
      assertEquals(plan.env, { ...base.env, ...HARNESS_ENV })
      assertEquals(plan.readOnly, [{
        source: lane.denoDir,
        dest: lane.denoDir,
        kind: "directory",
      }], "/bin/sh needs no bind; only the shared stage is bound")
      await assertRejects(
        () =>
          planConfinement(lane.confinement, {
            ...base,
            env: { ...base.env, DENO_NO_UPDATE_CHECK: "0" },
          }),
        ConfinementError,
        "harness constant",
      )
      await assertRejects(
        () => planConfinement(lane.confinement, { ...base, cwd: lane.dir }),
        ConfinementError,
        "cwd must be inside the case root",
      )
      await assertRejects(
        () => planConfinement(lane.confinement, { ...base, tmp: sandbox.root }),
        ConfinementError,
        "tmp must be inside the case root",
      )
      const inside = join(sandbox.bin, "inside.sh")
      await Deno.writeTextFile(inside, "#!/bin/sh\n", { mode: 0o755 })
      await assertRejects(
        () =>
          planConfinement(lane.confinement, { ...base, executable: inside }),
        ConfinementError,
        "inside the case root",
      )
      await assertRejects(
        () =>
          planConfinement(lane.confinement, {
            ...base,
            readOnly: ["/usr/bin/env"],
            executable: "/usr/bin/env",
          }),
        ConfinementError,
        "shadow the system tree",
      )
    })
    const tmpRoot = await Deno.makeTempDir({ prefix: "linear-parity-tmp-" })
    try {
      for (const dir of ["cwd", "tmp"]) await Deno.mkdir(join(tmpRoot, dir))
      await assertRejects(
        () =>
          planConfinement(lane.confinement, {
            executable: "/bin/sh",
            args: ["-c", "true"],
            readOnly: [],
            caseRoot: tmpRoot,
            cwd: join(tmpRoot, "cwd"),
            tmp: join(tmpRoot, "tmp"),
            env: {},
            stdin: new Uint8Array(),
            timeoutMs: 1000,
            outputCapBytes: 1024,
          }),
        ConfinementError,
        "never /tmp",
      )
    } finally {
      await Deno.remove(tmpRoot, { recursive: true })
    }
  })
})

Deno.test("plans reject symlinked case cwd and tmp bind sources", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const invocation = shell(sandbox, "true")
      await Deno.remove(sandbox.tmp)
      await Deno.symlink(lane.dir, sandbox.tmp)
      await assertRejects(
        () => planConfinement(lane.confinement, invocation),
        ConfinementError,
        "tmp must be a realpath",
      )
      await Deno.remove(sandbox.tmp)
      await Deno.mkdir(sandbox.tmp)
      await Deno.remove(sandbox.cwd)
      await Deno.symlink(lane.dir, sandbox.cwd)
      await assertRejects(
        () => planConfinement(lane.confinement, invocation),
        ConfinementError,
        "cwd must be a realpath",
      )
    })
  })
})

Deno.test("an interpreted reference binds deno, deno.json, deno.lock and src; an executable binds only itself", () => {
  assertEquals(
    programInvocation(
      { kind: "interpreted-reference", workspace: "/w", deno: "/d/deno" },
      ["--version"],
    ),
    {
      executable: "/d/deno",
      args: [
        "run",
        "--cached-only",
        "--frozen",
        "--allow-all",
        "--quiet",
        "--config",
        "/w/deno.json",
        "/w/src/main.ts",
        "--version",
      ],
      readOnly: ["/w/deno.json", "/w/deno.lock", "/w/src"],
    },
  )
  assertEquals(
    programInvocation({ kind: "executable", path: "/x/linear" }, ["a"]),
    {
      executable: "/x/linear",
      args: ["a"],
      readOnly: [],
    },
  )
})

Deno.test("the filesystem canary passes through the real wrapper and a forbidden bind is a harness error, not case output", async () => {
  await withLane(async (lane) => {
    const before: string[] = []
    for await (const entry of Deno.readDir(lane.dir)) before.push(entry.name)
    const probed = await probeConfinement({
      confinement: lane.confinement,
      denoPath: Deno.execPath(),
      denoDir: lane.denoDir,
      laneDir: lane.dir,
    })
    assertEquals(probed.executableProbe, null)
    assertEquals(probed.confinement.allowedRead, "allowed")
    assert(String(probed.confinement.markerRead).startsWith("NotFound"))
    assert(String(probed.confinement.socket).startsWith("NotFound"))
    assert(
      String(probed.confinement.rootWrite).includes("Read-only file system"),
    )
    assert(
      String(probed.confinement.hostHomeWrite).includes(
        "Read-only file system",
      ),
    )
    assertEquals(probed.confinement.hostname, SANDBOX_HOSTNAME)
    assertEquals(
      probed.confinement.binShRealpath,
      await Deno.realPath("/bin/sh"),
    )
    const after: string[] = []
    for await (const entry of Deno.readDir(lane.dir)) after.push(entry.name)
    assertEquals(after.sort(), before.sort(), "preflight leaves nothing behind")

    // Negative control: one bad bind and the run is a ConfinementError.
    await withSandbox(lane, async (sandbox) => {
      const broken: Confinement = {
        ...lane.confinement,
        systemBinds: [
          ...lane.confinement.systemBinds,
          join(lane.dir, "absent"),
        ],
      }
      await assertRejects(
        () => runConfined(broken, shell(sandbox, "echo should-not-run")),
        ConfinementError,
        "bwrap:",
      )
    })
    assertEquals(
      bwrapDiagnostic({
        pid: 1,
        exit: { code: 1 },
        stdout: new Uint8Array(),
        stderr: new TextEncoder().encode("bwrap: Can't find source path /x\n"),
        truncated: false,
        timedOut: false,
        durationMs: 1,
      }),
      "bwrap: Can't find source path /x",
    )
    assertEquals(
      bwrapDiagnostic({
        pid: 1,
        exit: { code: 1 },
        stdout: new TextEncoder().encode("x"),
        stderr: new TextEncoder().encode("bwrap: not really\n"),
        truncated: false,
        timedOut: false,
        durationMs: 1,
      }),
      null,
    )
  })
})

Deno.test("the child sees exactly the case environment plus harness constants, its /tmp writes land in the case tmp/, and /usr/local is empty", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const result = await runConfined(
        lane.confinement,
        shell(
          sandbox,
          "env | sort; printf leak > /tmp/note.txt; ls /usr/local | wc -l; cat /proc/1/comm",
          { env: { PATH: "/usr/bin:/bin", CASE_MARKER: "yes" } },
        ),
      )
      assertEquals(result.exit, { code: 0 })
      assertEquals(decoder.decode(result.stdout).trim().split("\n"), [
        "CASE_MARKER=yes",
        "DENO_NO_UPDATE_CHECK=1",
        "PATH=/usr/bin:/bin",
        `PWD=${sandbox.cwd}`,
        "0",
        "bwrap",
      ])
      assertEquals(
        await Deno.readTextFile(join(sandbox.tmp, "note.txt")),
        "leak",
      )
    })
  })
})

Deno.test("a program that exits on a signal is reported as code 128+n by bwrap's reaper (P04 limitation)", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const result = await runConfined(
        lane.confinement,
        shell(sandbox, "kill -TERM $$"),
      )
      assertEquals(result.exit, { code: 143 })
      assertEquals(result.timedOut, false)
    })
  })
})

Deno.test("deadline kills the sandbox including a setsid-escaped grandchild", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const token = "4801"
      assertEquals(await survivors(token), [])
      const result = await runConfined(
        lane.confinement,
        shell(sandbox, `${escaped(token)} /bin/sleep 300`, { timeoutMs: 500 }),
      )
      assertEquals(result.timedOut, true)
      assertEquals(result.exit, { signal: "SIGKILL" })
      assert(await waitGone(token), "escaped grandchild survived the deadline")
      assert(result.durationMs < 5000)
    })
  })
})

Deno.test("output cap kills the sandbox including a setsid-escaped grandchild and is distinct from timeout", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const token = "4802"
      const cap = 100_000
      const result = await runConfined(
        lane.confinement,
        shell(
          sandbox,
          `${escaped(token)} /usr/bin/head -c 5000000 /dev/zero; wait`,
          {
            outputCapBytes: cap,
          },
        ),
      )
      assertEquals(result.truncated, true)
      assertEquals(result.timedOut, false)
      assertEquals(result.stdout.length, cap)
      assert(
        await waitGone(token),
        "escaped grandchild survived the output cap",
      )
    })
  })
})

Deno.test("an abort kills the sandbox including a setsid-escaped grandchild and rejects", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const token = "4803"
      const abort = new AbortController()
      setTimeout(() => abort.abort(), 400)
      await assertRejects(
        () =>
          runConfined(
            lane.confinement,
            shell(sandbox, `${escaped(token)} /bin/sleep 300`, {
              signal: abort.signal,
            }),
          ),
        AbortedError,
      )
      assert(await waitGone(token), "escaped grandchild survived the abort")
    })
  })
})

Deno.test("a normal exit tears down the sandbox: a setsid-escaped grandchild with no pipes does not outlive the case", async () => {
  await withLane(async (lane) => {
    await withSandbox(lane, async (sandbox) => {
      const token = "4804"
      const result = await runConfined(
        lane.confinement,
        shell(sandbox, `${escaped(token)} printf done`),
      )
      assertEquals(result.exit, { code: 0 })
      assertEquals(decoder.decode(result.stdout), "done")
      assert(await waitGone(token), "escaped grandchild outlived the case")
    })
  })
})

Deno.test("killing the runner itself kills the sandbox (die-with-parent through setsid)", async () => {
  await withLane(async (lane) => {
    const token = "4805"
    const helper = join(lane.dir, "helper.ts")
    const runnerDir = fromFileUrl(new URL("./", import.meta.url))
    await Deno.writeTextFile(
      helper,
      `import { prepareConfinement, runConfined } from ${
        JSON.stringify(join(runnerDir, "bwrap.ts"))
      }
import { createSandbox } from ${JSON.stringify(join(runnerDir, "sandbox.ts"))}
const [lane, denoDir] = Deno.args
const confinement = await prepareConfinement({ denoDir })
const sandbox = await createSandbox(lane, null)
await runConfined(confinement, {
  executable: "/bin/sh",
  args: ["-c", "exec /bin/sleep ${token}"],
  readOnly: [],
  caseRoot: sandbox.root,
  cwd: sandbox.cwd,
  tmp: sandbox.tmp,
  env: { PATH: "/usr/bin:/bin" },
  stdin: new Uint8Array(),
  timeoutMs: 60_000,
  outputCapBytes: 1024,
})
`,
    )
    const child = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--cached-only",
        "--frozen",
        "--allow-all",
        "--quiet",
        "--config",
        join(runnerDir, "..", "deno.json"),
        helper,
        lane.dir,
        lane.denoDir,
      ],
      stdin: "null",
      stdout: "null",
      stderr: "piped",
    }).spawn()
    const stderr = new Response(child.stderr).text()
    try {
      assert(await waitPresent(token), "confined sleep never appeared")
      child.kill("SIGKILL")
      await child.status
      assert(await waitGone(token), "sandbox survived the runner's death")
    } finally {
      await stderr.catch(() => "")
    }
  })
})
