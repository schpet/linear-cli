import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C083-C084 freeze21 complete dynamic API and runtime schema request/output/effect contracts", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((r) => {
    assert(typeof r.path === "string")
    return r.path
  }))
  const frozen = join(root, "c083-c084-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "fdbb3718cd42930b5c56ab14457c12c3ead4e997c5c258da20433fc272f0004d",
  )
  const lines = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(lines.length, 24)
  for (const line of lines) {
    const [sha, path] = line.split("  ")
    const bytes = await Deno.readFile(join(frozen, path))
    assertEquals(await sha256Hex(bytes), sha)
    if (path.startsWith("fixtures/")) {
      assertEquals(await Deno.readFile(join(root, "cases", path)), bytes)
    }
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((e) => /^c08[34]-/.test(e.spec.id))
  assertEquals(entries.length, 21)
  let ua = 0, os = 0, exact = 0
  for (const e of entries) {
    const source = parseCase(
      JSON.parse(await Deno.readTextFile(join(frozen, `${e.spec.id}.json`))),
    )
    assertEquals({ ...e.spec, deviation: null }, source)
    const c = candidateCaseView(e)
    assertEquals(c.spec.argv, source.argv)
    assertEquals(c.spec.stdin, source.stdin)
    assertEquals(c.spec.graphql, source.graphql)
    assertEquals(c.spec.fixtureServer, source.fixtureServer)
    assertEquals(c.spec.expected.fileEffects, source.expected.fileEffects)
    assertEquals(c.spec.expected.stdout, source.expected.stdout)
    assertEquals(c.spec.expected.exit, source.expected.exit)
    if (e.golden == null) {
      exact++
      assertEquals(e.spec.deviation, null)
      assertEquals(c.spec.expected, source.expected)
      continue
    }
    assertEquals(e.golden.sha256, GOLD.get(source.id))
    assertEquals(e.golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
    if (source.id === "c084-missing-output-parent") {
      os++
      assertEquals(e.spec.deviation?.id, "SCHEMA-OUTPUT-OS-TEXT")
      assertEquals(e.golden.spec.approvedSurfaces, [
        "stderr",
        "graphql-user-agent",
      ])
      assertEquals(c.spec.expected.stderr, {
        utf8:
          "✗ Failed to fetch schema: Failed to write schema: {{cwd}}/missing/schema.graphql: No such file or directory (os error 2)\n",
      })
    } else {
      ua++
      assertEquals(e.spec.deviation?.id, "R01H-GRAPHQL-UA")
      assertEquals(e.golden.spec.approvedSurfaces, ["graphql-user-agent"])
      assertEquals(c.spec.expected, source.expected)
    }
  }
  assertEquals({ ua, os, exact }, { ua: 10, os: 1, exact: 10 })
})
const GOLD = new Map<string, string>([
  [
    "c083-arbitrary-mutation-effects",
    "eb523ee6bbee1ea66119efe8383bf161b62c99fe1cd18fccdfb63c467133ca4a",
  ],
  [
    "c083-paginate-second-nonjson-drops-prior-nodes",
    "6129b81e21543680aaf65a965782c93bd396353bf03e91bf4ad557fae14d1c77",
  ],
  [
    "c083-paginate-two-pages-after-override",
    "99ed92bf796264dcc5f08df36067843051b423b3b264cd3b31767de239033fe2",
  ],
  [
    "c083-silent-partial-mutation-errors",
    "58283c20a47587a5a427f26a4f0da8191f774b3761ba03b9a85d61189b497429",
  ],
  [
    "c084-friendly-sdk-error",
    "483b3c2ea883db3ab04a61d7963934ebacb77574ccac9eba609ad6a9e9f89240",
  ],
  [
    "c084-json-file-overwrite",
    "c5103f1919dbf006b0b0ba04992aa4e95f9eadba87a1937fd192537843b5003f",
  ],
  [
    "c084-json-raw-data-not-sdl-shaped",
    "f48f9cc7c142d6a1ee243d303d68b270b44fa9e634c17b3d5e3c684105ff092d",
  ],
  [
    "c084-missing-output-parent",
    "1dc05b074383cc9c09ce19ef7b22a96ace5e9bc6cdef3bc6002335eaf6207e46",
  ],
  [
    "c084-pinned-schema-output",
    "9d99309ee673d474bc6270df88f51bc3668921d5b2b105334d1472fc64fa62ba",
  ],
  [
    "c084-synthetic-json-stdout",
    "f68d11108ba368c3dd7e86993a582f4454d50044078002995731b78848676321",
  ],
  [
    "c084-synthetic-sdl-stdout",
    "b5ea88e020a7bc033395da9bd73d16f5e9d5a2869b5afaa1671eb3c86b8788fb",
  ],
])
