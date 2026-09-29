import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { candidateCaseView, loadCases, resolveCase } from "./cases.ts"
import { sha256Hex } from "./bytes.ts"
import { compareGraphQLFixture } from "./compare.ts"
import {
  loadPinnedGraphQLSchema,
  startGraphQLServer,
} from "./graphql-server.ts"
import {
  parseCase,
  parseReviewedGolden,
  type ReviewedGolden,
  RUST_CONTRACT,
  RUST_USER_AGENT,
  SchemaError,
} from "./schema.ts"

Deno.test("C033 decimal-parser mutation rejection pins source effects and rejects requests or state overrides", async () => {
  const source = parseCase(
    JSON.parse(
      await Deno.readTextFile(
        new URL("./c033-frozen-cases/c033-sort-radix.json", import.meta.url),
      ),
    ),
  )
  const golden = parseReviewedGolden(
    JSON.parse(
      await Deno.readTextFile(
        new URL(
          "./cases/rust-goldens/rust-3.0.0-alpha.1/c033-sort-radix.json",
          import.meta.url,
        ),
      ),
    ),
  )
  const dir = await Deno.makeTempDir({ prefix: "c033-rejection-" })
  const target = join(dir, "rust-goldens", RUST_CONTRACT)
  await Deno.mkdir(target, { recursive: true })
  const write = async (value: ReviewedGolden, spec = source) => {
    const raw = JSON.stringify(value, null, 2) + "\n"
    await Deno.writeTextFile(join(target, `${source.id}.json`), raw)
    await Deno.writeTextFile(
      join(dir, `${source.id}.json`),
      JSON.stringify({
        ...spec,
        deviation: {
          id: value.deviationId,
          contract: RUST_CONTRACT,
          sha256: await sha256Hex(new TextEncoder().encode(raw)),
        },
      }),
    )
  }
  const load = async () =>
    (await loadCases(dir, new Set([source.route]), undefined, RUST_CONTRACT))[0]
  try {
    await write(golden)
    const loaded = await load()
    const view = candidateCaseView(loaded)
    assertEquals(loaded.spec.graphql, source.graphql)
    assertEquals(view.spec.graphql?.expectedRequests, 0)
    assertEquals(view.spec.graphql?.groups, [])
    assertEquals(
      view.spec.graphql?.expectedRecords,
      source.graphql?.initialRecords,
    )
    const resolved = resolveCase(view.spec, {
      home: "h",
      configHome: "c",
      cwd: "d",
      cwdRoot: "r",
      bin: "b",
      denoDir: "x",
      fixturePort: "1",
      referenceModuleUrl: "file:///reference",
    }, RUST_USER_AGENT)
    const fixture = resolved.graphql
    assert(fixture != null)
    const server = startGraphQLServer(
      () => fixture,
      await loadPinnedGraphQLSchema(),
    )
    try {
      assertEquals(compareGraphQLFixture(fixture, server), [])
      const group = source.graphql?.groups[0]
      assert(group?.mode === "ordered" && group.steps[0].kind === "graphql")
      const request = group.steps[0].operation
      const response = await fetch(`http://127.0.0.1:${server.port}/graphql`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          authorization: "lin_api_fake",
          "user-agent": RUST_USER_AGENT,
        },
        body: JSON.stringify({
          query: request.document,
          variables: request.variables,
        }),
      })
      await response.text()
      assertEquals(response.status, 500)
      assertEquals(server.unexpected, 1)
      assert(
        compareGraphQLFixture(fixture, server).some((item) =>
          item.surface === "fixture"
        ),
      )
      const tampered = { ...fixture, expectedRecords: { changed: true } }
      assert(
        compareGraphQLFixture(tampered, server).some((item) =>
          item.detail.includes("records")
        ),
      )
    } finally {
      await server.stop()
    }
    const wrongName = { ...golden, deviationId: "OTHER" }
    await write(wrongName)
    await assertRejects(load, SchemaError, "exact C033 contract")
    const wrongExit = structuredClone(golden)
    assert(wrongExit.candidate.expected != null)
    wrongExit.candidate.expected.exit = { code: 0 }
    await write(wrongExit)
    await assertRejects(load, SchemaError, "exact C033 contract")
    const writes = structuredClone(golden)
    assert(writes.candidate.expected != null)
    writes.candidate.expected.fileEffects = [{
      path: "unexpected",
      kind: "directory",
      change: "created",
    }]
    await write(writes)
    await assertRejects(load, SchemaError, "exact C033 contract")
    const alteredSource = structuredClone(source)
    assert(alteredSource.graphql != null)
    alteredSource.graphql.expectedRecords = { changed: true }
    await write(golden, alteredSource)
    await assertRejects(load, SchemaError, "exact C033 contract")
    assertThrows(
      () =>
        parseReviewedGolden({
          ...golden,
          candidate: {
            ...golden.candidate,
            graphql: { steps: [], expectedRecords: {} },
          },
        }),
      SchemaError,
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
