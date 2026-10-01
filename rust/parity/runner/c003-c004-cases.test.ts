import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

Deno.test("C003/C004 pins source34, fake fixtures and only exact native/startup changed surfaces", async () => {
  const root = new URL("./", import.meta.url).pathname
  const frozen = join(root, "c003-c004-frozen-cases")
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const sourcePins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(sourcePins),
    "21d58b0dc63ffca3e94c73b4d0452a395cb20a56435ed9b8ead1c161a342af23",
  )
  const rows = new TextDecoder().decode(sourcePins).trimEnd().split("\n")
  assertEquals(rows.length, 34)
  const fixturePins = await Deno.readFile(join(frozen, "fixtures.sha256"))
  assertEquals(
    await sha256Hex(fixturePins),
    "9ec76c401c68fccc84fcf6f8fba72d40ca81a2b9a454330b70a65dbf9bab427f",
  )
  for (
    const row of new TextDecoder().decode(fixturePins).trimEnd().split("\n")
  ) {
    const [sha, name] = row.split("  ")
    assertEquals(
      await sha256Hex(await Deno.readFile(join(frozen, "fixtures", name))),
      sha,
    )
    assertEquals(
      await sha256Hex(await Deno.readFile(join(root, "cases/fixtures", name))),
      sha,
    )
  }
  const bindings = [
    {
      "caseId": "c003-extra-positional",
      "id": "CLAP-NATIVE-CLI-SURFACE",
      "surfaces": [
        "stdout",
        "stderr",
      ],
      "sha256":
        "dc38f1c8ba7f02f39dbff77eecbb99598e56e593d9d069be9c17003803c50599",
    },
    {
      "caseId": "c003-leaf-help",
      "id": "CLAP-NATIVE-CLI-SURFACE",
      "surfaces": [
        "stdout",
      ],
      "sha256":
        "fe046b68d9c8f87c7a99871b4a331b5cb6fe668f9eacfd556230269febf992c7",
    },
    {
      "caseId": "c004-extra-positional",
      "id": "CLAP-NATIVE-CLI-SURFACE",
      "surfaces": [
        "stdout",
        "stderr",
      ],
      "sha256":
        "c962508b5ec55accf342900a5c2660c36168678103ce785b753ef6cd79389f0a",
    },
    {
      "caseId": "c004-leaf-help",
      "id": "CLAP-NATIVE-CLI-SURFACE",
      "surfaces": [
        "stdout",
      ],
      "sha256":
        "58afb9f3d55fcc105e34f0617366b832263a3219219b7a3a882e01297b453914",
    },
    {
      "caseId": "c004-permissive-inline-rewrite",
      "id": "R02C2G-CREDENTIAL-STARTUP",
      "surfaces": [
        "exit",
        "stdout",
        "stderr",
        "files",
      ],
      "sha256":
        "0cf5b16b8dafa7b000dd6c4b2d71cfed7753015d18746e8be252832459d4380c",
    },
  ]
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => /^c00[34]-/.test(entry.spec.id))
  assertEquals(entries.length, 34)
  assertEquals(
    entries.filter((entry) => entry.spec.route === "linear auth token").length,
    19,
  )
  assertEquals(
    entries.filter((entry) => entry.spec.route === "linear auth default")
      .length,
    15,
  )
  for (const entry of entries) {
    const bytes = await Deno.readFile(join(frozen, `${entry.spec.id}.json`))
    assert(rows.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`))
    const source = parseCase(
      JSON.parse(new TextDecoder().decode(bytes)),
      entry.spec.id,
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    const binding = bindings.find((binding) => binding.caseId === source.id)
    const candidate = candidateCaseView(entry)
    assertEquals(entry.golden?.spec.candidate.argv, undefined)
    assertEquals(entry.golden?.spec.candidate.graphql, undefined)
    assertEquals(source.graphql, null)
    if (binding == null) {
      assertEquals(entry.spec.deviation, null)
      assertEquals(candidate.spec.expected, source.expected)
    } else {
      assertEquals(entry.spec.deviation?.id, binding.id)
      assertEquals(entry.spec.deviation?.sha256, binding.sha256)
      assertEquals(entry.golden?.spec.approvedSurfaces, binding.surfaces)
      if (source.id === "c004-permissive-inline-rewrite") {
        assertEquals(source.expected.exit, { code: 0 })
        assertEquals(source.expected.stdout, {
          utf8: "Default workspace set to: alpha\n",
        })
        assertEquals(source.expected.fileEffects.length, 1)
        assertEquals(candidate.spec.expected.exit, { code: 1 })
        assertEquals(candidate.spec.expected.stdout, { utf8: "" })
        assertEquals(candidate.spec.expected.fileEffects, [])
      } else {
        assertEquals(candidate.spec.expected.exit, source.expected.exit)
        assertEquals(
          candidate.spec.expected.fileEffects,
          source.expected.fileEffects,
        )
        if (source.id.endsWith("leaf-help")) {
          assertEquals(candidate.spec.expected.stderr, source.expected.stderr)
        }
      }
    }
  }
})
