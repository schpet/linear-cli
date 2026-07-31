import { Command } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { getTeamKey, getWorkflowStates } from "../../utils/linear.ts"
import { padDisplay } from "../../utils/display.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError, ValidationError } from "../../utils/errors.ts"

export const statesCommand = new Command()
  .name("states")
  .description("List workflow states for a team")
  .arguments("[teamKey:string]")
  .option("-j, --json", "Output as JSON")
  .action(async ({ json }, teamKey?: string) => {
    const showSpinner = !json && shouldShowSpinner()
    let spinner: { start: () => void; stop: () => void } | null = null

    try {
      const resolvedTeamKey = teamKey || getTeamKey()
      if (!resolvedTeamKey) {
        throw new ValidationError(
          "Could not determine team key from directory name",
          { suggestion: "Please specify a team key as an argument." },
        )
      }

      if (showSpinner) {
        const { Spinner } = await import("@std/cli/unstable-spinner")
        spinner = new Spinner()
        spinner.start()
      }

      const states = await getWorkflowStates(resolvedTeamKey)

      spinner?.stop()

      if (json) {
        console.log(JSON.stringify({ nodes: states }, null, 2))
        return
      }

      if (states.length === 0) {
        console.log("No workflow states found for this team.")
        return
      }

      // States arrive sorted by position; keep that order (it is meaningful).
      const NAME_WIDTH = Math.max(
        unicodeWidth("NAME"),
        ...states.map((s) => unicodeWidth(s.name)),
      )
      const TYPE_WIDTH = Math.max(
        unicodeWidth("TYPE"),
        ...states.map((s) => unicodeWidth(s.type)),
      )

      console.log(
        `%c${padDisplay("NAME", NAME_WIDTH)}%c %c${
          padDisplay("TYPE", TYPE_WIDTH)
        }%c`,
        "text-decoration: underline",
        "text-decoration: none",
        "text-decoration: underline",
        "text-decoration: none",
      )

      for (const state of states) {
        console.log(
          `${padDisplay(state.name, NAME_WIDTH)} ${
            padDisplay(state.type, TYPE_WIDTH)
          }`,
        )
      }
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to fetch workflow states")
    }
  })
