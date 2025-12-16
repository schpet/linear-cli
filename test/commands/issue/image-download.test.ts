import { assertEquals } from "@std/assert"
import {
  extractImageInfo,
  getUrlHash,
  replaceImageUrls,
} from "../../../src/commands/issue/issue-view.ts"
import {
  formatPathHyperlink,
  hyperlink,
  resolveHyperlinkFormat,
  shouldEnableHyperlinks,
} from "../../../src/utils/hyperlink.ts"

Deno.test("extractImageInfo - extracts markdown images", () => {
  const markdown = "Check this ![screenshot](https://example.com/img.png)"
  const images = extractImageInfo(markdown)
  assertEquals(images, [{
    url: "https://example.com/img.png",
    alt: "screenshot",
  }])
})

Deno.test("extractImageInfo - extracts multiple images", () => {
  const markdown = `
Here is ![first](https://example.com/1.png) and ![second](https://example.com/2.png)
`
  const images = extractImageInfo(markdown)
  assertEquals(images.length, 2)
  assertEquals(images[0], { url: "https://example.com/1.png", alt: "first" })
  assertEquals(images[1], { url: "https://example.com/2.png", alt: "second" })
})

Deno.test("extractImageInfo - handles image without alt text", () => {
  const markdown = "![](https://example.com/img.png)"
  const images = extractImageInfo(markdown)
  assertEquals(images, [{
    url: "https://example.com/img.png",
    alt: null,
  }])
})

Deno.test("extractImageInfo - handles empty content", () => {
  assertEquals(extractImageInfo(null), [])
  assertEquals(extractImageInfo(undefined), [])
  assertEquals(extractImageInfo(""), [])
})

Deno.test("extractImageInfo - handles markdown with no images", () => {
  const markdown = "# Hello\n\nThis is just text with no images."
  const images = extractImageInfo(markdown)
  assertEquals(images, [])
})

Deno.test("getUrlHash - generates consistent hash", async () => {
  const hash1 = await getUrlHash("https://example.com/img.png")
  const hash2 = await getUrlHash("https://example.com/img.png")
  assertEquals(hash1, hash2)
  assertEquals(hash1.length, 16)
})

Deno.test("getUrlHash - different URLs produce different hashes", async () => {
  const hash1 = await getUrlHash("https://example.com/img1.png")
  const hash2 = await getUrlHash("https://example.com/img2.png")
  assertEquals(hash1 !== hash2, true)
})

Deno.test("getUrlHash - hash is valid hex string", async () => {
  const hash = await getUrlHash("https://example.com/img.png")
  assertEquals(/^[0-9a-f]{16}$/.test(hash), true)
})

Deno.test("replaceImageUrls - replaces URLs with local paths", async () => {
  const markdown = "![alt](https://example.com/img.png)"
  const urlToPath = new Map([
    ["https://example.com/img.png", "/tmp/cached/img.png"],
  ])
  const result = await replaceImageUrls(markdown, urlToPath)
  assertEquals(result.includes("/tmp/cached/img.png"), true)
  assertEquals(result.includes("https://example.com/img.png"), false)
})

Deno.test("replaceImageUrls - replaces multiple URLs", async () => {
  const markdown = `
![first](https://example.com/1.png)
![second](https://example.com/2.png)
`
  const urlToPath = new Map([
    ["https://example.com/1.png", "/tmp/cached/1.png"],
    ["https://example.com/2.png", "/tmp/cached/2.png"],
  ])
  const result = await replaceImageUrls(markdown, urlToPath)
  assertEquals(result.includes("/tmp/cached/1.png"), true)
  assertEquals(result.includes("/tmp/cached/2.png"), true)
})

Deno.test("replaceImageUrls - leaves unmatched URLs unchanged", async () => {
  const markdown = "![alt](https://example.com/img.png)"
  const urlToPath = new Map([
    ["https://other.com/img.png", "/tmp/cached/img.png"],
  ])
  const result = await replaceImageUrls(markdown, urlToPath)
  assertEquals(result.includes("https://example.com/img.png"), true)
})

Deno.test("replaceImageUrls - handles empty map", async () => {
  const markdown = "![alt](https://example.com/img.png)"
  const urlToPath = new Map<string, string>()
  const result = await replaceImageUrls(markdown, urlToPath)
  assertEquals(result.includes("https://example.com/img.png"), true)
})

// Hyperlink utility tests

Deno.test("hyperlink - creates OSC-8 escape sequence", () => {
  const result = hyperlink("click me", "https://example.com")
  assertEquals(
    result,
    "\x1b]8;;https://example.com\x1b\\click me\x1b]8;;\x1b\\",
  )
})

Deno.test("hyperlink - handles empty text", () => {
  const result = hyperlink("", "https://example.com")
  assertEquals(result, "\x1b]8;;https://example.com\x1b\\\x1b]8;;\x1b\\")
})

Deno.test("resolveHyperlinkFormat - resolves default to file URL format", () => {
  assertEquals(resolveHyperlinkFormat("default"), "file://{host}{path}")
})

Deno.test("resolveHyperlinkFormat - passes through custom format", () => {
  assertEquals(resolveHyperlinkFormat("custom://{path}"), "custom://{path}")
})

Deno.test("formatPathHyperlink - wraps remote URL in hyperlink", () => {
  const result = formatPathHyperlink(
    "https://example.com/img.png",
    "https://example.com/img.png",
    "default",
  )
  // Remote URLs link directly to themselves
  assertEquals(
    result.includes("\x1b]8;;https://example.com/img.png\x1b\\"),
    true,
  )
  assertEquals(
    result.includes("https://example.com/img.png\x1b]8;;\x1b\\"),
    true,
  )
})

Deno.test("formatPathHyperlink - wraps local path with file URL format", () => {
  const result = formatPathHyperlink(
    "/tmp/test/image.png",
    "/tmp/test/image.png",
    "default",
  )
  // Local paths get file:// URL format
  assertEquals(result.includes("\x1b]8;;file://"), true)
  assertEquals(result.includes("/tmp/test/image.png"), true)
})

Deno.test("formatPathHyperlink - encodes special characters in path", () => {
  const result = formatPathHyperlink(
    "/tmp/test/my image#1.png",
    "/tmp/test/my image#1.png",
    "default",
  )
  // # should be percent-encoded
  assertEquals(result.includes("%23"), true)
  // Spaces should be percent-encoded
  assertEquals(result.includes("%20"), true)
})

Deno.test("shouldEnableHyperlinks - returns false when NO_COLOR is set", () => {
  const originalNoColor = Deno.env.get("NO_COLOR")
  try {
    Deno.env.set("NO_COLOR", "1")
    assertEquals(shouldEnableHyperlinks(), false)
  } finally {
    if (originalNoColor === undefined) {
      Deno.env.delete("NO_COLOR")
    } else {
      Deno.env.set("NO_COLOR", originalNoColor)
    }
  }
})

// formatPathHyperlink already tested above - it creates OSC-8 escape sequences
// The hyperlink application to rendered output happens in issue-view.ts
// and uses formatPathHyperlink internally

// Note: Image URL validation is tested indirectly through integration tests
// The validateImageUrl function in issue-view.ts restricts downloads to Linear domains only
// This prevents data leakage to external sources when processing issue markdown
