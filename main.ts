import { Command, EnumType } from "@cliffy/command";
import { load } from "@std/dotenv";
import { getOption } from "./config.ts";
import { prompt, Select } from "@cliffy/prompt";

// Try loading .env from current directory first, then from git root if not found
if (await Deno.stat(".env").catch(() => null)) {
  await load({ export: true });
} else {
  try {
    const gitRoot = new TextDecoder().decode(
      await new Deno.Command("git", {
        args: ["rev-parse", "--show-toplevel"],
      }).output().then((output) => output.stdout),
    ).trim();

    const gitRootEnvPath = join(gitRoot, ".env");
    if (await Deno.stat(gitRootEnvPath).catch(() => null)) {
      await load({ envPath: gitRootEnvPath, export: true });
    }
  } catch {
    // Silently continue if not in a git repo
  }
}

const SortType = new EnumType(["manual", "priority"]);
const StateType = new EnumType([
  "triage",
  "backlog",
  "unstarted",
  "started",
  "completed",
  "canceled",
]);
import { Spinner } from "@std/cli/unstable-spinner";
import { open } from "@opensrc/deno-open";
import { CompletionsCommand } from "@cliffy/command/completions";
import denoConfig from "./deno.json" with { type: "json" };
import { encodeBase64 } from "@std/encoding/base64";
import { renderMarkdown } from "@littletof/charmd";
import { basename, join } from "@std/path";
import { unicodeWidth } from "@std/cli";

interface Label {
  name: string;
  color: string;
}

interface Issue {
  id: string;
  identifier: string;
  title: string;
  priority: number;
  labels: { nodes: Label[] };
  state: {
    id: string;
    name: string;
    color: string;
  };
  updatedAt: string;
}

function padDisplay(s: string, width: number): string {
  const w = unicodeWidth(s);
  return s + " ".repeat(Math.max(0, width - w));
}

function stripConsoleFormat(s: string): string {
  return s.replace(/%c/g, "");
}

function padDisplayFormatted(s: string, width: number): string {
  const plain = stripConsoleFormat(s);
  const w = unicodeWidth(plain);
  return s + " ".repeat(Math.max(0, width - w));
}

function getTimeAgo(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return "just now";
  if (diffMins < 60) return `${diffMins} minutes ago`;
  if (diffHours < 24) {
    return `about ${diffHours} hour${diffHours === 1 ? "" : "s"} ago`;
  }
  return `${diffDays} day${diffDays === 1 ? "" : "s"} ago`;
}

async function getCurrentBranch(): Promise<string | null> {
  const process = new Deno.Command("git", {
    args: ["symbolic-ref", "--short", "HEAD"],
  });
  const { stdout } = await process.output();
  const branch = new TextDecoder().decode(stdout).trim();
  return branch || null;
}

async function getRepoDir(): Promise<string> {
  const process = new Deno.Command("git", {
    args: ["rev-parse", "--show-toplevel"],
  });
  const { stdout } = await process.output();
  const fullPath = new TextDecoder().decode(stdout).trim();
  return basename(fullPath);
}

async function branchExists(branch: string): Promise<boolean> {
  try {
    const process = new Deno.Command("git", {
      args: ["rev-parse", "--verify", branch],
    });
    const { success } = await process.output();
    return success;
  } catch {
    return false;
  }
}

async function getStartedState(
  teamId: string,
): Promise<{ id: string; name: string }> {
  const query = /* GraphQL */ `
    query($teamId: String!) {
      team(id: $teamId) {
        states {
          nodes {
            id
            name
            type
            position
          }
        }
      }
    }
  `;

  const result = await fetchGraphQL(query, { teamId });
  const states = result.data.team.states.nodes;
  const startedStates = states
    .filter((s: { type: string }) => s.type === "started")
    .sort((a: { position: number }, b: { position: number }) =>
      a.position - b.position
    );

  if (!startedStates.length) {
    throw new Error("No 'started' state found in workflow");
  }

  const startedState = startedStates[0];

  return { id: startedState.id, name: startedState.name };
}

async function updateIssueState(
  issueId: string,
  stateId: string,
): Promise<void> {
  const mutation = `
    mutation($issueId: String!, $stateId: String!) {
      issueUpdate(
        id: $issueId,
        input: { stateId: $stateId }
      ) {
        success
      }
    }
  `;

  await fetchGraphQL(mutation, { issueId, stateId });
}

function isValidLinearId(id: string): boolean {
  return /^[A-Za-z]{2,5}-[1-9][0-9]*$/.test(id);
}

async function getIssueId(providedId?: string): Promise<string | null> {
  if (providedId) {
    if (!isValidLinearId(providedId)) {
      console.error(`Invalid Linear issue ID format: ${providedId}`);
      console.error("Issue IDs should look like: ABC-123");
      Deno.exit(1);
    }
    return providedId.toUpperCase();
  }
  const branch = await getCurrentBranch();
  if (!branch) return null;
  const match = branch.match(/[a-zA-Z]{2,5}-[1-9][0-9]*/i);
  return match ? match[0].toUpperCase() : null;
}

async function getTeamId(): Promise<string | null> {
  const teamId = getOption("team_id");
  if (teamId) {
    return teamId.toUpperCase();
  }
  const dir = await getRepoDir();
  const match = dir.match(/^[a-zA-Z]{2,5}/);
  return match ? match[0].toUpperCase() : null;
}

async function fetchGraphQL(query: string, variables: Record<string, unknown>) {
  const apiKey = getOption("api_key");
  if (!apiKey) {
    throw new Error(
      "api_key is not set via command line, configuration file, or environment.",
    );
  }

  const response = await fetch("https://api.linear.app/graphql", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": apiKey,
    },
    body: JSON.stringify({ query, variables }),
  });

  const data = await response.json();
  if (data.errors) {
    throw new Error(JSON.stringify(data.errors, null, 2));
  }
  return data;
}

async function fetchIssueDetails(
  issueId: string,
  showSpinner = false,
): Promise<
  { title: string; description: string | null; url: string; branchName: string }
> {
  const spinner = showSpinner ? new Spinner() : null;
  spinner?.start();
  try {
    const query =
      `query($id: String!) { issue(id: $id) { title, description, url, branchName } }`;
    const data = await fetchGraphQL(query, { id: issueId });
    spinner?.stop();
    return data.data.issue;
  } catch (error) {
    spinner?.stop();
    console.error("✗ Failed to fetch issue details");
    throw error;
  }
}

async function openTeamPage() {
  const teamId = await getTeamId();
  if (!teamId) {
    console.error(
      "Could not determine team id from configuration or directory name.",
    );
    Deno.exit(1);
  }

  const workspace = getOption("workspace");
  if (!workspace) {
    console.error(
      "workspace is not set via command line, configuration file, or environment.",
    );
    Deno.exit(1);
  }

  const filterObj = {
    "and": [{ "assignee": { "or": [{ "isMe": { "eq": true } }] } }],
  };
  const filter = encodeBase64(JSON.stringify(filterObj)).replace(/=/g, "");
  const url =
    `https://linear.app/${workspace}/team/${teamId}/active?filter=${filter}`;
  await open(url, options.app ? { app: { name: "Linear" } } : undefined);
}

async function openIssuePage(providedId?: string, options: { app?: boolean; web?: boolean } = {}) {
  const issueId = await getIssueId(providedId);
  if (!issueId) {
    console.error(
      "The current branch does not contain a valid linear issue id.",
    );
    Deno.exit(1);
  }

  const workspace = getOption("workspace");
  if (!workspace) {
    console.error(
      "workspace is not set via command line, configuration file, or environment.",
    );
    Deno.exit(1);
  }

  const url = `https://linear.app/${workspace}/issue/${issueId}`;
  console.log(`Opening ${url} in Linear.app`);
  await open(url, { app: { name: "Linear" } });
}

const teamCommand = new Command()
  .description("Manage Linear teams")
  .action(openTeamPage)
  .command("open", "Open the team page in Linear.app")
  .alias("o")
  .action(openTeamPage)
  .command("id", "Print the team id derived from the repository name")
  .action(async () => {
    const teamId = await getTeamId();
    if (teamId) {
      console.log(teamId);
    } else {
      console.error("Could not determine team id from directory name.");
      Deno.exit(1);
    }
  })
  .command(
    "autolinks",
    "Configure GitHub repository autolinks for Linear issues",
  )
  .action(async () => {
    const teamId = await getTeamId();
    if (!teamId) {
      console.error("Could not determine team id from directory name.");
      Deno.exit(1);
    }

    const workspace = getOption("workspace");
    if (!workspace) {
      console.error(
        "workspace is not set via command line, configuration file, or environment.",
      );
      Deno.exit(1);
    }

    const process = new Deno.Command("gh", {
      args: [
        "api",
        "repos/{owner}/{repo}/autolinks",
        "-f",
        `key_prefix=${teamId}-`,
        "-f",
        `url_template=https://linear.app/${workspace}/issue/${teamId}-<num>`,
      ],
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });

    const status = await process.spawn().status;
    if (!status.success) {
      console.error("Failed to configure autolinks");
      Deno.exit(1);
    }
  });

const issueCommand = new Command()
  .description("Manage Linear issues")
  .arguments("[issueId:string]")
  .action((_, issueId) => openIssuePage(issueId))
  .command("open", "Open the issue in Linear.app")
  .alias("o")
  .arguments("[issueId:string]")
  .action((_, issueId) => openIssuePage(issueId, { app: true }))
  .command("print", "Print the issue details")
  .alias("p")
  .arguments("[issueId:string]")
  .option("--no-color", "Disable colored output")
  .action(async ({ color }, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const showSpinner = color && Deno.stdout.isTerminal();
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
    "--state <state:state>",
    "Filter by issue state",
    {
      default: "unstarted",
    },
  )
  .action(async ({ sort: sortFlag, state }) => {
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

    const query = /* GraphQL */ `
      query issues($teamId: String!, $sort: [IssueSortInput!], $states: [String!]) {
        issues(
          filter: {
            team: { key: { eq: $teamId } }
            assignee: { isMe: { eq: true } }
            state: { type: { in: $states } }
          }
          sort: $sort
        ) {
          nodes {
            id
            identifier
            title
            priority
            state {
              id
              name
              color
            }
            labels {
              nodes {
                id
                name
                color
              }
            }
            updatedAt
          }
        }
      }
    `;

    try {
      const sortPayload = sort === "manual"
        ? [{ manual: { nulls: "last", order: "Ascending" } }]
        : [{ priority: { nulls: "last", order: "Descending" } }];

      const result = await fetchGraphQL(query, {
        teamId,
        sort: sortPayload,
        states: [state],
      });
      const issues = result.data.issues.nodes;

      if (issues.length === 0) {
        console.log("No unstarted issues found.");
        return;
      }

      // Define column widths first
      const { columns } = Deno.consoleSize();
      const PRIORITY_WIDTH = 4;
      const ID_WIDTH = 8;
      const LABEL_WIDTH = columns <= 100 ? 12 : 24; // adjust label width based on terminal size
      const STATE_WIDTH = 12; // fixed width for state
      const SPACE_WIDTH = 4;
      const updatedHeader = "UPDATED";
      const UPDATED_WIDTH = Math.max(
        unicodeWidth(updatedHeader),
        ...issues.map((issue: Issue) =>
          unicodeWidth(getTimeAgo(new Date(issue.updatedAt)))
        ),
      );

      type TableRow = {
        priorityStr: string;
        priorityStyles: string[];
        identifier: string;
        title: string;
        labelsFormat: string;
        labelsStyles: string[];
        state: string;
        stateStyles: string[];
        timeAgo: string;
      };

      const tableData: Array<TableRow> = issues.map((issue: Issue) => {
        // First build the plain text version to measure length
        const plainLabels = issue.labels.nodes.map((l: Label) => l.name).join(
          ", ",
        );
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
            .flatMap((l: Label) => [`color: ${l.color}`, ""]);
        }
        const updatedAt = new Date(issue.updatedAt);
        const timeAgo = getTimeAgo(updatedAt);

        let priorityStr = "";
        let priorityStyles: string[] = [];
        if (issue.priority === 0) {
          priorityStr = "%c---%c";
          priorityStyles = ["color: silver", ""];
        } else if (issue.priority === 1 || issue.priority === 2) {
          // ▄▆█
          priorityStr = "%c▄%c▆%c█%c";
          priorityStyles = ["", "", "", ""];
        } else if (issue.priority === 3) {
          priorityStr = "%c▄%c▆%c█%c";
          priorityStyles = ["", "", "color: silver", ""];
        } else if (issue.priority === 4) {
          priorityStr = "%c▄%c▆%c█%c";
          priorityStyles = ["", "color: silver", "color: silver", ""];
        } else {
          priorityStr = issue.priority.toString();
          priorityStyles = [];
        }

        return {
          priorityStr,
          priorityStyles,
          identifier: issue.identifier,
          title: issue.title,
          labelsFormat,
          labelsStyles,
          state: `%c${issue.state.name}%c`,
          stateStyles: [`color: ${issue.state.color}`, ""],
          timeAgo,
        };
      });

      const fixed = PRIORITY_WIDTH + ID_WIDTH + UPDATED_WIDTH + SPACE_WIDTH +
        LABEL_WIDTH + STATE_WIDTH; // sum of fixed columns
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
        padDisplay("STATE", STATE_WIDTH),
        padDisplay(updatedHeader, UPDATED_WIDTH),
      ];
      let headerMsg = "";
      const headerStyles: Array<string> = [];
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
          priorityStyles,
          identifier,
          title,
          labelsFormat,
          labelsStyles,
          state,
          stateStyles,
          timeAgo,
        } = row;
        const truncTitle = title.length > titleWidth
          ? title.slice(0, titleWidth - 3) + "..."
          : padDisplay(title, titleWidth);

        console.log(
          `${padDisplayFormatted(priorityStr, 4)} ${
            padDisplay(identifier, 8)
          } ${truncTitle} ${padDisplayFormatted(labelsFormat, LABEL_WIDTH)} ${
            padDisplayFormatted(state, STATE_WIDTH)
          } %c${padDisplay(timeAgo, UPDATED_WIDTH)}%c`,
          ...priorityStyles,
          ...labelsStyles,
          ...stateStyles,
          "color: gray",
          "",
        );
      }
    } catch (error) {
      console.error("Failed to fetch issues:", error);
      Deno.exit(1);
    }
  })
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
  .action(async (_, issueId) => {
    const resolvedId = await getIssueId(issueId);
    if (!resolvedId) {
      console.error("Could not determine issue ID");
      Deno.exit(1);
    }

    const teamId = await getTeamId();
    if (!teamId) {
      console.error("Could not determine team ID");
      Deno.exit(1);
    }

    const { branchName } = await fetchIssueDetails(resolvedId, true);

    // Check if branch exists
    if (await branchExists(branchName)) {
      const answer = await Select.prompt({
        message:
          `Branch ${branchName} already exists. What would you like to do?`,
        options: [
          { name: "Switch to existing branch", value: "switch" },
          { name: "Create new branch with suffix", value: "create" },
        ],
      });

      if (answer === "switch") {
        const process = new Deno.Command("git", {
          args: ["checkout", branchName],
        });
        await process.output();
        console.log(`✓ Switched to '${branchName}'`);
      } else {
        // Find next available suffix
        let suffix = 1;
        let newBranch = `${branchName}-${suffix}`;
        while (await branchExists(newBranch)) {
          suffix++;
          newBranch = `${branchName}-${suffix}`;
        }

        const process = new Deno.Command("git", {
          args: ["checkout", "-b", newBranch],
        });
        await process.output();
        console.log(`✓ Created and switched to branch '${newBranch}'`);
      }
    } else {
      // Create and checkout the branch
      const process = new Deno.Command("git", {
        args: ["checkout", "-b", branchName],
      });
      await process.output();
      console.log(`✓ Created and switched to branch '${branchName}'`);
    }

    // Update issue state
    try {
      const state = await getStartedState(teamId);
      await updateIssueState(resolvedId, state.id);
      console.log(`✓ Issue state updated to '${state.name}'`);
    } catch (error) {
      console.error("Failed to update issue state:", error);
    }
  })
  .command("view", "View issue details (default) or open in browser/app")
  .alias("v")
  .arguments("[issueId:string]")
  .option("--web", "Open in web browser")
  .option("--app", "Open in Linear.app")
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
  });

await new Command()
  .name("linear")
  .version(denoConfig.version)
  .description("Handy linear commands from the command line")
  .action(() => {
    console.log("Use --help to see available commands");
  })
  .command("issue", issueCommand)
  .alias("i")
  .command("team", teamCommand)
  .alias("t")
  .command("completions", new CompletionsCommand())
  .command("config", "Interactively generate .linear.toml configuration")
  .action(async () => {
    const apiKey = Deno.env.get("LINEAR_API_KEY");
    if (!apiKey) {
      console.error("The LINEAR_API_KEY environment variable is required.");
      console.error("For bash/zsh, run: export LINEAR_API_KEY=your_key");
      console.error("For fish, run: set -gx LINEAR_API_KEY your_key");
      Deno.exit(1);
    }

    const query = `
      query {
        viewer {
          organization {
            urlKey
          }
        }
        teams {
          nodes {
            id
            key
            name
          }
        }
      }
    `;
    const response = await fetch("https://api.linear.app/graphql", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": apiKey,
      },
      body: JSON.stringify({ query }),
    });
    const result = await response.json();
    if (result.errors) {
      console.error(
        "Error fetching data from Linear GraphQL API:",
        result.errors,
      );
      Deno.exit(1);
    }
    const workspace = result.data.viewer.organization.urlKey;
    const teams = result.data.teams.nodes;

    interface Team {
      id: string;
      key: string;
      name: string;
    }

    teams.sort((a: Team, b: Team) => a.name.localeCompare(b.name));
    const teamChoices = teams.map((team: Team) => ({
      name: `${team.name} (${team.key})`,
      value: team.key,
    }));

    const responses = await prompt([
      {
        name: "team_id",
        message: "Select a team:",
        type: Select,
        options: teamChoices,
      },
      {
        name: "sort",
        message: "Select sort order:",
        type: Select,
        options: [
          { name: "manual", value: "manual" },
          { name: "priority", value: "priority" },
        ],
      },
    ]);
    const teamKey = responses.team_id;
    const sortChoice = responses.sort;

    // Determine file path for .linear.toml: prefer git root .config dir, then git root, then cwd.
    let filePath: string;
    try {
      const gitRootProcess = await new Deno.Command("git", {
        args: ["rev-parse", "--show-toplevel"],
      }).output();
      const gitRoot = new TextDecoder().decode(gitRootProcess.stdout).trim();
      const { join } = await import("@std/path");
      const configDir = join(gitRoot, ".config");
      try {
        await Deno.stat(configDir);
        filePath = join(configDir, "linear.toml");
      } catch {
        filePath = join(gitRoot, ".linear.toml");
      }
    } catch {
      filePath = "./.linear.toml";
    }

    const tomlContent = `workspace = "${workspace}"
team_id = "${teamKey}"
issue_sort = "${sortChoice}"
`;

    await Deno.writeTextFile(filePath, tomlContent);
    console.log("Configuration written to", filePath);
  })
  .parse(Deno.args);
