import { assertEquals, assertRejects } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { CASE_ROOT_PARENT, prepareConfinement, runConfined } from "./bwrap.ts"
import type { CaseSpec } from "./schema.ts"
import { testStatusHelper } from "./test-fixtures.ts"
import {
  createSandbox,
  diffTrees,
  gitHelperIntegrity,
  gitProbeExpected,
  gitProbeScript,
  hashTree,
} from "./sandbox.ts"

Deno.test("sandbox copies the cwd fixture, hashes the tree and reports exact file effects", async () => {
  const fixture = await Deno.makeTempDir({ prefix: "linear-parity-fixture-" })
  const parent = await Deno.makeTempDir({ prefix: "linear-parity-parent-" })
  try {
    await Deno.mkdir(join(fixture, "sub"))
    await Deno.writeTextFile(join(fixture, "sub/keep.txt"), "keep")
    await Deno.writeTextFile(join(fixture, "change.txt"), "before")
    const sandbox = await createSandbox(parent, fixture)
    assertEquals(
      await Deno.readTextFile(join(sandbox.cwd, "sub/keep.txt")),
      "keep",
    )
    for (
      const dir of [sandbox.home, sandbox.configHome, sandbox.bin, sandbox.tmp]
    ) {
      assertEquals((await Deno.stat(dir)).isDirectory, true)
    }
    const before = await hashTree(sandbox.root)
    assertEquals([...before.keys()], [
      "bin",
      "config",
      "cwd",
      "cwd/change.txt",
      "cwd/sub",
      "cwd/sub/keep.txt",
      "home",
      "tmp",
    ])

    await Deno.writeTextFile(join(sandbox.cwd, "change.txt"), "after")
    await Deno.remove(join(sandbox.cwd, "sub/keep.txt"))
    await Deno.mkdir(join(sandbox.home, ".config"))
    await Deno.writeTextFile(join(sandbox.home, ".config/new.toml"), "x = 1")
    await Deno.symlink("change.txt", join(sandbox.cwd, "link"))
    const effects = diffTrees(before, await hashTree(sandbox.root))
    assertEquals(effects, [
      {
        path: "cwd/change.txt",
        change: "modified",
        kind: "file",
        sha256: await sha256Hex(new TextEncoder().encode("after")),
      },
      {
        path: "cwd/link",
        change: "created",
        kind: "symlink",
        target: "change.txt",
      },
      { path: "cwd/sub/keep.txt", change: "removed", kind: "file" },
      { path: "home/.config", change: "created", kind: "directory" },
      {
        path: "home/.config/new.toml",
        change: "created",
        kind: "file",
        sha256: await sha256Hex(new TextEncoder().encode("x = 1")),
      },
    ])
    assertEquals(diffTrees(before, before), [])

    await sandbox.remove()
    await assertRejects(() => Deno.stat(sandbox.root), Deno.errors.NotFound)
  } finally {
    await Deno.remove(fixture, { recursive: true })
    await Deno.remove(parent, { recursive: true })
  }
})

Deno.test("symlink targets are compared exactly and special entries fail promptly", async () => {
  const root = await Deno.makeTempDir({ prefix: "linear-parity-special-" })
  try {
    await Deno.symlink("first", join(root, "link"))
    const before = await hashTree(root)
    await Deno.remove(join(root, "link"))
    await Deno.symlink("second", join(root, "link"))
    assertEquals(diffTrees(before, await hashTree(root)), [{
      path: "link",
      change: "modified",
      kind: "symlink",
      target: "second",
    }])
    const fifo = join(root, "pipe")
    const made = await new Deno.Command("/usr/bin/mkfifo", { args: [fifo] })
      .output()
    assertEquals(made.success, true)
    await assertRejects(
      () => hashTree(root),
      Error,
      "unsupported sandbox entry pipe",
    )
  } finally {
    await Deno.remove(root, { recursive: true })
  }
})

Deno.test("a missing fixture directory leaves no sandbox behind", async () => {
  const parent = await Deno.makeTempDir({ prefix: "linear-parity-parent-" })
  try {
    await assertRejects(() =>
      createSandbox(parent, join(parent, "missing-fixture"))
    )
    const entries: string[] = []
    for await (const entry of Deno.readDir(parent)) entries.push(entry.name)
    assertEquals(entries, [])
  } finally {
    await Deno.remove(parent, { recursive: true })
  }
})

Deno.test("closed Git probes stage a fixed executable before the baseline hash", async () => {
  const parent = await Deno.makeTempDir({ prefix: "linear-parity-git-probe-" })
  try {
    const probes: Array<NonNullable<CaseSpec["gitProbe"]>> = [
      "parent-root",
      "outside-repo",
    ]
    for (const probe of probes) {
      const sandbox = await createSandbox(parent, null, null, probe, "subdir")
      try {
        assertEquals(sandbox.invocationCwd, join(sandbox.cwd, "subdir"))
        assertEquals((await Deno.stat(sandbox.invocationCwd)).isDirectory, true)
        assertEquals(sandbox.gitHelperPath, join(sandbox.bin, "git"))
        const helper = sandbox.gitHelperPath
        if (helper == null) throw new Error("Git helper was not staged")
        assertEquals(await Deno.readTextFile(helper), gitProbeScript(probe))
        const mode = (await Deno.lstat(helper)).mode
        if (mode == null) throw new Error("Git helper has no file mode")
        assertEquals(mode & 0o777, 0o500)
        const before = await hashTree(sandbox.root)
        assertEquals(before.get("bin/git"), {
          kind: "file",
          sha256: await sha256Hex(
            new TextEncoder().encode(gitProbeScript(probe)),
          ),
        })
        const correct = await new Deno.Command(helper, {
          args: ["rev-parse", "--show-toplevel"],
          cwd: sandbox.invocationCwd,
          clearEnv: true,
          env: { PATH: sandbox.bin },
        }).output()
        assertEquals(correct.code, probe === "parent-root" ? 0 : 128)
        assertEquals(
          new TextDecoder().decode(correct.stdout),
          probe === "parent-root" ? `${sandbox.cwd}\n` : "",
        )
        assertEquals(
          new TextDecoder().decode(correct.stderr),
          gitProbeExpected(probe, sandbox.cwd).stderr,
        )
        const wrong = await new Deno.Command(helper, {
          args: ["status"],
          cwd: sandbox.invocationCwd,
          clearEnv: true,
          env: { PATH: sandbox.bin },
        }).output()
        assertEquals(wrong.code, 129)
        assertEquals(wrong.stdout.length, 0)
        assertEquals(wrong.stderr.length, 0)
        assertEquals(diffTrees(before, await hashTree(sandbox.root)), [])
        assertEquals(await gitHelperIntegrity(sandbox, probe), null)
        await Deno.chmod(helper, 0o700)
        assertEquals(
          await gitHelperIntegrity(sandbox, probe),
          "private Git probe helper type or mode changed",
        )
        await Deno.writeTextFile(helper, "#!/bin/sh\nexit 0\n")
        await Deno.chmod(helper, 0o500)
        assertEquals(
          await gitHelperIntegrity(sandbox, probe),
          "private Git probe helper content changed",
        )
      } finally {
        await sandbox.remove()
      }
    }
  } finally {
    await Deno.remove(parent, { recursive: true })
  }
})

Deno.test("both fixed Git modes execute under real Bubblewrap and reject wrong argv", async () => {
  const parent = await Deno.makeTempDir({
    dir: CASE_ROOT_PARENT,
    prefix: "linear-parity-git-bwrap-",
  })
  try {
    const denoDir = join(parent, "deno-dir")
    await Deno.mkdir(denoDir)
    const helper = await testStatusHelper(parent)
    const confinement = await prepareConfinement({
      denoDir,
      statusHelper: helper,
    })
    const modes: Array<NonNullable<CaseSpec["gitProbe"]>> = [
      "parent-root",
      "outside-repo",
    ]
    for (const mode of modes) {
      const sandbox = await createSandbox(parent, null, null, mode, "subdir")
      try {
        const git = sandbox.gitHelperPath
        if (git == null) throw new Error("Git helper was not staged")
        const before = await hashTree(sandbox.root)
        for (
          const args of [
            ["rev-parse", "--show-toplevel"],
            ["status"],
          ]
        ) {
          const result = await runConfined(confinement, {
            executable: "/bin/sh",
            args: ["-c", 'exec "$@"', "git-probe", git, ...args],
            readOnly: [],
            caseRoot: sandbox.root,
            cwd: sandbox.invocationCwd,
            tmp: sandbox.tmp,
            env: { PATH: sandbox.bin },
            stdin: new Uint8Array(),
            timeoutMs: 5_000,
            outputCapBytes: 4_096,
          })
          const expected = args[0] === "status"
            ? { code: 129, stdout: "", stderr: "" }
            : gitProbeExpected(mode, sandbox.cwd)
          assertEquals(result.targetExit, { code: expected.code })
          assertEquals(new TextDecoder().decode(result.stdout), expected.stdout)
          assertEquals(new TextDecoder().decode(result.stderr), expected.stderr)
          assertEquals(result.timedOut, false)
          assertEquals(result.truncated, false)
        }
        await Deno.chmod(git, 0o400)
        const unexecutable = await runConfined(confinement, {
          executable: "/bin/sh",
          args: [
            "-c",
            'exec "$@"',
            "git-probe",
            git,
            "rev-parse",
            "--show-toplevel",
          ],
          readOnly: [],
          caseRoot: sandbox.root,
          cwd: sandbox.invocationCwd,
          tmp: sandbox.tmp,
          env: { PATH: sandbox.bin },
          stdin: new Uint8Array(),
          timeoutMs: 5_000,
          outputCapBytes: 4_096,
        })
        assertEquals(unexecutable.targetExit, { code: 126 })
        await Deno.chmod(git, 0o500)
        assertEquals(diffTrees(before, await hashTree(sandbox.root)), [])
        assertEquals(await gitHelperIntegrity(sandbox, mode), null)
      } finally {
        await sandbox.remove()
      }
    }
  } finally {
    await Deno.remove(parent, { recursive: true })
  }
})

Deno.test("Git probe pairing and nested cwd type fail without leaked sandboxes", async () => {
  const parent = await Deno.makeTempDir({
    prefix: "linear-parity-git-invalid-",
  })
  const fixture = await Deno.makeTempDir({
    prefix: "linear-parity-git-fixture-",
  })
  try {
    await assertRejects(
      () => createSandbox(parent, null, null, "parent-root"),
      Error,
      "gitProbe and cwdSubdir must be specified together",
    )
    await Deno.writeTextFile(join(fixture, "subdir"), "not a directory")
    await assertRejects(
      () => createSandbox(parent, fixture, null, "parent-root", "subdir"),
      Error,
      "cwdSubdir must be a real directory",
    )
    await Deno.remove(join(fixture, "subdir"))
    await Deno.symlink("elsewhere", join(fixture, "subdir"))
    await assertRejects(
      () => createSandbox(parent, fixture, null, "outside-repo", "subdir"),
      Error,
      "cwdSubdir must be a real directory",
    )
    const remaining = []
    for await (const entry of Deno.readDir(parent)) remaining.push(entry.name)
    assertEquals(remaining, [])
  } finally {
    await Deno.remove(parent, { recursive: true })
    await Deno.remove(fixture, { recursive: true })
  }
})
