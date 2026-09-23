import { assertEquals, assertThrows } from "@std/assert"
import { fromFileUrl, join } from "@std/path"
import { withSourceMap } from "./source-map.ts"
import { compareManifest, exportRuntime, readManifest } from "./verify.ts"

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
  assertEquals(rootOptions.some((option) => option.name === "workspace"), true)
  assertEquals(labelOptions.some((option) => option.name === "workspace"), true)
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
