import { Command } from "@cliffy/command"
import denoConfig from "../../deno.json" with { type: "json" }
import { getGraphQLEndpoint, getResolvedApiKey } from "../utils/graphql.ts"
import { handleError, ValidationError } from "../utils/errors.ts"

export const apiCommand = new Command()
  .name("api")
  .description("Make a raw GraphQL API request")
  .arguments("[query:string]")
  .option(
    "-f, --field <field:string>",
    "String variable in key=value format",
    { collect: true },
  )
  .option(
    "-F, --typed-field <field:string>",
    "Typed variable in key=value format (coerces booleans, numbers, null)",
    { collect: true },
  )
  .action(async (options, query?: string) => {
    try {
      const resolvedQuery = await resolveQuery(query)
      const variables = buildVariables(options.field, options.typedField)

      const apiKey = getResolvedApiKey()
      if (!apiKey) {
        throw new ValidationError(
          "No API key configured",
          {
            suggestion:
              "Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.",
          },
        )
      }

      const body: Record<string, unknown> = { query: resolvedQuery }
      if (Object.keys(variables).length > 0) {
        body.variables = variables
      }

      const response = await fetch(getGraphQLEndpoint(), {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: apiKey,
          "User-Agent": `schpet-linear-cli/${denoConfig.version}`,
        },
        body: JSON.stringify(body),
      })

      const text = await response.text()

      if (response.status >= 400) {
        console.error(text)
        Deno.exit(1)
      }

      if (Deno.stdout.isTerminal()) {
        try {
          const parsed = JSON.parse(text)
          console.log(JSON.stringify(parsed, null, 2))
        } catch {
          console.log(text)
        }
      } else {
        Deno.stdout.writeSync(new TextEncoder().encode(text))
      }
    } catch (error) {
      handleError(error, "API request failed")
    }
  })

async function resolveQuery(positionalArg?: string): Promise<string> {
  if (positionalArg && positionalArg !== "-") {
    return positionalArg
  }

  const explicit = positionalArg === "-"

  if (explicit || !Deno.stdin.isTerminal()) {
    const content = explicit
      ? await readAllStdin()
      : await readStdinWithTimeout()
    if (content) {
      return content
    }
  }

  throw new ValidationError("No query provided", {
    suggestion:
      "Provide a query as an argument: linear api '{ viewer { id } }'\n  Or pipe from stdin: echo '{ viewer { id } }' | linear api",
  })
}

async function readAllStdin(): Promise<string | undefined> {
  const chunks: Uint8Array[] = []
  for await (const chunk of Deno.stdin.readable) {
    chunks.push(chunk)
  }
  const text = new TextDecoder().decode(concatChunks(chunks)).trim()
  return text.length > 0 ? text : undefined
}

async function readStdinWithTimeout(): Promise<string | undefined> {
  try {
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error("stdin timeout")), 100)
    })
    const result = await Promise.race([readAllStdin(), timeoutPromise])
    return result
  } catch {
    return undefined
  }
}

function concatChunks(chunks: Uint8Array[]): Uint8Array {
  const totalLength = chunks.reduce((sum, c) => sum + c.length, 0)
  const combined = new Uint8Array(totalLength)
  let offset = 0
  for (const chunk of chunks) {
    combined.set(chunk, offset)
    offset += chunk.length
  }
  return combined
}

function buildVariables(
  fields?: string[],
  typedFields?: string[],
): Record<string, unknown> {
  const variables: Record<string, unknown> = {}

  if (fields) {
    for (const entry of fields) {
      const [key, value] = parseFieldEntry(entry)
      variables[key] = value
    }
  }

  if (typedFields) {
    for (const entry of typedFields) {
      const [key, rawValue] = parseFieldEntry(entry)
      variables[key] = coerceValue(rawValue)
    }
  }

  return variables
}

function parseFieldEntry(entry: string): [string, string] {
  const eqIndex = entry.indexOf("=")
  if (eqIndex === -1) {
    throw new ValidationError(
      `Invalid variable format: ${entry}`,
      {
        suggestion: "Variables must be in key=value format, e.g. -f teamId=abc",
      },
    )
  }
  return [entry.slice(0, eqIndex), entry.slice(eqIndex + 1)]
}

function coerceValue(value: string): unknown {
  if (value === "true") return true
  if (value === "false") return false
  if (value === "null") return null

  const num = Number(value)
  if (value !== "" && !isNaN(num)) return num

  return value
}
