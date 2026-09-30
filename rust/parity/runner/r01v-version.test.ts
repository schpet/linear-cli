import { assertRejects } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { loadCases } from "./cases.ts"
import { SchemaError } from "./schema.ts"
const corpus = new URL("./cases/", import.meta.url).pathname
const CONTRACT = "rust-3.0.0-alpha.1"

Deno.test("R01V preflight rejects a stale v2 candidate after rebinding its hash", async () => {
  const dir = await Deno.makeTempDir({ prefix: "r01v-stale-" })
  try {
    const id = "root-version-short-default"
    const caseText = await Deno.readTextFile(join(corpus, `${id}.json`))
    const caseSpec = JSON.parse(caseText)
    const goldenPath = join(dir, "rust-goldens", CONTRACT, `${id}.json`)
    await Deno.mkdir(join(dir, "rust-goldens", CONTRACT), { recursive: true })
    const golden = JSON.parse(
      await Deno.readTextFile(
        join(corpus, "rust-goldens", CONTRACT, `${id}.json`),
      ),
    )
    golden.candidate.expected.stdout = caseSpec.expected.stdout
    const bytes = new TextEncoder().encode(
      JSON.stringify(golden, null, 2) + "\n",
    )
    await Deno.writeFile(goldenPath, bytes)
    caseSpec.deviation.sha256 = await sha256Hex(bytes)
    await Deno.writeTextFile(join(dir, `${id}.json`), JSON.stringify(caseSpec))
    await assertRejects(
      () => loadCases(dir, new Set(["linear"]), undefined, CONTRACT),
      SchemaError,
      "redundant unchanged candidate override",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})
