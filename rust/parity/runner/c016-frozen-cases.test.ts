import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c016-frozen-cases/", import.meta.url).pathname
const bundleSha256 =
  "e37ac39b88b8534338189aefcbfec0795a55211bc260afb7ba713aedf1c7e443"

async function filesUnder(dir: string, prefix = ""): Promise<string[]> {
  const files: string[] = []
  for await (const entry of Deno.readDir(dir)) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`
    assert(!entry.isSymlink, `unexpected symlink ${relative}`)
    if (entry.isDirectory) {
      files.push(...await filesUnder(join(dir, entry.name), relative))
    } else {
      assert(entry.isFile, `unexpected entry ${relative}`)
      files.push(relative)
    }
  }
  return files
}

Deno.test("C016 frozen label-list corpus is exact, private, and request-bound", async () => {
  const files = (await filesUnder(root)).sort()
  assertEquals(files.length, 55)
  const lines = await Promise.all(
    files.map(async (file) =>
      `${file}\0${await sha256Hex(await Deno.readFile(join(root, file)))}\n`
    ),
  )
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    bundleSha256,
  )

  const cases = await loadCases(root, new Set(["linear label list"]), "c016-")
  assertEquals(cases.length, 51)
  assertEquals(
    cases.map(({ spec }) => `${spec.id}.json`),
    files.filter((file) => file.endsWith(".json")),
  )
  let graphqlCases = 0
  let requests = 0
  for (const { spec } of cases) {
    assertEquals(spec.route, "linear label list")
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1")
    assertEquals(
      spec.env.NO_COLOR,
      spec.id === "c016-color-enabled" ? undefined : "1",
    )
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.expected.fileEffects, [])
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    const graphql = spec.graphql
    if (graphql == null) {
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
      )
      continue
    }
    graphqlCases++
    requests += graphql.expectedRequests
    assertEquals(graphql.path, "/graphql")
    assertEquals(graphql.groups.length, 1)
    const group = graphql.groups[0]
    if (group.mode !== "ordered") throw new Error(`${spec.id}: unordered`)
    assertEquals(group.steps.length, graphql.expectedRequests)
    for (const step of group.steps) {
      if (step.kind !== "graphql") throw new Error(`${spec.id}: asset`)
      assert(
        step.identity.authorization != null &&
          step.identity.authorization.startsWith("lin_api_fake"),
      )
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.effects, [])
    }
  }
  assertEquals(graphqlCases, 36)
  assertEquals(requests, 49)
})
