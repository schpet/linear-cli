import { nativeParserContract } from "./native-parser-contract.ts"
import { assert, assertEquals } from "@std/assert"
import { dirname, join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { RUST_CONTRACT } from "./schema.ts"

const root = new URL("./cases/", import.meta.url).pathname
const oldRoot = new URL("./r02c2-frozen-cases/", import.meta.url).pathname
const ids = [
  "r02c2-absent-version",
  "r02c2-bom-inline-help",
  "r02c2-inline-invalid-default-version",
  "r02c2-inline-version",
  "r02c2-invalid-default-help",
  "r02c2-malformed-config-and-credential-help",
  "r02c2-malformed-credential-and-bad-argv",
  "r02c2-malformed-help",
  "r02c2-metadata-missing-color-help",
  "r02c2-metadata-missing-help",
]
const fixtureHashes = new Map<string, string>([
  [
    "fixtures/r02c2-bom-inline/linear/credentials.toml",
    "ff16133cf30d380b6ac63342ab61d962a56b5836e38d2f018fed6560b49cea29",
  ],
  [
    "fixtures/r02c2-both-malformed/linear/credentials.toml",
    "9b48a3b05114031ed226a5939b8846b077b5f995cc224d74fd0cb9a4ae8c9f92",
  ],
  [
    "fixtures/r02c2-both-malformed/linear/linear.toml",
    "2fe3031e3700bae99f1061d8e1d86a16fe52e2140b514dd94549ca8f0b1ae108",
  ],
  [
    "fixtures/r02c2-inline/linear/credentials.toml",
    "bb27a2468ac23a997d81eef7dc2aba74930c5ac039b4d1cdcb9c8713bdcd7610",
  ],
  [
    "fixtures/r02c2-inline-invalid-default/linear/credentials.toml",
    "66f3289c94ca41640a2ffde9f2ecfe3693f57e4229966f528b0ca722023059a3",
  ],
  [
    "fixtures/r02c2-invalid-default/linear/credentials.toml",
    "17e4fa1e7d35d708b78a6f766e73122746e12946c45ee5184eeb73763c62481f",
  ],
  [
    "fixtures/r02c2-malformed/linear/credentials.toml",
    "9b48a3b05114031ed226a5939b8846b077b5f995cc224d74fd0cb9a4ae8c9f92",
  ],
  [
    "fixtures/r02c2-metadata/linear/credentials.toml",
    "60d2227690f27be67dc4957ef860d69a052f749602749f867dcd5ac39a653aaa",
  ],
])
const fileNames = [
  ...ids.map((id) => `${id}.json`),
  ...fixtureHashes.keys(),
].sort()
const frozenProjectionSha256 =
  "27f184039da7c857e65ae04b878b1bdc93a49a4943545df693b4a4244f5ba310"
const deviationId = "R02C2G-CREDENTIAL-STARTUP"

async function fixtureFileNames(corpusRoot: string): Promise<string[]> {
  const names: string[] = []
  async function walk(relativeDir: string): Promise<void> {
    for await (const entry of Deno.readDir(join(corpusRoot, relativeDir))) {
      const relativeName = `${relativeDir}/${entry.name}`
      if (entry.isDirectory) {
        await walk(relativeName)
      } else {
        assert(entry.isFile, `unexpected fixture entry ${relativeName}`)
        names.push(relativeName)
      }
    }
  }
  for await (const entry of Deno.readDir(join(corpusRoot, "fixtures"))) {
    if (!entry.name.startsWith("r02c2-")) continue
    assert(entry.isDirectory, `unexpected fixture root ${entry.name}`)
    await walk(`fixtures/${entry.name}`)
  }
  return names.sort()
}

function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical)
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).sort(([left], [right]) =>
        left < right ? -1 : left > right ? 1 : 0
      )
        .map(([key, entry]) => [key, canonical(entry)]),
    )
  }
  return value
}

/** Frozen projection: sorted relative path lines containing SHA-256 of each
 * raw fixture, or of sorted-key JSON for a case with only `deviation` omitted. */
async function projectionSha256(corpusRoot: string): Promise<string> {
  const lines: string[] = []
  for (const name of fileNames) {
    const raw = await Deno.readFile(join(corpusRoot, name))
    let content = raw
    if (name.endsWith(".json")) {
      const parsed: unknown = JSON.parse(new TextDecoder().decode(raw))
      if (
        parsed == null || typeof parsed !== "object" || Array.isArray(parsed)
      ) {
        throw new Error(`invalid frozen case ${name}`)
      }
      const withoutBinding = Object.fromEntries(
        Object.entries(parsed).filter(([key]) => key !== "deviation"),
      )
      content = new TextEncoder().encode(
        JSON.stringify(canonical(withoutBinding)),
      )
    } else {
      assertEquals(
        await sha256Hex(raw),
        fixtureHashes.get(name),
      )
    }
    lines.push(`${await sha256Hex(content)}  ${name}\n`)
  }
  return await sha256Hex(new TextEncoder().encode(lines.join("")))
}

Deno.test("R02C2G moves ten frozen cases without changing Deno fields or fixture bytes", async () => {
  assertEquals(await projectionSha256(root), frozenProjectionSha256)
  assertEquals(await fixtureFileNames(root), [...fixtureHashes.keys()].sort())
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const loaded = await loadCases(root, routes, "r02c2-", RUST_CONTRACT)
  assertEquals(loaded.map((item) => item.spec.id), ids)
  const surfaces = new Map<string, string[]>([
    ["r02c2-absent-version", ["stdout"]],
    ["r02c2-inline-version", ["stdout"]],
    ["r02c2-inline-invalid-default-version", ["stdout"]],
    ["r02c2-metadata-missing-help", ["stdout", "stderr"]],
    ["r02c2-metadata-missing-color-help", ["stdout", "stderr"]],
    ["r02c2-invalid-default-help", ["stdout", "stderr"]],
    ["r02c2-malformed-help", ["stderr"]],
    ["r02c2-malformed-credential-and-bad-argv", ["stderr"]],
    ["r02c2-malformed-config-and-credential-help", ["stderr"]],
    ["r02c2-bom-inline-help", ["stderr"]],
  ])
  for (const item of loaded) {
    assertEquals(item.spec.env.PATH, "{{bin}}", item.spec.id)
    assertEquals(item.spec.fixtureServer, null, item.spec.id)
    assertEquals(item.spec.graphql, undefined, item.spec.id)
    assertEquals(
      item.spec.deviation?.id,
      nativeParserContract(item.spec.id)?.[0] ?? deviationId,
      item.spec.id,
    )
    assertEquals(
      item.golden?.spec.approvedSurfaces,
      nativeParserContract(item.spec.id)?.[1] ?? surfaces.get(item.spec.id),
      item.spec.id,
    )
  }
  const old = await Deno.stat(oldRoot).catch(() => null)
  if (old != null) {
    for await (const entry of Deno.readDir(oldRoot)) {
      assert(
        !entry.isFile && !entry.isDirectory,
        `leftover dedicated evidence ${entry.name}`,
      )
    }
  }
})

Deno.test("R02C2G projection catches a changed frozen field in scratch", async () => {
  const scratch = await Deno.makeTempDir({
    prefix: "linear-r02c2g-projection-",
  })
  try {
    for (const name of fileNames) {
      const target = join(scratch, name)
      await Deno.mkdir(dirname(target), { recursive: true })
      await Deno.copyFile(join(root, name), target)
    }
    assertEquals(await projectionSha256(scratch), frozenProjectionSha256)
    assertEquals(
      await fixtureFileNames(scratch),
      [...fixtureHashes.keys()].sort(),
    )
    const target = join(scratch, "r02c2-absent-version.json")
    const parsed = JSON.parse(await Deno.readTextFile(target))
    parsed.expected.stdout.utf8 = "different frozen version\n"
    await Deno.writeTextFile(target, JSON.stringify(parsed))
    assert((await projectionSha256(scratch)) !== frozenProjectionSha256)
    const extra = join(scratch, "fixtures/r02c2-malformed/linear/extra.toml")
    await Deno.writeTextFile(extra, "synthetic = true\n")
    assert(
      JSON.stringify(await fixtureFileNames(scratch)) !==
        JSON.stringify([...fixtureHashes.keys()].sort()),
    )
  } finally {
    await Deno.remove(scratch, { recursive: true })
  }
})
