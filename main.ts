import { Command } from "@cliffy/command";
import { Spinner } from "@std/cli/unstable-spinner";
import { open } from "@opensrc/deno-open";
import { CompletionsCommand } from "@cliffy/command/completions";
import denoConfig from "./deno.json" with { type: "json" };
import { encodeBase64 } from "@std/encoding/base64";
import { renderMarkdown } from "@littletof/charmd";
import { basename } from "@std/path";
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
  updatedAt: string;
}

function padDisplay(s: string, width: number): string {
  const w = unicodeWidth(s);
  return s + " ".repeat(Math.max(0, width - w));
}

function stripConsoleFormat(s: string): string {
  return s.replace(/%c/g, '');
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

async function getCurrentBranch(): Promise<string> {
  const process = new Deno.Command("git", {
    args: ["symbolic-ref", "--short", "HEAD"],
  });
  const { stdout } = await process.output();
  return new TextDecoder().decode(stdout).trim();
}

async function getRepoDir(): Promise<string> {
  const process = new Deno.Command("git", {
    args: ["rev-parse", "--show-toplevel"],
  });
  const { stdout } = await process.output();
  const fullPath = new TextDecoder().decode(stdout).trim();
  return basename(fullPath);
}

async function getIssueId(providedId?: string): Promise<string | null> {
  if (providedId) {
    return providedId.toUpperCase();
  }
  const branch = await getCurrentBranch();
  const match = branch.match(/[a-zA-Z]{2,5}-[1-9][0-9]*/i);
  return match ? match[0].toUpperCase() : null;
}

async function getTeamId(): Promise<string | null> {
  const envTeamId = Deno.env.get("LINEAR_TEAM_ID");
  if (envTeamId) {
    return envTeamId.toUpperCase();
  }
  const dir = await getRepoDir();
  const match = dir.match(/^[a-zA-Z]{2,5}/);
  return match ? match[0].toUpperCase() : null;
}

async function fetchGraphQL(query: string, variables: Record<string, unknown>) {
  const apiKey = Deno.env.get("LINEAR_API_KEY");
  if (!apiKey) {
    throw new Error("LINEAR_API_KEY environment variable is not set.");
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
): Promise<{ title: string; description: string | null; url: string }> {
  const spinner = showSpinner ? new Spinner() : null;
  spinner?.start();
  try {
    const query =
      `query($id: String!) { issue(id: $id) { title, description, url } }`;
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
    console.error("Could not determine team id from directory name.");
    Deno.exit(1);
  }

  const workspace = Deno.env.get("LINEAR_WORKSPACE");
  if (!workspace) {
    console.error("LINEAR_WORKSPACE environment variable is not set.");
    Deno.exit(1);
  }

  const filterObj = {
    "and": [{ "assignee": { "or": [{ "isMe": { "eq": true } }] } }],
  };
  const filter = encodeBase64(JSON.stringify(filterObj)).replace(/=/g, "");
  const url =
    `https://linear.app/${workspace}/team/${teamId}/active?filter=${filter}`;
  await open(url, { app: { name: "Linear" } });
}

async function openIssuePage(providedId?: string) {
  const issueId = await getIssueId(providedId);
  if (!issueId) {
    console.error(
      "The current branch does not contain a valid linear issue id.",
    );
    Deno.exit(1);
  }

  const workspace = Deno.env.get("LINEAR_WORKSPACE");
  if (!workspace) {
    console.error("LINEAR_WORKSPACE environment variable is not set.");
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

    const workspace = Deno.env.get("LINEAR_WORKSPACE");
    if (!workspace) {
      console.error("LINEAR_WORKSPACE environment variable is not set.");
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
  .action((_, issueId) => openIssuePage(issueId))
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
  .option("--sort <sort:string>", "Sort order: 'manual' or 'priority'", {
    required: true,
    value: (value: string) => {
      if (!["manual", "priority"].includes(value)) {
        throw new Error("Sort must be either 'manual' or 'priority'");
      }
      return value;
    },
  })
  .option(
    "--state <state:string>",
    "Issue state: 'triage', 'backlog', 'unstarted', 'started', 'completed', or 'canceled'",
    {
      default: "unstarted",
      value: (value: string) => {
        const validStates = [
          "triage",
          "backlog",
          "unstarted",
          "started",
          "completed",
          "canceled",
        ];
        if (!validStates.includes(value)) {
          throw new Error(`State must be one of: ${validStates.join(", ")}`);
        }
        return value;
      },
    },
  )
  .action(async ({ sort, state }) => {
    const teamId = await getTeamId();
    if (!teamId) {
      console.error("Could not determine team id from directory name.");
      Deno.exit(1);
    }

    // LINEAR PRESETS FOR SORTING:
    //
    // {
    //   priority_desc: {
    //     "viewOrdering": "priority",
    //     "viewOrderingDirection": "desc",
    //   },
    //   priority_asc: {
    //     "viewOrdering": "priority",
    //     "viewOrderingDirection": "asc",
    //   },
    //   manual: { "viewOrdering": "manual", "viewOrderingDirection": "asc" },
    // };

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

      const tableData = issues.map((issue: Issue) => {
        const labelsFormat = issue.labels.nodes.map((l: Label) => `%c${l.name}%c`).join(", ");
        const labelsStyles = issue.labels.nodes.flatMap((l: Label) => [`color: ${l.color}`, ""]);
        const updatedAt = new Date(issue.updatedAt);
        const timeAgo = getTimeAgo(updatedAt);

        let priorityStr = "";
        let priorityStyles: string[] = [];
        if (issue.priority === 0) {
          priorityStr = "";
        } else if (issue.priority === 1 || issue.priority === 2) {
          priorityStr = "%c█%c█%c█";
          priorityStyles = ["", "", ""];
        } else if (issue.priority === 3) {
          priorityStr = "%c█%c█%c█";
          priorityStyles = ["", "", "color: silver"];
        } else if (issue.priority === 4) {
          priorityStr = "%c█%c█%c█";
          priorityStyles = ["", "color: silver", "color: silver"];
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
          timeAgo,
        };
      });

      // Print header with dynamic widths using defined constants
      const { columns } = Deno.consoleSize();
      const PRIORITY_WIDTH = 4;
      const ID_WIDTH = 8;
      const LABEL_WIDTH = 25; // fixed width for labels
      const SPACE_WIDTH = 4;
      const updatedHeader = "UPDATED";
      const UPDATED_WIDTH = Math.max(unicodeWidth(updatedHeader), ...tableData.map((row) => unicodeWidth(row.timeAgo)));
      const fixed = PRIORITY_WIDTH + ID_WIDTH + UPDATED_WIDTH + SPACE_WIDTH + LABEL_WIDTH; // sum of fixed columns
      const PADDING = 1
      const titleWidth = Math.max(columns - PADDING - fixed, 0); // use remaining space for title
      const headerCells = [
        padDisplay("P", PRIORITY_WIDTH),
        padDisplay("ID", ID_WIDTH),
        padDisplay("TITLE", titleWidth),
        padDisplay("LABELS", LABEL_WIDTH),
        padDisplay(updatedHeader, UPDATED_WIDTH),
      ];
      const headerStr = headerCells.join(" ");
      console.log(
        "%c" + headerCells.join(" %c"),
        ...headerCells.map(() => "text-decoration: underline")
      );

      // Print each issue
      for (const row of tableData) {
        const { priorityStr, priorityStyles, identifier, title, labelsFormat, labelsStyles, timeAgo } = row;
        const truncTitle = title.length > titleWidth
          ? title.slice(0, titleWidth - 3) + "..."
          : padDisplay(title, titleWidth);

        console.log(
          `${padDisplayFormatted(priorityStr, 4)} ${padDisplay(identifier, 8)} ${truncTitle} ${padDisplayFormatted(labelsFormat, LABEL_WIDTH)} ${padDisplay(timeAgo, UPDATED_WIDTH)}`,
          ...priorityStyles,
          ...labelsStyles
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
  .parse(Deno.args);
