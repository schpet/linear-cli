import { snapshotTest } from "@cliffy/testing"
import { deleteCommand } from "../../../src/commands/label/label-delete.ts"
import { MockLinearServer } from "../../utils/mock_linear_server.ts"
import { commonDenoArgs, resolveTeamMock } from "../../utils/test-helpers.ts"

await snapshotTest({
  name: "Label Delete Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await deleteCommand.parse()
  },
})

// --team by name disambiguates same-named labels on the canonical key: the
// APP label is deleted, not the ENG one that comes back first.
await snapshotTest({
  name: "Label Delete Command - Team By Name Disambiguates",
  meta: import.meta,
  colors: false,
  args: ["bug", "--team", "Apps", "--force"],
  denoArgs: commonDenoArgs,
  async fn() {
    const server = new MockLinearServer([
      resolveTeamMock("Apps", { id: "team-app-id", key: "APP", name: "Apps" }),
      {
        queryName: "GetLabelByName",
        variables: { name: "bug" },
        response: {
          data: {
            issueLabels: {
              nodes: [
                {
                  id: "label-eng-bug",
                  name: "bug",
                  color: "#ff0000",
                  team: { key: "ENG", name: "Engineering" },
                },
                {
                  id: "label-app-bug",
                  name: "bug",
                  color: "#ff0000",
                  team: { key: "APP", name: "Apps" },
                },
              ],
            },
          },
        },
      },
      {
        queryName: "DeleteIssueLabel",
        variables: { id: "label-app-bug" },
        response: { data: { issueLabelDelete: { success: true } } },
      },
    ])

    try {
      await server.start()
      Deno.env.set("LINEAR_GRAPHQL_ENDPOINT", server.getEndpoint())
      Deno.env.set("LINEAR_API_KEY", "Bearer test-token")

      await deleteCommand.parse()
    } finally {
      await server.stop()
      Deno.env.delete("LINEAR_GRAPHQL_ENDPOINT")
      Deno.env.delete("LINEAR_API_KEY")
    }
  },
})
