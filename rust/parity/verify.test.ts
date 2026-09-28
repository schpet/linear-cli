import { assertEquals, assertRejects, assertThrows } from "@std/assert"
import { fromFileUrl, join } from "@std/path"
import { withSourceMap } from "./source-map.ts"
import {
  APPROVED_ROOT_TASKS,
  compareManifest,
  compareRootConfig,
  exportRuntime,
  readBaseline,
  readManifest,
  verifyBaseline,
  verifySourceBinding,
} from "./verify.ts"

const parity = fromFileUrl(new URL("./", import.meta.url))

function findRoute(
  routes: Array<Record<string, unknown>>,
  path: string,
): Record<string, unknown> {
  const route = routes.find((item) => item.path === path)
  if (route == null) throw new Error(`missing ${path}`)
  return route
}

Deno.test("runtime inventory matches checked manifest and resolves key aliases", async () => {
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(parity, "manifest.json"))),
  )
  const runtime = await withSourceMap(await exportRuntime())
  compareManifest(manifest, runtime)

  assertEquals(findRoute(runtime, "linear issue mine").aliases, ["list", "l"])
  assertEquals(findRoute(runtime, "linear issue query").aliases, ["q"])
  const mineTypes = findRoute(runtime, "linear issue mine").localTypes
  if (!Array.isArray(mineTypes)) throw new Error("missing local types")
  assertEquals(mineTypes.find((type) => type.name === "sort")?.values, [
    "manual",
    "priority",
  ])
  assertEquals(findRoute(runtime, "linear api").examples instanceof Array, true)
  assertEquals(
    findRoute(runtime, "linear api").usage,
    "[graphqlDocument:string]",
  )
  assertEquals(findRoute(runtime, "linear document").aliases, ["docs", "doc"])
  assertEquals(findRoute(runtime, "linear completions complete").hidden, true)
  assertEquals(
    findRoute(runtime, "linear completions bash").kind,
    "generated_completion_child",
  )

  const rootOptions = findRoute(runtime, "linear").localOptions
  const labelOptions = findRoute(runtime, "linear label list").localOptions
  const labelGlobals =
    findRoute(runtime, "linear label list").inheritedGlobalOptions
  if (
    !Array.isArray(rootOptions) || !Array.isArray(labelOptions) ||
    !Array.isArray(labelGlobals)
  ) {
    throw new Error("missing option arrays")
  }
  for (const route of runtime) {
    for (const key of ["localOptions", "inheritedGlobalOptions"]) {
      const options = route[key]
      if (!Array.isArray(options)) {
        throw new Error(`${route.path} has no ${key}`)
      }
      for (const option of options) {
        if (
          typeof option.description !== "string" ||
          option.description.trim().length === 0
        ) {
          throw new Error(
            `${route.path} ${key} ${option.name} lacks a description`,
          )
        }
        assertEquals(option.name === "help" || option.name === "version", false)
      }
    }
  }
  assertEquals(rootOptions.some((option) => option.name === "workspace"), true)
  assertEquals(labelOptions.some((option) => option.name === "workspace"), true)
  const rootWorkspace = rootOptions.find((option) =>
    option.name === "workspace"
  )
  assertEquals(
    rootWorkspace?.description,
    "Target workspace (uses credentials)",
  )
  assertEquals(
    labelOptions.find((option) => option.name === "workspace")?.description,
    "Show only workspace-level labels (not team-specific)",
  )
  const mineOptions = findRoute(runtime, "linear issue mine").localOptions
  const mineGlobals =
    findRoute(runtime, "linear issue mine").inheritedGlobalOptions
  if (!Array.isArray(mineOptions) || !Array.isArray(mineGlobals)) {
    throw new Error("missing issue mine options")
  }
  assertEquals(
    mineOptions.find((option) => option.name === "sort")?.description,
    "Sort order (default: priority, can also be set via LINEAR_ISSUE_SORT)",
  )
  assertEquals(
    mineGlobals.find((option) => option.name === "workspace")?.description,
    rootWorkspace?.description,
  )
  // Cliffy shadows the inherited definition at this route. Its existence at
  // root is still recorded separately; parser resolution awaits P07 fixtures.
  assertEquals(
    labelGlobals.some((option) => option.name === "workspace"),
    false,
  )
})

Deno.test("an edited route alias or option fails inventory verification", async () => {
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(parity, "manifest.json"))),
  )
  const runtime = await withSourceMap(await exportRuntime())
  const changedAlias = structuredClone(manifest)
  findRoute(changedAlias.routes, "linear issue mine").aliases = ["list"]
  assertThrows(() => compareManifest(changedAlias, runtime), Error, "drift")

  const changedOption = structuredClone(manifest)
  findRoute(changedOption.routes, "linear issue query").localOptions = []
  assertThrows(() => compareManifest(changedOption, runtime), Error, "drift")

  const changedDescription = structuredClone(manifest)
  const mineOptions =
    findRoute(changedDescription.routes, "linear issue mine").localOptions
  if (!Array.isArray(mineOptions)) throw new Error("missing issue mine options")
  const sortOption = mineOptions.find((option) => option.name === "sort")
  if (sortOption == null) throw new Error("missing issue mine sort option")
  sortOption.description = "changed description"
  assertThrows(
    () => compareManifest(changedDescription, runtime),
    Error,
    "drift",
  )

  const changedEnum = structuredClone(manifest)
  const types = findRoute(changedEnum.routes, "linear issue mine").localTypes
  if (!Array.isArray(types)) throw new Error("missing local types")
  const sort = types.find((type) => type.name === "sort")
  if (sort == null || !Array.isArray(sort.values)) {
    throw new Error("missing sort enum")
  }
  sort.values[0] = "unexpected"
  assertThrows(() => compareManifest(changedEnum, runtime), Error, "drift")

  const changedSource = structuredClone(manifest)
  findRoute(changedSource.routes, "linear issue mine").source =
    "src/commands/issue/issue-query.ts"
  assertThrows(() => compareManifest(changedSource, runtime), Error, "drift")

  const updatedCoverage = structuredClone(manifest)
  findRoute(updatedCoverage.routes, "linear issue mine").fixtureStatus =
    "captured"
  compareManifest(updatedCoverage, runtime)
})

function frozenRootConfig(): Record<string, unknown> {
  return {
    name: "@schpet/linear-cli",
    version: "2.6.0",
    tasks: {
      test: "deno test --allow-all --quiet",
      check: "deno check src/main.ts",
    },
    imports: { valibot: "npm:valibot@^1.3.1" },
    unstable: ["sloppy-imports"],
  }
}

function withApprovedAdditions(): Record<string, unknown> {
  const current = frozenRootConfig()
  current.tasks = {
    ...recordField(current, "tasks"),
    ...APPROVED_ROOT_TASKS,
  }
  current.test = { exclude: ["rust/"] }
  return current
}

function recordField(
  value: Record<string, unknown>,
  key: string,
): Record<string, unknown> {
  const field = value[key]
  if (!isRecord(field)) {
    throw new Error(`${key} is not an object`)
  }
  return field
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value != null && !Array.isArray(value)
}

Deno.test("source binding accepts only the reviewed parity task and test.exclude additions", () => {
  compareRootConfig(frozenRootConfig(), frozenRootConfig())
  compareRootConfig(frozenRootConfig(), withApprovedAdditions())
  const cases: Array<[string, (current: Record<string, unknown>) => void]> = [
    ["version", (current) => (current.version = "2.6.1")],
    [
      "imports",
      (
        current,
      ) => (recordField(current, "imports").valibot = "npm:valibot@^1.4.0"),
    ],
    [
      "tasks.test",
      (
        current,
      ) => (recordField(current, "tasks").test = "deno test --allow-all"),
    ],
    ["unstable", (current) => (current.unstable = [])],
    [
      "extra task",
      (
        current,
      ) => (recordField(current, "tasks").extra = "deno run x"),
    ],
    [
      "changed parity task",
      (
        current,
      ) => (recordField(current, "tasks").parity =
        "deno run --allow-all rust/parity/runner/main.ts"),
    ],
    [
      "extra exclude",
      (current) => (current.test = { exclude: ["rust/", "test/"] }),
    ],
    [
      "extra test key",
      (current) => (current.test = { exclude: ["rust/"], include: ["src/"] }),
    ],
    ["new top-level key", (current) => (current.compilerOptions = {})],
  ]
  for (const [label, mutate] of cases) {
    const current = withApprovedAdditions()
    mutate(current)
    assertThrows(
      () => compareRootConfig(frozenRootConfig(), current),
      Error,
      "approved",
      label,
    )
  }
})

Deno.test("the working copy is bound to the frozen reference source", async () => {
  const baseline = readBaseline(
    JSON.parse(await Deno.readTextFile(join(parity, "baseline.json"))),
  )
  await verifySourceBinding(baseline)
})

Deno.test("an unrelated reference directory is rejected before any hash checks", async () => {
  const directory = await Deno.makeTempDir()
  try {
    await assertRejects(
      () => verifyBaseline({}, directory, join(directory, "missing-binary")),
      Error,
      "interpreted reference must use the repository working copy",
    )
  } finally {
    await Deno.remove(directory, { recursive: true })
  }
})
