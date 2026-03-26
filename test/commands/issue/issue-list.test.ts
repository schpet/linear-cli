import { snapshotTest } from "@cliffy/testing"
import { listCommand } from "../../../src/commands/issue/issue-list.ts"
import { commonDenoArgs } from "../../utils/test-helpers.ts"

// Test help output
await snapshotTest({
  name: "Issue List Command - Help Text",
  meta: import.meta,
  colors: false,
  args: ["--help"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// Test that comma-separated states with an invalid value produces a validation error
await snapshotTest({
  name: "Issue List Command - Invalid Comma-Separated State",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--state", "triage,invalid", "--sort", "priority", "--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})

// Test that a single invalid state produces a validation error
await snapshotTest({
  name: "Issue List Command - Invalid State",
  meta: import.meta,
  colors: false,
  canFail: true,
  args: ["--state", "bogus", "--sort", "priority", "--team", "ENG"],
  denoArgs: commonDenoArgs,
  async fn() {
    await listCommand.parse()
  },
})
