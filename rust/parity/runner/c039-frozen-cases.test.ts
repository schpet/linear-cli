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

const root = new URL("./c039-frozen-cases/", import.meta.url).pathname
const repo = new URL("../../../", import.meta.url).pathname
const bundleSha256 =
  "0ffcd74bb4a2bdf8868d33b306811084f4fbbbf394552f7649171e15f1500424"
const inputsSha256 =
  "5ce1424bd3f9675f6767c499088d0f298f9d8f909a95f8d6ab679bfaf7da4d8f"
const sourceFiles: Record<string, string> = {
  "src/commands/initiative/initiative-create.ts":
    "1d5caa60e4be8c9846ba3c4b00dd50f730d5b85e11e177764cebdc60c61e75c9",
  "src/commands/initiative/initiative.ts":
    "7d990eb6fa5bfd6ddc0bfd7116b319d6fb843098c436cd18e845725d439225cb",
  "src/utils/linear.ts":
    "57289613743ee08facba18e688d5ef639abb81f8992d250523196ec91f535d0e",
  "src/utils/linear-url.ts":
    "a64eb8e7ecbfe05b5bf030b817df6d89cd060b145bfdcfcf255a6e47b50cb5f5",
  "src/utils/errors.ts":
    "091c83791b9b20dfac3f73e94f4622e1781703b592803afc80a844eaca30e16a",
  "src/utils/graphql.ts":
    "0eab8c64fabb80b8a3e5f58032de7bec1d3123a47e6a917b6dcb8bccd0f4d322",
  "src/utils/hyperlink.ts":
    "6c575144c5eb26fe2c4528223288f8fd1e866078c11e6879091223928f14ee6f",
  "src/cli.ts":
    "d6e7b1d724e6bc28c3ff6ecc91095907274c8c6b84a37bd1ce2c42d3f44788bf",
  "src/main.ts":
    "2d402dc59c8f061d7b73ab2af6bd2933ec41d62401a7baaee6b5420b4e690468",
  "deno.lock":
    "3da729da08fe6d48236e055b2eaac95788b5e5ccfd0f66dacdc5f6a0b0b96403",
  "graphql/schema.graphql":
    "eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a",
}
const sequence: Record<string, string> = {
  "leaf-help": "0",
  "short-help": "0",
  "name-missing-value": "0",
  "unknown-option": "0",
  "surplus-positional": "0",
  "duplicate-name": "0",
  "no-key-before-validation": "0",
  "icon-interactive-no-name-pipe": "0",
  "empty-name": "0",
  "empty-description-value": "0",
  "invalid-status-before-color": "0",
  "invalid-color-before-date": "0",
  "invalid-date-before-owner": "0",
  "owner-me": "VM",
  "owner-self": "VM",
  "owner-email-priority": "UM",
  "owner-display-priority": "UM",
  "owner-first-fallback": "UM",
  "owner-miss": "U",
  "owner-profile-url": "0",
  "minimal-create": "M",
  "full-fields-pipe-interactive": "M",
  "empty-optionals-empty-url": "M",
  "whitespace-name-impossible-date": "M",
  "false-success": "M",
  "graphql-errors": "M",
  "partial-data-errors": "M",
  "http-500": "M",
  "closed-stdout-after-create": "M",
}
const writes = new Set([
  "owner-me",
  "owner-self",
  "owner-email-priority",
  "owner-display-priority",
  "owner-first-fallback",
  "minimal-create",
  "full-fields-pipe-interactive",
  "empty-optionals-empty-url",
  "whitespace-name-impossible-date",
  "partial-data-errors",
  "closed-stdout-after-create",
])
const fixtureName = "fixtures/workspace-credential/linear/credentials.toml"

function record(value: unknown): Record<string, unknown> {
  assert(value != null && typeof value === "object" && !Array.isArray(value))
  return Object.fromEntries(Object.entries(value))
}
function assertCreationBinding(
  effectRecord: string,
  after: unknown,
  responseData: unknown,
  expectedRecords: Record<string, unknown>,
): void {
  const payload = record(record(responseData).initiativeCreate)
  assertEquals(payload.success, true)
  const initiative = record(payload.initiative)
  assert(typeof initiative.id === "string")
  assertEquals(effectRecord, `Initiative:${initiative.id}`)
  assertEquals(after, initiative)
  assertEquals(expectedRecords[effectRecord], initiative)
}
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value != null && typeof value === "object") {
    return "{" + Object.entries(value).sort(([a], [b]) =>
      a < b ? -1 : a > b ? 1 : 0
    )
      .map(([key, entry]) => `${JSON.stringify(key)}:${canonical(entry)}`).join(
        ",",
      ) +
      "}"
  }
  return JSON.stringify(value)
}
async function digest(bytes: Uint8Array): Promise<string> {
  return await sha256Hex(bytes)
}
async function names(): Promise<string[]> {
  const output: string[] = []
  async function walk(path: string): Promise<void> {
    for await (const entry of Deno.readDir(path)) {
      const child = join(path, entry.name)
      const name = relative(root, child)
      assert(!entry.isSymlink && (entry.isDirectory || entry.isFile), name)
      if (entry.isDirectory) await walk(child)
      else output.push(name)
    }
  }
  await walk(root)
  return output.sort()
}
async function document(path: string, name: string): Promise<string> {
  const source = await Deno.readTextFile(join(repo, path))
  const match = source.match(
    new RegExp(`(?:query|mutation) ${name}\\b[^\x60]*`),
  )
  assert(match != null, name)
  return print(parse(match[0]))
}

Deno.test("C039 pins 29 source-only initiative create cases, ordered writes and exact provenance", async () => {
  const onDisk = await names()
  const caseNames = Object.keys(sequence).map((id) => `c039-${id}.json`).sort()
  assertEquals(caseNames.length, 29)
  assertEquals(onDisk, [...caseNames, "c039-inputs.sha256", fixtureName].sort())
  const bytes = new Map<string, Uint8Array>()
  for (const name of onDisk) {
    bytes.set(name, await Deno.readFile(join(root, name)))
  }
  const bundle = (await Promise.all(
    onDisk.map(async (name) => `${name}\0${await digest(bytes.get(name)!)}\n`),
  )).join("")
  assertEquals(await digest(new TextEncoder().encode(bundle)), bundleSha256)
  const inputs = (await Promise.all(
    onDisk.filter((name) => name !== "c039-inputs.sha256").map(
      async (name) => {
        let content = bytes.get(name)!
        if (name.endsWith(".json")) {
          const value = record(JSON.parse(new TextDecoder().decode(content)))
          delete value.expected
          delete value.deviation
          content = new TextEncoder().encode(canonical(value))
        }
        return `${await digest(content)}  ${name}\n`
      },
    ),
  )).join("")
  assertEquals(
    new TextDecoder().decode(bytes.get("c039-inputs.sha256")),
    inputs,
  )
  assertEquals(await digest(new TextEncoder().encode(inputs)), inputsSha256)
  assertEquals(
    new TextDecoder().decode(bytes.get(fixtureName)),
    'default = "alpha"\nalpha = "lin_api_fake_alpha"\nbeta = "lin_api_fake_beta"\n',
  )
  for (const [path, hash] of Object.entries(sourceFiles)) {
    assertEquals(
      await digest(await Deno.readFile(join(repo, path))),
      hash,
      path,
    )
  }
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
  assertEquals(baseline.schemaSha256, sourceFiles["graphql/schema.graphql"])
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const route = manifest.routes.find((item) =>
    item.path === "linear initiative create"
  )
  assert(route != null)
  assert(typeof route.path === "string" && typeof route.source === "string")
  assertEquals(route.source, "src/commands/initiative/initiative-create.ts")
  const docs = new Map(["M", "V", "U"].map((code) => [code, ""]))
  docs.set("M", await document(route.source, "CreateInitiative"))
  docs.set("V", await document("src/utils/linear.ts", "GetViewerId"))
  docs.set("U", await document("src/utils/linear.ts", "LookupUser"))
  const loaded = await loadCases(root, new Set([route.path]), "c039-")
  assertEquals(loaded.map((item) => `${item.spec.id}.json`), caseNames)
  assertEquals(loaded.filter((item) => item.spec.graphql != null).length, 15)
  assertEquals(
    loaded.reduce(
      (n, item) => n + (item.spec.graphql?.expectedRequests ?? 0),
      0,
    ),
    20,
  )
  for (const item of loaded) {
    const spec = item.spec
    const id = spec.id.slice(5)
    assertEquals(spec.deviation, null, id)
    assertEquals(spec.expected.fileEffects, [], id)
    assertEquals(spec.fixtureServer, null, id)
    assertEquals(spec.env.PATH, "{{bin}}", id)
    assertEquals(spec.env.NO_COLOR, "1", id)
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1", id)
    assertEquals(spec.env.TZ, "UTC", id)
    assertEquals(spec.env.LANG, "C.UTF-8", id)
    assertEquals(spec.env.LC_ALL, "C.UTF-8", id)
    const code = sequence[id]
    assert(code != null, id)
    if (code === "0") {
      assert(spec.graphql == null, id)
      assertEquals(
        spec.env.LINEAR_GRAPHQL_ENDPOINT,
        "http://127.0.0.1:1/graphql",
        id,
      )
      continue
    }
    const gql = spec.graphql
    assert(gql != null, id)
    assertEquals(
      spec.env.LINEAR_GRAPHQL_ENDPOINT,
      "http://127.0.0.1:{{fixturePort}}/graphql",
      id,
    )
    assertEquals(gql.path, "/graphql", id)
    assertEquals(gql.schemaSha256, baseline.schemaSha256, id)
    assertEquals(gql.initialRecords, {}, id)
    assertEquals(gql.expectedRequests, code.length, id)
    assertEquals(gql.groups.length, 1, id)
    const group = gql.groups[0]
    assert(group.mode === "ordered", id)
    assertEquals(group.steps.length, code.length, id)
    for (const [index, step] of group.steps.entries()) {
      assert(step.kind === "graphql")
      const letter = code[index]
      const operation = getOperationAST(parse(step.operation.document))
      assert(operation != null)
      assertEquals(
        operation.operation,
        letter === "M" ? "mutation" : "query",
        id,
      )
      assertEquals(
        operation.name?.value,
        { M: "CreateInitiative", V: "GetViewerId", U: "LookupUser" }[letter],
        id,
      )
      assertEquals(
        step.operation.operationName ?? operation.name?.value,
        operation.name?.value,
        id,
      )
      assertEquals(print(parse(step.operation.document)), docs.get(letter), id)
      assertEquals(step.identity, {
        authorization: "lin_api_fake_alpha",
        userAgent: "schpet-linear-cli/2.6.0",
        headers: {},
      }, id)
      if (letter !== "M") {
        assertEquals(step.effects, [], id)
        assertEquals(step.response.kind, "data", id)
      } else {
        const vars = record(step.operation.variables)
        assertEquals(Object.keys(vars), ["input"], id)
        const input = record(vars.input)
        assert(typeof input.name === "string", id)
        assert(!Object.values(input).some((value) => value === null), id)
        if (id === "minimal-create" || id === "empty-optionals-empty-url") {
          assertEquals(Object.keys(input), ["name"], id)
        }
        if (id === "full-fields-pipe-interactive") {
          assertEquals(input.status, "Active")
        }
        if (id === "whitespace-name-impossible-date") {
          assertEquals(input, { name: "  ", targetDate: "2026-02-30" })
        }
        assertEquals(step.effects.length, writes.has(id) ? 1 : 0, id)
        if (writes.has(id)) {
          const effect = step.effects[0]
          assertEquals(effect.kind, "put", id)
          if (effect.kind === "put") {
            assertEquals(effect.before, { absent: true }, id)
            if (
              step.response.kind === "data" ||
              step.response.kind === "graphqlErrors"
            ) {
              assertCreationBinding(
                effect.record,
                effect.after,
                step.response.data,
                gql.expectedRecords,
              )
            }
          }
        }
        assertEquals(
          step.response.kind,
          id === "http-500"
            ? "transport"
            : id === "graphql-errors" || id === "partial-data-errors"
            ? "graphqlErrors"
            : "data",
          id,
        )
        assertEquals(
          step.partialEffects === true,
          id === "partial-data-errors",
          id,
        )
        if (id === "false-success") {
          assert(step.response.kind === "data")
          assertEquals(
            record(record(step.response.data).initiativeCreate).success,
            false,
          )
        }
        if (id === "graphql-errors") {
          assert(step.response.kind === "graphqlErrors")
          assertEquals(step.response.data, null)
          assertEquals(step.effects, [])
        }
        if (id === "partial-data-errors") {
          assert(step.response.kind === "graphqlErrors")
          assertEquals(
            record(record(step.response.data).initiativeCreate).success,
            true,
          )
          assertEquals(step.effects.length, 1)
        }
        if (id === "http-500") {
          assert(step.response.kind === "transport")
          assertEquals(step.response.status, 500)
          assertEquals(step.response.headers, { "content-type": "text/plain" })
          assertEquals(step.response.body, { utf8: "upstream unavailable" })
        }
      }
    }
    assertEquals(
      Object.keys(gql.expectedRecords).length,
      writes.has(id) ? 1 : 0,
      id,
    )
  }
  const raw = (id: string) =>
    record(JSON.parse(new TextDecoder().decode(bytes.get(`c039-${id}.json`)!)))
  assertEquals(record(raw("false-success").graphql).expectedRecords, {})
  assertEquals(record(raw("duplicate-name").expected).exit, { code: 2 })
  assertEquals(record(raw("empty-name").expected).exit, { code: 2 })
  assertEquals(record(raw("empty-description-value").expected).exit, {
    code: 2,
  })
  assertEquals(raw("empty-description-value").graphql, undefined)
  assertEquals(record(raw("closed-stdout-after-create").expected).stdout, {
    mode: "closed-at-start",
  })
  assertNotEquals(
    record(raw("partial-data-errors").graphql).expectedRecords,
    {},
  )
  const minimal = record(raw("minimal-create").graphql)
  const groups = minimal.groups
  assert(Array.isArray(groups) && groups.length === 1)
  const steps = record(groups[0]).steps
  assert(Array.isArray(steps) && steps.length === 1)
  const step = record(steps[0])
  const effects = step.effects
  assert(Array.isArray(effects) && effects.length === 1)
  const effect = record(effects[0])
  assertThrows(() =>
    assertCreationBinding(
      "Initiative:wrong-id",
      effect.after,
      record(step.response).data,
      record(minimal.expectedRecords),
    )
  )
})
