import { assertEquals, assertStringIncludes } from "@std/assert"
import { buildSchema } from "graphql"
import * as v from "valibot"
import { compareGraphQLFixture } from "./compare.ts"
import { startGraphQLServer } from "./graphql-server.ts"
import { GraphQLFixtureSchema, type GraphQLStepSpec } from "./schema.ts"

const schema = buildSchema(`
  scalar UUID
  scalar JSON
  scalar DateTime
  scalar DateTimeOrDuration
  interface Node { id: ID! }
  scalar JSONObject
  type Issue implements Node { id: ID!, title: String, success: Boolean, uuid: UUID, meta: JSONObject, payload: JSON, when: DateTime, whenOrDuration: DateTimeOrDuration }
  type Query { issue: Issue, node: Node }
  type Mutation { setTitle(title: String!): Issue }
`)
const identity: GraphQLStepSpec["identity"] = {
  authorization: "lin_api_fake",
  userAgent: "schpet-linear-cli/2.6.0",
  headers: {},
}

function step(
  id: string,
  document: string,
  data: unknown,
  variables?: Record<string, unknown>,
): GraphQLStepSpec {
  return {
    kind: "graphql",
    id,
    operation: { document, ...(variables == null ? {} : { variables }) },
    identity,
    response: { kind: "data", data },
    effects: [],
  }
}

function fixture(
  steps: GraphQLStepSpec[],
  initialRecords: Record<string, unknown> = {},
  expectedRecords: Record<string, unknown> = initialRecords,
) {
  return v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: steps.length,
    initialRecords,
    expectedRecords,
    groups: [{ mode: "ordered", steps }],
  })
}

function post(
  port: number,
  body: unknown,
  headers: Record<string, string> = {},
): Promise<Response> {
  return fetch(`http://127.0.0.1:${port}/graphql`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: "lin_api_fake",
      "user-agent": "schpet-linear-cli/2.6.0",
      ...headers,
    },
    body: JSON.stringify(body),
  })
}
async function status(response: Response): Promise<number> {
  const code = response.status
  await response.arrayBuffer()
  return code
}

Deno.test("GraphQL server projects aliases and only requires fields actually executed", async () => {
  const document =
    "query($show: Boolean!) { alias: issue { id title @include(if: $show) } }"
  const spec = fixture([
    step("one", document, { issue: { id: "i1" } }, { show: false }),
  ])
  const server = startGraphQLServer(() => spec, schema)
  try {
    const response = await post(server.port, {
      query: document,
      variables: { show: false },
    })
    assertEquals(response.status, 200)
    assertEquals(await response.json(), { data: { alias: { id: "i1" } } })
    assertEquals(server.issues, [])
    assertEquals(server.consumed, 1)
  } finally {
    await server.stop()
  }
})

Deno.test("missing nullable field and wrong leaf JS type are fixture failures", async () => {
  const missing = fixture([
    step("missing", "{ issue { id title } }", { issue: { id: "i1" } }),
  ])
  const badType = fixture([
    step("bad-type", "{ issue { id success } }", {
      issue: { id: "i1", success: 1 },
    }),
  ])
  for (
    const { spec, query } of [{
      spec: missing,
      query: "{ issue { id title } }",
    }, { spec: badType, query: "{ issue { id success } }" }]
  ) {
    const server = startGraphQLServer(() => spec, schema)
    try {
      const response = await post(server.port, { query })
      assertEquals(response.status, 500)
      assertEquals(await response.json(), {
        errors: [{ message: "fixture mismatch" }],
      })
      assertEquals(server.consumed, 0)
      assertEquals(server.issues.length, 1)
    } finally {
      await server.stop()
    }
  }
})

Deno.test("request envelope, identity, exact count and unconsumed steps are observable", async () => {
  const query = "{ issue { id } }"
  const spec = fixture([
    step("first", query, { issue: { id: "i1" } }),
    step("second", query, { issue: { id: "i2" } }),
  ])
  const server = startGraphQLServer(() => spec, schema)
  try {
    assertEquals(
      await status(await post(server.port, { query, variables: {} })),
      500,
    )
    assertEquals(
      await status(
        await post(server.port, { query }, { authorization: "lin_api_other" }),
      ),
      500,
    )
    assertEquals(await status(await post(server.port, { query })), 500)
    assertEquals(server.consumed, 0)
    assertEquals(server.requests.length, 3)
    assertEquals(server.issues.length, 4)
    assertEquals(
      server.issues.filter((issue) =>
        issue === "request arrived after prior fixture failure"
      ).length,
      2,
    )
    assertEquals(server.unexpected, 0)
  } finally {
    await server.stop()
  }
})

Deno.test("mutation effects update named records; wrong prior state rolls back", async () => {
  const mutation = step(
    "update",
    "mutation($title: String!) { setTitle(title: $title) { id title } }",
    { setTitle: { id: "i1", title: "new" } },
    { title: "new" },
  )
  mutation.effects.push({
    kind: "put",
    record: "Issue:i1",
    before: { value: { id: "i1", title: "old" } },
    after: { id: "i1", title: "new" },
  })
  const read = step("read", "{ issue { id title } }", {
    issue: { $record: "Issue:i1" },
  })
  const initial = { "Issue:i1": { id: "i1", title: "old" } }
  const expected = { "Issue:i1": { id: "i1", title: "new" } }
  const server = startGraphQLServer(
    () => fixture([mutation, read], initial, expected),
    schema,
  )
  try {
    assertEquals(
      await status(
        await post(server.port, {
          query: mutation.operation.document,
          variables: { title: "new" },
        }),
      ),
      200,
    )
    const response = await post(server.port, { query: read.operation.document })
    assertEquals(await response.json(), {
      data: { issue: { id: "i1", title: "new" } },
    })
    assertEquals(server.state.matches(expected), true)
    assertEquals(server.consumed, 2)
    assertEquals(
      await status(await post(server.port, { query: read.operation.document })),
      500,
    )
  } finally {
    await server.stop()
  }

  mutation.effects[0] = {
    kind: "put",
    record: "Issue:i1",
    before: { value: { id: "i1", title: "wrong" } },
    after: { id: "i1", title: "new" },
  }
  const failed = startGraphQLServer(
    () => fixture([mutation], initial, expected),
    schema,
  )
  try {
    assertEquals(
      await status(
        await post(failed.port, {
          query: mutation.operation.document,
          variables: { title: "new" },
        }),
      ),
      500,
    )
    assertEquals(failed.state.matches(initial), true)
    assertEquals(failed.consumed, 0)
  } finally {
    await failed.stop()
  }
})

Deno.test("success:false through a named record does not apply effects", async () => {
  const initial = { "Issue:i1": { id: "i1", success: false } }
  const mutation = step(
    "declined",
    'mutation { setTitle(title: "new") { id success } }',
    { setTitle: { $record: "Issue:i1" } },
  )
  mutation.effects.push({
    kind: "put",
    record: "Issue:i1",
    before: { value: initial["Issue:i1"] },
    after: { id: "i1", success: true },
  })
  const server = startGraphQLServer(() => fixture([mutation], initial), schema)
  try {
    const response = await post(server.port, {
      query: mutation.operation.document,
    })
    assertEquals(await response.json(), {
      data: { setTitle: { id: "i1", success: false } },
    })
    assertEquals(server.state.matches(initial), true)
    assertEquals(server.consumed, 1)
  } finally {
    await server.stop()
  }
})

Deno.test("effect policy finds schema success despite alias or omitted selection, but ignores JSONObject data", async () => {
  for (const selection of ["id ok: success", "id"]) {
    const initial = { "Issue:i1": { id: "i1", success: false } }
    const mutation = step(
      "declined",
      `mutation { setTitle(title: "new") { ${selection} } }`,
      {
        setTitle: { $record: "Issue:i1" },
      },
    )
    mutation.effects.push({
      kind: "put",
      record: "Issue:i1",
      before: { value: initial["Issue:i1"] },
      after: { id: "i1", success: true },
    })
    const server = startGraphQLServer(
      () => fixture([mutation], initial),
      schema,
    )
    try {
      assertEquals(
        await status(
          await post(server.port, { query: mutation.operation.document }),
        ),
        200,
      )
      assertEquals(server.state.matches(initial), true)
    } finally {
      await server.stop()
    }
  }

  const initial = { "Issue:i1": { id: "i1", title: "old" } }
  const mutation = step(
    "json",
    'mutation { setTitle(title: "new") { id meta } }',
    {
      setTitle: { id: "i1", meta: { success: false } },
    },
  )
  mutation.effects.push({
    kind: "put",
    record: "Issue:i1",
    before: { value: initial["Issue:i1"] },
    after: { id: "i1", title: "new" },
  })
  const expected = { "Issue:i1": { id: "i1", title: "new" } }
  const server = startGraphQLServer(
    () => fixture([mutation], initial, expected),
    schema,
  )
  try {
    assertEquals(
      await status(
        await post(server.port, { query: mutation.operation.document }),
      ),
      200,
    )
    assertEquals(server.state.matches(expected), true)
  } finally {
    await server.stop()
  }
})

Deno.test("success:false suppresses effects when mutation payload selects only __typename", async () => {
  const initial = {
    "Issue:i1": { id: "i1", success: false, __typename: "Issue" },
  }
  const mutation = step(
    "typename-only",
    'mutation { setTitle(title: "new") { __typename } }',
    {
      setTitle: { $record: "Issue:i1" },
    },
  )
  mutation.effects.push({
    kind: "put",
    record: "Issue:i1",
    before: { value: initial["Issue:i1"] },
    after: { id: "i1", success: true, __typename: "Issue" },
  })
  const server = startGraphQLServer(() => fixture([mutation], initial), schema)
  try {
    const response = await post(server.port, {
      query: mutation.operation.document,
    })
    assertEquals(response.status, 200)
    assertEquals(await response.json(), {
      data: { setTitle: { __typename: "Issue" } },
    })
    assertEquals(server.state.matches(initial), true)
    assertEquals(server.consumed, 1)
  } finally {
    await server.stop()
  }
})

Deno.test("concrete payload __typename must match its SDL type even with only __typename selected", async () => {
  const initial = {
    "Issue:i1": { id: "i1", success: false, __typename: "Nope" },
  }
  const mutation = step(
    "wrong-type",
    'mutation { setTitle(title: "new") { __typename } }',
    {
      setTitle: { $record: "Issue:i1" },
    },
  )
  mutation.effects.push({
    kind: "put",
    record: "Issue:i1",
    before: { value: initial["Issue:i1"] },
    after: { id: "i1", success: true },
  })
  const server = startGraphQLServer(() => fixture([mutation], initial), schema)
  try {
    const response = await post(server.port, {
      query: mutation.operation.document,
    })
    assertEquals(response.status, 500)
    await response.arrayBuffer()
    assertStringIncludes(server.issues[0], "concrete record __typename")
    assertEquals(server.state.matches(initial), true)
    assertEquals(server.consumed, 0)
  } finally {
    await server.stop()
  }
})

Deno.test("statically skipped root mutation does not apply its effect", async () => {
  const mutation = step(
    "skipped",
    'mutation { setTitle(title: "new") @skip(if: true) { __typename } }',
    {},
  )
  mutation.effects.push({
    kind: "put",
    record: "Issue:i1",
    before: { absent: true },
    after: { id: "i1" },
  })
  const server = startGraphQLServer(() => fixture([mutation]), schema)
  try {
    const response = await post(server.port, {
      query: mutation.operation.document,
    })
    assertEquals(response.status, 200)
    assertEquals(await response.json(), { data: {} })
    assertEquals(server.state.matches({}), true)
    assertEquals(server.consumed, 1)
  } finally {
    await server.stop()
  }
})

Deno.test("JSON is stringified JSON and DateTime rejects impossible calendar days", async () => {
  const cases = [
    {
      query: "{ issue { id payload } }",
      data: { issue: { id: "i1", payload: { one: 1 } } },
      valid: false,
    },
    {
      query: "{ issue { id payload } }",
      data: { issue: { id: "i1", payload: "not-json" } },
      valid: false,
    },
    {
      query: "{ issue { id payload } }",
      data: { issue: { id: "i1", payload: '{"one":1}' } },
      valid: true,
    },
    {
      query: "{ issue { id when } }",
      data: { issue: { id: "i1", when: "2026-02-30T00:00:00Z" } },
      valid: false,
    },
    {
      query: "{ issue { id when } }",
      data: { issue: { id: "i1", when: "2026-02-28T00:00:00Z" } },
      valid: true,
    },
    {
      query: "{ issue { id whenOrDuration } }",
      data: { issue: { id: "i1", whenOrDuration: "2026-02-30T00:00:00Z" } },
      valid: false,
    },
  ]
  for (const item of cases) {
    const server = startGraphQLServer(
      () => fixture([step("one", item.query, item.data)]),
      schema,
    )
    try {
      assertEquals(
        await status(await post(server.port, { query: item.query })),
        item.valid ? 200 : 500,
      )
      assertEquals(server.consumed, item.valid ? 1 : 0)
    } finally {
      await server.stop()
    }
  }
})

Deno.test("abstract records require concrete __typename and transport override preserves bytes", async () => {
  const missingType = fixture([
    step("node", "{ node { id } }", { node: { id: "i1" } }),
  ])
  const server = startGraphQLServer(() => missingType, schema)
  try {
    assertEquals(
      await status(await post(server.port, { query: "{ node { id } }" })),
      500,
    )
    assertStringIncludes(server.issues[0], "__typename")
  } finally {
    await server.stop()
  }

  const transport = step("transport", "{ issue { id } }", {})
  transport.response = {
    kind: "transport",
    status: 503,
    headers: { "x-fixture": "yes" },
    body: { base64: "AAE=" },
  }
  const override = startGraphQLServer(() => fixture([transport]), schema)
  try {
    const response = await post(override.port, {
      query: transport.operation.document,
    })
    assertEquals(response.status, 503)
    assertEquals(response.headers.get("x-fixture"), "yes")
    assertEquals([...new Uint8Array(await response.arrayBuffer())], [0, 1])
  } finally {
    await override.stop()
  }
})

Deno.test("record references are only interpreted as composite sources", async () => {
  const literal = fixture([
    step("json", "{ issue { id meta } }", {
      issue: { id: "i1", meta: { $record: "literal" } },
    }),
  ])
  const server = startGraphQLServer(() => literal, schema)
  try {
    const response = await post(server.port, { query: "{ issue { id meta } }" })
    assertEquals(response.status, 200)
    assertEquals(await response.json(), {
      data: { issue: { id: "i1", meta: { $record: "literal" } } },
    })
  } finally {
    await server.stop()
  }

  const missing = fixture([
    step("missing", "{ issue { id } }", {
      issue: { $record: "Issue:missing" },
    }),
  ])
  const missingServer = startGraphQLServer(() => missing, schema)
  try {
    assertEquals(
      await status(
        await post(missingServer.port, { query: "{ issue { id } }" }),
      ),
      500,
    )
    assertStringIncludes(missingServer.issues[0], "record reference")
    assertEquals(missingServer.consumed, 0)
  } finally {
    await missingServer.stop()
  }

  const malformed = fixture([
    step("malformed", "{ issue { id } }", { issue: { $record: 42 } }),
  ])
  const malformedServer = startGraphQLServer(() => malformed, schema)
  try {
    assertEquals(
      await status(
        await post(malformedServer.port, { query: "{ issue { id } }" }),
      ),
      500,
    )
    assertStringIncludes(malformedServer.issues[0], "record reference")
    assertEquals(malformedServer.consumed, 0)
  } finally {
    await malformedServer.stop()
  }

  const mixed = fixture([
    step("mixed", "{ issue { id } }", {
      issue: { $record: "Issue:i1", id: "i1" },
    }),
  ], { "Issue:i1": { id: "i1" } })
  const mixedServer = startGraphQLServer(() => mixed, schema)
  try {
    assertEquals(
      await status(await post(mixedServer.port, { query: "{ issue { id } }" })),
      500,
    )
    assertStringIncludes(mixedServer.issues[0], "record reference")
    assertEquals(mixedServer.consumed, 0)
  } finally {
    await mixedServer.stop()
  }
})

Deno.test("fixture comparison catches skipped pages, duplicate requests and missing effects", async () => {
  const query = "{ issue { id } }"
  const twoPages = fixture([
    step("first", query, { issue: { id: "i1" } }),
    step("second", query, { issue: { id: "i2" } }),
  ])
  const skipped = startGraphQLServer(() => twoPages, schema)
  try {
    await status(await post(skipped.port, { query }))
    assertEquals(
      compareGraphQLFixture(twoPages, skipped).some((item) =>
        item.surface === "fixture" && item.detail.includes("consumed 1")
      ),
      true,
    )
  } finally {
    await skipped.stop()
  }

  const mutation = 'mutation { setTitle(title: "new") { id title } }'
  const one = fixture([
    step("once", mutation, { setTitle: { id: "i1", title: "new" } }),
  ])
  const duplicate = startGraphQLServer(() => one, schema)
  try {
    await status(await post(duplicate.port, { query: mutation }))
    assertEquals(
      await status(await post(duplicate.port, { query: mutation })),
      500,
    )
    assertEquals(
      compareGraphQLFixture(one, duplicate).some((item) =>
        item.detail.includes("unexpected request")
      ),
      true,
    )
    assertEquals(duplicate.unexpected, 1)
  } finally {
    await duplicate.stop()
  }

  const expectedRecords = { "Issue:i1": { id: "i1", title: "new" } }
  const missingEffect = fixture(
    [step("read", query, { issue: { id: "i1" } })],
    { "Issue:i1": { id: "i1", title: "old" } },
    expectedRecords,
  )
  const unchanged = startGraphQLServer(() => missingEffect, schema)
  try {
    await status(await post(unchanged.port, { query }))
    assertEquals(
      compareGraphQLFixture(missingEffect, unchanged).some((item) =>
        item.detail === 'final GraphQL records differ at $["Issue:i1"]["title"]'
      ),
      true,
    )
  } finally {
    await unchanged.stop()
  }
})

Deno.test("declared GraphQL errors preserve status, data, paths and extensions", async () => {
  const errorStep = step("error", "{ issue { id } }", null)
  errorStep.response = {
    kind: "graphqlErrors",
    status: 403,
    data: null,
    errors: [{
      message: "Forbidden",
      path: ["issue"],
      extensions: { userPresentableMessage: "Access denied" },
    }],
  }
  const server = startGraphQLServer(() => fixture([errorStep]), schema)
  try {
    const response = await post(server.port, {
      query: errorStep.operation.document,
    })
    assertEquals(response.status, 403)
    assertEquals(await response.json(), {
      data: null,
      errors: [{
        message: "Forbidden",
        path: ["issue"],
        extensions: { userPresentableMessage: "Access denied" },
      }],
    })
  } finally {
    await server.stop()
  }
})
