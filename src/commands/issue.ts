import { Command, EnumType } from "@cliffy/command";
import { Confirm, Select } from "@cliffy/prompt";
import { renderMarkdown } from "@littletof/charmd";
import { unicodeWidth } from "@std/cli";
import { gql } from "../../__generated__/gql.ts";
import { getOption } from "../config.ts";
import { getGraphQLClient } from "../utils/graphql.ts";
import {
  getPriorityDisplay,
  getTimeAgo,
  padDisplay,
  padDisplayFormatted,
} from "../utils/display.ts";
import {
  fetchIssueDetails,
  fetchIssuesForState,
  getIssueId,
  getTeamId,
} from "../utils/linear.ts";
import {
  doStartIssue as startIssue,
  openIssuePage,
  openTeamPage,
} from "../utils/actions.ts";
import { createCommand } from "./create.ts";

const SortType = new EnumType(["manual", "priority"]);
const StateType = new EnumType([
  "triage",
  "backlog",
  "unstarted",
  "started",
  "completed",
  "canceled",
]);

export const issueCommand = new Command()
  .description("Manage Linear issues")
  .action(function () {
    this.showHelp();
  })
  .command(
    "open",
    "Open the issue in Linear.app (deprecated: use `linear issue view --app` instead)",
  )
  .alias("o")
  .arguments("[issueId:string]")
  .action((_, issueId) => {
    console.error(
      "Warning: 'linear issue open' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue view --app' instead.");
    return openIssuePage(issueId, { app: true });
  })
  .command(
    "print",
    "Print the issue details (deprecated: use `linear issue view` instead)",
  )
  .alias("p")
  .arguments("[issueId:string]")
  .option("--no-color", "Disable colored output")
  .option("--no-interactive", "Disable interactive prompts")
  .action(async ({ color, interactive }, issueId) => {
    console.error(
      "Warning: 'linear issue print' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue view' instead.");
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const showSpinner = color && interactive && Deno.stdout.isTerminal();
    const { title, description } = await fetchIssueDetails(
      resolvedId,
      showSpinner,
    );
    const markdown = `# ${title}${description ? "\n\n" + description : ""}`;
    if (color && Deno.stdout.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  })
  .command("id", "Print the issue based on the current git branch")
  .action(async (_) => {
    const resolvedId = await getIssueId();
    if (resolvedId) {
      console.log(resolvedId);
    } else {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
  })
  .command("list", "List your issues")
  .type("sort", SortType)
  .type("state", StateType)
  .option(
    "--sort <sort:sort>",
    "Sort order (can also be set via LINEAR_ISSUE_SORT)",
    {
      required: false,
    },
  )
  .option(
    "-s, --state <state:state>",
    "Filter by issue state",
    {
      default: "unstarted",
    },
  )
  .option(
    "--assignee <assignee:string>",
    "Filter by assignee (username)",
  )
  .option(
    "-A, --all-assignees",
    "Show issues for all assignees",
  )
  .option(
    "-U, --unassigned",
    "Show only unassigned issues",
  )
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .action(
    async (
      { sort: sortFlag, state, assignee, allAssignees, unassigned, web, app },
    ) => {
      if (web || app) {
        await openTeamPage({ app });
        return;
      }

      // Validate that conflicting flags are not used together
      const flagCount =
        [assignee, allAssignees, unassigned].filter(Boolean).length;
      if (flagCount > 1) {
        console.error(
          "Cannot specify multiple assignee filters (--assignee, --all-assignees, --unassigned)",
        );
        Deno.exit(1);
      }

      const sort = sortFlag ||
        getOption("issue_sort") as "manual" | "priority" | undefined;
      if (!sort) {
        console.error(
          "Sort must be provided via command line flag, configuration file, or LINEAR_ISSUE_SORT environment variable",
        );
        Deno.exit(1);
      }
      if (!SortType.values().includes(sort)) {
        console.error(`Sort must be one of: ${SortType.values().join(", ")}`);
        Deno.exit(1);
      }
      const teamId = await getTeamId();
      if (!teamId) {
        console.error("Could not determine team id from directory name.");
        Deno.exit(1);
      }

      const { Spinner } = await import("@std/cli/unstable-spinner");
      const showSpinner = Deno.stdout.isTerminal();
      const spinner = showSpinner ? new Spinner() : null;
      spinner?.start();

      try {
        const result = await fetchIssuesForState(
          teamId,
          state,
          assignee,
          unassigned,
          allAssignees,
        );
        spinner?.stop();
        const issues = result.issues?.nodes || [];

        if (issues.length === 0) {
          console.log("No issues found.");
          return;
        }

        // Define column widths first
        const { columns } = Deno.consoleSize();
        const PRIORITY_WIDTH = 3;
        const ID_WIDTH = Math.max(
          2, // minimum width for "ID" header
          ...issues.map((issue) => issue.identifier.length),
        );
        const LABEL_WIDTH = columns <= 100 ? 12 : 24; // adjust label width based on terminal size
        const ESTIMATE_WIDTH = 1; // fixed width for estimate
        const STATE_WIDTH = 12; // fixed width for state
        const ASSIGNEE_WIDTH = 2; // fixed width for assignee initials
        const SPACE_WIDTH = 4;
        const showAssigneeColumn = allAssignees || unassigned;
        const updatedHeader = "UPDATED";
        const UPDATED_WIDTH = Math.max(
          unicodeWidth(updatedHeader),
          ...issues.map((issue) =>
            unicodeWidth(getTimeAgo(new Date(issue.updatedAt)))
          ),
        );

        type TableRow = {
          priorityStr: string;
          identifier: string;
          title: string;
          labelsFormat: string;
          labelsStyles: string[];
          state: string;
          stateStyles: string[];
          timeAgo: string;
          estimate: number | null | undefined;
          assignee?: string;
        };

        const tableData: Array<TableRow> = issues.map((issue) => {
          // First build the plain text version to measure length
          const plainLabels = issue.labels.nodes.map((l) => l.name).join(
            ", ",
          );

          // Get assignee initials if needed
          const assignee = showAssigneeColumn
            ? (issue.assignee?.initials?.slice(0, 2) || "-")
            : undefined;
          let labelsFormat: string;
          let labelsStyles: string[] = [];

          if (issue.labels.nodes.length === 0) {
            labelsFormat = "";
          } else {
            const truncatedLabels = plainLabels.length > LABEL_WIDTH
              ? plainLabels.slice(0, LABEL_WIDTH - 3) + "..."
              : plainLabels;

            // Then format the truncated version with colors
            labelsFormat = truncatedLabels
              .split(", ")
              .map((name) => `%c${name}%c`)
              .join(", ");
            labelsStyles = issue.labels.nodes
              .filter((_, i) => i < truncatedLabels.split(", ").length)
              .flatMap((l) => [`color: ${l.color}`, ""]);
          }
          const updatedAt = new Date(issue.updatedAt);
          const timeAgo = getTimeAgo(updatedAt);

          const priorityStr = getPriorityDisplay(issue.priority);

          return {
            priorityStr,
            identifier: issue.identifier,
            title: issue.title,
            labelsFormat,
            labelsStyles,
            state: `%c${issue.state.name}%c`,
            stateStyles: [`color: ${issue.state.color}`, ""],
            timeAgo,
            estimate: issue.estimate,
            assignee,
          };
        });

        const fixed = PRIORITY_WIDTH + ID_WIDTH + UPDATED_WIDTH + SPACE_WIDTH +
          LABEL_WIDTH + ESTIMATE_WIDTH + STATE_WIDTH + SPACE_WIDTH +
          (showAssigneeColumn ? ASSIGNEE_WIDTH + SPACE_WIDTH : 0); // sum of fixed columns including spacing for estimate
        const PADDING = 1;
        const maxTitleWidth = Math.max(
          ...tableData.map((row) => unicodeWidth(row.title)),
        );
        const availableWidth = Math.max(columns - PADDING - fixed, 0);
        const titleWidth = Math.min(maxTitleWidth, availableWidth); // use smaller of max title width or available space
        const headerCells = [
          padDisplay("◌", PRIORITY_WIDTH),
          padDisplay("ID", ID_WIDTH),
          padDisplay("TITLE", titleWidth),
          padDisplay("LABELS", LABEL_WIDTH),
          padDisplay("E", ESTIMATE_WIDTH),
          ...(showAssigneeColumn ? [padDisplay("A", ASSIGNEE_WIDTH)] : []),
          padDisplay("STATE", STATE_WIDTH),
          padDisplay(updatedHeader, UPDATED_WIDTH),
        ];
        let headerMsg = "";
        const headerStyles: string[] = [];
        headerCells.forEach((cell, index) => {
          headerMsg += `%c${cell}`;
          headerStyles.push("text-decoration: underline");
          if (index < headerCells.length - 1) {
            headerMsg += "%c %c"; // non-underlined space between cells
            headerStyles.push("text-decoration: none");
            headerStyles.push("text-decoration: underline");
          }
        });
        console.log(headerMsg, ...headerStyles);

        // Print each issue
        for (const row of tableData) {
          const {
            priorityStr,
            identifier,
            title,
            labelsFormat,
            labelsStyles,
            state,
            stateStyles,
            timeAgo,
            assignee,
          } = row;
          const truncTitle = title.length > titleWidth
            ? title.slice(0, titleWidth - 3) + "..."
            : padDisplay(title, titleWidth);

          const assigneeOutput = showAssigneeColumn
            ? `${padDisplay(assignee || "-", ASSIGNEE_WIDTH)} `
            : "";

          console.log(
            `${padDisplay(priorityStr, PRIORITY_WIDTH)} ${
              padDisplay(identifier, ID_WIDTH)
            } ${truncTitle} ${padDisplayFormatted(labelsFormat, LABEL_WIDTH)} ${
              padDisplay(row.estimate?.toString() || "-", ESTIMATE_WIDTH)
            } ${assigneeOutput}${padDisplayFormatted(state, STATE_WIDTH)} %c${
              padDisplay(timeAgo, UPDATED_WIDTH)
            }%c`,
            ...labelsStyles,
            ...stateStyles,
            "color: gray",
            "",
          );
        }
      } catch (error) {
        spinner?.stop();
        if (
          error instanceof Error && error.message.startsWith("User not found:")
        ) {
          console.error(error.message);
        } else {
          console.error("Failed to fetch issues:", error);
        }
        Deno.exit(1);
      }
    },
  )
  .command("title", "Print the issue title")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { title } = await fetchIssueDetails(resolvedId, false);
    console.log(title);
  })
  .command("start", "Start working on an issue")
  .arguments("[issueId:string]")
  .option(
    "-A, --all-assignees",
    "Show issues for all assignees",
  )
  .option(
    "-U, --unassigned",
    "Show only unassigned issues",
  )
  .action(async ({ allAssignees, unassigned }, issueId) => {
    const teamId = await getTeamId();
    if (!teamId) {
      console.error("Could not determine team ID");
      Deno.exit(1);
    }

    // Validate that conflicting flags are not used together
    if (allAssignees && unassigned) {
      console.error("Cannot specify both --all-assignees and --unassigned");
      Deno.exit(1);
    }

    let resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      try {
        const result = await fetchIssuesForState(
          teamId,
          "unstarted",
          undefined,
          unassigned,
          allAssignees,
        );
        const issues = result.issues?.nodes || [];

        if (issues.length === 0) {
          console.error("No unstarted issues found.");
          Deno.exit(1);
        }

        const answer = await Select.prompt({
          message: "Select an issue to start:",
          options: issues.map((
            issue: { identifier: string; title: string; priority: number },
          ) => ({
            name: getPriorityDisplay(issue.priority) +
              ` ${issue.identifier}: ${issue.title}`,
            value: issue.identifier,
          })),
        });

        resolvedId = answer as string;
      } catch (error) {
        console.error("Failed to fetch issues:", error);
        Deno.exit(1);
      }
    }

    if (!resolvedId) {
      console.error("No issue ID resolved");
      Deno.exit(1);
    }
    await startIssue(resolvedId, teamId);
  })
  .command("view", "View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .action(async ({ web, app }, issueId) => {
    if (web || app) {
      await openIssuePage(issueId, { app, web: !app });
      return;
    }

    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const { title, description } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    );
    const markdown = `# ${title}${description ? "\n\n" + description : ""}`;
    if (Deno.stdout.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  })
  .command("url", "Print the issue URL")
  .arguments("[issueId:string]")
  .action(async (_, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { url } = await fetchIssueDetails(resolvedId, false);
    console.log(url);
  })
  .command("pull-request", "Create a GitHub pull request with issue details")
  .alias("pr")
  .option(
    "--base <branch:string>",
    "The branch into which you want your code merged",
  )
  .option(
    "--draft",
    "Create the pull request as a draft",
  )
  .option(
    "-t, --title <title:string>",
    "Optional title for the pull request (Linear issue ID will be prefixed)",
  )
  .arguments("[issueId:string]")
  .action(async ({ base, draft, title: customTitle }, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { title, url } = await fetchIssueDetails(
      resolvedId,
      Deno.stdout.isTerminal(),
    );

    const process = new Deno.Command("gh", {
      args: [
        "pr",
        "create",
        "--title",
        `${resolvedId} ${customTitle ?? title}`,
        "--body",
        url,
        ...(base ? ["--base", base] : []),
        ...(draft ? ["--draft"] : ["--web"]),
      ],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });

    const status = await process.spawn().status;
    if (!status.success) {
      console.error("Failed to create pull request");
      Deno.exit(1);
    }
  })
  .command("delete", "Delete an issue")
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
  })
  .command("create", createCommand);
