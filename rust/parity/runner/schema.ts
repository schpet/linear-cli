// Strict case and candidate descriptor schemas for the P02 subprocess runner.
// Unknown keys are rejected so that later surfaces (GraphQL semantics, PTY,
// clock, keyring) cannot be smuggled in with weak defaults.
import * as v from "valibot"

export const SUBSTITUTION_NAMES = [
  "home",
  "configHome",
  "cwd",
  "bin",
  "denoDir",
  "fixturePort",
]
export type SubstitutionName =
  | "home"
  | "configHome"
  | "cwd"
  | "bin"
  | "denoDir"
  | "fixturePort"

const UNSUPPORTED_FIELDS: Record<string, string> = {
  graphql:
    "GraphQL operation expectations (documents, variables, pagination, effects, concurrency groups) are P03 work; the P02 runner only supports scripted loopback responses under fixtureServer",
  pty: "PTY keystrokes and terminal traces are P04 work",
  terminal: "terminal size and TTY modes are P04 work",
  clock: "virtual clock bootstrap is P04 work",
  keyring: "keyring backends are P04 work",
  processes: "fake executables and process traces are P04 work",
}

export const REQUIRED_ENV_KEYS = [
  "HOME",
  "XDG_CONFIG_HOME",
  "APPDATA",
  "PATH",
  "DENO_DIR",
]

// The compiled Deno reference resolves its embedded module root from the
// temp directory. A TMPDIR other than the compile-time one makes it fail with
// "Module not found" and write node_modules under that directory, so cases
// must not set it. Sandbox temp paths remain the runner's responsibility.
const FORBIDDEN_ENV_KEYS = ["TMPDIR", "TMP", "TEMP"]

const hex64 = v.pipe(
  v.string(),
  v.regex(/^[0-9a-f]{64}$/, "expected sha256 hex"),
)
const nonEmpty = v.pipe(v.string(), v.minLength(1, "must not be empty"))

export const ByteValueSchema = v.union([
  v.strictObject({ utf8: v.string() }),
  v.strictObject({
    base64: v.pipe(
      v.string(),
      v.regex(
        /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/,
        "invalid base64",
      ),
    ),
  }),
], "byte fields must be {utf8} or {base64} objects")

const ExitSchema = v.strictObject({
  code: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(255)),
})

const relativePath = v.pipe(
  v.string(),
  v.minLength(1),
  v.check(
    (path) =>
      !path.startsWith("/") &&
      path.split("/").every((part) =>
        part !== "" && part !== "." && part !== ".."
      ),
    "file effect paths are sandbox-relative without . or .. segments",
  ),
)

export const FileEffectSchema = v.union([
  v.strictObject({
    path: relativePath,
    change: v.picklist(["created", "modified"]),
    kind: v.literal("file"),
    sha256: hex64,
  }),
  v.strictObject({
    path: relativePath,
    change: v.literal("removed"),
    kind: v.literal("file"),
  }),
  v.strictObject({
    path: relativePath,
    change: v.picklist(["created", "modified", "removed"]),
    kind: v.literal("directory"),
  }),
  v.strictObject({
    path: relativePath,
    change: v.picklist(["created", "modified"]),
    kind: v.literal("symlink"),
    target: v.string(),
  }),
  v.strictObject({
    path: relativePath,
    change: v.literal("removed"),
    kind: v.literal("symlink"),
  }),
])

const FixtureResponseSchema = v.strictObject({
  status: v.pipe(v.number(), v.integer(), v.minValue(100), v.maxValue(599)),
  headers: v.record(v.string(), v.string()),
  body: ByteValueSchema,
})

export const FixtureServerSchema = v.strictObject({
  path: v.pipe(v.string(), v.startsWith("/")),
  responses: v.pipe(v.array(FixtureResponseSchema), v.minLength(1)),
  expectedRequests: v.pipe(v.number(), v.integer(), v.minValue(0)),
  expectedAuthorization: nonEmpty,
})

const EnvSchema = v.pipe(
  v.record(
    v.pipe(
      v.string(),
      v.regex(/^[A-Z_][A-Z0-9_]*$/, "env keys are upper-case"),
    ),
    v.string(),
  ),
  v.check(
    (env) => REQUIRED_ENV_KEYS.every((key) => key in env),
    `env must declare ${
      REQUIRED_ENV_KEYS.join(", ")
    } explicitly; nothing is inherited`,
  ),
  v.check(
    (env) => FORBIDDEN_ENV_KEYS.every((key) => !(key in env)),
    "env must not set TMPDIR/TMP/TEMP: the compiled Deno reference breaks under a foreign temp directory",
  ),
  v.check(
    (env) => env.HOME === "{{home}}",
    "HOME must be the sandbox placeholder {{home}}",
  ),
  v.check(
    (env) =>
      env.XDG_CONFIG_HOME === "{{configHome}}" &&
      env.APPDATA === "{{configHome}}",
    "XDG_CONFIG_HOME and APPDATA must be the sandbox placeholder {{configHome}}",
  ),
  v.check(
    (env) => env.DENO_DIR === "{{denoDir}}",
    "DENO_DIR must be the staged placeholder {{denoDir}}",
  ),
  v.check(
    (env) =>
      env.LINEAR_API_KEY == null ||
      env.LINEAR_API_KEY.startsWith("lin_api_fake"),
    "LINEAR_API_KEY must be a fake key starting with lin_api_fake",
  ),
  v.check(
    (env) =>
      Object.values(env).every((value) =>
        !/lin_(api|oauth)_(?!fake)/.test(value)
      ),
    "env values must not resemble real Linear credentials",
  ),
)

export const CaseSchema = v.pipe(
  v.strictObject({
    id: v.pipe(v.string(), v.regex(/^[a-z0-9][a-z0-9-]*$/, "id is kebab-case")),
    route: nonEmpty,
    reason: nonEmpty,
    argv: v.array(v.string()),
    stdin: ByteValueSchema,
    cwdFixture: v.pipe(
      v.string(),
      v.regex(
        /^[a-z0-9][a-z0-9-]*$/,
        "cwdFixture is a kebab-case fixture name or empty",
      ),
    ),
    env: EnvSchema,
    substitutions: v.pipe(
      v.array(v.picklist([
        "home",
        "configHome",
        "cwd",
        "bin",
        "denoDir",
        "fixturePort",
      ])),
      v.check(
        (names) => new Set(names).size === names.length,
        "substitutions must be unique",
      ),
    ),
    timeoutMs: v.pipe(
      v.number(),
      v.integer(),
      v.minValue(1),
      v.maxValue(600_000),
    ),
    outputCapBytes: v.pipe(
      v.number(),
      v.integer(),
      v.minValue(1),
      v.maxValue(64 * 1024 * 1024),
    ),
    fixtureServer: v.nullable(FixtureServerSchema),
    expected: v.strictObject({
      exit: ExitSchema,
      stdout: ByteValueSchema,
      stderr: ByteValueSchema,
      fileEffects: v.array(FileEffectSchema),
    }),
    deviation: v.null(
      "reviewed deviations are not accepted by the P02 runner; keep null",
    ),
  }),
  v.check(
    (spec) =>
      spec.fixtureServer != null || !spec.substitutions.includes("fixturePort"),
    "fixturePort substitution requires a fixtureServer",
  ),
  v.check(
    (spec) =>
      spec.fixtureServer == null || spec.substitutions.includes("fixturePort"),
    "a fixtureServer case must declare the fixturePort substitution so the endpoint is explicit",
  ),
  v.check(
    (spec) =>
      spec.fixtureServer == null ||
      spec.fixtureServer.responses.length ===
        spec.fixtureServer.expectedRequests,
    "fixture response count must equal expectedRequests",
  ),
)

export type CaseSpec = v.InferOutput<typeof CaseSchema>
export type FileEffect = v.InferOutput<typeof FileEffectSchema>
export type FixtureServerSpec = v.InferOutput<typeof FixtureServerSchema>
export type ExitExpectation = v.InferOutput<typeof ExitSchema>

export const CandidateDescriptorSchema = v.strictObject({
  name: nonEmpty,
  program: v.variant("kind", [
    v.strictObject({
      kind: v.literal("executable"),
      path: v.pipe(
        v.string(),
        v.startsWith("/", "program path must be absolute"),
      ),
    }),
    v.strictObject({
      kind: v.literal("interpreted-reference"),
      workspace: v.pipe(
        v.string(),
        v.startsWith("/", "workspace path must be absolute"),
      ),
    }),
  ]),
  implementedRoutes: v.union(
    [
      v.literal("every-manifest-route"),
      v.pipe(
        v.array(nonEmpty),
        v.check(
          (routes) => new Set(routes).size === routes.length,
          "implementedRoutes must be unique",
        ),
      ),
    ],
    "implementedRoutes must be an explicit route list or the literal every-manifest-route",
  ),
})
export type CandidateDescriptor = v.InferOutput<
  typeof CandidateDescriptorSchema
>

export class SchemaError extends Error {}

function formatIssues(issues: readonly v.BaseIssue<unknown>[]): string {
  return issues.map((issue) => {
    const path = issue.path?.map((segment) => String(segment.key)).join(".") ??
      ""
    return `${path === "" ? "<root>" : path}: ${issue.message}`
  }).join("; ")
}

function rejectUnsupported(input: unknown, label: string): void {
  if (typeof input !== "object" || input == null || Array.isArray(input)) {
    throw new SchemaError(`${label}: expected an object`)
  }
  for (const [key, guidance] of Object.entries(UNSUPPORTED_FIELDS)) {
    if (key in input) {
      throw new SchemaError(
        `${label}: field "${key}" is not supported: ${guidance}`,
      )
    }
  }
}

export function parseCase(input: unknown, label = "case"): CaseSpec {
  rejectUnsupported(input, label)
  if (
    typeof input === "object" && input != null && "expected" in input &&
    typeof input.expected === "object" && input.expected != null &&
    "exit" in input.expected && typeof input.expected.exit === "object" &&
    input.expected.exit != null && "signal" in input.expected.exit
  ) {
    throw new SchemaError(
      `${label}: expected.exit.signal is unavailable through the Bubblewrap reaper; P04 must add out-of-band signal status capture`,
    )
  }
  const result = v.safeParse(CaseSchema, input)
  if (!result.success) {
    throw new SchemaError(`${label}: ${formatIssues(result.issues)}`)
  }
  return result.output
}

export function parseCandidateDescriptor(
  input: unknown,
  label = "candidate descriptor",
): CandidateDescriptor {
  const result = v.safeParse(CandidateDescriptorSchema, input)
  if (!result.success) {
    throw new SchemaError(`${label}: ${formatIssues(result.issues)}`)
  }
  return result.output
}

const placeholder = /\{\{([^{}]*)\}\}/g

function isSubstitutionName(name: string): name is SubstitutionName {
  return SUBSTITUTION_NAMES.includes(name)
}

/** Replace declared placeholders; any undeclared placeholder is an error. */
export function substitute(
  text: string,
  declared: readonly SubstitutionName[],
  values: Readonly<Record<SubstitutionName, string>>,
  label: string,
): string {
  for (const opening of text.matchAll(/\{\{/g)) {
    if (!/^\{\{[^{}]*\}\}/.test(text.slice(opening.index))) {
      throw new SchemaError(`${label}: malformed placeholder syntax`)
    }
  }
  const resolved = text.replace(placeholder, (_match, name: string) => {
    if (!isSubstitutionName(name)) {
      throw new SchemaError(`${label}: unknown placeholder {{${name}}}`)
    }
    if (!declared.includes(name)) {
      throw new SchemaError(
        `${label}: placeholder {{${name}}} is not declared in substitutions`,
      )
    }
    return values[name]
  })
  return resolved
}
