import { Command } from "@cliffy/command";
import { open } from "@opensrc/deno-open";
import { CompletionsCommand } from "@cliffy/command/completions";
import denoConfig from "./deno.json" with { type: "json" };
import { encodeBase64 } from "@std/encoding/base64";
import { renderMarkdown } from "@littletof/charmd";
import { basename } from "@std/path";

function handleCommandFailure(
  output: Deno.CommandOutput,
  context: string,
) {
  const stdout = new TextDecoder().decode(output.stdout);
  const stderr = new TextDecoder().decode(output.stderr);

  console.error(
    "%cError: Failed to %s",
    "font-weight: bold; color: red",
    context,
  );
  console.log(`Exit code: ${output.code}`);

  if (stdout) {
    console.log("\n%cStandard output:", "font-weight: bold");
    console.log(stdout);
  }

  if (stderr) {
    console.error("\n%cError output:", "font-weight: bold");
    console.error("%c" + stderr, "color: red");
  }

  console.log("\n%cTroubleshooting:", "font-weight: bold");
  console.log("- Make sure you have gh CLI installed and configured");
  console.log("- Check your GitHub authentication");
  Deno.exit(1);
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

async function getIssueId(): Promise<string | null> {
  const branch = await getCurrentBranch();
  const match = branch.match(/[a-zA-Z]{2,5}-[1-9][0-9]*/i);
  return match ? match[0].toUpperCase() : null;
}

async function getTeamId(): Promise<string | null> {
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
): Promise<{ title: string; description: string | null; url: string }> {
  const query =
    `query($id: String!) { issue(id: $id) { title, description, url } }`;
  const data = await fetchGraphQL(query, { id: issueId });
  return data.data.issue;
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

async function openIssuePage() {
  const issueId = await getIssueId();
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
    });

    const output = await process.output();
    if (!output.success) {
      await handleCommandFailure(output, "configure autolinks");
    }
    console.log(`Successfully configured autolinks for team ${teamId}`);
  });

const issueCommand = new Command()
  .description("Manage Linear issues")
  .action(openIssuePage)
  .command("open", "Open the issue in Linear.app")
  .action(openIssuePage)
  .command("print", "Print the issue details")
  .option("--no-color", "Disable colored output")
  .action(async ({ color }) => {
    const issueId = await getIssueId();
    if (!issueId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }

    const { title, description } = await fetchIssueDetails(issueId);
    const markdown = `# ${title}${description ? "\n\n" + description : ""}`;
    if (color && Deno.stderr.isTerminal()) {
      console.log(renderMarkdown(markdown));
    } else {
      console.log(markdown);
    }
  })
  .command("id", "Print the issue id in the current git branch")
  .action(async () => {
    const issueId = await getIssueId();
    if (issueId) {
      console.log(issueId);
    } else {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
  })
  .command("title", "Print the issue title")
  .action(async () => {
    const issueId = await getIssueId();
    if (!issueId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { title } = await fetchIssueDetails(issueId);
    console.log(title);
  })
  .command("url", "Print the issue URL")
  .action(async () => {
    const issueId = await getIssueId();
    if (!issueId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { url } = await fetchIssueDetails(issueId);
    console.log(url);
  })
  .command("pull-request", "Create a GitHub pull request with issue details")
  .alias("pr")
  .action(async () => {
    const issueId = await getIssueId();
    if (!issueId) {
      console.error(
        "The current branch does not contain a valid linear issue id.",
      );
      Deno.exit(1);
    }
    const { title, url } = await fetchIssueDetails(issueId);

    const process = new Deno.Command("gh", {
      args: [
        "pr",
        "create",
        "--title",
        `${issueId} ${title}`,
        "--body",
        url,
        "--web",
      ],
    });

    const output = await process.output();
    if (!output.success) {
      await handleCommandFailure(output, "create pull request");
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
