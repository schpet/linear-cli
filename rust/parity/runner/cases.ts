// Case corpus loading: schema validation, manifest route binding, fixture
// existence, and placeholder resolution into concrete bytes and paths.
import { join } from "@std/path"
import {
  getNamedType,
  getOperationAST,
  type GraphQLSchema,
  type GraphQLType,
  isInterfaceType,
  isListType,
  isNonNullType,
  isObjectType,
  isUnionType,
  parse,
} from "graphql"
import { decodeByteValue } from "./bytes.ts"
import { buildPinnedSchema, matchGraphQL } from "./graphql-match.ts"
import {
  type CaseSpec,
  GraphQLFixtureSchema,
  type InteractionSpec,
  parseCase,
  SchemaError,
  substitute,
  type SubstitutionName,
} from "./schema.ts"
import * as v from "valibot"

export interface LoadedCase {
  file: string
  spec: CaseSpec
  /** Absolute fixture directory copied into the sandbox cwd, or null for empty. */
  fixtureDir: string | null
  configFixtureDir: string | null
}

export const LANE_CLEANUP_MARGIN_MS = 1000

export function checkLaneDeadline(
  spec: CaseSpec,
  effectiveTimeoutMs: number,
): void {
  const total = spec.graphql?.groups.reduce(
    (sum, group) => sum + (group.mode === "lanes" ? group.timeoutMs : 0),
    0,
  ) ?? 0
  if (total > 0 && total + LANE_CLEANUP_MARGIN_MS >= effectiveTimeoutMs) {
    throw new SchemaError(
      `case ${spec.id}: lane deadlines plus cleanup margin must be strictly below timeoutMs`,
    )
  }
}

function headersConflict(
  a: Readonly<Record<string, string>>,
  b: Readonly<Record<string, string>>,
): boolean {
  for (const [name, value] of Object.entries(a)) {
    const other = Object.entries(b).find(([key]) =>
      key.toLowerCase() === name.toLowerCase()
    )
    if (other != null && other[1] !== value) return true
  }
  return false
}

function firstStepsOverlap(
  a: InteractionSpec,
  b: InteractionSpec,
  schema: GraphQLSchema,
): boolean {
  if (a.kind !== b.kind) return false
  if (a.kind === "asset" && b.kind === "asset") {
    return a.method === b.method && a.path === b.path &&
      !headersConflict(a.requiredHeaders, b.requiredHeaders) &&
      !a.forbiddenHeaders.some((name) =>
        Object.keys(b.requiredHeaders).some((key) =>
          key.toLowerCase() === name.toLowerCase()
        )
      ) &&
      !b.forbiddenHeaders.some((name) =>
        Object.keys(a.requiredHeaders).some((key) =>
          key.toLowerCase() === name.toLowerCase()
        )
      )
  }
  if (a.kind !== "graphql" || b.kind !== "graphql") {
    throw new Error("unexpected interaction kind")
  }
  if (
    a.identity.authorization !== b.identity.authorization ||
    headersConflict(a.identity.headers, b.identity.headers)
  ) return false
  if (
    a.response.kind === "validationErrors" ||
    b.response.kind === "validationErrors"
  ) {
    return a.operation.document.trim() === b.operation.document.trim()
  }
  return matchGraphQL(a.operation, b.operation, schema).matches ||
    matchGraphQL(b.operation, a.operation, schema).matches
}

function sameEffectFreeLane(
  a: readonly InteractionSpec[],
  b: readonly InteractionSpec[],
): boolean {
  const shape = (steps: readonly InteractionSpec[]) =>
    steps.map(({ id: _id, ...step }) => step)
  return JSON.stringify(shape(a)) === JSON.stringify(shape(b)) &&
    a.every((step) => step.kind !== "graphql" || step.effects.length === 0)
}

interface TypedRecordRead {
  key: string
  type: GraphQLType
}

function laneDependencies(
  steps: readonly InteractionSpec[],
  schema: GraphQLSchema,
  recordValues: ReadonlyMap<string, readonly unknown[]>,
): { writes: Set<string>; reads: Set<string> } {
  const writes = new Set<string>()
  const reads = new Set<string>()
  const typedReads: TypedRecordRead[] = []
  for (const step of steps) {
    if (step.kind !== "graphql") continue
    for (const effect of step.effects) {
      writes.add(effect.record)
    }
    if (
      step.response.kind === "data" || step.response.kind === "graphqlErrors"
    ) {
      const operation = getOperationAST(
        parse(step.operation.document),
        step.operation.operationName,
      )
      const root = operation?.operation === "query"
        ? schema.getQueryType()
        : operation?.operation === "mutation"
        ? schema.getMutationType()
        : schema.getSubscriptionType()
      if (root != null) {
        checkResponseReferences(
          step.response.data,
          root,
          schema,
          null,
          step.id,
          reads,
          typedReads,
        )
      }
    }
  }
  // A returned record can itself point at another record. Every stored version
  // is possible while lanes interleave, so close reads over initial and after
  // values before comparing them with writes in another lane.
  const visited = new Set<string>()
  for (let index = 0; index < typedReads.length; index++) {
    const { key, type } = typedReads[index]
    const visit = `${key}\u0000${getNamedType(type).name}`
    if (visited.has(visit)) continue
    visited.add(visit)
    for (const value of recordValues.get(key) ?? []) {
      checkResponseReferences(
        value,
        type,
        schema,
        null,
        `record ${key}`,
        reads,
        typedReads,
      )
    }
  }
  return { writes, reads }
}

function checkRedirects(steps: readonly InteractionSpec[], file: string): void {
  for (const [index, step] of steps.entries()) {
    if (step.kind !== "asset" || step.response.location == null) continue
    const next = steps[index + 1]
    if (next?.kind !== "asset" || next.path !== step.response.location) {
      throw new SchemaError(
        `${file}: asset redirect must target the immediately following step in the same lane or group`,
      )
    }
  }
}

async function inspectConfigFixture(
  directory: string,
  file: string,
): Promise<void> {
  async function walk(path: string): Promise<void> {
    for await (const entry of Deno.readDir(path)) {
      const child = join(path, entry.name)
      const info = await Deno.lstat(child)
      if (info.isSymlink || (!info.isDirectory && !info.isFile)) {
        throw new SchemaError(
          `${file}: configFixture contains a symlink or unsupported entry`,
        )
      }
      if (info.isDirectory) await walk(child)
      else {
        let contents: string
        try {
          contents = new TextDecoder("utf-8", { fatal: true }).decode(
            await Deno.readFile(child),
          )
        } catch {
          throw new SchemaError(
            `${file}: configFixture must contain UTF-8 text files`,
          )
        }
        if (/lin_(?:api|oauth)_(?!fake)/i.test(contents)) {
          throw new SchemaError(
            `${file}: configFixture contains a non-fake Linear credential`,
          )
        }
      }
    }
  }
  await walk(directory)
}

export interface ResolvedCase {
  argv: string[]
  stdin: Uint8Array
  env: Record<string, string>
  expected: {
    exit: CaseSpec["expected"]["exit"]
    stdout: Uint8Array
    stderr: Uint8Array
    fileEffects: CaseSpec["expected"]["fileEffects"]
  }
  fixtureServer: CaseSpec["fixtureServer"]
  graphql: CaseSpec["graphql"]
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

function checkResponseReferences(
  value: unknown,
  type: GraphQLType,
  schema: GraphQLSchema,
  known: ReadonlySet<string> | null,
  label: string,
  found: Set<string> | null = null,
  typedReads: TypedRecordRead[] | null = null,
): void {
  if (isNonNullType(type)) {
    return checkResponseReferences(
      value,
      type.ofType,
      schema,
      known,
      label,
      found,
      typedReads,
    )
  }
  if (value == null) return
  if (isListType(type)) {
    if (Array.isArray(value)) {
      value.forEach((item, index) =>
        checkResponseReferences(
          item,
          type.ofType,
          schema,
          known,
          `${label}[${index}]`,
          found,
          typedReads,
        )
      )
    }
    return
  }
  const named = getNamedType(type)
  if (!isObjectType(named) && !isInterfaceType(named) && !isUnionType(named)) {
    return
  }
  if (!record(value)) return
  if (Object.hasOwn(value, "$record")) {
    if (Object.keys(value).length !== 1 || typeof value.$record !== "string") {
      throw new SchemaError(`${label}: malformed composite $record reference`)
    }
    if (known != null && !known.has(value.$record)) {
      throw new SchemaError(`${label}: unknown composite $record reference`)
    }
    found?.add(value.$record)
    typedReads?.push({ key: value.$record, type })
    return
  }
  const concrete = (isUnionType(named) || isInterfaceType(named)) &&
      typeof value.__typename === "string"
    ? schema.getType(value.__typename)
    : named
  if (!isObjectType(concrete) && !isInterfaceType(concrete)) return
  for (const [key, child] of Object.entries(value)) {
    const field = concrete.getFields()[key]
    if (field != null) {
      checkResponseReferences(
        child,
        field.type,
        schema,
        known,
        `${label}.${key}`,
        found,
        typedReads,
      )
    }
  }
}

async function checkGraphQLFixture(
  spec: CaseSpec,
  file: string,
): Promise<void> {
  const fixture = spec.graphql
  if (fixture == null) return
  const root = join(import.meta.dirname ?? ".", "../../..")
  const sdl = await Deno.readTextFile(join(root, "graphql/schema.graphql"))
  const bytes = new TextEncoder().encode(sdl)
  const hash = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
  ).map((value) => value.toString(16).padStart(2, "0")).join("")
  const baseline = v.parse(
    v.object({ schemaSha256: v.string() }),
    JSON.parse(
      await Deno.readTextFile(join(root, "rust/parity/baseline.json")),
    ),
  )
  if (hash !== baseline.schemaSha256 || fixture.schemaSha256 !== hash) {
    throw new SchemaError(
      `${file}: GraphQL schema digest differs from pinned baseline`,
    )
  }
  const schema = buildPinnedSchema(sdl)
  checkLaneDeadline(spec, spec.timeoutMs)
  const recordValues = new Map<string, unknown[]>()
  for (const [key, value] of Object.entries(fixture.initialRecords)) {
    recordValues.set(key, [value])
  }
  for (const group of fixture.groups) {
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    for (const step of steps) {
      if (step.kind !== "graphql") continue
      for (const effect of step.effects) {
        if (effect.kind !== "put") continue
        const values = recordValues.get(effect.record) ?? []
        values.push(effect.after)
        recordValues.set(effect.record, values)
      }
    }
  }
  for (const group of fixture.groups) {
    if (group.mode === "ordered") {
      checkRedirects(group.steps, file)
      continue
    }
    for (const lane of group.lanes) checkRedirects(lane.steps, file)
    for (let left = 0; left < group.lanes.length; left++) {
      for (let right = left + 1; right < group.lanes.length; right++) {
        const a = group.lanes[left]
        const b = group.lanes[right]
        if (
          firstStepsOverlap(a.steps[0], b.steps[0], schema) &&
          !sameEffectFreeLane(a.steps, b.steps)
        ) {
          throw new SchemaError(
            `${file}: ambiguous non-identical lane first steps`,
          )
        }
        const aa = laneDependencies(a.steps, schema, recordValues)
        const bb = laneDependencies(b.steps, schema, recordValues)
        if (
          [...aa.writes].some((key) =>
            bb.writes.has(key) || bb.reads.has(key)
          ) ||
          [...bb.writes].some((key) => aa.reads.has(key))
        ) {
          throw new SchemaError(
            `${file}: cross-lane record read/write dependency is unsupported`,
          )
        }
      }
    }
  }
  const knownRecords = new Set(Object.keys(fixture.initialRecords))
  for (const group of fixture.groups) {
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    for (const step of steps) {
      if (step.kind !== "graphql") continue
      for (const effect of step.effects) {
        if (effect.kind === "put") knownRecords.add(effect.record)
      }
    }
  }
  const seen = new Set<string>()
  const assets = new Set<string>()
  for (const group of fixture.groups) {
    const lanes = group.mode === "ordered"
      ? [{ id: "ordered", steps: group.steps }]
      : group.lanes
    if (new Set(lanes.map((lane) => lane.id)).size !== lanes.length) {
      throw new SchemaError(`${file}: duplicate lane id`)
    }
    const concurrentRecords = new Map<string, string>()
    for (const lane of lanes) {
      for (const step of lane.steps) {
        if (seen.has(step.id)) {
          throw new SchemaError(`${file}: duplicate interaction id ${step.id}`)
        }
        seen.add(step.id)
        if (step.kind === "asset") {
          assets.add(step.path)
          if (
            step.method === "GET" && decodeByteValue(step.body).length !== 0
          ) throw new SchemaError(`${file}: GET asset body must be empty`)
          continue
        }
        if (step.response.kind !== "validationErrors") {
          try {
            const selfMatch = matchGraphQL(
              step.operation,
              step.operation,
              schema,
            )
            if (!selfMatch.matches) {
              throw new Error(
                selfMatch.reason ?? "fixture operation does not match itself",
              )
            }
          } catch (error) {
            throw new SchemaError(
              `${file}: ${step.id}: ${
                error instanceof Error ? error.message : String(error)
              }`,
            )
          }
        }
        if (
          step.response.kind === "data" ||
          step.response.kind === "graphqlErrors"
        ) {
          const operation = getOperationAST(
            parse(step.operation.document),
            step.operation.operationName,
          )
          const root = operation?.operation === "query"
            ? schema.getQueryType()
            : operation?.operation === "mutation"
            ? schema.getMutationType()
            : schema.getSubscriptionType()
          if (root == null) {
            throw new SchemaError(
              `${file}: ${step.id}: GraphQL operation has no root type`,
            )
          }
          checkResponseReferences(
            step.response.data,
            root,
            schema,
            knownRecords,
            `${file}: ${step.id}: response.data`,
          )
        }
        for (const effect of step.effects) {
          if (
            group.mode === "lanes" && concurrentRecords.has(effect.record) &&
            concurrentRecords.get(effect.record) !== lane.id
          ) {
            throw new SchemaError(
              `${file}: concurrent effects on ${effect.record} are unsupported`,
            )
          }
          concurrentRecords.set(effect.record, lane.id)
        }
      }
    }
  }
  for (const group of fixture.groups) {
    const steps = group.mode === "ordered"
      ? group.steps
      : group.lanes.flatMap((lane) => lane.steps)
    for (const step of steps) {
      if (
        step.kind === "asset" && step.response.location != null &&
        !assets.has(step.response.location)
      ) {
        throw new SchemaError(
          `${file}: asset redirect target ${step.response.location} is undeclared`,
        )
      }
    }
  }
}

export async function loadCases(
  dir: string,
  manifestRoutes: ReadonlySet<string>,
  filter?: string,
): Promise<LoadedCase[]> {
  const files: string[] = []
  for await (const entry of Deno.readDir(dir)) {
    if (entry.isFile && entry.name.endsWith(".json")) files.push(entry.name)
  }
  files.sort()
  const seen = new Set<string>()
  const loaded: LoadedCase[] = []
  for (const name of files) {
    const file = join(dir, name)
    let parsed: unknown
    try {
      parsed = JSON.parse(await Deno.readTextFile(file))
    } catch (error) {
      throw new SchemaError(
        `${file}: invalid JSON: ${
          error instanceof Error ? error.message : String(error)
        }`,
      )
    }
    const spec = parseCase(parsed, file)
    await checkGraphQLFixture(spec, file)
    if (`${spec.id}.json` !== name) {
      throw new SchemaError(
        `${file}: id ${spec.id} does not match the file name`,
      )
    }
    if (seen.has(spec.id)) {
      throw new SchemaError(`${file}: duplicate case id ${spec.id}`)
    }
    seen.add(spec.id)
    if (!manifestRoutes.has(spec.route)) {
      throw new SchemaError(
        `${file}: route "${spec.route}" is not in rust/parity/manifest.json`,
      )
    }
    let fixtureDir: string | null = null
    if (spec.cwdFixture !== "empty") {
      fixtureDir = join(dir, "fixtures", spec.cwdFixture)
      const info = await Deno.lstat(fixtureDir).catch(() => null)
      if (info == null || !info.isDirectory) {
        throw new SchemaError(
          `${file}: cwd fixture directory ${fixtureDir} is missing`,
        )
      }
    }
    let configFixtureDir: string | null = null
    if (spec.configFixture != null) {
      configFixtureDir = join(dir, "fixtures", spec.configFixture)
      const info = await Deno.lstat(configFixtureDir).catch(() => null)
      if (info == null || !info.isDirectory || info.isSymlink) {
        throw new SchemaError(
          `${file}: config fixture directory is missing or unsafe`,
        )
      }
      await inspectConfigFixture(configFixtureDir, file)
    }
    // Resolve with dummy values now so undeclared placeholders fail at load time.
    resolveCase(spec, {
      home: "h",
      configHome: "c",
      cwd: "w",
      bin: "b",
      denoDir: "d",
      fixturePort: "0",
    })
    if (filter == null || spec.id.includes(filter)) {
      loaded.push({ file, spec, fixtureDir, configFixtureDir })
    }
  }
  return loaded
}

function resolveBytes(
  value: CaseSpec["stdin"],
  declared: readonly SubstitutionName[],
  values: Readonly<Record<SubstitutionName, string>>,
  label: string,
): Uint8Array {
  if ("utf8" in value) {
    return decodeByteValue({
      utf8: substitute(value.utf8, declared, values, label),
    })
  }
  return decodeByteValue(value)
}

function substituteValues(
  value: unknown,
  declared: readonly SubstitutionName[],
  values: Readonly<Record<SubstitutionName, string>>,
  label: string,
): unknown {
  if (typeof value === "string") {
    return substitute(value, declared, values, label)
  }
  if (Array.isArray(value)) {
    return value.map((entry, index) =>
      substituteValues(entry, declared, values, `${label}[${index}]`)
    )
  }
  if (typeof value === "object" && value != null) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key,
        substituteValues(entry, declared, values, `${label}.${key}`),
      ]),
    )
  }
  return value
}

export function resolveCase(
  spec: CaseSpec,
  values: Readonly<Record<SubstitutionName, string>>,
): ResolvedCase {
  const declared = spec.substitutions
  const label = `case ${spec.id}`
  const env: Record<string, string> = {}
  for (const [key, value] of Object.entries(spec.env)) {
    env[key] = substitute(value, declared, values, `${label} env ${key}`)
  }
  return {
    argv: spec.argv.map((arg, index) =>
      substitute(arg, declared, values, `${label} argv[${index}]`)
    ),
    stdin: resolveBytes(spec.stdin, declared, values, `${label} stdin`),
    env,
    expected: {
      exit: spec.expected.exit,
      stdout: resolveBytes(
        spec.expected.stdout,
        declared,
        values,
        `${label} expected stdout`,
      ),
      stderr: resolveBytes(
        spec.expected.stderr,
        declared,
        values,
        `${label} expected stderr`,
      ),
      fileEffects: spec.expected.fileEffects,
    },
    fixtureServer: spec.fixtureServer == null ? null : {
      ...spec.fixtureServer,
      responses: spec.fixtureServer.responses.map((response, index) => ({
        ...response,
        body: "utf8" in response.body
          ? {
            utf8: substitute(
              response.body.utf8,
              declared,
              values,
              `${label} fixture response ${index}`,
            ),
          }
          : response.body,
      })),
    },
    graphql: spec.graphql == null ? spec.graphql : v.parse(
      GraphQLFixtureSchema,
      substituteValues(spec.graphql, declared, values, `${label} graphql`),
    ),
  }
}
