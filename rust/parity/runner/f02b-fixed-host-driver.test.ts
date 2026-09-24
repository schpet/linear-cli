import { assert, assertEquals, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { loadCases } from "./cases.ts"
import {
  CASES_DIR,
  checkCaseTable,
  evaluateCase,
  parseOptions,
  type ProbeCase,
  repeatsIdentically,
  TABLE,
} from "./f02b-fixed-host-driver.ts"
import type { CaseRun } from "./run.ts"
import { FROZEN_USER_AGENT } from "./schema.ts"

const required = [
  "--probe",
  "/probe",
  "--reference",
  "/reference",
  "--reference-binary",
  "/binary",
]

async function manifestRoutes(): Promise<Set<string>> {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(
        new URL("../manifest.json", import.meta.url),
      ),
    ),
  )
  const routes = new Set<string>()
  for (const route of manifest.routes) {
    if (typeof route.path === "string") routes.add(route.path)
  }
  return routes
}

function run(overrides: Partial<CaseRun> = {}): CaseRun {
  return {
    program: "executable /probe",
    mismatches: [],
    observation: {
      targetExit: { code: 0 },
      outerExit: { code: 0 },
      targetStatus: { helperPid: 2, targetPid: 3 },
      stdoutClosure: {
        mode: "drain",
        count: 0,
        bytesRelayed: 0,
        closure: "none",
      },
      stdoutBytes: 3,
      stdoutSha256: "a".repeat(64),
      stderrBytes: 0,
      stderrSha256: "b".repeat(64),
      truncated: false,
      timedOut: false,
      durationMs: 10,
    },
    fileEffects: [],
    fixture: {
      requests: 3,
      unexpected: 0,
      authorizationMatched: [true, true, true],
      userAgents: [FROZEN_USER_AGENT, FROZEN_USER_AGENT, FROZEN_USER_AGENT],
      graphqlRequests: 1,
      assetRequests: 2,
    },
    raw: {
      stdout: new TextEncoder().encode("graphql ok teams=1\n"),
      stderr: new Uint8Array(),
    },
    ...overrides,
  }
}

const both = TABLE.find((entry) => entry.id === "f02b-fixed-host-both")
const wrongAuth = TABLE.find((entry) => entry.id === "f02b-control-wrong-auth")
if (both == null || wrongAuth == null) throw new Error("table entries missing")

Deno.test("every dedicated case loads through the ordinary loader with a manifest route and a fixed-host step", async () => {
  const cases = await loadCases(CASES_DIR, await manifestRoutes())
  assertEquals(cases.length, TABLE.length)
  checkCaseTable(cases)
  for (const loaded of cases) {
    assertEquals(loaded.spec.route, "linear document view")
    assertEquals(loaded.spec.argv.length, 1)
    assertEquals(loaded.spec.expected.fileEffects, [])
    assertEquals(loaded.spec.expected.stderr, { utf8: "" })
    for (
      const key of [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "NO_PROXY",
        "DENO_CERT",
        "SSL_CERT_FILE",
      ]
    ) assert(!(key in loaded.spec.env), `${loaded.spec.id} sets ${key}`)
    assertEquals(loaded.spec.env.LINEAR_API_KEY, "lin_api_fake_probe")
  }
  const scenarios = new Set(cases.map((loaded) => loaded.spec.argv[0]))
  assertEquals(
    [...scenarios].sort(),
    [
      "both",
      "cap-below-body",
      "direct-egress",
      "extra-get",
      "graphql-wrong-variables",
      "public-roots-only",
      "redirect",
      "third-host",
      "wrong-auth",
      "wrong-path",
      "wrong-proxy-port",
    ],
  )
})

Deno.test("the table and the case directory must agree exactly", async () => {
  const cases = await loadCases(CASES_DIR, await manifestRoutes())
  assertThrows(() => checkCaseTable(cases.slice(1)), Error, "differ")
  const extra = { ...cases[0], spec: { ...cases[0].spec, id: "f02b-extra" } }
  assertThrows(() => checkCaseTable([...cases, extra]), Error, "differ")
  const dir = await Deno.makeTempDir()
  try {
    const spec = structuredClone(cases[0].spec)
    spec.id = "f02b-fixed-host-both"
    if (spec.graphql?.groups[0].mode !== "ordered") throw new Error("shape")
    spec.graphql.groups[0].steps = spec.graphql.groups[0].steps.filter((
      step,
    ) => step.kind === "graphql")
    spec.graphql.expectedRequests = 1
    await Deno.writeTextFile(
      join(dir, `${spec.id}.json`),
      JSON.stringify(spec),
    )
    const loaded = await loadCases(dir, new Set(["linear document view"]))
    const onlyGraphQL = [
      loaded[0],
      ...cases.filter((item) => item.spec.id !== spec.id),
    ]
    assertThrows(
      () => checkCaseTable(onlyGraphQL),
      Error,
      "no fixed-host step",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("a positive passes only with no mismatch, exact fixture counts and the frozen user agent", () => {
  assertEquals(evaluateCase(both, run()), { ok: true, problems: [] })
  const surfaced = evaluateCase(
    both,
    run({ mismatches: [{ surface: "stdout", detail: "stdout differs" }] }),
  )
  assert(!surfaced.ok)
  assert(surfaced.problems[0].includes("[stdout] differ from expected []"))
  const fewer = evaluateCase(
    both,
    run({
      fixture: {
        requests: 2,
        unexpected: 0,
        authorizationMatched: [true, true],
        userAgents: [FROZEN_USER_AGENT, FROZEN_USER_AGENT],
        graphqlRequests: 1,
        assetRequests: 1,
      },
    }),
  )
  assert(fewer.problems.some((item) => item.startsWith("fixture summary")))
  const agent = evaluateCase(
    both,
    run({
      fixture: {
        ...run().fixture!,
        userAgents: [FROZEN_USER_AGENT, "curl/8", FROZEN_USER_AGENT],
      },
    }),
  )
  assert(agent.problems.some((item) => item.includes("curl/8")))
  const effects = evaluateCase(
    both,
    run({
      fileEffects: [{
        path: "home/leak",
        change: "created",
        kind: "file",
        sha256: "c".repeat(64),
      }],
    }),
  )
  assert(effects.problems.some((item) => item.startsWith("file effects")))
  const noFixture = evaluateCase(both, run({ fixture: null }))
  assertEquals(noFixture.problems, ["no fixture summary"])
  const timedOut = evaluateCase(
    both,
    run({ observation: { ...run().observation, timedOut: true } }),
  )
  assert(timedOut.problems.includes("case timed out"))
})

Deno.test("a control passes only with exactly its surfaces and every fragment", () => {
  const control = run({
    mismatches: [
      {
        surface: "fixture",
        detail: "asset required header Authorization differs",
      },
      {
        surface: "fixture",
        detail:
          "expected 1 interactions (GraphQL 0, assets 1); observed 1 (GraphQL 0, assets 1), consumed 0",
      },
    ],
    observation: { ...run().observation, targetExit: { code: 1 } },
    fixture: {
      requests: 1,
      unexpected: 0,
      authorizationMatched: [false],
      userAgents: [FROZEN_USER_AGENT],
      graphqlRequests: 0,
      assetRequests: 1,
    },
  })
  assertEquals(evaluateCase(wrongAuth, control), { ok: true, problems: [] })
  const clean = evaluateCase(wrongAuth, run())
  assert(
    clean.problems.some((item) =>
      item.includes("[] differ from expected [fixture]")
    ),
  )
  const missingFragment = evaluateCase(wrongAuth, {
    ...control,
    mismatches: [control.mismatches[1]],
  })
  assertEquals(missingFragment.problems, [
    "no mismatch detail or stdout contains asset required header Authorization differs",
  ])
  const extraSurface = evaluateCase(wrongAuth, {
    ...control,
    mismatches: [...control.mismatches, {
      surface: "stdout",
      detail: "stdout differs",
    }],
  })
  assert(extraSurface.problems[0].includes("[fixture, stdout] differ"))
  // Fragments may also be satisfied by the probe's own stdout (corrupt-body).
  const corrupt: ProbeCase = {
    id: "x",
    kind: "control",
    expectSurfaces: ["exit", "stdout"],
    expectFragments: ["public body differs"],
    expectFixture: wrongAuth.expectFixture,
  }
  const viaStdout = evaluateCase(corrupt, {
    ...control,
    mismatches: [
      { surface: "exit", detail: "exit code 1 is not 0" },
      { surface: "stdout", detail: "stdout differs at byte 40" },
    ],
    raw: {
      stdout: new TextEncoder().encode("public body differs (26 bytes)\n"),
      stderr: new Uint8Array(),
    },
  })
  assertEquals(viaStdout, { ok: true, problems: [] })
})

Deno.test("positive repeats compare exit, output digests, fixture summary and mismatches", () => {
  assert(repeatsIdentically(run(), run()))
  assert(
    !repeatsIdentically(
      run(),
      run({
        observation: { ...run().observation, stdoutSha256: "d".repeat(64) },
      }),
    ),
  )
  assert(
    !repeatsIdentically(
      run(),
      run({ fixture: { ...run().fixture!, requests: 2 } }),
    ),
  )
})

Deno.test("driver options are strict and machine paths stay on the command line", () => {
  const options = parseOptions(required)
  assertEquals(options.probe, "/probe")
  assertEquals(options.inside, false)
  assertThrows(() => parseOptions(required.slice(2)), Error, "usage")
  assertThrows(
    () => parseOptions([...required, "--deno-dir", "/cache"]),
    Error,
    "internal namespace",
  )
  assertThrows(
    () => parseOptions([...required, "--staged-reused", "maybe"]),
    Error,
    "true or false",
  )
  assertThrows(
    () => parseOptions([...required, "--inside-namespace"]),
    Error,
    "fresh PID namespace",
  )
  assertThrows(
    () => parseOptions([...required, "--cases", "/elsewhere"]),
    Error,
    "unknown argument",
  )
  assertThrows(
    () => parseOptions(["--probe", "relative/probe", ...required.slice(2)]),
    Error,
    "absolute",
  )
  assertThrows(
    () => parseOptions([...required, "--report"]),
    Error,
    "needs a value",
  )
})
