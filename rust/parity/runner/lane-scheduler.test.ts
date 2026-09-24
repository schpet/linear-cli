import { assertEquals, assertNotEquals } from "@std/assert"
import * as v from "valibot"
import { LaneScheduler } from "./lane-scheduler.ts"
import { GraphQLFixtureSchema } from "./schema.ts"

function asset(id: string, path: string) {
  return {
    kind: "asset",
    id,
    method: "GET",
    path,
    requiredHeaders: {},
    forbiddenHeaders: [],
    body: { utf8: "" },
    response: { status: 200, headers: {}, body: { utf8: "ok" } },
  }
}

function groups() {
  return v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: 5,
    initialRecords: {},
    expectedRecords: {},
    groups: [
      {
        mode: "lanes",
        timeoutMs: 100,
        lanes: [
          { id: "a", steps: [asset("a1", "/a1"), asset("a2", "/a2")] },
          { id: "b", steps: [asset("b1", "/b1"), asset("b2", "/b2")] },
        ],
      },
      { mode: "ordered", steps: [asset("last", "/last")] },
    ],
  }).groups
}

Deno.test("first steps overlap, later lane steps interleave, next group waits", async () => {
  const issues: string[] = []
  const scheduler = new LaneScheduler(groups(), (reason) => issues.push(reason))
  const a1 = scheduler.claim((step) => step.id === "a1")
  assertNotEquals(a1, null)
  if (a1 == null) throw new Error("missing claim")
  let released = false
  a1.ready.then(() => {
    released = true
  })
  await Promise.resolve()
  assertEquals(released, false)
  assertEquals(scheduler.claim((step) => step.id === "a2"), null)
  assertEquals(await a1.ready, false)
  assertEquals(issues.length, 1)

  const working = new LaneScheduler(groups(), (reason) => issues.push(reason))
  const firstA = working.claim((step) => step.id === "a1")
  const firstB = working.claim((step) => step.id === "b1")
  if (firstA == null || firstB == null) {
    throw new Error("missing first lane claim")
  }
  assertEquals(await firstA.ready, true)
  assertEquals(await firstB.ready, true)
  working.complete(firstB)
  working.complete(firstA)
  for (const id of ["b2", "a2"]) {
    const claim = working.claim((step) => step.id === id)
    if (claim == null) throw new Error(`missing ${id}`)
    assertEquals(await claim.ready, true)
    working.complete(claim)
  }
  assertEquals(working.groupIndex, 1)
  const last = working.claim((step) => step.id === "last")
  if (last == null) throw new Error("missing second group")
  working.complete(last)
  assertEquals(working.consumed, 5)
  assertEquals(working.finished, true)
  working.stop()
})

Deno.test("mismatch, deadline and stop release every first-step waiter", async () => {
  const reasons: string[] = []
  const mismatch = new LaneScheduler(groups(), (reason) => reasons.push(reason))
  const parked = mismatch.claim((step) => step.id === "a1")
  if (parked == null) throw new Error("missing parked claim")
  assertEquals(mismatch.claim(() => false), null)
  assertEquals(await parked.ready, false)
  assertEquals(reasons[0], "request does not match any eligible interaction")

  const stopped = new LaneScheduler(groups(), (reason) => reasons.push(reason))
  const held = stopped.claim((step) => step.id === "a1")
  if (held == null) throw new Error("missing held claim")
  stopped.stop()
  assertEquals(await held.ready, false)

  const short = v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: 2,
    initialRecords: {},
    expectedRecords: {},
    groups: [{
      mode: "lanes",
      timeoutMs: 10,
      lanes: [{ id: "a", steps: [asset("a", "/a")] }, {
        id: "b",
        steps: [asset("b", "/b")],
      }],
    }],
  })
  const deadline = new LaneScheduler(
    short.groups,
    (reason) => reasons.push(reason),
  )
  const waiting = deadline.claim((step) => step.id === "a")
  if (waiting == null) throw new Error("missing deadline claim")
  assertEquals(await waiting.ready, false)
  assertEquals(reasons.includes("lane overlap deadline exceeded"), true)
})

Deno.test("overlap deadline stops after first-step barrier releases", async () => {
  const issues: string[] = []
  const scheduler = new LaneScheduler(groups(), (reason) => issues.push(reason))
  const firstA = scheduler.claim((step) => step.id === "a1")
  const firstB = scheduler.claim((step) => step.id === "b1")
  if (firstA == null || firstB == null) throw new Error("first steps missing")
  assertEquals(await firstA.ready, true)
  assertEquals(await firstB.ready, true)
  scheduler.complete(firstA)
  scheduler.complete(firstB)
  await new Promise((resolve) => setTimeout(resolve, 120))
  assertEquals(scheduler.failure, null)
  assertEquals(issues, [])
  const laterA = scheduler.claim((step) => step.id === "a2")
  const laterB = scheduler.claim((step) => step.id === "b2")
  if (laterA == null || laterB == null) throw new Error("later steps missing")
  scheduler.complete(laterA)
  scheduler.complete(laterB)
  assertEquals(scheduler.groupIndex, 1)
})

Deno.test("identical effect-free first steps assign first unstarted lane; ambiguous different lanes fail", async () => {
  const identical = v.parse(GraphQLFixtureSchema, {
    path: "/graphql",
    schemaSha256: "0".repeat(64),
    expectedRequests: 2,
    initialRecords: {},
    expectedRecords: {},
    groups: [{
      mode: "lanes",
      timeoutMs: 100,
      lanes: [{ id: "a", steps: [asset("a", "/same")] }, {
        id: "b",
        steps: [asset("b", "/same")],
      }],
    }],
  })
  const scheduler = new LaneScheduler(identical.groups, () => {})
  const first = scheduler.claim(() => true)
  const second = scheduler.claim(() => true)
  if (first == null || second == null) {
    throw new Error("missing identical claims")
  }
  assertEquals([first.laneIndex, second.laneIndex], [0, 1])
  assertEquals(await first.ready, true)
  scheduler.complete(first)
  scheduler.complete(second)

  const distinct = groups()
  const bad = new LaneScheduler(distinct, () => {})
  assertEquals(bad.claim(() => true), null)
  assertEquals(bad.failure, "request ambiguously matches non-identical lanes")
})
