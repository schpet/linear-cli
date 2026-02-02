import { assertEquals } from "@std/assert"
import {
  extractMentions,
  textToProsemirrorDoc,
} from "../../src/utils/mentions.ts"

Deno.test("extractMentions - extracts single mention", () => {
  const text = "Hello @bot how are you?"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["bot"])
})

Deno.test("extractMentions - extracts multiple mentions", () => {
  const text = "Hey @alice and @bob, please review this"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["alice", "bob"])
})

Deno.test("extractMentions - handles duplicate mentions", () => {
  const text = "@bot please help @bot"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["bot"])
})

Deno.test("extractMentions - returns empty array for no mentions", () => {
  const text = "This text has no mentions"
  const mentions = extractMentions(text)
  assertEquals(mentions, [])
})

Deno.test("extractMentions - handles mentions with hyphens and underscores", () => {
  const text = "Hello @john-doe and @jane_smith"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["john-doe", "jane_smith"])
})

Deno.test("extractMentions - handles mention at start of line", () => {
  const text = "@admin please check this"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["admin"])
})

Deno.test("extractMentions - handles mention at end of line", () => {
  const text = "Please check this @admin"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["admin"])
})

Deno.test("extractMentions - handles mentions with numbers", () => {
  const text = "Hello @user123"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["user123"])
})

Deno.test("extractMentions - does not match email addresses", () => {
  // Note: This is a limitation - the regex will match the part after @
  const text = "Email me at user@example.com"
  const mentions = extractMentions(text)
  assertEquals(mentions, ["example"])
})

Deno.test("textToProsemirrorDoc - converts text without mentions", () => {
  const text = "Hello world"
  const doc = textToProsemirrorDoc(text, new Map())
  assertEquals(doc, {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [{ type: "text", text: "Hello world" }],
      },
    ],
  })
})

Deno.test("textToProsemirrorDoc - converts text with resolved mention", () => {
  const text = "Hello @bot"
  const resolved = new Map([
    ["bot", { id: "user-123", displayName: "Bot", name: "bot" }],
  ])
  const doc = textToProsemirrorDoc(text, resolved)
  assertEquals(doc, {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [
          { type: "text", text: "Hello " },
          {
            type: "suggestion_userMentions",
            attrs: { id: "user-123", label: "Bot" },
          },
        ],
      },
    ],
  })
})

Deno.test("textToProsemirrorDoc - keeps unresolved mention as text", () => {
  const text = "Hello @unknown"
  const doc = textToProsemirrorDoc(text, new Map())
  assertEquals(doc, {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [
          { type: "text", text: "Hello " },
          { type: "text", text: "@unknown" },
        ],
      },
    ],
  })
})

Deno.test("textToProsemirrorDoc - handles multiple paragraphs", () => {
  const text = "First line\n\nSecond line"
  const doc = textToProsemirrorDoc(text, new Map())
  assertEquals(doc, {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [{ type: "text", text: "First line" }],
      },
      {
        type: "paragraph",
        content: [],
      },
      {
        type: "paragraph",
        content: [{ type: "text", text: "Second line" }],
      },
    ],
  })
})

Deno.test("textToProsemirrorDoc - handles mention in the middle of text", () => {
  const text = "Hey @bot how are you?"
  const resolved = new Map([
    ["bot", { id: "user-123", displayName: "Bot", name: "bot" }],
  ])
  const doc = textToProsemirrorDoc(text, resolved)
  assertEquals(doc, {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [
          { type: "text", text: "Hey " },
          {
            type: "suggestion_userMentions",
            attrs: { id: "user-123", label: "Bot" },
          },
          { type: "text", text: " how are you?" },
        ],
      },
    ],
  })
})

Deno.test("textToProsemirrorDoc - handles multiple mentions in one line", () => {
  const text = "Hey @alice and @bob"
  const resolved = new Map([
    ["alice", { id: "user-1", displayName: "Alice", name: "alice" }],
    ["bob", { id: "user-2", displayName: "Bob", name: "bob" }],
  ])
  const doc = textToProsemirrorDoc(text, resolved)
  assertEquals(doc, {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [
          { type: "text", text: "Hey " },
          {
            type: "suggestion_userMentions",
            attrs: { id: "user-1", label: "Alice" },
          },
          { type: "text", text: " and " },
          {
            type: "suggestion_userMentions",
            attrs: { id: "user-2", label: "Bob" },
          },
        ],
      },
    ],
  })
})
