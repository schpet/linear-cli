import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
} from "@std/assert"
import { fromFileUrl, join } from "@std/path"
import { writeAllSync } from "./serve-case.ts"

const runnerDir = fromFileUrl(new URL(".", import.meta.url))
const parityDir = fromFileUrl(new URL("..", import.meta.url))
const root = fromFileUrl(new URL("../../..", import.meta.url))
const transportCases = join(runnerDir, "transport-cases")
const identity = {
  "content-type": "application/json",
  authorization: "lin_api_fake",
  "user-agent": "schpet-linear-cli/2.6.0",
}

interface Ready {
  event: "ready"
  port: number
  path: string
  expectedRequests: number
  expectedGraphQL: number
}
interface Final {
  event: "final"
  requests: Array<
    { kind: string; authorizationMatched: boolean; userAgent: string | null }
  >
  consumed: number
  expected: number
  unexpected: number
  issues: string[]
  mismatches: Array<{ surface: string; detail: string }>
}

class Driver {
  #child: Deno.ChildProcess
  #stdout: ReadableStreamDefaultReader<Uint8Array>
  #stdin: WritableStreamDefaultWriter<Uint8Array>
  #buffer = ""
  #stderr: Promise<string>

  constructor(args: string[]) {
    this.#child = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--frozen",
        "--cached-only",
        "--no-prompt",
        "--config",
        join(parityDir, "deno.json"),
        `--allow-read=${parityDir},${join(root, "graphql/schema.graphql")}`,
        "--allow-net=127.0.0.1",
        "--allow-env=NODE_ENV",
        join(runnerDir, "serve-case.ts"),
        ...args,
      ],
      stdin: "piped",
      stdout: "piped",
      stderr: "piped",
    }).spawn()
    this.#stdout = this.#child.stdout.getReader()
    this.#stdin = this.#child.stdin.getWriter()
    this.#stderr = new Response(this.#child.stderr).text()
  }

  /** Next stdout line, or null when stdout closed first. */
  async line(): Promise<string | null> {
    for (;;) {
      const newline = this.#buffer.indexOf("\n")
      if (newline >= 0) {
        const line = this.#buffer.slice(0, newline)
        this.#buffer = this.#buffer.slice(newline + 1)
        return line
      }
      const next = await this.#stdout.read()
      if (next.done) return null
      this.#buffer += new TextDecoder().decode(next.value)
    }
  }

  async ready(): Promise<Ready> {
    const line = await this.line()
    // Only await stderr once stdout has closed; the child's stderr stays open
    // until it exits, and it exits only after stdin EOF.
    if (line == null) {
      throw new Error(`no ready line; stderr: ${await this.#stderr}`)
    }
    const ready: unknown = JSON.parse(line)
    assertEquals((ready as Ready).event, "ready")
    return ready as Ready
  }

  async write(bytes: Uint8Array): Promise<void> {
    await this.#stdin.write(bytes)
  }

  async finish(): Promise<
    { final: Final | null; code: number; stderr: string; extra: string }
  > {
    await this.#stdin.close()
    const line = await this.line()
    const rest: string[] = []
    for (let next = await this.line(); next != null; next = await this.line()) {
      rest.push(next)
    }
    const status = await this.#child.status
    return {
      final: line == null ? null : JSON.parse(line) as Final,
      code: status.code,
      stderr: await this.#stderr,
      extra: rest.join("\n") + this.#buffer,
    }
  }
}

async function rejected(
  args: string[],
): Promise<{ code: number; stderr: string; stdout: string }> {
  const driver = new Driver(args)
  const first = await driver.line()
  const result = await driver.finish()
  return {
    code: result.code,
    stderr: result.stderr,
    stdout: [first, result.extra].filter((part) => part).join("\n"),
  }
}

Deno.test("serve-case announces the port, serves the case and reports a clean final line", async () => {
  const driver = new Driver([join(transportCases, "f02b-raw-viewer.json")])
  const ready = await driver.ready()
  assertEquals(ready.path, "/graphql")
  assertEquals(ready.expectedRequests, 1)
  assertEquals(ready.expectedGraphQL, 1)
  const response = await fetch(`http://127.0.0.1:${ready.port}${ready.path}`, {
    method: "POST",
    headers: identity,
    body: JSON.stringify({ query: "{ viewer { id } }" }),
  })
  assertEquals(response.status, 200)
  assertEquals(await response.text(), '{"data":{"viewer":{"id":"user-1"}}}')
  const result = await driver.finish()
  assertEquals(result.code, 0, result.stderr)
  assertEquals(result.extra, "", "exactly one final line")
  assertEquals(result.final, {
    event: "final",
    requests: [{
      kind: "graphql",
      authorizationMatched: true,
      userAgent: "schpet-linear-cli/2.6.0",
    }],
    consumed: 1,
    expected: 1,
    unexpected: 0,
    issues: [],
    mismatches: [],
  })
})

Deno.test("serve-case exits 1 with the mismatch report when steps stay unconsumed", async () => {
  const driver = new Driver([join(transportCases, "f02b-raw-viewer.json")])
  await driver.ready()
  const result = await driver.finish()
  assertEquals(result.code, 1)
  assert(result.final != null)
  assertEquals(result.final.consumed, 0)
  assertEquals(result.final.mismatches.length, 1)
  assertStringIncludes(
    result.final.mismatches[0].detail,
    "expected 1 interactions",
  )
})

Deno.test("serve-case treats stdin bytes before EOF as a protocol error", async () => {
  const driver = new Driver([join(transportCases, "f02b-raw-viewer.json")])
  await driver.ready()
  await driver.write(new TextEncoder().encode("x"))
  const result = await driver.finish()
  assertEquals(result.code, 2)
  assertEquals(result.final, null)
  assertStringIncludes(result.stderr, "unexpected stdin bytes")
})

Deno.test("serve-case rejects bad arguments before any stdout line", async () => {
  const good = join(transportCases, "f02b-raw-viewer.json")
  const checks: Array<[string[], string]> = [
    [[], "expected exactly one argument"],
    [[good, good], "expected exactly one argument"],
    [[""], "case path is empty"],
    [
      [`${transportCases}/../transport-cases/f02b-raw-viewer.json`],
      "must not contain .. segments",
    ],
    [
      [join(runnerDir, "cases", "api-loopback-viewer-200.json")],
      "directly under",
    ],
    [
      [join(transportCases, "nested", "f02b-raw-viewer.json")],
      "directly under",
    ],
    [[join(transportCases, "F02B-Upper.json")], "kebab-case"],
    [[join(transportCases, "f02b-raw-viewer.txt")], "kebab-case"],
    [[join(transportCases, "f02b-does-not-exist.json")], "does not exist"],
    [
      [join(transportCases, "f02b-control-no-graphql-fixture.json")],
      "has no GraphQL fixture",
    ],
  ]
  for (const [args, fragment] of checks) {
    const result = await rejected(args)
    assertEquals(result.code, 2, JSON.stringify(args))
    assertEquals(result.stdout, "", JSON.stringify(args))
    assertStringIncludes(result.stderr, fragment, JSON.stringify(args))
  }
})

// The driver only accepts a file directly under the tracked transport-cases
// directory, so a symlink control has to live there. The name is ignored by
// the root .gitignore (so jj/git never snapshot it if a run dies between
// creation and cleanup), a stale link from an earlier crash is removed before
// the test starts, and cleanup runs both in `finally` and on process unload.
const SYMLINK_CONTROL = join(transportCases, "zz-tmp-symlink-control.json")

function removeSymlinkControl(): void {
  let info: Deno.FileInfo
  try {
    info = Deno.lstatSync(SYMLINK_CONTROL)
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) return
    throw error
  }
  if (!info.isSymlink) {
    throw new Error(
      `${SYMLINK_CONTROL} exists but is not a symlink; refusing to remove it`,
    )
  }
  Deno.removeSync(SYMLINK_CONTROL)
}

Deno.test("serve-case rejects a symlinked case file", async () => {
  removeSymlinkControl()
  globalThis.addEventListener("unload", removeSymlinkControl)
  try {
    await Deno.symlink("f02b-raw-viewer.json", SYMLINK_CONTROL)
    const result = await rejected([SYMLINK_CONTROL])
    assertEquals(result.code, 2)
    assertStringIncludes(result.stderr, "must be a regular file")
  } finally {
    removeSymlinkControl()
    globalThis.removeEventListener("unload", removeSymlinkControl)
  }
  assertEquals(await Deno.lstat(SYMLINK_CONTROL).catch(() => null), null)
})

class ShortWriter {
  calls: Uint8Array[] = []
  #chunk: number
  constructor(chunk: number) {
    this.#chunk = chunk
  }
  writeSync(bytes: Uint8Array): number {
    const taken = Math.min(this.#chunk, bytes.length)
    this.calls.push(bytes.slice(0, taken))
    return taken
  }
}

Deno.test("writeAllSync completes a line across short writes", () => {
  const line = new TextEncoder().encode('{"event":"ready","port":12345}\n')
  for (const chunk of [1, 7, line.length, line.length + 100]) {
    const writer = new ShortWriter(chunk)
    writeAllSync(writer, line)
    const joined = new Uint8Array(line.length)
    let offset = 0
    for (const call of writer.calls) {
      joined.set(call, offset)
      offset += call.length
    }
    assertEquals(offset, line.length, `chunk ${chunk}`)
    assertEquals(joined, line, `chunk ${chunk}`)
    assertEquals(writer.calls.length, Math.ceil(line.length / chunk))
  }
  const empty = new ShortWriter(1)
  writeAllSync(empty, new Uint8Array())
  assertEquals(empty.calls, [])
})

Deno.test("writeAllSync refuses writes that make no progress or overreport", () => {
  const bytes = new TextEncoder().encode("abc\n")
  const stalled = { writeSync: () => 0 }
  assertThrows(() => writeAllSync(stalled, bytes), Error, "returned 0 for 4")
  const overreporting = { writeSync: () => 5 }
  assertThrows(
    () => writeAllSync(overreporting, bytes),
    Error,
    "returned 5 for 4",
  )
  const fractional = { writeSync: () => 1.5 }
  assertThrows(() => writeAllSync(fractional, bytes), Error, "returned 1.5")
})
