import { Command } from "@cliffy/command";
import { unicodeWidth } from "@std/cli";
import { open } from "@opensrc/deno-open";
import { gql } from "../../__codegen__/gql.ts";
import { getGraphQLClient } from "../../utils/graphql.ts";
import { getTimeAgo, padDisplay } from "../../utils/display.ts";
import { getOption } from "../../config.ts";

const GetTeams = gql(`
  query GetTeams($filter: TeamFilter) {
    teams(filter: $filter) {
      nodes {
        id
        name
        key
        description
        icon
        color
        cyclesEnabled
        createdAt
        updatedAt
        archivedAt
        organization {
          id
          name
        }
      }
    }
  }
`);

export const listCommand = new Command()
  .name("list")
  .description("List teams")
  .option("-w, --web", "Open in web browser")
  .option("-a, --app", "Open in Linear.app")
  .action(async ({ web, app }) => {
    if (web || app) {
      const workspace = getOption("workspace");
      if (!workspace) {
        console.error(
          "workspace is not set via command line, configuration file, or environment.",
        );
        Deno.exit(1);
      }

      const url = `https://linear.app/${workspace}/settings/teams`;
      const destination = app ? "Linear.app" : "web browser";
      console.log(`Opening ${url} in ${destination}`);
      await open(url, app ? { app: { name: "Linear" } } : undefined);
      return;
    }
    const { Spinner } = await import("@std/cli/unstable-spinner");
    const showSpinner = Deno.stdout.isTerminal();
    const spinner = showSpinner ? new Spinner() : null;
    spinner?.start();

    try {
      const client = getGraphQLClient();
      const result = await client.request(GetTeams, { filter: undefined });
      spinner?.stop();

      let teams = result.teams?.nodes || [];

      // Filter out archived teams
      teams = teams.filter((team) => !team.archivedAt);

      if (teams.length === 0) {
        console.log("No teams found.");
        return;
      }

      // Sort teams alphabetically by name
      teams = teams.sort((a, b) => a.name.localeCompare(b.name));

      // Define column widths based on actual data
      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 };
      const KEY_WIDTH = Math.max(
        3, // minimum width for "KEY" header
        ...teams.map((team) => team.key.length),
      );
      const CYCLES_WIDTH = Math.max(
        6, // minimum width for "CYCLES" header
        3, // "Yes" or "No"
      );
      const UPDATED_WIDTH = Math.max(
        7, // minimum width for "UPDATED" header
        ...teams.map((team) => getTimeAgo(new Date(team.updatedAt)).length),
      );

      const SPACE_WIDTH = 4;
      const fixed = KEY_WIDTH + CYCLES_WIDTH + UPDATED_WIDTH + SPACE_WIDTH;
      const PADDING = 1;
      const maxNameWidth = Math.max(
        ...teams.map((team) => unicodeWidth(team.name)),
      );
      const availableWidth = Math.max(columns - PADDING - fixed, 0);
      const nameWidth = Math.min(maxNameWidth, availableWidth);

      // Print header
      const headerCells = [
        padDisplay("KEY", KEY_WIDTH),
        padDisplay("NAME", nameWidth),
        padDisplay("CYCLES", CYCLES_WIDTH),
        padDisplay("UPDATED", UPDATED_WIDTH),
      ];

      let headerMsg = "";
      const headerStyles: string[] = [];
      headerCells.forEach((cell, index) => {
        headerMsg += `%c${cell}`;
        headerStyles.push("text-decoration: underline");
        if (index < headerCells.length - 1) {
          headerMsg += "%c %c";
          headerStyles.push("text-decoration: none");
          headerStyles.push("text-decoration: underline");
        }
      });
      console.log(headerMsg, ...headerStyles);

      // Print each team
      for (const team of teams) {
        const cycles = team.cyclesEnabled ? "Yes" : "No";
        const updated = getTimeAgo(new Date(team.updatedAt));

        const truncName = team.name.length > nameWidth
          ? team.name.slice(0, nameWidth - 3) + "..."
          : padDisplay(team.name, nameWidth);

        console.log(
          `%c${padDisplay(team.key, KEY_WIDTH)}%c ${truncName} ${
            padDisplay(cycles, CYCLES_WIDTH)
          } %c${padDisplay(updated, UPDATED_WIDTH)}%c`,
          `color: ${team.color || "#ffffff"}`,
          "",
          "color: gray",
          "",
        );
      }
    } catch (error) {
      spinner?.stop();
      console.error("Failed to fetch teams:", error);
      Deno.exit(1);
    }
  });
