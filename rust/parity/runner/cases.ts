// Case corpus loading: schema validation, manifest route binding, fixture
// existence, and placeholder resolution into concrete bytes and paths.
import { join } from "@std/path"
import { decodeByteValue } from "./bytes.ts"
import {
  type CaseSpec,
  parseCase,
  SchemaError,
  substitute,
  type SubstitutionName,
} from "./schema.ts"

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
      const info = await Deno.stat(fixtureDir).catch(() => null)
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
  }
}
