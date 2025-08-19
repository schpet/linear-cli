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
import { fetchIssuesForState, getTeamKey } from "../../utils/linear.ts";
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
      const teamKey = team || getTeamKey();
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
        const { columns } = Deno.stdout.isTerminal()
          ? Deno.consoleSize()
          : { columns: 120 };
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
          // Get assignee initials if needed
          const assignee = showAssigneeColumn
            ? (issue.assignee?.initials?.slice(0, 2) || "-")
            : undefined;

          // Format labels with colors
          let labels: string;
          if (issue.labels.nodes.length === 0) {
            labels = " ".repeat(LABEL_WIDTH);
          } else {
            // Build colored labels incrementally to ensure proper width
            const coloredLabels: string[] = [];
            let currentWidth = 0;

            for (let i = 0; i < issue.labels.nodes.length; i++) {
              const label = issue.labels.nodes[i];
              const coloredLabel = rgb24(
                label.name,
                parseInt(label.color.replace("#", ""), 16),
              );
              const separator = i > 0 ? ", " : "";
              const testText = separator + label.name;

              if (currentWidth + unicodeWidth(testText) > LABEL_WIDTH) {
                // If adding this label would exceed width, truncate if possible
                const remainingWidth = LABEL_WIDTH - currentWidth;
                if (remainingWidth >= 4) { // Need at least 4 chars for "..."
                  const truncatedName = truncateText(
                    label.name,
                    remainingWidth - (separator.length),
                  );
                  coloredLabels.push(
                    separator +
                      rgb24(
                        truncatedName,
                        parseInt(label.color.replace("#", ""), 16),
                      ),
                  );
                }
                break;
              }

              coloredLabels.push(separator + coloredLabel);
              currentWidth += unicodeWidth(testText);
            }

            labels = coloredLabels.join("");
            // Calculate actual width of the final labels string (excluding color codes)
            const ansiRegex = new RegExp("\u001B\\[[0-9;]*m", "g");
            const actualLabelsWidth = unicodeWidth(
              coloredLabels.join("").replace(ansiRegex, ""),
            );
            const remainingSpace = Math.max(0, LABEL_WIDTH - actualLabelsWidth);
            labels += " ".repeat(remainingSpace);
          }
          const updatedAt = new Date(issue.updatedAt);
          const timeAgo = getTimeAgo(updatedAt);

          const priorityStr = getPriorityDisplay(issue.priority);

          // Truncate state name if it exceeds the column width
          const stateName = truncateText(issue.state.name, STATE_WIDTH);
          const stateColored = rgb24(
            stateName,
            parseInt(issue.state.color.replace("#", ""), 16),
          );
          // Add padding to fill the remaining width
          const stateRemainingSpace = Math.max(
            0,
            STATE_WIDTH - unicodeWidth(stateName),
          );
          const statePadded = stateColored + " ".repeat(stateRemainingSpace);

          return {
            priorityStr,
            identifier: issue.identifier,
            title: issue.title,
            labels,
            state: statePadded,
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
          } ${truncTitle} ${labels} ${
            padDisplay(estimate?.toString() || "-", ESTIMATE_WIDTH)
          } ${assigneeOutput}${state} ${
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
