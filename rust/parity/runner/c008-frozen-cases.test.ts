import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c008-frozen-cases/", import.meta.url).pathname
const ids = [
  "c008-alias-json",
  "c008-app-web-missing-opener",
  "c008-empty-json",
  "c008-empty-text",
  "c008-graphql-error",
  "c008-json-filter-sort",
  "c008-json-two-pages",
  "c008-missing-cursor",
  "c008-no-key",
  "c008-raw-date",
  "c008-text-future",
  "c008-text-percent",
  "c008-text-wide-key",
  "c008-transport-error",
  "c008-web-cli-workspace-ignored",
  "c008-web-no-workspace",
]
const bundleSha256 =
  "7fe067544ae960d916676fce1a23979588c651de6c21652abefef2ba6582228e"

Deno.test("C008 freezes 16 exact team-list cases without claiming Rust action parity", async () => {
  const names: string[] = []
  for await (const entry of Deno.readDir(root)) {
    assert(
      entry.isFile && !entry.isSymlink,
      `unexpected C008 entry ${entry.name}`,
    )
    names.push(entry.name)
  }
  names.sort()
  assertEquals(names, ids.map((id) => `${id}.json`))
  const lines: string[] = []
  for (const name of names) {
    lines.push(
      `${name}\0${await sha256Hex(await Deno.readFile(join(root, name)))}\n`,
    )
  }
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
  const loaded = await loadCases(root, routes, "c008-")
  assertEquals(loaded.map((entry) => entry.spec.id), ids)
  let graphqlCases = 0
  let requests = 0
  for (const { spec } of loaded) {
    assertEquals(spec.route, "linear team list")
    assertEquals(spec.cwdFixture, "empty")
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1")
    assertEquals(spec.env.NO_COLOR, "1")
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.configFixture, undefined)
    assertEquals(spec.gitProbe, undefined)
    assertEquals(spec.deviation, null)
    assertEquals(spec.expected.fileEffects, [])
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    if (spec.graphql == null) {
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
      )
      assert(!spec.substitutions.includes("fixturePort"))
      continue
    }
    graphqlCases += 1
    requests += spec.graphql.expectedRequests
    assertEquals(spec.graphql.path, "/graphql")
    assertEquals(spec.graphql.groups.length, 1)
    assert(spec.substitutions.includes("fixturePort"))
    const group = spec.graphql.groups[0]
    if (group.mode !== "ordered") {
      throw new Error(`${spec.id}: unordered fixture`)
    }
    assertEquals(group.steps.length, spec.graphql.expectedRequests)
    for (const step of group.steps) {
      if (step.kind !== "graphql") {
        throw new Error(`${spec.id}: unexpected asset`)
      }
      assertEquals(step.identity.authorization, "lin_api_fake")
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.effects, [])
    }
  }
  assertEquals(graphqlCases, 12)
  assertEquals(requests, 13)
})
