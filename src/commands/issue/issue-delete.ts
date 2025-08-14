import { Command } from "@cliffy/command";
import { Confirm } from "@cliffy/prompt";
import { gql } from "../../__codegen__/gql.ts";
import { getGraphQLClient } from "../../utils/graphql.ts";
import { getIssueId } from "../../utils/linear.ts";

export const deleteCommand = new Command()
  .name("delete")
  .description("Delete an issue")
  .alias("d")
  .arguments("<issueId:string>")
  .action(async (_, issueId) => {
    // First resolve the issue ID to get the issue details
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error("Could not find issue with ID:", issueId);
      Deno.exit(1);
    }

    // Get issue details to show title in confirmation
    const client = getGraphQLClient();
    const detailsQuery = gql(`
      query GetIssueDeleteDetails($id: String!) {
        issue(id: $id) { title, identifier }
      }
    `);

    let issueDetails;
    try {
      issueDetails = await client.request(detailsQuery, { id: resolvedId });
    } catch (error) {
      console.error("Failed to fetch issue details:", error);
      Deno.exit(1);
    }

    if (!issueDetails?.issue) {
      console.error("Issue not found:", resolvedId);
      Deno.exit(1);
    }

    const { title, identifier } = issueDetails.issue;

    // Show confirmation prompt
    const confirmed = await Confirm.prompt({
      message: `Are you sure you want to delete "${identifier}: ${title}"?`,
      default: false,
    });

    if (!confirmed) {
      console.log("Delete cancelled.");
      return;
    }

    // Delete the issue
    const deleteQuery = gql(`
      mutation DeleteIssue($id: String!) {
        issueDelete(id: $id) {
          success
          entity {
            identifier
            title
          }
        }
      }
    `);

    try {
      const result = await client.request(deleteQuery, { id: resolvedId });

      if (result.issueDelete.success) {
        console.log(`✓ Successfully deleted issue: ${identifier}: ${title}`);
      } else {
        console.error("Failed to delete issue");
        Deno.exit(1);
      }
    } catch (error) {
      console.error("Failed to delete issue:", error);
      Deno.exit(1);
    }
  });
