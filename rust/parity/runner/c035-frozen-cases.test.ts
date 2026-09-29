import { assert, assertEquals, assertNotEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { parse, print } from "graphql"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c035-frozen-cases/", import.meta.url).pathname
const repo = new URL("../../../", import.meta.url).pathname
const fixture = "fixtures/workspace-credential/linear/credentials.toml"
const bundleSha256 =
  "3a664a563ad0210e1bb7050e735c29b974fed44d1ea06163b335eb1174ea0080"
const inputsSha256 =
  "306b2bb2cba82f7bda445dd6212ead33576a7384ddf3f19d25adf567061e4763"
const projectId = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d"
const alternateId = "9c8b7a6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d"
const rawIds = [
  "c035-author-null-raw",
  "c035-author-percent-health",
  "c035-date-invalid-raw",
  "c035-extra-wire-json",
  "c035-health-null-raw",
  "c035-health-unknown-raw",
  "c035-http-error",
  "c035-missing-required-raw",
  "c035-null-project",
  "c035-null-updates-json",
  "c035-null-updates-text",
  "c035-reordered-wire-json",
]
const firstOverrides = new Map([
  ["c035-limit-one-json", 1],
  ["c035-limit-twenty-text", 20],
  ["c035-limit-zero", 0],
  ["c035-limit-negative", -1],
  ["c035-limit-hex", 2],
  ["c035-limit-exponent", 10],
  ["c035-limit-max-int", 2147483647],
  ["c035-limit-equals-five", 5],
])

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
    if (name === "c035-inputs.sha256") continue
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
      assert(!entry.isSymlink, `unexpected C035 symlink ${name}`)
      if (entry.isDirectory) await walk(child)
      else {
        assert(entry.isFile, `unexpected C035 entry ${name}`)
        names.push(name)
      }
    }
  }
  await walk(root)
  return names.sort()
}
async function sourceDocument(path: string, name: string): Promise<string> {
  const source = await Deno.readTextFile(join(repo, path))
  const start = source.indexOf(`query ${name}(`)
  assert(start >= 0, `${path}: missing ${name}`)
  const end = source.indexOf("`", start)
  assert(end > start)
  return print(parse(source.slice(start, end)))
}
function replaceOnce(input: string, from: string, to: string): string {
  const start = input.indexOf(from)
  assert(start >= 0)
  return input.slice(0, start) + to + input.slice(start + from.length)
}

Deno.test("C035 freezes 75 strict project-update list Deno cases", async () => {
  const names = await fileNames()
  const caseNames = names.filter((name) => /^c035-.*\.json$/.test(name))
  assertEquals(caseNames.length, 75)
  assertEquals(names, [...caseNames, "c035-inputs.sha256", fixture].sort())
  const bytes = new Map<string, Uint8Array>()
  for (const name of names) {
    bytes.set(name, await Deno.readFile(join(root, name)))
  }
  assertEquals(await bundleDigest(bytes), bundleSha256)
  const list = await inputList(bytes)
  assertEquals(new TextDecoder().decode(bytes.get("c035-inputs.sha256")), list)
  assertEquals(await digest(new TextEncoder().encode(list)), inputsSha256)
  assertEquals(
    new TextDecoder().decode(bytes.get(fixture)),
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
  const route = manifest.routes.find((r) =>
    r.path === "linear project-update list"
  )
  assert(route != null)
  assert(typeof route.path === "string")
  assertEquals(route.aliases, ["l"])
  assertEquals(
    route.source,
    "src/commands/project-update/project-update-list.ts",
  )
  const documents = new Map<string, string>([
    [
      "ListProjectUpdates",
      await sourceDocument(
        "src/commands/project-update/project-update-list.ts",
        "ListProjectUpdates",
      ),
    ],
    [
      "GetProjectIdByName",
      await sourceDocument("src/utils/linear.ts", "GetProjectIdByName"),
    ],
    [
      "GetProjectIdBySlugId",
      await sourceDocument("src/utils/linear.ts", "GetProjectIdBySlugId"),
    ],
  ])
  const loaded = await loadCases(root, new Set([route.path]), "c035-")
  assertEquals(loaded.map((entry) => `${entry.spec.id}.json`), caseNames)
  const byCanonicalInput = new Map<string, string[]>()
  for (const entry of loaded) {
    const raw = bytes.get(`${entry.spec.id}.json`)
    assert(raw != null)
    const value = record(JSON.parse(new TextDecoder().decode(raw)))
    for (const field of ["id", "reason", "expected", "deviation"]) {
      delete value[field]
    }
    const key = canonical(value)
    byCanonicalInput.set(key, [
      ...(byCanonicalInput.get(key) ?? []),
      entry.spec.id,
    ])
  }
  // Closed stdout is a distinct harness mode recorded in expected. Those
  // exact two pairs intentionally share argv and fixtures with drain cases.
  const duplicates = [...byCanonicalInput.values()].filter((ids) =>
    ids.length > 1
  )
  assertEquals(duplicates, [
    ["c035-closed-stdout-json", "c035-default-json"],
    ["c035-closed-stdout-text", "c035-default-text"],
  ])
  for (const [closedId, drainId] of duplicates) {
    const closed = loaded.find((entry) => entry.spec.id === closedId)
    const drain = loaded.find((entry) => entry.spec.id === drainId)
    assert(closed != null && drain != null)
    assertEquals(closed.spec.expected.stdout, { mode: "closed-at-start" })
    assert("utf8" in drain.spec.expected.stdout)
  }
  assertEquals(loaded.filter((entry) => entry.spec.graphql != null).length, 57)
  assertEquals(
    loaded.reduce(
      (n, entry) => n + (entry.spec.graphql?.expectedRequests ?? 0),
      0,
    ),
    65,
  )
  assertEquals(
    loaded.filter((entry) =>
      entry.spec.graphql?.groups.some((g) =>
        g.mode === "ordered" &&
        g.steps.some((s) =>
          s.kind === "graphql" && s.response.kind === "transport"
        )
      )
    ).map((entry) => entry.spec.id),
    rawIds,
  )
  assertEquals(
    loaded.filter((entry) => "mode" in entry.spec.expected.stdout).map((
      entry,
    ) => entry.spec.id),
    ["c035-closed-stdout-json", "c035-closed-stdout-text"],
  )
  const byId = new Map(loaded.map((entry) => [entry.spec.id, entry.spec]))
  assertEquals(byId.get("c035-alias-text")?.argv.slice(0, 2), [
    "project-update",
    "l",
  ])
  assertEquals(byId.get("c035-url-foreign-no-key")?.argv.slice(0, 2), [
    "--workspace",
    "alpha",
  ])
  assertEquals(
    byId.get("c035-url-foreign-no-key")?.env.LINEAR_API_KEY,
    undefined,
  )
  assertEquals(
    byId.get("c035-url-wrong-kind")?.configFixture,
    "workspace-credential",
  )
  assertEquals(
    byId.get("c035-url-slug-hit")?.configFixture,
    "workspace-credential",
  )
  assertEquals(
    byId.get("c035-url-no-effective-workspace")?.configFixture,
    undefined,
  )
  assertEquals(
    byId.get("c035-workspace-credential")?.configFixture,
    "workspace-credential",
  )
  assertEquals(
    byId.get("c035-pageinfo-more-json")?.graphql?.expectedRequests,
    1,
  )
  const moreJson = byId.get("c035-pageinfo-more-json")?.expected.stdout
  assert(moreJson != null && "utf8" in moreJson)
  const moreUpdates = record(record(JSON.parse(moreJson.utf8)).projectUpdates)
  assertEquals(moreUpdates.nodes, [])
  assertEquals(moreUpdates.pageInfo, {
    hasNextPage: true,
    endCursor: "empty-more",
  })
  const afterId = byId.get("c035-json-after-id")
  assert(afterId?.graphql != null)
  assertEquals(afterId.argv.slice(-2), [projectId, "--json"])
  const afterOutput = afterId.expected.stdout
  assert("utf8" in afterOutput)
  const afterUpdates = record(
    record(JSON.parse(afterOutput.utf8)).projectUpdates,
  )
  const afterNodes = afterUpdates.nodes
  assert(Array.isArray(afterNodes) && afterNodes.length === 2)
  assertEquals(afterNodes.map((node) => record(node).id), [
    "00000000-0000-4000-9000-000000000031",
    "00000000-0000-4000-9000-000000000032",
  ])
  assertEquals(afterNodes.map((node) => record(node).health), [
    "atRisk",
    "offTrack",
  ])
  assertEquals(
    afterNodes.map((node) => record(record(node).user).displayName),
    ["Bob B", "Eve E"],
  )
  assertEquals(afterUpdates.pageInfo, {
    hasNextPage: false,
    endCursor: "terminal",
  })

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
    let lastLookupId: unknown
    for (const [index, step] of group.steps.entries()) {
      assert(step.kind === "graphql")
      assertEquals(
        step.identity.authorization,
        spec.id === "c035-workspace-credential"
          ? "lin_api_fake_beta"
          : spec.id === "c035-url-slug-hit"
          ? "lin_api_fake_alpha"
          : "lin_api_fake",
      )
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.identity.headers, {})
      assertEquals(step.effects, [])
      const doc = print(parse(step.operation.document))
      const operation = [...documents].find(([, source]) => source === doc)?.[0]
      assert(operation != null, `${spec.id}: foreign GraphQL document`)
      const vars = record(step.operation.variables)
      if (operation === "ListProjectUpdates") {
        listCount++
        assertEquals(index, group.steps.length - 1)
        assertEquals(Object.keys(vars), ["id", "first"])
        assertEquals(vars.first, firstOverrides.get(spec.id) ?? 10)
        assertEquals(
          vars.id,
          spec.id === "c035-uppercase-uuid"
            ? projectId.toUpperCase()
            : spec.id === "c035-name-miss-slug-hit"
            ? alternateId
            : projectId,
        )
        if (lastLookupId != null) assertEquals(vars.id, lastLookupId)
        if (step.response.kind === "data" && !spec.argv.includes("--json")) {
          const project = record(record(step.response.data).project)
          const updates = record(project.projectUpdates)
          const nodes = updates.nodes
          assert(Array.isArray(nodes))
          for (const node of nodes) {
            const timestamp = record(node).createdAt
            assert(
              typeof timestamp === "string" &&
                Number(timestamp.slice(0, 4)) >= 2999,
              `${spec.id}: moving date label`,
            )
          }
        }
      } else if (operation === "GetProjectIdByName") {
        assertEquals(Object.keys(vars), ["name"])
        assertEquals(index, 0)
        assertEquals(
          vars.name,
          spec.argv[spec.argv.length - 1] === "--json"
            ? spec.argv[spec.argv.length - 2]
            : spec.argv.at(-1),
        )
        const nodes = record(record(step.response).data).projects
        const ids = record(nodes).nodes
        assert(Array.isArray(ids))
        if (ids.length === 1) lastLookupId = record(ids[0]).id
      } else {
        assertEquals(Object.keys(vars), ["slugId"])
        assert(index === 0 || index === 1)
        assertEquals(
          vars.slugId,
          spec.id.startsWith("c035-url-")
            ? "abc123def456"
            : spec.argv.at(-1) === "--json"
            ? spec.argv.at(-2)
            : spec.argv.at(-1),
        )
        const nodes = record(record(step.response).data).projects
        const ids = record(nodes).nodes
        assert(Array.isArray(ids))
        if (ids.length === 1) lastLookupId = record(ids[0]).id
      }
    }
    assert(listCount <= 1, `${spec.id}: second page request`)
  }
  const nullProject = byId.get("c035-null-project")
  assert(nullProject?.graphql != null)
  const nullGroup = nullProject.graphql.groups[0]
  assert(nullGroup.mode === "ordered")
  const nullStep = nullGroup.steps[0]
  assert(nullStep.kind === "graphql" && nullStep.response.kind === "transport")
  assert("utf8" in nullStep.response.body)
  assertEquals(
    record(record(JSON.parse(nullStep.response.body.utf8)).data).project,
    null,
  )
  const missing = byId.get("c035-missing-required-raw")
  assert(missing?.graphql != null)
  const missingGroup = missing.graphql.groups[0]
  assert(missingGroup.mode === "ordered")
  const missingStep = missingGroup.steps[0]
  assert(
    missingStep.kind === "graphql" && missingStep.response.kind === "transport",
  )
  assert("utf8" in missingStep.response.body)
  const missingNodes = record(
    record(
      record(record(JSON.parse(missingStep.response.body.utf8)).data).project,
    ).projectUpdates,
  ).nodes
  assert(Array.isArray(missingNodes) && missingNodes.length === 1)
  assert(!("id" in record(missingNodes[0])))

  const baselineBundle = await bundleDigest(bytes)
  const baselineInputs = await inputList(bytes)
  function changed(
    name: string,
    from: string,
    to: string,
  ): Map<string, Uint8Array> {
    const next = new Map(bytes)
    const original = next.get(name)
    assert(original != null)
    next.set(
      name,
      new TextEncoder().encode(
        replaceOnce(new TextDecoder().decode(original), from, to),
      ),
    )
    return next
  }
  function changedSteps(
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
  const mutations: [Map<string, Uint8Array>, boolean][] = [
    [changed("c035-default-json.json", projectId, alternateId), true],
    [
      changed(
        "c035-empty-text.json",
        "No status updates found",
        "No updates found",
      ),
      false,
    ],
    [
      changed(
        "c035-name-hit.json",
        `"id": "${projectId}"`,
        `"id": "${alternateId}"`,
      ),
      true,
    ],
    [changed("c035-limit-one-json.json", '"first": 1', '"first": 2'), true],
    [changedSteps("c035-name-hit.json", "remove-list"), true],
    [changedSteps("c035-pageinfo-more-json.json", "add-page"), true],
    [
      changed(
        "c035-null-project.json",
        '\\"project\\":null',
        '\\"project\\":{}',
      ),
      true,
    ],
    [changed(fixture, 'default = "alpha"', 'default = "beta"'), true],
  ]
  for (const [mutation, changesInput] of mutations) {
    assertNotEquals(await bundleDigest(mutation), baselineBundle)
    if (changesInput) assertNotEquals(await inputList(mutation), baselineInputs)
    else assertEquals(await inputList(mutation), baselineInputs)
  }
})
