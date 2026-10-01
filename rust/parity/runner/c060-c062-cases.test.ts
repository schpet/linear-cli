import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C060-C062 freeze39 full issue read contracts with exact requests, connections, script bytes and file effects", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((r) => {
    assert(typeof r.path === "string")
    return r.path
  }))
  const frozen = join(root, "c060-c062-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "0cd51a3ef48f6758a51a78e93f2af0f6f1ad858ffec791ec04cba28c9ca1e7d8",
  )
  const lines = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(lines.length, 39)
  for (const row of lines) {
    const [sha, path] = row.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  for (const [path, sha] of FIXTURES) {
    const bytes = await Deno.readFile(join(frozen, path))
    assertEquals(await sha256Hex(bytes), sha)
    assertEquals(await Deno.readFile(join(root, "cases", path)), bytes)
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((e) => /^c06[012]-/.test(e.spec.id))
  assertEquals(entries.length, 39)
  const counts = new Map<string, number>()
  const leafCounts = new Map<string, number>()
  for (const e of entries) {
    const source = parseCase(
      JSON.parse(await Deno.readTextFile(join(frozen, `${e.spec.id}.json`))),
    )
    assertEquals({ ...e.spec, deviation: null }, source)
    const c = candidateCaseView(e)
    assertEquals(c.spec.argv, source.argv)
    assertEquals(c.spec.stdin, source.stdin)
    assertEquals(c.spec.graphql, source.graphql)
    assertEquals(c.spec.expected.fileEffects, source.expected.fileEffects)
    leafCounts.set(source.route, (leafCounts.get(source.route) ?? 0) + 1)
    const kind = e.spec.deviation?.id ?? "exact"
    counts.set(kind, (counts.get(kind) ?? 0) + 1)
    if (e.golden == null) {
      assertEquals(kind, "exact")
      assertEquals(c.spec.expected, source.expected)
      continue
    }
    assertEquals(e.golden.sha256, GOLD.get(source.id))
    if (kind === "CLAP-NATIVE-CLI-SURFACE") {
      assert(source.id.endsWith("-help"))
      assertEquals(e.golden.spec.approvedSurfaces, ["stdout"])
      assertEquals(source.graphql, null)
      assertEquals(c.spec.expected.exit, source.expected.exit)
      assertEquals(c.spec.expected.stderr, source.expected.stderr)
    } else if (kind === "ISSUE-READ-UNEXPECTED-SHAPE") {
      assertEquals(source.id, "c062-json-null-issue")
      assertEquals(source.expected.stdout, { utf8: "null\n" })
      assertEquals(c.spec.expected.exit, { code: 1 })
      assertEquals(c.spec.expected.stdout, { utf8: "" })
      assertEquals(e.golden.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
        "graphql-user-agent",
      ])
      assertEquals(e.golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
    } else {
      assertEquals(kind, "R01H-GRAPHQL-UA")
      assertEquals(e.golden.spec.approvedSurfaces, ["graphql-user-agent"])
      assertEquals(e.golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
      assertEquals(c.spec.expected, source.expected)
    }
  }
  assertEquals(Object.fromEntries(counts), {
    "R01H-GRAPHQL-UA": 23,
    "CLAP-NATIVE-CLI-SURFACE": 3,
    "ISSUE-READ-UNEXPECTED-SHAPE": 1,
    "exact": 12,
  })
  assertEquals(Object.fromEntries(leafCounts), {
    "linear issue mine": 13,
    "linear issue query": 14,
    "linear issue view": 12,
  })
})
const GOLD = new Map<string, string>([
  [
    "c060-allstates-unlimited",
    "f4f8155a65de7f5d9413411adb010e8bac39aa1bb56a1e75c857f460b1937891",
  ],
  [
    "c060-combined-filters",
    "8628a03549f162840f141b95bc8c671187ca6aa12babf97813de7baec1761908",
  ],
  [
    "c060-default-empty",
    "88707dfb4ce5e52e952a141bf5b60b47a7d9084bf4142a6d9ab89cf48738d991",
  ],
  [
    "c060-default-table",
    "ef279d24648ace82658689712039d28b188aecd3ff1ea970896893b677eddca7",
  ],
  [
    "c060-explicit-team-cycle-state",
    "3a0a76424fea0985c52088c6e22338b99e3c192794d4a51bb255d69d47db59da",
  ],
  [
    "c060-help",
    "065a3acfd3bd052fd8cd5589ac31259fc68c4cde7d2524cd6ae0c2cab015857f",
  ],
  [
    "c060-negative-limit-source",
    "c0735dbcb91a22c92b075cd2f3de35ff36da6683ae04edeeba7287ab5e7e4d52",
  ],
  [
    "c060-project-similar-pipe",
    "0335f3527a3244e77930a10a67dbfede1b33f63415a807d377cbfb851d46ec8e",
  ],
  [
    "c061-allteams-filter-pages",
    "8026075988cf20a5c764483b92d697bc7aa306824dfea84eff9c66c2225f1e8b",
  ],
  [
    "c061-ambient-note-json",
    "3aba6919169613772b3abf1bd6af89a3489fdc95febec29422a90f066dff80c4",
  ],
  [
    "c061-friendly-fetch-error",
    "4add1a23f2821754907d2efc85c81d08d1d7ec87136ed1e0469132f3edae83e1",
  ],
  [
    "c061-help",
    "d356f211f258a001f79e40e56610cb27e6dbd22f5eb57cd87f154464acf39881",
  ],
  [
    "c061-project-default-no-note",
    "eee59c32e12b898038ff97de0ad9151f25d760507d294dd9a9f2fc9bc118ecb3",
  ],
  [
    "c061-project-similar-pipe",
    "3d8adcdb32eb1b8c3c829398067be183aa5c93e06184ffb46ce52b3e82735336",
  ],
  [
    "c061-search-pages-json",
    "4b2dea259ad0dd17cfc9f1b32578d4d306d3938f127b8777126b25cc48501fc7",
  ],
  [
    "c061-state-missing",
    "ad7217b48f2bff1386f1269915d5ee1d01439496333350d934c985f2d57b61eb",
  ],
  [
    "c061-team-dedupe-state-name",
    "6409dda7015f4234e17702331fa236afcf569b87eed9e0ff85e667ecc82d9633",
  ],
  [
    "c062-alias-url-json",
    "1990c0a65ca633f6fddbf7f4f4fdc0b7a6a8187d532d1fde48dee403d2fa553c",
  ],
  [
    "c062-download-failures-continue",
    "d33ac5d682f9cdfb3e379c0fae6ac67bca9fe602137a2312f3fba147e505a5a3",
  ],
  [
    "c062-download-images-attachment",
    "d6852477e0ce040ba971b518a3b9763e5e77c77b9ee9fd4246db48d7b1e167b9",
  ],
  [
    "c062-help",
    "89b2c3858769abd477b027a048ad219379baf93dd928f5c0e32ed4aebb78f048",
  ],
  [
    "c062-json-nested-comments",
    "0daba1b300dd2319d56b1d8a801b519d045ab6829b1523878491b374f1d3c776",
  ],
  [
    "c062-json-null-issue",
    "8c0eed7aa7070d666415dfc6bdb5c7198856a8a1febc9ae67a31a217e876a9a2",
  ],
  [
    "c062-markdown-all-threads",
    "eabb341b6e16c7ec83ea1ce693d038d7cfd4cae50b63012a42238f4e669d30b8",
  ],
  [
    "c062-markdown-hidden-resolved",
    "cc37fd1d4cd9636ff85587c01a817abf50a7d07547fa34241438f8727b9b59bd",
  ],
  [
    "c062-markdown-no-comments",
    "507f104685ab7f0d864bffe4e97c2827660ea587d7f072c8a7022112f00000ba",
  ],
  [
    "c062-numeric-json-no-comments",
    "b557f7c9a482d6e6c63f59b60d5dd979973bc2fc455d76a734bc58bd562b5c15",
  ],
])
const FIXTURES = new Map<string, string>([
  [
    "fixtures/query-project/.linear.toml",
    "6759079412fa2f30cf370f34710432c665fac6dc1aab76fc81a00f43fc57ee53",
  ],
])
