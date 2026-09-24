import { assert, assertEquals, assertStringIncludes } from "@std/assert"
import { buildSchema } from "graphql"
import * as v from "valibot"
import { compareGraphQLFixture } from "./compare.ts"
import { startGraphQLServer } from "./graphql-server.ts"
import { type AssetStepSpec, GraphQLFixtureSchema } from "./schema.ts"

const schema = buildSchema("type Query { ping: String }")
function asset(
  id: string,
  method: "GET" | "PUT",
  path: string,
  requestBody = "",
  responseBody = "ok",
): AssetStepSpec {
  return {
    kind: "asset",
    id,
    method,
    path,
    requiredHeaders: {},
    forbiddenHeaders: [],
    body: { utf8: requestBody },
    response: { status: 200, headers: {}, body: { utf8: responseBody } },
  }
}
function fixture(
  steps: unknown[],
  expectedRecords: Record<string, unknown> = {},
) {
  return v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: steps.length,
    initialRecords: {},
    expectedRecords,
    groups: [{ mode: "ordered", steps }],
  })
}
function request(
  port: number,
  path: string,
  method = "GET",
  body?: string,
  headers?: Record<string, string>,
) {
  return fetch(`http://127.0.0.1:${port}${path}`, {
    method,
    body,
    headers,
    redirect: "manual",
  })
}

Deno.test("same listener matches exact signed PUT bytes/header and counts an origin-relative GET redirect hop", async () => {
  const upload = asset(
    "put",
    "PUT",
    "/signed/upload?token=fake",
    "bytes",
    "stored",
  )
  upload.requiredHeaders = { "content-type": "application/octet-stream" }
  const first = asset("redirect", "GET", "/signed/file?token=fake")
  first.response = {
    status: 302,
    headers: {},
    location: "/signed/final?token=fake",
    body: { utf8: "" },
  }
  const last = asset("final", "GET", "/signed/final?token=fake", "", "file")
  const spec = fixture([upload, first, last])
  const server = startGraphQLServer(() => spec, schema)
  try {
    const put = await request(server.port, upload.path, "PUT", "bytes", {
      "content-type": "application/octet-stream",
    })
    assertEquals(put.status, 200)
    assertEquals(await put.text(), "stored")
    const redirect = await request(server.port, first.path)
    assertEquals(redirect.status, 302)
    assertEquals(redirect.headers.get("location"), first.response.location)
    await redirect.arrayBuffer()
    const final = await request(server.port, last.path)
    assertEquals(await final.text(), "file")
    assertEquals(server.consumed, 3)
    assertEquals(server.expectedAssets, 3)
    assertEquals(server.expectedGraphQL, 0)
    assertEquals(compareGraphQLFixture(spec, server), [])
  } finally {
    await server.stop()
  }
})

Deno.test("wrong PUT body/header, missing PUT and extra asset fail on fixture", async () => {
  const put = asset("put", "PUT", "/signed?token=fake", "correct")
  put.requiredHeaders = { "content-type": "application/octet-stream" }
  for (
    const { body, headers } of [
      {
        body: "wrong",
        headers: { "content-type": "application/octet-stream" },
      },
      { body: "correct", headers: { "content-type": "text/plain" } },
    ]
  ) {
    const spec = fixture([put])
    const server = startGraphQLServer(() => spec, schema)
    try {
      const response = await request(
        server.port,
        put.path,
        "PUT",
        body,
        headers,
      )
      assertEquals(response.status, 500)
      await response.arrayBuffer()
      assertEquals(server.consumed, 0)
      assertEquals(
        compareGraphQLFixture(spec, server).some((item) =>
          item.surface === "fixture"
        ),
        true,
      )
    } finally {
      await server.stop()
    }
  }
  const spec = fixture([put])
  const server = startGraphQLServer(() => spec, schema)
  try {
    assertEquals(
      compareGraphQLFixture(spec, server).some((item) =>
        item.detail.includes("consumed 0")
      ),
      true,
    )
    const response = await request(server.port, "/extra")
    assertEquals(response.status, 500)
    await response.arrayBuffer()
    assertStringIncludes(server.issues[0], "eligible")
  } finally {
    await server.stop()
  }
})

Deno.test("stopping a held asset lane releases its handler before one second", async () => {
  const one = asset("one", "GET", "/one")
  const two = asset("two", "GET", "/two")
  const spec = v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: 2,
    initialRecords: {},
    expectedRecords: {},
    groups: [{
      mode: "lanes",
      timeoutMs: 5000,
      lanes: [{ id: "one", steps: [one] }, { id: "two", steps: [two] }],
    }],
  })
  const server = startGraphQLServer(() => spec, schema)
  const pending = request(server.port, "/one")
  await new Promise((resolve) => setTimeout(resolve, 30))
  const started = performance.now()
  await server.stop()
  const response = await pending
  assertEquals(response.status, 500)
  await response.arrayBuffer()
  assertEquals(performance.now() - started < 1000, true)
  assertEquals(server.issues, [])
})

Deno.test("a disconnected first-step client releases the lane barrier", async () => {
  const one = asset("one", "GET", "/one")
  const two = asset("two", "GET", "/two")
  const spec = v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: 2,
    initialRecords: {},
    expectedRecords: {},
    groups: [{
      mode: "lanes",
      timeoutMs: 5000,
      lanes: [{ id: "one", steps: [one] }, { id: "two", steps: [two] }],
    }],
  })
  const server = startGraphQLServer(() => spec, schema)
  try {
    const socket = await Deno.connect({
      hostname: "127.0.0.1",
      port: server.port,
    })
    const requestBytes = new TextEncoder().encode(
      `GET /one HTTP/1.1\r\nHost: 127.0.0.1:${server.port}\r\n\r\n`,
    )
    await socket.write(requestBytes)
    const acceptedAt = performance.now()
    while (
      server.requests.length === 0 && performance.now() - acceptedAt < 500
    ) {
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
    assertEquals(server.requests.length, 1)
    socket.close()
    const started = performance.now()
    while (
      !server.issues.some((issue) => issue.includes("client disconnected")) &&
      performance.now() - started < 900
    ) {
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
    assert(
      server.issues.some((issue) => issue.includes("client disconnected")),
    )
    assert(performance.now() - started < 1000)
    assertEquals(server.consumed, 0)
  } finally {
    await server.stop()
  }
})
