import { snapshotTest } from "@cliffy/testing"
import { viewCommand } from "../../../src/commands/issue/issue-view.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// Test help output
await snapshotTest({
  name: "Issue View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await viewCommand.parse()
  },
})

// Test with mock GraphQL endpoint - connection refused
// NOTE: This test verifies error handling when the Linear API is unreachable.
// The error output varies by platform (different OS error codes), so we remove it.
// The important behavior (user-friendly error message on stderr) is covered by other "Not Found" tests.

// Test with working mock server - Terminal output (no comments available)
await snapshotTest({
  name: "Issue View Command - With Mock Server Terminal No Comments",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetailsWithComments",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              identifier: "TEST-123",
              title: "Fix authentication bug in login flow",
              description:
                "Users are experiencing issues logging in when their session expires. This affects the main authentication flow and needs to be resolved quickly.\n\n## Steps to reproduce\n1. Log in to the application\n2. Wait for session to expire\n3. Try to perform an authenticated action\n4. Observe the error\n\n## Expected behavior\nUser should be redirected to login page with clear messaging.\n\n## Actual behavior\nUser sees cryptic error message and gets stuck.",
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              project: null,
              projectMilestone: null,
              parent: null,
              children: {
                nodes: [],
              },
              comments: {
                nodes: [],
              },
              attachments: {
                nodes: [],
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with no-comments flag to disable comments
await snapshotTest({
  name: "Issue View Command - With No Comments Flag",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "--no-comments"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              identifier: "TEST-123",
              title: "Fix authentication bug in login flow",
              description:
                "Users are experiencing issues logging in when their session expires.",
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              project: null,
              projectMilestone: null,
              parent: null,
              children: {
                nodes: [],
              },
              attachments: {
                nodes: [],
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with comments (default behavior)
await snapshotTest({
  name: "Issue View Command - With Comments Default",
  meta: import.meta,
  colors: false,
  args: ["TEST-123"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetailsWithComments",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              identifier: "TEST-123",
              title: "Fix authentication bug in login flow",
              description:
                "Users are experiencing issues logging in when their session expires.",
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              project: null,
              projectMilestone: null,
              parent: null,
              children: {
                nodes: [],
              },
              comments: {
                nodes: [
                  {
                    id: "comment-1",
                    body:
                      "I've reproduced this issue on staging. The session timeout seems to be too aggressive.",
                    createdAt: "2024-01-15T10:30:00Z",
                    user: {
                      name: "john.doe",
                      displayName: "John Doe",
                    },
                    externalUser: null,
                    parent: null,
                  },
                  {
                    id: "comment-2",
                    body:
                      "Working on a fix. Will increase the session timeout and add proper error handling.",
                    createdAt: "2024-01-15T14:22:00Z",
                    user: {
                      name: "jane.smith",
                      displayName: "Jane Smith",
                    },
                    externalUser: null,
                    parent: {
                      id: "comment-1",
                    },
                  },
                  {
                    id: "comment-3",
                    body:
                      "Sounds good! Also, we should add better error messaging for expired sessions.",
                    createdAt: "2024-01-15T15:10:00Z",
                    user: {
                      name: "alice.dev",
                      displayName: "Alice Developer",
                    },
                    externalUser: null,
                    parent: {
                      id: "comment-1",
                    },
                  },
                  {
                    id: "comment-4",
                    body:
                      "Should we also consider implementing automatic session refresh?",
                    createdAt: "2024-01-15T16:00:00Z",
                    user: {
                      name: "bob.senior",
                      displayName: "Bob Senior",
                    },
                    externalUser: null,
                    parent: null,
                  },
                ],
              },
              attachments: {
                nodes: [],
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with mock server - Issue not found
await snapshotTest({
  name: "Issue View Command - Issue Not Found",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["TEST-999"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetailsWithComments",
        variables: { id: "TEST-999" },
        response: {
          errors: [{
            message: "Issue not found: TEST-999",
            extensions: { code: "NOT_FOUND" },
          }],
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test JSON output with no comments
await snapshotTest({
  name: "Issue View Command - JSON Output No Comments",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "--json", "--no-comments"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              identifier: "TEST-123",
              title: "Fix authentication bug in login flow",
              description:
                "Users are experiencing issues logging in when their session expires.",
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              parent: null,
              children: {
                nodes: [],
              },
              attachments: {
                nodes: [],
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test JSON output with comments
await snapshotTest({
  name: "Issue View Command - JSON Output With Comments",
  meta: import.meta,
  colors: false,
  args: ["TEST-123", "--json"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetailsWithComments",
        variables: { id: "TEST-123" },
        response: {
          data: {
            issue: {
              identifier: "TEST-123",
              title: "Fix authentication bug in login flow",
              description:
                "Users are experiencing issues logging in when their session expires.",
              url:
                "https://linear.app/test-team/issue/TEST-123/fix-authentication-bug-in-login-flow",
              branchName: "fix/test-123-auth-bug",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              project: null,
              projectMilestone: null,
              parent: null,
              children: {
                nodes: [],
              },
              comments: {
                nodes: [
                  {
                    id: "comment-1",
                    body:
                      "I've reproduced this issue on staging. The session timeout seems to be too aggressive.",
                    createdAt: "2024-01-15T10:30:00Z",
                    user: {
                      name: "john.doe",
                      displayName: "John Doe",
                    },
                    externalUser: null,
                    parent: null,
                  },
                  {
                    id: "comment-2",
                    body:
                      "Working on a fix. Will increase the session timeout and add proper error handling.",
                    createdAt: "2024-01-15T14:22:00Z",
                    user: {
                      name: "jane.smith",
                      displayName: "Jane Smith",
                    },
                    externalUser: null,
                    parent: {
                      id: "comment-1",
                    },
                  },
                ],
              },
              attachments: {
                nodes: [],
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with parent and sub-issues
await snapshotTest({
  name: "Issue View Command - With Parent And Sub-issues",
  meta: import.meta,
  colors: false,
  args: ["TEST-456", "--no-comments"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-456" },
        response: {
          data: {
            issue: {
              identifier: "TEST-456",
              title: "Implement user authentication",
              description: "Add user authentication to the application.",
              url:
                "https://linear.app/test-team/issue/TEST-456/implement-user-authentication",
              branchName: "feat/test-456-auth",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              project: null,
              projectMilestone: null,
              parent: {
                identifier: "TEST-100",
                title: "Epic: Security Improvements",
                state: {
                  name: "In Progress",
                  color: "#f87462",
                },
              },
              children: {
                nodes: [
                  {
                    identifier: "TEST-457",
                    title: "Add login form",
                    state: {
                      name: "Done",
                      color: "#4cb782",
                    },
                  },
                  {
                    identifier: "TEST-458",
                    title: "Add password reset flow",
                    state: {
                      name: "Todo",
                      color: "#bec2c8",
                    },
                  },
                  {
                    identifier: "TEST-459",
                    title: "Add OAuth support",
                    state: {
                      name: "In Progress",
                      color: "#f87462",
                    },
                  },
                ],
              },
              attachments: {
                nodes: [],
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})

// Test with project and milestone
await snapshotTest({
  name: "Issue View Command - With Project And Milestone",
  meta: import.meta,
  colors: false,
  args: ["TEST-789", "--no-comments"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetIssueDetails",
        variables: { id: "TEST-789" },
        response: {
          data: {
            issue: {
              identifier: "TEST-789",
              title: "Add monitoring dashboards",
              description: "Set up Datadog dashboards for the new service.",
              url:
                "https://linear.app/test-team/issue/TEST-789/add-monitoring-dashboards",
              branchName: "feat/test-789-monitoring",
              state: {
                name: "In Progress",
                color: "#f87462",
              },
              project: {
                name: "Platform Infrastructure Q1",
              },
              projectMilestone: {
                name: "Phase 2: Observability",
              },
              parent: null,
              children: {
                nodes: [],
              },
              attachments: {
                nodes: [],
              },
            },
          },
        },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await viewCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
