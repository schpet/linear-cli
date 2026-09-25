import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const relativeRoot = "rust/parity/runner/f06-teamref-frozen-cases"
const root = new URL("./f06-teamref-frozen-cases/", import.meta.url).pathname
const manifest = new URL("./f06-teamref-frozen-cases.sha256", import.meta.url)
const manifestSha256 =
  "721d0976c9b8326d66f6b904937132500ee2d2ce484c070fd7ac5541023f5d3b"

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

Deno.test("F06 team resolver oracle is strict, public, and SHA-pinned", async () => {
  const cases = await loadCases(root, new Set(["linear team members"]))
  assertEquals(cases.length, 48)
  for (const item of cases) {
    const spec = item.spec
    assertEquals(spec.argv.includes("members"), true, spec.id)
    assertEquals(spec.argv.includes("--json"), true, spec.id)
    assertEquals(spec.fixtureServer, null, spec.id)
    if ("code" in spec.expected.exit && spec.expected.exit.code === 0) {
      const group = spec.graphql?.groups[0]
      if (group == null || group.mode !== "ordered") {
        throw new Error(`${spec.id}: successful case needs ordered GraphQL`)
      }
      const last = group.steps.at(-1)
      if (
        last?.kind !== "graphql" ||
        !last.operation.document.includes("query GetTeamMembers") ||
        typeof last.operation.variables?.teamKey !== "string"
      ) {
        throw new Error(`${spec.id}: success must assert selected teamKey`)
      }
      assertEquals(last.response.kind, "data", spec.id)
      if (last.response.kind === "data") {
        assertEquals(
          last.response.data,
          {
            team: {
              members: {
                nodes: [],
                pageInfo: { hasNextPage: false, endCursor: null },
              },
            },
          },
          spec.id,
        )
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
