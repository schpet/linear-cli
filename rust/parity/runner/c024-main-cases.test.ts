import { nativeParserContract } from "./native-parser-contract.ts"
import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT } from "./schema.ts"

const runner = new URL("./", import.meta.url).pathname
const frozen = join(runner, "c024-frozen-cases")
const main = join(runner, "cases")

async function names(root: string): Promise<string[]> {
  const found: string[] = []
  for await (const entry of Deno.readDir(root)) {
    if (
      entry.isFile && entry.name.startsWith("c024-") &&
      entry.name.endsWith(".json")
    ) {
      found.push(entry.name)
    }
  }
  return found.sort()
}

Deno.test("C024 promoted cases preserve all 54 frozen source contracts", async () => {
  const files = await names(frozen)
  assertEquals(files.length, 54)
  assertEquals(await names(main), files)
  for (const file of files) {
    const source = parseCase(
      JSON.parse(await Deno.readTextFile(join(frozen, file))),
      file,
    )
    const promoted = parseCase(
      JSON.parse(await Deno.readTextFile(join(main, file))),
      file,
    )
    assertEquals(source.deviation, null, file)
    assertEquals({ ...promoted, deviation: null }, source, file)
  }
  // The existing C023 workspace fixture has the same parsed value as C024's
  // differently quoted TOML; the promoted cases retain the original fixture
  // name and all case fields while sharing that fixture.
  assertEquals(
    await Deno.readTextFile(
      join(main, "fixtures/workspace-config/linear.toml"),
    ),
    "workspace = 'alpha'\n",
  )
  assertEquals(
    await Deno.readTextFile(
      join(frozen, "fixtures/workspace-config/linear.toml"),
    ),
    'workspace = "alpha"\n',
  )
})

const specific = new Map<string, [string, string[]]>([
  ["c024-alias-help", ["C024-CLI-VERSION", ["stdout"]]],
  ["c024-extra-arg", ["C024-CLI-VERSION", ["stdout"]]],
  ["c024-help", ["C024-CLI-VERSION", ["stdout"]]],
  ["c024-parent-help", ["C024-CLI-VERSION", ["stdout"]]],
  ["c024-unknown-flag", ["C024-CLI-VERSION", ["stdout"]]],
  ["c024-extra-wire-json", ["C024-TYPED-JSON-FIELDS", [
    "stdout",
    "graphql-user-agent",
  ]]],
  ["c024-one-null-sort-json", ["C024-STRICT-FLOAT-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c024-one-node-null-sort-text", ["C024-STRICT-FLOAT-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c024-two-null-sort-text", ["C024-STRICT-FLOAT-DECODE", [
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c024-overflow-json", ["C024-STRICT-NUMBER-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c024-overflow-text", ["C024-STRICT-NUMBER-DECODE", [
    "exit",
    "stdout",
    "stderr",
    "graphql-user-agent",
  ]]],
  ["c024-detail-http", ["C024-TRANSPORT-DIAGNOSTIC", [
    "stderr",
    "graphql-user-agent",
  ]]],
])

Deno.test("C024 goldens pin only reviewed Rust differences", async () => {
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(runner, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest path is not text")
    }
    return route.path
  }))
  const loaded = await loadCases(main, routes, "c024", RUST_CONTRACT)
  assertEquals(loaded.length, 54)
  assertEquals(loaded.filter((entry) => entry.spec.graphql != null).length, 35)
  assertEquals(loaded.filter((entry) => entry.golden != null).length, 40)
  assertEquals(specific.size, 12)
  for (const entry of loaded) {
    const exceptional = nativeParserContract(entry.spec.id) ??
      specific.get(entry.spec.id)
    const id = exceptional?.[0] ??
      (entry.spec.graphql == null ? null : "C024-GRAPHQL-UA")
    const surfaces = exceptional?.[1] ??
      (entry.spec.graphql == null ? null : ["graphql-user-agent"])
    assertEquals(entry.spec.deviation?.id ?? null, id, entry.spec.id)
    assertEquals(
      entry.golden?.spec.approvedSurfaces ?? null,
      surfaces,
      entry.spec.id,
    )
  }
})
