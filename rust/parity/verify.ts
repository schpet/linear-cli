import { fromFileUrl, join } from "@std/path"
import { withSourceMap } from "./source-map.ts"

export interface Manifest {
  baseline: Record<string, unknown>
  routes: Array<Record<string, unknown>>
  probes: Array<Record<string, unknown>>
  notes: Record<string, unknown>
}

const root = fromFileUrl(new URL("../../", import.meta.url))
const parity = join(root, "rust/parity")
const decoder = new TextDecoder()

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

function stringArray(value: unknown, label: string): string[] {
  assert(
    Array.isArray(value) && value.every((item) => typeof item === "string"),
    `${label} is not a string array`,
  )
  return value
}

function recordArray(
  value: unknown,
  label: string,
): Array<Record<string, unknown>> {
  assert(
    Array.isArray(value) && value.every(isRecord),
    `${label} is not a record array`,
  )
  return value
}

export function readBaseline(value: unknown): Record<string, string> {
  assert(isRecord(value), "baseline is not a record")
  const result: Record<string, string> = {}
  for (const [key, item] of Object.entries(value)) {
    assert(typeof item === "string", `baseline ${key} is not a string`)
    result[key] = item
  }
  return result
}

export function readManifest(value: unknown): Manifest {
  assert(isRecord(value), "manifest is not an object")
  assert(
    isRecord(value.baseline) && isRecord(value.notes),
    "manifest metadata is missing",
  )
  return {
    baseline: value.baseline,
    notes: value.notes,
    routes: recordArray(value.routes, "manifest routes"),
    probes: recordArray(value.probes, "manifest probes"),
  }
}

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

async function sha256(path: string): Promise<string> {
  const bytes = await Deno.readFile(path)
  const digest = await crypto.subtle.digest("SHA-256", bytes)
  return Array.from(
    new Uint8Array(digest),
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("")
}

async function run(
  executable: string,
  args: string[],
  cwd: string,
  env: Record<string, string>,
) {
  const result = await new Deno.Command(executable, {
    args,
    cwd,
    env,
    clearEnv: true,
    stdout: "piped",
    stderr: "piped",
  }).output()
  return {
    code: result.code,
    stdout: decoder.decode(result.stdout),
    stderr: decoder.decode(result.stderr),
  }
}

async function sandbox<T>(
  fn: (cwd: string, env: Record<string, string>) => Promise<T>,
): Promise<T> {
  const cwd = await Deno.makeTempDir({ prefix: "linear-parity-" })
  const home = join(cwd, "home")
  const config = join(cwd, "config")
  await Deno.mkdir(home)
  await Deno.mkdir(config)
  const env = {
    HOME: home,
    XDG_CONFIG_HOME: config,
    APPDATA: config,
    LINEAR_IGNORE_ENV_FILE: "1",
    LINEAR_GRAPHQL_ENDPOINT: "http://127.0.0.1:1/graphql",
    DENO_DIR: Deno.env.get("DENO_DIR") ??
      join(Deno.env.get("HOME") ?? "", ".cache/deno"),
    PATH: "/usr/local/bin:/usr/bin:/bin",
    NO_COLOR: "1",
  }
  try {
    return await fn(cwd, env)
  } finally {
    await Deno.remove(cwd, { recursive: true })
  }
}

function denoArgs(entrypoint: string, args: string[], cwd: string) {
  return [
    "run",
    "--cached-only",
    `--config=${join(root, "deno.json")}`,
    `--allow-read=${root},${cwd}`,
    "--allow-env",
    "--deny-net",
    "--deny-run",
    "--deny-ffi",
    entrypoint,
    ...args,
  ]
}

export async function exportRuntime(): Promise<Array<Record<string, unknown>>> {
  return await sandbox(async (cwd, env) => {
    const result = await run(
      Deno.execPath(),
      denoArgs(join(parity, "export.ts"), [], cwd),
      cwd,
      env,
    )
    assert(result.code === 0, `offline exporter failed: ${result.stderr}`)
    const parsed: unknown = JSON.parse(result.stdout)
    assert(isRecord(parsed), "exporter returned no object")
    return recordArray(parsed.routes, "exported routes")
  })
}

export function compareManifest(
  expected: Manifest,
  actualRoutes: Array<Record<string, unknown>>,
): void {
  const expectedRoutes = JSON.stringify(expected.routes.map((route) => {
    const { fixtureStatus, qaStatus, liveStatus, ...contract } = route
    assert(
      typeof fixtureStatus === "string" && typeof qaStatus === "string" &&
        typeof liveStatus === "string",
      `missing coverage status on ${route.path}`,
    )
    return contract
  }))
  const actual = JSON.stringify(actualRoutes)
  assert(
    expectedRoutes === actual,
    `runtime route/alias/option drift: manifest and exporter differ`,
  )
  const routes = actualRoutes
  assert(
    routes.filter((route) => route.kind === "source_leaf").length === 86,
    "source leaf count changed",
  )
  assert(
    routes.filter((route) => route.kind === "parent_route").length === 20,
    "parent route count changed",
  )
  assert(
    routes.filter((route) => route.kind === "generated_completion_child")
      .length === 4,
    "completion child count changed",
  )
  assert(
    routes.reduce(
      (count, route) =>
        count + stringArray(route.aliases, `${route.path} aliases`).length,
      0,
    ) ===
      36,
    "alias count changed",
  )
  for (const route of routes) {
    for (
      const resolution of recordArray(
        route.aliasResolution,
        `${route.path} alias resolution`,
      )
    ) {
      assert(
        typeof resolution.alias === "string" &&
          typeof resolution.resolves === "boolean",
        "invalid alias resolution",
      )
      assert(
        resolution.resolves,
        `alias failed to resolve: ${route.path} ${resolution.alias}`,
      )
    }
  }
}

export async function verifyBaseline(
  baseline: Record<string, string>,
  reference: string,
  binary: string,
): Promise<void> {
  assert(
    await sha256(join(reference, "deno.lock")) === baseline.lockSha256,
    "reference lockfile hash drift",
  )
  assert(
    await sha256(join(reference, "graphql/schema.graphql")) ===
      baseline.schemaSha256,
    "reference schema hash drift",
  )
  assert(
    await sha256(binary) === baseline.binarySha256,
    "reference binary hash drift",
  )
  assert(
    baseline.localRepeatBuildSha256 === baseline.binarySha256,
    "repeat build hash mismatch",
  )
  const version = await run(Deno.execPath(), ["--version"], reference, {})
  assert(
    version.code === 0 &&
      version.stdout.startsWith(`deno ${baseline.denoVersion} `) &&
      version.stdout.includes(baseline.target),
    "Deno version/target drift",
  )
  const revision = await run(
    "jj",
    ["log", "-R", reference, "-r", "@-", "--no-graph", "-T", "commit_id"],
    reference,
    { PATH: "/usr/local/bin:/usr/bin:/bin" },
  )
  assert(
    revision.code === 0 &&
      revision.stdout.trim() === baseline.referenceRevision,
    "reference revision drift",
  )
  const empty = await run(
    "jj",
    [
      "log",
      "-R",
      reference,
      "-r",
      "@",
      "--no-graph",
      "-T",
      'if(empty, "empty", "nonempty")',
    ],
    reference,
    { PATH: "/usr/local/bin:/usr/bin:/bin" },
  )
  assert(
    empty.code === 0 && empty.stdout.trim() === "empty",
    "reference workspace @ is not empty",
  )
}

export async function verifySourceBinding(
  baseline: Record<string, string>,
): Promise<void> {
  const diff = await run(
    "jj",
    [
      "diff",
      "-R",
      root,
      "--from",
      baseline.referenceRevision,
      "--to",
      "@",
      "--summary",
      "--",
      "src",
      "deno.json",
      "deno.lock",
      "graphql",
    ],
    root,
    { PATH: "/usr/local/bin:/usr/bin:/bin" },
  )
  assert(
    diff.code === 0 && diff.stdout.trim() === "",
    `exporter source differs from frozen reference: ${diff.stderr}${diff.stdout}`,
  )
}

export async function verifyProbes(
  probes: Array<Record<string, unknown>>,
  binary: string,
): Promise<void> {
  await sandbox(async (cwd, env) => {
    for (const probe of probes) {
      const args = stringArray(probe.args, "probe args")
      const result = await run(binary, args, cwd, env)
      assert(
        result.code === probe.exitCode,
        `probe ${args.join(" ")} exit ${result.code}: ${result.stderr}`,
      )
      for (
        const fragment of stringArray(
          probe.stdoutIncludes,
          "probe stdout fragments",
        )
      ) {
        assert(
          result.stdout.includes(fragment),
          `probe ${args.join(" ")} missing stdout ${fragment}`,
        )
      }
    }
  })
}

if (import.meta.main) {
  const [reference, binary] = Deno.args
  assert(
    reference != null && binary != null && Deno.args.length === 2,
    "usage: deno run ... rust/parity/verify.ts <reference-workspace> <reference-binary>",
  )
  const baseline = readBaseline(
    JSON.parse(await Deno.readTextFile(join(parity, "baseline.json"))),
  )
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(parity, "manifest.json"))),
  )
  assert(
    JSON.stringify(manifest.baseline) === JSON.stringify(baseline),
    "manifest baseline identity drift",
  )
  await verifyBaseline(baseline, reference, binary)
  await verifySourceBinding(baseline)
  compareManifest(manifest, await withSourceMap(await exportRuntime()))
  await verifyProbes(manifest.probes, binary)
  console.log(
    `verified ${manifest.routes.length} routes and ${manifest.probes.length} safe probes`,
  )
}
