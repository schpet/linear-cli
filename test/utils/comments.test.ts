import { assertEquals, assertRejects } from "@std/assert"
import { stripAnsiCode } from "@std/fmt/colors"
import { stub } from "@std/testing/mock"
import {
  buildCommentCreateInput,
  collectCommentPages,
  type CommentPageInfo,
  renderCommentThreads,
  resolveCommentBody,
} from "../../src/utils/comments.ts"
import { CliError, ValidationError } from "../../src/utils/errors.ts"

// Linear requires exactly one owning entity even on a reply, so the builder
// must always emit the target key next to parentId.
Deno.test("buildCommentCreateInput pairs the target with parentId on a reply", () => {
  assertEquals(
    buildCommentCreateInput(
      { kind: "document", documentContentId: "content-1" },
      { body: "hi", parentId: "root-1" },
    ),
    { body: "hi", parentId: "root-1", documentContentId: "content-1" },
  )
  assertEquals(
    buildCommentCreateInput(
      { kind: "initiative", initiativeId: "init-1" },
      { body: "hi" },
    ),
    { body: "hi", initiativeId: "init-1" },
  )
})

Deno.test("resolveCommentBody rejects --body together with --body-file", async () => {
  await assertRejects(
    () => resolveCommentBody({ body: "a", bodyFile: "b.md" }),
    ValidationError,
    "Cannot specify both",
  )
})

// Explicit input that is blank is an error, never a fall-through to the prompt.
Deno.test("resolveCommentBody rejects a whitespace-only --body", async () => {
  await assertRejects(
    () => resolveCommentBody({ body: "  \n" }),
    ValidationError,
    "cannot be empty",
  )
})

Deno.test("resolveCommentBody rejects an empty body file", async () => {
  const file = await Deno.makeTempFile({ suffix: ".md" })
  try {
    await Deno.writeTextFile(file, "\n\n")
    await assertRejects(
      () => resolveCommentBody({ bodyFile: file }),
      ValidationError,
      "Body file is empty",
    )
  } finally {
    await Deno.remove(file)
  }
})

Deno.test("resolveCommentBody wraps an unreadable body file", async () => {
  await assertRejects(
    () => resolveCommentBody({ bodyFile: "/nonexistent/comment.md" }),
    ValidationError,
    "Failed to read body file",
  )
})

Deno.test("resolveCommentBody returns undefined when neither flag is given", async () => {
  assertEquals(await resolveCommentBody({}), undefined)
})

Deno.test("collectCommentPages follows cursors and keeps the last pageInfo", async () => {
  const requested: (string | null)[] = []
  const result = await collectCommentPages<string, CommentPageInfo>((after) => {
    requested.push(after)
    if (after == null) {
      return Promise.resolve({
        nodes: ["a", "b"],
        pageInfo: { hasNextPage: true, endCursor: "cursor-1" },
      })
    }
    return Promise.resolve({
      nodes: ["c"],
      pageInfo: { hasNextPage: false, endCursor: "cursor-2" },
    })
  })

  assertEquals(requested, [null, "cursor-1"])
  assertEquals(result, {
    nodes: ["a", "b", "c"],
    pageInfo: { hasNextPage: false, endCursor: "cursor-2" },
  })
})

Deno.test("collectCommentPages refuses a next page without a cursor", async () => {
  await assertRejects(
    () =>
      collectCommentPages(() =>
        Promise.resolve({
          nodes: ["a"],
          pageInfo: { hasNextPage: true, endCursor: null },
        })
      ),
    CliError,
    "usable cursor",
  )
})

// A server that keeps handing back the same cursor must not spin forever.
Deno.test("collectCommentPages refuses a repeated cursor", async () => {
  let calls = 0
  await assertRejects(
    () =>
      collectCommentPages(() => {
        calls++
        return Promise.resolve({
          nodes: ["a"],
          pageInfo: { hasNextPage: true, endCursor: "same" },
        })
      }),
    CliError,
    "usable cursor",
  )
  assertEquals(calls, 2)
})

// A reply whose root is missing from the list (deleted, or paged out) used to
// vanish from the rendered output entirely.
Deno.test("renderCommentThreads keeps a reply whose parent is absent", () => {
  const lines: string[] = []
  // The renderer bolds the author, so strip ANSI before matching text: CI
  // runs without NO_COLOR and the escape codes would split "@Ada replied".
  const logStub = stub(console, "log", (...args: unknown[]) => {
    lines.push(stripAnsiCode(args.map(String).join(" ")))
  })
  try {
    renderCommentThreads(
      [
        {
          id: "reply-1",
          body: "still here",
          createdAt: "2024-01-15T10:30:00Z",
          user: { name: "ada", displayName: "Ada" },
          parent: { id: "gone-root" },
        },
      ],
      { emptyMessage: "none" },
    )
  } finally {
    logStub.restore()
  }

  assertEquals(
    lines.some((line) =>
      line.includes("@Ada replied to [gone-root]") && line.includes("[reply-1]")
    ),
    true,
  )
  assertEquals(lines.some((line) => line.includes("still here")), true)
  // It is not misrepresented as a root comment.
  assertEquals(lines.some((line) => line.includes("commented")), false)
})
