import { assert, assertEquals, assertThrows } from "@std/assert"
import * as v from "valibot"
import { compareGraphQLFixture } from "./compare.ts"
import {
  loadPinnedGraphQLSchema,
  startGraphQLServer,
} from "./graphql-server.ts"
import { GraphQLFixtureSchema } from "./schema.ts"

const root = new URL("./c039-frozen-cases/", import.meta.url)
type Fixture = v.InferOutput<typeof GraphQLFixtureSchema>

async function fixture(id: string): Promise<Fixture> {
  const source = JSON.parse(
    await Deno.readTextFile(new URL(`c039-${id}.json`, root)),
  )
  return v.parse(GraphQLFixtureSchema, source.graphql)
}
function first(spec: Fixture) {
  const group = spec.groups[0]
  assert(group.mode === "ordered")
  const step = group.steps[0]
  assert(step.kind === "graphql")
  return step
}
function requirePartialCreateData(spec: Fixture): void {
  const step = first(spec)
  assert(step.response.kind === "graphqlErrors")
  const data = step.response.data
  if (data == null || typeof data !== "object") {
    throw new Error("C039 partial create requires non-null data")
  }
  const payload = Reflect.get(data, "initiativeCreate")
  if (
    payload == null || typeof payload !== "object" ||
    Reflect.get(payload, "success") !== true
  ) {
    throw new Error("C039 partial create requires success:true")
  }
}
async function exercise(
  spec: Fixture,
  requests: Array<{ query: string; variables?: Record<string, unknown> }>,
) {
  const schema = await loadPinnedGraphQLSchema()
  const server = startGraphQLServer(() => spec, schema)
  try {
    const statuses: number[] = []
    for (const body of requests) {
      const response = await fetch(`http://127.0.0.1:${server.port}/graphql`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          authorization: "lin_api_fake_alpha",
          "user-agent": "schpet-linear-cli/2.6.0",
        },
        body: JSON.stringify(body),
      })
      statuses.push(response.status)
      await response.arrayBuffer()
    }
    return {
      statuses,
      consumed: server.consumed,
      unexpected: server.unexpected,
      mismatches: compareGraphQLFixture(spec, server),
      records: server.state.snapshot(),
    }
  } finally {
    await server.stop()
  }
}

Deno.test("C039 mutation fixture rejects omitted/null, extra input and injected status", async () => {
  const spec = await fixture("minimal-create")
  const step = first(spec)
  const query = step.operation.document
  const exact = { input: { name: "Initiative 701" } }
  const positive = await exercise(spec, [{ query, variables: exact }])
  assertEquals(positive.statuses, [200])
  assertEquals(positive.consumed, 1)
  assertEquals(positive.mismatches, [])
  assertEquals(positive.records, spec.expectedRecords)
  for (
    const variables of [
      { input: { name: "Initiative 701", description: null } },
      { input: { name: "Initiative 701" }, extra: 1 },
      { input: { name: "Initiative 701", status: "Planned" } },
    ]
  ) {
    const result = await exercise(spec, [{ query, variables }])
    assertEquals(result.statuses, [500])
    assertEquals(result.consumed, 0)
    assert(result.mismatches.length > 0)
    assertEquals(result.records, {})
  }
})

Deno.test("C039 mutation fixture rejects duplicate and unconsumed writes", async () => {
  const spec = await fixture("minimal-create")
  const step = first(spec)
  const request = {
    query: step.operation.document,
    variables: { input: { name: "Initiative 701" } },
  }
  const duplicate = await exercise(spec, [request, request])
  assertEquals(duplicate.statuses, [200, 500])
  assertEquals(duplicate.consumed, 1)
  assertEquals(duplicate.records, spec.expectedRecords)
  assert(duplicate.mismatches.length > 0)
  const missing = await exercise(spec, [])
  assertEquals(missing.consumed, 0)
  assertEquals(missing.records, {})
  assert(missing.mismatches.length > 0)
})

Deno.test("C039 false success suppresses a declared test put", async () => {
  const frozen = await fixture("false-success")
  assertEquals(first(frozen).effects, [])
  const modified = structuredClone(frozen)
  const step = first(modified)
  assert(step.response.kind === "data")
  const payload = step.response.data
  assert(payload != null && typeof payload === "object")
  const initiativeCreate = Reflect.get(payload, "initiativeCreate")
  assert(initiativeCreate != null && typeof initiativeCreate === "object")
  const initiative = Reflect.get(initiativeCreate, "initiative")
  assert(initiative != null && typeof initiative === "object")
  const id = Reflect.get(initiative, "id")
  assert(typeof id === "string")
  step.effects.push({
    kind: "put",
    record: `Initiative:${id}`,
    before: { absent: true },
    after: initiative,
  })
  const result = await exercise(modified, [{
    query: step.operation.document,
    variables: { input: { name: "Initiative 705" } },
  }])
  assertEquals(result.statuses, [200])
  assertEquals(result.consumed, 1)
  assertEquals(result.records, {})
  assertEquals(result.mismatches, [])
})

Deno.test("C039 partial GraphQL errors require explicit effect permission", async () => {
  const spec = await fixture("partial-data-errors")
  const step = first(spec)
  requirePartialCreateData(spec)
  assertEquals(step.partialEffects, true)
  assertEquals(step.effects.length, 1)
  const request = {
    query: step.operation.document,
    variables: { input: { name: "Initiative 707" } },
  }
  const accepted = await exercise(spec, [request])
  assertEquals(accepted.statuses, [200])
  assertEquals(accepted.records, spec.expectedRecords)
  assertEquals(accepted.mismatches, [])
  const withoutPermission = structuredClone(spec)
  delete first(withoutPermission).partialEffects
  assertEquals(
    v.safeParse(GraphQLFixtureSchema, withoutPermission).success,
    false,
  )
  const nullData = structuredClone(spec)
  const nullStep = first(nullData)
  assert(nullStep.response.kind === "graphqlErrors")
  nullStep.response.data = null
  // This source-specific invariant is stricter than generic GraphQL schema.
  assertThrows(() => requirePartialCreateData(nullData))
})
