import { assertEquals } from "@std/assert"
import { compareFixture } from "./compare.ts"
import { startFixtureServer } from "./fixture-server.ts"
import type { FixtureServerSpec } from "./schema.ts"

Deno.test("fixture server serves scripted responses in order, records requests and flags extras", async () => {
  const spec: FixtureServerSpec = {
    path: "/graphql",
    responses: [
      {
        status: 200,
        headers: { "content-type": "application/json" },
        body: { utf8: '{"data":{"port":{{PORT}}}}' },
      },
      { status: 401, headers: {}, body: { utf8: "denied" } },
    ],
    expectedRequests: 2,
    expectedAuthorization: "lin_api_fake",
  }
  const server = startFixtureServer((port) => ({
    ...spec,
    responses: spec.responses.map((response) => ({
      ...response,
      body: {
        utf8: ("utf8" in response.body ? response.body.utf8 : "").replace(
          "{{PORT}}",
          String(port),
        ),
      },
    })),
  }))
  try {
    const url = `http://127.0.0.1:${server.port}/graphql`
    const first = await fetch(url, {
      method: "POST",
      headers: { authorization: "lin_api_fake", "user-agent": "t/1" },
      body: '{"query":"{ viewer { id } }"}',
    })
    assertEquals(first.status, 200)
    assertEquals(await first.text(), `{"data":{"port":${server.port}}}`)
    const second = await fetch(url, { method: "POST", body: "{}" })
    assertEquals(second.status, 401)
    assertEquals(await second.text(), "denied")
    assertEquals(compareFixture(spec, server).map((m) => m.detail), [
      "request 1 Authorization missing",
    ])

    const third = await fetch(`http://127.0.0.1:${server.port}/other`, {
      method: "POST",
      headers: { authorization: "lin_api_fake" },
      body: "{}",
    })
    assertEquals(third.status, 500)
    await third.text()
    assertEquals(server.requests.length, 3)
    assertEquals(server.unexpected, 1)
    assertEquals(server.requests[0].authorization, "lin_api_fake")
    assertEquals(server.requests[0].userAgent, "t/1")
    assertEquals(
      new TextDecoder().decode(server.requests[0].body),
      '{"query":"{ viewer { id } }"}',
    )
    const mismatches = compareFixture(spec, server)
    assertEquals(mismatches.map((m) => m.surface), [
      "fixture",
      "fixture",
      "fixture",
    ])
    assertEquals(
      mismatches[0].detail,
      "expected 2 fixture requests, got 3 (1 unscripted)",
    )
    assertEquals(mismatches[2].detail, "request 2 path /other is not /graphql")
  } finally {
    await server.stop()
  }
})
