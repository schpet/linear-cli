import { snapshotTest } from "@cliffy/testing"
import { createCommand } from "../../../src/commands/label/label-create.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs, resolveTeamMock } from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Label Create Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await createCommand.parse()
  },
})

// --team accepts a name; the mutation receives the team UUID.
await snapshotTest({
  name: "Label Create Command - Team By Name",
  meta: import.meta,
  colors: false,
  args: ["--name", "backend", "--team", "Engineering"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      resolveTeamMock("Engineering"),
      {
        queryName: "CreateIssueLabel",
        variables: {
          input: { name: "backend", color: "#5E6AD2", teamId: "team-eng-id" },
        },
        response: {
          data: {
            issueLabelCreate: {
              success: true,
              issueLabel: {
                id: "label-1",
                name: "backend",
                color: "#5e6ad2",
                description: null,
                team: { key: "ENG", name: "Engineering" },
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

      await createCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
