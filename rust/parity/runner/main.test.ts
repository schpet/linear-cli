import { assertEquals, assertRejects, assertThrows } from "@std/assert"
import { join } from "@std/path"
import { loadCandidate, parseOptions } from "./main.ts"

const required = ["--reference", "/reference", "--reference-binary", "/binary"]

Deno.test("task separator is accepted only before runner flags", () => {
  assertEquals(parseOptions(["--", ...required]).reference, "/reference")
  assertThrows(
    () => parseOptions([...required, "--"]),
    Error,
    "only valid before",
  )
  assertThrows(
    () => parseOptions(["--", ...required, "--"]),
    Error,
    "only valid before",
  )
  assertThrows(
    () => parseOptions([...required, "--deno-dir", "/cache"]),
    Error,
    "internal namespace",
  )
  assertThrows(
    () => parseOptions([...required, "--staged-reused", "maybe"]),
    Error,
    "true or false",
  )
  assertThrows(
    () => parseOptions([...required, "--status-helper", "/helper"]),
    Error,
    "internal namespace",
  )
  assertThrows(
    () => parseOptions([...required, "--staged-reused", "false"]),
    Error,
    "internal namespace",
  )
  assertThrows(
    () => parseOptions([...required, "--inside-namespace"]),
    Error,
    "fresh PID namespace",
  )
})

Deno.test("a claimed candidate executable is checked before any case runs", async () => {
  const dir = await Deno.makeTempDir({ prefix: "linear-parity-candidate-" })
  try {
    const descriptor = join(dir, "candidate.json")
    await Deno.writeTextFile(
      descriptor,
      JSON.stringify({
        name: "missing",
        program: { kind: "executable", path: join(dir, "missing") },
        implementedRoutes: ["linear"],
      }),
    )
    const options = parseOptions([...required, "--candidate", descriptor])
    await assertRejects(
      () => loadCandidate(options, new Set(["linear"])),
      Error,
      "candidate executable is missing",
    )
  } finally {
    await Deno.remove(dir, { recursive: true })
  }
})

Deno.test("explicit baseline refresh flag is accepted", () => {
  assertEquals(
    parseOptions([...required, "--force-baseline"]).forceBaseline,
    true,
  )
  assertEquals(parseOptions(required).forceBaseline, false)
})
