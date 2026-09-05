import { Command, EnumType } from "@cliffy/command"
import { unicodeWidth } from "@std/cli"
import { padDisplay, truncateText } from "../../utils/display.ts"
import { resolveTeam } from "../../utils/linear.ts"
import { shouldShowSpinner } from "../../utils/hyperlink.ts"
import { handleError } from "../../utils/errors.ts"
import {
  fetchTemplates,
  type Template,
  TEMPLATE_TYPES,
  templateIsAvailableTo,
  templateScopeLabel,
} from "../../utils/templates.ts"

const TemplateTypeArg = new EnumType(TEMPLATE_TYPES)

/** Stable display order: type, then name, workspace templates before team ones. */
function compareTemplates(a: Template, b: Template): number {
  return a.type.localeCompare(b.type) ||
    a.name.toLowerCase().localeCompare(b.name.toLowerCase()) ||
    Number(a.team != null) - Number(b.team != null) ||
    (a.team?.key ?? "").localeCompare(b.team?.key ?? "")
}

function typeCell(template: Template): string {
  return template.hasFormFields ? `${template.type} (form)` : template.type
}

export const listCommand = new Command()
  .name("list")
  .description(
    "List templates. Without --team, every template in the workspace is shown.",
  )
  .type("template-type", TemplateTypeArg)
  .option(
    "--type <type:template-type>",
    "Only templates of this type (issue, project, or document)",
  )
  .option(
    "--team <team:string>",
    "Team key, name, or ID. Shows that team's templates plus workspace templates.",
  )
  .option("-j, --json", "Output as JSON")
  .action(async ({ type, team: teamReference, json }) => {
    const { Spinner } = await import("@std/cli/unstable-spinner")
    const showSpinner = !json && shouldShowSpinner()
    const spinner = showSpinner ? new Spinner() : null
    spinner?.start()

    try {
      const team = teamReference == null
        ? null
        : await resolveTeam(teamReference)
      const templates = (await fetchTemplates())
        .filter((template) => type == null || template.type === type)
        .filter((template) =>
          team == null || templateIsAvailableTo(template, [team.id])
        )
        .sort(compareTemplates)

      spinner?.stop()

      if (json) {
        console.log(JSON.stringify(templates, null, 2))
        return
      }

      if (templates.length === 0) {
        console.log("No templates found.")
        return
      }

      const { columns } = Deno.stdout.isTerminal()
        ? Deno.consoleSize()
        : { columns: 120 }

      const ID_WIDTH = 36
      const TYPE_WIDTH = Math.max(
        4,
        ...templates.map((t) => unicodeWidth(typeCell(t))),
      )
      const TEAM_WIDTH = Math.min(
        15,
        Math.max(
          4,
          ...templates.map((t) => unicodeWidth(templateScopeLabel(t))),
        ),
      )
      const SPACE_WIDTH = 3
      const fixed = ID_WIDTH + TYPE_WIDTH + TEAM_WIDTH + SPACE_WIDTH
      const maxNameWidth = Math.max(
        4,
        ...templates.map((t) => unicodeWidth(t.name)),
      )
      const availableWidth = Math.max(columns - 1 - fixed, 0)
      const nameWidth = Math.min(maxNameWidth, Math.max(20, availableWidth))

      const headerCells = [
        padDisplay("ID", ID_WIDTH),
        padDisplay("NAME", nameWidth),
        padDisplay("TYPE", TYPE_WIDTH),
        padDisplay("TEAM", TEAM_WIDTH),
      ]
      let headerMsg = ""
      const headerStyles: string[] = []
      headerCells.forEach((cell, index) => {
        headerMsg += `%c${cell}`
        headerStyles.push("text-decoration: underline")
        if (index < headerCells.length - 1) {
          headerMsg += "%c %c"
          headerStyles.push("text-decoration: none")
          headerStyles.push("text-decoration: underline")
        }
      })
      console.log(headerMsg, ...headerStyles)

      for (const template of templates) {
        const name = padDisplay(
          truncateText(template.name, nameWidth),
          nameWidth,
        )
        console.log(
          `${padDisplay(template.id, ID_WIDTH)} ${name} ${
            padDisplay(typeCell(template), TYPE_WIDTH)
          } ${padDisplay(templateScopeLabel(template), TEAM_WIDTH)}`,
        )
      }

      console.log(
        `\n${templates.length} ${
          templates.length === 1 ? "template" : "templates"
        } found.`,
      )
    } catch (error) {
      spinner?.stop()
      handleError(error, "Failed to list templates")
    }
  })
