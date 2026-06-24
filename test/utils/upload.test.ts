import { assertEquals, assertThrows } from "@std/assert"
import { resolveMakePublic } from "../../src/utils/upload.ts"
import { ValidationError } from "../../src/utils/errors.ts"

Deno.test("resolveMakePublic - defaults to private when not requested", () => {
  assertEquals(resolveMakePublic("image/png"), false)
  assertEquals(resolveMakePublic("image/png", undefined), false)
})

Deno.test("resolveMakePublic - defaults to private for non-image types", () => {
  assertEquals(resolveMakePublic("application/pdf"), false)
})

Deno.test("resolveMakePublic - allows public for raster images when requested", () => {
  for (
    const type of [
      "image/png",
      "image/jpeg",
      "image/gif",
      "image/webp",
      "image/bmp",
      "image/tiff",
    ]
  ) {
    assertEquals(resolveMakePublic(type, true), true)
  }
})

Deno.test("resolveMakePublic - explicit false stays private even for images", () => {
  assertEquals(resolveMakePublic("image/png", false), false)
})

Deno.test("resolveMakePublic - rejects public for non-public-capable types", () => {
  // SVG is an image but not allowed to be public by Linear
  assertThrows(
    () => resolveMakePublic("image/svg+xml", true),
    ValidationError,
  )
  assertThrows(
    () => resolveMakePublic("application/pdf", true),
    ValidationError,
  )
  assertThrows(
    () => resolveMakePublic("application/octet-stream", true),
    ValidationError,
  )
})
