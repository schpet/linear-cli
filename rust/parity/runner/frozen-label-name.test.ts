import {
  assertEquals,
  assertNotEquals,
  assertRejects,
  assertThrows,
} from "@std/assert"
import { buildSchema, parse, print } from "graphql"
import { harnessDigest } from "./baseline-cache.ts"
import { loadCases } from "./cases.ts"
import {
  C018_SOURCE_GET_LABEL_BY_NAME,
  withoutFrozenLabelTeamDeclaration,
} from "./frozen-label-name.ts"
import { matchGraphQL, type OperationInput } from "./graphql-match.ts"
import { startGraphQLServer } from "./graphql-server.ts"
import {
  FROZEN_USER_AGENT,
  parseCase,
  RUST_USER_AGENT,
  SchemaError,
} from "./schema.ts"

const schema = buildSchema(
  await Deno.readTextFile(
    new URL("../../../graphql/schema.graphql", import.meta.url),
  ),
)
const raw: OperationInput = {
  document: C018_SOURCE_GET_LABEL_BY_NAME,
  variables: { name: "A" },
}
const valid = withoutFrozenLabelTeamDeclaration(raw)

Deno.test("C018 comparator pins both frozen source literal and generated AST", async () => {
  const codegen = await Deno.readTextFile(
    new URL("../../../src/__codegen__/graphql.ts", import.meta.url),
  )
  const generatedJson = codegen.match(
    /export const GetLabelByNameDocument = (.*?) as unknown as DocumentNode/,
  )?.[1]
  if (generatedJson == null) throw new Error("generated AST missing")
  const generated: unknown = JSON.parse(generatedJson)
  const pinned: unknown = JSON.parse(
    JSON.stringify(
      parse(raw.document, { noLocation: true }),
      (_key, value: unknown) =>
        Array.isArray(value) && value.length === 0 ? undefined : value,
    ),
  )
  assertEquals(generated, pinned)
  const source = await Deno.readTextFile(
    new URL("../../../src/commands/label/label-delete.ts", import.meta.url),
  )
  const literal = source.match(/const GetLabelByName = gql\(`([\s\S]*?)`\)/)
    ?.[1]
  if (literal == null) throw new Error("source literal missing")
  assertEquals(print(parse(literal)), raw.document)
  assertEquals(raw.variables, { name: "A" })
  assertEquals(raw.document.includes("$teamKey: String"), true)
  for (const operationName of [undefined, "GetLabelByName"]) {
    const copy = withoutFrozenLabelTeamDeclaration({ ...raw, operationName })
    assertEquals(matchGraphQL(valid, copy, schema).matches, true)
    assertEquals(copy.variables, raw.variables)
    assertEquals(copy.document.includes("teamKey"), false)
  }
  assertEquals(matchGraphQL(valid, valid, schema).matches, true)
  assertThrows(() => matchGraphQL(raw, raw, schema), Error, "invalid fixture")
})

Deno.test("C018 comparator refuses all unpinned documents and variable origins", () => {
  const controls: OperationInput[] = [
    { ...raw, operationName: "Wrong" },
    { ...raw, variables: {} },
    { ...raw, variables: undefined },
    { ...raw, variables: { name: "A", teamKey: "ENG" } },
    { ...raw, variables: { name: "A", teamKey: null } },
    ...[
      raw.document.replace("$teamKey: String", "$teamKey: String!"),
      raw.document.replace("$teamKey: String", '$teamKey: String = "ENG"'),
      raw.document.replace(
        "$teamKey: String",
        "$teamKey: String @skip(if: true)",
      ),
      raw.document.replace("$teamKey: String", "$other: String"),
      raw.document.replace("eqIgnoreCase: $name", "eqIgnoreCase: $teamKey"),
      raw.document.replace("color", "description"),
      raw.document.replace("color", "color description"),
      "mutation DeleteIssueLabel($id: String!, $teamKey: String) { issueLabelDelete(id: $id) { success } }",
      "not a document",
    ].map((document) => ({ ...raw, document })),
  ]
  for (const input of controls) {
    assertEquals(withoutFrozenLabelTeamDeclaration(input), input)
    assertEquals(matchGraphQL(valid, input, schema).matches, false)
  }
})

Deno.test("C018 server accommodation is actual-only, source-only and preserves validationErrors", async () => {
  const modes: [string, boolean, number][] = [
    [FROZEN_USER_AGENT, false, 200],
    [RUST_USER_AGENT, false, 500],
    [FROZEN_USER_AGENT, true, 200],
  ]
  for (const [userAgent, validation, expectedStatus] of modes) {
    const document = validation ? raw.document : valid.document
    const response = validation
      ? {
        kind: "validationErrors",
        status: 200,
        errors: [{
          message:
            'Variable "$teamKey" is never used in operation "GetLabelByName".',
        }],
      }
      : { kind: "data", data: { issueLabels: { nodes: [] } } }
    const input = parseCase({
      ...JSON.parse(
        await Deno.readTextFile(
          new URL("c017-frozen-cases/c017-false-success.json", import.meta.url),
        ),
      ),
      deviation: null,
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
            kind: "graphql",
            id: "name",
            operation: { document, variables: { name: "A" } },
            identity: {
              authorization: "lin_api_fake",
              userAgent: FROZEN_USER_AGENT,
              headers: {},
            },
            response,
            effects: [],
          }],
        }],
      },
    })
    if (input.graphql == null) throw new Error("fixture missing")
    const fixture = {
      ...input.graphql,
      groups: input.graphql.groups.map((group) =>
        group.mode === "ordered"
          ? {
            ...group,
            steps: group.steps.map((step) =>
              step.kind === "graphql"
                ? { ...step, identity: { ...step.identity, userAgent } }
                : step
            ),
          }
          : group
      ),
    }
    const server = startGraphQLServer(() => fixture, schema)
    try {
      const requestBody = JSON.stringify({
        query: raw.document,
        variables: raw.variables,
      })
      const result = await fetch(`http://127.0.0.1:${server.port}/graphql`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          authorization: "lin_api_fake",
          "user-agent": userAgent,
        },
        body: requestBody,
      })
      assertEquals(result.status, expectedStatus)
      const body = await result.json()
      if (validation) assertEquals(body, { errors: response.errors })
      assertEquals(server.consumed, expectedStatus === 200 ? 1 : 0)
      assertEquals(JSON.parse(requestBody).query, raw.document)
    } finally {
      await server.stop()
    }
  }
})

Deno.test("C018 raw-invalid expected fixture remains rejected at load", async () => {
  const dir = await Deno.makeTempDir()
  try {
    const source = JSON.parse(
      await Deno.readTextFile(
        new URL("c017-frozen-cases/c017-false-success.json", import.meta.url),
      ),
    )
    source.deviation = null
    source.graphql.groups[0].steps[0] = {
      kind: "graphql",
      id: "name",
      operation: raw,
      identity: {
        authorization: "lin_api_fake",
        userAgent: FROZEN_USER_AGENT,
        headers: {},
      },
      response: { kind: "data", data: { issueLabels: { nodes: [] } } },
      effects: [],
    }
    await Deno.writeTextFile(`${dir}/raw.json`, JSON.stringify(source))
    await assertRejects(
      () => loadCases(dir, new Set([source.route])),
      SchemaError,
      "invalid fixture",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("C018 helper content independently invalidates baseline harness identity", async () => {
  const dir = await Deno.makeTempDir()
  const runner = `${dir}/runner`
  try {
    await Deno.mkdir(`${runner}/helpers`, { recursive: true })
    await Deno.mkdir(`${runner}/certs`)
    for (
      const name of ["deno.json", "deno.lock", "verify.ts", "source-map.ts"]
    ) {
      await Deno.writeTextFile(`${dir}/${name}`, "unchanged")
    }
    await Deno.writeTextFile(`${runner}/graphql-server.ts`, "unchanged")
    await Deno.writeTextFile(
      `${runner}/frozen-label-name.ts`,
      "first helper bytes",
    )
    const before = await harnessDigest(runner)
    await Deno.writeTextFile(
      `${runner}/frozen-label-name.ts`,
      "different helper bytes",
    )
    assertNotEquals(await harnessDigest(runner), before)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
