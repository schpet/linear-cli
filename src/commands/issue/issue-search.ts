import { Command } from "@cliffy/command"
import { CliError, handleError, ValidationError } from "../../utils/errors.ts"

export const searchCommand = new Command()
  .name("search")
  .description("Deprecated: use `issue list` or `api` for issue filtering")
  .arguments("<query:string>")
  .option("-j, --json", "Output as JSON")
  .option("-n, --limit <limit:number>", "Maximum number of results", {
    default: 20,
  })
  .option("-a, --include-archived", "Include archived issues in results")
  .action((_options, query) => {
    try {
      if (!query || query.trim().length === 0) {
        throw new ValidationError("Search query cannot be empty")
      }

      throw new CliError(
        "`issue search` is deprecated because Linear's dedicated search endpoint is deprecated.",
        {
          suggestion:
            "Use `linear issue list | grep <keyword>`, `linear issue list --json | jq ...`, or `linear api` for custom GraphQL queries.",
        },
      )
    } catch (error) {
      handleError(error, "Failed to search issues")
    }
  })
