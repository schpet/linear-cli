import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { NATIVE_PARSER_CONTRACTS } from "./native-parser-contract.ts"
import { RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"

Deno.test("Native clap and strict numeric parser goldens form a closed SHA-bound cohort with no requests or effects", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") throw new Error("route path")
    return route.path
  }))
  const cases = await loadCases(
    join(root, "cases"),
    routes,
    undefined,
    RUST_CONTRACT,
  )
  const pins =
    (await Deno.readTextFile(join(root, "native-parser-goldens.sha256")))
      .trimEnd().split("\n")
  assertEquals(pins.length, 107)
  assertEquals(NATIVE_PARSER_CONTRACTS.size, 107)
  let count = 0
  for (const entry of cases) {
    const contract = NATIVE_PARSER_CONTRACTS.get(entry.spec.id)
    if (contract == null) continue
    count++
    const golden = entry.golden
    assert(golden != null)
    assertEquals(golden.spec.deviationId, contract[0])
    assertEquals(golden.spec.approvedSurfaces, contract[1])
    const bytes = await Deno.readFile(
      join(root, "cases/rust-goldens", RUST_CONTRACT, `${entry.spec.id}.json`),
    )
    assert(pins.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`))
    const expected = golden.spec.candidate.expected
    assert(expected != null)
    assertEquals(expected.exit, { code: 2 })
    assertEquals(expected.stdout, { utf8: "" })
    assertEquals(expected.fileEffects, [])
    assert(
      "utf8" in expected.stderr && expected.stderr.utf8.startsWith("error: "),
    )
    assertEquals(golden.spec.candidate.argv ?? null, null)
    if (entry.spec.graphql != null) {
      assertEquals(golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
      assertEquals(golden.spec.candidate.graphql, { steps: [] })
      const candidate = candidateCaseView(entry)
      assertEquals(candidate.spec.graphql?.expectedRequests, 0)
      assertEquals(candidate.spec.graphql?.groups, [])
      assertEquals(
        candidate.spec.graphql?.expectedRecords,
        entry.spec.graphql.initialRecords,
      )
    } else assertEquals(golden.spec.candidate.graphql ?? null, null)
  }
  assertEquals(count, 107)
})
