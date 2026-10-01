import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { parseCase, RUST_CONTRACT, RUST_USER_AGENT } from "./schema.ts"
Deno.test("C073 freezes23 complete comment update contracts and exact body/request/effect semantics", async () => {
  const root = new URL("./", import.meta.url).pathname
  const manifest = readManifest(
    JSON.parse(await Deno.readTextFile(join(root, "../manifest.json"))),
  )
  const routes = new Set(manifest.routes.map((route) => {
    assert(typeof route.path === "string")
    return route.path
  }))
  const frozen = join(root, "c073-frozen-cases")
  const pins = await Deno.readFile(join(frozen, "source.sha256"))
  assertEquals(
    await sha256Hex(pins),
    "974446d64b87cb9a42cdd26bd45824a4114fcaa08419e68d47c531e6b4f44969",
  )
  const rows = new TextDecoder().decode(pins).trimEnd().split("\n")
  assertEquals(rows.length, 23)
  for (const row of rows) {
    const [sha, path] = row.split("  ")
    assertEquals(await sha256Hex(await Deno.readFile(join(frozen, path))), sha)
  }
  for (const [name, sha] of FILE_PINS) {
    const a = await Deno.readFile(join(frozen, "fixtures/comment-files", name))
    const b = await Deno.readFile(
      join(root, "cases/fixtures/comment-files", name),
    )
    assertEquals(await sha256Hex(a), sha)
    assertEquals(b, a)
  }
  for (const fixture of [frozen, join(root, "cases")]) {
    assert(
      (await Deno.stat(join(fixture, "fixtures/comment-files/directory")))
        .isDirectory,
    )
    assertEquals(
      await Deno.readFile(
        join(fixture, "fixtures/comment-files/directory/.keep"),
      ),
      new Uint8Array(),
    )
  }
  const entries =
    (await loadCases(join(root, "cases"), routes, undefined, RUST_CONTRACT))
      .filter((entry) => entry.spec.id.startsWith("c073-"))
  assertEquals(entries.length, 23)
  const counts = new Map<string, number>()
  for (const entry of entries) {
    const source = parseCase(
      JSON.parse(
        await Deno.readTextFile(join(frozen, `${entry.spec.id}.json`)),
      ),
    )
    assertEquals({ ...entry.spec, deviation: null }, source)
    assertEquals(source.route, "linear issue comment update")
    const candidate = candidateCaseView(entry)
    assertEquals(candidate.spec.argv, source.argv)
    assertEquals(candidate.spec.stdin, source.stdin)
    assertEquals(candidate.spec.graphql, source.graphql)
    assertEquals(
      candidate.spec.expected.fileEffects,
      source.expected.fileEffects,
    )
    assertEquals(candidate.spec.expected.exit, source.expected.exit)
    const kind = entry.spec.deviation?.id ?? "exact"
    counts.set(kind, (counts.get(kind) ?? 0) + 1)
    if (entry.golden == null) {
      assertEquals(kind, "exact")
      assertEquals(candidate.spec.expected, source.expected)
      continue
    }
    assertEquals(entry.golden.sha256, GOLD_PINS.get(source.id))
    if (kind === "CLAP-NATIVE-CLI-SURFACE") {
      assertEquals(source.graphql, null)
      assertEquals(
        entry.golden.spec.approvedSurfaces,
        source.id.endsWith("help") ? ["stdout"] : ["stdout", "stderr"],
      )
    } else if (kind === "C073-FILE-OS-TEXT") {
      assertEquals(entry.golden.spec.approvedSurfaces, ["stderr"])
      assertEquals(candidate.spec.expected.stdout, source.expected.stdout)
      assert(
        "utf8" in candidate.spec.expected.stderr &&
          "utf8" in source.expected.stderr,
      )
      assertEquals(
        candidate.spec.expected.stderr.utf8.split("\n")[0],
        source.expected.stderr.utf8.split("\n")[0],
      )
      assert(
        candidate.spec.expected.stderr.utf8.split("\n")[1].startsWith(
          "  Error: ",
        ),
      )
    } else {
      assertEquals(
        entry.golden.spec.candidate.graphqlUserAgent,
        RUST_USER_AGENT,
      )
      assertEquals(candidate.spec.expected.stderr, source.expected.stderr)
      if (kind === "C073-PROMPT-RENDER") {
        assertEquals(entry.golden.spec.approvedSurfaces, [
          "stdout",
          "graphql-user-agent",
        ])
        assert(source.id.includes("prompt") || source.id.includes("default"))
      } else {
        assertEquals(kind, "R01H-GRAPHQL-UA")
        assertEquals(entry.golden.spec.approvedSurfaces, ["graphql-user-agent"])
        assertEquals(candidate.spec.expected.stdout, source.expected.stdout)
      }
    }
  }
  assertEquals(Object.fromEntries(counts), {
    "exact": 3,
    "C073-PROMPT-RENDER": 4,
    "R01H-GRAPHQL-UA": 12,
    "C073-FILE-OS-TEXT": 2,
    "CLAP-NATIVE-CLI-SURFACE": 2,
  })
})
const GOLD_PINS = new Map<string, string>([
  [
    "c073-mutation-null-comment-response",
    "a9bcfdc0d57106fff85bd349afbc731aa8db24146e8a410e682fc3cd416a223d",
  ],
  [
    "c073-file-lossy",
    "e9eb57581a0eda9911bbf24a0860b72cf8fcedb1134e9d468d768f31af46bd6c",
  ],
  [
    "c073-leaf-help",
    "d3d385032289775a3694f17096ccbc3fe9418ae0bcb64e4d49175531aa3ca5fd",
  ],
  [
    "c073-nonlinear-url-passthrough",
    "2704ebe0453da2117a3453b6784f81c6825419e3d0fd02b6320bf984ff1a7b17",
  ],
  [
    "c073-empty-file-prompts-default",
    "dcaf58f12fdbe71e3e64e393e5238d00b6aca62d3dbc9522e8216028b50fe7c4",
  ],
  [
    "c073-null-get-prompts-new",
    "a4655a1bf832485f548f05dd8d709eee9091fe3820369067d346a38497cdc934",
  ],
  [
    "c073-get-friendly-error-before-prompt",
    "faccf8781b5cefd9473c998146239e4d8e47cc401d3a72f303a325764882bf40",
  ],
  [
    "c073-file-missing",
    "c8371e4aaa01873d65ea3b7c52ffec865ec6e06e402a9add9773988909ab275e",
  ],
  [
    "c073-mutation-friendly-error",
    "ab714abbcb05a043cfdc297c80e13f989de68d17783db5d8992f8d3da4f56a64",
  ],
  [
    "c073-json-rejected",
    "05ae58634b7ccc5dcf538f7bdcdd9ce5d111d67e0b19d9c67654420c7c9db41a",
  ],
  [
    "c073-file-whitespace",
    "456ac9081373d514da4c94ebf05caf71d1ccd2a7d2a84d5c400a7d0fd7dda10a",
  ],
  [
    "c073-whitespace-body-success",
    "bd33ae4a3314ee410899a113ccba70ec55c8a369ff3c584a1b89c9c6b1529168",
  ],
  [
    "c073-no-body-default-accepted",
    "d99658c1ce430348c1639cb671219859f09e5a9ad4c17f631721de8ddea901e4",
  ],
  [
    "c073-prompt-blank-rejected",
    "89f7938646b893593e787b628aeee8887419294ad4696dc6339f239518edd012",
  ],
  [
    "c073-raw-body-markdown",
    "507a6dd7e7c0970738f93ba693fc8f2347c8486832849c13907bdc7cb9b0e123",
  ],
  [
    "c073-opaque-id-preserved",
    "e6871b435de2916f35c83606f6bd7d3087c75e44fd133299a2ffd15ec1089fb0",
  ],
  [
    "c073-mutation-false-doublecontext",
    "c97d7abd0a29d7133bdc8744e51bfed084a99d9992faf208ad2d6bb969e9e16a",
  ],
  [
    "c073-file-bom-crlf",
    "333a38814b94c8bb7c9d627c95aae7c9255cc7add12c45655275f75269853829",
  ],
  [
    "c073-returned-url-empty-line",
    "cb43472ffadfd1e8048dc29fc81e1f010ac71a75cd6ba9ac0cf0f4853c9c66ce",
  ],
  [
    "c073-file-directory",
    "2884b45011be5fd8133ae23e5dbf5a9981cc2dc775b9cc51480b9321ce8116ef",
  ],
])
const FILE_PINS = new Map<string, string>([
  [
    "body.md",
    "4f3f9fca6077aa4f6b880b8e04e7607764fef2be6f7878cb60ffc04aedea78e5",
  ],
  [
    "empty.md",
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  ],
  [
    "invalid.md",
    "fcede4b5fdef31c2e1b78e0ca4c44035835b383e904100a35b86a5850668ac06",
  ],
  [
    "spaces.md",
    "9b8318187072010f3081af0de6d1c356f4695d1b5a5e1778629153663acad751",
  ],
])
