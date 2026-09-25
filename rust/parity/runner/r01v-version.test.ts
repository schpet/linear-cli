import { assert, assertEquals, assertRejects } from "@std/assert"
import { join } from "@std/path"
import { readManifest } from "../verify.ts"
import { sha256Hex } from "./bytes.ts"
import { candidateCaseView, loadCases } from "./cases.ts"
import { SchemaError } from "./schema.ts"

const CONTRACT = "rust-3.0.0-alpha.1"
const V2 = "2.6.0"
const V3 = "3.0.0-alpha.1"
const ANSI = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*m`, "g")
const corpus = new URL("./cases", import.meta.url).pathname
const transport = new URL("./transport-cases", import.meta.url).pathname

function visibleLength(line: string): number {
  return line.replace(ANSI, "").length
}

function expectedVersionStdout(
  frozen: string,
): { text: string; padded: boolean } {
  assertEquals(frozen.split(V2).length, 2)
  if (!frozen.includes("Version:")) {
    return { text: frozen.replace(V2, V3), padded: false }
  }
  const lines = frozen.split("\n")
  assert(lines[1].includes("Usage:"))
  assert(lines[2].includes("Version:"))
  const usage = lines[1].trimEnd()
  const version = lines[2].trimEnd()
  const oldWidth = Math.max(visibleLength(usage), visibleLength(version))
  assertEquals(visibleLength(lines[1]), oldWidth)
  assertEquals(visibleLength(lines[2]), oldWidth)
  const nextVersion = version.replace(V2, V3)
  const nextWidth = Math.max(visibleLength(usage), visibleLength(nextVersion))
  const padding = nextWidth - visibleLength(usage)
  lines[1] = usage + " ".repeat(padding)
  lines[2] = nextVersion + " ".repeat(nextWidth - visibleLength(nextVersion))
  return { text: lines.join("\n"), padded: padding > 0 }
}

Deno.test("R01V binds exactly the frozen version stdout cases", async () => {
  const manifest = readManifest(
    JSON.parse(
      await Deno.readTextFile(new URL("../manifest.json", import.meta.url)),
    ),
  )
  const routes = new Set<string>(manifest.routes.map((route) => {
    if (typeof route.path !== "string") {
      throw new Error("manifest route without path")
    }
    return route.path
  }))
  const cases = await loadCases(corpus, routes, undefined, CONTRACT)
  assertEquals(cases.length, 301)
  const ids: string[] = []
  let header = 0
  let bare = 0
  let long = 0
  let padded = 0
  let graphql = 0
  let startup = 0
  let credentialStartup = 0
  let authList = 0
  for (const loaded of cases) {
    if (loaded.spec.id.startsWith("c002-")) {
      // C002 pins its own frozen bytes, deviations and goldens.
      authList++
      continue
    }
    if (loaded.spec.graphql != null) {
      graphql++
      assertEquals(
        loaded.spec.deviation?.id,
        loaded.spec.id === "c008-text-percent"
          ? "C008-SAFE-CONSOLE-PERCENT"
          : loaded.spec.id === "c008-raw-null-name"
          ? "C008-STRICT-TEAM-NAME"
          : loaded.spec.id === "c008-raw-extra-field"
          ? "C008-TYPED-JSON-FIELDS"
          : "R01H-GRAPHQL-UA",
      )
      assertEquals(
        loaded.golden?.spec.approvedSurfaces,
        loaded.spec.id === "c008-text-percent"
          ? ["stdout", "graphql-user-agent"]
          : loaded.spec.id === "c008-raw-null-name"
          ? ["exit", "stdout", "stderr", "graphql-user-agent"]
          : loaded.spec.id === "c008-raw-extra-field"
          ? ["stdout", "graphql-user-agent"]
          : ["graphql-user-agent"],
      )
      assertEquals(
        loaded.golden?.spec.candidate.graphqlUserAgent,
        `schpet-linear-cli/${V3}`,
      )
    }
    if (loaded.spec.id.startsWith("r02b3-")) {
      startup++
      assert(
        loaded.spec.deviation?.id === "R02B3-STARTUP-VALIDATION" ||
          loaded.spec.deviation?.id === "R02B3-WARNING-VERSION",
      )
    } else if (loaded.spec.id.startsWith("r02c2-")) {
      credentialStartup++
      assertEquals(loaded.spec.deviation?.id, "R02C2G-CREDENTIAL-STARTUP")
    } else if (loaded.spec.id === "c2-label-list-help") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-LABEL-LIST-HELP")
      assertEquals(loaded.golden?.spec.approvedSurfaces, ["stdout"])
    } else if (loaded.spec.id === "c2-workspace-missing-help") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-WORKSPACE-HELP-VALUE")
      assertEquals(loaded.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
      ])
    } else if (loaded.spec.id === "c2-bulk-help") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-BULK-HELP-PRECEDENCE")
      assertEquals(loaded.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
      ])
    } else if (loaded.spec.id === "c2-mine-sort-help-value") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-ENUM-HELP-VALUE")
      assertEquals(loaded.golden?.spec.approvedSurfaces, ["stdout", "stderr"])
    } else if (loaded.spec.id === "c2-workspace-delimiter-value") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-WORKSPACE-DELIMITER-VALUE")
      assertEquals(loaded.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
      ])
    } else if (loaded.spec.id === "c2-bulk-empty-tail") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-BULK-EMPTY-TAIL")
      assertEquals(loaded.golden?.spec.approvedSurfaces, ["stdout", "stderr"])
    } else if (loaded.spec.id === "c2-bulk-unknown-option") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-BULK-UNKNOWN-OPTION")
      assertEquals(loaded.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
      ])
    } else if (loaded.spec.id === "c2-empty-help-suffix") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-EMPTY-HELP-SUFFIX")
      assertEquals(loaded.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
      ])
    } else if (loaded.spec.id === "c2-empty-switch-suffix") {
      assertEquals(loaded.spec.deviation?.id, "R01C2-EMPTY-SWITCH-SUFFIX")
      assertEquals(loaded.golden?.spec.approvedSurfaces, [
        "exit",
        "stdout",
        "stderr",
      ])
    } else if (loaded.spec.id.startsWith("c009-")) {
      assert(
        loaded.spec.deviation == null ||
          [
            "C009-CLI-VERSION",
            "R02B3-STARTUP-VALIDATION",
            "R02C2G-CREDENTIAL-STARTUP",
          ].includes(loaded.spec.deviation.id),
      )
    } else if (loaded.spec.id.startsWith("c009g-")) {
      assert(
        loaded.spec.deviation == null ||
          loaded.spec.deviation.id === "R02B3-STARTUP-VALIDATION",
      )
    } else if (loaded.spec.id.startsWith("c008-")) {
      assert(
        loaded.spec.deviation == null ||
          loaded.spec.deviation.id === "R01H-GRAPHQL-UA" ||
          loaded.spec.deviation.id === "C008-SAFE-CONSOLE-PERCENT" ||
          loaded.spec.deviation.id === "C008-STRICT-TEAM-NAME" ||
          loaded.spec.deviation.id === "C008-TYPED-JSON-FIELDS",
      )
    } else if (loaded.spec.id.startsWith("c086-")) {
      const c086Deviations = new Set<string>([
        "C086-CLI-VERSION",
        "C086-STRICT-COMMAND-NAME",
        "C086-CREDENTIAL-DIAGNOSTIC",
        "C086-SCRIPT-AND-STARTUP-WARNING",
        "C086-COMPLETE-ERROR",
        "C086-STATIC-SHELL-SCRIPT",
        "C086-CLAP-HELP",
      ])
      assert(
        loaded.spec.deviation == null ||
          c086Deviations.has(loaded.spec.deviation.id),
      )
    } else if (
      loaded.spec.deviation?.id !== "R01V-CLI-VERSION" &&
      loaded.spec.deviation?.id !== "R01H-GRAPHQL-UA"
    ) {
      assertEquals(loaded.spec.deviation, null)
    }
    // R01V's exact pinned set stays closed; later work items bind their own
    // case-specific v3 deviations without rewriting this evidence.
    if (loaded.spec.deviation?.id !== "R01V-CLI-VERSION") continue
    const frozen = loaded.spec.expected.stdout
    const stdout = "utf8" in frozen ? frozen.utf8 : ""
    assert(stdout.includes(V2))
    ids.push(loaded.spec.id)
    assertEquals(loaded.spec.deviation?.id, "R01V-CLI-VERSION")
    assertEquals(loaded.spec.deviation?.contract, CONTRACT)
    assertEquals(loaded.golden?.spec.approvedSurfaces, ["stdout"])
    assertEquals(loaded.golden?.spec.candidate.argv, undefined)
    const expected = loaded.golden?.spec.candidate.expected
    assert(expected != null)
    assertEquals(expected.exit, loaded.spec.expected.exit)
    assertEquals(expected.stderr, loaded.spec.expected.stderr)
    assertEquals(expected.fileEffects, loaded.spec.expected.fileEffects)
    const transformed = expectedVersionStdout(stdout)
    assertEquals(expected.stdout, { utf8: transformed.text }, loaded.spec.id)
    assertEquals(candidateCaseView(loaded).spec.expected, expected)
    if (stdout.includes("Version:")) {
      header++
      if (transformed.padded) padded++
    } else if (stdout.trim() === V2) {
      bare++
    } else {
      long++
    }
  }
  ids.sort()
  assertEquals([ids.length, header, bare, long, padded, graphql], [
    107,
    101,
    3,
    3,
    43,
    29,
  ])
  assertEquals(startup, 5)
  assertEquals(credentialStartup, 10)
  assertEquals(authList, 26)
  assertEquals(
    await sha256Hex(new TextEncoder().encode(ids.join("\n") + "\n")),
    "34bb064d9fe1d227ea4767737debb942366ca33cf58505a25d4b0307d4b452c6",
  )

  const transportCases = await loadCases(transport, routes, undefined, CONTRACT)
  assertEquals(transportCases.length, 6)
  assertEquals(
    transportCases.filter((entry) => entry.spec.graphql != null).length,
    5,
  )
  for (const entry of transportCases) {
    const source = await Deno.readTextFile(
      join(transport, `${entry.spec.id}.json`),
    )
    if (entry.spec.graphql == null) {
      assertEquals(entry.spec.deviation, null)
      continue
    }
    assertEquals(entry.spec.deviation?.id, "R01H-GRAPHQL-UA")
    assertEquals(entry.golden?.spec.approvedSurfaces, ["graphql-user-agent"])
    assertEquals(
      entry.golden?.spec.candidate.graphqlUserAgent,
      `schpet-linear-cli/${V3}`,
    )
    assert(source.includes(`schpet-linear-cli/${V2}`))
  }
})

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
