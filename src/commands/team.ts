import { Command } from "@cliffy/command";
import { encodeBase64 } from "@std/encoding/base64";
import { open } from "@opensrc/deno-open";
import { getOption } from "../config.ts";
import { getTeamId } from "../utils/linear.ts";

async function openTeamPage(options: { app?: boolean } = {}) {
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

export const teamCommand = new Command()
  .description(
    "Manage Linear teams (deprecated: use `linear issue list --app` instead)",
  )
  .action(() => {
    console.error(
      "Warning: 'linear team' is deprecated and will be removed in a future release.",
    );
    console.error("Please use 'linear issue list --app' instead.");
    return openTeamPage({ app: true });
  })
  .command("open", "Open the team page in Linear.app")
  .alias("o")
  .action(() => openTeamPage({ app: true }))
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
