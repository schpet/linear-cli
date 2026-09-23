// Network confinement canary. Runs as a child of the runner inside the lane
// and prints one JSON line; the runner decides pass/fail (preflight.ts).
const [portArg] = Deno.args
const port = Number(portArg)
const out: Record<string, unknown> = {}
out.interfaces = Deno.networkInterfaces().map((iface) =>
  `${iface.name}:${iface.address}`
)
try {
  const conn = await Deno.connect({ hostname: "127.0.0.1", port })
  await conn.write(new TextEncoder().encode("ping"))
  const buffer = new Uint8Array(4)
  const read = await conn.read(buffer)
  conn.close()
  out.loopback = read == null
    ? "no reply"
    : new TextDecoder().decode(buffer.subarray(0, read))
} catch (error) {
  out.loopback = error instanceof Error
    ? `${error.name}: ${error.message}`
    : String(error)
}
let started = performance.now()
try {
  const conn = await Deno.connect({ hostname: "1.1.1.1", port: 443 })
  conn.close()
  out.outbound = "CONNECTED"
} catch (error) {
  out.outbound = error instanceof Error
    ? `${error.name}: ${error.message}`
    : String(error)
}
out.outboundMs = Math.round(performance.now() - started)
started = performance.now()
try {
  out.dns = `RESOLVED ${
    JSON.stringify(await Deno.resolveDns("api.linear.app", "A"))
  }`
} catch (error) {
  out.dns = error instanceof Error
    ? `${error.name}: ${error.message}`
    : String(error)
}
out.dnsMs = Math.round(performance.now() - started)
started = performance.now()
try {
  const response = await fetch("https://api.linear.app/graphql", {
    method: "POST",
    body: "{}",
  })
  out.fetch = `RESPONDED ${response.status}`
} catch (error) {
  out.fetch = error instanceof Error
    ? `${error.name}: ${error.message}`
    : String(error)
}
out.fetchMs = Math.round(performance.now() - started)
out.pid = Deno.pid
out.uid = Deno.uid()
console.log(JSON.stringify(out))
