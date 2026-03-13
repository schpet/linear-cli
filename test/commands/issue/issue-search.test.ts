import { snapshotTest as cliffySnapshotTest } from "@cliffy/testing"
import { searchCommand } from "../../../src/commands/issue/issue-search.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await cliffySnapshotTest({
  name: "Issue Search Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await searchCommand.parse()
  },
})

// Test search with empty query
await cliffySnapshotTest({
  name: "Issue Search Command - Empty Query",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: [""],
  denoArgs: commonDenoArgs,
  async fn() {
    await searchCommand.parse()
  },
})

// Test deprecated command
await cliffySnapshotTest({
  name: "Issue Search Command - Deprecated",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["login bug"],
  denoArgs: commonDenoArgs,
  async fn() {
    await searchCommand.parse()
  },
})

// Test deprecated command with JSON flag
await cliffySnapshotTest({
  name: "Issue Search Command - Deprecated With JSON Flag",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["login bug", "--json"],
  denoArgs: commonDenoArgs,
  async fn() {
    await searchCommand.parse()
  },
})
