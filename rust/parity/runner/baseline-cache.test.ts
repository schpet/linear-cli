import {
  assert,
  assertEquals,
  assertNotEquals,
  assertRejects,
} from "@std/assert"
import { join } from "@std/path"
import {
  BaselineCache,
  baselineIdentity,
  fixtureDigest,
  harnessDigest,
} from "./baseline-cache.ts"
import { sha256Hex } from "./bytes.ts"
import type { LoadedCase } from "./cases.ts"
import type { Program } from "./program.ts"
import type { CaseRun, RunContext } from "./run.ts"
import { parseCase, RUST_USER_AGENT } from "./schema.ts"
import { validCase } from "./test-fixtures.ts"

async function withCase(
  fn: (dir: string, loaded: LoadedCase) => Promise<void>,
) {
  const dir = await Deno.makeTempDir()
  try {
    const file = join(dir, "case.json")
    const spec = parseCase(validCase())
    await Deno.writeTextFile(file, JSON.stringify(spec))
    await fn(dir, { file, spec, fixtureDir: null, configFixtureDir: null })
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}
async function proof(): Promise<CaseRun> {
  const stdout = new Uint8Array([0, 255, 65])
  const stderr = new TextEncoder().encode("original stderr\n")
  return {
    program: "real source",
    mismatches: [],
    fileEffects: [{
      path: "result",
      kind: "file",
      change: "created",
      sha256: "a".repeat(64),
    }],
    fixture: {
      requests: 1,
      unexpected: 0,
      authorizationMatched: [true],
      userAgents: ["frozen"],
    },
    raw: { stdout, stderr },
    observation: {
      targetExit: { code: 0 },
      outerExit: { code: 0 },
      targetStatus: { helperPid: 2, targetPid: 3 },
      stdoutClosure: {
        mode: "drain",
        count: 0,
        bytesRelayed: 0,
        closure: "none",
      },
      stdoutBytes: stdout.length,
      stdoutSha256: await sha256Hex(stdout),
      stderrBytes: stderr.length,
      stderrSha256: await sha256Hex(stderr),
      truncated: false,
      timedOut: false,
      durationMs: 123,
    },
  }
}
Deno.test("cache preserves exact complete proof bytes and reports historical duration; force executes", async () => {
  await withCase(async (dir, loaded) => {
    let executions = 0
    const run = await proof()
    const execute = () => {
      executions++
      return Promise.resolve(run)
    }
    const cache = new BaselineCache(join(dir, "cache"), "source identity")
    const cold = await cache.run(loaded, {}, execute)
    const warm = await cache.run(loaded, {}, execute)
    assertEquals(warm.run, cold.run)
    assertEquals(executions, 1)
    assertEquals(warm.evidence.origin, "cache")
    assertEquals(cache.metrics.hits, 1)
    assertEquals(cache.metrics.writes, 1)
    assertEquals(cache.metrics.historicalCachedDurationMs, 123)
    const forced = new BaselineCache(cache.directory, cache.identity, true)
    assertEquals(
      (await forced.run(loaded, {}, execute)).evidence.cache,
      "refresh",
    )
    assertEquals(executions, 2)
  })
})
Deno.test("all pinned identity dimensions, file/spec/limits/fixture content/mode change the key", async () => {
  await withCase(async (dir, loaded) => {
    const base = {
      reference: "binary",
      source: "source",
      lock: "lock",
      schema: "schema",
      runtime: "runtime",
      harness: "harness",
      program: "sourceProgram",
      settings: "settings",
    }
    const identity = JSON.stringify(base)
    const cache = new BaselineCache(dir, identity)
    const key = await cache.key(loaded, {})
    for (const dimension of Object.keys(base)) {
      assertNotEquals(
        await new BaselineCache(
          dir,
          JSON.stringify({ ...base, [dimension]: "changed" }),
        ).key(loaded, {}),
        key,
      )
    }
    assertNotEquals(
      await cache.key({
        ...loaded,
        spec: { ...loaded.spec, timeoutMs: loaded.spec.timeoutMs + 1 },
      }, {}),
      key,
    )
    assertNotEquals(await cache.key(loaded, { limits: { timeoutMs: 1 } }), key)
    assertNotEquals(
      await cache.key({ ...loaded, runtimeUserAgent: RUST_USER_AGENT }, {}),
      key,
    )
    await Deno.writeTextFile(loaded.file, JSON.stringify(loaded.spec) + "\n")
    assertNotEquals(await cache.key(loaded, {}), key)
    const fixture = join(dir, "fixture")
    await Deno.mkdir(fixture)
    const asset = join(fixture, "asset")
    await Deno.writeTextFile(asset, "one")
    const fixtureCase = { ...loaded, fixtureDir: fixture }
    const before = await cache.key(fixtureCase, {})
    await Deno.writeTextFile(asset, "two")
    assertNotEquals(await cache.key(fixtureCase, {}), before)
    const modesBefore = await fixtureDigest(fixture)
    await Deno.chmod(asset, 0o700)
    assertNotEquals(await fixtureDigest(fixture), modesBefore)
    assertNotEquals(
      await cache.key({ ...loaded, configFixtureDir: fixture }, {}),
      await cache.key(loaded, {}),
    )
    assertEquals(
      await cache.key({ ...loaded, golden: null }, {}),
      await cache.key(loaded, {}),
    )
    assert(await harnessDigest() != null)
  })
})
Deno.test("malformed/schema/integrity/raw-byte/stale records fail explicitly; forced refresh repairs", async () => {
  await withCase(async (dir, loaded) => {
    const cache = new BaselineCache(dir, "identity")
    const execute = () => proof()
    await cache.run(loaded, {}, execute)
    const path = join(dir, `${await cache.key(loaded, {})}.json`)
    const original = await Deno.readTextFile(path)
    const parsed: unknown = JSON.parse(original)
    assert(typeof parsed === "object" && parsed != null)
    const corruptions = [
      "{",
      JSON.stringify({ ...parsed, extra: true }),
      original.replace('"version":1', '"version":2'),
      original.replace('"identity":"identity"', '"identity":"stale"'),
      original.replace(
        /"recordedAt":"[^"]+"/,
        '"recordedAt":"2000-01-01T00:00:00.000Z"',
      ),
      original.replace("original stderr", "tampered stderr"),
    ]
    for (const corruption of corruptions) {
      await Deno.writeTextFile(path, corruption)
      await assertRejects(
        () => cache.run(loaded, {}, execute),
        Error,
        "invalid baseline cache record",
      )
    }
    await new BaselineCache(dir, "identity", true).run(loaded, {}, execute)
    assertEquals(
      (await cache.run(loaded, {}, execute)).evidence.origin,
      "cache",
    )
    // Recompute envelope integrity while lying about raw bytes: stream digests
    // remain an independent check, even with an otherwise valid schema.
    assert(
      "run" in parsed && typeof parsed.run === "object" && parsed.run != null,
    )
    const changed = {
      ...parsed.run,
      raw: { stdout: { utf8: "bad" }, stderr: { utf8: "original stderr\n" } },
    }
    const recordedAt = new Date().toISOString()
    const proofSha256 = await sha256Hex(
      new TextEncoder().encode(JSON.stringify({ recordedAt, run: changed })),
    )
    await Deno.writeTextFile(
      path,
      JSON.stringify({
        version: 1,
        key: await cache.key(loaded, {}),
        identity: "identity",
        recordedAt,
        proofSha256,
        run: changed,
      }),
    )
    await assertRejects(
      () => cache.run(loaded, {}, execute),
      Error,
      "stdout bytes mismatch",
    )
  })
})
Deno.test("drift, incomplete observations and execution errors never become reusable passes", async () => {
  await withCase(async (dir, loaded) => {
    const variants: CaseRun[] = []
    const successful = await proof()
    variants.push({
      ...successful,
      mismatches: [{ surface: "stdout", detail: "wrong" }],
    })
    variants.push({
      ...successful,
      observation: { ...successful.observation, timedOut: true },
    })
    variants.push({
      ...successful,
      observation: { ...successful.observation, truncated: true },
    })
    variants.push({
      ...successful,
      observation: { ...successful.observation, targetExit: null },
    })
    variants.push({
      ...successful,
      observation: { ...successful.observation, targetStatus: null },
    })
    for (let i = 0; i < variants.length; i++) {
      const cache = new BaselineCache(join(dir, String(i)), "identity")
      let calls = 0
      const execute = () => {
        calls++
        return Promise.resolve(variants[i])
      }
      await cache.run(loaded, {}, execute)
      await cache.run(loaded, {}, execute)
      assertEquals(calls, 2)
      assertEquals(cache.metrics.writes, 0)
    }
    const cache = new BaselineCache(join(dir, "error"), "identity")
    await assertRejects(
      () =>
        cache.run(loaded, {}, () => {
          throw new Error("execution failed")
        }),
      Error,
      "execution failed",
    )
    assertEquals(cache.metrics.writes, 0)
    await cache.run(loaded, {}, () => Promise.resolve(successful))
    await assertRejects(
      () =>
        new BaselineCache(cache.directory, cache.identity, true).run(
          loaded,
          {},
          () => {
            throw new Error("forced error")
          },
        ),
      Error,
      "forced error",
    )
    assertEquals(
      (await cache.run(loaded, {}, () => Promise.resolve(successful))).evidence
        .cache,
      "miss",
    )
    await new BaselineCache(cache.directory, cache.identity, true).run(
      loaded,
      {},
      () => Promise.resolve(variants[0]),
    )
    assertEquals(
      (await cache.run(loaded, {}, () => Promise.resolve(successful))).evidence
        .cache,
      "miss",
    )
  })
})

Deno.test("actual identity collection binds pinned source/reference/lock/schema/runtime and runner code without manifest/golden churn", async () => {
  await withCase(async (dir, _loaded) => {
    const workspace = join(dir, "source")
    await Deno.mkdir(join(workspace, "src"), { recursive: true })
    await Deno.writeTextFile(join(workspace, "src", "main.ts"), "source")
    await Deno.writeTextFile(join(workspace, "deno.json"), "{}")
    const binary = join(dir, "runtime")
    await Deno.writeTextFile(binary, "runtime-one")
    const pinned = {
      referenceRevision: "revision",
      binarySha256: "reference",
      lockSha256: "lock",
      schemaSha256: "schema",
      denoVersion: "version",
    }
    const ctx: RunContext = {
      denoDir: "staged",
      referenceBinary: "reference",
      sandboxParent: dir,
      confinement: {
        bwrap: binary,
        version: "bwrap",
        systemLinks: [],
        systemBinds: [],
        sharedReadOnly: [],
        statusHelper: {
          path: "helper",
          sourcePath: "source",
          sourceSha256: "sha",
          std: "c11",
          flags: [],
          binarySha256: "helper-sha",
          compiler: {
            path: "gcc",
            version: "gcc",
            sha256: "gcc-sha",
            target: "target",
          },
        },
      },
    }
    const sourceProgram: Program = {
      kind: "interpreted-reference",
      workspace,
      deno: binary,
    }
    const before = await baselineIdentity(pinned, sourceProgram, ctx, "stage")
    for (const field of Object.keys(pinned)) {
      assertNotEquals(
        await baselineIdentity(
          { ...pinned, [field]: "changed" },
          sourceProgram,
          ctx,
          "stage",
        ),
        before,
      )
    }
    assertNotEquals(
      await baselineIdentity(pinned, sourceProgram, ctx, "changed-stage"),
      before,
    )
    await Deno.writeTextFile(
      join(workspace, "src", "main.ts"),
      "changed-source",
    )
    assertNotEquals(
      await baselineIdentity(pinned, sourceProgram, ctx, "stage"),
      before,
    )
    await Deno.writeTextFile(join(workspace, "src", "main.ts"), "source")
    await Deno.writeTextFile(binary, "runtime-two")
    assertNotEquals(
      await baselineIdentity(pinned, sourceProgram, ctx, "stage"),
      before,
    )
    const parity = join(dir, "parity")
    const runner = join(parity, "runner")
    await Deno.mkdir(join(runner, "helpers"), { recursive: true })
    await Deno.mkdir(join(runner, "certs"))
    for (
      const name of ["deno.json", "deno.lock", "verify.ts", "source-map.ts"]
    ) await Deno.writeTextFile(join(parity, name), name)
    const comparator = join(runner, "compare.ts")
    await Deno.writeTextFile(comparator, "compare-one")
    const harness = await harnessDigest(runner)
    await Deno.writeTextFile(
      join(parity, "manifest.json"),
      "administrative status change",
    )
    await Deno.mkdir(join(runner, "cases", "rust-goldens"), { recursive: true })
    await Deno.writeTextFile(
      join(runner, "cases", "unrelated.json"),
      "new case",
    )
    await Deno.writeTextFile(
      join(runner, "cases", "rust-goldens", "new.json"),
      "new golden",
    )
    await Deno.writeTextFile(join(runner, "helpers", "new.test.ts"), "new test")
    assertEquals(await harnessDigest(runner), harness)
    await Deno.writeTextFile(comparator, "compare-two")
    assertNotEquals(await harnessDigest(runner), harness)
  })
})
