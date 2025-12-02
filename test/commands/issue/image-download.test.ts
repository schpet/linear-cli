import { assertEquals } from "@std/assert"
import {
  extractImageInfo,
  getUrlHash,
  replaceImageUrls,
} from "../../../src/commands/issue/issue-view.ts"

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
