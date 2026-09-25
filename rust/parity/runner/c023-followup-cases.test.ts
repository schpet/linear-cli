import { assert, assertEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c023-followup-cases/", import.meta.url).pathname
const caseIds = [
  "ambiguous-team",
  "branches-literal",
  "branches-relative",
  "extra-wire-fields",
  "help",
  "missing-team",
  "parent-help",
  "repeat-cursor",
  "short-help",
  "surplus",
  "team-flag-value",
  "truncate-mixed",
  "truncate-zero",
  "unknown-health",
  "unknown-option",
  "url-team",
  "uuid-team",
  "web-env-config",
  "web-env-flag",
  "web-unknown-team",
  "width-table",
]
const fixtureNames = [
  "fixtures/workspace-config/linear.toml",
]
const bundleSha256 =
  "4863010be5df7ad5c760d4c9e300b6ea4c9f373251cd7a147efb09c0151fea30"

async function files(): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected C023F symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected C023F entry ${name}`)
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

Deno.test("C023F freezes 21 strict source-backed project-list follow-ups", async () => {
  const names = caseIds.map((id) => `c023f-${id}.json`)
  const allNames = await files()
  assertEquals(
    allNames,
    [...names, "c023f-inputs.sha256", ...fixtureNames].sort(),
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
  assert(routes.has("linear project list"))
  assert(routes.has("linear project"))
  const loaded = await loadCases(root, routes, "c023f-")
  assertEquals(loaded.map((entry) => `${entry.spec.id}.json`), names)

  const lines: string[] = []
  for (const name of names) {
    const parsed = loaded.find((entry) => `${entry.spec.id}.json` === name)
      ?.spec
    assert(parsed != null)
    assertEquals(parsed.id + ".json", name)
    assertEquals(
      parsed.route,
      parsed.id === "c023f-parent-help"
        ? "linear project"
        : "linear project list",
    )
    assertEquals(parsed.expected.fileEffects, [])
    assertEquals(parsed.deviation, null)
    assertEquals(parsed.env.PATH, "{{bin}}")
    assertEquals(parsed.env.LANG, "C.UTF-8")
    assertEquals(parsed.env.NO_COLOR, "1")
    assert(parsed.timeoutMs <= 30_000)
    if (parsed.graphql != null) {
      assertEquals(parsed.graphql.path, "/graphql")
      assertEquals(
        parsed.graphql.schemaSha256,
        "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
      )
      for (const group of parsed.graphql.groups) {
        assertEquals(group.mode, "ordered")
        if (group.mode !== "ordered") throw new Error("unordered C023F fixture")
        for (const step of group.steps) {
          assertEquals(step.kind, "graphql")
          if (step.kind !== "graphql") continue
          assertEquals(step.identity.authorization, "lin_api_fake")
          assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
          assertEquals(step.identity.headers, {})
          assertEquals(step.effects, [])
          if (step.response.kind === "transport") {
            assertEquals(step.response.status, 200)
            assertEquals(step.response.headers, {
              "content-type": "application/json",
            })
          }
        }
      }
      assertEquals(
        parsed.graphql.expectedRequests,
        parsed.graphql.groups.reduce(
          (total, group) =>
            total + (group.mode === "ordered" ? group.steps.length : 0),
          0,
        ),
      )
    }
    const { expected: _expected, deviation: _deviation, ...input } = parsed
    const digest = await sha256Hex(new TextEncoder().encode(canonical(input)))
    lines.push(`${digest}  ${name}`)
  }
  assertEquals(
    loaded.filter((entry) =>
      entry.spec.graphql?.groups.some((group) =>
        group.mode === "ordered" &&
        group.steps.some((step) =>
          step.kind === "graphql" && step.response.kind === "transport"
        )
      )
    ).map((entry) => entry.spec.id),
    ["c023f-extra-wire-fields", "c023f-unknown-health", "c023f-uuid-team"],
  )
  for (const name of fixtureNames) {
    lines.push(
      `${await sha256Hex(await Deno.readFile(join(root, name)))}  ${name}`,
    )
  }
  assertEquals(
    lines.join("\n") + "\n",
    await Deno.readTextFile(join(root, "c023f-inputs.sha256")),
  )
})
