import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const relativeRoot = "rust/parity/runner/c011-frozen-cases"
const root = new URL("./c011-frozen-cases/", import.meta.url).pathname
const manifest = new URL("./c011-frozen-cases.sha256", import.meta.url)
const manifestSha256 =
  "cacb06721e17bf2c4d175c65df0f167a45397ccdac040fc00b5d42c0cfeeb7ab"

async function filesUnder(directory: string, prefix = ""): Promise<string[]> {
  const files: string[] = []
  for await (const entry of Deno.readDir(directory)) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`
    if (entry.isDirectory) {
      files.push(...await filesUnder(join(directory, entry.name), relative))
    } else if (entry.isFile) {
      files.push(relative)
    } else {
      throw new Error(`unexpected oracle fixture entry ${relative}`)
    }
  }
  return files
}

Deno.test("C011 team states oracle is strict, public, and SHA-pinned", async () => {
  const cases = await loadCases(root, new Set(["linear team states"]))
  assertEquals(cases.length, 40)
  for (const item of cases) {
    const spec = item.spec
    assertEquals(spec.id.startsWith("c011-"), true, spec.id)
    assertEquals(
      spec.argv.includes("team") || spec.argv.includes("t"),
      true,
      spec.id,
    )
    assertEquals(spec.argv.includes("states"), true, spec.id)
    assertEquals(spec.fixtureServer, null, spec.id)
    if (spec.env.LINEAR_API_KEY != null) {
      assertEquals(spec.env.LINEAR_API_KEY, "lin_api_fake", spec.id)
    }
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1", spec.id)
    assertEquals(spec.expected.fileEffects, [], spec.id)
    if (
      "code" in spec.expected.exit && spec.expected.exit.code === 0 &&
      !spec.argv.includes("--help")
    ) {
      const group = spec.graphql?.groups[0]
      if (group == null || group.mode !== "ordered") {
        throw new Error(`${spec.id}: successful case needs ordered GraphQL`)
      }
      const last = group.steps.at(-1)
      if (
        last?.kind !== "graphql" ||
        !last.operation.document.includes("query GetWorkflowStates") ||
        typeof last.operation.variables?.teamKey !== "string"
      ) {
        throw new Error(`${spec.id}: success must assert selected teamKey`)
      }
    }
  }
  const files = (await filesUnder(root)).sort()
  const lines = await Promise.all(
    files.map(async (file) =>
      `${await sha256Hex(
        await Deno.readFile(join(root, file)),
      )}  ${relativeRoot}/${file}\n`
    ),
  )
  assertEquals(await Deno.readTextFile(manifest), lines.join(""))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    manifestSha256,
  )
})
