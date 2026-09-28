import { assert, assertEquals, assertNotEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { parse, print } from "graphql"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c030-frozen-cases/", import.meta.url).pathname
const repo = new URL("../../../", import.meta.url).pathname
const fixtureNames = [
  "fixtures/api-key-config/linear.toml",
  "fixtures/malformed-credential/linear/credentials.toml",
  "fixtures/workspace-config/linear.toml",
  "fixtures/workspace-credential/linear/credentials.toml",
]
const bundleSha256 =
  "0f45295e43a691d24e83f79939e666a537486c3f1535c58d7ae858cda3b0597c"
const milestoneOperation = "GetProjectMilestones"
const lookupOperations = ["GetProjectIdByName", "GetProjectIdBySlugId"]

async function files(): Promise<string[]> {
  const found: string[] = []
  async function walk(dir: string): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = join(dir, entry.name)
      const name = relative(root, path)
      assert(!entry.isSymlink, `unexpected C030 symlink ${name}`)
      if (entry.isDirectory) await walk(path)
      else {
        assert(entry.isFile, `unexpected C030 entry ${name}`)
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
    return `{${
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
        .map(([key, entry]) => `${JSON.stringify(key)}:${canonical(entry)}`)
        .join(",")
    }}`
  }
  return JSON.stringify(value)
}

async function bundleDigest(bytes: Map<string, Uint8Array>): Promise<string> {
  const lines: string[] = []
  for (const name of [...bytes.keys()].sort()) {
    const content = bytes.get(name)
    assert(content != null)
    lines.push(`${name}\0${await sha256Hex(content)}\n`)
  }
  return await sha256Hex(new TextEncoder().encode(lines.join("")))
}

/** Canonical digest of a case with its expectation and deviation removed. */
async function inputDigest(bytes: Uint8Array): Promise<string> {
  const parsed: unknown = JSON.parse(new TextDecoder().decode(bytes))
  if (typeof parsed !== "object" || parsed == null || Array.isArray(parsed)) {
    throw new Error("C030 case is not an object")
  }
  const input = Object.fromEntries(
    Object.entries(parsed).filter(([key]) =>
      key !== "expected" && key !== "deviation"
    ),
  )
  return await sha256Hex(new TextEncoder().encode(canonical(input)))
}

/** The exact query text a source file declares for one named operation. */
async function sourceDocument(path: string, name: string): Promise<string> {
  const text = await Deno.readTextFile(join(repo, path))
  const start = text.indexOf(`query ${name}(`)
  assert(start >= 0, `${path} declares no ${name}`)
  const end = text.indexOf("`", start)
  assert(end > start, `${path} ${name} is not a template literal`)
  return print(parse(text.slice(start, end)))
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value == null || Array.isArray(value)) {
    throw new Error("expected a JSON object")
  }
  return Object.fromEntries(Object.entries(value))
}

function replaceOnce(text: string, from: string, to: string): string {
  const index = text.indexOf(from)
  assert(index >= 0, `mutation anchor missing: ${from}`)
  assertEquals(text.indexOf(from, index + 1), -1, `ambiguous anchor ${from}`)
  return text.slice(0, index) + to + text.slice(index + from.length)
}

Deno.test("C030 freezes 93 strict milestone-list Deno cases", async () => {
  const allNames = await files()
  const names = allNames.filter((name) => /^c030-.*\.json$/.test(name))
  assertEquals(names.length, 93)
  assertEquals(
    allNames,
    [...names, "c030-inputs.sha256", ...fixtureNames].sort(),
  )
  const bytes = new Map<string, Uint8Array>()
  for (const name of allNames) {
    bytes.set(name, await Deno.readFile(join(root, name)))
  }
  assertEquals(await bundleDigest(bytes), bundleSha256)

  const baseline = record(JSON.parse(
    await Deno.readTextFile(new URL("../baseline.json", import.meta.url)),
  ))
  assertEquals(
    baseline.referenceRevision,
    "d4fe6fa7358f018fd1da0c6b96ec2b022247e898",
  )
  assertEquals(
    baseline.binarySha256,
    "a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835",
  )
  assertEquals(
    baseline.lockSha256,
    "3da729da08fe6d48236e055b2eaac95788b5e5ccfd0f66dacdc5f6a0b0b96403",
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
  assert(routes.has("linear milestone list"))
  assert(routes.has("linear milestone"))

  // Every fixture document is the source declaration, token for token.
  const documents = new Map<string, string>([
    [
      milestoneOperation,
      await sourceDocument(
        "src/commands/milestone/milestone-list.ts",
        milestoneOperation,
      ),
    ],
    ...await Promise.all(
      lookupOperations.map(async (name): Promise<[string, string]> => [
        name,
        await sourceDocument("src/utils/linear.ts", name),
      ]),
    ),
  ])

  const loaded = await loadCases(root, routes, "c030-")
  assertEquals(loaded.map((entry) => `${entry.spec.id}.json`), names)
  const withGraphql = loaded.filter((entry) => entry.spec.graphql != null)
  assertEquals(withGraphql.length, 69)
  assertEquals(
    withGraphql.reduce(
      (sum, entry) => sum + (entry.spec.graphql?.expectedRequests ?? 0),
      0,
    ),
    101,
  )
  assertEquals(
    loaded.filter((entry) =>
      entry.spec.graphql?.groups.some((group) =>
        group.mode === "ordered" &&
        group.steps.some((step) =>
          step.kind === "graphql" && step.response.kind === "transport"
        )
      )
    ).map((entry) => entry.spec.id),
    [
      "c030-extra-wire-json",
      "c030-missing-nested-project-id-text",
      "c030-missing-outer-project-fields",
      "c030-missing-sortorder-json",
      "c030-missing-sortorder-text",
      "c030-name-lookup-null-projects",
      "c030-null-connection",
      "c030-null-empty-date-json",
      "c030-null-empty-date-text",
      "c030-null-nodes",
      "c030-null-root-first",
      "c030-null-root-later",
      "c030-null-root-upper",
      "c030-null-root-url",
      "c030-number-targetdate-single-json",
      "c030-number-targetdate-text",
      "c030-reordered-json",
      "c030-second-page-http-error",
      "c030-slug-lookup-http-error",
      "c030-sortorder-numbers-json",
      "c030-sortorder-overflow-json",
      "c030-sortorder-overflow-text",
      "c030-targetdate-wrong-type-object",
    ],
  )
  assertEquals(
    loaded.filter((entry) => "mode" in entry.spec.expected.stdout)
      .map((entry) => entry.spec.id),
    ["c030-closed-stdout-json", "c030-closed-stdout-text"],
  )

  const lines: string[] = []
  for (const name of names) {
    const parsed = loaded.find((entry) => `${entry.spec.id}.json` === name)
      ?.spec
    assert(parsed != null)
    assertEquals(parsed.id + ".json", name)
    assertEquals(
      parsed.route,
      name === "c030-parent-help.json"
        ? "linear milestone"
        : "linear milestone list",
    )
    assertEquals(parsed.expected.fileEffects, [])
    assertEquals(parsed.deviation, null)
    assertEquals(parsed.timeoutMs, 30000)
    assertEquals(parsed.outputCapBytes, 4194304)
    assertEquals(parsed.fixtureServer, null)
    assertEquals(parsed.env.PATH, "{{bin}}")
    assertEquals(parsed.env.LANG, "C.UTF-8")
    assertEquals(parsed.env.TZ, "UTC")
    assertEquals(parsed.env.LINEAR_IGNORE_ENV_FILE, "1")
    assert(
      parsed.env.LINEAR_API_KEY == null ||
        parsed.env.LINEAR_API_KEY === "lin_api_fake",
    )
    assertEquals(
      parsed.env.NO_COLOR,
      name === "c030-no-color-empty-text.json"
        ? ""
        : name.startsWith("c030-no-color-unset-")
        ? undefined
        : "1",
    )
    if (parsed.graphql == null) {
      assert(
        parsed.env.LINEAR_GRAPHQL_ENDPOINT == null ||
          parsed.env.LINEAR_GRAPHQL_ENDPOINT === "http://127.0.0.1:1/graphql",
      )
    } else {
      assertEquals(
        parsed.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:{{fixturePort}}/graphql",
      )
      assertEquals(parsed.graphql.path, "/graphql")
      assertEquals(
        parsed.graphql.schemaSha256,
        "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
      )
      assertEquals(parsed.graphql.initialRecords, {})
      assertEquals(parsed.graphql.expectedRecords, {})
      assertEquals(parsed.graphql.groups.length, 1)
      const [group] = parsed.graphql.groups
      if (group.mode !== "ordered") {
        throw new Error(`C030 ${parsed.id}: expected ordered fixture`)
      }
      // Milestone pages: first=100 on every page, first-page `after`
      // omitted, later pages chained to the previous page's endCursor.
      let previousCursor: unknown = undefined
      let milestonePages = 0
      for (const step of group.steps) {
        if (step.kind !== "graphql") {
          throw new Error(`C030 ${parsed.id}: unexpected asset step`)
        }
        const authorization = step.identity.authorization
        assert(
          authorization != null && [
            "lin_api_fake",
            "lin_api_fake_alpha",
            "lin_api_fake_beta",
            "lin_api_fake_config",
          ].includes(authorization),
        )
        assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
        assertEquals(step.identity.headers, {})
        assertEquals(step.effects, [])
        const document = print(parse(step.operation.document))
        const operation = [...documents].find(([, source]) =>
          source === document
        )?.[0]
        assert(operation != null, `${parsed.id} ${step.id}: foreign document`)
        const variables = record(step.operation.variables)
        if (operation !== milestoneOperation) continue
        assertEquals(variables.first, 100)
        if (milestonePages === 0) assert(!("after" in variables))
        else assertEquals(variables.after, previousCursor)
        milestonePages++
        previousCursor = step.response.kind === "data"
          ? record(
            record(
              record(record(step.response.data).project)
                .projectMilestones,
            ).pageInfo,
          ).endCursor
          : undefined
      }
    }
    lines.push(
      `${await inputDigest(bytes.get(name) ?? new Uint8Array())}  ${name}`,
    )
  }
  for (const name of fixtureNames) {
    lines.push(
      `${await sha256Hex(bytes.get(name) ?? new Uint8Array())}  ${name}`,
    )
  }
  const inputs = lines.join("\n") + "\n"
  assertEquals(
    inputs,
    await Deno.readTextFile(join(root, "c030-inputs.sha256")),
  )

  // Mutation controls: each in-memory edit must move the pinned bundle digest;
  // input edits must also move that case's canonical input digest, while an
  // expectation-only edit must leave the input digest alone.
  const decoder = new TextDecoder()
  const encoder = new TextEncoder()
  const mutations: Array<{
    file: string
    from: string
    to: string
    inputChanges: boolean
  }> = [
    {
      file: "c030-uuid-lower.json",
      from: '"--project",\n    "3b9a5c7e',
      to: '"--project",\n    "4b9a5c7e',
      inputChanges: true,
    },
    {
      file: "c030-one-page-text.json",
      from: "Launch 00000000",
      to: "Launch 00000001",
      inputChanges: false,
    },
    {
      file: "c030-two-pages-json.json",
      from: '"after": "cursor-1"',
      to: '"after": "cursor-2"',
      inputChanges: true,
    },
    {
      file: "c030-null-root-first.json",
      from: '{\\"data\\":{\\"project\\":null}}',
      to: '{\\"data\\":{\\"project\\":{}}}',
      inputChanges: true,
    },
    {
      file: "fixtures/workspace-credential/linear/credentials.toml",
      from: 'default = "alpha"',
      to: 'default = "beta"',
      inputChanges: true,
    },
  ]
  for (const mutation of mutations) {
    const original = bytes.get(mutation.file)
    assert(original != null, `mutation target ${mutation.file} missing`)
    const changed = encoder.encode(
      replaceOnce(decoder.decode(original), mutation.from, mutation.to),
    )
    const mutated = new Map(bytes)
    mutated.set(mutation.file, changed)
    assertNotEquals(await bundleDigest(mutated), bundleSha256, mutation.file)
    const before = mutation.file.endsWith(".json")
      ? await inputDigest(original)
      : await sha256Hex(original)
    const after = mutation.file.endsWith(".json")
      ? await inputDigest(changed)
      : await sha256Hex(changed)
    assertEquals(before !== after, mutation.inputChanges, mutation.file)
  }
})
