// Case corpus loading: schema validation, manifest route binding, fixture
// existence, and placeholder resolution into concrete bytes and paths.
import { join } from "@std/path"
import { decodeByteValue } from "./bytes.ts"
import { buildPinnedSchema, matchGraphQL } from "./graphql-match.ts"
import {
  type CaseSpec,
  GraphQLFixtureSchema,
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
      loaded.push({ file, spec, fixtureDir })
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
