import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases, resolveCase } from "./cases.ts"
import { parseCase } from "./schema.ts"

const cases = new URL("./cases/", import.meta.url)
const frozen = new URL("./c038-frozen-cases/", import.meta.url)
const contract = "rust-3.0.0-alpha.1"
const token = "referenceModuleUrl"
const values = {
  home: "/home/test",
  configHome: "/config",
  cwd: "/cwd",
  cwdRoot: "/root",
  bin: "/bin",
  denoDir: "/deno",
  fixturePort: "1234",
  referenceModuleUrl: "file:///reference",
}

async function corpus(
  fn: (
    dir: string,
    rebind: (
      id: string,
      source: Record<string, unknown>,
      golden: Record<string, unknown>,
    ) => Promise<void>,
  ) => Promise<void>,
) {
  const dir = await Deno.makeTempDir({ prefix: "c038-substitution-" })
  const gd = join(dir, "rust-goldens", contract)
  await Deno.mkdir(gd, { recursive: true })
  const fixture = join(dir, "fixtures", "workspace-credential", "linear")
  await Deno.mkdir(fixture, { recursive: true })
  await Deno.copyFile(
    new URL(
      "./cases/fixtures/workspace-credential/linear/credentials.toml",
      import.meta.url,
    ),
    join(fixture, "credentials.toml"),
  )
  const rebind = async (
    id: string,
    source: Record<string, unknown>,
    golden: Record<string, unknown>,
  ) => {
    const raw = JSON.stringify(golden, null, 2) + "\n"
    await Deno.writeTextFile(join(gd, `${id}.json`), raw)
    source.deviation = {
      id: golden.deviationId,
      contract,
      sha256: await sha256Hex(new TextEncoder().encode(raw)),
    }
    await Deno.writeTextFile(join(dir, `${id}.json`), JSON.stringify(source))
  }
  try {
    await fn(dir, rebind)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

async function load(dir: string) {
  return await loadCases(
    dir,
    new Set(["linear initiative view"]),
    undefined,
    contract,
  )
}

Deno.test("C038 v1 candidate drops only source-only module URL in two real reviewed deltas", async () => {
  await corpus(async (dir, rebind) => {
    for (const id of ["c038-empty-id", "c038-slug-and-name-errors"]) {
      const source = JSON.parse(
        await Deno.readTextFile(new URL(`${id}.json`, frozen)),
      )
      const caseSpec = JSON.parse(
        await Deno.readTextFile(new URL(`${id}.json`, cases)),
      )
      const golden = JSON.parse(
        await Deno.readTextFile(
          new URL(`rust-goldens/${contract}/${id}.json`, cases),
        ),
      )
      assertEquals(caseSpec.argv, source.argv)
      assertEquals(caseSpec.graphql, source.graphql)
      await rebind(id, caseSpec, golden)
    }
    const loaded = await load(dir)
    assertEquals(loaded.length, 2)
    for (const item of loaded) {
      const candidate = candidateCaseView(item)
      assert(item.spec.substitutions.includes(token))
      assertEquals(
        candidate.spec.substitutions,
        item.spec.substitutions.filter((name) => name !== token),
      )
      const baseline = resolveCase(item.spec, values)
      assert(
        new TextDecoder().decode(baseline.expected.stderr).includes(
          "file:///reference",
        ),
      )
      const resolved = resolveCase(
        candidate.spec,
        values,
        "schpet-linear-cli/3.0.0-alpha.1",
      )
      assert(
        new TextDecoder().decode(resolved.expected.stderr).startsWith(
          item.spec.id === "c038-empty-id" ? "  error:" : "✗ Failed",
        ),
      )
      assertEquals(resolved.env.HOME, "/home/test")
      assertEquals(
        candidate.spec.graphql?.expectedRequests,
        item.spec.id === "c038-empty-id" ? 0 : 1,
      )
    }
  })
})

Deno.test("C038 adapter retains source token unless a reviewed candidate expected override removes it", async () => {
  await corpus(async (dir, rebind) => {
    const id = "c038-app-null-root"
    const source = JSON.parse(
      await Deno.readTextFile(new URL(`${id}.json`, cases)),
    )
    const golden = {
      formatVersion: 1,
      caseId: id,
      deviationId: "TEST-UA-ONLY",
      contract,
      approvedSurfaces: ["graphql-user-agent"],
      candidate: { graphqlUserAgent: "schpet-linear-cli/3.0.0-alpha.1" },
    }
    await rebind(id, source, golden)
    const item = (await load(dir))[0]
    assert(candidateCaseView(item).spec.substitutions.includes(token))
    source.expected.stdout = { utf8: "{{referenceModuleUrl}}/source\n" }
    source.expected.stderr = { utf8: "ordinary stderr\n" }
    await rebind(id, source, golden)
    const stdoutItem = (await load(dir))[0]
    assert(candidateCaseView(stdoutItem).spec.substitutions.includes(token))
    assert(
      new TextDecoder().decode(
        resolveCase(stdoutItem.spec, values).expected.stdout,
      ).includes("file:///reference"),
    )
  })
})

Deno.test("C038 adapter keeps source and golden placeholder refusals", async () => {
  await corpus(async (dir, rebind) => {
    const id = "c038-app-null-root"
    const source = JSON.parse(
      await Deno.readTextFile(new URL(`${id}.json`, cases)),
    )
    const golden = JSON.parse(
      await Deno.readTextFile(
        new URL(`rust-goldens/${contract}/${id}.json`, cases),
      ),
    )
    const broken = structuredClone(source)
    broken.expected.stderr = { utf8: "ordinary" }
    assertThrows(() => parseCase(broken))
    const argv = structuredClone(source)
    argv.argv = ["initiative", "view", "{{referenceModuleUrl}}"]
    assertThrows(() => parseCase(argv))
    await rebind(id, source, {
      ...golden,
      candidate: {
        ...golden.candidate,
        expected: {
          ...golden.candidate.expected,
          stderr: { utf8: "{{referenceModuleUrl}}" },
        },
      },
    })
    await assertRejects(
      () => load(dir),
      Error,
      "Rust golden cannot use referenceModuleUrl",
    )
  })
})
