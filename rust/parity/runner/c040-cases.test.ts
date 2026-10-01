import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C040 freezes20 full initiative update contracts, exact scalar inputs and request/effect order", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c040-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "602df6a7fec597346f09775fcaca3cd478cf5f17b22e2bd2fe6824a8945cfa30",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 20)
  for (const row of rows) {
    const [sha, path] = row.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => entry.spec.id.startsWith("c040-"))
  assertEquals(entries.length, 20)
  let parser = 0, ua = 0, uncaught = 0
  for (const entry of entries) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    assertEquals(source.route, "linear initiative update")
    const candidate = candidateCaseView(entry)
    assertEquals(entry.golden?.sha256, GOLD_PINS.get(source.id))
    assertEquals(candidate.spec.argv, source.argv)
    assertEquals(candidate.spec.graphql, source.graphql)
    assertEquals(
      candidate.spec.expected.fileEffects,
      source.expected.fileEffects,
    )
    if (source.id === "c040-leaf-help" || source.id === "c040-json-rejected") {
      parser++
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(source.graphql, null)
      assertEquals(
        entry.golden?.spec.approvedSurfaces,
        source.id.endsWith("help") ? ["stdout"] : ["stdout", "stderr"],
      )
    } else {
      assertEquals(candidate.spec.expected.exit, source.expected.exit)
      assertEquals(candidate.spec.expected.stdout, source.expected.stdout)
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        RUST_USER_AGENT,
      )
      if (entry.spec.deviation?.id === "C040-UNCAUGHT-TYPED") {
        uncaught++
        assertEquals(entry.golden?.spec.approvedSurfaces, [
          "stderr",
          "graphql-user-agent",
        ])
        assert(
          "utf8" in source.expected.stderr &&
            source.expected.stderr.utf8.startsWith("error: Uncaught"),
        )
        assert(
          "utf8" in candidate.spec.expected.stderr &&
            candidate.spec.expected.stderr.utf8.startsWith("✗ "),
        )
      } else {
        ua++
        assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
        assertEquals(entry.golden?.spec.approvedSurfaces, [
          "graphql-user-agent",
        ])
        assertEquals(candidate.spec.expected.stderr, source.expected.stderr)
      }
    }
  }
  assertEquals({ parser, ua, uncaught }, { parser: 2, ua: 14, uncaught: 4 })
})
const GOLD_PINS = new Map<string, string>([
  [
    "c040-slug-error-name-first-match",
    "653218e339d3266fc385572f85c7f3784f08f9c92fca1b8660b6097ed2574520",
  ],
  [
    "c040-resolver-errors-no-details",
    "3cafabfd09854752eafd38b34a0e1b78235b554f724e66b386205801932ab9da",
  ],
  [
    "c040-null-details",
    "dca7505ac73e5ce1b16c716b6e6c446bd1f5d4b5cd93566b15ca3853e07809a1",
  ],
  [
    "c040-url-miss-no-name",
    "05f14baea3f3b590ec040bd1f7e68131f8d95ad60f54fa1336ba7292d9c2aabb",
  ],
  [
    "c040-url-only-slug",
    "fe33b5dd5d8c9b240eda351380b912fe9ccd99eaa40c11e9b919daad8af189af",
  ],
  [
    "c040-markdown-description-raw-stdin-ignored",
    "a8a72a90bee87854a595dc1670244668484165924b0eaae9f4374b92c5bcd379",
  ],
  [
    "c040-owner-email-wins-display-first",
    "eb3b4f750f3b7977dceb16eb70b41f1e0535b193b1453f20fd336fd6539e6880",
  ],
  [
    "c040-no-local-date-color-validation",
    "9fab2f071cf811f0eff7667a1bdf2d81707d281cda68e764e6ea9a084e76553d",
  ],
  [
    "c040-interactive-stdout-pipe-falls-through",
    "db714a1037a3d5c958e8b0517785afd66a0d205f75510220b1d13f2793275f4c",
  ],
  [
    "c040-slug-first-duplicate-picks-first",
    "1f7b08d4dee1cb9aaecec9391b301e139bca469df1c4553b64b623d50b673181",
  ],
  [
    "c040-same-name-still-mutates",
    "022ce49d2d63f8088826fe24da4e713bec990c0f4322287cd06ed76f81a5086b",
  ],
  [
    "c040-details-error-context",
    "fefeffeb3cd1e7505f520b01e82f1591ca2489b5d668fc875c1224a386e5a8cc",
  ],
  [
    "c040-empty-returned-url-omitted",
    "d140817a251e7172bc3214eaab2b9cc623cf9231453161675cede450b6b1dba5",
  ],
  [
    "c040-leaf-help",
    "dcf0f9ac74b79267da3e14164871ee245e87a17062f3a7403db72f215d65360a",
  ],
  [
    "c040-owner-missing-after-details",
    "223a3803389144cbbcacc307a7d0b18d7dcc42163ab016b5cde7d4fa4bcc0c3b",
  ],
  [
    "c040-mutation-error-context",
    "77196cf680a7e5974499075f948a927540b9cfe8de431b9092c8f89c451cb96f",
  ],
  [
    "c040-owner-self",
    "2e1b4f553cc26d2feabbf34a8ca2868b71b346175bb86e033f1f91eb777cacc3",
  ],
  [
    "c040-json-rejected",
    "2b107ef7b67024290b0fe663fadf07f3d1eaa3003599f694733b33272f59f4d9",
  ],
  [
    "c040-false-mutation-double-context",
    "5ac031cf398b101fdc0ce3610b2410d222468aabde250bd5f586df78a3d563c5",
  ],
  [
    "c040-no-flags-no-auto",
    "13c7d7bf850bfa45c1fe811356ad3b8d6e1391ecfb5b3869f6028f3b0788fc19",
  ],
])
