import { assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { checkLaneDeadline, loadCases } from "./cases.ts"
import {
  type AssetStepSpec,
  type GraphQLStepSpec,
  parseCase,
  SchemaError,
} from "./schema.ts"
import { validCase } from "./test-fixtures.ts"

const digest =
  "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a"
function query(id: string, authorization: string): GraphQLStepSpec {
  return {
    kind: "graphql",
    id,
    operation: { document: "{ viewer { id } }" },
    identity: {
      authorization,
      userAgent: "schpet-linear-cli/2.6.0",
      headers: {},
    },
    response: { kind: "data", data: { viewer: { id } } },
    effects: [],
  }
}
function asset(id: string, path: string): AssetStepSpec {
  return {
    kind: "asset",
    id,
    method: "GET",
    path,
    requiredHeaders: {},
    forbiddenHeaders: [],
    body: { utf8: "" },
    response: { status: 200, headers: {}, body: { utf8: "ok" } },
  }
}
function caseWith(
  groups: unknown[],
  timeoutMs = 5000,
): Record<string, unknown> {
  const spec = validCase()
  spec.timeoutMs = timeoutMs
  spec.substitutions = ["home", "configHome", "bin", "denoDir", "fixturePort"]
  spec.graphql = {
    path: "/graphql",
    schemaSha256: digest,
    expectedRequests: groups.flatMap((group) => {
      if (
        typeof group !== "object" || group == null || Array.isArray(group)
      ) throw new Error("bad group")
      if ("steps" in group && Array.isArray(group.steps)) return group.steps
      if ("lanes" in group && Array.isArray(group.lanes)) {
        return group.lanes.flatMap((lane) =>
          typeof lane === "object" && lane != null && "steps" in lane &&
            Array.isArray(lane.steps)
            ? lane.steps
            : []
        )
      }
      return []
    }).length,
    initialRecords: { "User:u1": { id: "u1" } },
    expectedRecords: { "User:u1": { id: "u1" } },
    groups,
  }
  return spec
}
async function rejected(
  spec: Record<string, unknown>,
  message: string,
): Promise<void> {
  const dir = await Deno.makeTempDir()
  try {
    await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"])),
      SchemaError,
      message,
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

Deno.test("lane deadline leaves cleanup margin at load and after runtime override", async () => {
  const lanes = [{
    mode: "lanes",
    timeoutMs: 500,
    lanes: [{ id: "a", steps: [asset("a", "/a")] }, {
      id: "b",
      steps: [asset("b", "/b")],
    }],
  }]
  const spec = caseWith(lanes, 1500)
  await rejected(spec, "lane deadlines plus cleanup margin")
  const good = parseCase(caseWith(lanes, 2000))
  assertThrows(
    () => checkLaneDeadline(good, 1500),
    SchemaError,
    "lane deadlines plus cleanup margin",
  )
})

Deno.test("ambiguous lane first steps and cross-lane record read/write dependencies reject at load", async () => {
  const sameAuth = caseWith([{
    mode: "lanes",
    timeoutMs: 500,
    lanes: [
      { id: "a", steps: [query("a", "lin_api_fake_same")] },
      { id: "b", steps: [query("b", "lin_api_fake_same")] },
    ],
  }])
  await rejected(sameAuth, "ambiguous non-identical lane")
  const writer = query("write", "lin_api_fake_a")
  writer.effects = [{
    kind: "put",
    record: "User:u1",
    before: { value: { id: "u1" } },
    after: { id: "u1", name: "new" },
  }]
  const reader = query("read", "lin_api_fake_b")
  reader.response = { kind: "data", data: { viewer: { $record: "User:u1" } } }
  await rejected(
    caseWith([{
      mode: "lanes",
      timeoutMs: 500,
      lanes: [
        { id: "a", steps: [writer] },
        { id: "b", steps: [reader] },
      ],
    }]),
    "cross-lane record read/write",
  )
})

Deno.test("cross-lane reads close transitively through initial and effect-after records", async () => {
  const reader = query("reader", "lin_api_fake_a")
  reader.operation.document = "{ viewer { name organization { name } } }"
  reader.response = {
    kind: "data",
    data: { viewer: { $record: "User:u1" } },
  }
  const writer = query("writer", "lin_api_fake_b")
  writer.operation.document =
    'mutation { organizationUpdate(input: {name: "new"}) { success } }'
  writer.response = {
    kind: "data",
    data: { organizationUpdate: { success: true } },
  }
  writer.effects = [{
    kind: "put",
    record: "Org:o1",
    before: { value: { name: "old" } },
    after: { name: "new" },
  }]
  const group = [{
    mode: "lanes",
    timeoutMs: 500,
    lanes: [
      { id: "reader", steps: [reader] },
      { id: "writer", steps: [writer] },
    ],
  }]
  const direct = caseWith(group)
  const directFixture = direct.graphql
  if (typeof directFixture !== "object" || directFixture == null) {
    throw new Error("missing GraphQL fixture")
  }
  Object.assign(directFixture, {
    initialRecords: {
      "User:u1": { name: "A", organization: { $record: "Org:o1" } },
      "Org:o1": { name: "old" },
    },
    expectedRecords: {
      "User:u1": { name: "A", organization: { $record: "Org:o1" } },
      "Org:o1": { name: "new" },
    },
  })
  await rejected(direct, "cross-lane record read/write")

  const viaAfter = caseWith(group)
  const afterFixture = viaAfter.graphql
  if (typeof afterFixture !== "object" || afterFixture == null) {
    throw new Error("missing GraphQL fixture")
  }
  Object.assign(afterFixture, {
    initialRecords: {
      "User:u1": { name: "A" },
      "Org:o1": { name: "old" },
    },
    expectedRecords: {
      "User:u1": { name: "A", organization: { $record: "Org:o1" } },
      "Org:o1": { name: "new" },
    },
    groups: [{
      mode: "ordered",
      steps: [{
        ...query("seed", "lin_api_fake_seed"),
        effects: [{
          kind: "put",
          record: "User:u1",
          before: { value: { name: "A" } },
          after: { name: "A", organization: { $record: "Org:o1" } },
        }],
      }],
    }, ...group],
    expectedRequests: 3,
  })
  await rejected(viaAfter, "cross-lane record read/write")

  // JSONObject content is literal data, even if it has a $record key.
  const literal = caseWith(group)
  const literalFixture = literal.graphql
  if (typeof literalFixture !== "object" || literalFixture == null) {
    throw new Error("missing GraphQL fixture")
  }
  const literalReader = query("literal-reader", "lin_api_fake_a")
  literalReader.operation.document =
    "{ viewer { organization { aiProviderConfiguration } } }"
  literalReader.response = {
    kind: "data",
    data: { viewer: { organization: { $record: "Org:o1" } } },
  }
  const literalWriter = query("literal-writer", "lin_api_fake_b")
  literalWriter.operation.document = writer.operation.document
  literalWriter.response = writer.response
  literalWriter.effects = [{
    kind: "put",
    record: "Other:o2",
    before: { value: { name: "old" } },
    after: { name: "new" },
  }]
  Object.assign(literalFixture, {
    initialRecords: {
      "Org:o1": { aiProviderConfiguration: { $record: "Other:o2" } },
      "Other:o2": { name: "old" },
    },
    expectedRecords: {
      "Org:o1": { aiProviderConfiguration: { $record: "Other:o2" } },
      "Other:o2": { name: "new" },
    },
    groups: [{
      mode: "lanes",
      timeoutMs: 500,
      lanes: [
        { id: "reader", steps: [literalReader] },
        { id: "writer", steps: [literalWriter] },
      ],
    }],
  })
  const dir = await Deno.makeTempDir()
  try {
    await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(literal))
    assertEquals((await loadCases(dir, new Set(["linear"]))).length, 1)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("asset Location is dedicated, 3xx GET-only, and immediately followed in its lane", async () => {
  const header = asset("header", "/first")
  header.response.headers = { LOCATION: "/second" }
  assertThrows(
    () => parseCase(caseWith([{ mode: "ordered", steps: [header] }])),
    SchemaError,
    "Location",
  )
  const malformed = asset("redirect", "/first")
  malformed.response.location = "/second"
  assertThrows(
    () => parseCase(caseWith([{ mode: "ordered", steps: [malformed] }])),
    SchemaError,
    "redirects require",
  )
  malformed.response.status = 302
  await rejected(
    caseWith([{
      mode: "ordered",
      steps: [malformed, asset("wrong", "/other")],
    }]),
    "immediately following",
  )
})
