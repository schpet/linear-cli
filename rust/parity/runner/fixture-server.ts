// Loopback fixture server for runner smoke cases. It serves scripted
// responses in order and records every request. Schema-validated GraphQL
// execution and semantic matching are P03 work and are deliberately absent.
import { decodeByteValue } from "./bytes.ts"
import type { FixtureServerSpec } from "./schema.ts"

export interface FixtureRequest {
  method: string
  path: string
  authorization: string | null
  userAgent: string | null
  contentType: string | null
  body: Uint8Array
}

export interface FixtureServer {
  port: number
  requests: FixtureRequest[]
  /** Requests beyond the scripted responses; they receive a 500. */
  unexpected: number
  stop(): Promise<void>
}

/**
 * The port is only known once the listener exists, and scripted bodies may
 * embed it through {{fixturePort}}, so the spec is resolved lazily per port.
 */
export function startFixtureServer(
  specFor: (port: number) => FixtureServerSpec,
): FixtureServer {
  const requests: FixtureRequest[] = []
  const state = { unexpected: 0 }
  let resolved: { spec: FixtureServerSpec; bodies: Uint8Array[] } | undefined
  const server = Deno.serve({
    hostname: "127.0.0.1",
    port: 0,
    onListen() {},
    async handler(request) {
      resolved ??= (() => {
        const spec = specFor(server.addr.port)
        return {
          spec,
          bodies: spec.responses.map((response) =>
            decodeByteValue(response.body)
          ),
        }
      })()
      const url = new URL(request.url)
      const index = requests.length
      requests.push({
        method: request.method,
        path: url.pathname,
        authorization: request.headers.get("authorization"),
        userAgent: request.headers.get("user-agent"),
        contentType: request.headers.get("content-type"),
        body: new Uint8Array(await request.arrayBuffer()),
      })
      const scripted = resolved.spec.responses[index]
      if (scripted == null) {
        state.unexpected++
        return new Response("unexpected request: no scripted response", {
          status: 500,
        })
      }
      return new Response(new Blob([new Uint8Array(resolved.bodies[index])]), {
        status: scripted.status,
        headers: scripted.headers,
      })
    },
  })
  return {
    port: server.addr.port,
    requests,
    get unexpected() {
      return state.unexpected
    },
    async stop() {
      let timer: number | undefined
      const timeout = new Promise<void>((resolve) => {
        timer = setTimeout(resolve, 5000)
      })
      try {
        await Promise.race([server.shutdown(), timeout])
      } finally {
        clearTimeout(timer)
      }
    },
  }
}
