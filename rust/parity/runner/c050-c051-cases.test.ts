import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"

Deno.test("C050/C051 binds all34 frozen document contracts and only named help/title boundaries", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const directory = join(root, "c050-c051-frozen-cases")
  const pins = await Deno.readFile(join(directory, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "7f71eab7cf7014cf9e16461809406a37abf9eb07e73251e646c653cbf2a17dba",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 34)
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => /^c05[01]-/.test(entry.spec.id))
  assertEquals(entries.length, 34)
  assertEquals(
    entries.filter((entry) => entry.spec.route === "linear document list")
      .length,
    15,
  )
  assertEquals(
    entries.filter((entry) => entry.spec.route === "linear document view")
      .length,
    19,
  )
  const boundaryPins = new Map<string, string>([
    [
      "c050-leaf-help",
      "72e985d88078718b8963bbea1b6ac66d003558eb68070847195871d0b88a7c8a",
    ],
    [
      "c051-leaf-help",
      "0a1c2a9fcafc4ebbb74a49767860c958138238c6346b418d8b67667d3463d722",
    ],
    [
      "c050-missing-required-title",
      "c1df7e8e55d0e10fc16f8032da4cf9e2bfd084dd3dbe07ce641c8aa8b8daf418",
    ],
    [
      "c051-missing-required-title",
      "49f645a5592528f2e2c4fa25f79992675359f98ea25abb52aca40b82668b337d",
    ],
  ])
  for (const entry of entries) {
    const bytes = await Deno.readFile(join(directory, `${entry.spec.id}.json`))
    assert(rows.includes(`${await sha256Hex(bytes)}  ${entry.spec.id}.json`))
    const source = parseCase(
      JSON.parse(new TextDecoder().decode(bytes)),
      entry.spec.id,
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    const candidate = candidateCaseView(entry)
    if (boundaryPins.has(source.id)) {
      assertEquals(entry.spec.deviation?.sha256, boundaryPins.get(source.id))
    }
    assertEquals(entry.golden?.spec.candidate.argv, undefined)
    assertEquals(entry.golden?.spec.candidate.graphql, undefined)
    assertEquals(
      candidate.spec.expected.fileEffects,
      source.expected.fileEffects,
    )
    if (source.graphql != null) {
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        RUST_USER_AGENT,
      )
    }
    if (source.id.endsWith("leaf-help")) {
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(entry.golden?.spec.approvedSurfaces, ["stdout"])
      assertEquals(candidate.spec.expected.exit, source.expected.exit)
      assertEquals(candidate.spec.expected.stderr, source.expected.stderr)
    } else if (source.id.endsWith("missing-required-title")) {
      assertEquals(entry.spec.deviation?.id, "C050-C051-STRICT-DOCUMENT-TITLE")
      assertEquals(entry.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
        "graphql-user-agent",
      ])
      assertEquals(source.expected.exit, { code: 0 })
      assertEquals(candidate.spec.expected.exit, { code: 1 })
      assertEquals(candidate.spec.expected.stdout, { utf8: "" })
    } else {
      assertEquals(candidate.spec.expected, source.expected)
      assertEquals(
        entry.golden?.spec.approvedSurfaces,
        source.graphql == null ? undefined : ["graphql-user-agent"],
      )
      assertEquals(
        entry.spec.deviation?.id ?? null,
        source.graphql == null ? null : "R01H-GRAPHQL-UA",
      )
    }
  }
})

Deno.test("C050/C051 pins the25 actual interpreted Markdown contracts, POSIX boundaries and codec bytes", async () => {
  const root =
    new URL("./c050-c051-helper-contracts/", import.meta.url).pathname
  const pins = await Deno.readFile(join(root, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "fcc594ea54a0e312fa15dc2b02043d1a4412728498ea4e34701180ad432800b9",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 5)
  for (const row of rows) {
    const [hash, filename] = row.split("  ")
    assertEquals(
      await sha256Hex(await Deno.readFile(join(root, filename))),
      hash,
    )
  }
  for (
    const { filename, count } of [
      { filename: "markdown-source-observations-expanded.json", count: 18 },
      { filename: "markdown-source-design-observations.json", count: 6 },
      { filename: "markdown-source-adjacent-task-observations.json", count: 1 },
      { filename: "codec-byte-fixtures.json", count: 3 },
    ]
  ) {
    const raw: unknown = JSON.parse(
      await Deno.readTextFile(join(root, filename)),
    )
    assert(Array.isArray(raw))
    assertEquals(raw.length, count)
  }
})
