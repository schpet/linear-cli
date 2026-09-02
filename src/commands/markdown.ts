import { Command } from "@cliffy/command"
import { LINEAR_MARKDOWN_REFERENCE } from "../utils/markdown-help.ts"

// The reference is both the description and the printed output: `--help` is
// what the skill-docs generator captures, while bare `linear markdown` prints
// it unindented so the `+++` syntax can be copied verbatim.
export const markdownCommand = new Command()
  .name("markdown")
  .description(LINEAR_MARKDOWN_REFERENCE)
  .action(() => {
    console.log(LINEAR_MARKDOWN_REFERENCE)
  })
