import { assert, assertEquals, assertNotEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { parse, print } from "graphql"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c037-frozen-cases/", import.meta.url).pathname
const repo = new URL("../../../", import.meta.url).pathname
const bundleSha256 =
  "d7bb5f66c3539c839dd0b49fc460fbc081b09c100b5b2aa0f8be8fb366dd5943"
const inputsSha256 =
  "5edd9abea4ccc4e8c6a3929b217c7a25de93fd7136083e71111008264da904b0"
const fixtureNames = [
  "fixtures/workspace-config/linear.toml",
  "fixtures/workspace-credential/linear/credentials.toml",
]
const rawIds = [
  "c037-extra-wire-json",
  "c037-http-error",
  "c037-missing-required-json",
  "c037-missing-slug-text",
  "c037-null-connection-json",
  "c037-raw-unknown-status-text",
  "c037-reordered-wire-json",
]
const allStatusesOnly = new Set([
  "c037-all-statuses-json",
  "c037-valid-other-statuses-text",
  "c037-valid-other-statuses-json",
])
const pageTwoEligible = [
  "c037-empty-first-page-has-next",
  "c037-first-page-more-json",
  "c037-first-page-more-text",
  "c037-first-page-repeat-cursor-proposal",
]

function record(value: unknown): Record<string, unknown> {
  assert(value != null && typeof value === "object" && !Array.isArray(value))
  return Object.fromEntries(Object.entries(value))
}
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value != null && typeof value === "object") {
    return `{${
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
        .map(([k, v]) => `${JSON.stringify(k)}:${canonical(v)}`).join(",")
    }}`
  }
  return JSON.stringify(value)
}
async function digest(bytes: Uint8Array): Promise<string> {
  return await sha256Hex(bytes)
}
async function caseInputDigest(bytes: Uint8Array): Promise<string> {
  const value = record(JSON.parse(new TextDecoder().decode(bytes)))
  delete value.expected
  delete value.deviation
  return await digest(new TextEncoder().encode(canonical(value)))
}
async function inputList(bytes: Map<string, Uint8Array>): Promise<string> {
  const lines: string[] = []
  for (const name of [...bytes.keys()].sort()) {
    if (name === "c037-inputs.sha256") continue
    const value = bytes.get(name)
    assert(value != null)
    const hash = name.endsWith(".json")
      ? await caseInputDigest(value)
      : await digest(value)
    lines.push(`${hash}  ${name}\n`)
  }
  return lines.join("")
}
async function bundleDigest(bytes: Map<string, Uint8Array>): Promise<string> {
  const lines = await Promise.all(
    [...bytes.keys()].sort().map(async (name) =>
      `${name}\0${await digest(bytes.get(name) ?? new Uint8Array())}\n`
    ),
  )
  return await digest(new TextEncoder().encode(lines.join("")))
}
async function fileNames(): Promise<string[]> {
  const names: string[] = []
  async function walk(path: string): Promise<void> {
    for await (const entry of Deno.readDir(path)) {
      const child = join(path, entry.name)
      const name = relative(root, child)
      assert(!entry.isSymlink, `unexpected C037 symlink ${name}`)
      if (entry.isDirectory) await walk(child)
      else {
        assert(entry.isFile, `unexpected C037 entry ${name}`)
        names.push(name)
      }
    }
  }
  await walk(root)
  return names.sort()
}
async function sourceDocument(path: string, name: string): Promise<string> {
  const source = await Deno.readTextFile(join(repo, path))
  const start = source.indexOf(`query ${name}`)
  assert(start >= 0, `${path}: missing ${name}`)
  const end = source.indexOf("`", start)
  assert(end > start)
  return print(parse(source.slice(start, end)))
}
function changed(
  bytes: Map<string, Uint8Array>,
  name: string,
  from: string,
  to: string,
): Map<string, Uint8Array> {
  const next = new Map(bytes)
  const original = next.get(name)
  assert(original != null)
  const input = new TextDecoder().decode(original)
  const index = input.indexOf(from)
  assert(index >= 0, `${name}: missing mutation target ${from}`)
  next.set(
    name,
    new TextEncoder().encode(
      input.slice(0, index) + to + input.slice(index + from.length),
    ),
  )
  return next
}
function changedSteps(
  bytes: Map<string, Uint8Array>,
  name: string,
  kind: "remove-list" | "add-page",
): Map<string, Uint8Array> {
  const next = new Map(bytes)
  const original = next.get(name)
  assert(original != null)
  const value = record(JSON.parse(new TextDecoder().decode(original)))
  const graphql = record(value.graphql)
  const groups = graphql.groups
  assert(Array.isArray(groups) && groups.length === 1)
  const group = record(groups[0])
  const steps = group.steps
  assert(Array.isArray(steps))
  if (kind === "remove-list") {
    assertEquals(steps.length, 2)
    steps.pop()
  } else {
    assertEquals(steps.length, 1)
    const page = structuredClone(steps[0])
    record(page).id = "page-two"
    steps.push(page)
  }
  graphql.expectedRequests = steps.length
  next.set(name, new TextEncoder().encode(JSON.stringify(value)))
  return next
}

Deno.test("C037 freezes 82 strict initiative list Deno cases", async () => {
  const names = await fileNames()
  const caseNames = names.filter((name) => /^c037-.*\.json$/.test(name))
  assertEquals(caseNames.length, 82)
  assertEquals(
    names,
    [...caseNames, "c037-inputs.sha256", ...fixtureNames].sort(),
  )
  const bytes = new Map<string, Uint8Array>()
  for (const name of names) {
    bytes.set(name, await Deno.readFile(join(root, name)))
  }
  assertEquals(await bundleDigest(bytes), bundleSha256)
  const list = await inputList(bytes)
  assertEquals(new TextDecoder().decode(bytes.get("c037-inputs.sha256")), list)
  assertEquals(await digest(new TextEncoder().encode(list)), inputsSha256)
  assertEquals(
    new TextDecoder().decode(bytes.get(fixtureNames[0])),
    'workspace = "alpha"\n',
  )
  assertEquals(
    new TextDecoder().decode(bytes.get(fixtureNames[1])),
    'default = "alpha"\nalpha = "lin_api_fake_alpha"\nbeta = "lin_api_fake_beta"\n',
  )

  const baseline = record(JSON.parse(
    await Deno.readTextFile(new URL("../baseline.json", import.meta.url)),
  ))
  assertEquals(
    baseline.referenceRevision,
    "d4fe6fa7358f018fd1da0c6b96ec2b022247e898",
  )
  assertEquals(baseline.denoVersion, "2.7.9")
  assertEquals(
    baseline.lockSha256,
    "3da729da08fe6d48236e055b2eaac95788b5e5ccfd0f66dacdc5f6a0b0b96403",
  )
  assertEquals(
    baseline.schemaSha256,
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
  )
  assertEquals(
    baseline.binarySha256,
    "a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835",
  )
  const manifest = readManifest(JSON.parse(
    await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
  ))
  const route = manifest.routes.find((r) => r.path === "linear initiative list")
  assert(route != null)
  assert(typeof route.path === "string" && typeof route.source === "string")
  assertEquals(route.aliases, ["ls"])
  assertEquals(route.source, "src/commands/initiative/initiative-list.ts")
  const documents = new Map<string, string>([
    ["GetInitiatives", await sourceDocument(route.source, "GetInitiatives")],
    [
      "GetViewerForInitiatives",
      await sourceDocument(route.source, "GetViewerForInitiatives"),
    ],
    ["GetViewerId", await sourceDocument("src/utils/linear.ts", "GetViewerId")],
    ["LookupUser", await sourceDocument("src/utils/linear.ts", "LookupUser")],
  ])
  const loaded = await loadCases(root, new Set([route.path]), "c037-")
  assertEquals(loaded.map((entry) => `${entry.spec.id}.json`), caseNames)
  assertEquals(loaded.filter((entry) => entry.spec.graphql != null).length, 56)
  assertEquals(
    loaded.reduce(
      (n, entry) => n + (entry.spec.graphql?.expectedRequests ?? 0),
      0,
    ),
    65,
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
    rawIds,
  )
  const byInput = new Map<string, string[]>()
  for (const entry of loaded) {
    const raw = bytes.get(`${entry.spec.id}.json`)
    assert(raw != null)
    const value = record(JSON.parse(new TextDecoder().decode(raw)))
    for (const field of ["id", "reason", "expected", "deviation"]) {
      delete value[field]
    }
    const key = canonical(value)
    byInput.set(key, [...(byInput.get(key) ?? []), entry.spec.id])
  }
  assertEquals(
    [...byInput.values()].filter((ids) => ids.length > 1),
    [
      ["c037-closed-stdout-json", "c037-default-json"],
      ["c037-closed-stdout-text", "c037-default-text"],
    ],
  )
  for (
    const [closedId, drainId] of [
      ["c037-closed-stdout-json", "c037-default-json"],
      ["c037-closed-stdout-text", "c037-default-text"],
    ]
  ) {
    const closed = loaded.find((entry) => entry.spec.id === closedId)
    const drain = loaded.find((entry) => entry.spec.id === drainId)
    assert(closed != null && drain != null)
    assertEquals(closed.spec.expected.stdout, { mode: "closed-at-start" })
    assert("utf8" in drain.spec.expected.stdout)
  }
  const byId = new Map(loaded.map((entry) => [entry.spec.id, entry.spec]))
  function responseData(id: string, index: number): Record<string, unknown> {
    const spec = byId.get(id)
    assert(spec?.graphql != null)
    const group = spec.graphql.groups[0]
    assert(group.mode === "ordered")
    const step = group.steps[index]
    assert(step.kind === "graphql" && step.response.kind === "data")
    return record(step.response.data)
  }
  function operationVariables(
    id: string,
    index: number,
  ): Record<string, unknown> {
    const spec = byId.get(id)
    assert(spec?.graphql != null)
    const group = spec.graphql.groups[0]
    assert(group.mode === "ordered")
    const step = group.steps[index]
    assert(step.kind === "graphql")
    return record(step.operation.variables)
  }
  assertEquals(byId.get("c037-owner-next-flag")?.argv, [
    "initiative",
    "list",
    "--owner",
    "--json",
  ])
  const emailInput = byId.get("c037-owner-email-first")
  assertEquals(emailInput?.argv.at(-2), "ALICE@EXAMPLE.COM")
  const emailUsers =
    record(responseData("c037-owner-email-first", 0).users).nodes
  assert(Array.isArray(emailUsers) && emailUsers.length === 2)
  assertEquals(record(emailUsers[0]).displayName, "ALICE@EXAMPLE.COM")
  assertEquals(record(emailUsers[1]).email, "alice@example.com")
  assertEquals(
    record(record(operationVariables("c037-owner-email-first", 1).filter).owner)
      .id,
    { eq: "00000000-0000-4000-9000-000000000702" },
  )
  assertEquals(byId.get("c037-owner-display-first")?.argv.at(-2), "ALICE A")
  const displayUsers =
    record(responseData("c037-owner-display-first", 0).users).nodes
  assert(Array.isArray(displayUsers) && displayUsers.length === 2)
  assertEquals(record(displayUsers[1]).displayName, "Alice A")
  const stableNodes =
    record(responseData("c037-same-name-stable-json", 0).initiatives).nodes
  assert(Array.isArray(stableNodes))
  assertEquals(stableNodes.map((node) => record(node).id), [
    "00000000-0000-4000-9000-000000000034",
    "00000000-0000-4000-9000-000000000032",
    "00000000-0000-4000-9000-000000000033",
  ])
  const stableStdout = byId.get("c037-same-name-stable-json")?.expected.stdout
  assert(stableStdout != null && "utf8" in stableStdout)
  const stableOutputNodes = record(JSON.parse(stableStdout.utf8)).nodes
  assert(Array.isArray(stableOutputNodes))
  assertEquals(
    stableOutputNodes.map((node) => record(node).id),
    stableNodes.map((node) => record(node).id),
  )
  const initialsNodes =
    record(responseData("c037-wide-initials-text", 0).initiatives).nodes
  assert(Array.isArray(initialsNodes))
  const wideInitials = record(record(initialsNodes[0]).owner).initials
  assertEquals(wideInitials, "𝒜𝒜𝒜")
  assert(typeof wideInitials === "string" && wideInitials.length === 6)
  const cutNodes =
    record(responseData("c037-wide-cut-straddle-text", 0).initiatives).nodes
  assert(Array.isArray(cutNodes))
  assertEquals(record(cutNodes[0]).name, "NNNNNN界tail")
  const cutStdout = byId.get("c037-wide-cut-straddle-text")?.expected.stdout
  assert(cutStdout != null && "utf8" in cutStdout)
  assert(cutStdout.utf8.includes("NNNNNN..."))
  assert(!cutStdout.utf8.includes("界"))
  assertEquals(byId.get("c037-short-web-app")?.argv, [
    "initiative",
    "list",
    "-wa",
  ])
  assertEquals(
    byId.get("c037-empty-first-page-has-next")?.argv.at(-1),
    "--json",
  )
  for (const id of pageTwoEligible) {
    const spec = byId.get(id)
    assert(spec?.graphql != null)
    assertEquals(spec.graphql.expectedRequests, 1)
    const group = spec.graphql.groups[0]
    assert(group.mode === "ordered")
    const step = group.steps[0]
    assert(step.kind === "graphql" && step.response.kind === "data")
    const connection = record(record(step.response.data).initiatives)
    const pageInfo = record(connection.pageInfo)
    assertEquals(pageInfo.hasNextPage, true)
    assert(
      typeof pageInfo.endCursor === "string" && pageInfo.endCursor.length > 0,
    )
  }
  for (const entry of loaded) {
    const spec = entry.spec
    assertEquals(spec.route, route.path)
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.deviation, null)
    assertEquals(spec.timeoutMs, 30000)
    assertEquals(spec.outputCapBytes, 4194304)
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(spec.env.TZ, "UTC")
    assertEquals(spec.env.LANG, "C.UTF-8")
    assertEquals(spec.env.NO_COLOR, "1")
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1")
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    if (spec.graphql == null) {
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
      )
      continue
    }
    assertEquals(
      spec.env.LINEAR_GRAPHQL_ENDPOINT,
      "http://127.0.0.1:{{fixturePort}}/graphql",
    )
    const gql = spec.graphql
    assertEquals(gql.path, "/graphql")
    assertEquals(gql.schemaSha256, baseline.schemaSha256)
    assertEquals(gql.initialRecords, {})
    assertEquals(gql.expectedRecords, {})
    assertEquals(gql.groups.length, 1)
    const group = gql.groups[0]
    assert(group.mode === "ordered")
    assertEquals(gql.expectedRequests, group.steps.length)
    let listCount = 0
    for (const [index, step] of group.steps.entries()) {
      assert(step.kind === "graphql")
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.identity.headers, {})
      assertEquals(step.effects, [])
      assertEquals(
        step.identity.authorization,
        spec.id === "c037-workspace-credential" ||
          spec.id === "c037-web-cli-workspace-viewer"
          ? "lin_api_fake_beta"
          : "lin_api_fake",
      )
      const operation = [...documents].find(([, doc]) =>
        doc === print(parse(step.operation.document))
      )?.[0]
      assert(operation != null, `${spec.id}: foreign GraphQL document`)
      if (operation === "GetInitiatives") {
        listCount++
        assertEquals(index, group.steps.length - 1)
        const variables = record(step.operation.variables)
        assertEquals(
          variables.includeArchived,
          spec.argv.includes("--archived"),
        )
        const filter = variables.filter
        if (allStatusesOnly.has(spec.id)) assertEquals(filter, undefined)
        if (spec.id === "c037-owner-only-all-statuses") {
          assertEquals(record(filter).status, undefined)
          assertEquals(record(record(filter).owner).id, {
            eq: "00000000-0000-4000-9000-000000000501",
          })
        }
      } else if (operation === "GetViewerForInitiatives") {
        assertEquals(step.operation.variables, undefined)
        assertEquals(index, 0)
      } else if (operation === "GetViewerId") {
        assertEquals(step.operation.variables, {})
        assertEquals(index, 0)
      } else {
        const variables = record(step.operation.variables)
        assertEquals(Object.keys(variables), ["input"])
        assertEquals(index, 0)
      }
    }
    assert(listCount <= 1, `${spec.id}: Deno issued a second list page`)
  }

  const originalBundle = await bundleDigest(bytes)
  const originalInputs = await inputList(bytes)
  const mutations: [Map<string, Uint8Array>, boolean][] = [
    [
      changed(bytes, "c037-default-json.json", '"--json"', '"--archived"'),
      true,
    ],
    [
      changed(
        bytes,
        "c037-empty-text.json",
        "No initiatives found.",
        "No records found.",
      ),
      false,
    ],
    [
      changed(bytes, "c037-status-planned-text.json", '"Planned"', '"Active"'),
      true,
    ],
    [
      changed(
        bytes,
        "c037-owner-self.json",
        '"00000000-0000-4000-9000-000000000501"',
        '"00000000-0000-4000-9000-000000000599"',
      ),
      true,
    ],
    [
      changed(
        bytes,
        "c037-archived-json.json",
        '"includeArchived": true',
        '"includeArchived": false',
      ),
      true,
    ],
    [
      changed(
        bytes,
        "c037-first-page-more-json.json",
        '"page-a"',
        '"other-page"',
      ),
      true,
    ],
    [changedSteps(bytes, "c037-owner-self.json", "remove-list"), true],
    [changedSteps(bytes, "c037-first-page-more-json.json", "add-page"), true],
    [
      changed(bytes, fixtureNames[1], 'default = "alpha"', 'default = "beta"'),
      true,
    ],
  ]
  for (const [mutation, changesInput] of mutations) {
    assertNotEquals(await bundleDigest(mutation), originalBundle)
    if (changesInput) assertNotEquals(await inputList(mutation), originalInputs)
    else assertEquals(await inputList(mutation), originalInputs)
  }
})
