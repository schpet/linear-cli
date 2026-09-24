// One-case GraphQL fixture driver for the Rust transport integration test.
//
// Protocol (all lines are single-line JSON on stdout):
//   1. argv is exactly one path to a case file directly under
//      `rust/parity/runner/transport-cases/`; anything else exits 2 before
//      any stdout line.
//   2. The case is loaded through the ordinary case loader, must carry a
//      GraphQL fixture, and is resolved with the server's bound port through
//      the ordinary `fixturePort` substitution.
//   3. Exactly one `{"event":"ready",...}` line carries the bound port.
//   4. Completion is stdin EOF; any stdin byte is a protocol error (exit 2).
//   5. Exactly one `{"event":"final",...}` line carries request summaries,
//      consumed/expected/unexpected counts, fixture issues and the
//      `compareGraphQLFixture` mismatches. Exit 0 only with no mismatch.
// The server is always stopped in `finally`. This is a transport foundation
// driver, not manifest-route parity: the case's argv/expected surfaces are not
// executed here.
import { basename, dirname, fromFileUrl, join, resolve } from "@std/path"
import { readManifest } from "../verify.ts"
import { loadCases, resolveCase } from "./cases.ts"
import { compareGraphQLFixture } from "./compare.ts"
import {
  loadPinnedGraphQLSchema,
  startGraphQLServer,
} from "./graphql-server.ts"
import type { CaseSpec, RuntimeGraphQLFixtureSpec } from "./schema.ts"

const TRANSPORT_CASES = fromFileUrl(
  new URL("./transport-cases", import.meta.url),
)
const PARITY_DIR = fromFileUrl(new URL("..", import.meta.url))
const CASE_FILE = /^[a-z0-9][a-z0-9-]*\.json$/

class DriverError extends Error {}

/**
 * Writes every byte of `bytes` to `writer`, retrying short writes. A write
 * that makes no progress or reports more than was pending is a driver error
 * rather than a silently truncated protocol line.
 */
export function writeAllSync(
  writer: { writeSync(bytes: Uint8Array): number },
  bytes: Uint8Array,
): void {
  let offset = 0
  while (offset < bytes.length) {
    const pending = bytes.length - offset
    const written = writer.writeSync(bytes.subarray(offset))
    if (!Number.isInteger(written) || written <= 0 || written > pending) {
      throw new DriverError(
        `stdout write returned ${written} for ${pending} pending bytes`,
      )
    }
    offset += written
  }
}

function emit(line: Record<string, unknown>): void {
  const text = JSON.stringify(line)
  if (text.includes("\n")) {
    throw new DriverError("protocol line contains a newline")
  }
  writeAllSync(Deno.stdout, new TextEncoder().encode(`${text}\n`))
}

async function selectCasePath(args: readonly string[]): Promise<string> {
  if (args.length !== 1) {
    throw new DriverError(
      `expected exactly one argument (a transport case path), got ${args.length}`,
    )
  }
  const raw = args[0]
  if (raw.length === 0) throw new DriverError("case path is empty")
  if (raw.split("/").some((segment) => segment === "..")) {
    throw new DriverError("case path must not contain .. segments")
  }
  const absolute = resolve(raw)
  if (dirname(absolute) !== TRANSPORT_CASES) {
    throw new DriverError(
      `case path must name a file directly under ${TRANSPORT_CASES}`,
    )
  }
  const name = basename(absolute)
  if (!CASE_FILE.test(name)) {
    throw new DriverError(
      "case file name must be kebab-case with a .json suffix",
    )
  }
  const directory = await Deno.lstat(TRANSPORT_CASES).catch(() => null)
  if (directory == null || !directory.isDirectory || directory.isSymlink) {
    throw new DriverError("transport case directory is missing or a symlink")
  }
  const info = await Deno.lstat(absolute).catch(() => null)
  if (info == null) throw new DriverError(`case file ${name} does not exist`)
  if (info.isSymlink || !info.isFile) {
    throw new DriverError(`case file ${name} must be a regular file`)
  }
  return absolute
}

async function loadTransportCase(absolute: string): Promise<CaseSpec> {
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(PARITY_DIR, "manifest.json"))),
  )
  const routes = new Set<string>()
  for (const route of manifest.routes) {
    if (typeof route.path !== "string") {
      throw new DriverError("manifest route without path")
    }
    routes.add(route.path)
  }
  const id = basename(absolute).slice(0, -".json".length)
  const loaded = await loadCases(TRANSPORT_CASES, routes, id)
  const found = loaded.find((entry) => entry.file === absolute)
  if (found == null) {
    throw new DriverError(`case ${id} was not loaded from ${TRANSPORT_CASES}`)
  }
  if (found.spec.graphql == null) {
    throw new DriverError(
      `case ${id} has no GraphQL fixture; only GraphQL fixture cases are transport cases`,
    )
  }
  return found.spec
}

function resolveWithPort(
  spec: CaseSpec,
  port: number,
): RuntimeGraphQLFixtureSpec {
  const graphql = resolveCase(spec, {
    home: "/nonexistent/home",
    configHome: "/nonexistent/config",
    cwd: "/nonexistent/cwd",
    bin: "/nonexistent/bin",
    denoDir: "/nonexistent/deno-dir",
    fixturePort: String(port),
  }).graphql
  if (graphql == null) {
    throw new DriverError("GraphQL fixture vanished during resolution")
  }
  return graphql
}

async function countStdinBytes(): Promise<number> {
  let total = 0
  for await (const chunk of Deno.stdin.readable) total += chunk.length
  return total
}

async function main(args: readonly string[]): Promise<number> {
  const absolute = await selectCasePath(args)
  const spec = await loadTransportCase(absolute)
  const schema = await loadPinnedGraphQLSchema()
  const server = startGraphQLServer(
    (port) => resolveWithPort(spec, port),
    schema,
  )
  try {
    const resolved = resolveWithPort(spec, server.port)
    // Reject P03C-only shapes (lanes, assets) before announcing readiness.
    const expectedGraphQL = server.expectedGraphQL
    emit({
      event: "ready",
      port: server.port,
      path: resolved.path,
      expectedRequests: resolved.expectedRequests,
      expectedGraphQL,
    })
    const stdinBytes = await countStdinBytes()
    if (stdinBytes > 0) {
      throw new DriverError(
        `protocol error: ${stdinBytes} unexpected stdin bytes before EOF`,
      )
    }
    const mismatches = compareGraphQLFixture(resolved, server)
    emit({
      event: "final",
      requests: server.requests,
      consumed: server.consumed,
      expected: resolved.expectedRequests,
      unexpected: server.unexpected,
      issues: [...server.issues],
      mismatches,
    })
    return mismatches.length === 0 ? 0 : 1
  } finally {
    await server.stop()
  }
}

if (import.meta.main) {
  let code: number
  try {
    code = await main(Deno.args)
  } catch (error) {
    console.error(
      `serve-case: ${error instanceof Error ? error.message : String(error)}`,
    )
    code = 2
  }
  Deno.exit(code)
}
