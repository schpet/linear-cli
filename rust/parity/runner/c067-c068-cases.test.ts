import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C067-C068 freeze11 representable command contracts and retain full28 private process controls", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c067-c068-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "afee8237303a20f0c62bab5f535faef785184fd1a762e747bf76212c5dc4ab8a",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 12)
  for (const row of rows) {
    const [sha, path] = row.split("  ")
    const bytes = await Deno.readFile(join(frozen, path))
    assertEquals(await sha256Hex(bytes), sha)
    if (path.startsWith("fixtures/")) {
      assertEquals(await Deno.readFile(join(root, "cases", path)), bytes)
    }
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((e) => /^c06[78]-/.test(e.spec.id))
  assertEquals(entries.length, 11)
  let exact = 0, ua = 0, shape = 0
  for (const entry of entries) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    const candidate = candidateCaseView(entry)
    assertEquals(candidate.spec.argv, source.argv)
    assertEquals(candidate.spec.stdin, source.stdin)
    assertEquals(candidate.spec.graphql, source.graphql)
    assertEquals(
      candidate.spec.expected.fileEffects,
      source.expected.fileEffects,
    )
    if (entry.golden == null) {
      exact++
      assertEquals(candidate.spec.expected, source.expected)
      continue
    }
    assertEquals(entry.golden.sha256, GOLD.get(source.id))
    assertEquals(entry.golden.spec.candidate.graphqlUserAgent, RUST_USER_AGENT)
    if (source.id === "c068-minimal-source-success") {
      shape++
      assertEquals(entry.spec.deviation?.id, "ISSUE-DESCRIBE-RESPONSE-SHAPE")
      assertEquals(entry.golden.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
        "graphql-user-agent",
      ])
      assertEquals(source.expected.exit, { code: 0 })
      assertEquals(candidate.spec.expected.exit, { code: 1 })
      assertEquals(candidate.spec.expected.stdout, { utf8: "" })
      assertEquals(candidate.spec.expected.stderr, {
        utf8:
          "✗ Failed to get issue description: response JSON did not match the expected operation shape: missing field `identifier`\n",
      })
      assertEquals(source.graphql?.expectedRequests, 1)
    } else {
      ua++
      assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
      assertEquals(entry.golden.spec.approvedSurfaces, ["graphql-user-agent"])
      assertEquals(candidate.spec.expected, source.expected)
    }
  }
  assertEquals({ exact, ua, shape }, { exact: 6, ua: 4, shape: 1 })
})
const GOLD = new Map<string, string>([
  [
    "c068-default-text-controls",
    "8cf1eeef68244a13373e75b27ba8b5c30156da86b6e768b5f5363ed3cb9c72a2",
  ],
  [
    "c068-ref-url",
    "fce06603dda053363d6bd822a7ae06b5be9d6a890cc8eb8c46d4e46aceb3b1d7",
  ],
  [
    "c068-numeric-team",
    "c620c1fb38be7e1761c118200d209ab2efea2752241fa62c4c92c0cfb5eceac9",
  ],
  [
    "c068-friendly-sdk-notfound",
    "e291628b32a5e1074383e4f341a6f03a9eea3212039885b4f651ff90eb002da2",
  ],
  [
    "c068-minimal-source-success",
    "4c8399fe3b24fd3ea45c0a6fd09935e2c04a9bfa5b5858d5d68da8aedebf2d52",
  ],
])
