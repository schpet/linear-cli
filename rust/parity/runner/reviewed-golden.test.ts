import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { CASE_ROOT_PARENT, prepareConfinement } from "./bwrap.ts"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases, resolveCase } from "./cases.ts"
import {
  assertProposalOutsideGoldens,
  loadCandidate,
  parseOptions,
} from "./main.ts"
import type { Program } from "./program.ts"
import {
  countReviewedDeviationPasses,
  countReviewedGraphqlUserAgentPasses,
  toReportCase,
} from "./report.ts"
import { runCorpus } from "./run.ts"
import { parseCase, parseReviewedGolden, SchemaError } from "./schema.ts"
import { testStatusHelper, validCase } from "./test-fixtures.ts"

const CONTRACT = "rust-3.0.0-alpha.1"
const USER_AGENT = "schpet-linear-cli/3.0.0-alpha.1"
const GOLDEN_ID = "R01H-SYNTHETIC"

function golden(candidate: Record<string, unknown>, surfaces: string[]) {
  return {
    formatVersion: 1,
    caseId: "sample",
    deviationId: GOLDEN_ID,
    contract: CONTRACT,
    approvedSurfaces: surfaces,
    candidate,
  }
}

async function withCorpus(
  fn: (
    dir: string,
    write: (
      value: Record<string, unknown>,
      caseSpec?: Record<string, unknown>,
    ) => Promise<void>,
  ) => Promise<void>,
): Promise<void> {
  const dir = await Deno.makeTempDir({ prefix: "linear-reviewed-golden-" })
  const root = join(dir, "rust-goldens", CONTRACT)
  await Deno.mkdir(root, { recursive: true })
  const write = async (
    value: Record<string, unknown>,
    caseSpec = validCase(),
  ) => {
    const raw = JSON.stringify(value, null, 2) + "\n"
    await Deno.writeTextFile(join(root, "sample.json"), raw)
    const spec = { ...caseSpec }
    spec.deviation = {
      id: GOLDEN_ID,
      contract: CONTRACT,
      sha256: await sha256Hex(new TextEncoder().encode(raw)),
    }
    await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
  }
  try {
    await fn(dir, write)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

Deno.test("golden format v1 is closed and complete", () => {
  const valid = golden({ argv: ["--v3"] }, ["argv"])
  assertEquals(parseReviewedGolden(valid).formatVersion, 1)
  for (
    const mutation of [
      { ...valid, formatVersion: 2 },
      { ...valid, caseId: "../escape" },
      { ...valid, contract: "rust-next" },
      { ...valid, extra: 1 },
      { ...valid, candidate: { argv: ["--v3"], env: { X: "Y" } } },
      { ...valid, candidate: { timeoutMs: 1 } },
      { ...valid, candidate: { outputCapBytes: 1 } },
      { ...valid, candidate: { expected: { stdout: { utf8: "x" } } } },
      { ...valid, approvedSurfaces: ["argv", "argv"] },
      { ...valid, approvedSurfaces: ["wildcard"] },
      { ...valid, candidate: { graphqlUserAgent: "schpet-linear-cli/3.0.0" } },
    ]
  ) {
    assertThrows(() => parseReviewedGolden(mutation), SchemaError)
  }
  const spec = validCase()
  spec.deviation = { id: GOLDEN_ID, contract: CONTRACT, sha256: "a".repeat(64) }
  assertEquals(parseCase(spec).deviation?.id, GOLDEN_ID)
})

Deno.test("nested cwd root is allowed only in reviewed expected output", async () => {
  await withCorpus(async (dir, write) => {
    const spec = validCase()
    spec.gitProbe = "parent-root"
    spec.cwdSubdir = "subdir"
    const substitutions = spec.substitutions
    if (!Array.isArray(substitutions)) throw new Error("missing substitutions")
    substitutions.push("cwd")
    substitutions.push("cwdRoot")
    const expected = parseCase(spec).expected
    await write(
      golden({
        expected: {
          ...expected,
          stderr: { utf8: "root={{cwdRoot}} invoked={{cwd}}" },
        },
      }, ["stderr"]),
      spec,
    )
    const [loaded] = await loadCases(
      dir,
      new Set(["linear"]),
      undefined,
      CONTRACT,
    )
    assertEquals(
      candidateCaseView(loaded).spec.expected.stderr,
      { utf8: "root={{cwdRoot}} invoked={{cwd}}" },
    )
    await write(golden({ argv: ["{{cwdRoot}}"] }, ["argv"]), spec)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "cwdRoot is restricted to expected output",
    )
    await write(
      golden({
        expected: {
          ...expected,
          fileEffects: [{
            path: "link",
            change: "created",
            kind: "symlink",
            target: "{{cwdRoot}}",
          }],
        },
      }, ["files"]),
      spec,
    )
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "cwdRoot is restricted to expected output",
    )
  })
})

Deno.test("corpus validates SHA, identity, surfaces and orphan files before filtering", async () => {
  await withCorpus(async (dir, write) => {
    const routes = new Set(["linear"])
    const good = golden({ argv: ["--v3"] }, ["argv"])
    await write(good)
    const [loaded] = await loadCases(dir, routes, undefined, CONTRACT)
    assertEquals(loaded.golden?.spec.caseId, "sample")
    assertEquals(candidateCaseView(loaded).spec.argv, ["--v3"])
    assertEquals(loaded.spec.argv, ["--version"])
    await assertProposalOutsideGoldens(join(dir, "proposals"), dir)
    await assertProposalOutsideGoldens(join(dir, "sibling"), dir)
    await assertRejects(
      () =>
        assertProposalOutsideGoldens(
          join(dir, "rust-goldens", CONTRACT, "new"),
          dir,
        ),
      Error,
      "outside",
    )
    await assertRejects(
      () =>
        assertProposalOutsideGoldens(
          join(dir, "rust-goldens", "..x"),
          dir,
        ),
      Error,
      "outside",
    )
    await Deno.symlink(join(dir, "rust-goldens"), join(dir, "linked-goldens"))
    await assertRejects(
      () =>
        assertProposalOutsideGoldens(
          join(dir, "linked-goldens", "future"),
          dir,
        ),
      Error,
      "outside",
    )

    const path = join(dir, "rust-goldens", CONTRACT, "sample.json")
    const root = join(dir, "rust-goldens")
    const contractRoot = join(root, CONTRACT)
    const writeRaw = async (bytes: Uint8Array) => {
      await Deno.writeFile(path, bytes)
      const spec = validCase()
      spec.deviation = {
        id: GOLDEN_ID,
        contract: CONTRACT,
        sha256: await sha256Hex(bytes),
      }
      await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
    }
    await Deno.remove(path)
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "missing or unsafe",
    )
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "missing or unsafe",
    )
    await writeRaw(new TextEncoder().encode("{"))
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "not UTF-8 JSON",
    )
    await writeRaw(new Uint8Array([0xff]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "not UTF-8 JSON",
    )
    await write(good)
    await Deno.writeTextFile(path, JSON.stringify({ ...good, caseId: "wrong" }))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "SHA-256",
    )
    await write({ ...good, caseId: "wrong" })
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "identity",
    )
    await write(golden({ argv: ["--version"] }, ["argv"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "redundant",
    )
    await write(golden({ argv: ["--v3"] }, ["stdout"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "approvedSurfaces",
    )
    await write(golden({ argv: ["{{fixturePort}}"] }, ["argv"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "not declared",
    )
    const baseExpected = parseCase(validCase()).expected
    await write(golden({
      expected: {
        ...baseExpected,
        stderr: { utf8: "{{referenceModuleUrl}}/src/credentials.ts" },
      },
    }, ["stderr"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "Rust golden cannot use referenceModuleUrl",
    )
    await write(golden({ argv: ["{{referenceModuleUrl}}"] }, ["argv"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "Rust golden cannot use referenceModuleUrl",
    )
    await write(golden({
      expected: { ...baseExpected, stdout: { base64: "eA==" } },
    }, ["stdout"]))
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "redundant",
    )
    await write(
      golden({
        expected: {
          ...baseExpected,
          stdout: {
            mode: "close-after-bytes",
            count: 2,
            prefix: { utf8: "ab" },
          },
        },
      }, ["stdout"]),
      { ...validCase(), outputCapBytes: 1 },
    )
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "exceeds outputCapBytes",
    )
    await write(good)
    await Deno.writeTextFile(
      join(dir, "rust-goldens", CONTRACT, "orphan.json"),
      "{}",
    )
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "orphan",
    )
    await Deno.remove(join(dir, "rust-goldens", CONTRACT, "orphan.json"))
    await Deno.writeTextFile(join(contractRoot, "notes.txt"), "not a golden")
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "unsafe reviewed golden entry",
    )
    await Deno.remove(join(contractRoot, "notes.txt"))
    await Deno.mkdir(join(root, "rust-next"))
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "unknown or unsafe reviewed golden contract",
    )
    await Deno.remove(join(root, "rust-next"))
    await Deno.rename(contractRoot, join(root, "contract-real"))
    await Deno.symlink(join(root, "contract-real"), contractRoot)
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "missing or unsafe",
    )
    await Deno.remove(contractRoot)
    await Deno.rename(join(root, "contract-real"), contractRoot)
    await Deno.rename(root, join(dir, "goldens-real"))
    await Deno.symlink(join(dir, "goldens-real"), root)
    await assertRejects(
      () => loadCases(dir, routes, "unselected"),
      SchemaError,
      "missing or unsafe",
    )
    await Deno.remove(root)
    await Deno.rename(join(dir, "goldens-real"), root)
    await Deno.remove(path)
    await Deno.symlink(join(dir, "sample.json"), path)
    await assertRejects(
      () => loadCases(dir, routes, "unselected", CONTRACT),
      SchemaError,
      "unsafe",
    )
  })
})

Deno.test("all committed GraphQL cases bind exact Rust User-Agent without changing frozen fixtures", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set<string>(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest route has no path")
    }
    return route.path
  }))
  const cases = await loadCases(
    new URL("./cases", import.meta.url).pathname,
    routes,
    undefined,
    CONTRACT,
  )
  const graphql = cases.filter((loaded) => loaded.spec.graphql != null)
  assertEquals(graphql.length, 11)
  const substitutions = {
    home: "h",
    configHome: "c",
    cwd: "w",
    cwdRoot: "r",
    bin: "b",
    denoDir: "d",
    fixturePort: "1234",
    referenceModuleUrl: "file:///reference",
  }
  for (const loaded of graphql) {
    assertEquals(loaded.golden?.spec.candidate.graphqlUserAgent, USER_AGENT)
    assertEquals(loaded.golden?.spec.approvedSurfaces, ["graphql-user-agent"])
    const frozen = resolveCase(loaded.spec, substitutions).graphql
    const candidate = resolveCase(
      candidateCaseView(loaded).spec,
      substitutions,
      candidateCaseView(loaded).runtimeUserAgent,
    ).graphql
    assert(frozen != null && candidate != null)
    const expected = structuredClone(frozen)
    for (const group of expected.groups) {
      const steps = group.mode === "ordered"
        ? group.steps
        : group.lanes.flatMap((lane) => lane.steps)
      for (const step of steps) {
        if (step.kind === "graphql") {
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          step.identity.userAgent = USER_AGENT
        }
      }
    }
    assertEquals(candidate, expected, loaded.spec.id)
  }
  const frozen = await loadCases(
    new URL("./cases", import.meta.url).pathname,
    routes,
    "api-graphql-viewer",
  )
  assertEquals(frozen.length, 1)
  assertEquals(
    frozen[0].spec.argv,
    cases.find((loaded) => loaded.spec.id === "api-graphql-viewer")?.spec.argv,
  )
})

Deno.test("Rust contract requires an executable descriptor", async () => {
  const dir = await Deno.makeTempDir({ prefix: "linear-rust-contract-" })
  try {
    const path = join(dir, "candidate.json")
    await Deno.writeTextFile(
      path,
      JSON.stringify({
        name: "interpreted",
        contract: CONTRACT,
        program: { kind: "interpreted-reference", workspace: dir },
        implementedRoutes: ["linear"],
      }),
    )
    const options = parseOptions([
      "--reference",
      "/reference",
      "--reference-binary",
      "/binary",
      "--candidate",
      path,
    ])
    await assertRejects(
      () => loadCandidate(options, new Set(["linear"])),
      Error,
      "requires an executable",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("legacy fixtureServer cases cannot claim a GraphQL User-Agent override", async () => {
  await withCorpus(async (dir, write) => {
    const spec = validCase()
    spec.substitutions = ["home", "configHome", "bin", "denoDir", "fixturePort"]
    spec.fixtureServer = {
      path: "/graphql",
      responses: [{ status: 200, headers: {}, body: { utf8: "ok" } }],
      expectedRequests: 1,
      expectedAuthorization: "lin_api_fake",
    }
    await write(
      golden({ graphqlUserAgent: USER_AGENT }, ["graphql-user-agent"]),
      spec,
    )
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "requires a GraphQL fixture",
    )
  })
})

Deno.test("reviewed argv and byte differences use separate exact candidate view", async () => {
  await withCorpus(async (dir, write) => {
    const baseExpected = parseCase(validCase()).expected
    await write(golden({
      argv: ["--v3"],
      expected: { ...baseExpected, stdout: { utf8: "y" } },
    }, ["argv", "stdout"]))
    const [loaded] = await loadCases(
      dir,
      new Set(["linear"]),
      undefined,
      CONTRACT,
    )
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "reviewed-run-",
    })
    try {
      const denoDir = join(runDir, "deno-dir")
      await Deno.mkdir(denoDir)
      const confinement = await prepareConfinement({
        denoDir,
        statusHelper: await testStatusHelper(runDir),
      })
      const ctx = {
        denoDir,
        referenceBinary: join(runDir, "pinned-reference"),
        confinement,
        sandboxParent: runDir,
      }
      const good = join(runDir, "good.sh")
      await Deno.writeTextFile(
        good,
        '#!/bin/sh\nif [ "$1" = "--v3" ]; then printf y; else printf x; fi\n',
        { mode: 0o755 },
      )
      const bad = join(runDir, "bad.sh")
      await Deno.writeTextFile(bad, "#!/bin/sh\nprintf z\n", { mode: 0o755 })
      const baseline: Program = { kind: "executable", path: good }
      const selected = new Set(["linear"])
      const [pass] = await runCorpus([loaded], baseline, {
        name: "rust",
        contract: CONTRACT,
        program: baseline,
        implementedRoutes: selected,
      }, ctx)
      assertEquals(pass.status, "pass")
      assertEquals(pass.baseline.observation.stdoutBytes, 1)
      assertEquals(pass.candidate?.observation.stdoutBytes, 1)
      assertEquals(pass.reviewedDeviation?.approvedSurfaces, ["argv", "stdout"])
      assertEquals(countReviewedDeviationPasses([pass]), 1)
      assertEquals(countReviewedGraphqlUserAgentPasses([pass]), 0)
      assert(pass.reviewedDeviation != null)
      assertEquals(
        countReviewedGraphqlUserAgentPasses([{
          ...pass,
          reviewedDeviation: {
            ...pass.reviewedDeviation,
            approvedSurfaces: ["graphql-user-agent"],
          },
        }]),
        1,
      )
      assertEquals(
        toReportCase(pass).reviewedDeviation?.sha256,
        loaded.golden?.sha256,
      )

      const [frozen] = await runCorpus([loaded], baseline, {
        name: "frozen",
        program: baseline,
        implementedRoutes: selected,
      }, ctx)
      assertEquals(frozen.status, "pass")
      assertEquals(frozen.reviewedDeviation, null)

      const [failed] = await runCorpus([loaded], baseline, {
        name: "bad",
        contract: CONTRACT,
        program: { kind: "executable", path: bad },
        implementedRoutes: selected,
      }, ctx)
      assertEquals(failed.status, "fail")
      assertEquals(
        failed.candidate?.mismatches.map((mismatch) => mismatch.surface),
        ["stdout"],
      )
      assertEquals(countReviewedDeviationPasses([failed]), 0)

      for (
        const variant of [
          {
            name: "stderr",
            source: "#!/bin/sh\nprintf y\nprintf z >&2\n",
            surface: "stderr",
          },
          {
            name: "exit-code",
            source: "#!/bin/sh\nprintf y\nexit 3\n",
            surface: "exit",
          },
          {
            name: "file-effect",
            source: '#!/bin/sh\nprintf y\nprintf z > "$HOME/extra"\n',
            surface: "files",
          },
        ]
      ) {
        const executable = join(runDir, `${variant.name}.sh`)
        await Deno.writeTextFile(executable, variant.source, { mode: 0o755 })
        const [result] = await runCorpus([loaded], baseline, {
          name: variant.name,
          contract: CONTRACT,
          program: { kind: "executable", path: executable },
          implementedRoutes: selected,
        }, ctx)
        assertEquals(result.status, "fail", variant.name)
        assertEquals(
          result.candidate?.mismatches.map((mismatch) => mismatch.surface),
          [variant.surface],
          variant.name,
        )
      }

      await write(golden({
        argv: ["--v3"],
        expected: {
          ...baseExpected,
          stdout: { utf8: "y" },
          exit: { signal: "SIGTERM" },
        },
      }, ["argv", "exit", "stdout"]))
      const [signalGolden] = await loadCases(dir, selected, undefined, CONTRACT)
      const exited = join(runDir, "exit-143.sh")
      await Deno.writeTextFile(exited, "#!/bin/sh\nprintf y\nexit 143\n", {
        mode: 0o755,
      })
      const [codeVsSignal] = await runCorpus([signalGolden], baseline, {
        name: "code versus signal",
        contract: CONTRACT,
        program: { kind: "executable", path: exited },
        implementedRoutes: selected,
      }, ctx)
      assertEquals(codeVsSignal.status, "fail")
      assertEquals(
        codeVsSignal.candidate?.mismatches.map((mismatch) => mismatch.surface),
        ["exit"],
      )

      const [unimplemented] = await runCorpus([loaded], baseline, {
        name: "missing",
        contract: CONTRACT,
        program: { kind: "executable", path: bad },
        implementedRoutes: new Set(),
      }, ctx)
      assertEquals(unimplemented.status, "not-implemented")
      assertEquals(unimplemented.candidate, null)

      const [drift] = await runCorpus([loaded], {
        kind: "executable",
        path: bad,
      }, {
        name: "rust",
        contract: CONTRACT,
        program: baseline,
        implementedRoutes: selected,
      }, ctx)
      assertEquals(drift.status, "baseline-drift")
      assertEquals(drift.candidate, null)
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})

Deno.test("GraphQL candidate fixture requires exact v3 identity and preserves request matching", async () => {
  await withCorpus(async (dir, write) => {
    const source = JSON.parse(
      await Deno.readTextFile(
        new URL("./cases/api-graphql-viewer.json", import.meta.url),
      ),
    )
    const request = '{"query":"{ viewer { id } }"}'
    const argv = (
      userAgent: string,
      authorization = "lin_api_fake",
      body = request,
    ) => [
      "--silent",
      "--show-error",
      "--noproxy",
      "*",
      "--request",
      "POST",
      "--header",
      "content-type: application/json",
      "--header",
      `authorization: ${authorization}`,
      "--header",
      `user-agent: ${userAgent}`,
      "--data-raw",
      body,
      "http://127.0.0.1:{{fixturePort}}/graphql",
    ]
    const caseSpec = {
      ...source,
      id: "sample",
      route: "linear",
      argv: argv("schpet-linear-cli/2.6.0"),
    }
    await write(golden({ argv: argv(USER_AGENT) }, ["argv"]), caseSpec)
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), "unselected", CONTRACT),
      SchemaError,
      "requires exact GraphQL User-Agent binding",
    )
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "reviewed-graphql-",
    })
    try {
      const denoDir = join(runDir, "deno-dir")
      await Deno.mkdir(denoDir)
      const ctx = {
        denoDir,
        referenceBinary: join(runDir, "pinned-reference"),
        confinement: await prepareConfinement({
          denoDir,
          statusHelper: await testStatusHelper(runDir),
        }),
        sandboxParent: runDir,
      }
      const program: Program = { kind: "executable", path: "/usr/bin/curl" }
      const selected = new Set(["linear"])
      const run = async (candidateArgv: string[]) => {
        await write(
          golden({
            argv: candidateArgv,
            graphqlUserAgent: USER_AGENT,
          }, ["argv", "graphql-user-agent"]),
          caseSpec,
        )
        const [loaded] = await loadCases(dir, selected, undefined, CONTRACT)
        const [result] = await runCorpus([loaded], program, {
          name: "synthetic v3",
          contract: CONTRACT,
          program,
          implementedRoutes: selected,
        }, ctx)
        assertEquals(result.baseline.mismatches, [])
        assertEquals(result.baseline.fixture?.userAgents, [
          "schpet-linear-cli/2.6.0",
        ])
        return result
      }
      const pass = await run(argv(USER_AGENT))
      assertEquals(
        pass.status,
        "pass",
        JSON.stringify(pass.candidate?.mismatches),
      )
      assertEquals(pass.candidate?.fixture?.userAgents, [USER_AGENT])
      assertEquals(pass.candidate?.fixture?.graphqlRequests, 1)

      for (
        const invalid of [
          argv("schpet-linear-cli/3.0.0"),
          argv(USER_AGENT, "lin_api_fake_wrong"),
          argv(
            USER_AGENT,
            "lin_api_fake",
            '{"query":"{ viewer { id } }","variables":{"x":1}}',
          ),
          [...argv(USER_AGENT), "http://127.0.0.1:{{fixturePort}}/graphql"],
        ]
      ) {
        const failed = await run(invalid)
        assertEquals(failed.status, "fail")
        assert(
          failed.candidate?.mismatches.some((mismatch) =>
            mismatch.surface === "fixture"
          ),
        )
      }
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})
