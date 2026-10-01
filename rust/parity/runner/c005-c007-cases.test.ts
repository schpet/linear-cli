import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C005-C007 freeze13 representable auth effects and keep15 backend/TTY controls private", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((r) => {
    assert(typeof r.path === "string")
    return r.path
  }))
  const frozen = join(root, "c005-c007-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "4f75cc20817febf8937e30ee8e945971242af1d0a16e4b59ae2ea4554cc06cc9",
  )
  const lines = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(lines.length, 18)
  const ids = new Set<string>()
  for (const line of lines) {
    const [sha, path] = line.split("  ")
    const bytes = await Deno.readFile(join(frozen, path))
    assertEquals(await sha256Hex(bytes), sha)
    if (path.startsWith("fixtures/")) {
      assertEquals(await Deno.readFile(join(root, "cases", path)), bytes)
    } else ids.add(path.replace(/\.json$/, ""))
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((e) => ids.has(e.spec.id))
  assertEquals(entries.length, 13)
  let ua = 0, exact = 0
  for (const e of entries) {
    const source = parseCase(
      JSON.parse(await Deno.readTextFile(join(frozen, `${e.spec.id}.json`))),
    )
    assertEquals({ ...e.spec, deviation: null }, source)
    if (source.id === "c005-friendly-sdk-failure") {
      const group = source.graphql?.groups[0]
      assert(group?.mode === "ordered")
      const step = group.steps[0]
      assert(step?.kind === "graphql")
      // Ordinary SDK rejection: schema validation of the sent query succeeds.
      // Exact original response bytes, not a validationErrors fixture contract.
      assertEquals(step.response, {
        kind: "transport",
        status: 200,
        headers: { "content-type": "application/json" },
        body: {
          utf8:
            '{"errors": [{"message": "DUMMY raw rejection", "extensions": {"userPresentableMessage": "DUMMY friendly rejection"}}]}',
        },
      })
    }
    const c = candidateCaseView(e)
    assertEquals(c.spec.argv, source.argv)
    assertEquals(c.spec.stdin, source.stdin)
    assertEquals(c.spec.graphql, source.graphql)
    assertEquals(c.spec.expected, source.expected)
    if (e.golden == null) exact++
    else {
      ua++
      assertEquals(e.spec.deviation?.id, "R01H-GRAPHQL-UA")
      assertEquals(e.golden.spec.approvedSurfaces, ["graphql-user-agent"])
      assertEquals(e.golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
      const expected: typeof e.golden.spec = {
        formatVersion: 1,
        caseId: source.id,
        deviationId: "R01H-GRAPHQL-UA",
        contract: RUST_CONTRACT,
        approvedSurfaces: ["graphql-user-agent"],
        candidate: { graphqlUserAgent: RUST_USER_AGENT },
      }
      assertEquals(e.golden.spec, expected)
    }
  }
  assertEquals({ ua, exact }, { ua: 6, exact: 7 })
})
