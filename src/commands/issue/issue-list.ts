import { Command, EnumType } from "@cliffy/command";
import { unicodeWidth } from "@std/cli";
import { rgb24 } from "@std/fmt/colors";
import { getOption } from "../../config.ts";
import {
  getPriorityDisplay,
  getTimeAgo,
  padDisplay,
  truncateText,
} from "../../utils/display.ts";
import { fetchIssuesForState, getTeamId } from "../../utils/linear.ts";
import { openTeamAssigneeView } from "../../utils/actions.ts";
import { pipeToUserPager, shouldUsePager } from "../../utils/pager.ts";
import { header, muted } from "../../utils/styling.ts";

const SortType = new EnumType(["manual", "priority"]);
const StateType = new EnumType([
  "triage",
  "backlog",
  "unstarted",
  "started",
  "completed",
  "canceled",
]);

export const listCommand = new Command()
  .name("list")
  .description("List your issues")
  .type("sort", SortType)
  .type("state", StateType)
  .option(
    "-s, --state <state:state>",
    "Filter by issue state (can be repeated for multiple states)",
    {
      default: ["unstarted"],
      collect: true,
    },
  )
  .option(
    "--all-states",
    "Show issues from all states",
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
  .option(
    "--sort <sort:sort>",
    "Sort order (can also be set via LINEAR_ISSUE_SORT)",
    {
      required: false,
    },
  )
  .option(
    "--team <team:string>",
    "Team to list issues for (if not your default team)",
  )
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .option("--no-pager", "Disable automatic paging for long output")
  .action(
    async (
      {
        sort: sortFlag,
        state,
        assignee,
        allAssignees,
        unassigned,
        web,
        app,
        allStates,
        team,
        pager,
      },
    ) => {
      const usePager = pager !== false;
      if (web || app) {
        await openTeamAssigneeView({ app: app });
        return;
      }

      // Validate that conflicting flags are not used together
      const assigneeFilterCount =
        [assignee, allAssignees, unassigned].filter(Boolean).length;
      if (assigneeFilterCount > 1) {
        console.error(
          "Cannot specify multiple assignee filters (--assignee, --all-assignees, --unassigned)",
        );
        Deno.exit(1);
      }

      // Convert state to proper string array type
      const stateArray: string[] = Array.isArray(state)
        ? state.flat()
        : [state];

      // Validate state filters are not used together
      if (
        allStates && (stateArray.length > 1 || stateArray[0] !== "unstarted")
      ) {
        console.error(
          "Cannot use --all-states with --state flag",
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
      const teamKey = team || await getTeamId();
      if (!teamKey) {
        console.error(
          "Could not determine team key from directory name or team flag.",
        );
        Deno.exit(1);
      }

      const { Spinner } = await import("@std/cli/unstable-spinner");
      const showSpinner = Deno.stdout.isTerminal();
      const spinner = showSpinner ? new Spinner() : null;
      spinner?.start();

      try {
        const result = await fetchIssuesForState(
          teamKey,
          allStates ? undefined : stateArray,
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
        const LABEL_WIDTH = Math.min(
          25, // maximum width for labels column
          Math.max(
            6, // minimum width for "LABELS" header
            ...issues.map((issue) =>
              unicodeWidth(issue.labels.nodes.map((l) => l.name).join(", "))
            ),
          ),
        );
        const ESTIMATE_WIDTH = 1; // fixed width for estimate
        const STATE_WIDTH = Math.min(
          20, // maximum width for state
          Math.max(
            5, // minimum width for "STATE" header
            ...issues.map((issue) => unicodeWidth(issue.state.name)),
          ),
        );
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
          labels: string;
          state: string;
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

          // Format labels with colors
          let labels: string;
          if (issue.labels.nodes.length === 0) {
            labels = "";
          } else {
            const truncatedLabels = truncateText(plainLabels, LABEL_WIDTH);

            // Format with colors using @std/fmt/colors
            labels = issue.labels.nodes
              .filter((_, i) => i < truncatedLabels.split(", ").length)
              .map((l) => rgb24(l.name, parseInt(l.color.replace("#", ""), 16)))
              .join(", ");
          }
          const updatedAt = new Date(issue.updatedAt);
          const timeAgo = getTimeAgo(updatedAt);

          const priorityStr = getPriorityDisplay(issue.priority);

          // Truncate state name if it exceeds the column width
          const stateName = truncateText(issue.state.name, STATE_WIDTH);

          return {
            priorityStr,
            identifier: issue.identifier,
            title: issue.title,
            labels,
            state: rgb24(
              stateName,
              parseInt(issue.state.color.replace("#", ""), 16),
            ),
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

        // Format header line
        const formattedHeaderLine = header(headerCells.join(" "));

        // Collect all output lines first to determine if paging is needed
        const outputLines: string[] = [];

        // Add header line
        outputLines.push(formattedHeaderLine);

        // Add issue lines
        for (const row of tableData) {
          const {
            priorityStr,
            identifier,
            title,
            labels,
            state,
            timeAgo,
            estimate,
            assignee,
          } = row;
          const truncTitle = padDisplay(
            truncateText(title, titleWidth),
            titleWidth,
          );

          const assigneeOutput = showAssigneeColumn
            ? `${padDisplay(assignee || "-", ASSIGNEE_WIDTH)} `
            : "";

          const issueLine = `${padDisplay(priorityStr, PRIORITY_WIDTH)} ${
            padDisplay(identifier, ID_WIDTH)
          } ${truncTitle} ${padDisplay(labels, LABEL_WIDTH)} ${
            padDisplay(estimate?.toString() || "-", ESTIMATE_WIDTH)
          } ${assigneeOutput}${padDisplay(state, STATE_WIDTH)} ${
            muted(padDisplay(timeAgo, UPDATED_WIDTH))
          }`;
          outputLines.push(issueLine);
        }

        // Check if we should use pager
        if (shouldUsePager(outputLines, usePager)) {
          await pipeToUserPager(outputLines.join("\n"));
        } else {
          // Print directly for shorter output - same logic as pager
          outputLines.forEach((line) => console.log(line));
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
  );
