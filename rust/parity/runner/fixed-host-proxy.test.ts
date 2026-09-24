import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
} from "@std/assert"
import { buildSchema } from "graphql"
import * as v from "valibot"
import { compareGraphQLFixture } from "./compare.ts"
import { startFixedHostProxy } from "./fixed-host-proxy.ts"
import { startGraphQLServer } from "./graphql-server.ts"
import { GraphQLFixtureSchema } from "./schema.ts"

const ca = await Deno.readTextFile(
  new URL("./certs/test-ca.pem", import.meta.url),
)
const schema = buildSchema("type Query { ping: String }")
function fixture() {
  return v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: 1,
    initialRecords: {},
    expectedRecords: {},
    groups: [{
      mode: "ordered",
      steps: [{
        kind: "asset",
        id: "private",
        method: "GET",
        fixedHost: "uploads.linear.app",
        path: "/private?token=fake",
        requiredHeaders: { Authorization: "lin_api_fake_probe" },
        forbiddenHeaders: [],
        body: { utf8: "" },
        response: {
          status: 200,
          headers: { "content-type": "text/plain" },
          body: { utf8: "bytes" },
        },
      }],
    }],
  })
}
async function withProxy(
  run: (
    server: ReturnType<typeof startGraphQLServer>,
    proxy: Awaited<ReturnType<typeof startFixedHostProxy>>,
  ) => Promise<void>,
): Promise<void> {
  const spec = fixture()
  const server = startGraphQLServer(() => spec, schema)
  const proxy = await startFixedHostProxy(
    server,
    new Set(["uploads.linear.app"]),
  )
  try {
    await run(server, proxy)
  } finally {
    await proxy.stop()
    await server.stop()
  }
}
function client(port: number, useCa = true) {
  return Deno.createHttpClient({
    proxy: { url: `http://127.0.0.1:${port}` },
    caCerts: useCa ? [ca] : [],
    http1: false,
    http2: true,
  })
}
async function rawRequest(port: number, text: string): Promise<string> {
  const conn = await Deno.connect({ hostname: "127.0.0.1", port })
  try {
    await conn.write(new TextEncoder().encode(text))
    const bytes = new Uint8Array(1024)
    const count = await conn.read(bytes)
    return count == null
      ? ""
      : new TextDecoder().decode(bytes.subarray(0, count))
  } finally {
    conn.close()
  }
}

function h2Frame(
  type: number,
  flags: number,
  stream: number,
  payload: Uint8Array,
): Uint8Array {
  const bytes = new Uint8Array(9 + payload.length)
  bytes[0] = (payload.length >>> 16) & 255
  bytes[1] = (payload.length >>> 8) & 255
  bytes[2] = payload.length & 255
  bytes[3] = type
  bytes[4] = flags
  bytes[5] = (stream >>> 24) & 127
  bytes[6] = (stream >>> 16) & 255
  bytes[7] = (stream >>> 8) & 255
  bytes[8] = stream & 255
  bytes.set(payload, 9)
  return bytes
}
function h2Headers(
  authority: string,
  host?: string,
  stream = 1,
  authorization?: string,
): Uint8Array {
  const authorityBytes = new TextEncoder().encode(authority)
  const pathBytes = new TextEncoder().encode("/private?token=fake")
  const hostBytes = host == null ? [] : [
    0x0f,
    0x17,
    host.length,
    ...new TextEncoder().encode(host),
  ]
  const authBytes = authorization == null ? [] : [
    0x0f,
    0x08,
    authorization.length,
    ...new TextEncoder().encode(authorization),
  ]
  return h2Frame(
    1,
    5,
    stream,
    new Uint8Array([
      0x82,
      0x87,
      0x04,
      pathBytes.length,
      ...pathBytes,
      0x01,
      authorityBytes.length,
      ...authorityBytes,
      ...hostBytes,
      ...authBytes,
    ]),
  )
}
async function rawTls(
  proxyPort: number,
  protocol: "h2" | "http/1.1",
): Promise<Deno.TlsConn> {
  const raw = await Deno.connect({ hostname: "127.0.0.1", port: proxyPort })
  await raw.write(new TextEncoder().encode(
    "CONNECT uploads.linear.app:443 HTTP/1.1\r\nHost: uploads.linear.app:443\r\n\r\n",
  ))
  const reply = new Uint8Array(1024)
  const count = await raw.read(reply)
  if (
    count == null ||
    !new TextDecoder().decode(reply.subarray(0, count)).startsWith(
      "HTTP/1.1 200",
    )
  ) throw new Error("CONNECT rejected")
  return Deno.startTls(raw, {
    hostname: "uploads.linear.app",
    caCerts: [ca],
    alpnProtocols: [protocol],
  })
}
async function waitIssue(
  server: ReturnType<typeof startGraphQLServer>,
): Promise<void> {
  const started = performance.now()
  while (server.issues.length === 0 && performance.now() - started < 900) {
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
  assert(server.issues.length > 0)
}

Deno.test("h2 fixed-host GET uses CONNECT mapping and C1's exact matcher", async () => {
  await withProxy(async (server, proxy) => {
    const http = client(proxy.port)
    try {
      const response = await fetch(
        "https://uploads.linear.app/private?token=fake",
        { client: http, headers: { Authorization: "lin_api_fake_probe" } },
      )
      assertEquals(response.status, 200)
      assertEquals(await response.text(), "bytes")
      assertEquals(server.consumed, 1)
      assertEquals(compareGraphQLFixture(fixture(), server), [])
    } finally {
      http.close()
    }
  })
})

Deno.test("fixed-host asset cannot be requested over the GraphQL loopback listener", async () => {
  await withProxy(async (server) => {
    const response = await fetch(
      `http://127.0.0.1:${server.port}/private?token=fake`,
      { headers: { Authorization: "lin_api_fake_probe" } },
    )
    assertEquals(response.status, 500)
    await response.arrayBuffer()
    assertEquals(server.consumed, 0)
    assert(server.issues.length > 0)
  })
})

Deno.test("CONNECT rejects third host and absolute-form non-CONNECT without forwarding", async () => {
  await withProxy(async (server, proxy) => {
    const denied = await rawRequest(
      proxy.port,
      "CONNECT other.invalid:443 HTTP/1.1\r\nHost: other.invalid:443\r\n\r\n",
    )
    assertStringIncludes(denied, "403 Forbidden")
    assertEquals(server.consumed, 0)
    assertStringIncludes(server.issues[0], "undeclared")
  })
  await withProxy(async (server, proxy) => {
    const denied = await rawRequest(
      proxy.port,
      "GET http://uploads.linear.app/private?token=fake HTTP/1.1\r\nHost: uploads.linear.app\r\n\r\n",
    )
    assertStringIncludes(denied, "400 Bad Request")
    assertEquals(server.consumed, 0)
    assertStringIncludes(server.issues[0], "malformed")
  })
})

Deno.test("CONNECT rejects contradictory Host and oversized headers", async () => {
  await withProxy(async (server, proxy) => {
    const denied = await rawRequest(
      proxy.port,
      "CONNECT uploads.linear.app:443 HTTP/1.1\r\nHost: public.linear.app:443\r\n\r\n",
    )
    assertStringIncludes(denied, "400 Bad Request")
    assertEquals(server.consumed, 0)
    assertStringIncludes(server.issues[0], "malformed")
  })
  await withProxy(async (server, proxy) => {
    const denied = await rawRequest(
      proxy.port,
      `CONNECT uploads.linear.app:443 HTTP/1.1\r\nHost: uploads.linear.app:443\r\nX-Pad: ${
        "a".repeat(8192)
      }\r\n\r\n`,
    )
    assertStringIncludes(denied, "400 Bad Request")
    assertEquals(server.consumed, 0)
    assertStringIncludes(server.issues[0], "malformed")
  })
})

Deno.test("wrong authorization, wrong path, and extra same-tunnel GET fail on fixture", async () => {
  await withProxy(async (server, proxy) => {
    const http = client(proxy.port)
    try {
      const response = await fetch(
        "https://uploads.linear.app/private?token=fake",
        { client: http },
      )
      assertEquals(response.status, 500)
      await response.arrayBuffer()
      assertEquals(server.consumed, 0)
      assert(server.issues.length > 0)
    } finally {
      http.close()
    }
  })
  await withProxy(async (server, proxy) => {
    const http = client(proxy.port)
    try {
      const good = await fetch(
        "https://uploads.linear.app/private?token=fake",
        { client: http, headers: { Authorization: "lin_api_fake_probe" } },
      )
      assertEquals(good.status, 200)
      await good.arrayBuffer()
      const extra = await fetch(
        "https://uploads.linear.app/extra?token=fake",
        { client: http, headers: { Authorization: "lin_api_fake_probe" } },
      )
      assertEquals(extra.status, 500)
      await extra.arrayBuffer()
      assertEquals(server.consumed, 1)
      assertEquals(server.unexpected, 1)
    } finally {
      http.close()
    }
  })
})

Deno.test("direct TLS without CONNECT mapping and missing CA fail closed", async () => {
  await withProxy(async (server, proxy) => {
    const raw = await Deno.connect({
      hostname: "127.0.0.1",
      port: proxy.tlsPort,
    })
    const tls = await Deno.startTls(raw, {
      hostname: "uploads.linear.app",
      caCerts: [ca],
      alpnProtocols: ["http/1.1"],
    })
    try {
      await tls.write(new TextEncoder().encode(
        "GET /private?token=fake HTTP/1.1\r\nHost: uploads.linear.app\r\nAuthorization: lin_api_fake_probe\r\n\r\n",
      ))
      const bytes = new Uint8Array(1024)
      await tls.read(bytes)
      assertStringIncludes(server.issues[0], "lacks CONNECT mapping")
      assertEquals(server.consumed, 0)
    } finally {
      tls.close()
    }
  })
  await withProxy(async (server, proxy) => {
    const http = client(proxy.port, false)
    try {
      await assertRejects(() =>
        fetch(
          "https://uploads.linear.app/private?token=fake",
          { client: http, headers: { Authorization: "lin_api_fake_probe" } },
        )
      )
      assertEquals(server.consumed, 0)
    } finally {
      http.close()
    }
  })
  await withProxy(async (server, proxy) => {
    const raw = await Deno.connect({
      hostname: "127.0.0.1",
      port: proxy.tlsPort,
    })
    try {
      const tls = await Deno.startTls(raw, {
        hostname: "wrong.invalid",
        caCerts: [ca],
      })
      try {
        await assertRejects(() => tls.handshake())
      } finally {
        tls.close()
      }
    } finally {
      try {
        raw.close()
      } catch { /* TLS failure may have closed it */ }
    }
    assertEquals(server.consumed, 0)
  })
})

Deno.test("an unexpected second h2 stream on the same tunnel is a fixture issue", async () => {
  await withProxy(async (server, proxy) => {
    const tls = await rawTls(proxy.port, "h2")
    try {
      const preface = new TextEncoder().encode(
        "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n",
      )
      await tls.write(
        new Uint8Array([
          ...preface,
          ...h2Frame(4, 0, 0, new Uint8Array()),
          ...h2Headers(
            "uploads.linear.app",
            undefined,
            1,
            "lin_api_fake_probe",
          ),
        ]),
      )
      const started = performance.now()
      while (server.consumed === 0 && performance.now() - started < 900) {
        await new Promise((resolve) => setTimeout(resolve, 10))
      }
      assertEquals(server.consumed, 1)
      const responseBytes = new Uint8Array(4096)
      await tls.read(responseBytes)
      await tls.write(h2Frame(4, 1, 0, new Uint8Array()))
      await tls.write(h2Headers(
        "uploads.linear.app",
        undefined,
        3,
        "lin_api_fake_probe",
      ))
      await waitIssue(server)
      assert(server.requests.length >= 2)
    } finally {
      tls.close()
    }
  })
})

Deno.test("h2 wrong authority and contradictory Host, plus h1 missing Host, fail on fixture", async () => {
  const scenarios: Array<{ authority: string; host?: string }> = [
    { authority: "public.linear.app" },
    { authority: "uploads.linear.app", host: "public.linear.app" },
  ]
  for (const { authority, host } of scenarios) {
    await withProxy(async (server, proxy) => {
      const tls = await rawTls(proxy.port, "h2")
      try {
        const preface = new TextEncoder().encode(
          "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n",
        )
        await tls.write(
          new Uint8Array([
            ...preface,
            ...h2Frame(4, 0, 0, new Uint8Array()),
            ...h2Headers(authority, host),
          ]),
        )
        await waitIssue(server)
        assert(
          server.issues.some((issue) => issue.includes("authority differs")),
        )
        assertEquals(server.consumed, 0)
      } finally {
        tls.close()
      }
    })
  }
  await withProxy(async (server, proxy) => {
    const tls = await rawTls(proxy.port, "http/1.1")
    try {
      await tls.write(new TextEncoder().encode(
        "GET /private?token=fake HTTP/1.1\r\nConnection: close\r\n\r\n",
      ))
      await waitIssue(server)
      assert(
        server.issues.some((issue) => issue.includes("authority differs")),
      )
      assertEquals(server.consumed, 0)
    } finally {
      tls.close()
    }
  })
})

Deno.test("h1 Host accepts only the CONNECT host with optional default :443", async () => {
  for (const host of ["uploads.linear.app", "uploads.linear.app:443"]) {
    await withProxy(async (server, proxy) => {
      const tls = await rawTls(proxy.port, "http/1.1")
      try {
        await tls.write(new TextEncoder().encode(
          `GET /private?token=fake HTTP/1.1\r\nHost: ${host}\r\nAuthorization: lin_api_fake_probe\r\nConnection: close\r\n\r\n`,
        ))
        const bytes = new Uint8Array(1024)
        const count = await tls.read(bytes)
        assert(count != null)
        assertStringIncludes(
          new TextDecoder().decode(bytes.subarray(0, count)),
          "200 OK",
        )
        assertEquals(server.consumed, 1)
        assertEquals(server.issues, [])
      } finally {
        tls.close()
      }
    })
  }
  for (
    const host of ["uploads.linear.app:444", "public.linear.app:443"]
  ) {
    await withProxy(async (server, proxy) => {
      const tls = await rawTls(proxy.port, "http/1.1")
      try {
        await tls.write(new TextEncoder().encode(
          `GET /private?token=fake HTTP/1.1\r\nHost: ${host}\r\nAuthorization: lin_api_fake_probe\r\nConnection: close\r\n\r\n`,
        ))
        await waitIssue(server)
        assert(
          server.issues.some((issue) => issue.includes("authority differs")),
        )
        assertEquals(server.consumed, 0)
      } finally {
        tls.close()
      }
    })
  }
})

Deno.test("stopping an active fixed-host tunnel releases a parked lane before one second", async () => {
  const asset = (
    id: string,
    host: "uploads.linear.app" | "public.linear.app",
  ) => ({
    kind: "asset",
    id,
    method: "GET",
    fixedHost: host,
    path: "/held",
    requiredHeaders: {},
    forbiddenHeaders: [],
    body: { utf8: "" },
    response: { status: 200, headers: {}, body: { utf8: "done" } },
  })
  const spec = v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: 2,
    initialRecords: {},
    expectedRecords: {},
    groups: [{
      mode: "lanes",
      timeoutMs: 5000,
      lanes: [
        { id: "private", steps: [asset("private", "uploads.linear.app")] },
        { id: "public", steps: [asset("public", "public.linear.app")] },
      ],
    }],
  })
  const server = startGraphQLServer(() => spec, schema)
  const proxy = await startFixedHostProxy(
    server,
    new Set(["uploads.linear.app", "public.linear.app"]),
  )
  const http = client(proxy.port)
  try {
    const pending = fetch("https://uploads.linear.app/held", { client: http })
      .then((response) => response.arrayBuffer()).catch(() => null)
    const started = performance.now()
    while (server.requests.length === 0 && performance.now() - started < 500) {
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
    assertEquals(server.requests.length, 1)
    const stopAt = performance.now()
    await proxy.stop()
    assert(performance.now() - stopAt < 1000)
    await pending
    assertEquals(server.consumed, 0)
  } finally {
    http.close()
    await proxy.stop()
    await server.stop()
  }
})
