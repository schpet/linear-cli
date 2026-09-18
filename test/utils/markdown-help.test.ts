import { assertEquals, assertStringIncludes } from "@std/assert"
import { cli } from "../../src/cli.ts"
import { membersCommand } from "../../src/commands/team/team-members.ts"
import { listCommand } from "../../src/commands/user/user-list.ts"

// Cliffy's Command type carries its options in its generic parameters, so a
// concrete command is not assignable to a bare `Command`. Introspecting the
// tree only needs this much of the shape.
interface CommandOption {
  flags: string[]
  description: string
}

interface IntrospectableCommand {
  getName(): string
  getDescription(): string
  getCommands(): IntrospectableCommand[]
  getOptions(hidden?: boolean): CommandOption[]
  getOption(name: string): CommandOption | undefined
}

// Declaring one of these means the command takes a body Linear renders as
// Markdown, which is exactly where the mention and collapsible rules apply.
const MARKDOWN_BODY_OPTIONS = [
  "--body-file",
  "--description-file",
  "--content-file",
  "--content",
]

// Pinned so that adding a Markdown-bodied command (or a --body-file flag to an
// existing one) forces a deliberate decision about the guidance rather than
// silently shipping a command an agent will misuse.
const EXPECTED_MARKDOWN_COMMANDS = [
  "document comment add",
  "document create",
  "document update",
  "initiative comment add",
  "initiative-update create",
  "issue comment add",
  "issue comment update",
  "issue create",
  "issue update",
  "project comment add",
  "project create",
  "project update",
  "project-update create",
]

function findMarkdownCommands(
  command: IntrospectableCommand,
  path: string[] = [],
): { path: string; command: IntrospectableCommand }[] {
  const found: { path: string; command: IntrospectableCommand }[] = []

  for (const sub of command.getCommands()) {
    const subPath = [...path, sub.getName()]
    const flags = sub.getOptions(true).flatMap((option) => option.flags)

    if (MARKDOWN_BODY_OPTIONS.some((flag) => flags.includes(flag))) {
      found.push({ path: subPath.join(" "), command: sub })
    }

    found.push(...findMarkdownCommands(sub, subPath))
  }

  return found
}

Deno.test("markdown help - every Markdown-bodied command is accounted for", () => {
  const discovered = findMarkdownCommands(cli).map((entry) => entry.path).sort()

  assertEquals(discovered, EXPECTED_MARKDOWN_COMMANDS)
})

// Asserting the substance rather than `includes(MARKDOWN_HINT)`: comparing a
// command's description against the same constant it was built from still
// passes if the shared hint is gutted to a bare "see `linear markdown`".
Deno.test("markdown help - every Markdown-bodied command teaches real mentions", () => {
  for (const { path, command } of findMarkdownCommands(cli)) {
    const description = command.getDescription()

    assertStringIncludes(
      description,
      "a plain Linear URL creates a mention",
      `${path} does not state how mentions are created`,
    )
    for (const wrongForm of ["`@name`", "`@[Name](id)`", "`[Name](url)`"]) {
      assertStringIncludes(
        description,
        wrongForm,
        `${path} does not warn that ${wrongForm} fails to mention anyone`,
      )
    }
    assertStringIncludes(
      description,
      "linear team members <TEAM> --json",
      `${path} does not say how to look a person's URL up`,
    )
    assertStringIncludes(
      description,
      "linear markdown",
      `${path} does not point at the full reference`,
    )
  }
})

function jsonOptionDescription(command: IntrospectableCommand): string {
  const option = command.getOption("json")
  if (option == null) throw new Error("expected a --json option")
  return option.description
}

// The `url` field only exists in --json output, so this is where an agent finds
// out what it is for.
Deno.test("markdown help - member listings explain the url field", () => {
  assertStringIncludes(
    jsonOptionDescription(membersCommand),
    "url mentions them when pasted into Markdown",
  )
  assertStringIncludes(
    jsonOptionDescription(listCommand),
    "url mentions them when pasted into Markdown",
  )
})

// Mentioning someone who is not on the team is usually a mistake, so the
// workspace-wide listing must not read as an equal alternative to the team one.
Deno.test("markdown help - workspace listing keeps the team-first safeguard", () => {
  const description = jsonOptionDescription(listCommand)

  assertStringIncludes(description, "searches the whole workspace")
  assertStringIncludes(description, "prefer `linear team members <TEAM>`")
  assertStringIncludes(description, "confirm before mentioning someone outside")
})
