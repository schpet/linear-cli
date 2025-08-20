import { snapshotTest } from "@cliffy/testing";
import { viewCommand } from "../../../src/commands/project/project-view.ts";
import { MockLinearServer } from "../../utils/mock_linear_server.ts";

// Common Deno args for permissions
const denoArgs = [
  "--allow-env=GITHUB_*,GH_*,LINEAR_*,NODE_ENV,EDITOR,SNAPSHOT_TEST_NAME",
  "--allow-read",
  "--allow-write",
  "--allow-run",
  "--allow-net",
  "--quiet",
];

// Test help output
await snapshotTest({
  name: "Project View Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await viewCommand.parse();
  },
});

// Test with mock server - Project details
await snapshotTest({
  name: "Project View Command - With Project Details",
  meta: import.meta,
  colors: false,
  args: ["project-123"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectDetails",
        variables: { id: "project-123" },
        response: {
          data: {
            project: {
              id: "project-123",
              name: "Authentication System Redesign",
              description:
                "Complete overhaul of the authentication system to improve security and user experience.\n\n## Goals\n- Implement OAuth 2.0 / OpenID Connect\n- Add multi-factor authentication\n- Improve password reset flow\n- Add social login options\n\n## Technical Requirements\n- JWT tokens with proper rotation\n- Rate limiting on auth endpoints\n- Audit logging for security events\n- GDPR compliance for user data",
              slugId: "auth-redesign-2024",
              icon: "🔐",
              color: "#3b82f6",
              status: {
                id: "status-started",
                name: "In Progress",
                color: "#f59e0b",
              },
              creator: {
                name: "john.admin",
                displayName: "John Admin",
              },
              lead: {
                name: "jane.lead",
                displayName: "Jane Lead",
              },
              priority: 2,
              health: "onTrack",
              startDate: "2024-01-15",
              targetDate: "2024-04-30",
              startedAt: "2024-01-16T09:00:00Z",
              completedAt: null,
              canceledAt: null,
              updatedAt: "2024-01-25T14:30:00Z",
              createdAt: "2024-01-10T10:00:00Z",
              url: "https://linear.app/acme/project/auth-redesign-2024",
              teams: {
                nodes: [
                  {
                    id: "team-1",
                    key: "BACKEND",
                    name: "Backend Team",
                  },
                  {
                    id: "team-2",
                    key: "SECURITY",
                    name: "Security Team",
                  },
                ],
              },
              issues: {
                nodes: [
                  {
                    id: "issue-1",
                    identifier: "AUTH-101",
                    title: "Implement OAuth 2.0 flow",
                    state: {
                      name: "In Progress",
                      type: "started",
                    },
                  },
                  {
                    id: "issue-2",
                    identifier: "AUTH-102",
                    title: "Add MFA support",
                    state: {
                      name: "To Do",
                      type: "unstarted",
                    },
                  },
                  {
                    id: "issue-3",
                    identifier: "AUTH-103",
                    title: "Design new login UI",
                    state: {
                      name: "Done",
                      type: "completed",
                    },
                  },
                  {
                    id: "issue-4",
                    identifier: "AUTH-104",
                    title: "Security audit of current system",
                    state: {
                      name: "Canceled",
                      type: "canceled",
                    },
                  },
                ],
              },
              lastUpdate: {
                id: "update-1",
                body:
                  "Great progress this week! The OAuth implementation is nearly complete and we're on track for our Q1 delivery. The team has been collaborating well across backend and security concerns.\n\n**This week's highlights:**\n- OAuth 2.0 flow implementation 80% complete\n- MFA design reviews completed\n- Security penetration testing scheduled\n\n**Next week:**\n- Complete OAuth testing\n- Begin MFA implementation\n- Finalize UI designs",
                health: "onTrack",
                createdAt: "2024-01-22T16:00:00Z",
                user: {
                  name: "jane.lead",
                  displayName: "Jane Lead",
                },
              },
            },
          },
        },
      },
    ]);

    try {
      await server.start();
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint());
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token");

      await viewCommand.parse();
    } finally {
      await server.stop();
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
    }
  },
});

// Test with minimal project (no optional fields)
await snapshotTest({
  name: "Project View Command - Minimal Project",
  meta: import.meta,
  colors: false,
  args: ["minimal-project"],
  denoArgs,
  async fn() {
    const server = new MockLinearServer([
      {
        queryName: "GetProjectDetails",
        variables: { id: "minimal-project" },
        response: {
          data: {
            project: {
              id: "minimal-project",
              name: "Simple Project",
              description: "",
              slugId: "simple",
              icon: null,
              color: "#64748b",
              status: {
                id: "status-backlog",
                name: "Backlog",
                color: "#94a3b8",
              },
              creator: null,
              lead: null,
              priority: 0,
              health: null,
              startDate: null,
              targetDate: null,
              startedAt: null,
              completedAt: null,
              canceledAt: null,
              updatedAt: "2024-01-20T12:00:00Z",
              createdAt: "2024-01-20T12:00:00Z",
              url: "https://linear.app/acme/project/simple",
              teams: {
                nodes: [],
              },
              issues: {
                nodes: [],
              },
              lastUpdate: null,
            },
          },
        },
      },
    ]);

    try {
      await server.start();
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint());
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token");

      await viewCommand.parse();
    } finally {
      await server.stop();
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT");
      Deno.env.delete("LINEAR_API_KEY");
    }
  },
});
