import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { CASE_ROOT_PARENT, prepareConfinement } from "./bwrap.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases, resolveCase } from "./cases.ts"
import type { Program } from "./program.ts"
import { countReviewedGraphqlUserAgentPasses } from "./report.ts"
import { runCorpus } from "./run.ts"
import { SchemaError } from "./schema.ts"
import { testStatusHelper } from "./test-fixtures.ts"

const CONTRACT = "rust-3.0.0-alpha.1"
const USER_AGENT = "schpet-linear-cli/3.0.0-alpha.1"
const ROUTE = "linear initiative view"
const ID = "c038-empty-id"
const DEVIATION = "C038P02-SYNTHETIC"
const SURFACES = [
  "exit",
  "stdout",
  "stderr",
  "graphql-fixture",
  "graphql-user-agent",
]
const VALUES = {
  home: "h",
  configHome: "c",
  cwd: "w",
  cwdRoot: "r",
  bin: "b",
  denoDir: "d",
  fixturePort: "0",
  referenceModuleUrl: "file:///reference",
}

function at(root: unknown, path: readonly (string | number)[]): object {
  let value = root
  for (const key of path) {
    if (value == null || typeof value !== "object") {
      throw new Error(`missing test path ${path.join(".")}`)
    }
    value = Reflect.get(value, key)
  }
  if (value == null || typeof value !== "object") {
    throw new Error(`missing object ${path.join(".")}`)
  }
  return value
}

function set(
  root: unknown,
  path: readonly (string | number)[],
  value: unknown,
) {
  Reflect.set(at(root, path.slice(0, -1)), path.at(-1)!, value)
}

async function withCorpus(
  fn: (
    dir: string,
    source: Record<string, unknown>,
    write: (
      golden: Record<string, unknown>,
      mutate?: (source: Record<string, unknown>) => void,
    ) => Promise<void>,
  ) => Promise<void>,
) {
  const dir = await Deno.makeTempDir({ prefix: "c038-p02-" })
  const goldens = join(dir, "rust-goldens", CONTRACT)
  await Deno.mkdir(goldens, { recursive: true })
  const source = JSON.parse(
    await Deno.readTextFile(
      new URL("./c038-frozen-cases/c038-empty-id.json", import.meta.url),
    ),
  )
  source.expected = {
    exit: { code: 0 },
    stdout: { utf8: "baseline\n" },
    stderr: { utf8: "" },
    fileEffects: [],
  }
  source.substitutions = source.substitutions.filter((name: string) =>
    name !== "referenceModuleUrl"
  )
  const fixture = join(dir, "fixtures", "workspace-credential", "linear")
  await Deno.mkdir(fixture, { recursive: true })
  await Deno.copyFile(
    new URL(
      "./c038-frozen-cases/fixtures/workspace-credential/linear/credentials.toml",
      import.meta.url,
    ),
    join(fixture, "credentials.toml"),
  )
  let priorId = ID
  const write = async (
    golden: Record<string, unknown>,
    mutate?: (source: Record<string, unknown>) => void,
  ) => {
    const spec = structuredClone(source)
    mutate?.(spec)
    const id = spec.id
    if (typeof id !== "string") throw new Error("test case ID must be a string")
    if (id !== priorId) {
      await Deno.remove(join(goldens, `${priorId}.json`))
      await Deno.remove(join(dir, `${priorId}.json`))
    }
    const raw = JSON.stringify({ ...golden, caseId: id }, null, 2) + "\n"
    await Deno.writeTextFile(join(goldens, `${id}.json`), raw)
    spec.deviation = {
      id: DEVIATION,
      contract: CONTRACT,
      sha256: await sha256Hex(new TextEncoder().encode(raw)),
    }
    await Deno.writeTextFile(join(dir, `${id}.json`), JSON.stringify(spec))
    priorId = id
  }
  try {
    await fn(dir, source, write)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

function golden(): Record<string, unknown> {
  return {
    formatVersion: 1,
    caseId: ID,
    deviationId: DEVIATION,
    contract: CONTRACT,
    approvedSurfaces: SURFACES,
    candidate: {
      expected: {
        exit: { code: 2 },
        stdout: { utf8: "" },
        stderr: { utf8: "error: Initiative reference cannot be empty.\n" },
        fileEffects: [],
      },
      graphql: { steps: [] },
      graphqlUserAgent: USER_AGENT,
    },
  }
}

Deno.test("C038P02 derives zero requests only from the exact committed empty-ID source", async () => {
  await withCorpus(async (dir, source, write) => {
    const selected = new Set([ROUTE, "linear initiative list"])
    const load = () => loadCases(dir, selected, undefined, CONTRACT)
    await write(golden())
    const [loaded] = await load()
    const frozen = structuredClone(loaded.spec)
    const candidate = candidateCaseView(loaded)
    assertEquals(loaded.spec, frozen)
    assertEquals(loaded.spec.graphql?.expectedRequests, 2)
    assertEquals(loaded.spec.graphql?.groups[0].mode, "ordered")
    assertEquals(candidate.spec.graphql?.expectedRequests, 0)
    assertEquals(candidate.spec.graphql?.groups, [])
    assertEquals(candidate.runtimeUserAgent, USER_AGENT)
    assertEquals(candidate.golden?.spec.approvedSurfaces, SURFACES)
    assertEquals(candidate.spec.expected.exit, { code: 2 })
    assertEquals(candidate.spec.expected.stderr, {
      utf8: "error: Initiative reference cannot be empty.\n",
    })
    assertEquals(
      resolveCase(candidate.spec, VALUES, USER_AGENT).graphql?.groups,
      [],
    )
    assertThrows(() => resolveCase(candidate.spec, VALUES))
    assertEquals(resolveCase(loaded.spec, VALUES).graphql?.expectedRequests, 2)

    const rejection =
      "zero-request GraphQL delta needs exactly one frozen query step"
    const changes: Array<[(spec: Record<string, unknown>) => void, string]> = [
      [(s) => {
        s.id = "c038-other"
      }, rejection],
      [(s) => {
        s.route = "linear initiative list"
      }, rejection],
      [(s) => {
        s.argv = ["initiative", "view", "other"]
      }, rejection],
      [(s) => {
        s.argv = ["initiative", "view", ""].concat("extra")
      }, rejection],
      [
        (s) => set(s, ["graphql", "groups", 0, "steps", 0, "id"], "renamed"),
        rejection,
      ],
      [
        (s) => set(s, ["graphql", "groups", 0, "steps", 1, "id"], "slug"),
        "duplicate interaction id",
      ],
      [
        (s) =>
          set(
            s,
            ["graphql", "groups", 0, "steps", 0, "operation", "document"],
            `${
              Reflect.get(
                at(source, ["graphql", "groups", 0, "steps", 0, "operation"]),
                "document",
              )
            } `,
          ),
        rejection,
      ],
      [
        (s) =>
          set(s, [
            "graphql",
            "groups",
            0,
            "steps",
            0,
            "operation",
            "operationName",
          ], "GetInitiativeBySlugForView"),
        rejection,
      ],
      [
        (s) =>
          set(s, [
            "graphql",
            "groups",
            0,
            "steps",
            0,
            "operation",
            "variables",
            "slugId",
          ], "x"),
        rejection,
      ],
      [
        (s) =>
          set(s, [
            "graphql",
            "groups",
            0,
            "steps",
            1,
            "operation",
            "variables",
            "name",
          ], "x"),
        rejection,
      ],
      [
        (s) =>
          set(
            s,
            ["graphql", "groups", 0, "steps", 0, "identity", "userAgent"],
            "changed",
          ),
        "Invalid type",
      ],
      [
        (s) =>
          set(s, [
            "graphql",
            "groups",
            0,
            "steps",
            0,
            "response",
            "data",
            "initiatives",
            "nodes",
          ], [{ id: "x" }]),
        rejection,
      ],
      [
        (s) =>
          set(s, [
            "graphql",
            "groups",
            0,
            "steps",
            0,
            "response",
            "data",
            "extra",
          ], true),
        rejection,
      ],
      [
        (s) =>
          set(s, [
            "graphql",
            "groups",
            0,
            "steps",
            0,
            "response",
            "partialEffects",
          ], true),
        "Invalid key",
      ],
      [
        (s) =>
          set(s, ["graphql", "groups", 0, "steps", 0, "effects"], [{
            kind: "put",
            record: "x",
            before: { absent: true },
            after: { value: "x" },
          }]),
        "effect-free query steps",
      ],
      [
        (s) => set(s, ["graphql", "expectedRequests"], 3),
        "expectedRequests must equal",
      ],
      [(s) =>
        set(s, ["graphql", "groups", 0, "steps"], [
          Reflect.get(at(source, ["graphql", "groups", 0, "steps"]), "0"),
        ]), "expectedRequests must equal"],
    ]
    for (const [mutate, message] of changes) {
      await write(golden(), mutate)
      await assertRejects(load, SchemaError, message)
    }
    await write(golden(), (s) => {
      const steps = at(s, ["graphql", "groups", 0])
      const value = Reflect.get(steps, "steps")
      assert(Array.isArray(value))
      Reflect.set(steps, "steps", value.toReversed())
    })
    await assertRejects(load, SchemaError, rejection)
    await write(golden(), (s) => {
      s.fixtureServer = { path: "/graphql" }
    })
    await assertRejects(load, SchemaError, "fixtureServer.responses")
    const wrongUserAgent = golden()
    set(wrongUserAgent, ["candidate", "graphqlUserAgent"], "wrong")
    await write(wrongUserAgent)
    await assertRejects(load, SchemaError, "graphqlUserAgent")
    const missingApproval = golden()
    set(missingApproval, ["approvedSurfaces"], ["graphql-fixture"])
    await write(missingApproval)
    await assertRejects(load, SchemaError, "approvedSurfaces")
    const prefix = golden()
    set(prefix, ["candidate", "graphql", "steps"], [{ id: "slug" }])
    await write(prefix)
    assertEquals(
      candidateCaseView((await load())[0]).spec.graphql?.expectedRequests,
      1,
    )
    await write(golden())
    await load()
    const casePath = join(dir, `${ID}.json`)
    const forged = JSON.parse(await Deno.readTextFile(casePath))
    forged.deviation.sha256 = "0".repeat(64)
    await Deno.writeTextFile(casePath, JSON.stringify(forged))
    await assertRejects(load, SchemaError, "SHA-256 differs")
  })
})

Deno.test("C038P02 confined candidate sends no request and unexpected requests fail", async () => {
  await withCorpus(async (dir, source, write) => {
    await write(golden())
    const [loaded] = await loadCases(dir, new Set([ROUTE]), undefined, CONTRACT)
    const runDir = await Deno.makeTempDir({
      dir: CASE_ROOT_PARENT,
      prefix: "c038-p02-run-",
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
      const steps = Reflect.get(at(source, ["graphql", "groups", 0]), "steps")
      assert(Array.isArray(steps))
      const script = async (
        name: string,
        requestIds: string[],
        candidate: boolean,
      ): Promise<Program> => {
        const path = join(runDir, name)
        const lines = requestIds.map((id) => {
          const step = steps.find((entry: unknown) =>
            entry != null && typeof entry === "object" &&
            Reflect.get(entry, "id") === id
          )
          assert(step != null)
          const operation = Reflect.get(step, "operation")
          const body = JSON.stringify({
            query: Reflect.get(operation, "document"),
            variables: Reflect.get(operation, "variables"),
          })
          return `/usr/bin/curl --silent --show-error --noproxy '*' --request POST --header 'content-type: application/json' --header 'authorization: lin_api_fake_alpha' --header 'user-agent: ${
            name === "baseline.sh" ? "schpet-linear-cli/2.6.0" : USER_AGENT
          }' --data-raw '${body}' "$LINEAR_GRAPHQL_ENDPOINT" >/dev/null`
        })
        await Deno.writeTextFile(
          path,
          `#!/bin/sh\n${lines.join("\n")}\n${
            candidate
              ? "printf 'error: Initiative reference cannot be empty.\\n' >&2\nexit 2"
              : "printf 'baseline\\n'"
          }\n`,
          { mode: 0o755 },
        )
        return { kind: "executable", path }
      }
      const baseline = await script("baseline.sh", ["slug", "name"], false)
      const zero = await script("zero.sh", [], true)
      const slug = await script("slug.sh", ["slug"], true)
      const name = await script("name.sh", ["name"], true)
      const run = async (program: Program, base = baseline) => {
        const [result] = await runCorpus([loaded], base, {
          name: "synthetic C038 zero-request Rust candidate",
          contract: CONTRACT,
          program,
          implementedRoutes: new Set([ROUTE]),
        }, ctx)
        return result
      }
      const pass = await run(zero)
      assertEquals(
        pass.status,
        "pass",
        JSON.stringify(pass.candidate?.mismatches),
      )
      assertEquals(pass.baseline.fixture?.graphqlRequests, 2)
      assertEquals(pass.candidate?.fixture?.userAgents, [])
      assertEquals(pass.candidate?.fixture?.graphqlRequests, 0)
      assertEquals(countReviewedGraphqlUserAgentPasses([pass]), 0)
      for (const program of [slug, name]) {
        const fail = await run(program)
        assertEquals(fail.status, "fail")
        assertEquals(fail.candidate?.fixture?.unexpected, 1)
        assert(fail.candidate?.mismatches.some((m) => m.surface === "fixture"))
      }
      const drift = await run(
        zero,
        await script("drift.sh", ["slug"], false),
      )
      assertEquals(drift.status, "baseline-drift")
      assertEquals(drift.candidate, null)
    } finally {
      await Deno.remove(runDir, { recursive: true })
    }
  })
})
