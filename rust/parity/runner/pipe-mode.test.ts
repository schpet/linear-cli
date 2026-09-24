import { assertEquals } from "@std/assert"
import { fromFileUrl, join } from "@std/path"
import { CASE_ROOT_PARENT, prepareConfinement, runConfined } from "./bwrap.ts"
import { sha256Hex } from "./bytes.ts"
import { compareObservation } from "./compare.ts"
import { createSandbox } from "./sandbox.ts"
import { testStatusHelper } from "./test-fixtures.ts"

const decoder = new TextDecoder()

Deno.test("native stdout consumer closes the target pipe at start and after N, while drain stays exact", async () => {
  const dir = await Deno.makeTempDir({
    dir: CASE_ROOT_PARENT,
    prefix: "linear-parity-pipe-",
  })
  const denoDir = join(dir, "deno-dir")
  await Deno.mkdir(denoDir)
  try {
    const confinement = await prepareConfinement({
      denoDir,
      statusHelper: await testStatusHelper(dir),
    })
    const sandbox = await createSandbox(dir, null)
    try {
      const invocation = {
        executable: "/usr/bin/perl",
        args: ["-e", 'for(;;) { syswrite(STDOUT, "abcd") or die "EPIPE" }'],
        readOnly: [],
        caseRoot: sandbox.root,
        cwd: sandbox.cwd,
        tmp: sandbox.tmp,
        env: { HOME: sandbox.home, PATH: "/usr/bin:/bin" },
        stdin: new Uint8Array(),
        timeoutMs: 5000,
        outputCapBytes: 1024 * 1024,
      }
      const closed = await runConfined(confinement, {
        ...invocation,
        stdoutMode: { mode: "closed-at-start" },
      })
      assertEquals(closed.stdout.length, 0)
      assertEquals(closed.targetExit, { signal: "SIGPIPE", number: 13 })
      assertEquals(closed.outerExit, { code: 141 })
      assertEquals(closed.stdoutClosure, {
        mode: "closed-at-start",
        count: 0,
        bytesRelayed: 0,
        closure: "before-start",
      })

      const after = await runConfined(confinement, {
        ...invocation,
        stdoutMode: { mode: "close-after-bytes", count: 4 },
      })
      assertEquals(decoder.decode(after.stdout), "abcd")
      assertEquals(after.targetExit, { signal: "SIGPIPE", number: 13 })
      assertEquals(after.outerExit, { code: 141 })
      assertEquals(after.stdoutClosure, {
        mode: "close-after-bytes",
        count: 4,
        bytesRelayed: 4,
        closure: "after-N",
      })

      const short = await runConfined(confinement, {
        ...invocation,
        args: ["-e", 'syswrite(STDOUT, "ab")'],
        stdoutMode: { mode: "close-after-bytes", count: 4 },
      })
      assertEquals(decoder.decode(short.stdout), "ab")
      assertEquals(short.targetExit, { code: 0 })
      assertEquals(short.stdoutClosure, {
        mode: "close-after-bytes",
        count: 4,
        bytesRelayed: 2,
        closure: "threshold-not-reached",
      })

      const stderr = await runConfined(confinement, {
        ...invocation,
        args: ["-e", 'print STDERR "after-close"; syswrite(STDOUT, "x")'],
        stdoutMode: { mode: "closed-at-start" },
      })
      assertEquals(decoder.decode(stderr.stderr), "after-close")
      assertEquals(stderr.targetExit, { signal: "SIGPIPE", number: 13 })

      const drain = await runConfined(confinement, {
        ...invocation,
        args: ["-e", 'syswrite(STDOUT, "abcd")'],
      })
      assertEquals(decoder.decode(drain.stdout), "abcd")
      assertEquals(drain.stdoutClosure, {
        mode: "drain",
        count: 0,
        bytesRelayed: 0,
        closure: "none",
      })

      const ignored = await runConfined(confinement, {
        ...invocation,
        args: [
          "-e",
          '$SIG{PIPE}="IGNORE"; for(;;) { my $n=syswrite(STDOUT, "abcd"); if(!defined($n)) { exit 77 } }',
        ],
        stdoutMode: { mode: "close-after-bytes", count: 4 },
      })
      assertEquals(ignored.targetExit, { code: 77 })
      assertEquals(ignored.stdoutClosure?.closure, "after-N")

      const consoleScript = join(sandbox.cwd, "console-loop.ts")
      await Deno.writeTextFile(consoleScript, 'for (;;) console.log("x")\n')
      const consoleLoop = await runConfined(confinement, {
        ...invocation,
        executable: Deno.execPath(),
        args: [
          "run",
          "--cached-only",
          "--no-config",
          "--no-lock",
          "--quiet",
          consoleScript,
        ],
        env: { ...invocation.env, DENO_DIR: denoDir },
        timeoutMs: 1000,
        stdoutMode: { mode: "closed-at-start" },
      })
      assertEquals(consoleLoop.timedOut, true)
      assertEquals(consoleLoop.targetExit, null)
      assertEquals(consoleLoop.stdoutClosure, null)
      assertEquals(consoleLoop.stdout.length, 0)
    } finally {
      await sandbox.remove()
    }
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("faulty helper that keeps draining after N cannot prove target closure", async () => {
  const dir = await Deno.makeTempDir({
    dir: CASE_ROOT_PARENT,
    prefix: "linear-parity-faulty-pipe-",
  })
  const denoDir = join(dir, "deno-dir")
  await Deno.mkdir(denoDir)
  try {
    const genuine = await testStatusHelper(dir)
    const source = await Deno.readTextFile(
      fromFileUrl(new URL("./helpers/status-helper.c", import.meta.url)),
    )
    const original =
      'if (close(stdout_pipe[0]) != 0) fail(EXIT_PROTOCOL, "close private read");'
    const faulty =
      'while (read(stdout_pipe[0], frame, sizeof frame) > 0) {}\n    if (close(stdout_pipe[0]) != 0) fail(EXIT_PROTOCOL, "close private read");'
    assertEquals(source.includes(original), true)
    const sourcePath = join(dir, "faulty-status-helper.c")
    const binaryPath = join(dir, "faulty-status-helper")
    await Deno.writeTextFile(sourcePath, source.replace(original, faulty))
    const compile = await new Deno.Command(genuine.compiler.path, {
      args: [...genuine.flags, "-o", binaryPath, sourcePath],
      clearEnv: true,
      env: { PATH: "/usr/bin:/bin" },
      stdout: "piped",
      stderr: "piped",
    }).output()
    assertEquals(compile.success, true, decoder.decode(compile.stderr))
    const faultyHelper = {
      ...genuine,
      path: binaryPath,
      binarySha256: await sha256Hex(await Deno.readFile(binaryPath)),
    }
    const confinement = await prepareConfinement({
      denoDir,
      statusHelper: faultyHelper,
    })
    const sandbox = await createSandbox(dir, null)
    try {
      const run = await runConfined(confinement, {
        executable: "/usr/bin/perl",
        args: ["-e", 'for(;;) { syswrite(STDOUT, "abcd") or die "EPIPE" }'],
        readOnly: [],
        caseRoot: sandbox.root,
        cwd: sandbox.cwd,
        tmp: sandbox.tmp,
        env: { HOME: sandbox.home, PATH: "/usr/bin:/bin" },
        stdin: new Uint8Array(),
        timeoutMs: 1000,
        outputCapBytes: 1024 * 1024,
        stdoutMode: { mode: "close-after-bytes", count: 4 },
      })
      assertEquals(decoder.decode(run.stdout), "abcd")
      assertEquals(run.timedOut, true)
      assertEquals(run.targetExit, null)
      assertEquals(run.stdoutClosure, null)

      // A dishonest helper can claim after-N after the target naturally
      // finishes. An expected target SIGPIPE still fails on the independent
      // authenticated exit surface; closure alone is not target EPIPE proof.
      const finite = await runConfined(confinement, {
        executable: "/usr/bin/perl",
        args: ["-e", 'for(1..1000) { syswrite(STDOUT, "abcd") }'],
        readOnly: [],
        caseRoot: sandbox.root,
        cwd: sandbox.cwd,
        tmp: sandbox.tmp,
        env: { HOME: sandbox.home, PATH: "/usr/bin:/bin" },
        stdin: new Uint8Array(),
        timeoutMs: 5000,
        outputCapBytes: 1024 * 1024,
        stdoutMode: { mode: "close-after-bytes", count: 4 },
      })
      assertEquals(finite.stdoutClosure?.closure, "after-N")
      assertEquals(finite.targetExit, { code: 0 })
      assertEquals(
        compareObservation(
          {
            exit: { signal: "SIGPIPE" },
            stdout: new TextEncoder().encode("abcd"),
            stdoutMode: { mode: "close-after-bytes", count: 4 },
            stderr: new Uint8Array(),
            fileEffects: [],
          },
          finite,
          [],
        ).map((mismatch) => mismatch.surface),
        ["exit"],
      )
    } finally {
      await sandbox.remove()
    }
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
