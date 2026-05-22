import { assertEquals, assertRejects } from "@std/assert"
import {
  PROJECT_DESCRIPTION_MAX_LENGTH,
  resolveProjectDescription,
} from "../../../src/commands/project/project-description.ts"
import { NotFoundError, ValidationError } from "../../../src/utils/errors.ts"

Deno.test("resolveProjectDescription - returns undefined when neither flag set", async () => {
  const result = await resolveProjectDescription(undefined, undefined)
  assertEquals(result, undefined)
})

Deno.test("resolveProjectDescription - returns inline description", async () => {
  const result = await resolveProjectDescription("hello", undefined)
  assertEquals(result, "hello")
})

Deno.test("resolveProjectDescription - reads file content", async () => {
  const tmp = await Deno.makeTempFile({ suffix: ".md" })
  try {
    await Deno.writeTextFile(tmp, "from-file")
    const result = await resolveProjectDescription(undefined, tmp)
    assertEquals(result, "from-file")
  } finally {
    await Deno.remove(tmp)
  }
})

Deno.test("resolveProjectDescription - rejects passing both flags", async () => {
  await assertRejects(
    () => resolveProjectDescription("inline", "/tmp/some.md"),
    ValidationError,
    "Cannot use --description and --description-file together",
  )
})

Deno.test("resolveProjectDescription - rejects inline description over the cap", async () => {
  const tooLong = "x".repeat(PROJECT_DESCRIPTION_MAX_LENGTH + 1)
  await assertRejects(
    () => resolveProjectDescription(tooLong, undefined),
    ValidationError,
    `Project description is ${tooLong.length} characters`,
  )
})

Deno.test("resolveProjectDescription - rejects file content over the cap", async () => {
  const tmp = await Deno.makeTempFile({ suffix: ".md" })
  try {
    await Deno.writeTextFile(
      tmp,
      "y".repeat(PROJECT_DESCRIPTION_MAX_LENGTH + 50),
    )
    await assertRejects(
      () => resolveProjectDescription(undefined, tmp),
      ValidationError,
      "exceeds the 255-character limit",
    )
  } finally {
    await Deno.remove(tmp)
  }
})

Deno.test("resolveProjectDescription - accepts description exactly at the cap", async () => {
  const exact = "z".repeat(PROJECT_DESCRIPTION_MAX_LENGTH)
  const result = await resolveProjectDescription(exact, undefined)
  assertEquals(result, exact)
})

Deno.test("resolveProjectDescription - throws NotFoundError for missing file", async () => {
  await assertRejects(
    () => resolveProjectDescription(undefined, "/tmp/does-not-exist-xyz.md"),
    NotFoundError,
    "File not found",
  )
})
