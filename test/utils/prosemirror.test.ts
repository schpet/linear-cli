import { assertEquals, assertThrows } from "@std/assert"
import { proseMirrorToMarkdown } from "../../src/utils/prosemirror.ts"
import { ValidationError } from "../../src/utils/errors.ts"

function text(value: string, marks?: { type: string; attrs?: unknown }[]) {
  return marks == null ? { type: "text", text: value } : {
    type: "text",
    text: value,
    marks,
  }
}

function paragraph(...content: unknown[]) {
  return { type: "paragraph", content }
}

Deno.test("proseMirrorToMarkdown renders the body Linear stores for an issue template", () => {
  // The exact shape Linear returned for a template created with a markdown
  // description: headings, an ordered list with an empty item, headings.
  const doc = {
    type: "doc",
    content: [
      {
        type: "heading",
        attrs: { level: 2, id: "f0ba1592" },
        content: [text("Steps to reproduce")],
      },
      {
        type: "ordered_list",
        attrs: { order: 1 },
        content: [{ type: "list_item", content: [{ type: "paragraph" }] }],
      },
      { type: "heading", attrs: { level: 2 }, content: [text("Expected")] },
      { type: "heading", attrs: { level: 2 }, content: [text("Actual")] },
    ],
  }
  assertEquals(
    proseMirrorToMarkdown(doc),
    "## Steps to reproduce\n\n1. \n\n## Expected\n\n## Actual",
  )
})

Deno.test("proseMirrorToMarkdown renders marks, links, and hard breaks", () => {
  const doc = {
    type: "doc",
    content: [
      paragraph(
        text("Hello "),
        text("world", [{ type: "bold" }]),
        text(" and "),
        text("code", [{ type: "code" }]),
        text(" "),
        text("link", [{
          type: "link",
          attrs: { href: "https://example.com" },
        }]),
        { type: "hard_break" },
        text("next", [{ type: "em" }, { type: "strike" }]),
      ),
    ],
  }
  assertEquals(
    proseMirrorToMarkdown(doc),
    "Hello **world** and `code` [link](https://example.com)\n~~_next_~~",
  )
})

Deno.test("proseMirrorToMarkdown renders nested lists, todo lists, code, and quotes", () => {
  const doc = {
    type: "doc",
    content: [
      {
        type: "bullet_list",
        content: [
          {
            type: "list_item",
            content: [
              paragraph(text("one")),
              {
                type: "bullet_list",
                content: [
                  { type: "list_item", content: [paragraph(text("nested"))] },
                ],
              },
            ],
          },
          { type: "list_item", content: [paragraph(text("two"))] },
        ],
      },
      {
        type: "todo_list",
        content: [
          {
            type: "todo_item",
            attrs: { done: true },
            content: [paragraph(text("done"))],
          },
          { type: "todo_item", content: [paragraph(text("open"))] },
        ],
      },
      {
        type: "code_block",
        attrs: { language: "ts" },
        content: [text("const a = 1")],
      },
      { type: "blockquote", content: [paragraph(text("quoted"))] },
      { type: "horizontal_rule" },
    ],
  }
  assertEquals(
    proseMirrorToMarkdown(doc),
    [
      "- one\n\n  - nested\n- two",
      "- [x] done\n- [ ] open",
      "```ts\nconst a = 1\n```",
      "> quoted",
      "---",
    ].join("\n\n"),
  )
})

Deno.test("proseMirrorToMarkdown keeps unknown nodes visible instead of dropping them", () => {
  const doc = {
    type: "doc",
    content: [
      paragraph(
        text("cc "),
        { type: "suggestion_userMentions", attrs: { id: "u1", label: "@sam" } },
        text(" and "),
        { type: "mystery_inline" },
      ),
      { type: "embed", attrs: { url: "https://example.com" } },
      { type: "wrapper", content: [paragraph(text("inside a wrapper"))] },
    ],
  }
  assertEquals(
    proseMirrorToMarkdown(doc),
    "cc @sam and [mystery_inline]\n\n[unsupported embed node]\n\ninside a wrapper",
  )
})

Deno.test("proseMirrorToMarkdown escapes literal Markdown punctuation but not code", () => {
  const doc = {
    type: "doc",
    content: [
      paragraph(
        text("*literal* `tick` [x] a_b # 1 > 2 "),
        text("a ` b", [{ type: "code" }]),
        text(" "),
        text("*bold*", [{ type: "bold" }]),
      ),
      paragraph(text("- not a list")),
      paragraph(text("1. not ordered")),
      paragraph(text("2026 roadmap")),
      {
        type: "code_block",
        content: [text("```md\nfenced inside\n```")],
      },
    ],
  }
  assertEquals(
    proseMirrorToMarkdown(doc),
    [
      "\\*literal\\* \\`tick\\` \\[x\\] a\\_b \\# 1 \\> 2 `` a ` b `` **\\*bold\\***",
      "\\- not a list",
      "1\\. not ordered",
      "2026 roadmap",
      "````\n```md\nfenced inside\n```\n````",
    ].join("\n\n"),
  )
})

Deno.test("proseMirrorToMarkdown rejects values that are not a document", () => {
  assertThrows(
    () => proseMirrorToMarkdown({ type: "paragraph", content: [] }),
    ValidationError,
    'Expected a ProseMirror document, got a "paragraph" node',
  )
  assertThrows(
    () => proseMirrorToMarkdown({ type: "doc", content: "not an array" }),
    ValidationError,
    '"content" must be an array',
  )
  assertThrows(
    () => proseMirrorToMarkdown("## markdown"),
    ValidationError,
    "Invalid ProseMirror node at doc",
  )
})
