import type { GraphQLServer } from "./graphql-server.ts"
import type { AssetStepSpec } from "./schema.ts"

export type FixedHost = NonNullable<AssetStepSpec["fixedHost"]>

const MAX_HEADER_BYTES = 8192
const MAX_TUNNELS = 16
const HEADER_TIMEOUT_MS = 1000
const SHUTDOWN_TIMEOUT_MS = 900
const encoder = new TextEncoder()

function response(status: 200 | 403 | 400): Uint8Array {
  const message = status === 200
    ? "Connection Established"
    : status === 403
    ? "Forbidden"
    : "Bad Request"
  return encoder.encode(
    `HTTP/1.1 ${status} ${message}\r\nContent-Length: 0\r\n\r\n`,
  )
}

async function readConnect(conn: Deno.Conn): Promise<string> {
  const chunks: Uint8Array[] = []
  let total = 0
  const timer = setTimeout(() => conn.close(), HEADER_TIMEOUT_MS)
  try {
    for (;;) {
      const buffer = new Uint8Array(1024)
      const count = await conn.read(buffer)
      if (count == null) throw new Error("CONNECT header ended early")
      total += count
      if (total > MAX_HEADER_BYTES) throw new Error("CONNECT header too large")
      chunks.push(buffer.subarray(0, count))
      const bytes = new Uint8Array(total)
      let offset = 0
      for (const chunk of chunks) {
        bytes.set(chunk, offset)
        offset += chunk.length
      }
      const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
      const end = text.indexOf("\r\n\r\n")
      if (end < 0) continue
      if (end + 4 !== text.length || /[^\x20-\x7e\r\n]/.test(text)) {
        throw new Error("malformed CONNECT headers")
      }
      const lines = text.slice(0, end).split("\r\n")
      const first = lines.shift()
      if (
        first == null || !/^CONNECT [A-Za-z0-9.]+:443 HTTP\/1\.1$/.test(first)
      ) {
        throw new Error("expected CONNECT target on port 443")
      }
      const target = first.split(" ")[1]
      const hosts = lines.filter((line) => /^host:/i.test(line))
      if (hosts.length !== 1 || hosts[0].slice(5).trim() !== target) {
        throw new Error("CONNECT Host differs from target")
      }
      if (lines.some((line) => !/^[A-Za-z0-9-]+: [^\r\n]*$/.test(line))) {
        throw new Error("malformed CONNECT header")
      }
      return target.slice(0, -4)
    }
  } finally {
    clearTimeout(timer)
  }
}

function isFixedHost(host: string): host is FixedHost {
  return host === "uploads.linear.app" || host === "public.linear.app"
}

async function bounded(task: Promise<unknown>, label: string): Promise<void> {
  let timer: number | null = null
  try {
    await Promise.race([
      task,
      new Promise<never>((_resolve, reject) => {
        timer = setTimeout(
          () => reject(new Error(`${label} exceeded 900 ms`)),
          SHUTDOWN_TIMEOUT_MS,
        )
      }),
    ])
  } finally {
    if (timer != null) clearTimeout(timer)
  }
}

export interface FixedHostProxy {
  port: number
  /** Internal loopback TLS port, exposed for missing-correlation controls. */
  tlsPort: number
  stop(): Promise<void>
}

/** No upstream connection exists: CONNECT is relayed only to an internal TLS listener. */
export async function startFixedHostProxy(
  fixture: GraphQLServer,
  declared: ReadonlySet<FixedHost>,
): Promise<FixedHostProxy> {
  if (declared.size === 0) {
    throw new Error("fixed-host proxy needs a declared host")
  }
  const cert = await Deno.readTextFile(
    new URL("./certs/leaf.pem", import.meta.url),
  )
  const key = await Deno.readTextFile(
    new URL("./certs/leaf.key", import.meta.url),
  )
  const byPort = new Map<number, FixedHost>()
  const tls = Deno.serve({
    hostname: "127.0.0.1",
    port: 0,
    cert,
    key,
    onListen() {},
  }, (request, info) => {
    if (info.remoteAddr.transport !== "tcp") {
      fixture.failFixture("fixed-host TLS connection is not TCP")
      return new Response("fixture mismatch", { status: 500 })
    }
    const host = byPort.get(info.remoteAddr.port)
    if (host == null) {
      fixture.failFixture("fixed-host TLS connection lacks CONNECT mapping")
      return new Response("fixture mismatch", { status: 500 })
    }
    return fixture.handleFixedHost(request, host)
  })
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 })
  const active = new Set<Deno.Conn>()
  const tasks = new Set<Promise<void>>()
  let closed = false

  async function handle(client: Deno.Conn): Promise<void> {
    let upstream: Deno.TcpConn | null = null
    let mappedPort: number | null = null
    active.add(client)
    try {
      let requested: string
      try {
        requested = await readConnect(client)
      } catch {
        fixture.failFixture(
          "fixed-host CONNECT request is malformed or timed out",
        )
        await client.write(response(400)).catch(() => {})
        return
      }
      if (!isFixedHost(requested) || !declared.has(requested)) {
        fixture.failFixture("fixed-host CONNECT target is undeclared")
        await client.write(response(403)).catch(() => {})
        return
      }
      if (closed) return
      upstream = await Deno.connect({
        hostname: "127.0.0.1",
        port: tls.addr.port,
      })
      active.add(upstream)
      const candidatePort = upstream.localAddr.port
      if (byPort.has(candidatePort)) {
        throw new Error("duplicate fixed-host relay mapping")
      }
      byPort.set(candidatePort, requested)
      mappedPort = candidatePort
      await client.write(response(200))
      await Promise.race([
        client.readable.pipeTo(upstream.writable).catch(() => {}),
        upstream.readable.pipeTo(client.writable).catch(() => {}),
      ])
    } catch {
      fixture.failFixture("fixed-host relay failed")
    } finally {
      if (mappedPort != null) byPort.delete(mappedPort)
      active.delete(client)
      try {
        client.close()
      } catch { /* already closed */ }
      if (upstream != null) {
        active.delete(upstream)
        try {
          upstream.close()
        } catch { /* already closed */ }
      }
    }
  }
  const accepting = (async () => {
    try {
      for await (const client of listener) {
        if (closed) {
          client.close()
          continue
        }
        if (tasks.size >= MAX_TUNNELS) {
          fixture.failFixture("fixed-host tunnel limit exceeded")
          await client.write(response(403)).catch(() => {})
          client.close()
          continue
        }
        const task = handle(client)
        tasks.add(task)
        task.finally(() => tasks.delete(task))
      }
    } catch (error) {
      if (!closed) throw error
    }
  })()
  return {
    port: listener.addr.port,
    tlsPort: tls.addr.port,
    async stop() {
      if (closed) return
      closed = true
      listener.close()
      for (const conn of active) {
        try {
          conn.close()
        } catch { /* already closed */ }
      }
      await bounded(
        Promise.allSettled([accepting, ...tasks]),
        "fixed-host relay shutdown",
      )
      await bounded(tls.shutdown(), "fixed-host TLS shutdown")
    },
  }
}
