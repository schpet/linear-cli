import { assertEquals, assertThrows } from "@std/assert"
import {
  parseCandidateDescriptor,
  parseCase,
  SchemaError,
  substitute,
} from "./schema.ts"
import { validCase } from "./test-fixtures.ts"
import { candidateCaseView, type LoadedCase } from "./cases.ts"

function rejects(
  mutate: (spec: Record<string, unknown>) => void,
  fragment: string,
) {
  const spec = validCase()
  mutate(spec)
  assertThrows(() => parseCase(spec), SchemaError, fragment)
}

function objectField(
  spec: Record<string, unknown>,
  key: string,
): Record<string, unknown> {
  const value = spec[key]
  if (!isRecord(value)) {
    throw new Error(`${key} is not an object`)
  }
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

function listField(spec: Record<string, unknown>, key: string): unknown[] {
  const value = spec[key]
  if (!Array.isArray(value)) throw new Error(`${key} is not an array`)
  return value
}

Deno.test("a complete case parses and keeps byte fields as utf8/base64 objects", () => {
  const spec = parseCase(validCase())
  assertEquals(spec.expected.stdout, { utf8: "x" })
  assertEquals(spec.expected.stderr, { base64: "" })
})

Deno.test("Git probe is a paired, closed candidate-preserved case choice", () => {
  rejects((spec) => (spec.gitProbe = "parent-root"), "gitProbe")
  rejects((spec) => (spec.cwdSubdir = "subdir"), "gitProbe")
  rejects((spec) => {
    spec.gitProbe = "arbitrary-command"
    spec.cwdSubdir = "subdir"
  }, "gitProbe")
  rejects((spec) => {
    spec.gitProbe = "outside-repo"
    spec.cwdSubdir = "../host"
  }, "cwdSubdir")
  const raw = validCase()
  raw.gitProbe = "parent-root"
  raw.cwdSubdir = "subdir"
  const spec = parseCase(raw)
  const loaded: LoadedCase = {
    file: "sample.json",
    spec,
    fixtureDir: null,
    configFixtureDir: null,
  }
  const candidate = candidateCaseView(loaded)
  assertEquals(candidate.spec.gitProbe, "parent-root")
  assertEquals(candidate.spec.cwdSubdir, "subdir")
  assertEquals(candidate.spec.argv, ["--version"])
})

Deno.test("cwdRoot is confined to nested-cwd expected output", () => {
  const nested = validCase()
  nested.gitProbe = "parent-root"
  nested.cwdSubdir = "subdir"
  listField(nested, "substitutions").push("cwd")
  listField(nested, "substitutions").push("cwdRoot")
  objectField(nested, "expected").stderr = {
    utf8: "root={{cwdRoot}} invoked={{cwd}}",
  }
  const parsed = parseCase(nested)
  assertEquals(parsed.substitutions.includes("cwdRoot"), true)
  assertEquals(
    substitute(
      "root={{cwdRoot}} invoked={{cwd}}",
      parsed.substitutions,
      {
        home: "h",
        configHome: "c",
        cwd: "/case/cwd/subdir",
        cwdRoot: "/case/cwd",
        bin: "b",
        denoDir: "d",
        fixturePort: "0",
        referenceModuleUrl: "file:///reference",
      },
      "test",
    ),
    "root=/case/cwd invoked=/case/cwd/subdir",
  )
  rejects((spec) => {
    listField(spec, "substitutions").push("cwdRoot")
  }, "cwdRoot substitution requires cwdSubdir")
  for (const field of ["argv", "stdin", "env"]) {
    const forbidden = structuredClone(nested)
    if (field === "argv") forbidden.argv = ["{{cwdRoot}}"]
    if (field === "stdin") forbidden.stdin = { utf8: "{{cwdRoot}}" }
    if (field === "env") objectField(forbidden, "env").EXTRA = "{{cwdRoot}}"
    assertThrows(
      () => parseCase(forbidden),
      SchemaError,
      "cwdRoot is restricted to expected output",
    )
  }
  const fileEffect = structuredClone(nested)
  objectField(fileEffect, "expected").fileEffects = [{
    path: "link",
    change: "created",
    kind: "symlink",
    target: "{{cwdRoot}}",
  }]
  assertThrows(
    () => parseCase(fileEffect),
    SchemaError,
    "cwdRoot is restricted to expected output",
  )
  assertThrows(
    () =>
      substitute("{{cwdRoot}}", [], {
        home: "h",
        configHome: "c",
        cwd: "w",
        cwdRoot: "r",
        bin: "b",
        denoDir: "d",
        fixturePort: "0",
        referenceModuleUrl: "file:///reference",
      }, "test"),
    SchemaError,
    "not declared",
  )
})

Deno.test("cwdRoot cannot enter fixture or GraphQL traffic", async () => {
  const fixture = JSON.parse(
    await Deno.readTextFile(
      new URL("./cases/api-loopback-viewer-200.json", import.meta.url),
    ),
  )
  if (!isRecord(fixture)) throw new Error("fixture case is not an object")
  fixture.gitProbe = "parent-root"
  fixture.cwdSubdir = "subdir"
  listField(fixture, "substitutions").push("cwdRoot")
  const response =
    listField(objectField(fixture, "fixtureServer"), "responses")[0]
  if (!isRecord(response)) throw new Error("fixture response is not an object")
  objectField(response, "body").utf8 = "{{cwdRoot}}"
  assertThrows(
    () => parseCase(fixture),
    SchemaError,
    "cwdRoot is restricted to expected output",
  )

  const graphql = JSON.parse(
    await Deno.readTextFile(
      new URL("./cases/api-graphql-variable.json", import.meta.url),
    ),
  )
  if (!isRecord(graphql)) throw new Error("GraphQL case is not an object")
  graphql.gitProbe = "parent-root"
  graphql.cwdSubdir = "subdir"
  listField(graphql, "substitutions").push("cwdRoot")
  const group = listField(objectField(graphql, "graphql"), "groups")[0]
  if (!isRecord(group)) throw new Error("GraphQL group is not an object")
  const step = listField(group, "steps")[0]
  if (!isRecord(step)) throw new Error("GraphQL step is not an object")
  objectField(objectField(objectField(step, "response"), "data"), "issue")
    .identifier = "{{cwdRoot}}"
  assertThrows(
    () => parseCase(graphql),
    SchemaError,
    "cwdRoot is restricted to expected output",
  )
})

Deno.test("pipe stdout variants require literal byte-exact prefixes within the declared cap", () => {
  const withStdout = (stdout: unknown, cap = 1024) => {
    const spec = validCase()
    spec.outputCapBytes = cap
    objectField(spec, "expected").stdout = stdout
    return parseCase(spec).expected.stdout
  }
  assertEquals(withStdout({ mode: "closed-at-start" }), {
    mode: "closed-at-start",
  })
  assertEquals(
    withStdout({ mode: "close-after-bytes", count: 2, prefix: { utf8: "é" } }),
    { mode: "close-after-bytes", count: 2, prefix: { utf8: "é" } },
  )
  assertEquals(
    withStdout({
      mode: "close-after-bytes",
      count: 1,
      prefix: { base64: "ww==" },
    }),
    { mode: "close-after-bytes", count: 1, prefix: { base64: "ww==" } },
  )
  for (
    const stdout of [
      { mode: "closed-at-start", count: 0 },
      { mode: "close-after-bytes", count: 0, prefix: { utf8: "" } },
      { mode: "close-after-bytes", count: 1, prefix: { utf8: "é" } },
      { mode: "close-after-bytes", count: 4, prefix: { utf8: "abc" } },
      { mode: "close-after-bytes", count: 4, prefix: { utf8: "abcde" } },
      { mode: "close-after-bytes", count: 8, prefix: { utf8: "{{home}}" } },
      {
        mode: "close-after-bytes",
        count: 8,
        prefix: { base64: "e3tob21lfX0=" },
      },
      { mode: "close-after-bytes", count: 1, prefix: { utf8: "x" }, utf8: "x" },
      { mode: "close-after-bytes", count: 1025, prefix: { utf8: "x" } },
      { mode: "bad", count: 1, prefix: { utf8: "x" } },
    ]
  ) {
    assertThrows(() => withStdout(stdout), SchemaError)
  }
  assertThrows(
    () =>
      withStdout({
        mode: "close-after-bytes",
        count: 2,
        prefix: { utf8: "ab" },
      }, 1),
    SchemaError,
    "outputCapBytes",
  )
})

Deno.test("exit is exactly one of a code or a proved signal name; {code:143} and {signal:SIGTERM} are distinct expectations", () => {
  const withExit = (exit: unknown) => {
    const spec = validCase()
    objectField(spec, "expected").exit = exit
    return parseCase(spec).expected.exit
  }
  assertEquals(withExit({ code: 143 }), { code: 143 })
  assertEquals(withExit({ signal: "SIGTERM" }), { signal: "SIGTERM" })
  assertEquals(withExit({ signal: "SIGPIPE" }), { signal: "SIGPIPE" })
  assertEquals(withExit({ signal: "SIGINT" }), { signal: "SIGINT" })
  for (
    const exit of [
      { signal: "SIGKILL" },
      { signal: "SIGHUP" },
      { signal: "sigterm" },
      { signal: "TERM" },
      { signal: 15 },
      { signal: "SIGTERM", code: 143 },
      { code: 143, signal: "SIGTERM" },
      { code: 143, extra: 1 },
      { signal: "SIGTERM", extra: 1 },
      {},
      { code: -1 },
      { code: 1.5 },
      { code: "0" },
      null,
      "SIGTERM",
      143,
    ]
  ) {
    rejects((spec) => (objectField(spec, "expected").exit = exit), "exit")
  }
})

Deno.test("unknown keys, unsupported surfaces and weak fields are rejected", () => {
  rejects((spec) => (spec.extra = 1), "extra")
  rejects((spec) => (spec.graphql = {}), "P03")
  rejects((spec) => (spec.pty = {}), "P04")
  rejects((spec) => (spec.clock = {}), "P04")
  rejects((spec) => (spec.keyring = {}), "P04")
  rejects(
    (spec) => (objectField(spec, "expected").stdout = "plain"),
    "stdout",
  )
  rejects(
    (
      spec,
    ) => (objectField(spec, "expected").stdout = {
      base64: "not base64!",
    }),
    "base64",
  )
  rejects(
    (spec) => (objectField(spec, "expected").exit = { code: 256 }),
    "exit",
  )
  rejects((spec) => (spec.deviation = { id: "D1" }), "deviation")
  rejects((spec) => (spec.timeoutMs = 0), "timeoutMs")
  rejects((spec) => (spec.id = "Bad Id"), "id")
  rejects(
    (
      spec,
    ) => (objectField(spec, "expected").fileEffects = [{
      path: "../x",
      change: "created",
      kind: "file",
      sha256: "0".repeat(64),
    }]),
    "sandbox-relative",
  )
  rejects(
    (
      spec,
    ) => (objectField(spec, "expected").fileEffects = [{
      path: "x",
      change: "created",
      kind: "file",
    }]),
    "fileEffects",
  )
})

Deno.test("the environment must be complete, synthetic and credential-free", () => {
  rejects((spec) => delete objectField(spec, "env").PATH, "PATH")
  rejects(
    (spec) => delete objectField(spec, "env").DENO_DIR,
    "DENO_DIR",
  )
  rejects(
    (
      spec,
    ) => (objectField(spec, "env").DENO_DIR = "/home/someone/.cache/deno"),
    "{{denoDir}}",
  )
  rejects(
    (spec) => (objectField(spec, "env").HOME = "/home/someone"),
    "{{home}}",
  )
  rejects(
    (spec) => (objectField(spec, "env").TMPDIR = "/tmp"),
    "TMPDIR",
  )
  rejects(
    (
      spec,
    ) => (objectField(spec, "env").LINEAR_API_KEY = "lin_api_0123456789abcdef"),
    "lin_api_fake",
  )
  rejects(
    (
      spec,
    ) => (objectField(spec, "env").OTHER = "token lin_oauth_abc"),
    "credentials",
  )
  const spec = validCase()
  objectField(spec, "env").LINEAR_API_KEY = "lin_api_fake"
  parseCase(spec)
})

Deno.test("fixturePort must be declared together with a fixture server", () => {
  rejects(
    (spec) => {
      listField(spec, "substitutions").push("fixturePort")
    },
    "fixturePort",
  )
  rejects(
    (
      spec,
    ) => (spec.fixtureServer = {
      path: "/graphql",
      responses: [{ status: 200, headers: {}, body: { utf8: "{}" } }],
      expectedRequests: 1,
      expectedAuthorization: "lin_api_fake",
    }),
    "fixturePort",
  )
  rejects(
    (
      spec,
    ) => (spec.fixtureServer = {
      path: "/graphql",
      responses: [],
      expectedRequests: 0,
      expectedAuthorization: "x",
    }),
    "responses",
  )
  rejects((spec) => {
    listField(spec, "substitutions").push("fixturePort")
    spec.fixtureServer = {
      path: "/graphql",
      responses: [{ status: 200, headers: {}, body: { utf8: "{}" } }],
      expectedRequests: 2,
      expectedAuthorization: "lin_api_fake",
    }
  }, "response count")
})

Deno.test("substitution replaces only declared placeholders and rejects the rest", () => {
  const values = {
    home: "H",
    configHome: "C",
    cwd: "W",
    cwdRoot: "R",
    bin: "B",
    denoDir: "D",
    fixturePort: "8",
    referenceModuleUrl: "file:///reference",
  }
  assertEquals(
    substitute(
      "{{home}}/x:{{fixturePort}}",
      ["home", "fixturePort"],
      values,
      "t",
    ),
    "H/x:8",
  )
  assertThrows(
    () => substitute("{{home}}", [], values, "t"),
    SchemaError,
    "not declared",
  )
  assertThrows(
    () => substitute("{{nope}}", ["home"], values, "t"),
    SchemaError,
    "unknown placeholder",
  )
  assertThrows(
    () => substitute("{no}} {{ home }}", ["home"], values, "t"),
    SchemaError,
  )
  assertThrows(() => substitute("{{home", ["home"], values, "t"), SchemaError)
})

Deno.test("reference module URL is confined to expected output and PATH is private", () => {
  rejects(
    (spec) => (objectField(spec, "env").PATH = "/usr/bin"),
    "PATH must be the private sandbox placeholder",
  )
  const valid = validCase()
  listField(valid, "substitutions").push("referenceModuleUrl")
  objectField(valid, "expected").stderr = {
    utf8: "at {{referenceModuleUrl}}/src/credentials.ts",
  }
  parseCase(valid)
  rejects((spec) => {
    listField(spec, "substitutions").push("referenceModuleUrl")
    objectField(spec, "expected").stderr = {
      utf8: "{{referenceModuleUrl}}",
    }
    spec.argv = ["{{referenceModuleUrl}}"]
  }, "referenceModuleUrl is restricted to expected output")
  rejects((spec) => {
    listField(spec, "substitutions").push("referenceModuleUrl")
    objectField(spec, "expected").stderr = {
      utf8: "{{referenceModuleUrl}}",
    }
    spec.stdin = { utf8: "{{referenceModuleUrl}}" }
  }, "referenceModuleUrl is restricted to expected output")
  rejects((spec) => {
    listField(spec, "substitutions").push("referenceModuleUrl")
    objectField(spec, "expected").stderr = {
      utf8: "{{referenceModuleUrl}}",
    }
    objectField(spec, "env").OTHER = "{{referenceModuleUrl}}"
  }, "referenceModuleUrl is restricted to expected output")
})

Deno.test("candidate descriptors need an explicit program and route list", () => {
  const descriptor = parseCandidateDescriptor({
    name: "rust",
    program: { kind: "executable", path: "/opt/linear" },
    implementedRoutes: ["linear", "linear api"],
  })
  assertEquals(descriptor.implementedRoutes, ["linear", "linear api"])
  assertEquals(
    parseCandidateDescriptor({
      name: "ref",
      program: { kind: "interpreted-reference", workspace: "/ws" },
      implementedRoutes: "every-manifest-route",
    }).implementedRoutes,
    "every-manifest-route",
  )
  assertThrows(
    () =>
      parseCandidateDescriptor({
        name: "x",
        program: { kind: "executable", path: "relative" },
        implementedRoutes: [],
      }),
    SchemaError,
    "absolute",
  )
  assertThrows(
    () =>
      parseCandidateDescriptor({
        name: "x",
        program: { kind: "executable", path: "/x" },
      }),
    SchemaError,
    "implementedRoutes",
  )
  assertThrows(
    () =>
      parseCandidateDescriptor({
        name: "x",
        program: { kind: "executable", path: "/x" },
        implementedRoutes: "all",
      }),
    SchemaError,
    "implementedRoutes",
  )
  assertThrows(
    () =>
      parseCandidateDescriptor({
        name: "x",
        program: { kind: "deno", path: "/x" },
        implementedRoutes: [],
      }),
    SchemaError,
    "program",
  )
  assertThrows(
    () =>
      parseCandidateDescriptor({
        name: "x",
        program: { kind: "executable", path: "/x" },
        implementedRoutes: ["a", "a"],
      }),
    SchemaError,
    "unique",
  )
})
