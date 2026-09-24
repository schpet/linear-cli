import { assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { loadCases } from "./cases.ts"
import { parseCase, SchemaError } from "./schema.ts"

async function source() {
  return parseCase(JSON.parse(
    await Deno.readTextFile(
      new URL("./cases/document-fixed-host-both.json", import.meta.url),
    ),
  ))
}

async function loaded(spec: Awaited<ReturnType<typeof source>>): Promise<void> {
  const dir = await Deno.makeTempDir()
  try {
    spec.id = "sample"
    await Deno.writeTextFile(join(dir, "sample.json"), JSON.stringify(spec))
    await loadCases(dir, new Set(["linear document view"]))
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

Deno.test("fixedHost is exactly two GET hosts and transport env is runner-owned", async () => {
  const valid = await source()
  assertEquals(valid.graphql?.expectedRequests, 3)
  const fixture = valid.graphql
  if (fixture?.groups[0].mode !== "ordered") throw new Error("bad test fixture")
  const asset = fixture.groups[0].steps[1]
  if (asset?.kind !== "asset") throw new Error("bad asset")
  Object.assign(asset, { fixedHost: "example.invalid" })
  assertThrows(() => parseCase(valid), SchemaError, "fixedHost")
  asset.fixedHost = "uploads.linear.app"
  asset.method = "PUT"
  assertThrows(() => parseCase(valid), SchemaError, "GET downloads only")
  asset.method = "GET"
  valid.env.HTTPS_PROXY = "http://127.0.0.1:1"
  assertThrows(() => parseCase(valid), SchemaError, "runner-owned")
})

Deno.test("fixed-host redirects stay same-host and GraphQL URLs have declared assets", async () => {
  const cross = await source()
  const fixture = cross.graphql
  if (fixture?.groups[0].mode !== "ordered") throw new Error("bad test fixture")
  const first = fixture.groups[0].steps[1]
  const second = fixture.groups[0].steps[2]
  if (first?.kind !== "asset" || second?.kind !== "asset") {
    throw new Error("bad assets")
  }
  first.response.status = 302
  first.response.body = { utf8: "" }
  first.response.location = second.path
  await assertRejects(() => loaded(cross), SchemaError, "same-host")

  const undeclared = await source()
  const other = undeclared.graphql
  if (other?.groups[0].mode !== "ordered") throw new Error("bad test fixture")
  other.groups[0].steps.pop()
  other.expectedRequests = 2
  await assertRejects(
    () => loaded(undeclared),
    SchemaError,
    "no declared asset",
  )
})
