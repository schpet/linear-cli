// Integration of sandbox, confinement wrapper, engine, fixture server,
// comparison and descriptor logic with small shell programs; the namespace
// lane is exercised by `deno task parity` itself.
import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { BaselineCache } from "./baseline-cache.ts"
import { CASE_ROOT_PARENT, prepareConfinement } from "./bwrap.ts"
import type { LoadedCase } from "./cases.ts"
import { loadCases } from "./cases.ts"
import {
  executeCase,
  referenceModuleUrl,
  type RunContext,
  runCorpus,
} from "./run.ts"
import type { Program } from "./program.ts"
import { readManifest } from "../verify.ts"
import { toReportCase } from "./report.ts"
import { parseCase, SchemaError } from "./schema.ts"
import { testStatusHelper, validCase } from "./test-fixtures.ts"

async function withDir<T>(
  fn: (dir: string, ctx: RunContext) => T | Promise<T>,
): Promise<T> {
  const dir = await Deno.makeTempDir({
    dir: CASE_ROOT_PARENT,
    prefix: "linear-parity-run-",
  })
  const denoDir = join(dir, "deno-dir")
  await Deno.mkdir(denoDir)
  try {
    const confinement = await prepareConfinement({
      denoDir,
      statusHelper: await testStatusHelper(dir),
    })
    return await fn(dir, {
      denoDir,
      referenceBinary: join(dir, "pinned-reference"),
      confinement,
      sandboxParent: dir,
    })
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

async function script(
  dir: string,
  name: string,
  body: string,
): Promise<string> {
  const path = join(dir, `${name}.sh`)
  await Deno.writeTextFile(path, `#!/bin/sh\n${body}\n`, { mode: 0o755 })
  return path
}

function loadedCase(overrides: Record<string, unknown>): LoadedCase {
  const spec = parseCase({ ...validCase(), ...overrides })
  return {
    file: `${spec.id}.json`,
    spec,
    fixtureDir: null,
    configFixtureDir: null,
  }
}

Deno.test("reference module URL distinguishes interpreted and pinned compiled programs", async () => {
  await withDir((_dir, ctx) => {
    assertEquals(
      referenceModuleUrl({
        kind: "interpreted-reference",
        workspace: "/tmp/reference dir/",
        deno: "/bin/deno",
      }, ctx),
      "file:///tmp/reference%20dir",
    )
    assertEquals(
      referenceModuleUrl(
        { kind: "executable", path: ctx.referenceBinary },
        ctx,
      ),
      "file:///tmp/deno-compile-reference-linear",
    )
    assertThrows(
      () => referenceModuleUrl({ kind: "executable", path: "/tmp/other" }, ctx),
      SchemaError,
      "requires the interpreted or pinned compiled reference",
    )
  })
})

Deno.test("nested cwd substitutions resolve to distinct invocation and fixture roots", async () => {
  await withDir(async (_dir, ctx) => {
    const loaded = loadedCase({
      id: "cwd-root-public",
      gitProbe: "parent-root",
      cwdSubdir: "subdir",
      argv: ["-c", "pwd -P; cd -P ..; pwd -P"],
      substitutions: ["home", "configHome", "cwd", "cwdRoot", "bin", "denoDir"],
      expected: {
        exit: { code: 0 },
        stdout: { utf8: "{{cwd}}\n{{cwdRoot}}\n" },
        stderr: { utf8: "" },
        fileEffects: [],
      },
    })
    const result = await executeCase(
      loaded,
      { kind: "executable", path: "/bin/sh" },
      ctx,
    )
    assertEquals(result.mismatches, [])
  })
})

Deno.test("public case runner compares pipe prefix, authenticated closure and effective cap", async () => {
  await withDir(async (_dir, ctx) => {
    const loop = 'for(;;) { syswrite(STDOUT, "abcd") or die "EPIPE" }'
    const make = (stdout: unknown, argv: string[], exit: unknown) =>
      loadedCase({
        id: "pipe-public",
        argv,
        outputCapBytes: 1024,
        expected: {
          exit,
          stdout,
          stderr: { utf8: "" },
          fileEffects: [],
        },
      })
    const program: Program = { kind: "executable", path: "/usr/bin/perl" }
    const closed = await executeCase(
      make({ mode: "closed-at-start" }, ["-e", loop], {
        signal: "SIGPIPE",
      }),
      program,
      ctx,
    )
    assertEquals(closed.mismatches, [])
    assertEquals(closed.observation.stdoutClosure?.closure, "before-start")

    const after = await executeCase(
      make(
        { mode: "close-after-bytes", count: 4, prefix: { utf8: "abcd" } },
        ["-e", loop],
        { signal: "SIGPIPE" },
      ),
      program,
      ctx,
    )
    assertEquals(after.mismatches, [])
    assertEquals(after.observation.stdoutClosure?.closure, "after-N")

    const short = make(
      { mode: "close-after-bytes", count: 4, prefix: { utf8: "abcd" } },
      ["-e", 'syswrite(STDOUT, "ab")'],
      { code: 0 },
    )
    const shortRun = await executeCase(short, program, ctx)
    assertEquals(shortRun.mismatches.map((m) => m.surface), [
      "stdout",
      "stdout",
    ])
    assert(
      shortRun.mismatches.some((m) =>
        m.detail.includes("threshold-not-reached")
      ),
    )
    await assertRejects(
      () =>
        executeCase(short, program, {
          ...ctx,
          limits: { outputCapBytes: 3 },
        }),
      SchemaError,
      "effective outputCapBytes",
    )
  })
})

Deno.test("a missing declared asset remains an unconsumed fixture interaction", async () => {
  await withDir(async (_dir, ctx) => {
    const loaded = loadedCase({
      substitutions: ["home", "configHome", "bin", "denoDir", "fixturePort"],
      graphql: {
        path: "/graphql",
        schemaSha256:
          "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
        expectedRequests: 1,
        initialRecords: {},
        expectedRecords: {},
        groups: [{
          mode: "ordered",
          steps: [{
            kind: "asset",
            id: "file",
            method: "GET",
            path: "/file",
            requiredHeaders: {},
            forbiddenHeaders: [],
            body: { utf8: "" },
            response: { status: 200, headers: {}, body: { utf8: "data" } },
          }],
        }],
      },
    })
    const result = await executeCase(loaded, {
      kind: "executable",
      path: "/bin/true",
    }, ctx)
    assertEquals(
      result.mismatches.some((item) =>
        item.surface === "fixture" && item.detail.includes("consumed 0")
      ),
      true,
    )
    assertEquals(result.fixture?.assetRequests, 0)
  })
})

Deno.test("descriptor decides not-implemented without invoking the candidate; claimed routes must pass or fail", async () => {
  await withDir(async (dir, ctx) => {
    const marker = join(dir, "invoked.marker")
    const good = await script(dir, "good", `printf 'hello'`)
    const bad = await script(dir, "bad", `printf 'hello'; exit 3`)
    const spy = await script(
      dir,
      "spy",
      `printf 'x' > ${marker}; printf 'hello'`,
    )
    const loaded = loadedCase({
      id: "hello",
      route: "linear",
      argv: [],
      expected: {
        exit: { code: 0 },
        stdout: { utf8: "hello" },
        stderr: { utf8: "" },
        fileEffects: [],
      },
    })
    const baseline: Program = { kind: "executable", path: good }

    const [identical] = await runCorpus([loaded], baseline, {
      name: "same",
      program: baseline,
      implementedRoutes: new Set(["linear"]),
    }, ctx)
    assertEquals(identical.status, "pass")
    assertEquals(identical.candidate?.mismatches, [])

    const [missing] = await runCorpus([loaded], baseline, {
      name: "spy",
      program: { kind: "executable", path: spy },
      implementedRoutes: new Set(["linear api"]),
    }, ctx)
    assertEquals(missing.status, "not-implemented")
    assertEquals(missing.candidate, null)
    assertEquals(await Deno.stat(marker).then(() => true, () => false), false)

    const [claimed] = await runCorpus([loaded], baseline, {
      name: "bad",
      program: { kind: "executable", path: bad },
      implementedRoutes: new Set(["linear"]),
    }, ctx)
    assertEquals(claimed.status, "fail")
    assertEquals(claimed.candidate?.mismatches.map((m) => m.surface), ["exit"])

    const [drift] = await runCorpus(
      [loaded],
      { kind: "executable", path: bad },
      {
        name: "same",
        program: baseline,
        implementedRoutes: new Set(["linear"]),
      },
      ctx,
    )
    assertEquals(drift.status, "baseline-drift")
    assertEquals(drift.candidate, null)
    assertEquals(drift.baseline.mismatches.map((m) => m.surface), ["exit"])
  })
})

Deno.test("executeCase distinguishes exit 143 from SIGTERM in both directions and reports both exits", async () => {
  await withDir(async (dir, ctx) => {
    const exits143 = await script(dir, "exits143", `printf out; exit 143`)
    const sigterm = await script(dir, "sigterm", `printf out; kill -TERM $$`)
    const expectCode = loadedCase({
      id: "code",
      argv: [],
      expected: {
        exit: { code: 143 },
        stdout: { utf8: "out" },
        stderr: { utf8: "" },
        fileEffects: [],
      },
    })
    const expectSignal = loadedCase({
      id: "signal",
      argv: [],
      expected: {
        exit: { signal: "SIGTERM" },
        stdout: { utf8: "out" },
        stderr: { utf8: "" },
        fileEffects: [],
      },
    })
    const codeRun = await executeCase(expectCode, {
      kind: "executable",
      path: exits143,
    }, ctx)
    assertEquals(codeRun.mismatches, [])
    assertEquals(codeRun.observation.targetExit, { code: 143 })
    assertEquals(codeRun.observation.outerExit, { code: 143 })
    const signalRun = await executeCase(expectSignal, {
      kind: "executable",
      path: sigterm,
    }, ctx)
    assertEquals(signalRun.mismatches, [])
    assertEquals(signalRun.observation.targetExit, {
      signal: "SIGTERM",
      number: 15,
    })
    assertEquals(signalRun.observation.outerExit, { code: 143 })
    assertEquals(signalRun.observation.targetStatus, {
      helperPid: 2,
      targetPid: 3,
    })
    const crossed = await executeCase(expectCode, {
      kind: "executable",
      path: sigterm,
    }, ctx)
    assertEquals(crossed.mismatches.map((m) => m.surface), ["exit"])
    assert(crossed.mismatches[0].detail.includes("got signal SIGTERM"))
    const crossedBack = await executeCase(expectSignal, {
      kind: "executable",
      path: exits143,
    }, ctx)
    assertEquals(crossedBack.mismatches.map((m) => m.surface), ["exit"])
    assert(crossedBack.mismatches[0].detail.includes("got code 143"))
  })
})

Deno.test("sandbox paths, file effects and sanitized evidence flow through a case run", async () => {
  await withDir(async (dir, ctx) => {
    const writer = await script(
      dir,
      "writer",
      `printf 'k = 1' > "$HOME/config.toml"; case "$PWD" in */cwd) printf ok ;; esac`,
    )
    const sha =
      "d6c92dbc3ea62b6b32ac6ea33b7b1ea9f6f8bf1f2e9d3c9d3b5bde7d9e58d2f2"
    const loaded = loadedCase({
      id: "writer",
      argv: [],
      expected: {
        exit: { code: 0 },
        stdout: { utf8: "ok" },
        stderr: { utf8: "" },
        fileEffects: [{
          path: "home/config.toml",
          change: "created",
          kind: "file",
          sha256: sha,
        }],
      },
    })
    const program: Program = { kind: "executable", path: writer }
    const [result] = await runCorpus([loaded], program, {
      name: "w",
      program,
      implementedRoutes: new Set(["linear"]),
    }, ctx)
    assertEquals(result.status, "baseline-drift")
    const detail = result.baseline.mismatches[0].detail
    assert(
      detail.startsWith(
        "file effects differ; missing [created file home/config.toml " + sha +
          "] unexpected [created file home/config.toml ",
      ),
      detail,
    )
    assert(!detail.includes(dir), "evidence must not leak sandbox paths")
    assertEquals(result.baseline.fileEffects.length, 1)
    const entries: string[] = []
    for await (const entry of Deno.readDir(dir)) entries.push(entry.name)
    assertEquals(
      entries.filter((name) => name.startsWith("linear-parity-case-")),
      [],
      "sandboxes are removed",
    )
  })
})

Deno.test("a child-created FIFO fails only its case on the files surface", async () => {
  await withDir(async (dir, ctx) => {
    const baseline: Program = {
      kind: "executable",
      path: await script(dir, "baseline", "printf x"),
    }
    const candidate: Program = {
      kind: "executable",
      path: await script(
        dir,
        "candidate",
        'if [ "$1" = fifo ]; then /usr/bin/mkfifo "$HOME/pipe"; fi; printf x',
      ),
    }
    const expected = {
      exit: { code: 0 },
      stdout: { utf8: "x" },
      stderr: { utf8: "" },
      fileEffects: [],
    }
    const first = loadedCase({ id: "fifo", argv: ["fifo"], expected })
    const second = loadedCase({ id: "normal", argv: ["normal"], expected })
    const results = await runCorpus([first, second], baseline, {
      name: "candidate",
      program: candidate,
      implementedRoutes: new Set(["linear"]),
    }, ctx)
    assertEquals(results.map((result) => result.status), ["fail", "pass"])
    assertEquals(results[0].candidate?.mismatches.map((item) => item.surface), [
      "files",
    ])
    assert(results[0].candidate?.mismatches[0].detail.includes("home/pipe"))
    const leftovers: string[] = []
    for await (const entry of Deno.readDir(dir)) {
      if (entry.name.startsWith("linear-parity-case-")) {
        leftovers.push(entry.name)
      }
    }
    assertEquals(leftovers, [])
  })
})

Deno.test("fixture server port substitution reaches argv, env and expected output, and Authorization is checked", async () => {
  await withDir(async (dir, ctx) => {
    // A bash loopback client: only /usr is visible inside the sandbox.
    const client = join(dir, "client.sh")
    await Deno.writeTextFile(
      client,
      `#!/usr/bin/bash
port="\${URL##*:}"; port="\${port%%/*}"
exec 3<>"/dev/tcp/127.0.0.1/$port"
printf 'POST /graphql HTTP/1.0\r\nauthorization: %s\r\ncontent-length: 2\r\n\r\n{}' "$KEY" >&3
IFS= read -r status <&3
/usr/bin/sed '1,/^\r$/d' <&3
case "$status" in *" 200 "*) exit 0 ;; *) exit 1 ;; esac
`,
      { mode: 0o755 },
    )
    const program: Program = { kind: "executable", path: client }
    const withKey = loadedCase({
      id: "fixture",
      argv: [],
      env: {
        ...parseCase(validCase()).env,
        URL: "http://127.0.0.1:{{fixturePort}}/graphql",
        KEY: "lin_api_fake",
      },
      substitutions: ["home", "configHome", "bin", "denoDir", "fixturePort"],
      fixtureServer: {
        path: "/graphql",
        responses: [{
          status: 200,
          headers: {},
          body: { utf8: "port {{fixturePort}}" },
        }],
        expectedRequests: 1,
        expectedAuthorization: "lin_api_fake",
      },
      expected: {
        exit: { code: 0 },
        stdout: { utf8: "port {{fixturePort}}" },
        stderr: { utf8: "" },
        fileEffects: [],
      },
    })
    const [ok] = await runCorpus([withKey], program, {
      name: "c",
      program,
      implementedRoutes: new Set(["linear"]),
    }, ctx)
    assertEquals(ok.status, "pass", JSON.stringify(ok.baseline.mismatches))
    assertEquals(ok.baseline.fixture?.authorizationMatched, [true])
    assert(!JSON.stringify(toReportCase(ok)).includes("lin_api_fake"))

    const withoutKey = loadedCase({
      ...withKey.spec,
      env: { ...withKey.spec.env, KEY: "" },
    })
    const [noAuth] = await runCorpus([withoutKey], program, {
      name: "c",
      program,
      implementedRoutes: new Set(["linear"]),
    }, ctx)
    assertEquals(noAuth.status, "baseline-drift")
    assertEquals(noAuth.baseline.mismatches.map((m) => m.surface), ["fixture"])

    // Count mismatch is rejected at case loading, before either child runs.
  })
})

Deno.test("the committed corpus loads against the manifest and rejects a broken file", async () => {
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set<string>(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest route has no path")
    }
    return route.path
  }))
  const cases = await loadCases(
    new URL("./cases", import.meta.url).pathname,
    routes,
  )
  assert(cases.length >= 13)
  assert(cases.some((loaded) => loaded.spec.fixtureServer != null))
  const filtered = await loadCases(
    new URL("./cases", import.meta.url).pathname,
    routes,
    "loopback",
  )
  assertEquals(
    filtered.every((loaded) => loaded.spec.id.includes("loopback")),
    true,
  )

  const dir = await Deno.makeTempDir({ prefix: "linear-parity-cases-" })
  try {
    await Deno.writeTextFile(
      join(dir, "sample.json"),
      JSON.stringify({ ...validCase(), route: "linear nope" }),
    )
    await loadCases(dir, routes).then(
      () => assert(false, "unknown route accepted"),
      (error) =>
        assert(String(error).includes("not in rust/parity/manifest.json")),
    )
    await Deno.writeTextFile(
      join(dir, "sample.json"),
      JSON.stringify({ ...validCase(), argv: ["{{cwd}}"] }),
    )
    await loadCases(dir, routes).then(
      () => assert(false, "undeclared placeholder accepted"),
      (error) => assert(String(error).includes("not declared")),
    )
    await Deno.writeTextFile(
      join(dir, "sample.json"),
      JSON.stringify({ ...validCase(), id: "other" }),
    )
    await loadCases(dir, routes).then(
      () => assert(false, "id/file mismatch accepted"),
      (error) => assert(String(error).includes("does not match the file name")),
    )
    await Deno.writeTextFile(
      join(dir, "sample.json"),
      JSON.stringify({ ...validCase(), cwdFixture: "missing" }),
    )
    await loadCases(dir, routes).then(
      () => assert(false, "missing fixture accepted"),
      (error) => assert(String(error).includes("fixture directory")),
    )
    await Deno.mkdir(join(dir, "fixtures"))
    await Deno.symlink(dir, join(dir, "fixtures", "linked"))
    await Deno.writeTextFile(
      join(dir, "sample.json"),
      JSON.stringify({ ...validCase(), cwdFixture: "linked" }),
    )
    await loadCases(dir, routes).then(
      () => assert(false, "symlinked fixture accepted"),
      (error) => assert(String(error).includes("fixture directory")),
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("warm baseline proof skips source invocation, candidate executes and rejects broken controls with dynamic sandbox paths", async () => {
  await withDir(async (dir, ctx) => {
    const source = await script(dir, "cache-source", 'printf "%s\\n" "$PWD"')
    const candidatePath = await script(
      dir,
      "cache-candidate",
      'printf "%s\\n" "$PWD"',
    )
    const loaded = loadedCase({
      substitutions: [...parseCase(validCase()).substitutions, "cwd"],
      expected: {
        ...parseCase(validCase()).expected,
        stdout: { utf8: "{{cwd}}\n" },
      },
    })
    loaded.file = join(dir, "cache-case.json")
    await Deno.writeTextFile(loaded.file, JSON.stringify(loaded.spec))
    const cache = new BaselineCache(join(dir, "cache"), "bound test source")
    const baseline: Program = { kind: "executable", path: source }
    const candidate = {
      name: "candidate",
      program: { kind: "executable", path: candidatePath },
      implementedRoutes: new Set([loaded.spec.route]),
    }
    // Explicit Program type preserves strict discriminant typing.
    const candidateProgram: Program = {
      kind: "executable",
      path: candidatePath,
    }
    const descriptor = { ...candidate, program: candidateProgram }
    const [cold] = await runCorpus(
      [loaded],
      baseline,
      descriptor,
      ctx,
      undefined,
      cache,
    )
    assertEquals(cold.status, "pass")
    await Deno.remove(source)
    const [warm] = await runCorpus(
      [loaded],
      baseline,
      descriptor,
      ctx,
      undefined,
      cache,
    )
    assertEquals(warm.status, "pass")
    assertEquals(warm.baseline, cold.baseline)
    assertEquals(warm.baselineEvidence?.origin, "cache")
    assert(warm.candidate != null)
    // Candidate remains fresh and independently resolves its new sandbox path.
    assert(
      new TextDecoder().decode(warm.candidate.raw.stdout) !==
        new TextDecoder().decode(warm.baseline.raw.stdout),
    )
    await script(dir, "cache-candidate", "printf broken")
    const [negative] = await runCorpus(
      [loaded],
      baseline,
      descriptor,
      ctx,
      undefined,
      cache,
    )
    assertEquals(negative.status, "fail")
    assertEquals(negative.candidate?.mismatches.map((m) => m.surface), [
      "stdout",
    ])
    assertEquals(cache.metrics.baselineExecutions, 1)
    assertEquals(cache.metrics.hits, 2)
    await assertRejects(() =>
      runCorpus(
        [loaded],
        baseline,
        descriptor,
        ctx,
        undefined,
        new BaselineCache(cache.directory, cache.identity, true),
      )
    )
  })
})
