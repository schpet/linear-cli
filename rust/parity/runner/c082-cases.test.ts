import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C082 freezes11 finite config source contracts and exact auth/query/no-write effects", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c082-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "0e88f64134d4a289f57680a87c16addeb41122cd7c01a1ad8e5353d524db8eb1",
  )
  const lines = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(lines.length, 11)
  for (const row of lines) {
    const [sha, path] = row.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  for (const [path, sha] of FIXTURES) {
    const source = await Deno.readFile(join(frozen, path))
    assertEquals(await sha256Hex(source), sha)
    assertEquals(await Deno.readFile(join(root, "cases", path)), source)
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => entry.spec.id.startsWith("c082-"))
  assertEquals(entries.length, 11)
  const counts = new Map<string, number>()
  for (const entry of entries) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    assertEquals(source.route, "linear config")
    const candidate = candidateCaseView(entry)
    assertEquals(candidate.spec.argv, source.argv)
    assertEquals(candidate.spec.stdin, source.stdin)
    assertEquals(candidate.spec.graphql, source.graphql)
    assertEquals(candidate.spec.expected.fileEffects, [])
    assertEquals(candidate.spec.expected.exit, source.expected.exit)
    const kind = entry.spec.deviation?.id ?? "exact"
    counts.set(kind, (counts.get(kind) ?? 0) + 1)
    if (source.graphql != null) {
      assertEquals(source.graphql.expectedRequests, 1)
      for (const group of source.graphql.groups) {
        assertEquals(group.mode, "ordered")
        assert("steps" in group)
        for (const step of group.steps) {
          assertEquals(step.kind, "graphql")
          assert("operation" in step)
          assert(!("variables" in step.operation))
          assertEquals(step.effects, [])
        }
      }
    }
    if (entry.golden == null) {
      assertEquals(kind, "exact")
      assertEquals(candidate.spec.expected, source.expected)
      continue
    }
    assertEquals(entry.golden.sha256, GOLD.get(source.id))
    if (kind === "CLAP-NATIVE-CLI-SURFACE") {
      assertEquals(source.graphql, null)
      const actual = entry.golden.spec.approvedSurfaces
      assert(actual.length > 0)
      assert(
        actual.every((surface) => surface === "stdout" || surface === "stderr"),
      )
      if (source.id === "c082-leaf-help") assertEquals(actual, ["stdout"])
    } else {
      assertEquals(kind, "R01H-GRAPHQL-UA")
      assertEquals(entry.golden.spec.approvedSurfaces, ["graphql-user-agent"])
      assertEquals(
        entry.golden.spec.candidate.graphqlUserAgent,
        RUST_USER_AGENT,
      )
      assertEquals(candidate.spec.expected, source.expected)
    }
  }
  assertEquals(Object.fromEntries(counts), {
    "CLAP-NATIVE-CLI-SURFACE": 3,
    "R01H-GRAPHQL-UA": 5,
    "exact": 3,
  })
  const route = manifest.routes.find((route) => route.path === "linear config")
  assert(route != null)
  assertEquals(route.aliases, ["configure"])
  assertEquals(route.children, [])
})
const GOLD = new Map<string, string>([
  [
    "c082-alias-json-rejected",
    "4d118d03d686a43d2132f07d3ec3670e902e9a4ed05974505f843ea6336f9717",
  ],
  [
    "c082-dotenv-key-query-error",
    "f369178b22462101f31fa42b438ac9ef5457c108857ba47928b7fa890aa12b2b",
  ],
  [
    "c082-extra-positional",
    "3aaf79198543462f042636c498032fabf66cf0bca6b5860978e2692a20d630a7",
  ],
  [
    "c082-global-key-query-error",
    "8e7ba2f933e259f020812289317af278650582976590b93166bdf5076a346c91",
  ],
  [
    "c082-leaf-help",
    "80f34dbebeb000777ee70e714dc0cfd537421c162c8ae7f79d8da02cd2a5c65f",
  ],
  [
    "c082-project-key-query-error",
    "d40878f0b64520fabbe8c223621ed7f3bad4c6b84e35fe5a2a716fe2d33f1a0e",
  ],
  [
    "c082-single-auto-over-project-workspace",
    "5a95bc4e3c8171fdb1ac683441ee88a684061634b42492b54ab27e73d9150e6d",
  ],
  [
    "c082-single-auto-query-error",
    "b4724b50bce965e4529307ab50d01dd67b58c826bf9a8559c656048d2d1324e0",
  ],
])
const FIXTURES = new Map<string, string>([
  [
    "fixtures/config-dotenv/.env",
    "46f08dbf2a164c72905991cba8dd1ec3400d7ca25ba0c265e4d684af70ab909c",
  ],
  [
    "fixtures/config-global/linear/credentials.toml",
    "f22cab1cb4c781a155476519eba59533680a91d11897c318e4f1bf58c44526ea",
  ],
  [
    "fixtures/config-global/linear/linear.toml",
    "1d8408b48b6222f778cae50ca7db242d35122a8a29025878e8a8723668602069",
  ],
  [
    "fixtures/config-one/linear/credentials.toml",
    "f22cab1cb4c781a155476519eba59533680a91d11897c318e4f1bf58c44526ea",
  ],
  [
    "fixtures/config-project/.linear.toml",
    "4e383baa54a85564bb3638f229e4fad4fe30874fe815e8508d47384c298b054f",
  ],
  [
    "fixtures/config-project-workspace/.linear.toml",
    "7865212c4239c38a0b315b0a6749988a78dfe956a29160f0ff1d6c887d6cc1ce",
  ],
  [
    "fixtures/config-two/linear/credentials.toml",
    "986e0799c2a1ca46436a74c00c76ef174755f2581c250ae744282535065fd4b5",
  ],
])
