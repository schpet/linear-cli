import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { idCommand } from "../../../src/commands/team/team-id.ts"

// Common Deno args for permissions
const denoArgs = ["--allow-all", "--quiet"]

// Test help output
await cliffySnapshotTest({
  name: "Team Id Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs,
  async fn() {
    await idCommand.parse()
  },
})

// Regression test for #245: with no team configured, the suggestion must point
// at the real `linear config` command, not the non-existent `linear configure`.
await cliffySnapshotTest({
  name: "Team Id Command - No Team Configured",
  meta: import.meta,
  colors: false,
  args: [],
  denoArgs,
  canFail: true,
  async fn() {
    // An empty team id is falsy, so getTeamKey() resolves to undefined even
    // though the repo's .linear.toml sets one — this exercises the error path.
    Deno.env.set("LINEAR_TEAM_ID", "")
    try {
      await idCommand.parse()
    } finally {
      Deno.env.delete("LINEAR_TEAM_ID")
    }
  },
})
