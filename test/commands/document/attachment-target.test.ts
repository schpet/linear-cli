import { assertEquals, assertRejects, assertThrows } from "@std/assert"
import {
  parseDocumentTargetOptions,
  resolveDocumentTarget,
  toDocumentTargetFilter,
  toDocumentTargetInput,
} from "../../../src/commands/document/attachment-target.ts"
import { ValidationError } from "../../../src/utils/errors.ts"

Deno.test("parseDocumentTargetOptions - zero targets", () => {
  // at-most-one: fine, no selector
  assertEquals(parseDocumentTargetOptions({}, "at-most-one"), undefined)

  // exactly-one: required
  assertThrows(
    () => parseDocumentTargetOptions({}, "exactly-one"),
    ValidationError,
    "A document attachment target is required",
  )
})

Deno.test("parseDocumentTargetOptions - single targets", () => {
  assertEquals(
    parseDocumentTargetOptions({ project: "roadmap" }, "exactly-one"),
    { kind: "project", project: "roadmap" },
  )
  assertEquals(
    parseDocumentTargetOptions({ team: "ENG" }, "exactly-one"),
    { kind: "team", team: "ENG" },
  )
  assertEquals(
    parseDocumentTargetOptions({ release: "1.0" }, "at-most-one"),
    { kind: "release", release: "1.0" },
  )
})

Deno.test("parseDocumentTargetOptions - team plus cycle is one cycle target", () => {
  assertEquals(
    parseDocumentTargetOptions({ team: "ENG", cycle: "next" }, "exactly-one"),
    { kind: "cycle", cycle: "next", team: "ENG" },
  )
})

Deno.test("parseDocumentTargetOptions - multiple targets rejected with flag names", () => {
  assertThrows(
    () =>
      parseDocumentTargetOptions(
        { project: "roadmap", team: "ENG" },
        "at-most-one",
      ),
    ValidationError,
    "--project, --team",
  )

  // team is consumed by cycle, so project + cycle are the two named targets
  assertThrows(
    () =>
      parseDocumentTargetOptions(
        { project: "roadmap", team: "ENG", cycle: "next" },
        "exactly-one",
      ),
    ValidationError,
    "--project, --cycle",
  )
})

Deno.test("resolveDocumentTarget - cycle without any team fails before network", async () => {
  // No explicit team and no configured default: must fail with guidance
  // (before this even needs a server, so no mock is running). An empty team
  // id is falsy, so getTeamKey() resolves to undefined even though the
  // repo's .linear.toml sets one — this exercises the no-team branch.
  const previous = Deno.env.get("LINEAR_TEAM_ID")
  Deno.env.set("LINEAR_TEAM_ID", "")
  try {
    await assertRejects(
      () => resolveDocumentTarget({ kind: "cycle", cycle: "next" }),
      ValidationError,
      "--cycle requires a team",
    )
  } finally {
    if (previous != null) {
      Deno.env.set("LINEAR_TEAM_ID", previous)
    } else {
      Deno.env.delete("LINEAR_TEAM_ID")
    }
  }
})

Deno.test("toDocumentTargetInput - maps each kind to its single id field", () => {
  assertEquals(toDocumentTargetInput({ kind: "project", id: "p1" }), {
    projectId: "p1",
  })
  assertEquals(toDocumentTargetInput({ kind: "issue", id: "i1" }), {
    issueId: "i1",
  })
  assertEquals(toDocumentTargetInput({ kind: "initiative", id: "n1" }), {
    initiativeId: "n1",
  })
  assertEquals(toDocumentTargetInput({ kind: "team", id: "t1" }), {
    teamId: "t1",
  })
  assertEquals(toDocumentTargetInput({ kind: "cycle", id: "c1" }), {
    cycleId: "c1",
  })
  assertEquals(toDocumentTargetInput({ kind: "release", id: "r1" }), {
    releaseId: "r1",
  })
})

Deno.test("toDocumentTargetFilter - maps each kind to its relation id filter", () => {
  assertEquals(toDocumentTargetFilter({ kind: "team", id: "t1" }), {
    team: { id: { eq: "t1" } },
  })
  assertEquals(toDocumentTargetFilter({ kind: "cycle", id: "c1" }), {
    cycle: { id: { eq: "c1" } },
  })
  assertEquals(toDocumentTargetFilter({ kind: "release", id: "r1" }), {
    release: { id: { eq: "r1" } },
  })
})
