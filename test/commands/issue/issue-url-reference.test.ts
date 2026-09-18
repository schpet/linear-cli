import { assertEquals, assertStringIncludes } from "@std/assert"
import { linkCommand } from "../../../src/commands/issue/issue-link.ts"
import { setupMockLinearServer } from "../../utils/test-helpers.ts"

const ISSUE_URL = "https://linear.app/url-test-workspace/issue/ENG-123/a-title"

function withWorkspace(): () => void {
  const previous = Deno.env.get("LINEAR_WORKSPACE")
  Deno.env.set("LINEAR_WORKSPACE", "url-test-workspace")
  return () => {
    if (previous == null) {
      Deno.env.delete("LINEAR_WORKSPACE")
    } else {
      Deno.env.set("LINEAR_WORKSPACE", previous)
    }
  }
}

/**
 * `issue link` treats a single URL argument as the thing being linked, not as
 * the issue to link it to. A Linear URL is a perfectly reasonable thing to
 * attach to an issue, so URL-as-identifier must not reach this argument.
 */
Deno.test("issue link still treats a lone Linear URL as the link target", async () => {
  const restore = withWorkspace()
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetIssueId",
      variables: { id: "ENG-999" },
      response: { data: { issue: { id: "issue-uuid-999" } } },
    },
    {
      queryName: "AttachmentLinkURL",
      response: {
        data: {
          attachmentLinkURL: {
            success: true,
            attachment: { id: "a1", title: "Linked", url: ISSUE_URL },
          },
        },
      },
    },
  ])

  const originalLog = console.log
  const output: string[] = []
  console.log = (...args: unknown[]) => {
    output.push(args.map(String).join(" "))
  }
  try {
    // Two arguments: the first is the issue, the second is the URL to attach.
    // If the lone-URL branch regressed, the Linear URL would be consumed as an
    // identifier and this attachment would never be created.
    await linkCommand.parse(["ENG-999", ISSUE_URL])
  } finally {
    console.log = originalLog
    restore()
    await cleanup()
  }

  assertStringIncludes(output.join("\n"), "Linked")
})

/**
 * Resolution has to happen inside each action's `try`, or the ValidationError it
 * raises for a wrong-kind URL escapes `cli.parse` and prints a stack trace
 * instead of the clean message handleError produces.
 */
Deno.test("a wrong-kind URL is reported, not thrown out of the action", async () => {
  const { commentListCommand } = await import(
    "../../../src/commands/document/document-comment-list.ts"
  )
  const restore = withWorkspace()
  const { cleanup } = await setupMockLinearServer([])

  const originalError = console.error
  const originalExit = Deno.exit
  const errors: string[] = []
  let exitCode: number | undefined
  console.error = (...args: unknown[]) => {
    errors.push(args.map(String).join(" "))
  }
  Deno.exit = ((code?: number) => {
    exitCode = code
    throw new Error("exit")
  }) as typeof Deno.exit

  try {
    await commentListCommand.parse([ISSUE_URL])
  } catch (error) {
    if (!(error instanceof Error) || error.message !== "exit") throw error
  } finally {
    console.error = originalError
    Deno.exit = originalExit
    restore()
    await cleanup()
  }

  assertEquals(exitCode, 1)
  assertStringIncludes(errors.join("\n"), "is an issue URL, not a document URL")
})
