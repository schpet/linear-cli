import { CliError, NotFoundError, ValidationError } from "../../utils/errors.ts"

// Linear's API rejects project descriptions longer than this. The web UI
// accepts longer descriptions through a different endpoint, but the
// projectCreate / projectUpdate mutations exposed here are bound to this cap.
export const PROJECT_DESCRIPTION_MAX_LENGTH = 255

export async function resolveProjectDescription(
  description: string | undefined,
  descriptionFile: string | undefined,
): Promise<string | undefined> {
  if (description != null && descriptionFile != null) {
    throw new ValidationError(
      "Cannot use --description and --description-file together",
      {
        suggestion: "Pass only one of --description or --description-file.",
      },
    )
  }

  let value: string | undefined
  if (description != null) {
    value = description
  } else if (descriptionFile != null) {
    try {
      value = await Deno.readTextFile(descriptionFile)
    } catch (error) {
      if (error instanceof Deno.errors.NotFound) {
        throw new NotFoundError("File", descriptionFile)
      }
      throw new CliError(
        `Failed to read description file: ${
          error instanceof Error ? error.message : String(error)
        }`,
        { cause: error },
      )
    }
  }

  if (value != null && value.length > PROJECT_DESCRIPTION_MAX_LENGTH) {
    throw new ValidationError(
      `Project description is ${value.length} characters, exceeds the ${PROJECT_DESCRIPTION_MAX_LENGTH}-character limit enforced by Linear's API`,
      {
        suggestion:
          `Shorten the description to ${PROJECT_DESCRIPTION_MAX_LENGTH} characters or fewer, or move the long content into an attached document via \`linear document create --project <slug>\`.`,
      },
    )
  }

  return value
}
