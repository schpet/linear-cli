import { assert, assertEquals } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"

const root = new URL("./c010-frozen-cases/", import.meta.url).pathname
const manifest = new URL("./c010-frozen-cases.sha256", import.meta.url)
const manifestSha256 =
  "7355f44d3360c739883b19c607424366ab7ea6feffd499ecb47a085ccf80e92e"

Deno.test("C010 freezes strict team-members oracle cases", async () => {
  const loaded = await loadCases(root, new Set(["linear team members"]))
  assertEquals(loaded.length, 45)
  const files: string[] = []
  async function collect(dir: string, prefix = ""): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const name = prefix === "" ? entry.name : `${prefix}/${entry.name}`
      assert(!entry.isSymlink, `unexpected C010 symlink ${name}`)
      if (entry.isDirectory) await collect(join(dir, entry.name), name)
      else {
        assert(entry.isFile, `unexpected C010 entry ${name}`)
        files.push(name)
      }
    }
  }
  await collect(root)
  files.sort()
  assertEquals(
    files.filter((name) => name.endsWith(".json")),
    loaded.map(({ spec }) => `${spec.id}.json`),
  )
  assertEquals(files.filter((name) => !name.endsWith(".json")), [
    "fixtures/c010-project/linear.toml",
  ])
  let requests = 0
  let memberRequests = 0
  for (const { spec } of loaded) {
    assert(spec.id.startsWith("c010-"), spec.id)
    assertEquals(spec.route, "linear team members", spec.id)
    assertEquals(spec.fixtureServer, null, spec.id)
    assertEquals(spec.deviation, null, spec.id)
    assertEquals(spec.expected.fileEffects, [], spec.id)
    assertEquals(spec.env.PATH, "{{bin}}", spec.id)
    assertEquals(spec.env.LINEAR_IGNORE_ENV_FILE, "1", spec.id)
    assertEquals(spec.env.NO_COLOR, "1", spec.id)
    assertEquals(
      spec.env.LINEAR_API_KEY == null ||
        spec.env.LINEAR_API_KEY === "lin_api_fake",
      true,
      spec.id,
    )
    if (spec.graphql == null) continue
    assertEquals(spec.graphql.path, "/graphql", spec.id)
    assertEquals(spec.graphql.groups.length, 1, spec.id)
    const group = spec.graphql.groups[0]
    assertEquals(group.mode, "ordered", spec.id)
    if (group.mode !== "ordered") throw new Error("ordered group required")
    assertEquals(group.steps.length, spec.graphql.expectedRequests, spec.id)
    requests += group.steps.length
    for (const step of group.steps) {
      assertEquals(step.kind, "graphql", spec.id)
      if (step.kind !== "graphql") throw new Error("GraphQL step required")
      assertEquals(step.identity.authorization, "lin_api_fake", spec.id)
      assertEquals(step.identity.userAgent, "schpet-linear-cli/2.6.0", spec.id)
      assertEquals(step.effects, [], spec.id)
      if (step.operation.document.includes("GetTeamMembers")) {
        memberRequests++
        assertEquals(step.operation.variables?.first, 100, spec.id)
        assertEquals(
          typeof step.operation.variables?.teamKey,
          "string",
          spec.id,
        )
        assertEquals(
          typeof step.operation.variables?.includeDisabled,
          "boolean",
          spec.id,
        )
      }
    }
  }
  assertEquals(requests, 75)
  assertEquals(memberRequests, 41)
  const lines = await Promise.all(
    files.map(async (file) =>
      `${await sha256Hex(
        await Deno.readFile(join(root, file)),
      )}  rust/parity/runner/c010-frozen-cases/${file}\n`
    ),
  )
  assertEquals(await Deno.readTextFile(manifest), lines.join(""))
  assertEquals(
    await sha256Hex(new TextEncoder().encode(lines.join(""))),
    manifestSha256,
  )
})
