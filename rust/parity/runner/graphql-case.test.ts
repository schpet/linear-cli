import { assertEquals, assertRejects, assertThrows } from "@std/assert"
import { loadCases, resolveCase } from "./cases.ts"
import { parseCase, SchemaError } from "./schema.ts"
import { validCase } from "./test-fixtures.ts"
import * as v from "valibot"

const digest =
  "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a"

function graphqlCase(): Record<string, unknown> {
  const spec = validCase()
  const substitutions = spec.substitutions
  if (!Array.isArray(substitutions)) throw new Error("invalid test fixture")
  substitutions.push("fixturePort")
  spec.graphql = {
    path: "/graphql",
    schemaSha256: digest,
    expectedRequests: 2,
    initialRecords: { "User:u1": { id: "u1" } },
    expectedRecords: { "User:u1": { id: "u1" } },
    groups: [{
      mode: "ordered",
      steps: [
        {
          kind: "graphql",
          id: "viewer",
          operation: { document: "{ viewer { id } }" },
          identity: {
            authorization: "lin_api_fake",
            userAgent: "schpet-linear-cli/2.6.0",
            headers: {},
          },
          response: { kind: "data", data: { viewer: { id: "u1" } } },
          effects: [],
        },
        {
          kind: "asset",
          id: "download",
          method: "GET",
          path: "/asset?id=1",
          requiredHeaders: {},
          forbiddenHeaders: ["Authorization"],
          body: { utf8: "" },
          response: { status: 200, headers: {}, body: { utf8: "bytes" } },
        },
      ],
    }],
  }
  return spec
}

function fixture(spec: Record<string, unknown>): Record<string, unknown> {
  const value = spec.graphql
  if (typeof value !== "object" || value == null || Array.isArray(value)) {
    throw new Error("invalid test fixture")
  }
  if (!v.is(v.record(v.string(), v.unknown()), value)) {
    throw new Error("invalid test fixture")
  }
  return value
}

function firstStep(spec: Record<string, unknown>): Record<string, unknown> {
  const groups = fixture(spec).groups
  if (
    !Array.isArray(groups) || typeof groups[0] !== "object" ||
    groups[0] == null || !Array.isArray(groups[0].steps)
  ) throw new Error("invalid test fixture")
  return groups[0].steps[0]
}

Deno.test("P02 cases remain loadable and GraphQL case resolves", async () => {
  const old = parseCase(validCase())
  assertEquals(old.graphql, undefined)
  const parsed = parseCase(graphqlCase())
  assertEquals(parsed.graphql?.expectedRequests, 2)
  const values = {
    home: "h",
    configHome: "c",
    cwd: "w",
    cwdRoot: "r",
    bin: "b",
    denoDir: "d",
    fixturePort: "123",
    referenceModuleUrl: "file:///reference",
  }
  assertEquals(resolveCase(parsed, values).graphql?.path, "/graphql")
  const caseDir = new URL("./cases", import.meta.url).pathname
  const routes = new Set<string>()
  for await (const entry of Deno.readDir(caseDir)) {
    if (!entry.name.endsWith(".json")) continue
    const value: unknown = JSON.parse(
      await Deno.readTextFile(`${caseDir}/${entry.name}`),
    )
    routes.add(parseCase(value).route)
  }
  const loaded = await loadCases(caseDir, routes)
  const originalCaseIds = [
    "api-loopback-port-echo-200",
    "api-loopback-unauthorized-401",
    "api-loopback-viewer-200",
    "api-no-key",
    "api-no-query",
    "help-api",
    "help-issue-mine",
    "help-root",
    "parser-invalid-variable",
    "parser-unknown-command",
    "parser-unknown-option",
    "version-no-color",
    "version",
  ]
  const graphqlCaseIds = [
    "api-graphql-paginate",
    "api-graphql-validation-error",
    "api-graphql-variable",
    "api-graphql-viewer",
    "schema-graphql-introspection",
  ]
  for (const id of originalCaseIds) {
    assertEquals(
      loaded.some((item) => item.spec.id === id && item.spec.graphql == null),
      true,
      `${id} should remain a non-GraphQL case`,
    )
  }
  for (const id of graphqlCaseIds) {
    assertEquals(
      loaded.some((item) => item.spec.id === id && item.spec.graphql != null),
      true,
      `${id} should remain a GraphQL case`,
    )
  }
  // Keep C002's closed 26-case set separate from the other GraphQL cases.
  const c002 = (item: { spec: { id: string } }) =>
    item.spec.id.startsWith("c002-")
  const others = loaded.filter((item) => !c002(item))
  // These corpus-wide counts also guard older fixtures without cohort tests.
  // C033n adds 4 local positive-limit cases.
  // Update them when adding reviewed command cases.
  // C039/C048/C043/C054/C032/C033/C074 add 14/4/4/3/6/7/9 local and
  // 15/22/20/18/17/17/9 GraphQL cases.
  // C029/C034 add 18 local and 23 GraphQL cases across the two frozen cohorts.
  // C057–C059/C012/C017 add 11/2/3 local and 8/5/5 GraphQL cases.
  // C046 adds 18 GraphQL cases and no local cases.
  // C018 adds 1 local and 12 GraphQL cases.
  // C028/C044/C055 add 4/3/1 local and 9/8/8 GraphQL cases.
  // C066/C077/C078/C076 + C065/C064 add 8 local and 44 GraphQL cases.
  assertEquals(others.filter((item) => item.spec.graphql == null).length, 617)
  // C041/C042 add 24 typed association GraphQL cases.
  assertEquals(others.filter((item) => item.spec.graphql != null).length, 1142)
  assertEquals(loaded.filter(c002).length, 26)
})

Deno.test("strict GraphQL shape rejects unknown fields, count drift, and fixture mixing", () => {
  const unknown = graphqlCase()
  fixture(unknown).surprise = true
  assertThrows(() => parseCase(unknown), SchemaError, "surprise")
  const count = graphqlCase()
  fixture(count).expectedRequests = 1
  assertThrows(() => parseCase(count), SchemaError, "expectedRequests")
  const mixed = graphqlCase()
  mixed.fixtureServer = {
    path: "/graphql",
    responses: [{ status: 200, headers: {}, body: { utf8: "{}" } }],
    expectedRequests: 1,
    expectedAuthorization: "lin_api_fake",
  }
  assertThrows(() => parseCase(mixed), SchemaError, "mutually exclusive")
  const unsafe = graphqlCase()
  firstStep(unsafe).extra = true
  assertThrows(() => parseCase(unsafe), SchemaError, "extra")
  const badAuth = graphqlCase()
  const identity = firstStep(badAuth).identity
  if (typeof identity !== "object" || identity == null) {
    throw new Error("invalid test fixture")
  }
  Object.assign(identity, { authorization: "lin_api_real" })
  assertThrows(() => parseCase(badAuth), SchemaError, "authorization")
  const badAgent = graphqlCase()
  const agentIdentity = firstStep(badAgent).identity
  if (typeof agentIdentity !== "object" || agentIdentity == null) {
    throw new Error("invalid test fixture")
  }
  Object.assign(agentIdentity, { userAgent: "schpet-linear-cli/2.6.1" })
  assertThrows(() => parseCase(badAgent), SchemaError, "userAgent")
  const badPath = graphqlCase()
  const groups = fixture(badPath).groups
  if (!Array.isArray(groups) || !Array.isArray(groups[0].steps)) {
    throw new Error("invalid test fixture")
  }
  groups[0].steps[1].path = "//example.com/asset"
  assertThrows(() => parseCase(badPath), SchemaError, "path")
  const assetAuth = graphqlCase()
  const assetGroups = fixture(assetAuth).groups
  if (!Array.isArray(assetGroups) || !Array.isArray(assetGroups[0].steps)) {
    throw new Error("invalid test fixture")
  }
  assetGroups[0].steps[1].requiredHeaders = { Authorization: "lin_api_real" }
  assertThrows(() => parseCase(assetAuth), SchemaError, "asset headers")

  for (const kind of ["transport", "validationErrors"]) {
    const invalid = graphqlCase()
    const step = firstStep(invalid)
    step.response = kind === "transport"
      ? { kind, status: 503, headers: {}, body: { utf8: "offline" } }
      : { kind, status: 400, errors: [{ message: "bad query" }] }
    step.effects = [{
      kind: "put",
      record: "User:u1",
      before: { absent: true },
      after: { id: "u1" },
    }]
    assertThrows(() => parseCase(invalid), SchemaError, "effects require")
  }
  for (const kind of ["data", "transport", "validationErrors"]) {
    const invalid = graphqlCase()
    const step = firstStep(invalid)
    step.partialEffects = true
    if (kind === "transport") {
      step.response = {
        kind,
        status: 503,
        headers: {},
        body: { utf8: "offline" },
      }
    }
    if (kind === "validationErrors") {
      step.response = { kind, status: 400, errors: [{ message: "bad query" }] }
    }
    assertThrows(() => parseCase(invalid), SchemaError, "partialEffects:true")
  }
  const missingPartial = graphqlCase()
  const errorStep = firstStep(missingPartial)
  errorStep.response = {
    kind: "graphqlErrors",
    status: 200,
    data: null,
    errors: [{ message: "partial failure" }],
  }
  errorStep.effects = [{
    kind: "put",
    record: "User:u2",
    before: { absent: true },
    after: { id: "u2" },
  }]
  assertThrows(
    () => parseCase(missingPartial),
    SchemaError,
    "graphqlErrors effects require partialEffects:true",
  )
  errorStep.partialEffects = true
  parseCase(missingPartial)
})

Deno.test("loader rejects invalid fixture operations and duplicate IDs", async () => {
  const dir = await Deno.makeTempDir()
  try {
    const bad = graphqlCase()
    const step = firstStep(bad)
    step.operation = { document: "{ definitelyMissing }" }
    await Deno.writeTextFile(`${dir}/sample.json`, JSON.stringify(bad))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "invalid fixture expectation",
    )
    const duplicate = graphqlCase()
    const groups = fixture(duplicate).groups
    if (
      !Array.isArray(groups) || typeof groups[0] !== "object" ||
      groups[0] == null || !Array.isArray(groups[0].steps)
    ) throw new Error("invalid test fixture")
    groups[0].steps[1].id = "viewer"
    await Deno.writeTextFile(`${dir}/sample.json`, JSON.stringify(duplicate))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "duplicate interaction id",
    )
    const redirect = graphqlCase()
    const redirectGroups = fixture(redirect).groups
    if (
      !Array.isArray(redirectGroups) || !Array.isArray(redirectGroups[0].steps)
    ) throw new Error("invalid test fixture")
    redirectGroups[0].steps[1].response.location = "/undeclared"
    redirectGroups[0].steps[1].response.status = 302
    await Deno.writeTextFile(`${dir}/sample.json`, JSON.stringify(redirect))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "immediately following",
    )
    const badOrigin = graphqlCase()
    const operation = firstStep(badOrigin).operation
    if (typeof operation !== "object" || operation == null) {
      throw new Error("invalid test fixture")
    }
    Object.assign(operation, { exactOrigins: ["$.nowhere"] })
    await Deno.writeTextFile(`${dir}/sample.json`, JSON.stringify(badOrigin))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "explicit origin differs",
    )
    for (
      const { value, message } of [
        {
          value: { $record: "User:u1", id: "u1" },
          message: "malformed composite $record",
        },
        {
          value: { $record: "User:missing" },
          message: "unknown composite $record",
        },
      ]
    ) {
      const badReference = graphqlCase()
      firstStep(badReference).response = {
        kind: "data",
        data: { viewer: value },
      }
      await Deno.writeTextFile(
        `${dir}/sample.json`,
        JSON.stringify(badReference),
      )
      await assertRejects(
        () => loadCases(dir, new Set(["linear"])),
        SchemaError,
        message,
      )
    }
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("loader checks every GraphQL case digest after warming the schema", async () => {
  const dir = await Deno.makeTempDir()
  try {
    const first = graphqlCase()
    first.id = "a"
    const second = graphqlCase()
    second.id = "b"
    fixture(second).schemaSha256 = "0".repeat(64)
    await Deno.writeTextFile(`${dir}/a.json`, JSON.stringify(first))
    await Deno.writeTextFile(`${dir}/b.json`, JSON.stringify(second))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      `${dir}/b.json: GraphQL schema digest differs from pinned baseline`,
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
