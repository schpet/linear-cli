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
    bin: "b",
    denoDir: "d",
    fixturePort: "123",
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
  assertEquals(loaded.length, 13)
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
    await Deno.writeTextFile(`${dir}/sample.json`, JSON.stringify(redirect))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      "undeclared",
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
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
