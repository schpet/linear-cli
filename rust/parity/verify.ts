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
const GENERATED_CODEGEN_SHA256: Record<string, string> = {
  "gql.ts": "8dfea6a7d53a9bc2fab795de6e41872ad1ccbb70acd884d9ac35b97fb4701abc",
  "graphql.ts":
    "4ec1201b8b94287c6227bfc413becac85b155300527245bde904a3b6257b73a4",
  "index.ts":
    "3498fb63273000c22776eaeb5a66de3ec701a57ee448e36ce578a149fd9e1482",
}

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

/**
 * jj snapshots the working copy on every command and needs HOME (and the XDG
 * config dir) to read the user's git config and global excludes; with a bare
 * PATH it would snapshot globally ignored files into `@`.
 */
function jjEnv(): Record<string, string> {
  const env: Record<string, string> = { PATH: "/usr/local/bin:/usr/bin:/bin" }
  for (const key of ["HOME", "XDG_CONFIG_HOME"]) {
    const value = Deno.env.get(key)
    if (value != null && value !== "") env[key] = value
  }
  return env
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
    await Deno.realPath(reference) === await Deno.realPath(root),
    "interpreted reference must use the repository working copy",
  )
  await verifySourceBinding(baseline)
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
}

/**
 * Root deno.json additions reviewed for the parity harness. Everything else in
 * the root config, and all of src, deno.lock and graphql, must match the
 * frozen reference exactly.
 */
export const APPROVED_ROOT_TASKS: Record<string, string> = {
  parity:
    "deno run --frozen --allow-all --quiet --config rust/parity/deno.json rust/parity/runner/main.ts",
  "parity:test":
    "deno test --frozen --allow-all --quiet --config rust/parity/deno.json rust/parity/",
}
export const APPROVED_ROOT_TEST_EXCLUDE = ["rust/", "untracked/"]
export const APPROVED_ROOT_LINT_EXCLUDE = ["untracked/"]

// Exact appended paths protect hash-bound native distribution inputs from fmt.
export const APPROVED_ROOT_FMT_EXCLUDE = [
  "rust/licenses/",
  "docs/rust-port.md",
  "skills/linear-cli/SKILL.native.template.md",
  "skills/linear-cli/SKILL.md",
  "skills/linear-cli/references/api.md",
  "skills/linear-cli/references/auth.md",
  "skills/linear-cli/references/commands.md",
  "skills/linear-cli/references/config.md",
  "skills/linear-cli/references/cycle.md",
  "skills/linear-cli/references/document.md",
  "skills/linear-cli/references/initiative-update.md",
  "skills/linear-cli/references/initiative.md",
  "skills/linear-cli/references/issue.md",
  "skills/linear-cli/references/label.md",
  "skills/linear-cli/references/markdown.md",
  "skills/linear-cli/references/milestone.md",
  "skills/linear-cli/references/project-update.md",
  "skills/linear-cli/references/project.md",
  "skills/linear-cli/references/schema.md",
  "skills/linear-cli/references/team.md",
  "skills/linear-cli/references/template.md",
  "skills/linear-cli/references/user.md",
  "untracked/",
  "rust/parity/runner/cases/",
  "rust/parity/runner/*-frozen-cases/",
  "rust/crates/linear-cli/src/graphql/schema_builtin_types.json",
  "rust/crates/linear-cli/tests/commands/fixtures/api-schema/",
]

/** Structural comparison of the current root config with the frozen one. */
export function compareRootConfig(frozen: unknown, current: unknown): void {
  assert(
    isRecord(frozen) && isRecord(current),
    "root deno.json is not an object",
  )
  const pruned = structuredClone(current)
  if (isRecord(pruned.tasks)) {
    for (const [name, command] of Object.entries(APPROVED_ROOT_TASKS)) {
      if (pruned.tasks[name] === command) delete pruned.tasks[name]
    }
  }
  if (
    isRecord(pruned.test) &&
    JSON.stringify(pruned.test) ===
      JSON.stringify({ exclude: APPROVED_ROOT_TEST_EXCLUDE })
  ) {
    delete pruned.test
  }
  if (isRecord(pruned.fmt) && Array.isArray(pruned.fmt.exclude)) {
    const excludes = pruned.fmt.exclude
    const suffix = excludes.slice(-APPROVED_ROOT_FMT_EXCLUDE.length)
    if (JSON.stringify(suffix) === JSON.stringify(APPROVED_ROOT_FMT_EXCLUDE)) {
      pruned.fmt.exclude = excludes.slice(0, -APPROVED_ROOT_FMT_EXCLUDE.length)
    }
  }
  if (isRecord(pruned.lint) && Array.isArray(pruned.lint.exclude)) {
    const excludes = pruned.lint.exclude
    const suffix = excludes.slice(-APPROVED_ROOT_LINT_EXCLUDE.length)
    if (JSON.stringify(suffix) === JSON.stringify(APPROVED_ROOT_LINT_EXCLUDE)) {
      pruned.lint.exclude = excludes.slice(
        0,
        -APPROVED_ROOT_LINT_EXCLUDE.length,
      )
    }
  }
  assert(
    JSON.stringify(pruned) === JSON.stringify(frozen),
    "root deno.json differs from the frozen reference beyond the approved parity task, test.exclude, fmt.exclude and lint.exclude additions",
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
      "deno.lock",
      "graphql",
    ],
    root,
    jjEnv(),
  )
  assert(
    diff.code === 0 && diff.stdout.trim() === "",
    `exporter source differs from frozen reference: ${diff.stderr}${diff.stdout}`,
  )
  const codegen = join(root, "src/__codegen__")
  const actual: string[] = []
  for await (const entry of Deno.readDir(codegen)) {
    assert(entry.isFile, `unexpected generated-code entry: ${entry.name}`)
    actual.push(entry.name)
  }
  assert(
    JSON.stringify(actual.sort()) ===
      JSON.stringify(Object.keys(GENERATED_CODEGEN_SHA256).sort()),
    `generated GraphQL file set drift: ${actual.join(", ")}`,
  )
  for (const [name, expected] of Object.entries(GENERATED_CODEGEN_SHA256)) {
    assert(
      await sha256(join(codegen, name)) === expected,
      `generated GraphQL file drift: ${name}`,
    )
  }
  const frozen = await run(
    "jj",
    ["file", "show", "-R", root, "-r", baseline.referenceRevision, "deno.json"],
    root,
    jjEnv(),
  )
  assert(frozen.code === 0, `cannot read frozen deno.json: ${frozen.stderr}`)
  compareRootConfig(
    JSON.parse(frozen.stdout),
    JSON.parse(await Deno.readTextFile(join(root, "deno.json"))),
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
    "usage: deno run ... rust/parity/verify.ts <working-copy> <reference-binary>",
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
  compareManifest(manifest, await withSourceMap(await exportRuntime()))
  await verifyProbes(manifest.probes, binary)
  console.log(
    `verified ${manifest.routes.length} routes and ${manifest.probes.length} safe probes`,
  )
}
