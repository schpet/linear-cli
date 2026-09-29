import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrows,
} from "@std/assert"
import { join, relative } from "@std/path"
import { getOperationAST, parse, print } from "graphql"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c038-frozen-cases/", import.meta.url).pathname
const repo = new URL("../../../", import.meta.url).pathname
const bundleSha256 =
  "e4fbd84731a8e2e6544a5bed60722549d476726fa74816b1ddaef860868b6a32"
const inputsSha256 =
  "a1cdabc4d55d36cd62d91653e21365fe5aafcc97f28072fd264e5b1530e549c2"
const sourceFiles: Record<string, string> = {
  "src/commands/initiative/initiative-view.ts":
    "209097b60e574600bfd49fd51aafbf513be8e884b3d61eb4d47cb13c948321a3",
  "src/utils/linear.ts":
    "57289613743ee08facba18e688d5ef639abb81f8992d250523196ec91f535d0e",
  "src/utils/linear-url.ts":
    "a64eb8e7ecbfe05b5bf030b817df6d89cd060b145bfdcfcf255a6e47b50cb5f5",
  "src/utils/display.ts":
    "35384c6651b6f5dd830093a43b9dc7671aee01185d1b81627e8c1e7bba84222f",
  "src/utils/errors.ts":
    "091c83791b9b20dfac3f73e94f4622e1781703b592803afc80a844eaca30e16a",
  "src/utils/graphql.ts":
    "0eab8c64fabb80b8a3e5f58032de7bec1d3123a47e6a917b6dcb8bccd0f4d322",
  "src/config.ts":
    "f3c3897e3010f4d659a38686537897e3fcbca8f582dbd5616d65db94fb1a85e2",
}
const fixtureNames = [
  "fixtures/config-key-beta/linear.toml",
  "fixtures/workspace-beta/linear.toml",
  "fixtures/workspace-credential/linear/credentials.toml",
]
const rawIds = new Set([
  "app-null-root",
  "detail-extra-wire-json",
  "detail-http-503",
  "detail-missing-id-json",
  "detail-null-root",
  "detail-reordered-wire-json",
  "pipe-raw-lowercase-paused",
  "pipe-raw-unknown-projects",
  "slug-and-name-errors",
  "slug-first-missing-id",
  "slug-http-401-name-hit",
  "web-empty-url",
])
const sequence: Record<string, string> = {
  "alias-help": "0",
  "alias-short-json": "D",
  "ambiguous-name-first": "SND",
  "app-null-root": "D",
  "app-slug-alias-short": "SD",
  "closed-stdout-json": "D",
  "closed-stdout-text": "D",
  "detail-50-projects": "D",
  "detail-entity-not-found": "D",
  "detail-extra-wire-json": "D",
  "detail-full-json": "D",
  "detail-graphql-error": "D",
  "detail-http-503": "D",
  "detail-minimal-json": "D",
  "detail-missing-id-json": "D",
  "detail-null-root": "D",
  "detail-partial-data-error": "D",
  "detail-presentable-not-found": "D",
  "detail-reordered-wire-json": "D",
  "empty-id": "SN",
  "env-workspace-conflict": "0",
  "extra-id": "0",
  "late-workspace": "D",
  "leaf-help": "0",
  "missing-id": "0",
  "name-case-hit": "SND",
  "no-api-key-web": "0",
  "parent-help": "0",
  "pipe-archived-old": "D",
  "pipe-description-markdown": "D",
  "pipe-future-date": "D",
  "pipe-health-target": "D",
  "pipe-icon-title": "D",
  "pipe-known-project-groups": "D",
  "pipe-no-projects": "D",
  "pipe-owner-display": "D",
  "pipe-owner-name-fallback": "D",
  "pipe-raw-lowercase-paused": "D",
  "pipe-raw-unknown-projects": "D",
  "pipe-status-active": "D",
  "pipe-status-proposed": "D",
  "repeated-json": "0",
  "slug-and-name-errors": "SN",
  "slug-and-name-miss": "SN",
  "slug-first-missing-id": "S",
  "slug-hit": "SD",
  "slug-http-401-name-hit": "SND",
  "slug-miss-name-hit": "SND",
  "unknown-cli-workspace": "0",
  "unknown-flag": "0",
  "url-hit": "UD",
  "url-mismatch-before-kind": "0",
  "url-mismatch-config-key": "0",
  "url-mismatch-credential": "0",
  "url-mismatch-env-key": "0",
  "url-miss": "U",
  "url-no-configured-workspace": "UD",
  "url-nonuuid-name-hit": "USND",
  "url-nonuuid-slug-hit": "USD",
  "url-schemeless-host-anchor": "UD",
  "url-unsupported-page": "0",
  "url-wrong-kind": "0",
  "uuid-json": "D",
  "uuid-uppercase": "D",
  "web-app-json-precedence": "D",
  "web-empty-url": "D",
  "web-uuid-short": "D",
}

function record(value: unknown): Record<string, unknown> {
  assert(value != null && typeof value === "object" && !Array.isArray(value))
  return Object.fromEntries(Object.entries(value))
}
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value != null && typeof value === "object") {
    return "{" +
      Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
        .map(([key, entry]) => `${JSON.stringify(key)}:${canonical(entry)}`)
        .join(",") +
      "}"
  }
  return JSON.stringify(value)
}
async function digest(bytes: Uint8Array): Promise<string> {
  return await sha256Hex(bytes)
}
async function inputDigest(bytes: Uint8Array): Promise<string> {
  const value = record(JSON.parse(new TextDecoder().decode(bytes)))
  delete value.expected
  delete value.deviation
  return await digest(new TextEncoder().encode(canonical(value)))
}
async function inputList(bytes: Map<string, Uint8Array>): Promise<string> {
  const lines: string[] = []
  for (const name of [...bytes.keys()].sort()) {
    if (name === "c038-inputs.sha256") continue
    const value = bytes.get(name)
    assert(value != null)
    const hash = name.endsWith(".json")
      ? await inputDigest(value)
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
async function names(): Promise<string[]> {
  const output: string[] = []
  async function walk(path: string): Promise<void> {
    for await (const entry of Deno.readDir(path)) {
      const child = join(path, entry.name)
      const name = relative(root, child)
      assert(!entry.isSymlink, `unexpected C038 symlink ${name}`)
      if (entry.isDirectory) await walk(child)
      else {
        assert(entry.isFile, `unexpected C038 entry ${name}`)
        output.push(name)
      }
    }
  }
  await walk(root)
  return output.sort()
}
async function sourceDocument(path: string, name: string): Promise<string> {
  const source = await Deno.readTextFile(join(repo, path))
  const start = source.indexOf(`query ${name}`)
  assert(start >= 0, `${path}: missing ${name}`)
  const end = source.indexOf("`", start)
  assert(end > start)
  return print(parse(source.slice(start, end)))
}

Deno.test("C038 pins 67 source-only initiative view cases and exact request scripts", async () => {
  const onDisk = await names()
  const caseNames = Object.keys(sequence).map((id) => `c038-${id}.json`).sort()
  assertEquals(caseNames.length, 67)
  assertEquals(
    onDisk,
    [...caseNames, "c038-inputs.sha256", ...fixtureNames].sort(),
  )
  const bytes = new Map<string, Uint8Array>()
  for (const name of onDisk) {
    bytes.set(name, await Deno.readFile(join(root, name)))
  }
  assertEquals(await bundleDigest(bytes), bundleSha256)
  const listed = await inputList(bytes)
  assertEquals(
    new TextDecoder().decode(bytes.get("c038-inputs.sha256")),
    listed,
  )
  assertEquals(await digest(new TextEncoder().encode(listed)), inputsSha256)
  assertEquals(
    new TextDecoder().decode(bytes.get(fixtureNames[0])),
    'workspace = "beta"\napi_key = "lin_api_fake_config"\n',
  )
  assertEquals(
    new TextDecoder().decode(bytes.get(fixtureNames[1])),
    'workspace = "beta"\n',
  )
  assertEquals(
    new TextDecoder().decode(bytes.get(fixtureNames[2])),
    'default = "alpha"\nalpha = "lin_api_fake_alpha"\nbeta = "lin_api_fake_beta"\n',
  )

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
    baseline.schemaSha256,
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
  )
  assertEquals(
    baseline.lockSha256,
    "3da729da08fe6d48236e055b2eaac95788b5e5ccfd0f66dacdc5f6a0b0b96403",
  )
  assertEquals(
    baseline.binarySha256,
    "a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835",
  )
  for (const [path, hash] of Object.entries(sourceFiles)) {
    assertEquals(
      await digest(await Deno.readFile(join(repo, path))),
      hash,
      path,
    )
  }
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const route = manifest.routes.find((item) =>
    item.path === "linear initiative view"
  )
  assert(route != null)
  assert(typeof route.path === "string" && typeof route.source === "string")
  assertEquals(route.aliases, ["v"])
  assertEquals(route.source, "src/commands/initiative/initiative-view.ts")
  const documents = new Map<string, string>([
    ["D", await sourceDocument(route.source, "GetInitiativeDetails")],
    ["S", await sourceDocument(route.source, "GetInitiativeBySlugForView")],
    ["N", await sourceDocument(route.source, "GetInitiativeByNameForView")],
    [
      "U",
      await sourceDocument("src/utils/linear.ts", "ResolveInitiativeBySlug"),
    ],
  ])
  const loaded = await loadCases(root, new Set([route.path]), "c038-")
  assertEquals(loaded.map((entry) => `${entry.spec.id}.json`), caseNames)
  assertEquals(loaded.filter((entry) => entry.spec.graphql != null).length, 51)
  assertEquals(
    loaded.reduce(
      (n, entry) => n + (entry.spec.graphql?.expectedRequests ?? 0),
      0,
    ),
    72,
  )
  assertEquals(
    loaded.some((entry) => entry.spec.id === "c038-web-details-error"),
    false,
  )
  const duplicateInputs = new Map<string, string[]>()
  for (const entry of loaded) {
    const raw = bytes.get(`${entry.spec.id}.json`)
    assert(raw != null)
    const value = record(JSON.parse(new TextDecoder().decode(raw)))
    for (const key of ["id", "reason", "expected", "deviation"]) {
      delete value[key]
    }
    const identity = canonical(value)
    duplicateInputs.set(identity, [
      ...(duplicateInputs.get(identity) ?? []),
      entry.spec.id,
    ])
  }
  assertEquals([...duplicateInputs.values()].filter((ids) => ids.length > 1), [
    ["c038-closed-stdout-json", "c038-uuid-json"],
    ["c038-closed-stdout-text", "c038-pipe-no-projects"],
  ])
  for (const entry of loaded) {
    const spec = entry.spec
    const id = spec.id.slice(5)
    assertEquals(spec.route, route.path)
    assertEquals(spec.deviation, null)
    assertEquals(spec.expected.fileEffects, [])
    assertEquals(spec.timeoutMs, 30000)
    assertEquals(spec.outputCapBytes, 4194304)
    assertEquals(spec.fixtureServer, null)
    assertEquals(spec.env.PATH, "{{bin}}")
    assertEquals(spec.env.TZ, "UTC")
    assertEquals(spec.env.LANG, "C.UTF-8")
    assertEquals(spec.env.LC_ALL, "C.UTF-8")
    assertEquals(spec.env.NO_COLOR, "1")
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1")
    assert(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
    )
    const expectedSequence = sequence[id]
    assert(expectedSequence != null)
    const gql = spec.graphql
    if (expectedSequence === "0") {
      assert(gql == null)
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
      )
      continue
    }
    assert(gql != null)
    assertEquals(
      spec.env.LINEAR_GRAPHQL_ENDPOINT,
      "http://127.0.0.1:{{fixturePort}}/graphql",
    )
    assertEquals(gql.path, "/graphql")
    assertEquals(gql.schemaSha256, baseline.schemaSha256)
    assertEquals(gql.initialRecords, {})
    assertEquals(gql.expectedRecords, {})
    assertEquals(gql.groups.length, 1)
    const group = gql.groups[0]
    assert(group.mode === "ordered")
    assertEquals(gql.expectedRequests, expectedSequence.length)
    assertEquals(group.steps.length, expectedSequence.length)
    for (const [index, step] of group.steps.entries()) {
      assert(step.kind === "graphql")
      assertEquals(step.effects, [])
      assertEquals(step.identity.headers, {})
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0")
      assertEquals(
        step.identity.authorization,
        id === "late-workspace"
          ? "lin_api_fake_beta"
          : id === "url-no-configured-workspace"
          ? "lin_api_fake"
          : "lin_api_fake_alpha",
      )
      const code = expectedSequence[index]
      const parsed = parse(step.operation.document)
      const operation = getOperationAST(parsed)
      assert(operation != null)
      const name = {
        D: "GetInitiativeDetails",
        S: "GetInitiativeBySlugForView",
        N: "GetInitiativeByNameForView",
        U: "ResolveInitiativeBySlug",
      }[code]
      assertEquals(operation.name?.value, name)
      assertEquals(step.operation.operationName ?? name, name)
      assertEquals(
        print(parsed),
        documents.get(code),
        `${id}: document ${index}`,
      )
      const variables = record(step.operation.variables)
      const keys = Object.keys(variables).sort()
      assertEquals(
        keys,
        code === "D"
          ? ["id"]
          : code === "S"
          ? ["slugId"]
          : code === "N"
          ? ["name"]
          : ["includeArchived", "slugId"],
      )
      if (code === "U") {
        assertEquals(variables.includeArchived, false)
        assert(
          typeof variables.slugId === "string" &&
            /^[0-9a-f]{12}$/.test(variables.slugId),
        )
      }
      if (code === "D") {
        assert(typeof variables.id === "string")
        assert(!step.operation.document.includes("first:"))
        assert(!step.operation.document.includes("pageInfo"))
      }
      if (code === "S" || code === "N") {
        assert(typeof variables[code === "S" ? "slugId" : "name"] === "string")
      }
      assertEquals(
        step.response.kind === "transport",
        rawIds.has(id) &&
          (id === "slug-and-name-errors" || id === "slug-http-401-name-hit"
            ? index === 0
            : true),
      )
    }
  }
  function rawCase(id: string): Record<string, unknown> {
    const value = bytes.get(`c038-${id}.json`)
    assert(value != null)
    return record(JSON.parse(new TextDecoder().decode(value)))
  }
  function rawStep(id: string, index: number): Record<string, unknown> {
    const groups = record(rawCase(id).graphql).groups
    assert(Array.isArray(groups) && groups.length === 1)
    const steps = record(groups[0]).steps
    assert(Array.isArray(steps))
    return record(steps[index])
  }
  function vars(id: string, index: number): Record<string, unknown> {
    return record(record(rawStep(id, index).operation).variables)
  }
  function response(id: string, index: number): Record<string, unknown> {
    return record(rawStep(id, index).response)
  }
  assertEquals(
    vars("uuid-uppercase", 0).id,
    "00000000-0000-4000-9000-0000000000AB",
  )
  assertEquals(vars("empty-id", 0), { slugId: "" })
  assertEquals(vars("empty-id", 1), { name: "" })
  assertEquals(vars("url-schemeless-host-anchor", 0), {
    slugId: "0000000000ab",
    includeArchived: false,
  })
  assertEquals(vars("url-nonuuid-slug-hit", 1).slugId, "legacy-id")
  assertEquals(vars("url-nonuuid-name-hit", 1).slugId, "legacy-name")
  assertEquals(vars("url-nonuuid-name-hit", 2).name, "legacy-name")
  assertEquals(
    vars("url-nonuuid-name-hit", 3).id,
    "00000000-0000-4000-9000-000000000050",
  )
  assertEquals(vars("slug-hit", 1).id, "00000000-0000-4000-9000-000000000038")
  assertEquals(
    vars("ambiguous-name-first", 2).id,
    "00000000-0000-4000-9000-000000000045",
  )
  assertEquals(vars("slug-http-401-name-hit", 1).name, "Lookup fallback")
  const mismatchStderr = record(rawCase("url-mismatch-env-key").expected).stderr
  for (
    const id of [
      "url-mismatch-config-key",
      "url-mismatch-credential",
      "url-mismatch-before-kind",
    ]
  ) {
    assertEquals(record(rawCase(id).expected).stderr, mismatchStderr, id)
  }
  assertEquals(vars("web-uuid-short", 0).id, vars("uuid-json", 0).id)
  assertEquals(
    record(record(response("web-uuid-short", 0).data).initiative).id,
    vars("uuid-json", 0).id,
  )
  const firstWire = record(response("slug-first-missing-id", 0).body).utf8
  assert(typeof firstWire === "string")
  const firstNodes = record(record(JSON.parse(firstWire)).data).initiatives
  const firstList = record(firstNodes).nodes
  assert(Array.isArray(firstList) && firstList.length === 2)
  assertEquals(Object.keys(record(firstList[0])), ["slugId"])
  assertEquals(record(firstList[1]).id, "00000000-0000-4000-9000-000000000047")
  const large = record(
    record(response("detail-50-projects", 0).data).initiative,
  )
  const projects = record(large.projects).nodes
  assert(Array.isArray(projects) && projects.length === 50)
  assertEquals(new Set(projects.map((project) => record(project).id)).size, 50)
  assertEquals(
    record(response("pipe-future-date", 0).data).initiative != null,
    true,
  )
  const future = record(record(response("pipe-future-date", 0).data).initiative)
  for (const key of ["createdAt", "updatedAt", "archivedAt"]) {
    assert(typeof future[key] === "string" && future[key].startsWith("2999-"))
  }
  const json = record(rawCase("detail-extra-wire-json").expected)
  assert("utf8" in record(json.stdout))
  assertEquals(record(rawCase("closed-stdout-json").expected).stdout, {
    mode: "closed-at-start",
  })
  assertEquals(record(rawCase("closed-stdout-text").expected).stdout, {
    mode: "closed-at-start",
  })
})

Deno.test("C038 input and bundle drift controls reject case and fixture mutations", async () => {
  const original = new Map<string, Uint8Array>()
  for (const name of await names()) {
    original.set(name, await Deno.readFile(join(root, name)))
  }
  assertEquals(await bundleDigest(original), bundleSha256)
  assertEquals(
    await digest(new TextEncoder().encode(await inputList(original))),
    inputsSha256,
  )
  async function changed(
    id: string,
    mutate: (value: Record<string, unknown>) => void,
    inputChanged = true,
  ): Promise<void> {
    const name = `c038-${id}.json`
    const bytes = original.get(name)
    assert(bytes != null)
    const value = record(JSON.parse(new TextDecoder().decode(bytes)))
    mutate(value)
    const revised = new Map(original)
    revised.set(name, new TextEncoder().encode(JSON.stringify(value) + "\n"))
    assertNotEquals(await bundleDigest(revised), bundleSha256, id)
    const input = await digest(
      new TextEncoder().encode(await inputList(revised)),
    )
    if (inputChanged) assertNotEquals(input, inputsSha256, id)
    else assertEquals(input, inputsSha256, id)
  }
  function step(
    value: Record<string, unknown>,
    index: number,
  ): Record<string, unknown> {
    const groups = record(value.graphql).groups
    assert(Array.isArray(groups) && groups.length === 1)
    const steps = record(groups[0]).steps
    assert(Array.isArray(steps))
    return record(steps[index])
  }
  function setExisting(target: unknown, key: string, value: unknown): void {
    assert(target != null && typeof target === "object")
    assert(Reflect.has(target, key))
    assert(Reflect.set(target, key, value))
  }
  await changed("alias-short-json", (value) => {
    const argv = value.argv
    assert(Array.isArray(argv))
    argv[1] = "view"
  })
  await changed("url-hit", (value) => {
    const operation = step(value, 0).operation
    const variables = record(record(operation).variables)
    variables.includeArchived = true
    setExisting(operation, "variables", variables)
  })
  await changed("slug-miss-name-hit", (value) => {
    const groups = record(value.graphql).groups
    assert(Array.isArray(groups))
    const steps = record(groups[0]).steps
    assert(Array.isArray(steps))
    ;[steps[0], steps[1]] = [steps[1], steps[0]]
  })
  await changed("slug-hit", (value) => {
    const operation = record(step(value, 1).operation)
    const variables = record(operation.variables)
    variables.id = "00000000-0000-4000-9000-000000000099"
    setExisting(step(value, 1).operation, "variables", variables)
  })
  await changed("web-uuid-short", (value) => {
    const expected = record(value.expected)
    const stdout = record(expected.stdout).utf8
    assert(typeof stdout === "string" && stdout.includes("web browser"))
    expected.stdout = { utf8: stdout.replace("web browser", "Linear.app") }
    value.expected = expected
  }, false)
  await changed("pipe-known-project-groups", (value) => {
    const response = record(step(value, 0).response)
    const data = record(response.data)
    const initiative = record(data.initiative)
    const projects = record(initiative.projects)
    const nodes = projects.nodes
    assert(Array.isArray(nodes))
    nodes.reverse()
    projects.nodes = nodes
    initiative.projects = projects
    data.initiative = initiative
    response.data = data
    setExisting(step(value, 0).response, "data", data)
  })
  await changed("detail-full-json", (value) => {
    const expected = record(value.expected)
    expected.exitCode = 9
    value.expected = expected
  }, false)
  await changed("closed-stdout-text", (value) => {
    const expected = record(value.expected)
    expected.stdout = { utf8: "" }
    value.expected = expected
  }, false)
  const fixture = fixtureNames[1]
  const revised = new Map(original)
  revised.set(fixture, new TextEncoder().encode('workspace = "alpha"\n'))
  assertNotEquals(await bundleDigest(revised), bundleSha256)
  assertNotEquals(
    await digest(new TextEncoder().encode(await inputList(revised))),
    inputsSha256,
  )
  assertThrows(() => record(null))
})
