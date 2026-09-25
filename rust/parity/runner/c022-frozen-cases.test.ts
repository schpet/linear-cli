import { assert, assertEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c022-frozen-cases/", import.meta.url).pathname
const ids = [
  "c022-alias-help",
  "c022-alias-json",
  "c022-ambiguous-name",
  "c022-ambiguous-workspace",
  "c022-bad-option",
  "c022-closed-error-stdout",
  "c022-closed-stdout",
  "c022-closed-text-stdout",
  "c022-date-future",
  "c022-date-invalid",
  "c022-date-los-angeles",
  "c022-date-only",
  "c022-date-only-los-angeles",
  "c022-duplicate-index-keys",
  "c022-empty-array",
  "c022-empty-name",
  "c022-empty-object",
  "c022-extra-arg",
  "c022-help",
  "c022-http-401",
  "c022-http-500",
  "c022-index-boundaries",
  "c022-item-label-both",
  "c022-item-label-empty-title",
  "c022-json-bad-prosemirror",
  "c022-json-before-reference",
  "c022-json-extra-field",
  "c022-json-invalid-inner",
  "c022-json-missing-name",
  "c022-json-nested-bad-prosemirror",
  "c022-json-null-outer",
  "c022-json-null-template",
  "c022-json-object-outer",
  "c022-list-graphql-error",
  "c022-lone-surrogate",
  "c022-markdown-escapes",
  "c022-metadata-custom-type",
  "c022-metadata-form-inherited",
  "c022-missing-arg",
  "c022-missing-uuid",
  "c022-missing-uuid-http500",
  "c022-missing-uuid-presentable",
  "c022-missing-uuid-second",
  "c022-missing-uuid-upper-http400",
  "c022-multiline-crlf",
  "c022-name-casefold",
  "c022-name-json",
  "c022-name-no-key",
  "c022-name-not-found",
  "c022-name-suggestion-dedup",
  "c022-name-text",
  "c022-name-unicode-casefold",
  "c022-nested-rich-error-first",
  "c022-nested-rich-last",
  "c022-no-color-empty",
  "c022-no-color-unset",
  "c022-no-templates",
  "c022-non-json-response",
  "c022-number-infinity",
  "c022-number-large",
  "c022-number-notation",
  "c022-one-item",
  "c022-partial-data-error",
  "c022-plural-mixed-items",
  "c022-presentable-unrelated-error",
  "c022-priority-negative-zero",
  "c022-priority-five",
  "c022-priority-string",
  "c022-prosemirror-invalid-mark",
  "c022-prosemirror-invalid-node",
  "c022-prosemirror-invalid-root",
  "c022-prosemirror-unknown-node",
  "c022-rich-nonobject",
  "c022-short-help",
  "c022-string-array-empty",
  "c022-text-invalid-inner",
  "c022-text-missing-name",
  "c022-text-null-outer",
  "c022-text-null-template",
  "c022-text-object-outer",
  "c022-transport-refused",
  "c022-unrelated-graphql-error",
  "c022-uppercase-uuid-not-found",
  "c022-url-bad-option",
  "c022-url-help",
  "c022-url-no-key",
  "c022-url-unknown-workspace",
  "c022-url-with-key",
  "c022-url-workspace-conflict",
  "c022-uuid-json",
  "c022-uuid-no-key",
  "c022-uuid-short-json",
  "c022-uuid-text",
  "c022-uuid-uppercase",
  "c022-whitespace-lines",
  "c022-wide-rich-body",
  "c022-workspace-default",
  "c022-workspace-env-conflict",
  "c022-workspace-leaf",
  "c022-workspace-missing-value",
  "c022-workspace-selected",
  "c022-workspace-unknown",
]
const BUNDLE_SHA =
  "e31c67f09bb13ce361737ba3a3659e545a843becbad9b6d49ff8b2c68eae3869"
const GRAPHQL_COUNT = 84

const getOne =
  "query GetTemplate($id: String!) { template(id: $id) { id name description type icon color hasFormFields lastAppliedAt sortOrder createdAt updatedAt team { id key name } inheritedFrom { id name } creator { id name } templateData } }"
const getAll =
  "query GetTemplates { templates { id name description type icon color hasFormFields lastAppliedAt sortOrder createdAt updatedAt team { id key name } inheritedFrom { id name } creator { id name } templateData } }"

async function files(): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected C022 symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected C022 entry ${name}`)
        found.push(name)
      }
    }
  }
  await walk(root)
  return found.sort()
}

Deno.test("C022 freezes 102 strict template-view cases", async () => {
  const names = await files()
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
    BUNDLE_SHA,
  )

  const baseline: unknown = JSON.parse(
    await Deno.readTextFile(
      new URL("../baseline.json", import.meta.url),
    ),
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
  assertEquals(
    "schemaSha256" in baseline && baseline.schemaSha256,
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
  )
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(
      new URL("../manifest.json", import.meta.url),
    ),
  ))
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  assert(routes.has("linear template view"))
  const loaded = await loadCases(root, routes, "c022-")
  assertEquals(
    loaded.map((entry) => `${entry.spec.id}.json`),
    ids.map((id) => `${id}.json`).sort(),
  )
  let graphqlCases = 0
  for (const { spec } of loaded) {
    assertEquals(spec.route, "linear template view")
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.deviation, null)
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(
      spec.env.TZ === "UTC" || spec.env.TZ === "America/Los_Angeles",
      true,
    )
    assertEquals(spec.env.LANG, "C.UTF-8")
    if (spec.id === "c022-no-color-unset") {
      assertEquals("NO_COLOR" in spec.env, false)
    } else {
      assertEquals(spec.env.NO_COLOR === "1" || spec.env.NO_COLOR === "", true)
    }
    const configCases = new Set([
      "c022-workspace-default",
      "c022-workspace-env-conflict",
      "c022-workspace-leaf",
      "c022-workspace-selected",
      "c022-workspace-unknown",
      "c022-url-unknown-workspace",
      "c022-url-workspace-conflict",
    ])
    assertEquals(
      spec.configFixture ?? null,
      configCases.has(spec.id) ? "workspace-credential" : null,
    )
    if (spec.graphql == null) continue
    graphqlCases++
    assertEquals(spec.graphql.path, "/graphql")
    assertEquals(
      spec.graphql.schemaSha256,
      "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
    )
    assertEquals(spec.graphql.expectedRequests, 1)
    assertEquals(spec.graphql.groups.length, 1)
    const group = spec.graphql.groups[0]
    if (group.mode !== "ordered") {
      throw new Error(`C022 ${spec.id}: expected ordered fixture`)
    }
    assertEquals(group.steps.length, 1)
    const step = group.steps[0]
    if (step.kind !== "graphql") {
      throw new Error(`C022 ${spec.id}: expected GraphQL request`)
    }
    assertEquals(
      step.identity.authorization,
      spec.id === "c022-workspace-selected" || spec.id === "c022-workspace-leaf"
        ? "lin_api_fake_beta"
        : spec.id === "c022-workspace-default"
        ? "lin_api_fake_alpha"
        : "lin_api_fake",
    )
    assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
    assertEquals(step.identity.headers, {})
    assertEquals(step.effects, [])
    if (
      spec.id === "c022-json-null-template" ||
      spec.id === "c022-text-null-template" ||
      spec.id === "c022-text-missing-name"
    ) {
      // These raw responses deliberately violate the GraphQL schema. The
      // fixture still checks the exact request, then returns literal HTTP JSON.
      assertEquals(step.response.kind, "transport")
      if (step.response.kind !== "transport") {
        throw new Error(`C022 ${spec.id}: expected raw transport response`)
      }
      assertEquals(step.response.status, 200)
      assertEquals(step.response.headers, {
        "content-type": "application/json",
      })
      if (!("utf8" in step.response.body)) {
        throw new Error(`C022 ${spec.id}: expected raw UTF-8 response`)
      }
      if (spec.id === "c022-text-missing-name") {
        assert(step.response.body.utf8.includes('"templateData":'))
        assert(!step.response.body.utf8.includes('"name":"Bug report"'))
      } else {
        assertEquals(step.response.body.utf8, '{"data":{"template":null}}')
      }
    }
    if (step.operation.document === getOne) {
      assertEquals(Object.keys(step.operation.variables ?? {}), ["id"])
      assertEquals(typeof step.operation.variables?.id, "string")
      const reference = spec.argv.find((arg) =>
        /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
          arg,
        )
      )
      assertEquals(step.operation.variables?.id, reference)
    } else {
      assertEquals(step.operation.document, getAll)
      assertEquals(step.operation.variables, undefined)
    }
  }
  assertEquals(graphqlCases, GRAPHQL_COUNT)
})
