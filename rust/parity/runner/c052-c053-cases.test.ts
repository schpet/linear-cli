import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { sha256Hex } from "./bytes.ts"
import {
  parseCase,
  parseReviewedGolden,
  RUST_CONTRACT,
  RUST_USER_AGENT,
} from "./schema.ts"

const root = new URL("./", import.meta.url).pathname
const frozen = join(root, "c052-c053-frozen-cases")
const guardId = "c053-repeated-cursor-source-finite-success"
const parserIds = new Set([
  "c052-empty-inline-falls-empty-file",
  "c053-metadata-empty-content-does-not-read-stdin",
  "c053-no-fields-empty-title-icon",
])
function routes() {
  const manifest = readManifest(
    JSON.parse(Deno.readTextFileSync(join(root, "../manifest.json"))),
  )
  return new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
}
Deno.test("C052/C053 binds all38 exact source cases and only named native diagnostic/cursor surfaces", async () => {
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "5328f99f01443df798c9ef5b541b3b60e6a97507a61e35b9e94da00a4c6dcaa1",
  )
  const lines = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(lines.length, 40)
  for (const line of lines) {
    const [sha, path] = line.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  const cases =
    (await loadCases(join(root, "cases"), routes(), undefined, RUST_CONTRACT))
      .filter((entry) => /^c05[23]-/.test(entry.spec.id))
  assertEquals(cases.length, 38)
  assertEquals(
    cases.filter((entry) => entry.spec.route === "linear document create")
      .length,
    18,
  )
  assertEquals(
    cases.filter((entry) => entry.spec.route === "linear document update")
      .length,
    20,
  )
  let ua = 0, parser = 0, http = 0, cursor = 0, ordinaryLocal = 0
  for (const entry of cases) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    const candidate = candidateCaseView(entry)
    assertEquals(entry.golden?.spec.candidate.argv, undefined)
    assertEquals(
      candidate.spec.expected.fileEffects,
      source.expected.fileEffects,
    )
    if (entry.spec.id === guardId) {
      cursor++
      assertEquals(entry.spec.deviation?.id, "DOC-GUARD-PAGINATION")
      assertEquals(entry.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
        "graphql-fixture",
        "graphql-user-agent",
      ])
      assertEquals(source.expected.exit, { code: 0 })
      assertEquals(candidate.spec.expected.exit, { code: 1 })
      assertEquals(source.graphql?.expectedRequests, 4)
      assert(
        source.graphql != null && source.graphql.groups[0].mode === "ordered",
      )
      const mutation = source.graphql.groups[0].steps[3]
      assert(mutation.kind === "graphql")
      assertEquals(mutation.effects.length, 1)
      assertEquals(candidate.spec.graphql?.expectedRequests, 2)
      assertEquals(
        candidate.spec.graphql?.expectedRecords,
        source.graphql.initialRecords,
      )
      assertEquals(entry.golden?.spec.candidate.graphql, {
        steps: [{ id: "first" }, { id: "second" }],
      })
    } else if (parserIds.has(source.id)) {
      parser++
      assertEquals(entry.spec.deviation?.id, "CLAP-NATIVE-CLI-SURFACE")
      assertEquals(entry.golden?.spec.approvedSurfaces, ["stdout", "stderr"])
      assertEquals(candidate.spec.expected.exit, source.expected.exit)
      assertEquals(source.graphql, null)
    } else if (source.id.endsWith("http-failure")) {
      http++
      assertEquals(entry.spec.deviation?.id, "DOC-WRITE-HTTP-DIAGNOSTIC")
      assertEquals(entry.golden?.spec.approvedSurfaces, [
        "stderr",
        "graphql-user-agent",
      ])
      assertEquals(candidate.spec.expected.exit, source.expected.exit)
      assertEquals(candidate.spec.expected.stdout, source.expected.stdout)
      assertEquals(candidate.spec.graphql, source.graphql)
    } else {
      assertEquals(candidate.spec.expected, source.expected)
      assertEquals(candidate.spec.graphql, source.graphql)
      if (source.graphql != null) {
        ua++
        assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
        assertEquals(entry.golden?.spec.approvedSurfaces, [
          "graphql-user-agent",
        ])
      } else {
        ordinaryLocal++
        assertEquals(entry.spec.deviation, null)
      }
    }
    if (source.graphql != null) {
      assertEquals(
        entry.golden?.spec.candidate.graphqlUserAgent,
        RUST_USER_AGENT,
      )
    }
  }
  assertEquals([ua, parser, http, cursor, ordinaryLocal], [26, 3, 2, 1, 6])
  for (const name of ["document-write-empty", "document-write-lossy"]) {
    assertEquals(
      await Deno.readFile(join(root, "cases/fixtures", name, "body.md")),
      await Deno.readFile(join(frozen, "fixtures", name, "body.md")),
    )
  }
})

Deno.test("pinned document cursor adapter rejects altered source success, effects, prefix and outcome", async () => {
  const source = parseCase(
    JSON.parse(await Deno.readTextFile(join(frozen, `${guardId}.json`))),
  )
  const golden = parseReviewedGolden(
    JSON.parse(
      await Deno.readTextFile(
        join(root, "cases/rust-goldens", RUST_CONTRACT, `${guardId}.json`),
      ),
    ),
  )
  for (
    const change of [
      "source-input",
      "source-exit",
      "source-record",
      "source-response",
      "prefix",
      "candidate-exit",
      "candidate-stdout",
      "candidate-files",
      "agent",
      "renamed-case",
    ]
  ) {
    const dir = await Deno.makeTempDir()
    try {
      const spec = structuredClone(source), candidate = structuredClone(golden)
      assert(spec.graphql != null && spec.graphql.groups[0].mode === "ordered")
      assert(
        candidate.candidate.expected != null &&
          candidate.candidate.graphql != null,
      )
      switch (change) {
        case "source-input":
          spec.argv.push("--force")
          break
        case "source-exit":
          spec.expected.exit = { code: 1 }
          break
        case "source-record":
          spec.graphql.expectedRecords = {}
          break
        case "source-response":
          spec.graphql.groups[0].steps[1].response = { kind: "data", data: {} }
          break
        case "prefix":
          candidate.candidate.graphql.steps.push({ id: "third" })
          break
        case "candidate-exit":
          candidate.candidate.expected.exit = { code: 0 }
          break
        case "candidate-stdout":
          candidate.candidate.expected.stdout = { utf8: "partial\n" }
          break
        case "candidate-files":
          candidate.candidate.expected.fileEffects = [{
            path: "written",
            change: "removed",
            kind: "file",
          }]
          break
        case "agent":
          delete candidate.candidate.graphqlUserAgent
          break
        case "renamed-case":
          spec.id = "c053-another-case"
          candidate.caseId = spec.id
          break
        default:
          throw new Error("unexpected tamper case")
      }
      const text = JSON.stringify(candidate)
      spec.deviation = {
        id: candidate.deviationId,
        contract: RUST_CONTRACT,
        sha256: await sha256Hex(new TextEncoder().encode(text)),
      }
      const goldens = join(dir, "rust-goldens", RUST_CONTRACT)
      await Deno.mkdir(goldens, { recursive: true })
      await Deno.writeTextFile(join(goldens, `${spec.id}.json`), text)
      await Deno.writeTextFile(
        join(dir, `${spec.id}.json`),
        JSON.stringify(spec),
      )
      await assertRejects(() =>
        loadCases(dir, routes(), undefined, RUST_CONTRACT)
      )
    } finally {
      await Deno.remove(dir, { recursive: true })
    }
  }
})

Deno.test("document cursor binding rejects a source object mutated after loading", async () => {
  const loaded =
    (await loadCases(join(root, "cases"), routes(), guardId, RUST_CONTRACT))[0]
  assertEquals(candidateCaseView(loaded).spec.graphql?.expectedRequests, 2)
  loaded.spec.expected.exit = { code: 1 }
  assertThrows(() => candidateCaseView(loaded))
})
