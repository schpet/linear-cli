import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrows,
} from "@std/assert"
import { join, relative } from "@std/path"
import { parse, print } from "graphql"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c027-frozen-cases/", import.meta.url).pathname
const repo = new URL("../../../", import.meta.url).pathname
const fixtureNames = ["fixtures/workspace-credential/linear/credentials.toml"]
const bundleSha256 =
  "345dba4597a91cf9093dfed0812e2f575fc6013bb5a312d6f68ae8467dfcbe13"
const inputsSha256 =
  "a04afd8993bef4fc0e07b5b823990b9a04499679c81b3bee056bd2a07c7735f9"
const rawBodyIds = [
  "c027-extra-wire-json",
  "c027-invalid-date-text",
  "c027-missing-required-body",
  "c027-null-comments",
  "c027-null-project-later",
  "c027-reordered-wire-json",
  "c027-resolved-thread",
  "c027-unknown-uuid-null",
]
const rawStatusIds = ["c027-http-first-error", "c027-http-second-error"]
const closedIds = ["c027-closed-stdout-json", "c027-closed-stdout-text"]

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
async function inputList(bytes: Map<string, Uint8Array>): Promise<string> {
  const lines: string[] = []
  for (const name of [...bytes.keys()].sort()) {
    if (name === "c027-inputs.sha256") continue
    const value = bytes.get(name)
    assert(value != null)
    const hash = name.endsWith(".json")
      ? await inputDigest(value)
      : await sha256Hex(value)
    lines.push(`${hash}  ${name}\n`)
  }
  return lines.join("")
}
async function files(): Promise<string[]> {
  const names: string[] = []
  async function walk(path: string): Promise<void> {
    for await (const entry of Deno.readDir(path)) {
      const child = join(path, entry.name)
      const name = relative(root, child)
      assert(!entry.isSymlink, `unexpected C027 symlink ${name}`)
      if (entry.isDirectory) await walk(child)
      else {
        assert(entry.isFile, `unexpected C027 entry ${name}`)
        names.push(name)
      }
    }
  }
  await walk(root)
  return names.sort()
}
async function sourceDocument(path: string, name: string): Promise<string> {
  const source = await Deno.readTextFile(join(repo, path))
  const start = source.indexOf(
    `${name.startsWith("Comment") ? "fragment" : "query"} ${name}`,
  )
  assert(start >= 0, `${path}: missing ${name}`)
  const end = source.indexOf("`", start)
  assert(end > start)
  return source.slice(start, end)
}
function replaceOnce(input: string, from: string, to: string): string {
  const first = input.indexOf(from)
  assert(first >= 0 && input.indexOf(from, first + from.length) === -1)
  return input.slice(0, first) + to + input.slice(first + from.length)
}

function assertCommentVariables(
  vars: Record<string, unknown>,
  page: number,
  priorCursor: unknown,
  resolvedId: unknown,
  caseId: string,
): void {
  assertEquals(Object.keys(vars), ["id", "filterId", "after"])
  assertEquals(
    vars.id,
    vars.filterId,
    `${caseId}: project/filter UUID mismatch`,
  )
  if (page === 0) {
    assertEquals(vars.after, null, `${caseId}: page one must send after:null`)
  } else {
    assertEquals(vars.after, priorCursor, `${caseId}: wrong page cursor`)
    assertEquals(vars.id, resolvedId, `${caseId}: changed project UUID`)
  }
}

Deno.test("C027 freezes 66 strict project-comment-list Deno cases", async () => {
  const names = await files()
  const cases = names.filter((name) => /^c027-.*\.json$/.test(name))
  assertEquals(cases.length, 66)
  assertEquals(names, [...cases, "c027-inputs.sha256", ...fixtureNames].sort())
  const bytes = new Map<string, Uint8Array>()
  for (const name of names) {
    bytes.set(name, await Deno.readFile(join(root, name)))
  }
  assertEquals(await bundleDigest(bytes), bundleSha256)
  const list = await inputList(bytes)
  assertEquals(new TextDecoder().decode(bytes.get("c027-inputs.sha256")), list)
  assertEquals(await sha256Hex(new TextEncoder().encode(list)), inputsSha256)
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
  assertEquals(
    baseline.schemaSha256,
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
  )
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routeMap = new Map<string, typeof manifest.routes[number]>()
  for (const route of manifest.routes) {
    assert(typeof route.path === "string")
    routeMap.set(route.path, route)
  }
  assertEquals(routeMap.get("linear project comment list")?.workItem, "C027")
  assertEquals(routeMap.get("linear project comment")?.workItem, "F01")
  const commentsDoc = await sourceDocument(
    "src/commands/project/project-comment-list.ts",
    "GetProjectComments",
  )
  const fragmentDoc = await sourceDocument(
    "src/utils/comments.ts",
    "CommentListFields",
  )
  const documents = new Map([
    ["GetProjectComments", print(parse(`${commentsDoc}\n${fragmentDoc}`))],
    [
      "GetProjectIdByName",
      print(
        parse(
          await sourceDocument("src/utils/linear.ts", "GetProjectIdByName"),
        ),
      ),
    ],
    [
      "GetProjectIdBySlugId",
      print(
        parse(
          await sourceDocument("src/utils/linear.ts", "GetProjectIdBySlugId"),
        ),
      ),
    ],
  ])
  const loaded = await loadCases(root, new Set(routeMap.keys()), "c027-")
  assertEquals(loaded.map((entry) => `${entry.spec.id}.json`), cases)
  const rawBody: string[] = []
  const rawStatus: string[] = []
  const closed: string[] = []
  let graphCases = 0
  let graphRequests = 0
  for (const { spec } of loaded) {
    const parent = spec.id === "c027-parent-help" ||
      spec.id === "c027-parent-bare"
    assertEquals(
      spec.route,
      parent ? "linear project comment" : "linear project comment list",
    )
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.deviation, null)
    assertEquals(spec.timeoutMs, 30000)
    assertEquals(spec.outputCapBytes, 4194304)
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.stdin, { utf8: "" })
    assertEquals(spec.cwdFixture, "empty")
    assertEquals(spec.env.HOME, "{{home}}")
    assertEquals(spec.env.XDG_CONFIG_HOME, "{{configHome}}")
    assertEquals(spec.env.APPDATA, "{{configHome}}")
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(spec.env.DENO_DIR, "{{denoDir}}")
    assertEquals(spec.env.LANG, "C.UTF-8")
    assertEquals(spec.env.TZ, "UTC")
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1")
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    assertEquals(
      spec.env.NO_COLOR,
      spec.id === "c027-no-color-unset" ? undefined : "1",
    )
    assertEquals(
      Object.keys(spec.env).sort(),
      [
        "HOME",
        "XDG_CONFIG_HOME",
        "APPDATA",
        "PATH",
        "DENO_DIR",
        "TZ",
        "LANG",
        "LINEAR_IGNORE_ENV_FILE",
        "LINEAR_GRAPHQL_ENDPOINT",
        ...(spec.env.NO_COLOR == null ? [] : ["NO_COLOR"]),
        ...(spec.env.LINEAR_API_KEY == null ? [] : ["LINEAR_API_KEY"]),
      ].sort(),
    )
    assertEquals(
      spec.configFixture,
      ["c027-url-foreign-workspace", "c027-workspace-credential"].includes(
          spec.id,
        )
        ? "workspace-credential"
        : undefined,
    )
    if ("mode" in spec.expected.stdout) closed.push(spec.id)
    if (spec.graphql == null) {
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
      )
      assertEquals(spec.substitutions, ["home", "configHome", "bin", "denoDir"])
      continue
    }
    graphCases++
    const gql = spec.graphql
    assertEquals(
      spec.env.LINEAR_GRAPHQL_ENDPOINT,
      "http://127.0.0.1:{{fixturePort}}/graphql",
    )
    assertEquals(spec.substitutions, [
      "home",
      "configHome",
      "bin",
      "denoDir",
      "fixturePort",
    ])
    assertEquals(gql.path, "/graphql")
    assertEquals(gql.schemaSha256, baseline.schemaSha256)
    assertEquals(gql.initialRecords, {})
    assertEquals(gql.expectedRecords, {})
    assertEquals(gql.groups.length, 1)
    const group = gql.groups[0]
    assertEquals(group.mode, "ordered")
    if (group.mode !== "ordered") throw new Error("unreachable group mode")
    assertEquals(gql.expectedRequests, group.steps.length)
    graphRequests += group.steps.length
    let priorCursor: unknown
    let commentCount = 0
    let resolvedId: unknown
    for (const step of group.steps) {
      assertEquals(step.kind, "graphql")
      if (step.kind !== "graphql") throw new Error("unreachable step kind")
      assertEquals(step.effects, [])
      assert(
        ["lin_api_fake", "lin_api_fake_beta"].includes(
          step.identity.authorization ?? "",
        ),
      )
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(step.identity.headers, {})
      const document = print(parse(step.operation.document))
      const operation = [...documents].find(([, value]) => value === document)
        ?.[0]
      assert(operation != null, `${spec.id}: foreign GraphQL document`)
      const vars = record(step.operation.variables)
      if (operation === "GetProjectComments") {
        assertCommentVariables(
          vars,
          commentCount,
          priorCursor,
          resolvedId,
          spec.id,
        )
        if (commentCount === 0) resolvedId = vars.id
        commentCount++
        if (step.response.kind === "data") {
          const data = record(step.response.data)
          const connection = record(data.comments)
          priorCursor = record(connection.pageInfo).endCursor
        } else if (step.response.kind === "transport") {
          if (step.response.status === 200) {
            rawBody.push(spec.id)
            assert("utf8" in step.response.body)
            const payload = record(JSON.parse(step.response.body.utf8))
            const data = record(payload.data)
            if (data.comments != null) {
              priorCursor = record(record(data.comments).pageInfo).endCursor
            }
          } else rawStatus.push(spec.id)
        }
      } else if (operation === "GetProjectIdByName") {
        assertEquals(Object.keys(vars), ["name"])
      } else {
        assertEquals(operation, "GetProjectIdBySlugId")
        assertEquals(Object.keys(vars), ["slugId"])
      }
    }
    assert(commentCount <= 3)
  }
  assertEquals([...new Set(rawBody)].sort(), rawBodyIds)
  assertEquals([...new Set(rawStatus)].sort(), rawStatusIds)
  assertEquals(closed, closedIds)
  assertEquals(graphCases, 54)
  assertEquals(graphRequests, 73)
  const byId = new Map(loaded.map(({ spec }) => [spec.id, spec]))
  const full = byId.get("c027-uuid-json-full")
  assert(full != null && "utf8" in full.expected.stdout)
  const json = record(JSON.parse(full.expected.stdout.utf8))
  assertEquals(Object.keys(json), ["nodes", "pageInfo"])
  const nodes = json.nodes
  assert(Array.isArray(nodes) && nodes.length === 3)
  assertEquals(Object.keys(record(nodes[0])), [
    "id",
    "body",
    "quotedText",
    "createdAt",
    "updatedAt",
    "editedAt",
    "url",
    "user",
    "externalUser",
    "botActor",
    "parent",
  ])
  assertEquals(
    record(nodes[0]).quotedText,
    "First quoted line\nSecond quoted line",
  )
  assertEquals(record(nodes[0]).editedAt, "2020-03-01T00:00:00Z")
  const external = record(record(nodes[1]).externalUser)
  assertEquals(Object.keys(external), ["id", "name", "displayName"])
  assertEquals(external.displayName, "External 28")
  const bot = record(record(nodes[2]).botActor)
  assertEquals(Object.keys(bot), ["id", "name", "type", "subType"])
  assertEquals(bot.name, "Release Bot")
  assertEquals(bot.type, "github")
  assertEquals(bot.subType, "automation")
  const authorBot = byId.get("c027-author-bot")
  assert(authorBot != null && "utf8" in authorBot.expected.stdout)
  for (
    const label of [
      "@github commented",
      "@Release Bot commented",
      "@ commented",
    ]
  ) {
    assert(authorBot.expected.stdout.utf8.includes(label))
  }
  const authorUnknown = byId.get("c027-author-unknown")
  assert(authorUnknown != null && "utf8" in authorUnknown.expected.stdout)
  assertEquals(
    (authorUnknown.expected.stdout.utf8.match(/@Unknown commented/g) ?? [])
      .length,
    2,
  )
  const sort = byId.get("c027-root-sort-ties")
  assert(sort?.graphql != null && "utf8" in sort.expected.stdout)
  const sortGroup = sort.graphql.groups[0]
  assert(sortGroup.mode === "ordered")
  const sortStep = sortGroup.steps[0]
  assert(sortStep.kind === "graphql" && sortStep.response.kind === "data")
  const sortNodes = record(record(sortStep.response.data).comments).nodes
  assert(Array.isArray(sortNodes))
  assert(
    sortNodes.some((node) =>
      record(node).createdAt === "2020-01-01T11:00:00.500Z"
    ),
  )
  assert(
    sort.expected.stdout.utf8.indexOf("Comment 33") <
      sort.expected.stdout.utf8.indexOf("Comment 16"),
  )
  const extra = byId.get("c027-extra-wire-json")
  assert(extra?.graphql != null && "utf8" in extra.expected.stdout)
  const extraGroup = extra.graphql.groups[0]
  assert(extraGroup.mode === "ordered")
  const first = extraGroup.steps[0]
  assert(
    first.kind === "graphql" && first.response.kind === "transport" &&
      "utf8" in first.response.body,
  )
  const wire = record(record(JSON.parse(first.response.body.utf8)).data)
  assertEquals(Object.keys(record(wire.comments)), [
    "pageInfo",
    "extraConnection",
    "nodes",
  ])
  const extraJson = record(JSON.parse(extra.expected.stdout.utf8))
  assertEquals(Object.keys(extraJson), ["nodes", "pageInfo"])
  const extraNodes = extraJson.nodes
  assert(Array.isArray(extraNodes))
  assertEquals(record(extraNodes[0]).resolvedAt, "2020-03-01T00:00:00Z")
  const reordered = byId.get("c027-reordered-wire-json")
  assert(reordered?.graphql != null && "utf8" in reordered.expected.stdout)
  const reorderGroup = reordered.graphql.groups[0]
  assert(reorderGroup.mode === "ordered")
  const reorderStep = reorderGroup.steps[0]
  assert(
    reorderStep.kind === "graphql" &&
      reorderStep.response.kind === "transport" &&
      "utf8" in reorderStep.response.body,
  )
  const reorderWire = record(
    record(JSON.parse(reorderStep.response.body.utf8)).data,
  )
  const reorderWireNodes = record(reorderWire.comments).nodes
  assert(Array.isArray(reorderWireNodes))
  const reorderOutputNodes =
    record(JSON.parse(reordered.expected.stdout.utf8)).nodes
  assert(Array.isArray(reorderOutputNodes))
  assertEquals(Object.keys(record(reorderOutputNodes[0])), [
    "parent",
    "botActor",
    "externalUser",
    "user",
    "url",
    "editedAt",
    "updatedAt",
    "createdAt",
    "quotedText",
    "body",
    "id",
  ])
  assertEquals(Object.keys(record(record(reorderOutputNodes[0]).user)), [
    "displayName",
    "name",
    "id",
    "extraUser",
  ])
  assertEquals(Object.keys(record(record(reorderOutputNodes[0]).parent)), [
    "extraParent",
    "id",
  ])
  assertEquals(
    Object.keys(record(reorderWireNodes[0])),
    Object.keys(record(reorderOutputNodes[0])),
  )
  const translated = byId.get("c027-unknown-uuid-graphql")
  assert(translated?.graphql != null)
  const translatedGroup = translated.graphql.groups[0]
  assert(translatedGroup.mode === "ordered")
  const translatedStep = translatedGroup.steps[0]
  assert(
    translatedStep.kind === "graphql" &&
      translatedStep.response.kind === "graphqlErrors",
  )
  assertEquals(
    translatedStep.response.errors[0].message,
    "Entity not found: Project",
  )
  assert("utf8" in translated.expected.stderr)
  assert(!translated.expected.stderr.utf8.includes("Pass a project UUID"))
  const firstError = byId.get("c027-graphql-first-error")
  assert(firstError?.graphql != null && "utf8" in firstError.expected.stderr)
  const firstErrorGroup = firstError.graphql.groups[0]
  assert(firstErrorGroup.mode === "ordered")
  const firstErrorStep = firstErrorGroup.steps[0]
  assert(
    firstErrorStep.kind === "graphql" &&
      firstErrorStep.response.kind === "graphqlErrors",
  )
  assertEquals(
    firstErrorStep.response.errors[0].message,
    "Permission denied for comments",
  )
  assertEquals(
    record(firstErrorStep.response.errors[0].extensions).userPresentableMessage,
    "You cannot view this project discussion.",
  )
  assert(
    firstError.expected.stderr.utf8.includes(
      "You cannot view this project discussion.",
    ),
  )
  assert(
    !firstError.expected.stderr.utf8.includes("Permission denied for comments"),
  )
  const secondError = byId.get("c027-graphql-second-error")
  assert(secondError?.graphql != null && "utf8" in secondError.expected.stderr)
  const secondErrorGroup = secondError.graphql.groups[0]
  assert(secondErrorGroup.mode === "ordered")
  const secondErrorStep = secondErrorGroup.steps[1]
  assert(
    secondErrorStep.kind === "graphql" &&
      secondErrorStep.response.kind === "graphqlErrors",
  )
  assertEquals(
    secondErrorStep.response.errors[0].message,
    "Entity not found: Project",
  )
  assertEquals(
    record(secondErrorStep.response.errors[0].extensions)
      .userPresentableMessage,
    "Project discussion access denied.",
  )
  assert(
    secondError.expected.stderr.utf8.includes(
      "Project discussion access denied.",
    ),
  )
  assert(!secondError.expected.stderr.utf8.includes("Project not found:"))
  const anchor = byId.get("c027-url-comment-anchor")
  assert(anchor != null)
  assert(
    anchor.argv.includes(
      "https://linear.app/alpha/project/mobile-app-abc123def456/activity#comment-1234abcd",
    ),
  )
  const resolverMiss = byId.get("c027-name-slug-miss")
  assert(resolverMiss != null && "utf8" in resolverMiss.expected.stderr)
  assert(resolverMiss.expected.stderr.utf8.includes("Pass a project UUID"))
  const baselineBytes = await bundleDigest(bytes)
  const baselineInputs = await inputList(bytes)
  function changed(
    name: string,
    mutate: (value: string) => string,
  ): Map<string, Uint8Array> {
    const next = new Map(bytes)
    const original = next.get(name)
    assert(original != null)
    next.set(
      name,
      new TextEncoder().encode(mutate(new TextDecoder().decode(original))),
    )
    return next
  }
  const mutations: [Map<string, Uint8Array>, boolean][] = [
    [
      changed(
        "c027-leaf-help.json",
        (s) => replaceOnce(s, '"--help"', '"--version"'),
      ),
      true,
    ],
    [
      changed("c027-empty-text.json", (s) =>
        replaceOnce(
          s,
          "No comments found for this project",
          "No comments found for that project",
        )),
      false,
    ],
    [
      changed("c027-uuid-json-full.json", (s) =>
        replaceOnce(
          s,
          '"filterId": "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d"',
          '"filterId": "wrong"',
        )),
      true,
    ],
    [
      changed(
        "c027-two-page-json.json",
        (s) => replaceOnce(s, ',\n                "after": null', ""),
      ),
      true,
    ],
    [
      changed(
        "c027-two-page-json.json",
        (s) => replaceOnce(s, '"after": "one"', '"after": "wrong"'),
      ),
      true,
    ],
    [
      changed(
        "c027-unknown-uuid-null.json",
        (s) => replaceOnce(s, '\\"project\\":null', '\\"project\\":{}'),
      ),
      true,
    ],
    [
      changed(
        "fixtures/workspace-credential/linear/credentials.toml",
        (s) => replaceOnce(s, 'default = "alpha"', 'default = "beta"'),
      ),
      true,
    ],
  ]
  for (const [mutation, inputChanges] of mutations) {
    assertNotEquals(await bundleDigest(mutation), baselineBytes)
    const next = await inputList(mutation)
    if (inputChanges) assertNotEquals(next, baselineInputs)
    else assertEquals(next, baselineInputs)
  }
  function mutatedVariables(
    mutation: Map<string, Uint8Array>,
    filename: string,
    page: number,
  ): Record<string, unknown> {
    const changedBytes = mutation.get(filename)
    assert(changedBytes != null)
    const changedCase = record(
      JSON.parse(new TextDecoder().decode(changedBytes)),
    )
    const groups = record(changedCase.graphql).groups
    assert(Array.isArray(groups))
    const steps = record(groups[0]).steps
    assert(Array.isArray(steps))
    return record(record(record(steps[page]).operation).variables)
  }
  assertThrows(() =>
    assertCommentVariables(
      mutatedVariables(mutations[2][0], "c027-uuid-json-full.json", 0),
      0,
      undefined,
      undefined,
      "wrong-filter",
    )
  )
  assertThrows(() =>
    assertCommentVariables(
      mutatedVariables(mutations[3][0], "c027-two-page-json.json", 0),
      0,
      undefined,
      undefined,
      "omitted-after",
    )
  )
  assertThrows(() =>
    assertCommentVariables(
      mutatedVariables(mutations[4][0], "c027-two-page-json.json", 1),
      1,
      "one",
      "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d",
      "wrong-cursor",
    )
  )
})
