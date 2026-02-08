import {
  type ArgumentValue,
  Command,
  Type,
  ValidationError,
} from "@cliffy/command"
import denoConfig from "../../deno.json" with { type: "json" }
import { getGraphQLEndpoint, getResolvedApiKey } from "../utils/graphql.ts"
import {
  CliError,
  handleError,
  ValidationError as AppValidationError,
} from "../utils/errors.ts"

class VariableType extends Type<[string, string]> {
  parse({ value }: ArgumentValue): [string, string] {
    const [key, ...rest] = value.split("=")
    if (rest.length === 0) {
      throw new ValidationError(
        `Invalid variable format: ${value}. Variables must be in key=value format, e.g. --variable teamId=abc`,
      )
    }
    return [key, rest.join("=")]
  }
}

export const apiCommand = new Command()
  .name("api")
  .description("Make a raw GraphQL API request")
  .type("variable", new VariableType())
  .arguments("[query:string]")
  .option(
    "--variable <variable:variable>",
    "Variable in key=value format (coerces booleans, numbers, null; @file reads from path)",
    { collect: true },
  )
  .option(
    "--variables-json <json:string>",
    "JSON object of variables (merged with --variable, which takes precedence)",
  )
  .option("--paginate", "Automatically fetch all pages using cursor pagination")
  .option(
    "--silent",
    "Suppress response output (exit code still reflects errors)",
  )
  .action(async (options, query?: string) => {
    try {
      const resolvedQuery = await resolveQuery(query)
      const variables = await buildVariables(
        options.variable,
        options.variablesJson,
      )

      const apiKey = getResolvedApiKey()
      if (!apiKey) {
        throw new AppValidationError(
          "No API key configured",
          {
            suggestion:
              "Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.",
          },
        )
      }

      const headers = {
        "Content-Type": "application/json",
        Authorization: apiKey,
        "User-Agent": `schpet-linear-cli/${denoConfig.version}`,
      }

      if (options.paginate) {
        await executePaginated(
          resolvedQuery,
          variables,
          headers,
          options.silent ?? false,
        )
      } else {
        await executeSingle(
          resolvedQuery,
          variables,
          headers,
          options.silent ?? false,
        )
      }
    } catch (error) {
      handleError(error, "API request failed")
    }
  })

async function executeSingle(
  query: string,
  variables: Record<string, unknown>,
  headers: Record<string, string>,
  silent: boolean,
): Promise<void> {
  const body: Record<string, unknown> = { query }
  if (Object.keys(variables).length > 0) {
    body.variables = variables
  }

  const response = await fetch(getGraphQLEndpoint(), {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  })

  const text = await response.text()

  if (response.status >= 400) {
    if (!silent) {
      console.error(text)
    }
    Deno.exit(1)
  }

  let hasGraphQLErrors = false
  try {
    const parsed = JSON.parse(text)
    hasGraphQLErrors = Array.isArray(parsed.errors) && parsed.errors.length > 0
    if (!silent) {
      outputJSON(parsed, text)
    }
  } catch {
    if (!silent) {
      console.log(text)
    }
  }

  if (hasGraphQLErrors) {
    Deno.exit(1)
  }
}

async function executePaginated(
  query: string,
  variables: Record<string, unknown>,
  headers: Record<string, string>,
  silent: boolean,
): Promise<void> {
  const allNodes: unknown[] = []
  let cursor: string | undefined

  for (;;) {
    const vars = { ...variables, after: cursor ?? null }

    const body: Record<string, unknown> = { query }
    if (Object.keys(vars).length > 0) {
      body.variables = vars
    }

    const response = await fetch(getGraphQLEndpoint(), {
      method: "POST",
      headers,
      body: JSON.stringify(body),
    })

    const text = await response.text()

    if (response.status >= 400) {
      if (!silent) {
        console.error(text)
      }
      Deno.exit(1)
    }

    let parsed: Record<string, unknown>
    try {
      parsed = JSON.parse(text)
    } catch {
      if (!silent) {
        console.log(text)
      }
      Deno.exit(1)
    }

    if (Array.isArray(parsed.errors) && parsed.errors.length > 0) {
      if (!silent) {
        outputJSON(parsed, text)
      }
      Deno.exit(1)
    }

    const pageResult = extractPageInfo(parsed)

    if (!pageResult) {
      if (!silent) {
        outputJSON(parsed, text)
      }
      return
    }

    allNodes.push(...pageResult.nodes)

    if (!pageResult.hasNextPage || !pageResult.endCursor) {
      break
    }

    cursor = pageResult.endCursor
  }

  if (!silent) {
    outputJSON(allNodes, JSON.stringify(allNodes))
  }
}

interface PageResult {
  nodes: unknown[]
  hasNextPage: boolean
  endCursor: string | null
  connectionPath: string[]
}

function extractPageInfo(
  data: Record<string, unknown>,
): PageResult | null {
  return findPageInfo(data, [])
}

function findPageInfo(
  obj: unknown,
  path: string[],
): PageResult | null {
  if (obj == null || typeof obj !== "object") return null

  const record = obj as Record<string, unknown>

  if (
    "pageInfo" in record &&
    "nodes" in record &&
    record.pageInfo != null &&
    typeof record.pageInfo === "object"
  ) {
    const pageInfo = record.pageInfo as Record<string, unknown>
    return {
      nodes: Array.isArray(record.nodes) ? record.nodes : [],
      hasNextPage: Boolean(pageInfo.hasNextPage),
      endCursor: (pageInfo.endCursor as string) ?? null,
      connectionPath: path,
    }
  }

  for (const [key, value] of Object.entries(record)) {
    const result = findPageInfo(value, [...path, key])
    if (result) return result
  }

  return null
}

function outputJSON(parsed: unknown, rawText: string): void {
  if (Deno.stdout.isTerminal()) {
    try {
      console.log(JSON.stringify(parsed, null, 2))
    } catch {
      console.log(rawText)
    }
  } else {
    Deno.stdout.writeSync(new TextEncoder().encode(
      typeof parsed === "string" ? rawText : JSON.stringify(parsed),
    ))
  }
}

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

  throw new AppValidationError("No query provided", {
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

async function buildVariables(
  variableEntries?: [string, string][],
  variablesJson?: string,
): Promise<Record<string, unknown>> {
  const variables: Record<string, unknown> = {}

  if (variablesJson) {
    let parsed: unknown
    try {
      parsed = JSON.parse(variablesJson)
    } catch {
      throw new AppValidationError(
        `Invalid JSON for --variables-json: ${variablesJson}`,
        {
          suggestion:
            'Provide a valid JSON object, e.g. --variables-json \'{"key": "value"}\'',
        },
      )
    }
    if (parsed == null || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new AppValidationError(
        `--variables-json must be a JSON object, got ${
          Array.isArray(parsed) ? "array" : typeof parsed
        }`,
        {
          suggestion:
            'Provide a JSON object, e.g. --variables-json \'{"key": "value"}\'',
        },
      )
    }
    Object.assign(variables, parsed)
  }

  if (variableEntries) {
    for (const [key, rawValue] of variableEntries) {
      variables[key] = await resolveTypedValue(rawValue)
    }
  }

  return variables
}

async function resolveTypedValue(value: string): Promise<unknown> {
  if (value === "@-") {
    const content = await readAllStdin()
    if (content == null) {
      throw new AppValidationError("No data on stdin for @- value")
    }
    return parseJSONOrString(content)
  }

  if (value.startsWith("@")) {
    const filePath = value.slice(1)
    try {
      const content = await Deno.readTextFile(filePath)
      return parseJSONOrString(content.trim())
    } catch (error) {
      if (error instanceof Deno.errors.NotFound) {
        throw new AppValidationError(`File not found: ${filePath}`)
      }
      throw new CliError(
        `Failed to read file: ${filePath}`,
        { cause: error },
      )
    }
  }

  return coerceValue(value)
}

function parseJSONOrString(content: string): unknown {
  try {
    return JSON.parse(content)
  } catch {
    return content
  }
}

function coerceValue(value: string): unknown {
  if (value === "true") return true
  if (value === "false") return false
  if (value === "null") return null

  const num = Number(value)
  if (value !== "" && !isNaN(num) && String(num) === value) return num

  return value
}
