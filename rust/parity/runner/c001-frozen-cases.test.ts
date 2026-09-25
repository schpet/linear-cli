import { assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const relativeRoot = "rust/parity/runner/c001-frozen-cases"
const root = new URL("./c001-frozen-cases/", import.meta.url).pathname

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

Deno.test("auth whoami oracle keeps exact cases, private paths, and request identity", async () => {
  const cases = await loadCases(root, new Set(["linear auth whoami"]))
  assertEquals(cases.length, 10)
  let graphqlCount = 0
  for (const item of cases) {
    assertEquals(item.spec.env.PATH, "{{bin}}", item.spec.id)
    assertEquals(item.spec.fixtureServer, null, item.spec.id)
    if (item.spec.graphql == null) continue
    graphqlCount += 1
    assertEquals(item.spec.graphql.expectedRequests, 1, item.spec.id)
    assertEquals(item.spec.graphql.groups.length, 1, item.spec.id)
    const group = item.spec.graphql.groups[0]
    if (group.mode !== "ordered") {
      throw new Error(`${item.spec.id}: expected ordered group`)
    }
    assertEquals(group.steps.length, 1, item.spec.id)
    const step = group.steps[0]
    if (step.kind !== "graphql") {
      throw new Error(`${item.spec.id}: expected GraphQL step`)
    }
    assertEquals(
      step.identity.userAgent,
      "schpet-linear-cli/2.6.0",
      item.spec.id,
    )
  }
  assertEquals(graphqlCount, 7)
  const files = (await filesUnder(root)).sort()
  assertEquals(files.length, 11)
  const lines = await Promise.all(
    files.map(async (file) =>
      `${await sha256Hex(
        await Deno.readFile(join(root, file)),
      )}  ${relativeRoot}/${file}\n`
    ),
  )
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    "8c0e12ccac1bf1043996d75e83960605b09a1c5e97018f9afb5b0b4cae9eefe4",
  )
})
