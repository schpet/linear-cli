// Strict case and candidate descriptor schemas for the P02 subprocess runner.
// Unknown keys are rejected so that later surfaces (GraphQL semantics, PTY,
// clock, keyring) cannot be smuggled in with weak defaults.
import * as v from "valibot"
import { decodeByteValue } from "./bytes.ts"

// Frozen reference d4fe6fa7 embeds version 2.6.0. Do not follow the moving
// checkout's deno.json: a later version bump must not change pinned fixtures.
export const FROZEN_USER_AGENT = "schpet-linear-cli/2.6.0"

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

const StdoutSchema = v.union([
  ByteValueSchema,
  v.strictObject({ mode: v.literal("closed-at-start") }),
  v.pipe(
    v.strictObject({
      mode: v.literal("close-after-bytes"),
      count: v.pipe(
        v.number(),
        v.integer(),
        v.minValue(1),
        v.maxValue(64 * 1024 * 1024),
      ),
      prefix: ByteValueSchema,
    }),
    v.check(
      ({ count, prefix }) =>
        decodeByteValue(prefix).length === count &&
        !/\{\{[^{}]*\}\}/.test(
          new TextDecoder().decode(decodeByteValue(prefix)),
        ),
      "after-N prefix must be literal and exactly count bytes",
    ),
  ),
], "stdout must be bytes or a supported pipe mode")

/**
 * Signal names a case may expect. Each entry was proved end to end through
 * the P04A1 status helper under Bubblewrap with a controlled native child;
 * SIGKILL stays out until a target-kill control (not the runner's own group
 * kill) is proved separately.
 */
export const EXPECTED_SIGNALS = ["SIGTERM", "SIGPIPE", "SIGINT"] as const
export type ExpectedSignal = (typeof EXPECTED_SIGNALS)[number]

/**
 * Exactly one of `{code}` or `{signal}`. `{"exit":{"code":143}}` and
 * `{"exit":{"signal":"SIGTERM"}}` are distinct expectations: the authenticated
 * target status separates them even though bwrap folds both to outer 143.
 */
const ExitSchema = v.union([
  v.strictObject({
    code: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(255)),
  }),
  v.strictObject({ signal: v.picklist(EXPECTED_SIGNALS) }),
], "exit must be exactly {code: 0..255} or {signal: SIGTERM|SIGPIPE|SIGINT}")

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

function isJson(value: unknown): boolean {
  if (
    value == null || typeof value === "string" || typeof value === "boolean"
  ) return true
  if (typeof value === "number") return Number.isFinite(value)
  if (Array.isArray(value)) return value.every(isJson)
  if (typeof value !== "object") return false
  if (
    Object.getPrototypeOf(value) !== Object.prototype &&
    Object.getPrototypeOf(value) !== null
  ) return false
  return Object.entries(value).every(([key, entry]) =>
    key !== "__proto__" && isJson(entry)
  )
}

const JsonSchema = v.pipe(
  v.unknown(),
  v.check(isJson, "expected finite JSON data"),
)
const JsonObjectSchema = v.pipe(
  v.record(v.string(), JsonSchema),
  v.check(
    (value) => Object.keys(value).every((key) => key !== "__proto__"),
    "unsafe object key",
  ),
)
const count = v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(10_000))
const boundedBytes = v.pipe(
  ByteValueSchema,
  v.check((value) => {
    if ("utf8" in value) {
      return new TextEncoder().encode(value.utf8).length <= 4 * 1024 * 1024
    }
    return value.base64.length <= 6 * 1024 * 1024
  }, "GraphQL fixture byte payload exceeds 4 MiB"),
)
const safePath = v.pipe(
  v.string(),
  v.regex(
    /^\/(?:[A-Za-z0-9._~!$&'()*+,;=:@%-]|\/)*(?:\?[A-Za-z0-9._~!$&'()*+,;=:@%/?-]*)?$/,
    "path must be a safe origin-relative URL",
  ),
  v.check(
    (path) =>
      !path.startsWith("//") &&
      !path.split("?")[0].split("/").some((part) =>
        part === "." || part === ".." || /%2e/i.test(part)
      ),
    "path must not traverse or name another host",
  ),
)
const headers = v.pipe(
  v.record(v.string(), v.string()),
  v.check(
    (value) => Object.keys(value).every((key) => /^[A-Za-z0-9-]+$/.test(key)),
    "invalid header name",
  ),
  v.check(
    (value) => Object.values(value).every((entry) => !/[\r\n]/.test(entry)),
    "header values must not contain newlines",
  ),
  v.check(
    (value) =>
      new Set(Object.keys(value).map((key) => key.toLowerCase())).size ===
        Object.keys(value).length,
    "header names must be unique case-insensitively",
  ),
)
const requestIdentity = v.pipe(
  v.strictObject({
    authorization: v.nullable(v.pipe(v.string(), v.startsWith("lin_api_fake"))),
    userAgent: v.literal(FROZEN_USER_AGENT),
    headers,
  }),
  v.check(
    (identity) =>
      !Object.keys(identity.headers).some((key) =>
        ["authorization", "user-agent"].includes(key.toLowerCase())
      ),
    "Authorization and User-Agent belong in identity fields",
  ),
)
const GraphQLOperationSchema = v.strictObject({
  document: nonEmpty,
  operationName: v.optional(v.string()),
  variables: v.optional(JsonObjectSchema),
  exactOrigins: v.optional(v.array(nonEmpty)),
  allowExtraTypename: v.optional(v.boolean()),
})
const GraphQLErrorSchema = v.strictObject({
  message: nonEmpty,
  locations: v.optional(
    v.array(v.strictObject({ line: count, column: count })),
  ),
  path: v.optional(v.array(v.union([v.string(), count]))),
  extensions: v.optional(JsonObjectSchema),
})
const GraphQLResponseSchema = v.variant("kind", [
  v.strictObject({ kind: v.literal("data"), data: JsonSchema }),
  v.strictObject({
    kind: v.literal("graphqlErrors"),
    status: v.pipe(v.number(), v.integer(), v.minValue(100), v.maxValue(599)),
    data: JsonSchema,
    errors: v.pipe(v.array(GraphQLErrorSchema), v.minLength(1)),
  }),
  v.strictObject({
    kind: v.literal("validationErrors"),
    status: v.pipe(v.number(), v.integer(), v.minValue(100), v.maxValue(599)),
    errors: v.pipe(v.array(GraphQLErrorSchema), v.minLength(1)),
  }),
  v.strictObject({
    kind: v.literal("transport"),
    status: v.pipe(v.number(), v.integer(), v.minValue(100), v.maxValue(599)),
    headers,
    body: boundedBytes,
  }),
])
const EffectSchema = v.variant("kind", [
  v.strictObject({
    kind: v.literal("put"),
    record: nonEmpty,
    before: v.union([
      v.strictObject({ absent: v.literal(true) }),
      v.strictObject({ value: JsonSchema }),
    ]),
    after: JsonSchema,
  }),
  v.strictObject({
    kind: v.literal("delete"),
    record: nonEmpty,
    before: v.strictObject({ value: JsonSchema }),
  }),
])
export type GraphQLEffectSpec = v.InferOutput<typeof EffectSchema>
const GraphQLStepSchema = v.pipe(
  v.strictObject({
    kind: v.literal("graphql"),
    id: nonEmpty,
    operation: GraphQLOperationSchema,
    identity: requestIdentity,
    response: GraphQLResponseSchema,
    effects: v.array(EffectSchema),
    partialEffects: v.optional(v.boolean()),
  }),
  v.check(
    (step) =>
      step.effects.length === 0 || step.response.kind === "data" ||
      step.response.kind === "graphqlErrors",
    "effects require a data or graphqlErrors response",
  ),
  v.check(
    (step) =>
      step.partialEffects !== true || step.response.kind === "graphqlErrors",
    "partialEffects:true requires a graphqlErrors response",
  ),
  v.check(
    (step) =>
      step.response.kind !== "graphqlErrors" || step.effects.length === 0 ||
      step.partialEffects === true,
    "graphqlErrors effects require partialEffects:true",
  ),
)
export type GraphQLStepSpec = v.InferOutput<typeof GraphQLStepSchema>
const AssetStepSchema = v.pipe(
  v.strictObject({
    kind: v.literal("asset"),
    id: nonEmpty,
    method: v.picklist(["GET", "PUT"]),
    fixedHost: v.optional(v.picklist([
      "uploads.linear.app",
      "public.linear.app",
    ])),
    path: safePath,
    requiredHeaders: headers,
    forbiddenHeaders: v.array(
      v.pipe(v.string(), v.regex(/^[A-Za-z0-9-]+$/, "invalid header name")),
    ),
    body: boundedBytes,
    response: v.strictObject({
      status: v.pipe(v.number(), v.integer(), v.minValue(100), v.maxValue(599)),
      headers,
      body: boundedBytes,
      location: v.optional(safePath),
    }),
  }),
  v.check(
    (step) => {
      const required = new Set(
        Object.keys(step.requiredHeaders).map((key) => key.toLowerCase()),
      )
      const forbidden = step.forbiddenHeaders.map((key) => key.toLowerCase())
      const authorization = Object.entries(step.requiredHeaders).find(([key]) =>
        key.toLowerCase() === "authorization"
      )?.[1]
      return new Set(forbidden).size === forbidden.length &&
        forbidden.every((key) => !required.has(key)) &&
        (authorization == null || authorization.startsWith("lin_api_fake"))
    },
    "asset headers must be unique, nonconflicting, and use only fake Authorization",
  ),
  v.check(
    (step) =>
      !Object.keys(step.response.headers).some((name) =>
        name.toLowerCase() === "location"
      ),
    "asset Location must use response.location only",
  ),
  v.check(
    (step) => {
      const redirect = step.response.status >= 300 && step.response.status < 400
      return redirect === (step.response.location != null) &&
        (!redirect || step.method === "GET")
    },
    "asset redirects require GET, 3xx and response.location together",
  ),
  v.check(
    (step) => step.fixedHost == null || step.method === "GET",
    "fixedHost assets are GET downloads only",
  ),
)
export type AssetStepSpec = v.InferOutput<typeof AssetStepSchema>
export type InteractionSpec = GraphQLStepSpec | AssetStepSpec
const InteractionSchema = v.variant("kind", [
  GraphQLStepSchema,
  AssetStepSchema,
])
const GroupSchema = v.variant("mode", [
  v.strictObject({
    mode: v.literal("ordered"),
    steps: v.pipe(v.array(InteractionSchema), v.minLength(1)),
  }),
  v.strictObject({
    mode: v.literal("lanes"),
    timeoutMs: v.pipe(
      v.number(),
      v.integer(),
      v.minValue(1),
      v.maxValue(600_000),
    ),
    lanes: v.pipe(
      v.array(
        v.strictObject({
          id: nonEmpty,
          steps: v.pipe(v.array(InteractionSchema), v.minLength(1)),
        }),
      ),
      v.minLength(2),
      v.maxLength(5),
    ),
  }),
])
export const GraphQLFixtureSchema = v.strictObject({
  path: safePath,
  schemaSha256: hex64,
  expectedRequests: count,
  initialRecords: JsonObjectSchema,
  expectedRecords: JsonObjectSchema,
  groups: v.pipe(v.array(GroupSchema), v.minLength(1)),
})
export type GraphQLFixtureSpec = v.InferOutput<typeof GraphQLFixtureSchema>

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
    configFixture: v.optional(v.pipe(
      v.string(),
      v.regex(
        /^[a-z0-9][a-z0-9-]*$/,
        "configFixture is a kebab-case fixture name",
      ),
    )),
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
    graphql: v.optional(v.nullable(GraphQLFixtureSchema)),
    expected: v.strictObject({
      exit: ExitSchema,
      stdout: StdoutSchema,
      stderr: ByteValueSchema,
      fileEffects: v.array(FileEffectSchema),
    }),
    deviation: v.null(
      "reviewed deviations are not accepted by the P02 runner; keep null",
    ),
  }),
  v.check(
    (spec) =>
      spec.fixtureServer != null || spec.graphql != null ||
      !spec.substitutions.includes("fixturePort"),
    "fixturePort substitution requires a fixtureServer or graphql fixture",
  ),
  v.check(
    (spec) =>
      !("mode" in spec.expected.stdout) ||
      spec.expected.stdout.mode !== "close-after-bytes" ||
      spec.expected.stdout.count <= spec.outputCapBytes,
    "close-after-bytes count must not exceed outputCapBytes",
  ),
  v.check(
    (spec) =>
      (spec.fixtureServer == null && spec.graphql == null) ||
      spec.substitutions.includes("fixturePort"),
    "a network fixture case must declare the fixturePort substitution so the endpoint is explicit",
  ),
  v.check(
    (spec) => spec.fixtureServer == null || spec.graphql == null,
    "fixtureServer and graphql are mutually exclusive",
  ),
  v.check(
    (spec) =>
      spec.fixtureServer == null ||
      spec.fixtureServer.responses.length ===
        spec.fixtureServer.expectedRequests,
    "fixture response count must equal expectedRequests",
  ),
  v.check(
    (spec) =>
      spec.graphql == null ||
      spec.graphql.groups.flatMap((group) =>
          group.mode === "ordered"
            ? group.steps
            : group.lanes.flatMap((lane) => lane.steps)
        ).length === spec.graphql.expectedRequests,
    "graphql expectedRequests must equal GraphQL plus asset interactions",
  ),
  v.check(
    (spec) => {
      const fixedHost =
        spec.graphql?.groups.some((group) =>
          (group.mode === "ordered"
            ? group.steps
            : group.lanes.flatMap((lane) => lane.steps)).some((step) =>
              step.kind === "asset" && step.fixedHost != null
            )
        ) ?? false
      return !fixedHost || [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "NO_PROXY",
        "DENO_CERT",
        "DENO_TLS_CA_STORE",
        "SSL_CERT_FILE",
      ].every((key) => !(key in spec.env))
    },
    "fixedHost transport environment is runner-owned",
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
  if (
    "graphql" in input && typeof input.graphql === "object" &&
    input.graphql != null && !Array.isArray(input.graphql) &&
    Object.keys(input.graphql).length === 0
  ) {
    throw new SchemaError(
      `${label}: P03 graphql fixture requires a complete schema, groups, and state`,
    )
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
