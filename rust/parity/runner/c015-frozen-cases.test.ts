import { assert, assertEquals, assertNotEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { buildPinnedSchema, matchGraphQL } from "./graphql-match.ts"

const root = new URL("./c015-frozen-cases/", import.meta.url).pathname
const ids = [
  "c015-alias-json",
  "c015-all-two-pages",
  "c015-closed-stdout",
  "c015-collation-json",
  "c015-date-la",
  "c015-date-utc",
  "c015-empty-cursor",
  "c015-empty-json",
  "c015-empty-text",
  "c015-extra-argument",
  "c015-graphql-error",
  "c015-help",
  "c015-inactive-json",
  "c015-inactive-text",
  "c015-json-all",
  "c015-json-filter",
  "c015-missing-cursor",
  "c015-no-key-help",
  "c015-no-key",
  "c015-raw-extra-json",
  "c015-raw-invalid-date",
  "c015-repeated-cursor",
  "c015-rich-text",
  "c015-text-all",
  "c015-text-filter",
  "c015-transport-error",
  "c015-two-pages",
  "c015-unknown-option",
  "c015-workspace-after",
  "c015-workspace-before",
  "c015-workspace-conflict",
  "c015-workspace-missing",
]
const bundleSha256 =
  "125e58dfc2c65789664acc5788a7470b4ece1ba4a21e6a8533bf638b09772c54"
const queryPath = "$.viewer.organization.users.after"

Deno.test("C015 pins the exact frozen user-list corpus and GraphQL request contract", async () => {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    assert(!entry.isSymlink, `unexpected C015 symlink ${entry.name}`)
    if (entry.isDirectory) {
      assertEquals(entry.name, "fixtures")
      continue
    }
    assert(entry.isFile, `unexpected C015 entry ${entry.name}`)
    names.push(entry.name)
  }
  names.sort()
  assertEquals(names, ids.map((id) => `${id}.json`))
  assertEquals(
    [...Deno.readDirSync(join(root, "fixtures"))].map((entry) => entry.name),
    ["c015-two"],
  )
  assertEquals(
    [...Deno.readDirSync(join(root, "fixtures", "c015-two"))].map((entry) =>
      entry.name
    ),
    ["linear"],
  )
  assertEquals(
    [...Deno.readDirSync(join(root, "fixtures", "c015-two", "linear"))]
      .map((entry) => entry.name),
    ["credentials.toml"],
  )
  const lines: string[] = []
  for (const name of names) {
    lines.push(
      `${name}\0${await sha256Hex(await Deno.readFile(join(root, name)))}\n`,
    )
  }
  const credentialPath = "fixtures/c015-two/linear/credentials.toml"
  lines.push(
    `${credentialPath}\0${await sha256Hex(
      await Deno.readFile(join(root, credentialPath)),
    )}\n`,
  )
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    bundleSha256,
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
  const loaded = await loadCases(root, routes, "c015-")
  assertEquals(loaded.map(({ spec }) => spec.id), ids)
  let requests = 0
  for (const { spec } of loaded) {
    assertEquals(spec.route, "linear user list")
    assertEquals(spec.cwdFixture, "empty")
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1")
    assertEquals(spec.env.NO_COLOR, "1")
    assertEquals(spec.env.LANG, "C.UTF-8")
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.deviation, null)
    if (spec.graphql == null) {
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
      )
      assert(!spec.substitutions.includes("fixturePort"))
      continue
    }
    assertEquals(spec.graphql.path, "/graphql")
    assertEquals(spec.graphql.groups.length, 1)
    assert(spec.substitutions.includes("fixturePort"))
    assertEquals(
      spec.graphql.expectedRequests,
      spec.graphql.groups[0].mode === "ordered"
        ? spec.graphql.groups[0].steps.length
        : -1,
    )
    requests += spec.graphql.expectedRequests
    const group = spec.graphql.groups[0]
    if (group.mode !== "ordered") {
      throw new Error(`${spec.id}: expected ordered requests`)
    }
    for (const [index, step] of group.steps.entries()) {
      if (step.kind !== "graphql") {
        throw new Error(`${spec.id}: unexpected asset request`)
      }
      assertEquals(
        step.identity.authorization,
        spec.id === "c015-workspace-before" ||
          spec.id === "c015-workspace-after"
          ? "lin_api_fake_beta"
          : "lin_api_fake",
      )
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.effects, [])
      assertEquals(step.operation.variables?.first, 100)
      assertEquals(
        step.operation.variables?.includeDisabled,
        spec.argv.includes("--all") || spec.argv.includes("-a") ||
          spec.argv.includes("-aj"),
      )
      if (index === 0) {
        assert(!Object.hasOwn(step.operation.variables ?? {}, "after"))
      }
    }
  }
  assertEquals(requests, 29)
})

Deno.test("C015 negative controls reject wrong path, filter, page size, and cursor", async () => {
  const loaded = await loadCases(root, new Set(["linear user list"]), "c015-")
  const byId = new Map(loaded.map(({ spec }) => [spec.id, spec]))
  const twoPages = byId.get("c015-two-pages")?.graphql?.groups[0]
  if (twoPages?.mode !== "ordered") throw new Error("two page fixture missing")
  const first = twoPages.steps[0]
  const second = twoPages.steps[1]
  if (first.kind !== "graphql" || second.kind !== "graphql") {
    throw new Error("GraphQL step missing")
  }
  const schema = buildPinnedSchema(
    await Deno.readTextFile(
      new URL("../../../graphql/schema.graphql", import.meta.url),
    ),
  )
  const expected = second.operation
  const matches = (actual: typeof expected) =>
    matchGraphQL(expected, actual, schema).matches
  assert(matches(expected))
  assertEquals(matches({ document: "query { viewer { id } }" }), false)
  assertEquals(
    matches({
      ...expected,
      variables: { ...expected.variables, includeDisabled: true },
    }),
    false,
  )
  assertEquals(
    matches({ ...expected, variables: { ...expected.variables, first: 50 } }),
    false,
  )
  assertEquals(
    matches({
      ...expected,
      variables: { ...expected.variables, after: "wrong" },
    }),
    false,
  )
  assertEquals(
    matches({ ...expected, variables: { includeDisabled: false, first: 100 } }),
    false,
  )
  assertEquals(first.operation.variables?.after, undefined)
  assertEquals(second.operation.variables?.after, "next")
  assertEquals(
    matchGraphQL({ ...expected, exactOrigins: [queryPath] }, expected, schema)
      .matches,
    true,
  )
})

Deno.test("C015 negative controls catch inactive leaks, flattening, and local-only ordering", async () => {
  const loaded = await loadCases(root, new Set(["linear user list"]), "c015-")
  const byId = new Map(loaded.map(({ spec }) => [spec.id, spec]))
  const output = (id: string) => {
    const stdout = byId.get(id)?.expected.stdout
    if (stdout == null || !("utf8" in stdout)) {
      throw new Error(`${id}: text stdout missing`)
    }
    return stdout.utf8
  }
  const filtered = JSON.parse(output("c015-json-filter"))
  assertEquals(filtered.nodes.map((member: { id: string }) => member.id), [
    "u-pat",
    "u-zoe",
  ])
  assertNotEquals(
    JSON.stringify(
      {
        ...filtered,
        nodes: [...filtered.nodes, { id: "u-old", active: false }],
      },
      null,
      2,
    ) + "\n",
    output("c015-json-filter"),
  )
  assertNotEquals(
    JSON.stringify(filtered.nodes, null, 2) + "\n",
    output("c015-json-filter"),
  )
  const paged = JSON.parse(output("c015-two-pages"))
  assertEquals(paged.nodes.map((member: { id: string }) => member.id), [
    "u-aaron",
    "u-mona",
  ])
  assertEquals(paged.pageInfo, { hasNextPage: false, endCursor: "final" })
  assertNotEquals(
    JSON.stringify({ ...paged, nodes: [...paged.nodes].reverse() }, null, 2) +
      "\n",
    output("c015-two-pages"),
  )
  assertEquals(JSON.parse(output("c015-inactive-json")).nodes, [])
  assertEquals(JSON.parse(output("c015-json-all")).nodes.length, 3)
  assertEquals(JSON.parse(output("c015-raw-extra-json")).nodes[0].serverOnly, {
    note: "unexpected",
  })
})
