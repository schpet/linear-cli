import { assert, assertEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c024-frozen-cases/", import.meta.url).pathname
const fixtureNames = ["fixtures/workspace-config/linear.toml"]
const bundleSha256 =
  "434fc53620bd9013f6a0351b0ce6dc29248899950df96861833821d8501a745c"
const caseCount = 54

async function files(): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected C024 symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected C024 entry ${name}`)
        found.push(name)
      }
    }
  }
  await walk(root)
  return found.sort()
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return "{" +
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
        .map(([key, entry]) => `${JSON.stringify(key)}:${canonical(entry)}`)
        .join(",") +
      "}"
  }
  return JSON.stringify(value)
}

Deno.test("C024 freezes strict project-view Deno inputs and outputs", async () => {
  const allNames = await files()
  const names = allNames.filter((name) => /^c024-.*\.json$/.test(name))
  assertEquals(names.length, caseCount)
  assertEquals(
    allNames,
    [...names, "c024-inputs.sha256", ...fixtureNames].sort(),
  )
  const bundleLines: string[] = []
  for (const name of allNames) {
    bundleLines.push(
      `${name}\0${await sha256Hex(await Deno.readFile(join(root, name)))}\n`,
    )
  }
  assertEquals(
    await sha256Hex(new TextEncoder().encode(bundleLines.join(""))),
    bundleSha256,
  )

  const baseline: unknown = JSON.parse(
    await Deno.readTextFile(new URL("../baseline.json", import.meta.url)),
  )
  if (typeof baseline !== "object" || baseline == null) {
    throw new Error("baseline is not an object")
  }
  assertEquals(
    "referenceRevision" in baseline && baseline.referenceRevision,
    "d4fe6fa7358f018fd1da0c6b96ec2b022247e898",
  )
  assertEquals(
    "binarySha256" in baseline && baseline.binarySha256,
    "a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835",
  )
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const routes = new Set<string>(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  assert(routes.has("linear project view"))
  const loaded = await loadCases(root, routes, "c024-")
  assertEquals(loaded.map((entry) => `${entry.spec.id}.json`), names)

  const lines: string[] = []
  for (const name of names) {
    const parsed = loaded.find((entry) => `${entry.spec.id}.json` === name)
      ?.spec
    assert(parsed != null)
    assertEquals(parsed.id + ".json", name)
    assertEquals(parsed.route, "linear project view")
    assertEquals(parsed.expected.fileEffects, [])
    assertEquals(parsed.deviation, null)
    assertEquals(parsed.env.PATH, "{{bin}}")
    assertEquals(parsed.env.LANG, "C.UTF-8")
    assertEquals(parsed.env.NO_COLOR, "1")
    if (parsed.graphql != null) {
      assertEquals(parsed.graphql.path, "/graphql")
      assertEquals(
        parsed.graphql.schemaSha256,
        "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
      )
      for (const group of parsed.graphql.groups) {
        assertEquals(group.mode, "ordered")
        if (group.mode !== "ordered") {
          throw new Error(`C024 ${parsed.id}: expected ordered fixture`)
        }
        for (const step of group.steps) {
          assertEquals(step.kind, "graphql")
          if (step.kind !== "graphql") continue
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          assertEquals(step.identity.headers, {})
          assertEquals(step.effects, [])
        }
      }
    }
    const { expected: _expected, deviation: _deviation, ...input } = parsed
    const digest = await sha256Hex(new TextEncoder().encode(canonical(input)))
    lines.push(`${digest}  ${name}`)
  }
  for (const name of fixtureNames) {
    const digest = await sha256Hex(await Deno.readFile(join(root, name)))
    lines.push(`${digest}  ${name}`)
  }
  assertEquals(
    lines.join("\n") + "\n",
    await Deno.readTextFile(join(root, "c024-inputs.sha256")),
  )
})
