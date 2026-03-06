import { snapshotTest } from "@cliffy/testing"
import { relationCommand } from "../../../src/commands/issue/issue-relation.ts"
import {
  commonDenoArgs,
  setupMockLinearServer,
} from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue Relation Add Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["add", "--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await relationCommand.parse()
  },
})

// Test: relation add with "blocks" - success message shows original order
await snapshotTest({
  name: "Issue Relation Add Command - blocks",
  meta: import.meta,
  colors: false,
  args: ["add", "ENG-123", "blocks", "ENG-456"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-123" },
        response: {
          data: { issue: { id: "issue-id-123" } },
        },
      },
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-456" },
        response: {
          data: { issue: { id: "issue-id-456" } },
        },
      },
      {
        queryName: "CreateIssueRelation",
        response: {
          data: {
            issueRelationCreate: {
              success: true,
              issueRelation: { id: "relation-id-1" },
            },
          },
        },
      },
    ])

    try {
      await relationCommand.parse()
    } finally {
      await cleanup()
    }
  },
})

// Test: relation add with "blocked-by" - success message should show original user-specified order
// i.e. "ENG-123 blocked-by ENG-456" NOT "ENG-456 blocked-by ENG-123"
await snapshotTest({
  name: "Issue Relation Add Command - blocked-by shows correct order",
  meta: import.meta,
  colors: false,
  args: ["add", "ENG-123", "blocked-by", "ENG-456"],
  denoArgs: commonDenoArgs,
  async fn() {
    const { cleanup } = await setupMockLinearServer([
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-123" },
        response: {
          data: { issue: { id: "issue-id-123" } },
        },
      },
      {
        queryName: "GetIssueId",
        variables: { id: "ENG-456" },
        response: {
          data: { issue: { id: "issue-id-456" } },
        },
      },
      {
        queryName: "CreateIssueRelation",
        response: {
          data: {
            issueRelationCreate: {
              success: true,
              // API is called with swapped IDs (ENG-456 blocks ENG-123),
              // but we should display the user-specified order in the message
              issueRelation: { id: "relation-id-2" },
            },
          },
        },
      },
    ])

    try {
      await relationCommand.parse()
    } finally {
      await cleanup()
    }
  },
})
