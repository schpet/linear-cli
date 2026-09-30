import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

Deno.test("C072/C075 pin full source upload pipelines, exact bytes/effects and only GraphQL identity deviation", async () => {
  const root = new URL("./", import.meta.url).pathname
  const frozen = join(root, "c072-c075-frozen-cases")
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => /^c0(72|75)-/.test(entry.spec.id))
  const pins = (await Deno.readTextFile(join(frozen, "source.sha256")))
    .trimEnd().split("\n")
  assertEquals(pins.length, 24)
  assertEquals(entries.length, 24)
  assertEquals(
    entries.filter((entry) => entry.spec.id.startsWith("c072-")).length,
    12,
  )
  assertEquals(entries.filter((entry) => entry.spec.graphql != null).length, 17)
  for (const entry of entries) {
    const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
    const source = parseCase(
      JSON.parse(new TextDecoder().decode(bytes)),
      entry.spec.id,
    )
    assert(pins.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`))
    assertEquals(source.deviation, null)
    assertEquals({ ...entry.spec, deviation: null }, source)
    assertEquals(
      source.route,
      entry.spec.id.startsWith("c072-")
        ? "linear issue comment add"
        : "linear issue attach",
    )
    const hasGraphql = source.graphql != null
    assertEquals(
      entry.spec.deviation?.id ?? null,
      hasGraphql ? "R01H-GRAPHQL-UA" : null,
    )
    assertEquals(
      entry.golden?.spec.approvedSurfaces ?? null,
      hasGraphql ? ["graphql-user-agent"] : null,
    )
    assertEquals(entry.golden?.spec.candidate.expected ?? null, null)
    assertEquals(entry.golden?.spec.candidate.argv ?? null, null)
    assertEquals(entry.golden?.spec.candidate.graphql ?? null, null)
    assertEquals(candidateCaseView(entry).spec.graphql, source.graphql)
    if (source.graphql != null) {
      let count = 0
      for (const group of source.graphql.groups) {
        assert(group.mode === "ordered")
        for (const step of group.steps) {
          count++
          if (step.kind === "asset") {
            assertEquals(step.method, "PUT")
            assert(step.path.startsWith("/signed/"))
            assert(
              step.forbiddenHeaders.some((name) =>
                name.toLowerCase() === "authorization"
              ),
            )
          } else {
            assertEquals(step.identity.authorization, "lin_api_fake")
            assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          }
        }
      }
      assertEquals(count, source.graphql.expectedRequests)
    }
  }
})
