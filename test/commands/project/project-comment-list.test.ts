import { snapshotTest } from "@cliffy/testing"
import { assertEquals } from "@std/assert"
import { stub } from "@std/testing/mock"
import { commentListCommand } from "../../../src/commands/project/project-comment-list.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

const PROJECT_ID = "6f1c3b8a-2d4e-4a5b-9c7d-1e2f3a4b5c6d"

const projectComments = {
  nodes: [
    {
      id: "comment-uuid-1",
      body: "Kickoff is Monday.",
      quotedText: null,
      createdAt: "2024-01-15T10:30:00Z",
      updatedAt: "2024-01-15T10:30:00Z",
      editedAt: null,
      url:
        "https://linear.app/team/project/mobile-abc123/activity#comment-uuid-1",
      user: { id: "user-uuid-1", name: "ada", displayName: "Ada Lovelace" },
      externalUser: null,
      botActor: null,
      parent: null,
    },
    {
      id: "comment-uuid-2",
      body: "I'll be there.",
      quotedText: null,
      createdAt: "2024-01-15T11:00:00Z",
      updatedAt: "2024-01-15T11:00:00Z",
      editedAt: null,
      url:
        "https://linear.app/team/project/mobile-abc123/activity#comment-uuid-2",
      user: { id: "user-uuid-2", name: "grace", displayName: "Grace Hopper" },
      externalUser: null,
      botActor: null,
      parent: { id: "comment-uuid-1" },
    },
  ],
  pageInfo: { hasNextPage: false, endCursor: "comment-uuid-2" },
}

// Project-thread comments are not returned by `Project.comments`; the command
// must go through the root `comments` query filtered by project, and it must
// send the UUID both as the entity lookup id and as the filter id.
await snapshotTest({
  name: "Project Comment List Command - By UUID",
  meta: import.meta,
  colors: false,
  args: [PROJECT_ID],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetProjectComments",
        queryIncludes: "project: {id: {eq: $filterId}}",
        variables: { id: PROJECT_ID, filterId: PROJECT_ID, after: null },
        response: {
          data: {
            project: { id: PROJECT_ID, name: "Mobile launch" },
            comments: projectComments,
          },
        },
      },
    ])

    try {
      await commentListCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// A name goes through the shared project resolver first.
await snapshotTest({
  name: "Project Comment List Command - By Name JSON Output",
  meta: import.meta,
  colors: false,
  args: ["Mobile launch", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetProjectIdByName",
        variables: { name: "Mobile launch" },
        response: { data: { projects: { nodes: [{ id: PROJECT_ID }] } } },
      },
      {
        queryName: "GetProjectComments",
        queryIncludes: "quotedText",
        variables: { id: PROJECT_ID, filterId: PROJECT_ID, after: null },
        response: {
          data: {
            project: { id: PROJECT_ID, name: "Mobile launch" },
            comments: projectComments,
          },
        },
      },
    ])

    try {
      await commentListCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

await snapshotTest({
  name: "Project Comment List Command - No Comments",
  meta: import.meta,
  colors: false,
  args: [PROJECT_ID],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetProjectComments",
        response: {
          data: {
            project: { id: PROJECT_ID, name: "Mobile launch" },
            comments: {
              nodes: [],
              pageInfo: { hasNextPage: false, endCursor: null },
            },
          },
        },
      },
    ])

    try {
      await commentListCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Both ids and the cursor must be sent on every page of the combined
// entity-plus-root-comments operation.
await snapshotTest({
  name: "Project Comment List Command - JSON Output Follows Pagination",
  meta: import.meta,
  colors: false,
  args: [PROJECT_ID, "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    const [first, second] = projectComments.nodes
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetProjectComments",
        variables: { id: PROJECT_ID, filterId: PROJECT_ID, after: null },
        response: {
          data: {
            project: { id: PROJECT_ID, name: "Mobile launch" },
            comments: {
              nodes: [first],
              pageInfo: { hasNextPage: true, endCursor: "cursor-1" },
            },
          },
        },
      },
      {
        queryName: "GetProjectComments",
        variables: { id: PROJECT_ID, filterId: PROJECT_ID, after: "cursor-1" },
        response: {
          data: {
            project: { id: PROJECT_ID, name: "Mobile launch" },
            comments: {
              nodes: [second],
              pageInfo: { hasNextPage: false, endCursor: "cursor-2" },
            },
          },
        },
      },
    ])

    try {
      await commentListCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// resolveProjectId passes a UUID through unchecked; the entity lookup in the
// list operation is what turns an unknown UUID into a not-found error instead
// of an empty list.
Deno.test("Project Comment List Command - unknown project UUID is reported as not found", async () => {
  const { cleanup } = await setupMockLinearServer([
    {
      queryName: "GetProjectComments",
      response: {
        errors: [{
          message: "Entity not found: Project",
          extensions: {
            type: "invalid input",
            userError: true,
            userPresentableMessage: "Could not find referenced Project.",
          },
        }],
      },
    },
  ])

  const errorLogs: string[] = []
  const errorStub = stub(console, "error", (...args: unknown[]) => {
    errorLogs.push(args.map(String).join(" "))
  })
  const exitStub = stub(Deno, "exit", (_code?: number) => {
    throw new Error("EXIT")
  })

  let exited = false
  try {
    await commentListCommand.parse([PROJECT_ID])
  } catch (e) {
    if (!(e instanceof Error) || e.message !== "EXIT") throw e
    exited = true
  } finally {
    errorStub.restore()
    exitStub.restore()
    await cleanup()
  }

  assertEquals(exited, true)
  assertEquals(
    errorLogs.some((l) =>
      l.toLowerCase().includes("not found") && l.includes(PROJECT_ID)
    ),
    true,
    errorLogs.join("\n"),
  )
})
