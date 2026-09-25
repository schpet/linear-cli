import { assert, assertEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c021-frozen-cases/", import.meta.url).pathname
// The 27 cases replayed privately on 2026-09-24, copied byte-for-byte.
const provenanceIds = [
  "c021-closed-after-header",
  "c021-closed-stdout",
  "c021-collation-json",
  "c021-default-json",
  "c021-default-text",
  "c021-empty-json",
  "c021-empty-text",
  "c021-graphql-error",
  "c021-help",
  "c021-json-short",
  "c021-no-color-unset",
  "c021-no-key",
  "c021-number-json",
  "c021-team-ambiguous",
  "c021-team-empty",
  "c021-team-key-text",
  "c021-team-miss-pages",
  "c021-team-name-json",
  "c021-team-uuid-json",
  "c021-transport-error",
  "c021-type-case",
  "c021-type-document",
  "c021-type-invalid",
  "c021-type-issue",
  "c021-type-project",
  "c021-unknown-server-type",
  "c021-wide-text",
]
// Sorted `name + NUL + bytes` over the 27 provenance files.
const provenanceSha256 =
  "e7c2e3fb9dbb15292667452278e5515b01550ebeb1c47f82f56a05eab4052262"
const ids = [
  "c021-broad-widths-text",
  "c021-closed-after-header",
  "c021-closed-mid-count",
  "c021-closed-mid-row",
  "c021-closed-stdout",
  "c021-collation-json",
  "c021-default-json",
  "c021-default-text",
  "c021-empty-json",
  "c021-empty-text",
  "c021-graphql-error",
  "c021-help",
  "c021-hexagram-width-text",
  "c021-http-401",
  "c021-http-500",
  "c021-json-short",
  "c021-lowercase-collation-text",
  "c021-no-color-unset",
  "c021-no-key",
  "c021-number-json",
  "c021-numeric-raw-json",
  "c021-partial-data-error",
  "c021-presentable-error",
  "c021-raw-extra-field",
  "c021-raw-lone-surrogate",
  "c021-raw-missing-null",
  "c021-raw-template-data-invalid",
  "c021-raw-template-data-object",
  "c021-resolve-team-error",
  "c021-team-ambiguous",
  "c021-team-blank-no-key",
  "c021-team-empty",
  "c021-team-key-text",
  "c021-team-miss-no-teams",
  "c021-team-miss-pages",
  "c021-team-name-json",
  "c021-team-unsupported-url",
  "c021-team-url-text",
  "c021-team-uuid-json",
  "c021-team-wrong-url-no-key",
  "c021-transport-error",
  "c021-type-case",
  "c021-type-document",
  "c021-type-invalid",
  "c021-type-issue",
  "c021-type-project",
  "c021-unknown-server-type",
  "c021-wide-text",
  "c021-workspace-env-conflict",
  "c021-workspace-team-url",
  "c021-workspace-unknown",
  "c021-workspace-url-mismatch",
]
const bundleSha256 =
  "3a151cb88494fbd914f4cf41651d6cdd6e59c1969ec39d8b7bbec2c930d4d022"
const getTemplates =
  "query GetTemplates { templates { id name description type icon color hasFormFields lastAppliedAt sortOrder createdAt updatedAt team { id key name } inheritedFrom { id name } creator { id name } templateData } }"

async function files(): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected C021 symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected C021 entry ${name}`)
        found.push(name)
      }
    }
  }
  await walk(root)
  return found.sort()
}

Deno.test("C021 freezes 52 strict template-list cases", async () => {
  const names = await files()
  assertEquals(
    names.filter((name) => name.endsWith(".json")),
    ids.map((id) => `${id}.json`),
  )
  assertEquals(
    names,
    [
      ...ids.map((id) => `${id}.json`),
      "fixtures/workspace-credential/linear/credentials.toml",
    ].sort(),
  )
  const lines: string[] = []
  for (const name of names) {
    lines.push(
      `${name}\0${await sha256Hex(await Deno.readFile(join(root, name)))}\n`,
    )
  }
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    bundleSha256,
  )

  const provenance: number[] = []
  for (const id of provenanceIds) {
    const name = `${id}.json`
    provenance.push(...new TextEncoder().encode(name), 0)
    provenance.push(...await Deno.readFile(join(root, name)))
  }
  assertEquals(
    await sha256Hex(new Uint8Array(provenance)),
    provenanceSha256,
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
  const schemaSha256 = "schemaSha256" in baseline ? baseline.schemaSha256 : null
  if (typeof schemaSha256 !== "string") {
    throw new Error("baseline schemaSha256 is not text")
  }
  assertEquals(
    schemaSha256,
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
  )

  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const loaded = await loadCases(root, routes, "c021-")
  assertEquals(loaded.map((entry) => entry.spec.id), ids)
  let graphqlCases = 0
  let requests = 0
  for (const { spec } of loaded) {
    assertEquals(spec.route, "linear template list")
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.deviation, null)
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.env.PATH, "{{bin}}")
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    if (spec.id !== "c021-no-color-unset") assertEquals(spec.env.NO_COLOR, "1")
    if (spec.graphql == null) continue
    graphqlCases++
    assertEquals(spec.graphql.path, "/graphql")
    assertEquals(spec.graphql.schemaSha256, schemaSha256)
    assertEquals(spec.graphql.groups.length, 1)
    const group = spec.graphql.groups[0]
    if (group.mode !== "ordered") {
      throw new Error("expected ordered GraphQL fixture")
    }
    assertEquals(group.steps.length, spec.graphql.expectedRequests)
    requests += group.steps.length
    group.steps.forEach((step, index) => {
      if (step.kind !== "graphql") throw new Error("expected GraphQL step")
      assertEquals(
        step.identity.authorization,
        spec.id === "c021-workspace-team-url"
          ? "lin_api_fake_beta"
          : "lin_api_fake",
      )
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.identity.headers, {})
      assertEquals(step.effects, [])
      if (step.operation.document.startsWith("query GetTemplates ")) {
        // One variable-free GetTemplates, always the last request.
        assertEquals(step.operation.document, getTemplates)
        assertEquals(step.operation.variables, undefined)
        assertEquals(index, group.steps.length - 1, spec.id)
      }
      if (step.operation.document.startsWith("query ResolveTeam(")) {
        assertEquals(index, 0, spec.id)
        const variables = step.operation.variables
        if (variables == null) throw new Error("ResolveTeam needs variables")
        const uuid = spec.id === "c021-team-uuid-json"
        assertEquals(variables.isUuid, uuid, spec.id)
        assertEquals(variables.id, uuid ? variables.reference : null, spec.id)
      }
    })
  }
  assertEquals(graphqlCases, 41)
  assertEquals(requests, 49)
})
