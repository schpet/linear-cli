/**
 * User-friendly error handling for the Linear CLI.
 *
 * Design philosophy (inspired by Rust's error handling ecosystem):
 * - User-facing messages should be clean and actionable
 * - Stack traces only shown when LINEAR_DEBUG=1
 * - Errors should explain what went wrong and suggest how to fix it
 * - GraphQL errors should be parsed and presented nicely
 */

import { ClientError } from "graphql-request"
import { gray, red, setColorEnabled } from "@std/fmt/colors"

/**
 * Check if debug mode is enabled via LINEAR_DEBUG environment variable.
 */
export function isDebugMode(): boolean {
  const debug = Deno.env.get("LINEAR_DEBUG")
  return debug === "1" || debug === "true"
}

/**
 * Base class for CLI errors with user-friendly messages.
 */
export class CliError extends Error {
  /** The clean, user-facing message */
  readonly userMessage: string
  /** Suggestion for how to fix the issue (optional) */
  readonly suggestion?: string

  constructor(
    userMessage: string,
    options?: { suggestion?: string; cause?: unknown },
  ) {
    super(userMessage)
    this.name = "CliError"
    this.userMessage = userMessage
    this.suggestion = options?.suggestion
    if (options?.cause) {
      this.cause = options.cause
    }
  }
}

/**
 * Error for when an entity (issue, project, team, etc.) is not found.
 */
export class NotFoundError extends CliError {
  readonly entityType: string
  readonly identifier: string

  constructor(
    entityType: string,
    identifier: string,
    options?: { suggestion?: string },
  ) {
    const message = `${entityType} not found: ${identifier}`
    super(message, options)
    this.name = "NotFoundError"
    this.entityType = entityType
    this.identifier = identifier
  }
}

/**
 * Error for invalid user input (arguments, flags, etc.).
 */
export class ValidationError extends CliError {
  constructor(message: string, options?: { suggestion?: string }) {
    super(message, options)
    this.name = "ValidationError"
  }
}

/**
 * Error for authentication/authorization issues.
 */
export class AuthError extends CliError {
  constructor(message: string, options?: { suggestion?: string }) {
    super(message, {
      suggestion: options?.suggestion ??
        "Run `linear auth login` to authenticate.",
      ...options,
    })
    this.name = "AuthError"
  }
}

/**
 * Extract a user-friendly message from a GraphQL ClientError.
 *
 * Tries to find:
 * 1. userPresentableMessage from Linear's API
 * 2. First error message from the response
 * 3. Falls back to the error message
 */
export function extractGraphQLMessage(error: ClientError): string {
  const extensions = error.response?.errors?.[0]?.extensions
  const userMessage = extensions?.userPresentableMessage as string | undefined

  if (userMessage) {
    return userMessage
  }

  const firstError = error.response?.errors?.[0]
  if (firstError?.message) {
    return firstError.message
  }

  return error.message
}

/**
 * Check if a GraphQL error indicates an entity was not found.
 */
export function isNotFoundError(error: ClientError): boolean {
  const message = extractGraphQLMessage(error).toLowerCase()
  return message.includes("not found") || message.includes("entity not found")
}

/**
 * Check if an error is a GraphQL ClientError.
 */
export function isClientError(error: unknown): error is ClientError {
  return error instanceof ClientError
}

/**
 * Format and display an error to the user.
 *
 * In normal mode: Shows a clean, user-friendly message
 * In debug mode (LINEAR_DEBUG=1): Also shows the full error details
 */
export function handleError(error: unknown, context?: string): never {
  setColorEnabled(Deno.stderr.isTerminal())

  if (error instanceof CliError) {
    printCliError(error, context)
  } else if (isClientError(error)) {
    printGraphQLError(error, context)
  } else if (error instanceof Error) {
    printGenericError(error, context)
  } else {
    printUnknownError(error, context)
  }

  Deno.exit(1)
}

function printCliError(error: CliError, context?: string): void {
  const prefix = context ? `${context}: ` : ""
  console.error(red(`✗ ${prefix}${error.userMessage}`))

  if (error.suggestion) {
    console.error(gray(`  ${error.suggestion}`))
  }

  if (isDebugMode() && error.cause) {
    printDebugInfo(error.cause)
  }
}

function printGraphQLError(error: ClientError, context?: string): void {
  const message = extractGraphQLMessage(error)
  const prefix = context ? `${context}: ` : ""

  // Check for common error patterns and provide helpful messages
  if (isNotFoundError(error)) {
    console.error(red(`✗ ${prefix}${message}`))
  } else {
    console.error(red(`✗ ${prefix}${message}`))
  }

  if (isDebugMode()) {
    printDebugInfo(error)
    const query = error.request?.query
    const vars = error.request?.variables
    if (query) {
      console.error(gray("\nQuery:"))
      console.error(gray(String(query).trim()))
    }
    if (vars) {
      console.error(gray("\nVariables:"))
      console.error(gray(JSON.stringify(vars, null, 2)))
    }
  }
}

function printGenericError(error: Error, context?: string): void {
  const prefix = context ? `${context}: ` : ""
  console.error(red(`✗ ${prefix}${error.message}`))

  if (isDebugMode()) {
    printDebugInfo(error)
  }
}

function printUnknownError(error: unknown, context?: string): void {
  const prefix = context ? `${context}: ` : ""
  console.error(red(`✗ ${prefix}${String(error)}`))

  if (isDebugMode()) {
    console.error(gray("\nDebug info:"))
    console.error(gray(JSON.stringify(error, null, 2)))
  }
}

function printDebugInfo(error: unknown): void {
  console.error(gray("\nStack trace (LINEAR_DEBUG=1):"))
  if (error instanceof Error && error.stack) {
    console.error(gray(error.stack))
  }
}

/**
 * Wrap an async operation with error handling.
 * Similar to Rust's .context() for adding context to errors.
 *
 * @example
 * const issue = await withContext(
 *   () => getIssue(id),
 *   "Failed to fetch issue"
 * );
 */
export async function withContext<T>(
  fn: () => Promise<T>,
  context: string,
): Promise<T> {
  try {
    return await fn()
  } catch (error) {
    if (error instanceof CliError) {
      // Re-throw with context added
      throw new CliError(`${context}: ${error.userMessage}`, {
        suggestion: error.suggestion,
        cause: error.cause ?? error,
      })
    }
    if (isClientError(error)) {
      const message = extractGraphQLMessage(error)
      throw new CliError(`${context}: ${message}`, { cause: error })
    }
    if (error instanceof Error) {
      throw new CliError(`${context}: ${error.message}`, { cause: error })
    }
    throw new CliError(`${context}: ${String(error)}`, { cause: error })
  }
}

/**
 * Create a standardized "not found" error handler for GraphQL queries.
 *
 * @example
 * const issue = await client.request(query, { id })
 *   .catch(handleNotFound("Issue", issueIdentifier));
 */
export function handleNotFound(
  entityType: string,
  identifier: string,
): (error: unknown) => never {
  return (error: unknown) => {
    if (isClientError(error) && isNotFoundError(error)) {
      throw new NotFoundError(entityType, identifier)
    }
    throw error
  }
}
