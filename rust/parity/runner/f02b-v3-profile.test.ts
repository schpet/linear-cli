import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { decodeByteValue } from "./bytes.ts"
import { loadCases, resolveCase } from "./cases.ts"
import { CASES_DIR, checkCaseTable, TABLE } from "./f02b-fixed-host-driver.ts"
import {
  assertExactProjection,
  changedPaths,
  loadV3ProbeCases,
  PROFILE_ID,
  projectCase,
  RAW_PINS,
} from "./f02b-v3-profile.ts"
import { matchAssetRequest } from "./http-assets.ts"
import { FROZEN_USER_AGENT, RUST_USER_AGENT } from "./schema.ts"

const routes = new Set(["linear document view"])
const dummy = {
  home: "h",
  configHome: "c",
  cwd: "w",
  cwdRoot: "r",
  bin: "b",
  denoDir: "d",
  fixturePort: "0",
  referenceModuleUrl: "file:///reference",
}

async function temporaryCorpus(
  action: (dir: string) => Promise<void>,
): Promise<void> {
  const dir = await Deno.makeTempDir()
  try {
    for (const name of Object.keys(RAW_PINS)) {
      await Deno.copyFile(join(CASES_DIR, name), join(dir, name))
    }
    await action(dir)
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
}

Deno.test("v3 profile pins all 14 raw files and projects only asset headers", async () => {
  const frozen = await loadCases(CASES_DIR, routes)
  const before = JSON.stringify(frozen)
  const profile = await loadV3ProbeCases(CASES_DIR, routes)
  assertEquals(PROFILE_ID, "f02b-fixed-host-rust-3.0.0-alpha.1")
  assertEquals(profile.cases.length, 14)
  checkCaseTable(profile.cases)
  assertEquals(profile.projectedSha256.length, 64)
  assertEquals(JSON.stringify(frozen), before)
  for (const item of profile.cases) {
    const source = frozen.find((caseItem) => caseItem.spec.id === item.spec.id)
    if (source == null) throw new Error("source case missing")
    const name = `${item.spec.id}.json`
    const pin = Object.entries(RAW_PINS).find(([entry]) => entry === name)?.[1]
    if (pin == null) throw new Error("raw pin missing")
    assertEquals(changedPaths(source.spec, item.spec).length, pin.asset)
    assertEquals(item.runtimeUserAgent, RUST_USER_AGENT)
    const resolved = resolveCase(item.spec, dummy, item.runtimeUserAgent)
    for (const group of resolved.graphql?.groups ?? []) {
      if (group.mode !== "ordered") throw new Error("unexpected lanes mode")
      for (const step of group.steps) {
        if (step.kind === "graphql") {
          assertEquals(step.identity.userAgent, RUST_USER_AGENT)
        }
        if (step.kind === "asset") {
          assertEquals(step.requiredHeaders["User-Agent"], RUST_USER_AGENT)
        }
      }
    }
    for (const group of source.spec.graphql?.groups ?? []) {
      if (group.mode !== "ordered") throw new Error("unexpected lanes mode")
      for (const step of group.steps) {
        if (step.kind === "graphql") {
          assertEquals(step.identity.userAgent, FROZEN_USER_AGENT)
        }
        if (step.kind === "asset") {
          assertEquals(step.requiredHeaders["User-Agent"], FROZEN_USER_AGENT)
        }
      }
    }
  }
})

Deno.test("raw profile rejects a stale hash, missing case, and extra case", async () => {
  await temporaryCorpus(async (dir) => {
    const file = join(dir, "f02b-fixed-host-both.json")
    await Deno.writeTextFile(file, (await Deno.readTextFile(file)) + " ")
    await assertRejects(
      () => loadV3ProbeCases(dir, routes),
      Error,
      "SHA-256 differs",
    )
  })
  await temporaryCorpus(async (dir) => {
    await Deno.remove(join(dir, "f02b-fixed-host-both.json"))
    await assertRejects(
      () => loadV3ProbeCases(dir, routes),
      Error,
      "filename set",
    )
  })
  await temporaryCorpus(async (dir) => {
    await Deno.writeTextFile(join(dir, "unexpected.json"), "{}")
    await assertRejects(
      () => loadV3ProbeCases(dir, routes),
      Error,
      "filename set",
    )
  })
})

Deno.test("projection rejects count, source identity, and accidental changes", async () => {
  const [source] = await loadCases(CASES_DIR, routes, "f02b-fixed-host-both")
  const pin = RAW_PINS["f02b-fixed-host-both.json"]
  assertThrows(
    () => projectCase(source, { ...pin, asset: 1 }),
    Error,
    "target counts",
  )
  const sourceBytes = JSON.stringify(source)
  projectCase(source, pin)
  assertEquals(JSON.stringify(source), sourceBytes)
  const badSource = structuredClone(source)
  const first = badSource.spec.graphql?.groups[0]
  if (first?.mode !== "ordered" || first.steps[0].kind !== "graphql") {
    throw new Error("bad fixture")
  }
  Object.assign(first.steps[0].identity, { userAgent: RUST_USER_AGENT })
  assertThrows(() => projectCase(badSource, pin), Error, "wrong source GraphQL")
  const badAssetSource = structuredClone(source)
  const assetGroup = badAssetSource.spec.graphql?.groups[0]
  if (assetGroup?.mode !== "ordered" || assetGroup.steps[1].kind !== "asset") {
    throw new Error("bad fixture")
  }
  assetGroup.steps[1].requiredHeaders["User-Agent"] = RUST_USER_AGENT
  assertThrows(
    () => projectCase(badAssetSource, pin),
    Error,
    "wrong source asset",
  )
  const duplicateHeaderSource = structuredClone(source)
  const duplicateGroup = duplicateHeaderSource.spec.graphql?.groups[0]
  if (
    duplicateGroup?.mode !== "ordered" ||
    duplicateGroup.steps[1].kind !== "asset"
  ) {
    throw new Error("bad fixture")
  }
  duplicateGroup.steps[1].requiredHeaders["user-agent"] = FROZEN_USER_AGENT
  assertThrows(
    () => projectCase(duplicateHeaderSource, pin),
    Error,
    "wrong source asset",
  )
  const lanesSource = structuredClone(source)
  const lanesGroup = lanesSource.spec.graphql?.groups[0]
  if (lanesGroup == null) throw new Error("bad fixture")
  Object.assign(lanesGroup, { mode: "lanes", lanes: [] })
  assertThrows(() => projectCase(lanesSource, pin), Error, "lanes mode")
  const projected = projectCase(source, pin)
  const expected = changedPaths(source.spec, projected.spec)
  const group = projected.spec.graphql?.groups[0]
  if (group?.mode !== "ordered") throw new Error("bad fixture")
  const asset = group.steps[1]
  if (asset.kind !== "asset") throw new Error("bad fixture")
  asset.requiredHeaders["User-Agent"] = FROZEN_USER_AGENT
  assertThrows(
    () => assertExactProjection(source.spec, projected.spec, expected),
    Error,
    "changed paths",
  )
  asset.requiredHeaders["User-Agent"] = RUST_USER_AGENT
  asset.requiredHeaders.Authorization = "lin_api_fake_wrong"
  assertThrows(
    () => assertExactProjection(source.spec, projected.spec, expected),
    Error,
    "changed paths",
  )
  delete asset.requiredHeaders.Authorization
  asset.response.status = 299
  assertThrows(
    () => assertExactProjection(source.spec, projected.spec, expected),
    Error,
    "changed paths",
  )
  asset.response.status = 200
  const graphql = group.steps[0]
  if (graphql.kind !== "graphql") throw new Error("bad fixture")
  graphql.effects.push({ kind: "delete", record: "x", before: { value: null } })
  assertThrows(
    () => assertExactProjection(source.spec, projected.spec, expected),
    Error,
    "changed paths",
  )
  graphql.effects.pop()
  Object.assign(graphql.identity, { userAgent: RUST_USER_AGENT })
  assertThrows(
    () => assertExactProjection(source.spec, projected.spec, expected),
    Error,
    "changed paths",
  )
})

Deno.test("projection cannot carry a zero-request fixture through any pin", async () => {
  const [source] = await loadCases(CASES_DIR, routes, "f02b-fixed-host-both")
  const graphql = source.spec.graphql
  assert(graphql != null)
  const zero = structuredClone(source)
  zero.spec.graphql = { ...graphql, expectedRequests: 0, groups: [] }
  for (const [name, pin] of Object.entries(RAW_PINS)) {
    assert(pin.graphql + pin.asset > 0, `${name} pins no interaction`)
    assertThrows(() => projectCase(zero, pin), Error, "target counts 0/0")
  }
})

Deno.test("asset matcher accepts v3 with correct auth and rejects v2 exactly", async () => {
  const profile = await loadV3ProbeCases(CASES_DIR, routes)
  const item = profile.cases.find((candidate) =>
    candidate.spec.id === "f02b-fixed-host-both"
  )
  const group = item?.spec.graphql?.groups[0]
  if (group?.mode !== "ordered") throw new Error("bad fixture")
  const step = group.steps[1]
  if (step.kind !== "asset") throw new Error("bad fixture")
  const url = `https://${step.fixedHost}${step.path}`
  const headers = { ...step.requiredHeaders }
  const body = decodeByteValue(step.body)
  assertEquals(
    matchAssetRequest(step, new Request(url, { headers }), body),
    null,
  )
  headers["User-Agent"] = FROZEN_USER_AGENT
  assertEquals(
    matchAssetRequest(step, new Request(url, { headers }), body),
    "asset required header User-Agent differs",
  )
  assert(item != null)
  assert(TABLE.some((entry) => entry.id === item.spec.id))
})
