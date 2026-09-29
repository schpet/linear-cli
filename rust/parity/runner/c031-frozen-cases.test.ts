import { assert, assertEquals, assertNotEquals } from "@std/assert"
import { join, relative } from "@std/path"
import { parse, print } from "graphql"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c031-frozen-cases/", import.meta.url).pathname
const repo = new URL("../../../", import.meta.url).pathname
const fixtureNames = ["fixtures/workspace-credential/linear/credentials.toml"]
const bundleSha256 =
  "3f376134bf8af31718410a2fb546c12adaafa682555ea7a9dcf9c21676203c0d"
const rawIds = [
  "c031-all-http-second-error",
  "c031-all-null-second-root",
  "c031-all-two-pages-json",
  "c031-bare-name-null",
  "c031-details-http-error",
  "c031-details-null-root",
  "c031-extra-wire-json",
  "c031-future-invalid-time-text",
  "c031-json-reordered-fields",
  "c031-lookup-null-connection",
  "c031-lookup-null-project",
  "c031-missing-nested-issues-text",
  "c031-missing-state-text",
  "c031-null-empty-fields-text",
  "c031-sortorder-overflow-json",
]
const closedIds = ["c031-closed-stdout-json", "c031-closed-stdout-text"]

function record(value: unknown): Record<string, unknown> {
  if (value == null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("expected object")
  }
  return Object.fromEntries(Object.entries(value))
}
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value != null && typeof value === "object") {
    return `{${
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map((
        [k, v],
      ) => `${JSON.stringify(k)}:${canonical(v)}`).join(",")
    }}`
  }
  return JSON.stringify(value)
}
async function inputDigest(bytes: Uint8Array): Promise<string> {
  const value = record(JSON.parse(new TextDecoder().decode(bytes)))
  const input = Object.fromEntries(
    Object.entries(value).filter(([k]) =>
      k !== "expected" && k !== "deviation"
    ),
  )
  return await sha256Hex(new TextEncoder().encode(canonical(input)))
}
async function bundleDigest(bytes: Map<string, Uint8Array>): Promise<string> {
  const lines = await Promise.all(
    [...bytes.keys()].sort().map(async (name) =>
      `${name}\0${await sha256Hex(bytes.get(name) ?? new Uint8Array())}\n`
    ),
  )
  return await sha256Hex(new TextEncoder().encode(lines.join("")))
}
async function files(): Promise<string[]> {
  const names: string[] = []
  async function walk(path: string): Promise<void> {
    for await (const entry of Deno.readDir(path)) {
      const child = join(path, entry.name)
      const name = relative(root, child)
      assert(!entry.isSymlink, `unexpected C031 symlink ${name}`)
      if (entry.isDirectory) await walk(child)
      else {
        assert(entry.isFile, `unexpected C031 entry ${name}`)
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
function returnedMilestone(
  response: unknown,
): Record<string, unknown> | undefined {
  const value = record(response)
  let payload: Record<string, unknown>
  if (value.kind === "data") payload = record(value.data)
  else if (value.kind === "transport" && value.status === 200) {
    const body = record(value.body)
    assert(typeof body.utf8 === "string")
    payload = record(record(JSON.parse(body.utf8)).data)
  } else return undefined
  const milestone = payload.projectMilestone
  return milestone == null ? undefined : record(milestone)
}
function replaceOnce(text: string, from: string, to: string): string {
  const index = text.indexOf(from)
  assert(index >= 0, `missing mutation anchor ${from}`)
  assertEquals(
    text.indexOf(from, index + from.length),
    -1,
    `ambiguous mutation anchor ${from}`,
  )
  return text.slice(0, index) + to + text.slice(index + from.length)
}

Deno.test("C031 freezes 76 strict milestone-view Deno cases", async () => {
  const allNames = await files()
  const names = allNames.filter((n) => /^c031-.*\.json$/.test(n))
  assertEquals(names.length, 76)
  assertEquals(
    allNames,
    [...names, "c031-inputs.sha256", ...fixtureNames].sort(),
  )
  const bytes = new Map<string, Uint8Array>()
  for (const name of allNames) {
    bytes.set(name, await Deno.readFile(join(root, name)))
  }
  assertEquals(await bundleDigest(bytes), bundleSha256)
  const baseline = record(
    JSON.parse(
      await Deno.readTextFile(new URL("../baseline.json", import.meta.url)),
    ),
  )
  assertEquals(
    baseline.referenceRevision,
    "d4fe6fa7358f018fd1da0c6b96ec2b022247e898",
  )
  assertEquals(baseline.denoVersion, "2.7.9")
  assertEquals(
    baseline.binarySha256,
    "a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835",
  )
  assertEquals(
    baseline.lockSha256,
    "3da729da08fe6d48236e055b2eaac95788b5e5ccfd0f66dacdc5f6a0b0b96403",
  )
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set<string>(manifest.routes.map((r) => {
    if (typeof r.path !== "string") throw new Error("manifest path is not text")
    return r.path
  }))
  assert(routes.has("linear milestone view") && routes.has("linear milestone"))
  const documents = new Map<string, string>([
    [
      "GetMilestoneDetails",
      await sourceDocument(
        "src/commands/milestone/milestone-view.ts",
        "GetMilestoneDetails",
      ),
    ],
    ...await Promise.all(
      [
        "GetProjectIdByName",
        "GetProjectIdBySlugId",
        "GetProjectMilestonesForLookup",
      ].map(async (
        name,
      ): Promise<[string, string]> => [
        name,
        await sourceDocument("src/utils/linear.ts", name),
      ]),
    ),
  ])
  const loaded = await loadCases(root, routes, "c031-")
  assertEquals(loaded.map((x) => `${x.spec.id}.json`), names)
  assertEquals(loaded.filter((x) => x.spec.graphql != null).length, 59)
  assertEquals(
    loaded.reduce((sum, x) => sum + (x.spec.graphql?.expectedRequests ?? 0), 0),
    84,
  )
  assertEquals(
    loaded.filter((x) =>
      x.spec.graphql?.groups.some((g) =>
        g.mode === "ordered" &&
        g.steps.some((s) =>
          s.kind === "graphql" && s.response.kind === "transport"
        )
      )
    ).map((x) => x.spec.id),
    rawIds,
  )
  assertEquals(
    loaded.filter((x) => "mode" in x.spec.expected.stdout).map((x) =>
      x.spec.id
    ),
    closedIds,
  )
  const byId = new Map(loaded.map((x) => [x.spec.id, x.spec]))
  assertEquals(byId.get("c031-uuid-short-j")?.argv.slice(0, 2), [
    "milestone",
    "v",
  ])
  assertEquals(byId.get("c031-uuid-short-j")?.argv.at(-1), "-j")
  assertEquals(byId.get("c031-workspace-credential-direct")?.argv.slice(0, 2), [
    "--workspace",
    "beta",
  ])
  assertEquals(byId.get("c031-uuid-short-j")?.expected.exit, { code: 0 })
  const selectedJson = byId.get("c031-uuid-short-j")?.expected.stdout
  assert(selectedJson != null && "utf8" in selectedJson)
  const selected = record(JSON.parse(selectedJson.utf8))
  assertEquals(Object.keys(selected), [
    "id",
    "name",
    "description",
    "targetDate",
    "sortOrder",
    "createdAt",
    "updatedAt",
    "project",
    "issues",
  ])
  assertEquals(Object.keys(record(selected.issues)), ["nodes", "pageInfo"])
  const firstVariant = byId.get("c031-lookup-first-case-variant")
  assert(firstVariant != null && firstVariant.graphql != null)
  assertEquals(firstVariant.argv[2], "Launch")
  const variantGroup = firstVariant.graphql.groups[0]
  assert(variantGroup.mode === "ordered")
  const variantLookup = variantGroup.steps[0]
  const variantDetails = variantGroup.steps[1]
  assert(variantLookup.kind === "graphql" && variantDetails.kind === "graphql")
  assertEquals(
    record(variantDetails.operation.variables).id,
    "00000000-0000-4000-8000-000000000001",
  )
  const missingCursor = byId.get("c031-all-missing-cursor-second")
  assert(missingCursor != null && missingCursor.graphql != null)
  const missingGroup = missingCursor.graphql.groups[0]
  assert(missingGroup.mode === "ordered")
  const missingFirst = missingGroup.steps[0]
  const missingSecond = missingGroup.steps[1]
  assert(missingFirst.kind === "graphql" && missingSecond.kind === "graphql")
  const firstId = returnedMilestone(missingFirst.response)?.id
  const secondId = returnedMilestone(missingSecond.response)?.id
  assert(typeof firstId === "string" && typeof secondId === "string")
  assertNotEquals(firstId, secondId)
  assert("utf8" in missingCursor.expected.stderr)
  assert(
    missingCursor.expected.stderr.utf8.includes(
      `--milestone ${firstId} --json`,
    ),
  )
  assert(
    !missingCursor.expected.stderr.utf8.includes(
      `--milestone ${secondId} --json`,
    ),
  )
  const ten = byId.get("c031-preview-10-final-text")
  assert(ten != null && "utf8" in ten.expected.stdout)
  assertEquals((ten.expected.stdout.utf8.match(/^- APP-/gm) ?? []).length, 10)
  assert(!ten.expected.stdout.utf8.includes("_...and "))
  assert(!ten.expected.stdout.utf8.includes("_Showing "))
  const repeat = byId.get("c031-all-repeat-cursor-finite")
  assert(repeat != null && repeat.graphql != null)
  const repeatGroup = repeat.graphql.groups[0]
  assert(repeatGroup.mode === "ordered")
  const secondRepeat = repeatGroup.steps[1]
  const thirdRepeat = repeatGroup.steps[2]
  assert(secondRepeat.kind === "graphql" && thirdRepeat.kind === "graphql")
  const secondNodes =
    record(returnedMilestone(secondRepeat.response)?.issues).nodes
  const thirdNodes =
    record(returnedMilestone(thirdRepeat.response)?.issues).nodes
  assert(Array.isArray(secondNodes) && Array.isArray(thirdNodes))
  assertEquals(thirdNodes, secondNodes)
  assert("utf8" in repeat.expected.stdout)
  const repeatedOutput = record(JSON.parse(repeat.expected.stdout.utf8))
  const outputNodes = record(repeatedOutput.issues).nodes
  assert(Array.isArray(outputNodes))
  assertEquals(outputNodes.slice(-2), [secondNodes[0], thirdNodes[0]])
  const allJson = byId.get("c031-all-two-pages-json")
  assert(allJson != null && allJson.graphql != null)
  const allGroup = allJson.graphql.groups[0]
  assert(allGroup.mode === "ordered")
  const allFirst = allGroup.steps[0]
  assert(allFirst.kind === "graphql" && allFirst.response.kind === "transport")
  assert("utf8" in allFirst.response.body)
  const wireIssues = record(
    record(record(JSON.parse(allFirst.response.body.utf8)).data)
      .projectMilestone,
  ).issues
  assertEquals(Object.keys(record(wireIssues)), [
    "pageInfo",
    "extraIssues",
    "nodes",
  ])
  assert("utf8" in allJson.expected.stdout)
  assertEquals(
    Object.keys(
      record(record(JSON.parse(allJson.expected.stdout.utf8)).issues),
    ),
    ["nodes", "pageInfo"],
  )
  const lines: string[] = []
  for (const entry of loaded) {
    const spec = entry.spec
    const file = `${spec.id}.json`
    assertEquals(
      spec.route,
      spec.id === "c031-parent-help"
        ? "linear milestone"
        : "linear milestone view",
    )
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.deviation, null)
    assertEquals(spec.timeoutMs, 30000)
    assertEquals(spec.outputCapBytes, 4194304)
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(spec.env.LANG, "C.UTF-8")
    assertEquals(spec.env.TZ, "UTC")
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1")
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    assertEquals(
      spec.env.NO_COLOR,
      spec.id === "c031-no-color-unset-text" ? undefined : "1",
    )
    if (spec.graphql == null) {
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
      )
    } else {
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:{{fixturePort}}/graphql",
      )
      const gql = spec.graphql
      assertEquals(gql.path, "/graphql")
      assertEquals(
        gql.schemaSha256,
        "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
      )
      assertEquals(gql.initialRecords, {})
      assertEquals(gql.expectedRecords, {})
      assertEquals(gql.groups.length, 1)
      const group = gql.groups[0]
      if (group.mode !== "ordered") {
        throw new Error(`${spec.id}: expected ordered group`)
      }
      assertEquals(gql.expectedRequests, group.steps.length)
      let previousCursor: unknown
      let detailId: unknown
      let details = 0
      let firstReturned: unknown
      for (const step of group.steps) {
        if (step.kind !== "graphql") {
          throw new Error(`${spec.id}: unexpected asset step`)
        }
        assert(
          ["lin_api_fake", "lin_api_fake_beta"].includes(
            step.identity.authorization ?? "",
          ),
        )
        assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
        assertEquals(step.identity.headers, {})
        assertEquals(step.effects, [])
        const document = print(parse(step.operation.document))
        const operation = [...documents].find(([, source]) =>
          source === document
        )?.[0]
        assert(operation != null, `${spec.id}: foreign GraphQL document`)
        const vars = record(step.operation.variables)
        if (operation === "GetMilestoneDetails") {
          assertEquals(vars.first, 50)
          assertEquals(
            Object.keys(vars),
            details === 0 ? ["id", "first"] : ["id", "first", "after"],
          )
          if (details === 0) {
            assert(!("after" in vars))
            detailId = vars.id
          } else {
            assertEquals(vars.after, previousCursor)
            assertEquals(vars.id, detailId)
          }
          details++
          const ms = step.response.kind === "data" ||
              (spec.id === "c031-all-two-pages-json" && details === 1)
            ? returnedMilestone(step.response)
            : undefined
          if (ms != null) {
            const info = record(record(ms.issues).pageInfo)
            previousCursor = info.endCursor
            if (details === 1) firstReturned = ms.id
            if (!spec.argv.includes("--json")) {
              for (const field of ["createdAt", "updatedAt"]) {
                const value = ms[field]
                if (typeof value === "string" && /^\d{4}-/.test(value)) {
                  const year = Number(value.slice(0, 4))
                  assert(
                    year <= 2025 || year >= 2999,
                    `${spec.id}: near-now ${field}`,
                  )
                }
              }
            }
            if (details === 2 && spec.argv.includes("--all")) {
              const first = group.steps.find((s) =>
                s.kind === "graphql" &&
                s.operation.document === step.operation.document
              )
              assert(first != null && first.kind === "graphql")
              const original = returnedMilestone(first.response)
              assert(original != null)
              if (spec.id === "c031-all-missing-cursor-second") {
                assertNotEquals(ms.id, original.id)
              }
              assertNotEquals(ms.name, original.name)
              assertNotEquals(ms.description, original.description)
              assertNotEquals(
                record(ms.project).id,
                record(original.project).id,
              )
            }
          }
        } else {
          assert(!("first" in vars) && !("after" in vars))
          if (operation === "GetProjectIdByName") {
            assertEquals(Object.keys(vars), ["name"])
          }
          if (operation === "GetProjectIdBySlugId") {
            assertEquals(Object.keys(vars), ["slugId"])
          }
          if (operation === "GetProjectMilestonesForLookup") {
            assertEquals(Object.keys(vars), ["projectId"])
          }
        }
      }
      if (
        spec.id === "c031-bare-linear-url-legacy" ||
        spec.id === "c031-bare-linear-unsupported-legacy"
      ) {
        assertEquals(group.steps.length, 1)
        assertEquals(detailId, spec.argv[2])
      }
      if (spec.argv.includes("--all")) assertNotEquals(detailId, firstReturned)
      if (spec.id === "c031-future-invalid-time-text") {
        const first = group.steps[0]
        assert(first.kind === "graphql" && first.response.kind === "transport")
        assert("utf8" in first.response.body)
        const body = JSON.parse(first.response.body.utf8)
        const ms = record(record(body.data).projectMilestone)
        assert(Number(String(ms.createdAt).slice(0, 4)) >= 2999)
      }
    }
    lines.push(
      `${await inputDigest(bytes.get(file) ?? new Uint8Array())}  ${file}`,
    )
  }
  for (const name of fixtureNames) {
    lines.push(
      `${await sha256Hex(bytes.get(name) ?? new Uint8Array())}  ${name}`,
    )
  }
  assertEquals(
    lines.join("\n") + "\n",
    await Deno.readTextFile(join(root, "c031-inputs.sha256")),
  )
  const decoder = new TextDecoder()
  const encoder = new TextEncoder()
  const mutations: Array<[string, string, string, boolean]> = [
    [
      "c031-uuid-short-j.json",
      '"v",\n    "00000000-0000-4000-8000-000000000001"',
      '"v",\n    "00000000-0000-4000-8000-000000000002"',
      true,
    ],
    ["c031-uuid-text-full.json", "# Launch", "# Different", false],
    [
      "c031-all-two-pages-json.json",
      '"after": "cursor-1"',
      '"after": "cursor-2"',
      true,
    ],
    [
      "c031-details-null-root.json",
      '{\\"data\\":{\\"projectMilestone\\":null}}',
      '{\\"data\\":{\\"projectMilestone\\":{}}}',
      true,
    ],
    [fixtureNames[0], 'default = "alpha"', 'default = "beta"', true],
  ]
  for (const [file, from, to, inputChanges] of mutations) {
    const original = bytes.get(file)
    assert(original != null)
    const changed = encoder.encode(
      replaceOnce(decoder.decode(original), from, to),
    )
    const mutated = new Map(bytes)
    mutated.set(file, changed)
    assertNotEquals(await bundleDigest(mutated), bundleSha256)
    const before = file.endsWith(".json")
      ? await inputDigest(original)
      : await sha256Hex(original)
    const after = file.endsWith(".json")
      ? await inputDigest(changed)
      : await sha256Hex(changed)
    assertEquals(before !== after, inputChanges, file)
  }
})
