import { assert, assertEquals } from "@std/assert"
import { dirname, join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { FROZEN_USER_AGENT, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"

const root = new URL("./cases/", import.meta.url).pathname
const goldenDir = `rust-goldens/${RUST_CONTRACT}`
const UA = "R01H-GRAPHQL-UA"
const ROW = "C002-ROW-ERROR-TEXT"
const HELP = "C002-HELP-VERSION"
const POLICY = "C002-TRANSPORT-POLICY"
const ENDPOINT = "C002-STRICT-ENDPOINT"
const ua = ["graphql-user-agent"]
const rowUa = ["stdout", "graphql-user-agent"]
const fatal = ["exit", "stdout", "stderr"]

/** Case id -> [deviation, approved surfaces], or null for an exact case. */
const bindings = new Map<string, [string, string[]] | null>([
  ["c002-ca-build-failure", [POLICY, fatal]],
  ["c002-empty", null],
  ["c002-empty-stdout-closed", null],
  ["c002-env-key-unknown-workspace", [UA, ua]],
  ["c002-extra-positional", [HELP, ["stdout"]]],
  ["c002-graphql-error-width", [ROW, rowUa]],
  ["c002-help", [HELP, ["stdout"]]],
  ["c002-http-500", [ROW, rowUa]],
  ["c002-invalid-credentials", [UA, ua]],
  ["c002-invalid-endpoint-empty", [ENDPOINT, fatal]],
  ["c002-invalid-endpoint-row", [ENDPOINT, fatal]],
  ["c002-invalid-keys-proxy", [ROW, ["stdout"]]],
  ["c002-json-flag", [HELP, ["stdout"]]],
  ["c002-known-workspace-after-route", [UA, ua]],
  ["c002-malformed-json", [ROW, rowUa]],
  ["c002-metadata-lookup-failed", ["C002-KEYRING-WARNING", ["stderr"]]],
  ["c002-numeric-names", [UA, ua]],
  ["c002-proxy-policy", [POLICY, fatal]],
  ["c002-redirect", [ROW, rowUa]],
  ["c002-refused-endpoint", [ROW, ["stdout"]]],
  ["c002-source-order-default", [UA, ua]],
  ["c002-unicode-width-a", [UA, ua]],
  ["c002-unicode-width-b", [UA, ua]],
  ["c002-unknown-default-unicode-org", [UA, ua]],
  ["c002-viewer-null", [ROW, rowUa]],
  ["c002-width-table", ["C002-WIDTH-TABLE", rowUa]],
])
/** GraphQL cases: expected request count and whether requests run in lanes. */
const requests = new Map<string, [number, "ordered" | "lanes"]>([
  ["c002-env-key-unknown-workspace", [1, "ordered"]],
  ["c002-graphql-error-width", [2, "lanes"]],
  ["c002-http-500", [2, "lanes"]],
  ["c002-invalid-credentials", [2, "lanes"]],
  ["c002-known-workspace-after-route", [1, "ordered"]],
  ["c002-malformed-json", [2, "lanes"]],
  ["c002-numeric-names", [4, "lanes"]],
  ["c002-redirect", [2, "lanes"]],
  ["c002-source-order-default", [3, "lanes"]],
  ["c002-unicode-width-a", [5, "lanes"]],
  ["c002-unicode-width-b", [4, "lanes"]],
  ["c002-unknown-default-unicode-org", [1, "ordered"]],
  ["c002-viewer-null", [2, "lanes"]],
  ["c002-width-table", [1, "ordered"]],
])
const fixtureHashes = new Map<string, string>([
  [
    "fixtures/c002-errors/linear/credentials.toml",
    "70e6f743c42143296a044def073357c721950dc2ea990973aaccdfe60b603b57",
  ],
  [
    "fixtures/c002-invalid-keys/linear/credentials.toml",
    "f4abf2c08e31816b58f0155a6e9feea6d3e4717867909ea4749bb47ae9e6d7e2",
  ],
  [
    "fixtures/c002-metadata/linear/credentials.toml",
    "fd1a3679585f851d49079654248ab899fc031fa8190792310a817a084a53464d",
  ],
  [
    "fixtures/c002-numeric-names/linear/credentials.toml",
    "49a344b915e6b68a38364b25fb6e14d55e858587bde9df48a86cef90e0379aff",
  ],
  [
    "fixtures/c002-solo/linear/credentials.toml",
    "4c59ca03c9ff8f9133efa44860cebb4fe80b7d0b5c02966d933d432d45e337c9",
  ],
  [
    "fixtures/c002-source-order/linear/credentials.toml",
    "e15c6ba0452d162324a976c1824c2126d682253f2250a43801ad8ed8bf520eb4",
  ],
  [
    "fixtures/c002-two/linear/credentials.toml",
    "986e0799c2a1ca46436a74c00c76ef174755f2581c250ae744282535065fd4b5",
  ],
  [
    "fixtures/c002-unicode-a/linear/credentials.toml",
    "ff7348f531e8ac637e8ea6eb9e6d7849f99b28d44183290d1bfc1daeff4e5923",
  ],
  [
    "fixtures/c002-unicode-b/linear/credentials.toml",
    "1bc7000e09a4b4f40c594ed1c74ebdf502b2c69c5fc4e15aa1e6ab4c6c1ecdba",
  ],
  [
    "fixtures/c002-width-table/linear/credentials.toml",
    "cd46375209ab02ff722c9d76d8e1cbf53c735ca91dda62dfdb20377d39ab1e36",
  ],
])
const fileNames = [
  ...[...bindings.keys()].map((id) => `${id}.json`),
  ...fixtureHashes.keys(),
].sort()
const frozenProjectionSha256 =
  "d94246f82006e506c6283a8a14a59611b4ffa389667df13ab1e6539a2a135cf4"
const goldenBundleSha256 =
  "5b36e09b763acbe9318ba1e9d1187a0685bed99eed033201d19fe36714c5a261"

function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical)
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).sort(([left], [right]) =>
        left < right ? -1 : left > right ? 1 : 0
      )
        .map(([key, entry]) => [key, canonical(entry)]),
    )
  }
  return value
}

/** Sorted `sha256  path` lines: raw fixture bytes, or sorted-key JSON of each
 * case with only its Rust `deviation` binding omitted. */
async function projectionSha256(corpusRoot: string): Promise<string> {
  const lines: string[] = []
  for (const name of fileNames) {
    const raw = await Deno.readFile(join(corpusRoot, name))
    let content = raw
    if (name.endsWith(".json")) {
      const parsed: unknown = JSON.parse(new TextDecoder().decode(raw))
      if (
        parsed == null || typeof parsed !== "object" || Array.isArray(parsed)
      ) {
        throw new Error(`invalid frozen case ${name}`)
      }
      content = new TextEncoder().encode(JSON.stringify(canonical(
        Object.fromEntries(
          Object.entries(parsed).filter(([key]) => key !== "deviation"),
        ),
      )))
    } else {
      assertEquals(await sha256Hex(raw), fixtureHashes.get(name), name)
    }
    lines.push(`${await sha256Hex(content)}  ${name}\n`)
  }
  return await sha256Hex(new TextEncoder().encode(lines.join("")))
}

async function c002Entries(
  corpusRoot: string,
): Promise<{ fixtures: string[]; goldens: string[]; cases: string[] }> {
  const fixtures: string[] = []
  async function walk(relativeDir: string): Promise<void> {
    for await (const entry of Deno.readDir(join(corpusRoot, relativeDir))) {
      const relativeName = `${relativeDir}/${entry.name}`
      if (entry.isDirectory) {
        await walk(relativeName)
      } else {
        assert(entry.isFile, `unexpected fixture entry ${relativeName}`)
        fixtures.push(relativeName)
      }
    }
  }
  for await (const entry of Deno.readDir(join(corpusRoot, "fixtures"))) {
    if (!entry.name.startsWith("c002-")) continue
    assert(entry.isDirectory, `unexpected fixture root ${entry.name}`)
    await walk(`fixtures/${entry.name}`)
  }
  const goldens: string[] = []
  for await (const entry of Deno.readDir(join(corpusRoot, goldenDir))) {
    if (entry.name.startsWith("c002-")) {
      assert(entry.isFile, `unexpected golden entry ${entry.name}`)
      goldens.push(`${goldenDir}/${entry.name}`)
    }
  }
  const cases: string[] = []
  for await (const entry of Deno.readDir(corpusRoot)) {
    if (entry.name.startsWith("c002-")) {
      assert(entry.isFile, `unexpected case entry ${entry.name}`)
      cases.push(entry.name)
    }
  }
  return {
    fixtures: fixtures.sort(),
    goldens: goldens.sort(),
    cases: cases.sort(),
  }
}

async function goldenSha256(corpusRoot: string, names: string[]) {
  const lines = await Promise.all(
    names.map(async (name) =>
      `${await sha256Hex(
        await Deno.readFile(join(corpusRoot, name)),
      )}  ${name}\n`
    ),
  )
  return await sha256Hex(new TextEncoder().encode(lines.join("")))
}

Deno.test("C002 auth list cases keep frozen Deno bytes and reviewed v3 goldens", async () => {
  assertEquals(await projectionSha256(root), frozenProjectionSha256)
  const entries = await c002Entries(root)
  assertEquals(entries.fixtures, [...fixtureHashes.keys()].sort())
  assertEquals(
    entries.cases,
    [...bindings.keys()].map((id) => `${id}.json`).sort(),
  )
  const expectedGoldens = [...bindings].filter(([, binding]) => binding != null)
    .map(([id]) => `${goldenDir}/${id}.json`).sort()
  assertEquals(entries.goldens, expectedGoldens)
  assertEquals(expectedGoldens.length, 24)
  assertEquals(
    await goldenSha256(root, entries.goldens),
    goldenBundleSha256,
  )

  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const loaded = await loadCases(root, routes, "c002-", RUST_CONTRACT)
  assertEquals(
    loaded.map((item) => item.spec.id).sort(),
    [...bindings.keys()].sort(),
  )
  let graphql = 0
  for (const item of loaded) {
    const id = item.spec.id
    assertEquals(item.spec.route, "linear auth list", id)
    assertEquals(item.spec.env.PATH, "{{bin}}", id)
    assertEquals(item.spec.fixtureServer, null, id)
    const binding = bindings.get(id)
    assert(binding !== undefined, id)
    if (binding == null) {
      assertEquals(item.spec.deviation, null, id)
      assertEquals(item.golden ?? null, null, id)
    } else {
      assertEquals(item.spec.deviation?.id, binding[0], id)
      assertEquals(item.spec.deviation?.contract, RUST_CONTRACT, id)
      assertEquals(item.golden?.spec.approvedSurfaces, binding[1], id)
    }
    const expected = requests.get(id)
    if (item.spec.graphql == null) {
      assertEquals(expected, undefined, id)
      continue
    }
    graphql++
    assert(expected != null, id)
    assertEquals(item.spec.graphql.expectedRequests, expected[0], id)
    assertEquals(item.spec.graphql.groups.length, 1, id)
    const group = item.spec.graphql.groups[0]
    assertEquals(group.mode, expected[1], id)
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    const keys = new Set<string>()
    for (const step of steps) {
      assert(step.kind === "graphql", id)
      assertEquals(step.identity.userAgent, FROZEN_USER_AGENT, id)
      assertEquals(step.operation.variables, undefined, id)
      assert(step.operation.document.startsWith("query AuthListViewer "), id)
      const key = step.identity.authorization
      assert(key != null && key.startsWith("lin_api_fake_"), id)
      keys.add(key)
    }
    // One request per usable stored key, each with its own fake key.
    assertEquals(keys.size, expected[0], id)
    assertEquals(
      item.golden?.spec.candidate.graphqlUserAgent,
      RUST_USER_AGENT,
      id,
    )
  }
  assertEquals(graphql, requests.size)
  const route = manifest.routes.find((entry) =>
    entry.path === "linear auth list"
  )
  assertEquals(route?.workItem, "C002")
  assertEquals(route?.fixtureStatus, "pending")
})

Deno.test("C002 projection and golden bundle catch changed bytes in scratch", async () => {
  const scratch = await Deno.makeTempDir({ prefix: "linear-c002-projection-" })
  try {
    const goldens = (await c002Entries(root)).goldens
    for (const name of [...fileNames, ...goldens]) {
      const target = join(scratch, name)
      await Deno.mkdir(dirname(target), { recursive: true })
      await Deno.copyFile(join(root, name), target)
    }
    assertEquals(await projectionSha256(scratch), frozenProjectionSha256)
    assertEquals(await goldenSha256(scratch, goldens), goldenBundleSha256)

    const casePath = join(scratch, "c002-empty.json")
    const parsed = JSON.parse(await Deno.readTextFile(casePath))
    parsed.expected.stdout.utf8 = "No workspaces\n"
    await Deno.writeTextFile(casePath, JSON.stringify(parsed))
    assert((await projectionSha256(scratch)) !== frozenProjectionSha256)

    const goldenPath = join(scratch, goldenDir, "c002-refused-endpoint.json")
    const golden = await Deno.readTextFile(goldenPath)
    await Deno.writeTextFile(goldenPath, golden.replace("  alpha", "* alpha"))
    assert((await goldenSha256(scratch, goldens)) !== goldenBundleSha256)

    const extra = join(scratch, "fixtures/c002-two/linear/extra.toml")
    await Deno.writeTextFile(extra, "synthetic = true\n")
    assert(
      JSON.stringify((await c002Entries(scratch)).fixtures) !==
        JSON.stringify([...fixtureHashes.keys()].sort()),
    )
  } finally {
    await Deno.remove(scratch, { recursive: true })
  }
})
